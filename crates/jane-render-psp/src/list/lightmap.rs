//! The lamps on C2: the lightmap on the GE (`gpu_lightmap`: the ambient, each pool, the lamps'
//! shadows marked in its stencil from the casters' slabs and the blocks' sides), a sprite's lamp
//! CLUT (`lamp_relief`), the chunks' glows, and the night's band as strips (NIGHT.md §4.7).
//! `gpu_lightmap` is one long fn and is left whole.

use alloc::vec::Vec;

use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, SpriteCmd};

use super::{LAMP_RELIEFS, Lister, Mode, POLYS_MOST, PagePx, Poly, Quad, QuadBuf, Tex, abgr, fx};

/// The lightmap target's side, cells (a mark past it is dropped).
const SIDE_CELLS: i32 = crate::light::SIDE as i32;
/// A caster's rows a lamp's slab pair spans: two of the lightmap's cells.
const BAND: i32 = 2 * crate::light::CELL;
/// The still casters a light not cached (her lantern) casts a frame at most, nearest first.
const PLAIN_CASTERS: usize = 24;

/// Block `b`'s shadow from `lamp` on the lightmap's cells (`STEP` [`SUB`](jane_present::shadow::SUB)
/// steps a cell): its footprint's corners at its foot (or a rail's underside) and at its top, each
/// laid on the ground along the ray from the light (`t = d * h / (h - z)`, held to the light's
/// reach and [`SHADOW_PAST`](jane_present::shadow::SHADOW_PAST) past it, as `shadow::project`),
/// and its sides turned from the light as quads between them (the stencil takes their union).
/// `None` for a block too low, too far from the light, or off the target.
fn lamp_block_poly(
    b: &jane_present::Block,
    lamp: &jane_present::shadow::Lamp,
    (w, h): (i32, i32),
) -> Option<([Poly; 4], usize)> {
    use jane_present::shadow::{self, SUB};
    // A constant, a power of two: its floor divisions are shifts, not sixteen divides a block.
    const STEP: i32 = SUB * crate::light::CELL;
    const _: () = assert!(STEP.count_ones() == 1);
    let step = STEP;
    let hgt = i32::from(b.height);
    if hgt <= shadow::GROUND {
        return None;
    }
    let (x0, y0, x1, y1) = (i32::from(b.x0) * SUB, i32::from(b.y0) * SUB, i32::from(b.x1) * SUB, i32::from(b.y1) * SUB);
    // Near enough the light to throw any of it (`block_slabs`'s rule); most of a frame's blocks
    // are further off than that along one axis alone, told without a multiply.
    let (nx, ny) = (lamp.x.clamp(x0, x1) - lamp.x, lamp.y.clamp(y0, y1) - lamp.y);
    let near = (lamp.r + 48) * SUB;
    if nx.abs() > near
        || ny.abs() > near
        || i64::from(nx) * i64::from(nx) + i64::from(ny) * i64::from(ny) > i64::from(near) * i64::from(near)
    {
        return None;
    }
    // A light inside its footprint lights nothing past it: no shadow drawn.
    if nx == 0 && ny == 0 {
        return None;
    }
    let lo = i32::from(b.lo).min(hgt);
    let far = (lamp.r + shadow::SHADOW_PAST) * SUB;
    let lh = lamp.h.max(1);
    let lay = |(px, py): (i32, i32), z: i32| -> (i32, i32) {
        let (dx, dy) = (px - lamp.x, py - lamp.y);
        // Along the ray `h / (h - z)` times as far, unless that passes `far` (or the point is at
        // or over the light): then `far` from it, the one case that needs the distance.
        let d2 = i64::from(dx * dx + dy * dy);
        let over = 2 * z >= 2 * lh - 1
            || d2 * i64::from(lh) * i64::from(lh) > i64::from(far) * i64::from(far) * i64::from((lh - z) * (lh - z));
        if over {
            let d = isqrt(d2 as u32).max(1) as i32;
            (lamp.x + dx * far / d, lamp.y + dy * far / d)
        } else {
            (lamp.x + dx * lh / (lh - z), lamp.y + dy * lh / (lh - z))
        }
    };
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut pts = [(0i32, 0i32); 8];
    for (k, &c) in corners.iter().enumerate() {
        let (ax, ay) = lay(c, lo);
        let (bx, by) = lay(c, hgt);
        pts[k] = (ax.div_euclid(step), ay.div_euclid(step));
        pts[4 + k] = (bx.div_euclid(step), by.div_euclid(step));
    }
    let (mx0, my0) = pts.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
    let (mx1, my1) = pts.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
    if mx1 < 0 || my1 < 0 || mx0 >= w || my0 >= h {
        return None;
    }
    // Its sides turned from the light (`block_slabs`'s), each a quad from its foot to its top.
    let sides = [(lamp.y > y0, 0, 1), (lamp.y < y1, 3, 2), (lamp.x > x0, 0, 3), (lamp.x < x1, 1, 2)];
    let mut out = [([(0i16, 0i16); 8], 0u8); 4];
    let mut n = 0;
    let c16 = |p: (i32, i32)| (p.0.clamp(-2048, 2047) as i16, p.1.clamp(-2048, 2047) as i16);
    for (away, a, b) in sides {
        if away {
            out[n].0[..4].copy_from_slice(&[c16(pts[a]), c16(pts[b]), c16(pts[4 + b]), c16(pts[4 + a])]);
            out[n].1 = 4;
            n += 1;
        }
    }
    Some((out, n))
}

