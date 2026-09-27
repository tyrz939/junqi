//! The F2 overlay (PRESENTATION.md §3.2, §1.12): off, compact, full.
//!
//! **Compact** is a small box top right: fps, the frame's ms, its p50 and p99, the sim's tick,
//! the backend and tier, dropped ticks; its border goes amber then red over budget.
//!
//! **Full** adds the frame-time graph (the last 240 frames as stacked bars by stage: sim step,
//! presenter tick, frame build and UI, backend draw, the wait for the display; budget lines at
//! 16.7 and 8.3 ms), a second graph of the sim's tick, the per-pass table from the backend's own
//! clock (`Backend::stats`), the sim's numbers (`Sim::metrics`, a second's means: units awake,
//! paths, events, and the step's time in eight groups of its phases), the presenter's and the
//! backend's counts, the last eight hitches over 20 ms with the stage that dominated each, and the
//! passes in force.
//!
//! The app measures; this file keeps the rings and draws. Nothing here allocates per frame
//! after the first.

use std::fmt::Write as _;

use jane_art::font::Face;
use jane_art::palette::{Ix, Ramp, Tone};
use jane_sim::metrics::PHASES;
use jane_sim::{Phase, SimMetrics};

use crate::backend::{FrameStats, StatPass};
use crate::frame::Tier;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, PanelStyle, Ui, fmt_u32, line_h, text_w};
use crate::ui::style::{self, argb};

/// Frames the graph shows.
pub const FRAMES: usize = 240;
/// The stages a frame's time is split into, in the order the bars stack.
pub const STAGES: [&str; 5] = ["sim", "pres", "build", "back", "wait"];
/// A frame this long or longer is a hitch, µs.
pub const HITCH_US: u32 = 20_000;
/// The budgets, µs: 60 and 120 frames a second.
pub const BUDGET_60: u32 = 16_667;
pub const BUDGET_120: u32 = 8_333;

/// A frame over the hitch line: its length, the stage that took most of it, the tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hitch {
    pub total_us: u32,
    pub stage: u8,
    pub tick: u64,
}

/// What the app measured, frame by frame and tick by tick.
#[derive(Clone, Debug)]
pub struct PerfLog {
    frames: Box<[[u32; 5]; FRAMES]>,
    n_frames: usize,
    ticks: Box<[u32; FRAMES]>,
    n_ticks: usize,
    hitches: [Hitch; 8],
    n_hitches: usize,
    /// Frames a second, as last counted.
    pub fps: u32,
    fps_count: u32,
    fps_since: u64,
}

impl Default for PerfLog {
    fn default() -> PerfLog {
        PerfLog {
            frames: Box::new([[0; 5]; FRAMES]),
            n_frames: 0,
            ticks: Box::new([0; FRAMES]),
            n_ticks: 0,
            hitches: [Hitch::default(); 8],
            n_hitches: 0,
            fps: 0,
            fps_count: 0,
            fps_since: 0,
        }
    }
}

/// Percentile `q` (0..=100) of `v`, sorting a copy on the stack.
fn percentile(v: &[u32], q: usize) -> u32 {
    if v.is_empty() {
        return 0;
    }
    let mut s = [0u32; FRAMES];
    let n = v.len().min(FRAMES);
    s[..n].copy_from_slice(&v[..n]);
    let s = &mut s[..n];
    s.sort_unstable();
    s[((n - 1) * q / 100).min(n - 1)]
}

impl PerfLog {
    /// One frame's stages, µs, and the app's clock (ms since start) and tick.
    pub fn frame(&mut self, stages: [u32; 5], now_ms: u64, tick: u64) {
        self.frames[self.n_frames % FRAMES] = stages;
        self.n_frames += 1;
        let total: u32 = stages.iter().sum();
        if total >= HITCH_US {
            let stage = stages.iter().enumerate().max_by_key(|s| s.1).map_or(0, |s| s.0) as u8;
            self.hitches[self.n_hitches % 8] = Hitch { total_us: total, stage, tick };
            self.n_hitches += 1;
        }
        self.fps_count += 1;
        if now_ms >= self.fps_since + 1000 {
            self.fps = (u64::from(self.fps_count) * 1000 / (now_ms - self.fps_since).max(1)) as u32;
            self.fps_count = 0;
            self.fps_since = now_ms;
        }
    }

