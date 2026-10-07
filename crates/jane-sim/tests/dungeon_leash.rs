//! WoW's instance rules in a dungeon (the owner, 2026-10-07; PLAN.md §2.6 *Leash*): nothing but
//! a boss has a leash there, it follows her until it or she is dead; but it never follows her
//! into sanctuary (the rest room, the threshold of a way out), nor steps into it itself: at that
//! edge it evades home, whole and untouchable until it is there. A boss keeps its row and its
//! lock-in; the county and the buildings keep their leash.
//!
//! The field: every zone 128 x 64 of grass, `start` at (10, 10); in the Mine (a dungeon) a rest
//! room of sanctuary over x 90..128, y 0..30, and a way out's threshold over x 0..8, y 40..48.

mod field;

use std::sync::Arc;

use field::{body_of, cmd, edit, events, learn, spell, start_sym, steps, unit};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::num::{dist_sq, isqrt};
use jane_core::{Blueprint, Cell, Key, Milli, Rect, Tile, Vec2, ZoneId};
use jane_sim::ai::{chases_to_the_end, leash_in};
use jane_sim::state::CombatState;
use jane_sim::units::{max_hp, new_unit};
use jane_sim::{Blueprints, Command, DevOp, Sim, UnitId};

const M: ZoneId = ZoneId::Mine;
const REST: Rect = Rect { x: 90, y: 0, w: 38, h: 30 };
const DOOR: Rect = Rect { x: 0, y: 40, w: 8, h: 8 };

fn world() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        if z == M {
            bp.indoor = true;
            bp.sanctuary = vec![REST, DOOR];
        }
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    s
}

/// Her into `z`, stood on cell (x, y).
fn put_her(s: &mut Sim, z: ZoneId, x: i32, y: i32) -> UnitId {
    cmd(s, Some(0), Command::Dev(DevOp::Tp { zone: z, mark: start_sym() }));
    steps(s, 2);
    let her = body_of(s, 0);
    edit(s, her, |u| u.pos = Vec2::centre(x, y));
    her
}

/// A `def` awake on cell (x, y) of `z`, its post there.
fn spawn_in(s: &mut Sim, z: ZoneId, def: &str, x: i32, y: i32) -> UnitId {
    let d = jane_data::catalog().combat.unit_id(def).unwrap();
    let st = s.state_mut();
    let id = st.next.unit();
    let mut u = new_unit(id, None, d, Vec2::centre(x, y), Facing::West, st.tick);
    u.awake = true;
    st.zone_mut(z).unwrap().insert_unit(u);
    s.rebuild_runtimes();
    id
}

/// On her, its post at (hx, hy), wounded to half.
fn on_her(s: &mut Sim, id: UnitId, her: UnitId, hx: i32, hy: i32) {
    edit(s, id, |u| {
        u.home = Vec2::centre(hx, hy);
        u.hp = Milli(u.hp.0 / 2);
        u.target = Some(her);
        u.combat = CombatState::Combat;
    });
}

fn state(s: &Sim, id: UnitId) -> (CombatState, Option<UnitId>) {
    let u = unit(s, id);
    (u.combat, u.target)
}

fn cells(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64)) / i64::from(jane_core::num::CELL_FX)
}

/// The rows: only a boss in a dungeon keeps a leash; the county's and a building's are as they
/// were (3.75 x aggro in the county, the yard's bones 7.5 x, 2.5 x in a building).
#[test]
fn only_a_dungeon_drops_the_leash_and_only_for_what_is_no_boss() {
    let cat = jane_data::catalog();
    let row = |id: &str| cat.combat.unit(cat.combat.unit_id(id).unwrap());
    let (bones, yard, boss) = (row("skeleton"), row("yard_bones"), row("headmaster"));
    for z in [ZoneId::Mine, ZoneId::Museum, ZoneId::Burial, ZoneId::School, ZoneId::Factory, ZoneId::Forest] {
        assert!(chases_to_the_end(z, bones), "{z:?}");
        assert!(!chases_to_the_end(z, boss), "a boss keeps its leash in {z:?}");
    }
    for z in [ZoneId::County, ZoneId::House, ZoneId::Cellar, ZoneId::Arms, ZoneId::Church] {
        assert!(!chases_to_the_end(z, bones), "{z:?}");
    }
    let aggro = i64::from(bones.aggro.0);
    assert_eq!(leash_in(ZoneId::County, bones) * 4, aggro * 15);
    assert_eq!(leash_in(ZoneId::County, yard) * 4, i64::from(yard.aggro.0) * 30);
    assert_eq!(leash_in(ZoneId::House, bones) * 2, aggro * 5);
    assert_eq!(leash_in(ZoneId::Mine, boss), i64::from(boss.leash.0));
}

/// In a dungeon a skeleton eighty-seven metres from its post keeps on her, and comes to her.
#[test]
fn in_a_dungeon_nothing_but_a_boss_lets_her_go() {
    let mut s = world();
    let her = put_her(&mut s, M, 40, 50);
    let foe = spawn_in(&mut s, M, "skeleton", 44, 50);
    on_her(&mut s, foe, her, 127, 63);
    steps(&mut s, 1);
    assert_eq!(state(&s, foe), (CombatState::Combat, Some(her)), "no leash in a dungeon");
    steps(&mut s, 60 * 20);
    assert_eq!(state(&s, foe), (CombatState::Combat, Some(her)), "twenty seconds on, still on her");
    assert!(cells(unit(&s, foe).pos, unit(&s, her).pos) <= 2, "and at her elbow");
}

