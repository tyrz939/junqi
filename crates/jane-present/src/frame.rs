//! The `Frame` (PRESENTATION.md §1.1): what one frame draws, in canvas px, naming no texture,
//! shader or pixel format. Filled into reused buffers; no allocation after the second frame.
//!
//! Layout: the passes are a list in draw order, and each names a [`Span`] of one of the frame's
//! flat command lists (`chunks`, `sprites`) instead of holding a slice of its own, so every list
//! is one `Vec` reserved once and cleared each frame. A terrain command names its chunk's layers
//! by slot in [`Frame::layers`], the presenter's chunk cache, which the frame carries.

use std::ops::Range;

use jane_core::Angle;

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

/// The 3/4 view's one projection (ART.md §1.1, PRESENTATION.md §1.7): heights are true px, a
/// thing `h` px up is drawn `rows_up(h)` rows over its ground point (four fifths, rounded up).
/// Every tier's shadow reads heights through this and nothing else.
pub use jane_art::canvas::{height_of_rows, rows_up};

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

/// A colour, a channel a byte; 255 is full.
pub type Rgb = [u8; 3];

/// One pass, in draw order. Each is a `Features` row's worth of drawing (§1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Terrain chunks, `Frame::chunks[chunks]`.
    Terrain { chunks: Span },
    /// Sprites of one depth, already in draw order (y-sorted for `Standing`): `Frame::sprites[cmds]`.
    Sprites { layer: Depth, cmds: Span },
    /// Silhouette sun shadows (§1.3 `silhouettes`, T0 and T1): each caster's albedo mask sheared
    /// along `sun` by its height and laid on the ground under the `Standing` pass, each shadowed
    /// pixel multiplied by `shade` (the ambient's colour, never grey), its edge one dither step
    /// soft. `Frame::casters[casters]`, in draw order.
    Silhouettes { sun: Directional, shade: Rgb, casters: Span },
    /// The light pass (§1.7). What is lit was decided by the view; each tier draws it its own way.
    /// `ambient` is the flat light T0 multiplies by (255 is full; on T0 the pass is left out when
    /// it would change nothing). `fill` is the light a surface gets from the sky alone, what a
    /// shadow is lit by on T1 and T2 (blue by day, violet at dusk, the zone's own indoors); `sun`
    /// is the sun or the moon, added on top where it is not shadowed; `points` are
    /// `Frame::lights[points]`, `casters` are `Frame::casters[casters]`.
    Lights { ambient: Rgb, fill: Rgb, sun: Option<Directional>, points: Span, casters: Span },
    /// The grade and the bloom (§1.9), last before the UI: all of it at T2; at T1 its tint and
    /// lift alone (the other fields [`Post::NONE`]'s), which is what T1's `grade` row draws.
    Post(Post),
}

impl Pass {
    /// The lowest tier that draws this pass as it stands (its `Features` row, §1.3).
    pub fn needs(&self) -> Tier {
        match self {
            Pass::Terrain { .. } | Pass::Sprites { .. } | Pass::Lights { .. } | Pass::Silhouettes { .. } => Tier::T0,
            Pass::Post(p) if p.saturation == 128 && p.bloom == 0 && p.exposure == 128 => Tier::T1,
            Pass::Post(_) => Tier::T2,
        }
    }
}

/// The sun or the moon: one directional light whose angle follows the clock (§1.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directional {
    /// Where the light is, seen across the ground from a lit thing: 0 east, a quarter turn
    /// south (jane-core's [`Angle`], clockwise with y down). Shadows point the other way.
    pub azimuth: Angle,
    /// Above the horizon: 0 grazes the ground, a quarter turn (16384) is overhead.
    pub elevation: Angle,
    /// Its colour on a surface square to it.
    pub colour: Rgb,
    /// How soft its shadows are: the light's angular radius (jane-core `Angle` units). A
    /// penumbra widens by this much for every px it lies from what casts it.
    pub spread: u16,
}

/// A point light's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    Point,
    /// A cone along `dir` (across the ground), `cone` its half-angle: a sentry's eye.
    Spot {
        dir: Angle,
        cone: Angle,
    },
}

/// A light (§1.7), from the view: a prop the sim says is lit, a unit's glow, her lantern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Light {
    /// Where it stands on the ground, canvas px (the camera taken off). It shines from
    /// `height` px above that, so it is seen at `(pos.0, pos.1 - rows_up(height))`.
    pub pos: (i32, i32),
    /// Its height above the ground, true px (a lamp's glass, a lantern at her hip).
    pub height: u8,
    /// Its colour at the centre, the flicker applied.
    pub colour: Rgb,
    /// Where it reaches nothing, canvas px.
    pub radius: u16,
    /// The size of the glowing thing, px: a penumbra widens with it.
    pub size: u8,
    /// Throws shadows from the casters (T1: the nearest 8; T2: the nearest 32).
    pub casts: bool,
    pub kind: LightKind,
    /// What carries it, `Frame::sprites[holder]`: a lamp's post, a torch's bracket, the one
    /// holding a lantern. A light never shadows what holds it (PRESENTATION.md §1.7).
    pub holder: Option<u32>,
}