/// The lightmap's cells over the px fence part `b`'s fence draws over (`shadow::fence_drawn`),
/// every cell they touch, held to the target's `(w, h)` cells: what its lamp shadows are cleared
/// from. `None` for a block not a fence's, or one off the target.
fn fence_cells(b: &jane_present::Block, (w, h): (i32, i32)) -> Option<(i32, i32, i32, i32)> {
    use crate::light::CELL;
    let (x0, y0, x1, y1) = jane_present::shadow::fence_drawn(b)?;
    let (cx0, cy0) = (x0.div_euclid(CELL).max(0), y0.div_euclid(CELL).max(0));
    let (cx1, cy1) = ((x1 + CELL - 1).div_euclid(CELL).min(w), (y1 + CELL - 1).div_euclid(CELL).min(h));
    (cx0 < cx1 && cy0 < cy1).then_some((cx0, cy0, cx1, cy1))
}

/// The integer square root.
fn isqrt(n: u32) -> u32 {
    // The floor of the root, as Newton's from `n` gave it, in a few steps rather than a score.
    n.isqrt()
}

/// `c` at `k` of 256 (the night's turn's share of the sky's light, over 256 on its crest).
pub(super) fn dim(c: [u8; 3], k: u32) -> [u8; 3] {
    c.map(|v| (u32::from(v) * k.min(512) / 256).min(255) as u8)
}

/// `a - b` per channel of two `0xAABBGGRR` colours, floored at 0, alpha full.
pub(super) fn sub_rgb(a: u32, b: u32) -> u32 {
    let ch = |s: u32| ((a >> s) & 0xff).saturating_sub((b >> s) & 0xff) << s;
    0xff00_0000 | ch(16) | ch(8) | ch(0)
}

/// The night's turn as horizontal strips over `rows` rows (each `unit` canvas px tall: a
/// lightmap cell's, or a px): `f(y0, y1, share)` for each run of rows whose share of the sky's
/// light (`Band::at`, sixteenths through the soft edge) is the same, top to bottom. A band of
/// nothing is one strip at all of it.
pub(super) fn band_strips(band: jane_present::Band, rows: i32, unit: i32, mut f: impl FnMut(i32, i32, u32)) {
    let unit = unit.max(1);
    // Through the soft edge a strip is at least 8 canvas px, the GE's filter softening the steps.
    let step = (8 / unit).max(1);
    let share = |y: i32| {
        let k = band.at(y * unit + step * unit / 2);
        if k == 256 { 256 } else { k / 16 * 16 }
    };
    let mut y = 0;
    while y < rows {
        let k = share(y);
        let mut y1 = (y + step).min(rows);
        while y1 < rows && share(y1) == k {
            y1 = (y1 + step).min(rows);
        }
        f(y, y1, k);
        y = y1;
    }
}

