//! The pad and `script.txt` (PORT.md §13.12, §13.13): the pad's sample, the scripted walk,
//! `Script` and its words, when a script's moment falls (`At`, `crossed`), the ballast held
//! back from the heap, and the dev words that set the atmosphere, the weather and flags.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::sync::atomic::Ordering;

use crate::shell;
use crate::shell::PspPad;
use jane_core::{Angle, ZoneId};
use jane_present::Present;
use jane_sim::Sim;

use crate::platform::free_mem;
use crate::workers::SLOW_PAINT;
use crate::Line;
use psp::sys;

// ---------------------------------------------------------------- the pad and the script

/// The scripted walk through the town: `(heading in degrees, or none to stand; ticks)`.
const WALK: &[(Option<i32>, u32)] = &[(None, 60), (Some(180), 330), (Some(270), 260), (Some(0), 400), (Some(90), 200)];

/// The walk's stick at play tick `tick`, as the PSP's stick would lean.
pub(crate) fn walk_at(tick: u32) -> (u8, u8) {
    let mut t = tick;
    for &(dir, n) in WALK {
        if t < n {
            return dir.map_or((128, 128), |d| {
                let a = Angle::from_degrees(d);
                let (c, s) = (jane_core::angle::cos_q15(a).0, jane_core::angle::sin_q15(a).0);
                ((128 + c * 127 / 32_768) as u8, (128 + s * 127 / 32_768) as u8)
            });
        }
        t -= n;
    }
    (128, 128)
}

/// The pad, one sample.
pub(crate) fn pad() -> PspPad {
    let mut d = sys::SceCtrlData { timestamp: 0, buttons: sys::CtrlButtons::empty(), lx: 128, ly: 128, rsrv: [0; 6] };
    // SAFETY: one sample into our struct.
    unsafe { sys::sceCtrlPeekBufferPositive(&mut d, 1) };
    PspPad { buttons: d.buttons.bits(), lx: d.lx, ly: d.ly }
}

/// A play session's ticks in [`At::Play`]: session `k`'s tick `n` is `k * SESSION + n`.
pub(crate) const SESSION: u32 = 1_000_000;

/// When a script's word acts: frames since the title came up (`t`), or ticks of play (`p` in the
/// first session, `q` in the second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum At {
    Title(u32),
    Play(u32),
}

impl At {
    fn parse(s: &str) -> Option<At> {
        let (k, n) = s.split_at(1);
        let n = n.parse().ok()?;
        match k {
            "t" => Some(At::Title(n)),
            // Play sessions: `p` the first, `q` the second (after a load or a new game).
            "p" => Some(At::Play(SESSION + n)),
            "q" => Some(At::Play(2 * SESSION + n)),
            _ => None,
        }
    }
}

/// What a script's `ballast:` holds back from the heap, to run short of RAM on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ballast {
    /// `ballast:KB`: this many KB, in 64 KB kernel blocks.
    Kb(u32),
    /// `ballast:autoKB`: blocks until this many KB of the kernel's RAM are free (the overlay's
    /// `free`), as when the world has just been built.
    Free(u32),
}