/// A thing that throws a shadow: a unit or a prop standing (§1.7 occluders). Wall runs and
/// canopy cast from the terrain's height layer on T2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caster {
    /// The sprite that draws it: `Frame::sprites[sprite]`. Its albedo is the silhouette.
    pub sprite: u32,
    /// Its ground point, canvas px: a unit's feet, the middle of a prop's front edge.
    pub foot: (i16, i16),
    /// How tall it stands, px.
    pub height: u8,
    /// How deep it is across the ground, px: a person is thin, a crate is its footprint.
    pub depth: u8,
}

/// The grade and the bloom (§1.9): a row per region by hour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Post {
    /// Multiplies the lit frame (255 is none).
    pub tint: Rgb,
    /// Added to the darkest tones: the colour the shadows lean to.
    pub lift: Rgb,
    /// 128 is as lit; 0 is grey, 255 twice as rich.
    pub saturation: u8,
    /// How much the emissive and the brightest light bloom, 0..255.
    pub bloom: u8,
    /// Exposure before the tone curve, 128 is 1.
    pub exposure: u8,
}

impl Post {
    /// No grade at all: the tint and the lift a T1 frame carries are laid over this.
    pub const NONE: Post = Post { tint: [255; 3], lift: [0; 3], saturation: 128, bloom: 0, exposure: 128 };
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

/// A painted chunk, `CHUNK_PX` square, four layers (§1.6). `soft` reads the albedo alone, and a
/// T0 presenter leaves the other three empty; T1 and T2 get all four.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkLayers {
    /// `0xAARRGGBB`, resolved through the CLUT as it was painted (a chunk is never tinted).
    pub albedo: Vec<u32>,
    /// Tangent-space `[nx, ny]`, 128 is 0 (`jane_art::canvas::Normal`): `+x` east, `+y` south.
    pub normal: Vec<[u8; 2]>,
    /// `0xAARRGGBB` resolved like the albedo; 0 where nothing glows.
    pub emissive: Vec<u32>,
    /// Px above the ground (a wall's face rises to its height, a roof is its height).
    pub height: Vec<u8>,
}

impl ChunkLayers {
    /// A chunk's layers: the albedo alone at T0, all four above it.
    pub fn new(tier: Tier) -> ChunkLayers {
        let n = (CHUNK_PX * CHUNK_PX) as usize;
        if tier == Tier::T0 {
            ChunkLayers { albedo: vec![0; n], ..ChunkLayers::default() }
        } else {
            ChunkLayers { albedo: vec![0; n], normal: vec![[128, 128]; n], emissive: vec![0; n], height: vec![0; n] }
        }
    }

    /// Whether the normal, emissive and height layers are carried.
    pub fn lit(&self) -> bool {
        !self.height.is_empty()
    }
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
    /// Lights in view and in reach of it (`Pass::Lights::points`).
    pub lights: Vec<Light>,
    /// Things that throw shadows (`Pass::Lights::casters`, `Pass::Silhouettes::casters`).
    pub casters: Vec<Caster>,
    /// The `Ui` pass (PRESENTATION.md §3.1): drawn after every pass above, unlit, in order.
    /// Filled by `ui::Ui::finish`; the contract is `ui::cmd`'s module doc.
    pub ui: Vec<crate::ui::UiCmd>,
    /// The UI's run-time pictures by slot (`UiCmd::Image`); they persist across frames.
    pub ui_images: Vec<crate::ui::UiImage>,
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
            lights: Vec::with_capacity(256),
            casters: Vec::with_capacity(1024),
            ui: Vec::with_capacity(4096),
            ui_images: Vec::new(),
        }
    }

    pub fn chunks_in(&self, s: Span) -> &[ChunkCmd] {
        &self.chunks[s.range()]
    }

    pub fn sprites_in(&self, s: Span) -> &[SpriteCmd] {
        &self.sprites[s.range()]
    }

    pub fn lights_in(&self, s: Span) -> &[Light] {
        &self.lights[s.range()]
    }

    pub fn casters_in(&self, s: Span) -> &[Caster] {
        &self.casters[s.range()]
    }

    /// The layers a chunk command draws.
    pub fn layers_of(&self, c: &ChunkCmd) -> &ChunkLayers {
        &self.layers[usize::from(c.slot)]
    }
}
