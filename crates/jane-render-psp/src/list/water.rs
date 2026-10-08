//! The water and the wet ground as T1 draws them (PRESENTATION.md §1.8; `jane-render-gl2`'s
//! compose), as far as the GE goes: the frame's water and wet ground marked in the stencil from
//! the chunks' CLUTs (a console chunk's colours carry what the ground is, `frame::T8_WATER`, ..),
//! then over the light: wet ground darker; puddles where T1's noise lies over the rain's edge;
//! the sky mirrored in the water and the puddles, the water's crest and trough rows; what stands
//! over them mirrored, rippled row by row and tinted toward the water; each lamp's glint run
//! long down them toward her. The stencil is cleared after. No VRAM: the mirrored things are their
//! own pages again.

use jane_present::Frame;
use jane_present::frame::{CHUNK_PX, SkyLook, T8_SHINE, T8_WATER, T8_WET};

use super::{Blend, Lister, Mode, Quad, StripTex, Tex, Where, atmos_fx, marks};
use crate::water::{NOISE, isin, puddle_edge, turns};

/// T1's water: the sky's share of a shallow pond (`0.55 + 0.3 * deep`) and its tint, of 256.
const SHARE: u32 = 184;
const TINT: [u32; 3] = [196, 218, 232];
/// The rows a mirrored thing is laid in at once (each its own ripple).
const BAND: i32 = 2;
/// The far things' share over the mirrored sky: hazed toward it, as T1's backdrop holds them.
const FAR: u32 = 104;
/// `jane_present::light::pool` by full day (its `POOL_DAY`).
const POOL_DAY: u32 = 20;

const fn quad(tex: Tex, mode: Mode, colour: u32, r: (i32, i32, i32, i32), uv: (i32, i32, i32, i32)) -> Quad {
    Quad {
        tex,
        mode,
        colour,
        x0: r.0 as i16,
        y0: r.1 as i16,
        x1: r.2 as i16,
        y1: r.3 as i16,
        u0: uv.0 as u16,
        v0: uv.1 as u16,
        u1: uv.2 as u16,
        v1: uv.3 as u16,
    }
}

/// `q` cut to `b` (its texels with it, linearly): false when nothing is left.
fn clip(q: &mut Quad, b: (i32, i32, i32, i32)) -> bool {
    let (x0, y0, x1, y1) = (i32::from(q.x0), i32::from(q.y0), i32::from(q.x1), i32::from(q.y1));
    let (cx0, cy0, cx1, cy1) = (x0.max(b.0), y0.max(b.1), x1.min(b.2), y1.min(b.3));
    if cx0 >= cx1 || cy0 >= cy1 {
        return false;
    }
    let lerp = |a: u16, z: u16, p: i32, p0: i32, p1: i32| {
        (i32::from(a) + (i32::from(z) - i32::from(a)) * (p - p0) / (p1 - p0)) as u16
    };
    let (u0, u1, v0, v1) = (q.u0, q.u1, q.v0, q.v1);
    q.u0 = lerp(u0, u1, cx0, x0, x1);
    q.u1 = lerp(u0, u1, cx1, x0, x1);
    q.v0 = lerp(v0, v1, cy0, y0, y1);
    q.v1 = lerp(v0, v1, cy1, y0, y1);
    (q.x0, q.y0, q.x1, q.y1) = (cx0 as i16, cy0 as i16, cx1 as i16, cy1 as i16);
    true
}

const fn grey(v: u32) -> u32 {
    0xff00_0000 | v << 16 | v << 8 | v
}

/// The ripple's shift at world px `(wx, wy)` and tick `t`, whole px (T1's two swells, quickened by
/// the wind and the rain; `rough` of 256).
fn ripple(wx: i32, wy: i32, t: u32, rough: i32) -> i32 {
    let t = t as i32;
    let a = isin(wy.wrapping_mul(S055).wrapping_add(t.wrapping_mul(S0045))) * 9 / 10;
    let b = isin(wy.wrapping_mul(S17).wrapping_sub(t.wrapping_mul(S011)).wrapping_add(wx.wrapping_mul(S004)))
        * (128 + rough * 154 / 256)
        / 256;
    (a + b + 2048) >> 12
}

