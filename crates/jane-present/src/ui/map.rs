//! The map tab (PRESENTATION.md §3.5): the TS chart, carried whole on `u32` buffers. A rough
//! chart, not a photograph: outdoors one chart px is a 4 x 4 block of cells inked with the most
//! telling thing in it (weighted: ground 1, water 2.5, road and rail 4, roofs 6, a solid prop of
//! 20 cells or more counting as roof), the ground inks smoothed by a 3 x 3 majority, woods
//! stippled, roads, water and roofs never smoothed away. Indoors each block is its commonest
//! tile. Unseen ground is a drifting four-step smoke outdoors and nothing indoors.
//!
//! `terrain` is painted once a zone; `composed` again only when the fog's seen-bits change
//! (looked at every 30 ticks). Zoom is `[fit, 1, 2, 3, 4, 6]`, nearest; drag or the stick pans;
//! `0` recentres. The School has its mark; she is a blinking dot. Never lit.
//!
//! Over the chart, what the map remembers (`crate::memory`), inked only at a rest: the fires she
//! rested at as warm dots (a made fire with its burn as an arc, a cold pit as a grey ring), the
//! signs she read as little posts whose words come up when the pointer is on one (or the keys
//! bring the view's middle to it), the places' names, and her own pins: a right click (or X, at
//! the view's middle) puts one in or takes it out, five at most.

use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_core::{Tile, ZoneId};
use jane_sim::View;

use crate::memory::{FireState, MapMemory, Note, PINS};

use crate::frame::Src;
use crate::input::{UiAction, sc};
use crate::ui::art::Mark;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui};
use crate::ui::style::{self, argb};

/// The UI image slot the chart is composed into.
pub const CHART: u16 = 2;
/// Outdoors, cells on a side of one chart px.
pub const OUT_STEP: u32 = 4;
/// Indoors the chart is finer, but never wider or taller than this.
const MAX_CHART_PX: u32 = 600;
/// How often the fog is looked at, ticks.
pub const RECOMPOSE_TICKS: u32 = 30;
/// Zooms after "fit": whole chart px per canvas px... per chart px, canvas px.
pub const ZOOMS: [i32; 5] = [1, 2, 3, 4, 6];

/// The chart's inks outdoors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum ChartInk {
    None,
    Meadow,
    Woods,
    Heath,
    Marsh,
    Farm,
    Sand,
    Rock,
    Water,
    Road,
    Rail,
    Roofs,
}

const INKS: usize = 12;

impl ChartInk {
    const fn area(self) -> bool {
        matches!(
            self,
            ChartInk::Meadow
                | ChartInk::Woods
                | ChartInk::Heath
                | ChartInk::Marsh
                | ChartInk::Farm
                | ChartInk::Sand
                | ChartInk::Rock
        )
    }

    /// Weight in halves (water's 2.5 is 5).
    const fn weight2(self) -> u32 {
        match self {
            ChartInk::None => 0,
            ChartInk::Water => 5,
            ChartInk::Road | ChartInk::Rail => 8,
            ChartInk::Roofs => 12,
            _ => 2,
        }
    }

    fn of(t: Tile) -> ChartInk {
        use Tile as T;
        match t {
            T::Void => ChartInk::None,
            T::Tree | T::Bush | T::Hedge => ChartInk::Woods,
            T::Dirt | T::DryBed | T::DeadTree => ChartInk::Heath,
            T::Track => ChartInk::Rail,
            T::Moss => ChartInk::Marsh,
            T::Crops | T::Garden | T::FlowerBed => ChartInk::Farm,
            T::Sand => ChartInk::Sand,
            T::Cliff | T::Rubble | T::Cobble | T::CaveWall | T::CaveFloor | T::Stepping => ChartInk::Rock,
            T::Water | T::Ice => ChartInk::Water,
            T::Road | T::Rail | T::Boardwalk | T::GrownPath => ChartInk::Road,
            T::HouseWall | T::HouseRoof | T::Eaves | T::StoneWall | T::Floor | T::FloorWood => ChartInk::Roofs,
            _ => ChartInk::Meadow,
        }
    }

