//! The sim's numbers. Structural constants (cell, tick rate, ring block, path window, seats)
//! stay Rust `const` for good (ARCHITECTURE.md §1 "Where things went"); the tunables below are
//! here until `data/tuning/sim.json` lands and `TUNING` carries them (§6 "Tuning").

use jane_core::num::{CELL_FX, FX_ONE, TICK_RATE};
use jane_core::{Angle, Fx, Milli, Permille, Tick};

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
/// The ecology steps every ten game minutes (ARCHITECTURE.md §4.6.c): pressure comes off and a
/// held corpse is looked at again. Divides the hour, so every hour is also a mark.
pub const ECOLOGY_EVERY: u32 = TICKS_PER_HOUR / 6;
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

// --- combat (`constants.ts`, `combat.ts`, `loot.ts`) -----------------------------------------

/// The global cooldown: 1.5 s.
pub const GCD: Tick = Tick(90);
/// One hit in twenty lands double, unless a status says otherwise (`critOneIn`).
pub const CRIT_ONE_IN: u32 = 20;
/// Idle regen: a whole bar in 300 ticks (`max / 300` per tick, floored in milli-points).
pub const REGEN_DIVISOR: i32 = 300;
/// What each of the party deals and takes, by how many are connected (1 to 4), in permille.
/// Healing is never scaled. Together is a little better than alone; alone in a party of four
/// is 38 %.
pub const PARTY_DEALT: [Permille; MAX_PLAYERS] = [Permille(1000), Permille(620), Permille(460), Permille(380)];
pub const PARTY_TAKEN: [Permille; MAX_PLAYERS] = [Permille(1000), Permille(1150), Permille(1300), Permille(1450)];
/// A drop on the ground ages out after five minutes (2020's `obj_drop_parent`), unless the story
/// needs it or it is bound.
pub const DROP_LIFE: Tick = Tick(18_000);
/// Loot fans out on a 3-wide grid this far apart, so stacked drops are each visible.
pub const LOOT_SPREAD_FX: i32 = 6 * FX_ONE;
/// How close to a prop's middle a bolt must end to switch on a prop that answers its school,
/// when the spell's row says nothing (`interact.ts SCHOOL_TOUCH`).
pub const SCHOOL_TOUCH_FX: Fx = Fx(14 * FX_ONE);
/// A bolt starts this far along its line from the caster's feet.
pub const BOLT_START_FX: Fx = Fx(4 * FX_ONE);
/// A friendly spell cast with no cursor lands on the friend nearest the aim line, if she stands
/// closer to it than this (`combat.ts ALLY_AIM_SLACK`, 16 px: generous, she is moving).
pub const ALLY_AIM_SLACK_FX: i64 = 17 * FX_ONE as i64;
/// Melee prefers what it faces: something more than half a cell behind loses every tie to what
/// is in front (`combat.ts`, a 1000 m penalty).
pub const MELEE_BEHIND_FX: i64 = 1000 * CELL_FX as i64;
/// `Dev(Kill)`: everything hostile she can see within this box (25 m, roughly the screen).
pub const DEV_KILL_REACH_FX: i32 = 200 * FX_ONE;
/// `Dev(Kill)` hits for this much; nothing in the game has a million points.
pub const DEV_KILL_HIT: Milli = Milli(1_000_000_000);
/// `Dev(Spawn)`: this many cells east of her, on the nearest free cell within this radius.
pub const DEV_SPAWN_OFFSET: i32 = 3;
pub const DEV_SPAWN_RADIUS: i32 = 6;
/// Nearest-free-cell radius for a creature standing up again at home.
pub const RESPAWN_RADIUS: i32 = 6;
/// Nearest-free-cell radius for a seat waking at her mark.
pub const REVIVE_RADIUS: i32 = 8;
/// A snake's body is hit-tested at every this-many trail points (`snake.ts HITBOX_EVERY`).
pub const SNAKE_HITBOX_EVERY: usize = 8;

// --- aim assist (ARCHITECTURE.md §5.4, §12) ---------------------------------------------------

/// One aim-assist profile. `Pad` is wider than `Mouse` on every field; `Off` is raw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assist {
    /// Half-angle about the raw aim inside which a hostile unit is a candidate.
    pub cone: Angle,
    /// Within this of the best candidate's bearing, the cast takes the bearing exactly.
    pub snap: Angle,
    /// Otherwise the raw aim moves this much of the way toward it.
    pub magnet: Permille,
    /// How long the chosen unit stays sticky (`PlayerState.assist`).
    pub sticky_ticks: Tick,
    /// The sticky unit stays a candidate this much outside the cone.
    pub slack: Angle,
}

pub const ASSIST_PAD: Assist = Assist {
    cone: Angle::from_degrees(20),
    snap: Angle::from_degrees(4),
    magnet: Permille(350),
    sticky_ticks: Tick(30),
    slack: Angle::from_degrees(5),
};