    /// One frame's sim ticks: their time, µs.
    pub fn tick(&mut self, us: u32) {
        self.ticks[self.n_ticks % FRAMES] = us;
        self.n_ticks += 1;
    }

    /// The last `k` frames' stages, oldest first.
    fn recent(&self) -> impl Iterator<Item = &[u32; 5]> {
        let k = self.n_frames.min(FRAMES);
        (self.n_frames - k..self.n_frames).map(move |i| &self.frames[i % FRAMES])
    }

    fn totals(&self, out: &mut [u32; FRAMES]) -> usize {
        let mut n = 0;
        for f in self.recent() {
            out[n] = f.iter().sum();
            n += 1;
        }
        n
    }

    /// The whole frame's p50 and p99 over the window, and the last frame, µs.
    pub fn frame_times(&self) -> (u32, u32, u32) {
        let mut t = [0u32; FRAMES];
        let n = self.totals(&mut t);
        let last = if n == 0 { 0 } else { t[n - 1] };
        (percentile(&t[..n], 50), percentile(&t[..n], 99), last)
    }

    /// The sim tick's p50, p99 and max over the window, µs.
    pub fn tick_times(&self) -> (u32, u32, u32) {
        let n = self.n_ticks.min(FRAMES);
        let v = &self.ticks[..n];
        (percentile(v, 50), percentile(v, 99), v.iter().copied().max().unwrap_or(0))
    }

    /// The hitches kept, newest first.
    pub fn hitches(&self) -> impl Iterator<Item = &Hitch> {
        let k = self.n_hitches.min(8);
        (0..k).map(move |i| &self.hitches[(self.n_hitches - 1 - i) % 8])
    }
}