impl Lister {
    /// The CLUT of the lamp that lights sprite `s` most at its middle, if one does and the
    /// effect is on: its index in `lamp_reliefs`.
    pub(super) fn lamp_relief(&mut self, lights: &[jane_present::Light], s: &SpriteCmd, index: u32) -> Option<u16> {
        if self.effects & fx::LAMP_RELIEF == 0 || self.lamp_reliefs.len() >= LAMP_RELIEFS {
            return None;
        }
        let (fx_, fy) = (i32::from(s.x) + i32::from(s.src.w) / 2, i32::from(s.y) + i32::from(s.src.h));
        let mut best: Option<(u32, &jane_present::Light)> = None;
        for l in lights {
            // Not by the light it holds (her lantern lights her evenly).
            if l.holder == Some(index) {
                continue;
            }
            let r = i32::from(l.radius);
            let (dx, dy) = (l.pos.0 - fx_, l.pos.1 - fy);
            let d2 = dx * dx + dy * dy;
            if r <= 0 || d2 >= r * r {
                continue;
            }
            let k = crate::light::falloff_at((d2 as u32) * 255 / (r * r) as u32);
            if best.is_none_or(|b| k > b.0) {
                best = Some((k, l));
            }
        }
        let (k, l) = best?;
        // Toward the light from the sprite's middle, the light's glass its height over it.
        let mid = i32::from(s.src.h) / 2;
        let d = (l.pos.0 - fx_, l.pos.1 - fy, i32::from(l.height) - mid);
        // A pool counts for its strength there, at most half again or half less.
        self.lamp_reliefs.push(crate::normals::clut_lamp(d, (k * 3 / 4).min(256)));
        Some((self.lamp_reliefs.len() - 1) as u16)
    }

    /// The chunks' glowing px (`ChunkLayers::glow`: lit windows, lamps in the walls) as runs of
    /// one colour along a row, added over the light.
    pub(super) fn chunk_glows(&mut self, frame: &Frame) {
        for &(cx, cy, slot) in &self.placed {
            let Some(l) = frame.layers.get(usize::from(slot)) else { continue };
            let mut run: Option<(i32, i32, i32, u32)> = None;
            let flush = |r: Option<(i32, i32, i32, u32)>, out: &mut QuadBuf| {
                let Some((x0, x1, y, c)) = r else { return };
                let (x0, x1) = (cx + x0, cx + x1);
                let y = cy + y;
                if y < 0 || y >= self.h || x1 <= 0 || x0 >= self.w {
                    return;
                }
                out.push(Quad {
                    tex: Tex::None,
                    mode: Mode::AddGlow,
                    colour: abgr(c) | 0xff00_0000,
                    x0: x0.max(0) as i16,
                    y0: y as i16,
                    x1: x1.min(self.w) as i16,
                    y1: (y + 1) as i16,
                    u0: 0,
                    v0: 0,
                    u1: 0,
                    v1: 0,
                });
            };
            for &(k, c) in &l.glow {
                let (x, y) = (i32::from(k) % CHUNK_PX, i32::from(k) / CHUNK_PX);
                run = match run {
                    Some((x0, x1, ry, rc)) if ry == y && x1 == x && rc == c => Some((x0, x + 1, ry, rc)),
                    r => {
                        flush(r, &mut self.glows);
                        Some((x, x + 1, y, c))
                    }
                };
            }
            flush(run, &mut self.glows);
        }
    }

