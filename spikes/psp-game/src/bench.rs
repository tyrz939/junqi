//! The built-in benchmark on the PSP (`jane_present::bench` holds the scenes and the measures):
//! the title's Benchmark row, or a `script.txt` holding the word `bench`, runs the seven scenes
//! back to back on a fresh game that is never saved, then writes `bench.txt` to the save folder
//! and shows the results over the title (O closes them). A scripted run ends with the bench.
//!
//! Each scene is the script machinery's own (`crate::Script`, parsed from the scene's words plus
//! `vclock`, so the sim steps on the display's clock as a player's game does); this module adds
//! the bench's words (`at:`, `route:`, `loop:`, `spawn:`, `every:`, `tough`), the measures and the
//! watchdog. Each scene's measured frames are summed into a small histogram
//! (`capture::Summary`, the whole window) and the dashcam's last ten seconds of that scene are
//! written to `capture-<scene>.bin`. No dashcam, no summary RAM or a failed write leaves
//! `cap -` or `sum_frames -` and the bench goes on. The game's loop calls in at a few marked
//! hooks only (`main.rs`, "the bench").
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
use jane_render_psp::capture::{self, Summary};
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
    /// The save folder (`ms0:/PSP/SAVEDATA/JANE00001/`), captures beside `bench.txt`.
    dir: String,
    path: String,
    /// `{settings}vclock {extra}`, so the header can be rewritten once `vfpu_pan` is known.
    settings_tail: String,
    /// The whole measured window. `None` when the histogram could not be allocated.
    sum: Option<Summary>,
    /// [`PARTS`]' sums over the measured frames, and the frames.
    parts: [u64; capture::summary::PARTS.len()],
    parts_n: u32,
    /// `Recorder::pushed` just before the first measured frame, when a dashcam exists.
    cap_from: Option<u32>,
    /// Captures written this run.
    caps_saved: u32,
    last_us: u32,
    frames: u32,
    paint_fails0: u32,
    hangs0: u32,
    /// The results are up over the title.
    showing: bool,
    /// O (or X) closed them: the pad is still the bench's until every button is let go, so the
    /// press does not reach the title under them (the owner's X there picked Benchmark again,
    /// 2026-10-21: the micro-bench wrote over `bench.txt` and a second county ran out of RAM).
    closing: bool,
    /// The loader's counts at the scene's start (`loader::counts`).
    baked0: [u32; 8],
    /// Writes of `bench.txt` that failed even when tried again.
    unsaved: u32,
}

impl Bench {
    /// A bench for a script that says `bench`.
    pub fn from_script(text: Option<&str>, dir: &str, pack: &str, shell: &Shell) -> Option<Bench> {
        let t = text?;
        t.split_whitespace().any(|w| w == "bench").then(|| {
            let extra: Vec<&str> = t.split_whitespace().filter(|&w| w != "bench").collect();
            Bench::new(true, &extra.join(" "), dir, pack, shell)
        })
    }

