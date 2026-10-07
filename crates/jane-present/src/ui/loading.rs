//! The loading screen (PRESENTATION.md §3.2), while New Game or Load builds the county on the
//! app's thread. It gives nothing of the county away.
//!
//! **The scroll** (the default): a train window at one o'clock, the country going past it
//! (hills, hedges and a cottage, telegraph poles, fog), none of it this seed's. Beneath it, one
//! line in the game's voice for each stage the build reports as it happens
//! (`jane_world::Report`, the words in `text::loading`), fading in at the foot and scrolling up
//! and away; a lantern at the corner fills with the stages done. When the county is built a last
//! line is said ("The train slows for Castle Halt.") and play starts. The screen holds
//! [`MIN_TICKS`] at the least so the first lines are read, and no longer than the build and the
//! last line need; once built, any press goes straight on.
//!
//! **The map** (`jane-app --loading map`, a developer's view): the county's skeleton forms on a
//! card: the land, then the river and the lake, then the roads and the railway, then the sites,
//! lamps and patches, then one mark per zone; a stage name in the Small face beneath. The card
//! is painted from the skeleton (`jane_world::skeleton`) the loader sends first, at 2x (a macro
//! cell is 2 x 2 px), each layer revealed by a sweep as its stage comes up.

use std::collections::VecDeque;

use jane_art::font::Face;
use jane_art::hash::h32;
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_world::skeleton::{Biome, ROAD, ROAD_BRIDGE, ROAD_LIT, SKEL_H, SKEL_W, Skeleton, Water};

use crate::frame::Src;
use crate::text::loading as words;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, PanelStyle, Ui, line_h, text_w};
use crate::ui::style::{self, argb};

/// The UI image slot the card is painted into: the map, or the train window's four layers.
pub const CARD: u16 = 1;
/// Px per macro cell on the card.
const SCALE: i32 = 2;
/// Ticks each stage takes to sweep in.
pub const STAGE_TICKS: u32 = 20;
/// The stages, in order.
pub const STAGES: [&str; 5] =
    ["The land", "The river and the lake", "Roads and the railway", "Sites, lamps and patches", "The zones"];

/// The layers of the card, painted once from the skeleton.
#[derive(Clone, Debug, Default)]
pub struct Card {
    pub w: i32,
    pub h: i32,
    /// Per stage, the colour it lays on each px (0: nothing).
    layers: [Vec<u32>; 4],
    /// Where each zone's mark goes, card px, in `ZoneId` order (the county's is its crown).
    zones: Vec<(i32, i32)>,
}

impl Card {
    pub fn from_skeleton(s: &Skeleton) -> Card {
        let (w, h) = (SKEL_W * SCALE, SKEL_H * SCALE);
        let n = (w * h) as usize;
        let mut c = Card { w, h, layers: [vec![0; n], vec![0; n], vec![0; n], vec![0; n]], zones: Vec::new() };
        let t = &s.terrain;
        let ink = |ix: Ix| {
            let [r, g, b] = palette::rgb(ix);
            0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
        };
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let ramp = match t.biome.read(x, y, Biome::Field) {
                    Biome::Field => Ramp::Grass,
                    Biome::Hedge | Biome::Garden => Ramp::Leaf,
                    Biome::Wood | Biome::WetWood => Ramp::ClothGreen,
                    Biome::Foothill | Biome::Hill => Ramp::Reed,
                    Biome::Reed | Biome::Marsh => Ramp::ClothMoss,
                    Biome::Slag | Biome::Yard => Ramp::Slate,
                    Biome::Town => Ramp::Plaster,
                };
                // Height as tone: low ground in shade, the crown lit.
                let ht = i32::from(t.height.read(x, y, 0));
                let lit = (x + y) & 1 == 0 && ht % 32 > 24;
                let tone = Tone::ALL[(1 + ht * 4 / 256 + i32::from(lit)).clamp(1, 5) as usize];
                let water = t.water.read(x, y, Water::Dry);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = ((y * SCALE + dy) * w + x * SCALE + dx) as usize;
                    c.layers[0][i] = ink(ramp.at(tone));
                    if water != Water::Dry {
                        let deep = if water == Water::Lake { Tone::Shade } else { Tone::Mid };
                        let sparkle = (x * 7 + y * 3 + dx) % 11 == 0;
                        c.layers[1][i] = ink(Ramp::Water.at(if sparkle { Tone::Light } else { deep }));
                    }
                }
            }
        }
        let mut dot = |layer: usize, x: i32, y: i32, v: u32| {
            if x >= 0 && y >= 0 && x < w && y < h {
                c.layers[layer][(y * w + x) as usize] = v;
            }
        };
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let bits = s.road.read(x, y, 0);
                if bits & ROAD == 0 {
                    continue;
                }
                let col = if bits & ROAD_BRIDGE != 0 {
                    ink(Ramp::WoodDark.at(Tone::Base))
                } else if bits & ROAD_LIT != 0 {
                    ink(Ramp::GlassLit.at(Tone::Light))
                } else {
                    ink(Ramp::WoodPale.at(Tone::Mid))
                };
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    dot(2, x * SCALE + dx, y * SCALE + dy, col);
                }
            }
        }
        for (k, &(x, y)) in s.rail.iter().enumerate() {
            let col = if k % 2 == 0 { ink(Ramp::Bone.at(Tone::Light)) } else { ink(Ix::INK) };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                dot(2, x * SCALE + dx, y * SCALE + dy, col);
            }
        }
        for p in &s.pois {
            let col =
                ink(if p.anchor.is_some() { Ramp::GlassLit.at(Tone::High) } else { Ramp::Plaster.at(Tone::High) });
            dot(3, p.mx * SCALE, p.my * SCALE, col);
            dot(3, p.mx * SCALE + 1, p.my * SCALE, col);
        }
        for a in &s.areas {
            let r = i32::from(a.def.radius) * SCALE / jane_world::skeleton::MACRO;
            let col = ink(Ramp::ClothRed.at(if a.def.threat >= 4 { Tone::High } else { Tone::Mid }));
            for k in 0..(r * 6).max(8) {
                // A dotted ring: every other step of a circle.
                if k % 2 == 1 {
                    continue;
                }
                let ang = jane_core::Angle((k * 65536 / (r * 6).max(8)) as u16);
                let (cx, cy) = (a.mx * SCALE + 1, a.my * SCALE + 1);
                let x = cx + jane_core::angle::cos_q15(ang).0 * r / 32768;
                let y = cy + jane_core::angle::sin_q15(ang).0 * r / 32768;
                dot(3, x, y, col);
            }
        }
        for site in &s.sites {
            let col = ink(if site.def.dungeon { Ramp::ClothRed.at(Tone::High) } else { Ramp::UiGold.at(Tone::Light) });
            let (x, y) = (site.mx * SCALE, site.my * SCALE);
            for (dx, dy) in [
                (0, -2),
                (-1, -1),
                (0, -1),
                (1, -1),
                (-2, 0),
                (-1, 0),
                (0, 0),
                (1, 0),
                (2, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
                (0, 2),
            ] {
                dot(3, x + dx, y + dy, col);
            }
        }
        // One mark a zone: the county at the crown, the rest at the sites that lead to them.
        let (cx, cy) = t.crown;
        c.zones.push((cx * SCALE, cy * SCALE));
        for site in s.sites.iter().take(12) {
            c.zones.push((site.mx * SCALE, site.my * SCALE));
        }
        c
    }
}

