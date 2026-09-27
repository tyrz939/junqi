//! The one four-layer pixel canvas at 16 px a cell, and every drawing primitive (ART.md §1.1,
//! §2.3).
//!
//! Units throughout: x and y are canvas pixels (16 to a cell, y down), a [`Normal`] is two bytes
//! for `nx, ny` in `-1..=1` (128 is 0; see [`normal`]), a height is screen px above the ground
//! (0..=255), and albedo and emissive are palette [`Ix`]s.
//!
//! **Rule:** a generator never writes a normal or a height by hand. Each primitive emits them
//! from the shape it drew, in drawing order: a later shape overwrites an earlier one's layers
//! where it covers it. Every primitive also remembers which draw call made each pixel, which is
//! how [`Canvas::outline`] finds the interior seams it inks in `K`.
//!
//! Normal convention: tangent space in screen axes, `+x` east (right), `+y` south (down the
//! screen), `+z` out of the ground toward the sky. A mirrored sprite flips `nx`
//! ([`Canvas::mirror_x`]); the blit does the same for a frame drawn mirrored.

use jane_core::angle::{Angle, cos_q15, sin_q15};
use jane_core::grid::Rect;
use jane_core::hash::Fnv;
use jane_core::num::isqrt;

use crate::hash::{below, h32, salt};
use crate::palette::{Ix, Ramp, Tone};

/// Screen pixels per sim cell (ART.md §0).
pub const CELL_PX: i32 = 16;

/// A tangent-space normal: `[nx, ny]`, each `128 + 127 * component` for a component in
/// `-1..=1`; `nz` is the remainder, `sqrt(1 - nx² - ny²)`.
pub type Normal = [u8; 2];

/// The normal of a surface facing straight up, toward the sky (and the viewer).
pub const FLAT: Normal = [128, 128];

/// One unit of a decoded normal component: normals are carried in 1/127ths.
pub const UNIT: i32 = 127;

/// The xy component of a 45° slope (`127 * sin 45°`), for bevels.
const SLOPE: i32 = 90;

/// The four neighbours of a pixel.
const N4: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The rise in px at which a later part standing above an earlier one gets a `K` seam.
const SEAM_RISE: u8 = 2;

/// A `deep` brighter than this (Rec. 601 luma in thousandths) would not stand off a mid ground,
/// so the selective outline keeps `k` there.
pub const SELOUT_INK_LUMA: u32 = 72_000;

/// Encode `nx, ny` (in 1/127ths, so `-127..=127`) as a [`Normal`], pulled back onto the unit
/// disc if they fall outside it.
pub fn normal(nx: i32, ny: i32) -> Normal {
    let s = nx * nx + ny * ny;
    let (nx, ny) = if s > UNIT * UNIT {
        let len = isqrt(s as u64) as i32;
        (nx * UNIT / len, ny * UNIT / len)
    } else {
        (nx, ny)
    };
    [(128 + nx).clamp(1, 255) as u8, (128 + ny).clamp(1, 255) as u8]
}

/// Decode a [`Normal`] to `[nx, ny, nz]` in 1/127ths; `nz >= 0`.
pub fn decode(n: Normal) -> [i32; 3] {
    let (x, y) = (i32::from(n[0]) - 128, i32::from(n[1]) - 128);
    let s = x * x + y * y;
    let z = if s >= UNIT * UNIT { 0 } else { isqrt((UNIT * UNIT - s) as u64) as i32 };
    [x, y, z]
}

/// A height span in screen px: where a shape meets what is under it (`lo`) and its top (`hi`).
/// A bevel falls from `hi` to `lo` across its width; a dome rises from `lo` at its rim to `hi`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Z {
    /// Where the shape meets what is under it, px.
    pub lo: u8,
    /// The shape's top, px.
    pub hi: u8,
}

impl Z {
    /// A span from `lo` to `hi` px.
    pub const fn new(lo: u8, hi: u8) -> Z {
        Z { lo, hi }
    }

    /// The same height everywhere.
    pub const fn flat(h: u8) -> Z {
        Z { lo: h, hi: h }
    }

    /// `lo` plus `num / den` of the way to `hi`.
    fn at(self, num: i32, den: i32) -> u8 {
        let (lo, hi) = (i32::from(self.lo), i32::from(self.hi));
        (lo + (hi - lo) * num.clamp(0, den) / den.max(1)) as u8
    }
}

/// The 4 x 4 ordered-dither matrix, thresholds 0..16. Dither is for gradients and soft edges
/// only, never texture noise (ART.md §2.3).
pub const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// The Bayer threshold (0..16) at pixel `(x, y)`.
pub const fn bayer(x: i32, y: i32) -> u8 {
    BAYER4[(y & 3) as usize][(x & 3) as usize]
}

/// The light albedo is baked under: from the top-left and in front, in 1/127ths (`x, y, z`).
/// So a sprite reads right with the light pass off (ART.md §3).
pub const BAKE_LIGHT: [i32; 3] = [-62, -62, 93];

/// Sphere shading: the lambert term (1/127ths, against [`BAKE_LIGHT`]) at which each tone of
/// the ramp starts, darkest first.
const SHADE_AT: [i32; 8] = [i32::MIN, -30, 10, 40, 72, 96, 110, 121];

fn band(lam: i32) -> usize {
    SHADE_AT.iter().rposition(|&t| lam >= t).unwrap_or(0)
}

/// A direction a gradient walks: its first tone at the start side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// First tone on the top row.
    Down,
    /// First tone on the bottom row.
    Up,
    /// First tone on the left column.
    Right,
    /// First tone on the right column.
    Left,
    /// First tone at the top-left corner.
    DownRight,
}

/// What [`Canvas::strokes`] lays: each kind has its own length, direction and shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeKind {
    /// Two or three px, down or down-right: pelts.
    Fur,
    /// A three-px scallop: plumage.
    Feather,
    /// Two to four px blades standing up from their root, tip lit: turf and crowns.
    Grass,
    /// Three to six px falling locks with a lean.
    Hair,
}

/// Bresenham's line from `(x0, y0)` to `(x1, y1)` inclusive, calling `f` on each pixel.
pub fn bresenham(x0: i32, y0: i32, x1: i32, y1: i32, mut f: impl FnMut(i32, i32)) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        f(x, y);
        if x == x1 && y == y1 {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// The pixels a pen of width `w` px covers around its point, as offsets from its top-left: a
/// `w x w` square, with its four corner pixels cut when `w >= 3` so a stroke reads as ink and
/// not as a staircase (ART.md §6).
pub fn pen(w: i32) -> impl Iterator<Item = (i32, i32)> {
    let w = w.max(1);
    (0..w * w).map(move |i| (i % w, i / w)).filter(move |&(x, y)| {
        let corner = (x == 0 || x == w - 1) && (y == 0 || y == w - 1);
        w < 3 || !corner
    })
}

/// A four-layer pixel canvas: albedo, normal, emissive and height, one size (ART.md §1.1).
///
/// A *flat* canvas (the font, the chrome) carries albedo alone: its normals stay [`FLAT`], its
/// heights 0 and its emissive layer empty, whatever is drawn on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    w: i32,
    h: i32,
    flat: bool,
    emitting: bool,
    part: u16,
    albedo: Vec<Ix>,
    normal: Vec<Normal>,
    emissive: Vec<Ix>,
    height: Vec<u8>,
    parts: Vec<u16>,
    clip: Option<Rect>,
}

impl Canvas {
    /// A clear canvas of `w x h` px.
    pub fn new(w: i32, h: i32) -> Canvas {
        let n = (w.max(0) * h.max(0)) as usize;
        Canvas {
            w: w.max(0),
            h: h.max(0),
            flat: false,
            emitting: false,
            part: 0,
            albedo: vec![Ix::CLEAR; n],
            normal: vec![FLAT; n],
            emissive: vec![Ix::CLEAR; n],
            height: vec![0; n],
            parts: vec![0; n],
            clip: None,
        }
    }

    /// Limit every later write to `r` (or lift the limit with `None`): a hair cap cut at the
    /// brow, a skirt cut at the hem. Reading is never clipped.
    pub fn set_clip(&mut self, r: Option<Rect>) {
        self.clip = r;
    }

    /// A clear canvas `cw x ch` cells in size (16 px a cell).
    pub fn cells(cw: i32, ch: i32) -> Canvas {
        Canvas::new(cw * CELL_PX, ch * CELL_PX)
    }

    /// A clear flat canvas of `w x h` px: albedo only, for the font and the chrome.
    pub fn flat(w: i32, h: i32) -> Canvas {
        Canvas { flat: true, ..Canvas::new(w, h) }
    }

    /// Clear all four layers, keeping the size and the flat flag: a scratch canvas reused
    /// without allocating (the chunk painter's strips).
    pub fn clear(&mut self) {
        self.albedo.fill(Ix::CLEAR);
        self.normal.fill(FLAT);
        self.emissive.fill(Ix::CLEAR);
        self.height.fill(0);
        self.parts.fill(0);
        self.part = 0;
        self.emitting = false;
    }

