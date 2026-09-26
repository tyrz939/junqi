//! The Lowfields side quests played end to end by the headless player on the real seed
//! (`jane/test/quests.test.ts` "the Lowfields side quests, played"): offered, accepted, done,
//! handed in, paid, and not payable twice; growth by a page or a jar found once. A to H, all but
//! what waits on the presence step (Plot 9's tenant, the living-world unit's): the TS's `strike`
//! is the bot walking up and meleeing, so a place on the way is visited on the way.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::ZoneId;
use jane_sim::input::DevOp;
use jane_sim::interact::FocusRef;
use jane_sim::{Command, Seat, Sim};

/// `dev tp` to a county mark: the long walks are the county's business, not this file's.
fn tp(s: &mut Sim, mark: &str) {
    let m = sym(s, mark);
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: m }));
    idle(s, 3);
    assert_eq!(zone_of(s), ZoneId::County);
}

/// Walk up to a prop and press USE on it, and on nothing else.
fn press(s: &mut Sim, key: &str, mark: Option<&str>) {
    if let Some(m) = mark {
        tp(s, m);
    }
    assert!(walk_to_prop(s, key), "walk to {key}");
    let id = prop_id(s, key);
    for _ in 0..6 {
        match s.view(Seat(0)).unwrap().focus().map(|f| f.target) {
            // Something lying in the way: pick it up.
            Some(FocusRef::Drop(_)) => cmd(s, Command::Use),
            _ => break,
        }
    }
    let f = s.view(Seat(0)).unwrap().focus().map(|f| f.target);
    assert_eq!(f, Some(FocusRef::Prop(id)), "USE beside {key} acts on {key}");
    cmd(s, Command::Use);
    idle(s, 2);
}

/// Press USE on a talking prop, check which node answers, and click through it.
fn read(s: &mut Sim, key: &str, node: &str, choices: &[u8], mark: Option<&str>) {
    press(s, key, mark);
    let d = s.view(Seat(0)).unwrap().dialogue().unwrap_or_else(|| panic!("{key} says nothing"));
    assert_eq!(d.node.map(|n| n.id), Some(node), "{key} answers with");
    talk_through(s, choices);
    assert!(s.view(Seat(0)).unwrap().dialogue().is_none());
    idle(s, 1);
}

fn expect_done(s: &Sim, quest: &str) {
    assert!(quest_done(s, quest), "{quest} is done");
    assert!(!quest_active(s, quest));
}

