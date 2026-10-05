//! PLAY-PLAN.md §2.2's L0 and L1 proofs for made fires: with empty bags she can always reach a save
//! and a heal, and every made fire can be made.
//!
//! - every place the skeleton counts as a rest (the proof that each region has one inside its
//!   base threat, `skeleton::build`) stands a *kept* fire: a `rest` prop with a light, never a pit;
//! - deadwood (a stump, a woodpile, a log) within 12 cells of every made fire's pit;
//! - a match source before the first cold pit: the Halt trunk by the fire she steps off the train
//!   beside, and Julie's kitchen drawer, kept full;
//! - two or three old grates, and only Fire lights them.

mod common;

use jane_core::{Blueprint, Key, ZoneId};
use jane_world::county::{PIT_WOOD_CELLS, build_proven};

fn seeds() -> u32 {
    common::seeds().min(16)
}

fn county(seed: u32) -> Blueprint {
    build_proven(seed).expect("a county")
}

fn at(p: &jane_core::blueprint::PropSpawn) -> (i32, i32) {
    (i32::from(p.cell.x), i32::from(p.cell.y))
}

#[test]
fn every_made_fire_has_deadwood_within_twelve_cells() {
    let cat = jane_data::catalog();
    for seed in 1..=seeds() {
        let bp = county(seed);
        let woods: Vec<(i32, i32)> = bp.props.iter().filter(|p| cat.story.prop(p.def).wood > 0).map(at).collect();
        let mut pits = 0;
        for p in bp.props.iter().filter(|p| cat.story.prop(p.def).made) {
            pits += 1;
            let (x, y) = at(p);
            let r2 = PIT_WOOD_CELLS * PIT_WOOD_CELLS;
            assert!(
                woods.iter().any(|&(wx, wy)| (wx - x) * (wx - x) + (wy - y) * (wy - y) <= r2),
                "seed {seed}: the {} at {x},{y} has no deadwood within {PIT_WOOD_CELLS} cells",
                cat.story.prop(p.def).id
            );
        }
        assert!(pits >= 3, "seed {seed}: made fires to make ({pits})");
    }
}

#[test]
fn every_rest_place_keeps_a_kept_fire() {
    let cat = jane_data::catalog();
    for seed in 1..=seeds() {
        let bp = county(seed);
        let mut seen = 0;
        for site in cat.county.sites.iter().filter(|s| s.rest) {
            let name = format!("site_{}", site.id);
            let Some(i) = bp.local_names.iter().position(|n| *n == name) else { continue };
            let Some(&rect) = bp.rects.get(&Key::Local(i as u32)) else { continue };
            seen += 1;
            let kept = bp.props.iter().any(|p| {
                let d = cat.story.prop(p.def);
                d.rest && d.light.is_some() && !d.made && !p.hidden && rect.contains(at(p).0, at(p).1)
            });
            assert!(kept, "seed {seed}: {} keeps a fire that is always lit", site.id);
        }
        assert!(seen >= 3, "seed {seed}: the rest places were found ({seen})");
    }
}

#[test]
fn matches_wait_by_the_first_fire_and_in_julies_drawer() {
    let cat = jane_data::catalog();
    let m = cat.combat.item_id("match").expect("a match");
    for seed in 1..=seeds().min(4) {
        let bp = county(seed);
        let start = bp.marks[&Key::Name(cat.name_id("start").unwrap())].cell;
        let near = bp.props.iter().any(|p| {
            let (dx, dy) = (i32::from(p.cell.x) - i32::from(start.x), i32::from(p.cell.y) - i32::from(start.y));
            dx * dx + dy * dy <= 40 * 40 && p.loot.iter().any(|s| s.item == m)
        });
        assert!(near, "seed {seed}: a box of matches by the Halt fire");
    }
    let drawer = cat.living.tuning.regrow.restock.iter().find(|r| cat.name(r.prop) == "kitchen_drawer");
    assert!(drawer.is_some_and(|r| r.loot.iter().any(|s| s.item == m)), "Julie's drawer fills again with matches");
    let house = jane_world::build_zone(ZoneId::House, 1).expect("the house");
    assert!(house.props.iter().any(|p| p.loot.iter().any(|s| s.item == m)), "the drawer is in her kitchen");
}

#[test]
fn two_or_three_old_grates_and_only_fire_lights_them() {
    let cat = jane_data::catalog();
    let grate = cat.story.prop_id("old_grate").expect("an old grate");
    assert!(cat.story.prop(grate).fire_only && cat.story.prop(grate).made);
    for seed in 1..=seeds().min(8) {
        let n = county(seed).props.iter().filter(|p| p.def == grate).count();
        assert!((2..=3).contains(&n), "seed {seed}: {n} old grates");
    }
}
