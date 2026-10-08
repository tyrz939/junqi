//! The atmosphere's passes on C2 (PRESENTATION.md §1.8, §1.9, §1.12): the grade as `soft` draws
//! it, as quads and strips for the GE.

use jane_present::Post;
use jane_present::frame::{FogVolume, PartShape, Particle, SkyLook, StarCmd, WaterCmd};
use jane_present::{SpriteCmd, Tint};

use super::{Lister, Mode, Quad, Strip, StripTex, Tex, Vert};
use crate::grade::{BAND, Grade, luma_clut};

/// A quad over canvas px `(x0, y0)..(x1, y1)` from texels `(u0, v0)..(u1, v1)`.
#[allow(clippy::too_many_arguments)]
const fn quad(tex: Tex, mode: Mode, colour: u32, x0: i32, y0: i32, x1: i32, y1: i32, uv: (i32, i32, i32, i32)) -> Quad {
    Quad {
        tex,
        mode,
        colour,
        x0: x0 as i16,
        y0: y0 as i16,
        x1: x1 as i16,
        y1: y1 as i16,
        u0: uv.0 as u16,
        v0: uv.1 as u16,
        u1: uv.2 as u16,
        v1: uv.3 as u16,
    }
}

/// An `Rgb` and an alpha as `0xAABBGGRR`.
const fn rgba3(c: [u8; 3], a: u32) -> u32 {
    a << 24 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32
}

/// Grey `v` (0..=255), opaque, `0xAABBGGRR`.
const fn grey(v: u32) -> u32 {
    0xff00_0000 | v << 16 | v << 8 | v
}

impl Lister {
    /// Opens a strip: its vertices are pushed after this, then [`Lister::end_strip`].
    pub(super) fn begin_strip(&self) -> u32 {
        self.verts.len() as u32
    }

    /// Closes the strip begun at `start` and lays it with `mode`.
    pub(super) fn end_strip(&mut self, start: u32, tex: StripTex, mode: Mode) {
        let len = self.verts.len() as u32 - start;
        if len < 3 {
            self.verts.truncate(start as usize);
            return;
        }
        let i = self.strips.len() as u16;
        self.strips.push(Strip { start, len: len as u16, tex });
        self.quads.push(quad(Tex::Strip(i), mode, 0xffff_ffff, 0, 0, self.w, self.h, (0, 0, 0, 0)));
    }

    pub(super) fn vert(&mut self, x: i32, y: i32, colour: u32) {
        self.verts.push(Vert { x: x as i16, y: y as i16, u: 0, v: 0, colour });
    }

