//! The `Frame` as the GE's quads (PORT.md §13.12): every pass C2 draws, in the frame's order,
//! each sprite resolved from the presenter's page and rect to the PSP pack's page and trimmed
//! rect, clipped to the canvas, bent rows split into strips. Pure and integer: what `ge.rs`
//! sends is exactly this list, so a PC test reads what the PSP will draw.

use alloc::vec::Vec;

use jane_present::atlas::SpriteRef;
use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, Pass, SpriteCmd, Tint};

use crate::pack::Pack;

/// What a quad samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    /// A flat colour.
    None,
    /// A pack page (`T8` over its CLUT).
    Page(u16),
    /// A painted terrain chunk, by the frame's slot (`Frame::layers`).
    Chunk(u16),
    /// This frame's patch of terrain laid back over a sprite it stands in front of
    /// ([`Lister::patches`]).
    Patch(u16),
    /// This frame's lightmap ([`Lister::light`]), stretched four times with bilinear filtering.
    Lightmap,
}

/// The terrain in front of a sprite whose feet it hides (PRESENTATION.md §1.6, *behind the
/// terrain*): the terrain's px over the sprite's rect where it stands on ground south of the
/// sprite's feet, clear elsewhere, `0xAABBGGRR`, `tw` px a row (a power of two, as `th`).
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub w: u16,
    pub h: u16,
    pub tw: u16,
    pub th: u16,
    pub px: Vec<u32>,
}

/// The largest patch laid: a sprite past it is drawn whole over the terrain.
const PATCH_MOST: i32 = 256;

/// How a quad lays its colour on what is under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The texel times the quad's colour, over by its alpha (a ghost is the colour's alpha).
    Alpha,
    /// The texel plus the quad's colour, over by the texel's alpha (the hurt flash).
    Add,
    /// What is under it times the quad's colour (the ambient light).
    Multiply,
    /// What is under it times twice the texel (the lightmap, which holds half the light).
    Multiply2,
}

/// One quad: canvas px `x0..x1`, `y0..y1`, its texels from `(u0, v0)` one px a texel, `u`
/// running backward when `u0 > u1` (mirrored; `u0` is then one past the first texel drawn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quad {
    pub tex: Tex,
    pub mode: Mode,
    /// `0xAABBGGRR`, the GE's order.
    pub colour: u32,
    pub x0: i16,
    pub y0: i16,
    pub x1: i16,
    pub y1: i16,
    pub u0: u16,
    pub v0: u16,
    pub u1: u16,
    pub v1: u16,
}

/// A presenter sprite's rect on the PSP: its page and trimmed rect, and where that rect's
/// top-left sits in the presenter's rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    page: u16,
    u: u16,
    v: u16,
    w: u16,
    h: u16,
    tx: i16,
    ty: i16,
}

/// `0xAARRGGBB` (the `Frame`'s) as `0xAABBGGRR` (the GE's).
#[inline]
pub const fn abgr(argb: u32) -> u32 {
    argb & 0xff00_ff00 | (argb >> 16) & 0xff | (argb & 0xff) << 16
}

/// An `Rgb` and an alpha as `0xAABBGGRR`.
#[inline]
pub const fn rgba(c: [u8; 3], a: u8) -> u32 {
    (a as u32) << 24 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32
}

/// The frame's quads, rebuilt each frame into reused buffers.
#[derive(Debug)]
pub struct Lister {
    /// The presenter's sprites by where they sit: `(page, y, x, ref)`, sorted.
    by_pos: Vec<(u8, u16, u16, u16)>,
    /// Each presenter ref's rect.
    rects: Vec<(u16, u16)>,
    targets: Vec<Option<Target>>,
    pub quads: Vec<Quad>,
    /// This frame's chunks: `(slot, generation)`.
    pub chunks: Vec<(u16, u32)>,
    /// This frame's chunks where they are drawn: `(x, y, slot)`, canvas px.
    placed: Vec<(i32, i32, u16)>,
    /// This frame's patches (`Tex::Patch`), and how many of the pool are in use.
    pub patches: Vec<Patch>,
    patches_used: usize,
    /// This frame's lightmap, when a light shows.
    pub light: crate::light::LightMap,
    /// The frame's clear, `0xAABBGGRR`.
    pub clear: u32,
    /// Sprites this frame that resolved to nothing on the PSP (the UI page's, a ref C2 leaves out).
    pub misses: u32,
    w: i32,
    h: i32,
}