/// Which loading screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The train window and the lines: what a player sees.
    #[default]
    Scroll,
    /// The county's skeleton forming: a developer's view, and a spoiler.
    Map,
}

/// The least the scroll is up, in ticks (2.5 s), so its first lines are read on a fast machine.
pub const MIN_TICKS: u32 = 150;
/// Ticks between one line and the next, at the quickest.
pub const LINE_TICKS: u32 = 32;
/// Ticks before the first line.
const FIRST_TICKS: u32 = 10;
/// Ticks the last line holds before play, and the least gap before it.
pub const CLOSE_TICKS: u32 = 40;
const CLOSE_GAP: u32 = 16;
/// Ticks a line takes to fade in, and the scroll to move up a line.
const FADE_TICKS: u32 = 16;
/// Lines kept on the card; the oldest fades out as it rises.
const KEEP: usize = 5;
/// The lantern's fill is out of this.
pub const FULL: i32 = 1024;

/// A line on the card.
#[derive(Clone, Copy, Debug)]
struct Said {
    text: &'static str,
    at: u32,
    last: bool,
}

/// The loading screen's state.
#[derive(Clone, Debug, Default)]
pub struct LoadingState {
    pub mode: Mode,
    pub card: Option<Card>,
    /// The app tick the screen came up (in the map, the tick the card arrived).
    pub since: u32,
    /// The build has finished.
    pub built: bool,
    /// The seed: it picks the lines, and the foot says it.
    pub seed: u32,
    /// "New Game", "Continue", "Load".
    pub verb: &'static str,
    /// The app tick [`LoadingState::tick`] last saw.
    pub now: u32,
    /// The stages reported so far, each once (a county re-rolled says its stages again).
    seen: Vec<&'static str>,
    /// How many stages a build reports ([`jane_world::build_stages`]).
    total: u32,
    /// Lines reported and not yet said.
    queue: VecDeque<&'static str>,
    /// Lines said, oldest first.
    said: Vec<Said>,
    /// When the last line was said.
    closing_at: Option<u32>,
    /// A press after the build: play at once.
    skipped: bool,
    /// The lantern as drawn, out of [`FULL`]; it follows the stages done.
    pub fill: i32,
    /// The canvas the train window's layers were painted for.
    painted: (i32, i32),
}

impl LoadingState {
    /// A loading screen coming up at app tick `now`.
    pub fn new(mode: Mode, seed: u32, verb: &'static str, now: u32) -> LoadingState {
        LoadingState {
            mode,
            since: now,
            now,
            seed,
            verb,
            total: jane_world::build_stages().len() as u32,
            ..LoadingState::default()
        }
    }

    /// The build says `stage` has started (`jane_world::Report`).
    pub fn stage(&mut self, stage: &'static str) {
        if self.seen.contains(&stage) {
            return;
        }
        self.seen.push(stage);
        if let Some(line) = words::line(stage, self.seed) {
            self.queue.push_back(line);
        }
    }

