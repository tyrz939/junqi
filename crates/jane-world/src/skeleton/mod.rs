//! The county's skeleton (WORLDGEN.md): regions, the river, the hill, where the story's places
//! are, the roads between them, how dangerous each patch is. Decided on the 125 x 125 macro grid
//! before a tile is drawn.

pub mod anchors;
pub mod build;
pub mod place;
pub mod rail;
pub mod rank;
pub mod roads;
pub mod terrain;
pub mod types;

pub use anchors::PlacedAnchor;
pub use build::{
    Check, DUNGEON_RING, Failed, GenStats, MAX_ATTEMPTS, Named, Skeleton, SkeletonError, SkeletonRows, build_skeleton,
    failures, mouth_phase, skeleton, threat_at,
};
pub use place::{POI_BUDGET, PlacedArea, PlacedPoi, PlacedSite};
pub use roads::{ROAD, ROAD_BRIDGE, ROAD_LIT, Road, RoadEnd};
pub use terrain::{Lake, Terrain, build_terrain};
pub use types::*;