/// `script.txt` (PORT.md §13.12, §13.13): `ticks [hour [effects]] [zone:mark] [still]`, and the
/// presses and shots: `press:<button>@t<frame>` or `@p<tick>` (a press of six frames),
/// `hold:<button>@p<from>-<to>`, `shot@p<tick>` (the screen to `shot-p<tick>.bmp` beside the
/// program), `new` (New Game at once from the title, else the title waits for presses).
/// The pacing and the squeeze (the PSP v0.4.1 plan): `tpf:N` steps N ticks a frame (1 to 4,
/// default 1; `tpf:2` is a steady 30; `vclock` has the display's vblanks step the sim instead,
/// as a player's game does), `stand:X,Y` puts her at that county cell, `ballast:KB`
/// or `ballast:autoKB` (`...@p<tick>`: at that tick of play) holds RAM back once the world is
/// built, `listcap:KB` lowers the display list's cap (`Ge::list_cap`), `slowpaint:PCT` has each
/// paint job hold the painter's thread PCT% of its own time (the real PSP's slower painter on
/// an emulator: about 330), `perf` turns the overlay
/// and `perf.txt` on. `retitle@p<tick>` quits to the title then and starts New Game again: the
/// memory test of a county built in the RAM a played world left (`GAME let go` lines log the
/// partition's free holes and the heap's kernel blocks at each world let go).
#[derive(Debug, Default)]
pub(crate) struct Script {
    /// Play ticks to stop at.
    pub(crate) end: u32,
    pub(crate) hour: Option<u8>,
    pub(crate) effects: Option<u8>,
    pub(crate) place: Option<(ZoneId, String)>,
    pub(crate) still: bool,
    pub(crate) walk: bool,
    pub(crate) new: bool,
    /// `seed:N`: New Game's seed (else the clock's).
    pub(crate) seed: Option<u32>,
    /// `clock:HH:MM`: the clock set to that minute at the scripted travel, in place of the
    /// hour's (a frame a second short of nine: `clock:20:59`, the night's turn, NIGHT.md §2.2).
    pub(crate) clock: Option<u32>,
    /// `flag:NAME`: a world flag (or the spine's consequence of the name) set at the travel, as
    /// the sim's tests set one (`flag:mine_quiet`: the night at N2).
    pub(crate) flags: Vec<String>,
    /// `tp:<zone>:<mark>@p<tick>`: a dev travel at that tick (PORT.md §13.3's on-demand runs);
    /// `tp:<zone>:<mark>:X,Y@p<tick>` sets her down at that cell of the zone instead of the mark.
    pub(crate) tps: Vec<(ZoneId, String, At)>,
    /// `rest@p<tick>`: the slot written then, as a rest writes it (a scripted save anywhere).
    pub(crate) rests: Vec<At>,
    /// `retitle@p<tick>`: back to the title then (as the pause menu's Quit to the title), and New
    /// Game again from it: a county built in the RAM a played world left (the `q` session).
    pub(crate) retitles: Vec<At>,
    /// `learn:<spell>@p<tick>`: the spell learned then (a dev learn, as the sim's tests teach one).
    pub(crate) learns: Vec<(String, At)>,
    /// `noahead`: no zone built ahead of her (`ahead`), to measure without it.
    pub(crate) noahead: bool,
    /// `novfpu`: the mixer's group pan and send in integers. On by default (2.5-2.8x on
    /// hardware, maxdiff 0); `vfpu` is still accepted and turns it on. `vfpubench`: its
    /// micro-bench printed at boot (`micro vfpu`).
    pub(crate) novfpu: bool,
    pub(crate) vfpubench: bool,
    /// `nopaint`: the painter's thread handed no job (the ground new to the view stays the
    /// stand-in's; what a starved painter shows, to judge the fallback).
    pub(crate) nopaint: bool,
    /// `nobake`: the baked county (`county.jck`) not opened: the painter paints it, as before P1c.
    pub(crate) nobake: bool,
    /// `nopipe`: each frame waits for its own list (no CPU and GE overlap), as before.
    pub(crate) nopipe: bool,
    /// `fps:60|30|free`: the frame rate's pacing for this run.
    pub(crate) pacing: Option<jane_render_psp::ge::Pacing>,
    /// `gfx:full|balanced|fast` (a preset), `gfx:palette|readback` (the grade's way),
    /// `gfx-<effect>`, `gfx+<effect>` (`gfx_psp::Effect`
    /// keys): the Graphics page for this run.
    gfx_words: Vec<String>,
    /// `sleep@p<n>` and `resume@p<n>`: what a sleep does that an emulator does not (VRAM's
    /// slots and pools overwritten, the pages in RAM let go, the pack's handle closed under it),
    /// then for `resume` the power callback's resume as the hardware sends it.
    pub(crate) sleeps: Vec<(At, bool)>,
    /// `perf`: the performance overlay on from the start (as L + R + SELECT turns it on).
    pub(crate) perf: bool,
    /// `stale@p<n>`: the pack's handle closed under it and every page let go (the reads' retry).
    pub(crate) stales: Vec<At>,
    /// `crashtest:N` (the `crashtest` feature only): panic at frame N.
    #[cfg(feature = "crashtest")]
    crash_at: Option<u32>,
    /// `hangtest:N` (the `crashtest` feature only): spin for ever at frame N (the watchdog's test).
    #[cfg(feature = "crashtest")]
    hang_at: Option<u32>,
    /// `frames:N`: stop after N frames of play (a held world's ticks stand still: the map open).
    pub(crate) frames: Option<u32>,
    /// `framed`: the `p` clock counts frames of play, not ticks (presses in a held world).
    pub(crate) framed: bool,
    presses: Vec<(u32, At, At)>,
    /// `stick:<degrees>@p<a>-p<b>`: the stick leaned that way (0 east, 90 south) meanwhile.
    pub(crate) sticks: Vec<(i32, At, At)>,
    pub(crate) shots: Vec<At>,
    /// `walk:stick|dpad|both`, `dead:low|medium|high`: the pad settings for this run (not
    /// written to the stick unless the Controls page turns one).
    pub(crate) walk_with: Option<jane_present::pad_psp::WalkWith>,
    pub(crate) dead: Option<jane_present::pad_psp::DeadZone>,
    /// `ground:terraces|viaduct` (MAP.md §9 R3): New Game on a dev ground in the county's place
    /// (`jane_sim::dev_ground`); `stand:X,Y`: she starts at that cell of it, or of the county.
    pub(crate) ground: Option<jane_sim::dev_ground::Ground>,
    pub(crate) stand: Option<(i32, i32)>,
    /// `tpf:N`: the sim's ticks a frame (1 to 4, default 1). A script's pad is read once a frame,
    /// so the ticks cannot follow the display's vblanks (a run's state would hang on the
    /// emulator's timing); this is the fixed count that does, `tpf:2` the steady 30's.
    pub(crate) tpf: Option<u32>,
    /// `vclock`: the sim steps on the display's clock as a player's game does (a tick a vblank
    /// the last flip took), not by `tpf`: to see that clock on an emulator. The run's state then
    /// hangs on the emulator's timing (a script's hash is not repeatable).
    pub(crate) vclock: bool,
    /// `ballast:KB` or `ballast:autoKB`: RAM held back once the world is built (the crash's
    /// short heap, reproduced); with `@p<tick>` at that tick of play instead, when the page
    /// cache and the chunks are warm and `auto` means the free RAM of play, not of the build.
    pub(crate) ballast: Option<(Ballast, Option<At>)>,
    /// `listcap:KB`: the display list's cap, lowered to see the list guard act.
    pub(crate) list_cap_kb: Option<u32>,
    /// `order:N`: the passes whose draws are grouped by texture (`Lister::order_passes`, bit a
    /// pass id; 0 none): to weigh what the grouping costs against the binds it saves.
    pub(crate) order: Option<u32>,
}

