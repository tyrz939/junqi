//! The `Frame` as the GE's quads (PORT.md §13.12): every pass C2 draws, in the frame's order,
//! each sprite resolved from the presenter's page and rect to the PSP pack's page and trimmed
//! rect, clipped to the canvas, bent rows split into strips. Pure and integer: what `ge.rs`
//! sends is exactly this list, so a PC test reads what the PSP will draw.

use alloc::vec::Vec;

use crate::capture::pass as P;
use sprites::{CellRect, water_cells};
use sun::KeptRows;

mod atmos;
mod build;
pub mod guard;
mod lightmap;
pub mod order;
mod shafts;
mod sprites;
mod sun;
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

/// Lamp CLUTs a frame's relief takes at most (64 bytes each).
pub const LAMP_RELIEFS: usize = 96;
/// The stencil's value over raised terrain while the sun's silhouettes are laid
/// ([`Mode::RaisedMark`]); the shade is 1.
pub const RAISED: u8 = 2;

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
    /// `rows_kept`'s hashes, in its order: the lookup scans these 512 B, not the entries (a
    /// scan of the entries was most of a caster's cost on the PSP, its cache missed at each).
    rows_hash: Vec<u32>,
    rows_clock: u32,
    /// Casters' opacity by ref, `(ref, 2 bits a px, last used)` ([`Lister::opacity`]).
    masks: Vec<(usize, Vec<u8>, u32)>,
    mask_clock: u32,
    /// Shadow runs laid this frame (a stat).
    pub shadow_runs: u32,
    /// A clock (microseconds) the lister times its parts by, if the platform gives one, and
    /// what it measured: lightmap pools, lamp shadows, lightmap finish, casters' slabs, blocks'
    /// slabs (summed until read); `[10]` the draws grouped by texture ([`order`]), and `[11]`
    /// counts the blocks that threw a shadow from a light cast each frame; `[12]` the water and
    /// wet ground (`water_fx`, holding `[0]` and `[7]`).
    pub clock: Option<fn() -> u32>,
    pub prof: [u32; 13],
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

#[cfg(test)]
mod tests;