    /// Clear the rect `r` (clipped to the canvas) in all four layers: a scratch canvas cleared
    /// only where it was drawn.
    pub fn clear_rect(&mut self, r: Rect) {
        let (x0, y0) = (r.x.max(0), r.y.max(0));
        let (x1, y1) = (r.right().min(self.w), r.bottom().min(self.h));
        for y in y0..y1 {
            let s = (y * self.w + x0) as usize..(y * self.w + x1.max(x0)) as usize;
            self.albedo[s.clone()].fill(Ix::CLEAR);
            self.normal[s.clone()].fill(FLAT);
            self.emissive[s.clone()].fill(Ix::CLEAR);
            self.height[s.clone()].fill(0);
            self.parts[s].fill(0);
        }
    }

    /// Width in px.
    pub fn w(&self) -> i32 {
        self.w
    }

    /// Height in px.
    pub fn h(&self) -> i32 {
        self.h
    }

    /// Whether this canvas carries albedo alone.
    pub fn is_flat(&self) -> bool {
        self.flat
    }

    /// The albedo layer, row-major, `w * h` palette indices.
    pub fn albedo(&self) -> &[Ix] {
        &self.albedo
    }

    /// The normal layer, row-major, `w * h`.
    pub fn normals(&self) -> &[Normal] {
        &self.normal
    }

    /// The emissive layer, row-major, `w * h` palette indices (0 is none).
    pub fn emissive(&self) -> &[Ix] {
        &self.emissive
    }

