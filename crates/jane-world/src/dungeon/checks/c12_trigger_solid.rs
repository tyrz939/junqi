//! C12: nothing solid appears on whoever caused it. A trigger the blueprint carries never shows
//! or locks a gate or a solid prop over its own rect. (The engine would stand her aside if it
//! did; a dungeon should still never ask it to.)

use alloc::format;
use alloc::vec::Vec;
use jane_core::action::Action;
use jane_core::grid::Rect;

use super::{Check, Ctx, Fault};
use crate::solve::rows::each_action;

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (bp, cat) = (c.bp, c.cat);
    let mut out = Vec::new();
    for (&id, t) in &bp.triggers {
        let Some(&rect) = bp.rects.get(&t.rect) else { continue };
        let mut hits = Vec::new();
        each_action(bp, cat, t.actions, &mut |a| {
            let (Action::Show(k) | Action::Lock(k)) = *a else { return };
            let Some(p) = c.prop(k) else { return };
            let d = cat.story.prop(p.def);
            if !(d.gate || d.solid) {
                return;
            }
            let foot = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
            if foot.overlaps(rect) {
                hits.push(k);
            }
        });
        for k in hits {
            out.push(Fault::new(
                Check::C12,
                format!("trigger {} would put {} on top of whoever tripped it", c.name(id), c.name(k)),
            ));
        }
    }
    out
}
