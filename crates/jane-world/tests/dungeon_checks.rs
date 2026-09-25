//! PORT.md §6.m stage 15: checks C1 to C12 really reject, each its own bad case, and only that
//! check fires. Carries `jane/test/dungeon-gen.test.ts` "the checks really reject, each by name:
//! C1 and C3 to C12", `forest.test.ts` "the checks really reject" and `library.test.ts` "C5
//! really rejects". Each test builds a proven dungeon, breaks exactly the thing one check
//! guards (in the blueprint, or in the mission it is judged against), and asserts that that
//! check, and no other, fires.

use jane_core::action::Action;
use jane_core::blueprint::PropSpawn;
use jane_core::{Blueprint, Key, ZoneId};
use jane_data::{MissionDef, MissionNodeKind, catalog};
use jane_world::dungeon::checks::{Check, Fault, ORDER, check};
use jane_world::dungeon::{BuildInfo, Built, build};
use jane_world::solve::report::name_of;

fn key(bp: &Blueprint, name: &str) -> Key {
    if let Some(i) = bp.local_names.iter().position(|n| n == name) {
        return Key::Local(i as u32);
    }
    Key::Name(catalog().name_id(name).unwrap_or_else(|| panic!("no name \"{name}\"")))
}

fn prop<'a>(bp: &'a mut Blueprint, name: &str) -> &'a mut PropSpawn {
    let k = key(bp, name);
    bp.props.iter_mut().find(|p| p.key == k).unwrap_or_else(|| panic!("no prop \"{name}\""))
}

fn text(faults: &[Fault]) -> String {
    faults.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
}

/// The faults, all of them from `check`, and at least one.
#[track_caller]
fn only(want: Check, faults: &[Fault]) -> String {
    let t = text(faults);
    assert!(!faults.is_empty(), "{} did not fire", want.name());
    assert!(faults.iter().all(|f| f.check == want), "wanted only {}, got:\n{t}", want.name());
    t
}

fn proven(zone: ZoneId, seed: u32) -> Built {
    let b = build(zone, seed);
    assert!(b.info.errors.is_empty(), "{:?}", b.info.errors);
    assert_eq!(check(&b.blueprint, &b.info), Vec::new(), "{} seed {seed} is proven", zone.name());
    b
}

/// The blueprint broken by `change`, judged.
fn broken(b: &Built, change: impl FnOnce(&mut Blueprint)) -> Vec<Fault> {
    let mut bp = b.blueprint.clone();
    change(&mut bp);
    check(&bp, &b.info)
}

/// The same blueprint, judged against a mission that asks for something else.
fn against(b: &Built, change: impl FnOnce(&mut MissionDef)) -> Vec<Fault> {
    let mut m = *b.info.mission;
    change(&mut m);
    let info = BuildInfo { mission: Box::leak(Box::new(m)), ..b.info.clone() };
    check(&b.blueprint, &info)
}

fn node(m: &MissionDef, id: &str) -> usize {
    m.node_index(id).unwrap_or_else(|| panic!("no node {id}"))
}

fn mine() -> Built {
    proven(ZoneId::Mine, 31)
}

#[test]
fn c1_an_unlocked_gate_does_not_hold() {
    let t = only(Check::C1, &broken(&mine(), |bp| prop(bp, "gate_generic_b").locked = false));
    assert!(t.contains("the lock on store -> core does not hold"), "{t}");
}

#[test]
fn c1_steps_that_are_not_there_do_not_hold() {
    let t = only(Check::C1, &broken(&mine(), |bp| prop(bp, "broken_steps").hidden = true));
    assert!(t.contains("the lock on core -> gallery does not hold"), "{t}");
}

#[test]
fn c3_a_layout_that_drops_a_plain_key_strands_her() {
    // C3 at build time searched the whole mission; here it searches the rooms this seed placed.
    // Say the layout dropped the plate room (and its key): the one plain key left can go on the
    // guard room's lock, and then the hub is out of reach. Only C3 reads what was dropped.
    let b = mine();
    let mut info = b.info.clone();
    let plate = node(info.mission, "plate") as u8;
    info.layout.as_mut().unwrap().dropped.push(plate);
    let t = only(Check::C3, &check(&b.blueprint, &info));
    assert!(t.contains("C3: opening plain locks in the order [entry-guard] strands her short of core"), "{t}");
}