impl Script {
    /// The Graphics page with this script's `gfx` words over `g`.
    pub(crate) fn gfx(&self, g: jane_present::gfx_psp::Graphics) -> jane_present::gfx_psp::Graphics {
        use jane_present::gfx_psp::{Effect, Preset};
        let mut g = g;
        for w in &self.gfx_words {
            if let Some(p) = w.strip_prefix("gfx:").and_then(Preset::from_key) {
                g.take_preset(p);
            } else if w == "gfx:palette" || w == "gfx:readback" {
                g.set(Effect::Grade, true);
                g.grade_full = w == "gfx:readback";
            } else if let Some((on, k)) =
                w.strip_prefix("gfx+").map(|k| (true, k)).or(w.strip_prefix("gfx-").map(|k| (false, k)))
            {
                if let Some(e) = Effect::ALL.into_iter().find(|e| e.key() == k) {
                    g.set(e, on);
                }
            }
        }
        g
    }

    pub(crate) fn parse(text: &str) -> Script {
        let mut s = Script::default();
        let mut nums = Vec::new();
        let mut words = 0;
        for w in text.split_whitespace() {
            words += 1;
            if let Ok(n) = w.parse::<u32>() {
                nums.push(n);
            } else if w == "still" {
                s.still = true;
            } else if w == "noahead" {
                s.noahead = true;
            } else if w == "novfpu" {
                s.novfpu = true;
            } else if w == "vfpu" {
                s.novfpu = false;
            } else if w == "vfpubench" {
                s.vfpubench = true;
            } else if w == "nopaint" {
                s.nopaint = true;
            } else if w == "nobake" {
                s.nobake = true;
            } else if w == "nopipe" {
                s.nopipe = true;
            } else if w.starts_with("gfx") {
                s.gfx_words.push(String::from(w));
            } else if let Some(v) = w.strip_prefix("fps:") {
                use jane_render_psp::ge::Pacing;
                s.pacing = match v {
                    "30" => Some(Pacing::Locked30),
                    "free" => Some(Pacing::Unlocked),
                    _ => Some(Pacing::Vsync),
                };
            } else if w == "perf" {
                s.perf = true;
            } else if w == "framed" {
                s.framed = true;
            } else if w == "new" {
                s.new = true;
            } else if let Some(rest) = w.strip_prefix("press:").or_else(|| w.strip_prefix("hold:")) {
                let hold = w.starts_with("hold:");
                let Some((b, at)) = rest.split_once('@') else { continue };
                let Some(bit) = shell::psp::NAMES.iter().find(|e| e.0 == b).map(|e| e.1) else { continue };
                if hold {
                    let Some((a, z)) = at.split_once('-') else { continue };
                    let (Some(a), Some(z)) = (At::parse(a), At::parse(z)) else { continue };
                    s.presses.push((bit, a, z));
                } else if let Some(a) = At::parse(at) {
                    let z = match a {
                        At::Title(n) => At::Title(n + 6),
                        At::Play(n) => At::Play(n + 6),
                    };
                    s.presses.push((bit, a, z));
                }
            } else if let Some(rest) = w.strip_prefix("stick:") {
                let Some((d, at)) = rest.split_once('@') else { continue };
                let Some((a, z)) = at.split_once('-') else { continue };
                if let (Ok(d), Some(a), Some(z)) = (d.parse(), At::parse(a), At::parse(z)) {
                    s.sticks.push((d, a, z));
                }
            } else if let Some((resume, at)) =
                w.strip_prefix("sleep@").map(|a| (false, a)).or_else(|| w.strip_prefix("resume@").map(|a| (true, a)))
            {
                if let Some(a) = At::parse(at) {
                    s.sleeps.push((a, resume));
                }
            } else if let Some(at) = w.strip_prefix("stale@") {
                if let Some(a) = At::parse(at) {
                    s.stales.push(a);
                }
            } else if let Some(at) = w.strip_prefix("rest@") {
                if let Some(a) = At::parse(at) {
                    s.rests.push(a);
                }
            } else if let Some(at) = w.strip_prefix("retitle@") {
                if let Some(a) = At::parse(at) {
                    s.retitles.push(a);
                }
            } else if let Some(rest) = w.strip_prefix("learn:") {
                if let Some((sp, a)) = rest.split_once('@').and_then(|(sp, at)| Some((sp, At::parse(at)?))) {
                    s.learns.push((String::from(sp), a));
                }
            } else if let Some(rest) = w.strip_prefix("tp:") {
                let Some((zm, at)) = rest.split_once('@') else { continue };
                let Some((z, m)) = zm.split_once(':') else { continue };
                if let (Some(zz), Some(a)) = (ZoneId::ALL.into_iter().find(|zz| zz.name() == z), At::parse(at)) {
                    s.tps.push((zz, String::from(m), a));
                }
            } else if let Some(n) = w.strip_prefix("crashtest:").filter(|_| cfg!(feature = "crashtest")) {
                #[cfg(feature = "crashtest")]
                {
                    s.crash_at = n.parse().ok();
                }
                #[cfg(not(feature = "crashtest"))]
                let _ = n;
            } else if let Some(n) = w.strip_prefix("hangtest:").filter(|_| cfg!(feature = "crashtest")) {
                #[cfg(feature = "crashtest")]
                {
                    s.hang_at = n.parse().ok();
                }
                #[cfg(not(feature = "crashtest"))]
                let _ = n;
            } else if let Some(n) = w.strip_prefix("frames:") {
                s.frames = n.parse().ok();
            } else if let Some(v) = w.strip_prefix("walk:") {
                s.walk_with = jane_present::pad_psp::WalkWith::from_key(v);
            } else if let Some(v) = w.strip_prefix("dead:") {
                s.dead = jane_present::pad_psp::DeadZone::from_key(v);
            } else if let Some(n) = w.strip_prefix("seed:") {
                s.seed = n.parse().ok();
            } else if let Some(c) = w.strip_prefix("clock:") {
                let mut hm = c.split(':').map(|v| v.parse::<u32>().ok());
                if let (Some(Some(h)), Some(Some(m))) = (hm.next(), hm.next()) {
                    let hour = jane_sim::tuning::TICKS_PER_HOUR;
                    s.clock = Some((h % 24) * hour + m.min(59) * hour / 60);
                }
            } else if let Some(f) = w.strip_prefix("flag:") {
                s.flags.push(String::from(f));
            } else if let Some(g) = w.strip_prefix("ground:") {
                s.ground = jane_sim::dev_ground::Ground::parse(g);
            } else if let Some(n) = w.strip_prefix("tpf:") {
                s.tpf = n.parse::<u32>().ok().map(|n| n.clamp(1, 4));
            } else if w == "vclock" {
                s.vclock = true;
            } else if let Some(v) = w.strip_prefix("ballast:") {
                let (v, at) = v.split_once('@').map_or((v, None), |(v, at)| (v, At::parse(at)));
                let b = match v.strip_prefix("auto") {
                    Some(kb) => kb.parse().ok().map(Ballast::Free),
                    None => v.parse().ok().map(Ballast::Kb),
                };
                s.ballast = b.map(|b| (b, at));
            } else if let Some(pc) = w.strip_prefix("slowpaint:").and_then(|v| v.parse::<u32>().ok()) {
                SLOW_PAINT.store(pc.max(100), Ordering::Relaxed);
            } else if let Some(kb) = w.strip_prefix("listcap:") {
                s.list_cap_kb = kb.parse().ok();
            } else if let Some(n) = w.strip_prefix("order:") {
                s.order = n.parse().ok();
            } else if let Some((x, y)) = w.strip_prefix("stand:").and_then(|v| v.split_once(',')) {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    s.stand = Some((x, y));
                }
            } else if let Some(at) = w.strip_prefix("shot@") {
                if let Some(a) = At::parse(at) {
                    s.shots.push(a);
                }
            } else if let Some((z, m)) = w.split_once(':') {
                if let Some(zz) = ZoneId::ALL.into_iter().find(|zz| zz.name() == z) {
                    s.place = Some((zz, String::from(m)));
                }
            }
        }
        let _ = words;
        s.end = nums.first().copied().unwrap_or(600);
        s.hour = nums.get(1).map(|&h| h.min(23) as u8);
        s.effects = nums.get(2).map(|&e| e as u8);
        // The old scripts walk the town from the square; one that presses its way stands.
        s.walk = !s.still && s.presses.is_empty() && s.sticks.is_empty();
        s.new = s.new || !s.presses.iter().any(|p| matches!(p.1, At::Title(_)));
        s
    }

    /// The buttons the script holds at `now` (and in which phase).
    pub(crate) fn held(&self, now: At) -> u32 {
        self.presses
            .iter()
            .filter(|(_, a, z)| match (now, *a, *z) {
                (At::Title(t), At::Title(a), At::Title(z)) | (At::Play(t), At::Play(a), At::Play(z)) => t >= a && t < z,
                _ => false,
            })
            .fold(0, |m, p| m | p.0)
    }
}

