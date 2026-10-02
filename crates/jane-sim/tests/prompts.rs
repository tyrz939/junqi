//! A prompt never promises what E does not do. The owner's playtest: "pushing e at craft bench
//! does nothing even though it says to". Every prop that offers a verb, pressed: something
//! changes (the prop, her hands, where she is) or she is told or shown something (a toast, a
//! window, a conversation, a rest). A sound alone is not an answer.

mod common;

use jane_core::action::Facing;
use jane_sim::event::PropChange;
use jane_sim::{Command, EventKind, Seat};

use common::bot::*;
use common::room::Room;

/// What E did that she could see, else `None`.
fn answered(s: &mut jane_sim::Sim, before: (&jane_sim::Prop, &jane_sim::Unit)) -> Option<String> {
    let ev = events(s);
    let seen = ev.iter().find(|e| {
        e.to.is_none_or(|t| t == Seat(0))
            && !matches!(e.kind, EventKind::Sfx { .. } | EventKind::Prop { change: PropChange::Use, .. })
    });
    if let Some(e) = seen {
        return Some(format!("{:?}", e.kind));
    }
    let (p, u) = (prop(s, "it"), me(s).clone());
    if &p != before.0 {
        return Some("the prop changed".into());
    }
    (u.carrying != before.1.carrying || u.pos != before.1.pos).then(|| "she moved or holds it".into())
}

#[test]
fn every_prop_that_offers_a_verb_answers_e() {
    let cat = jane_data::catalog();
    let (px, py) = (20u16, 12u16);
    let (mut silent, mut offered) = (Vec::new(), 0);
    for def in cat.story.props {
        for locked in [false, true] {
            let mut r = Room::new(true);
            r.prop("it", def.id, px, py, |p| p.locked = locked);
            let mut s = r.build();
            // Below its front row, facing it.
            place(&mut s, i32::from(px) + i32::from(def.w) / 2, i32::from(py) + i32::from(def.h), Facing::North);
            let target = prop_id(&s, "it");
            let Some(f) = s.view(Seat(0)).unwrap().focus() else { continue };
            if f.target != jane_sim::interact::FocusRef::Prop(target) {
                continue;
            }
            offered += 1;
            let before = (prop(&s, "it"), me(&s).clone());
            events(&mut s);
            cmd(&mut s, Command::Use);
            idle(&mut s, 2);
            if answered(&mut s, (&before.0, &before.1)).is_none() {
                silent.push(format!(
                    "{}{}: says {:?}, and E does nothing",
                    def.id,
                    if locked { " (locked)" } else { "" },
                    f.verb
                ));
            }
        }
    }
    assert!(offered > 40, "props offered a verb ({offered})");
    assert!(silent.is_empty(), "a prompt E does not answer: {silent:#?}");
}

/// Julie's kitchen bench on a new game: E opens her bag on the craft row.
#[test]
fn e_at_the_kitchen_bench_opens_the_craft_row() {
    let mut s = common::new_game();
    let front = sym(&s, "front");
    cmd(&mut s, Command::Dev(jane_sim::DevOp::Tp { zone: jane_core::ZoneId::House, mark: front }));
    idle(&mut s, 2);
    let cat = jane_data::catalog();
    let bench = s
        .view(Seat(0))
        .unwrap()
        .props()
        .find(|p| cat.story.prop(p.def).bench)
        .cloned()
        .expect("the kitchen has a bench");
    let def = cat.story.prop(bench.def);
    place(
        &mut s,
        i32::from(bench.cell.x) + i32::from(def.w) / 2,
        i32::from(bench.cell.y) + i32::from(def.h),
        Facing::North,
    );
    let v = s.view(Seat(0)).unwrap();
    assert!(v.near_bench(), "the bench is in reach");
    assert_eq!(v.focus().map(|f| f.verb), Some(jane_sim::interact::Verb::Craft));
    events(&mut s);
    cmd(&mut s, Command::Use);
    let ev = events(&mut s);
    assert!(
        ev.iter().any(|e| e.kind == EventKind::Bench { prop: bench.id } && e.to == Some(Seat(0))),
        "E at the bench opens the craft row: {ev:?}"
    );
}
