//! The legibility pass on Butterfly Forest, the Burial Chamber and the School (DUNGEONS.md §3.3,
//! §3.5 and §3.6, "Is it clear what to do?"): the cues a first-time player reads, there on every
//! seed. Boards beside the doors of the rooms she decides in name where each goes, in the words of
//! the notice by the way in, and the quest log names the same places.

use jane_core::ZoneId;
use jane_data::{MissionEdgeKind, MissionNodeKind, catalog};
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

fn steps_of(quest: &str) -> Vec<&'static str> {
    let cat = catalog();
    let q = cat.story.quest_id(quest).unwrap_or_else(|| panic!("no quest {quest}"));
    cat.story.quests[q.index()].requirements.iter().map(|r| cat.text(r.text)).collect()
}

/// Every door of an entrance, a hub or a mini-boss's room, and every door a state's gate stands
/// in, has the board of the room it leads to beside it, its prompt the board's name.
fn boards_by_every_door(zone: ZoneId) {
    let cat = catalog();
    for seed in 1..=16 {
        let b = build(zone, seed);
        let (bp, m) = (&b.blueprint, b.info.mission);
        let layout = b.info.layout.as_ref().expect("a generated layout");
        for r in &b.info.rooms {
            let decides = matches!(
                m.nodes[r.node].kind,
                MissionNodeKind::Entrance | MissionNodeKind::Hub | MissionNodeKind::Miniboss
            );
            for &di in &r.doors {
                let (other, edge) = layout
                    .corridors
                    .iter()
                    .find_map(|c| {
                        if c.a.node == r.node && c.a.door == di {
                            Some((c.b.node, c.edge))
                        } else if c.b.node == r.node && c.b.door == di {
                            Some((c.a.node, c.edge))
                        } else {
                            None
                        }
                    })
                    .expect("a door in use has a corridor");
                if !decides && !matches!(m.edges[edge].kind, MissionEdgeKind::State { .. }) {
                    continue;
                }
                let want = format!("board_{}_{}", m.id, m.nodes[other].id);
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
                let label = board.label.map(|l| match l {
                    jane_core::TextRef::Text(t) => cat.text(t),
                    jane_core::TextRef::Local(_) => "",
                });
                let speaker = cat.text(cat.story.dialogue[board.talk.expect("read").index()].speaker);
                assert_eq!(label, Some(speaker), "{zone:?} seed {seed}: {want}");
            }
        }
    }
}

/// The notice by the way in names every room a board names, in the same words.
fn the_notice_names_every_board(zone: ZoneId, notice: &str) {
    let cat = catalog();
    let words = lines_of(notice);
    let m = cat.dungeons.mission_of(zone).expect("a mission");
    let prefix = format!("board_{}_", m.id);
    let boards: Vec<_> = cat.story.dialogue.iter().filter(|d| d.id.starts_with(&prefix)).collect();
    assert!(boards.len() >= 8, "{zone:?}: only {} boards", boards.len());
    for d in boards {
        let node = d.id.trim_start_matches(&prefix);
        assert!(m.nodes.iter().any(|n| n.id == node), "{} names no room", d.id);
        let name = cat.text(d.speaker);
        assert!(words.contains(name), "{notice} does not name {name}");
    }
}

// --- Butterfly Forest ---------------------------------------------------------------------------

#[test]
fn every_way_on_from_the_forests_gate_hut_and_rides_has_its_name_on_a_post() {
    boards_by_every_door(ZoneId::Forest);
}

#[test]
fn the_forests_sign_names_every_garden_a_post_names_and_the_plan() {
    the_notice_names_every_board(ZoneId::Forest, "notice_forest");
    let sign = lines_of("notice_forest");
    // The whole plan, in the sign's words: nets from the hut, five to the stone.
    assert!(sign.contains("five at a time") && sign.contains("Nets from THE KEEPER'S HUT"), "{sign}");
    assert!(lines_of("board_forest_stone").contains("five"));
    let steps = steps_of("the_forest");
    assert!(steps.iter().any(|s| s.contains("five butterflies") && s.contains("THE STANDING STONE")), "{steps:?}");
}
