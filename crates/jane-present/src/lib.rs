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

pub mod backend;
pub mod frame;
pub mod present;

pub use backend::{AtlasPages, Backend, Caps};
pub use frame::{CANVAS_H, CANVAS_W, Frame, Tier};
pub use present::Present;
