//! The legibility pass on the Factory, the Museum, the Pipes and the Library (DUNGEONS.md §3.2
//! to §3.4, "Is it clear what to do?"), as `dungeon_cues.rs` holds it for the Gold Mine: the
//! cues a first-time player reads, there on every seed. A board beside each way on from the way
//! in, the hub and the mini-boss's room names where it goes; the notice's plan names the same
//! rooms in the same words; the quest log names the room its step is in.

use jane_core::ZoneId;
use jane_data::{MissionNodeKind, catalog};
use jane_world::dungeon::build;

fn lines_of(id: &str) -> String {
    let cat = catalog();
    let d = cat.story.dialogue_id(id).unwrap_or_else(|| panic!("no dialogue {id}"));
    cat.story.dialogue[d.index()]
        .nodes
        .iter()
        .flat_map(|n| n.lines)
        .map(|l| cat.text(l.text))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The steps of a quest as the log shows them.
fn quest_steps(id: &str) -> Vec<&'static str> {
    let cat = catalog();
    let q = cat.story.quest_id(id).unwrap_or_else(|| panic!("no quest {id}"));
    cat.story.quests[q.index()].requirements.iter().map(|r| cat.text(r.text)).collect()
}

/// Every way on from the way in, the hub and a mini-boss's room has its board, on seeds 1 to 16.
fn boards_by_every_way_on(zone: ZoneId, notice: &str) {
    let cat = catalog();
    let plan = lines_of(notice);
    for seed in 1..=16 {
        let b = build(zone, seed);
        assert!(b.info.errors.is_empty(), "{zone:?} seed {seed}: {:?}", b.info.errors);
        let (bp, m) = (&b.blueprint, b.info.mission);
        let layout = b.info.layout.as_ref().expect("a generated layout");
        for r in &b.info.rooms {
            if !matches!(
                m.nodes[r.node].kind,
                MissionNodeKind::Entrance | MissionNodeKind::Hub | MissionNodeKind::Miniboss
            ) {
                continue;
            }
            for &di in &r.doors {
                let other = layout
                    .corridors
                    .iter()
                    .find_map(|c| {
                        if c.a.node == r.node && c.a.door == di {
                            Some(c.b.node)
                        } else if c.b.node == r.node && c.b.door == di {
                            Some(c.a.node)
                        } else {
                            None
                        }
                    })
                    .expect("a door in use has a corridor");
                let want = format!("board_{}_{}", m.id, m.nodes[other].id);
                assert!(cat.story.dialogue_id(&want).is_some(), "{zone:?}: no {want} row");
                let d = &r.shape.doors[di];
                let (dx, dy) = (r.x + i32::from(d.x), r.y + i32::from(d.y));
                let board = bp.props.iter().find(|p| {
                    p.talk.is_some_and(|t| cat.story.dialogue[t.index()].id == want)
                        && (i32::from(p.cell.x) - dx).abs() <= 5
                        && (i32::from(p.cell.y) - dy).abs() <= 5
                });
                let board = board.unwrap_or_else(|| {
                    panic!(
                        "{zone:?} seed {seed}: no {want} board by {}'s door to {}",
                        m.nodes[r.node].id, m.nodes[other].id
                    )
                });
                // The prompt beside the door reads the board's name, and the plan says it too.
                let speaker = cat.text(cat.story.dialogue[board.talk.expect("read").index()].speaker);
                let label = board.label.map(|l| match l {
                    jane_core::TextRef::Text(t) => cat.text(t),
                    jane_core::TextRef::Local(_) => "",
                });
                assert_eq!(label, Some(speaker), "{zone:?} seed {seed}: {want}");
                assert!(plan.contains(speaker), "{zone:?}: the plan does not name {speaker}");
            }
        }
    }
}

#[test]
fn every_way_on_from_the_factorys_decisions_has_its_name_painted_beside_it() {
    boards_by_every_way_on(ZoneId::Factory, "notice_factory");
}

