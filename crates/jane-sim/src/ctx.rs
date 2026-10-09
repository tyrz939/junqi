//! The context every system takes (ARCHITECTURE.md §5.2): one live zone, the world, and
//! optionally the seat a call is made for. The old sim's import cycles become modules sharing
//! this. A zone is taken out of `GameState.zones` for the length of its ctx, so `world` and
//! `zone` are two plain `&mut`s.

use alloc::vec::Vec;

use jane_core::{Blueprint, Key, Sym, Vec2, ZONE_COUNT, ZoneId};
use jane_data::Catalog;

use crate::event::{DevNote, Event, EventKind};
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
    /// A bed's `Rest { until }` with the whole party resting: the hour to sleep to. The step runs
    /// it (`living::Sim::sleep_to`) after the commands or at its end, never mid-zone, so it is
    /// empty between steps and never saved. The first asked in a step wins.
    pub sleep: Option<u8>,
}

/// Per-sim scratch (§3.3 `Sim.scratch`): every temporary buffer a tick uses, `clear()`ed and
/// never dropped, so the tick allocates nothing once warm.
#[derive(Debug)]
pub struct Scratch {
    pub path: PathScratch,
    pub props: Vec<PropIx>,
    /// Step 7's snapshot of the awake list.
    pub units: Vec<UnitId>,
    /// A second prop buffer, for a search inside a loop over `props` (focus, world spells).
    pub props_b: Vec<PropIx>,
    /// Cells: the nudge search's queue (`clear.rs`).
    pub cells: Vec<(i32, i32)>,
    /// Blows waiting for their zone's flush (step 10), per zone, in the order they were dealt.
    /// Empty between steps: what a step deals, the step lands (`combat::Hit`).
    pub hits: [Vec<crate::combat::Hit>; ZONE_COUNT],
    /// The flush's own buffers: the pass being landed, and who it lands on.
    pub flushing: Vec<crate::combat::Hit>,
    pub hit_ids: Vec<UnitId>,
    /// Combat's searches through `unit_blocks` (victims, splash, pulses, assist).
    pub near: Vec<UnitId>,
    /// Corpses due to stand up this tick.
    pub due: Vec<UnitId>,
    /// The lights near a search by something that shuns light (`ai::follow_to`).
    pub lights: crate::ai::LitField,
}

/// Blows a zone's step has room for before its first: a zone's first blow is not a growth in
/// the tick.
const HITS_ROOM: usize = 16;

impl Default for Scratch {
    fn default() -> Self {
        Self {
            path: PathScratch::default(),
            props: Vec::new(),
            units: Vec::new(),
            props_b: Vec::new(),
            cells: Vec::new(),
            hits: core::array::from_fn(|_| Vec::with_capacity(HITS_ROOM)),
            flushing: Vec::with_capacity(HITS_ROOM),
            hit_ids: Vec::with_capacity(HITS_ROOM),
            near: Vec::new(),
            due: Vec::new(),
            lights: crate::ai::LitField::default(),
        }
    }
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

impl core::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

    /// A unit this step was working on is not in its zone (a content or engine bug: valid play
    /// never does it). The step skips it and says so to dev tools ([`EventKind::Dev`]) instead
    /// of aborting the host. See [`unit_or_skip`] and [`unit_mut_or_skip`].
    pub fn missing_unit(&mut self, unit: UnitId, at: &'static str) {
        self.emit_all(EventKind::Dev(DevNote::MissingUnit { unit, at }));
    }

    /// The actor's body, if she has one here.
    pub fn actor_unit(&self) -> Option<UnitId> {
        let seat = self.actor?;
        let p = self.world.player(seat)?;
        (p.zone == self.zone.id && self.zone.unit(p.unit).is_some()).then_some(p.unit)
    }

    /// What a condition reads here: whom the actor is talking to is her conversation's speaker.
    pub fn ask(&self) -> crate::actions::Ask<'_> {
        let speaker = self
            .actor
            .and_then(|s| self.world.player(s))
            .and_then(|p| p.dialogue)
            .map_or(crate::state::Speaker::None, |d| d.speaker);
        crate::actions::Ask {
            cat: self.cat,
            world: self.world,
            zone: self.zone,
            rt: self.rt,
            bp: self.bp,
            actor: self.actor,
            speaker,
        }
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
    core::array::from_fn(|_| None)
}

/// `cx.zone.unit(id)`, or (the unit is gone: a bug) [`Ctx::missing_unit`] and `return` (with
/// the value given, if any) from the step.
macro_rules! unit_or_skip {
    ($cx:ident, $id:expr, $at:expr $(, $ret:expr)?) => {
        match $cx.zone.unit($id) {
            Some(u) => u,
            None => {
                $cx.missing_unit($id, $at);
                return $($ret)?;
            }
        }
    };
}
pub(crate) use unit_or_skip;

/// [`unit_or_skip`] for `cx.zone.unit_mut(id)`.
macro_rules! unit_mut_or_skip {
    ($cx:ident, $id:expr, $at:expr $(, $ret:expr)?) => {
        match $cx.zone.unit_mut($id) {
            Some(u) => u,
            None => {
                $cx.missing_unit($id, $at);
                return $($ret)?;
            }
        }
    };
}
pub(crate) use unit_mut_or_skip;