    /// The height layer, row-major, `w * h`, screen px above the ground.
    pub fn heights(&self) -> &[u8] {
        &self.height
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    /// The albedo at `(x, y)`; clear outside the canvas.
    pub fn get(&self, x: i32, y: i32) -> Ix {
        self.idx(x, y).map_or(Ix::CLEAR, |i| self.albedo[i])
    }

    /// The normal at `(x, y)`; [`FLAT`] outside the canvas.
    pub fn normal_at(&self, x: i32, y: i32) -> Normal {
        self.idx(x, y).map_or(FLAT, |i| self.normal[i])
    }

    /// The height at `(x, y)` in px; 0 outside the canvas.
    pub fn height_at(&self, x: i32, y: i32) -> u8 {
        self.idx(x, y).map_or(0, |i| self.height[i])
    }

    /// The emissive index at `(x, y)`; clear outside the canvas.
    pub fn emissive_at(&self, x: i32, y: i32) -> Ix {
        self.idx(x, y).map_or(Ix::CLEAR, |i| self.emissive[i])
    }

    /// Turn the pen's emission on or off. While on, every pixel a primitive writes is also
    /// written to the emissive layer in its albedo colour (lamp glass, lit windows, eyes).
    pub fn set_emitting(&mut self, on: bool) {
        self.emitting = on;
    }

    /// FNV-1a over the size and all four layers: the golden hash (ART.md §1).
    pub fn hash(&self) -> u32 {
        let mut f = Fnv::new().i32(self.w).i32(self.h).u8(u8::from(self.flat));
        for a in &self.albedo {
            f = f.u16(a.0);
        }
        for n in &self.normal {
            f = f.u8(n[0]).u8(n[1]);
        }
        for e in &self.emissive {
            f = f.u16(e.0);
        }
        f.bytes(&self.height).finish()
    }

    /// Start a new draw call: pixels it writes form one part for seam finding.
    fn begin(&mut self) {
        self.part = self.part.wrapping_add(1).max(1);
    }

    /// Write one pixel's four layers. Clear and AO are not surfaces: height 0, normal flat.
    pub(crate) fn put(&mut self, x: i32, y: i32, ix: Ix, n: Normal, z: u8) {
        let Some(i) = self.idx(x, y) else { return };
        if self.clip.is_some_and(|c| !c.contains(x, y)) {
            return;
        }
        self.albedo[i] = ix;
        self.parts[i] = if ix.is_opaque() { self.part } else { 0 };
        if self.flat || !ix.is_opaque() {
            self.normal[i] = FLAT;
            self.height[i] = 0;
            self.emissive[i] = Ix::CLEAR;
        } else {
            self.normal[i] = n;
            self.height[i] = z.max(1);
            self.emissive[i] = if self.emitting { ix } else { Ix::CLEAR };
        }
    }

    /// Change a drawn pixel's colour and nothing else.
    pub(crate) fn recolour(&mut self, x: i32, y: i32, ix: Ix) {
        let Some(i) = self.idx(x, y) else { return };
        if self.albedo[i].is_opaque() {
            self.albedo[i] = ix;
            if !self.flat {
                self.emissive[i] = if self.emitting { ix } else { Ix::CLEAR };
            }
        }
    }

    /// Clear one pixel in all four layers.
    pub fn clear_px(&mut self, x: i32, y: i32) {
        self.put(x, y, Ix::CLEAR, FLAT, 0);
    }

    /// Set one flat pixel of `ix`, `z` px high (a glint, a stud, a glyph's ink).
    pub fn dot(&mut self, x: i32, y: i32, ix: Ix, z: u8) {
        self.begin();
        self.put(x, y, ix, FLAT, z);
    }

    /// Stamp a 1-bit mask `w` px wide (row-major, `bits.len() / w` rows) in `ix` with its
    /// top-left at `(x, y)`, as one part: normal flat, height `z` px. The font's ink.
    pub fn mask(&mut self, x: i32, y: i32, w: i32, bits: &[bool], ix: Ix, z: u8) {
        self.begin();
        for (i, _) in bits.iter().enumerate().filter(|(_, b)| **b) {
            let i = i as i32;
            self.put(x + i % w, y + i / w, ix, FLAT, z);
        }
    }

    /// Fill `r` with `ix`: normal flat, height `z` px.
    pub fn fill_rect(&mut self, r: Rect, ix: Ix, z: u8) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                self.put(x, y, ix, FLAT, z);
            }
        }
    }

    /// A lit block: `ramp`'s light tone on the top row and left column, its shade on the bottom
    /// row and right column, base inside. Normal flat, height `z` px (the rect's rise).
    pub fn rect_lit(&mut self, r: Rect, ramp: Ramp, z: u8) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let tone = if y == r.y || x == r.x {
                    Tone::Light
                } else if y == r.bottom() - 1 || x == r.right() - 1 {
                    Tone::Shade
                } else {
                    Tone::Base
                };
                self.put(x, y, ramp.at(tone), FLAT, z);
            }
        }
    }

    /// A bevelled block: a two-tone bevel `b` px wide (`ramp`'s light on the top and left
    /// slopes, its shade on the bottom and right), base inside. Normals are the four 45° slopes,
    /// then flat; height falls across the bevel from `z.hi` inside to `z.lo` at the edge.
    pub fn rect_bevel(&mut self, r: Rect, ramp: Ramp, b: i32, z: Z) {
        self.rect_round(r, ramp, b, 0, z);
    }

    /// [`Canvas::rect_bevel`] with its corners rounded to radius `rad` px; a rounded corner's
    /// bevel faces outward along the radius, a quarter of a torus.
    pub fn rect_round(&mut self, r: Rect, ramp: Ramp, b: i32, rad: i32, z: Z) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let Some((d, dir)) = bevel_at(r, rad, x, y) else { continue };
                if d >= b {
                    self.put(x, y, ramp.at(Tone::Base), FLAT, z.hi);
                } else {
                    let tone = if dir[0] + dir[1] <= 0 { Tone::Light } else { Tone::Shade };
                    let n = normal(dir[0] * SLOPE / UNIT, dir[1] * SLOPE / UNIT);
                    self.put(x, y, ramp.at(tone), n, z.at(d + 1, b + 1));
                }
            }
        }
    }

    /// A flat ellipse filling the box `r` in `ix`: normal flat, height `z` px.
    pub fn ellipse(&mut self, r: Rect, ix: Ix, z: u8) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if sphere_at(r, x, y).is_some() {
                    self.put(x, y, ix, FLAT, z);
                }
            }
        }
    }

    /// A lit ellipsoid filling the box `r`: `ramp` in hard tone bands shaded from the top-left
    /// ([`BAKE_LIGHT`]) with a glint, and a one-tone rim of reflected light just inside the edge
    /// on the shadow side. Normals are the sphere's; height a dome from `z.lo` at the rim to
    /// `z.hi` at the crown.
    pub fn ellipse_lit(&mut self, r: Rect, ramp: Ramp, z: Z) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let Some(n) = sphere_at(r, x, y) else { continue };
                let lam = lambert(n);
                let mut tone = Tone::ALL[band(lam)];
                let inner = N4.iter().all(|&(dx, dy)| sphere_at(r, x + dx, y + dy).is_some());
                let near = sphere_at(r, x + 2, y).is_none() || sphere_at(r, x, y + 2).is_none();
                if lam < 0 && inner && near {
                    tone = tone.step(1);
                }
                self.put(x, y, ramp.at(tone), normal(n[0], n[1]), z.at(n[2], UNIT));
            }
        }
    }

    /// A soft lit ellipsoid filling the box `r`: [`Canvas::ellipse_lit`]'s shading in all eight
    /// tones of the ramp (the half-steps are what soften a boundary), each boundary broken into
    /// 2 px clusters by a hash so no band runs ruled, and a tone of reflected light just inside
    /// the edge on the shadow side. No dither: soft is made of more tones, never of a checker
    /// (ART.md §3.1). Same normals and dome height.
    pub fn soft_ellipse(&mut self, r: Rect, ramp: Ramp, z: Z) {
        self.begin();
        let seed = h32(r.x as u32, r.y as u32, (r.w * 131 + r.h) as u32);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let Some(n) = sphere_at(r, x, y) else { continue };
                let lam = lambert(n);
                let jitter = below(h32((x >> 1) as u32, (y >> 1) as u32, seed ^ salt::STROKES), 7) as i32 - 3;
                let mut tone = Tone::ALL[band(lam + jitter)];
                let inner = N4.iter().all(|&(dx, dy)| sphere_at(r, x + dx, y + dy).is_some());
                let near = sphere_at(r, x + 2, y).is_none() || sphere_at(r, x, y + 2).is_none();
                if lam < 0 && inner && near {
                    tone = tone.step(1);
                }
                self.put(x, y, ramp.at(tone), normal(n[0], n[1]), z.at(n[2], UNIT));
            }
        }
    }

    /// A lit disc of radius `r` px centred on pixel `(cx, cy)`: [`Canvas::ellipse_lit`] in a
    /// `2r + 1` square.
    pub fn disc_lit(&mut self, cx: i32, cy: i32, r: i32, ramp: Ramp, z: Z) {
        self.ellipse_lit(Rect::new(cx - r, cy - r, 2 * r + 1, 2 * r + 1), ramp, z);
    }

    /// A row of `ix` from `x0` to `x1` inclusive: normal flat, height `z` px.
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, ix: Ix, z: u8) {
        self.line((x0, y), (x1, y), ix, 1, z);
    }

    /// A column of `ix` from `y0` to `y1` inclusive: normal flat, height `z` px.
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, ix: Ix, z: u8) {
        self.line((x, y0), (x, y1), ix, 1, z);
    }

    /// A line from pixel `a` to pixel `b` (`(x, y)`, both inked) drawn with a [`pen`] `w` px
    /// wide: normal flat, height `z` px.
    pub fn line(&mut self, a: (i32, i32), b: (i32, i32), ix: Ix, w: i32, z: u8) {
        self.begin();
        self.stroke_seg(a, b, ix, w, z);
    }

    fn stroke_seg(&mut self, a: (i32, i32), b: (i32, i32), ix: Ix, w: i32, z: u8) {
        let o = (w - 1) / 2;
        let mut pts = Vec::new();
        bresenham(a.0, a.1, b.0, b.1, |x, y| pts.push((x, y)));
        for (x, y) in pts {
            for (dx, dy) in pen(w) {
                self.put(x + dx - o, y + dy - o, ix, FLAT, z);
            }
        }
    }

    /// Connected lines through `pts` with a pen `w` px wide, as one part.
    pub fn polyline(&mut self, pts: &[(i32, i32)], ix: Ix, w: i32, z: u8) {
        self.begin();
        for s in pts.windows(2) {
            self.stroke_seg(s[0], s[1], ix, w, z);
        }
    }

    /// The closed polygon through `pts` filled in `ix` (even-odd, by pixel centre, edges
    /// included): normal flat, height `z` px. Silhouettes: a hill, a roof, a wing.
    pub fn polyline_fill(&mut self, pts: &[(i32, i32)], ix: Ix, z: u8) {
        self.begin();
        if pts.len() < 2 {
            return;
        }
        let (y0, y1) = pts.iter().fold((i32::MAX, i32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        let n = pts.len();
        let mut xs: Vec<i32> = Vec::new();
        for y in y0..=y1 {
            xs.clear();
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                if (a.1 <= y && y < b.1) || (b.1 <= y && y < a.1) {
                    // The crossing in 1/256 px.
                    xs.push(a.0 * 256 + (y - a.1) * (b.0 - a.0) * 256 / (b.1 - a.1));
                }
            }
            xs.sort();
            for pair in xs.chunks(2) {
                if let [xa, xb] = *pair {
                    for x in (xa + 255) >> 8..=xb >> 8 {
                        self.put(x, y, ix, FLAT, z);
                    }
                }
            }
        }
        for i in 0..n {
            self.stroke_seg(pts[i], pts[(i + 1) % n], ix, 1, z);
        }
    }

    /// `ramp` walked from tone `from` to tone `to` across `r` in direction `dir`, with the 4 x 4
    /// Bayer dither between tones when `dither`. Recolours what is already drawn in `r` and
    /// nothing else: albedo only, normals and heights unchanged.
    pub fn gradient(&mut self, r: Rect, ramp: Ramp, dir: Dir, from: Tone, to: Tone, dither: bool) {
        let len = match dir {
            Dir::Down | Dir::Up => r.h,
            Dir::Right | Dir::Left => r.w,
            Dir::DownRight => r.w + r.h - 1,
        };
        let span = (len - 1).max(1);
        let (a, b) = (from as i32 * 16, to as i32 * 16);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if !self.get(x, y).is_opaque() {
                    continue;
                }
                let (u, v) = (x - r.x, y - r.y);
                let t = match dir {
                    Dir::Down => v,
                    Dir::Up => r.h - 1 - v,
                    Dir::Right => u,
                    Dir::Left => r.w - 1 - u,
                    Dir::DownRight => u + v,
                };
                let pos = a + (b - a) * t / span;
                let mut tone = pos.div_euclid(16);
                if dither && pos.rem_euclid(16) > i32::from(bayer(x, y)) {
                    tone += 1;
                }
                self.recolour(x, y, ramp.at(Tone::ALL[tone.clamp(0, 7) as usize]));
            }
        }
    }

    /// Short hashed strokes over what is already drawn in `r`: each a tone lighter or darker
    /// than the pixel under it (so the shading beneath shows through), `density` strokes per
    /// 64 px², placed and shaped by `seed`. Strokes never leave the drawn shape. Each stroke
    /// lifts its pixels 1 px and tilts their normals a little toward its lean.
    pub fn strokes(&mut self, r: Rect, ramp: Ramp, kind: StrokeKind, density: i32, seed: u32) {
        let count = (r.w * r.h * density / 64).max(1);
        for k in 0..count as u32 {
            let h = h32(seed, k, salt::STROKES);
            let g = h32(seed, k, salt::STROKES ^ 1);
            let (x, y) = (r.x + below(h, r.w as u32) as i32, r.y + below(g, r.h as u32) as i32);
            let lighter = h >> 31 == 1;
            let lean = below(g.rotate_right(8), 3) as i32 - 1;
            let long = (g >> 20 & 3) as i32;
            let mut px: Vec<(i32, i32, i32)> = Vec::new(); // x, y, tone step
            match kind {
                StrokeKind::Grass => {
                    let n = 2 + long.min(2);
                    for i in 0..n {
                        let step = if i == n - 1 { 2 } else { i32::from(i > 0) };
                        px.push((x + if i * 2 >= n { lean } else { 0 }, y - i, step));
                    }
                }
                StrokeKind::Fur => {
                    let s = if lighter { 1 } else { -1 };
                    for i in 0..2 + (long & 1) {
                        px.push((x + if lean > 0 { i } else { 0 }, y + i, s));
                    }
                }
                StrokeKind::Hair => {
                    let s = if lighter { 1 } else { -1 };
                    for i in 0..3 + long {
                        px.push((x + if i >= 2 { lean } else { 0 }, y + i, s));
                    }
                }
                StrokeKind::Feather => {
                    px.extend([(x - 1, y, -1), (x, y + 1, -1), (x + 1, y, -1), (x, y, 1)]);
                }
            }
            for (sx, sy, step) in px {
                let Some(i) = self.idx(sx, sy) else { continue };
                if !r.contains(sx, sy) || !self.albedo[i].is_opaque() {
                    continue;
                }
                let tone = match Ramp::of(self.albedo[i]) {
                    Some((rr, t)) if rr == ramp => t,
                    _ => Tone::Base,
                };
                let ix = ramp.at(tone.step(step));
                self.albedo[i] = ix;
                if !self.flat {
                    let [nx, ny, _] = decode(self.normal[i]);
                    self.normal[i] = normal(nx + lean * 12, ny - 16);
                    self.height[i] = self.height[i].saturating_add(1);
                    self.emissive[i] = if self.emitting { ix } else { Ix::CLEAR };
                }
            }
        }
    }

    /// The baked contact shadow: index 1 on the clear pixels of the ellipse in `r` (under a
    /// thing, where it meets what it stands on), drawn `spread / 2` px inside the ellipse's edge
    /// so it hugs the foot. Solid: a contact shadow is never a checker (ART.md §3.1).
    pub fn ao_contact(&mut self, r: Rect, spread: i32) {
        self.begin();
        let small = r.w.min(r.h);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let Some(n) = sphere_at(r, x, y) else { continue };
                if self.get(x, y) != Ix::CLEAR {
                    continue;
                }
                let len = isqrt((n[0] * n[0] + n[1] * n[1]) as u64) as i32;
                let e8 = (UNIT - len) * small * 4 / UNIT;
                if e8 >= spread * 4 {
                    self.put(x, y, Ix::AO, FLAT, 0);
                }
            }
        }
    }

    /// Copy `src`'s drawn pixels (all four layers) with its top-left at `(x, y)`, as one part.
    /// AO from `src` lands only on clear pixels here.
    pub fn stamp(&mut self, src: &Canvas, x: i32, y: i32) {
        self.begin();
        for sy in 0..src.h {
            for sx in 0..src.w {
                let i = (sy * src.w + sx) as usize;
                let ix = src.albedo[i];
                if ix == Ix::CLEAR || (ix == Ix::AO && self.get(x + sx, y + sy) != Ix::CLEAR) {
                    continue;
                }
                self.put(x + sx, y + sy, ix, src.normal[i], src.height[i]);
                if let Some(j) = self.idx(x + sx, y + sy) {
                    if !self.flat && ix.is_opaque() {
                        self.emissive[j] = src.emissive[i];
                    }
                }
            }
        }
    }

    /// Mirror left to right in all four layers; normals have `nx` flipped.
    pub fn mirror_x(&mut self) {
        let w = self.w as usize;
        for row in 0..self.h as usize {
            let s = row * w..row * w + w;
            self.albedo[s.clone()].reverse();
            self.normal[s.clone()].reverse();
            self.emissive[s.clone()].reverse();
            self.height[s.clone()].reverse();
            self.parts[s.clone()].reverse();
            for n in &mut self.normal[s] {
                n[0] = (256 - i32::from(n[0])).clamp(1, 255) as u8;
            }
        }
    }

    /// Check the layer contract (ART.md §1.1): one size; every normal inside the unit disc (so
    /// it decodes to length 255 within 2); emissive only on drawn pixels; height at least 1 on
    /// every drawn pixel and 0 elsewhere. A flat canvas must have flat normals, zero heights and
    /// no emissive.
    pub fn validate(&self) -> Result<(), String> {
        let n = (self.w * self.h) as usize;
        if [self.albedo.len(), self.normal.len(), self.emissive.len(), self.height.len()] != [n; 4] {
            return Err("layers differ in size".into());
        }
        for i in 0..n {
            let (x, y) = (i as i32 % self.w.max(1), i as i32 / self.w.max(1));
            let [nx, ny, nz] = decode(self.normal[i]);
            let len = isqrt(((nx * nx + ny * ny + nz * nz) * 255 * 255 / (UNIT * UNIT)) as u64);
            if len.abs_diff(255) > 2 {
                return Err(format!("normal at ({x}, {y}) has length {len} of 255"));
            }
            let opaque = self.albedo[i].is_opaque();
            if self.emissive[i] != Ix::CLEAR && !opaque {
                return Err(format!("emissive on an undrawn pixel at ({x}, {y})"));
            }
            if self.flat {
                if self.normal[i] != FLAT || self.height[i] != 0 || self.emissive[i] != Ix::CLEAR {
                    return Err(format!("a flat canvas has depth at ({x}, {y})"));
                }
            } else if opaque != (self.height[i] >= 1) {
                return Err(format!("height {} at ({x}, {y}) for {:?}", self.height[i], self.albedo[i]));
            }
        }
        Ok(())
    }
}

