//! Where combat calls into systems other units own (PORT.md §8: two agents never own one file,
//! so the seams are here, small and named). Each body is the documented placeholder its owner
//! replaces; the signature is the contract. Nothing else in combat reaches into those systems.
//!
//! | Hook | Owner | Called from |
//! | --- | --- | --- |
//! | [`world_verb`] | interact (`interact.ts worldVerb`) | a `World` spell's cast |
//! | [`school_touch`] | interact (`interact.ts schoolTouch`) | a bolt's end (step 8) |
//! | [`use_item`] | inventory (`inventory.ts useItem`) | `Command::Bar` on an item slot |
//! | [`quest_kill`] | quests (`quests.ts onUnitKilled`) | a kill by one of the party (step 10) |
//! | [`unit_died`] | journal, living world (§3.7, §4.6.c) | every creature's death (step 10) |
//! | [`respawn_allowed`] | living world (§4.6.c ecology) | a corpse due to stand up (step 12) |
//! | [`put_down_dead`] | interact (`sim.ts revivePlayer`, `moveProp`) | a seat waking (step 6) |
//! | [`reset_lock_ins`] | triggers (`sim.ts revivePlayer`, trigger `reset`) | a seat waking (step 6) |

use jane_core::action::School;
use jane_core::{Fx, ItemId, UnitDefId, Vec2};
use jane_data::WorldSpell;

use crate::ctx::Ctx;
use crate::ids::{Seat, UnitId};

/// A `World` spell (Repair, Grow) does its verb to the prop in front of `caster`: the nearest
/// unused one within 2 m that answers it (Grow only in light), paying what the prop `needs`
/// from her bag, then marking it used and on and running its `use` list. Returns whether it
/// found something to do; `false` makes the cast fail and cost nothing, and the interact unit
/// says why ("Nothing here to repair", "Nothing grows without light").
///
/// Placeholder: nothing answers yet, so every world spell fails.
pub fn world_verb(cx: &mut Ctx<'_>, caster: UnitId, verb: WorldSpell) -> bool {
    let _ = (cx, caster, verb);
    false
}

/// A bolt of `school` ended at `at`: every unhidden prop not yet on that answers the school and
/// whose middle is within `touch` is switched on and used, and its `use` list runs for `from`.
///
/// Placeholder: no-op.
pub fn school_touch(cx: &mut Ctx<'_>, school: School, at: Vec2, from: Option<UnitId>, touch: Fx) {
    let _ = (cx, school, at, from, touch);
}

/// A bar slot holding an item was pressed.
///
/// Placeholder: no-op.
pub fn use_item(cx: &mut Ctx<'_>, seat: Seat, item: ItemId) {
    let _ = (cx, seat, item);
}

/// One of the party (`seat`) killed a unit of `def`: kill requirements count, the progress is
/// said to everyone. Called after the kill counts in her stats and before loot and `on_death`.
///
/// Placeholder: no-op.
pub fn quest_kill(cx: &mut Ctx<'_>, seat: Seat, def: UnitDefId) {
    let _ = (cx, seat, def);
}

/// A creature (not a seat's body) died, killed by `slayer` if one of the party: the journal's
/// `Person Dead` and `Danger AttackedIn`, the ecology's pressure in its area. Called after loot,
/// before `on_death`.
///
/// Placeholder: no-op.
pub fn unit_died(cx: &mut Ctx<'_>, unit: UnitId, slayer: Option<Seat>) {
    let _ = (cx, unit, slayer);
}

/// May this corpse stand up now? The ecology says no while its def is at its area's `cap` or the
/// pressure is over the row's `hold` line, and pushes it back onto `sleeping_due` at the next
/// hour itself.
///
/// Placeholder: always.
pub fn respawn_allowed(cx: &mut Ctx<'_>, unit: UnitId) -> bool {
    let _ = (cx, unit);
    true
}

/// A seat wakes: what her body carried when she fell stays where she fell (the prop is moved to
/// the nearest free cell by her body and made solid as its row says). Called before she stands.
///
/// Placeholder: the prop goes back to its row's solidity where it was lifted from (the same
/// fallback `Leave` uses); `revive_player` clears `carrying` after.
pub fn put_down_dead(cx: &mut Ctx<'_>, seat: Seat, body: UnitId) {
    let _ = seat;
    let Some(pid) = cx.zone.unit(body).and_then(|u| u.carrying) else { return };
    if let Some(pix) = cx.zone.prop_ix(pid) {
        let p = &mut cx.zone.props[pix as usize];
        p.solid = cx.cat.story.prop(p.def).solid;
        cx.rt.touch_prop(cx.zone, pix);
    }
}

/// A seat wakes: every fired trigger with a `reset` in her zone undoes itself and re-arms, unless
/// a friend still alive stands in its rect (so a death never leaves a gate shut in her face, and
/// with company a lock-in holds while someone is still inside).
///
/// Placeholder: no-op.
pub fn reset_lock_ins(cx: &mut Ctx<'_>, seat: Seat, body: UnitId) {
    let _ = (cx, seat, body);
}