    fn colour(self) -> Ix {
        match self {
            ChartInk::None => Ix::CLEAR,
            ChartInk::Meadow => Ramp::Grass.at(Tone::Mid),
            ChartInk::Woods => Ramp::ClothGreen.at(Tone::Shade),
            ChartInk::Heath => Ramp::Reed.at(Tone::Mid),
            ChartInk::Marsh => Ramp::ClothMoss.at(Tone::Shade),
            ChartInk::Farm => Ramp::ClothMustard.at(Tone::Mid),
            ChartInk::Sand => Ramp::Plaster.at(Tone::Base),
            ChartInk::Rock => Ramp::Stone.at(Tone::Mid),
            ChartInk::Water => Ramp::Water.at(Tone::Base),
            ChartInk::Road => Ramp::Plaster.at(Tone::Light),
            ChartInk::Rail => Ramp::ClothBlack.at(Tone::Shade),
            ChartInk::Roofs => Ramp::Brick.at(Tone::Mid),
        }
    }
}

fn pack(ix: Ix) -> u32 {
    if ix == Ix::CLEAR {
        return 0;
    }
    let [r, g, b] = palette::rgb(ix);
    0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

fn scale(c: u32, k256: u32) -> u32 {
    let ch = |s: u32| ((((c >> s) & 0xff) * k256 / 256).min(255)) << s;
    (c & 0xff00_0000) | ch(16) | ch(8) | ch(0)
}

/// A stable per-px hash, 0..255.
fn hash(x: u32, y: u32) -> u32 {
    let mut v = x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1);
    v = (v ^ (v >> 15)).wrapping_mul(0x85eb_ca6b);
    (v ^ (v >> 13)) & 255
}

/// Value noise, 0..=255, over a lattice `size` px apart, eased between the corners.
fn value_noise(x: u32, y: u32, size: u32, salt: u32) -> u32 {
    let (gx, gy) = (x / size, y / size);
    let (fx, fy) = ((x % size) * 256 / size, (y % size) * 256 / size);
    let ease = |f: u32| f * f * (768 - 2 * f) / 65536;
    let (ex, ey) = (ease(fx), ease(fy));
    let c = |i: u32, j: u32| hash(gx + i + salt, gy + j + salt.wrapping_mul(3));
    let top = c(0, 0) * (256 - ex) / 256 + c(1, 0) * ex / 256;
    let bot = c(0, 1) * (256 - ex) / 256 + c(1, 1) * ex / 256;
    top * (256 - ey) / 256 + bot * ey / 256
}

/// Smoke's shade at a chart px: 0..=4, soft banks of it, every px still one flat shade.
fn smoke(x: u32, y: u32) -> u32 {
    let v = (value_noise(x, y, 9, 17) * 65 + value_noise(x, y, 4, 91) * 35) / 100;
    (v.saturating_sub(51) * 4 / 153).min(4)
}

/// A chart of one zone.
#[derive(Clone, Debug, Default)]
pub struct MapChart {
    pub zone: Option<(ZoneId, u32)>,
    pub step: u32,
    pub w: u32,
    pub h: u32,
    pub indoor: bool,
    terrain: Vec<u32>,
    seen: Vec<u8>,
    composed: Vec<u32>,
    /// The tick the fog was last looked at, and whether anything changed.
    looked: u32,
    /// Where the School's door is, chart px.
    pub school: Option<(i32, i32)>,
    /// Zoom: 0 fit, then `ZOOMS[z - 1]`.
    pub zoom: usize,
    /// The chart px at the viewport's centre; `None` follows her.
    pub pan: Option<(i32, i32)>,
    drag_from: Option<((i32, i32), (i32, i32))>,
    /// Presses on the chart that put a pin in or take one out, in cells of the zone: the app
    /// hands them to [`MapMemory::toggle_pin`] after the frame.
    pub pin_edits: Vec<(ZoneId, (i32, i32))>,
}

impl MapChart {
    /// Paints the terrain for `v`'s zone if it is not the one painted.
    pub fn follow(&mut self, v: &View<'_>, tick: u32) -> bool {
        let key = (v.zone(), v.seed());
        if self.zone == Some(key) {
            return false;
        }
        self.zone = Some(key);
        let (zw, zh) = v.size();
        self.indoor = v.indoor();
        self.step = if self.indoor { zw.max(zh).div_ceil(MAX_CHART_PX).max(1) } else { OUT_STEP };
        self.w = zw.div_ceil(self.step);
        self.h = zh.div_ceil(self.step);
        self.paint(v);
        self.looked = tick.wrapping_sub(RECOMPOSE_TICKS);
        self.zoom = if self.indoor { 0 } else { 2 };
        self.pan = None;
        self.school = v
            .marks()
            .find(|(s, _)| v.name(*s).contains("school"))
            .map(|(_, m)| (i32::from(m.cell.x) / self.step as i32, i32::from(m.cell.y) / self.step as i32));
        true
    }