/// The sim's own numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimInfo {
    pub units_awake: u32,
    pub units_total: u32,
    pub path_searches_per_tick: u32,
    pub path_nodes_per_search: u32,
    pub events_per_tick: u32,
    /// The whole step, µs (0 without a wall clock).
    pub step_us: u32,
    /// Per-phase µs from `Sim::metrics()`, grouped by [`PHASE_SLOTS`], when the sim has a wall
    /// clock to time them by.
    pub phases: Option<[(&'static str, u32); 8]>,
}

/// The overlay's eight rows of the step's fifteen phases (`jane_sim::Phase`): each a name and the
/// phases it sums. The commands (phase 0) go with the clock, the head of the step.
pub const PHASE_SLOTS: [(&str, &[Phase]); 8] = [
    ("clock", &[Phase::Commands, Phase::Clock]),
    ("presence", &[Phase::Presence, Phase::Ring]),
    ("players", &[Phase::CatchUp, Phase::Players]),
    ("control", &[Phase::Controllers]),
    ("combat", &[Phase::Projectiles, Phase::Statuses, Phase::Flush]),
    ("triggers", &[Phase::Triggers]),
    ("housekeep", &[Phase::Housekeeping]),
    ("ops/drop", &[Phase::ZoneOps, Phase::Travel, Phase::Drop]),
];

/// Ticks of `Sim::metrics()` the overlay means over: a second.
const TALLY_TICKS: u32 = 60;

/// `Sim::metrics()` step by step, meant over a second for the overlay: one step's numbers
/// jitter too much to read. The units are the last step's. Presentation only, like the metrics.
#[derive(Clone, Copy, Debug, Default)]
pub struct SimTally {
    last: SimMetrics,
    n: u32,
    step_ns: u64,
    phase_ns: [u64; PHASES],
    searches: u64,
    expanded: u64,
    events: u64,
    /// The last full second's means, once there has been one.
    shown: Option<SimInfo>,
}

impl SimTally {
    /// One step's metrics.
    pub fn add(&mut self, m: &SimMetrics) {
        self.last = *m;
        self.n += 1;
        self.step_ns += u64::from(m.step_ns);
        for (sum, &t) in self.phase_ns.iter_mut().zip(&m.phase_ns) {
            *sum += u64::from(t);
        }
        self.searches += u64::from(m.path_searches);
        self.expanded += u64::from(m.path_expanded);
        self.events += u64::from(m.events);
        if self.n >= TALLY_TICKS {
            self.shown = Some(self.mean());
            *self = SimTally { last: self.last, shown: self.shown, ..SimTally::default() };
        }
    }

    /// The means over the steps summed so far.
    fn mean(&self) -> SimInfo {
        let n = u64::from(self.n.max(1));
        let us = |ns: u64| (ns / n / 1000) as u32;
        let phases = (self.step_ns > 0)
            .then(|| PHASE_SLOTS.map(|(name, ph)| (name, us(ph.iter().map(|p| self.phase_ns[p.index()]).sum()))));
        SimInfo {
            path_searches_per_tick: (self.searches / n) as u32,
            path_nodes_per_search: (self.expanded / self.searches.max(1)) as u32,
            events_per_tick: (self.events / n) as u32,
            step_us: us(self.step_ns),
            phases,
            ..SimInfo::default()
        }
    }

    /// What the overlay shows: the last second's means (the steps so far before there is one)
    /// and the last step's units.
    pub fn info(&self) -> SimInfo {
        let mut i = self.shown.unwrap_or_else(|| self.mean());
        i.units_awake = self.last.units_awake;
        i.units_total = self.last.units_total;
        i
    }
}

/// The presenter's and the frame's counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameInfo {
    pub sprites: u32,
    pub lights: u32,
    pub casters: u32,
    pub chunks_live: u32,
    pub chunks_painted: u32,
    pub ui_cmds: u32,
    pub atlas_pages: u32,
    /// The passes the frame holds, by name.
    pub passes: [Option<&'static str>; 8],
}

/// The line across the top while F2 or F3 is up.
#[derive(Clone, Copy, Debug, Default)]
pub struct TopLine<'a> {
    pub seed: u32,
    pub zone: &'a str,
    pub tick: u32,
    pub clock: &'a str,
    pub hash: u64,
    /// "x0.25", "x4", "held", or empty at 1x.
    pub speed: &'a str,
}

/// Everything F2 reads.
#[derive(Clone, Copy, Debug)]
pub struct PerfView<'a> {
    pub log: &'a PerfLog,
    pub backend: Option<FrameStats>,
    /// `soft`, `wgpu, Vulkan, <adapter>`.
    pub backend_name: &'a str,
    pub tier: Tier,
    pub dropped: u64,
    pub sim: SimInfo,
    pub frame: FrameInfo,
    pub top: TopLine<'a>,
}

/// Stage colours: each a ramp's light tone, told apart by hue.
fn stage_ix(k: usize) -> Ix {
    match k {
        0 => Ramp::Sky.at(Tone::Light),
        1 => Ramp::Grass.at(Tone::Light),
        2 => Ramp::UiGold.at(Tone::Light),
        3 => Ramp::Bloom.at(Tone::Light),
        _ => Ramp::UiInk.at(Tone::Shade),
    }
}

/// Green under budget, amber near it, red over.
fn judge(us: u32, budget: u32) -> Ix {
    if us <= budget * 3 / 4 {
        style::good()
    } else if us <= budget {
        style::warn()
    } else {
        style::bad()
    }
}

/// `us` as "4.21" ms into `buf`.
fn ms(us: u32, buf: &mut String) -> &str {
    buf.clear();
    let _ = write!(buf, "{}.{:02}", us / 1000, us % 1000 / 10);
    buf
}