// T1's ripple and crest rates, in turns of 65536 a px or a tick.
const S055: i32 = turns(550);
const S0045: i32 = turns(45);
const S17: i32 = turns(1700);
const S011: i32 = turns(110);
const S004: i32 = turns(40);
const S083: i32 = turns(830);
const S005: i32 = turns(50);
const S013: i32 = turns(130);
/// 2.4 radians in turns, of a sine's 4096.
const S24: i32 = turns(2400);

impl Lister {
    /// The water's and the wet ground's passes, over the light (see the module).
    pub(super) fn water_fx(&mut self, frame: &Frame) {
        let off = self.atmos_off;
        let (reflect, streaks) = (off & atmos_fx::REFLECT == 0, off & atmos_fx::STREAKS == 0);
        let a = self.weather.unwrap_or_default();
        let wet = if off & atmos_fx::PUDDLES == 0 { a.wet } else { 0 };
        let pools = wet > 0;
        let water: alloc::vec::Vec<(i32, i32, u16)> = self
            .placed
            .iter()
            .copied()
            .filter(|&(_, _, s)| frame.layers.get(usize::from(s)).is_some_and(|l| l.is_t8() && !l.water.is_empty()))
            .collect();
        let has_water = !water.is_empty() && (reflect || streaks);
        if !has_water && !pools {
            return;
        }
        let (w, h) = (self.w, self.h);
        // Where the water and the wet ground can be: the water's cells' bounds, or the whole view
        // in rain. Everything here is drawn inside it (the stencil past it is the shadows').
        let mut bounds = if pools { Some((0, 0, w, h)) } else { None };
        if has_water {
            for c in &frame.water {
                let (x, y) = (i32::from(c.x), i32::from(c.y));
                let b = bounds.get_or_insert((x, y, x + 16, y + 16));
                *b = (b.0.min(x), b.1.min(y), b.2.max(x + 16), b.3.max(y + 16));
            }
        }
        let Some(b) =
            bounds.map(|b| (b.0.max(0), b.1.max(0), b.2.min(w), b.3.min(h))).filter(|b| b.0 < b.2 && b.1 < b.3)
        else {
            return;
        };
        let all = b;
        let none = (0, 0, 0, 0);
        let first = self.quads.len();
        self.quads.push(quad(Tex::None, Mode::StencilClear, 0xff00_0000, all, none));
        let chunk = |_: &Self, cx: i32, cy: i32, slot: u16, mode: Mode| -> Option<Quad> {
            let (x0, y0, x1, y1) = (cx.max(b.0), cy.max(b.1), (cx + CHUNK_PX).min(b.2), (cy + CHUNK_PX).min(b.3));
            (x0 < x1 && y0 < y1).then(|| {
                quad(Tex::Chunk(slot), mode, 0xffff_ffff, (x0, y0, x1, y1), (x0 - cx, y0 - cy, x1 - cx, y1 - cy))
            })
        };
        // Wet ground: marked, darker (T1: `1 - 0.4 * wet`, a matt cell's a little over half).
        let edge = if pools { puddle_edge(wet) } else { None };
        if pools {
            let t8 = self.placed.clone();
            for (tag, value) in [(T8_WET, marks::WET), (T8_SHINE, marks::SHINE)] {
                for &(cx, cy, slot) in &t8 {
                    if let Some(q) = chunk(self, cx, cy, slot, Mode::Mark { tag, value, keep: 0, need: None }) {
                        self.quads.push(q);
                    }
                }
            }
            let dark = |share: u32| grey(255 - u32::from(wet) * 102 * share / (255 * 256));
            self.quads.push(quad(
                Tex::None,
                Mode::Masked(Blend::Multiply, Where::Bits(marks::WET, marks::GROUND)),
                dark(141),
                all,
                none,
            ));
            self.quads.push(quad(
                Tex::None,
                Mode::Masked(Blend::Multiply, Where::Bits(marks::SHINE, marks::GROUND)),
                dark(256),
                all,
                none,
            ));
            // The sky's sheen on it (T1: `fill^2 * shine * 0.07` in linear light, about an eighth of
            // the fill over the darks): a shining cell's whole, a matt one's a third.
            let fill = self.fill;
            let sheen = |a: u32| a << 24 | u32::from(fill[2]) << 16 | u32::from(fill[1]) << 8 | u32::from(fill[0]);
            let k = u32::from(wet);
            self.quads.push(quad(
                Tex::None,
                Mode::Masked(Blend::Add, Where::Bits(marks::SHINE, marks::GROUND)),
                sheen(k * 30 / 255),
                all,
                none,
            ));
            self.quads.push(quad(
                Tex::None,
                Mode::Masked(Blend::Add, Where::Bits(marks::WET, marks::GROUND)),
                sheen(k * 11 / 255),
                all,
                none,
            ));
            // The puddles: T1's noise over the rain's edge, on wet ground only.
            if let Some(e) = edge {
                for (v, c) in self.noise_clut.iter_mut().enumerate() {
                    *c = if v as u32 > e { 0xffff_ffff } else { 0 };
                }
                let n = NOISE as i32;
                let (u, v) = ((frame.camera.0).rem_euclid(n), (frame.camera.1).rem_euclid(n));
                self.quads.push(quad(
                    Tex::Noise,
                    Mode::Mark { tag: 0, value: marks::PUDDLE, keep: marks::GROUND, need: Some(marks::GROUND) },
                    0xffff_ffff,
                    all,
                    (u + b.0, v + b.1, u + b.2, v + b.3),
                ));
            }
        }
        if has_water {
            for &(cx, cy, slot) in &water {
                if let Some(q) =
                    chunk(self, cx, cy, slot, Mode::Mark { tag: T8_WATER, value: marks::WATER, keep: 0, need: None })
                {
                    self.quads.push(q);
                }
            }
        }
        // What is drawn over the water and the wet ground (a lily pad, a reed, her feet) is not
        // water: its px unmarked again from its own quads.
        {
            for k in 0..first {
                let q = self.quads[k];
                if matches!(q.tex, Tex::Page(_))
                    && matches!(q.mode, Mode::Alpha)
                    && i32::from(q.x1) > b.0
                    && i32::from(q.x0) < b.2
                    && i32::from(q.y1) > b.1
                    && i32::from(q.y0) < b.3
                {
                    self.quads.push(Quad { mode: Mode::Mark { tag: 0, value: 0, keep: 0, need: None }, ..q });
                }
            }
        }
        let rough = i32::from(a.wind.unsigned_abs()) * 32 + i32::from(a.rain) * 3 / 2;
        if reflect && let Some(sky) = self.sky {
            if has_water {
                self.sky_mirror(&sky, b, Where::Is(marks::WATER), TINT, SHARE);
                let t0 = self.now();
                self.crests(frame, b, &sky, rough);
                self.prof[7] += self.now().wrapping_sub(t0);
            }
            if edge.is_some() {
                // A puddle: T1's `mix(c * 0.8, refl * 0.9, 0.5)` in linear light, as display values.
                self.quads.push(quad(
                    Tex::None,
                    Mode::Masked(Blend::Multiply, Where::Bits(marks::PUDDLE, marks::PUDDLE)),
                    grey(229),
                    all,
                    none,
                ));
                self.sky_mirror(&sky, b, Where::Bits(marks::PUDDLE, marks::PUDDLE), [230, 230, 230], 150);
            }
            let lit = self.ambient.map(|c| (u32::from(c) * 3 / 2).min(255));
            let tint = [0, 1, 2].map(|c| TINT[c] * lit[c] / 255);
            let colour = SHARE.min(255) << 24 | tint[2] << 16 | tint[1] << 8 | tint[0];
            // The far things on the backdrop hang in it too, where the sky does (T1's backdrop
            // laid down the screen: its row `up` at canvas row `(up + 2) / 0.62`).
            for p in &frame.passes {
                if let jane_present::Pass::Parallax { sprites, .. } = *p {
                    for sp in frame.sprites_in(sprites) {
                        let t0 = self.now();
                        self.far_mirror(sp, b, colour & 0x00ff_ffff | FAR << 24, (frame.tick, frame.camera, rough));
                        self.prof[0] += self.now().wrapping_sub(t0);
                    }
                }
            }
            // What stands over them (people, creatures, the ducks, props, trees), mirrored at the
            // foot of what it draws.
            for p in &frame.passes {
                let jane_present::Pass::Sprites { layer: jane_present::Depth::Standing, cmds } = *p else { continue };
                for s in frame.sprites_in(cmds) {
                    let top = i32::from(s.y);
                    let foot = top + i32::from(s.src.h);
                    let (x0, x1) = (i32::from(s.x), i32::from(s.x) + i32::from(s.src.w));
                    if s.flags.tint != jane_present::Tint::None
                        || foot >= b.3
                        || foot + (foot - top) <= b.1
                        || x1 <= b.0
                        || x0 >= b.2
                    {
                        continue;
                    }
                    self.mirror(s, foot, colour, frame.tick, (frame.camera, rough));
                }
            }
        }
        if streaks {
            self.streaks(frame, b, rough);
        }
        // The stencil is left as it is: whatever reads it next clears it first.
        if self.quads.len() == first + 1 {
            self.quads.truncate(first);
        }
    }

