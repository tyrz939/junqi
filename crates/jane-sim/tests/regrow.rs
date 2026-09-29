//! Food comes back (WORLD.md §4.6, `regrow.rs`): an apple tree she empties is full again some
//! days on and not a tick before; the time it is due survives a save and a load; the party
//! shares it (a tree one seat stripped is bare for the other until it comes back); and Julie's
//! pantry fills again with its staples, not with what it first held.

mod common;

use common::bot::*;
use common::{bps, new_game};
use jane_core::action::Facing;
use jane_core::{Tick, ZoneId};
use jane_sim::input::DevOp;
use jane_sim::interact::{FocusRef, loot_of};
use jane_sim::regrow::{Refill, refill_of};
use jane_sim::state::LootState;
use jane_sim::{ClientToken, Command, PropId, Seat, Sim, StampedCommand, StepInput};

const DAY: u32 = 24 * 7200;

fn county_prop(s: &Sim, id: PropId) -> jane_sim::Prop {
    s.state().zone(ZoneId::County).unwrap().props.iter().find(|p| p.id == id).unwrap().clone()
}

/// Stand seat `seat` south of an apple tree that holds apples, facing it with it in focus.
/// The tree's id.
fn at_a_full_tree(s: &mut Sim, seat: u8) -> PropId {
    let cat = jane_data::catalog();
    let tree = cat.story.prop_id("apple_tree").unwrap();
    let bp = bps();
    let bp = bp.get(ZoneId::County);
    let trees: Vec<_> = s
        .state()
        .zone(ZoneId::County)
        .unwrap()
        .props
        .iter()
        .filter(|p| p.def == tree && !p.used && !loot_of(bp, p).is_empty())
        .map(|p| (p.id, p.cell))
        .collect();
    assert!(trees.len() >= 4, "the county has apple trees with apples on ({})", trees.len());
    for (id, cell) in trees {
        let (x, y) = (i32::from(cell.x), i32::from(cell.y) + 2);
        if s.runtime(ZoneId::County).unwrap().grid.solid(x, y) {
            continue;
        }
        place_seat(s, seat, x, y, Facing::North);
        idle(s, 1);
        let focus = s.view(Seat(seat)).unwrap().focus();
        if focus.is_some_and(|f| f.target == FocusRef::Prop(id)) {
            return id;
        }
    }
    panic!("no apple tree to stand under");
}

fn apples(s: &Sim, seat: usize) -> u32 {
    jane_sim::bag::bag_count(&s.state().players[seat].bag[..], jane_data::catalog().combat.item_id("apple").unwrap())
}

/// Jump the clock to just before `at`, then step onto it.
fn to_just_before(s: &mut Sim, at: Tick) {
    s.state_mut().tick = Tick(at.0 - 2);
    idle(s, 1);
    assert_eq!(s.state().tick, Tick(at.0 - 1));
}

#[test]
fn a_picked_tree_comes_back_after_its_days_and_not_before() {
    let mut s = new_game();
    let id = at_a_full_tree(&mut s, 0);
    let before = apples(&s, 0);
    let picked = s.state().tick;
    cmd(&mut s, Command::Use);
    assert!(apples(&s, 0) > before, "she has the apples");
    let tree = county_prop(&s, id);
    assert!(tree.used && refill_of(&tree) == Some(Refill::Spawn));
    let at = tree.regrow.expect("its clock started");
    let days = at.0 - picked.0;
    assert!((5 * DAY / 2..=7 * DAY / 2).contains(&days), "about three days ({days} ticks)");
    // Bare meanwhile: pressing again gives nothing.
    let held = apples(&s, 0);
    cmd(&mut s, Command::Use);
    assert_eq!(apples(&s, 0), held);

    to_just_before(&mut s, at);
    assert!(county_prop(&s, id).used, "not a tick early");
    idle(&mut s, 1);
    let tree = county_prop(&s, id);
    assert!(!tree.used && tree.regrow.is_none() && tree.loot == LootState::AsSpawned, "{tree:?}");
    place_seat(&mut s, 0, i32::from(tree.cell.x), i32::from(tree.cell.y) + 2, Facing::North);
    idle(&mut s, 1);
    cmd(&mut s, Command::Use);
    assert!(apples(&s, 0) > held, "picked again");
}