    fn paint(&mut self, v: &View<'_>) {
        let cat = jane_data::catalog();
        let (w, h, step) = (self.w, self.h, self.step);
        let (zw, zh) = v.size();
        let n = (w * h) as usize;
        self.terrain.clear();
        self.terrain.resize(n, 0);
        self.seen.clear();
        self.seen.resize(n, 0);
        self.composed.clear();
        self.composed.resize(n, 0);
        // Houses are props on grass, not tiles: their roofs are counted in by area.
        let mut roofs = vec![0u32; if self.indoor { 0 } else { n }];
        if !self.indoor {
            for p in v.props() {
                let d = cat.story.prop(p.def);
                if !(d.solid && u32::from(d.w) * u32::from(d.h) >= 20) {
                    continue;
                }
                let (x0, y0) = (u32::from(p.cell.x), u32::from(p.cell.y));
                for cy in y0..(y0 + u32::from(d.h)).min(zh) {
                    for cx in x0..(x0 + u32::from(d.w)).min(zw) {
                        roofs[((cy / step) * w + cx / step) as usize] += 1;
                    }
                }
            }
        }
        let mut ink = vec![ChartInk::None; n];
        let mut score = [0u32; 64];
        for my in 0..h {
            for mx in 0..w {
                score.fill(0);
                let (mut best, mut best_score) = (0usize, 0u32);
                for cy in my * step..((my + 1) * step).min(zh) {
                    for cx in mx * step..((mx + 1) * step).min(zw) {
                        let t = v.tile(cx as i32, cy as i32);
                        if t == Tile::Void {
                            continue;
                        }
                        let (k, wgt) = if self.indoor {
                            (usize::from(t.id()) % 64, 1)
                        } else {
                            let i = ChartInk::of(t);
                            (i as usize, i.weight2())
                        };
                        score[k] += wgt;
                        if score[k] > best_score {
                            (best, best_score) = (k, score[k]);
                        }
                    }
                }
                let i = (my * w + mx) as usize;
                if self.indoor {
                    if best_score > 0 {
                        let t = Tile::from_id(best as u8).unwrap_or(Tile::Floor);
                        let [r, g, b] = crate::stand_in::tile_rgb(t);
                        self.terrain[i] = chart_colour(r, g, b);
                    }
                } else {
                    let r = roofs[i] * ChartInk::Roofs.weight2();
                    ink[i] = if best != 0 && r > best_score { ChartInk::Roofs } else { INKS_ALL[best] };
                }
            }
        }
        if self.indoor {
            return;
        }
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut k = ink[i];
                if k.area() {
                    let mut counts = [0u8; INKS];
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                            if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                                continue;
                            }
                            let o = ink[(yy as u32 * w + xx as u32) as usize];
                            if o.area() {
                                counts[o as usize] += 1;
                            }
                        }
                    }
                    let mut most = counts[k as usize];
                    for c in INKS_ALL.iter().filter(|c| c.area()) {
                        if counts[*c as usize] > most {
                            most = counts[*c as usize];
                            k = *c;
                        }
                    }
                }
                if k == ChartInk::None {
                    continue;
                }
                let mut c = pack(k.colour());
                let hs = hash(x, y);
                if k == ChartInk::Woods {
                    c = scale(
                        c,
                        if hs < 70 {
                            200
                        } else if hs > 230 {
                            287
                        } else {
                            256
                        },
                    );
                } else if k.area() && hs < 26 {
                    c = scale(c, 230);
                }
                self.terrain[i] = c;
            }
        }
    }

    /// Folds the fog in again if it changed; `true` when the image needs uploading.
    pub fn recompose(&mut self, v: &View<'_>, tick: u32) -> bool {
        if tick.wrapping_sub(self.looked) < RECOMPOSE_TICKS {
            return false;
        }
        self.looked = tick;
        let (w, h, step) = (self.w, self.h, self.step);
        let (zw, zh) = v.size();
        let mut changed = false;
        // The seen fraction of each px, in quarters: where a fog block holds whole chart px
        // (outdoors, 8 cells to 4) one look answers for all four corners.
        let whole = v.fog_block() >= step;
        for my in 0..h {
            for mx in 0..w {
                let (x0, y0) = ((mx * step) as i32, (my * step) as i32);
                let s = step as i32 - 1;
                let n = if whole {
                    if v.seen(x0, y0) { 4 } else { 0 }
                } else {
                    let pts = [(x0, y0), (x0 + s, y0), (x0, y0 + s), (x0 + s, y0 + s)];
                    pts.iter().filter(|&&(x, y)| x < zw as i32 && y < zh as i32 && v.seen(x, y)).count() as u8
                };
                let i = (my * w + mx) as usize;
                if self.seen[i] != n {
                    self.seen[i] = n;
                    changed = true;
                }
            }
        }
        if !changed && !self.composed.iter().all(|&c| c == 0) {
            return false;
        }
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let t = self.terrain[i];
                if t == 0 {
                    self.composed[i] = 0;
                    continue;
                }
                if self.indoor {
                    self.composed[i] = if self.seen[i] > 0 { t } else { 0 };
                    continue;
                }
                // A 3 x 3 blur of the seen quarters, the centre counting double.
                let mut sum = u32::from(self.seen[i]) * 4;
                let mut wsum = 4;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                        if (dx, dy) == (0, 0) || xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                            continue;
                        }
                        let k = if dx == 0 || dy == 0 { 2 } else { 1 };
                        sum += u32::from(self.seen[(yy as u32 * w + xx as u32) as usize]) * k;
                        wsum += k;
                    }
                }
                // Seen in 0..=64: max of the px's own and the blur.
                let own = u32::from(self.seen[i]) * 16;
                let v64 = own.max(sum * 16 / wsum);
                let d = hash(x, y) as i32 * 8 / 255 - 4;
                let vv = v64 as i32 + d;
                let lvl = if vv < 8 {
                    0
                } else if vv < 26 {
                    1
                } else if vv < 45 {
                    2
                } else {
                    3
                };
                let n = smoke(x, y);
                let lo = [18u32, 15, 26];
                let hi = [36u32, 31, 46];
                let sm = |c: usize| lo[c] + (hi[c] - lo[c]) * n / 4;
                let tc = |s: u32| (t >> s) & 0xff;
                let mixc = |s: u32, c: usize| (sm(c) as i32 + (tc(s) as i32 - sm(c) as i32) * lvl / 3) as u32;
                self.composed[i] = 0xff00_0000 | mixc(16, 0) << 16 | mixc(8, 1) << 8 | mixc(0, 2);
            }
        }
        true
    }

    /// Copies the composed chart into `out` (`w * h`).
    pub fn write(&self, out: &mut [u32]) {
        out.copy_from_slice(&self.composed);
    }
}