#[test]
fn the_factorys_plan_the_press_hall_and_the_quest_log_say_where_the_foreman_is() {
    let plan = lines_of("notice_factory");
    for room in ["LOCKERS", "No. 1 LINE", "GENERATOR HALL", "STORES", "FOREMAN'S OFFICE", "ASSEMBLY"] {
        assert!(plan.contains(room), "the plan does not name {room}");
    }
    // The wall the call box brings down is tied, on the wall and in the toast, to the office gate.
    assert!(lines_of("notice_press").contains("FOREMAN'S OFFICE gate"));
    let steps = quest_steps("the_factory");
    assert!(steps.iter().any(|s| s.contains("ASSEMBLY") && s.contains("FOREMAN'S OFFICE")), "{steps:?}");
    // The Shop Key says what it opens.
    let cat = catalog();
    let key = cat.combat.item(cat.combat.item_id("key_shop").expect("the shop key"));
    assert!(cat.text(key.description).contains("Factory floor"));
    // The press hall's power shutter is named beside its door (a door a state lifts is boarded
    // from any room), wherever the seed keeps the stores.
    let stores = cat.story.dialogue_id("board_factory_stores").expect("the stores board row");
    for seed in 1..=16 {
        let b = build(ZoneId::Factory, seed);
        let kept = b.info.rooms.iter().any(|r| b.info.mission.nodes[r.node].id == "stores");
        let boarded = b.blueprint.props.iter().any(|p| p.talk == Some(stores));
        assert_eq!(boarded, kept, "seed {seed}");
    }
}

#[test]
fn every_way_on_from_the_museums_decisions_has_its_name_painted_beside_it() {
    boards_by_every_way_on(ZoneId::Museum, "notice_museum");
}

#[test]
fn the_museums_plan_names_rooms_not_compass_points_and_its_boards_say_what_opens_them() {
    // "ARTS west. SCIENCE east." was false on most seeds: the wings go where the lattice puts them.
    let plan = lines_of("notice_museum");
    for word in ["north", "south", "east", "west"] {
        let lower = plan.to_lowercase();
        assert!(!lower.split(|c: char| !c.is_alphabetic()).any(|w| w == word), "{word}: {plan}");
    }
    assert!(plan.contains("PAINTED OVER") && plan.contains("Open ten to four"), "{plan}");
    // The dark is what empties the ARTS doorway, and the light is what lifts the STORES shutter.
    assert!(lines_of("board_museum_arts").contains("while the lights are on"));
    assert!(lines_of("board_museum_stores").contains("Power shutter"));
    assert!(lines_of("board_museum_science").contains("ARTS"), "the floor key is from ARTS");
}

#[test]
fn every_way_on_from_the_pipes_sump_and_junction_has_its_name_stencilled_beside_it() {
    boards_by_every_way_on(ZoneId::Pipes, "notice_pipes");
}

#[test]
fn the_pipes_say_which_penstock_the_valve_lifts_and_where_the_outfall_comes_up() {
    // "East run to the Works" sent her east; it is the OUTFALL's ladder that comes up in the Works.
    let plan = lines_of("notice_pipes");
    assert!(plan.contains("OUTFALL's ladder comes up in the Works yard"), "{plan}");
    assert!(!plan.contains("East run to the Works"));
    let steps = quest_steps("the_factory");
    assert!(steps.iter().any(|s| s.contains("OUTFALL")), "{steps:?}");
    // Each way the valve turns, the toast names the run that opens.
    let cat = catalog();
    let m = cat.dungeons.mission_of(ZoneId::Pipes).expect("the pipes");
    let valve = m.nodes.iter().flat_map(|n| n.holds).find(|h| h.controls.is_some()).expect("the valve");
    let b = build(ZoneId::Pipes, 1);
    let mut said = Vec::new();
    for &list in valve.becomes.iter().flatten() {
        jane_world::solve::rows::each_action(&b.blueprint, cat, list, &mut |a| {
            if let jane_core::Action::Toast(jane_core::TextRef::Text(t)) = *a {
                said.push(cat.text(t));
            }
        });
    }
    assert!(said.iter().any(|t| t.contains("WEST RUN") && t.contains("penstock lifts")), "{said:?}");
    assert!(said.iter().any(|t| t.contains("EAST RUN drains, and its penstock lifts")), "{said:?}");
}

#[test]
fn every_way_on_from_the_librarys_desk_and_stacks_has_its_name_beside_it() {
    boards_by_every_way_on(ZoneId::Library, "notice_library");
}

#[test]
fn the_library_says_where_the_steps_go_and_where_the_last_book_is() {
    // The page is out of reach until the library steps stand on the plate.
    assert!(lines_of("board_library_stacks").contains("put back on their mark"));
    assert!(lines_of("board_library_shelf").contains("one left"));
    let cat = catalog();
    let b = build(ZoneId::Library, 1);
    let page = jane_core::Key::Name(cat.name_id("library_page").expect("the page"));
    let page = b.blueprint.props.iter().find(|p| p.key == page).expect("the stacks' page");
    let label = page.label.map(|l| match l {
        jane_core::TextRef::Text(t) => cat.text(t),
        jane_core::TextRef::Local(_) => "",
    });
    assert_eq!(label, Some("A page on the top shelf"));
}
