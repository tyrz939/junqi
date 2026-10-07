//! C6: every lock-in can be followed into. The way in is hidden until the room seals, shown when
//! it does, hidden again after (by the clear and by the reset), reached with the gate down, and
//! lands inside the sealed rect: how a friend who was late, or who died and walked back, follows.

use alloc::format;
use alloc::vec::Vec;
use jane_core::action::Action;

use super::{Check, Ctx, Fault};
use crate::solve::ablate::Grant;

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let bp = c.bp;
    let mut out = Vec::new();
    for l in &c.info.lockins {
        let node = c.m.nodes[l.node].id;
        let way_in = c.prop(l.way_in);
        let mark = bp.marks.get(&l.mark);
        let rect = bp.rects.get(&l.rect);
        let lock_row = bp.triggers.get(&l.lock);
        let clear_row = bp.triggers.get(&l.clear);
        let (Some(way_in), Some(mark), Some(&rect), Some(lock_row), Some(clear_row)) =
            (way_in, mark, rect, lock_row, clear_row)
        else {
            out.push(Fault::new(Check::C6, format!("the lock-in of {node} is missing a part")));
            continue;
        };
        let (wi, gate) = (c.name(l.way_in), c.name(l.gate));
        if !way_in.hidden {
            out.push(Fault::new(Check::C6, format!("{wi} is not hidden, so it is a way round {gate}")));
        }
        if !rect.contains(i32::from(mark.cell.x), i32::from(mark.cell.y)) {
            out.push(Fault::new(Check::C6, format!("{} is outside {}", c.name(l.mark), c.name(l.rect))));
        }
        let list = |r| bp.list(r).unwrap_or(&[]);
        let actions = list(lock_row.actions);
        let reset = lock_row.reset.map_or(&[][..], list);
        let shows = actions.contains(&Action::Show(l.way_in)) && actions.contains(&Action::Lock(l.gate));
        let hides =
            list(clear_row.actions).contains(&Action::Hide(l.way_in)) && reset.contains(&Action::Hide(l.way_in));
        if !shows || !hides {
            out.push(Fault::new(
                Check::C6,
                format!(
                    "{} must show {wi} when it locks {gate}, and the clear and the reset must hide it",
                    c.name(l.lock)
                ),
            ));
        }
        let shut = c.resolve(&c.opts.without(Grant::Shut(l.gate)));
        if shut.info.first_seen_at(i32::from(way_in.cell.x), i32::from(way_in.cell.y)).is_none() {
            out.push(Fault::new(Check::C6, format!("{wi} cannot be reached with {gate} down")));
        }
    }
    out
}