pub(crate) fn at_reached(at: At, now: At) -> bool {
    match (at, now) {
        (At::Title(a), At::Title(n)) | (At::Play(a), At::Play(n)) => n >= a,
        _ => false,
    }
}

/// Whether a script's moment `at` fell in the frame that took the clock from `prev` to `now`: at
/// `now`, or between the two (a frame steps `tpf` ticks, so `now` can skip a tick).
pub(crate) fn crossed(at: At, prev: At, now: At) -> bool {
    match (at, prev, now) {
        (At::Play(a), At::Play(p), At::Play(n)) => a == n || (p < a && a < n),
        _ => at == now,
    }
}

/// The ballast's block: kernel RAM held for the run, never freed.
const BALLAST_BLOCK: u32 = 64 * 1024;

/// A script's `ballast:` (the v0.4.1 plan, E): kernel blocks of 64 KB taken and held, so the run
/// is short of RAM as the owner's was (2.0 MB free, the largest block 1.4 MB). `Free(kb)` takes
/// blocks until `kb` KB of the kernel's RAM are free, and a last smaller one for the remainder.
/// Taken from the top, where the heap's own big blocks come from.
pub(crate) fn hold_ballast(b: Ballast) {
    let take = |bytes: u32| -> bool {
        // SAFETY: plain syscalls; the block is never freed.
        let id = unsafe {
            sys::sceKernelAllocPartitionMemory(
                sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
                b"ballast\0".as_ptr(),
                sys::SceSysMemBlockTypes::High,
                bytes,
                core::ptr::null_mut(),
            )
        };
        id.0 >= 0
    };
    let before = free_mem();
    let (mut blocks, mut held) = (0u32, 0u32);
    match b {
        Ballast::Kb(kb) => {
            let mut left = kb * 1024;
            while left > 0 {
                let n = left.min(BALLAST_BLOCK);
                if !take(n) {
                    break;
                }
                (blocks, held, left) = (blocks + 1, held + n, left - n);
            }
        }
        Ballast::Free(kb) => {
            let keep = kb * 1024;
            while free_mem().1 > keep + BALLAST_BLOCK {
                if !take(BALLAST_BLOCK) {
                    break;
                }
                (blocks, held) = (blocks + 1, held + BALLAST_BLOCK);
            }
            // The remainder in one block, rounded down to the kernel's 256-byte grain with room
            // for its header.
            let rest = free_mem().1.saturating_sub(keep).saturating_sub(512) & !255;
            if rest >= 4096 && take(rest) {
                (blocks, held) = (blocks + 1, held + rest);
            }
        }
    }
    let after = free_mem();
    say!(
        "GAME ballast {:?} blocks={blocks} held={held} free {}K->{}K largest {}K->{}K",
        b,
        before.1 / 1024,
        after.1 / 1024,
        before.0 / 1024,
        after.0 / 1024
    );
}

