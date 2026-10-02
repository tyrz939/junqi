//! The Waters and the Works side content (PLAY-PLAN.md Phase 5.4 to 5.8; QUESTS.md §2.3, §2.4):
//! every quest in `data/quests/waters.json`, `works.json` and `tuesday.json` leaves the county
//! changed when it is handed in (a consequence row on it, WORLD.md §6), pays after the first link of
//! its chain in growth, keys, materials or fires and never in food (QUESTS.md K10), and the side
//! content is no longer piled up in the Lowfields.

use std::path::Path;

use jane_core::action::{Action, Condition};

/// What only feeds her: allowed on the first link of a chain and nowhere else (K10).
const FOOD: [&str; 3] = ["apple", "grape", "small_water"];

/// The first link of each chain or household, where food is still allowed.
const FIRST_LINKS: [&str; 14] = [
    "the_toll",
    "cutting_order",
    "one_hook_empty",
    "the_planting_book",
    "overdue",
    "road_closed",
    "nobody_has_drowned",
    "loveday_landing",
    "tuesday_round",
    "shift_rota",
    "dinners_out",
    "the_foremans_diary",
    "register_of_burials",
    "breen_coal",
];

fn ids(file: &str) -> Vec<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/quests").join(file);
    let raw = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    v.as_object().expect("an object of quests").keys().cloned().collect()
}

fn new_quests() -> Vec<String> {
    ["waters.json", "works.json", "tuesday.json"].iter().flat_map(|f| ids(f)).collect()
}

#[test]
fn every_new_quest_changes_the_county_when_it_is_done() {
    let c = jane_data::catalog();
    let mut bad = Vec::new();
    for id in new_quests() {
        let q = c.story.quest_id(&id).expect("a quest");
        let row = c.living.consequences.iter().find(|r| r.on == Condition::QuestDone(q));
        match row {
            None => bad.push(format!("{id}: no consequence row")),
            Some(r) => {
                let edits = c.list(r.edits);
                if !edits.iter().any(|a| !matches!(a, Action::Flag { .. })) {
                    bad.push(format!("{id}: its consequence is a flag alone"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn after_the_first_link_nothing_pays_in_food() {
    let c = jane_data::catalog();
    let food: Vec<_> = FOOD.iter().map(|f| c.combat.item_id(f).expect("an item")).collect();
    let mut bad = Vec::new();
    for id in new_quests() {
        if FIRST_LINKS.contains(&id.as_str()) {
            continue;
        }
        let q = c.story.quest(c.story.quest_id(&id).expect("a quest"));
        let pays: Vec<&Action> = c.list(q.rewards).iter().collect();
        assert!(!pays.is_empty(), "{id} pays nothing");
        if pays.iter().any(|a| matches!(a, Action::Give(s) if food.contains(&s.item))) {
            bad.push(id);
        }
    }
    assert!(bad.is_empty(), "food after the first link: {bad:?}");
}

#[test]
fn the_waters_and_the_works_carry_side_content_of_their_own() {
    let waters = ids("waters.json").len();
    let works = ids("works.json").len();
    println!("Waters {waters} new quests, Works {works}, the Tuesday round 1 more");
    assert!(waters >= 15, "{waters}");
    assert!(works >= 15, "{works}");
}