/// Primitives the people composer brought (ART.md §2.1, §4.1, §8 step 2): upright lit bodies,
/// cloth folds, and the whole-canvas moves of the dead frames and the seat swaps.
impl Canvas {
    /// The closed polygon through `pts` filled (as [`Canvas::polyline_fill`]) and shaded as an
    /// upright cylinder: along each row, the normal turns from `-curve` (1/127ths of `nx`) at the
    /// span's left edge to `+curve` at its right, and the top two rows and the bottom row tilt up
    /// and down as a rounded end. Tones are bands of `ramp` by the lambert term, as
    /// [`Canvas::ellipse_lit`]. Height runs from `z.lo` on the bottom row to `z.hi` on the top,
    /// 1 px more down the middle. Bodies, limbs, skirts, trunks.
    pub fn polygon_lit(&mut self, pts: &[(i32, i32)], ramp: Ramp, curve: i32, z: Z) {
        self.polygon_shaded(pts, ramp, curve, z, None);
    }

    /// [`Canvas::polygon_lit`]'s shape, normals and height, with cloth's three calm tones
    /// instead of the light's bands: along each row, the light's side (the first two px) is
    /// `light`, the far side (the last three tenths, two px at least) is `shade`, and the rest
    /// is `base`; the second row, where a shoulder turns up to the light, is `lift` but for its
    /// shaded end. Large quiet areas, a lit edge and a clear shadow side: what the painter adds
    /// (folds, seams, a belt) reads against them.
    pub fn polygon_cloth(&mut self, pts: &[(i32, i32)], ramp: Ramp, curve: i32, z: Z) {
        self.polygon_shaded(pts, ramp, curve, z, Some(()));
    }

    fn polygon_shaded(&mut self, pts: &[(i32, i32)], ramp: Ramp, curve: i32, z: Z, cloth: Option<()>) {
        let mut mask = Canvas::new(self.w, self.h);
        mask.polyline_fill(pts, Ix::INK, 1);
        self.begin();
        let (y0, y1) = (pts.iter().map(|p| p.1).min().unwrap_or(0), pts.iter().map(|p| p.1).max().unwrap_or(0));
        let rows = (y1 - y0).max(1);
        // Each row's one span (doubled centre, width), where the row has exactly one: the bands
        // follow the three rows' mean, so they run as straight as the shape does and never
        // fray where a row steps in or out a pixel.
        let spans: Vec<Option<(i32, i32)>> = (0..self.h)
            .map(|y| {
                let cols: Vec<i32> = (0..self.w).filter(|&x| mask.get(x, y).is_opaque()).collect();
                let (a, b) = (*cols.first()?, *cols.last()?);
                (b - a + 1 == cols.len() as i32).then_some((a + b, b - a))
            })
            .collect();
        let smooth = |y: i32| -> Option<(i32, i32)> {
            let own = spans.get(y as usize).copied().flatten()?;
            let near: Vec<(i32, i32)> = [y - 1, y, y + 1]
                .iter()
                .filter_map(|&k| usize::try_from(k).ok().and_then(|k| spans.get(k).copied().flatten()))
                .collect();
            let n = near.len() as i32;
            let (c, w) = near.iter().fold((0, 0), |(c, w), s| (c + s.0, w + s.1));
            Some(if n > 0 { ((c + n / 2) / n, (w + n / 2) / n) } else { own })
        };
        for y in y0.max(0)..=y1.min(self.h - 1) {
            let ny = if y == y0 {
                -70
            } else if y == y0 + 1 {
                -35
            } else if y == y1 && y1 - y0 > 3 {
                40
            } else {
                0
            };
            let zr = z.at(y1 - y, rows);
            let mut x = 0;
            while x < self.w {
                if !mask.get(x, y).is_opaque() {
                    x += 1;
                    continue;
                }
                let xl = x;
                while x < self.w && mask.get(x, y).is_opaque() {
                    x += 1;
                }
                let xr = x - 1;
                let span = xr - xl + 1;
                let (centre, width) = smooth(y).unwrap_or((xl + xr, xr - xl));
                for px in xl..=xr {
                    let u = if width <= 0 { 0 } else { ((2 * px - centre) * curve / width).clamp(-curve, curve) };
                    let n = normal(u, ny);
                    let [nx, nyy, nz] = decode(n);
                    let tone = if cloth.is_some() {
                        let (i, j) = (px - xl, xr - px);
                        let dark = (span * 3 / 10).max(2);
                        let lit = if span >= 6 { 2 } else { 1 };
                        if j < dark && span >= 3 {
                            Tone::Shade
                        } else if i < lit {
                            Tone::Light
                        } else if y == y0 + 1 {
                            Tone::Lift
                        } else {
                            Tone::Base
                        }
                    } else {
                        Tone::ALL[band(lambert([nx, nyy, nz]))]
                    };
                    let bump = u8::from(span >= 4 && 2 * (px - xl) >= span / 2 && 2 * (px - xl) < span + span / 2);
                    self.put(px, y, ramp.at(tone), n, zr.saturating_add(bump));
                }
            }
        }
    }