#[test]
fn lost_property_three_things_by_reading_then_the_carters_key_and_the_trunk() {
    let mut s = new_game();
    // A1. Offered from the first minute, by a book.
    read(&mut s, "lost_property_book", "lp_offer", &[0], Some("start"));
    assert!(quest_active(&s, "lost_property"));
    read(&mut s, "lost_property_book", "lp_wait", &[], None);
    // Each of the three places says what it is and what is lying at it.
    read(&mut s, "halt_well_head", "glove", &[], Some("halt_well"));
    press(&mut s, "lost_glove_drop", None);
    read(&mut s, "halt_well_head", "drawn", &[], None);
    read(&mut s, "halt_signpost_post", "hat", &[], Some("halt_signpost"));
    press(&mut s, "lost_hat_drop", None);
    let lp = jane_data::catalog().story.quest_id("lost_property").unwrap();
    assert!(!jane_sim::quests::ready(s.state(), lp));
    read(&mut s, "halt_cart_body", "tin", &[], Some("halt_cart"));
    press(&mut s, "lost_tin_drop", None);
    assert_eq!([holds(&s, "lost_glove"), holds(&s, "lost_hat"), holds(&s, "lost_tin")], [1, 1, 1]);
    assert!(prop(&s, "lost_tin_drop").hidden);
    let apples = holds(&s, "apple");
    read(&mut s, "lost_property_book", "lp_in", &[0], Some("start"));
    expect_done(&s, "lost_property");
    assert_eq!([holds(&s, "lost_glove"), holds(&s, "lost_hat"), holds(&s, "lost_tin")], [0, 0, 0]);
    assert_eq!(holds(&s, "apple"), apples + 2);
    // One article from the unclaimed shelf, and only one.
    let before = [holds(&s, "light_stone"), holds(&s, "small_water"), holds(&s, "grape")];
    read(&mut s, "unclaimed_shelf", "shelf_choose", &[0], None);
    read(&mut s, "unclaimed_shelf", "shelf_after", &[], None);
    assert_eq!(
        [holds(&s, "light_stone"), holds(&s, "small_water"), holds(&s, "grape")],
        [before[0] + 1, before[1], before[2]]
    );

    // A2. The trunk was there all along, and is locked.
    read(&mut s, "lost_property_book", "ll_offer", &[0], None);
    press(&mut s, "left_luggage_trunk", None);
    assert!(prop(&s, "left_luggage_trunk").locked);
    read(&mut s, "lost_property_book", "ll_wait", &[], None);
    read(&mut s, "carters_cart_body", "key", &[], Some("carters_cart"));
    press(&mut s, "carters_key_drop", None);
    read(&mut s, "carters_cart_body", "stood", &[], None);
    read(&mut s, "carters_note", "read", &[], None);
    assert_eq!(holds(&s, "key_left_luggage"), 1);
    let been = jane_sim::state::FlagKey::Been(sym(&s, "carters_cart"));
    assert_eq!(s.state().flags.get(&been), Some(&1));
    let spirit = me(&s).spirit;
    tp(&mut s, "start");
    press(&mut s, "left_luggage_trunk", None); // the key turns, and is consumed
    assert_eq!(holds(&s, "key_left_luggage"), 0);
    assert!(!quest_done(&s, "left_luggage"));
    press(&mut s, "left_luggage_trunk", None); // the lid: what is inside, and the hand-in
    expect_done(&s, "left_luggage");
    let page = sym(&s, "page_left_luggage");
    assert!(s.state().growth.found.contains(&page));
    assert_eq!(me(&s).spirit, spirit + 3);
    assert_eq!(s.state().growth.spirit, 3);
    assert_eq!(holds(&s, "gold_dust"), 2);

    // A3. The same platform, after the bell. She is standing on it when the bell goes: credited
    // without stepping off.
    read(&mut s, "lost_property_book", "tbc_offer", &[0], None);
    tp(&mut s, "start");
    let after_nine = jane_sim::state::FlagKey::Been(sym(&s, "platform_after_nine"));
    assert_eq!(s.state().flags.get(&after_nine), None);
    assert!(prop(&s, "night_parcel").hidden);
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 22 }));
    idle(&mut s, 3);
    assert_eq!(s.state().flags.get(&after_nine), Some(&1));
    assert!(prop(&s, "night_parcel").hidden, "she never sees it arrive");
    read(&mut s, "lost_property_book", "tbc_in", &[0], None);
    expect_done(&s, "to_be_collected");
    assert_eq!(holds(&s, "light_stone"), 2, "one from the shelf, one for checking");
    read(&mut s, "lost_property_book", "closed", &[], None);
    // Walk away and come back: it is there. Come back by day: it is not.
    tp(&mut s, "yard_gate");
    tp(&mut s, "start");
    assert!(!prop(&s, "night_parcel").hidden);
    tp(&mut s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    tp(&mut s, "start");
    assert!(prop(&s, "night_parcel").hidden);
    tp(&mut s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 22 }));
    tp(&mut s, "start");
    // The decision: a page, and the platform lamp never lights again.
    press(&mut s, "night_parcel", None);
    let page = sym(&s, "page_night_parcel");
    assert!(s.state().growth.found.contains(&page));
    assert_eq!(me(&s).spirit, spirit + 6);
    assert!(prop(&s, "night_parcel").hidden);
    assert!(prop(&s, "station_lamp").hidden && !prop(&s, "station_lamp_dead").hidden);
    read(&mut s, "lost_property_book", "closed_taken", &[], None);
    tp(&mut s, "yard_gate");
    tp(&mut s, "start");
    assert!(prop(&s, "night_parcel").hidden, "taken is taken");
}

fn night(s: &mut Sim) {
    cmd(s, Command::Dev(DevOp::Time { hour: 22 }));
    assert!(s.state().is_night());
}

fn day(s: &mut Sim) {
    cmd(s, Command::Dev(DevOp::Time { hour: 10 }));
}

fn flag(s: &Sim, name: &str) -> Option<i32> {
    s.state().flags.get(&jane_sim::state::FlagKey::Named(sym(s, name))).copied()
}