const INKS_ALL: [ChartInk; INKS] = [
    ChartInk::None,
    ChartInk::Meadow,
    ChartInk::Woods,
    ChartInk::Heath,
    ChartInk::Marsh,
    ChartInk::Farm,
    ChartInk::Sand,
    ChartInk::Rock,
    ChartInk::Water,
    ChartInk::Road,
    ChartInk::Rail,
    ChartInk::Roofs,
];

/// A tile colour as a kept chart inks it: a little greyed, a little warmed, cut to a coarse ramp.
pub fn chart_colour(r: u8, g: u8, b: u8) -> u32 {
    let (r, g, b) = (u32::from(r), u32::from(g), u32::from(b));
    let grey = (r * 3 + g * 4 + b) / 8;
    let q = |v: u32, warm: u32| ((v * 72 + grey * 18 + warm * 100) / 100 / 12 * 12).min(255);
    0xff00_0000 | q(r, 14) << 16 | q(g, 9) << 8 | q(b, 0)
}

/// Draws the chart in `r` for `v`, answering zoom, pan and recentre while `ui` is interactive.
pub fn draw(ui: &mut Ui, m: &mut MapChart, mem: &MapMemory, v: &View<'_>, r: Rect) {
    let tick = ui.tick;
    if m.follow(v, tick) {
        let img = ui.image_mut(CHART, m.w as u16, m.h as u16);
        img.argb.fill(0);
        ui.image_changed(CHART);
    }
    if m.recompose(v, tick) {
        let img = ui.image_mut(CHART, m.w as u16, m.h as u16);
        m.write(&mut img.argb);
        ui.image_changed(CHART);
    }
    // Her place on the chart.
    let body = v.body();
    let step = m.step as i32;
    let her = (body.pos.cell().0 / step, body.pos.cell().1 / step);
    let (rw, rh) = (i32::from(r.w), i32::from(r.h));
    // Zoom: fit is the whole chart in the box; the rest are whole px per chart px.
    let fit = |len: i32, box_: i32| (box_ * 256 / len.max(1)).max(1);
    let z256 = if m.zoom == 0 { fit(m.w as i32, rw).min(fit(m.h as i32, rh)) } else { ZOOMS[m.zoom - 1] * 256 };
    if ui.interactive {
        let mut dz = 0i32;
        if ui.hover(r) {
            dz += ui.input.wheel.signum();
        }
        if ui.input.keys.has(sc::N0) {
            m.pan = None;
        }
        if ui.input.keys.has(46) || ui.input.keys.has(87) {
            dz += 1;
        }
        if ui.input.keys.has(45) || ui.input.keys.has(86) {
            dz -= 1;
        }
        m.zoom = (m.zoom as i32 + dz).clamp(0, ZOOMS.len() as i32) as usize;
        // Drag to pan.
        if let Some(p) = ui.input.pointer {
            if ui.input.pressed && r.contains(p) {
                m.drag_from = Some((p, m.pan.unwrap_or(her)));
            }
            if let Some((from, start)) = m.drag_from {
                if ui.input.held {
                    let k = z256.max(1);
                    m.pan = Some((start.0 - (p.0 - from.0) * 256 / k, start.1 - (p.1 - from.1) * 256 / k));
                } else {
                    m.drag_from = None;
                }
            }
        }
        // The arrow keys and the stick pan too.
        let step_px = (24 * 256 / z256.max(1)).max(1);
        for a in &ui.input.actions {
            let c = m.pan.unwrap_or(her);
            m.pan = match a {
                UiAction::Left => Some((c.0 - step_px, c.1)),
                UiAction::Right => Some((c.0 + step_px, c.1)),
                UiAction::Up => Some((c.0, c.1 - step_px)),
                UiAction::Down => Some((c.0, c.1 + step_px)),
                _ => m.pan,
            };
        }
    }
    let z256 = if m.zoom == 0 { fit(m.w as i32, rw).min(fit(m.h as i32, rh)) } else { ZOOMS[m.zoom - 1] * 256 };
    // The window onto the chart: centred on the pan (or on her), held inside the chart at fit.
    let centre = if m.zoom == 0 { (m.w as i32 / 2, m.h as i32 / 2) } else { m.pan.unwrap_or(her) };
    let vis_w = (rw * 256 / z256).min(m.w as i32).max(1);
    let vis_h = (rh * 256 / z256).min(m.h as i32).max(1);
    let sx = (centre.0 - vis_w / 2).clamp(0, (m.w as i32 - vis_w).max(0));
    let sy = (centre.1 - vis_h / 2).clamp(0, (m.h as i32 - vis_h).max(0));
    let dw = vis_w * z256 / 256;
    let dh = vis_h * z256 / 256;
    let (dx, dy) = (i32::from(r.x) + (rw - dw) / 2, i32::from(r.y) + (rh - dh) / 2);
    // The paper under the chart.
    ui.fill(r, argb(Ramp::UiSlot.at(Tone::Deep), 255));
    ui.set_clip(r);
    ui.image(
        CHART,
        Src { x: sx as u16, y: sy as u16, w: vis_w as u16, h: vis_h as u16 },
        Rect::new(dx, dy, dw, dh),
        255,
    );
    let to_canvas =
        |p: (i32, i32)| (dx + (p.0 - sx) * z256 / 256 + z256 / 512, dy + (p.1 - sy) * z256 / 256 + z256 / 512);
    if let Some(s) = m.school {
        let (x, y) = to_canvas(s);
        ui.mark(Mark::Star, x - 8, y - 8, 255);
    }
    let read = marks(ui, m, mem, v, r, &to_canvas, Window { z256, sx, sy, dx, dy });
    // Her: a dot that blinks, ringed so it reads on any ink.
    let (hx, hy) = to_canvas(her);
    if (tick / 20) % 3 != 0 {
        ui.fill(Rect::new(hx - 3, hy - 3, 7, 7), argb(Ix::INK, 230));
        ui.fill(Rect::new(hx - 2, hy - 2, 5, 5), argb(style::gold(), 255));
        ui.fill(Rect::new(hx - 1, hy - 2, 2, 1), argb(Ramp::UiGold.at(Tone::Glint), 255));
    }
    ui.set_clip(Rect::CANVAS);
    if let Some((words, at)) = read {
        sign_words(ui, r, &words, at);
    }
    // The legend of the zoom, quiet under the chart.
    let zl = if m.zoom == 0 { "fit".to_string() } else { format!("x{}", ZOOMS[m.zoom - 1]) };
    let pin = if ui.input.pad { "X pin" } else { "right click pin" };
    let hint = format!("{zl}   wheel or +/- zoom   drag to pan   0 her   {pin} {}/{PINS}", mem.pins.len());
    ui.text(i32::from(r.x) + 6, r.bottom() + 4, &hint, Ink::fine(style::quiet()).shadow());
}