    /// Cloth folds over what is drawn in `ramp` inside `r`: vertical bands a period of `period`
    /// px, the sine at `phase` (a quarter turn is 16384) on `r`'s left column. Where the sine is
    /// low the cloth goes a tone darker, where it peaks a tone lighter; the normal leans with the
    /// band's slope. Albedo and normal only; height unchanged.
    pub fn folds(&mut self, r: Rect, ramp: Ramp, period: i32, phase: u16) {
        let period = period.max(2);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let Some(i) = self.idx(x, y) else { continue };
                let Some((rr, t)) = Ramp::of(self.albedo[i]) else { continue };
                if rr != ramp {
                    continue;
                }
                let a = i32::from(phase) + (x - r.x) * 65536 / period;
                let s = sin_q15(Angle(a as u16)).0;
                let c = cos_q15(Angle(a as u16)).0;
                let step = i32::from(s > 27000) - i32::from(s < -22000);
                self.albedo[i] = ramp.at(t.step(step));
                if !self.flat {
                    let [nx, ny, _] = decode(self.normal[i]);
                    self.normal[i] = normal(nx + (c >> 10), ny);
                    self.emissive[i] = if self.emitting { self.albedo[i] } else { Ix::CLEAR };
                }
            }
        }
    }

    /// Replace every albedo index by `f` of it (a seat's coat, a dead frame's pallor, a ghost's
    /// mist): albedo only; normals, heights and emissive stay. Clear and AO never change.
    pub fn remap(&mut self, f: impl Fn(Ix) -> Ix) {
        for a in &mut self.albedo {
            if a.is_opaque() {
                let b = f(*a);
                if b.is_opaque() {
                    *a = b;
                }
            }
        }
    }

    /// The outline, run last and never typed by a generator (ART.md §3): **selective** (sel-out),
    /// the one outline every sprite and tile thing takes. An edge takes its own material's dark
    /// instead of `k`. Where a drawn pixel meets clear below it or to its right (away from the
    /// top-left light) it becomes its ramp's `deep`; where it meets clear only above or to its left
    /// (the lit edges) it goes two tones darker than itself and no darker than `shade`, so the
    /// line lightens and breaks where the light falls. `k` stays where the ground needs it: under a
    /// light material whose `deep` would not stand off a mid ground (luma over
    /// [`SELOUT_INK_LUMA`]), on the soles, and on anything not in a ramp. Interior seams (a later
    /// part standing 2 px over an earlier one it touches) are the upper part's own ramp two tones
    /// down (`K` off a ramp). A flat canvas (the font, the chrome) keeps the plain `k` and `K` of
    /// the UI (§7). Normals and heights stay; outlined pixels stop emitting.
    pub fn outline(&mut self) {
        let mut out = self.albedo.clone();
        let (bottom, _) = (0..self.h).rev().fold((0, false), |(b, found), y| {
            if found || !(0..self.w).any(|x| self.get(x, y).is_opaque()) { (b, found) } else { (y, true) }
        });
        for y in 0..self.h {
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                let ix = self.albedo[i];
                if !ix.is_opaque() {
                    continue;
                }
                let open = |dx: i32, dy: i32| !self.get(x + dx, y + dy).is_opaque();
                let away = open(1, 0) || open(0, 1);
                let lit = open(-1, 0) || open(0, -1);
                let seam = N4.iter().any(|&(dx, dy)| {
                    let j = ((y + dy) * self.w + x + dx) as usize;
                    self.idx(x + dx, y + dy).is_some()
                        && self.parts[j] < self.parts[i]
                        && self.height[i] >= self.height[j].saturating_add(SEAM_RISE)
                });
                let ramp = if self.flat { None } else { Ramp::of(ix) };
                out[i] = match ramp {
                    _ if !(away || lit || seam) => ix,
                    None => {
                        if away || lit {
                            Ix::INK
                        } else {
                            Ix::SEAM
                        }
                    }
                    Some((r, t)) => {
                        if away {
                            let deep = r.at(Tone::Deep);
                            let sole = open(0, 1) && y >= bottom - 1;
                            if sole || crate::palette::luma(deep) > SELOUT_INK_LUMA { Ix::INK } else { deep }
                        } else {
                            // Two tones down, held between the shade and the base: a white
                            // tail's tip still gets a line.
                            r.at(t.step(-2).clamp(Tone::Shade, Tone::Base))
                        }
                    }
                };
                if out[i] != ix {
                    self.emissive[i] = Ix::CLEAR;
                }
            }
        }
        self.albedo = out;
    }

    /// Stand an upright sprite up: every drawn pixel's height becomes its row's height above the
    /// feet on row `ay`, 5 px for every 4 rows, so a head 32 rows up stands 40 px and the height
    /// field is the true one a sun or a lamp casts from, pixel by pixel. What the primitives drew
    /// until now was relief (what stands in front of what), which the outline reads for seams:
    /// run [`Canvas::outline`] first.
    pub fn upright(&mut self, ay: i32) {
        for y in 0..self.h {
            let row = ((ay - y).max(0) * 5 / 4).clamp(1, 255) as u8;
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                if self.albedo[i].is_opaque() {
                    self.height[i] = row;
                }
            }
        }
    }

    /// A body lying down: every drawn pixel's height becomes its distance in from the silhouette
    /// (chessboard, the edge 1) up to `max`, a dome over the shape as thick as the shape is
    /// wide, so its cast shadow is a sliver with a soft top.
    pub fn dome_heights(&mut self, max: u8) {
        let mut d: Vec<u8> = self.albedo.iter().map(|a| if a.is_opaque() { u8::MAX } else { 0 }).collect();
        for _ in 0..max {
            let prev = d.clone();
            for y in 0..self.h {
                for x in 0..self.w {
                    let i = (y * self.w + x) as usize;
                    if prev[i] == 0 {
                        continue;
                    }
                    let mut m = u8::MAX;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let v = self.idx(x + dx, y + dy).map_or(0, |j| prev[j]);
                            m = m.min(v);
                        }
                    }
                    d[i] = m.saturating_add(1).min(d[i]);
                }
            }
        }
        for (h, (a, v)) in self.height.iter_mut().zip(self.albedo.iter().zip(d)) {
            if a.is_opaque() {
                *h = v.clamp(1, max);
            }
        }
    }

    /// A cast shade (or, with negative `steps`, a lift) as one cluster: every pixel of `ramp` in
    /// the ellipse filling `r` goes `steps` tones darker. The shadow of a hat brim on a brow, of
    /// a chin on a collar, of a fringe on a forehead. Albedo only.
    pub fn shade(&mut self, r: Rect, ramp: Ramp, steps: i32) {
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if sphere_at(r, x, y).is_none() {
                    continue;
                }
                if let Some((rr, t)) = Ramp::of(self.get(x, y)) {
                    if rr == ramp {
                        self.recolour(x, y, ramp.at(t.step(-steps)));
                    }
                }
            }
        }
    }

    /// Recolour pixel `(x, y)` to `ramp`'s `tone` if it is drawn in `ramp` now: a highlight
    /// band laid along a curve, a strand, a glint on what is already there. Albedo only.
    pub fn tint(&mut self, x: i32, y: i32, ramp: Ramp, tone: Tone) {
        if matches!(Ramp::of(self.get(x, y)), Some((r, _)) if r == ramp) {
            self.recolour(x, y, ramp.at(tone));
        }
    }

    /// Clean clusters: a pixel of `ramp` whose tone none of its eight neighbours shares, with at
    /// least two neighbours in `ramp`, takes the tone most of those neighbours have (the
    /// nearer to its own on a tie). A band edge that frays across a row, a fold that ends in a
    /// fleck: gone, and the shading reads in clusters of two px or more. Edge pixels (the
    /// outline's) are left alone. Albedo only.
    pub fn declutter(&mut self, ramp: Ramp) {
        let tone_of = |c: &Canvas, x: i32, y: i32| Ramp::of(c.get(x, y)).filter(|(r, _)| *r == ramp).map(|(_, t)| t);
        let mut fix = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                let Some(t) = tone_of(self, x, y) else { continue };
                if N4.iter().any(|&(dx, dy)| !self.get(x + dx, y + dy).is_opaque()) {
                    continue;
                }
                let mut count = [0u8; 8];
                let (mut same, mut kin) = (false, 0);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if (dx, dy) == (0, 0) {
                            continue;
                        }
                        if let Some(u) = tone_of(self, x + dx, y + dy) {
                            kin += 1;
                            same |= u == t;
                            count[u as usize] += 1;
                        }
                    }
                }
                if same || kin < 2 {
                    continue;
                }
                let best = (0..8).max_by_key(|&k| (count[k], -(k as i32 - t as i32).abs())).map_or(t, |k| Tone::ALL[k]);
                fix.push((x, y, best));
            }
        }
        for (x, y, t) in fix {
            self.recolour(x, y, ramp.at(t));
        }
    }

    /// Take off every spike: a drawn pixel with nothing drawn on three of its four sides is a
    /// jaggy where two shapes' edges met badly, and goes, until none is left. Run before the
    /// outline, so the line follows the clean edge.
    pub fn despike(&mut self) {
        loop {
            let mut gone = Vec::new();
            for y in 0..self.h {
                for x in 0..self.w {
                    if !self.get(x, y).is_opaque() {
                        continue;
                    }
                    let open = N4.iter().filter(|&&(dx, dy)| !self.get(x + dx, y + dy).is_opaque()).count();
                    if open >= 3 {
                        gone.push((x, y));
                    }
                }
            }
            if gone.is_empty() {
                return;
            }
            for (x, y) in gone {
                self.clear_px(x, y);
            }
        }
    }

    /// Put each tone of `ramp` to `map[tone]`: a material's own few tones, so shading falls in
    /// clusters and not in every band the light makes. Albedo only.
    pub fn retone(&mut self, ramp: Ramp, map: [Tone; 8]) {
        for a in &mut self.albedo {
            if let Some((r, t)) = Ramp::of(*a) {
                if r == ramp {
                    *a = ramp.at(map[t as usize]);
                }
            }
        }
    }

    /// Take the checker out of `ramp`: wherever a 2 x 2 block holds two of its tones in a
    /// checker (a band edge falling across a pixel grid), the lighter pair takes the darker tone.
    /// Albedo only. Skin is never dithered (ART.md §3).
    pub fn unchecker(&mut self, ramp: Ramp) {
        for y in 0..self.h - 1 {
            for x in 0..self.w - 1 {
                let (a, b, c, d) = (self.get(x, y), self.get(x + 1, y), self.get(x, y + 1), self.get(x + 1, y + 1));
                let of = |i: Ix| Ramp::of(i).filter(|(r, _)| *r == ramp).map(|(_, t)| t);
                let (Some(ta), Some(tb)) = (of(a), of(b)) else { continue };
                if a == d && b == c && ta != tb {
                    let (dark, light) =
                        if ta < tb { (a, [(x + 1, y), (x, y + 1)]) } else { (b, [(x, y), (x + 1, y + 1)]) };
                    for (px, py) in light {
                        self.recolour(px, py, dark);
                    }
                }
            }
        }
    }

    /// Stop emitting everywhere: a dead frame never emits (ART.md §4.1).
    pub fn quench(&mut self) {
        self.emissive.fill(Ix::CLEAR);
    }

    /// The canvas turned a quarter anticlockwise (`w x h` becomes `h x w`), all four layers;
    /// normals turn with it, so what faced east faces north.
    pub fn rotate_ccw(&self) -> Canvas {
        let mut out = Canvas { flat: self.flat, ..Canvas::new(self.h, self.w) };
        for y in 0..self.h {
            for x in 0..self.w {
                let (i, j) = ((y * self.w + x) as usize, ((self.w - 1 - x) * self.h + y) as usize);
                out.albedo[j] = self.albedo[i];
                out.emissive[j] = self.emissive[i];
                out.height[j] = self.height[i];
                out.parts[j] = self.parts[i];
                let [nx, ny, _] = decode(self.normal[i]);
                out.normal[j] = if self.normal[i] == FLAT { FLAT } else { normal(ny, -nx) };
            }
        }
        out
    }

    /// Remove columns that repeat the column to their left, from the middle outward, until the
    /// drawn width is at most `max_w` or no column repeats; what is right of a removed column
    /// moves left. The width of the canvas stays.
    pub fn shorten_to(&mut self, max_w: i32) {
        let drawn = |c: &Canvas| {
            let cols: Vec<i32> = (0..c.w).filter(|&x| (0..c.h).any(|y| c.get(x, y) != Ix::CLEAR)).collect();
            cols.first().zip(cols.last()).map_or(0, |(a, b)| b - a + 1)
        };
        while drawn(self) > max_w {
            let same = |c: &Canvas, x: i32| {
                (0..c.h).all(|y| {
                    let (i, j) = ((y * c.w + x) as usize, (y * c.w + x - 1) as usize);
                    c.albedo[i] == c.albedo[j] && c.height[i] == c.height[j]
                }) && (0..c.h).any(|y| c.get(x, y) != Ix::CLEAR)
            };
            let mid = self.w / 2;
            let pick = (0..self.w)
                .map(|d| if d % 2 == 0 { mid + d / 2 } else { mid - 1 - d / 2 })
                .find(|&x| x >= 1 && x < self.w && same(self, x));
            let Some(x0) = pick else { return };
            for y in 0..self.h {
                for x in x0..self.w {
                    let i = (y * self.w + x) as usize;
                    if x + 1 < self.w {
                        let j = i + 1;
                        self.albedo[i] = self.albedo[j];
                        self.normal[i] = self.normal[j];
                        self.emissive[i] = self.emissive[j];
                        self.height[i] = self.height[j];
                        self.parts[i] = self.parts[j];
                    } else {
                        self.albedo[i] = Ix::CLEAR;
                        self.normal[i] = FLAT;
                        self.emissive[i] = Ix::CLEAR;
                        self.height[i] = 0;
                        self.parts[i] = 0;
                    }
                }
            }
        }
    }

    /// Scale every height by `num / den`, keeping drawn pixels at least 1 (a body lying down, a
    /// ghost's faint shadow).
    pub fn scale_heights(&mut self, num: i32, den: i32) {
        for (h, a) in self.height.iter_mut().zip(&self.albedo) {
            if a.is_opaque() {
                *h = ((i32::from(*h) * num / den.max(1)).clamp(1, 255)) as u8;
            }
        }
    }

    /// The drawn box: the smallest rect holding every pixel that is not clear (AO counts).
    pub fn bounds(&self) -> Option<Rect> {
        let mut b: Option<(i32, i32, i32, i32)> = None;
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) != Ix::CLEAR {
                    b = Some(b.map_or((x, y, x, y), |(a, c, d, e)| (a.min(x), c.min(y), d.max(x), e.max(y))));
                }
            }
        }
        b.map(|(x0, y0, x1, y1)| Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }
}

