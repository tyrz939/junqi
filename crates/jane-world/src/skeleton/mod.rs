//! The county's skeleton (WORLDGEN.md): regions, the river, the hill, where the story's places
//! are, the roads between them, how dangerous each patch is. Decided on the 125 x 125 macro grid
//! before a tile is drawn.

pub mod rail;
pub mod rank;
pub mod roads;
pub mod terrain;
pub mod types;

pub use terrain::{Lake, Terrain, build_terrain};
pub use types::*;