    /// The grade (`Post`), `soft`'s terms (`crate::grade`): the far edge pulled toward the
    /// horizon's air at dusk and dawn (a smooth strip down the frame), the saturation (the frame
    /// mixed with its luma, made at half size in the lightmap's target), then each channel read
    /// back through its table and written alone (one table a band of columns at dusk).
    pub(super) fn grade(&mut self, p: &Post) {
        let (w, h) = (self.w, self.h);
        if self.grade.update(p, self.sky.as_ref(), (w as u16, h as u16)) {
            self.cluts.clear();
            for c in 0..3 {
                self.cluts.push(luma_clut(c));
            }
            for lut in &self.grade.luts {
                for (c, t) in lut.iter().enumerate() {
                    self.cluts.push(core::array::from_fn(|v| 0xff00_0000 | u32::from(t[v]) << (8 * c)));
                }
            }
            self.cluts_gen = self.cluts_gen.wrapping_add(1);
        }
        // The far edge: weight `(1 - y / h)^2` of the top row's, eight rows of a strip.
        if let Some((c, top)) = self.grade.far {
            let s = self.begin_strip();
            for k in 0..=8 {
                let y = h * k / 8;
                let a = (Grade::pull_at(top, y, h) * 255 / 256).min(255);
                let col = a << 24 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32;
                self.vert(0, y, col);
                self.vert(w, y, col);
            }
            self.end_strip(s, StripTex::Flat, Mode::Alpha);
        }
        // The saturation: the luma at half size (each channel's share added), then mixed in.
        let sat = self.grade.saturation;
        if sat.abs_diff(128) > 1 && self.atmos_off & super::atmos_fx::SATURATION == 0 {
            let (hw, hh) = (w / 2, h / 2);
            self.quads.push(quad(Tex::None, Mode::RtBegin, 0xff00_0000, 0, 0, hw, hh, (0, 0, 0, 0)));
            for c in 0..3u8 {
                self.quads.push(quad(
                    Tex::Frame(c, u16::from(c)),
                    Mode::AddGlow,
                    0xffff_ffff,
                    0,
                    0,
                    hw,
                    hh,
                    (0, 0, w, h),
                ));
            }
            self.quads.push(quad(Tex::None, Mode::RtEnd, 0, 0, 0, 0, 0, (0, 0, 0, 0)));
            if sat < 128 {
                // `c * s + y * (1 - s)`.
                let k = ((128 - sat) * 255 / 128) as u32;
                self.quads.push(quad(Tex::LightRt, Mode::Desaturate, grey(k), 0, 0, w, h, (0, 0, hw, hh)));
            } else {
                // `(c - y * (s - 1) / s) * s`.
                let k = ((sat - 128) * 255 / sat) as u32;
                self.quads.push(quad(Tex::LightRt, Mode::Subtract, grey(k), 0, 0, w, h, (0, 0, hw, hh)));
                let m = (sat as u32 * 255 / 256).min(255);
                self.quads.push(quad(Tex::None, Mode::Multiply2, grey(m), 0, 0, w, h, (0, 0, 0, 0)));
            }
        }
        // The tables: a channel at a time, each band its own CLUT.
        if !self.grade.identity {
            let bands = self.grade.luts.len() as i32;
            let band = if bands > 1 { BAND as i32 } else { w };
            for c in 0..3u8 {
                for b in 0..bands {
                    let (x0, x1) = (b * band, ((b + 1) * band).min(w));
                    let k = 3 + 3 * b as u16 + u16::from(c);
                    self.quads.push(quad(Tex::Frame(c, k), Mode::Lut(c), 0xffff_ffff, x0, 0, x1, h, (x0, 0, x1, h)));
                }
            }
        }
    }

    /// The particles (§2), as `soft` draws them, their colours already lit by the presenter: a
    /// streak a line fading toward its tail, a dot a square, a ring a flat ellipse of lines, a
    /// glow the halo disc. The rain is the weather's share of them.
    pub(super) fn particles(&mut self, parts: &[Particle]) {
        // A ring's outline: twelve points round, Q8, the height halved.
        const ROUND: [(i32, i32); 12] = [
            (256, 0),
            (222, 64),
            (128, 111),
            (0, 128),
            (-128, 111),
            (-222, 64),
            (-256, 0),
            (-222, -64),
            (-128, -111),
            (0, -128),
            (128, -111),
            (222, -64),
        ];
        let (w, h) = (self.w, self.h);
        for p in parts {
            let (x, y) = (i32::from(p.x), i32::from(p.y));
            let c =
                u32::from(p.alpha) << 24 | (p.colour[2] as u32) << 16 | (p.colour[1] as u32) << 8 | p.colour[0] as u32;
            match p.shape {
                PartShape::Streak { dx, dy } => {
                    let (tx, ty) = (x + i32::from(dx), y + i32::from(dy));
                    if x.max(tx) < 0 || y.max(ty) < 0 || x.min(tx) >= w || y.min(ty) >= h {
                        continue;
                    }
                    self.quads.push(quad(Tex::Line(true), Mode::Alpha, c, x, y, tx, ty, (0, 0, 0, 0)));
                }
                PartShape::Dot { size } => {
                    let s = i32::from(size.max(1));
                    if x + s <= 0 || y + s <= 0 || x >= w || y >= h {
                        continue;
                    }
                    self.quads.push(quad(Tex::None, Mode::Alpha, c, x, y, x + s, y + s, (0, 0, 0, 0)));
                }
                PartShape::Ring { r } => {
                    let r = i32::from(r);
                    if x + r < 0 || y + r < 0 || x - r >= w || y - r >= h {
                        continue;
                    }
                    if r <= 1 {
                        self.quads.push(quad(Tex::None, Mode::Alpha, c, x - r, y, x + r + 1, y + 1, (0, 0, 0, 0)));
                        continue;
                    }
                    // Fewer sides for a small ring.
                    let step = if r < 4 { 2 } else { 1 };
                    let at = |k: usize| {
                        let (cx, cy) = ROUND[k % 12];
                        (x + (cx * r + 128).div_euclid(256), y + (cy * r + 128).div_euclid(256))
                    };
                    for k in (0..12).step_by(step) {
                        let (a, b) = (at(k), at(k + step));
                        self.quads.push(quad(Tex::Line(false), Mode::Alpha, c, a.0, a.1, b.0, b.1, (0, 0, 0, 0)));
                    }
                }
                PartShape::Glow { r } => {
                    let r = i32::from(r.max(1));
                    if x + r < 0 || y + r < 0 || x - r >= w || y - r >= h {
                        continue;
                    }
                    let d = crate::light::DISC as i32;
                    self.quads.push(quad(Tex::Disc, Mode::Alpha, c, x - r, y - r, x + r + 1, y + r + 1, (0, 0, d, d)));
                }
            }
        }
    }

