//! The legibility pass on the Gold Mine (DUNGEONS.md §3.1, "Is it clear what to do?"): the
//! cues a first-time player reads, there on every seed. Boards beside the doors off the way in,
//! the Shaft Hall and the Pay Office name where each goes; the notice's plan names the same rooms
//! in the same words; the weighbridge's chest says why a key will not open it.

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

#[test]
fn every_way_on_from_the_mines_decisions_has_its_name_painted_beside_it() {
    let cat = catalog();
    for seed in 1..=16 {
        let b = build(ZoneId::Mine, seed);
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
                let want = format!("board_mine_{}", m.nodes[other].id);
                let d = &r.shape.doors[di];
                let (dx, dy) = (r.x + i32::from(d.x), r.y + i32::from(d.y));
                let board = bp.props.iter().find(|p| {
                    p.talk.is_some_and(|t| cat.story.dialogue[t.index()].id == want)
                        && (i32::from(p.cell.x) - dx).abs() <= 5
                        && (i32::from(p.cell.y) - dy).abs() <= 5
                });
                let board = board.unwrap_or_else(|| {
                    panic!("seed {seed}: no {want} board by {}'s door to {}", m.nodes[r.node].id, m.nodes[other].id)
                });
                // The prompt beside the door reads the board's name.
                let label = board.label.map(|l| match l {
                    jane_core::TextRef::Text(t) => cat.text(t),
                    jane_core::TextRef::Local(_) => "",
                });
                let speaker = cat.text(cat.story.dialogue[board.talk.expect("read").index()].speaker);
                assert_eq!(label, Some(speaker), "seed {seed}: {want}");
            }
        }
    }
}

#[test]
fn the_notice_names_every_room_a_board_names_in_the_same_words() {
    let cat = catalog();
    let notice = lines_of("notice_mine");
    let boards: Vec<_> = cat.story.dialogue.iter().filter(|d| d.id.starts_with("board_mine_")).collect();
    assert!(boards.len() >= 8, "only {} boards", boards.len());
    let m = cat.dungeons.mission_of(ZoneId::Mine).expect("the mine");
    for d in boards {
        let node = d.id.trim_start_matches("board_mine_");
        assert!(m.nodes.iter().any(|n| n.id == node), "{} names no room", d.id);
        let name = cat.text(d.speaker);
        assert!(notice.contains(name), "the notice does not name {name}");
    }
    // The quest log names the Headmaster's and Iron Knuckles' rooms as their boards do.
    let q = cat.story.quest_id("the_mine").expect("the mine's quest");
    let steps: Vec<&str> = cat.story.quests[q.index()].requirements.iter().map(|r| cat.text(r.text)).collect();
    assert!(steps.iter().any(|s| s.contains("PAY OFFICE")), "{steps:?}");
    assert!(steps.iter().any(|s| s.contains("No. 3 PIT")), "{steps:?}");
}

#[test]
fn the_weighbridge_chest_says_a_key_will_not_open_it() {
    let cat = catalog();
    let def = cat.story.prop_id("weighed_chest").expect("the weighed chest");
    let says = cat.story.prop(def).locked_says.map_or("", |t| cat.text(t));
    assert!(says.contains("no keyhole"), "{says}");
    let b = build(ZoneId::Mine, 1);
    let chest_key = jane_core::Key::Name(cat.name_id("plate_chest").expect("plate_chest"));
    let chest = b.blueprint.props.iter().find(|p| p.key == chest_key).expect("the plate chest");
    assert_eq!(chest.def, def);
    assert!(chest.key_tag.is_none(), "no key fits it");
}