    /// The sky mirrored where `at` holds inside `b`: T1's `sky_px(x, 0.62 y - 2)`, tinted, `share`
    /// of 256 over what is there; smooth strips 16 rows deep, a column every 64 px.
    fn sky_mirror(&mut self, sky: &SkyLook, b: (i32, i32, i32, i32), at: Where, tint: [u32; 3], share: u32) {
        let a = share.min(255);
        let mut y = b.1;
        while y < b.3 {
            let y1 = (y + 16).min(b.3);
            let st = self.begin_strip();
            let mut x = b.0;
            loop {
                for yy in [y, y1] {
                    let c = jane_present::atmos::sky_at(sky, x, yy * 62 / 100 - 2);
                    let t = [0, 1, 2].map(|k| u32::from(c[k]) * tint[k] / 256);
                    self.vert(x, yy, a << 24 | t[2] << 16 | t[1] << 8 | t[0]);
                }
                if x >= b.2 {
                    break;
                }
                x = (x + 64).min(b.2);
            }
            self.end_strip(st, StripTex::Flat, Mode::Masked(Blend::Alpha, at));
            y = y1;
        }
    }

    /// T1's shimmer rows: where a row catches the sky brighter (a crest) or lies in a trough,
    /// wandering with the swell and quickened by the wind; 32-px runs, the water's alone.
    fn crests(&mut self, frame: &Frame, b: (i32, i32, i32, i32), sky: &SkyLook, rough: i32) {
        const STEP: i32 = 16;
        let t = frame.tick as i32;
        let (cx, cy) = frame.camera;
        let hi = 3809 - rough * 205 / 256;
        let at = Mode::Masked(Blend::Add, Where::Is(marks::WATER));
        let dim = Mode::Masked(Blend::Multiply, Where::Is(marks::WATER));
        self.water_dim.clear();
        for y in b.1..b.3 {
            let wy = y + cy;
            let base = wy.wrapping_mul(S083).wrapping_add(t.wrapping_mul(S005));
            let mut arg = (b.0 + STEP / 2 + cx).wrapping_mul(S0045).wrapping_add(wy.wrapping_mul(S013));
            let mut bright: Option<u32> = None;
            let mut run: Option<(i32, i8)> = None;
            let mut x = b.0;
            while x <= b.2 {
                let state = if x < b.2 {
                    let inner = (isin(arg) * (S24 >> 4)) >> 8;
                    arg = arg.wrapping_add(STEP * S0045);
                    let crest = isin(base.wrapping_add(inner));
                    i8::from(crest > hi) - i8::from(crest < -3686)
                } else {
                    0
                };
                match run {
                    Some((x0, s)) if s != state => {
                        let r = (x0, y, x, y + 1);
                        if s > 0 {
                            // A crest catches the sky brighter (T1: 1.28 of its share).
                            let c = *bright.get_or_insert_with(|| {
                                let c = jane_present::atmos::sky_at(sky, (b.0 + b.2) / 2, y * 62 / 100 - 2);
                                0x2c00_0000 | u32::from(c[2]) << 16 | u32::from(c[1]) << 8 | u32::from(c[0])
                            });
                            self.quads.push(quad(Tex::None, at, c, r, (0, 0, 0, 0)));
                        } else {
                            self.water_dim.push(quad(Tex::None, dim, grey(233), r, (0, 0, 0, 0)));
                        }
                        run = (state != 0).then_some((x, state));
                    }
                    None if state != 0 => run = Some((x, state)),
                    _ => {}
                }
                x += STEP;
            }
        }
        // The troughs after the crests: one batch each.
        self.quads.append(&mut self.water_dim);
    }

