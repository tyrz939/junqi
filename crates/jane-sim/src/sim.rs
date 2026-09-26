//! The scheduler (ARCHITECTURE.md §4, `sim/sim.ts`). `Sim::step` is the only way time or state
//! moves; the order is written down in [`Sim::step`] and nothing is implicit.

use std::collections::BTreeMap;
use std::sync::Arc;

use jane_core::angle::along;
use jane_core::num::mul_div_floor;
use jane_core::{Blueprint, Fx, Sfc32, Sym, Tick, UnitDefId, ZONE_COUNT, ZoneId};

use crate::actions::{Subject, run_actions};
use crate::blueprints::Blueprints;
use crate::ctx::{Ctx, PartySnap, Scratch, WorldOp, WorldOps, ZoneOps, forget_unit, none_per_zone};
use crate::event::{Event, EventKind};
use crate::fog::stamp_fog;
use crate::ids::Seat;
use crate::input::{InputFrame, StepInput, Stepped};
use crate::ring::{Watchers, step_ring, wake_props};
use crate::runtime::{ZoneRuntime, find_locals};
use crate::state::{Bits, GameState, Growth, Journal, Quests, SAVE_VERSION};
use crate::sym::SymTable;
use crate::tuning::{
    ENERGY_CARRY, ENERGY_REGEN, ENERGY_SPRINT, FOG_EVERY, MAX_PLAYERS, MOVE_DEADZONE, PLAYER_RESPAWN, START_HOUR,
    TICKS_PER_DAY, TICKS_PER_HOUR,
};
use crate::units::{def_of, face_angle, move_unit, restore_energy, spend_energy};
use crate::zone::create_zone_state;
use crate::{clear, interact, triggers};

/// The simulation: the authoritative state, the blueprints it was built from, and every derived
/// runtime and buffer. Only `state` is saved and hashed.
#[derive(Debug)]
pub struct Sim {
    pub(crate) state: GameState,
    pub(crate) bps: Blueprints,
    /// Runtimes of live zones (and, for a step, of a zone a clock row runs in).
    pub(crate) rts: [Option<Box<ZoneRuntime>>; ZONE_COUNT],
    pub(crate) scratch: Scratch,
    pub(crate) ops: ZoneOps,
    pub(crate) wops: WorldOps,
    pub(crate) events: Vec<Event>,
    drained: Vec<Event>,
    /// `"start"`, the mark every zone falls back on.
    pub(crate) start_sym: Sym,
    /// The heroine's unit row.
    pub(crate) jane: UnitDefId,
    /// §8 `awake_only_equals_everyone`: tick every unit the old way (regen paid to every unit
    /// every tick, the controllers' pass over every unit asking whether it is awake). A test
    /// switch; never saved, and it must never change a hash.
    everyone: bool,
}

impl Sim {
    /// A new world: every blueprint built for `seed`, and the host sat down (the `Join { host }`
    /// of frame 0; no step is taken). `name` is the heroine's, chosen by the host.
    ///
    /// Panics if a zone does not build, which worldgen guarantees never happens.
    pub fn new_game(seed: u32, name: &str) -> Sim {
        let bps = Blueprints::build(seed).unwrap_or_else(|e| panic!("new game, seed {seed}: {e}"));
        Self::new_game_with(bps, name)
    }

    /// The same over blueprints already built (tests, benches and peers share one set).
    pub fn new_game_with(bps: Blueprints, name: &str) -> Sim {
        let seed = bps.seed();
        let state = GameState {
            version: SAVE_VERSION,
            seed,
            frame: 0,
            tick: Tick::ZERO,
            clock: START_HOUR * TICKS_PER_HOUR,
            day: 0,
            name: name.to_owned(),
            open: false,
            next: crate::ids::Counters::default(),
            rng: Sfc32::seeded(seed, 1),
            players: Vec::with_capacity(MAX_PLAYERS),
            zones: none_per_zone(),
            flags: BTreeMap::new(),
            quests: Quests::default(),
            rest: None,
            growth: Growth::default(),
            syms: SymTable::default(),
            journal: Journal::default(),
            weather: crate::living::first_skies(),
            consequences_done: Bits::new(jane_data::catalog().living.consequences.len() as u32),
            consequences_owed: Vec::new(),
            rumours: BTreeMap::new(),
        };
        let mut sim = Self::adopt(state, bps);
        sim.join(crate::ids::ClientToken::HOST);
        sim
    }

