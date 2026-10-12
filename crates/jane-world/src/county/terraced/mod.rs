//! The terraced county (MAP.md): the new layout, built beside the current one and wired to nothing
//! live yet. Round R0 is paper only: [`macro_plan`] decides districts, levels, the gate graph, ledges,
//! sites and routes on the 96 x 96 macro grid (K1 to K5 of MAP.md section 4.1), and
//! [`macro_check`] proves the graph (reachable on foot honouring one-way ledges, no pits, gates
//! per border, ledges toward hubs). Round R4 carves it ([`carve`]), reads the carve as a skeleton
//! ([`skel`]), lays the county's stages over it ([`build`], [`stage`], [`roads`]) and proves the
//! result ([`prove`]). No hash or golden of the live county moves.
//!
//! The game ships one county, not a random one: [`CHOSEN`] pins it.

pub mod build;
pub mod carve;
pub mod macro_check;
pub mod macro_plan;
pub mod prove;
pub mod roads;
pub mod skel;
pub mod stage;
#[cfg(test)]
mod tests;
pub mod threat;

use jane_core::{Plane, Rect};

/// The county the game ships, chosen by the owner from the carved sheets: seed 7, the island layout,
/// the macro plan `plan_from(7, 0)` accepts (attempt 2) and the carve's salt 1. `build::build_pinned` makes
/// it, the same on every target.
pub const CHOSEN: build::Pin = build::Pin { seed: 7, plan: 2, salt: 1 };

/// Whether every cell of `r` is at one level (a chunk's box that crosses a face is not).
pub fn one_level(level: &Plane, r: Rect) -> bool {
    let mut cells = r.cells();
    let Some((x, y)) = cells.next() else { return true };
    let first = level.get(x as u32, y as u32);
    cells.all(|(x, y)| level.get(x as u32, y as u32) == first)
}
