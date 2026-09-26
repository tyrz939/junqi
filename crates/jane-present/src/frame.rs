//! The `Frame` (PRESENTATION.md §1.1): what one frame draws, in canvas px, naming no texture,
//! shader or pixel format. Filled into reused buffers; no allocation after the second frame.
//!
//! Layout: the passes are a list in draw order, and each names a [`Span`] of one of the frame's
//! flat command lists (`chunks`, `sprites`) instead of holding a slice of its own, so every list
//! is one `Vec` reserved once and cleared each frame. A terrain command names its chunk's layers
//! by slot in [`Frame::layers`], the presenter's chunk cache, which the frame carries.

use std::ops::Range;

/// The canvas height in px, always (PRESENTATION.md, the canvas): 27 cells of 16.
pub const CANVAS_H: u16 = 432;
/// The canvas width at 16:9; a wider window widens the canvas at the same height.
pub const CANVAS_W: u16 = 768;
/// Canvas px per sim cell: the render scale is 2 over `jane_core::view::CELL_PX`.
pub const CELL: i32 = 16;
/// A sim position (`Fx`, 1/256 sim px) becomes canvas px by this shift (256 / 2 = 128).
pub const FX_TO_CANVAS: u32 = 7;
/// Cells on a side of a terrain chunk (PRESENTATION.md §1.6).
pub const CHUNK_CELLS: i32 = 16;
/// Canvas px on a side of a terrain chunk.
pub const CHUNK_PX: i32 = CHUNK_CELLS * CELL;

/// The render tier a backend draws at (PRESENTATION.md §1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// `soft`: the CPU framebuffer.
    T0,
    /// `gl2`: OpenGL 2.1 / GLES 2.
    T1,
    /// `wgpu`: Vulkan 1.1 class.
    T2,
}

/// A run of one of the frame's command lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub len: u32,
}

impl Span {
    /// The span from `start` to the list's current end.
    pub fn since(start: usize, end: usize) -> Span {
        Span { start: start as u32, len: (end - start) as u32 }
    }

    pub fn range(self) -> Range<usize> {
        self.start as usize..(self.start + self.len) as usize
    }
}

/// The depth layers of PRESENTATION.md §1.9, back to front.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Depth {
    Sky,
    FarLandmark,
    FarTreeline,
    Ground,
    Standing,
    Canopy,
    NearFog,
    Weather,
    Ui,
}

/// One pass, in draw order. Each is a `Features` row's worth of drawing (§1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Terrain chunks, `Frame::chunks[chunks]`.
    Terrain { chunks: Span },
    /// Sprites of one depth, already in draw order (y-sorted for `Standing`): `Frame::sprites[cmds]`.
    Sprites { layer: Depth, cmds: Span },
    /// The light pass. So far the ambient alone (`0xRRGGBB` per channel, 255 is full); the
    /// point lights and casters land with the lightmap (PORT.md §7.1 step 5). Present only when
    /// the ambient is below full: at 254 and up the multiply is a no-op and the pass is left out.
    Lights { ambient: [u8; 3] },
}

impl Pass {
    /// The lowest tier that draws this pass as it stands (its `Features` row, §1.3).
    pub fn needs(&self) -> Tier {
        match self {
            Pass::Terrain { .. } | Pass::Sprites { .. } | Pass::Lights { .. } => Tier::T0,
        }
    }
}

/// How a sprite is coloured as it is blitted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tint {
    #[default]
    None,
    /// Toward white by `a` of 255 (the hurt flash).
    Flash(u8),
    /// Over what is under it at `a` of 255 (a ghost, the canopy ghost, the dead).
    Ghost(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// Walk the source columns backwards.
    pub mirror: bool,
    pub tint: Tint,
}

/// A rect of an atlas page, in texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Src {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// One sprite: a rect of a page to the canvas at `(x, y)` (its top-left, canvas px).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteCmd {
    pub page: u8,
    pub src: Src,
    pub x: i16,
    pub y: i16,
    pub flags: Flags,
    /// How tall the thing stands, screen px: shadow length and water on T1 and T2.
    pub height_px: u8,
}

/// A chunk's place in its zone, in chunks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    pub cx: u16,
    pub cy: u16,
}

/// One terrain chunk to the canvas at `(x, y)` (its top-left, canvas px).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkCmd {
    pub id: ChunkId,
    /// Bumped each time the chunk is painted: a backend caches by `(id, generation)`.
    pub generation: u32,
    pub x: i32,
    pub y: i32,
    /// Its layers: `Frame::layers[slot]`.
    pub slot: u16,
}

/// A painted chunk, `CHUNK_PX` square. `soft` reads the albedo alone (§1.6); the normal,
/// emissive and height layers land with the chunk painter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkLayers {
    /// `0xAARRGGBB`, resolved through the CLUT as it was painted (a chunk is never tinted).
    pub albedo: Vec<u32>,
}

/// One frame's draw, in pass order.
#[derive(Debug)]
pub struct Frame {
    /// The tier this frame was built for; no pass above it is present.
    pub tier: Tier,
    /// Canvas size in px: `CANVAS_H` tall, `CANVAS_W` or wider.
    pub canvas: (u16, u16),
    /// Top-left of the view in the zone, canvas px, interpolated. Every command below is
    /// already in canvas coordinates (the camera taken off); this is for parallax.
    pub camera: (i32, i32),
    /// Filled before anything else, as `0xAARRGGBB`.
    pub clear: u32,
    /// In draw order.
    pub passes: Vec<Pass>,
    pub chunks: Vec<ChunkCmd>,
    pub sprites: Vec<SpriteCmd>,
    /// The presenter's painted chunks by slot (what `ChunkCmd::slot` names).
    pub layers: Vec<ChunkLayers>,
}

impl Frame {
    /// An empty frame at `tier`, 16:9.
    pub fn new(tier: Tier) -> Frame {
        Frame {
            tier,
            canvas: (CANVAS_W, CANVAS_H),
            camera: (0, 0),
            clear: 0xff10_1014,
            passes: Vec::with_capacity(16),
            chunks: Vec::with_capacity(64),
            sprites: Vec::with_capacity(4096),
            layers: Vec::new(),
        }
    }

    pub fn chunks_in(&self, s: Span) -> &[ChunkCmd] {
        &self.chunks[s.range()]
    }

    pub fn sprites_in(&self, s: Span) -> &[SpriteCmd] {
        &self.sprites[s.range()]
    }

    /// The layers a chunk command draws.
    pub fn layers_of(&self, c: &ChunkCmd) -> &ChunkLayers {
        &self.layers[usize::from(c.slot)]
    }
}
