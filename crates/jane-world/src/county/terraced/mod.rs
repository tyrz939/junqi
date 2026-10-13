//! The terraced county (MAP.md), the game's one county. Its source is `data/world/county/` ([`source`]):
//! the macro plan ([`macro_plan`]: districts, levels, the gate graph, ledges, sites and routes on the
//! 96 x 96 macro grid, K1 to K5 of MAP.md section 4.1), where its things stand, and its roads and paths.
//! [`build`] compiles it: [`macro_check`] judges the plan's graph, the carve makes its ground ([`carve`]),
//! the skeleton stands its things ([`skel`]), the county's stages lay it ([`stage`], [`roads`]) and the
//! proofs judge the result ([`prove`]). `jane bake-world` writes it to `assets/world/county.bp`.

pub mod build;
pub mod carve;
pub mod macro_check;
pub mod macro_plan;
pub mod prove;
pub mod roads;
pub mod skel;
pub mod source;
pub mod stage;
#[cfg(test)]
mod tests;
pub mod threat;

use jane_core::{Plane, Rect};

/// Whether every cell of `r` is at one level (a chunk's box that crosses a face is not).
pub fn one_level(level: &Plane, r: Rect) -> bool {
    let mut cells = r.cells();
    let Some((x, y)) = cells.next() else { return true };
    let first = level.get(x as u32, y as u32);
    cells.all(|(x, y)| level.get(x as u32, y as u32) == first)
}
