//! A `Frame` turned into what the GL draws: vertex lists in the shaders' layouts and the steps of
//! the albedo pass, in order. Plain CPU work over the frame and the atlas's albedo, so it runs in
//! a test with no context.
//!
//! The albedo pass has two modes (`Rows::exact`):
//!
//! - **Exact** (desktop GL): `soft`'s blit, byte for byte. A texel that depends on what is under
//!   it (the contact shadow's cover, a ghost) reads a snapshot of the albedo target, so the
//!   sprites are drawn in *runs*: a sprite that reads what is under it joins the run only if no
//!   sprite already in the run overlaps it, and before each run the rects its readers will read
//!   are copied into the snapshot. One draw call a run.
//! - **Fast** (GLES and tile GPUs, where a copy out of the target flushes it): as T2 draws them,
//!   each pass's contact shadows first by a multiply onto what the pass lies on, then its opaque
//!   and ghost runs blended. The same picture to a byte's rounding, except that a nearer thing's
//!   contact shadow no longer darkens the feet of the thing behind it.

use std::ops::Range;

use jane_present::frame::CHUNK_PX;
use jane_present::shadow;
use jane_present::{Caster, Directional, Frame, LightKind, Pass, Post, Rgb, Tint, height_of_rows};

use crate::gl::Blend;
use crate::shaders::{LIGHT_SIZES, SPAN_SIZES, SPRITE_SIZES};

/// Chunk slots across the chunk atlas (8 x 6 slots of 256 px: 2048 x 1536).
pub const SLOTS_ACROSS: u32 = 8;
pub const SLOT_ROWS: u32 = 6;
/// Canvas px above a light's disc its quad reaches: a thing standing in the disc is drawn up to
/// this far above its ground point.
const LIGHT_REACH_UP: i32 = 72;
/// Point-light shadows reach this far past the light's radius, px, then stop.
const SHADOW_PAST: f32 = 24.0;
/// Rows over the silhouette mask's box a lifted receiver may stand and still take its shadow
/// from inside it (`jane-render-soft::silhouette`'s, the same).
const CLIMB: i32 = jane_present::rows_up(100);
/// How much of a lamp's colour lights its pool, over the dark (soft's `GAIN`, a little more:
/// N dot L takes some back on the ground's edges).
const POINT_GAIN: f32 = 0.95;

/// The Features rows T1 reads (PRESENTATION.md §1.3), with their T1 defaults, and the backend's
/// own two settings. A row off draws the row below it, never nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rows {
    /// `normal_light`: N dot L per px. Off: every surface faces up (T0's pools, in the shader).
    pub normal_light: bool,
    /// `shadows`: how many point lights throw hard shadows (8 at T1; 0 is the row off).
    pub shadows: u8,
    /// `silhouettes`: the sun's and moon's silhouette shadows.
    pub silhouettes: bool,
    /// `sharp`: sharp bilinear to the window; off, nearest.
    pub sharp: bool,
    /// `max_lights`: point lights drawn at most (32 at T1).
    pub max_lights: u16,
    /// The light target at half the canvas each way (the Pi's budget, §1.7) or at full.
    pub half_light: bool,
    /// The exact albedo pass (see the module doc) or the fast one.
    pub exact: bool,
}

impl Rows {
    /// T1's column, with the light target full size and the exact albedo.
    pub const T1: Rows = Rows {
        normal_light: true,
        shadows: 8,
        silhouettes: true,
        sharp: true,
        max_lights: 32,
        half_light: false,
        exact: true,
    };
}

/// The albedo layer of an atlas page kept on the CPU, for the silhouettes' and shadows' rows,
/// and which 8 x 8 blocks of it hold a contact-shadow texel.
#[derive(Clone, Debug, Default)]
pub struct PageCpu {
    pub w: u16,
    pub h: u16,
    pub albedo: Vec<u16>,
    ao: Vec<bool>,
    bw: usize,
}