    /// The build is done.
    pub fn finish(&mut self) {
        self.built = true;
    }

    /// A key, a button or a click: once the county is built, play starts now.
    pub fn press(&mut self) {
        if self.built {
            self.skipped = true;
        }
    }

    /// Stages done, out of [`FULL`].
    pub fn progress(&self) -> i32 {
        if self.built { FULL } else { (self.seen.len() as i32 * FULL / self.total.max(1) as i32).min(FULL - 1) }
    }

    /// One app tick: the next line is said when its turn comes, and the lantern follows.
    pub fn tick(&mut self, now: u32) {
        self.now = now;
        let t = now.wrapping_sub(self.since);
        let gap = |g: u32, said: &[Said]| said.last().map_or(t >= FIRST_TICKS, |s| now.wrapping_sub(s.at) >= g);
        if self.mode == Mode::Scroll && self.closing_at.is_none() {
            if self.built && t + CLOSE_TICKS >= MIN_TICKS && gap(CLOSE_GAP, &self.said) {
                let key = if self.verb == "New Game" { words::CLOSING_NEW } else { words::CLOSING_LOAD };
                self.queue.clear();
                if let Some(text) = words::line(key, self.seed) {
                    self.said.push(Said { text, at: now, last: true });
                }
                self.closing_at = Some(now);
            } else if gap(LINE_TICKS, &self.said)
                && let Some(text) = self.queue.pop_front()
            {
                self.said.push(Said { text, at: now, last: false });
            }
            if self.said.len() > KEEP + 1 {
                self.said.remove(0);
            }
        }
        // The lantern eases toward what is done, never past it.
        let target = self.progress();
        if self.fill < target {
            self.fill = (self.fill + ((target - self.fill) / 6).max(4)).min(target);
        }
    }

    /// The build is done and the screen has had its time: play can start.
    pub fn done(&self, tick: u32) -> bool {
        if !self.built {
            return false;
        }
        match self.mode {
            Mode::Map => self.card.is_none() || tick.wrapping_sub(self.since) >= STAGE_TICKS * STAGES.len() as u32 + 20,
            Mode::Scroll => {
                self.skipped
                    || (tick.wrapping_sub(self.since) >= MIN_TICKS
                        && self.closing_at.is_some_and(|c| tick.wrapping_sub(c) >= CLOSE_TICKS))
            }
        }
    }
}

pub fn draw(ui: &mut Ui, st: &mut LoadingState) {
    match st.mode {
        Mode::Scroll => draw_scroll(ui, st),
        Mode::Map => draw_map(ui, st),
    }
}

// --- the train window ---------------------------------------------------------------------------

fn rgb_of(ix: Ix) -> [i32; 3] {
    let [r, g, b] = palette::rgb(ix);
    [i32::from(r), i32::from(g), i32::from(b)]
}

fn mix(a: [i32; 3], b: [i32; 3], t: i32) -> [i32; 3] {
    [a[0] + (b[0] - a[0]) * t / 256, a[1] + (b[1] - a[1]) * t / 256, a[2] + (b[2] - a[2]) * t / 256]
}

fn pack(c: [i32; 3], a: u8) -> u32 {
    u32::from(a) << 24
        | (c[0].clamp(0, 255) as u32) << 16
        | (c[1].clamp(0, 255) as u32) << 8
        | c[2].clamp(0, 255) as u32
}

const SALT: u32 = 0x5452_4e57;

/// A smooth ridge that repeats every `period` px (a multiple of `step`): control points hashed
/// every `step` px, eased between.
fn ridge(x: i32, step: i32, amp: i32, seed: u32, period: i32) -> i32 {
    let n = (period / step).max(1);
    let k = x.div_euclid(step);
    let f = x.rem_euclid(step) * 256 / step;
    let at = |i: i32| (h32(i.rem_euclid(n) as u32, seed, SALT) % (amp as u32 + 1)) as i32;
    let (a, b) = (at(k), at(k + 1));
    let s = f * f * (768 - 2 * f) / 65536;
    a + (b - a) * s / 256
}

/// The window on a canvas `cw x ch`: its rect, in canvas px. Its width is a multiple of 64, the
/// period every layer repeats at.
pub fn window(cw: i32, ch: i32) -> Rect {
    let w = ((cw - 128).min(576) / 64 * 64).max(128);
    let h = ch * 50 / 100;
    Rect::new((cw - w) / 2, 34, w, h)
}

/// The horizon in the window, px from its top.
const fn horizon(h: i32) -> i32 {
    h * 64 / 100
}