    fn new(scripted: bool, extra: &str, dir: &str, pack: &str, shell: &Shell) -> Bench {
        let mut settings = String::new();
        for l in shell.settings().write().lines() {
            settings.push_str(l);
            settings.push(' ');
        }
        // `vfpu_pan` is filled in by `stamp_vfpu` once the scene script has stored the flag.
        let settings_tail = format!("{settings}vclock {extra}");
        let mut text = b::header("psp", env!("JANE_COMMIT"), &settings_tail);
        text.push('\n');
        let path = format!("{dir}bench.txt");
        // The storage and decode micro-bench (micro.rs) first, written at once, so a scene that
        // hangs later still leaves its lines; the watchdog is not armed yet (it runs for seconds
        // with no frame drawn).
        let micro = crate::micro::run(pack);
        say!("GAME bench micro {}", micro.trim_end());
        text.push_str(&micro);
        let unsaved = u32::from(!save(&path, &text));
        watch_set(&text, b::SCENES[0].name, &path);
        ARMED.store(true, Ordering::Release);
        say!("GAME bench start scripted={scripted} {}", text.trim_end());
        let sum = Summary::new();
        if sum.is_none() {
            say!("GAME bench summary: no RAM");
        }
        let mut bench = Bench {
            scripted,
            extra: String::from(extra),
            k: 0,
            plan: Plan::default(),
            steer: Steer::default(),
            stats: Stats::default(),
            text,
            dir: String::from(dir),
            path,
            settings_tail,
            sum,
            parts: [0; capture::summary::PARTS.len()],
            parts_n: 0,
            cap_from: None,
            caps_saved: 0,
            last_us: crate::now_us(),
            frames: 0,
            paint_fails0: crate::diag::paint_fails(),
            hangs0: HANGS.load(Ordering::Relaxed),
            showing: false,
            closing: false,
            baked0: crate::loader::counts(),
            unsaved,
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
        self.baked0 = crate::loader::counts();
        self.cap_from = None;
        (self.parts, self.parts_n) = ([0; capture::summary::PARTS.len()], 0);
        if let Some(sum) = self.sum.as_mut() {
            sum.clear();
        }
        watch_set(&self.text, b::SCENES[k].name, &self.path);
        say!("GAME bench scene {} {}", k + 1, b::SCENES[k].name);
    }

    /// The header's `vfpu_pan`, from the flag the scene script stored. Called once that store
    /// has happened (a scripted bench's script is applied after `new`).
    pub fn stamp_vfpu(&mut self) {
        let pan = if jane_audio_psp::psp::VFPU_PAN.load(Ordering::Relaxed) { "on" } else { "off" };
        let line = b::header("psp", env!("JANE_COMMIT"), &format!("vfpu_pan {pan} {}", self.settings_tail));
        let skip = self.text.find('\n').map_or(self.text.len(), |i| i + 1);
        let rest = String::from(&self.text[skip..]);
        self.text = line;
        self.text.push('\n');
        self.text.push_str(&rest);
        let armed = ARMED.swap(false, Ordering::AcqRel);
        self.unsaved += u32::from(!save(&self.path, &self.text));
        watch_set(&self.text, b::SCENES[self.k].name, &self.path);
        if armed {
            ARMED.store(true, Ordering::Release);
        }
        self.last_us = crate::now_us();
    }

    /// One drawn frame. Measured frames are the ones [`Stats::frame`] counts: in play, the
    /// results not up, and the scene's tick past its settle. The summary takes every one of
    /// those whether or not the dashcam exists; the capture's window opens on the first, when
    /// the ring is there.
    pub fn record(
        &mut self,
        rec: Option<&capture::Recorder>,
        fields: &[u32; capture::field::N],
        passes: &[u32; capture::pass::N],
        parts: &[u32; capture::summary::PARTS.len()],
        in_play: bool,
        wall_us: u32,
    ) {
        if self.showing || !in_play || SCENE_TICK.load(Ordering::Relaxed) <= self.plan.settle {
            return;
        }
        if self.cap_from.is_none() {
            if let Some(r) = rec {
                self.cap_from = Some(r.pushed.wrapping_sub(1));
            }
        }
        if let Some(sum) = self.sum.as_mut() {
            sum.push(fields, passes, wall_us);
        }
        for (s, &v) in self.parts.iter_mut().zip(parts) {
            *s += u64::from(v);
        }
        self.parts_n += 1;
    }

    /// The scene's script: its words, the display's clock, New Game's seed, frames unpaced (so a
    /// scene shows its headroom past the Graphics page's 30 or 60), the script's own.
    pub fn script(&self) -> crate::Script {
        let words = format!("{} vclock seed:{} fps:free {}", b::SCENES[self.k].words, b::SEED, self.extra);
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
    /// `pacing` is the capture header's `pacing …` line, already ending in a newline.
    fn next(
        &mut self,
        sim: Option<&Sim>,
        shell: &mut Shell,
        dash: Option<&capture::Recorder>,
        pacing: &str,
    ) -> Option<crate::Script> {
        let st = &mut self.stats;
        st.paint_fails = crate::diag::paint_fails().wrapping_sub(self.paint_fails0);
        st.hangs = HANGS.load(Ordering::Relaxed).wrapping_sub(self.hangs0);
        st.at = sim
            .and_then(|s| s.view(Seat(0)))
            .and_then(|v| v.unit(v.me().unit).map(|u| (v.zone(), u.pos.cell())))
            .map_or_else(|| String::from("-"), |(z, (x, y))| format!("{}:{x},{y}", z.name()));
        let hangs = st.hangs;
        let mut line = st.line(b::SCENES[self.k].name);
        // The baked county's loader over the scene: blobs read and their mean time, chunks
        // decoded and theirs, landed, refused by the presenter, failed, and the frames that ended
        // with a chunk to land still unlanded.
        let counts = crate::loader::counts();
        let d: [u32; 8] = core::array::from_fn(|i| counts[i].wrapping_sub(self.baked0[i]));
        let mean = |us: u32, n: u32| if n == 0 { 0 } else { us / n };
        line.push_str(&format!(
            " baked_reads {} read_us {} baked_decodes {} dec_us {} landed {} refused {} bfail {} baked_wait {}",
            d[0],
            mean(d[1], d[0]),
            d[2],
            mean(d[3], d[2]),
            d[4],
            d[5],
            d[6],
            d[7]
        ));
        let name = b::SCENES[self.k].name;
        let n = match (dash, self.cap_from) {
            (Some(r), Some(from)) => (r.pushed.wrapping_sub(from) as usize).min(r.frames()),
            _ => 0,
        };
        // The stick writes sit inside the frame that just ended. Disarm the watchdog for them,
        // or a slow card on an already long frame looks like a hang and truncates bench.txt.
        ARMED.store(false, Ordering::Release);
        let wrote = match dash {
            Some(rec) if n > 0 => self.write_capture(rec, sim, shell, name, n, pacing),
            // No capture this run: an earlier bench's file must not pass for this one's.
            _ => {
                remove(&format!("{}capture-{name}.bin", self.dir));
                false
            }
        };
        let sum_frames = self.sum.as_ref().map_or_else(|| String::from("-"), |s| format!("{}", s.frames()));
        let cap_frames = if wrote { n } else { 0 };
        let cap = if wrote { format!("capture-{name}.bin") } else { String::from("-") };
        line.push_str(&format!(" sum_frames {sum_frames} cap_frames {cap_frames} cap {cap}"));
        if let Some(sum) = &self.sum {
            sum.write(&mut line);
        } else {
            capture::summary::write_absent(&mut line);
        }
        for (name, &sum) in capture::summary::PARTS.iter().zip(&self.parts) {
            let us = sum / u64::from(self.parts_n.max(1));
            line.push_str(&format!(" {name}_ms {}.{}", us / 1000, us % 1000 / 100));
        }
        say!("GAME bench {line}");
        self.text.push_str(&line);
        self.text.push('\n');
        // The watchdog's own line went into the file it wrote; this one keeps the hang in it.
        if hangs > 0 {
            self.text.push_str(&format!("hang scene {name} count {hangs} (see crash.txt)\n"));
        }
        if self.k + 1 < b::SCENES.len() {
            self.unsaved += u32::from(!save(&self.path, &self.text));
            self.begin(self.k + 1);
            ARMED.store(true, Ordering::Release);
            self.last_us = crate::now_us();
            return Some(self.script());
        }
        // The last scene is done (the micro-bench ran first, in `new`). The watchdog stays down.
        let saved = save(&self.path, &self.text);
        self.last_us = crate::now_us();
        say!("GAME bench done {} saved={saved} failed_writes={}", self.path, self.unsaved);
        let mut lines = Vec::new();
        let caps = self.caps_saved;
        lines.push(if saved && self.unsaved == 0 {
            format!("Benchmark: bench.txt saved, {caps}/7 captures")
        } else if saved {
            format!("Benchmark: bench.txt saved ({} earlier writes failed), {caps}/7 captures", self.unsaved)
        } else {
            format!("Benchmark: bench.txt NOT saved, {caps}/7 captures")
        });
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
        lines.push(String::from("O or X to close"));
        shell.overlay = lines;
        self.showing = true;
        None
    }

    /// The newest `n` frames to `capture-<name>.bin`. False (and the partial file removed) when
    /// the bytes cannot be reserved or the stick write fails.
    fn write_capture(
        &mut self,
        rec: &capture::Recorder,
        sim: Option<&Sim>,
        shell: &Shell,
        name: &str,
        n: usize,
        pacing: &str,
    ) -> bool {
        let pan = if jane_audio_psp::psp::VFPU_PAN.load(Ordering::Relaxed) { "on" } else { "off" };
        let header = format!(
            "{}bench_scene {name}\nbench_frames {n}\n{pacing}vfpu_pan {pan}\n",
            crate::capture_header(shell, sim, None)
        );
        let path = format!("{}capture-{name}.bin", self.dir);
        let Some(bytes) = rec.encode_last(&header, n) else {
            say!("GAME bench capture {path}: no RAM");
            remove(&path);
            return false;
        };
        match crate::write_file(&path, &bytes) {
            Ok(()) => {
                self.caps_saved += 1;
                true
            }
            Err(e) => {
                say!("GAME bench capture {path} failed: {e}");
                remove(&path);
                false
            }
        }
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
    dash: Option<&capture::Recorder>,
    pacing: &str,
) -> bool {
    let Some(bn) = bench.as_mut().filter(|bn| !bn.showing) else { return true };
    match bn.next(sim, shell, dash, pacing) {
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

/// `text` written over `path`, tried once more if the write fails; whether it was written. A
/// failure is logged (and counted by the caller, shown with the results).
fn save(path: &str, text: &str) -> bool {
    let first = crate::write_file(path, text.as_bytes());
    let Err(e) = first else { return true };
    say!("GAME bench write failed: {e}; trying again");
    // SAFETY: plain syscall: a moment for the stick before the second try.
    unsafe { sys::sceKernelDelayThread(20_000) };
    match crate::write_file(path, text.as_bytes()) {
        Ok(()) => true,
        Err(e) => {
            say!("GAME bench write failed again: {e}");
            false
        }
    }
}

/// The Benchmark row picked: its ask turned into New Game on the bench's seed, and the bench
/// made. Returns the bench, whose script the loop takes.
pub fn start_asked(shell: &mut Shell, dir: &str, pack: &str) -> Option<Bench> {
    let at = shell.asks.iter().position(|a| matches!(a, Ask::Bench))?;
    shell.asks[at] = Ask::NewGame;
    shell.overlay.clear();
    Some(Bench::new(false, "", dir, pack, shell))
}

/// The pad's hook (after the script's own presses and stick): the bench's `every:` presses in
/// play; with the results up, the pad is theirs alone (the title under them sees nothing), O or
/// X closes them, and the bench ends once every button is let go.
pub fn pad(bench: &mut Option<Bench>, p: &mut PspPad, in_play: bool, tick: u32, shell: &mut Shell) {
    let Some(bn) = bench.as_mut() else { return };
    if bn.showing {
        let raw = p.buttons;
        (p.buttons, p.lx, p.ly) = (0, 128, 128);
        if bn.closing {
            if raw == 0 {
                *bench = None;
            }
        } else if raw & (button::CIRCLE | button::CROSS) != 0 {
            shell.overlay.clear();
            bn.closing = true;
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
/// The bench's lines copied for the watchdog. 16 KB: seven scene lines with their summaries.
const TEXT: usize = 16384;

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

static W: Cell =
    Cell(UnsafeCell::new(Shared { text: [0; TEXT], text_len: 0, scene: [0; 16], scene_len: 0, path: [0; 160] }));

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
/// A file gone from the stick (a missing one is no error).
fn remove(path: &str) {
    let z = crate::cpath(path);
    // SAFETY: a NUL-terminated path that outlives the call.
    unsafe { sys::sceIoRemove(z.as_ptr()) };
}

fn hang_report(stalled_ms: u32, frame: u32) {
    // Stood down meanwhile (the scene's own stick writes): its line must not be truncated.
    if !ARMED.load(Ordering::Acquire) {
        return;
    }
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