    /// The lightmap on the GE (`soft`'s method, at half the canvas each way): its target cleared
    /// to the ambient and each light's pool added, every light the presenter lets cast with its
    /// shadows (a console's presenter lets all of them). A light that stands still takes its pool
    /// from the lamp cache ([`crate::lamps`]: built once with the shadows of what stands still
    /// round it, laid bilinear, so soft-edged) and casts only what moves (people, creatures) each
    /// frame; a light that moves (her lantern), or one not cached yet, casts what moves and
    /// its nearest still casters each frame. A frame's casts are marked in the target's stencil
    /// (casters' slabs in bands of rows, coarser further off, blocks' sides turned from the light, projected from it), the pool added
    /// outside them and only its bounce inside, so the other lights fill them; what holds a light
    /// casts none. The frame is then multiplied by the target.
    pub(super) fn gpu_lightmap(
        &mut self,
        frame: &Frame,
        ambient: jane_present::Rgb,
        band: jane_present::Band,
        lights: &[jane_present::Light],
        (casters, blocks): (jane_present::Span, jane_present::Span),
        px: &mut dyn PagePx,
    ) {
        use crate::lamps::{TEX, Use};
        use crate::light::{CELL, POOL, SIDE, base_colour, pool_colour};
        use jane_present::shadow::{self, Lamp, Slab};
        let (w, h) = ((self.w / CELL + 2).min(SIDE as i32), (self.h / CELL + 2).min(crate::light::ROWS as i32 - 1));
        (self.light.w, self.light.h) = (w, h);
        let rt = |mode: Mode, colour: u32, tex: Tex, (x0, y0, x1, y1): (i32, i32, i32, i32), uv: u16| Quad {
            tex,
            mode,
            colour,
            x0: x0 as i16,
            y0: y0 as i16,
            x1: x1 as i16,
            y1: y1 as i16,
            u0: 0,
            v0: 0,
            u1: uv,
            v1: uv,
        };
        let all = (0, 0, w, h);
        // Cleared to the ambient; through the night's turn to its dark side's, and the rows the
        // band has reached given the rest back (NIGHT.md §4.7: the band as quads on the
        // lightmap, no read-back).
        let clear = base_colour(dim(ambient, u32::from(band.dark)));
        self.quads.push(rt(Mode::RtBegin, clear, Tex::None, all, 0));
        if !band.is_none() {
            let quads = &mut self.quads;
            band_strips(band, h, CELL, |y0, y1, k| {
                let add = sub_rgb(base_colour(dim(ambient, k)), clear);
                if add & 0x00ff_ffff != 0 {
                    quads.push(rt(Mode::Halo, add, Tex::None, (0, y0, w, y1), 0));
                }
            });
        }
        // A light's disc in cells: its middle at its ground point over the cell, as `soft`'s cells.
        let disc = |l: &jane_present::Light| {
            let r = (i32::from(l.radius) + CELL / 2) / CELL;
            let (cx, cy) = (l.pos.0 / CELL, l.pos.1 / CELL);
            (cx - r, cy - r, cx + r + 1, cy + r + 1)
        };
        let shadows = self.effects & fx::LAMP_SHADOWS != 0;
        let mut slabs: Vec<Slab> = core::mem::take(&mut self.slabs);
        let mut still: Vec<Slab> = core::mem::take(&mut self.still);
        let step = shadow::SUB * CELL;
        let cam = frame.camera;
        let bounce = jane_present::shadow::LAMP_BOUNCE;
        // What stands still, once a frame: blocks and props in zone px, with a word each for
        // the checksum (`(x0, y0, x1, y1, word)`); and which casters move.
        let mut fixed = core::mem::take(&mut self.fixed);
        fixed.clear();
        let mut moving = core::mem::take(&mut self.moving);
        moving.clear();
        moving.extend(frame.casters_in(casters).iter().map(|c| self.moves(frame, c)));
        // Read the first time a light is looked up (a frame whose lights are all fresh, the
        // most, reads none of it).
        let mut fixed_read = false;
        for l in lights {
            if l.radius == 0 {
                continue;
            }
            if !(shadows && l.casts) {
                self.quads.push(rt(Mode::Halo, pool_colour(l, ambient, 256), Tex::Pool, disc(l), POOL as u16));
                continue;
            }
            let lamp = Lamp::of(l);
            let t0 = self.now();
            // What stands still round it, summed: its texture holds while this does.
            let cell = crate::lamps::cell_of(l.radius);
            let (lx, ly) = (l.pos.0 + cam.0, l.pos.1 + cam.1);
            let key = ((lx + 2).div_euclid(CELL), (ly + 2).div_euclid(CELL), l.height, l.radius);
            // A light carried by someone moves with them (her lantern): never cached.
            let carried = l.holder.and_then(|h| frame.sprites.get(h as usize)).is_some_and(|s| self.sprite_moves(s));
            let found = if carried {
                Use::Plain
            } else if let Some(u) = self.lamps.fresh(key) {
                u
            } else {
                if !fixed_read {
                    fixed_read = true;
                    for b in frame.blocks_in(blocks) {
                        let (x0, y0) = (i32::from(b.x0) + cam.0, i32::from(b.y0) + cam.1);
                        let word = (i32::from(b.x1) - i32::from(b.x0)) << 20
                            ^ (i32::from(b.y1) - i32::from(b.y0)) << 10
                            ^ i32::from(b.height) << 4
                            ^ i32::from(b.lo);
                        fixed.push((x0, y0, i32::from(b.x1) + cam.0, i32::from(b.y1) + cam.1, word, u32::MAX));
                    }
                    for (ci, c) in frame.casters_in(casters).iter().enumerate() {
                        let Some(s) = frame.sprites.get(c.sprite as usize) else { continue };
                        if moving[ci] {
                            continue;
                        }
                        let (x, y) = (i32::from(c.foot.0) + cam.0, i32::from(c.foot.1) + cam.1);
                        let word = i32::from(s.src.x) << 16 ^ i32::from(s.src.y) ^ i32::from(s.flags.mirror) << 31;
                        fixed.push((x, y, x, y, word, ci as u32));
                    }
                }
                // Its square in zone px: what stands still in it, summed and counted.
                let half = cell * crate::lamps::TEX as i32 / 2;
                let (mut sum, mut count) = (0u32, 0u32);
                for &(x0, y0, x1, y1, word, ci) in &fixed {
                    if x1 < lx - half || x0 > lx + half || y1 < ly - half || y0 > ly + half {
                        continue;
                    }
                    if ci != u32::MAX
                        && frame.casters_in(casters).get(ci as usize).is_some_and(|c| l.holder == Some(c.sprite))
                    {
                        continue;
                    }
                    // By the cell (a moving camera puts a thing a px either side of where it was),
                    // order-free: a sum of each one's hash.
                    let (qx, qy) = ((x0 + 2).div_euclid(CELL), (y0 + 2).div_euclid(CELL));
                    let hsh =
                        (qx as u32).wrapping_mul(0x9e37_79b1) ^ (qy as u32).wrapping_mul(0x85eb_ca77) ^ word as u32;
                    sum = sum.wrapping_add(hsh.wrapping_mul(0xc2b2_ae3d) ^ hsh >> 15);
                    count += 1;
                }
                self.lamps.lookup(key, sum, count)
            };
            let cached = match found {
                Use::Cached { slot, cell } => Some((slot, cell)),
                Use::Build { slot, cell } => {
                    // What stands still round it, cast once into its texture: the fences' first,
                    // cleared off the px their fences draw over (`shadow::fence_drawn`).
                    still.clear();
                    let mut drawn = core::mem::take(&mut self.fence_rects);
                    drawn.clear();
                    for b in frame.blocks_in(blocks).iter().filter(|b| b.fence) {
                        let n = still.len();
                        shadow::block_slabs(b, &lamp, |q| still.push(q));
                        if still.len() > n {
                            drawn.extend(shadow::fence_drawn(b));
                        }
                    }
                    let fenced = still.len();
                    for c in frame.casters_in(casters) {
                        if self.moves(frame, c) || l.holder == Some(c.sprite) || !shadow::reaches(c, &lamp) {
                            continue;
                        }
                        if let Some((e, x)) = self.rows_of(frame, c, px) {
                            self.coarsen(e, BAND, i32::from(l.height));
                            if self.stands_over(x, c, l.pos) {
                                continue;
                            }
                            shadow::row_slabs(&self.coarse, x, c, &lamp, |q| still.push(q));
                            shadow::side_slabs(&self.coarse, x, c, &lamp, |q| still.push(q));
                        }
                    }
                    for b in frame.blocks_in(blocks).iter().filter(|b| !b.fence) {
                        shadow::block_slabs(b, &lamp, |q| still.push(q));
                    }
                    self.lamps.build_fenced(slot, l.pos, l.radius, cell, (&still, fenced), &drawn);
                    self.fence_rects = drawn;
                    Some((slot, cell))
                }
                Use::Plain => None,
            };
            // This frame's casts: what moves, or everything when the light is not cached.
            slabs.clear();
            // What it casts this frame, nearest first: everything that moves, and of what stands
            // still (a light not cached: her lantern) the nearest `PLAIN_CASTERS`; coarser rows the
            // further off (a far caster's shadow is long and thin).
            let mut near = core::mem::take(&mut self.near);
            near.clear();
            for (ci, c) in frame.casters_in(casters).iter().enumerate() {
                if (cached.is_some() && !moving[ci]) || l.holder == Some(c.sprite) || !shadow::reaches(c, &lamp) {
                    continue;
                }
                let (dx, dy) = (i32::from(c.foot.0) - l.pos.0, i32::from(c.foot.1) - l.pos.1);
                near.push((dx * dx + dy * dy, ci as u32));
            }
            near.sort_unstable();
            let mut still_cast = 0;
            let r2 = i32::from(l.radius).pow(2);
            for &(d2, ci) in &near {
                let ci = ci as usize;
                if !moving[ci] {
                    still_cast += 1;
                    if still_cast > PLAIN_CASTERS {
                        continue;
                    }
                }
                let c = &frame.casters_in(casters)[ci];
                let Some((e, x)) = self.rows_of(frame, c, px) else { continue };
                let band = if 4 * d2 < r2 {
                    BAND
                } else if d2 < r2 {
                    2 * BAND
                } else {
                    4 * BAND
                };
                self.coarsen(e, band, i32::from(l.height));
                if self.stands_over(x, c, l.pos) {
                    continue;
                }
                shadow::row_slabs(&self.coarse, x, c, &lamp, |q| slabs.push(q));
                // Its far ends too, as in the cached textures: a band's one span seen edge on
                // (her beside a campfire) else throws its front and back as two shadows apart.
                // What moves is cast each frame: its ends only for a light near level beside it,
                // where the faces come apart (`shadow::side_on`).
                if !moving[ci] || shadow::side_on(c, &lamp) {
                    shadow::side_slabs(&self.coarse, x, c, &lamp, |q| slabs.push(q));
                }
            }
            self.near = near;
            let t1 = self.now();
            if cached.is_none() {
                // A fence's shadow first, then cleared off the px its fence draws over (the light
                // multiplies them as the ground: `shadow::fence_drawn`), then the rest over it.
                let mut drawn = core::mem::take(&mut self.fence_rects);
                drawn.clear();
                for b in frame.blocks_in(blocks).iter().filter(|b| b.fence) {
                    let Some((quads, n)) = lamp_block_poly(b, &lamp, (w, h)) else { continue };
                    self.prof[11] += 1;
                    for p in &quads[..n] {
                        self.mark(*p);
                    }
                    drawn.extend(fence_cells(b, (w, h)));
                }
                for &r in &drawn {
                    self.lamp_marks.push(rt(Mode::StencilClear, 0xff00_0000, Tex::None, r, 0));
                }
                self.fence_rects = drawn;
                for b in frame.blocks_in(blocks).iter().filter(|b| !b.fence) {
                    let Some((quads, n)) = lamp_block_poly(b, &lamp, (w, h)) else { continue };
                    self.prof[11] += 1;
                    for p in &quads[..n] {
                        self.mark(*p);
                    }
                }
            }
            self.prof[3] += t1.wrapping_sub(t0);
            self.prof[4] += self.now().wrapping_sub(t1);
            for q in &slabs {
                // The quad round its corners (a0, b0, b1, a1), in cells.
                let pts = [q.c[0], q.c[1], q.c[3], q.c[2]].map(|(x, y, _)| {
                    ((x.div_euclid(step)).clamp(-2048, 2047) as i16, (y.div_euclid(step)).clamp(-2048, 2047) as i16)
                });
                let mut p8 = [(0i16, 0i16); 8];
                p8[..4].copy_from_slice(&pts);
                self.mark((p8, 4));
            }
            self.slab_count += self.lamp_marks.len() as u32;
            let marked = !self.lamp_marks.is_empty();
            if marked {
                self.quads.push(rt(Mode::StencilClear, 0xff00_0000, Tex::None, all, 0));
                self.quads.append(&mut self.lamp_marks);
            }
            let (tex, area, uv) = match cached {
                Some((slot, cell)) => {
                    let half = cell * TEX as i32 / 2;
                    let (x0, y0) = ((l.pos.0 - half).div_euclid(CELL), (l.pos.1 - half).div_euclid(CELL));
                    let side = cell * TEX as i32 / CELL;
                    (Tex::LampTex(slot), (x0, y0, x0 + side, y0 + side), TEX as u16)
                }
                None => (Tex::Pool, disc(l), POOL as u16),
            };
            if marked {
                self.quads.push(rt(Mode::PoolLit, pool_colour(l, ambient, 256), tex, area, uv));
                self.quads.push(rt(Mode::PoolShade, pool_colour(l, ambient, bounce), tex, area, uv));
            } else {
                self.quads.push(rt(Mode::Halo, pool_colour(l, ambient, 256), tex, area, uv));
            }
        }
        self.slabs = slabs;
        self.still = still;
        self.fixed = fixed;
        self.moving = moving;
        self.quads.push(rt(Mode::RtEnd, 0, Tex::None, all, 0));
    }

