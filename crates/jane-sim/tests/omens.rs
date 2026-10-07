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
use jane_sim::tuning::TICKS_PER_HOUR;
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

// --- the five built with the clock's minutes and weekdays, lamp runs and the bridge -------------

/// The clock set to a tick before `hour:minute` of `day` (0 is New Game's Sunday) and run across
/// it, so the clock rows of that mark fire; what the step said.
fn cross(s: &mut Sim, day: u32, hour: u32, minute: u32) -> Vec<jane_sim::Event> {
    let at = hour * TICKS_PER_HOUR + minute * TICKS_PER_HOUR / 60;
    s.drain_events();
    let st = s.state_mut();
    st.day = day;
    st.clock = at - 1;
    idle(s, 2);
    s.drain_events().to_vec()
}

/// The toasts in `events` that say `text`.
fn says(events: &[jane_sim::Event], text: &str) -> usize {
    let cat = jane_data::catalog();
    events
        .iter()
        .filter(|e| match e.kind {
            EventKind::Toast(ToastKind::Text(jane_core::TextRef::Text(t))) => cat.text(t) == text,
            _ => false,
        })
        .count()
}

/// The School's bell rung in `events` (`EventKind::Bell`, the audio's cue): strike counts.
fn bells(events: &[jane_sim::Event]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Bell { strikes, church: false, at } => {
                assert!(at.is_some_and(|(z, _)| z == ZoneId::County), "the bell hangs on the hill: {at:?}");
                assert!(e.to.is_none() && e.in_zone.is_none(), "heard by the whole party, wherever they are");
                Some(strikes)
            }
            _ => None,
        })
        .collect()
}

fn flag(s: &Sim, name: &str) -> i32 {
    s.state().syms.find(name).and_then(|k| s.state().flags.get(&FlagKey::Named(k)).copied()).unwrap_or(0)
}

const BELL: &str = "A school bell, a long way off. Nine o'clock.";

/// "The lamps on the east road go out at ten": true, at ten every lamp of the east road's run
/// goes dark, and the next evening they are lit again; false, they burn all night. The lamps by
/// the Museum's door and the bridge's are not the run: the next lit place is never far.
#[test]
fn the_east_road_lamps_go_out_at_ten_some_nights() {
    let e = jane_data::catalog().county.furnishing.east_road.expect("an east road");
    let run = |s: &Sim| -> Vec<bool> {
        s.state().zone(ZoneId::County).unwrap().props.iter().filter(|p| p.def == e.lamp).map(|p| p.on).collect()
    };
    let lit = |s: &Sim| {
        let v = s.view(Seat(0)).unwrap();
        let st = s.state().zone(ZoneId::County).unwrap();
        st.props.iter().filter(|p| p.def == e.lamp).filter(|p| v.light_showing(p).is_some()).count()
    };
    for true_here in [true, false] {
        let mut s = common::new_game();
        set_omen(&mut s, "east_lamps", true_here);
        let n = run(&s).len();
        assert!(n >= 2, "the east road has lamps: {n}");
        assert!(run(&s).iter().all(|&on| on), "switched on at New Game");
        cross(&mut s, 0, 21, 0);
        assert_eq!(lit(&s), n, "all lit at nine, omen {true_here}");
        cross(&mut s, 0, 22, 0);
        assert_eq!(lit(&s), if true_here { 0 } else { n }, "at ten, omen {true_here}");
        cross(&mut s, 1, 18, 0);
        assert!(run(&s).iter().all(|&on| on), "switched on again for the evening, omen {true_here}");
    }
}

