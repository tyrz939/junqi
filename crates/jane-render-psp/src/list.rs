//! The `Frame` as the GE's quads (PORT.md §13.12): every pass C2 draws, in the frame's order,
//! each sprite resolved from the presenter's page and rect to the PSP pack's page and trimmed
//! rect, clipped to the canvas, bent rows split into strips. Pure and integer: what `ge.rs`
//! sends is exactly this list, so a PC test reads what the PSP will draw.

use alloc::vec::Vec;

use jane_present::atlas::SpriteRef;
use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, Pass, SpriteCmd, Tint};

use crate::pack::Pack;

/// The lighting effects ([`Lister::effects`]).
pub mod fx {
    /// The sprites' relief to the sun (the normal pages).
    pub const RELIEF: u8 = 1;
    /// The sun's silhouettes.
    pub const SHADOWS: u8 = 2;
    /// What glows (lamp glass, lit windows, flames) over the light.
    pub const GLOW: u8 = 4;
    /// The lightmap's pools (else the flat ambient).
    pub const LAMPS: u8 = 8;
    pub const ALL: u8 = 15;
}

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
    /// The halo disc (`light::disc`), stretched with bilinear filtering.
    Disc,
    /// No texture: this frame's convex polygon `polys[i]` (a block's shadow, swept along the
    /// sun), drawn as a triangle fan; the quad's rect is its bounds.
    Poly(u16),
    /// A pack page through its glow CLUT, added over the light (what glows on it).
    Glow(u16),
    /// A chunk slot's height layer as `T8` through a CLUT that is clear at ground height: what
    /// marks the raised terrain in the stencil.
    Height(u16),
    /// A pack page's normal page (`T4`) through this frame's light CLUT
    /// ([`Lister::relief`]): the sprite's relief to the sun.
    Normal(u16),
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

/// What a caster's rows (`shadow::rows`) depend on: its sprite's rect and page, mirror and
/// bend, its foot's row under the sprite's top, the rows it burns and its height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RowsKey {
    src: (u16, u16, u16, u16, u8),
    mirror: bool,
    bend: u32,
    foot: i16,
    burn: (u8, u8),
    height: u8,
}

/// A caster's rows kept: what they depend on, the rows, the draw they were last used in.
type KeptRows = (RowsKey, Vec<(i32, i32, i32)>, u32);

/// Casters' rows kept between frames.
const ROWS_KEPT: usize = 128;
/// Bytes of casters' opacity kept between frames.
const MASKS_KEPT: usize = 96 * 1024;
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
    /// The texel added to what is under it (a glow).
    AddGlow,
    /// Its colour added toward the darks: `src * (1 - dst) + dst` (the grade's lift).
    Lift,
    /// The texel's alpha times the colour added (a light's halo).
    Halo,
    /// The stencil (the framebuffer's alpha) cleared under it, no colour written.
    StencilClear,
    /// The stencil set where its texel is not clear, no colour written.
    StencilMark,
    /// What is under it times its colour where the stencil is clear, and the stencil set: each
    /// px shaded once.
    ShadowBand,
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

/// The pack pages' texels as the lister reads them (a caster's silhouette): the GE glue holds
/// them, loaded from the pack as asked.
pub trait PagePx {
    /// Page `page`'s `T8` index at `(u, v)`, 0 (clear) when it is not held.
    fn texel(&mut self, page: u16, u: u16, v: u16) -> u8;
}

/// No page held: every texel clear (a frame without silhouettes, a test).
#[derive(Debug)]
pub struct NoPx;

