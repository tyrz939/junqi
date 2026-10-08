//! The `Frame` as the GE's quads (PORT.md §13.12): every pass C2 draws, in the frame's order,
//! each sprite resolved from the presenter's page and rect to the PSP pack's page and trimmed
//! rect, clipped to the canvas, bent rows split into strips. Pure and integer: what `ge.rs`
//! sends is exactly this list, so a PC test reads what the PSP will draw.

use alloc::vec::Vec;

use jane_present::atlas::SpriteRef;
use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, Pass, SpriteCmd, Tint};

use crate::pack::Pack;

mod atmos;

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
    /// The casting lamps' shadows in the lightmap.
    pub const LAMP_SHADOWS: u8 = 16;
    /// The sprites' relief to the lamp that lights them most (else the sun's).
    pub const LAMP_RELIEF: u8 = 32;
    pub const ALL: u8 = 63;
}

/// The atmosphere's passes ([`Lister::atmos_off`]: each bit set leaves one out, to measure it or
/// to degrade by the `Features` ladder).
pub mod atmos_fx {
    /// The water's shimmer.
    pub const WATER: u8 = 1;
    /// The particles: rain, splashes, sparks, smoke, leaves.
    pub const PARTICLES: u8 = 2;
    /// The fog's mist tile.
    pub const FOG: u8 = 4;
    /// The sky beyond the zone's edge and its far things.
    pub const SKY: u8 = 8;
    /// The grade's saturation (its tables stay).
    pub const SATURATION: u8 = 16;
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
    /// The halo disc (`light::disc`), stretched with bilinear filtering.
    Disc,
    /// A glowing particle's disc (`light::spot`: `soft`'s falloff), stretched bilinear.
    Spot,
    /// A light's pool (`light::pool_disc`), stretched with bilinear filtering.
    Pool,
    /// A cached pool, the shadows of what stands still in it (`lamps`), by its slot.
    LampTex(u16),
    /// The GE's own lightmap (its render target in VRAM), stretched four times.
    LightRt,
    /// No texture: this frame's convex polygon `polys[i]` (a block's shadow, swept along the
    /// sun), drawn as a triangle fan; the quad's rect is its bounds.
    Poly(u16),
    /// A pack page through its glow CLUT, added over the light (what glows on it).
    Glow(u16),
    /// A chunk slot's height layer as `T8` through a CLUT that is clear at ground height: what
    /// marks the raised terrain in the stencil.
    Height(u16),
    /// A pack page's normal page (`T4`) through a light CLUT: the sun's ([`Lister::relief`],
    /// `u16::MAX`) or a lamp's (`Lister::lamp_reliefs[i]`): the sprite's relief.
    Normal(u16, u16),
    /// The frame drawn so far (the back buffer) read as `T32`, channel `c` (0 red, 1 green, 2
    /// blue) through CLUT `cluts[k]`: the grade's tables and its luma (`grade`).
    Frame(u8, u16),
    /// A smooth-shaded triangle strip, `strips[i]` (the sky, the grade's far pull, the fog).
    Strip(u16),
    /// A 1-px line from `(x0, y0)` to `(x1, y1)`, the quad's colour at the first end and, when
    /// it fades, clear at the second (a streak of rain, a spark); a ring is lines that do not.
    Line(bool),
}

/// A vertex of a smooth-shaded strip ([`Tex::Strip`]): canvas px, a texel (a textured strip's),
/// `0xAABBGGRR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vert {
    pub x: i16,
    pub y: i16,
    pub u: u16,
    pub v: u16,
    pub colour: u32,
}

/// What a strip samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripTex {
    /// Its vertices' colours alone.
    Flat,
    /// The mist tile through this frame's fog CLUT ([`Lister::fog_clut`]), repeating.
    Mist,
}

/// A triangle strip of `verts[start..start + len]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strip {
    pub start: u32,
    pub len: u16,
    pub tex: StripTex,
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