/// "It went before nine on Tuesday": true, on a Tuesday the bell rings at ten to nine and not at
/// nine; any other day, or on a seed it is false, at nine. The night keeps its hour either way.
#[test]
fn the_bell_goes_early_on_tuesdays_some_weeks() {
    for true_here in [true, false] {
        for (day, tuesday) in [(2, true), (9, true), (1, false), (3, false)] {
            let mut s = common::new_game();
            set_omen(&mut s, "early_bell", true_here);
            let early = true_here && tuesday;
            let at_ten_to = cross(&mut s, day, 20, 50);
            assert_eq!(says(&at_ten_to, BELL), usize::from(early), "20:50 day {day}, omen {true_here}");
            assert_eq!(bells(&at_ten_to), if early { vec![9] } else { vec![] }, "the bell rung at 20:50");
            assert!(!s.state().is_night(), "the night keeps its hour");
            let at_nine = cross(&mut s, day, 21, 0);
            assert_eq!(says(&at_nine, BELL), usize::from(!early), "21:00 day {day}, omen {true_here}");
            assert_eq!(bells(&at_nine), if early { vec![] } else { vec![9] }, "the bell rung at nine");
            assert!(s.state().is_night());
        }
    }
}

/// The School's bell lets the county out at six (six strikes), and the church rings five for
/// evensong at six in the evening: not the same bell (Miss Orme).
#[test]
fn the_bell_rings_out_at_six_and_the_church_rings_for_evensong() {
    let mut s = common::new_game();
    assert_eq!(bells(&cross(&mut s, 1, 6, 0)), vec![6]);
    let ev = cross(&mut s, 1, 18, 0);
    assert!(bells(&ev).is_empty());
    let church: Vec<u8> = ev
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Bell { strikes, church: true, at } => {
                assert!(at.is_some(), "the church's bell hangs at the church");
                Some(strikes)
            }
            _ => None,
        })
        .collect();
    assert_eq!(church, vec![5]);
}

/// "Does not always stop": the Sunday train whistles twice when it stops (it comes in, and goes)
/// and leaves the Sunday sacks for the post office; true, one Sunday in three (the first after
/// she came, then every third) it whistles once and leaves nothing. Only on Sundays.
#[test]
fn the_sunday_train_does_not_always_stop() {
    const ONCE: &str = "A train whistle, a long way off.";
    const TWICE: &str = "A train whistle, a long way off. A while after, another.";
    for true_here in [true, false] {
        let mut s = common::new_game();
        set_omen(&mut s, "train_through", true_here);
        let mut through = Vec::new();
        for week in 1..=6 {
            let monday = cross(&mut s, 7 * week + 1, 13, 0);
            assert_eq!(says(&monday, ONCE) + says(&monday, TWICE), 0, "no train on a Monday");
            let ev = cross(&mut s, 7 * week, 13, 0);
            let stopped = says(&ev, TWICE) == 1;
            assert_eq!(says(&ev, ONCE) + says(&ev, TWICE), 1, "one train on a Sunday");
            assert_eq!(flag(&s, "sunday_sacks"), i32::from(stopped), "the sacks, week {week}");
            through.push(!stopped);
            cross(&mut s, 7 * week + 1, 6, 0);
            assert_eq!(flag(&s, "sunday_sacks"), 0, "taken up on Monday morning");
        }
        let want = if true_here { vec![true, false, false, true, false, false] } else { vec![false; 6] };
        assert_eq!(through, want, "omen {true_here}");
    }
    // The sacks are on the platform when she comes up to the Halt after a train that stopped.
    let mut s = common::new_game();
    set_omen(&mut s, "train_through", false);
    assert!(prop(&s, "sunday_sacks").hidden, "none at New Game");
    cross(&mut s, 7, 13, 0);
    tp(&mut s, ZoneId::County, "house_front");
    tp(&mut s, ZoneId::County, "start");
    assert!(!prop(&s, "sunday_sacks").hidden, "on the platform after a Sunday's train");
    cross(&mut s, 8, 6, 0);
    tp(&mut s, ZoneId::County, "house_front");
    tp(&mut s, ZoneId::County, "start");
    assert!(prop(&s, "sunday_sacks").hidden, "gone on Monday");
}