/// Paints the window's four layers, each `w x h`, one under the next into `px` (`w x 4h`,
/// `0xAARRGGBB`): the sky (opaque, still), the far hills, the hedges and trees with a cottage,
/// and the near bank with its telegraph poles. The last three repeat every `w` px and are clear
/// above their line, so each slides over the one behind.
pub fn paint_view(px: &mut [u32], w: i32, h: i32) {
    let hz = horizon(h);
    let at = |layer: i32, x: i32, y: i32| ((layer * h + y) * w + x) as usize;
    // The sky at one: an autumn afternoon's, blue-grey overhead and pale and bright low down.
    let bands = [
        rgb_of(Ramp::ClothNavy.at(Tone::Mid)),
        rgb_of(Ramp::ClothNavy.at(Tone::Base)),
        mix(rgb_of(Ramp::ClothNavy.at(Tone::Lift)), rgb_of(Ramp::ClothGrey.at(Tone::Lift)), 128),
        rgb_of(Ramp::ClothGrey.at(Tone::Lift)),
        rgb_of(Ramp::ClothGrey.at(Tone::Light)),
        mix(rgb_of(Ramp::ClothGrey.at(Tone::Light)), rgb_of(Ramp::GlassLit.at(Tone::Light)), 60),
    ];
    let nb = bands.len() as i32;
    for y in 0..h {
        let t = (y * y / hz.max(1)).min(hz) * (nb - 1) * 16 / hz.max(1);
        for x in 0..w {
            let (k, f) = (t / 16, t % 16);
            let k = if f > i32::from(jane_art::canvas::bayer(x, y)) { k + 1 } else { k };
            px[at(0, x, y)] = pack(bands[k.clamp(0, nb - 1) as usize], 255);
        }
    }
    // The sun, low and pale behind the haze, with a long thin cloud across it.
    let (sx, sy, sr) = (w * 28 / 100, hz - 46, 9);
    let sun = mix(rgb_of(Ramp::Bone.at(Tone::High)), rgb_of(Ramp::GlassLit.at(Tone::Light)), 170);
    for y in (sy - sr * 3).max(0)..(sy + sr * 3).min(h) {
        for x in (sx - sr * 3).max(0)..(sx + sr * 3).min(w) {
            let d2 = (x - sx) * (x - sx) + (y - sy) * (y - sy);
            let i = at(0, x, y);
            let c = [(px[i] >> 16 & 0xff) as i32, (px[i] >> 8 & 0xff) as i32, (px[i] & 0xff) as i32];
            if d2 <= sr * sr {
                px[i] = pack(sun, 255);
            } else if d2 < sr * sr * 9 && 60 - d2 * 60 / (sr * sr * 9) > i32::from(jane_art::canvas::bayer(x, y)) * 3 {
                px[i] = pack(mix(c, sun, 60), 255);
            }
        }
    }
    let cloud = rgb_of(Ramp::ClothPlum.at(Tone::Mid));
    for k in 0..5 {
        let hs = h32(k, 2, SALT);
        let y = hz * 3 / 10 + (hs % (hz as u32 * 6 / 10)) as i32;
        let len = w / 6 + ((hs >> 8) % (w as u32 / 4)) as i32;
        let x0 = ((hs >> 16) % w as u32) as i32 - len / 3;
        for x in x0.max(0)..(x0 + len).min(w) {
            for dy in 0..2 {
                let i = at(0, x, y + dy);
                let c = [(px[i] >> 16 & 0xff) as i32, (px[i] >> 8 & 0xff) as i32, (px[i] & 0xff) as i32];
                let edge = (x - x0).min(x0 + len - x);
                px[i] = pack(mix(c, cloud, (edge * 16).min(120 - dy * 50)), 255);
            }
        }
    }
    // The far hills: a haze a little darker than the sky at the horizon.
    let low = bands[nb as usize - 1];
    let far = mix(low, rgb_of(Ramp::ClothNavy.at(Tone::Mid)), 150);
    let far_top = mix(far, low, 60);
    for x in 0..w {
        let top = hz - 8 - ridge(x, 64, 22, 1, w) - ridge(x, 16, 4, 2, w);
        // Now and then a clump of trees on the skyline.
        let clump = if h32(x as u32 / 24, 3, SALT) % 4 == 0 { ridge(x, 8, 5, 3, w) } else { 0 };
        let top = top - clump;
        for y in top.max(0)..h {
            px[at(1, x, y)] = pack(if y == top { far_top } else { far }, 255);
        }
    }
    // The middle distance: fields, a hedge line, round trees, and one cottage with its lamp lit.
    let mid = mix(rgb_of(Ramp::ClothNavy.at(Tone::Shade)), rgb_of(Ramp::Leaf.at(Tone::Deep)), 110);
    let field = mix(mix(mid, rgb_of(Ramp::Bark.at(Tone::Shade)), 130), far, 20);
    let ground = |x: i32| hz + 4 + ridge(x, 32, 4, 4, w);
    for x in 0..w {
        let g = ground(x);
        for y in g..h {
            // Field boundaries: darker hedge lines, closer together as they go away, broken
            // where one field ends and the next begins.
            let d = y - g;
            let line = [3, 7, 13, 22, 36, 56].contains(&d) && h32(x as u32 / 64, d as u32, SALT) % 4 != 0;
            px[at(2, x, y)] = pack(if line { mid } else { field }, 255);
        }
        let hedge = g - 3 - ridge(x, 8, 2, 5, w);
        for y in hedge.max(0)..g {
            px[at(2, x, y)] = pack(mid, 255);
        }
    }
    let n_trees = w / 48;
    for k in 0..n_trees {
        let hs = h32(k as u32, 6, SALT);
        if hs % 3 == 0 {
            continue;
        }
        let cx = k * 48 + (hs >> 4) as i32 % 30;
        let r = 7 + (hs >> 12) as i32 % 9;
        let base = ground(cx.rem_euclid(w)) - 2;
        let cy = base - r - 4;
        for y in (cy - r).max(0)..base {
            for x in cx - r..=cx + r {
                let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy) * 5 / 4;
                let trunk = (x - cx).abs() <= 1 && y > cy;
                if d2 <= r * r || trunk {
                    px[at(2, x.rem_euclid(w), y)] = pack(mid, 255);
                }
            }
        }
    }
    let (hx, hw) = (w * 62 / 100, 26);
    let hb = ground(hx) - 1;
    for y in hb - 14..hb {
        for x in hx..hx + hw {
            px[at(2, x, y)] = pack(mid, 255);
        }
    }
    for k in 0..10 {
        for x in hx - 2 + k..hx + hw + 2 - k {
            px[at(2, x, hb - 14 - k)] = pack(mid, 255);
        }
    }
    for y in hb - 26..hb - 18 {
        for x in hx + 18..hx + 21 {
            px[at(2, x, y)] = pack(mid, 255);
        }
    }
    let lamp = rgb_of(Ramp::GlassLit.at(Tone::Light));
    for y in hb - 9..hb - 5 {
        for x in hx + 6..hx + 9 {
            px[at(2, x, y)] = pack(lamp, 255);
        }
    }
    // The near bank, the grass on it, and the telegraph poles with their wires.
    let near = mix(rgb_of(Ix::INK), rgb_of(Ramp::WoodDark.at(Tone::Deep)), 70);
    let grass = mix(near, rgb_of(Ramp::Grass.at(Tone::Deep)), 60);
    for x in 0..w {
        let top = h - 22 - ridge(x, 16, 5, 7, w);
        for y in top.max(0)..h {
            px[at(3, x, y)] = pack(near, 255);
        }
        let hs = h32(x as u32, 8, SALT);
        if hs % 4 == 0 {
            for k in 0..3 + (hs >> 8) as i32 % 5 {
                px[at(3, x, (top - k).max(0))] = pack(grass, 255);
            }
        }
    }
    let span = w / 3;
    for p in 0..3 {
        let x = p * span + span / 3;
        for y in 14..h {
            for dx in 0..3 {
                px[at(3, x + dx, y)] = pack(near, 255);
            }
        }
        for dx in -7..10 {
            px[at(3, x + dx, 20)] = pack(near, 255);
            px[at(3, x + dx, 21)] = pack(near, 255);
        }
        // Two wires to the next pole, sagging.
        for u in 0..span {
            let sag = 12 * 4 * u * (span - u) / (span * span);
            for (k, y0) in [21, 27].into_iter().enumerate() {
                let xx = (x + 1 + u + k as i32 * 3).rem_euclid(w);
                px[at(3, xx, y0 + sag)] = pack(near, 200);
            }
        }
    }
}