/// The top line while an overlay is up: seed, zone, tick, clock, hash.
pub fn top_line(ui: &mut Ui, t: &TopLine<'_>) {
    let (cw, _) = ui.canvas;
    ui.fill(Rect::new(0, 0, cw, 13), argb(Ramp::UiSlot.at(Tone::Deep), 215));
    ui.fill(Rect::new(0, 13, cw, 1), argb(style::gold_deep(), 120));
    let mut s = String::with_capacity(96);
    let _ = write!(s, "seed {}  {}  tick {}  {}  hash {:016x}", t.seed, t.zone, t.tick, t.clock, t.hash);
    ui.text(6, 1, &s, Ink::fine(style::text()));
    if !t.speed.is_empty() {
        let w = text_w(Face::Fine, t.speed);
        ui.fill(Rect::new(cw - w - 12, 1, w + 8, 11), argb(style::warn(), 220));
        ui.text(cw - w - 8, 1, t.speed, Ink::fine(style::INK));
    }
}

/// The passes in force as lines of at most `cols` characters, each handed to `out`.
fn wrap_passes(f: &FrameInfo, cols: usize, line: &mut String, out: &mut dyn FnMut(&str)) {
    line.clear();
    line.push_str(" passes");
    for p in f.passes.iter().flatten() {
        if line.len() + 1 + p.len() > cols {
            out(line);
            line.clear();
            line.push_str("       ");
        }
        line.push(' ');
        line.push_str(p);
    }
    out(line);
}

/// The rows the full view's sim column takes: the sim's (four, and the phases' four or one), the
/// frame's, the passes' and the hitches'.
fn sim_rows(v: &PerfView<'_>, cols: usize, line: &mut String) -> i32 {
    let mut n = 4 + if v.sim.phases.is_some() { 4 } else { 1 };
    n += 4 + i32::from(v.backend.is_some());
    wrap_passes(&v.frame, cols, line, &mut |_| n += 1);
    n + 1 + (v.log.hitches().take(4).count() as i32).max(1)
}

