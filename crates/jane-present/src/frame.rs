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
    /// The grade and the bloom (§1.9), last before the UI, on every tier: exposure, saturation,
    /// tint and lift drawn as T2 draws them (the tiers are one look, decided 2026-09-27), and the
    /// bloom where the tier's `bloom` row is on (T2 and T1 by their chains, T0 a quarter-size blur
    /// of what glows).
    Post(Post),
    /// The sky over the view (§1.9 `Sky`): bands by hour, stars, the moon. Seen where the view
    /// meets the zone's edge (`SkyLook::zone`) and, on T2, in every water cell's reflection.
    Sky(SkyLook),
    /// Far things on the sky's horizon (§1.9 `FarLandmark`, `FarTreeline`): `Frame::sprites[sprites]`,
    /// each placed on the sky backdrop (`x` across the canvas, `y` from the horizon, negative up).
    Parallax { layer: Depth, factor: u8, sprites: Span },
    /// The water cells in view (§1.8): T0 draws their shimmer; T2 reflects and refracts in its
    /// water pass from the chunks' water layer. `Frame::water[cells]`.
    Water { cells: Span },
    /// What the sky is doing this frame (§1.9 `Weather`), read by the passes after it: the rain
    /// and the mist, the wind, a lightning flash, how wet the ground is.
    Weather(Atmos),
    /// Fog volumes (§1.9 `NearFog`): `Frame::fog[volumes]`. T0 draws one drift of the mist tile
    /// at the strongest density; T2 two drifting layers, lit by the lights they stand in.
    Fog { volumes: Span, drift: (i16, i16) },
    /// Light shafts through what stands against a low sun, where the air holds them (§1.9 god
    /// rays): T2.
    Rays { strength: u8 },
    /// Particles, strokes and rings of the fx pool and the weather (§2): `Frame::parts[parts]`, in
    /// canvas px, drawn over what is lit (their colour already lit by the ambient below T2; T2
    /// lights them itself). `layer` is `Ground` (splashes, ripples, scorch), `Canopy` (casts,
    /// bolts, impacts) or `Weather` (rain).
    Particles { layer: Depth, parts: Span },
}

impl Pass {
    /// Its name as F2 prints it.
    pub const fn name(&self) -> &'static str {
        match self {
            Pass::Terrain { .. } => "terrain",
            Pass::Sprites { .. } => "sprites",
            Pass::Silhouettes { .. } => "silhouettes",
            Pass::Lights { .. } => "lights",
            Pass::Post(_) => "post",
            Pass::Sky(_) => "sky",
            Pass::Parallax { .. } => "far",
            Pass::Water { .. } => "water",
            Pass::Weather(_) => "weather",
            Pass::Fog { .. } => "fog",
            Pass::Rays { .. } => "rays",
            Pass::Particles { .. } => "particles",
        }
    }

    /// The lowest tier that draws this pass as it stands (its `Features` row, §1.3).
    pub fn needs(&self) -> Tier {
        match self {
            Pass::Terrain { .. }
            | Pass::Sprites { .. }
            | Pass::Lights { .. }
            | Pass::Silhouettes { .. }
            | Pass::Sky(_)
            | Pass::Parallax { .. }
            | Pass::Water { .. }
            | Pass::Weather(_)
            | Pass::Fog { .. }
            | Pass::Particles { .. }
            | Pass::Post(_) => Tier::T0,
            Pass::Rays { .. } => Tier::T1,
        }
    }
}

/// What the sky is doing over the view (WORLD.md §5.1), as the presenter eases it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeatherKind {
    #[default]
    Clear,
    Mist,
    Rain,
    Storm,
}

/// The weather of one frame (§1.9): plain values, every level eased by tick so a turn of the
/// sky comes on over seconds, not in a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Atmos {
    pub kind: WeatherKind,
    /// How hard it rains, 0..=255.
    pub rain: u8,
    /// How thick the weather's mist is, 0..=255.
    pub mist: u8,
    /// The wind across the view, canvas px a tick (east is positive).
    pub wind: i8,
    /// A lightning flash, 0..=255: full for two ticks, then gone (§1.11).
    pub flash: u8,
    /// How wet the ground is, 0..=255 (§1.8): the sim's rain ramp under her feet.
    pub wet: u8,
}

/// The moon on the sky backdrop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moon {
    /// Canvas x of its middle; px above the horizon.
    pub x: i16,
    pub up: i16,
    /// 0 new .. 8 full .. 15 (a sixteen-day month, `light::moon_phase`).
    pub phase: u8,
    pub colour: Rgb,
}

