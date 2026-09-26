//! Triggers and plates (`sim/triggers.ts`), step 11. "When someone is in this named rect (and these
//! conditions hold), run this list." 2020 had a dozen `obj_trigger_*` objects; they were all this.
//!
//! - `Enter` fires on the tick the first of the party walks in; `While` fires as soon as anyone
//!   is inside and `when` holds ("kill everything to unlock the chest").
//! - With up to four seats a rect is occupied while any living one of them is in it, and the
//!   list runs on behalf of the first in seat order.
//! - `once` rows fire once; a row with a `reset` list undoes itself and re-arms when a seat dies
//!   after it fired ([`reset_on_death`], which the combat unit's revive calls), unless someone
//!   else is still alive inside a lock-in.
//!
//! **Plates**, every 6 ticks, with no actor: a plate is pressed while any living unhidden unit or
//! a pushable prop covers it (2020: "any obj_in_world_parent presses a button, so barrels work");
//! it runs its `use` list on press and its `release` list when it clears.

use jane_core::blueprint::TriggerMode;
use jane_core::{Rect, Vec2};

use crate::actions::{Subject, conditions_met, conds, run_actions};
use crate::ctx::Ctx;
use crate::event::{EventKind, PropChange, Sfx};
use crate::ids::{PropIx, Seat};
use crate::interact::{footprint, spawn_of};
use crate::tuning::PLATE_PERIOD;

/// The first living seat standing in the rect, in seat order, other than `except`.
pub fn seat_in_rect(cx: &Ctx<'_>, r: Rect, except: Option<Seat>) -> Option<Seat> {
    let z = cx.zone.id;
    cx.world.players.iter().filter(|p| p.connected && p.zone == z && Some(p.seat) != except).find_map(|p| {
        let u = cx.zone.unit(p.unit)?;
        let (x, y) = u.pos.cell();
        (u.alive && r.contains(x, y)).then_some(p.seat)
    })
}

/// Step 11: every row of the zone's table, then the plates every 6 ticks.
pub fn step_triggers(cx: &mut Ctx<'_>) {
    for i in 0..cx.rt.triggers.len() {
        let t = cx.rt.triggers[i];
        let Some(r) = cx.rt.rects.get(&t.rect).copied() else { continue };
        let who = seat_in_rect(cx, r, None);
        let bits = &mut cx.zone.triggers;
        let (was_inside, fired) = (bits.inside.get(i as u32), bits.fired.get(i as u32));
        let inside = who.is_some();
        bits.inside.set(i as u32, inside);
        let Some(seat) = who else { continue };
        if fired && t.trigger.once {
            continue;
        }
        if t.trigger.mode == TriggerMode::Enter && was_inside {
            continue;
        }
        let before = cx.actor.replace(seat);
        if t.trigger.when.is_none_or(|w| conditions_met(cx, conds(cx, w))) {
            cx.zone.triggers.fired.set(i as u32, true);
            let body = cx.actor_unit().map_or(Subject::None, Subject::Unit);
            run_actions(cx, t.trigger.actions, body);
        }
        cx.actor = before;
    }
    if cx.world.tick.0 % PLATE_PERIOD == 0 {
        step_plates(cx);
    }
}

/// A seat died here: every row that fired and has a `reset` undoes itself and re-arms, unless it
/// is a room (narrower than the zone) with someone else still alive inside.
pub fn reset_on_death(cx: &mut Ctx<'_>, seat: Seat) {
    let grid_w = cx.rt.grid.w() as i32;
    let before = cx.actor.replace(seat);
    for i in 0..cx.rt.triggers.len() {
        let t = cx.rt.triggers[i];
        let Some(reset) = t.trigger.reset else { continue };
        let Some(r) = cx.rt.rects.get(&t.rect).copied() else { continue };
        if !cx.zone.triggers.fired.get(i as u32) {
            continue;
        }
        if r.w < grid_w && seat_in_rect(cx, r, Some(seat)).is_some() {
            continue;
        }
        let body = cx.actor_unit().map_or(Subject::None, Subject::Unit);
        run_actions(cx, reset, body);
        cx.zone.triggers.fired.set(i as u32, false);
        cx.zone.triggers.inside.set(i as u32, false);
    }
    cx.actor = before;
}

fn plate_covered(cx: &mut Ctx<'_>, plate: PropIx) -> bool {
    let cat = cx.cat;
    let p = &cx.zone.props[plate as usize];
    let r = footprint(cat.story.prop(p.def), p);
    if cx.zone.units.iter().any(|u| {
        let (x, y) = u.pos.cell();
        u.alive && !u.hidden && r.contains(x, y)
    }) {
        return true;
    }
    // A pushable or carryable prop on it, not in someone's arms (it keeps its old cell there).
    cx.rt.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, &mut cx.scratch.props_b);
    let zone = &*cx.zone;
    cx.scratch.props_b.iter().any(|&i| {
        let o = &zone.props[i as usize];
        let d = cat.story.prop(o.def);
        i != plate
            && !o.hidden
            && (d.push || d.carry)
            && footprint(d, o).overlaps(r)
            && !zone.units.iter().any(|u| u.carrying == Some(o.id))
    })
}

fn step_plates(cx: &mut Ctx<'_>) {
    // A handful per zone, in id order. Plates far from everyone still answer: a barrel left on
    // one holds its gate open from the other side of the map.
    for i in 0..cx.rt.plates.len() {
        let ix = cx.rt.plates[i];
        let p = &cx.zone.props[ix as usize];
        if p.hidden {
            continue;
        }
        let on = p.on;
        let pressed = plate_covered(cx, ix);
        if pressed == on {
            continue;
        }
        let p = &mut cx.zone.props[ix as usize];
        p.on = pressed;
        let (pid, cell) = (p.id, p.cell);
        let at = Vec2::centre(i32::from(cell.x), i32::from(cell.y));
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
        cx.emit(EventKind::Sfx { kind: if pressed { Sfx::PlateDown } else { Sfx::PlateUp }, at });
        let bp = cx.bp;
        let s = spawn_of(bp, &cx.zone.props[ix as usize]);
        let list = if pressed { s.and_then(|s| s.use_list) } else { s.and_then(|s| s.release) };
        // A plate has no actor: a barrel can press it. Its lists are world verbs.
        if let Some(list) = list {
            let before = cx.actor.take();
            run_actions(cx, list, Subject::None);
            cx.actor = before;
        }
    }
}