/// Primitives the creatures and the prop kit brought (ART.md §2.2, §2.3, §8 steps 4 and 5): a
/// silhouette of any shape lit as a soft volume, a material dyed over another without losing
/// its shading, and the heights of things that are not people (a flat top, a face).
impl Canvas {
    /// The drawn pixels of `mask` (any canvas of this size: only what is opaque on it is read)
    /// filled in `ramp` as one soft volume: each pixel's normal leans out toward the nearest
    /// edge of the silhouette, as far as it is within `radius` px of it, and stands up flat
    /// beyond, so any shape (a dog's body, a sack, a fleece) turns like a body and not like a
    /// cut-out. Tones are bands of the lambert term against [`BAKE_LIGHT`], as
    /// [`Canvas::ellipse_lit`]; height rises from `z.lo` at the edge to `z.hi` where the volume
    /// is flat. The silhouette is `mask`'s, clipped by the clip.
    pub fn inflate(&mut self, mask: &Canvas, ramp: Ramp, radius: i32, z: Z) {
        let d = mask.distance();
        self.begin();
        let r10 = radius.max(1) * 10;
        let at = |x: i32, y: i32| -> i32 {
            if x < 0 || y < 0 || x >= mask.w || y >= mask.h { 0 } else { d[(y * mask.w + x) as usize] }
        };
        for y in 0..mask.h {
            for x in 0..mask.w {
                let dd = at(x, y);
                if dd == 0 {
                    continue;
                }
                // The distance field rises inward; the surface faces down its slope, outward.
                let (gx, gy) = (at(x + 1, y) - at(x - 1, y), at(x, y + 1) - at(x, y - 1));
                let len = isqrt((gx * gx + gy * gy) as u64) as i32;
                // How far out from the flat middle this px is: 0 there, UNIT at the edge.
                let u = ((r10 - dd + 5) * UNIT / r10).clamp(0, UNIT);
                let (nx, ny) = if len == 0 || u == 0 { (0, 0) } else { (-gx * u * 7 / (8 * len), -gy * u * 7 / (8 * len)) };
                let n = normal(nx, ny);
                let [a, b, c] = decode(n);
                let tone = Tone::ALL[band(lambert([a, b, c]))];
                let h = z.at(isqrt((UNIT * UNIT - (u * u).min(UNIT * UNIT)) as u64) as i32, UNIT);
                self.put(x, y, ramp.at(tone), n, h);
            }
        }
    }

    /// The chamfer distance of every opaque pixel to the nearest clear one (or the canvas edge),
    /// in tenths of a px (a step 10, a diagonal 14); 0 on clear pixels.
    fn distance(&self) -> Vec<i32> {
        let (w, h) = (self.w, self.h);
        let mut d: Vec<i32> = self.albedo.iter().map(|a| if a.is_opaque() { i32::MAX / 2 } else { 0 }).collect();
        let get = |d: &[i32], x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0 } else { d[(y * w + x) as usize] };
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if d[i] == 0 {
                    continue;
                }
                let m = (get(&d, x - 1, y) + 10)
                    .min(get(&d, x, y - 1) + 10)
                    .min(get(&d, x - 1, y - 1) + 14)
                    .min(get(&d, x + 1, y - 1) + 14);
                d[i] = d[i].min(m);
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = (y * w + x) as usize;
                if d[i] == 0 {
                    continue;
                }
                let m = (get(&d, x + 1, y) + 10)
                    .min(get(&d, x, y + 1) + 10)
                    .min(get(&d, x + 1, y + 1) + 14)
                    .min(get(&d, x - 1, y + 1) + 14);
                d[i] = d[i].min(m);
            }
        }
        d
    }

    /// Dye what is drawn in `from` inside the drawn pixels of `mask` into `to`, tone for tone,
    /// so a marking (a white blaze, a tan eyebrow, a painted band) keeps the shading under it.
    /// Albedo only.
    pub fn dye(&mut self, mask: &Canvas, from: Ramp, to: Ramp) {
        for y in 0..self.h.min(mask.h) {
            for x in 0..self.w.min(mask.w) {
                if !mask.get(x, y).is_opaque() {
                    continue;
                }
                if let Some((r, t)) = Ramp::of(self.get(x, y)) {
                    if r == from {
                        self.recolour(x, y, to.at(t));
                    }
                }
            }
        }
    }

    /// [`Canvas::dye`] inside the ellipse filling `r`.
    pub fn dye_ellipse(&mut self, r: Rect, from: Ramp, to: Ramp) {
        let mut m = Canvas::new(self.w, self.h);
        m.ellipse(r, Ix::INK, 1);
        self.dye(&m, from, to);
    }

    /// [`Canvas::dye`] inside the polygon through `pts`.
    pub fn dye_poly(&mut self, pts: &[(i32, i32)], from: Ramp, to: Ramp) {
        let mut m = Canvas::new(self.w, self.h);
        m.polyline_fill(pts, Ix::INK, 1);
        self.dye(&m, from, to);
    }

    /// A flat top: every drawn pixel in `r` stands `h` px high (a chest's lid, a table's top, a
    /// well's rim), run after [`Canvas::upright`], which stood its faces up.
    pub fn lid(&mut self, r: Rect, h: u8) {
        for y in r.y.max(0)..r.bottom().min(self.h) {
            for x in r.x.max(0)..r.right().min(self.w) {
                let i = (y * self.w + x) as usize;
                if self.albedo[i].is_opaque() {
                    self.height[i] = h.max(1);
                }
            }
        }
    }

    /// Hold every drawn pixel's height to at most `h` px: a thing lying flat on the ground (a
    /// hatch, a plate, a note), whose shadow is a sliver.
    pub fn cap_heights(&mut self, h: u8) {
        for (z, a) in self.height.iter_mut().zip(&self.albedo) {
            if a.is_opaque() {
                *z = (*z).clamp(1, h.max(1));
            }
        }
    }

    /// A face standing up and looking at the viewer (south): `r` in `ramp`, `light` on its top
    /// row and `shade` on its bottom and right, the base between; its normal faces south and a
    /// little up. Height `z` (a relief: [`Canvas::upright`] stands it after).
    pub fn face(&mut self, r: Rect, ramp: Ramp, z: u8) {
        self.begin();
        let n = normal(0, 88);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let tone = if y == r.y {
                    Tone::Base
                } else if x == r.right() - 1 && r.w > 3 {
                    Tone::Shade
                } else {
                    Tone::Mid
                };
                self.put(x, y, ramp.at(tone), n, z);
            }
        }
    }

    /// Fill `r` with `ix` facing `n` at height `z`, as one part: a flat top faces up, a board
    /// faces south; the kit's parts paint over it in the same part.
    pub fn fill_normal(&mut self, r: Rect, ix: Ix, n: Normal, z: u8) {
        self.begin();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                self.put(x, y, ix, n, z);
            }
        }
    }

    /// Recolour what emits by `f`, in the albedo and the emissive alike: a flame turned cold, a
    /// lens turned red.
    pub fn remap_emitting(&mut self, f: impl Fn(Ix) -> Ix) {
        for (a, e) in self.albedo.iter_mut().zip(self.emissive.iter_mut()) {
            if *e != Ix::CLEAR {
                *a = f(*a);
                *e = *a;
            }
        }
    }

    /// Put back what glowed: each `(x, y, ix)` drawn in `ix` and emitting it again, where the
    /// pixel is still drawn (a lamp's glass after the outline).
    pub fn relight(&mut self, glow: &[(i32, i32, Ix)]) {
        for &(x, y, ix) in glow {
            if let Some(i) = self.idx(x, y) {
                if self.albedo[i].is_opaque() && !self.flat {
                    self.albedo[i] = ix;
                    self.emissive[i] = ix;
                }
            }
        }
    }

    /// Set the height of every drawn pixel in `r` to `f(x, y)` (at least 1): a roof that lands
    /// on the house under it, rising from the eave's height to the ridge's.
    pub fn heights_by(&mut self, r: Rect, f: impl Fn(i32, i32) -> i32) {
        for y in r.y.max(0)..r.bottom().min(self.h) {
            for x in r.x.max(0)..r.right().min(self.w) {
                let i = (y * self.w + x) as usize;
                if self.albedo[i].is_opaque() {
                    self.height[i] = f(x, y).clamp(1, 255) as u8;
                }
            }
        }
    }

    /// Whether any drawn pixel of this canvas has the albedo `ix`.
    pub fn has(&self, ix: Ix) -> bool {
        self.albedo.contains(&ix)
    }
}