/// Draws a layer of the window `off` px along, wrapped, into `r` (the window) at `dy`.
fn layer(ui: &mut Ui, k: i32, off: i32, r: Rect, dy: i32) {
    let (w, h) = (i32::from(r.w), i32::from(r.h));
    let o = off.rem_euclid(w);
    let (x0, y0) = (i32::from(r.x), i32::from(r.y) + dy);
    let sy = (k * h) as u16;
    ui.image(CARD, Src { x: o as u16, y: sy, w: (w - o) as u16, h: h as u16 }, Rect::new(x0, y0, w - o, h), 255);
    if o > 0 {
        ui.image(CARD, Src { x: 0, y: sy, w: o as u16, h: h as u16 }, Rect::new(x0 + w - o, y0, o, h), 255);
    }
}

/// Fog: long thin bands drifting over `r` between `y0` and `y1` (window px), faint.
fn fog(ui: &mut Ui, r: Rect, t: u32, (y0, y1): (i32, i32), (bands, alpha): (i32, u8), salt: u32) {
    let w = i32::from(r.w);
    let fog = Ramp::Bone.at(Tone::Light);
    for k in 0..bands {
        let hs = h32(k as u32, salt, SALT);
        let y = y0 + (hs % (y1 - y0).max(1) as u32) as i32;
        let len = w * (35 + (hs >> 8) as i32 % 30) / 100;
        let speed = 2 + (hs >> 16) as i32 % 3;
        let x = (t as i32 * speed / 3 + (hs >> 20) as i32).rem_euclid(w + len) - len;
        let (fx, fy) = (i32::from(r.x) + x, i32::from(r.y) + y);
        ui.fill(Rect::new(fx, fy, len, 3), argb(fog, alpha));
        ui.fill(Rect::new(fx + len / 5, fy - 1, len * 3 / 5, 5), argb(fog, alpha / 2));
    }
}