#[test]
fn c3_one_plain_key_for_two_plain_locks_strands_her() {
    // `dungeon-gen.test.ts`: the store holds nothing, so one plain key must choose between two locks.
    let t = text(&against(&mine(), |m| {
        let store = node(m, "store");
        let mut nodes = m.nodes.to_vec();
        nodes[store].holds = &[];
        m.nodes = Box::leak(nodes.into_boxed_slice());
    }));
    assert!(t.contains("C3: opening plain locks in the order [entry-guard] strands her"), "{t}");
}

#[test]
fn c4_a_sink_that_eats_more_than_the_zone_supplies() {
    // The broken track (a shortcut, which the solver does not need) wants more iron than there is.
    let t = only(
        Check::C4,
        &broken(&mine(), |bp| {
            prop(bp, "broken_track").needs[0].qty += 1;
        }),
    );
    assert!(t.contains("C4: the zone can eat 9 iron and supplies 8"), "{t}");
}

#[test]
fn c5_a_teacher_with_its_guard_still_up() {
    let t = only(
        Check::C5,
        &broken(&mine(), |bp| {
            let free = key(bp, "hm_drawer_free");
            bp.triggers.shift_remove(&free);
            prop(bp, "hm_drawer").locked = false;
        }),
    );
    assert!(t.contains("C5: repair can be learned in office while headmaster is still up"), "{t}");
}

#[test]
fn c5_the_teachers_room_has_nothing_to_try_the_spell_on() {
    // `library.test.ts`: take the planter's answer away (here: stand a row that answers nothing,
    // of its size, in its place) and the room no longer teaches Grow in safety.
    let b = proven(ZoneId::Library, 31);
    let t = only(
        Check::C5,
        &broken(&b, |bp| {
            let cat = catalog();
            let p = prop(bp, "library_planter");
            let d = cat.story.prop(p.def);
            let plain = cat
                .story
                .props
                .iter()
                .position(|q| {
                    q.w == d.w && q.h == d.h && q.answers.is_none() && !q.solid && !q.gate && !q.plate && !q.rest
                })
                .expect("a plain row of the planter's size");
            p.def = jane_core::PropDefId(plain as u16);
        }),
    );
    assert!(t.contains("C5: shelf teaches grow and has nothing to try it on"), "{t}");
}

#[test]
fn c6_a_way_in_the_clear_does_not_hide_again() {
    let t = only(
        Check::C6,
        &broken(&mine(), |bp| {
            let (clear, way_in) = (key(bp, "mine_arena_clear"), key(bp, "mine_arena_wayin"));
            let list = bp.triggers[&clear].actions;
            let jane_core::ListRef::Blueprint(i) = list else { panic!("a generated list") };
            bp.lists[usize::from(i)].retain(|a| *a != Action::Hide(way_in));
        }),
    );
    assert!(t.contains("the clear and the reset must hide it"), "{t}");
}

#[test]
fn c6_a_way_in_that_is_not_hidden_is_a_way_round_the_gate() {
    // `dungeon-gen.test.ts`. A way in that stands open is a door into the arena, so C1 (the boss
    // gate no longer holds) fires beside it: the two checks guard the same thing from two sides.
    let f = broken(&mine(), |bp| prop(bp, "mine_arena_wayin").hidden = false);
    let t = text(&f);
    assert!(t.contains("C6: mine_arena_wayin is not hidden, so it is a way round gate_boss"), "{t}");
    assert!(f.iter().all(|f| matches!(f.check, Check::C6 | Check::C1)), "{t}");
}

#[test]
fn c7_the_rest_room_too_late_on_the_walk() {
    let t = only(
        Check::C7,
        &against(&mine(), |m| m.budget.rest_at = Some((jane_core::Permille(900), jane_core::Permille(1000)))),
    );
    assert!(t.contains("C7: the rest room is first reached"), "{t}");
}