/// The chart's window onto the canvas: zoom (canvas px per chart px, in 256ths), the first
/// chart px in sight, and the canvas px it is drawn from.
#[derive(Clone, Copy)]
struct Window {
    z256: i32,
    sx: i32,
    sy: i32,
    dx: i32,
    dy: i32,
}

/// The ember's colour.
fn ember() -> u32 {
    argb(Ramp::Ember.at(Tone::Base), 255)
}

/// A mark this near the pointer (canvas px, each way) is the one it is on.
const POINT_NEAR: i32 = 6;

/// The pixels of a ring of radius 6 round a mark, in order from twelve o'clock, clockwise.
fn arc_ring() -> Vec<(i32, i32)> {
    let mut px: Vec<(i32, i32)> = Vec::new();
    for dy in -7i32..=7 {
        for dx in -7i32..=7 {
            if (30..=44).contains(&(dx * dx + dy * dy)) {
                px.push((dx, dy));
            }
        }
    }
    // A pseudo-angle, 0..1024 clockwise from straight up (screen y runs down).
    let ang = |(x, y): (i32, i32)| -> i32 {
        let (ax, ay) = (x.abs(), y.abs());
        let t = if ax + ay == 0 { 0 } else { ax * 256 / (ax + ay) };
        match (x >= 0, y < 0) {
            (true, true) => t,
            (true, false) => 512 - t,
            (false, false) => 512 + t,
            (false, true) => 1024 - t,
        }
    };
    px.sort_by_key(|&p| ang(p));
    px
}