/// The sky at this hour (§1.9 `Sky`): a backdrop whose row 0 is the horizon, rising to the
/// zenith `ZENITH_PX` px up, drawn behind the zone's edge and reflected in water.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkyLook {
    /// At the zenith, at the horizon, and the afterglow's colour low in the west.
    pub zenith: Rgb,
    pub horizon: Rgb,
    pub glow: Rgb,
    /// Canvas x the afterglow is centred on (the sun's bearing), and its strength 0..=255.
    pub glow_x: i16,
    pub glow_amount: u8,
    /// How many of the stars show, 0..=255 (none by day, all on a clear night; cloud hides them).
    pub stars: u8,
    /// The stars as they stand this frame, twinkle and cloud applied: `Frame::stars[star_list]`.
    pub star_list: Span,
    pub moon: Option<Moon>,
    /// The zone in canvas px `(x0, y0, x1, y1)`: the sky is seen outside it, its horizon along
    /// the zone's top edge.
    pub zone: (i32, i32, i32, i32),
    /// Tick, for the stars' twinkle.
    pub tick: u32,
}

/// Px from the horizon to the zenith on the sky backdrop.
pub const ZENITH_PX: i32 = 240;

/// A star on the sky backdrop: canvas x, px above the horizon, and its brightness this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarCmd {
    pub x: i16,
    pub up: i16,
    pub bright: u8,
}

/// A water cell in view (§1.8): its top-left in canvas px and its shimmer phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterCmd {
    pub x: i16,
    pub y: i16,
    pub phase: u8,
}

/// A fog volume (§1.9): a rect of the view with its density, colour and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FogVolume {
    /// Canvas px `(x0, y0, x1, y1)`; the density fades over `edge` px inside it.
    pub rect: (i32, i32, i32, i32),
    pub edge: u16,
    /// 0..=255 at its thickest.
    pub density: u8,
    pub colour: Rgb,
    /// How high it stands, px: 0 is full height; a ground fog stands `top` px and a thing taller
    /// than that shows its head and shoulders over it.
    pub top: u8,
}

/// How a particle is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartShape {
    /// A 1-px stroke from `(x, y)` to `(x + dx, y + dy)`, fading toward its tail: rain, a
    /// bolt's trail, a spark.
    Streak { dx: i8, dy: i8 },
    /// A square `size` px across (a mote, an ember, a drop of spray).
    Dot { size: u8 },
    /// A 1-px ring of radius `r`, squashed to half height on the ground (a splash, a ripple, a
    /// blast's front).
    Ring { r: u8 },
    /// A soft disc of radius `r`, bright at the middle (a bolt's head, a cast's gather).
    Glow { r: u8 },
}

/// One particle of the fx pool or the weather, in canvas px (§2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Particle {
    pub x: i16,
    pub y: i16,
    pub shape: PartShape,
    pub colour: Rgb,
    /// 0..=255 opacity.
    pub alpha: u8,
    /// 0..=255 of it that glows unlit (and blooms on T2).
    pub glow: u8,
    /// Px above its ground point, which is `height` px below it: what lights it on T2.
    pub height: u8,
}

/// T1's particle pool (§1.3 `max_particles`): the rain a third of it, as T2's. Raised from 2000
/// on 2026-09-27 so a wet night on a Pi reads as T2's rain (one quad a drop).
pub const T1_PARTICLES: u16 = 6000;
/// Whether T0 blooms by default (§1.3 `bloom`): a quarter-size blur of what glows.
pub const T0_BLOOM: bool = true;