fn been(s: &Sim, name: &str) -> Option<i32> {
    s.state().flags.get(&jane_sim::state::FlagKey::Been(sym(s, name))).copied()
}

#[test]
fn the_nurses_round_three_notes_her_coat_and_a_parcel_through_mrs_allens_door() {
    // quests.test.ts "D. The Nurse's Round".
    let mut s = new_game();
    // The car keeps its planks; once they are taken, USE reads the glovebox.
    press(&mut s, "car_wreck", Some("nurses_case"));
    assert_eq!(holds(&s, "wood"), 2);
    read(&mut s, "car_wreck", "glovebox_first", &[0], None);
    // Nobody is seeing anyone. Without the parcel the door has nothing to hand in.
    read(&mut s, "door_allen", "allen_before", &[], Some("allen_door"));
    read(&mut s, "note_allen", "read", &[], Some("cottage_allen"));
    read(&mut s, "note_pike", "read", &[], Some("cottage_pike"));
    let round = jane_data::catalog().story.quest_id("the_nurses_round").unwrap();
    assert!(!jane_sim::quests::ready(s.state(), round));
    read(&mut s, "note_crane", "read", &[], Some("cottage_crane"));
    // The second sheet is under the first: handing in the round opens her warning.
    read(&mut s, "car_wreck", "glovebox_in_1", &[0, 0], Some("nurses_case"));
    expect_done(&s, "the_nurses_round");
    assert_eq!(holds(&s, "potion_manashield"), 1);
    assert!(quest_active(&s, "her_coat"));
    read(&mut s, "car_wreck", "glovebox_wait_2", &[], None);

    press(&mut s, "nurses_case", None);
    assert!(prop(&s, "nurses_case").locked);
    press(&mut s, "nurses_coat_drop", Some("nurses_coat"));
    assert_eq!(holds(&s, "key_nurses_case"), 1);
    assert_eq!(been(&s, "nurses_coat"), Some(1));
    press(&mut s, "nurses_case", Some("nurses_case"));
    assert_eq!(holds(&s, "key_nurses_case"), 0);
    press(&mut s, "nurses_case", None);
    expect_done(&s, "her_coat");
    assert_eq!(holds(&s, "potion_lifesteal"), 1);
    assert_eq!(holds(&s, "nurses_parcel"), 1);
    let errand = jane_data::catalog().story.quest_id("mrs_allens_dressing").unwrap();
    assert!(jane_sim::quests::ready(s.state(), errand), "the case gives the last errand, already in hand");
    read(&mut s, "car_wreck", "glovebox_empty", &[], None);

    let strength = me(&s).strength;
    read(&mut s, "door_allen", "allen_in", &[0], Some("allen_door"));
    expect_done(&s, "mrs_allens_dressing");
    assert_eq!(holds(&s, "nurses_parcel"), 0);
    assert!(s.state().growth.found.contains(&sym(&s, "jar_mrs_allen")));
    assert_eq!(me(&s).strength, strength + 3);
    read(&mut s, "door_allen", "allen_after", &[], None);
    assert_eq!(me(&s).strength, strength + 3, "a jar is found once");
}

