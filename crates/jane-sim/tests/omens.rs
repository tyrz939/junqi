//! Omens on the real seed (PLAN.md §5, WORLD.md §7.3, `omens.rs`): rolled into flags at New
//! Game from the world stream's first draws, and the rows that read them make the county do what
//! the claim says; a false omen changes nothing. Here each omen is set true (or false) by hand,
//! as the seed would, and the county is watched doing it.

mod common;

use common::bot::*;
use jane_core::ZoneId;
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::interact::Verb;
use jane_sim::state::{FlagKey, NightState};
use jane_sim::{Command, Seat, Sim};

fn set_omen(s: &mut Sim, id: &str, on: bool) {
    let k = FlagKey::Named(sym(s, &format!("omen:{id}")));
    if on {
        s.state_mut().flags.insert(k, 1);
    } else {
        s.state_mut().flags.remove(&k);
    }
}

fn tp(s: &mut Sim, zone: ZoneId, mark: &str) {
    let mark = sym(s, mark);
    cmd(s, Command::Dev(DevOp::Tp { zone, mark }));
    idle(s, 2);
    assert_eq!(zone_of(s), zone);
}

fn hour(s: &mut Sim, hour: u8) {
    cmd(s, Command::Dev(DevOp::Time { hour }));
}

fn verb(s: &Sim) -> Option<Verb> {
    s.view(Seat(0)).unwrap().focus().map(|f| f.verb)
}

fn night_locked(s: &mut Sim) -> bool {
    s.drain_events();
    cmd(s, Command::Use);
    s.drain_events().iter().any(|e| matches!(e.kind, EventKind::Toast(ToastKind::NightLock(_))))
}

/// New Game rolled the omens: exactly the true ones' flags are set, and they are the stream's first
/// draws (the roll is repeated from a fresh stream and agrees).
#[test]
fn new_game_rolls_the_omens_into_flags() {
    let s = common::new_game();
    let rows = jane_data::catalog().story.omens;
    let want = jane_sim::omens::roll(rows, &mut jane_core::Sfc32::seeded(common::SEED, 1));
    for (i, o) in rows.iter().enumerate() {
        assert_eq!(jane_sim::omens::is_true(s.state(), o.id), want.contains(&i), "{}", o.id);
    }
}

/// "The scarecrow in the top field was not there yesterday": true, after the bell the top field's
/// scarecrow is gone from its place and its twin stands nearer the farm, and by day they swap
/// back; false, it stands where it stood. Either way the swap happens out of her sight (the
/// trigger's rect is wide on purpose).
#[test]
fn the_scarecrow_is_closer_some_nights() {
    for true_here in [true, false] {
        let mut s = common::new_game();
        set_omen(&mut s, "scarecrow_closer", true_here);
        hour(&mut s, 22);
        tp(&mut s, ZoneId::County, "scarecrow_top");
        let (top, near) = (prop(&s, "scarecrow_top"), prop(&s, "scarecrow_top_near"));
        assert_eq!((top.hidden, near.hidden), (true_here, !true_here), "after the bell, omen {true_here}");
        tp(&mut s, ZoneId::County, "start");
        hour(&mut s, 10);
        tp(&mut s, ZoneId::County, "scarecrow_top");
        let (top, near) = (prop(&s, "scarecrow_top"), prop(&s, "scarecrow_top_near"));
        assert_eq!((top.hidden, near.hidden), (false, true), "by day, omen {true_here}");
    }
}