/// A caster's sun bands kept: the shear and depth they are for, and the rects.
type SunBands = ((i32, i32), u8, Vec<(i32, i32, i32, i32, u8)>);

/// A caster's rows kept: what they depend on (and its hash, compared first), the rows, the
/// draw they were last used in, and its sun bands for a shear and a depth (merged runs of rows,
/// `(x0, y0, x1, y1, level)` from its sprite's left edge and its foot row).
#[derive(Debug)]
struct KeptRows {
    key: RowsKey,
    hash: u32,
    rows: Vec<(i32, i32, i32)>,
    clock: u32,
    sun: Option<SunBands>,
}

impl RowsKey {
    fn hash(&self) -> u32 {
        let (x, y, w, h, p) = self.src;
        let mut v = u32::from(x) << 16 ^ u32::from(y) ^ (u32::from(w) << 8 ^ u32::from(h)).rotate_left(7);
        v = v.wrapping_mul(0x9e37_79b1) ^ u32::from(p) ^ u32::from(self.mirror) << 8 ^ self.bend.rotate_left(13);
        v = v.wrapping_mul(0x85eb_ca77)
            ^ self.foot as u32
            ^ u32::from(self.burn.0) << 16
            ^ u32::from(self.burn.1) << 24;
        v.wrapping_mul(0xc2b2_ae3d) ^ u32::from(self.height)
    }
}

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
    /// As `Multiply2`, the texel's alpha not read (the GE's lightmap keeps its stencil there).
    Multiply2Opaque,
    /// The texel added to what is under it (a glow).
    AddGlow,
    /// Its colour added toward the darks: `src * (1 - dst) + dst` (the grade's lift).
    Lift,
    /// The texel's alpha times the colour added (a light's halo, a pool on the lightmap).
    Halo,
    /// As `Halo` where the stencil is not set (a casting light's pool outside its shadows).
    PoolLit,
    /// As `Halo` where the stencil is set (the light bouncing into its own shadows).
    PoolShade,
    /// Draw into the lightmap target from here, cleared to the quad's colour (its rect).
    RtBegin,
    /// Back to the frame.
    RtEnd,
    /// The stencil (the framebuffer's alpha) cleared under it, no colour written.
    StencilClear,
    /// The stencil set where its texel is not clear, no colour written.
    StencilMark,
    /// What is under it times its colour where the stencil is clear, and the stencil set: each
    /// px shaded once.
    ShadowBand,
    /// Only channel `c` written, the texel as it is (the grade's table for that channel).
    Lut(u8),
    /// `src + dst * (1 - colour)`, the texel times the colour: the frame mixed toward its luma
    /// by the colour's share (the grade's saturation, down).
    Desaturate,
    /// `dst - src`, the texel times the colour (the grade's saturation, up: less of its luma).
    Subtract,
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
    /// Its pack category (`pack::UNITS`: it moves).
    cat: u8,
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

/// Lamp CLUTs a frame's relief takes at most (64 bytes each).
pub const LAMP_RELIEFS: usize = 96;

/// The lightmap target's side, cells (a mark past it is dropped).
const SIDE_CELLS: i32 = crate::light::SIDE as i32;
/// A caster's rows a lamp's slab pair spans: two of the lightmap's cells.
const BAND: i32 = 2 * crate::light::CELL;
/// The still casters a light not cached (her lantern) casts a frame at most, nearest first.
const PLAIN_CASTERS: usize = 24;

/// A polygon: up to eight corners in order round it, and how many.
pub type Poly = ([(i16, i16); 8], u8);