    pub(crate) fn adopt(state: GameState, bps: Blueprints) -> Sim {
        let cat = jane_data::catalog();
        Sim {
            state,
            bps,
            rts: none_per_zone(),
            scratch: Scratch::default(),
            ops: ZoneOps::default(),
            wops: WorldOps::default(),
            events: Vec::with_capacity(64),
            drained: Vec::with_capacity(64),
            start_sym: crate::sym::of_name(cat.name_id("start").expect("the catalog names \"start\"")),
            jane: cat.combat.unit_id("jane").expect("units.json has \"jane\""),
            everyone: false,
        }
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// The state, to change outside a step: tests and tools only. What is changed here is in no
    /// tape, and a change to something a runtime derives from (a position, a prop) must be
    /// followed by [`rebuild_runtimes`](Self::rebuild_runtimes).
    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    pub fn blueprints(&self) -> &Blueprints {
        &self.bps
    }

    pub fn blueprint(&self, z: ZoneId) -> &Arc<Blueprint> {
        self.bps.get(z)
    }

    /// §8 `awake_only_equals_everyone`: with `on`, every unit is ticked the old way (see the
    /// field). For that test alone.
    #[doc(hidden)]
    pub fn tick_everyone(&mut self, on: bool) {
        self.everyone = on;
    }

    /// Pay every unit of every zone its regen to now (§4.3). Invisible to every future tick:
    /// regen paid early is the regen paid late, which is the rule this proves; only `synced`,
    /// the bookkeeping, moves. `awake_only_equals_everyone` settles both sims before it
    /// compares them, since the old way keeps every sleeper's `synced` current.
    #[doc(hidden)]
    pub fn settle(&mut self) {
        let now = self.state.tick;
        for zs in self.state.zones.iter_mut().flatten() {
            for u in &mut zs.units {
                crate::life::pay_regen(u, now);
            }
        }
    }

    /// The path finder's counters (derived; reset by a runtime rebuild).
    pub fn path_stats(&self) -> crate::path::PathStats {
        self.scratch.path.stats
    }

    /// A live zone's runtime.
    pub fn runtime(&self, z: ZoneId) -> Option<&ZoneRuntime> {
        self.rts[z.index()].as_deref()
    }

    /// Alone, the world holds still while she talks. With company, nothing pauses.
    pub fn frozen(&self) -> bool {
        let mut n = 0;
        let mut talking = false;
        for p in self.state.connected() {
            n += 1;
            talking = p.dialogue.is_some();
        }
        n == 1 && talking
    }

    /// Everything emitted since the last drain.
    pub fn drain_events(&mut self) -> &[Event] {
        std::mem::swap(&mut self.events, &mut self.drained);
        self.events.clear();
        &self.drained
    }

    // --- zones -------------------------------------------------------------------------

    /// Make the zone's state if this is its first visit. Returns whether it was.
    pub(crate) fn ensure_zone(&mut self, z: ZoneId) -> bool {
        if self.state.zones[z.index()].is_some() {
            return false;
        }
        let bp = Arc::clone(self.bps.get(z));
        let zs = create_zone_state(&mut self.state, &bp);
        self.state.zones[z.index()] = Some(Box::new(zs));
        true
    }

    /// Build the zone's runtime if it has none. The zone's state must exist and be in place.
    pub(crate) fn ensure_runtime(&mut self, z: ZoneId) {
        if self.rts[z.index()].is_some() {
            return;
        }
        let bp = self.bps.get(z);
        let zone = self.state.zones[z.index()].as_deref().expect("a runtime is built over a zone's state");
        let locals = find_locals(&self.state.syms, bp);
        let mut rt = ZoneRuntime::build(bp, zone, locals);
        wake_props(zone, &mut rt, &mut self.scratch.props);
        self.rts[z.index()] = Some(Box::new(rt));
    }

    /// Drop the runtimes of zones nobody is in (step 15).
    pub(crate) fn drop_empty(&mut self) {
        for z in ZoneId::ALL {
            if self.rts[z.index()].is_some() && !self.state.is_live(z) {
                self.rts[z.index()] = None;
            }
        }
    }

    /// Throw every runtime away and rebuild those of live zones. Invisible to every future
    /// tick (§8 `runtime_rebuild_is_invisible`); tests call it mid-run.
    pub fn rebuild_runtimes(&mut self) {
        self.rts = none_per_zone();
        self.scratch = Scratch::default();
        for z in ZoneId::ALL {
            if self.state.is_live(z) {
                self.ensure_runtime(z);
            }
        }
    }

    /// Run `f` in zone `z`'s context, making its state and runtime if need be. The zone is
    /// taken out of `state.zones` for the length of the call; its `ZoneOps` land before it is
    /// put back (step 13) and the `WorldOps` are drained after.
    pub(crate) fn with_ctx<R>(
        &mut self,
        z: ZoneId,
        actor: Option<Seat>,
        snap: &PartySnap,
        broadcast: bool,
        f: impl FnOnce(&mut Ctx<'_>) -> R,
    ) -> R {
        self.ensure_zone(z);
        self.ensure_runtime(z);
        let mut zone = self.state.zones[z.index()].take().expect("zone state");
        let rt = self.rts[z.index()].as_deref_mut().expect("runtime");
        let bp: &Blueprint = self.bps.get(z);
        let r = {
            let mut cx = Ctx {
                cat: jane_data::catalog(),
                world: &mut self.state,
                zone: &mut zone,
                rt,
                bp,
                party: snap,
                actor,
                ops: &mut self.ops,
                wops: &mut self.wops,
                events: &mut self.events,
                scratch: &mut self.scratch,
                broadcast,
            };
            let r = f(&mut cx);
            apply_zone_ops(&mut cx);
            r
        };
        self.state.zones[z.index()] = Some(zone);
        self.drain_world_ops();
        r
    }

    /// In the order they were queued. A reward list may queue more (a quest given, a jar found):
    /// those land after the ones already waiting.
    fn drain_world_ops(&mut self) {
        while !self.wops.ops.is_empty() {
            let op = self.wops.ops.remove(0);
            match op {
                WorldOp::Announce(kind) => self.events.push(Event { to: None, in_zone: None, kind }),
                // Everyone in the party is paid, wherever she stands, once each, with her as the
                // actor: nobody is left without the house key because a friend did the talking.
                WorldOp::PayRewards { list, .. } => {
                    for i in 0..self.state.players.len() {
                        let p = &self.state.players[i];
                        if !p.connected {
                            continue;
                        }
                        let (zone, unit, seat) = (p.zone, p.unit, p.seat);
                        let snap = PartySnap::of(&self.state);
                        self.with_ctx(zone, Some(seat), &snap, false, |cx| {
                            run_actions(cx, list, Subject::Unit(unit));
                        });
                    }
                }
                WorldOp::Grow { stat, amount } => crate::verbs::grow_bodies(&mut self.state, stat, amount),
                WorldOp::Teach(spell) => {
                    if crate::combat::teach(&mut self.state, spell) {
                        for kind in crate::combat::learned_events(spell) {
                            self.events.push(Event { to: None, in_zone: None, kind });
                        }
                    }
                }
            }
        }
    }

    // --- the tick ------------------------------------------------------------------------

    /// One frame. Always applies the commands; advances time unless frozen.
    ///
    /// ```text
    ///  0 commands      (seat, seq) order; Join / Leave / Open are world level
    ///  1 freeze        frozen: stop here
    ///  2 clock         tick, clock, day; on the hour the world rolls, then clock rows once,
    ///                  actor None; the rain ramp, consequences, rumours, owed edits (living)
    ///    for each live zone in ZoneId order, taken out of state.zones:
    ///  3 presence      every 30 ticks: schedules, dayOnly, nightOnly
    ///  4 ring          on a seat's block change
    ///  5 catch-up      units that woke: regen, pulses        (combat and status units)
    ///  6 players       seat order: input, energy, movement   (hold-to-push, carry: interact)
    ///  7 controllers   ai | snake | npc (orders, patrols)
    ///  8 projectiles   and grounds                           (combat unit)
    ///  9 statuses                                            (status unit)
    /// 10 flush x2      the only place hp changes             (combat unit)
    /// 11 triggers      and plates
    /// 12 housekeeping  prop flags, fills owed, fog every 10   (drops: loot unit)
    /// 13 zone ops      spawn, despawn, wake; the zone goes back; world ops drain
    /// 14 travel        seat order
    /// 15 drop          runtimes of zones nobody is in
    /// ```
    pub fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        // 0
        debug_assert!(input.commands.is_sorted_by_key(|c| (c.seat, c.seq)), "commands in (seat, seq) order");
        for c in input.commands {
            self.command(c, &input.frames);
        }
        // 1
        if self.frozen() {
            // Only the console can deal a blow while she reads alone; a step that does not run
            // does not land it.
            self.clear_hits();
            self.drop_empty();
            self.state.frame += 1;
            return Stepped { ran: false };
        }
        // 2
        self.step_clock();
        let snap = PartySnap::of(&self.state);
        for z in ZoneId::ALL {
            if self.state.is_live(z) {
                self.step_zone(z, input, &snap);
            }
        }
        // 14
        for seat in 0..self.state.players.len() {
            let p = &self.state.players[seat];
            if p.connected && p.travel.is_some() {
                self.perform_travel(Seat(seat as u8));
            }
        }
        // 15
        self.drop_empty();
        // A blow queued on a zone that did not tick this step (its last seat left it this frame)
        // is dropped with the room: the queue is empty between steps.
        self.clear_hits();
        self.state.frame += 1;
        Stepped { ran: true }
    }

    fn clear_hits(&mut self) {
        for q in &mut self.scratch.hits {
            q.clear();
        }
    }

    /// Queue a blow on a unit in zone `z`; it lands at the next step's flush. Tests and tools
    /// only (the console's `Dev` commands are how a tape deals one): nothing here is in a tape.
    /// The queue lives in the scratch, so [`rebuild_runtimes`](Self::rebuild_runtimes) empties
    /// it: queue after any edit that rebuilds.
    pub fn queue_hit(&mut self, z: ZoneId, hit: crate::combat::Hit) {
        self.scratch.hits[z.index()].push(hit);
    }

    fn step_clock(&mut self) {
        let s = &mut self.state;
        s.tick = Tick(s.tick.0 + 1);
        s.clock += 1;
        if s.clock >= TICKS_PER_DAY {
            s.clock = 0;
            s.day += 1;
        }
        if s.clock % TICKS_PER_HOUR == 0 {
            let hour = (s.clock / TICKS_PER_HOUR) as u8;
            // The world stream, once an hour, in its fixed order: the sky of every region, then
            // every area's ecology (§4.4, `living.rs`).
            self.world_rolls();
            // Clock rows run once, with no actor, in the county (the rows name county things);
            // what they say is heard by the whole party. The county's runtime is made for them if
            // nobody is there, and dropped again at step 15.
            let cat = jane_data::catalog();
            if cat.story.clock_at(hour).next().is_some() {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(ZoneId::County, None, &snap, true, |cx| {
                    for row in cat.story.clock_at(hour) {
                        run_actions(cx, row.actions, Subject::None);
                    }
                });
            }
        }
        // Every tick: the rain ramp, consequences that fire, rumours that start, edits owed to a
        // zone that is live now (§4.6, `living.rs`).
        self.step_living();
    }

    fn step_zone(&mut self, z: ZoneId, input: &StepInput<'_>, snap: &PartySnap) {
        let everyone = self.everyone;
        self.with_ctx(z, None, snap, false, |cx| {
            cx.rt.paths_this_tick = 0;
            // 3 presence: schedules (dayOnly and nightOnly are two-slot schedules); nothing
            // appears, vanishes or jumps inside a watcher's box.
            crate::presence::step_presence(cx);

            // 4 ring
            let w = Watchers::of(cx.world, z, cx.zone);
            step_ring(cx.zone, cx.rt, &w, false, &mut cx.scratch.props);

            // 5 catch-up: every awake unit's regen paid to now (a unit that woke this tick
            // catches up here, its phase reset with it); its missed pulses land at step 9.
            if everyone {
                let now = cx.world.tick;
                for u in &mut cx.zone.units {
                    crate::life::pay_regen(u, now);
                }
            } else {
                crate::life::pay_awake(cx);
            }

            // Where everything awake stood before anyone moved (View's `prev_pos`).
            cx.rt.prev_pos.clear();
            for &id in &cx.rt.awake_units {
                if let Some(u) = cx.zone.unit(id) {
                    cx.rt.prev_pos.push((id, u.pos));
                }
            }

            // 6 players, in seat order
            for seat in 0..cx.world.players.len() {
                let p = &cx.world.players[seat];
                if p.connected && p.zone == z {
                    cx.actor = Some(Seat(seat as u8));
                    tick_player(cx, seat, input.frames.get(seat).copied().unwrap_or(InputFrame::IDLE));
                    cx.actor = None;
                }
            }

            // 7 controllers, over the awake_units snapshot: ai | snake | npc; stunned skip.
            crate::ai::step_controllers(cx, everyone);

            // 8 projectiles and grounds
            crate::flight::step_projectiles(cx);
            crate::flight::step_grounds(cx);
            // 9 statuses
            crate::status::step_statuses(cx);
            // 10 flush: the only place a blow changes hp
            crate::flush::flush(cx);

            // 11 triggers, and plates every 6 ticks
            triggers::step_triggers(cx);

            // 12 housekeeping: drops expire, corpses due stand up; prop flags are re-stamped over
            // what changed; the fills still owed land if their rect is clear; fog every 10.
            crate::loot::step_drops(cx);
            crate::life::respawn_due(cx);
            cx.rt.flush_prop_flags(cx.zone, &mut cx.scratch.props);
            clear::step_pending_fill(cx);
            if cx.world.tick.0 % FOG_EVERY == 0 {
                stamp_seats_fog(cx);
            }
        });
        // 13: `with_ctx` applied the zone ops and put the zone back.
    }
}

/// Step 6 for one seat (`sim.ts tickPlayer`).
fn tick_player(cx: &mut Ctx<'_>, seat: usize, frame: InputFrame) {
    let tick = cx.world.tick;
    let p = &cx.world.players[seat];
    let (busy, god, respawn_at, body) = (p.dialogue.is_some(), p.god, p.respawn_at, p.unit);
    let Some(ix) = cx.zone.unit_ix(body) else { return };
    if !cx.zone.units[ix].alive {
        // She lies until `respawn_at` (the flush set it when she fell), then wakes.
        match respawn_at {
            None => cx.world.players[seat].respawn_at = Some(tick.after(PLAYER_RESPAWN)),
            Some(at) if tick >= at => crate::life::revive_player(cx, Seat(seat as u8)),
            Some(_) => {}
        }
        return;
    }
    // With company the world does not stop for a conversation, but she does.
    let stunned = crate::status::is_stunned(&cx.zone.units[ix], tick);
    let mag = if busy { 0 } else { frame.mv_mag.min(127) };
    let wants_move = mag > MOVE_DEADZONE && tick >= cx.zone.units[ix].stop_until && !stunned;
    let held = frame.use_held && !busy;
    // Held against a pushable she is braced: she leans into it (or pulls it) instead of walking.
    let braced = if held && !stunned {
        let (mx, my) = interact::stick_axes(frame.mv_dir, mag);
        interact::hold_use(cx, body, mx, my)
    } else {
        false
    };
    let u = &mut cx.zone.units[ix];
    if !held {
        u.hold = 0;
    }
    let mut sprinting = false;
    if wants_move && !braced {
        let def = def_of(u);
        sprinting = frame.sprint && !u.energy_locked && u.energy.0 > 0 && u.carrying.is_none();
        let mut speed = if sprinting { def.run.0 } else { def.walk.0 };
        speed = speed * crate::status::speed_factor(u, tick) / 1000;
        if god {
            speed *= 2;
        }
        let dist = mul_div_floor(speed, i32::from(mag), 127);
        face_angle(u, frame.mv_dir);
        let d = along(frame.mv_dir, Fx(dist));
        move_unit(cx.rt, u, d.x, d.y);
    }
    // Energy: sprint spends, carrying spends, anything else (walking included) restores.
    if sprinting {
        spend_energy(u, ENERGY_SPRINT);
    } else if u.carrying.is_some() {
        spend_energy(u, ENERGY_CARRY);
        // Arms give out: she puts it down.
        if u.energy.0 == 0 {
            interact::put_down(cx, body);
        }
    } else {
        restore_energy(u, ENERGY_REGEN);
    }
}

/// Stamp the fog round every seat in the ctx's zone.
pub(crate) fn stamp_seats_fog(cx: &mut Ctx<'_>) {
    let z = cx.zone.id;
    for p in cx.world.players.iter().filter(|p| p.connected && p.zone == z) {
        if let Some(pos) = cx.zone.unit(p.unit).map(|u| u.pos) {
            stamp_fog(&mut cx.zone.fog, cx.rt.fog, cx.bp.indoor, pos);
        }
    }
}

/// Step 13: what lists did to `zone.units`, applied now that nothing iterates it.
fn apply_zone_ops(cx: &mut Ctx<'_>) {
    if cx.ops.is_empty() {
        return;
    }
    for i in 0..cx.ops.despawn.len() {
        let id = cx.ops.despawn[i];
        forget_unit(cx.zone, cx.rt, cx.party, id);
        if let Some(u) = cx.zone.remove_unit(id) {
            cx.rt.remove_unit(&u);
            cx.events.push(Event { to: None, in_zone: Some(cx.zone.id), kind: EventKind::Despawned { unit: id } });
        }
    }
    cx.ops.despawn.clear();
    for mut u in cx.ops.spawn.drain(..) {
        // Awake as it lands (`createUnit`); the ring puts it to sleep when it next runs.
        u.awake = true;
        let id = u.id;
        let at = cx.zone.insert_unit(u);
        cx.rt.add_unit(&cx.zone.units[at]);
        cx.events.push(Event { to: None, in_zone: Some(cx.zone.id), kind: EventKind::Spawned { unit: id } });
    }
    for i in 0..cx.ops.wake.len() {
        let id = cx.ops.wake[i];
        if let Some(u) = cx.zone.unit_mut(id) {
            cx.rt.set_awake(u, true);
        }
    }
    cx.ops.wake.clear();
}