/// The carriage, the window and what goes past it.
fn draw_train(ui: &mut Ui, st: &mut LoadingState) {
    let (cw, ch) = ui.canvas;
    let r = window(cw, ch);
    let (ww, wh) = (i32::from(r.w), i32::from(r.h));
    if st.painted != (cw, ch) {
        let img = ui.image_mut(CARD, ww as u16, (wh * 4) as u16);
        paint_view(&mut img.argb, ww, wh);
        ui.image_changed(CARD);
        st.painted = (cw, ch);
    }
    let t = ui.tick;
    // The carriage: dark panelling, a brass rack over the window, a rail under it.
    let wall = Ramp::WoodDark.at(Tone::Deep);
    ui.fill(Rect::new(0, 0, cw, ch), argb(wall, 255));
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ix::INK, 120));
    let (x0, y0) = (i32::from(r.x), i32::from(r.y));
    ui.fill(Rect::new(x0 - 20, y0 - 16, ww + 40, 2), argb(Ramp::Brass.at(Tone::Shade), 200));
    let rail = y0 + wh + 16;
    ui.fill(Rect::new(0, rail, cw, 3), argb(Ramp::WoodDark.at(Tone::Shade), 255));
    ui.fill(Rect::new(0, rail, cw, 1), argb(Ramp::WoodOak.at(Tone::Shade), 255));
    // Through the glass: every rail joint the view jumps a pixel.
    ui.set_clip(r);
    let jolt = i32::from(t % 53 < 2);
    layer(ui, 0, 0, r, 0);
    layer(ui, 1, (t / 12) as i32, r, jolt);
    fog(ui, r, t, (horizon(wh) - 20, horizon(wh) + 6), (4, 26), 1);
    layer(ui, 2, (t * 2 / 3) as i32, r, jolt);
    fog(ui, r, t * 2, (horizon(wh) + 2, wh - 30), (3, 20), 2);
    layer(ui, 3, (t * 4) as i32, r, jolt);
    // The glass: a streak of light across it.
    for k in 0..40 {
        ui.fill(Rect::new(x0 + 30 + k, y0 + 50 - k, 2, 1), argb(Ramp::Bone.at(Tone::High), 12));
    }
    ui.set_clip(Rect::CANVAS);
    // The frame: a wooden surround with a lit edge, and the corners rounded off.
    let frame = Ramp::WoodDark.at(Tone::Shade);
    let edge = Ramp::WoodOak.at(Tone::Mid);
    ui.fill(Rect::new(x0 - 6, y0 - 6, ww + 12, 6), argb(frame, 255));
    ui.fill(Rect::new(x0 - 6, y0 + wh, ww + 12, 6), argb(frame, 255));
    ui.fill(Rect::new(x0 - 6, y0, 6, wh), argb(frame, 255));
    ui.fill(Rect::new(x0 + ww, y0, 6, wh), argb(frame, 255));
    ui.fill(Rect::new(x0 - 6, y0 + wh + 5, ww + 12, 1), argb(edge, 255));
    ui.fill(Rect::new(x0 - 6, y0 - 6, ww + 12, 1), argb(Ix::INK, 160));
    let rad = 9;
    for dy in 0..rad {
        let d = rad - dy;
        let mut s = 0;
        while (s + 1) * (s + 1) <= rad * rad - d * d {
            s += 1;
        }
        let cut = rad - s;
        if cut <= 0 {
            continue;
        }
        let top = y0 + dy;
        let bot = y0 + wh - 1 - dy;
        for (x, y) in [(x0, top), (x0 + ww - cut, top), (x0, bot), (x0 + ww - cut, bot)] {
            ui.fill(Rect::new(x, y, cut, 1), argb(frame, 255));
        }
    }
}

/// The lantern at the corner: its glass fills from the foot as the stages are done.
fn lantern(ui: &mut Ui, x: i32, y: i32, fill: i32) {
    let brass = Ramp::Brass.at(Tone::Shade);
    let lit = Ramp::GlassLit.at(Tone::Light);
    let glow = (fill * 36 / FULL) as u8;
    ui.fill(Rect::new(x - 6, y - 4, 21, 26), argb(lit, glow / 3));
    ui.fill(Rect::new(x - 3, y - 1, 15, 20), argb(lit, glow / 2));
    ui.fill(Rect::new(x + 3, y - 6, 3, 3), argb(brass, 255));
    ui.fill(Rect::new(x + 4, y - 7, 1, 1), argb(brass, 255));
    ui.fill(Rect::new(x, y - 3, 9, 2), argb(brass, 255));
    ui.fill(Rect::new(x + 1, y - 1, 7, 12), argb(Ramp::UiSlot.at(Tone::Deep), 255));
    let h = (fill * 12 / FULL).clamp(0, 12);
    if h > 0 {
        ui.fill(Rect::new(x + 1, y + 11 - h, 7, h), argb(lit, 255));
        ui.fill(Rect::new(x + 3, y + 11 - h, 3, 1), argb(Ramp::GlassLit.at(Tone::Glint), 255));
    }
    ui.fill(Rect::new(x, y - 1, 1, 12), argb(brass, 255));
    ui.fill(Rect::new(x + 8, y - 1, 1, 12), argb(brass, 255));
    ui.fill(Rect::new(x - 1, y + 11, 11, 2), argb(brass, 255));
}

