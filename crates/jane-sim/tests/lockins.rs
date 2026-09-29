//! A boss's lock-in drops its gate only with the boss inside (DUNGEONS.md §3.3, "Can she get
//! stuck?"). The Emperor chased her out of its glade, and she ran back in a step ahead of it: the
//! hedge closed between them, with nothing inside to fight and nothing to die to.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::action::Facing;
use jane_core::{Rect, Vec2, ZoneId};
use jane_sim::input::DevOp;
use jane_sim::{Command, Sim};

fn arena(s: &Sim) -> Rect {
    let rt = s.runtime(ZoneId::Forest).expect("the forest is loaded");
    *rt.rects.get(&sym(s, "forest_arena")).expect("the glade's rect")
}

fn put_emperor(s: &mut Sim, x: i32, y: i32) {
    let k = sym(s, "emperor");
    let z = s.state_mut().zone_mut(ZoneId::Forest).expect("the forest");
    let u = z.units.iter_mut().find(|u| u.key == Some(k)).expect("the Emperor");
    u.pos = Vec2::centre(x, y);
    u.home = u.pos;
    s.rebuild_runtimes();
}

fn rect_of(s: &Sim, z: ZoneId, name: &str) -> Rect {
    *s.runtime(z).expect("loaded").rects.get(&sym(s, name)).unwrap_or_else(|| panic!("no rect {name}"))
}

fn middle(r: Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

fn kill_guards(s: &mut Sim) {
    let keys: Vec<_> = ["guard_a", "guard_b", "guard_c", "guard_d"].iter().map(|k| sym(s, k)).collect();
    let z = s.state_mut().zone_mut(ZoneId::Burial).expect("the burial");
    for u in z.units.iter_mut().filter(|u| u.key.is_some_and(|k| keys.contains(&k))) {
        u.alive = false;
    }
    s.rebuild_runtimes();
}

fn guards_standing(s: &Sim) -> usize {
    let keys: Vec<_> = ["guard_a", "guard_b", "guard_c", "guard_d"].iter().map(|k| sym(s, k)).collect();
    let z = s.state().zone(ZoneId::Burial).expect("the burial");
    z.units.iter().filter(|u| u.alive && u.key.is_some_and(|k| keys.contains(&k))).count()
}

/// The Snake's lock-in room is written by hand (`data/triggers.json`), not by the generator, and
/// had no memory of being cleared: a death anywhere after it re-armed it, the next walk through
/// stood four guards up and dropped the scaled door again, and the clear that lifts it had
/// already fired once and never would again (DUNGEONS.md §3.5, "Can she get stuck?").
#[test]
fn the_snakes_lock_in_stays_cleared_after_she_dies_elsewhere() {
    let mut s = new_game();
    let entry = sym(&s, "entry");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Burial, mark: entry }));
    idle(&mut s, 3);
    assert_eq!(zone_of(&s), ZoneId::Burial);
    let room = rect_of(&s, ZoneId::Burial, "lockin_room");
    let (mx, my) = middle(room);
    let (ox, oy) = me(&s).pos.cell();

    place(&mut s, mx, my, Facing::South);
    idle(&mut s, 2);
    assert!(prop(&s, "gate_snake").locked, "the scaled door drops behind her");
    assert_eq!(guards_standing(&s), 4);
    kill_guards(&mut s);
    idle(&mut s, 2);
    assert!(!prop(&s, "gate_snake").locked, "and lifts when the four are down");
    assert!(!prop(&s, "giant_key_chest").locked);

    // She goes on, and dies somewhere else.
    place(&mut s, ox, oy, Facing::South);
    idle(&mut s, 2);
    let guard = jane_data::catalog().combat.unit_id("skeleton_guard").expect("a guard row");
    cmd(&mut s, Command::Dev(DevOp::Hp(1)));
    cmd(&mut s, Command::Dev(DevOp::Spawn(guard)));
    let mut died = false;
    for _ in 0..6000 {
        idle(&mut s, 1);
        died |= !me(&s).alive;
        if died && me(&s).alive {
            break;
        }
    }
    assert!(died && me(&s).alive, "she fell and is up again");
    if zone_of(&s) != ZoneId::Burial {
        cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Burial, mark: entry }));
        idle(&mut s, 3);
    }
    cmd(&mut s, Command::Dev(DevOp::Kill));

    // Back through the room: nothing stands up and the door stays where it is.
    place(&mut s, mx, my, Facing::South);
    idle(&mut s, 2);
    assert_eq!(guards_standing(&s), 0, "the four stood up again");
    assert!(!prop(&s, "gate_snake").locked, "the scaled door dropped again, and nothing will lift it now");
}

#[test]
fn the_glade_does_not_close_on_her_while_the_emperor_is_outside_it() {
    let mut s = new_game();
    cmd(&mut s, Command::Dev(DevOp::God(true)));
    let entry = sym(&s, "entry");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Forest, mark: entry }));
    idle(&mut s, 3);
    assert_eq!(zone_of(&s), ZoneId::Forest);
    let r = arena(&s);
    let (mx, my) = (r.x + r.w / 2, r.y + r.h / 2);
    let (ox, oy) = me(&s).pos.cell();
    assert!(!r.contains(ox, oy), "she starts outside the glade");

    // The Emperor a long way outside, at the gate with her: she walks in, and the hedge stays open.
    put_emperor(&mut s, ox + 1, oy);
    place(&mut s, mx, my + 2, Facing::South);
    idle(&mut s, 2);
    assert!(!prop(&s, "forest_hedge_gate").locked, "the hedge closed with the Emperor outside");

    // Out again, the Emperor home in the middle of its glade: in she comes, and it closes.
    place(&mut s, ox, oy, Facing::South);
    idle(&mut s, 2);
    put_emperor(&mut s, mx, my);
    place(&mut s, mx, my + 2, Facing::South);
    idle(&mut s, 2);
    assert!(prop(&s, "forest_hedge_gate").locked, "the hedge stayed open with the Emperor inside");
}