#[test]
fn the_lampman_twelve_thirteen_fifteen_after_the_bell_a_fire_lit_somewhere_new_and_the_dog() {
    // quests.test.ts "E. The Lampman".
    let mut s = new_game();
    read(&mut s, "pell_stone", "pell_first", &[0], Some("pell_shrine"));
    // By day there is no telling a dead lamp from a live one.
    read(&mut s, "lamp_12", "day", &[], Some("lamp_12"));
    assert_eq!(been(&s, "lamp_12"), None);
    night(&mut s);
    for lamp in ["lamp_12", "lamp_13", "lamp_15"] {
        read(&mut s, lamp, "night", &[], Some(lamp));
    }
    read(&mut s, "pell_stone", "pell_in", &[1], Some("pell_shrine")); // "Fourteen, then"
    expect_done(&s, "number_fourteen");
    assert_eq!(flag(&s, "said_fourteen"), Some(1));
    assert_eq!(holds(&s, "light_stone"), 2);

    // The brazier wants what the car holds, and what the mine's stair wants.
    read(&mut s, "pell_stone", "pell_second", &[0], None);
    assert!(!prop(&s, "pell_brazier_cold").hidden && prop(&s, "pell_brazier").hidden);
    read(&mut s, "pell_brazier_cold", "brazier_empty", &[], Some("pell_brazier"));
    press(&mut s, "car_wreck", Some("nurses_case"));
    assert_eq!([holds(&s, "wood"), holds(&s, "fire_stone")], [2, 1]);
    read(&mut s, "pell_brazier_cold", "brazier_ready", &[0], Some("pell_brazier"));
    expect_done(&s, "the_lampmans_brazier");
    assert_eq!([holds(&s, "wood"), holds(&s, "fire_stone")], [0, 0]);
    assert!(prop(&s, "pell_brazier_cold").hidden && !prop(&s, "pell_brazier").hidden);
    // It rests and saves like any other fire.
    read(&mut s, "pell_brazier", "fire", &[], None);
    assert_eq!(s.state().rest.map(|r| r.zone), Some(ZoneId::County));
    read(&mut s, "pell_stone", "pell_after", &[], Some("pell_shrine"));

    // Next morning, once, below every hand-in and above the poke chain.
    let skeleton = jane_data::catalog().story.quest_id("defeat_skeleton").unwrap();
    s.state_mut().quests.done.push(skeleton); // past the intro, as in rest.test
    day(&mut s);
    idle(&mut s, 40);
    tp(&mut s, "house_front");
    let talk = |s: &mut Sim| {
        assert!(walk_to_unit(s, "dog"));
        cmd(s, Command::Use);
        let node = s.view(Seat(0)).unwrap().dialogue().and_then(|d| d.node).map(|n| n.id);
        talk_through(s, &[]);
        node
    };
    assert_eq!(talk(&mut s), Some("lamps_counted_f"));
    assert_eq!(flag(&s, "dog_heard_lamps"), Some(1));
    assert_eq!(talk(&mut s), Some("idle"));
}

#[test]
fn the_garden_book_three_roses_before_the_bell_and_a_potion_the_dog_wants_to_see() {
    // quests.test.ts "F. The Garden Book".
    let mut s = new_game();
    read(&mut s, "garden_book", "book_first", &[0], Some("garden_book"));
    tp(&mut s, "sallow_jetty");
    for n in 1..=3 {
        press(&mut s, &format!("sallow_rose_{n}"), None);
    }
    assert_eq!(holds(&s, "white_water_rose"), 3);
    read(&mut s, "garden_book", "book_in_1", &[0], Some("garden_book"));
    expect_done(&s, "before_the_bell");
    // "two rocks and two vials of water on the ground by the book that you would swear were not there"
    assert!(!prop(&s, "garden_book_gift").hidden);
    press(&mut s, "garden_book_gift", None);
    assert_eq!([holds(&s, "rock"), holds(&s, "small_water")], [2, 2]);
    assert_eq!(holds(&s, "white_water_rose"), 3, "the roses are kept: the next page needs one");
    read(&mut s, "garden_book", "book_second", &[0], None);
    read(&mut s, "garden_book", "book_wait_2", &[], None);
    cmd(
        &mut s,
        Command::Dev(DevOp::Give { item: jane_data::catalog().combat.item_id("potion_stoneskin").unwrap(), qty: 1 }),
    );
    let q = jane_data::catalog().story.quest_id("rose_and_stone").unwrap();
    assert!(jane_sim::quests::ready(s.state(), q));
    // At night the book only says the dog is not there. It cannot hand in.
    night(&mut s);
    read(&mut s, "garden_book", "book_in_2", &[], None);
    assert!(!quest_done(&s, "rose_and_stone"));
    day(&mut s);
    tp(&mut s, "yard_gate");
    idle(&mut s, 40);
    tp(&mut s, "house_front");
    assert!(walk_to_unit(&mut s, "dog"));
    cmd(&mut s, Command::Use);
    assert_eq!(s.view(Seat(0)).unwrap().dialogue().and_then(|d| d.node).map(|n| n.id), Some("stoneskin_done"));
    talk_through(&mut s, &[0]);
    expect_done(&s, "rose_and_stone");
    assert_eq!(holds(&s, "potion_stoneskin"), 1, "shown, not taken");
    assert_eq!([holds(&s, "honeylace_lily"), holds(&s, "gold_dust")], [2, 1]);
    read(&mut s, "garden_book", "book_after", &[], Some("garden_book"));
}