fn draw_scroll(ui: &mut Ui, st: &mut LoadingState) {
    let (cw, ch) = ui.canvas;
    draw_train(ui, st);
    let now = st.now;
    // The lines: the newest at the foot, fading in as it rises a line; the rest move up a line
    // as it comes, dimmer the older they are, the oldest fading out at the top.
    let lh = line_h(Face::Small) + 4;
    let foot = ch - 58;
    let newest = st.said.last().map_or(0, |s| now.wrapping_sub(s.at));
    let p = newest.min(FADE_TICKS) as i32;
    let ease = 256 - (FADE_TICKS as i32 - p) * (FADE_TICKS as i32 - p) * 256 / (FADE_TICKS * FADE_TICKS) as i32;
    let n = st.said.len();
    for (i, s) in st.said.iter().enumerate() {
        let k = (n - 1 - i) as i32;
        // Rank in 256ths: the newest is 0, the one before it moves from 0 to 1 as it arrives.
        let rank = if k == 0 { 0 } else { (k - 1) * 256 + ease };
        let y = foot - rank * lh / 256 + if k == 0 { (256 - ease) * lh / 256 } else { 0 };
        let a = if k == 0 {
            ease * 255 / 256
        } else {
            let fall = 255 - rank * 50 / 256;
            if rank > 256 * (KEEP as i32 - 1) { fall * (256 * KEEP as i32 - rank).max(0) / 256 } else { fall }
        }
        .clamp(0, 255) as u8;
        if a == 0 {
            continue;
        }
        let ix = if s.last { style::gold() } else { Ramp::Bone.at(Tone::Light) };
        ui.text((cw - text_w(Face::Small, s.text)) / 2, y, s.text, Ink::small(ix).shadow().alpha(a));
    }
    lantern(ui, cw - 46, ch - 40, st.fill);
    let mut foot_s = String::with_capacity(32);
    foot_s.push_str(st.verb);
    foot_s.push_str(" · seed ");
    foot_s.push_str(&st.seed.to_string());
    ui.text((cw - text_w(Face::Fine, &foot_s)) / 2, ch - 18, &foot_s, Ink::fine(style::dim()));
    // Up from dark, and down to it as the train stops.
    let t = now.wrapping_sub(st.since);
    let mut dark = if t < 14 { 255 - t * 255 / 14 } else { 0 };
    if let Some(c) = st.closing_at
        && !st.skipped
    {
        let e = now.wrapping_sub(c);
        if e + 12 > CLOSE_TICKS {
            dark = dark.max(((e + 12 - CLOSE_TICKS) * 255 / 12).min(255));
        }
    }
    if dark > 0 {
        ui.fill(Rect::new(0, 0, cw, ch), argb(Ix::INK, dark as u8));
    }
}

// --- the map (a developer's view) ---------------------------------------------------------------

