//! The built-in benchmark on the PSP (`jane_present::bench` holds the scenes and the measures):
//! the title's Benchmark row, or a `script.txt` holding the word `bench`, runs the six scenes
//! back to back on a fresh game that is never saved, then writes `bench.txt` to the save folder
//! and shows the results over the title (O closes them). A scripted run ends with the bench.
//!
//! Each scene is the script machinery's own (`crate::Script`, parsed from the scene's words plus
//! `vclock`, so the sim steps on the display's clock as a player's game does); this module adds
//! the bench's words (`at:`, `route:`, `loop:`, `spawn:`, `every:`, `tough`), the measures and the
//! watchdog. The game's loop calls in at a few marked hooks only (`main.rs`, "the bench").
//!
//! The watchdog is a thread above the game's: when no frame begins for
//! [`jane_present::bench::HANG_US`] it writes `bench.txt` (the scenes done and the hang) and
//! `crash.txt` (the scene and the last stage events, `diag::hang`) from static buffers, allocating
//! nothing; if the game comes back the bench goes on and the scene counts a hang, and after ten
//! times as long stuck it ends the program so the owner is not left at a frozen screen.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use jane_present::bench::{self as b, Plan, Sample, Stats, Steer};
use jane_present::pad_psp::{psp as button, PspPad};
use jane_present::Present;
use jane_sim::{Command, Seat, Sim};
use psp::sys;

use crate::shell::{Ask, Shell};
use crate::Line;
// `say!` writes into a `Line`.
use core::fmt::Write as _;

/// A bench running (or its results showing).
pub struct Bench {
    /// Started by a script's `bench`: the program ends with it.
    scripted: bool,
    /// The script's other words, put after each scene's.
    extra: String,
    k: usize,
    plan: Plan,
    steer: Steer,
    stats: Stats,
    text: String,
    path: String,
    last_us: u32,
    frames: u32,
    paint_fails0: u32,
    hangs0: u32,
    /// The results are up over the title.
    showing: bool,
}

impl Bench {
    /// A bench for a script that says `bench`.
    pub fn from_script(text: Option<&str>, dir: &str, shell: &Shell) -> Option<Bench> {
        let t = text?;
        t.split_whitespace().any(|w| w == "bench").then(|| {
            let extra: Vec<&str> = t.split_whitespace().filter(|&w| w != "bench").collect();
            Bench::new(true, &extra.join(" "), dir, shell)
        })
    }

    fn new(scripted: bool, extra: &str, dir: &str, shell: &Shell) -> Bench {
        let mut settings = String::new();
        for l in shell.settings().write().lines() {
            settings.push_str(l);
            settings.push(' ');
        }
        let mut text = b::header("psp", env!("JANE_COMMIT"), &format!("{settings}vclock {extra}"));
        text.push('\n');
        let path = format!("{dir}bench.txt");
        watch_set(&text, b::SCENES[0].name, &path);
        ARMED.store(true, Ordering::Release);
        say!("GAME bench start scripted={scripted} {}", text.trim_end());
        let mut bench = Bench {
            scripted,
            extra: String::from(extra),
            k: 0,
            plan: Plan::default(),
            steer: Steer::default(),
            stats: Stats::default(),
            text,
            path,
            last_us: crate::now_us(),
            frames: 0,
            paint_fails0: crate::diag::paint_fails(),
            hangs0: HANGS.load(Ordering::Relaxed),
            showing: false,
        };
        bench.begin(0);
        bench
    }

    fn begin(&mut self, k: usize) {
        self.k = k;
        self.plan = Plan::parse(b::SCENES[k].words);
        self.steer = Steer::default();
        self.stats = Stats::default();
        self.paint_fails0 = crate::diag::paint_fails();
        self.hangs0 = HANGS.load(Ordering::Relaxed);
        watch_set(&self.text, b::SCENES[k].name, &self.path);
        say!("GAME bench scene {} {}", k + 1, b::SCENES[k].name);
    }

    /// The scene's script: its words, the display's clock, New Game's seed, the script's own.
    pub fn script(&self) -> crate::Script {
        let words = format!("{} vclock seed:{} {}", b::SCENES[self.k].words, b::SEED, self.extra);
        let mut s = crate::Script::parse(&words);
        // The bench's stick is its routes' and `stick:`s', never the old scripted town walk.
        s.walk = false;
        s
    }

    /// A sim tick about to step (the step loop, before `Shell::step`): the scene's travel and its
    /// dev commands (health, spawns) at `tick` of the scene.
    pub fn tick(&mut self, sim: &mut Sim, tick: u32, pending: &mut Vec<Command>) {
        if self.showing {
            return;
        }
        if let Some(req) = self.plan.travel_at(tick).and_then(|t| Plan::travel_request(sim, t)) {
            sim.state_mut().players[0].travel = Some(req);
        }
        pending.extend(self.plan.commands_at(tick));
        let her = sim.view(Seat(0)).map(|v| v.body().pos.cell());
        pending.extend(her.and_then(|h| self.steer.goto(&self.plan, tick, h)));
    }