#[test]
fn the_right_of_way_stile_to_stile_and_a_haversack_found_not_offered() {
    // quests.test.ts "H. The Right of Way".
    let mut s = new_game();
    read(&mut s, "parish_board", "path_offer", &[0], Some("town_square"));
    read(&mut s, "parish_board", "path_wait", &[], None);
    // Told once; then the board moves on to its next notice.
    read(&mut s, "parish_board", "rats_offer", &[1], None);
    read(&mut s, "fingerpost_town", "fingerpost_plain", &[], Some("hedge_stile_town"));
    assert_eq!(been(&s, "hedge_stile_town"), Some(1));
    let q = jane_data::catalog().story.quest_id("footpath_three").unwrap();
    assert!(!jane_sim::quests::ready(s.state(), q));
    let apples = holds(&s, "apple");
    read(&mut s, "fingerpost_farm", "fingerpost_in", &[0], Some("hedge_stile_farm"));
    expect_done(&s, "footpath_three");
    assert_eq!(holds(&s, "apple"), apples + 3);
    assert_eq!(flag(&s, "footpath_walked"), Some(1));

    // Picking it up IS the offer, and it is already in hand.
    press(&mut s, "haversack_drop", Some("hedge_tree"));
    let found = jane_data::catalog().story.quest_id("if_found").unwrap();
    assert!(jane_sim::quests::ready(s.state(), found));
    read(&mut s, "lost_property_book", "haversack_in", &[0], Some("start"));
    expect_done(&s, "if_found");
    assert_eq!(holds(&s, "haversack"), 0);
    assert_eq!([holds(&s, "hemshade_root"), holds(&s, "small_water")], [3, 2]);
    read(&mut s, "lost_property_book", "lp_offer", &[1], None);
}

/// What a fight comes to (`quests.test.ts strike`, which struck from afar): she walks up and puts
/// it down with the bar. These tests are about the quests, not the balance: with the AI in, the
/// rest of a group comes for her too, so she fights with the console's god mode on (hits still
/// land on her; none hurts), as the TS's remote strike never let anything reach her.
fn strike(s: &mut Sim, key: &str) {
    let u = unit(s, key);
    assert!(!u.hidden, "{key} is there");
    cmd(s, Command::Dev(DevOp::God(true)));
    assert!(walk_to(s, u.pos, jane_core::Fx::from_px(16)), "walk to {key}");
    assert!(fight(s, key), "{key} goes down");
    cmd(s, Command::Dev(DevOp::God(false)));
    idle(s, 2);
}

#[test]
fn the_company_five_men_paid_off_and_the_one_who_never_came_down() {
    // quests.test.ts "G. The Company".
    let mut s = new_game();
    // Before the gang is stood down the slate is only a slate.
    read(&mut s, "tally_slate", "slate_plain", &[], Some("quarry_camp"));
    read(&mut s, "company_notice", "company_offer", &[0], Some("company_notice"));
    for key in ["quarryman_a_1", "quarryman_a_2", "quarryman_a_3", "quarryman_b_1"] {
        strike(&mut s, key);
    }
    read(&mut s, "company_notice", "company_wait", &[], Some("company_notice"));
    strike(&mut s, "quarryman_b_2");
    read(&mut s, "company_notice", "company_in", &[0], Some("company_notice"));
    expect_done(&s, "stood_down");
    assert_eq!([holds(&s, "iron"), holds(&s, "wood")], [4, 2]);
    read(&mut s, "company_notice", "company_after", &[], None);

    // The TS struck from afar; walking up to the gang she has been to the quarry top already, and
    // a place visited before the quest was taken still counts (the `Been` flag).
    read(&mut s, "tally_slate", "slate_offer", &[0], Some("quarry_camp"));
    tp(&mut s, "quarry_top");
    assert_eq!(been(&s, "quarry_top"), Some(1));
    read(&mut s, "quarry_adit", "look", &[], None);
    let spirit = me(&s).spirit;
    read(&mut s, "tally_slate", "slate_in", &[0], Some("quarry_camp"));
    expect_done(&s, "down_at_five");
    assert!(s.state().growth.found.contains(&sym(&s, "page_quarry")));
    assert_eq!(me(&s).spirit, spirit + 3);
    read(&mut s, "tally_slate", "slate_after", &[], None);
}