#[test]
fn save_and_load_keep_the_clock() {
    let mut s = new_game();
    let id = at_a_full_tree(&mut s, 0);
    cmd(&mut s, Command::Use);
    let at = county_prop(&s, id).regrow.expect("its clock started");
    let mut t = Sim::from_save_with(&s.save(), bps()).expect("loads");
    assert_eq!(t.state(), s.state());
    assert_eq!(county_prop(&t, id).regrow, Some(at));
    for sim in [&mut s, &mut t] {
        to_just_before(sim, at);
        assert!(county_prop(sim, id).used);
        idle(sim, 1);
        assert!(!county_prop(sim, id).used, "back at the same tick, loaded or not");
    }
    assert_eq!(t.hash(), s.hash());
}

#[test]
fn the_party_shares_the_orchard() {
    let mut s = new_game();
    cmd(&mut s, Command::Open(true));
    let cmds = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(11) } }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
    assert_eq!(s.state().party_size(), 2);
    let start = sym(&s, "start");
    cmd_as(&mut s, 1, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    let id = at_a_full_tree(&mut s, 0);
    cmd(&mut s, Command::Use);
    let at = county_prop(&s, id).regrow.expect("its clock started");
    // The other seat under the same tree finds it bare.
    let tree = county_prop(&s, id);
    place_seat(&mut s, 1, i32::from(tree.cell.x), i32::from(tree.cell.y) + 2, Facing::North);
    idle(&mut s, 1);
    let f = s.view(Seat(1)).unwrap().focus();
    assert!(f.is_none_or(|f| f.target != FocusRef::Prop(id)), "nothing to pick");
    let theirs = apples(&s, 1);
    cmd_as(&mut s, 1, Command::Use);
    assert_eq!(apples(&s, 1), theirs, "stripped for everyone");
    to_just_before(&mut s, at);
    idle(&mut s, 1);
    cmd_as(&mut s, 1, Command::Use);
    assert!(apples(&s, 1) > theirs, "and back for everyone");
}

#[test]
fn julies_pantry_fills_again_with_its_staples() {
    let cat = jane_data::catalog();
    let mut s = new_game();
    let front = sym(&s, "front");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    assert_eq!(zone_of(&s), ZoneId::House);
    assert!(walk_to_prop(&mut s, "pantry_chest"));
    let dust = holds(&s, "gold_dust");
    cmd(&mut s, Command::Use);
    assert!(holds(&s, "gold_dust") > dust);
    let pantry = prop(&s, "pantry_chest");
    let Some(Refill::Restock(staples)) = refill_of(&pantry) else { panic!("the pantry is a larder") };
    let at = pantry.regrow.expect("its clock started");
    to_just_before(&mut s, at);
    assert!(prop(&s, "pantry_chest").used);
    idle(&mut s, 1);
    assert_eq!(prop(&s, "pantry_chest").loot, LootState::Left(staples.to_vec()));
    let (dust, water, grapes) = (holds(&s, "gold_dust"), holds(&s, "small_water"), holds(&s, "grape"));
    cmd(&mut s, Command::Use);
    assert_eq!(holds(&s, "gold_dust"), dust, "the makings are not a staple");
    assert!(holds(&s, "small_water") > water && holds(&s, "grape") > grapes);
    // The fruit bowl is a larder too; any other chest is not (taken is taken).
    assert!(matches!(refill_of(&prop(&s, "fruit_bowl")), Some(Refill::Restock(_))));
    let mut other = prop(&s, "pantry_chest");
    assert_eq!(other.def, cat.story.prop_id("chest").unwrap());
    other.key = sym(&s, "front");
    assert_eq!(refill_of(&other), None);
}