/// Draws F2 at `level` (1 compact, 2 full).
pub fn draw(ui: &mut Ui, level: u8, v: &PerfView<'_>) {
    let (cw, _) = ui.canvas;
    let (p50, p99, last) = v.log.frame_times();
    let (tp50, tp99, tmax) = v.log.tick_times();
    let over = last > BUDGET_60 || p99 > BUDGET_60;
    let near = !over && p99 > BUDGET_60 * 3 / 4;
    let mut buf = String::with_capacity(16);
    let mut line = String::with_capacity(64);

    // Compact: always, top right under the top line.
    let (w, h) = (196, 66);
    let x = cw - w - 6;
    // Compact: bottom right, clear of the HUD's plates and the bar. Full: top right, the graphs
    // and tables to its left and below.
    let y = if level >= 2 { 18 } else { ui.canvas.1 - h - 34 };
    ui.panel(Rect::new(x, y, w, h), PanelStyle::Debug);
    if over || near {
        let c = argb(if over { style::bad() } else { style::warn() }, 230);
        ui.fill(Rect::new(x + 2, y, w - 4, 1), c);
        ui.fill(Rect::new(x + 2, y + h - 1, w - 4, 1), c);
        ui.fill(Rect::new(x, y + 2, 1, h - 4), c);
        ui.fill(Rect::new(x + w - 1, y + 2, 1, h - 4), c);
    }
    let lh = line_h(Face::Fine);
    let fps = v.log.fps;
    let fps_ink = if fps >= 58 {
        style::good()
    } else if fps >= 50 {
        style::warn()
    } else {
        style::bad()
    };
    let mut fb = [0u8; 8];
    ui.text(x + 8, y + 5, fmt_u32(fps, &mut fb), Ink::small(fps_ink).shadow());
    ui.text(x + 8 + text_w(Face::Small, fmt_u32(fps, &mut fb)) + 4, y + 10, "fps", Ink::fine(style::quiet()));
    line.clear();
    let _ = write!(line, "{} ms", ms(last, &mut buf));
    ui.text_right(x + w - 8, y + 5, &line, Ink::fine(judge(last, BUDGET_60)).shadow());
    line.clear();
    let _ = write!(line, "p50 {}", ms(p50, &mut buf));
    let _ = write!(line, "  p99 {}", ms(p99, &mut buf));
    ui.text(x + 8, y + 9 + lh, &line, Ink::fine(judge(p99, BUDGET_60)).shadow());
    line.clear();
    let _ = write!(line, "tick {tp50} us");
    ui.text(x + 8, y + 10 + 2 * lh, &line, Ink::fine(judge(tp50, 4000)).shadow());
    if v.dropped > 0 {
        line.clear();
        let _ = write!(line, "{} dropped", v.dropped);
        ui.text_right(x + w - 8, y + 10 + 2 * lh, &line, Ink::fine(style::bad()).shadow());
    }
    line.clear();
    let tier = match v.tier {
        Tier::T0 => "T0",
        Tier::T1 => "T1",
        Tier::T2 => "T2",
    };
    let name: String = v.backend_name.chars().take(22).collect();
    let _ = write!(line, "{tier} {name}");
    ui.text(x + 8, y + 10 + 3 * lh, &line, Ink::fine(style::quiet()).shadow());
    if level < 2 {
        return;
    }

    // Full: the graph panel left of the compact box, the tables under it.
    let gw = FRAMES as i32 + 16;
    let gx = cw - gw - 6 - w - 6;
    let gy = 18;
    let gh = 104;
    ui.panel(Rect::new(gx, gy, gw, gh), PanelStyle::Debug);
    let base = gy + gh - 18;
    let scale = |us: u32| (us as i32 * 70 / 33_333).min(80);
    // The bars, oldest on the left.
    let n = v.log.n_frames.min(FRAMES);
    for (k, f) in v.log.recent().enumerate() {
        let bx = gx + 8 + (FRAMES - n + k) as i32;
        let mut y0 = base;
        for (s, &us) in f.iter().enumerate() {
            let hgt = scale(us).max(i32::from(us > 0));
            if hgt > 0 {
                ui.fill(Rect::new(bx, y0 - hgt, 1, hgt), argb(stage_ix(s), if s == 4 { 150 } else { 235 }));
                y0 -= hgt;
            }
        }
        let total: u32 = f.iter().sum();
        if total >= HITCH_US {
            ui.fill(Rect::new(bx, base - 82, 1, 3), argb(style::bad(), 255));
        }
    }
    for (budget, label) in [(BUDGET_60, "16.7"), (BUDGET_120, "8.3")] {
        let by = base - scale(budget);
        for xx in (gx + 8..gx + 8 + FRAMES as i32).step_by(4) {
            ui.fill(Rect::new(xx, by, 2, 1), argb(if budget == BUDGET_60 { style::bad() } else { style::warn() }, 170));
        }
        ui.text(gx + 10, by - 10, label, Ink::fine(style::quiet()));
    }
    ui.fill(Rect::new(gx + 8, base, FRAMES as i32, 1), argb(style::dim(), 200));
    // The legend under the bars.
    let mut lx = gx + 8;
    for (s, name) in STAGES.iter().enumerate() {
        ui.fill(Rect::new(lx, base + 5, 6, 6), argb(stage_ix(s), 255));
        ui.text(lx + 8, base + 3, name, Ink::fine(style::quiet()));
        lx += 8 + text_w(Face::Fine, name) + 6;
    }
    // The sim's tick, small, beside the frame graph's label row.
    let sy = gy + gh + 4;
    let sh = 40;
    ui.panel(Rect::new(gx, sy, gw, sh), PanelStyle::Debug);
    let tick_base = sy + sh - 6;
    let tn = v.log.n_ticks.min(FRAMES);
    // Scaled to the window's worst, at least a millisecond, so a quiet sim still shows its shape.
    let top_us = tmax.max(1000) as i32;
    for i in 0..tn {
        let us = v.log.ticks[(v.log.n_ticks - tn + i) % FRAMES];
        let hgt = (us as i32 * 22 / top_us).clamp(1, 22);
        ui.fill(Rect::new(gx + 8 + (FRAMES - tn) as i32 + i as i32, tick_base - hgt, 1, hgt), argb(stage_ix(0), 220));
    }
    line.clear();
    let _ = write!(line, "sim tick us  p50 {tp50}  p99 {tp99}  max {tmax}");
    ui.text(gx + 8, sy + 3, &line, Ink::fine(style::text()).shadow());

    // Per pass: the backend's own clock.
    let ty = sy + sh + 4;
    let tw = gw + w + 6;
    let cwid = tw - 200;
    let cols = ((cwid - 16) / crate::ui::core::advance(Face::Fine)) as usize;
    // Tall enough for the pass table and for the sim's column beside it, whichever is longer.
    let th = (StatPass::COUNT as i32 * lh + 26).max(sim_rows(v, cols, &mut line) * lh + 8);
    let tx = gx;
    ui.panel(Rect::new(tx, ty, 194, th), PanelStyle::Debug);
    let clock = match v.backend {
        Some(s) if s.gpu_clock => "gpu",
        Some(_) => "cpu",
        None => "n/a",
    };
    line.clear();
    let _ = write!(line, "pass ({clock})");
    ui.text(tx + 8, ty + 4, &line, Ink::fine(style::gold()).shadow());
    ui.text_right(tx + 130, ty + 4, "p50", Ink::fine(style::gold()).shadow());
    ui.text_right(tx + 186, ty + 4, "p99", Ink::fine(style::gold()).shadow());
    for (i, p) in StatPass::ALL.iter().enumerate() {
        let ry = ty + 18 + i as i32 * lh;
        ui.text(tx + 8, ry, p.name(), Ink::fine(style::quiet()));
        let (a, b) = v.backend.map_or((0, 0), |s| (s.pass_p50_us[i], s.pass_p99_us[i]));
        if a == 0 && b == 0 {
            ui.text_right(tx + 186, ry, "-       -", Ink::fine(style::dim()));
        } else {
            ui.text_right(tx + 130, ry, ms(a, &mut buf), Ink::fine(judge(a, 4000)));
            ui.text_right(tx + 186, ry, ms(b, &mut buf), Ink::fine(judge(b, 4000)));
        }
    }

    // The sim, the frame, the hitches.
    let cx = tx + 200;
    ui.panel(Rect::new(cx, ty, cwid, th), PanelStyle::Debug);
    let mut row = 0;
    let mut put = |ui: &mut Ui, s: &str, ink: Ix| {
        ui.text(cx + 8, ty + 4 + row * lh, s, Ink::fine(ink).shadow());
        row += 1;
    };
    line.clear();
    let _ = write!(line, "sim  step {} us  (phases, us)", v.sim.step_us);
    put(ui, if v.sim.phases.is_some() { &line } else { "sim" }, style::gold());
    line.clear();
    let _ = write!(line, " units {} awake of {}", v.sim.units_awake, v.sim.units_total);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " paths {}/tick, {} nodes each", v.sim.path_searches_per_tick, v.sim.path_nodes_per_search);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " events {}/tick", v.sim.events_per_tick);
    put(ui, &line, style::text());
    match v.sim.phases {
        // Two to a row, in the step's order down the left and then the right.
        Some(ph) => {
            for k in 0..4 {
                line.clear();
                for (name, us) in [ph[k], ph[k + 4]] {
                    let _ = write!(line, "  {name:<9}{us:>4}");
                }
                put(ui, &line, style::quiet());
            }
        }
        None => put(ui, " phases n/a (no wall clock)", style::dim()),
    }
    put(ui, "frame", style::gold());
    let f = &v.frame;
    line.clear();
    let _ = write!(line, " list {}  lights {}  casters {}", f.sprites, f.lights, f.casters);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " chunks {} live, {} painted", f.chunks_live, f.chunks_painted);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " ui {}  atlas {} pages", f.ui_cmds, f.atlas_pages);
    put(ui, &line, style::text());
    if let Some(s) = v.backend {
        line.clear();
        if s.pixels_written > 0 {
            let _ = write!(line, " draws {}  px {}k", s.draw_calls, s.pixels_written / 1000);
        } else {
            let _ = write!(line, " draws {}", s.draw_calls);
        }
        put(ui, &line, style::text());
    }
    wrap_passes(f, cols, &mut line, &mut |s| put(ui, s, style::quiet()));
    put(ui, "hitches over 20 ms", style::gold());
    let mut any = false;
    for hch in v.log.hitches().take(4) {
        any = true;
        line.clear();
        let _ =
            write!(line, " {} ms  {}  tick {}", ms(hch.total_us, &mut buf), STAGES[usize::from(hch.stage)], hch.tick);
        put(ui, &line, style::bad());
    }
    if !any {
        put(ui, " none", style::good());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_keeps_the_window_and_finds_the_hitches() {
        let mut log = PerfLog::default();
        for i in 0..300u32 {
            let slow = if i == 250 { 30_000 } else { 0 };
            log.frame([1000, 500, 800 + slow, 2000, 12_000], u64::from(i) * 16, u64::from(i));
            log.tick(900 + i);
        }
        let (p50, p99, last) = log.frame_times();
        assert_eq!(last, 16_300);
        assert_eq!(p50, 16_300);
        assert!(p99 >= 16_300);
        let h: Vec<&Hitch> = log.hitches().collect();
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].stage, h[0].tick), (2, 250), "the build stage, at tick 250");
        let (tp50, _, tmax) = log.tick_times();
        assert_eq!(tmax, 900 + 299);
        assert!(tp50 > 900 + 60);
        assert!(log.fps > 0);
    }

    #[test]
    fn the_tally_means_a_second_of_metrics_into_the_eight_rows() {
        assert_eq!(PHASE_SLOTS.iter().map(|s| s.1.len()).sum::<usize>(), PHASES, "every phase in one row");
        let mut seen = [false; PHASES];
        for (_, ph) in PHASE_SLOTS {
            for p in ph {
                assert!(!seen[p.index()], "{} twice", p.name());
                seen[p.index()] = true;
            }
        }
        let mut t = SimTally::default();
        assert_eq!(t.info().phases, None);
        let mut m = SimMetrics {
            units_awake: 5,
            units_total: 9,
            path_searches: 2,
            path_expanded: 40,
            events: 3,
            ..SimMetrics::default()
        };
        t.add(&m);
        let i = t.info();
        assert_eq!((i.units_awake, i.units_total, i.path_searches_per_tick, i.path_nodes_per_search), (5, 9, 2, 20));
        assert_eq!((i.events_per_tick, i.phases), (3, None), "no clock, no times");
        m.step_ns = 150_000;
        m.phase_ns[Phase::Controllers.index()] = 100_000;
        m.phase_ns[Phase::Statuses.index()] = 20_000;
        m.phase_ns[Phase::Flush.index()] = 10_000;
        let mut t = SimTally::default();
        for _ in 0..TALLY_TICKS {
            t.add(&m);
        }
        let ph = t.info().phases.expect("timed");
        assert_eq!(ph[3], ("control", 100));
        assert_eq!(ph[4], ("combat", 30));
        assert_eq!(t.info().step_us, 150);
    }
}