impl PagePx for NoPx {
    fn texel(&mut self, _: u16, _: u16, _: u16) -> u8 {
        0
    }
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

/// Block `b`'s shadow as a convex polygon: its footprint at the height its shadow starts from
/// (the ground, or a fence rail's underside) and at its top, offset along the sun `k` (Q8 px a
/// px of height), and their hull. `None` for a block too low to cast, or off a `w x h` canvas.
fn block_poly(b: &jane_present::Block, (kx, ky): (i32, i32), (w, h): (i32, i32)) -> Option<([(i16, i16); 8], u8)> {
    use jane_present::shadow;
    let hgt = i32::from(b.height);
    let (mut x0, y0, mut x1, y1) = (i32::from(b.x0), i32::from(b.y0), i32::from(b.x1), i32::from(b.y1));
    if hgt <= shadow::GROUND || x0 >= x1 || y0 >= y1 {
        return None;
    }
    let lo = if shadow::spills(b) {
        // A block of the height field stands a px wider each side than what is drawn.
        if !b.fence && x1 - x0 > 2 {
            (x0, x1) = (x0 + 1, x1 - 1);
        }
        i32::from(b.lo).min(hgt)
    } else {
        0
    };
    let (ax, ay, bx, by) = ((lo * kx) >> 8, (lo * ky) >> 8, (hgt * kx) >> 8, (hgt * ky) >> 8);
    let pts = [
        (x0 + ax, y0 + ay),
        (x1 + ax, y0 + ay),
        (x1 + ax, y1 + ay),
        (x0 + ax, y1 + ay),
        (x0 + bx, y0 + by),
        (x1 + bx, y0 + by),
        (x1 + bx, y1 + by),
        (x0 + bx, y1 + by),
    ];
    let (mx0, my0) = pts.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
    let (mx1, my1) = pts.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
    if mx1 <= 0 || my1 <= 0 || mx0 >= w || my0 >= h {
        return None;
    }
    Some(hull(pts))
}

/// The convex hull of eight points, round it (Andrew's monotone chain), at most eight corners.
fn hull(mut p: [(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    p.sort_unstable();
    let cross = |o: (i32, i32), a: (i32, i32), b: (i32, i32)| {
        i64::from(a.0 - o.0) * i64::from(b.1 - o.1) - i64::from(a.1 - o.1) * i64::from(b.0 - o.0)
    };
    let mut out = [(0i32, 0i32); 17];
    let mut n = 0usize;
    for &q in &p {
        while n >= 2 && cross(out[n - 2], out[n - 1], q) <= 0 {
            n -= 1;
        }
        out[n] = q;
        n += 1;
    }
    let lower = n + 1;
    for &q in p.iter().rev().skip(1) {
        while n >= lower && cross(out[n - 2], out[n - 1], q) <= 0 {
            n -= 1;
        }
        out[n] = q;
        n += 1;
    }
    let n = (n - 1).min(8);
    let mut r = [(0i16, 0i16); 8];
    for (d, s) in r.iter_mut().zip(&out[..n]) {
        *d = (s.0.clamp(-2048, 2047) as i16, s.1.clamp(-2048, 2047) as i16);
    }
    (r, n as u8)
}

/// The integer square root.
fn isqrt(n: u32) -> u32 {
    let (mut x, mut y) = (n, n.div_ceil(2));
    while y < x {
        x = y;
        y = u32::midpoint(x, n / x);
    }
    x
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
    /// This frame's response of each normal direction to the sun (`normals::clut`), when the sun
    /// is up and throws its silhouettes.
    pub relief: Option<[u32; crate::normals::DIRS]>,
    /// Which pack pages have a normal page, and a glow CLUT.
    has_normals: Vec<bool>,
    has_glow: Vec<bool>,
    /// This frame's glows, laid after the light pass.
    glows: Vec<Quad>,
    /// This frame's polygons (`Tex::Poly`): up to eight corners each, in order round it.
    pub polys: Vec<([(i16, i16); 8], u8)>,
    /// The silhouettes' scratch: the bands by strength (sixteenths), a caster's rows and its
    /// sprite's opacity.
    shadow_levels: Vec<Vec<(i16, i16, i16, i16)>>,
    caster_rows: Vec<(i32, i32, i32)>,
    caster_px: Vec<u16>,
    /// Casters' rows as `shadow::rows` gave them, `(what they depend on, rows, last used)`.
    rows_kept: Vec<KeptRows>,
    rows_clock: u32,
    /// Casters' opacity by ref, `(ref, 2 bits a px, last used)` ([`Lister::opacity`]).
    masks: Vec<(usize, Vec<u8>, u32)>,
    mask_clock: u32,
    /// Shadow runs laid this frame (a stat).
    pub shadow_runs: u32,
    /// The lighting effects drawn ([`fx`] bits; all by default): what a bench turns off to
    /// measure each one's cost.
    pub effects: u8,
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
            relief: None,
            has_normals: pack.pages.iter().map(|p| p.normal.is_some()).collect(),
            has_glow: pack.pages.iter().map(|p| p.glow.is_some()).collect(),
            glows: Vec::with_capacity(64),
            polys: Vec::with_capacity(256),
            shadow_levels: Vec::new(),
            caster_rows: Vec::with_capacity(256),
            caster_px: Vec::new(),
            masks: Vec::new(),
            mask_clock: 0,
            rows_kept: Vec::new(),
            rows_clock: 0,
            shadow_runs: 0,
            effects: fx::ALL,
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
        self.build_with(frame, &mut NoPx)
    }

    /// Builds the frame's quads, reading casters' texels through `px`.
    pub fn build_with(&mut self, frame: &Frame, px: &mut dyn PagePx) -> &[Quad] {
        self.shadow_runs = 0;
        self.glows.clear();
        self.polys.clear();
        self.quads.clear();
        self.chunks.clear();
        self.placed.clear();
        self.patches_used = 0;
        self.misses = 0;
        (self.w, self.h) = (i32::from(frame.canvas.0), i32::from(frame.canvas.1));
        self.clear = abgr(frame.clear);
        // The sun, read ahead: the ground sprites, drawn before its pass, take their relief too.
        self.relief = frame.passes.iter().filter(|_| self.effects & fx::RELIEF != 0).find_map(|p| match *p {
            Pass::Silhouettes { sun, shade, .. } => {
                crate::normals::clut(&sun, jane_present::shadow::shade_at(shade, sun.strength))
            }
            _ => None,
        });
        let mut lit = false;
        for pass in &frame.passes {
            // What glows goes over the light: laid as the pass after the light comes.
            if lit && !self.glows.is_empty() {
                self.quads.append(&mut self.glows);
            }
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
                Pass::Silhouettes { sun, shade, casters, blocks } => {
                    if self.effects & fx::SHADOWS != 0 {
                        self.silhouettes(frame, &sun, shade, (casters, blocks), px);
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
                    lit = true;
                    // The terrain's own glow (lit windows), over the light with the sprites'.
                    if self.effects & fx::GLOW != 0 {
                        self.chunk_glows(frame);
                    }
                    let points = frame.lights_in(points);
                    if !points.is_empty() && self.effects & fx::LAMPS != 0 {
                        // T0's lightmap (`soft`'s): the ambient and every pool, a quarter size,
                        // half a cell back so the GE's filter lands its cells where `soft`'s do.
                        self.light.build((self.w, self.h), ambient, points);
                        // Each light's halo over it, as T0's bloom gathers round what glows,
                        // stronger as the dark comes.
                        if self.effects & fx::GLOW != 0 {
                            let dark = jane_present::light::pool(ambient);
                            for l in points {
                                let r = (i32::from(l.radius) / 6).clamp(8, 28);
                                let (x, y) = (l.pos.0, l.pos.1 - jane_present::rows_up(i32::from(l.height)));
                                let k = |c: u8| (u32::from(c) * dark / 512).min(255);
                                let c = 0xff00_0000 | k(l.colour[2]) << 16 | k(l.colour[1]) << 8 | k(l.colour[0]);
                                if x + r <= 0 || y + r <= 0 || x - r >= self.w || y - r >= self.h {
                                    continue;
                                }
                                let d = crate::light::DISC as u16;
                                self.glows.push(Quad {
                                    tex: Tex::Disc,
                                    mode: Mode::Halo,
                                    colour: c,
                                    x0: (x - r) as i16,
                                    y0: (y - r) as i16,
                                    x1: (x + r) as i16,
                                    y1: (y + r) as i16,
                                    u0: 0,
                                    v0: 0,
                                    u1: d,
                                    v1: d,
                                });
                            }
                        }
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
                | Pass::Rays { .. } => {}
                Pass::Post(p) => self.grade(&p),
            }
        }
        // A frame with no light pass (the day on T0), or a light pass last: its glows now.
        self.quads.append(&mut self.glows);
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
        let first = self.quads.len();
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
        // What glows on it, laid over the light later: the same quads through the glow CLUT.
        if self.effects & fx::GLOW != 0
            && s.flags.tint == Tint::None
            && self.has_glow.get(usize::from(t.page)).copied().unwrap_or(false)
        {
            for k in first..self.quads.len() {
                let q = self.quads[k];
                self.glows.push(Quad { tex: Tex::Glow(t.page), mode: Mode::AddGlow, colour: 0xffff_ffff, ..q });
            }
        }
        // Its relief to the sun: the same quads again over its normal page.
        if self.relief.is_some()
            && s.flags.tint == Tint::None
            && self.has_normals.get(usize::from(t.page)).copied().unwrap_or(false)
        {
            for k in first..self.quads.len() {
                let q = self.quads[k];
                self.quads.push(Quad { tex: Tex::Normal(t.page), mode: Mode::Multiply2, colour: 0xffff_ffff, ..q });
            }
        }
    }

    /// The chunks' glowing px (`ChunkLayers::glow`: lit windows, lamps in the walls) as runs of
    /// one colour along a row, added over the light.
    fn chunk_glows(&mut self, frame: &Frame) {
        for &(cx, cy, slot) in &self.placed {
            let Some(l) = frame.layers.get(usize::from(slot)) else { continue };
            let mut run: Option<(i32, i32, i32, u32)> = None;
            let flush = |r: Option<(i32, i32, i32, u32)>, out: &mut Vec<Quad>| {
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

    /// The grade (`Post`, T2's terms, `soft`'s tables) as the GE can lay it: the exposure and
    /// the tint as one doubled multiply, then the lift added toward the darks (`src * (1 - dst)`:
    /// a dark px takes most of it, a light one little). Saturation and the shoulder are left out.
    fn grade(&mut self, p: &jane_present::Post) {
        let all = |mode: Mode, colour: u32| Quad {
            tex: Tex::None,
            mode,
            colour,
            x0: 0,
            y0: 0,
            x1: self.w as i16,
            y1: self.h as i16,
            u0: 0,
            v0: 0,
            u1: 0,
            v1: 0,
        };
        // Linear gains to display ones by a square root (a gamma of 2), halved for the multiply.
        let gain = |k: usize| {
            let lin = u32::from(p.exposure) * u32::from(p.tint[k]) * 256 / (128 * 255);
            (isqrt(lin * 256) * 128 / 256).min(255)
        };
        let g = [gain(0), gain(1), gain(2)];
        if g.iter().any(|&v| v.abs_diff(128) > 1) {
            self.quads.push(all(Mode::Multiply2, 0xff00_0000 | g[2] << 16 | g[1] << 8 | g[0]));
        }
        let lift = p.lift.map(|v| (isqrt(u32::from(v) * 255) * 3 / 4).min(255));
        if lift.iter().any(|&v| v > 0) {
            self.quads.push(all(Mode::Lift, 0xff00_0000 | lift[2] << 16 | lift[1] << 8 | lift[0]));
        }
    }

    /// The sun's silhouettes (`soft`'s method, `jane_present::shadow`): each caster's opaque rows
    /// read from its pack page (kept while they hold), sheared along the sun into bands, and each
    /// block of the terrain swept by its height. The GE lays them with its stencil (the
    /// framebuffer's alpha): cleared, then raised terrain marked from each chunk's height layer
    /// (it keeps its light), then the bands strongest first, each px shaded once toward `shade`
    /// by the strongest band over it, as `soft`'s mask takes the most. Strengths in sixteenths.
    fn silhouettes(
        &mut self,
        frame: &Frame,
        sun: &jane_present::Directional,
        shade: jane_present::Rgb,
        (casters, blocks): (jane_present::Span, jane_present::Span),
        px: &mut dyn PagePx,
    ) {
        use jane_present::shadow;
        let Some(k) = shadow::shear(sun) else { return };
        let (w, h) = (self.w, self.h);
        let mut levels = core::mem::take(&mut self.shadow_levels);
        levels.resize_with(16, Vec::new);
        for l in &mut levels {
            l.clear();
        }
        let mut band = |b: shadow::Band| {
            let (x0, x1, y0, y1) = (b.x0.max(0), b.x1.min(w), b.y0.max(0), b.y1.min(h));
            if x0 < x1 && y0 < y1 && b.strength >= 16 {
                let (x0, y0, x1, y1) = (x0 as i16, y0 as i16, x1 as i16, y1 as i16);
                let l = &mut levels[usize::from(b.strength >> 4)];
                // A band just under the last of its strength, as wide (a fence's rows, a
                // column's), or just beside it, as tall: one rect.
                match l.last_mut() {
                    Some(r) if r.0 == x0 && r.2 == x1 && (r.3 == y0 || r.1 == y1) => {
                        (r.1, r.3) = (r.1.min(y0), r.3.max(y1));
                    }
                    Some(r) if r.1 == y0 && r.3 == y1 && (r.2 == x0 || r.0 == x1) => {
                        (r.0, r.2) = (r.0.min(x0), r.2.max(x1));
                    }
                    _ => l.push((x0, y0, x1, y1)),
                }
            }
        };
        for c in frame.casters_in(casters) {
            let Some(s) = frame.sprites.get(c.sprite as usize) else { continue };
            let Some(i) = self.find(s.page, s.src.x, s.src.y) else { continue };
            let Some(t) = self.targets[i] else { continue };
            let (rx, ry) = self.rects[i];
            // Its rows depend on its px and where it stands over its foot: kept from frame to
            // frame while those hold (a prop, a house; a walking unit's frame changes).
            let key = RowsKey {
                src: (s.src.x, s.src.y, s.src.w, s.src.h, s.page),
                mirror: s.flags.mirror,
                bend: s.flags.bend.packed(),
                foot: c.foot.1 - s.y,
                burn: c.burn,
                height: c.height,
            };
            self.rows_clock = self.rows_clock.wrapping_add(1);
            if let Some(e) = self.rows_kept.iter_mut().find(|e| e.0 == key) {
                e.2 = self.rows_clock;
                shadow::bands(&e.1, i32::from(s.x), c, k, &mut band);
                continue;
            }
            // The sprite's px in the presenter's rect, from its pack page: 2 opaque, 1 the
            // contact shadow, 0 clear (all `shadow::rows` asks).
            let (sw, sh) = (usize::from(s.src.w), usize::from(s.src.h));
            let (lx0, ly0) = (i32::from(s.src.x) - i32::from(rx), i32::from(s.src.y) - i32::from(ry));
            let mask = self.opacity(i, t, px);
            let tw = usize::from(t.w);
            self.caster_px.clear();
            self.caster_px.resize(sw * sh, 0);
            for v in 0..sh as i32 {
                let ty = ly0 + v - i32::from(t.ty);
                if ty < 0 || ty >= i32::from(t.h) {
                    continue;
                }
                for u in 0..sw as i32 {
                    let tx = lx0 + u - i32::from(t.tx);
                    if tx < 0 || tx >= i32::from(t.w) {
                        continue;
                    }
                    let k = ty as usize * tw + tx as usize;
                    let o = (self.masks[mask].1[k / 4] >> ((k % 4) * 2)) & 3;
                    self.caster_px[v as usize * sw + u as usize] = u16::from(o);
                }
            }
            let local = SpriteCmd { src: jane_present::Src { x: 0, y: 0, w: s.src.w, h: s.src.h }, ..*s };
            self.caster_rows.clear();
            shadow::rows(&self.caster_px, s.src.w, &local, c, &mut self.caster_rows);
            shadow::bands(&self.caster_rows, i32::from(s.x), c, k, &mut band);
            if self.rows_kept.len() >= ROWS_KEPT {
                let old = (0..self.rows_kept.len()).min_by_key(|&e| self.rows_kept[e].2).unwrap_or(0);
                self.rows_kept.swap_remove(old);
            }
            self.rows_kept.push((key, self.caster_rows.clone(), self.rows_clock));
        }
        // The terrain's blocks: each its footprint swept along the sun over its height, a
        // convex polygon at full strength (where `soft` lays the same sweep a row at a time).
        let mut polys = core::mem::take(&mut self.polys);
        for b in frame.blocks_in(blocks) {
            if let Some(p) = block_poly(b, k, (w, h)) {
                polys.push(p);
            }
        }
        if levels.iter().all(Vec::is_empty) && polys.is_empty() {
            self.polys = polys;
            self.shadow_levels = levels;
            return;
        }
        let quad = |tex: Tex, mode: Mode, colour: u32, (x0, y0, x1, y1): (i16, i16, i16, i16), uv: (u16, u16)| Quad {
            tex,
            mode,
            colour,
            x0,
            y0,
            x1,
            y1,
            u0: 0,
            v0: 0,
            u1: uv.0,
            v1: uv.1,
        };
        let all = (0, 0, w as i16, h as i16);
        self.quads.push(quad(Tex::None, Mode::StencilClear, 0xff00_0000, all, (0, 0)));
        for &(cx, cy, slot) in &self.placed {
            if frame.layers.get(usize::from(slot)).is_some_and(jane_present::ChunkLayers::has_height) {
                let r = (cx as i16, cy as i16, (cx + CHUNK_PX) as i16, (cy + CHUNK_PX) as i16);
                self.quads.push(quad(
                    Tex::Height(slot),
                    Mode::StencilMark,
                    0xffff_ffff,
                    r,
                    (CHUNK_PX as u16, CHUNK_PX as u16),
                ));
            }
        }
        let full = rgba(shadow::shade_at(shade, 255), 255);
        for (i, (pts, n)) in polys.iter().enumerate() {
            let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
            for &(x, y) in &pts[..usize::from(*n)] {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            self.quads.push(quad(Tex::Poly(i as u16), Mode::ShadowBand, full, (x0, y0, x1, y1), (0, 0)));
            self.shadow_runs += 1;
        }
        self.polys = polys;
        for (lv, rects) in levels.iter().enumerate().rev() {
            let tint = rgba(shadow::shade_at(shade, (lv << 4 | lv) as u8), 255);
            for &r in rects {
                self.quads.push(quad(Tex::None, Mode::ShadowBand, tint, r, (0, 0)));
                self.shadow_runs += 1;
            }
        }
        self.shadow_levels = levels;
    }

    /// Ref `i`'s opacity on its trimmed rect (2 bits a px: 0 clear, 1 the contact shadow, 2
    /// opaque), read from its pack page once and kept (the least recently used goes past
    /// [`MASKS_KEPT`] bytes). Its index in `masks`.
    fn opacity(&mut self, i: usize, t: Target, px: &mut dyn PagePx) -> usize {
        self.mask_clock = self.mask_clock.wrapping_add(1);
        if let Some(m) = self.masks.iter().position(|m| m.0 == i) {
            self.masks[m].2 = self.mask_clock;
            return m;
        }
        let (w, h) = (usize::from(t.w), usize::from(t.h));
        let mut bits = alloc::vec![0u8; (w * h).div_ceil(4)];
        for y in 0..h {
            for x in 0..w {
                let o = px.texel(t.page, t.u + x as u16, t.v + y as u16).min(2);
                let k = y * w + x;
                bits[k / 4] |= o << ((k % 4) * 2);
            }
        }
        let mut held: usize = self.masks.iter().map(|m| m.1.len()).sum::<usize>() + bits.len();
        while held > MASKS_KEPT && !self.masks.is_empty() {
            let old = (0..self.masks.len()).min_by_key(|&m| self.masks[m].2).unwrap_or(0);
            held -= self.masks[old].1.len();
            self.masks.swap_remove(old);
        }
        self.masks.push((i, bits, self.mask_clock));
        self.masks.len() - 1
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
                        let c = if l.is_t8() { l.t8_abgr(k) } else { abgr(l.albedo[k]) };
                        p.px[((y - y0) as u32 * tw + (x - x0) as u32) as usize] = c | 0xff00_0000;
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
    fn a_block_swept_along_the_sun_is_a_hexagon() {
        // A 10 x 4 footprint, 16 px tall, the sun throwing a px of height a px west and up.
        let b = jane_present::Block { x0: 20, y0: 30, x1: 30, y1: 34, height: 16, lo: 0, fence: false };
        let (pts, n) = block_poly(&b, (-256, -256), (480, 272)).expect("it casts");
        assert_eq!(n, 6);
        let mut got: Vec<(i16, i16)> = pts[..6].to_vec();
        got.sort_unstable();
        assert_eq!(got, [(4, 14), (4, 18), (14, 14), (20, 34), (30, 30), (30, 34)]);
        // Too low to cast: nothing.
        assert!(block_poly(&jane_present::Block { height: 3, ..b }, (-256, -256), (480, 272)).is_none());
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