    /// Whether caster `c` moves (a person, a creature, a critter: its sprite is a unit's or the
    /// scene's), so is cast each
    /// frame rather than cached.
    fn moves(&self, frame: &Frame, c: &jane_present::Caster) -> bool {
        frame.sprites.get(c.sprite as usize).is_none_or(|s| self.sprite_moves(s))
    }

    /// Whether sprite `s` is one that moves (a unit's or the scene's).
    fn sprite_moves(&self, s: &SpriteCmd) -> bool {
        // People and creatures, and the scene's own sprites (critters, birds, stand-ins).
        self.find(s.page, s.src.x, s.src.y)
            .and_then(|i| self.targets[i])
            .is_none_or(|t| t.cat == crate::pack::UNITS || t.cat == crate::pack::SCENE)
    }

    /// `rows_kept[e]`'s rows coarsened into `coarse`, one span each `band_rows` rows: a slab pair
    /// a band of rows, not one a run.
    ///
    /// Only the rows that start under `cap` px (the light's height): a row wholly over the light
    /// throws its shadow to the rim and no nearer, where the run under it already reaches, so its
    /// slab has no area.
    fn coarsen(&mut self, e: usize, band_rows: i32, cap: i32) {
        let all = &self.rows_kept[e].rows;
        let n = all.iter().position(|r| jane_present::frame::height_of_rows(r.0 - 1) >= cap).unwrap_or(all.len());
        let rows = &all[..n];
        self.coarse.clear();
        let mut k = 0;
        while k < rows.len() {
            let band = rows[k].0 / band_rows;
            let mut j = k;
            let (mut u0, mut u1) = (i32::MAX, i32::MIN);
            while j < rows.len() && rows[j].0 / band_rows == band {
                (u0, u1) = (u0.min(rows[j].1), u1.max(rows[j].2));
                j += 1;
            }
            let (lo, hi) = (rows[k].0, rows[j - 1].0);
            self.coarse.extend((lo..=hi).map(|hv| (hv, u0, u1)));
            k = j;
        }
    }