    /// The fog (§1.9, T0's): one drift of the mist tile at the strongest volume in view, toward
    /// its colour by the tile's weight (a base of haze under the wisps, so thick fog is never
    /// holed: `soft`'s table, in this frame's fog CLUT), fading in over the volume's edge (the
    /// vertices' alpha), clipped to the canvas. Nothing for a volume thinner than 20 of 255.
    pub(super) fn fog(&mut self, vols: &[FogVolume], camera: (i32, i32), drift: (i16, i16)) {
        let Some(v) = vols.iter().max_by_key(|v| v.density) else { return };
        if v.density < 20 {
            return;
        }
        let (rx0, ry0, rx1, ry1) = v.rect;
        let (x0, y0, x1, y1) = (rx0.max(0), ry0.max(0), rx1.min(self.w), ry1.min(self.h));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let dens = u32::from(v.density);
        let col = (v.colour[2] as u32) << 16 | (v.colour[1] as u32) << 8 | v.colour[0] as u32;
        for (m, e) in self.fog_clut.iter_mut().enumerate() {
            let a = ((m as u32 / 2 + 64) * dens / 255).min(230);
            *e = (a * 255 / 256) << 24 | col;
        }
        let edge = i32::from(v.edge.max(1));
        // The edge's stops across and down, inside the canvas: alpha by the nearer edge.
        let stops = |a0: i32, a1: i32, c0: i32, c1: i32| {
            let mut s = [c0, (a0 + edge).clamp(c0, c1), (a1 - edge).clamp(c0, c1), c1];
            for k in 1..4 {
                s[k] = s[k].max(s[k - 1]);
            }
            s
        };
        let xs = stops(rx0, rx1, x0, x1);
        let ys = stops(ry0, ry1, y0, y1);
        let near = |p: i32, a0: i32, a1: i32| (p - a0).min(a1 - p).clamp(0, edge);
        let (ux, vy) = (
            (x0 + camera.0 - i32::from(drift.0)).rem_euclid(256),
            (y0 + camera.1 - i32::from(drift.1)).rem_euclid(256),
        );
        for r in 0..3 {
            if ys[r] >= ys[r + 1] {
                continue;
            }
            let s = self.begin_strip();
            for &x in &xs {
                for y in [ys[r], ys[r + 1]] {
                    let a = near(x, rx0, rx1).min(near(y, ry0, ry1)) * 255 / edge;
                    let (u, vv) = (ux + x - x0, vy + y - y0);
                    let colour = (a as u32) << 24 | 0x00ff_ffff;
                    self.verts.push(Vert { x: x as i16, y: y as i16, u: u as u16, v: vv as u16, colour });
                }
            }
            self.end_strip(s, StripTex::Mist, Mode::Alpha);
        }
    }