    /// Sprite `s` mirrored at canvas row `foot` (its row `foot - 1 - k` laid at `foot + k`), in
    /// bands of rows each shifted by the ripple there, `colour` over the water and the puddles.
    fn mirror(
        &mut self,
        s: &jane_present::SpriteCmd,
        foot: i32,
        colour: u32,
        tick: u32,
        (cam, rough): ((i32, i32), i32),
    ) {
        let Some(i) = self.find(s.page, s.src.x, s.src.y) else { return };
        let Some(t) = self.targets[i] else { return };
        let (rx, ry) = self.rects[i];
        let (lx0, ly0) = (i32::from(s.src.x) - i32::from(rx), i32::from(s.src.y) - i32::from(ry));
        let (sw, sh) = (i32::from(s.src.w), i32::from(s.src.h));
        let (tx, ty) = (i32::from(t.tx), i32::from(t.ty));
        let ix0 = lx0.max(tx);
        let ix1 = (lx0 + sw).min(tx + i32::from(t.w));
        let iy0 = ly0.max(ty);
        // Only what stands over the foot is mirrored.
        let iy1 = (ly0 + sh).min(ty + i32::from(t.h)).min(ly0 + foot - i32::from(s.y));
        if ix0 >= ix1 || iy0 >= iy1 {
            return;
        }
        // T1 mirrors only what stands up (a px `h` up shows `2h` under itself).
        // What moves (people, creatures, critters) always stands up; a prop only when it is tall
        // and its rows are its height, not a footprint (a fountain's basin, a bench, a bed).
        let moves = matches!(t.cat, crate::pack::UNITS | crate::pack::SCENE);
        let top = i32::from(self.tops[i]);
        if !moves && (top < 14 || iy1 - iy0 > top + 6) {
            return;
        }
        // The mirror's line: under the lowest row it draws.
        let foot = foot.min(i32::from(s.y) + iy1 - ly0);
        let mirror = s.flags.mirror;
        let x = i32::from(s.x) + if mirror { lx0 + sw - ix1 } else { ix0 - lx0 };
        let u = i32::from(t.u) + ix0 - tx;
        let wide = ix1 - ix0;
        let mode = Mode::Masked(Blend::Alpha, Where::Over(marks::SHINE));
        let mut r1 = iy1;
        while r1 > iy0 {
            let r0 = (r1 - BAND).max(iy0);
            // Source rows r0..r1 land at canvas rows y0..y1, the last row first.
            let y0 = 2 * foot - (i32::from(s.y) + r1 - ly0);
            let y1 = y0 + (r1 - r0);
            r1 = r0;
            if y1 <= 0 || y0 >= self.h {
                continue;
            }
            let dx = ripple(x + cam.0, y0 + cam.1, tick, rough);
            let (cy0, cy1) = (y0.max(0), y1.min(self.h));
            // Flipped: v runs from one past the band's last row back to its first.
            let v_top = i32::from(t.v) + (r0 + (y1 - y0)) - ty - (cy0 - y0);
            let v_bot = v_top - (cy1 - cy0);
            let (cx0, cx1) = ((x + dx).max(0), (x + dx + wide).min(self.w));
            if cx0 >= cx1 {
                continue;
            }
            let (u0, u1) = if mirror {
                let first = u + wide - (cx0 - x - dx);
                (first, first - (cx1 - cx0))
            } else {
                let first = u + (cx0 - x - dx);
                (first, first + (cx1 - cx0))
            };
            self.quads.push(quad(Tex::Page(t.page), mode, colour, (cx0, cy0, cx1, cy1), (u0, v_top, u1, v_bot)));
        }
    }