fn draw_map(ui: &mut Ui, st: &mut LoadingState) {
    let (cw, ch) = ui.canvas;
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ramp::UiSlot.at(Tone::Deep), 255));
    let t = ui.tick.wrapping_sub(st.since);
    let (w, h) = (SKEL_W * SCALE, SKEL_H * SCALE);
    let r = Rect::new((cw - w) / 2 - 8, 36, w + 16, h + 16);
    ui.panel(r, PanelStyle::Window);
    let (x0, y0) = (i32::from(r.x) + 8, i32::from(r.y) + 8);
    let stage = (t / STAGE_TICKS).min(STAGES.len() as u32 - 1) as usize;
    if let Some(card) = &st.card {
        // Compose the stages so far; the current one sweeps down from the top.
        let img = ui.image_mut(CARD, w as u16, h as u16);
        let sweep_row = |k: usize| -> i32 {
            let s = t.saturating_sub(k as u32 * STAGE_TICKS);
            (s as i32 * h / STAGE_TICKS as i32).min(h)
        };
        let rows: Vec<i32> = (0..4).map(sweep_row).collect();
        let bg = argb(Ramp::UiSlot.at(Tone::Shade), 255);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let mut p = bg;
                for (k, layer) in card.layers.iter().enumerate() {
                    if y < rows[k] && layer[i] != 0 {
                        p = layer[i];
                    }
                }
                img.argb[i] = p;
            }
        }
        ui.image_changed(CARD);
        ui.image(CARD, Src { x: 0, y: 0, w: w as u16, h: h as u16 }, Rect::new(x0, y0, w, h), 255);
        // The leading edge of the sweep, a lamp-coloured line.
        if stage < 4 {
            let y = rows[stage];
            if y < h {
                ui.fill(Rect::new(x0, y0 + y, w, 1), argb(style::gold(), 200));
            }
        }
        // The zones' marks, one by one.
        if stage == 4 {
            let n = (t.saturating_sub(4 * STAGE_TICKS) * card.zones.len() as u32 / STAGE_TICKS) as usize;
            for (i, &(zx, zy)) in card.zones.iter().enumerate().take(n.min(card.zones.len())) {
                let pulse = if i + 1 == n { 3 } else { 2 };
                ui.fill(Rect::new(x0 + zx - pulse, y0 + zy - pulse, pulse * 2 + 1, pulse * 2 + 1), argb(Ix::INK, 200));
                ui.fill(Rect::new(x0 + zx - 1, y0 + zy - 1, 3, 3), argb(style::gold(), 255));
            }
        }
    }
    // The stage's name, and the dots of the stages beneath it.
    let name = STAGES[stage];
    let y = i32::from(r.y) + i32::from(r.h) + 12;
    ui.text((cw - text_w(Face::Small, name)) / 2, y, name, Ink::small(style::text_bright()).shadow());
    for k in 0..STAGES.len() {
        let x = cw / 2 - 40 + k as i32 * 20;
        let on = k <= stage;
        ui.fill(Rect::new(x, y + 26, 8, 2), argb(if on { style::gold() } else { style::dim() }, 255));
    }
    let mut foot = String::with_capacity(32);
    foot.push_str(st.verb);
    foot.push_str(" · seed ");
    foot.push_str(&st.seed.to_string());
    ui.text((cw - text_w(Face::Fine, &foot)) / 2, ch - 20, &foot, Ink::fine(style::dim()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_paints_every_layer_of_a_real_skeleton() {
        let s = jane_world::skeleton::skeleton(7).expect("seed 7 builds");
        let c = Card::from_skeleton(&s);
        assert_eq!((c.w, c.h), (SKEL_W * SCALE, SKEL_H * SCALE));
        assert!(c.layers[0].iter().all(|&p| p != 0), "land everywhere");
        for k in 1..4 {
            assert!(c.layers[k].iter().any(|&p| p != 0), "layer {k} draws");
        }
        assert_eq!(c.zones.len(), 13);
        let mut st = LoadingState::new(Mode::Map, 7, "New Game", 0);
        st.card = Some(c);
        st.finish();
        assert!(!st.done(10));
        assert!(st.done(10_000));
    }

    /// Runs a scroll from tick 0 with every stage reported at `report_at` and the build done at
    /// `built_at`; returns the tick play starts and the lines said.
    fn run(built_at: u32, report_at: u32, press_at: Option<u32>) -> (u32, Vec<Said>) {
        let mut st = LoadingState::new(Mode::Scroll, 7, "New Game", 0);
        let stages = jane_world::build_stages();
        let mut all = Vec::new();
        for now in 0..2000 {
            if now == report_at {
                for s in &stages {
                    st.stage(s);
                }
                // A county re-rolled says its stages again: nothing more is said for them.
                st.stage("land");
            }
            if now == built_at {
                st.finish();
            }
            if press_at == Some(now) {
                st.press();
            }
            st.tick(now);
            if let Some(s) = st.said.last()
                && all.last().is_none_or(|l: &Said| l.at != s.at)
            {
                all.push(*s);
            }
            if st.done(now) {
                assert_eq!(st.fill, FULL, "the lantern is full at the end");
                return (now, all);
            }
        }
        panic!("the scroll never ended");
    }

    #[test]
    fn a_fast_build_holds_long_enough_to_read_and_ends_on_the_train_slowing() {
        let (end, said) = run(10, 1, None);
        assert!((MIN_TICKS..MIN_TICKS + 40).contains(&end), "ends at {end}");
        assert!(said.len() >= 4, "{} lines", said.len());
        let last = said.last().expect("a last line");
        assert!(last.last && end - last.at >= CLOSE_TICKS);
        assert_eq!(said[0].text, words::line("skeleton", 7).expect("the first stage speaks"));
        for w in said.windows(2) {
            assert!(w[1].at - w[0].at >= CLOSE_GAP, "lines are paced");
        }
    }

    #[test]
    fn a_slow_build_ends_soon_after_it_is_built_and_a_press_ends_it_at_once() {
        let (end, _) = run(900, 800, None);
        assert!((900..900 + CLOSE_TICKS + LINE_TICKS).contains(&end), "ends at {end}");
        // A press before the build is ignored; after it, play starts at once.
        let (end, _) = run(10, 1, Some(5));
        assert!(end >= MIN_TICKS);
        let mut st = LoadingState::new(Mode::Scroll, 7, "Load", 0);
        st.finish();
        st.press();
        st.tick(1);
        assert!(st.done(1));
    }

    #[test]
    fn the_window_paints_a_sky_and_three_layers_that_slide_over_it() {
        let r = window(768, 432);
        let (w, h) = (i32::from(r.w), i32::from(r.h));
        assert_eq!(w % 64, 0);
        let mut px = vec![0u32; (w * h * 4) as usize];
        paint_view(&mut px, w, h);
        let layer = |k: i32| &px[(k * w * h) as usize..((k + 1) * w * h) as usize];
        assert!(layer(0).iter().all(|&p| p >> 24 == 0xff), "the sky is opaque");
        for k in 1..4 {
            let l = layer(k);
            assert!(l[..w as usize].iter().all(|&p| p >> 24 == 0), "layer {k} is clear at the top");
            assert!(l.iter().any(|&p| p >> 24 == 0xff), "layer {k} draws");
            // It repeats every `w`: its first and last columns meet.
            let row = (h - 1) * w;
            assert_eq!(l[row as usize] >> 24, l[(row + w - 1) as usize] >> 24, "layer {k}");
        }
    }
}
