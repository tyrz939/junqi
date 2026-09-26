//! Things under things (`sim/under.ts`). A key under a stone, a tin under a loose flag, a letter
//! under a crate: the thing is a hidden prop at a cell, and the prop on top names it in its spawn
//! row's `under`. Push the top off it and it is there, to be picked up like anything else.
//!
//! - It is under the prop cell for cell: while any cell of the top still covers it, nothing shows.
//! - `under_when` holds, or nothing is there yet. When the condition comes true later with the top
//!   already off the spot, the thing is found lying there then ([`uncover_all`], from a quest
//!   given), so no order of doing things loses it.
//!
//! Runs inside the step and reads only sim state; what changes is saved (`hidden`, `under_done`).

use jane_data::PropDef;

use crate::actions::{conditions_met, conds};
use crate::ctx::Ctx;
use crate::event::{EventKind, PropChange, ToastKind};
use crate::ids::PropIx;
use crate::state::Prop;

fn covers(a: &PropDef, top: &Prop, b: &PropDef, low: &Prop) -> bool {
    let (tx, ty, lx, ly) = (i32::from(top.cell.x), i32::from(top.cell.y), i32::from(low.cell.x), i32::from(low.cell.y));
    tx < lx + i32::from(b.w) && lx < tx + i32::from(a.w) && ty < ly + i32::from(b.h) && ly < ty + i32::from(a.h)
}

/// `top` has moved, or a condition may have changed: show what it lay on, if it can be seen
/// now. Returns whether `top` is done with it.
pub fn uncover(cx: &mut Ctx<'_>, top: PropIx) -> bool {
    let t = &cx.zone.props[top as usize];
    if t.under_done {
        return false;
    }
    let Some(spawn) = t.spawn.and_then(|s| cx.bp.props.get(usize::from(s))) else { return false };
    let Some(under) = spawn.under else { return false };
    let when = spawn.under_when;
    let key = cx.sym(under);
    let Some(low) = cx.rt.names.get(&key).copied() else {
        cx.zone.props[top as usize].under_done = true;
        return false;
    };
    let cat = cx.cat;
    let (t, l) = (&cx.zone.props[top as usize], &cx.zone.props[low as usize]);
    if covers(cat.story.prop(t.def), t, cat.story.prop(l.def), l) {
        return false;
    }
    if let Some(when) = when {
        if !conditions_met(cx, conds(cx, when)) {
            return false;
        }
    }
    cx.zone.props[top as usize].under_done = true;
    if cx.zone.props[low as usize].hidden {
        cx.zone.props[low as usize].hidden = false;
        cx.rt.touch_prop(cx.zone, low);
        let (top_id, low_id) = (cx.zone.props[top as usize].id, cx.zone.props[low as usize].id);
        cx.emit(EventKind::Prop { prop: low_id, change: PropChange::Show });
        cx.emit(EventKind::Toast(ToastKind::Under { top: top_id, found: low_id }));
    }
    true
}

/// Every prop in this zone that has something under it still: a quest given may make one true.
pub fn uncover_all(cx: &mut Ctx<'_>) {
    for ix in 0..cx.zone.props.len() as PropIx {
        let p = &cx.zone.props[ix as usize];
        if p.under_done {
            continue;
        }
        let has = p.spawn.and_then(|s| cx.bp.props.get(usize::from(s))).is_some_and(|s| s.under.is_some());
        if has {
            uncover(cx, ix);
        }
    }
}
