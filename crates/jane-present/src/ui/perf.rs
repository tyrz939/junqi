//! The F2 overlay (PRESENTATION.md §3.2, §1.12): off, compact, full.
//!
//! **Compact** is a small box top right: fps, the frame's ms, its p50 and p99, the sim's tick,
//! the backend and tier, dropped ticks; its border goes amber then red over budget.
//!
//! **Full** adds the frame-time graph (the last 240 frames as stacked bars by stage: sim step,
//! presenter tick, frame build and UI, backend draw, the wait for the display; budget lines at
//! 16.7 and 8.3 ms), a second graph of the sim's tick, the per-pass table from the backend's own
//! clock (`Backend::stats`), the sim's numbers, the presenter's and the backend's counts, the
//! last eight hitches over 20 ms with the stage that dominated each, and the passes in force.
//!
//! The app measures; this file keeps the rings and draws. Nothing here allocates per frame
//! after the first.

use std::fmt::Write as _;

use jane_art::font::Face;
use jane_art::palette::{Ix, Ramp, Tone};

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
    /// Events the sim emitted, per tick, over the same window.
    events: Box<[u16; FRAMES]>,
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
            events: Box::new([0; FRAMES]),
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

    /// One sim tick: its time, µs, and the events it emitted.
    pub fn tick(&mut self, us: u32, events: usize) {
        self.ticks[self.n_ticks % FRAMES] = us;
        self.events[self.n_ticks % FRAMES] = events.min(usize::from(u16::MAX)) as u16;
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

    /// Events a tick, the mean over the window.
    pub fn events_per_tick(&self) -> u32 {
        let n = self.n_ticks.min(FRAMES);
        if n == 0 {
            return 0;
        }
        (self.events[..n].iter().map(|&e| u32::from(e)).sum::<u32>()) / n as u32
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
    /// Per-phase µs from `Sim::metrics()`, when the sim reports them.
    pub phases: Option<[(&'static str, u32); 8]>,
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
    let rows = StatPass::COUNT as i32;
    let th = 20 + rows * lh + 6;
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
    let cwid = tw - 200;
    ui.panel(Rect::new(cx, ty, cwid, th), PanelStyle::Debug);
    let mut row = 0;
    let mut put = |ui: &mut Ui, s: &str, ink: Ix| {
        ui.text(cx + 8, ty + 4 + row * lh, s, Ink::fine(ink).shadow());
        row += 1;
    };
    put(ui, "sim", style::gold());
    line.clear();
    let _ = write!(line, " units {} awake of {}", v.sim.units_awake, v.sim.units_total);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " paths {}/tick, {} nodes each", v.sim.path_searches_per_tick, v.sim.path_nodes_per_search);
    put(ui, &line, style::text());
    line.clear();
    let _ = write!(line, " events {}/tick", v.log.events_per_tick());
    put(ui, &line, style::text());
    match v.sim.phases {
        Some(ph) => {
            for (name, us) in ph.iter().filter(|p| !p.0.is_empty()) {
                line.clear();
                let _ = write!(line, "  {name:<10} {us} us");
                put(ui, &line, style::quiet());
            }
        }
        None => put(ui, " phases n/a (Sim::metrics)", style::dim()),
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
    line.clear();
    line.push_str(" passes");
    let cols = ((cwid - 16) / crate::ui::core::advance(Face::Fine)) as usize;
    for p in f.passes.iter().flatten() {
        if line.len() + 1 + p.len() > cols {
            put(ui, &line, style::quiet());
            line.clear();
            line.push_str("       ");
        }
        line.push(' ');
        line.push_str(p);
    }
    put(ui, &line, style::quiet());
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
            log.tick(900 + i, 3);
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
        assert_eq!(log.events_per_tick(), 3);
        assert!(log.fps > 0);
    }
}
