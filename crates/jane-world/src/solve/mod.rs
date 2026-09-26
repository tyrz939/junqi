//! The lock-and-key solver (PORT.md §6.k; `jane/src/world/validate.ts`). The 2020 design folder
//! holds a "Dungeon Graph Making Tools" kit (entrance -> small key -> locked door -> boss key ->
//! boss door -> BOSS); this is that graph, checked by machine on every seed.
//!
//! It plays the zone as a flood fill: walk everywhere reachable, pick up every key in reach
//! (chests, guaranteed drops), open every gate a held key fits, work every lever, plate,
//! repairable and kill-trigger in reach, and repeat until nothing changes. A zone holds when its
//! contract exists, every list names things in it, nothing stands in a wall, and everything
//! required is reached (ARCHITECTURE.md §5.3 point 2). A refused candidate is re-rolled, never
//! thrown at the player.
//!
//! What it knows beyond keys (DUNGEONS.md §2.6):
//! - **verbs**: given [`ZoneRules::given_verbs`], a prop that answers Repair, Grow or a school
//!   fires only once a spell of that kind is known; spells are learned from `learn` rows in a
//!   reached prop's `use` or `talk` tree. Without them nothing is gated.
//! - **when**: a `while` trigger fires only when its conditions can hold. A negated condition
//!   never blocks: it held earlier if at all.
//! - **if**: an `if` runs its `then` once its conditions can hold and its `else` if they did not
//!   when the list first ran; a `then` that could not run yet is kept, unless the list's owner
//!   only happens once.
//! - **hops**: a `to` that names this zone is a one-way edge to its mark.
//! - **states**: the stateful flood (`passes/states.rs`).
//! - **ablation**: [`ablate`] takes one thing away, so a caller can prove a lock holds.
//!
//! Callers: `buildZone`'s loop asks [`validate`] with [`ZoneRules::for_zone`] of each candidate
//! and re-rolls on `!report.ok()`; the dungeon checks (C1 to C12) ask [`solve`] traced and
//! [`ablate::lock_holds`]; the template harness asks [`solve`] with `fragment` and `entry`.

pub mod ablate;
pub(crate) mod flood;
pub mod model;
pub(crate) mod passes;
pub mod report;
pub mod rows;
pub mod run;
pub mod sketch;

pub use ablate::{Grant, Withhold, lock_holds, without};
pub use model::{Contract, KeyTag, MAX_STATES, Options, Trail, ZoneRules};
pub use report::{BuildInfo, Owner, Report, RowFault, SolveError, TriggerName};
pub use run::{resolve, solve, solve_kept, validate};
pub use sketch::Sketch;