    /// A far thing of the backdrop mirrored in the water as T1 lays the backdrop there: its row
    /// at `up` px over the horizon at canvas row `(up + 2) * 100 / 62`, a quad a row, rippled.
    fn far_mirror(
        &mut self,
        sp: &jane_present::SpriteCmd,
        b: (i32, i32, i32, i32),
        colour: u32,
        (tick, cam, rough): (u32, (i32, i32), i32),
    ) {
        let Some(i) = self.find(sp.page, sp.src.x, sp.src.y) else { return };
        let Some(t) = self.targets[i] else { return };
        let (rx, ry) = self.rects[i];
        let (lx0, ly0) = (i32::from(sp.src.x) - i32::from(rx), i32::from(sp.src.y) - i32::from(ry));
        let (sw, sh) = (i32::from(sp.src.w), i32::from(sp.src.h));
        let (tx, ty) = (i32::from(t.tx), i32::from(t.ty));
        let (ix0, ix1) = (lx0.max(tx), (lx0 + sw).min(tx + i32::from(t.w)));
        let (iy0, iy1) = (ly0.max(ty), (ly0 + sh).min(ty + i32::from(t.h)));
        if ix0 >= ix1 || iy0 >= iy1 {
            return;
        }
        let x = i32::from(sp.x) + if sp.flags.mirror { lx0 + sw - ix1 } else { ix0 - lx0 };
        let u = i32::from(t.u) + ix0 - tx;
        let wide = ix1 - ix0;
        let mode = Mode::Masked(Blend::Alpha, Where::Over(marks::SHINE));
        for r in iy0..iy1 {
            // Its row's height over the horizon (`sp.y` is the sprite's top, up from it).
            let up = -i32::from(sp.y) - (r - ly0) - 1;
            let (y0, y1) = ((up + 2) * 100 / 62, (up + 3) * 100 / 62);
            if y1 <= y0 || y1 <= b.1 || y0 >= b.3 {
                continue;
            }
            let dx = ripple(x + cam.0, y0 + cam.1, tick, rough);
            let (cx0, cx1) = ((x + dx).max(b.0), (x + dx + wide).min(b.2));
            if cx0 >= cx1 {
                continue;
            }
            let v = i32::from(t.v) + r - ty;
            let (u0, u1) = if sp.flags.mirror {
                let first = u + wide - (cx0 - x - dx);
                (first, first - (cx1 - cx0))
            } else {
                let first = u + (cx0 - x - dx);
                (first, first + (cx1 - cx0))
            };
            self.quads.push(quad(
                Tex::Page(t.page),
                mode,
                colour,
                (cx0, y0.max(b.1), cx1, y1.min(b.3)),
                (u0, v, u1, v + 1),
            ));
        }
    }

