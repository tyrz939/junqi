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
//! | [`sim`], `seats`, `travel` | the scheduler, seats and commands, travel |
//! | [`save`] | save, load and the hash |
//! | [`view`] | what a seat sees |

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod actions;
pub mod bag;
pub mod blueprints;
pub mod codec;
pub mod ctx;
pub mod event;
pub mod fog;
pub mod grid;
pub mod ids;
pub mod input;
pub mod los;
pub mod path;
pub mod ring;
pub mod runtime;
pub mod save;
mod seats;
pub mod sim;
pub mod state;
pub mod sym;
mod travel;
pub mod tuning;
pub mod units;
pub mod view;
pub mod zone;

pub use blueprints::Blueprints;
pub use event::{Event, EventKind};
pub use ids::{ClientToken, DropId, GroundId, ProjId, PropId, Seat, UnitId};
pub use input::{AssistProfile, Command, DevOp, InputFrame, StampedCommand, StepInput, Stepped};
pub use save::{Header, SaveError, Summary};
pub use sim::Sim;
pub use state::{GameState, PlayerState, Prop, Unit, ZoneState};
pub use view::View;