pub const ASSIST_MOUSE: Assist = Assist {
    cone: Angle::from_degrees(8),
    snap: Angle::from_degrees(2),
    magnet: Permille(200),
    sticky_ticks: Tick(30),
    slack: Angle::from_degrees(2),
};

/// Score bonuses, as angles taken off a candidate's distance from the raw aim: the caster's own
/// target, and (smaller) the sticky unit.
pub const ASSIST_TARGET_BONUS: i32 = Angle::from_degrees(2).0 as i32;
pub const ASSIST_STICKY_BONUS: i32 = Angle::from_degrees(1).0 as i32;

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
/// A world spell (Repair, Grow) finds the nearest answering prop within 2 m.
pub const WORLD_SPELL_REACH_FX: i32 = 2 * CELL_FX;
/// Using an item roots her for half a second.
pub const ITEM_STOP: Tick = Tick(30);
/// Plates are looked at every this many ticks (`triggers.ts PLATE_PERIOD`).
pub const PLATE_PERIOD: u32 = 6;
/// Journal entries kept per kind (ARCHITECTURE.md §12).
pub fn journal_ring() -> u32 {
    u32::from(jane_data::catalog().living.tuning.journal_ring)
}

// --- controllers (`ai.ts`, `snake.ts`, `sim.ts stepDayOnly`) ----------------------------------

/// An idle creature looks about it every this many ticks, staggered by `think_offset`.
pub const AGGRO_PERIOD: u32 = 10;
/// Night reach, tenths: aggro `* (10 + 4 * dark) / 10`, leash `* (10 + 6 * dark) / 10` (§2).
pub const NIGHT_AGGRO: i32 = 4;
pub const NIGHT_LEASH: i32 = 6;
/// By day (six to the bell) a county creature at this threat or under starts no fight: the
/// Lowfields' own ground leaves her be until nine (PLAN.md §2.6, `ai::wary`).
pub const WARY_THREAT: u8 = 1;
/// A creature at this multiple of its row's strength (the phase table's threat 4) is deep
/// county: the night counts twice for it.
pub const WORKS_SCALE: u16 = PHASE_SCALE[4] as u16;
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
/// A patrol plans at most this far in one go, in cells (`ai.ts patrol`, 200 m).
pub const PATROL_PATH_CELLS: u32 = 200;
/// A patrol point is reached within a cell (a snake's within a cell and a half).
pub const PATROL_REACHED_FX: i32 = CELL_FX;
pub const SNAKE_PATROL_REACHED_FX: i32 = CELL_FX * 3 / 2;
/// A leashing creature steps the last of the way home in one move when this close, or within
/// its own run speed (`Math.max(run, 1.5)`).
pub const LEASH_SNAP_FX: i32 = FX_ONE * 3 / 2;
/// A leash's path may be this many times the leash long; a chase's twice it (`ai.ts`).
pub const LEASH_PATH_TIMES: i32 = 4;
pub const CHASE_PATH_TIMES: i32 = 2;
/// A creature that cannot resist its bait eats it within 16 px, and it is poisoned: 10 000
/// nature, from nobody (`ai.ts seekBait`).
pub const BAIT_EAT_FX: i32 = 16 * FX_ONE;
pub const BAIT_HIT: Milli = Milli(10_000_000);
/// It smells its bait from this many times as far as it notices anyone (its aggro): the meat is
/// thrown from outside its notice, or it could never be fed at all (DUNGEONS.md §3.5).
pub const BAIT_NOSE_TIMES: i64 = 2;
/// A walker whose way is blocked, or lit, plans again within this many ticks.
pub const REPATH_SOON: Tick = Tick(4);
/// The snake: its phase clock (`snake.ts SNAKE_FOLLOW_TICKS`, `SNAKE_SPIT_TICKS`), and a trail
/// point every this many moving ticks.
pub const SNAKE_FOLLOW_TICKS: u32 = 900;
pub const SNAKE_SPIT_TICKS: u32 = 300;
pub const SNAKE_NODE_EVERY: u16 = 4;
/// It coils at home within 16 px before it spits.
pub const SNAKE_HOME_FX: i32 = 16 * FX_ONE;
/// Its turn per tick is `speed x 4` degrees for a speed in px per tick: with `speed` in `Fx`,
/// `speed * 4 * 65 536 / (360 * 256)` angle units (0.75 px a tick turns 3°, 1.2 turns 4.8°).
pub const SNAKE_TURN_NUM: i32 = 4 * 65_536;
pub const SNAKE_TURN_DEN: i32 = 360 * FX_ONE;
/// Presence (§4.6.a): nothing appears, vanishes or jumps while a seat here stands within this
/// box of it (120 x 80 px each way, `sim.ts stepDayOnly`).
pub const WATCH_X_FX: i32 = 120 * FX_ONE;
pub const WATCH_Y_FX: i32 = 80 * FX_ONE;
/// A unit shown again where something solid now stands comes back on the nearest free cell
/// within this many.
pub const PRESENCE_NUDGE_RADIUS: i32 = 6;
