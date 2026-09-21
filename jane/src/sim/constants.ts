// Engine constants. 2020 ran a 60 fps frame-stepped GameMaker loop and every
// number in it was "per frame". The sim keeps that clock: one tick = 1/60 s,
// all timers are integer ticks, so nothing drifts and nothing depends on the
// display's refresh rate. Data files speak seconds; the catalog converts once.

export const TICK_RATE = 60;
export const TICK_SECONDS = 1 / TICK_RATE;

/** Path / collision / occupancy cell, in pixels. One value, everywhere. */
export const CELL = 8;

/** Combat talks in metres, path talks in pixels. 2020: one metre is one path cell. */
export const PX_PER_METRE = 8;

export const BAG_SLOTS = 24;
export const BAR_SLOTS = 8;
export const CRAFT_INPUTS = 3;

/** Global cooldown: 1.5 s. */
export const GCD_TICKS = 90;

/** `CreatureStatCalc`: maxhp = strength * 5, maxmp = spirit * 5. */
export const HP_PER_STRENGTH = 5;
export const MP_PER_SPIRIT = 5;

/** 1-in-20 hits double. */
export const CRIT_ONE_IN = 20;

// Energy, per tick, straight from obj_player. Empty locks sprint until full again.
export const ENERGY_MAX = 100;
export const ENERGY_SPRINT = 0.5;
export const ENERGY_CARRY = 0.25;
export const ENERGY_REGEN = 0.5;

/** Idle AI regen: max/300 per tick when `autoRegen`. */
export const REGEN_DIVISOR = 300;

/** Aggro scan cadence. 2020: every 10 frames, staggered with irandom(9). */
export const AGGRO_PERIOD = 10;

/** Day clock: `time += 1/7200` hours per tick -> one game hour is 2 real minutes. */
export const TICKS_PER_HOUR = 7200;
export const TICKS_PER_DAY = TICKS_PER_HOUR * 24;

/** Default respawn: 36000 frames = 10 minutes. */
export const RESPAWN_TICKS = 36000;

// Load ring, from objects/distance_unload: block = 2 cells, ring = 24 blocks each way.
export const RING_BLOCK = CELL * 2;
export const RING_RADIUS = 24 * RING_BLOCK;

/** Toasts: at most 3 stacked, 180 ticks each. */
export const TOAST_MAX = 3;
export const TOAST_TICKS = 180;

/** Unit body: a box centred on the feet, smaller than a cell so 1-cell corridors work. */
export const BODY_HALF = 3;

/** Interact reach in pixels, measured from the feet along facing. */
export const USE_REACH = 12;

/** Path search limits. */
export const PATH_BUDGET = 6000;
export const PATHS_PER_TICK = 4;
export const REPATH_TICKS = 20;

/**
 * Night, for the sim: 21:00 to 06:00. "By day Castle is the town that was. By night
 * it is what it became." The dog is not seen in it. (Lamp posts light earlier, at
 * 18:30, as they did in 2020; that is presentation.)
 */
export const NIGHT_START_HOUR = 21;
export const NIGHT_END_HOUR = 6;

/**
 * Co-op does not rebalance the world; it weakens the people in it. The penalty is set
 * by how many are CONNECTED, not by who is standing next to you, so a party that
 * splits up is very weak until it regroups. That is by design.
 *
 *   players   each deals   party together   each takes
 *      1        100%          100%            100%
 *      2         62%          124%            115%
 *      3         46%          138%            130%
 *      4         38%          152%            145%
 *
 * Together is a little better than alone. Alone, in a party of four, is 38%.
 * Healing is never scaled.
 */
export const PARTY_DEALT = [1, 0.62, 0.46, 0.38];
export const PARTY_TAKEN = [1, 1.15, 1.3, 1.45];

/**
 * 2: playerName, rest point, Unit.hidden, Unit.pathGoal. 3: players[] (co-op-ready). 7: the mine is
 * generated; a saved hand-built one is dropped. 8: units may be sent somewhere and may wait at a
 * patrol point (Unit.order, patrolDwell, dwell); a zone remembers the cells a fill is still owed.
 */
export const SAVE_VERSION = 8;

/**
 * What a creature costs at each threat, as a multiple of its row. This IS 2020's balance
 * sheet (DESIGN-2020.md 3.1) read as a curve: enemy health 100, 200, 300, 400, 600, 800
 * across the six phases is 1, 2, 3, 4, 6, 8 times the phase-1 row, and the rows are written
 * at phase 1. Index 0 is unused (nothing spawns in a haven).
 *
 * The old table (1, 1.7, 2.8, 4.2, 6, 8) rounded the sheet's middle DOWN, which made the
 * near county and the middle county feel like the same place. Nothing is invented here: the
 * numbers are the column.
 */
export const PHASE_SCALE: readonly number[] = [1, 1, 2, 3, 4, 6, 8];

/**
 * Night, for a creature standing outside lamplight (PLAN.md 2.6: "+1 everywhere outside
 * lamplight, +2 in the Works"). Spawns carry their threat in their strength, not in a field
 * the sim can read, so the night is paid in ATTENTION rather than in a phase: a thing notices
 * further off and follows much further before it gives up. In the deep county it is worth
 * double, which is the "+2 in the Works" of the same rule.
 */
export const NIGHT_AGGRO = 0.4;
export const NIGHT_LEASH = 0.6;
