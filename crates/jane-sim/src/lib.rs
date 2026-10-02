//! The deterministic simulation: state tree, tick, verbs, save, replay, hash, View.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).
//!
//! The contract is ARCHITECTURE.md §3 to §5, §8, §9 and §11. Module map:
//!
//! | Module | What |
//! | --- | --- |
//! | [`ids`] | instance ids, counters, seats |
//! | [`sym`] | runtime names |
//! | [`state`] | the authoritative tree (`GameState`, `PlayerState`, `ZoneState`, `Unit`, `Prop`) |
//! | [`zone`] | a zone's state from its blueprint; tile deltas |
//! | [`blueprints`] | the thirteen blueprints of a seed |
//! | [`grid`], [`runtime`] | the derived runtime: grid, occupancy, buckets, awake sets, names, triggers |
//! | [`path`], [`los`], [`units`], [`ring`], [`fog`] | feet, sight, movement, the load ring, fog |
//! | [`input`], [`event`] | frames, commands, events |
//! | [`ctx`], [`actions`] | the context every system takes; the verb runner |
//! | [`combat`], [`assist`] | the cast pipeline, the combat verbs and the console's; aim assist |
//! | [`flight`], [`status`], [`flush`] | steps 8, 9 and 10: bolts and pools, statuses, the flush (death, phases) |
//! | [`life`], [`loot`] | regen, respawn and waking; drops |
//! | [`regrow`] | food that comes back: emptied trees, windfalls and larders fill again |
//! | [`interact`], [`inventory`], [`bag`], [`dialogue`], [`quests`] | USE and the world verbs; bags, items and crafting; conversations; the quest log |
//! | [`triggers`], [`under`], [`clear`], [`light`], [`verbs`] | triggers and plates; things under things; nothing solid lands on a unit; the light rule; rest and growth |
//! | [`journal`] | what is known (§3.7) |
//! | [`metrics`] | what the last step cost, for an overlay and a profiler (never state) |
//! | [`living`] | the living world (§4.6): the sky, the rain ramp, ecology, consequences, rumours |
//! | [`hooks`] | combat's calls into the interact, inventory, quests, triggers, journal and living-world units |
//! | [`ai`], [`snake`], [`npc`], [`presence`] | step 7's controllers (the AI loop, the snake, orders and npcs), step 3's schedules |
//! | [`sim`], `seats`, `travel` | the scheduler, seats and commands, travel |
//! | [`save`] | save, load and the hash |
//! | [`replay`] | tapes: record, re-simulate, verify (`.jrp`) |
//! | [`trace`] | traces: what a played session was like, observed (`.jtr`, VERIFICATION.md §3.1) |
//! | [`view`] | what a seat sees |

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod actions;
pub mod ai;
#[cfg(test)]
mod ai_tests;
pub mod assist;
pub mod bag;
pub mod blueprints;
pub mod clear;
pub mod codec;
pub mod combat;
#[cfg(test)]
mod combat_tests;
pub mod ctx;
pub mod dialogue;
pub mod event;
pub mod fire;
pub mod flight;
pub mod flush;
pub mod fog;
pub mod grid;
pub mod hooks;
pub mod ids;
pub mod input;
pub mod interact;
pub mod inventory;
pub mod journal;
pub mod life;
pub mod light;
pub mod living;
pub mod loot;
pub mod los;
pub mod metrics;
pub mod npc;
pub mod omens;
pub mod path;
pub mod presence;
pub mod quests;
pub mod regrow;
pub mod replay;
pub mod ring;
pub mod route;
pub mod runtime;
pub mod save;
mod seats;
pub mod sim;
pub mod snake;
pub mod state;
pub mod status;
pub mod store;
pub mod sym;
pub mod trace;
mod travel;
pub mod triggers;
pub mod tuning;
pub mod under;
pub mod units;
pub mod verbs;
pub mod view;
pub mod zone;

pub use blueprints::Blueprints;
pub use combat::Hit;
pub use event::{Event, EventKind, SpellError};
pub use ids::{ClientToken, DropId, GroundId, ProjId, PropId, Seat, UnitId};
pub use input::{AssistProfile, Command, DevOp, InputFrame, StampedCommand, StepInput, Stepped};
pub use metrics::{Phase, SimMetrics, WallClock};
pub use save::{Header, SaveError, Snapshot, Summary};
pub use sim::Sim;
pub use state::{GameState, PlayerState, Prop, Unit, ZoneState};
pub use view::View;