    /// A step taken: its sim time, and the slot never written (a rest in a scene is not saved).
    pub fn stepped(&mut self, sim_us: u32, shell: &mut Shell) {
        self.stats.tick(sim_us);
        shell.rested = false;
    }

    /// A frame drawn and shown: its measures.
    pub fn frame(&mut self, in_play: bool, missed: u32, vblanks: u32, sheds: u32, present: Option<&Present>) {
        let now = crate::now_us();
        let us = now.wrapping_sub(self.last_us);
        self.last_us = now;
        if self.showing || !in_play {
            return;
        }
        self.frames += 1;
        let mem = (self.frames % 16 == 1).then(|| {
            let (largest, free) = crate::free_mem();
            (free, largest)
        });
        let (swatch, casters, sprites) = present.map_or((false, 0, 0), |p| {
            let f = p.frame();
            (p.chunks_waiting() > 0, f.casters.len() as u32, f.sprites.len() as u32)
        });
        self.stats.frame(&Sample {
            us,
            settling: SCENE_TICK.load(Ordering::Relaxed) <= self.plan.settle,
            missed,
            vblanks,
            swatch,
            sheds,
            mem,
            casters,
            sprites,
        });
    }

    /// The scene ended: its line kept and written, and the next scene's script; `None` when the
    /// bench is done (`bench.txt` written; a menu's run shows its results over the title).
    fn next(&mut self, sim: Option<&Sim>, shell: &mut Shell) -> Option<crate::Script> {
        let st = &mut self.stats;
        st.paint_fails = crate::diag::paint_fails().wrapping_sub(self.paint_fails0);
        st.hangs = HANGS.load(Ordering::Relaxed).wrapping_sub(self.hangs0);
        st.at = sim
            .and_then(|s| s.view(Seat(0)))
            .and_then(|v| v.unit(v.me().unit).map(|u| (v.zone(), u.pos.cell())))
            .map_or_else(|| String::from("-"), |(z, (x, y))| format!("{}:{x},{y}", z.name()));
        let line = st.line(b::SCENES[self.k].name);
        say!("GAME bench {line}");
        self.text.push_str(&line);
        self.text.push('\n');
        // The watchdog's own line went into the file it wrote; this one keeps the hang in it.
        if st.hangs > 0 {
            self.text.push_str(&format!("hang scene {} count {} (see crash.txt)\n", b::SCENES[self.k].name, st.hangs));
        }
        let r = crate::write_file(&self.path, self.text.as_bytes());
        if self.k + 1 < b::SCENES.len() {
            self.begin(self.k + 1);
            return Some(self.script());
        }
        ARMED.store(false, Ordering::Release);
        say!("GAME bench done {} {r:?}", self.path);
        let mut lines = Vec::new();
        lines.push(String::from(if r.is_ok() { "Benchmark: bench.txt saved" } else { "Benchmark: bench.txt NOT saved" }));
        for l in self.text.lines().filter(|l| l.starts_with("scene ")) {
            let f = b::fields(l);
            let get = |k: &str| f.iter().find(|p| p.0 == k).map_or("-", |p| p.1);
            lines.push(format!(
                "{:<10} {:>5} fps  low {:>5}  worst {:>4}ms  {}",
                get("scene"),
                get("fps"),
                get("low1"),
                get("worst_ms"),
                get("verdict")
            ));
        }
        lines.push(String::from("O to close"));
        shell.overlay = lines;
        self.showing = true;
        None
    }
}

/// The hook at a scripted scene's end (`done`, in the game's loop): the next scene set going and
/// the clocks reset, so the run is not done; or, the bench over, done for a scripted run, and for
/// a menu's the title with the results. Returns whether the loop is still to end.
#[allow(clippy::too_many_arguments)]
pub fn scene_end(
    bench: &mut Option<Bench>,
    script: &mut Option<crate::Script>,
    sim: Option<&Sim>,
    shell: &mut Shell,
    play_ticks: &mut u32,
    travel_sent: &mut bool,
) -> bool {
    let Some(bn) = bench.as_mut().filter(|bn| !bn.showing) else { return true };
    match bn.next(sim, shell) {
        Some(s) => {
            *script = Some(s);
            (*play_ticks, *travel_sent) = (0, false);
            SCENE_TICK.store(0, Ordering::Relaxed);
            false
        }
        None if bn.scripted => true,
        None => {
            *script = None;
            shell.asks.push(Ask::ToTitle);
            false
        }
    }
}

/// The Benchmark row picked: its ask turned into New Game on the bench's seed, and the bench
/// made. Returns the bench, whose script the loop takes.
pub fn start_asked(shell: &mut Shell, dir: &str) -> Option<Bench> {
    let at = shell.asks.iter().position(|a| matches!(a, Ask::Bench))?;
    shell.asks[at] = Ask::NewGame;
    shell.overlay.clear();
    Some(Bench::new(false, "", dir, shell))
}