/// The visual features (§1.3): a row each, with the tier it needs and its `config.json` key
/// under `present`. `Features::of(tier)` is each tier's default; a row set above its tier's
/// reach is held to what the tier draws. The presenter reads the rows that change what a frame
/// holds (the atmosphere, the lights, the silhouettes, the grade); a backend is handed the rest
/// (`Backend::set_features`: `normal_light` and `sharp`); the app reads `frame_skip`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features {
    /// `normal_light`: N dot L per px (T1; T2 always). Off: every surface faces up.
    pub normal_light: bool,
    /// `shadows`: how many point lights throw shadows (T1 8, T2 32; 0 is the row off).
    pub shadows: u8,
    /// `silhouettes`: the sun's and moon's silhouette shadows (T0, T1).
    pub silhouettes: bool,
    /// `max_lights`: point lights drawn at most (16, 32, 128).
    pub max_lights: u16,
    /// `bloom`: what glows blooms (T2 and T1: a chain of halvings; T0: a quarter-size blur of
    /// the emissive, 2026-09-27).
    pub bloom: bool,
    /// `glow`: T0's emissive, lamp glass and lit windows added unlit over the lightmap (T1 and T2
    /// always draw it; off, T0's glass is its albedo, lit like the rest).
    pub glow: bool,
    /// `grade`: the grade per region and hour (exposure, saturation, tint and lift, and the dusk's
    /// afterglow, on every tier; the bloom is its own row).
    pub grade: bool,
    /// `sharp`: sharp bilinear to the window (T1, T2); off, nearest.
    pub sharp: bool,
    /// `frame_skip`: draw every other tick (30 fps); the sim still steps at 60.
    pub frame_skip: bool,
    /// `weather`: rain, storm and mist, their particles and their grade.
    pub weather: bool,
    /// `fog`: fog volumes per area (one drift tile on T0, two layers on T1 and T2).
    pub fog: bool,
    /// `water`: shimmer on T0, reflection and refraction on T2.
    pub water: bool,
    /// `wet`: the wet ground's darkening and specular (T1 and T2).
    pub wet: bool,
    /// `god_rays`: light shafts (T2; T1 from the silhouettes' mask, 2026-09-27).
    pub god_rays: bool,
    /// `sky`: the sky, the far landmark and the far treeline.
    pub sky: bool,
    /// `max_particles`: the particle pool; a third of it is the weather's.
    pub max_particles: u16,
}

/// A `Features` row as the Controls screen offers it (§1.3): its `config.json` key, its label,
/// and whether a change shows at once (else on the next start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureRow {
    pub key: &'static str,
    pub label: &'static str,
    pub live: bool,
}

const fn row(key: &'static str, label: &'static str) -> FeatureRow {
    FeatureRow { key, label, live: true }
}

/// Every row a player can turn, in the Controls screen's order; `Features::rows` keeps the ones a
/// tier has. `soft_shadows` and `sun_shadows` are not among them: T2's traced shadows are its
/// only ones, and T1's sun is its silhouettes. `half_res` is not built.
const ROWS: [FeatureRow; 16] = [
    row("normal_light", "Lit relief"),
    row("shadows", "Lamp shadows"),
    row("silhouettes", "Sun shadows"),
    row("max_lights", "Lights"),
    row("weather", "Weather"),
    row("fog", "Fog"),
    row("water", "Water"),
    row("wet", "Wet ground"),
    row("god_rays", "Light shafts"),
    row("sky", "Sky"),
    row("bloom", "Bloom"),
    row("glow", "Lit windows"),
    row("grade", "Grade"),
    row("max_particles", "Particles"),
    row("sharp", "Sharp upscale"),
    row("frame_skip", "Frame skip"),
];

impl Features {
    /// The rows' keys, in the order `jane bench` and F2 print them and `config.json` holds them.
    pub const KEYS: [&'static str; 16] = [
        "normal_light",
        "shadows",
        "silhouettes",
        "max_lights",
        "weather",
        "fog",
        "water",
        "wet",
        "god_rays",
        "sky",
        "bloom",
        "glow",
        "grade",
        "max_particles",
        "sharp",
        "frame_skip",
    ];

    /// A tier's defaults (§1.3).
    pub const fn of(tier: Tier) -> Features {
        Features {
            normal_light: !matches!(tier, Tier::T0),
            shadows: match tier {
                Tier::T0 => 0,
                Tier::T1 => 8,
                Tier::T2 => 32,
            },
            silhouettes: !matches!(tier, Tier::T2),
            max_lights: match tier {
                Tier::T0 => 16,
                Tier::T1 => 32,
                Tier::T2 => 128,
            },
            bloom: T0_BLOOM || !matches!(tier, Tier::T0),
            glow: true,
            grade: true,
            sharp: !matches!(tier, Tier::T0),
            frame_skip: false,
            weather: true,
            fog: true,
            water: true,
            wet: !matches!(tier, Tier::T0),
            god_rays: !matches!(tier, Tier::T0),
            sky: true,
            max_particles: match tier {
                Tier::T0 => 900,
                Tier::T1 => T1_PARTICLES,
                Tier::T2 => 8000,
            },
        }
    }

