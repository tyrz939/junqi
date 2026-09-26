//! The context every system takes (ARCHITECTURE.md §5.2): one live zone, the world, and
//! optionally the seat a call is made for. The old sim's import cycles become modules sharing
//! this. A zone is taken out of `GameState.zones` for the length of its ctx, so `world` and
//! `zone` are two plain `&mut`s.

use jane_core::{Blueprint, Key, Sym, Vec2, ZONE_COUNT, ZoneId};
use jane_data::Catalog;

use crate::event::{Event, EventKind};
use crate::ids::{PropIx, Seat, UnitId};
use crate::path::PathScratch;
use crate::runtime::ZoneRuntime;
use crate::state::{CombatState, GameState, Unit, ZoneState};
use crate::sym::of_key;
use crate::tuning::MAX_PLAYERS;

/// The party as it stood at the end of the clock step: one tick stale, which is unobservable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartySnap {
    pub size: u8,
    pub bodies: [Option<(ZoneId, UnitId, Vec2)>; MAX_PLAYERS],
    /// Within reach of a bed or a fire. Unfilled: `Rest { until }` asks it live of every seat
    /// (`verbs::everyone_resting`), as the TS did.
    pub resting: [bool; MAX_PLAYERS],
}

impl PartySnap {
    pub fn of(state: &GameState) -> Self {
        let mut s = Self::default();
        for p in state.players.iter().filter(|p| p.connected) {
            s.size += 1;
            let pos = state.zone(p.zone).and_then(|z| z.unit(p.unit)).map(|u| u.pos);
            s.bodies[p.seat.index()] = pos.map(|pos| (p.zone, p.unit, pos));
        }
        s
    }

    /// The seat whose body this is, if a connected one.
    pub fn seat_of(&self, u: UnitId) -> Option<Seat> {
        self.bodies.iter().position(|b| b.is_some_and(|(_, id, _)| id == u)).map(|i| Seat(i as u8))
    }
}

/// What a list did to `zone.units` mid-iteration, applied in step 13 (§4.2).
#[derive(Debug, Default)]
pub struct ZoneOps {
    /// Appended in this order (ids ascend, so the list stays sorted).
    pub spawn: Vec<Unit>,
    pub despawn: Vec<UnitId>,
    pub wake: Vec<UnitId>,
}

impl ZoneOps {
    pub fn is_empty(&self) -> bool {
        self.spawn.is_empty() && self.despawn.is_empty() && self.wake.is_empty()
    }
}

/// Cross-zone work, drained in order after the zone is put back (§4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldOp {
    /// Once per connected seat in her zone, actor her (`quests::hand_in`).
    PayRewards { quest: jane_core::QuestId, list: jane_core::ListRef },
    /// The spell is the world's; every bar learns it (`verbs::teach`).
    Teach(jane_core::SpellId),
    /// Patches every body, parked included (`verbs::grow`).
    Grow { stat: jane_core::action::Stat, amount: i16 },
    /// An event for the whole party.
    Announce(EventKind),
}

#[derive(Debug, Default)]
pub struct WorldOps {
    pub ops: Vec<WorldOp>,
}

/// Per-sim scratch (§3.3 `Sim.scratch`): every temporary buffer a tick uses, `clear()`ed and
/// never dropped, so the tick allocates nothing once warm.
#[derive(Debug, Default)]
pub struct Scratch {
    pub path: PathScratch,
    pub props: Vec<PropIx>,
    pub units: Vec<UnitId>,
    /// A second prop buffer, for a search inside a loop over `props` (focus, world spells).
    pub props_b: Vec<PropIx>,
    /// Cells: the nudge search's queue (`clear.rs`).
    pub cells: Vec<(i32, i32)>,
}

pub struct Ctx<'a> {
    pub cat: &'static Catalog,
    pub world: &'a mut GameState,
    pub zone: &'a mut ZoneState,
    pub rt: &'a mut ZoneRuntime,
    pub bp: &'a Blueprint,
    pub party: &'a PartySnap,
    /// Who this call is for; by value. `None` in zone-wide steps and for clock rows.
    pub actor: Option<Seat>,
    pub ops: &'a mut ZoneOps,
    pub wops: &'a mut WorldOps,
    pub events: &'a mut Vec<Event>,
    pub scratch: &'a mut Scratch,
    /// Clock rows: what they say is heard by the whole party, wherever it stands.
    pub broadcast: bool,
}

impl std::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx").field("zone", &self.zone.id).field("actor", &self.actor).finish_non_exhaustive()
    }
}

impl Ctx<'_> {
    pub fn zone_id(&self) -> ZoneId {
        self.zone.id
    }

    /// A key of this zone's blueprint as a sym.
    pub fn sym(&self, k: Key) -> Sym {
        of_key(k, &self.rt.locals)
    }

    /// Emit an event: personal kinds to the actor, the rest to whoever is in this zone; with
    /// `broadcast`, to everyone.
    pub fn emit(&mut self, kind: EventKind) {
        let (to, in_zone) =
            if self.broadcast { (None, None) } else { (self.actor.filter(|_| kind.personal()), Some(self.zone.id)) };
        self.events.push(Event { to, in_zone, kind });
    }

    /// An event for the whole party.
    pub fn emit_all(&mut self, kind: EventKind) {
        self.events.push(Event { to: None, in_zone: None, kind });
    }

    /// The actor's body, if she has one here.
    pub fn actor_unit(&self) -> Option<UnitId> {
        let seat = self.actor?;
        let p = self.world.player(seat)?;
        (p.zone == self.zone.id && self.zone.unit(p.unit).is_some()).then_some(p.unit)
    }
}

/// The one function that lets go of a unit (§4.2): everyone's target on it is cleared, a unit
/// in combat left with no seat to fight leashes, and it leaves occupancy and the unit blocks.
/// Kill, despawn, travel and leave all call it. The unit stays in `zone.units`; the caller
/// removes it if it is going, or kills it.
pub fn forget_unit(zone: &mut ZoneState, rt: &mut ZoneRuntime, party: &PartySnap, id: UnitId) {
    for u in &mut zone.units {
        if u.id == id {
            continue;
        }
        if u.target == Some(id) {
            u.target = None;
        }
        if u.combat == CombatState::Combat && u.target.is_none_or(|t| party.seat_of(t).is_none()) {
            u.combat = CombatState::Leash;
        }
    }
    rt.leave(id);
}

/// `[Option<T>; ZONE_COUNT]` with nothing in it.
pub fn none_per_zone<T>() -> [Option<T>; ZONE_COUNT] {
    std::array::from_fn(|_| None)
}