#[test]
fn c7_a_rest_room_with_nowhere_to_rest() {
    let b = mine();
    let rest = b.info.rooms.iter().find(|r| b.info.mission.nodes[r.node].kind == MissionNodeKind::Rest).unwrap().rect;
    let t = only(
        Check::C7,
        &broken(&b, |bp| {
            let cat = catalog();
            for p in &mut bp.props {
                let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
                if rest.contains(x, y) && cat.story.prop(p.def).rest {
                    // A stove that is not one: the same size, and nothing to rest at.
                    let d = cat.story.prop(p.def);
                    let plain = cat
                        .story
                        .props
                        .iter()
                        .position(|q| {
                            q.w == d.w
                                && q.h == d.h
                                && !q.rest
                                && q.solid == d.solid
                                && q.answers.is_none()
                                && !q.gate
                                && !q.plate
                        })
                        .unwrap();
                    p.def = jane_core::PropDefId(plain as u16);
                }
            }
        }),
    );
    assert!(t.contains("C7: the rest room has nowhere to rest"), "{t}");
}

#[test]
fn c8_the_first_completion_outside_its_band() {
    let t = only(Check::C8, &against(&mine(), |m| m.budget.crit_path_cells = (10, 20)));
    assert!(t.contains("C8: the first completion is") && t.contains("outside 10 to 20"), "{t}");
}

#[test]
fn c9_the_boss_too_far_from_the_rest_room() {
    let t = only(Check::C9, &against(&mine(), |m| m.budget.rest_to_boss_cells = 10));
    assert!(t.contains("C9: the boss is") && t.contains("over 10"), "{t}");
}

#[test]
fn c9_no_shortcut_was_placed() {
    let t = only(
        Check::C9,
        &against(&mine(), |m| {
            let edges: Vec<_> = m.edges.iter().map(|e| jane_data::MissionEdge { shortcut: false, ..*e }).collect();
            m.edges = Box::leak(edges.into_boxed_slice());
        }),
    );
    assert!(t.contains("C9: no shortcut was placed"), "{t}");
}

#[test]
fn c10_a_gate_opened_the_first_time_it_is_seen() {
    // Handed the key to the assembly hall at the door, she opens its gate the first time she
    // sees it: no tease. (The Works' boss room keeps its order however early its gate opens.)
    let b = proven(ZoneId::Factory, 31);
    let t = only(
        Check::C10,
        &against(&b, |m| {
            let boss = m.nodes.iter().position(|n| n.kind == MissionNodeKind::Boss).unwrap();
            let tag = b
                .info
                .locks
                .iter()
                .find_map(|l| match l.kind {
                    jane_data::MissionEdgeKind::Key { tag, .. } if usize::from(m.edges[l.edge].to) == boss => Some(tag),
                    _ => None,
                })
                .expect("a keyed boss gate");
            let mut keys = m.given_keys.to_vec();
            keys.push(tag);
            m.given_keys = Box::leak(keys.into_boxed_slice());
        }),
    );
    assert!(t.contains("C10: factory_gate_assembly is not seen before it can be opened"), "{t}");
}

#[test]
fn c10_the_mines_boss_door_handed_its_key_at_the_door() {
    // `dungeon-gen.test.ts`. In the mine the arena then opens out of order too, and through the
    // nook's shortcut the gallery with it, so C1 says so beside C10.
    let boss = catalog().name_id("mine_boss").unwrap();
    let f = against(&mine(), |m| m.given_keys = Box::leak(Box::new([boss])));
    let t = text(&f);
    assert!(t.contains("C10: gate_boss is not seen before it can be opened"), "{t}");
    assert!(f.iter().all(|f| matches!(f.check, Check::C10 | Check::C1)), "{t}");
}