impl Lister {
    /// Over the presenter's sprite table (`Present::sprites().refs`) and the pack's.
    pub fn new(refs: &[SpriteRef], pack: &Pack) -> Lister {
        let mut by_pos: Vec<(u8, u16, u16, u16)> =
            refs.iter().enumerate().map(|(i, r)| (r.page, r.src.y, r.src.x, i as u16)).collect();
        by_pos.sort_unstable();
        let targets = refs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let link = pack.refs.get(i)?;
                let rec = pack.recs.get(link.rec as usize)?;
                (rec.page != u16::MAX && rec.w > 0 && rec.h > 0).then_some(Target {
                    page: rec.page,
                    u: rec.u,
                    v: rec.v,
                    w: rec.w,
                    h: rec.h,
                    tx: r.ax - link.ax,
                    ty: r.ay - link.ay,
                })
            })
            .collect();
        Lister {
            by_pos,
            rects: refs.iter().map(|r| (r.src.x, r.src.y)).collect(),
            targets,
            quads: Vec::with_capacity(1024),
            chunks: Vec::with_capacity(32),
            placed: Vec::with_capacity(32),
            patches: Vec::new(),
            patches_used: 0,
            light: crate::light::LightMap::default(),
            clear: 0xff00_0000,
            misses: 0,
            w: 480,
            h: 272,
        }
    }

    /// The presenter's ref whose rect holds `(x, y)` of `page`: the shelf (rects packed in rows
    /// at one `y`) the last at or above `y`, then in it the last at or left of `x`.
    fn find(&self, page: u8, x: u16, y: u16) -> Option<usize> {
        let at = self.by_pos.partition_point(|e| (e.0, e.1) <= (page, y));
        let shelf = self.by_pos.get(at.checked_sub(1)?)?;
        if shelf.0 != page {
            return None;
        }
        let row = shelf.1;
        let i = self.by_pos.partition_point(|e| (e.0, e.1, e.2) <= (page, row, x)).checked_sub(1)?;
        let e = self.by_pos[i];
        (e.0 == page && e.1 == row).then_some(usize::from(e.3))
    }

    /// Builds the frame's quads.
    pub fn build(&mut self, frame: &Frame) -> &[Quad] {
        self.quads.clear();
        self.chunks.clear();
        self.placed.clear();
        self.patches_used = 0;
        self.misses = 0;
        (self.w, self.h) = (i32::from(frame.canvas.0), i32::from(frame.canvas.1));
        self.clear = abgr(frame.clear);
        for pass in &frame.passes {
            match *pass {
                Pass::Terrain { chunks } => {
                    for c in frame.chunks_in(chunks) {
                        self.chunks.push((c.slot, c.generation));
                        self.placed.push((c.x, c.y, c.slot));
                        let q = Quad {
                            tex: Tex::Chunk(c.slot),
                            mode: Mode::Alpha,
                            colour: 0xffff_ffff,
                            x0: 0,
                            y0: 0,
                            x1: 0,
                            y1: 0,
                            u0: 0,
                            v0: 0,
                            u1: 0,
                            v1: 0,
                        };
                        self.push(q, c.x, c.y, CHUNK_PX, CHUNK_PX, 0, 0, false);
                    }
                }
                Pass::Sprites { cmds, .. } => {
                    for s in frame.sprites_in(cmds) {
                        self.sprite(s);
                        if let Some(f) = s.foot
                            && s.flags.tint != Tint::Seen
                        {
                            self.patch(frame, s, f);
                        }
                    }
                }
                Pass::Lights { ambient, points, .. } => {
                    let points = frame.lights_in(points);
                    if !points.is_empty() {
                        // T0's lightmap (`soft`'s): the ambient and every pool, a quarter size,
                        // half a cell back so the GE's filter lands its cells where `soft`'s do.
                        self.light.build((self.w, self.h), ambient, points);
                        let (w, h) = (self.light.w, self.light.h);
                        let half = crate::light::CELL as i16 / 2;
                        self.quads.push(Quad {
                            tex: Tex::Lightmap,
                            mode: Mode::Multiply2,
                            colour: 0xffff_ffff,
                            x0: -half,
                            y0: -half,
                            x1: (w * crate::light::CELL) as i16 - half,
                            y1: (h * crate::light::CELL) as i16 - half,
                            u0: 0,
                            v0: 0,
                            u1: w as u16,
                            v1: h as u16,
                        });
                    } else if ambient.iter().any(|&c| c < 254) {
                        // The flat light (T0's ambient, the sun's share in it): what is drawn,
                        // times it.
                        let q = Quad {
                            tex: Tex::None,
                            mode: Mode::Multiply,
                            colour: rgba(ambient, 255),
                            x0: 0,
                            y0: 0,
                            x1: self.w as i16,
                            y1: self.h as i16,
                            u0: 0,
                            v0: 0,
                            u1: 0,
                            v1: 0,
                        };
                        self.quads.push(q);
                    }
                }
                // Not drawn on C2 yet (PORT.md §13.12, the gaps): the sky backdrop and its far
                // things, the water's glints, the particles, the fog and the weather. The
                // silhouettes, rays and post a C2 frame never holds (`Features::c2`).
                Pass::Sky(_)
                | Pass::Parallax { .. }
                | Pass::Water { .. }
                | Pass::Particles { .. }
                | Pass::Fog { .. }
                | Pass::Weather(_)
                | Pass::Silhouettes { .. }
                | Pass::Rays { .. }
                | Pass::Post(_) => {}
            }
        }
        &self.quads
    }

    /// A sprite's quads: its trimmed rect on its PSP page, by strips where it bends.
    fn sprite(&mut self, s: &SpriteCmd) {
        let Some(i) = self.find(s.page, s.src.x, s.src.y) else {
            self.misses += 1;
            return;
        };
        let Some(t) = self.targets[i] else {
            self.misses += 1;
            return;
        };
        let (rx, ry) = self.rects[i];
        // The command's rect in the presenter sprite's own px, and the trimmed rect in them.
        let (lx0, ly0) = (i32::from(s.src.x) - i32::from(rx), i32::from(s.src.y) - i32::from(ry));
        let (sw, sh) = (i32::from(s.src.w), i32::from(s.src.h));
        let (tx, ty) = (i32::from(t.tx), i32::from(t.ty));
        let ix0 = lx0.max(tx);
        let ix1 = (lx0 + sw).min(tx + i32::from(t.w));
        let iy0 = ly0.max(ty);
        let iy1 = (ly0 + sh).min(ty + i32::from(t.h));
        if ix0 >= ix1 || iy0 >= iy1 {
            return;
        }
        let (mode, colour) = match s.flags.tint {
            Tint::None => (Mode::Alpha, 0xffff_ffff),
            Tint::Ghost(a) => (Mode::Alpha, u32::from(a) << 24 | 0x00ff_ffff),
            Tint::Flash(a) => (Mode::Add, rgba([a, a, a], 255)),
            // One texel in two on the canvas's checker, here half of each (PORT.md §13.12).
            Tint::Seen => (Mode::Alpha, 0x80ff_ffff),
        };
        let base =
            Quad { tex: Tex::Page(t.page), mode, colour, x0: 0, y0: 0, x1: 0, y1: 0, u0: 0, v0: 0, u1: 0, v1: 0 };
        let mirror = s.flags.mirror;
        // Canvas x of the drawn columns' left edge before any bend.
        let x = i32::from(s.x) + if mirror { lx0 + sw - ix1 } else { ix0 - lx0 };
        let u = i32::from(t.u) + ix0 - tx;
        let bend = s.flags.bend;
        // Rows `iy0..iy1` in runs of one shift (a row's index is from the command's top).
        let mut r0 = iy0;
        while r0 < iy1 {
            let shift = bend.shift(r0 - ly0);
            let mut r1 = r0 + 1;
            while r1 < iy1 && bend.shift(r1 - ly0) == shift {
                r1 += 1;
            }
            let y = i32::from(s.y) + r0 - ly0;
            let v = i32::from(t.v) + r0 - ty;
            self.push(base, x + shift, y, ix1 - ix0, r1 - r0, u, v, mirror);
            r0 = r1;
        }
    }

    /// Lays back over sprite `s` the terrain in front of its feet (`Foot::hides`), as a patch
    /// of the chunks' px over its rect: what `soft` leaves out of the sprite, drawn over it. Her
    /// `Tint::Seen` pass, drawn later, shows through it.
    fn patch(&mut self, frame: &Frame, s: &SpriteCmd, f: jane_present::Foot) {
        let reach = s.flags.bend.reach();
        let (x0, y0) = ((i32::from(s.x) - reach).max(0), i32::from(s.y).max(0));
        let x1 = (i32::from(s.x) + i32::from(s.src.w) + reach).min(self.w);
        let y1 = (i32::from(s.y) + i32::from(s.src.h)).min(self.h);
        let (w, h) = (x1 - x0, y1 - y0);
        if w <= 0 || h <= 0 || w > PATCH_MOST || h > PATCH_MOST {
            return;
        }
        let (tw, th) = ((w as u32).next_power_of_two().max(4), (h as u32).next_power_of_two());
        if self.patches.len() == self.patches_used {
            self.patches.push(Patch::default());
        }
        let p = &mut self.patches[self.patches_used];
        p.px.clear();
        p.px.resize((tw * th) as usize, 0);
        (p.w, p.h, p.tw, p.th) = (w as u16, h as u16, tw as u16, th as u16);
        let mut any = false;
        for &(cx, cy, slot) in &self.placed {
            let Some(l) = frame.layers.get(usize::from(slot)) else { continue };
            if !l.has_height() {
                continue;
            }
            let (ax0, ax1) = (x0.max(cx), x1.min(cx + CHUNK_PX));
            let (ay0, ay1) = (y0.max(cy), y1.min(cy + CHUNK_PX));
            for y in ay0..ay1 {
                let row = ((y - cy) * CHUNK_PX) as usize;
                for x in ax0..ax1 {
                    let k = row + (x - cx) as usize;
                    if jane_present::Foot::hides(l.height[k], y, i32::from(f.y)) {
                        p.px[((y - y0) as u32 * tw + (x - x0) as u32) as usize] = abgr(l.albedo[k]) | 0xff00_0000;
                        any = true;
                    }
                }
            }
        }
        if !any {
            return;
        }
        let i = self.patches_used as u16;
        self.patches_used += 1;
        self.quads.push(Quad {
            tex: Tex::Patch(i),
            mode: Mode::Alpha,
            colour: 0xffff_ffff,
            x0: x0 as i16,
            y0: y0 as i16,
            x1: x1 as i16,
            y1: y1 as i16,
            u0: 0,
            v0: 0,
            u1: w as u16,
            v1: h as u16,
        });
    }

    /// Pushes `w x h` px at canvas `(x, y)` from texels `(u, v)` (mirrored: `u..u + w` laid right
    /// to left), clipped to the canvas.
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, mut q: Quad, x: i32, y: i32, w: i32, h: i32, u: i32, v: i32, mirror: bool) {
        let (cx0, cy0) = (x.max(0), y.max(0));
        let (cx1, cy1) = ((x + w).min(self.w), (y + h).min(self.h));
        if cx0 >= cx1 || cy0 >= cy1 {
            return;
        }
        let v0 = v + (cy0 - y);
        let v1 = v0 + (cy1 - cy0);
        let (u0, u1) = if mirror {
            // Canvas column x + k shows texel u + w - 1 - k.
            let first = u + w - (cx0 - x);
            (first, first - (cx1 - cx0))
        } else {
            let first = u + (cx0 - x);
            (first, first + (cx1 - cx0))
        };
        q.x0 = cx0 as i16;
        q.y0 = cy0 as i16;
        q.x1 = cx1 as i16;
        q.y1 = cy1 as i16;
        q.u0 = u0 as u16;
        q.u1 = u1 as u16;
        q.v0 = v0 as u16;
        q.v1 = v1 as u16;
        self.quads.push(q);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::frame::Src;

    fn sref(page: u8, x: u16, y: u16, w: u16, h: u16, ax: i16, ay: i16) -> SpriteRef {
        SpriteRef { page, src: Src { x, y, w, h }, ax, ay, height: 1, top: 1, base: 0, base_height: 0, burn: (-1, -1) }
    }

    /// Two refs on one shelf, one on the next; the first trimmed 2 px in from its left and 3 from
    /// its top on the PSP, at (100, 50) of page 4.
    fn lister() -> Lister {
        use crate::pack::{Rec, RefLink};
        let refs = [sref(0, 0, 0, 10, 12, 5, 12), sref(0, 10, 0, 8, 8, 4, 8), sref(0, 0, 12, 6, 6, 3, 6)];
        let rec = |page, u, v, w, h| Rec { cat: 2, frame: 0, sprite: 0, vs: 0, page, u, v, w, h, ax: 0, ay: 0 };
        let pack = Pack {
            recs: alloc::vec![rec(4, 100, 50, 6, 9), rec(5, 0, 0, 8, 8), rec(u16::MAX, 0, 0, 0, 0)],
            refs: alloc::vec![
                RefLink { rec: 0, ax: 3, ay: 9 },
                RefLink { rec: 1, ax: 4, ay: 8 },
                RefLink { rec: 2, ax: 0, ay: 0 }
            ],
            ..Pack::default()
        };
        Lister::new(&refs, &pack)
    }

    #[test]
    fn a_rect_finds_the_ref_it_lies_in() {
        let l = lister();
        assert_eq!(l.find(0, 0, 0), Some(0));
        assert_eq!(l.find(0, 9, 11), Some(0));
        assert_eq!(l.find(0, 10, 0), Some(1));
        assert_eq!(l.find(0, 12, 5), Some(1));
        assert_eq!(l.find(0, 2, 13), Some(2));
        assert_eq!(l.find(1, 0, 0), None);
    }

    fn cmd(src: Src, x: i16, y: i16, mirror: bool) -> SpriteCmd {
        SpriteCmd {
            page: 0,
            src,
            x,
            y,
            flags: jane_present::Flags { mirror, ..jane_present::Flags::default() },
            height_px: 0,
            foot: None,
        }
    }

    #[test]
    fn a_trimmed_sprite_lands_where_its_px_were() {
        let mut l = lister();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false));
        // Trim (2, 3): px (2, 3) of the presenter's rect is texel (100, 50).
        assert_eq!(
            l.quads,
            [Quad {
                tex: Tex::Page(4),
                mode: Mode::Alpha,
                colour: 0xffff_ffff,
                x0: 22,
                y0: 33,
                x1: 28,
                y1: 42,
                u0: 100,
                v0: 50,
                u1: 106,
                v1: 59
            }]
        );
        // Mirrored: column k of the 10 wide rect shows px 9 - k; px 2..8 land at 20 + 2..20 + 8.
        l.quads.clear();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, true));
        let q = l.quads[0];
        assert_eq!((q.x0, q.x1, q.u0, q.u1), (22, 28, 106, 100));
        // Clipped at the canvas's left: the first 3 columns drawn go.
        l.quads.clear();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, false));
        let q = l.quads[0];
        // Px 2..8 at -5 + 2.. -5 + 8 = -3..3: columns 0..3 show texels 103..106.
        assert_eq!((q.x0, q.x1, q.u0, q.u1), (0, 3, 103, 106));
        l.quads.clear();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, true));
        let q = l.quads[0];
        // Mirrored px 2..8 at -5 + 2.. -5 + 8 = -3..3: columns 0..3 show texels 102, 101, 100.
        assert_eq!((q.x0, q.x1, q.u0, q.u1), (0, 3, 103, 100));
    }

    #[test]
    fn a_ref_with_nothing_on_the_psp_is_a_miss() {
        let mut l = lister();
        l.sprite(&cmd(Src { x: 0, y: 12, w: 6, h: 6 }, 0, 0, false));
        assert!(l.quads.is_empty());
        assert_eq!(l.misses, 1);
    }

    #[test]
    fn a_bent_sprite_is_strips_shifted_by_the_bend() {
        let mut l = lister();
        let mut c = cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false);
        c.flags.bend = jane_present::Bend { lean: 2, from: 8, span: 4 };
        l.sprite(&c);
        // Rows 3..12 drawn; every row's shift is the bend's, and they tile the rows once.
        let mut rows = 0;
        for q in &l.quads {
            for y in q.y0..q.y1 {
                let r = i32::from(y) - 30;
                assert_eq!(i32::from(q.x0), 22 + c.flags.bend.shift(r), "row {r}");
                rows += 1;
            }
        }
        assert_eq!(rows, 9);
        assert!(l.quads.len() > 1);
    }
}