    /// The sky (§1.9, T0's) where the view shows past the zone's top edge: the backdrop's rows
    /// as smooth strips (its colour a row by `sky_at`, a column every 32 px for the afterglow),
    /// and its stars.
    pub(super) fn sky(&mut self, s: &SkyLook, stars: &[StarCmd]) {
        let top = s.zone.1.clamp(0, self.h);
        if top <= 0 {
            return;
        }
        let w = self.w;
        let mut y = 0;
        while y < top {
            let y1 = (y + 8).min(top);
            let st = self.begin_strip();
            let mut x = 0;
            loop {
                for yy in [y, y1] {
                    let c = jane_present::atmos::sky_at(s, x, top - yy);
                    self.vert(x, yy, rgba3(c, 255));
                }
                if x >= w {
                    break;
                }
                x = (x + 32).min(w);
            }
            self.end_strip(st, StripTex::Flat, Mode::Alpha);
            y = y1;
        }
        for st in stars {
            let (x, y) = (i32::from(st.x), top - i32::from(st.up));
            if y >= 0 && y < top && (0..w).contains(&x) {
                let a = (u32::from(st.bright) + 1) * 255 / 256;
                self.quads.push(quad(Tex::None, Mode::Alpha, a << 24 | 0x00ff_f0f0, x, y, x + 1, y + 1, (0, 0, 0, 0)));
            }
        }
    }

    /// A far thing on the sky's horizon (`Parallax`, T0's): its sprite standing on the zone's top
    /// edge, cut at it; drawn plain (no glow over the light, no relief).
    pub(super) fn far_thing(&mut self, s: &SkyLook, sp: &SpriteCmd, index: u32) {
        let top = s.zone.1.clamp(0, self.h);
        if top <= 0 {
            return;
        }
        let cmd = SpriteCmd { y: (top + i32::from(sp.y)) as i16, ..*sp };
        let (effects, relief) = (self.effects, self.relief.take());
        self.effects &= !(super::fx::GLOW | super::fx::LAMP_RELIEF);
        let first = self.quads.len();
        let mut flat = cmd;
        flat.flags.tint = Tint::None;
        self.sprite(&flat, &[], index);
        (self.effects, self.relief) = (effects, relief);
        // Cut at the horizon: what falls below it goes.
        let mut k = first;
        while k < self.quads.len() {
            let q = &mut self.quads[k];
            let y1 = i32::from(q.y1).min(top);
            if i32::from(q.y0) >= y1 {
                self.quads.remove(k);
                continue;
            }
            q.v1 = (i32::from(q.v1) - (i32::from(q.y1) - y1)) as u16;
            q.y1 = y1 as i16;
            k += 1;
        }
    }

    /// The water's shimmer (§1.8, T0's): a glint two px wide a water cell, walking by tick from a
    /// place the cell's phase sets, 40 steps of every 64; `soft`'s positions and strengths.
    pub(super) fn water(&mut self, cells: &[WaterCmd], tick: u32) {
        for c in cells {
            let p = u32::from(c.phase);
            let step = (tick / 6 + p * 5) % 64;
            if step >= 40 {
                continue;
            }
            let gx = i32::from(c.x) + 2 + ((p * 7 + step / 5) % 12) as i32;
            let gy = i32::from(c.y) + 3 + ((p * 3 + step / 10) % 10) as i32;
            if gy < 0 || gy >= self.h || gx + 2 <= 0 || gx >= self.w {
                continue;
            }
            // `soft` lerps by 70 or 40 of 256 toward a pale blue-white.
            let a: u32 = if step % 8 < 4 { 70 } else { 40 };
            let colour = (a * 255 / 256) << 24 | 0x00ff_f4e8;
            self.quads.push(quad(
                Tex::None,
                Mode::Alpha,
                colour,
                gx.max(0),
                gy,
                (gx + 2).min(self.w),
                gy + 1,
                (0, 0, 0, 0),
            ));
        }
    }
}
