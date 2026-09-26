//! The journal (ARCHITECTURE.md §3.7): the world's understood record. The fog is what was seen;
//! this is what was understood: places been to and named, people met and talked to, things
//! held, what she was told and read, and whether the world later bore it out.
//!
//! **One writer.** Every write goes through [`record`]: a new fact, or a stronger source for a
//! fact already known ([`Source::rank`]), changes `known`, appends an entry to the ring and
//! emits `EventKind::Journal(kind)`. A repeat, or a weaker source, changes nothing and emits
//! nothing, so the shell can mark the log without diffing it. `since` keeps the first tick;
//! `Confirmed` and `Contradicted` are final.
//!
//! **Writers** (each names its source): `Location` (Place Visited); arriving in a zone for the
//! first time (Place Seen, the zone's name); a dialogue line's `tells` (Place Named, Person
//! Talked, Thing Seen, Claim Told, Route and Danger Told as claims are, Rumour Heard); talking
//! to someone with a name (Person Met); a `Read` of a content text (Claim Read); a kill of a
//! named unit (Person Dead, through `hooks::on_kill`); a chest, a pick-up, a reward or a craft
//! (Thing Held); `Travel` and a door (Route Walked, from the mark she came in by to the one she
//! arrives at).
//!
//! **Readers:** `Condition::Knows` and `Condition::Heard` (`actions.rs`), `View::journal()` and
//! `View::known()`.

use jane_core::action::Thing;
use jane_core::{Cell, ItemId, ZoneId};

use crate::ctx::Ctx;
use crate::event::{Event, EventKind};
use crate::state::{FactKey, GameState, JournalEntry, Known, Source};
use crate::tuning::JOURNAL_RING;

/// Would `how` change what is known of `fact`?
pub fn improves(state: &GameState, fact: FactKey, how: Source) -> bool {
    match state.journal.known.get(&fact) {
        None => true,
        Some(k) => !k.how.is_final() && how.rank() > k.how.rank(),
    }
}

/// Write a fact. Returns whether anything changed (and so whether an event went out).
pub fn record(
    state: &mut GameState,
    events: &mut Vec<Event>,
    fact: FactKey,
    how: Source,
    zone: ZoneId,
    at: Cell,
) -> bool {
    if !improves(state, fact, how) {
        return false;
    }
    let tick = state.tick;
    let j = &mut state.journal;
    let since = j.known.get(&fact).map_or(tick, |k| k.since);
    j.known.insert(fact, Known { since, how });
    let kind = fact.kind();
    // The ring: at its bound for this kind, the oldest entry of the kind makes way.
    let mut of_kind = 0;
    let mut oldest = None;
    for (i, e) in j.entries.iter().enumerate() {
        if e.kind() == kind {
            of_kind += 1;
            if oldest.is_none() {
                oldest = Some(i);
            }
        }
    }
    if of_kind >= JOURNAL_RING {
        if let Some(i) = oldest {
            j.entries.remove(i);
        }
    }
    j.entries.push(JournalEntry { tick, fact, how, zone, at });
    events.push(Event { to: None, in_zone: None, kind: EventKind::Journal(kind) });
    true
}

/// Write a fact from inside a zone's context: at the actor's feet, else the zone's origin.
pub fn learn(cx: &mut Ctx<'_>, fact: FactKey, how: Source) -> bool {
    let at = cx.actor_unit().and_then(|id| cx.zone.unit(id)).map_or(Cell::new(0, 0), |u| {
        let (x, y) = u.pos.cell();
        Cell::new(x.max(0) as u16, y.max(0) as u16)
    });
    let zone = cx.zone.id;
    record(cx.world, cx.events, fact, how, zone, at)
}

/// Something went into her hands.
pub fn held(cx: &mut Ctx<'_>, item: ItemId) {
    learn(cx, FactKey::Thing(Thing::Item(item)), Source::Held);
}

/// Is the fact known at all?
pub fn knows(state: &GameState, fact: FactKey) -> bool {
    state.journal.known.contains_key(&fact)
}

/// Was she told this, or did she read it (whatever the world did to it since)?
pub fn heard(state: &GameState, claim: jane_core::TextId) -> bool {
    state.journal.known.contains_key(&FactKey::Claim(claim))
}

/// When a fact was first known, and how well it is known now.
pub fn known(state: &GameState, fact: FactKey) -> Option<Known> {
    state.journal.known.get(&fact).copied()
}