/// The lambert term of a normal (1/127ths) against [`BAKE_LIGHT`].
fn lambert(n: [i32; 3]) -> i32 {
    (n[0] * BAKE_LIGHT[0] + n[1] * BAKE_LIGHT[1] + n[2] * BAKE_LIGHT[2]) / UNIT
}

/// The unit-sphere normal (1/127ths) of pixel `(x, y)` in the ellipse filling the box `r`, or
/// `None` outside it. Pixel centres are tested, in doubled coordinates so an even box has a
/// half-pixel centre.
fn sphere_at(r: Rect, x: i32, y: i32) -> Option<[i32; 3]> {
    if !r.contains(x, y) {
        return None;
    }
    let (ux, uy) = (i64::from(2 * x + 1 - (2 * r.x + r.w)), i64::from(2 * y + 1 - (2 * r.y + r.h)));
    let (a, b) = (i64::from(r.w), i64::from(r.h));
    if ux * ux * b * b + uy * uy * a * a > a * a * b * b {
        return None;
    }
    let (nx, ny) = ((ux * 127 / a) as i32, (uy * 127 / b) as i32);
    let s = (nx * nx + ny * ny).min(UNIT * UNIT);
    Some([nx, ny, isqrt((UNIT * UNIT - s) as u64) as i32])
}

