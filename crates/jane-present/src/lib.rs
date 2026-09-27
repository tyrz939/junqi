//! The scene (PRESENTATION.md): builds the backend-agnostic `Frame` from a `View`; ui, input
//! mapping, text. Headless and GPU-free: every function here runs in a test with no window.
//!
//! The contract the app and the backends hold to (PORT.md §7.1):
//!
//! ```text
//! let mut present = Present::new(Tier::T0);
//! backend.upload_atlas(present.atlas());           // once at boot
//! present.tick(&view, events);                     // once per accumulated tick, sim run or not
//! let frame = present.draw(alpha, canvas);         // once per frame; pure in tick and alpha
//! backend.draw(frame);
//! ```

pub mod atlas;
pub mod atmos;
pub mod audio;
pub mod backend;
pub mod camera;
pub mod chunks;
pub mod creatures;
pub mod drawlist;
pub mod facing;
pub mod frame;
pub mod fx;
pub mod input;
/// The names bindings use in data and in `config.json` (shared with build.rs).
pub mod input_names;
pub mod lesson;
pub mod light;
pub mod people;
pub mod present;
pub mod props;
pub mod shadow;
pub mod stand_in;
pub mod terrain;
pub mod text;
pub mod ui;
pub mod view;

pub use backend::{AO_TINT, AtlasPages, Backend, CLUT_LEN, Caps, FrameStats, FrameTimes, Page, StatPass};
pub use frame::{
    Atmos, Block, CANVAS_H, CANVAS_W, Caster, ChunkCmd, ChunkId, ChunkLayers, Depth, Directional, FeatureRow, Features,
    Flags, FogVolume, Frame, Light, LightKind, Moon, PartShape, Particle, Pass, Post, Rgb, SkyLook, Span, SpriteCmd,
    Src, StarCmd, Tier, Tint, WaterCmd, WeatherKind, height_of_rows, rows_up,
};
pub use present::Present;