    /// Sets a row by its `config.json` key (`"on"`, `"off"`, or a number for `shadows`,
    /// `max_lights` and `max_particles`, where `"on"` is the tier's own and `"off"` none), held to
    /// what `tier` can draw. `false` if the key or value is not one.
    pub fn set(&mut self, tier: Tier, key: &str, value: &str) -> bool {
        let on = match value {
            "on" | "true" | "1" => Some(true),
            "off" | "false" | "0" => Some(false),
            _ => None,
        };
        let top = Features::of(tier);
        let count = |most: u16| match (on, value.parse::<u16>()) {
            (_, Ok(n)) => Some(n.min(most)),
            (Some(true), _) => Some(most),
            (Some(false), _) => Some(0),
            (None, Err(_)) => None,
        };
        match (key, on) {
            ("normal_light", Some(v)) => self.normal_light = v && tier > Tier::T0,
            ("silhouettes", Some(v)) => self.silhouettes = v && tier < Tier::T2,
            ("bloom", Some(v)) => self.bloom = v,
            ("glow", Some(v)) => self.glow = v || tier > Tier::T0,
            ("grade", Some(v)) => self.grade = v,
            ("sharp", Some(v)) => self.sharp = v && tier > Tier::T0,
            ("frame_skip", Some(v)) => self.frame_skip = v,
            ("weather", Some(v)) => self.weather = v,
            ("fog", Some(v)) => self.fog = v,
            ("water", Some(v)) => self.water = v,
            ("wet", Some(v)) => self.wet = v && tier > Tier::T0,
            ("god_rays", Some(v)) => self.god_rays = v && tier > Tier::T0,
            ("sky", Some(v)) => self.sky = v,
            ("shadows", _) => match count(u16::from(top.shadows)) {
                Some(n) => self.shadows = n as u8,
                None => return false,
            },
            ("max_lights", _) => match count(top.max_lights) {
                // No light at all is not a row: the lamps are what the sim says is lit.
                Some(n) => self.max_lights = n.max(1),
                None => return false,
            },
            ("max_particles", _) => match count(top.max_particles) {
                Some(n) => self.max_particles = n,
                None => return false,
            },
            _ => return false,
        }
        true
    }

    /// A row's value as `config.json` holds it: `"on"` or `"off"`, or the count.
    pub fn get(&self, key: &str) -> Option<String> {
        let b = |v: bool| Some(if v { "on" } else { "off" }.to_owned());
        match key {
            "normal_light" => b(self.normal_light),
            "shadows" => Some(self.shadows.to_string()),
            "silhouettes" => b(self.silhouettes),
            "max_lights" => Some(self.max_lights.to_string()),
            "bloom" => b(self.bloom),
            "glow" => b(self.glow),
            "grade" => b(self.grade),
            "sharp" => b(self.sharp),
            "frame_skip" => b(self.frame_skip),
            "weather" => b(self.weather),
            "fog" => b(self.fog),
            "water" => b(self.water),
            "wet" => b(self.wet),
            "god_rays" => b(self.god_rays),
            "sky" => b(self.sky),
            "max_particles" => Some(self.max_particles.to_string()),
            _ => None,
        }
    }

    /// The rows a player can turn at `tier` (the Controls screen's toggles): those the tier draws
    /// (§1.3's `Needs` column and the per-tier cells that say "no").
    pub fn rows(tier: Tier) -> impl Iterator<Item = FeatureRow> {
        ROWS.into_iter().filter(move |r| match r.key {
            "normal_light" | "sharp" | "wet" => tier == Tier::T1 || (tier == Tier::T2 && r.key != "normal_light"),
            "shadows" | "god_rays" => tier > Tier::T0,
            "silhouettes" => tier < Tier::T2,
            "glow" => tier == Tier::T0,
            _ => true,
        })
    }