#[test]
fn c11_a_plate_that_relocks_with_nothing_to_push_onto_it() {
    // A plate whose release locks something again, in a dungeon that places one on this seed:
    // turn the pushables of its room into rows that do not move.
    let cat = catalog();
    let relocking = |bp: &Blueprint| {
        bp.props.iter().any(|p| {
            cat.story.prop(p.def).plate
                && p.release.and_then(|r| bp.list(r)).is_some_and(|l| l.iter().any(|a| matches!(a, Action::Lock(_))))
        })
    };
    let (zone, seed) = cat
        .dungeons
        .missions
        .iter()
        .flat_map(|m| (1..40).map(move |s| (m.zone, s)))
        .find(|&(z, s)| relocking(&build(z, s).blueprint))
        .expect("some dungeon places a plate that relocks");
    let b = proven(zone, seed);
    let mut plates = Vec::new();
    let t = only(
        Check::C11,
        &broken(&b, |bp| {
            let rooms: Vec<_> = bp
                .props
                .iter()
                .filter(|p| {
                    cat.story.prop(p.def).plate
                        && p.release
                            .and_then(|r| bp.list(r))
                            .is_some_and(|l| l.iter().any(|a| matches!(a, Action::Lock(_))))
                })
                .filter_map(|p| {
                    plates.push(name_of(bp, p.key).to_owned());
                    b.info
                        .rooms
                        .iter()
                        .find(|r| r.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y)))
                        .map(|r| r.rect)
                })
                .collect();
            for p in &mut bp.props {
                let d = cat.story.prop(p.def);
                if d.push && rooms.iter().any(|r| r.contains(i32::from(p.cell.x), i32::from(p.cell.y))) {
                    let still = cat
                        .story
                        .props
                        .iter()
                        .position(|q| {
                            q.w == d.w && q.h == d.h && !q.push && !q.carry && q.solid && !q.gate && q.answers.is_none()
                        })
                        .unwrap();
                    p.def = jane_core::PropDefId(still as u16);
                }
            }
        }),
    );
    assert!(plates.iter().any(|p| t.contains(&format!("C11: nothing can be pushed onto {p}"))), "{t}");
}

#[test]
fn c12_a_trigger_that_drops_a_gate_on_whoever_tripped_it() {
    let t = only(
        Check::C12,
        &broken(&mine(), |bp| {
            let (lock, all) = (key(bp, "mine_arena_lock"), key(bp, "mine_all"));
            bp.triggers.get_mut(&lock).unwrap().rect = all;
        }),
    );
    assert!(t.contains("C12: trigger mine_arena_lock would put gate_boss on top of whoever tripped it"), "{t}");
}

#[test]
fn the_forest_checks_really_reject() {
    // `forest.test.ts`: the lock-in, the light on the bank, and the boss glade behind it.
    let b = proven(ZoneId::Forest, 31);
    let t = only(
        Check::C12,
        &broken(&b, |bp| {
            let (lock, all) = (key(bp, "forest_stone_lock"), key(bp, "forest_all"));
            bp.triggers.get_mut(&lock).unwrap().rect = all;
        }),
    );
    assert!(t.contains("forest_hedge_gate"), "{t}");
    let t = text(&broken(&b, |bp| prop(bp, "forest_stone_wayin").hidden = false));
    assert!(t.contains("C6"), "{t}");
    // Take the bank away and the boss glade cannot be reached at all.
    let t = text(&broken(&b, |bp| prop(bp, "forest_stone_bank").use_list = None));
    assert!(
        t.contains("solver: prop \"forest_stone\" is unreachable") || t.contains("C1: stone is never reached"),
        "{t}"
    );
}

#[test]
fn a_refusal_by_the_solver_is_the_solvers_alone() {
    // A contract name gone: the solver refuses before any check is asked.
    let t = only(
        Check::Solver,
        &broken(&mine(), |bp| {
            let gate = key(bp, "gate_boss");
            bp.props.retain(|p| p.key != gate);
        }),
    );
    assert!(t.contains("solver: missing prop \"gate_boss\""), "{t}");
}

#[test]
fn the_checks_run_in_the_designed_order() {
    // Break three at once, out of order: the faults come out C1, then C8, then C12.
    let b = mine();
    let mut bp = b.blueprint.clone();
    let (lock, all) = (key(&bp, "mine_arena_lock"), key(&bp, "mine_all"));
    bp.triggers.get_mut(&lock).unwrap().rect = all;
    prop(&mut bp, "gate_generic_b").locked = false;
    let mut m = *b.info.mission;
    m.budget.crit_path_cells = (10, 20);
    let info = BuildInfo { mission: Box::leak(Box::new(m)), ..b.info.clone() };
    let faults = check(&bp, &info);
    let mut order: Vec<Check> = faults.iter().map(|f| f.check).collect();
    order.dedup();
    assert_eq!(order, vec![Check::C1, Check::C8, Check::C12], "{}", text(&faults));
    // And ORDER is the TypeScript's: the walk (C8) before the rest room that reads it (C7).
    let pos = |c: Check| ORDER.iter().position(|&x| x == c).unwrap();
    assert!(pos(Check::C8) < pos(Check::C7));
    assert_eq!(ORDER.len(), 11, "C1, C3 to C12; C2 falls out of C1");
}