/// The inked marks of her zone and her pins over the chart; the words of the sign the pointer
/// (or the view's middle) is on, and where.
fn marks(
    ui: &mut Ui,
    m: &mut MapChart,
    mem: &MapMemory,
    v: &View<'_>,
    r: Rect,
    to_canvas: &dyn Fn((i32, i32)) -> (i32, i32),
    win: Window,
) -> Option<(String, (i32, i32))> {
    let zone = v.zone();
    let step = m.step as i32;
    let chart = |c: (i32, i32)| (c.0 / step, c.1 / step);
    // Names first, under everything: each once, never over another.
    let mut taken: Vec<Rect> = Vec::new();
    for mk in mem.of(zone) {
        let Note::Name(name) = &mk.note else { continue };
        let (x, y) = to_canvas(chart(mk.at));
        let w = crate::ui::core::text_w(jane_art::font::Face::Fine, name);
        let b = Rect::new(x - w / 2 - 2, y - 4, w + 4, 11);
        if !r.contains((x, y)) || taken.iter().any(|t| !t.intersect(b).is_empty()) {
            continue;
        }
        taken.push(b);
        // A place she has been is inked bold; one she has only read of, fainter, over the smoke.
        let ink = if v.seen(mk.at.0, mk.at.1) {
            Ink::fine(Ramp::Bone.at(Tone::Light)).shadow()
        } else {
            Ink::fine(Ramp::Stone.at(Tone::Lift)).shadow().alpha(200)
        };
        ui.text(x - w / 2, y - 3, name, ink);
    }
    // Where the pointer is; else, when the keys have moved the view, its middle.
    let mid = (i32::from(r.x) + i32::from(r.w) / 2, i32::from(r.y) + i32::from(r.h) / 2);
    let at = ui.input.pointer.filter(|p| r.contains(*p)).or(m.pan.map(|_| mid));
    let mut read: Option<(String, (i32, i32), i32)> = None;
    let ring = arc_ring();
    for mk in mem.of(zone) {
        let (x, y) = to_canvas(chart(mk.at));
        if !r.contains((x, y)) {
            continue;
        }
        match &mk.note {
            Note::Fire(state) => {
                match *state {
                    FireState::Cold => {
                        let grey = argb(Ramp::Stone.at(Tone::Mid), 230);
                        ui.fill(Rect::new(x - 2, y - 3, 5, 1), grey);
                        ui.fill(Rect::new(x - 2, y + 3, 5, 1), grey);
                        ui.fill(Rect::new(x - 3, y - 2, 1, 5), grey);
                        ui.fill(Rect::new(x + 3, y - 2, 1, 5), grey);
                        ui.fill(Rect::new(x - 1, y - 1, 3, 3), argb(Ramp::Stone.at(Tone::Deep), 200));
                        continue;
                    }
                    FireState::Made { burn } => {
                        let n = ring.len() * usize::from(burn) / 255;
                        for (k, &(ax, ay)) in ring.iter().enumerate() {
                            let c = if k < n { ember() } else { argb(Ramp::Stone.at(Tone::Shade), 160) };
                            ui.fill(Rect::new(x + ax, y + ay, 1, 1), c);
                        }
                    }
                    FireState::Kept => {}
                }
                // A warm dot: a glow that breathes, an ink rim, the ember, its heart.
                let breathe = 36 + (ui.tick / 6 % 16).abs_diff(8) as u8 * 4;
                let glow = argb(Ramp::Ember.at(Tone::Light), breathe);
                ui.fill(Rect::new(x - 6, y - 3, 13, 7), glow);
                ui.fill(Rect::new(x - 3, y - 6, 7, 13), glow);
                ui.fill(Rect::new(x - 5, y - 2, 11, 5), glow);
                ui.fill(Rect::new(x - 2, y - 5, 5, 11), glow);
                ui.fill(Rect::new(x - 4, y - 2, 9, 5), argb(Ix::INK, 235));
                ui.fill(Rect::new(x - 2, y - 4, 5, 9), argb(Ix::INK, 235));
                ui.fill(Rect::new(x - 3, y - 3, 7, 7), argb(Ix::INK, 235));
                ui.fill(Rect::new(x - 3, y - 1, 7, 3), ember());
                ui.fill(Rect::new(x - 1, y - 3, 3, 7), ember());
                ui.fill(Rect::new(x - 2, y - 2, 5, 5), ember());
                ui.fill(Rect::new(x - 1, y - 1, 3, 3), argb(Ramp::Ember.at(Tone::Light), 255));
                ui.fill(Rect::new(x - 1, y - 2, 1, 1), argb(Ramp::Ember.at(Tone::Glint), 255));
                ui.fill(Rect::new(x, y - 1, 1, 1), argb(Ramp::Ember.at(Tone::Glint), 255));
            }
            Note::Sign(words) => {
                // A little post: an ink stake and a pale board, as the fingerposts stand.
                let lit = at.is_some_and(|p| (p.0 - x).abs() <= POINT_NEAR && (p.1 - y).abs() <= POINT_NEAR);
                let board = if lit { Ramp::UiGold.at(Tone::Light) } else { Ramp::WoodPale.at(Tone::Light) };
                ui.fill(Rect::new(x - 1, y - 2, 3, 8), argb(Ix::INK, 235));
                ui.fill(Rect::new(x - 4, y - 5, 9, 5), argb(Ix::INK, 235));
                ui.fill(Rect::new(x - 3, y - 4, 7, 3), argb(board, 255));
                ui.fill(Rect::new(x - 3, y - 2, 7, 1), argb(Ramp::WoodPale.at(Tone::Base), 255));
                ui.fill(Rect::new(x, y, 1, 5), argb(Ramp::WoodDark.at(Tone::Light), 255));
                if let Some(p) = at.filter(|_| lit) {
                    let d = (p.0 - x).pow(2) + (p.1 - y).pow(2);
                    if read.as_ref().is_none_or(|b| d < b.2) {
                        read = Some((words.clone(), (x, y), d));
                    }
                }
            }
            Note::Name(_) => {}
        }
    }
    // Her pins: a red head on an ink stem, its point on the place.
    for p in mem.pins.iter().filter(|p| p.zone == zone) {
        let (x, y) = to_canvas(chart(p.at));
        if !r.contains((x, y)) {
            continue;
        }
        // Its shadow on the paper, the steel, then the head: an ink rim, the red, a lit side.
        ui.fill(Rect::new(x + 1, y, 3, 1), argb(Ix::INK, 120));
        ui.fill(Rect::new(x - 1, y - 7, 3, 8), argb(Ix::INK, 240));
        ui.fill(Rect::new(x, y - 6, 1, 6), argb(Ramp::Iron.at(Tone::Light), 255));
        ui.fill(Rect::new(x - 4, y - 13, 9, 7), argb(Ix::INK, 245));
        ui.fill(Rect::new(x - 3, y - 14, 7, 9), argb(Ix::INK, 245));
        ui.fill(Rect::new(x - 3, y - 12, 7, 5), argb(Ramp::ClothRed.at(Tone::Base), 255));
        ui.fill(Rect::new(x - 2, y - 13, 5, 7), argb(Ramp::ClothRed.at(Tone::Base), 255));
        ui.fill(Rect::new(x + 1, y - 11, 2, 4), argb(Ramp::ClothRed.at(Tone::Shade), 255));
        ui.fill(Rect::new(x - 2, y - 12, 2, 2), argb(Ramp::ClothRed.at(Tone::Light), 255));
        ui.fill(Rect::new(x - 2, y - 12, 1, 1), argb(Ramp::ClothRed.at(Tone::Glint), 255));
    }
    // A pin put in or taken out: a right click where the pointer is, or X at the view's middle.
    if ui.interactive {
        let cell_of = |(px, py): (i32, i32)| {
            let cx = win.sx + (px - win.dx) * 256 / win.z256.max(1);
            let cy = win.sy + (py - win.dy) * 256 / win.z256.max(1);
            (cx * step + step / 2, cy * step + step / 2)
        };
        if ui.input.right_pressed
            && let Some(p) = ui.input.pointer.filter(|p| r.contains(*p))
        {
            m.pin_edits.push((zone, cell_of(p)));
        }
        if ui.input.has(UiAction::Quick) {
            m.pin_edits.push((zone, cell_of(mid)));
        }
    }
    read.map(|(w, at, _)| (w, at))
}