/// The same pull in a building (the House) and in the county still lets go: an evade home.
#[test]
fn the_county_and_a_building_keep_their_leash() {
    for z in [ZoneId::County, ZoneId::House] {
        let mut s = world();
        let her = put_her(&mut s, z, 40, 50);
        let foe = spawn_in(&mut s, z, "skeleton", 44, 50);
        on_her(&mut s, foe, her, 127, 63);
        steps(&mut s, 1);
        let f = unit(&s, foe);
        assert_eq!((f.combat, f.target), (CombatState::Evade, None), "{z:?}");
        assert_eq!(f.hp, max_hp(f), "{z:?}: whole the tick it lets go");
    }
}

/// A boss in a dungeon pulled past its row's leash goes home as it always did (a leash, not an
/// evade: its arena is its leash), and the rest room does not turn it.
#[test]
fn a_boss_keeps_its_row() {
    let mut s = world();
    let her = put_her(&mut s, M, 40, 50);
    let boss = spawn_in(&mut s, M, "headmaster", 44, 50);
    on_her(&mut s, boss, her, 127, 63);
    steps(&mut s, 1);
    assert_eq!(state(&s, boss).0, CombatState::Leash);
    // Near home, and her in the rest room: it keeps on her.
    edit(&mut s, her, |u| u.pos = Vec2::centre(95, 10));
    let boss2 = spawn_in(&mut s, M, "headmaster", 86, 10);
    on_her(&mut s, boss2, her, 86, 10);
    steps(&mut s, 1);
    assert_eq!(state(&s, boss2), (CombatState::Combat, Some(her)));
}

/// She steps into the rest room: what was on her lets go at its door, whole, nothing lands on it
/// on the way, and it goes home. Stood inside, she is not noticed; out again, she is.
#[test]
fn the_rest_room_is_sanctuary() {
    let mut s = world();
    learn(&mut s, "spark");
    let her = put_her(&mut s, M, 92, 10);
    let foe = spawn_in(&mut s, M, "skeleton", 86, 10);
    on_her(&mut s, foe, her, 70, 10);
    steps(&mut s, 1);
    let f = unit(&s, foe);
    assert_eq!((f.combat, f.target), (CombatState::Evade, None), "it lets go at the door");
    assert_eq!(f.hp, max_hp(f), "whole");
    s.drain_events();
    cmd(&mut s, Some(0), Command::Dev(DevOp::Mp(100)));
    cmd(&mut s, Some(0), Command::Cast { spell: spell("spark"), on: Some(foe) });
    steps(&mut s, 30);
    let ev = events(&mut s);
    assert!(!ev.iter().any(|e| matches!(e.kind, jane_sim::EventKind::Damage { unit, .. } if unit == foe)));
    steps(&mut s, 60 * 15);
    let f = unit(&s, foe);
    assert_eq!((f.combat, f.target), (CombatState::Idle, None));
    assert!(cells(f.pos, Vec2::centre(70, 10)) <= 1, "home: {:?}", f.pos);
    // Inside, a few cells from a skeleton by the door, she is let be (a god is let be anywhere).
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(false)));
    let watcher = spawn_in(&mut s, M, "skeleton", 87, 12);
    edit(&mut s, her, |u| u.pos = Vec2::centre(91, 12));
    steps(&mut s, 60 * 3);
    assert_ne!(state(&s, watcher).1, Some(her), "nothing notices her in sanctuary");
    edit(&mut s, her, |u| u.pos = Vec2::centre(88, 14));
    steps(&mut s, 60);
    assert_eq!(state(&s, watcher), (CombatState::Combat, Some(her)), "out of it, she is fair game");
}

/// The threshold of a way out is sanctuary too, and a foe that would cut through the rest room
/// to get at her turns back at its door.
#[test]
fn the_way_out_is_sanctuary_and_nothing_walks_in() {
    let mut s = world();
    let her = put_her(&mut s, M, 3, 44);
    let foe = spawn_in(&mut s, M, "skeleton", 12, 44);
    on_her(&mut s, foe, her, 30, 44);
    steps(&mut s, 1);
    assert_eq!(state(&s, foe), (CombatState::Evade, None), "at the threshold");

    let mut s = world();
    let her = put_her(&mut s, M, 80, 40);
    let foe = spawn_in(&mut s, M, "skeleton", 92, 20);
    on_her(&mut s, foe, her, 60, 20);
    steps(&mut s, 1);
    assert_eq!(state(&s, foe), (CombatState::Evade, None), "it stood in the rest room");
    steps(&mut s, 60 * 20);
    let f = unit(&s, foe);
    assert_eq!(f.combat, CombatState::Idle);
    assert!(!REST.contains(f.pos.cell().0, f.pos.cell().1), "out of the rest room");
}
