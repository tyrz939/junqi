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
use crate::event::{EventKind, PropChange, SfxKind};
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

/// The living seats in the ctx's zone and the cells they stand on, in seat order: what
/// [`seat_in_rect`] reads, gathered once for a pass over every row instead of once a row (the
/// county has hundreds). Gathered again after a row runs its list, which may move or kill.
#[derive(Clone, Copy)]
struct Standing {
    n: usize,
    at: [(Seat, (i32, i32)); crate::tuning::MAX_PLAYERS],
}

impl Standing {
    fn of(cx: &Ctx<'_>) -> Standing {
        let z = cx.zone.id;
        let mut s = Standing { n: 0, at: [(Seat(0), (0, 0)); crate::tuning::MAX_PLAYERS] };
        for p in cx.world.players.iter().filter(|p| p.connected && p.zone == z) {
            if let Some(u) = cx.zone.unit(p.unit).filter(|u| u.alive) {
                s.at[s.n] = (p.seat, u.pos.cell());
                s.n += 1;
            }
        }
        s
    }

    /// [`seat_in_rect`] with no exception.
    fn in_rect(&self, r: Rect) -> Option<Seat> {
        self.at[..self.n].iter().find(|&&(_, (x, y))| r.contains(x, y)).map(|&(s, _)| s)
    }
}

/// Step 11: every row of the zone's table, then the plates every 6 ticks.
pub fn step_triggers(cx: &mut Ctx<'_>) {
    let mut standing = Standing::of(cx);
    for i in 0..cx.rt.triggers.len() {
        let t = cx.rt.triggers[i];
        let Some(r) = cx.rt.rects.get(&t.rect).copied() else { continue };
        let who = standing.in_rect(r);
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
            standing = Standing::of(cx);
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

/// The plates [`units_on_plates`] answers for at once, by their place in `ZoneRuntime::plates`;
/// any past these are asked one by one.
const PLATES_AT_ONCE: usize = 64;

/// Which of the zone's first [`PLATES_AT_ONCE`] plates a living, unhidden unit stands on, a bit
/// each: one pass over the zone's units for every plate, where a pass a plate cost the county
/// thousands of units a plate every sixth tick (sleepers press a plate as well as anyone, so no
/// index of the awake will do).
fn units_on_plates(cx: &Ctx<'_>) -> u64 {
    let cat = cx.cat;
    let n = cx.rt.plates.len().min(PLATES_AT_ONCE);
    let mut rects = [Rect::new(0, 0, 0, 0); PLATES_AT_ONCE];
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (k, r) in rects.iter_mut().enumerate().take(n) {
        let p = &cx.zone.props[cx.rt.plates[k] as usize];
        *r = footprint(cat.story.prop(p.def), p);
        (x0, y0, x1, y1) = (x0.min(r.x), y0.min(r.y), x1.max(r.right()), y1.max(r.bottom()));
    }
    let mut on = 0u64;
    for u in &cx.zone.units {
        let (x, y) = u.pos.cell();
        if x < x0 || y < y0 || x >= x1 || y >= y1 || !u.alive || u.hidden {
            continue;
        }
        for (k, r) in rects.iter().enumerate().take(n) {
            if r.contains(x, y) {
                on |= 1 << k;
            }
        }
    }
    on
}

/// Is the plate pressed? `units_on`: whether a unit stands on it, when [`units_on_plates`] has
/// answered for it.
fn plate_covered(cx: &mut Ctx<'_>, plate: PropIx, units_on: Option<bool>) -> bool {
    let cat = cx.cat;
    let p = &cx.zone.props[plate as usize];
    let r = footprint(cat.story.prop(p.def), p);
    let stood_on = units_on.unwrap_or_else(|| {
        cx.zone.units.iter().any(|u| {
            let (x, y) = u.pos.cell();
            u.alive && !u.hidden && r.contains(x, y)
        })
    });
    if stood_on {
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
    let mut units_on = units_on_plates(cx);
    for i in 0..cx.rt.plates.len() {
        let ix = cx.rt.plates[i];
        let p = &cx.zone.props[ix as usize];
        if p.hidden {
            continue;
        }
        let on = p.on;
        let pressed = plate_covered(cx, ix, (i < PLATES_AT_ONCE).then(|| units_on >> i & 1 != 0));
        if pressed == on {
            continue;
        }
        let p = &mut cx.zone.props[ix as usize];
        p.on = pressed;
        let (pid, cell) = (p.id, p.cell);
        let at = Vec2::centre(i32::from(cell.x), i32::from(cell.y));
        cx.emit(EventKind::Prop { prop: pid, change: PropChange::Switch });
        cx.emit(EventKind::Sfx { kind: if pressed { SfxKind::PlateDown } else { SfxKind::PlateUp }, at });
        let bp = cx.bp;
        let s = spawn_of(bp, &cx.zone.props[ix as usize]);
        let list = if pressed { s.and_then(|s| s.use_list) } else { s.and_then(|s| s.release) };
        // A plate has no actor: a barrel can press it. Its lists are world verbs.
        if let Some(list) = list {
            let before = cx.actor.take();
            run_actions(cx, list, Subject::None);
            cx.actor = before;
            // What it ran may have moved, raised or felled someone: ask again for the rest.
            units_on = units_on_plates(cx);
        }
    }
}