#[test]
fn the_allotments_one_notice_at_a_time_six_rats_and_the_shed() {
    // quests.test.ts "B. The Allotments", to the shed (Plot 9's tenant keeps his hours by the
    // presence step, the living-world unit's).
    let mut s = new_game();
    // The board shows one notice. Refuse the footpath and it moves on.
    read(&mut s, "parish_board", "path_offer", &[1], Some("town_square"));
    read(&mut s, "parish_board", "rats_offer", &[0], None);
    read(&mut s, "parish_board", "rats_wait", &[], None);
    tp(&mut s, "allotment_shed");
    let rats = jane_data::catalog().story.quest_id("rats_in_the_sheds").unwrap();
    for key in ["rat_shed_1", "rat_shed_2", "rat_shed_3", "rat_shed_4", "rat_allotment_1"] {
        strike(&mut s, key);
    }
    assert!(!jane_sim::quests::ready(s.state(), rats));
    strike(&mut s, "rat_allotment_2");
    assert!(jane_sim::quests::ready(s.state(), rats));
    press(&mut s, "allotment_shed", None);
    assert!(prop(&s, "allotment_shed").locked, "members only");
    read(&mut s, "parish_board", "rats_in", &[0], Some("town_square"));
    expect_done(&s, "rats_in_the_sheds");
    assert_eq!(holds(&s, "key_generic"), 1);
    assert_eq!(holds(&s, "pansy"), 2);
    press(&mut s, "allotment_shed", Some("allotment_shed"));
    press(&mut s, "allotment_shed", None);
    assert_eq!(holds(&s, "key_generic"), 0);
    assert_eq!(holds(&s, "honeylace_lily"), 2);
}

#[test]
fn lowfield_farm_three_scarecrows_by_day_the_same_after_the_bell_and_six_that_are_not_turnips() {
    // quests.test.ts "C. Lowfield Farm".
    let mut s = new_game();
    let scarecrows = ["scarecrow_gate", "scarecrow_hedge", "scarecrow_top"];
    read(&mut s, "farm_door", "farmer_first", &[0], Some("farm_door"));
    read(&mut s, "farm_door", "farmer_wait_1", &[], None);
    for k in scarecrows {
        read(&mut s, k, "day", &[], Some(k));
    }
    let apples = holds(&s, "apple");
    read(&mut s, "farm_door", "farmer_in_1", &[0], Some("farm_door"));
    expect_done(&s, "three_scarecrows");
    assert_eq!(holds(&s, "apple"), apples + 4);

    // The same walk with the county's one rule applied to it: by day it does not count.
    read(&mut s, "farm_door", "farmer_second", &[0], None);
    read(&mut s, "scarecrow_gate", "day", &[], Some("scarecrow_gate"));
    let after = jane_data::catalog().story.quest_id("after_the_bell").unwrap();
    assert!(!jane_sim::quests::ready(s.state(), after));
    night(&mut s);
    for k in scarecrows {
        read(&mut s, k, "night", &[], Some(k));
    }
    assert!(jane_sim::quests::ready(s.state(), after));
    read(&mut s, "farm_door", "farmer_in_2", &[0], Some("farm_door"));
    expect_done(&s, "after_the_bell");
    assert_eq!(holds(&s, "potion_stoneskin"), 1);
    assert_eq!(holds(&s, "nasturtium"), 2);

    // By day, as he said. Seven stand in the field; six are asked for.
    day(&mut s);
    read(&mut s, "farm_door", "farmer_third", &[0], None);
    tp(&mut s, "top_field");
    let turnips = jane_data::catalog().story.quest_id("not_turnips").unwrap();
    for n in 1..=5 {
        strike(&mut s, &format!("pumpkin_top_{n}"));
    }
    assert!(!jane_sim::quests::ready(s.state(), turnips));
    strike(&mut s, "pumpkin_top_6");
    let strength = me(&s).strength;
    read(&mut s, "farm_door", "farmer_in_3", &[0], Some("farm_door"));
    expect_done(&s, "not_turnips");
    assert!(s.state().growth.found.contains(&sym(&s, "jar_farm")));
    assert_eq!(me(&s).strength, strength + 3);
    read(&mut s, "farm_door", "farmer_idle", &[], None);
}