/// "No exit either, some nights": true, the mine's front door is barred from nine to five, from
/// outside (the county's door) and so from inside too; the adit is the second way out. Once Iron
/// Knuckles is down the bar is gone for good. False, the door is a door.
#[test]
fn the_mine_is_barred_some_nights() {
    for true_here in [true, false] {
        let mut s = common::new_game();
        set_omen(&mut s, "mine_no_exit", true_here);
        tp(&mut s, ZoneId::County, "mine_mouth");
        assert!(walk_to_prop(&mut s, "mine_door"));
        hour(&mut s, 22);
        assert_eq!(night_locked(&mut s), true_here, "the county's door at ten, omen {true_here}");
        idle(&mut s, 2);
        assert_eq!(zone_of(&s), if true_here { ZoneId::County } else { ZoneId::Mine });
        if true_here {
            hour(&mut s, 4);
            assert_eq!(verb(&s), Some(Verb::TryTheDoor), "still barred at four");
            hour(&mut s, 5);
            assert_ne!(verb(&s), Some(Verb::TryTheDoor), "off at five");
        }
        // Inside, the way out is barred the same hours.
        tp(&mut s, ZoneId::Mine, "entry");
        assert!(walk_to_prop(&mut s, "exit_door"));
        hour(&mut s, 23);
        assert_eq!(night_locked(&mut s), true_here, "the mine's way out at eleven, omen {true_here}");
        idle(&mut s, 2);
        assert_eq!(zone_of(&s), if true_here { ZoneId::Mine } else { ZoneId::County });
    }
    // Iron Knuckles down: the county's door, and the way out, answer at every hour.
    let mut s = common::new_game();
    set_omen(&mut s, "mine_no_exit", true);
    tp(&mut s, ZoneId::County, "mine_mouth");
    assert!(matches!(prop(&s, "mine_door").night, NightState::Locked(_)));
    tp(&mut s, ZoneId::Mine, "entry");
    assert!(matches!(prop(&s, "exit_door").night, NightState::Locked(_)));
    let boss = unit(&s, "iron_knuckles").id;
    s.state_mut().zone_mut(ZoneId::Mine).unwrap().unit_mut(boss).unwrap().alive = false;
    let dead = FlagKey::Dead(sym(&s, "iron_knuckles"));
    s.state_mut().flags.insert(dead, 1);
    s.rebuild_runtimes();
    tp(&mut s, ZoneId::County, "mine_mouth");
    assert_eq!(prop(&s, "mine_door").night, NightState::Open, "the mine quiet: unbarred");
    // Back in by the adit, and through the first room on the way out.
    tp(&mut s, ZoneId::Mine, "adit");
    tp(&mut s, ZoneId::Mine, "entry");
    assert_eq!(prop(&s, "exit_door").night, NightState::Open);
}

/// "Please do not pick the white roses after dark": true, a rose gathered after dark (Julie's
/// cellar's, or Sallow Bottom's) is noticed, and one soldier stands in the cellar's study, never
/// on the stairs. By day, or on a seed it is false, nothing.
#[test]
fn a_white_rose_picked_after_dark_is_noticed_in_the_cellar() {
    let soldier = |s: &Sim| {
        let k = sym(s, "rose_soldier");
        s.state().zone(ZoneId::Cellar).unwrap().units.iter().any(|u| u.key == Some(k))
    };
    for (true_here, at) in [(true, 22), (true, 12), (false, 22)] {
        let mut s = common::new_game();
        set_omen(&mut s, "roses_after_dark", true_here);
        tp(&mut s, ZoneId::Cellar, "stair_a");
        hour(&mut s, at);
        let rose = jane_data::catalog().story.prop_id("rose").unwrap();
        let p = s.state().zone(ZoneId::Cellar).unwrap().props.iter().find(|p| p.def == rose).unwrap().clone();
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        let grid = &s.runtime(ZoneId::Cellar).unwrap().grid;
        let (fx, fy) = grid.nearest_free(x, y + 1, 3, None).expect("a cell by the rose");
        place(&mut s, fx, fy, jane_core::action::Facing::North);
        face(&mut s, prop_centre(&p));
        let roses = holds(&s, "white_water_rose");
        cmd(&mut s, Command::Use);
        assert_eq!(holds(&s, "white_water_rose"), roses + 1, "gathered");
        idle(&mut s, 4);
        let noticed = true_here && at == 22;
        assert_eq!(soldier(&s), noticed, "omen {true_here} at {at}:00");
        if noticed {
            let study = s.runtime(ZoneId::Cellar).unwrap().mark(sym(&s, "cellar_study")).unwrap().cell;
            let u = unit(&s, "rose_soldier");
            let (x, y) = u.pos.cell();
            assert!((x - i32::from(study.x)).abs() <= 3 && (y - i32::from(study.y)).abs() <= 3, "in the study");
        }
    }
}
