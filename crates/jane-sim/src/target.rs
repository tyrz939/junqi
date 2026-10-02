//! Targets (PLAY-PLAN §2.1): a target when she has one, free aim when she wants it.
//!
//! **The client holds the target** and carries it in every [`InputFrame`] (a unit or prop id);
//! the sim validates it every tick ([`valid`]) and keeps the result on `PlayerState::fight`, which
//! the view shows. Nothing the client says is trusted: a unit gone, dead, hidden or too far, a
//! prop hidden or that answers no Word, is no target, and a cast at it goes along the aim.
//!
//! **Choosing.** Clicking chooses (the client resolves the click); Tab, RB and LB walk
//! [`foes_in_front`], nearest first; an attack with no target takes the nearest of them (the soft
//! target, [`soft_target`]). Each is a pure function of the zone, so the client, the bots and
//! the sim agree on them.

use jane_core::num::dist_sq;
use jane_core::{Angle, Vec2};
use jane_data::Controller;

use crate::combat::{is_enemy, max_bounds, query_near};
use crate::ids::UnitId;
use crate::input::TargetRef;
use crate::light::prop_centre;
use crate::los::line_of_sight;
use crate::runtime::ZoneRuntime;
use crate::state::{Unit, ZoneState};
use crate::tuning::{FRONT_HALF, TAB_REACH_FX, TARGET_KEEP_FX};

/// A prop worth targeting: one that answers a Word (a school or a verb).
pub fn prop_targetable(zone: &ZoneState, id: crate::ids::PropId) -> bool {
    zone.prop_ix(id)
        .and_then(|ix| zone.props.get(ix as usize))
        .is_some_and(|p| !p.hidden && jane_data::catalog().story.prop(p.def).answers.is_some())
}

/// Where a target is: a unit's feet, a prop's middle. `None` when it is not here.
pub fn pos_of(zone: &ZoneState, t: TargetRef) -> Option<Vec2> {
    match t {
        TargetRef::Unit(id) => zone.unit(id).map(|u| u.pos),
        TargetRef::Prop(id) => zone
            .prop_ix(id)
            .and_then(|ix| zone.props.get(ix as usize))
            .map(|p| prop_centre(jane_data::catalog().story.prop(p.def), p)),
    }
}

/// May `body` hold `t` as its target? A unit here, not her, alive and shown; a prop here, shown,
/// that answers a Word. Either within [`TARGET_KEEP_FX`].
pub fn valid(zone: &ZoneState, body: &Unit, t: TargetRef) -> bool {
    let near = |at: Vec2| dist_sq(at, body.pos) <= TARGET_KEEP_FX * TARGET_KEEP_FX;
    match t {
        TargetRef::Unit(id) => zone.unit(id).is_some_and(|u| id != body.id && u.alive && !u.hidden && near(u.pos)),
        TargetRef::Prop(id) => prop_targetable(zone, id) && pos_of(zone, t).is_some_and(near),
    }
}

/// Is `t` a unit `body` fights?
pub fn hostile(zone: &ZoneState, body: &Unit, t: TargetRef) -> bool {
    match t {
        TargetRef::Unit(id) => zone
            .unit(id)
            .is_some_and(|u| u.alive && u.controller != Controller::Npc && is_enemy(body.faction, u.faction)),
        TargetRef::Prop(_) => false,
    }
}

/// Foes `body` could choose with Tab: hostile, alive, awake and shown, within [`TAB_REACH_FX`],
/// within [`FRONT_HALF`] of `facing` and in sight; nearest first, ties to the lower id.
pub fn foes_in_front(zone: &ZoneState, rt: &ZoneRuntime, body: &Unit, facing: Angle, out: &mut Vec<UnitId>) {
    let mut near = Vec::new();
    query_near(rt, body.pos, TAB_REACH_FX + i64::from(max_bounds()), &mut near);
    let mut found: Vec<(i64, UnitId)> = Vec::new();
    for id in near {
        let Some(u) = zone.unit(id) else { continue };
        if id == body.id
            || !u.alive
            || !u.awake
            || u.hidden
            || u.controller == Controller::Npc
            || !is_enemy(body.faction, u.faction)
        {
            continue;
        }
        let d = dist_sq(body.pos, u.pos);
        if d > TAB_REACH_FX * TAB_REACH_FX {
            continue;
        }
        if u.pos != body.pos && facing.diff(jane_core::angle::bearing(body.pos, u.pos)).abs() > FRONT_HALF {
            continue;
        }
        if !line_of_sight(&rt.grid, body.pos, u.pos) {
            continue;
        }
        found.push((d, id));
    }
    found.sort();
    out.clear();
    out.extend(found.into_iter().map(|(_, id)| id));
}

/// Tab: the foe after `current` in [`foes_in_front`]'s order, round to the nearest; the nearest
/// with none (or one no longer in the list).
pub fn tab_next(order: &[UnitId], current: Option<TargetRef>) -> Option<UnitId> {
    let at = current.and_then(|c| match c {
        TargetRef::Unit(u) => order.iter().position(|&o| o == u),
        TargetRef::Prop(_) => None,
    });
    match at {
        Some(i) => order.get((i + 1) % order.len()).copied(),
        None => order.first().copied(),
    }
}

/// Shift-Tab (LB): the foe before `current`, round to the farthest.
pub fn tab_prev(order: &[UnitId], current: Option<TargetRef>) -> Option<UnitId> {
    let at = current.and_then(|c| match c {
        TargetRef::Unit(u) => order.iter().position(|&o| o == u),
        TargetRef::Prop(_) => None,
    });
    match at {
        Some(i) => order.get((i + order.len() - 1) % order.len()).copied(),
        None => order.first().copied(),
    }
}

/// An attack with no target takes the nearest foe in front (WoW's target on attack).
pub fn soft_target(zone: &ZoneState, rt: &ZoneRuntime, body: &Unit, facing: Angle) -> Option<UnitId> {
    let mut v = Vec::new();
    foes_in_front(zone, rt, body, facing, &mut v);
    v.first().copied()
}
