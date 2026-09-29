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
}
