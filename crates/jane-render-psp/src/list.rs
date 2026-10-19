//! The `Frame` as the GE's quads (PORT.md §13.12): every pass C2 draws, in the frame's order,
//! each sprite resolved from the presenter's page and rect to the PSP pack's page and trimmed
//! rect, clipped to the canvas, bent rows split into strips. Pure and integer: what `ge.rs`
//! sends is exactly this list, so a PC test reads what the PSP will draw.

use alloc::vec::Vec;

use jane_present::atlas::SpriteRef;
use jane_present::frame::CHUNK_PX;
use jane_present::{Frame, Pass, SpriteCmd, Tint};

use crate::capture::pass as P;
use crate::pack::Pack;

mod atmos;
pub mod guard;
pub mod order;
mod shafts;
mod water;

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
    pub const WATER: u16 = 1;
    /// The particles: rain, splashes, sparks, smoke, leaves.
    pub const PARTICLES: u16 = 2;
    /// The fog's mist tile.
    pub const FOG: u16 = 4;
    /// The sky beyond the zone's edge and its far things.
    pub const SKY: u16 = 8;
    /// The grade's saturation (its tables stay).
    pub const SATURATION: u16 = 16;
    /// Water as T1 draws it: the sky mirrored in it, its crest and trough rows, and what stands
    /// over it mirrored and rippled.
    pub const REFLECT: u16 = 32;
    /// Rain on the ground: wet ground darker, puddles mirroring as the water does.
    pub const PUDDLES: u16 = 64;
    /// Each lamp's glint run long down the water and the wet ground toward her.
    pub const STREAKS: u16 = 128;
    /// Light shafts through what stands against a low sun.
    pub const SHAFTS: u16 = 256;
    /// The grade: its tables, its saturation and its far pull (the shafts' CLUT is still made).
    pub const GRADE: u16 = 512;
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
    /// The puddles' noise tile (`water::noise_tile`) through this frame's threshold CLUT
    /// ([`Lister::noise_clut`]), repeating, world-anchored.
    Noise,
    /// A smooth-shaded triangle strip, `strips[i]` (the sky, the grade's far pull, the fog).
    Strip(u16),
    /// A 1-px line from `(x0, y0)` to `(x1, y1)`, the quad's colour at the first end and, when
    /// it fades, clear at the second (a streak of rain, a spark); a ring is lines that do not.
    Line(bool),
    /// A pack page through an all-white CLUT, times the quad's colour: the UI's ink (text, a
    /// mark flattened to one colour; `ui`).
    Ink(u16),
    /// The frame's UI image in this slot (`Frame::ui_images`), as an `8888` texture (`ui`).
    Image(u16),
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
/// sprite's feet, clear elsewhere, `0xAABBGGRR`. Its texels are `h` rows of `stride` words at
/// `at` in [`Lister::patch_px`], the one arena every patch of the frame shares ([`PATCH_WORDS`]),
/// and its texture is declared `tw` by `th` (powers of two over `w` and `h`: the GE reads only the
/// `w` by `h` the quad names).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Patch {
    /// Where its first row begins in the arena, words (a multiple of 4: 16-byte aligned).
    pub at: u32,
    pub w: u16,
    pub h: u16,
    /// A row's words: `w` up to a multiple of 4 (a texture's buffer width).
    pub stride: u16,
    pub tw: u16,
    pub th: u16,
}

/// The words of a frame's patches, all of them, at most: 128 KB. Patches were a buffer each, kept
/// for ever and each at least 16 KB; a wood of swatches once held two megabytes of them (PSP v0.4.1
/// plan, E). The lister fills one arena (reserved as the frames need it, from [`PATCH_FIRST`]),
/// the GE glue copies what is used into its own once a frame, and a patch that does not fit is
/// not laid.
pub const PATCH_WORDS: usize = 32 * 1024;
/// The arena's first size, words: 16 KB.
pub const PATCH_FIRST: usize = 4 * 1024;
/// The patches a frame holds at most.
pub const PATCHES_MOST: usize = 96;
/// The polygons a frame holds at most (34 bytes each: a 35 KB table, small enough for the heap's
/// pool), of which the sun's blocks take [`SUN_POLYS_MOST`]; the lamps' marks take the rest.
pub const POLYS_MOST: usize = 1024;
/// The sun's block polygons a frame lays at most; the rest of its blocks cast no shadow.
pub const SUN_POLYS_MOST: usize = 768;

/// The quads the sun's shadow pass lays at most (its clear, its raised marks, its polygons and
/// its bands): the bands, a dozen to each caster, are what a wood's frame is made of (6,279 of
/// the 6,688 quads at the densest place two scans of 134 county places found, 150,1250 at
/// 17:22), and a still denser wood would starve the passes after them (the sprites over their
/// shadows, the light) of the buffer. Held to this, the weakest bands, laid last, are
/// the ones left out. It is about the most a list can carry of them beside the rest of a frame
/// and the UI: 7,168 flat quads are 172 KB of the 232 KB the world may have, so a wood past it
/// thins its faintest shadows by degrees rather than losing the pass whole to the list guard.
pub const SUN_QUADS_MOST: usize = 7168;

/// The most quads a buffer of the lister holds (a frame's, its glows', ..): the sun's pass's most
/// and 4,096 for the rest of the frame (the densest seen takes 600). A frame past this could not
/// be drawn whole whatever else was cut (a quad takes 24 to 32 bytes of the 256 KB list), and it
/// is the most the heap is ever asked for in one piece, 352 KB, asked fallibly. An infallible
/// growth once asked for 464,192 bytes of a heap whose largest free block was 204 KB, and the
/// program ended (PSP v0.4.1 plan, E).
pub const QUADS_MOST: usize = 11264;
/// A buffer's first size, and the least it grows by.
const QUADS_FIRST: usize = 1024;

/// A `Vec<Quad>` that only grows by what the heap can give, to [`QUADS_MOST`]: a quad that does
/// not fit is counted in [`QuadBuf::dropped`] and left out, where a `Vec` would end the program
/// on a failed allocation. Reads as a slice.
#[derive(Debug, Default)]
pub struct QuadBuf {
    v: Vec<Quad>,
    /// Quads left out since the last [`QuadBuf::take_dropped`].
    pub dropped: u32,
}

impl QuadBuf {
    pub fn with_capacity(n: usize) -> QuadBuf {
        QuadBuf { v: Vec::with_capacity(n), dropped: 0 }
    }

    /// Adds `q`, or drops it when the buffer is at its most or the heap cannot grow it.
    #[inline]
    pub fn push(&mut self, q: Quad) {
        if self.v.len() == self.v.capacity() && !self.grow() {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.v.push(q);
    }

    /// Room for more (doubled, to [`QUADS_MOST`]), if the heap has it.
    #[cold]
    fn grow(&mut self) -> bool {
        let len = self.v.len();
        if len >= QUADS_MOST {
            return false;
        }
        let more = (len * 2).clamp(QUADS_FIRST, QUADS_MOST) - len;
        self.v.try_reserve_exact(more).is_ok()
    }

    /// Every quad of `other` after this one's, `other` emptied; those that do not fit dropped.
    pub fn append(&mut self, other: &mut QuadBuf) {
        self.dropped = self.dropped.saturating_add(core::mem::take(&mut other.dropped));
        if self.v.capacity() - self.v.len() >= other.v.len() {
            self.v.append(&mut other.v);
            return;
        }
        for q in other.v.drain(..) {
            self.push(q);
        }
    }

    pub fn clear(&mut self) {
        self.v.clear();
    }

    pub fn truncate(&mut self, n: usize) {
        self.v.truncate(n);
    }

    pub fn pop(&mut self) -> Option<Quad> {
        self.v.pop()
    }

    pub fn remove(&mut self, k: usize) -> Quad {
        self.v.remove(k)
    }

    /// Quads the buffer can hold without growing.
    pub fn capacity(&self) -> usize {
        self.v.capacity()
    }

    /// The count of quads dropped since the last call, and zero.
    pub fn take_dropped(&mut self) -> u32 {
        core::mem::take(&mut self.dropped)
    }
}

impl core::ops::Deref for QuadBuf {
    type Target = [Quad];
    fn deref(&self) -> &[Quad] {
        &self.v
    }
}

impl core::ops::DerefMut for QuadBuf {
    fn deref_mut(&mut self) -> &mut [Quad] {
        &mut self.v
    }
}

impl<'a> IntoIterator for &'a QuadBuf {
    type Item = &'a Quad;
    type IntoIter = core::slice::Iter<'a, Quad>;
    fn into_iter(self) -> Self::IntoIter {
        self.v.iter()
    }
}

impl<const N: usize> PartialEq<[Quad; N]> for QuadBuf {
    fn eq(&self, other: &[Quad; N]) -> bool {
        self.v == other[..]
    }
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
type SunBands = ((i32, i32), u8, Vec<(i32, i32, i32, i32, u8)>, (i32, i32, i32, i32));

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

/// Casters' rows kept between frames. (Measured at the PSP v0.4.1 plan's dense wood, 393,1305 at
/// 17:22 on PPSSPP: 0.45 to 1.1 rows built a frame with 105 to 180 casters, so a swaying caster's
/// `bend` in the key costs a miss or two a frame, not the plan's three; the key stays.)
const ROWS_KEPT: usize = 128;
/// Bytes of casters' opacity kept between frames.
const MASKS_KEPT: usize = 96 * 1024;
/// The largest patch laid: a sprite past it is drawn whole over the terrain.
const PATCH_MOST: i32 = 256;
// A patch is read from the relief cells alone (`relief_cells`, over `shadow::GROUND`): terrain
// that hides a foot stands over `Foot::RELIEF`, higher still.
const _: () = assert!((jane_present::shadow::GROUND as u8) < jane_present::Foot::RELIEF);

/// How a quad lays its colour on what is under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The texel times the quad's colour, over by its alpha (a ghost is the colour's alpha).
    Alpha,
    /// The texel's colour, its alpha not read (a console chunk, whose CLUT's alpha marks its
    /// water and wet ground).
    Opaque,
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
    /// The stencil set to [`RAISED`] where its texel is not clear, no colour written: the
    /// raised terrain the sun's silhouettes leave lit. Not 1, the shade's value, so what reads
    /// the shade after (the light shafts) does not take a roof or a hedge for shade.
    RaisedMark,
    /// What is under it times its colour where the stencil is clear (0: neither shaded nor
    /// raised), and the stencil set to 1: each px shaded once.
    ShadowBand,
    /// Only channel `c` written, the texel as it is (the grade's table for that channel).
    Lut(u8),
    /// `src + dst * (1 - colour)`, the texel times the colour: the frame mixed toward its luma
    /// by the colour's share (the grade's saturation, down).
    Desaturate,
    /// `dst - src`, the texel times the colour (the grade's saturation, up: less of its luma).
    Subtract,
    /// The stencil's bits not in `keep` set from `value` where the texel's alpha is `tag` (0: any
    /// texel not clear) and, with `need`, where the stencil has one of `need`'s bits; no colour
    /// written. What the water and the wet ground are, marked from the chunks' CLUTs
    /// (`frame::T8_WATER`, ..), and the puddles over the wet ground.
    Mark { tag: u8, value: u8, keep: u8, need: Option<u8> },
    /// Laid by `Blend` only where the stencil passes `Where`; the stencil kept.
    Masked(Blend, Where),
}

/// How a [`Mode::Masked`] quad lays its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// As [`Mode::Alpha`].
    Alpha,
    /// As [`Mode::Multiply`] (flat).
    Multiply,
    /// The texel's alpha times the colour, added (as [`Mode::Halo`]).
    Add,
    /// The texel's colour times the quad's, added whole (its alpha not read).
    Glow,
}