    /// Whether a light at `(lx, ly)` (canvas px) stands inside the footprint of the caster whose
    /// rows are in `coarse` (its lowest row's span, its foot row and as deep behind it): a lantern
    /// held behind a trunk, a lamp in a bush. Its slabs would then throw the whole pool into shadow
    /// (they are faces seen from inside), so it casts none from that light.
    fn stands_over(&self, x: i32, c: &jane_present::Caster, (lx, ly): (i32, i32)) -> bool {
        let Some(&(_, u0, u1)) = self.coarse.first() else { return false };
        let fy = i32::from(c.foot.1);
        let deep = i32::from(c.depth.max(2)).min(2 * ((u1 - u0) / 2) + 2);
        (x + u0 - 1..=x + u1 + 1).contains(&lx) && (fy - deep - 1..=fy + 1).contains(&ly)
    }

    /// A polygon marked in the lightmap target's stencil (`lamp_marks`), unless off it.
    pub(super) fn mark(&mut self, p: Poly) {
        let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
        for &(x, y) in &p.0[..usize::from(p.1)] {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        if x1 < 0 || y1 < 0 || i32::from(x0) >= SIDE_CELLS || i32::from(y0) >= SIDE_CELLS {
            return;
        }
        if self.polys.len() >= POLYS_MOST {
            self.polys_skipped += 1;
            return;
        }
        self.polys.push(p);
        let pi = (self.polys.len() - 1) as u16;
        self.lamp_marks.push(Quad {
            tex: Tex::Poly(pi),
            mode: Mode::StencilMark,
            colour: 0xffff_ffff,
            x0,
            y0,
            x1,
            y1,
            u0: 0,
            v0: 0,
            u1: 0,
            v1: 0,
        });
    }
}