/// "The bridge counts who crosses": true, the third time in a night she steps onto the east road's
/// river bridge its two lamps go out together, until the next evening; the count starts again in
/// the morning. By day, or on a seed it is false, a bridge is a bridge.
#[test]
fn the_bridge_counts_who_crosses_some_nights() {
    let cat = jane_data::catalog();
    let e = cat.county.furnishing.east_road.expect("an east road");
    for (true_here, at) in [(true, 23), (true, 12), (false, 23)] {
        let mut s = common::new_game();
        set_omen(&mut s, "bridge_counts", true_here);
        tp(&mut s, ZoneId::County, "start");
        hour(&mut s, at);
        let name = sym(&s, "east_bridge");
        let bridge = *s.runtime(ZoneId::County).unwrap().rects.get(&name).expect("the east bridge");
        let lamps = |s: &Sim| -> Vec<bool> {
            s.state()
                .zone(ZoneId::County)
                .unwrap()
                .props
                .iter()
                .filter(|p| p.def == e.bridge_lamp)
                .map(|p| p.on)
                .collect()
        };
        assert!(!lamps(&s).is_empty(), "the bridge has its lamps");
        let (cx, cy) = (bridge.x + bridge.w / 2, bridge.y + bridge.h / 2);
        for n in 1..=3 {
            // Off the bridge, then onto it.
            place(&mut s, bridge.x - 6, cy, jane_core::action::Facing::East);
            idle(&mut s, 2);
            s.drain_events();
            place(&mut s, cx, cy, jane_core::action::Facing::East);
            idle(&mut s, 2);
            let counts = true_here && at == 23;
            assert_eq!(
                flag(&s, "bridge_crossings"),
                if counts { n } else { 0 },
                "crossing {n}, omen {true_here} at {at}"
            );
            let dark = counts && n == 3;
            assert_eq!(lamps(&s).iter().all(|&on| !on), dark, "crossing {n}, omen {true_here} at {at}");
        }
    }
}

/// "Nobody has drowned in this lake": when she first comes near the lake, whoever stands on its
/// bank by night is decided: true, something that is not a person (and it fights); false, a man
/// wet to the collar who says nobody ever drowned in it. By day nobody is there.
#[test]
fn nobody_has_drowned_in_the_lake_some_seeds() {
    let cat = jane_data::catalog();
    for true_here in [true, false] {
        let mut s = common::new_game();
        set_omen(&mut s, "nobody_drowned", true_here);
        hour(&mut s, 23);
        tp(&mut s, ZoneId::County, "lake_statue_mouth");
        idle(&mut s, 40);
        let u = unit(&s, "lake_figure");
        let want = if true_here { "lake_shape" } else { "lake_man" };
        assert_eq!(cat.combat.unit(u.def).id, want, "omen {true_here}");
        assert!(!u.hidden, "on the bank at night");
        hour(&mut s, 12);
        tp(&mut s, ZoneId::County, "start");
        tp(&mut s, ZoneId::County, "lake_statue_mouth");
        idle(&mut s, 40);
        assert!(unit(&s, "lake_figure").hidden, "nobody on the bank by day, omen {true_here}");
    }
}

/// The rows in the order WORLD.md §7.3 documents: the three first shipped first, so no seed's
/// roll of them moved when the five were added after; every region claims at least two; at most
/// one lethal row a region can come true (`omens::roll`).
#[test]
fn the_omen_rows_keep_their_order_and_every_region_has_two() {
    use jane_data::Region;
    let rows = jane_data::catalog().story.omens;
    let ids: Vec<&str> = rows.iter().map(|o| o.id).collect();
    let want = [
        "scarecrow_closer",
        "mine_no_exit",
        "roses_after_dark",
        "east_lamps",
        "early_bell",
        "train_through",
        "bridge_counts",
        "nobody_drowned",
    ];
    assert_eq!(ids, want);
    for r in [Region::Lowfields, Region::Waters, Region::Works] {
        assert!(rows.iter().filter(|o| o.region == r).count() >= 2, "{r:?}");
    }
    for seed in 0..500 {
        let trues = jane_sim::omens::roll(rows, &mut jane_core::Sfc32::seeded(seed, 1));
        for r in [Region::Lowfields, Region::Waters, Region::Works] {
            assert!(trues.iter().filter(|&&i| rows[i].lethal && rows[i].region == r).count() <= 1);
        }
    }
}
