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
pub mod backend;
pub mod camera;
pub mod chunks;
pub mod drawlist;
pub mod frame;
pub mod input;
pub mod light;
pub mod people;
pub mod present;
pub mod stand_in;
pub mod text;
pub mod ui;
pub mod view;

pub use backend::{AO_TINT, AtlasPages, Backend, CLUT_LEN, Caps, FrameStats, FrameTimes, Page, StatPass};
pub use frame::{
    CANVAS_H, CANVAS_W, Caster, ChunkCmd, ChunkId, ChunkLayers, Depth, Directional, Flags, Frame, Light, LightKind,
    Pass, Post, Rgb, Span, SpriteCmd, Src, Tier, Tint,
};
pub use present::Present;