/// A sign's words, read again off the chart: a tip beside its post.
fn sign_words(ui: &mut Ui, r: Rect, words: &str, (x, y): (i32, i32)) {
    use crate::ui::core::{advance, line_h};
    let face = jane_art::font::Face::Fine;
    // As `Ui::wrapped` lays it: whole columns of the face's advance, a word never split.
    let len = words.chars().count() as i32;
    let w = (len * advance(face) + 14).clamp(60, 300);
    let cols = ((w - 12) / advance(face)).max(1);
    let mut lines = 1;
    let mut col = 0;
    for word in words.split(' ') {
        let n = word.chars().count() as i32;
        if col > 0 && col + 1 + n > cols {
            lines += 1;
            col = n;
        } else {
            col += n + i32::from(col > 0);
        }
    }
    let lines = (lines + words.matches('\n').count() as i32).clamp(1, 9);
    let h = lines * line_h(face) + 12;
    let right = i32::from(r.x) + i32::from(r.w);
    let tx = (x + 10).min(right - w - 2).max(i32::from(r.x) + 2);
    let ty = if y - h - 6 > i32::from(r.y) { y - h - 6 } else { y + 10 };
    ui.panel(Rect::new(tx, ty, w, h), crate::ui::core::PanelStyle::Tip);
    ui.wrapped(Rect::new(tx + 6, ty + 6, w - 12, h - 10), words, Ink::fine(style::text_bright()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_county_charts_at_four_cells_a_px_and_starts_in_smoke() {
        let sim = jane_sim::Sim::new_game(7, "Tess");
        let v = sim.view(jane_sim::Seat(0)).unwrap();
        let mut m = MapChart::default();
        assert!(m.follow(&v, 100));
        assert!(!m.follow(&v, 101), "painted once a zone");
        let (zw, zh) = v.size();
        assert_eq!((m.w, m.h), (zw.div_ceil(4), zh.div_ceil(4)));
        assert!(m.terrain.iter().filter(|&&c| c != 0).count() > (m.w * m.h) as usize / 2);
        assert!(m.recompose(&v, 200));
        // Where she stands is clear; far away is smoke, darker than any ink.
        let her = v.body().pos.cell();
        let i = ((her.1 as u32 / 4) * m.w + her.0 as u32 / 4) as usize;
        assert_eq!(m.composed[i], m.terrain[i], "her own block is clear");
        let far = ((m.h - 1) * m.w + (m.w - 1)) as usize;
        let luma = |c: u32| (c >> 16 & 0xff) + (c >> 8 & 0xff) + (c & 0xff);
        if m.terrain[far] != 0 && !v.seen(zw as i32 - 1, zh as i32 - 1) {
            assert!(luma(m.composed[far]) < 150, "unseen is smoke");
        }
        assert!(!m.recompose(&v, 210), "not again inside 30 ticks");
    }

    #[test]
    fn smoke_is_four_steps_and_the_chart_colour_is_coarse() {
        for x in 0..200 {
            assert!(smoke(x, x * 3) <= 4);
        }
        let c = chart_colour(118, 164, 84);
        assert_eq!((c >> 16 & 0xff) % 12, 0);
    }
}