/// Where a [`Mode::Masked`] quad lays: the stencil equal to a value, its bits under a mask equal
/// to a value's, or the stencil over a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    Is(u8),
    Bits(u8, u8),
    Over(u8),
}

/// The stencil's values while the water is drawn (`list/water.rs`), cleared after.
pub mod marks {
    /// Ground that darkens in rain, and ground that shines too (the low two bits).
    pub const WET: u8 = 1;
    pub const SHINE: u8 = 2;
    pub const GROUND: u8 = 3;
    /// Water (the whole value).
    pub const WATER: u8 = 3;
    /// A puddle on wet ground: a bit over its ground's.
    pub const PUDDLE: u8 = 4;
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
/// Whether a quad of mode `m` reads or writes the stencil (the framebuffer's alpha), or leaves
/// the frame (the lightmap's target), or writes the alpha on purpose: what must come after the
/// sun's pass for the frame's clear to stand for its stencil clear.
fn touches_stencil(m: Mode) -> bool {
    matches!(
        m,
        Mode::StencilClear
            | Mode::StencilMark
            | Mode::RaisedMark
            | Mode::ShadowBand
            | Mode::PoolLit
            | Mode::PoolShade
            | Mode::RtBegin
            | Mode::RtEnd
            | Mode::Lut(_)
            | Mode::Mark { .. }
            | Mode::Masked(..)
    )
}

/// Most rects a chunk's relief is laid in; past it, one span a row of cells (still only rows
/// with relief, from their first raised cell to their last).
const RELIEF_RECTS_MOST: usize = 24;

/// A rect of a chunk's cells, `(x0, y0, x1, y1)`.
type CellRect = (u8, u8, u8, u8);

/// A row of cells' bits laid as spans of cells, `(x0, x1)`, each handed on.
type Spans<'a> = dyn Fn(u32, &mut dyn FnMut(u8, u8)) + 'a;

/// The cells of a chunk's height layer (`CHUNK_PX` square, a byte a px) with any px over
/// `shadow::GROUND` (what `Mode::RaisedMark`'s CLUT marks), as rects of cells `(x0, y0, x1, y1)`:
/// each row's runs, a run joined to the one above it when they span the same cells.
fn relief_cells(height: &[u8], out: &mut Vec<CellRect>) {
    // Four px a word: a px over 4 is one of 8 or more (a high bit) or 5 to 7 (bit 2 and bit 0
    // or 1), tested a byte at a time inside the word with no carry between them.
    const _: () = assert!(jane_present::shadow::GROUND == 4);
    let raised = |w: u32| w & 0xf8f8_f8f8 != 0 || (w >> 2) & (w | w >> 1) & 0x0101_0101 != 0;
    let (n, cells, cell) =
        (CHUNK_PX as usize, jane_present::frame::CHUNK_CELLS as usize, jane_present::frame::CELL as usize);
    out.clear();
    if height.len() < n * n {
        out.push((0, 0, cells as u8, cells as u8));
        return;
    }
    let mut rows = [0u32; 16];
    for (cy, row) in rows.iter_mut().enumerate().take(cells) {
        for y in cy * cell..(cy + 1) * cell {
            let line = &height[y * n..(y + 1) * n];
            for (cx, px) in line.chunks_exact(cell).enumerate() {
                if *row >> cx & 1 == 0
                    && px.chunks_exact(4).any(|b| raised(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
                {
                    *row |= 1 << cx;
                }
            }
        }
    }
    lay_cells(&rows, out);
}

/// The cells of a `T8` chunk whose CLUT marks any px of them as water (`frame::T8_WATER`, what
/// the water's `Mode::Mark` keeps), as rects of cells as [`relief_cells`] lays them: the water
/// passes' tight bounds (PLAN-PSP-SPEED.md W1).
fn water_cells(l: &jane_present::ChunkLayers, out: &mut Vec<CellRect>) {
    let (n, cells, cell) =
        (CHUNK_PX as usize, jane_present::frame::CHUNK_CELLS as usize, jane_present::frame::CELL as usize);
    out.clear();
    if !l.is_t8() || l.albedo.len() * 4 < n * n || l.clut.len() < 256 {
        out.push((0, 0, cells as u8, cells as u8));
        return;
    }
    // The CLUT's water entries, a bit each.
    let mut wet = [0u64; 4];
    for (i, &c) in l.clut[..256].iter().enumerate() {
        if (c >> 24) as u8 == jane_present::frame::T8_WATER {
            wet[i / 64] |= 1 << (i % 64);
        }
    }
    let mut rows = [0u32; 16];
    if wet != [0; 4] {
        let is = |ix: u32| wet[(ix >> 6) as usize] >> (ix & 63) & 1 != 0;
        // Four px a word, a cell's row four words.
        let (words, per) = (n / 4, cell / 4);
        for (cy, row) in rows.iter_mut().enumerate().take(cells) {
            for y in cy * cell..(cy + 1) * cell {
                let line = &l.albedo[y * words..(y + 1) * words];
                for (cx, px) in line.chunks_exact(per).enumerate() {
                    if *row >> cx & 1 == 0
                        && px.iter().any(|&w| is(w & 0xff) || is(w >> 8 & 0xff) || is(w >> 16 & 0xff) || is(w >> 24))
                    {
                        *row |= 1 << cx;
                    }
                }
            }
        }
    }
    lay_cells(&rows, out);
}

/// A chunk's rows of cells (a bit a cell) laid as rects of cells `(x0, y0, x1, y1)`: each row's
/// runs, a run joined to the one above it when they span the same cells; past
/// [`RELIEF_RECTS_MOST`], one span a row.
fn lay_cells(rows: &[u32; 16], out: &mut Vec<CellRect>) {
    let cells = jane_present::frame::CHUNK_CELLS as usize;
    let runs = |m: u32, f: &mut dyn FnMut(u8, u8)| {
        let mut x = 0;
        while x < cells {
            if m >> x & 1 == 0 {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < cells && m >> x & 1 != 0 {
                x += 1;
            }
            f(x0 as u8, x as u8);
        }
    };
    let lay = |spans: &Spans<'_>, out: &mut Vec<CellRect>| {
        out.clear();
        // The rects still open: those ending at the row above.
        for (cy, &m) in rows.iter().enumerate().take(cells) {
            let y = cy as u8;
            spans(m, &mut |x0, x1| match out.iter_mut().find(|r| r.3 == y && r.0 == x0 && r.2 == x1) {
                Some(r) => r.3 = y + 1,
                None => out.push((x0, y, x1, y + 1)),
            });
        }
    };
    lay(&runs, out);
    if out.len() > RELIEF_RECTS_MOST {
        let span = |m: u32, f: &mut dyn FnMut(u8, u8)| {
            if m != 0 {
                f(m.trailing_zeros() as u8, (32 - m.leading_zeros()) as u8);
            }
        };
        lay(&span, out);
    }
}

fn block_poly(b: &jane_present::Block, k: (i32, i32), (w, h): (i32, i32)) -> Option<([(i16, i16); 8], u8)> {
    let pts = block_sweep(b, k, (0, 0))?;
    let (mx0, my0) = pts.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
    let (mx1, my1) = pts.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
    if mx1 <= 0 || my1 <= 0 || mx0 >= w || my0 >= h {
        return None;
    }
    Some(sweep_hull(&pts))
}

/// [`hull`] of [`block_sweep`]'s eight points (a rect at its foot and the same rect at its top)
/// without sorting them: the hull of a rect swept along a segment is the rect plus the segment,
/// whose edges are the rect's four and the sweep's two (merged with the rect's where they lie
/// along an axis) in order of angle, from the lowest of the leftmost corners: the same corners
/// in the same order as `hull` lays them (a test holds it to that).
fn sweep_hull(p: &[(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    let (a, b) = (p[0], p[4]);
    let (w, h) = (p[1].0 - p[0].0, p[3].1 - p[0].1);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let start = if a <= b { a } else { b };
    let (ex, ey) = match (dx, dy) {
        (_, 0) => (w + dx.abs(), h),
        (0, _) => (w, h + dy.abs()),
        _ => (w, h),
    };
    // The sweep pointing right (none along an axis: merged into the rect's edges above).
    let v = if dx == 0 || dy == 0 {
        None
    } else if dx > 0 {
        Some((dx, dy))
    } else {
        Some((-dx, -dy))
    };
    let mut edges = [(0i32, 0i32); 6];
    let mut n = 0;
    let mut edge = |e: (i32, i32)| {
        edges[n] = e;
        n += 1;
    };
    let down = v.filter(|v| v.1 < 0);
    let up = v.filter(|v| v.1 > 0);
    if let Some(v) = down {
        edge(v);
    }
    edge((ex, 0));
    if let Some(v) = up {
        edge(v);
    }
    edge((0, ey));
    if let Some(v) = down {
        edge((-v.0, -v.1));
    }
    edge((-ex, 0));
    if let Some(v) = up {
        edge((-v.0, -v.1));
    }
    edge((0, -ey));
    let mut r = [(0i16, 0i16); 8];
    let mut at = start;
    for (d, e) in r.iter_mut().zip(&edges[..n]) {
        *d = (at.0.clamp(-2048, 2047) as i16, at.1.clamp(-2048, 2047) as i16);
        at = (at.0 + e.0, at.1 + e.1);
    }
    (r, n as u8)
}

/// [`block_poly`]'s eight points before the hull, `o` px over the block's (a camera's offset:
/// zone px for a kept polygon); `None` for a block too low to cast.
fn block_sweep(b: &jane_present::Block, (kx, ky): (i32, i32), o: (i32, i32)) -> Option<[(i32, i32); 8]> {
    use jane_present::shadow;
    let hgt = i32::from(b.height);
    let (mut x0, y0, mut x1, y1) =
        (i32::from(b.x0) + o.0, i32::from(b.y0) + o.1, i32::from(b.x1) + o.0, i32::from(b.y1) + o.1);
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
    Some(pts)
}

/// The convex hull of eight points, round it (Andrew's monotone chain), at most eight corners:
/// what [`sweep_hull`] is held to.
#[cfg(test)]
fn hull(p: [(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    // Within 8192 px of the canvas's corner every cross product fits an `i32` (the MIPS has no
    // 64-bit multiply): the same answers, a third of the time.
    if p.iter().all(|q| q.0.unsigned_abs() < 8192 && q.1.unsigned_abs() < 8192) {
        hull_with::<true>(p)
    } else {
        hull_with::<false>(p)
    }
}

#[cfg(test)]
fn hull_with<const SMALL: bool>(mut p: [(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    p.sort_unstable();
    let cross = |o: (i32, i32), a: (i32, i32), b: (i32, i32)| {
        if SMALL {
            i64::from((a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0))
        } else {
            i64::from(a.0 - o.0) * i64::from(b.1 - o.1) - i64::from(a.1 - o.1) * i64::from(b.0 - o.0)
        }
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
/// The stencil's value over raised terrain while the sun's silhouettes are laid
/// ([`Mode::RaisedMark`]); the shade is 1.
pub const RAISED: u8 = 2;

/// The lightmap target's side, cells (a mark past it is dropped).
const SIDE_CELLS: i32 = crate::light::SIDE as i32;
/// A caster's rows a lamp's slab pair spans: two of the lightmap's cells.
const BAND: i32 = 2 * crate::light::CELL;
/// The still casters a light not cached (her lantern) casts a frame at most, nearest first.
const PLAIN_CASTERS: usize = 24;

/// The palette grade (the Graphics page's cheap grade, PORT.md §13.13): the grade's tables laid
/// on each colour as it is drawn, before the light (the GE's CLUTs, the quads' and strips'
/// colours, the patches' px), instead of reading the lit frame back through them; the lift laid
/// after the light. `generation` moves when the tables do.
///
/// The night (NIGHT.md §4.2, §4.7) is folded in here too: at `night` 1 to 4 a world page's and a
/// chunk's CLUT entries go to the night's materials first (`jane_art::palette::night_of`, by
/// colour: the console's CLUTs hold colours, not indices), so the turn is one rebuild a page and
/// no new VRAM. `grade` says whether the tables grade at all (else they are the identity and
/// only the night is folded: the full grade reads the frame back).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub generation: u32,
    pub lut: [[u8; 256]; 3],
    pub sat: i32,
    pub night: u8,
    pub grade: bool,
}

impl Palette {
    /// A colour (`0xAABBGGRR`) graded.
    pub fn colour(&self, c: u32) -> u32 {
        if !self.grade {
            return c;
        }
        crate::grade::palette_colour(c, &self.lut, self.sat)
    }

    /// A world CLUT's colour (`0xAABBGGRR`) at the night's intensity, then graded.
    pub fn world(&self, c: u32) -> u32 {
        self.colour(night_abgr(c, self.night))
    }

    /// The tables only fold the night: nothing else is laid by them.
    pub fn night_only(&self) -> bool {
        !self.grade
    }
}

/// `c` (`0xAABBGGRR`) as the night's material at intensity `i` (NIGHT.md §4.2), its alpha kept.
pub fn night_abgr(c: u32, i: u8) -> u32 {
    if i == 0 || c >> 24 == 0 {
        return c;
    }
    let [r, g, b] = jane_art::palette::night_of([c as u8, (c >> 8) as u8, (c >> 16) as u8], i);
    c & 0xff00_0000 | u32::from(b) << 16 | u32::from(g) << 8 | u32::from(r)
}

/// The identity tables: a palette that only folds the night.
pub const IDENTITY: [[u8; 256]; 3] = {
    let mut t = [[0u8; 256]; 3];
    let mut k = 0;
    while k < 3 {
        let mut v = 0;
        while v < 256 {
            t[k][v] = v as u8;
            v += 1;
        }
        k += 1;
    }
    t
};

/// A polygon: up to eight corners in order round it, and how many.
pub type Poly = ([(i16, i16); 8], u8);

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

/// The texels a slice of a wide read-back reads across at most: 64 bytes of a 32-bit texture
/// (PORT.md §13.13). The GE's texture cache (8 KB) holds blocks of 16 bytes by 8 rows; a quad
/// 480 px across a 32-bit texture touches 120 blocks a row, more than the cache holds, so each
/// block is fetched once a row instead of once in eight. In slices of 16 texels each block is
/// used for its eight rows before it is let go (the PSP SDK's "blit in slices").
pub const SLICE_TEXELS: i32 = 16;

/// `q` cut into vertical slices reading at most [`SLICE_TEXELS`] texels across each, cut only
/// where both the px and the texel are whole (a quad's px a texel stays the same in each), into
/// `out`; a mirrored quad, or one that cannot be cut so, is `q` alone.
pub fn slices(q: &Quad, out: &mut Vec<Quad>) {
    out.clear();
    let (dx, du) = (i32::from(q.x1) - i32::from(q.x0), i32::from(q.u1) - i32::from(q.u0));
    if dx <= 0 || du <= SLICE_TEXELS {
        out.push(*q);
        return;
    }
    // The smallest step whose px and texels are both whole, then as many as fit a slice.
    let g = gcd(dx, du);
    let (px_unit, tx_unit) = (dx / g, du / g);
    let k = (SLICE_TEXELS / tx_unit).max(1);
    let (step_px, step_tx) = (px_unit * k, tx_unit * k);
    let mut x = i32::from(q.x0);
    let mut u = i32::from(q.u0);
    while x < i32::from(q.x1) {
        let x1 = (x + step_px).min(i32::from(q.x1));
        let u1 = (u + step_tx).min(i32::from(q.u1));
        out.push(Quad { x0: x as i16, x1: x1 as i16, u0: u as u16, u1: u1 as u16, ..*q });
        (x, u) = (x1, u1);
    }
}

fn gcd(a: i32, b: i32) -> i32 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
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

/// `c` at `k` of 256 (the night's turn's share of the sky's light, over 256 on its crest).
fn dim(c: [u8; 3], k: u32) -> [u8; 3] {
    c.map(|v| (u32::from(v) * k.min(512) / 256).min(255) as u8)
}

/// `a - b` per channel of two `0xAABBGGRR` colours, floored at 0, alpha full.
fn sub_rgb(a: u32, b: u32) -> u32 {
    let ch = |s: u32| ((a >> s) & 0xff).saturating_sub((b >> s) & 0xff) << s;
    0xff00_0000 | ch(16) | ch(8) | ch(0)
}

/// The night's turn as horizontal strips over `rows` rows (each `unit` canvas px tall: a
/// lightmap cell's, or a px): `f(y0, y1, share)` for each run of rows whose share of the sky's
/// light (`Band::at`, sixteenths through the soft edge) is the same, top to bottom. A band of
/// nothing is one strip at all of it.
fn band_strips(band: jane_present::Band, rows: i32, unit: i32, mut f: impl FnMut(i32, i32, u32)) {
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

/// The passes whose quads may be grouped by texture ([`Lister::order_passes`]): those whose
/// quads only blend each px with what is under it (no stencil, no target, no read-back).
pub const ORDERED: u32 = 1 << P::SPRITES | 1 << P::PARTICLES | 1 << P::GLOW;

/// The frame's quads, rebuilt each frame into reused buffers.
#[derive(Debug)]
pub struct Lister {
    /// The presenter's sprites by where they sit: `(page, y, x, ref)`, sorted.
    by_pos: Vec<(u8, u16, u16, u16)>,
    /// Each presenter ref's rect, and how tall its thing truly stands (`SpriteRef::top`).
    rects: Vec<(u16, u16)>,
    tops: Vec<u8>,
    targets: Vec<Option<Target>>,
    pub quads: QuadBuf,
    /// This frame's chunks: `(slot, generation)`.
    pub chunks: Vec<(u16, u32)>,
    /// This frame's chunks where they are drawn: `(x, y, slot)`, canvas px.
    placed: Vec<(i32, i32, u16)>,
    /// Each chunk slot's cells with relief as rects (`relief_cells`), by the generation they were
    /// read at: where the sun's pass marks raised terrain.
    relief_rects: Vec<Option<(u32, Vec<CellRect>)>>,
    /// `relief_rects` read this frame for the first this many chunks placed (`None`: not yet).
    relief_read: Option<usize>,
    /// Each chunk slot's cells with water as rects (`water_cells`), by generation: the water's
    /// tight bounds (`list/water.rs`, W1).
    water_rects: Vec<Option<(u32, Vec<CellRect>)>>,
    /// This frame's water as a grid of cells, a row of cells a word (`list/water.rs`'s `Cover`).
    water_cover: Vec<u64>,
    /// Tests: the water laid as before W1 and W3 (one box, a quad a band), to hold the tight
    /// passes to it px for px.
    #[cfg(test)]
    water_loose: bool,
    /// The quad the sun's shadow pass begins at when its stencil clear is the frame's own: the
    /// GE clears the stencil with the frame and writes no alpha before that quad (Q1).
    pub stencil_from_clear: Option<u32>,
    /// This frame's patches (`Tex::Patch`, by index), and their texels: every patch's rows one
    /// after another, up to [`PATCH_WORDS`] words, reserved once (the first patch wanted).
    pub patches: Vec<Patch>,
    pub patch_px: Vec<u32>,
    /// Patches this frame left out for want of room in the arena or the count.
    pub patches_skipped: u32,
    /// The heap could not give the arena this frame (it is asked once a frame).
    arena_failed: bool,
    /// Polygons this frame left out for want of room ([`POLYS_MOST`], [`SUN_POLYS_MOST`]).
    pub polys_skipped: u32,
    /// The sun's shadow bands this frame left out for want of room ([`SUN_QUADS_MOST`]).
    bands_skipped: u32,
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
    glows: QuadBuf,
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
    /// slabs (summed until read); `[10]` the draws grouped by texture ([`order`]), and `[11]`
    /// counts the blocks that threw a shadow from a light cast each frame.
    pub clock: Option<fn() -> u32>,
    pub prof: [u32; 12],
    /// Lamp shadow slabs this frame (a stat), and their scratch.
    pub slab_count: u32,
    slabs: Vec<jane_present::shadow::Slab>,
    /// A lamp's stencil marks (scratch), and what stands still round a light being cached.
    lamp_marks: QuadBuf,
    still: Vec<jane_present::shadow::Slab>,
    /// Scratch: the px (canvas px, or the lightmap's cells) a light's fences draw over.
    fence_rects: Vec<(i32, i32, i32, i32)>,
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
    pub atmos_off: u16,
    /// The grade laid on the colours as they are drawn ([`Palette`]) rather than read back.
    pub palette_grade: bool,
    /// This frame's palette grade, when it has one (the GE grades its CLUTs by it).
    pub palette: Option<Palette>,
    /// The night's intensity this frame's world CLUTs fold (`Frame::night`, NIGHT.md §4.2).
    pub night: u8,
    /// The frame's clear, `0xAABBGGRR`.
    pub clear: u32,
    /// Where each GE pass begins: `(quad index, capture::pass id)`, in order (the GE's signals
    /// time them, PORT.md §13.13).
    pub marks: Vec<(u32, u8)>,
    /// Sub-passes inside a pass (`capture::pass::parent`), as `marks`: signalled only when the
    /// passes are timed, never read by the list guard (a pass is shed whole by its own mark).
    pub sub_marks: Vec<(u32, u8)>,
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
    /// The water's troughs (scratch: laid after its crests, one batch each).
    water_dim: QuadBuf,
    /// This frame's threshold CLUT for the puddles' noise tile (`Tex::Noise`).
    pub noise_clut: [u32; 256],
    /// The sun or the moon as the silhouettes have it (the light shafts').
    sun: Option<jane_present::Directional>,
    /// The frame's weather and flat light, as their passes come (the water's).
    weather: Option<jane_present::frame::Atmos>,
    ambient: [u8; 3],
    fill: [u8; 3],
    /// This frame's fog CLUT for the mist tile (`StripTex::Mist`): entry `m` the fog's colour at
    /// its weight for `m`.
    pub fog_clut: [u32; 256],
    /// The passes whose quads are grouped by texture where no two that touch swap ([`order`],
    /// PLAN-PSP-SPEED.md Q6a), bit `capture::pass` id, of [`ORDERED`] (a script's `order:N`; 0:
    /// each pass in its own order). The sprites by default: a wood's trees on a few pages give
    /// a fifth of its binds back (PPSSPP, the bench's wood: 94 to 74); the particles and the
    /// glows gave two to four for twice the sprites' cost.
    pub order_passes: u32,
    order: order::Order,
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
            tops: refs.iter().map(|r| r.top).collect(),
            targets,
            quads: QuadBuf::with_capacity(QUADS_FIRST),
            chunks: Vec::with_capacity(32),
            placed: Vec::with_capacity(32),
            relief_rects: Vec::new(),
            relief_read: None,
            water_rects: Vec::new(),
            water_cover: Vec::with_capacity(24),
            #[cfg(test)]
            water_loose: false,
            stencil_from_clear: None,
            patches: Vec::new(),
            patch_px: Vec::new(),
            patches_skipped: 0,
            arena_failed: false,
            polys_skipped: 0,
            bands_skipped: 0,
            light: crate::light::LightMap::default(),
            relief: None,
            has_normals: pack.pages.iter().map(|p| p.normal.is_some()).collect(),
            lamp_reliefs: Vec::with_capacity(LAMP_RELIEFS),
            has_glow: pack.pages.iter().map(|p| p.glow.is_some()).collect(),
            glows: QuadBuf::with_capacity(64),
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
            prof: [0; 12],
            slabs: Vec::new(),
            coarse: Vec::new(),
            lamp_marks: QuadBuf::default(),
            still: Vec::new(),
            fence_rects: Vec::new(),
            fixed: Vec::new(),
            moving: Vec::new(),
            near: Vec::new(),
            lamps: crate::lamps::LampCache::default(),
            effects: fx::ALL,
            atmos_off: 0,
            palette_grade: false,
            palette: None,
            night: 0,
            clear: 0xff00_0000,
            marks: Vec::with_capacity(32),
            sub_marks: Vec::with_capacity(16),
            misses: 0,
            strips: Vec::with_capacity(32),
            verts: Vec::with_capacity(512),
            cluts: Vec::new(),
            cluts_gen: 0,
            grade: crate::grade::Grade::default(),
            sky: None,
            fog_clut: [0; 256],
            noise_clut: [0; 256],
            water_dim: QuadBuf::default(),
            weather: None,
            sun: None,
            ambient: [255; 3],
            fill: [255; 3],
            order_passes: 1 << P::SPRITES,
            order: order::Order::default(),
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

    /// The patches' arena: 1 once it is reserved (the first patch wanted; 0 until), and its
    /// bytes. Flat from then on, whatever a frame lays.
    pub fn patch_bytes(&self) -> (usize, usize) {
        (usize::from(self.patch_px.capacity() > 0), self.patch_px.capacity() * 4)
    }

    /// The patches' arena able to hold `end` words: reserved as the frames need it (doubled from
    /// [`PATCH_FIRST`] to [`PATCH_WORDS`], what it holds kept) and then kept, so a scene with a
    /// patch or two costs 16 KB and a wood of them at most 128; false when the heap cannot give
    /// it (it is asked once a frame).
    fn arena(&mut self, end: usize) -> bool {
        if self.patch_px.capacity() >= end {
            return true;
        }
        if self.arena_failed {
            return false;
        }
        let target = end.next_power_of_two().clamp(PATCH_FIRST, PATCH_WORDS);
        if self.patch_px.try_reserve_exact(target - self.patch_px.len()).is_ok() {
            return true;
        }
        self.arena_failed = true;
        false
    }

    /// What this frame left out for want of room: quads (any of the buffers at [`QUADS_MOST`],
    /// or the heap unable to grow one, and the sun's bands past [`SUN_QUADS_MOST`]), polygons and
    /// patches. The frame is the poorer; the program still runs.
    pub fn left_out(&self) -> (u32, u32, u32) {
        let q = self.quads.dropped + self.glows.dropped + self.lamp_marks.dropped + self.water_dim.dropped;
        (q + self.bands_skipped, self.polys_skipped, self.patches_skipped)
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
        self.marks.clear();
        self.sub_marks.clear();
        self.stencil_from_clear = None;
        self.palette = None;
        self.night = frame.night.min(4);
        self.chunks.clear();
        self.placed.clear();
        self.relief_read = None;
        self.patches.clear();
        self.patch_px.clear();
        (self.patches_skipped, self.polys_skipped, self.bands_skipped, self.arena_failed) = (0, 0, 0, false);
        self.quads.dropped = 0;
        self.glows.dropped = 0;
        self.lamp_marks.dropped = 0;
        self.water_dim.dropped = 0;
        self.misses = 0;
        self.strips.clear();
        self.verts.clear();
        self.sky = None;
        self.weather = None;
        self.sun = None;
        self.ambient = [255; 3];
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
        let mut watered = false;
        for pass in &frame.passes {
            // The water and the wet ground as T1 draws them, over the light (the sky they mirror
            // is not lit by the ground's light), under what glows.
            if lit && !watered {
                watered = true;
                self.pass_mark(P::SHAFTS);
                self.shafts();
                self.pass_mark(P::WATER_FX);
                self.water_fx(frame);
            }
            // What glows goes over the light: laid as the pass after the light comes.
            if lit && !self.glows.is_empty() {
                self.pass_mark(P::GLOW);
                self.quads.append(&mut self.glows);
            }
            self.pass_mark(match *pass {
                Pass::Terrain { .. } => P::TERRAIN,
                Pass::Silhouettes { .. } => P::SUN_SHADOWS,
                Pass::Sprites { .. } => P::SPRITES,
                Pass::Lights { .. } => P::LIGHTMAP,
                Pass::Sky(_) | Pass::Parallax { .. } => P::SKY,
                Pass::Fog { .. } => P::FOG,
                Pass::Water { .. } => P::SHIMMER,
                Pass::Particles { .. } => P::PARTICLES,
                Pass::Weather(_) | Pass::Rays { .. } | Pass::Post(_) => P::SATURATION,
            });
            match *pass {
                Pass::Terrain { chunks } => {
                    for c in frame.chunks_in(chunks) {
                        self.chunks.push((c.slot, c.generation));
                        self.placed.push((c.x, c.y, c.slot));
                        let q = Quad {
                            tex: Tex::Chunk(c.slot),
                            mode: Mode::Opaque,
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
                    self.sun = Some(sun);
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
                Pass::Lights { ambient, fill, points, casters, blocks, band, .. } => {
                    lit = true;
                    self.ambient = ambient;
                    self.fill = fill;
                    // The terrain's own glow (lit windows), over the light with the sprites'.
                    if self.effects & fx::GLOW != 0 {
                        self.chunk_glows(frame);
                    }
                    let points = frame.lights_in(points);
                    if !points.is_empty() && self.effects & fx::LAMPS != 0 {
                        // T0's lightmap (`soft`'s): the ambient and every pool, at half size,
                        // half a cell back so the GE's filter lands its cells where `soft`'s do.
                        let t0 = self.now();
                        self.gpu_lightmap(frame, ambient, band, points, (casters, blocks), px);
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
                        self.pass_mark(P::LIGHT_MUL);
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
                    } else if ambient.iter().any(|&c| c < 254) || !band.is_none() {
                        self.pass_mark(P::LIGHT_MUL);
                        // The flat light (T0's ambient, the sun's share in it): what is drawn,
                        // times it; through the night's turn a strip at a time down the band.
                        let w = self.w as i16;
                        let quads = &mut self.quads;
                        band_strips(band, self.h, 8, |y0, y1, k| {
                            quads.push(Quad {
                                tex: Tex::None,
                                mode: Mode::Multiply,
                                colour: rgba(dim(ambient, k), 255),
                                x0: 0,
                                y0: y0 as i16,
                                x1: w,
                                y1: y1 as i16,
                                u0: 0,
                                v0: 0,
                                u1: 0,
                                v1: 0,
                            });
                        });
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
                Pass::Weather(a) => self.weather = Some(a),
                Pass::Rays { .. } => {}
                Pass::Post(p) => self.grade(&p),
            }
        }
        if !watered {
            self.pass_mark(P::SHAFTS);
            self.shafts();
            self.pass_mark(P::WATER_FX);
            self.water_fx(frame);
        }
        // A frame with no light pass (the day on T0), or a light pass last: its glows now.
        if !self.glows.is_empty() {
            self.pass_mark(P::GLOW);
            self.quads.append(&mut self.glows);
        }
        // The night folded into the CLUTs even where nothing grades them (the full grade reads
        // the frame back; the grade off).
        if self.palette.is_none() && self.night > 0 {
            let n = self.night;
            self.palette = Some(Palette {
                generation: 0x8000_0000 | u32::from(n),
                lut: IDENTITY,
                sat: 128,
                night: n,
                grade: false,
            });
        }
        if let Some(p) = self.palette.filter(|p| p.grade) {
            self.palette_pass(&p);
        }
        // The passes that only blend, grouped by texture (fewer binds and CLUT loads, the same
        // px).
        let passes = self.order_passes & ORDERED;
        if passes != 0 {
            let t = self.now();
            let n = self.quads.len();
            for k in 0..self.marks.len() {
                let (at, id) = self.marks[k];
                if id < 32 && passes >> id & 1 != 0 {
                    let end = self.marks.get(k + 1).map_or(n, |m| m.0 as usize).min(n);
                    let at = (at as usize).min(end);
                    self.order.by_texture(&mut self.quads[at..end]);
                }
            }
            self.prof[10] += self.now().wrapping_sub(t);
        }
        // Marks with nothing after them go (the GE signals only where something is drawn).
        let n = self.quads.len() as u32;
        self.marks.retain(|m| m.0 < n);
        // A sub-pass with nothing in it goes: one at the quad another pass begins at (not its own
        // parent's first quad, where its first sub-pass begins with it).
        let marks = &self.marks;
        self.sub_marks
            .retain(|m| m.0 < n && !marks.iter().any(|p| p.0 == m.0 && Some(p.1) != crate::capture::pass::parent(m.1)));
        &self.quads
    }

    /// The palette grade on what the GE does not draw through a CLUT: the flat quads' and lines'
    /// colours and the smooth strips' over what is there (not the light's multiplies and adds),
    /// the fog's CLUT, the clear and the patches' px.
    fn palette_pass(&mut self, p: &Palette) {
        let graded = |q: &Quad| {
            matches!(q.tex, Tex::None | Tex::Line(_) | Tex::Spot | Tex::Strip(_))
                && matches!(q.mode, Mode::Alpha | Mode::AddGlow | Mode::Masked(Blend::Alpha, _))
        };
        for k in 0..self.quads.len() {
            let q = self.quads[k];
            if !graded(&q) {
                continue;
            }
            if let Tex::Strip(s) = q.tex {
                if let Some(st) = self.strips.get(usize::from(s)).filter(|st| st.tex == StripTex::Flat) {
                    let (a, n) = (st.start as usize, usize::from(st.len));
                    for v in &mut self.verts[a..a + n] {
                        v.colour = p.colour(v.colour);
                    }
                }
            } else {
                self.quads[k].colour = p.colour(q.colour);
            }
        }
        for e in &mut self.fog_clut {
            *e = p.colour(*e);
        }
        self.clear = p.colour(self.clear);
        for px in &mut self.patch_px {
            if *px >> 24 != 0 {
                *px = p.colour(*px);
            }
        }
    }

    /// GE pass `id` begins at the next quad (a mark at the same quad is replaced).
    pub(crate) fn pass_mark(&mut self, id: u8) {
        let at = self.quads.len() as u32;
        match self.marks.last_mut() {
            Some(m) if m.0 == at => m.1 = id,
            Some(m) if m.1 == id => {}
            _ => self.marks.push((at, id)),
        }
    }

    /// Sub-pass `id` begins at the next quad (a sub-mark at the same quad is replaced).
    pub(crate) fn sub_mark(&mut self, id: u8) {
        let at = self.quads.len() as u32;
        match self.sub_marks.last_mut() {
            Some(m) if m.0 == at => m.1 = id,
            _ => self.sub_marks.push((at, id)),
        }
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
    fn mark(&mut self, p: Poly) {
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

    /// Each placed chunk's relief cells (`relief_rects`) read for its generation, once a frame:
    /// where the sun's pass marks raised terrain, and the only cells a patch can come from (L4).
    fn relief_fresh(&mut self, frame: &Frame) {
        if self.relief_read == Some(self.placed.len()) {
            return;
        }
        self.relief_read = Some(self.placed.len());
        for k in 0..self.placed.len() {
            let slot = self.placed[k].2;
            let Some(l) = frame.layers.get(usize::from(slot)).filter(|l| l.has_height()) else { continue };
            let generation = self.chunks.iter().find(|c| c.0 == slot).map(|c| c.1);
            let s = usize::from(slot);
            if self.relief_rects.len() <= s {
                self.relief_rects.resize_with(s + 1, || None);
            }
            if generation.is_none() || self.relief_rects[s].as_ref().is_none_or(|r| Some(r.0) != generation) {
                let mut rects = self.relief_rects[s].take().map(|r| r.1).unwrap_or_default();
                relief_cells(&l.height, &mut rects);
                self.relief_rects[s] = Some((generation.unwrap_or(u32::MAX), rects));
            }
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
        let t_c = self.now();
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
                let bounds = rects.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |a, r| {
                    (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3))
                });
                kept.sun = Some((k, c.depth, rects, bounds));
            }
            let fy = i32::from(c.foot.1);
            if let Some((_, _, rects, b)) = &self.rows_kept[e].sun {
                // Wholly off the canvas: every band of it would be clipped away.
                if b.2 + x <= 0 || b.0 + x >= w || b.3 + fy <= 0 || b.1 + fy >= h {
                    continue;
                }
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
        self.prof[8] += self.now().wrapping_sub(t_c);
        // The terrain's blocks: each its footprint swept along the sun over its height, a
        // convex polygon at full strength (where `soft` lays the same sweep a row at a time).
        let mut polys = core::mem::take(&mut self.polys);
        let t_b = self.now();
        for b in frame.blocks_in(blocks) {
            if let Some(p) = block_poly(b, k, (w, h)) {
                // A frame's polygons are held ([`SUN_POLYS_MOST`]): the rest cast no shadow.
                if polys.len() >= SUN_POLYS_MOST {
                    self.polys_skipped += 1;
                    continue;
                }
                polys.push(p);
            }
        }
        self.prof[9] += self.now().wrapping_sub(t_b);
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
        let first = self.quads.len();
        // The stencil clear: the frame's own `sceGuClear` when nothing drawn before this pass
        // touched the stencil (`stencil_from_clear`: the GE keeps the alpha unwritten until here),
        // else a quad. The quads the pass may lay are counted as they were with the quad and one
        // whole-chunk mark a chunk, so the bands it keeps are the same.
        let mut counted = 1;
        if self.quads.iter().any(|q| touches_stencil(q.mode)) {
            self.sub_mark(P::SUN_CLEAR);
            self.quads.push(quad(Tex::None, Mode::StencilClear, 0xff00_0000, all, (0, 0)));
        } else {
            self.stencil_from_clear = Some(first as u32);
        }
        self.sub_mark(P::SUN_RAISED);
        // Raised terrain marked where the chunk has relief alone (most cells are flat), and on
        // the canvas alone: the same texels at the same px as a whole chunk's quad marks.
        self.relief_fresh(frame);
        for k in 0..self.placed.len() {
            let (cx, cy, slot) = self.placed[k];
            if !frame.layers.get(usize::from(slot)).is_some_and(jane_present::ChunkLayers::has_height) {
                continue;
            }
            counted += 1;
            let Some(Some((_, rects))) = self.relief_rects.get(usize::from(slot)) else { continue };
            for &(rx0, ry0, rx1, ry1) in rects {
                let c = |v: u8| i32::from(v) * jane_present::frame::CELL;
                let (x0, y0) = ((cx + c(rx0)).max(0), (cy + c(ry0)).max(0));
                let (x1, y1) = ((cx + c(rx1)).min(w), (cy + c(ry1)).min(h));
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                self.quads.push(Quad {
                    tex: Tex::Height(slot),
                    mode: Mode::RaisedMark,
                    colour: 0xffff_ffff,
                    x0: x0 as i16,
                    y0: y0 as i16,
                    x1: x1 as i16,
                    y1: y1 as i16,
                    u0: (x0 - cx) as u16,
                    v0: (y0 - cy) as u16,
                    u1: (x1 - cx) as u16,
                    v1: (y1 - cy) as u16,
                });
            }
        }
        self.sub_mark(P::SUN_POLYS);
        let full = rgba(shadow::shade_at(shade, 255), 255);
        for (i, (pts, n)) in polys.iter().enumerate() {
            let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
            for &(x, y) in &pts[..usize::from(*n)] {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            self.quads.push(quad(Tex::Poly(i as u16), Mode::ShadowBand, full, (x0, y0, x1, y1), (0, 0)));
            self.shadow_runs += 1;
            counted += 1;
        }
        self.polys = polys;
        // The bands strongest first, as many as the pass has room for: what it must leave out is
        // the faintest.
        self.sub_mark(P::SUN_BANDS);
        let mut room = SUN_QUADS_MOST.saturating_sub(counted);
        for (lv, rects) in levels.iter().enumerate().rev() {
            let tint = rgba(shadow::shade_at(shade, (lv << 4 | lv) as u8), 255);
            for &r in rects {
                if room == 0 {
                    self.bands_skipped += 1;
                    continue;
                }
                room -= 1;
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
        // Its rows in the arena, after the last patch's: each a multiple of 4 words (a texture's
        // buffer width), the patch 16-byte aligned. A patch past the count or the arena is not
        // laid (its sprite's feet show through what hides them), but only counted as left out
        // once something does hide them: found by the same scan, written or not.
        let stride = (w as usize).next_multiple_of(4);
        let at = self.patch_px.len().next_multiple_of(4);
        let words = stride * h as usize;
        let room = self.patches.len() < PATCHES_MOST && at + words <= PATCH_WORDS && self.arena(at + words);
        if room {
            self.patch_px.resize(at + words, 0);
        }
        let mut any = false;
        // Only the chunks' cells with relief (L4): a px no higher than `shadow::GROUND` hides
        // nothing (`Foot::hides` asks for more than `Foot::RELIEF`), and most cells are flat.
        self.relief_fresh(frame);
        'scan: for &(cx, cy, slot) in &self.placed {
            let Some(l) = frame.layers.get(usize::from(slot)) else { continue };
            if !l.has_height() {
                continue;
            }
            let Some(Some((_, rects))) = self.relief_rects.get(usize::from(slot)) else { continue };
            let cell = |v: u8| i32::from(v) * jane_present::frame::CELL;
            for &(rx0, ry0, rx1, ry1) in rects {
                let (ax0, ax1) = (x0.max(cx + cell(rx0)), x1.min(cx + cell(rx1)));
                let (ay0, ay1) = (y0.max(cy + cell(ry0)), y1.min(cy + cell(ry1)));
                for y in ay0..ay1 {
                    let row = ((y - cy) * CHUNK_PX) as usize;
                    for x in ax0..ax1 {
                        let k = row + (x - cx) as usize;
                        if jane_present::Foot::hides(l.height[k], y, i32::from(f.y)) {
                            any = true;
                            if !room {
                                break 'scan;
                            }
                            let c = if l.is_t8() { l.t8_abgr(k) } else { abgr(l.albedo[k]) };
                            self.patch_px[at + (y - y0) as usize * stride + (x - x0) as usize] = c | 0xff00_0000;
                        }
                    }
                }
            }
        }
        if !any || !room {
            if room {
                // Nothing hid its feet: the room it was given goes back.
                self.patch_px.truncate(at.min(self.patch_px.len()));
            } else if any {
                self.patches_skipped += 1;
            }
            return;
        }
        let i = self.patches.len() as u16;
        self.patches.push(Patch {
            at: at as u32,
            w: w as u16,
            h: h as u16,
            stride: stride as u16,
            tw: tw as u16,
            th: th as u16,
        });
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
    fn a_wide_read_back_is_sliced_where_px_and_texels_are_whole() {
        let base = Quad {
            tex: Tex::LightRt,
            mode: Mode::Multiply2Opaque,
            colour: 0,
            x0: -1,
            y0: -1,
            x1: 479,
            y1: 271,
            u0: 0,
            v0: 0,
            u1: 240,
            v1: 136,
        };
        let mut out = Vec::new();
        slices(&base, &mut out);
        assert_eq!(out.len(), 15);
        assert!(out.iter().all(|q| q.u1 - q.u0 <= SLICE_TEXELS as u16));
        assert!(out.windows(2).all(|w| w[0].x1 == w[1].x0 && w[0].u1 == w[1].u0));
        assert!(out.iter().all(|q| (q.x1 - q.x0) as i32 * 240 == (q.u1 - q.u0) as i32 * 480), "the same scale");
        assert_eq!((out[0].x0, out[14].x1, out[14].u1), (-1, 479, 240));
        // 1:1 (the grade), 2:1 down (the saturation's half size), and a narrow one left whole.
        let grade = Quad { tex: Tex::Frame(0, 4), x0: 0, x1: 480, u0: 0, u1: 480, ..base };
        slices(&grade, &mut out);
        assert_eq!(out.len(), 30);
        let half = Quad { tex: Tex::Frame(0, 0), x0: 0, x1: 240, u0: 0, u1: 480, ..base };
        slices(&half, &mut out);
        assert!(out.iter().all(|q| q.u1 - q.u0 == 16 && q.x1 - q.x0 == 8));
        let thin = Quad { x0: 0, x1: 32, u0: 0, u1: 16, ..base };
        slices(&thin, &mut out);
        assert_eq!(out, [thin]);
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

    fn quad_at(k: u16) -> Quad {
        Quad {
            tex: Tex::None,
            mode: Mode::Alpha,
            colour: 0,
            x0: k as i16,
            y0: 0,
            x1: 1,
            y1: 1,
            u0: 0,
            v0: 0,
            u1: 0,
            v1: 0,
        }
    }

    /// The program once ended on a buffer's growth to 14,506 quads (464,192 bytes, one block of a
    /// heap whose largest was 204 KB). A buffer grows by what the heap gives, to a most, and drops.
    #[test]
    fn a_quad_buffer_grows_to_its_most_and_leaves_out_the_rest() {
        let mut b = QuadBuf::with_capacity(4);
        for k in 0..QUADS_MOST as u16 + 5 {
            b.push(quad_at(k));
        }
        assert_eq!((b.len(), b.dropped, b.capacity()), (QUADS_MOST, 5, QUADS_MOST));
        assert_eq!(b[QUADS_MOST - 1], quad_at(QUADS_MOST as u16 - 1), "what is held is the first");
        // Appended: what fits goes, the rest is counted, the other is emptied.
        let mut a = QuadBuf::with_capacity(8);
        let mut other = QuadBuf::with_capacity(8);
        for k in 0..QUADS_MOST as u16 - 2 {
            a.push(quad_at(k));
        }
        for k in 0..5 {
            other.push(quad_at(k));
        }
        other.dropped = 7;
        a.append(&mut other);
        assert_eq!((a.len(), a.dropped, other.len(), other.dropped), (QUADS_MOST, 3 + 7, 0, 0));
        // It reads and compares as the slice it holds.
        let mut c = QuadBuf::default();
        c.push(quad_at(9));
        assert_eq!(c, [quad_at(9)]);
        assert_eq!((&c).into_iter().count(), 1);
        assert_eq!(c.take_dropped(), 0);
    }

    /// A frame's patches (PSP v0.4.1 plan, E): 500 sprites whose feet a wall hides, over three
    /// frames. They lay in one arena that never passes its words, nor its count, and never moves.
    #[test]
    fn patches_share_one_arena_that_holds_to_its_cap_and_does_not_move() {
        use jane_present::{ChunkLayers, Foot, Tier};
        let mut l = lister();
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        let mut layer = ChunkLayers::new(Tier::T0);
        // The chunk's top 120 rows are walls 40 px tall: they stand in front of feet at row 2.
        for k in 0..120 * CHUNK_PX as usize {
            layer.height[k] = 40;
            layer.albedo[k] = 0xff20_4060;
        }
        f.layers.push(layer);
        let foot = Foot::at(2, false);
        let sprite = |k: i16| cmd(Src { x: 0, y: 0, w: 10, h: 12 }, (k * 3) % 240, 10 + (k % 8) * 13, false);
        let mut capacity = None;
        for round in 0..3 {
            l.build(&f);
            assert!(l.patches.is_empty() && l.patch_px.is_empty(), "a frame begins with none");
            l.placed.push((0, 0, 0));
            for k in 0..500 {
                l.patch(&f, &sprite(k), foot);
            }
            assert_eq!(l.patches.len(), PATCHES_MOST, "round {round}");
            assert_eq!(l.patches_skipped as usize, 500 - PATCHES_MOST);
            assert!(l.patch_px.len() <= PATCH_WORDS);
            // Rows a multiple of 4 words, each patch 16-byte aligned, in order, none overlapping.
            let mut end = 0;
            for p in &l.patches {
                assert_eq!((p.at % 4, p.stride % 4), (0, 0));
                assert!(p.at as usize >= end, "{p:?} over the one before");
                assert!(p.stride >= p.w && p.tw >= p.w && p.th >= p.h);
                end = p.at as usize + usize::from(p.stride) * usize::from(p.h);
            }
            assert_eq!(end, l.patch_px.len(), "the arena holds the patches and nothing else");
            // The quads name them in order, one each; the first holds the wall's colour and a
            // clear px past its rect's right edge in the padding.
            let named: Vec<u16> =
                l.quads.iter().filter_map(|q| if let Tex::Patch(i) = q.tex { Some(i) } else { None }).collect();
            assert_eq!(named, (0..PATCHES_MOST as u16).collect::<Vec<_>>());
            let p = l.patches[0];
            assert_eq!(l.patch_px[p.at as usize] >> 24, 0xff);
            assert_eq!(l.patch_px[p.at as usize + usize::from(p.stride) - 1] >> 24, 0, "the padding is clear");
            let cap = l.patch_px.capacity();
            assert_eq!(*capacity.get_or_insert(cap), cap, "the arena did not move in round {round}");
            assert!((PATCH_FIRST..=PATCH_WORDS).contains(&cap));
        }
        assert_eq!(l.patch_bytes(), (1, l.patch_px.capacity() * 4));
        assert_eq!(l.left_out().2 as usize, 500 - PATCHES_MOST);
    }

    #[test]
    fn a_patch_over_nothing_takes_no_room_and_one_past_the_arena_is_left_out() {
        use jane_present::{ChunkLayers, Foot, Tier};
        let mut l = lister();
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        let mut layer = ChunkLayers::new(Tier::T0);
        // Walls only in the corner at rows 200..272, columns 0..256.
        for y in 200..CHUNK_PX as usize {
            for x in 0..CHUNK_PX as usize {
                layer.height[y * CHUNK_PX as usize + x] = 40;
            }
        }
        f.layers.push(layer);
        l.build(&f);
        l.placed.push((0, 0, 0));
        // Nothing hides its feet at the top of the chunk: no patch, no room taken, nothing counted.
        l.patch(&f, &cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 10, false), Foot::at(150, false));
        assert!((l.patches.len(), l.patch_px.len(), l.patches_skipped) == (0, 0, 0));
        // One a wall hides: laid.
        l.patch(&f, &cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 20, 205, false), Foot::at(205, false));
        assert_eq!((l.patches.len(), l.patch_px.len()), (1, 12 * 12));
        // A sprite as wide as the chunk and half as tall cannot fit the arena: counted, not laid.
        let wide = cmd(Src { x: 0, y: 0, w: 256, h: 140 }, 0, 130, false);
        l.patch(&f, &wide, Foot::at(205, false));
        assert_eq!((l.patches.len(), l.patches_skipped), (1, 1));
        assert!(l.patch_px.len() <= PATCH_WORDS);
    }

    /// A wood of stand-in blocks with a lantern among them once marked 8,100 polygons in a frame
    /// (the frame that ended the program held 14,506 quads).
    #[test]
    fn a_wood_of_blocks_lays_no_more_polygons_than_the_cap() {
        use jane_present::{Block, Directional, Span, Tier};
        let mut l = lister();
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        for k in 0..3000i16 {
            f.blocks.push(Block {
                x0: (k % 100) * 4,
                y0: 20 + (k / 100) * 8,
                x1: (k % 100) * 4 + 3,
                y1: 26 + (k / 100) * 8,
                height: 30,
                lo: 0,
                fence: false,
            });
        }
        let sun = Directional {
            azimuth: jane_core::Angle(9000),
            elevation: jane_core::Angle(4000),
            colour: [255, 240, 200],
            spread: 100,
            strength: 200,
        };
        f.passes.push(Pass::Silhouettes {
            sun,
            shade: [60, 70, 110],
            casters: Span { start: 0, len: 0 },
            blocks: Span { start: 0, len: 3000 },
        });
        let k = jane_present::shadow::shear(&sun).expect("a sun that throws");
        let castable = f.blocks.iter().filter(|b| block_poly(b, k, (480, 272)).is_some()).count();
        assert!(castable > POLYS_MOST, "{castable} blocks cast: enough to pass the cap");
        l.build(&f);
        let polys = l.quads.iter().filter(|q| matches!(q.tex, Tex::Poly(_))).count();
        assert_eq!((polys, l.polys.len()), (SUN_POLYS_MOST, SUN_POLYS_MOST));
        assert_eq!(l.polys_skipped as usize, castable - SUN_POLYS_MOST);
        // The lights' marks take what the cap leaves, and no more; the rest are counted.
        let square: Poly = ([(2, 2), (6, 2), (6, 6), (2, 6), (0, 0), (0, 0), (0, 0), (0, 0)], 4);
        for _ in 0..2000 {
            l.mark(square);
        }
        assert_eq!(l.polys.len(), POLYS_MOST);
        assert_eq!(l.lamp_marks.len(), POLYS_MOST - SUN_POLYS_MOST);
        assert_eq!(l.polys_skipped as usize, castable - SUN_POLYS_MOST + 2000 - (POLYS_MOST - SUN_POLYS_MOST));
        // The next frame begins with the room back.
        l.lamp_marks.clear();
        l.build(&f);
        assert_eq!((l.polys.len(), l.polys_skipped as usize), (SUN_POLYS_MOST, castable - SUN_POLYS_MOST));
    }

    /// Every texel of every page opaque: a caster's silhouette is its whole trimmed rect.
    struct Solid;

    impl PagePx for Solid {
        fn texel(&mut self, _: u16, _: u16, _: u16) -> u8 {
            2
        }
    }

    /// A wood's shadow bands, a dozen to a caster, were 5,079 of 5,423 quads at the densest place
    /// walked: a wood twice as dense must not take the buffer from the sprites and the light laid
    /// after them. The pass keeps its most, the strongest bands first.
    #[test]
    fn a_wood_of_casters_lays_no_more_shadow_quads_than_the_pass_may() {
        use jane_present::{Caster, Directional, Foot, Span, Tier};
        let mut l = lister();
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        let n = 6000u32;
        for k in 0..n {
            let (x, y) = (20 + (k * 37 % 420) as i16, 40 + (k * 53 % 220) as i16);
            f.sprites.push(SpriteCmd {
                foot: Some(Foot::at(y, false)),
                ..cmd(Src { x: 0, y: 0, w: 10, h: 12 }, x, y - 12, false)
            });
            f.casters.push(Caster {
                sprite: k,
                foot: (x + 5, y),
                height: 12,
                depth: 4,
                sink: 0,
                burn: (0, 0),
                base: 0,
            });
        }
        let sun = Directional {
            azimuth: jane_core::Angle(9000),
            elevation: jane_core::Angle(4000),
            colour: [255, 240, 200],
            spread: 100,
            strength: 200,
        };
        f.passes.push(Pass::Silhouettes {
            sun,
            shade: [60, 70, 110],
            casters: Span { start: 0, len: n },
            blocks: Span { start: 0, len: 0 },
        });
        l.build_with(&f, &mut Solid);
        let (a, b) = l
            .marks
            .first()
            .map(|m| (m.0 as usize, l.marks.get(1).map_or(l.quads.len(), |m| m.0 as usize)))
            .expect("a pass");
        assert_eq!(l.marks[0].1, P::SUN_SHADOWS);
        // Its clear is the frame's (nothing before it touched the stencil): counted, not laid.
        assert_eq!(l.stencil_from_clear, Some(a as u32));
        assert_eq!(b - a, SUN_QUADS_MOST - 1, "the pass holds its most");
        let skipped = l.left_out().0;
        assert!(skipped > 0, "{n} casters of a few bands each are more than {SUN_QUADS_MOST}");
        // Strongest first: the bands' tints never get darker down the pass, so what is left out
        // is the faintest.
        let tints: Vec<u32> = l.quads[a..b]
            .iter()
            .filter(|q| q.mode == Mode::ShadowBand && q.tex == Tex::None)
            .map(|q| q.colour & 0xff)
            .collect();
        assert!(tints.windows(2).all(|w| w[0] <= w[1]), "strongest (darkest) bands first");
        // The next frame starts afresh.
        l.build_with(&f, &mut Solid);
        assert_eq!(l.left_out().0, skipped);
    }

    /// A frame of T8 chunks (their height layers from `height(slot, px)`) placed at `at`, one
    /// caster in view so the sun's pass is laid.
    fn relief_frame(at: &[(i32, i32)], height: impl Fn(usize, usize) -> u8) -> Frame {
        use jane_present::{Caster, ChunkCmd, ChunkLayers, Directional, Foot, Span, Tier};
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        for (k, &(x, y)) in at.iter().enumerate() {
            let mut layers = ChunkLayers::new_t8();
            for (i, h) in layers.height.iter_mut().enumerate() {
                *h = height(k, i);
            }
            f.layers.push(layers);
            let id = jane_present::frame::ChunkId { cx: k as u16, cy: 0 };
            f.chunks.push(ChunkCmd { id, generation: 7 + k as u32, x, y, slot: k as u16 });
        }
        f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: at.len() as u32 } });
        f.sprites.push(SpriteCmd {
            foot: Some(Foot::at(150, false)),
            ..cmd(Src { x: 0, y: 0, w: 10, h: 12 }, 200, 138, false)
        });
        f.casters.push(Caster { sprite: 0, foot: (205, 150), height: 12, depth: 4, sink: 0, burn: (0, 0), base: 0 });
        let sun = Directional {
            azimuth: jane_core::Angle(9000),
            elevation: jane_core::Angle(4000),
            colour: [255, 240, 200],
            spread: 100,
            strength: 200,
        };
        f.passes.push(Pass::Silhouettes {
            sun,
            shade: [60, 70, 110],
            casters: Span { start: 0, len: 1 },
            blocks: Span { start: 0, len: 0 },
        });
        f
    }

    /// The stencil the sun's raised marks leave (PLAN-PSP-SPEED.md Q1): each canvas px set where
    /// a `RaisedMark` quad's texel (its slot's height, 1:1) is over the ground, as the GE lays
    /// them; and as the whole-chunk quads laid them before.
    fn raised_px(l: &Lister, f: &Frame) -> (Vec<bool>, Vec<bool>) {
        let (w, h) = (480usize, 272usize);
        let mut now = alloc::vec![false; w * h];
        for q in l.quads.iter().filter(|q| q.mode == Mode::RaisedMark) {
            let Tex::Height(slot) = q.tex else { panic!("a raised mark samples a height layer") };
            let (x0, y0, x1, y1) = (i32::from(q.x0), i32::from(q.y0), i32::from(q.x1), i32::from(q.y1));
            assert!(x0 >= 0 && y0 >= 0 && x1 <= w as i32 && y1 <= h as i32, "on the canvas: {q:?}");
            assert_eq!((i32::from(q.u1 - q.u0), i32::from(q.v1 - q.v0)), (x1 - x0, y1 - y0), "1:1 {q:?}");
            let hl = &f.layers[usize::from(slot)].height;
            for y in y0..y1 {
                for x in x0..x1 {
                    let (u, v) = (i32::from(q.u0) + x - x0, i32::from(q.v0) + y - y0);
                    if i32::from(hl[(v * CHUNK_PX + u) as usize]) > jane_present::shadow::GROUND {
                        now[y as usize * w + x as usize] = true;
                    }
                }
            }
        }
        let mut was = alloc::vec![false; w * h];
        for c in &f.chunks {
            let hl = &f.layers[usize::from(c.slot)].height;
            for v in 0..CHUNK_PX {
                for u in 0..CHUNK_PX {
                    let (x, y) = (c.x + u, c.y + v);
                    if (0..w as i32).contains(&x)
                        && (0..h as i32).contains(&y)
                        && i32::from(hl[(v * CHUNK_PX + u) as usize]) > jane_present::shadow::GROUND
                    {
                        was[y as usize * w + x as usize] = true;
                    }
                }
            }
        }
        (now, was)
    }

    #[test]
    fn the_raised_marks_cover_the_relief_alone_and_stencil_what_whole_chunks_did() {
        // A chunk hanging off each edge and corner, one inside: a house's roof in a few cells,
        // lone px at and round the ground's height (4 and 5 either side of it), a flat chunk.
        let at = [(-100, -60), (156, -60), (412, -60), (-100, 196), (156, 196), (412, 196), (60, 10)];
        let mut x = 0x2545_f491u32;
        let mut lone = alloc::vec::Vec::new();
        for _ in 0..16 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            lone.push(x % 65536);
        }
        let height = |k: usize, i: usize| -> u8 {
            let (px, py) = ((i % 256) as u32, (i / 256) as u32);
            if k == 6 {
                0
            } else if (k == 0 || k == 4) && (40..90).contains(&px) && (100..150).contains(&py) {
                40
            } else if lone.contains(&(i as u32 ^ k as u32)) {
                [4u8, 5, 6, 7, 8, 200][i % 6]
            } else {
                (i % 5) as u8
            }
        };
        let f = relief_frame(&at, height);
        let mut l = lister();
        l.build_with(&f, &mut Solid);
        let (now, was) = raised_px(&l, &f);
        assert!(was.iter().any(|&r| r), "some relief is on the canvas");
        assert!(now == was, "the clipped marks stencil every px the whole chunks did, and no other");
        let area: i32 = l
            .quads
            .iter()
            .filter(|q| q.mode == Mode::RaisedMark)
            .map(|q| i32::from(q.x1 - q.x0) * i32::from(q.y1 - q.y0))
            .sum();
        assert!(area < 480 * 272 / 2, "the marks rasterise a fraction of the canvas, not 7 chunks: {area}");
        // Nothing touched the stencil before the pass: the frame's clear stands for its own.
        let first = l.marks.iter().find(|m| m.1 == P::SUN_SHADOWS).map(|m| m.0);
        assert!(first.is_some());
        assert_eq!(l.stencil_from_clear, first);
        assert!(!l.quads.iter().any(|q| q.mode == Mode::StencilClear));
        assert!(l.sub_marks.iter().any(|m| m.1 == P::SUN_RAISED));
        assert!(l.sub_marks.iter().any(|m| m.1 == P::SUN_BANDS));
        // A second frame reads the kept rects (the same generations) to the same stencil.
        l.build_with(&f, &mut Solid);
        assert!(raised_px(&l, &f).0 == was);
    }

    /// A lake over three `T8` chunks (two hanging off the canvas): water px under a ragged
    /// diagonal shore that spills past the water cells (`ChunkLayers::water`, the frame's
    /// `water`), the land's cells wet, shining or matt; the sky; people standing on the shore,
    /// their feet at the water; rain when `wet`.
    fn lake_frame(wet: u8, tick: u32, camera: (i32, i32)) -> Frame {
        use jane_present::frame::{SkyLook, T8_SHINE, T8_WATER, T8_WET, WaterCmd};
        use jane_present::{Atmos, ChunkCmd, ChunkLayers, Depth, Span, Tier};
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        f.tick = tick;
        f.camera = camera;
        f.passes.push(Pass::Sky(SkyLook {
            zenith: [20, 30, 80],
            horizon: [200, 140, 100],
            glow: [255, 140, 60],
            glow_x: 100,
            glow_amount: 40,
            stars: 0,
            star_list: Span { start: 0, len: 0 },
            moon: None,
            zone: (0, 0, 480, 272),
            tick,
        }));
        f.passes.push(Pass::Weather(Atmos { rain: wet, wet, wind: 2, ..Atmos::default() }));
        let at = [(-37, -23), (219, -23), (-37, 233)];
        let mut x = 0x2545_f491u32;
        for (k, &(cx, cy)) in at.iter().enumerate() {
            let mut l = ChunkLayers::new_t8();
            l.clut[0] = 0xff20_6040;
            l.clut[1] = u32::from(T8_WATER) << 24 | 0x0080_4020;
            l.clut[2] = u32::from(T8_WET) << 24 | 0x0030_5050;
            l.clut[3] = u32::from(T8_SHINE) << 24 | 0x0060_6060;
            // The shore: px (u, v) wet where u + v + 2 v / 3 passes a line, give or take a few px.
            let line = [300, 260, 420][k];
            for i in 0..(CHUNK_PX * CHUNK_PX) as usize {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let (u, v) = ((i % 256) as i32, (i / 256) as i32);
                let water = u + v + 2 * v / 3 + (x % 23) as i32 > line + 11;
                let ix = if water { 1 } else { [0, 2, 3][(u / 16 + v / 16) as usize % 3] };
                l.t8_set(i, ix);
            }
            // The water cells: those wholly under the line (the shore's px spill past them).
            for j in 0..16 {
                for i in 0..16 {
                    let (u, v) = (i * 16, j * 16);
                    if u + v + 2 * v / 3 > line + 11 {
                        l.water.push((i as u8, j as u8, 0));
                        f.water.push(WaterCmd { x: (cx + u) as i16, y: (cy + v) as i16, phase: 0 });
                    }
                }
            }
            f.layers.push(l);
            let id = jane_present::frame::ChunkId { cx: k as u16, cy: 0 };
            f.chunks.push(ChunkCmd { id, generation: 3 + k as u32, x: cx, y: cy, slot: k as u16 });
        }
        f.passes.push(Pass::Terrain { chunks: Span { start: 0, len: at.len() as u32 } });
        // Standing at the shore, tall and short: their mirrors in bands, rippled.
        for (k, &(sx, sy)) in [(150, 120), (200, 140), (330, 60), (90, 200), (420, 30), (240, 230)].iter().enumerate() {
            let src = if k % 2 == 0 { Src { x: 0, y: 0, w: 10, h: 12 } } else { Src { x: 10, y: 0, w: 8, h: 8 } };
            f.sprites.push(cmd(src, sx, sy, k % 3 == 1));
        }
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 6 } });
        f
    }

    /// What the water's pass leaves (PLAN-PSP-SPEED.md W1, W3), as the GE lays it: each canvas
    /// px's stencil and a hash of what was laid on it in turn (each draw's mode, colour and the
    /// texel it read; a strip's segment), from a stencil of noise (whatever the passes before
    /// left in the alpha).
    fn water_px(l: &Lister, f: &Frame) -> (Vec<u8>, Vec<u64>) {
        let (w, h) = (480i32, 272i32);
        let mut st: Vec<u8> = (0..w * h).map(|i| (i as u32).wrapping_mul(0x9e37_79b1).rotate_left(7) as u8).collect();
        let mut px = alloc::vec![0u64; (w * h) as usize];
        let a = l.marks.iter().position(|m| m.1 == P::WATER_FX).expect("the water's pass");
        let (q0, q1) = (l.marks[a].0 as usize, l.marks.get(a + 1).map_or(l.quads.len(), |m| m.0 as usize));
        let mix = |h: u64, k: u64| (h ^ k).wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(29);
        let passes = |s: u8, at: Where| match at {
            Where::Is(v) => s == v,
            Where::Bits(v, m) => s & m == v & m,
            Where::Over(v) => v < s,
        };
        // Texel `k` of `n` across a quad from `t0` to `t1` (backward when `t0 > t1`), sampled
        // at the px's centre.
        let texel = |t0: u16, t1: u16, k: i32, n: i32| {
            i32::from(t0) + ((2 * k + 1) * (i32::from(t1) - i32::from(t0))).div_euclid(2 * n)
        };
        for q in &l.quads[q0..q1] {
            if let Tex::Strip(i) = q.tex {
                let Mode::Masked(_, at) = q.mode else { panic!("a water strip is masked") };
                let s = l.strips[usize::from(i)];
                let v = &l.verts[s.start as usize..(s.start + u32::from(s.len)) as usize];
                for seg in v.windows(4).step_by(2) {
                    let key = seg.iter().fold(7u64, |h, v| {
                        mix(h, (v.x as u16 as u64) << 48 | (v.y as u16 as u64) << 32 | u64::from(v.colour))
                    });
                    for y in i32::from(seg[0].y).max(0)..i32::from(seg[1].y).min(h) {
                        for x in i32::from(seg[0].x).max(0)..i32::from(seg[2].x).min(w) {
                            let i = (y * w + x) as usize;
                            if passes(st[i], at) {
                                px[i] = mix(px[i], key);
                            }
                        }
                    }
                }
                continue;
            }
            let (x0, y0, x1, y1) = (i32::from(q.x0), i32::from(q.y0), i32::from(q.x1), i32::from(q.y1));
            let what = alloc::format!("{:?}", (q.tex, q.mode)).bytes().fold(0u64, |h, b| mix(h, u64::from(b)));
            for y in y0.max(0)..y1.min(h) {
                for x in x0.max(0)..x1.min(w) {
                    let i = (y * w + x) as usize;
                    let (u, v) = (texel(q.u0, q.u1, x - x0, x1 - x0), texel(q.v0, q.v1, y - y0, y1 - y0));
                    let alpha = match q.tex {
                        Tex::Chunk(slot) => {
                            (f.layers[usize::from(slot)].t8_abgr((v * CHUNK_PX + u) as usize) >> 24) as u8
                        }
                        Tex::Noise => (u ^ v.rotate_left(3)) as u8 & 0x80,
                        _ => 0xff,
                    };
                    match q.mode {
                        Mode::StencilClear => st[i] = 0,
                        Mode::Mark { tag, value, keep, need } => {
                            let texel_in = if tag == 0 { alpha != 0 } else { alpha == tag };
                            if texel_in && need.is_none_or(|n| st[i] & n != value & n) {
                                st[i] = st[i] & keep | value & !keep;
                            }
                        }
                        Mode::Masked(_, at) => {
                            if passes(st[i], at) {
                                let key = (u as u16 as u64) << 48 | (v as u16 as u64) << 32 | u64::from(q.colour);
                                px[i] = mix(mix(px[i], key), what);
                            }
                        }
                        m => panic!("the water's pass lays no {m:?}"),
                    }
                }
            }
        }
        (st, px)
    }

    #[test]
    fn the_tight_water_lays_every_px_the_box_did_and_fewer_quads() {
        use jane_present::frame::T8_WATER;
        let (mut quads, mut area) = ([0usize; 2], [0i64; 2]);
        for (wet, tick, cam) in [(0, 0, (0, 0)), (0, 377, (5, 11)), (180, 91, (-9, 3)), (255, 1234, (13, -6))] {
            let f = lake_frame(wet, tick, cam);
            let mut lay = |loose: bool| {
                let mut l = lister();
                l.water_loose = loose;
                l.build(&f);
                let a = l.marks.iter().position(|m| m.1 == P::WATER_FX).expect("the water's pass");
                let (q0, q1) = (l.marks[a].0 as usize, l.marks.get(a + 1).map_or(l.quads.len(), |m| m.0 as usize));
                let k = usize::from(!loose);
                quads[k] += q1 - q0;
                // The fill of what reads the water's mark: its mark, its sky and its crests.
                area[k] += l.quads[q0..q1]
                    .iter()
                    .filter(|q| matches!(q.mode, Mode::Mark { tag: T8_WATER, .. } | Mode::Masked(_, Where::Is(3))))
                    .map(|q| match q.tex {
                        // A strip's own rect, not its quad's (the canvas).
                        Tex::Strip(i) => {
                            let s = l.strips[usize::from(i)];
                            let v = &l.verts[s.start as usize..(s.start + u32::from(s.len)) as usize];
                            i64::from(v[v.len() - 1].x - v[0].x) * i64::from(v[1].y - v[0].y)
                        }
                        _ => i64::from(q.x1 - q.x0) * i64::from(q.y1 - q.y0),
                    })
                    .sum::<i64>();
                (water_px(&l, &f), l)
            };
            let ((was_st, was_px), _) = lay(true);
            let ((st, px), l) = lay(false);
            assert!(was_px.iter().any(|&h| h != 0), "the water lays something");
            assert!(st == was_st, "wet {wet} tick {tick}: the stencil as the box left it");
            let diff = px.iter().zip(&was_px).filter(|(a, b)| a != b).count();
            assert_eq!(diff, 0, "wet {wet} tick {tick}: every px laid as the box laid it");
            // A second frame reads the kept rects to the same.
            let mut l = l;
            l.build(&f);
            assert!(water_px(&l, &f).1 == was_px);
        }
        assert!(quads[1] < quads[0], "fewer quads: {quads:?}");
        // (A lake over most of the canvas: its box is the canvas, its cells about four fifths.)
        assert!(area[1] * 5 < area[0] * 4, "the water's mark, sky and crests fill less: {area:?}");
    }

    #[test]
    fn a_mirror_joins_bands_shifted_alike() {
        let f = lake_frame(0, 377, (5, 11));
        let count = |loose: bool| {
            let mut l = lister();
            l.water_loose = loose;
            l.build(&f);
            l.quads
                .iter()
                .filter(|q| matches!(q.tex, Tex::Page(_)) && q.mode == Mode::Masked(Blend::Alpha, Where::Over(2)))
                .count()
        };
        let (was, now) = (count(true), count(false));
        assert!(now < was && now > 0, "{now} quads for {was} bands");
    }

    #[test]
    fn relief_past_the_rects_most_is_one_span_a_row() {
        // A chequer of raised cells: 128 runs, laid as 16 spans.
        let mut hl = alloc::vec![0u8; (CHUNK_PX * CHUNK_PX) as usize];
        for (i, h) in hl.iter_mut().enumerate() {
            let (cx, cy) = (i % 256 / 16, i / 256 / 16);
            if (cx + cy) % 2 == 0 && i % 16 == 3 {
                *h = 9;
            }
        }
        let mut out = alloc::vec::Vec::new();
        relief_cells(&hl, &mut out);
        assert_eq!(out.len(), 16);
        for (k, r) in out.iter().enumerate() {
            let x0 = (k % 2) as u8;
            assert_eq!(*r, (x0, k as u8, x0 + 15, k as u8 + 1));
        }
        // Flat: nothing. Raised everywhere: one rect.
        relief_cells(&alloc::vec![4u8; hl.len()], &mut out);
        assert!(out.is_empty());
        relief_cells(&alloc::vec![5u8; hl.len()], &mut out);
        assert_eq!(out, [(0, 0, 16, 16)]);
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

    /// The night's turn on C2 (NIGHT.md §4.7): the band as a few strips over the lightmap or the
    /// flat light, every row covered once, the top lit and the bottom held dark mid-band; no
    /// band, one strip at all of the light.
    #[test]
    fn the_turns_band_is_a_few_strips_down_the_screen() {
        let strips = |b: &jane_present::Band, rows: i32, unit: i32| {
            let mut v = Vec::new();
            band_strips(*b, rows, unit, |y0, y1, k| v.push((y0, y1, k)));
            v
        };
        assert_eq!(strips(&jane_present::Band::NONE, 136, 2), vec![(0, 136, 256)]);
        let b = jane_present::Band { edge: 140, soft: 48, dark: 51, down: true, crest: 0 };
        for (rows, unit) in [(138, 2), (272, 1)] {
            let v = strips(&b, rows, unit);
            assert_eq!((v[0].0, v[v.len() - 1].1), (0, rows));
            assert!(v.windows(2).all(|w| w[0].1 == w[1].0 && w[1].2 <= w[0].2), "{v:?}");
            assert!(v[0].2 == 256 && v[v.len() - 1].2 == 48, "{v:?}");
            assert!(v.len() <= 10, "a few strips: {}", v.len());
        }
        assert_eq!(sub_rgb(0xff40_8020, 0xff10_9010), 0xff30_0010);
    }

    /// A street of sprites over two pages (Q6a): grouped by page, every pair of quads whose px
    /// touch is drawn in the order it was, the quads are the same, and the batches fall.
    #[test]
    fn sprites_grouped_by_page_keep_every_pair_that_touches() {
        use jane_present::{Depth, Span};
        let mut f = Frame::new(jane_present::Tier::T0);
        f.canvas = (480, 272);
        for k in 0..80i16 {
            // Refs 0 (page 4) and 1 (page 5) by turns, down the screen by depth, some over others.
            let src = if k % 3 == 0 { Src { x: 10, y: 0, w: 8, h: 8 } } else { Src { x: 0, y: 0, w: 10, h: 12 } };
            f.sprites.push(cmd(src, (k * 37) % 460, k * 3, k % 5 == 0));
        }
        f.passes.push(Pass::Sprites { layer: Depth::Standing, cmds: Span { start: 0, len: 80 } });
        let mut l = lister();
        l.order_passes = 0;
        let a = l.build(&f).to_vec();
        l.order_passes = ORDERED;
        let b = l.build(&f).to_vec();
        let mut sa = a.clone();
        let mut sb = b.clone();
        let key = |q: &Quad| (q.x0, q.y0, q.x1, q.y1, q.u0, q.v0);
        sa.sort_by_key(key);
        sb.sort_by_key(key);
        assert_eq!(sa, sb, "the same quads");
        let pos = |q: &Quad| b.iter().position(|p| p == q).expect("kept");
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                let touch = match (order::reach(&a[i]), order::reach(&a[j])) {
                    (Some(x), Some(y)) => x.0 < y.2 && y.0 < x.2 && x.1 < y.3 && y.1 < x.3,
                    _ => true,
                };
                if touch {
                    assert!(pos(&a[i]) < pos(&a[j]), "quads {i} and {j} swapped");
                }
            }
        }
        let batches = |q: &[Quad]| 1 + q.windows(2).filter(|w| !guard::same_batch(&w[0], &w[1])).count();
        assert!(batches(&b) * 2 < batches(&a), "{} -> {}", batches(&a), batches(&b));
    }

    /// A block's swept hull laid from its edges is the sorted hull, corner for corner, along every
    /// sun, near the canvas and far past it (clamped alike).
    #[test]
    fn a_swept_blocks_hull_from_its_edges_is_the_sorted_hull() {
        let mut s = 0x9e37_79b9u32;
        let mut rnd = |n: u32| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s % n
        };
        let mut checked = 0;
        for k in 0..200_000 {
            let x0 = rnd(1200) as i32 - 600;
            let y0 = rnd(1200) as i32 - 600;
            let (bw, bh) = (1 + rnd(if k % 7 == 0 { 3 } else { 90 }) as i32, 1 + rnd(60) as i32);
            let b = jane_present::Block {
                x0: x0 as i16,
                y0: y0 as i16,
                x1: (x0 + bw) as i16,
                y1: (y0 + bh) as i16,
                height: rnd(256) as u8,
                lo: rnd(256) as u8,
                fence: rnd(4) == 0,
            };
            // Suns along the axes and between them, short and long; a camera near and far.
            let pick = |r: u32, v: u32| match r % 4 {
                0 => 0,
                1 => v as i32 - 300,
                2 => -(v as i32 % 40),
                _ => v as i32 % 40,
            };
            let (r0, v0, r1, v1) = (rnd(4), rnd(600), rnd(4), rnd(600));
            let kk = (pick(r0, v0), pick(r1, v1));
            let o = if k % 3 == 0 { (rnd(30_000) as i32 - 15_000, rnd(30_000) as i32 - 15_000) } else { (0, 0) };
            let Some(pts) = block_sweep(&b, kk, o) else { continue };
            assert_eq!(sweep_hull(&pts), hull(pts), "{b:?} k {kk:?} o {o:?}");
            checked += 1;
        }
        assert!(checked > 100_000);
    }

    /// L4: a patch scans only the chunk's cells with relief, and lays the px the whole scan laid.
    #[test]
    fn a_patch_from_the_relief_cells_alone_is_the_whole_scans_patch() {
        use jane_present::{ChunkLayers, Foot, Tier};
        let mut f = Frame::new(Tier::T0);
        f.canvas = (480, 272);
        let mut layer = ChunkLayers::new(Tier::T0);
        let mut s = 0x2545_f491u32;
        for k in 0..(CHUNK_PX * CHUNK_PX) as usize {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let (x, y) = (k % CHUNK_PX as usize, k / CHUNK_PX as usize);
            // A wall over a few cells, a kerb (under the relief) over others, flat the rest.
            layer.height[k] = if (x / 16 + y / 16) % 5 == 0 {
                (s % 60) as u8
            } else if (x / 16) % 3 == 0 {
                (s % 5) as u8
            } else {
                0
            };
            layer.albedo[k] = 0xff00_0000 | s >> 8;
        }
        f.layers.push(layer);
        let feet = [Foot::at(40, false), Foot::at(120, false), Foot::at(250, false)];
        let mut l = lister();
        l.build(&f);
        l.placed.push((-10, 7, 0));
        for k in 0..60i16 {
            let s = cmd(Src { x: 0, y: 0, w: 10, h: 12 }, (k * 41) % 470 - 5, (k * 29) % 260, false);
            l.patch(&f, &SpriteCmd { src: Src { x: 0, y: 0, w: 24, h: 40 }, ..s }, feet[k as usize % 3]);
        }
        // The whole scan, as it was: every px of each patch's rect in the chunk.
        let mut patches = 0;
        for q in &l.quads {
            let Tex::Patch(i) = q.tex else { continue };
            let p = l.patches[usize::from(i)];
            let k = (0..60i16).find(|&k| {
                let (x, y) = ((k * 41) % 470 - 5, (k * 29) % 260);
                (i32::from(x).max(0), i32::from(y).max(0)) == (i32::from(q.x0), i32::from(q.y0))
            });
            let foot = feet[k.expect("its sprite") as usize % 3];
            for y in 0..i32::from(p.h) {
                for x in 0..i32::from(p.w) {
                    let (cx, cy) = (i32::from(q.x0) + x + 10, i32::from(q.y0) + y - 7);
                    let mut want = 0;
                    if (0..CHUNK_PX).contains(&cx) && (0..CHUNK_PX).contains(&cy) {
                        let k = (cy * CHUNK_PX + cx) as usize;
                        if Foot::hides(f.layers[0].height[k], i32::from(q.y0) + y, i32::from(foot.y)) {
                            want = abgr(f.layers[0].albedo[k]) | 0xff00_0000;
                        }
                    }
                    let got = l.patch_px[p.at as usize + y as usize * usize::from(p.stride) + x as usize];
                    assert_eq!(got, want, "patch {i} px ({x}, {y})");
                }
            }
            patches += 1;
        }
        assert!(patches > 10, "{patches} patches");
    }
}
