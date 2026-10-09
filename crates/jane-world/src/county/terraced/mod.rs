//! The terraced county (MAP.md): the new layout, built beside the current one and wired to nothing
//! yet. Round R0 is paper only: [`macro_plan`] decides districts, levels, the gate graph, ledges,
//! sites and routes on the 96 x 96 macro grid (K1 to K5 of MAP.md section 4.1), and
//! [`macro_check`] proves the graph (reachable on foot honouring one-way ledges, no pits, gates
//! per border, ledges toward hubs). No cell is carved and no blueprint, hash or golden moves.

pub mod macro_check;
pub mod macro_plan;
#[cfg(test)]
mod tests;