/// Block `b`'s shadow from `lamp` on the lightmap's cells (`step` [`SUB`](jane_present::shadow::SUB)
/// steps a cell): its footprint's corners at its foot (or a rail's underside) and at its top, each
/// laid on the ground along the ray from the light (`t = d * h / (h - z)`, held to the light's
/// reach and [`SHADOW_PAST`](jane_present::shadow::SHADOW_PAST) past it, as `shadow::project`),
/// and its sides turned from the light as quads between them (the stencil takes their union).
/// `None` for a block too low, too far from the light, or off the target.
fn lamp_block_poly(
    b: &jane_present::Block,
    lamp: &jane_present::shadow::Lamp,
    step: i32,
    (w, h): (i32, i32),
) -> Option<([Poly; 4], usize)> {
    use jane_present::shadow::{self, SUB};
    let hgt = i32::from(b.height);
    if hgt <= shadow::GROUND {
        return None;
    }
    let (x0, y0, x1, y1) = (i32::from(b.x0) * SUB, i32::from(b.y0) * SUB, i32::from(b.x1) * SUB, i32::from(b.y1) * SUB);
    // Near enough the light to throw any of it (`block_slabs`'s rule).
    let (nx, ny) = (lamp.x.clamp(x0, x1) - lamp.x, lamp.y.clamp(y0, y1) - lamp.y);
    let near = (lamp.r + 48) * SUB;
    if i64::from(nx) * i64::from(nx) + i64::from(ny) * i64::from(ny) > i64::from(near) * i64::from(near) {
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
    /// This frame's lamp CLUTs for the relief of sprites a lamp lights (`Tex::Normal`).
    pub lamp_reliefs: Vec<[u32; crate::normals::DIRS]>,
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
    caster_px: Vec<u16>,
    /// Casters' rows as `shadow::rows` gave them, `(what they depend on, rows, last used)`.
    rows_kept: Vec<KeptRows>,
    rows_clock: u32,
    /// Casters' opacity by ref, `(ref, 2 bits a px, last used)` ([`Lister::opacity`]).
    masks: Vec<(usize, Vec<u8>, u32)>,
    mask_clock: u32,
    /// Shadow runs laid this frame (a stat).
    pub shadow_runs: u32,
    /// A clock (microseconds) the lister times its parts by, if the platform gives one, and
    /// what it measured: lightmap pools, lamp shadows, lightmap finish, casters' slabs, blocks'
    /// slabs (summed until read).
    pub clock: Option<fn() -> u32>,
    pub prof: [u32; 8],
    /// Lamp shadow slabs this frame (a stat), and their scratch.
    pub slab_count: u32,
    slabs: Vec<jane_present::shadow::Slab>,
    /// A lamp's stencil marks (scratch), and what stands still round a light being cached.
    lamp_marks: Vec<Quad>,
    still: Vec<jane_present::shadow::Slab>,
    /// What stands still this frame, for the lamp cache's checksums (scratch).
    fixed: Vec<(i32, i32, i32, i32, i32, u32)>,
    /// Which of this frame's casters move (scratch).
    moving: Vec<bool>,
    /// A light's casters this frame by distance (scratch).
    near: Vec<(i32, u32)>,
    /// The static lamps' pools with their shadows (`lamps`).
    pub lamps: crate::lamps::LampCache,
    /// A caster's rows coarsened for a lamp's shadow (scratch).
    coarse: Vec<(i32, i32, i32)>,
    /// The lighting effects drawn ([`fx`] bits; all by default): what a bench turns off to
    /// measure each one's cost.
    pub effects: u8,
    /// The atmosphere's passes left out ([`atmos_fx`] bits; none by default).
    pub atmos_off: u8,
    /// The frame's clear, `0xAABBGGRR`.
    pub clear: u32,
    /// Sprites this frame that resolved to nothing on the PSP (the UI page's, a ref C2 leaves out).
    pub misses: u32,
    /// This frame's smooth strips and their vertices (`Tex::Strip`).
    pub strips: Vec<Strip>,
    pub verts: Vec<Vert>,
    /// The CLUTs `Tex::Frame` reads through: the three lumas, then each band's three tables
    /// (`grade`); `cluts_gen` counts their rebuilds.
    pub cluts: Vec<[u32; 256]>,
    pub cluts_gen: u32,
    grade: crate::grade::Grade,
    /// The frame's sky, when it has one (the grade's afterglow, the backdrop).
    sky: Option<jane_present::frame::SkyLook>,
    /// This frame's fog CLUT for the mist tile (`StripTex::Mist`): entry `m` the fog's colour at
    /// its weight for `m`.
    pub fog_clut: [u32; 256],
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
                    cat: rec.cat,
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
            lamp_reliefs: Vec::with_capacity(LAMP_RELIEFS),
            has_glow: pack.pages.iter().map(|p| p.glow.is_some()).collect(),
            glows: Vec::with_capacity(64),
            polys: Vec::with_capacity(256),
            shadow_levels: Vec::new(),
            caster_px: Vec::new(),
            masks: Vec::new(),
            mask_clock: 0,
            rows_kept: Vec::new(),
            rows_clock: 0,
            shadow_runs: 0,
            slab_count: 0,
            clock: None,
            prof: [0; 8],
            slabs: Vec::new(),
            coarse: Vec::new(),
            lamp_marks: Vec::new(),
            still: Vec::new(),
            fixed: Vec::new(),
            moving: Vec::new(),
            near: Vec::new(),
            lamps: crate::lamps::LampCache::default(),
            effects: fx::ALL,
            atmos_off: 0,
            clear: 0xff00_0000,
            misses: 0,
            strips: Vec::with_capacity(32),
            verts: Vec::with_capacity(512),
            cluts: Vec::new(),
            cluts_gen: 0,
            grade: crate::grade::Grade::default(),
            sky: None,
            fog_clut: [0; 256],
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
        self.lamp_reliefs.clear();
        self.slab_count = 0;
        self.lamps.begin();
        self.glows.clear();
        self.polys.clear();
        self.quads.clear();
        self.chunks.clear();
        self.placed.clear();
        self.patches_used = 0;
        self.misses = 0;
        self.strips.clear();
        self.verts.clear();
        self.sky = None;
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
                        let t = self.now();
                        self.silhouettes(frame, &sun, shade, (casters, blocks), px);
                        self.prof[5] += self.now().wrapping_sub(t);
                    }
                }
                Pass::Sprites { cmds, .. } => {
                    let t = self.now();
                    for (k, s) in frame.sprites_in(cmds).iter().enumerate() {
                        self.sprite(s, &frame.lights, cmds.start + k as u32);
                        if let Some(f) = s.foot
                            && s.flags.tint != Tint::Seen
                        {
                            self.patch(frame, s, f);
                        }
                    }
                    self.prof[6] += self.now().wrapping_sub(t);
                }
                Pass::Lights { ambient, points, casters, blocks, .. } => {
                    lit = true;
                    // The terrain's own glow (lit windows), over the light with the sprites'.
                    if self.effects & fx::GLOW != 0 {
                        self.chunk_glows(frame);
                    }
                    let points = frame.lights_in(points);
                    if !points.is_empty() && self.effects & fx::LAMPS != 0 {
                        // T0's lightmap (`soft`'s): the ambient and every pool, at half size,
                        // half a cell back so the GE's filter lands its cells where `soft`'s do.
                        let t0 = self.now();
                        self.gpu_lightmap(frame, ambient, points, (casters, blocks), px);
                        self.prof[1] += self.now().wrapping_sub(t0);
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
                            tex: Tex::LightRt,
                            mode: Mode::Multiply2Opaque,
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
                // The sky's look, read by the grade (its afterglow).
                Pass::Sky(sky) => {
                    self.sky = Some(sky);
                    if self.atmos_off & atmos_fx::SKY == 0 {
                        self.sky(&sky, &frame.stars[sky.star_list.range()]);
                    }
                }
                Pass::Parallax { sprites, .. } => {
                    if let Some(sky) = self.sky.filter(|_| self.atmos_off & atmos_fx::SKY == 0) {
                        for (k, sp) in frame.sprites_in(sprites).iter().enumerate() {
                            self.far_thing(&sky, sp, sprites.start + k as u32);
                        }
                    }
                }
                Pass::Fog { volumes, drift } => {
                    if self.atmos_off & atmos_fx::FOG == 0 {
                        self.fog(frame.fog_in(volumes), frame.camera, drift);
                    }
                }
                Pass::Water { cells } => {
                    if self.atmos_off & atmos_fx::WATER == 0 {
                        self.water(frame.water_in(cells), frame.tick);
                    }
                }
                Pass::Particles { parts, .. } => {
                    if self.atmos_off & atmos_fx::PARTICLES == 0 {
                        self.particles(frame.parts_in(parts));
                    }
                }
                // What the sky is doing reaches C2 as it reaches T0: through the ambient and the
                // grade, its rain as particles. Rays a C2 frame never holds (`Features::c2`).
                Pass::Weather(_) | Pass::Rays { .. } => {}
                Pass::Post(p) => self.grade(&p),
            }
        }
        // A frame with no light pass (the day on T0), or a light pass last: its glows now.
        self.quads.append(&mut self.glows);
        &self.quads
    }

    /// A sprite's quads: its trimmed rect on its PSP page, by strips where it bends.
    fn sprite(&mut self, s: &SpriteCmd, lights: &[jane_present::Light], index: u32) {
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
        // Its relief: the same quads again over its normal page, through the CLUT of the lamp
        // that lights it most if one does, else the sun's.
        if s.flags.tint == Tint::None && self.has_normals.get(usize::from(t.page)).copied().unwrap_or(false) {
            let clut = self.lamp_relief(lights, s, index).or(self.relief.map(|_| u16::MAX));
            if let Some(c) = clut {
                for k in first..self.quads.len() {
                    let q = self.quads[k];
                    self.quads.push(Quad {
                        tex: Tex::Normal(t.page, c),
                        mode: Mode::Multiply2,
                        colour: 0xffff_ffff,
                        ..q
                    });
                }
            }
        }
    }

    /// The CLUT of the lamp that lights sprite `s` most at its middle, if one does and the
    /// effect is on: its index in `lamp_reliefs`.
    fn lamp_relief(&mut self, lights: &[jane_present::Light], s: &SpriteCmd, index: u32) -> Option<u16> {
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

    /// Caster `c`'s rows (`shadow::rows`) from its pack page, kept from frame to frame while its
    /// px and where it stands over its foot hold (a prop, a house; a walking unit's frame
    /// changes): their index in `rows_kept`, and its sprite's left edge.
    fn rows_of(&mut self, frame: &Frame, c: &jane_present::Caster, px: &mut dyn PagePx) -> Option<(usize, i32)> {
        use jane_present::shadow;
        let s = frame.sprites.get(c.sprite as usize)?;
        let i = self.find(s.page, s.src.x, s.src.y)?;
        let t = self.targets[i]?;
        let (rx, ry) = self.rects[i];
        let key = RowsKey {
            src: (s.src.x, s.src.y, s.src.w, s.src.h, s.page),
            mirror: s.flags.mirror,
            bend: s.flags.bend.packed(),
            foot: c.foot.1 - s.y,
            burn: c.burn,
            height: c.height,
        };
        self.rows_clock = self.rows_clock.wrapping_add(1);
        let hash = key.hash();
        if let Some(e) = self.rows_kept.iter().position(|e| e.hash == hash && e.key == key) {
            self.rows_kept[e].clock = self.rows_clock;
            return Some((e, i32::from(s.x)));
        }
        // The sprite's px in the presenter's rect, from its pack page: 2 opaque, 1 the contact
        // shadow, 0 clear (all `shadow::rows` asks).
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
        let mut rows = Vec::new();
        shadow::rows(&self.caster_px, s.src.w, &local, c, &mut rows);
        if self.rows_kept.len() >= ROWS_KEPT {
            let old = (0..self.rows_kept.len()).min_by_key(|&e| self.rows_kept[e].clock).unwrap_or(0);
            self.rows_kept.swap_remove(old);
        }
        self.rows_kept.push(KeptRows { key, hash, rows, clock: self.rows_clock, sun: None });
        self.prof[2] += 1;
        Some((self.rows_kept.len() - 1, i32::from(s.x)))
    }

    fn now(&self) -> u32 {
        self.clock.map_or(0, |c| c())
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
    fn gpu_lightmap(
        &mut self,
        frame: &Frame,
        ambient: jane_present::Rgb,
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
        self.quads.push(rt(Mode::RtBegin, base_colour(ambient), Tex::None, all, 0));
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
        if shadows {
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
                    // What stands still round it, cast once into its texture.
                    still.clear();
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
                    for b in frame.blocks_in(blocks) {
                        shadow::block_slabs(b, &lamp, |q| still.push(q));
                    }
                    self.lamps.build(slot, l.pos, l.radius, cell, &still);
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
                // A prop's box sides too, as in the cached textures; people stay their outline.
                if !moving[ci] {
                    shadow::side_slabs(&self.coarse, x, c, &lamp, |q| slabs.push(q));
                }
            }
            self.near = near;
            let t1 = self.now();
            if cached.is_none() {
                for b in frame.blocks_in(blocks) {
                    let Some((quads, n)) = lamp_block_poly(b, &lamp, step, (w, h)) else { continue };
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
    fn mark(&mut self, p: Poly) {
        let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
        for &(x, y) in &p.0[..usize::from(p.1)] {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        if x1 < 0 || y1 < 0 || i32::from(x0) >= SIDE_CELLS || i32::from(y0) >= SIDE_CELLS {
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
            let Some((e, x)) = self.rows_of(frame, c, px) else { continue };
            // Its bands from its own rows once for this sun (they move with it, so are kept from
            // its left edge and foot row), runs of rows merged; then laid where it stands.
            let kept = &mut self.rows_kept[e];
            if kept.sun.as_ref().is_none_or(|s| s.0 != k || s.1 != c.depth) {
                let mut rects: Vec<(i32, i32, i32, i32, u8)> = Vec::new();
                let at_home = jane_present::Caster { foot: (c.foot.0, 0), ..*c };
                shadow::bands(&kept.rows, 0, &at_home, k, |b| {
                    if b.strength < 16 {
                        return;
                    }
                    let lv = b.strength >> 4;
                    // A row within a px of the run's each side joins it (C2: a crown's ragged edge
                    // a px coarser, a third of the rects).
                    match rects.iter_mut().rev().find(|r| r.4 == lv) {
                        Some(r) if (r.0 - b.x0).abs() <= 1 && (r.2 - b.x1).abs() <= 1 && b.y0 <= r.3 && b.y1 >= r.1 => {
                            (r.0, r.1, r.2, r.3) = (r.0.min(b.x0), r.1.min(b.y0), r.2.max(b.x1), r.3.max(b.y1));
                        }
                        _ => rects.push((b.x0, b.y0, b.x1, b.y1, lv)),
                    }
                });
                kept.sun = Some((k, c.depth, rects));
            }
            let fy = i32::from(c.foot.1);
            if let Some((_, _, rects)) = &self.rows_kept[e].sun {
                for &(x0, y0, x1, y1, lv) in rects {
                    band(shadow::Band {
                        x0: x0 + x,
                        y0: y0 + fy,
                        x1: x1 + x,
                        y1: y1 + fy,
                        strength: lv << 4,
                        reach: 1,
                    });
                }
            }
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
    fn the_atmosphere_is_drawn_as_soft_draws_it_and_each_pass_can_be_left_out() {
        use jane_present::Span;
        use jane_present::frame::{FogVolume, PartShape, Particle, SkyLook, StarCmd, WaterCmd};
        let mut f = Frame::new(jane_present::Tier::T0);
        f.canvas = (480, 272);
        let sky = SkyLook {
            zenith: [20, 30, 80],
            horizon: [200, 140, 100],
            glow: [255, 140, 60],
            glow_x: 100,
            glow_amount: 0,
            stars: 255,
            star_list: Span { start: 0, len: 1 },
            moon: None,
            zone: (0, 40, 480, 272),
            tick: 0,
        };
        f.stars.push(StarCmd { x: 10, up: 20, bright: 200 });
        f.passes.push(Pass::Sky(sky));
        f.water.push(WaterCmd { x: 100, y: 100, phase: 0 });
        f.passes.push(Pass::Water { cells: Span { start: 0, len: 1 } });
        let part = |shape| Particle { x: 200, y: 150, shape, colour: [200, 210, 230], alpha: 200, glow: 0, height: 0 };
        f.parts.extend([
            part(PartShape::Streak { dx: -3, dy: -12 }),
            part(PartShape::Ring { r: 5 }),
            part(PartShape::Dot { size: 2 }),
        ]);
        f.passes.push(Pass::Particles { layer: jane_present::Depth::Weather, parts: Span { start: 0, len: 3 } });
        f.fog.push(FogVolume { rect: (-64, -64, 544, 336), edge: 16, density: 120, colour: [200, 200, 210], top: 0 });
        f.passes.push(Pass::Fog { volumes: Span { start: 0, len: 1 }, drift: (3, 0) });
        let mut l = lister();
        let q = l.build(&f).to_vec();
        let count = |t: fn(&Tex) -> bool| q.iter().filter(|q| t(&q.tex)).count();
        // The sky's rows above the zone's top (40 px: five strips); the fog, its edges off the
        // canvas, one.
        assert_eq!(l.strips.iter().filter(|s| s.tex == StripTex::Flat).count(), 5);
        assert_eq!(l.strips.iter().filter(|s| s.tex == StripTex::Mist).count(), 1);
        // The star, the glint and the dot are flat quads; the streak fades, the ring does not.
        assert_eq!(count(|t| *t == Tex::None), 3);
        assert_eq!(count(|t| *t == Tex::Line(true)), 1);
        assert_eq!(count(|t| *t == Tex::Line(false)), 12);
        // The fog's CLUT is never clear (a haze under the wisps), at most 230 of 256.
        assert!(l.fog_clut.iter().all(|&c| c >> 24 > 0 && c >> 24 <= 230));
        l.atmos_off = atmos_fx::SKY | atmos_fx::FOG | atmos_fx::PARTICLES | atmos_fx::WATER;
        l.build(&f);
        assert!(l.quads.is_empty() && l.strips.is_empty());
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
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false), &[], 0);
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
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, true), &[], 0);
        let q = l.quads[0];
        assert_eq!((q.x0, q.x1, q.u0, q.u1), (22, 28, 106, 100));
        // Clipped at the canvas's left: the first 3 columns drawn go.
        l.quads.clear();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, false), &[], 0);
        let q = l.quads[0];
        // Px 2..8 at -5 + 2.. -5 + 8 = -3..3: columns 0..3 show texels 103..106.
        assert_eq!((q.x0, q.x1, q.u0, q.u1), (0, 3, 103, 106));
        l.quads.clear();
        l.sprite(&cmd(Src { x: 0, y: 0, w: 10, h: 12 }, -5, 30, true), &[], 0);
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
        l.sprite(&cmd(Src { x: 0, y: 12, w: 6, h: 6 }, 0, 0, false), &[], 0);
        assert!(l.quads.is_empty());
        assert_eq!(l.misses, 1);
    }

    #[test]
    fn a_bent_sprite_is_strips_shifted_by_the_bend() {
        let mut l = lister();
        let mut c = cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 30, false);
        c.flags.bend = jane_present::Bend { lean: 2, from: 8, span: 4 };
        l.sprite(&c, &[], 0);
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
