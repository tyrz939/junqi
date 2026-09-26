//! The sim's numbers. Structural constants (cell, tick rate, ring block, path window, seats)
//! stay Rust `const` for good (ARCHITECTURE.md §1 "Where things went"); the tunables below are
//! here until `data/tuning/sim.json` lands and `TUNING` carries them (§6 "Tuning").

use jane_core::num::{CELL_FX, FX_ONE, TICK_RATE};
use jane_core::{Fx, Milli, Tick};

/// Up to four seats share a world (PLATFORM.md).
pub const MAX_PLAYERS: usize = 4;
pub const BAG_SLOTS: usize = 24;
pub const BAR_SLOTS: usize = 8;
pub const CRAFT_INPUTS: usize = 3;

/// `CreatureStatCalc`: max hp = strength x 5, max mp = spirit x 5.
pub const HP_PER_STRENGTH: i32 = 5;
pub const MP_PER_SPIRIT: i32 = 5;

/// Energy, milli-points per tick (`constants.ts` 0.5, 0.25, 0.5 of a 100 max).
pub const ENERGY_MAX: Milli = Milli(100_000);
pub const ENERGY_SPRINT: Milli = Milli(500);
pub const ENERGY_CARRY: Milli = Milli(250);
pub const ENERGY_REGEN: Milli = Milli(500);

/// The body: a 6 x 6 px box on the feet, smaller than a cell so one-cell corridors work.
pub const BODY_HALF_FX: i32 = 3 * FX_ONE;

/// Day clock: one game hour is two real minutes.
pub const TICKS_PER_HOUR: u32 = 7200;
pub const TICKS_PER_DAY: u32 = TICKS_PER_HOUR * 24;
/// New games begin at 17:00 ("When she arrived the town it was already 5pm").
pub const START_HOUR: u32 = 17;
/// Night, for the sim: 21:00 to 06:00.
pub const NIGHT_START_HOUR: u32 = 21;
pub const NIGHT_END_HOUR: u32 = 6;

/// The load ring (`objects/distance_unload`, 2020): 2-cell blocks, 24 blocks each way.
pub const RING_BLOCK_FX: i32 = 2 * CELL_FX;
pub const RING_RADIUS_FX: i32 = 24 * RING_BLOCK_FX;
/// Props stay up 64 px beyond the ring, measured from their origin cell.
pub const PROP_SLACK_FX: i32 = 64 * FX_ONE;

/// Props by block, units by block: 16 cells (128 px) a side.
pub const BLOCK_CELLS: i32 = 16;

/// Fog: cells per bit on a side, indoors and out; the radius stamped round each seat, in bits.
pub const FOG_CELLS_IN: u32 = 2;
pub const FOG_CELLS_OUT: u32 = 8;
pub const FOG_RADIUS_IN: i32 = 9;
pub const FOG_RADIUS_OUT: i32 = 4;
/// Fog is stamped every this many ticks (housekeeping), and on arrival.
pub const FOG_EVERY: u32 = 10;

/// A stick pushed less than this far (of 127) does not move her (`len > 0.05` in the TS).
pub const MOVE_DEADZONE: u8 = 6;
/// Ticks a dead seat lies before she stands back up.
pub const PLAYER_RESPAWN: Tick = Tick(240);

/// Nearest-free-cell search radii (cells): arrival and join, spawn.
pub const ARRIVAL_RADIUS: i32 = 8;
pub const SPAWN_RADIUS: i32 = 6;

/// What a creature costs at each threat, as a multiple of its row; index 0 unused.
pub const PHASE_SCALE: [u8; 7] = [1, 1, 2, 3, 4, 6, 8];

/// The presence step's cadence (dayOnly / nightOnly, later schedules).
pub const PRESENCE_EVERY: u32 = 30;

pub const fn ticks(secs: u32) -> Tick {
    Tick(secs * TICK_RATE)
}

pub const fn px(p: i32) -> Fx {
    Fx(p * FX_ONE)
}

// --- the world verbs (interact, inventory, dialogue, journal) --------------------------------

/// Interact reach from the feet, measured to a prop's footprint (`constants.ts USE_REACH`, 12 px).
pub const USE_REACH_FX: i32 = 12 * FX_ONE;
/// Talking reach, feet to feet (`interact.ts TALK_REACH`, 20 px).
pub const TALK_REACH_FX: i32 = 20 * FX_ONE;
/// Picking a drop up, feet to the drop (`loot.ts PICKUP_REACH`, 14 px).
pub const PICKUP_REACH_FX: i32 = 14 * FX_ONE;
/// Something behind her scores this much further away (`focusOf`, 8 px).
pub const FOCUS_BEHIND_FX: i32 = 8 * FX_ONE;
/// A thing that only pushes loses a tie to one with words or contents (`PUSH_ONLY_PENALTY`, 6 px).
pub const FOCUS_PUSH_ONLY_FX: i32 = 6 * FX_ONE;
/// Hold USE this many ticks against a pushable to move it one cell (2020: 30 frames).
pub const PUSH_HOLD_TICKS: u8 = 30;
/// A push, and lifting something, want this much energy; a push spends it.
pub const PUSH_ENERGY: Milli = Milli(20_000);
/// How close to a prop's middle a bolt must end to switch it on, unless its spell row says.
pub const SCHOOL_TOUCH_FX: i32 = 14 * FX_ONE;
/// A world spell (Repair, Grow) finds the nearest answering prop within 2 m.
pub const WORLD_SPELL_REACH_FX: i32 = 2 * CELL_FX;
/// Global cooldown (1.5 s); an item used starts it.
pub const GCD: Tick = Tick(90);
/// Using an item roots her for half a second.
pub const ITEM_STOP: Tick = Tick(30);
/// Plates are looked at every this many ticks (`triggers.ts PLATE_PERIOD`).
pub const PLATE_PERIOD: u32 = 6;
/// Journal entries kept per kind (ARCHITECTURE.md §12).
pub const JOURNAL_RING: u32 = 512;
/// A sent unit gives up after this long plus three times what the straight walk would take.
pub const ORDER_BASE: Tick = Tick(600);
/// How far a sent unit plans in one go, in cells (`ai.ts ORDER_PATH_METRES`).
pub const ORDER_PATH_CELLS: u32 = 400;
/// "Beside the mark will do": arrived within 1.5 cells.
pub const ORDER_ARRIVED_FX: i32 = CELL_FX * 3 / 2;
/// Lamp posts burn from 18:30 to 06:30 (`light.ts LAMPS_ON`, `LAMPS_OFF`), in clock ticks.
pub const LAMPS_ON: u32 = TICKS_PER_HOUR * 37 / 2;
pub const LAMPS_OFF: u32 = TICKS_PER_HOUR * 13 / 2;
/// The nudge search's most cells (`clear.ts NUDGE_CELLS`).
pub const NUDGE_CELLS: u32 = 1500;