impl PageCpu {
    pub fn new(w: u16, h: u16, albedo: &[u16]) -> PageCpu {
        let bw = usize::from(w).div_ceil(8);
        let bh = usize::from(h).div_ceil(8);
        let mut ao = vec![false; bw * bh];
        for (i, &a) in albedo.iter().enumerate() {
            if a == 1 {
                let (x, y) = (i % usize::from(w.max(1)), i / usize::from(w.max(1)));
                ao[(y / 8) * bw + x / 8] = true;
            }
        }
        PageCpu { w, h, albedo: albedo.to_vec(), ao, bw }
    }

    /// Whether any texel of `src` may be the contact shadow (by 8 x 8 blocks: may say yes when
    /// the answer is no, never the other way).
    pub fn has_ao(&self, s: jane_present::Src) -> bool {
        if s.w == 0 || s.h == 0 || self.bw == 0 {
            return false;
        }
        let (x0, x1) = (usize::from(s.x) / 8, (usize::from(s.x) + usize::from(s.w) - 1) / 8);
        let (y0, y1) = (usize::from(s.y) / 8, (usize::from(s.y) + usize::from(s.h) - 1) / 8);
        (y0..=y1).any(|by| (x0..=x1).any(|bx| self.ao.get(by * self.bw + bx).copied().unwrap_or(false)))
    }
}

/// One sprite of a pass as the albedo steps see it: its quad, its page, whether it reads what is
/// under it, whether it is a ghost, and its rect clipped to the canvas.
type Quad = (usize, u8, bool, bool, (i32, i32, i32, i32));

/// One step of the albedo pass.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Chunk quads `quads` of `chunk_v`.
    Chunks(Range<usize>),
    /// Copy this rect of the albedo target into the snapshot: `(x, y, w, h)`.
    Copy(i32, i32, i32, i32),
    /// Sprite quads `quads` of `sprite_v`, all of page `page`, in `mode` (the sprite shader's).
    Sprites { page: u8, quads: Range<usize>, mode: f32, blend: Blend },
    /// The silhouettes: span quads `spans` of `span_v` into the mask (its box `(x0, y0, x1, y1)`),
    /// then the mask applied over `(x, y, w, h)` with the shade's per-channel weights.
    Silhouette { spans: Range<usize>, mask: (i32, i32, i32, i32), apply: (i32, i32, i32, i32), k: [f32; 3] },
}

/// The sky of the light pass, in linear light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub fill: [f32; 3],
    /// Toward the sun (x east, y south, z up), and its colour on flat ground.
    pub sun: Option<([f32; 3], [f32; 3])>,
}

/// Everything one frame draws, reused frame to frame.
#[derive(Debug, Default)]
pub struct Prep {
    /// Chunk quads: pos, uv (chunk-atlas texels).
    pub chunk_v: Vec<f32>,
    /// `(slot, generation)` of every chunk drawn.
    pub chunk_slots: Vec<(u16, u32)>,
    /// Sprite quads: pos, uv, rect, info.
    pub sprite_v: Vec<f32>,
    /// The albedo pass in order.
    pub steps: Vec<Step>,
    /// Ranges of chunk quads, then of sprite quads by page, that the normal and emissive passes
    /// draw (no ghosts: a ghost neither catches a height nor glows).
    pub solid_chunks: Vec<Range<usize>>,
    pub solid: Vec<(u8, Range<usize>)>,
    /// Mask quads: pos, two values (silhouettes' strength and reach, then the point lights'
    /// shadows' reach twice).
    pub span_v: Vec<f32>,
    /// Per shadow-casting light: its mask slot (1 to 8) and its quads in `span_v`.
    pub shadow_draws: Vec<(u8, Range<usize>)>,
    /// Point-light quads: pos, l0, l1, l2.
    pub light_v: Vec<f32>,
    pub n_lights: usize,
    /// The light pass's sky, if the frame has a light pass.
    pub sky: Option<Sky>,
    pub post: Option<Post>,
    /// Draw calls the albedo pass will issue (for the stats).
    pub casters: usize,
    depth: Vec<u8>,
    run: Vec<(i32, i32, i32, i32)>,
    /// One pass's quads: (quad, page, reads what is under it, ghost, clipped rect).
    quads: Vec<Quad>,
    /// Per caster of the frame: its rows' `(height above the foot, first, last)` in `rows`.
    profiles: Vec<Option<Range<usize>>>,
    rows: Vec<(i32, i32, i32)>,
}