/// For pixel `(x, y)` of the rounded rect `r` (corner radius `rad`): its distance in px from the
/// outer edge and the outward direction there in 1/127ths, or `None` outside. Ties between two
/// straight edges go to the top, then the left, so a square corner splits on its diagonal.
fn bevel_at(r: Rect, rad: i32, x: i32, y: i32) -> Option<(i32, [i32; 2])> {
    if !r.contains(x, y) {
        return None;
    }
    let rad = rad.min(r.w / 2).min(r.h / 2).max(0);
    let (px, py) = (2 * x + 1, 2 * y + 1);
    let (x0, x1, y0, y1) = (2 * (r.x + rad), 2 * (r.right() - rad), 2 * (r.y + rad), 2 * (r.bottom() - rad));
    let vx = if px < x0 {
        px - x0
    } else if px > x1 {
        px - x1
    } else {
        0
    };
    let vy = if py < y0 {
        py - y0
    } else if py > y1 {
        py - y1
    } else {
        0
    };
    if vx != 0 && vy != 0 {
        let len2 = vx * vx + vy * vy;
        if len2 > 4 * rad * rad {
            return None;
        }
        let len = (isqrt(len2 as u64) as i32).max(1);
        return Some(((2 * rad - len) / 2, [vx * UNIT / len, vy * UNIT / len]));
    }
    let (dt, dl, db, dr) = (y - r.y, x - r.x, r.bottom() - 1 - y, r.right() - 1 - x);
    let d = dt.min(dl).min(db).min(dr);
    let dir = if dt == d {
        [0, -UNIT]
    } else if dl == d {
        [-UNIT, 0]
    } else if db == d {
        [0, UNIT]
    } else {
        [UNIT, 0]
    };
    Some((d, dir))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn normals_round_trip_and_stay_on_the_disc() {
        assert_eq!(decode(FLAT), [0, 0, 127]);
        assert_eq!(decode(normal(127, 0)), [127, 0, 0]);
        let [x, y, _] = decode(normal(500, 500));
        assert!(x * x + y * y <= UNIT * UNIT);
    }

    #[test]
    fn a_sphere_faces_outward_and_domes() {
        let mut c = Canvas::new(20, 20);
        c.ellipse_lit(Rect::new(2, 2, 16, 16), Ramp::Stone, Z::new(1, 10));
        let [lx, _, _] = decode(c.normal_at(3, 10));
        let [rx, _, _] = decode(c.normal_at(16, 10));
        let [_, ty, _] = decode(c.normal_at(10, 3));
        assert!(lx < -60 && rx > 60 && ty < -60);
        assert!(c.height_at(10, 10) > c.height_at(3, 10));
        // Lit from the top-left: the top-left quadrant is lighter than the bottom-right.
        let tone = |x, y| Ramp::of(c.get(x, y)).map(|(_, t)| t as i32).unwrap();
        assert!(tone(6, 6) > tone(14, 14));
        c.validate().unwrap();
    }

    #[test]
    fn a_bevel_faces_its_four_ways_and_falls_to_its_edge() {
        let mut c = Canvas::new(12, 12);
        c.rect_bevel(Rect::new(0, 0, 12, 12), Ramp::WoodOak, 2, Z::new(2, 8));
        assert_eq!(decode(c.normal_at(6, 0))[..2], [0, -SLOPE]);
        assert_eq!(decode(c.normal_at(0, 6))[..2], [-SLOPE, 0]);
        assert_eq!(decode(c.normal_at(6, 11))[..2], [0, SLOPE]);
        assert_eq!(decode(c.normal_at(11, 6))[..2], [SLOPE, 0]);
        assert_eq!(c.normal_at(6, 6), FLAT);
        assert_eq!(c.height_at(6, 6), 8);
        assert!(c.height_at(6, 0) < c.height_at(6, 1));
        assert_eq!(c.get(6, 0), Ramp::WoodOak.at(Tone::Light));
        assert_eq!(c.get(11, 6), Ramp::WoodOak.at(Tone::Shade));
        c.validate().unwrap();
    }

    #[test]
    fn outline_closes_and_seams_mark_a_raised_part() {
        let mut c = Canvas::new(16, 16);
        c.fill_rect(Rect::new(1, 1, 14, 14), Ramp::Stone.at(Tone::Base), 2);
        c.fill_rect(Rect::new(5, 5, 6, 6), Ramp::Iron.at(Tone::Base), 6);
        c.fill_rect(Rect::new(9, 9, 3, 3), Ramp::Stone.at(Tone::Light), 9);
        c.outline();
        assert_eq!(c.get(1, 1), Ramp::Stone.at(Tone::Shade), "a lit top-left edge: the material's shade");
        let away = c.get(14, 7);
        assert!(
            away == Ramp::Stone.at(Tone::Deep) || away == Ix::INK,
            "an edge away from the light: its deep, or k off a light material"
        );
        assert_eq!(c.get(7, 14), Ix::INK, "the sole keeps its k");
        assert_eq!(c.get(5, 7), Ramp::Iron.at(Tone::Shade), "a seam: the upper part's own ramp two tones down");
        assert_eq!(c.get(9, 10), Ramp::Stone.at(Tone::Base), "light stone over iron: two tones down");
        assert_eq!(c.get(4, 7), Ramp::Stone.at(Tone::Base), "the lower part is untouched");
        assert_eq!(c.get(7, 7), Ramp::Iron.at(Tone::Base));
        let mut flat = Canvas::flat(8, 8);
        flat.fill_rect(Rect::new(1, 1, 6, 6), Ramp::UiPanel.at(Tone::Base), 0);
        flat.outline();
        assert_eq!(flat.get(1, 1), Ix::INK, "the chrome keeps its k");
    }

    #[test]
    fn a_seam_within_one_material_is_its_own_dark() {
        let mut c = Canvas::new(12, 12);
        c.fill_rect(Rect::new(1, 1, 10, 10), Ramp::Stone.at(Tone::Base), 2);
        c.fill_rect(Rect::new(4, 4, 4, 4), Ramp::Stone.at(Tone::Light), 6);
        c.outline();
        assert_eq!(c.get(4, 5), Ramp::Stone.at(Tone::Base));
    }

    #[test]
    fn a_polygon_fills_inside_its_edges() {
        let mut c = Canvas::new(10, 10);
        c.polyline_fill(&[(0, 9), (5, 0), (9, 9)], Ix::SEAM, 1);
        assert_eq!(c.get(5, 5), Ix::SEAM);
        assert_eq!(c.get(0, 0), Ix::CLEAR);
        assert_eq!(c.get(5, 0), Ix::SEAM);
        c.validate().unwrap();
    }

    #[test]
    fn a_gradient_walks_its_ramp() {
        let mut c = Canvas::new(4, 16);
        c.fill_rect(Rect::new(0, 0, 4, 16), Ix::SEAM, 3);
        c.gradient(Rect::new(0, 0, 4, 16), Ramp::Sky, Dir::Down, Tone::Deep, Tone::Glint, true);
        assert_eq!(c.get(0, 0), Ramp::Sky.at(Tone::Deep));
        assert_eq!(c.get(0, 15), Ramp::Sky.at(Tone::Glint));
        assert_eq!(c.height_at(0, 8), 3, "heights stay");
        c.validate().unwrap();
    }

    #[test]
    fn strokes_stay_on_the_shape_and_mirror_flips_nx() {
        let mut c = Canvas::new(24, 24);
        c.soft_ellipse(Rect::new(4, 4, 16, 16), Ramp::Leaf, Z::new(1, 8));
        let before: Vec<bool> = c.albedo().iter().map(|a| a.is_opaque()).collect();
        c.strokes(Rect::new(0, 0, 24, 24), Ramp::Leaf, StrokeKind::Grass, 12, 7);
        let after: Vec<bool> = c.albedo().iter().map(|a| a.is_opaque()).collect();
        assert_eq!(before, after);
        c.validate().unwrap();
        let n = c.normal_at(5, 12);
        c.mirror_x();
        assert_eq!(decode(c.normal_at(18, 12))[0], -decode(n)[0]);
    }

    #[test]
    fn pens_round_their_corners_from_three() {
        assert_eq!(pen(1).count(), 1);
        assert_eq!(pen(2).count(), 4);
        assert_eq!(pen(3).count(), 5);
        assert_eq!(pen(4).count(), 12);
    }

    #[test]
    fn an_upright_body_turns_across_each_row_and_rises_up_it() {
        let mut c = Canvas::new(16, 20);
        c.polygon_lit(&[(3, 2), (12, 2), (13, 17), (2, 17)], Ramp::ClothPlum, 90, Z::new(2, 6));
        let [lx, _, _] = decode(c.normal_at(3, 10));
        let [rx, _, _] = decode(c.normal_at(12, 10));
        assert!(lx < -60 && rx > 60, "{lx} {rx}");
        assert!(decode(c.normal_at(7, 2))[1] < 0, "the top row tilts up");
        assert!(c.height_at(7, 3) > c.height_at(7, 16));
        let tone = |x, y| Ramp::of(c.get(x, y)).map(|(_, t)| t as i32).unwrap();
        assert!(tone(4, 10) > tone(12, 10), "lit from the left");
        c.validate().unwrap();
    }

    #[test]
    fn folds_band_the_cloth_and_touch_nothing_else() {
        let mut c = Canvas::new(16, 8);
        c.fill_rect(Rect::new(0, 0, 16, 8), Ramp::ClothGrey.at(Tone::Base), 3);
        c.fill_rect(Rect::new(0, 0, 16, 2), Ramp::Leather.at(Tone::Base), 3);
        c.folds(Rect::new(0, 0, 16, 8), Ramp::ClothGrey, 4, 0);
        let row: BTreeSet<Ix> = (0..16).map(|x| c.get(x, 5)).collect();
        assert!(row.len() >= 2, "bands across the row");
        assert!((0..16).all(|x| c.get(x, 0) == Ramp::Leather.at(Tone::Base)), "another ramp is left alone");
        assert_eq!(c.height_at(3, 5), 3);
        c.validate().unwrap();
    }

    #[test]
    fn a_clip_limits_writes() {
        let mut c = Canvas::new(8, 8);
        c.set_clip(Some(Rect::new(0, 0, 8, 4)));
        c.fill_rect(Rect::new(0, 0, 8, 8), Ix::SEAM, 1);
        c.set_clip(None);
        assert_eq!(c.get(3, 3), Ix::SEAM);
        assert_eq!(c.get(3, 4), Ix::CLEAR);
    }

    #[test]
    fn rotating_turns_the_normals_and_shortening_drops_repeats() {
        let mut c = Canvas::new(6, 10);
        c.rect_bevel(Rect::new(1, 1, 4, 8), Ramp::Stone, 1, Z::new(1, 3));
        let r = c.rotate_ccw();
        assert_eq!((r.w(), r.h()), (10, 6));
        // What faced east (the right bevel) now faces north.
        let [nx, ny, _] = decode(c.normal_at(4, 4));
        let [rx, ry, _] = decode(r.normal_at(4, 1));
        assert_eq!((nx, ny), (-ry, rx));
        r.validate().unwrap();
        let mut s = r.clone();
        s.shorten_to(5);
        let b = s.bounds().unwrap();
        assert!(b.w <= 5 && b.w >= 4, "{b:?}");
        s.validate().unwrap();
    }

    #[test]
    fn upright_writes_each_rows_true_height() {
        let mut c = Canvas::new(4, 40);
        c.fill_rect(Rect::new(0, 4, 4, 33), Ix::SEAM, 2);
        c.upright(36);
        assert_eq!(c.height_at(1, 36), 1, "the soles stand on the ground");
        assert_eq!(c.height_at(1, 20), 20);
        assert_eq!(c.height_at(1, 4), 40, "a head 32 rows up stands 40 px");
        c.validate().unwrap();
    }

    #[test]
    fn a_lying_body_domes_as_thick_as_it_is_wide() {
        let mut c = Canvas::new(20, 10);
        c.fill_rect(Rect::new(2, 2, 16, 6), Ix::SEAM, 30);
        c.dome_heights(4);
        assert_eq!(c.height_at(2, 4), 1, "the edge");
        assert_eq!(c.height_at(9, 4), 3, "three in from the edge");
        assert!(c.heights().iter().all(|&h| h <= 4));
        c.validate().unwrap();
    }

    #[test]
    fn the_selective_outline_draws_edges_in_their_own_dark() {
        let mut c = Canvas::new(12, 12);
        c.fill_rect(Rect::new(2, 2, 8, 8), Ramp::ClothPlum.at(Tone::Light), 3);
        c.fill_rect(Rect::new(2, 11, 8, 1), Ramp::ClothPlum.at(Tone::Base), 3);
        c.outline();
        assert_eq!(c.get(5, 2), Ramp::ClothPlum.at(Tone::Base), "a lit top edge: two tones down");
        assert_eq!(c.get(9, 5), Ramp::ClothPlum.at(Tone::Deep), "the side away from the light");
        assert_eq!(c.get(5, 11), Ix::INK, "the soles keep k");
        assert_eq!(c.get(5, 5), Ramp::ClothPlum.at(Tone::Light), "inside is untouched");
        let mut pale = Canvas::new(6, 6);
        pale.fill_rect(Rect::new(1, 1, 4, 3), Ramp::ClothLinen.at(Tone::Base), 3);
        pale.fill_rect(Rect::new(0, 5, 6, 1), Ix::SEAM, 1);
        pale.outline();
        assert_eq!(pale.get(4, 2), Ix::INK, "a pale deep would not stand off the ground");
    }

    #[test]
    fn uncheckering_takes_the_checker_out_of_one_ramp() {
        let mut c = Canvas::new(4, 4);
        for (x, y) in [(0, 0), (1, 1), (2, 0), (3, 1)] {
            c.dot(x, y, Ramp::Skin.at(Tone::Base), 2);
        }
        for (x, y) in [(1, 0), (0, 1), (3, 0), (2, 1)] {
            c.dot(x, y, Ramp::Skin.at(Tone::Shade), 2);
        }
        c.unchecker(Ramp::Skin);
        assert!((0..4).all(|x| c.get(x, 0) == Ramp::Skin.at(Tone::Shade) && c.get(x, 1) == Ramp::Skin.at(Tone::Shade)));
    }
}
