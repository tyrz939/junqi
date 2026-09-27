//! What the last step cost, for a debug overlay and a profiler (ARCHITECTURE.md §11,
//! `Sim::metrics`): wall time per numbered phase of [`Sim::step`](crate::Sim::step), how many
//! units were awake, how many paths were searched, how many events were said.
//!
//! **Nothing here is state.** The counts are read off the runtime after the step; the times come
//! from a wall clock the presentation lends the sim ([`Sim::set_wall_clock`](crate::Sim::set_wall_clock)),
//! since wall time never reaches a deterministic crate on its own (`clippy.toml`). Without one
//! every time reads 0 and the counts are still kept. None of it is saved, hashed, or read by
//! anything that moves the state: a sim with a clock and one without step to the same hash
//! (`tests/metrics.rs`).

/// A wall clock: monotonic nanoseconds from any origin (`Instant::now()` against a fixed start,
/// in the presentation). A plain `fn` so it is `Copy` and holds nothing.
pub type WallClock = fn() -> u64;

/// The numbered phases of [`Sim::step`](crate::Sim::step), in its doc comment's order and with
/// its names. A zone phase (3 to 12) is summed over the live zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Phase {
    /// 0: the commands, and a bed's night chosen by one.
    Commands,
    /// 2: tick, clock, day; the world's rolls; clock rows; the living world.
    Clock,
    /// 3: schedules, dayOnly, nightOnly.
    Presence,
    /// 4: the load ring.
    Ring,
    /// 5: regen and pulses owed to units that woke.
    CatchUp,
    /// 6: every seat's input, energy and movement.
    Players,
    /// 7: ai, snake, npc.
    Controllers,
    /// 8: bolts and grounds.
    Projectiles,
    /// 9: statuses.
    Statuses,
    /// 10: the flush, where hp changes.
    Flush,
    /// 11: triggers and plates.
    Triggers,
    /// 12: drops, respawns, prop flags, fills owed, fog.
    Housekeeping,
    /// 13: zone ops (spawn, despawn, wake), the zone put back, world ops drained.
    ZoneOps,
    /// 14: travel.
    Travel,
    /// The sleep after the commands, the skies told, and 15: runtimes of empty zones dropped.
    Drop,
}

/// How many [`Phase`]s there are.
pub const PHASES: usize = 15;

impl Phase {
    pub const ALL: [Phase; PHASES] = [
        Phase::Commands,
        Phase::Clock,
        Phase::Presence,
        Phase::Ring,
        Phase::CatchUp,
        Phase::Players,
        Phase::Controllers,
        Phase::Projectiles,
        Phase::Statuses,
        Phase::Flush,
        Phase::Triggers,
        Phase::Housekeeping,
        Phase::ZoneOps,
        Phase::Travel,
        Phase::Drop,
    ];

    /// The step doc comment's name for it.
    pub const fn name(self) -> &'static str {
        match self {
            Phase::Commands => "commands",
            Phase::Clock => "clock",
            Phase::Presence => "presence",
            Phase::Ring => "ring",
            Phase::CatchUp => "catch-up",
            Phase::Players => "players",
            Phase::Controllers => "controllers",
            Phase::Projectiles => "projectiles",
            Phase::Statuses => "statuses",
            Phase::Flush => "flush",
            Phase::Triggers => "triggers",
            Phase::Housekeeping => "housekeeping",
            Phase::ZoneOps => "zone ops",
            Phase::Travel => "travel",
            Phase::Drop => "drop",
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The last step, measured. `Copy`; the shape is stable: add fields at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimMetrics {
    /// The frame the step ran (`GameState::frame` before it).
    pub frame: u32,
    /// Did time move (false: frozen at a conversation alone)?
    pub ran: bool,
    /// Nanoseconds for the whole step (0 without a wall clock).
    pub step_ns: u32,
    /// Nanoseconds per [`Phase`], indexed by [`Phase::index`] (0 without a wall clock).
    pub phase_ns: [u32; PHASES],
    /// Zones stepped.
    pub zones_live: u32,
    /// Units awake, and all units, over the live zones after the step.
    pub units_awake: u32,
    pub units_total: u32,
    /// Path searches this step (every controller's ask that reached the search), and the nodes
    /// they expanded.
    pub path_searches: u32,
    pub path_expanded: u32,
    /// Events said this step.
    pub events: u32,
}

impl SimMetrics {
    /// Microseconds for the whole step.
    pub const fn step_us(&self) -> u32 {
        self.step_ns / 1000
    }

    /// Microseconds for one phase.
    pub const fn phase_us(&self, p: Phase) -> u32 {
        self.phase_ns[p.index()] / 1000
    }
}

/// A running stopwatch over one step: `lap(phase)` charges the time since the last lap.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Laps {
    clock: Option<WallClock>,
    start: u64,
    last: u64,
}

impl Laps {
    pub(crate) fn start(clock: Option<WallClock>) -> Laps {
        let now = clock.map_or(0, |c| c());
        Laps { clock, start: now, last: now }
    }

    /// Charge the time since the last lap to `p`.
    pub(crate) fn lap(&mut self, m: &mut SimMetrics, p: Phase) {
        if let Some(c) = self.clock {
            let now = c();
            let d = now.saturating_sub(self.last);
            m.phase_ns[p.index()] = m.phase_ns[p.index()].saturating_add(d.min(u64::from(u32::MAX)) as u32);
            self.last = now;
        }
    }

    /// The whole step, from the start.
    pub(crate) fn total(&self) -> u32 {
        self.clock.map_or(0, |c| c().saturating_sub(self.start).min(u64::from(u32::MAX)) as u32)
    }
}