fn push(v: &mut Vec<f32>, xs: &[f32]) {
    v.extend_from_slice(xs);
}

/// An sRGB byte as linear light.
pub fn linear(c: u8) -> f32 {
    let v = f32::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// A jane-core angle (65536 a turn) in radians.
fn rad(a: u16) -> f32 {
    f32::from(a) * std::f32::consts::TAU / 65536.0
}

/// The silhouettes' shear per px of height, Q8: `soft`'s (`jane_present::shadow::shear`), the
/// same integer sun, so both tiers lay the same shadow.
pub use jane_present::shadow::shear;

/// The light pass's sky in linear light, exposed so flat ground is as bright as T0's `ambient`
/// (the clock's keyframes, the brightness the county was tuned to) but keeps the sky's own colour:
/// the fill and the sun are scaled together by `luma(ambient) / luma(fill + sun on flat ground)`.
/// So the dusk is the fill's violet, not the keyframes' orange, and what N dot L adds is relief,
/// never a second exposure. A low sun lights flat ground by its elevation's sine over 0.2 (the
/// shader's floor).
pub fn sky(ambient: Rgb, fill: Rgb, sun: Option<Directional>) -> Sky {
    let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let fill = fill.map(linear);
    let sun = sun.map(|s| {
        let (az, el) = (rad(s.azimuth.0), rad(s.elevation.0));
        ([el.cos() * az.cos(), el.cos() * az.sin(), el.sin()], s.colour.map(linear))
    });
    let flat = sun.map_or(0.0, |(d, _)| (d[2].max(0.0) / d[2].max(0.2)).min(1.0));
    let lit: [f32; 3] = std::array::from_fn(|c| fill[c] + sun.map_or(0.0, |(_, col)| col[c]) * flat);
    let l = luma(lit);
    let k = if l > 1e-4 { (luma(ambient.map(linear)) / l).clamp(0.25, 2.0) } else { 1.0 };
    Sky { fill: fill.map(|v| v * k), sun: sun.map(|(d, col)| (d, col.map(|v| v * k))) }
}

/// Whether two `(x0, y0, x1, y1)` rects share a px.
fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

impl Prep {
    /// Fills every list from `frame`, reading the atlas's albedo from `pages`.
    pub fn build(&mut self, frame: &Frame, pages: &[PageCpu], rows: &Rows) {
        self.chunk_v.clear();
        self.chunk_slots.clear();
        self.sprite_v.clear();
        self.steps.clear();
        self.solid_chunks.clear();
        self.solid.clear();
        self.span_v.clear();
        self.shadow_draws.clear();
        self.light_v.clear();
        self.n_lights = 0;
        self.sky = None;
        self.post = None;
        self.rows.clear();
        self.profiles.clear();
        self.profiles.resize(frame.casters.len(), None);
        self.casters = frame.casters.len();
        let (cw, ch) = (i32::from(frame.canvas.0), i32::from(frame.canvas.1));

        // Each sprite's depth across the ground: its caster's, else thin.
        self.depth.clear();
        self.depth.resize(frame.sprites.len(), 2);
        for c in &frame.casters {
            if let Some(d) = self.depth.get_mut(c.sprite as usize) {
                *d = c.depth.max(1);
            }
        }

        for pass in &frame.passes {
            match *pass {
                Pass::Terrain { chunks } => {
                    let first = self.chunk_v.len() / 16;
                    for c in frame.chunks_in(chunks) {
                        let side = CHUNK_PX;
                        if c.x + side <= 0 || c.y + side <= 0 || c.x >= cw || c.y >= ch {
                            continue;
                        }
                        let slot = u32::from(c.slot);
                        let (u, v) = ((slot % SLOTS_ACROSS) as f32 * 256.0, (slot / SLOTS_ACROSS) as f32 * 256.0);
                        let (x0, y0, x1, y1) = (c.x as f32, c.y as f32, (c.x + side) as f32, (c.y + side) as f32);
                        push(&mut self.chunk_v, &[x0, y0, u, v, x1, y0, u + 256.0, v]);
                        push(&mut self.chunk_v, &[x0, y1, u, v + 256.0, x1, y1, u + 256.0, v + 256.0]);
                        self.chunk_slots.push((c.slot, c.generation));
                    }
                    let end = self.chunk_v.len() / 16;
                    if end > first {
                        self.steps.push(Step::Chunks(first..end));
                        self.solid_chunks.push(first..end);
                    }
                }
                Pass::Sprites { cmds, .. } => self.sprites(frame, cmds.range(), pages, *rows, (cw, ch)),
                Pass::Silhouettes { sun, shade, casters } => {
                    if rows.silhouettes {
                        self.silhouettes(frame, &sun, shade, casters.range(), pages, (cw, ch));
                    }
                }
                Pass::Lights { ambient, fill, sun, points, casters } => {
                    self.sky = Some(sky(ambient, fill, sun));
                    self.lights(frame, ambient, points.range(), casters.range(), pages, *rows);
                }
                Pass::Post(p) => self.post = Some(p),
                // The atmosphere's passes (PRESENTATION.md §1.9, §2) are not drawn by gl2 yet:
                // its rows (layered fog, particles, the shimmer and refraction, the wet
                // specular) are the T1 gap §1.3 names.
                Pass::Sky(_)
                | Pass::Parallax { .. }
                | Pass::Water { .. }
                | Pass::Weather(_)
                | Pass::Fog { .. }
                | Pass::Rays { .. }
                | Pass::Particles { .. } => {}
            }
        }
    }

    /// One sprite pass: its quads, its albedo steps, its solid ranges.
    fn sprites(&mut self, frame: &Frame, range: Range<usize>, pages: &[PageCpu], rows: Rows, (cw, ch): (i32, i32)) {
        let first = self.sprite_v.len() / (4 * 12);
        let mut quads = std::mem::take(&mut self.quads);
        quads.clear();
        for i in range {
            let s = &frame.sprites[i];
            let (x, y, w, h) = (i32::from(s.x), i32::from(s.y), i32::from(s.src.w), i32::from(s.src.h));
            let r = (x.max(0), y.max(0), (x + w).min(cw), (y + h).min(ch));
            if r.0 >= r.2 || r.1 >= r.3 || usize::from(s.page) >= pages.len() {
                continue;
            }
            let (kind, weight) = match s.flags.tint {
                Tint::None => (0.0, 0.0),
                Tint::Flash(a) => (1.0, f32::from(a) + f32::from(a >> 7)),
                Tint::Ghost(a) => (2.0, f32::from(a) + f32::from(a >> 7)),
            };
            let ghost = kind > 1.5;
            let depth = f32::from(self.depth.get(i).copied().unwrap_or(2));
            let (sx, sy, sw, sh) = (f32::from(s.src.x), f32::from(s.src.y), f32::from(s.src.w), f32::from(s.src.h));
            let (ul, ur) = if s.flags.mirror { (sx + sw, sx) } else { (sx, sx + sw) };
            let rect = [sx, sy, sx + sw, sy + sh];
            let info = [kind, weight, if s.flags.mirror { 1.0 } else { 0.0 }, depth];
            let (x0, y0, x1, y1) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
            for (px, py, u, v) in [(x0, y0, ul, sy), (x1, y0, ur, sy), (x0, y1, ul, sy + sh), (x1, y1, ur, sy + sh)] {
                push(&mut self.sprite_v, &[px, py, u, v]);
                push(&mut self.sprite_v, &rect);
                push(&mut self.sprite_v, &info);
            }
            let q = self.sprite_v.len() / (4 * 12) - 1;
            let reads = ghost || pages[usize::from(s.page)].has_ao(s.src);
            quads.push((q, s.page, reads, ghost, r));
        }
        let end = self.sprite_v.len() / (4 * 12);
        if end > first {
            self.sprite_steps(&quads, end, rows);
        }
        self.quads = quads;
    }

    /// The albedo steps and the solid runs of one pass's quads.
    fn sprite_steps(&mut self, quads: &[Quad], end: usize, rows: Rows) {
        // The solid runs by page, for the normal and emissive passes.
        let mut k = 0;
        while k < quads.len() {
            if quads[k].3 {
                k += 1;
                continue;
            }
            let (q0, page) = (quads[k].0, quads[k].1);
            let mut j = k + 1;
            while j < quads.len() && !quads[j].3 && quads[j].1 == page && quads[j].0 == quads[j - 1].0 + 1 {
                j += 1;
            }
            self.solid.push((page, q0..quads[j - 1].0 + 1));
            k = j;
        }
        if rows.exact {
            self.run.clear();
            let mut start: Option<(usize, u8)> = None;
            for &(q, page, reads, _, r) in quads {
                let clash = reads && self.run.iter().any(|&o| overlaps(o, r));
                if let Some((s0, p)) = start
                    && (clash || p != page)
                {
                    self.steps.push(Step::Sprites { page: p, quads: s0..q, mode: 0.0, blend: Blend::Off });
                    self.run.clear();
                    start = None;
                }
                if start.is_none() {
                    start = Some((q, page));
                }
                if reads {
                    self.steps.push(Step::Copy(r.0, r.1, r.2 - r.0, r.3 - r.1));
                }
                self.run.push(r);
            }
            if let Some((s0, p)) = start {
                self.steps.push(Step::Sprites { page: p, quads: s0..end, mode: 0.0, blend: Blend::Off });
            }
        } else {
            // The contact shadows of the whole pass, onto what it lies on; then runs of one kind.
            let mut k = 0;
            while k < quads.len() {
                let page = quads[k].1;
                let mut j = k + 1;
                while j < quads.len() && quads[j].1 == page {
                    j += 1;
                }
                self.steps.push(Step::Sprites {
                    page,
                    quads: quads[k].0..quads[j - 1].0 + 1,
                    mode: 2.0,
                    blend: Blend::Multiply,
                });
                k = j;
            }
            let mut k = 0;
            while k < quads.len() {
                let (page, ghost) = (quads[k].1, quads[k].3);
                let mut j = k + 1;
                while j < quads.len() && quads[j].1 == page && quads[j].3 == ghost {
                    j += 1;
                }
                let (mode, blend) = if ghost { (3.0, Blend::Over) } else { (1.0, Blend::Off) };
                self.steps.push(Step::Sprites { page, quads: quads[k].0..quads[j - 1].0 + 1, mode, blend });
                k = j;
            }
        }
    }

    /// Caster `ci`'s rows: `(height above the foot, first, last)` px from the sprite's left, from
    /// the foot up, each row's opaque run.
    fn profile(&mut self, frame: &Frame, ci: usize, pages: &[PageCpu]) -> Range<usize> {
        if let Some(r) = self.profiles.get(ci).cloned().flatten() {
            return r;
        }
        let start = self.rows.len();
        let c = &frame.casters[ci];
        if let Some(s) = frame.sprites.get(c.sprite as usize)
            && let Some(page) = pages.get(usize::from(s.page))
        {
            shadow::rows(&page.albedo, page.w, s, i32::from(c.foot.1), &mut self.rows);
        }
        let r = start..self.rows.len();
        if let Some(p) = self.profiles.get_mut(ci) {
            *p = Some(r.clone());
        }
        r
    }

    /// The silhouettes' spans (`jane-render-soft::silhouette::cast`, the same integers) and the
    /// step that lays and applies them.
    fn silhouettes(
        &mut self,
        frame: &Frame,
        sun: &Directional,
        shade: Rgb,
        casters: Range<usize>,
        pages: &[PageCpu],
        (cw, ch): (i32, i32),
    ) {
        let Some(k) = shear(sun) else { return };
        let first = self.span_v.len() / (4 * 4);
        let mut dirty: Option<(i32, i32, i32, i32)> = None;
        for ci in casters {
            let c: Caster = frame.casters[ci];
            let Some(s) = frame.sprites.get(c.sprite as usize) else { continue };
            let prof = self.profile(frame, ci, pages);
            let (span_v, rows) = (&mut self.span_v, &self.rows[prof]);
            shadow::bands(rows, i32::from(s.x), &c, k, |b| {
                let (x0, x1, y0, y1) = (b.x0.max(0), b.x1.min(cw), b.y0.max(0), b.y1.min(ch));
                if x0 >= x1 || y0 >= y1 {
                    return;
                }
                let (v, r) = (f32::from(b.strength), f32::from(b.reach));
                let (a, bb, c2, d) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
                push(span_v, &[a, bb, v, r, c2, bb, v, r, a, d, v, r, c2, d, v, r]);
                dirty = Some(match dirty {
                    None => (x0, y0, x1, y1),
                    Some((p, q, rr, t)) => (p.min(x0), q.min(y0), rr.max(x1), t.max(y1)),
                });
            });
        }
        let end = self.span_v.len() / (4 * 4);
        let Some((x0, y0, x1, y1)) = dirty else { return };
        // The mask's box and the ring round it, and above it as high as a lifted receiver whose
        // ground lies inside it can stand.
        let (ax0, ay0, ax1, ay1) = ((x0 - 1).max(0), (y0 - 1 - CLIMB).max(0), (x1 + 1).min(cw), (y1 + 1).min(ch));
        let k = shade.map(|c| f32::from(256 - u16::from(c) - u16::from(c >> 7)));
        self.steps.push(Step::Silhouette {
            spans: first..end,
            mask: (x0, y0, x1, y1),
            apply: (ax0, ay0, ax1 - ax0, ay1 - ay0),
            k,
        });
    }

    /// The point lights' quads, and the shadow geometry of the ones that cast.
    fn lights(
        &mut self,
        frame: &Frame,
        ambient: Rgb,
        points: Range<usize>,
        casters: Range<usize>,
        pages: &[PageCpu],
        rows: Rows,
    ) {
        // A pool shows against the dark: by day a little, at night all of it (soft's rule).
        let avg = ambient.iter().map(|&c| u32::from(c)).sum::<u32>() / 3;
        let dark = (300u32.saturating_sub(avg)).min(220) as f32 / 220.0;
        let mut slot = 0u8;
        for li in points.take(usize::from(rows.max_lights)) {
            let l = frame.lights[li];
            let r = f32::from(l.radius);
            if l.radius == 0 {
                continue;
            }
            let (lx, ly) = (l.pos.0 as f32 + 0.5, l.pos.1 as f32 + 0.5);
            let lh = f32::from(l.height.max(1));
            let mut mask = 0.0;
            if l.casts && slot < rows.shadows.min(8) {
                slot += 1;
                let first = self.span_v.len() / (4 * 4);
                for ci in casters.clone() {
                    // A light never shadows what holds it: her lantern's hand, a lamp's post.
                    if l.holder != Some(frame.casters[ci].sprite) {
                        self.shadow(frame, ci, pages, (lx, ly, lh, r));
                    }
                }
                let end = self.span_v.len() / (4 * 4);
                if end > first {
                    self.shadow_draws.push((slot, first..end));
                    mask = f32::from(slot);
                }
            }
            let g = POINT_GAIN * dark;
            let [cr, cg, cb] = l.colour.map(|c| f32::from(c) / 255.0 * g);
            // Flame light leans warm, as on T0 and T2: a yellow lamp on green grass is not lime.
            let col = [cr, cg * 13.0 / 16.0, cb * 11.0 / 16.0, 0.0];
            let spot = match l.kind {
                LightKind::Point => [0.0, 0.0, -2.0, mask],
                LightKind::Spot { dir, cone } => [rad(dir.0).cos(), rad(dir.0).sin(), rad(cone.0).cos(), mask],
            };
            let l0 = [lx, ly, lh, r];
            let (x0, y0) = (lx - r - 1.0, ly - r - LIGHT_REACH_UP as f32 - 1.0);
            let (x1, y1) = (lx + r + 1.0, ly + r + 1.0);
            for (px, py) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                push(&mut self.light_v, &[px, py]);
                push(&mut self.light_v, &l0);
                push(&mut self.light_v, &col);
                push(&mut self.light_v, &spot);
            }
            self.n_lights += 1;
        }
    }

    /// Caster `ci`'s shadow from a point light at `(lx, ly)`, `lh` up, reaching `r`: each run of
    /// equal rows of its sprite, a vertical slab from the front and from the back of its
    /// footprint, projected from the light onto the ground (a row higher than the light reaches
    /// past the rim, where it stops). Each corner carries how high the shadow reaches there: the
    /// ray over the caster's top, `lh + (H - lh) * t / d` at `t` from the light for a caster `d`
    /// away, so a thing standing in the shadow is dark only up to it.
    fn shadow(&mut self, frame: &Frame, ci: usize, pages: &[PageCpu], (lx, ly, lh, r): (f32, f32, f32, f32)) {
        let c = frame.casters[ci];
        let Some(s) = frame.sprites.get(c.sprite as usize) else { return };
        let (fx, fy) = (f32::from(c.foot.0) + 0.5, f32::from(c.foot.1) + 0.5);
        let d = ((fx - lx).powi(2) + (fy - ly).powi(2)).sqrt();
        if d > r + 48.0 {
            return;
        }
        let far = r + SHADOW_PAST;
        let x = f32::from(s.x);
        let depth = f32::from(c.depth.max(2));
        let backs = [fy - (depth / 2.0).floor(), fy + depth - (depth / 2.0).floor()];
        let prof = self.profile(frame, ci, pages);
        // Its top, true px: the ray over it is how high the shadow reaches.
        let tall = i32::from(c.height).max(1);
        let up = |hv: i32| height_of_rows(hv).min(tall) as f32;
        let top = self.rows[prof.clone()].last().map_or(1.0, |r| up(r.0));
        // Runs of rows with the same extent, from the foot up: (from, to, first, last).
        let mut k = prof.start;
        while k < prof.end {
            let (h0, u0, u1) = self.rows[k];
            let mut j = k + 1;
            while j < prof.end
                && self.rows[j].1 == u0
                && self.rows[j].2 == u1
                && self.rows[j].0 == self.rows[j - 1].0 + 1
            {
                j += 1;
            }
            let h1 = self.rows[j - 1].0 + 1;
            k = j;
            // The run's bottom and top as true px (a row `hv` above the foot is `5 hv / 4` up).
            let (za, zb) = (up(h0 - 1), up(h1));
            let (xa, xb) = (x + u0 as f32, x + u1 as f32 + 1.0);
            for by in backs {
                let mut quad = [0.0f32; 16];
                for (n, (px, z)) in [(xa, za), (xb, za), (xa, zb), (xb, zb)].into_iter().enumerate() {
                    let (dx, dy) = (px - lx, by - ly);
                    let dist = (dx * dx + dy * dy).sqrt().max(0.5);
                    let t = if z >= lh - 0.5 { far } else { (dist * lh / (lh - z)).min(far) };
                    let (gx, gy) = (lx + dx / dist * t, ly + dy / dist * t);
                    let reach = (lh + (top - lh) * t / dist).clamp(0.0, 255.0);
                    quad[n * 4..n * 4 + 4].copy_from_slice(&[gx, gy, reach, reach]);
                }
                push(&mut self.span_v, &quad);
            }
        }
    }

    /// Quads in each list.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let per = |sizes: &[i32]| sizes.iter().sum::<i32>() as usize * 4;
        (
            self.chunk_v.len() / 16,
            self.sprite_v.len() / per(&SPRITE_SIZES),
            self.span_v.len() / per(&SPAN_SIZES),
            self.light_v.len() / per(&LIGHT_SIZES),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::{Depth, Flags, Span, SpriteCmd, Src, Tier};

    fn sprite(x: i16, tint: Tint) -> SpriteCmd {
        SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 8, h: 8 },
            x,
            y: 0,
            flags: Flags { mirror: false, tint },
            height_px: 8,
        }
    }

    /// An 8 x 8 page: opaque above, the contact shadow on its bottom row.
    fn page() -> PageCpu {
        let mut a = vec![2u16; 64];
        for v in &mut a[56..] {
            *v = 1;
        }
        PageCpu::new(8, 8, &a)
    }

    #[test]
    fn exact_runs_break_where_a_reader_overlaps_and_copy_what_it_reads() {
        let mut f = Frame::new(Tier::T1);
        // 0 and 1 overlap; 2 is clear of both.
        f.sprites.extend([sprite(0, Tint::None), sprite(4, Tint::None), sprite(20, Tint::Ghost(90))]);
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 3 } });
        let mut p = Prep::default();
        p.build(&f, &[page()], &Rows::T1);
        assert_eq!(
            p.steps,
            [
                Step::Copy(0, 0, 8, 8),
                Step::Sprites { page: 0, quads: 0..1, mode: 0.0, blend: Blend::Off },
                Step::Copy(4, 0, 8, 8),
                Step::Copy(20, 0, 8, 8),
                Step::Sprites { page: 0, quads: 1..3, mode: 0.0, blend: Blend::Off },
            ]
        );
        // The ghost neither catches a height nor glows.
        assert_eq!(p.solid, [(0, 0..2)]);
        let fast = Rows { exact: false, ..Rows::T1 };
        p.build(&f, &[page()], &fast);
        assert_eq!(
            p.steps,
            [
                Step::Sprites { page: 0, quads: 0..3, mode: 2.0, blend: Blend::Multiply },
                Step::Sprites { page: 0, quads: 0..2, mode: 1.0, blend: Blend::Off },
                Step::Sprites { page: 0, quads: 2..3, mode: 3.0, blend: Blend::Over },
            ]
        );
    }

    #[test]
    fn a_page_knows_which_blocks_hold_the_contact_shadow() {
        let p = page();
        assert!(p.has_ao(Src { x: 0, y: 0, w: 8, h: 8 }));
        let mut a = vec![2u16; 256];
        a[16 * 12 + 12] = 1;
        let p = PageCpu::new(16, 16, &a);
        assert!(!p.has_ao(Src { x: 0, y: 0, w: 8, h: 8 }));
        assert!(p.has_ao(Src { x: 4, y: 4, w: 6, h: 6 }));
    }

    #[test]
    fn a_post_casts_a_shadow_away_from_a_lamp_lower_than_its_top() {
        let mut f = Frame::new(Tier::T1);
        // A 2 x 10 post standing on row 10 at x 50; a lamp 40 px west of it, 6 px up.
        f.sprites.push(SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 2, h: 10 },
            x: 49,
            y: 0,
            flags: Flags::default(),
            height_px: 10,
        });
        f.casters.push(Caster { sprite: 0, foot: (50, 10), height: 10, depth: 2 });
        f.lights.push(jane_present::Light {
            pos: (10, 10),
            height: 6,
            colour: [255, 200, 100],
            radius: 100,
            size: 4,
            casts: true,
            kind: LightKind::Point,
            holder: None,
        });
        f.passes.push(Pass::Lights {
            ambient: [60; 3],
            fill: [40; 3],
            sun: None,
            points: Span { start: 0, len: 1 },
            casters: Span { start: 0, len: 1 },
        });
        let pages = [PageCpu::new(2, 10, &[2; 20])];
        let mut p = Prep::default();
        p.build(&f, &pages, &Rows::T1);
        assert_eq!(p.n_lights, 1);
        assert_eq!(p.shadow_draws.len(), 1);
        // Every corner lies east of the post's foot and within the light's reach past its rim.
        let v = &p.span_v;
        let xs: Vec<f32> = v.chunks_exact(4).map(|c| c[0]).collect();
        assert!(xs.iter().all(|&x| (49.0..=10.5 + 124.0).contains(&x)), "{xs:?}");
        assert!(xs.iter().any(|&x| x > 120.0), "the top, above the lamp, reaches the rim");
        // The reach over the foot is the post's height; far out it rises past it.
        let reach: Vec<f32> = v.chunks_exact(4).map(|c| c[2]).collect();
        assert!(reach.iter().any(|&z| (z - 10.0).abs() < 1.0));
        assert!(reach.iter().all(|&z| z >= 9.0));
        // No shadows when the row is off.
        p.build(&f, &pages, &Rows { shadows: 0, ..Rows::T1 });
        assert!(p.shadow_draws.is_empty() && p.span_v.is_empty());
        // A light never shadows what holds it: the post holding the lamp throws nothing of it.
        f.lights[0].holder = Some(0);
        p.build(&f, &pages, &Rows::T1);
        assert!(p.shadow_draws.is_empty() && p.span_v.is_empty());
    }
}