/// A script's `atmos=N` word: the atmosphere's passes left out (`jane_render_psp::list::atmos_fx`
/// bits), to measure each one's cost.
pub(crate) fn script_atmos(text: Option<&str>) -> u16 {
    text.and_then(|t| t.split_whitespace().find_map(|w| w.strip_prefix("atmos=")?.parse().ok())).unwrap_or(0)
}

/// A script's weather word (`clear`, `mist`, `rain` or `storm`): the sky held to it, and in rain
/// or a storm the sim's sky and the county's ground too, as `jane sheet scene --weather` holds
/// them, so a scripted frame matches a PC one at the same state (the atmosphere's shots).
pub(crate) fn script_weather(text: Option<&str>, present: &mut Present, sim: &mut Sim) {
    use jane_present::WeatherKind as K;
    let Some(kind) = text.and_then(|t| {
        t.split_whitespace().find_map(|w| match w {
            "clear" => Some(K::Clear),
            "mist" => Some(K::Mist),
            "rain" => Some(K::Rain),
            "storm" => Some(K::Storm),
            _ => None,
        })
    }) else {
        return;
    };
    let wet = if matches!(kind, K::Rain | K::Storm) { 255 } else { 0 };
    present.atmos_mut().force(Some((kind, wet)));
    if wet > 0 {
        use jane_sim::state::{WeatherKind as Sky, WeatherState};
        let sky = if kind == K::Storm { Sky::Storm } else { Sky::Rain };
        let now = sim.state().tick;
        let st = sim.state_mut();
        st.weather = st.weather.map(|_| WeatherState { kind: sky, since: now, until: jane_core::Tick(u32::MAX) });
        if let Some(z) = st.zone_mut(ZoneId::County) {
            z.wetness = z.wetness.map(|_| 255);
        }
    }
}

/// A script's `flag:NAME` words: each world flag set to 1, and the spine's consequence of the
/// name marked done (`mine_quiet`, `works_dark`, `burial_quiet`: the night's stage, NIGHT.md
/// §3.2), as the sim's own tests set them. A dev script's, never a player's.
pub(crate) fn script_flags(names: &[String], sim: &mut Sim) {
    use jane_sim::state::FlagKey;
    for name in names {
        if let Some(c) = jane_data::catalog().living.consequence_id(name) {
            sim.state_mut().consequences_done.set(u32::from(c.0), true);
        }
        if let Some(k) = sim.state().syms.find(name) {
            sim.state_mut().flags.insert(FlagKey::Named(k), 1);
        }
        say!("GAME flag {name}");
    }
}