/// The pad's hook (after the script's own presses and stick): the bench's `every:` presses in
/// play; with the results up, O closes them (and the bench).
pub fn pad(bench: &mut Option<Bench>, p: &mut PspPad, in_play: bool, tick: u32, shell: &mut Shell) {
    let Some(bn) = bench.as_mut() else { return };
    if bn.showing {
        if p.buttons & button::CIRCLE != 0 {
            shell.overlay.clear();
            p.buttons &= !button::CIRCLE;
            *bench = None;
        }
        return;
    }
    if !in_play {
        return;
    }
    SCENE_TICK.store(tick, Ordering::Relaxed);
    p.buttons |= bn.plan.every_held(tick);
}

// ---------------------------------------------------------------- the watchdog

/// The watchdog's thread priority: above the game's (32), below the audio's (16).
const WATCH_PRIO: i32 = 20;
const TEXT: usize = 4096;

static ARMED: AtomicBool = AtomicBool::new(false);
static HANGS: AtomicU32 = AtomicU32::new(0);
/// The scene's tick (its settle reads it).
static SCENE_TICK: AtomicU32 = AtomicU32::new(0);

struct Shared {
    text: [u8; TEXT],
    text_len: usize,
    scene: [u8; 16],
    scene_len: usize,
    path: [u8; 160],
}

struct Cell(UnsafeCell<Shared>);

// SAFETY: written by the game's thread between scenes, read by the watchdog's when the game's has
// stalled; a torn read in a hang report is accepted (as `diag`'s).
unsafe impl Sync for Cell {}

static W: Cell = Cell(UnsafeCell::new(Shared { text: [0; TEXT], text_len: 0, scene: [0; 16], scene_len: 0, path: [0; 160] }));

/// What the watchdog writes if the game stalls now: the lines so far, the scene, the file.
fn watch_set(text: &str, scene: &str, path: &str) {
    // SAFETY: see `Cell`.
    let w = unsafe { &mut *W.0.get() };
    let n = text.len().min(TEXT);
    w.text[..n].copy_from_slice(&text.as_bytes()[..n]);
    w.text_len = n;
    let n = scene.len().min(w.scene.len());
    w.scene[..n].copy_from_slice(&scene.as_bytes()[..n]);
    w.scene_len = n;
    let n = path.len().min(w.path.len() - 1);
    w.path[..n].copy_from_slice(&path.as_bytes()[..n]);
    w.path[n] = 0;
}

/// The watchdog's thread, made at boot (it sleeps until a bench arms it).
pub fn start_watchdog() {
    let _ = crate::start_thread(b"watchdog\0", watchdog, WATCH_PRIO, 16 * 1024);
}

extern "C" fn watchdog(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    let (mut last, mut since, mut said) = (crate::diag::frames_now(), crate::now_us(), false);
    loop {
        // SAFETY: plain syscall.
        unsafe { sys::sceKernelDelayThread(250_000) };
        let f = crate::diag::frames_now();
        let now = crate::now_us();
        if f != last || !ARMED.load(Ordering::Acquire) {
            (last, since, said) = (f, now, false);
            continue;
        }
        let stalled = now.wrapping_sub(since);
        if !said && stalled >= b::HANG_US {
            said = true;
            HANGS.fetch_add(1, Ordering::Relaxed);
            hang_report(stalled / 1000, f);
        }
        if stalled >= 10 * b::HANG_US {
            // SAFETY: plain syscall; the evidence is written.
            unsafe { sys::sceKernelExitGame() };
        }
    }
}

struct Buf<'a>(&'a mut [u8], usize);

impl core::fmt::Write for Buf<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.0.len() - self.1);
        self.0[self.1..self.1 + n].copy_from_slice(&s.as_bytes()[..n]);
        self.1 += n;
        Ok(())
    }
}

/// `bench.txt` with the hang's line, and `crash.txt`: from the static buffers, allocating
/// nothing (the game's thread may hold the heap).
fn hang_report(stalled_ms: u32, frame: u32) {
    use core::fmt::Write as _;
    // SAFETY: see `Cell`.
    let w = unsafe { &mut *W.0.get() };
    let scene = core::str::from_utf8(&w.scene[..w.scene_len]).unwrap_or("?");
    let mut line = [0u8; 160];
    let mut l = Buf(&mut line, 0);
    let _ = writeln!(l, "hang scene {scene} stalled_ms {stalled_ms} frame {frame}");
    let len = l.1;
    // SAFETY: a NUL-terminated path in a static; the buffers outlive the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            w.path.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, w.text.as_ptr().cast(), w.text_len);
            sys::sceIoWrite(fd, line.as_ptr().cast(), len);
            sys::sceIoClose(fd);
        }
    }
    crate::diag::hang(scene, stalled_ms, crate::HEAP.live.get(), crate::HEAP.peak.get());
}