    /// Each lamp's glint on the water and the wet ground (T1's Blinn term toward the 3/4 eye): a
    /// broken streak run down from under it toward her, in its light's hue, stronger as the dark
    /// comes, each two rows shifted by the ripple.
    fn streaks(&mut self, frame: &Frame, b: (i32, i32, i32, i32), rough: i32) {
        // Full at night, none by day: the pool's own strength over its day's.
        let dark = jane_present::light::pool(self.ambient).saturating_sub(POOL_DAY) * 256 / (256 - POOL_DAY).max(1);
        if dark < 16 {
            return;
        }
        for l in &frame.lights {
            let (lx, ly) = l.pos;
            let len = (i32::from(l.radius) / 3).clamp(12, 48);
            if lx + 4 <= b.0 || lx - 4 >= b.2 || ly + len <= b.1 || ly >= b.3 {
                continue;
            }
            let d = crate::light::DISC as i32;
            for j in (2..len).step_by(3) {
                let y = ly + j;
                if y < b.1 || y >= b.3 {
                    continue;
                }
                // A soft disc every three rows, smaller and fainter the further down: where the
                // water begins under the lamp, the brightest that falls on it is the glint.
                let f = 256 - j * 256 / len;
                let a = (f * f / 256 * 3 / 2) as u32 * dark.min(256) / 256;
                let r = 2 + f * 5 / 256;
                let dx = ripple(lx + frame.camera.0, y + frame.camera.1, frame.tick, rough);
                let c = a.min(255) << 24
                    | u32::from(l.colour[2]) << 16
                    | u32::from(l.colour[1]) << 8
                    | u32::from(l.colour[0]);
                self.quads.push(quad(
                    Tex::Spot,
                    Mode::Masked(Blend::Add, Where::Over(0)),
                    c,
                    (lx + dx - r, y - r, lx + dx + r + 1, y + r + 1),
                    (0, 0, d, d),
                ));
                // Inside the bounds only: past them the stencil is the shadows'.
                let q = self.quads.last_mut().expect("pushed");
                if !clip(q, b) {
                    self.quads.pop();
                }
            }
        }
    }
}