    /// The row's next value as the Controls screen turns it: a switch flips; a count steps down
    /// by halves from the tier's own (lamp shadows to none), then back up to it.
    pub fn cycle(&mut self, tier: Tier, key: &str) {
        let top = Features::of(tier);
        let half = |v: u16, most: u16, floor: u16| if v <= floor { most } else { (v / 2).max(floor) };
        match key {
            "shadows" => {
                let v = u16::from(self.shadows);
                self.shadows = if v == 0 {
                    top.shadows
                } else if v <= 2 {
                    0
                } else {
                    (v / 2) as u8
                };
            }
            "max_lights" => self.max_lights = half(self.max_lights, top.max_lights, top.max_lights / 4),
            "max_particles" => self.max_particles = half(self.max_particles, top.max_particles, top.max_particles / 4),
            _ => {
                if let Some(v) = self.get(key) {
                    self.set(tier, key, if v == "on" { "off" } else { "on" });
                }
            }
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
    /// How tall it stands, true px: its sprite's tallest px (`SpriteRef::top`). A silhouette's
    /// row stands no higher than this, so a low wide thing seen from above (a bed, a cart's
    /// load) throws the short shadow of what it is, not of how many rows it takes on screen.
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

/// A painted chunk, `CHUNK_PX` square, four layers (§1.6). A T0 presenter carries the albedo and
/// the height (what its silhouettes climb) and leaves the normal and the emissive empty; T1 and
/// T2 get all four.
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
    /// Per px, what the ground is to the weather and the water (§1.8), T1 and T2 only:
    /// [`SURFACE_OUTSIDE`] beyond the zone (where the sky shows), else `water * 4 + wet`: `water`
    /// the px's distance to land, 1..=16 in water and 0 on land, and `wet` how the ground takes
    /// rain (0 matt, 1 darkens, 2 darkens and shines).
    pub surface: Vec<u8>,
    /// The chunk's water cells, chunk-local `(x, y, shimmer phase)`: every tier.
    pub water: Vec<(u8, u8, u8)>,
    /// T0 only (no emissive layer): what glows in the chunk, sparse, each px's index in the chunk
    /// and its colour `0xAARRGGBB`, at most [`GLOW_CAP`] (reserved once, so painting never
    /// allocates). The lit tiers read the emissive layer.
    pub glow: Vec<(u16, u32)>,
}

/// The most glowing px a T0 chunk keeps (a street of lit windows is a few hundred).
pub const GLOW_CAP: usize = 2048;

/// A chunk px beyond the zone's edge: the sky shows there.
pub const SURFACE_OUTSIDE: u8 = 254;

impl ChunkLayers {
    /// A chunk's layers: the albedo and the height at T0, all four above it.
    pub fn new(tier: Tier) -> ChunkLayers {
        let n = (CHUNK_PX * CHUNK_PX) as usize;
        let water = Vec::with_capacity((CHUNK_CELLS * CHUNK_CELLS) as usize);
        if tier == Tier::T0 {
            ChunkLayers {
                albedo: vec![0; n],
                height: vec![0; n],
                water,
                glow: Vec::with_capacity(GLOW_CAP),
                ..ChunkLayers::default()
            }
        } else {
            ChunkLayers {
                albedo: vec![0; n],
                normal: vec![[128, 128]; n],
                emissive: vec![0; n],
                height: vec![0; n],
                surface: vec![0; n],
                water,
                glow: Vec::new(),
            }
        }
    }

    /// Whether the normal and emissive layers are carried (the height is, from T0 up).
    pub fn lit(&self) -> bool {
        !self.normal.is_empty()
    }

    /// Whether the height layer is carried.
    pub fn has_height(&self) -> bool {
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
    /// Water cells in view (`Pass::Water`).
    pub water: Vec<WaterCmd>,
    /// Fog volumes (`Pass::Fog`).
    pub fog: Vec<FogVolume>,
    /// Particles (`Pass::Particles`).
    pub parts: Vec<Particle>,
    /// Stars on the sky backdrop (`SkyLook::star_list`).
    pub stars: Vec<StarCmd>,
    /// Ticks presented: every drift, shimmer and twinkle is by tick (§1.11).
    pub tick: u32,
}

impl Frame {
    /// An empty frame at `tier`, 16:9.
    pub fn new(tier: Tier) -> Frame {
        Frame {
            tier,
            canvas: (CANVAS_W, CANVAS_H),
            camera: (0, 0),
            clear: 0xff10_1014,
            passes: Vec::with_capacity(24),
            chunks: Vec::with_capacity(64),
            sprites: Vec::with_capacity(4096),
            layers: Vec::new(),
            lights: Vec::with_capacity(256),
            casters: Vec::with_capacity(1024),
            ui: Vec::with_capacity(4096),
            ui_images: Vec::new(),
            water: Vec::with_capacity(1024),
            fog: Vec::with_capacity(32),
            parts: Vec::with_capacity(usize::from(Features::of(tier).max_particles) + 256),
            stars: Vec::with_capacity(128),
            tick: 0,
        }
    }

    pub fn water_in(&self, s: Span) -> &[WaterCmd] {
        &self.water[s.range()]
    }

    pub fn fog_in(&self, s: Span) -> &[FogVolume] {
        &self.fog[s.range()]
    }

    pub fn parts_in(&self, s: Span) -> &[Particle] {
        &self.parts[s.range()]
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
