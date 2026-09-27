//! The set pieces (DUNGEONS.md §2.10): dressing that never costs a check. Every candidate of
//! every dungeon is judged with its set pieces and without them, and the verdicts must agree:
//! a piece that turned a proven candidate into a refused one (a door, a path, a plate or a
//! light it stood in the way of) fails here with the check that refused it.

use jane_data::{MissionDef, catalog};
use jane_world::dungeon::{Proven, Validate, generate};

fn verdict(m: &'static MissionDef, seed: u32, attempt: u8, bare: bool) -> Result<(), Vec<String>> {
    let b =
        if bare { generate::build_mission_bare(m, seed, attempt) } else { generate::build_mission(m, seed, attempt) };
    if !b.info.errors.is_empty() {
        return Err(b.info.errors);
    }
    Proven.validate(&b)
}

/// Seeds swept: `DUNGEON_SEEDS`, 12 by default (every attempt of every dungeon is built twice).
#[allow(clippy::disallowed_methods)]
fn seeds() -> u32 {
    std::env::var("DUNGEON_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(12)
}

#[test]
fn a_set_piece_never_changes_a_verdict() {
    let mut bad = Vec::new();
    for m in catalog().dungeons.missions {
        for seed in 1..=seeds() {
            // The attempts a build would try: up to the first the bare generator accepts.
            for attempt in 0..jane_core::blueprint::ZONE_ATTEMPTS {
                let bare = verdict(m, seed, attempt, true);
                let dressed = verdict(m, seed, attempt, false);
                if bare.is_ok() != dressed.is_ok() {
                    bad.push(format!("{} seed {seed} attempt {attempt}: bare {bare:?}, dressed {dressed:?}", m.id));
                }
                if bare.is_ok() {
                    break;
                }
            }
        }
    }
    assert!(bad.is_empty(), "set pieces changed verdicts:\n{}", bad.join("\n"));
}

#[test]
fn set_pieces_stand_whole_on_open_floor() {
    // Every room that lists pieces gets at least one on most seeds, every part of a stood piece
    // is inside its room, and no two solid things share a cell.
    for m in catalog().dungeons.missions {
        let mut furnished = 0;
        let mut listed = 0;
        for seed in 1..=seeds() {
            let b = jane_world::dungeon::build(m.zone, seed);
            let bp = &b.blueprint;
            let c = catalog();
            let mut solid = vec![0u8; (bp.tiles.w() * bp.tiles.h()) as usize];
            for p in &bp.props {
                let d = c.story.prop(p.def);
                if !d.solid || p.hidden {
                    continue;
                }
                for y in 0..i32::from(d.h) {
                    for x in 0..i32::from(d.w) {
                        let (cx, cy) = (i32::from(p.cell.x) + x, i32::from(p.cell.y) + y);
                        solid[(cy * bp.tiles.w() as i32 + cx) as usize] += 1;
                    }
                }
            }
            for p in &bp.props {
                let jane_core::Key::Local(i) = p.key else { continue };
                if !bp.local_names[i as usize].contains("_set_") {
                    continue;
                }
                let d = c.story.prop(p.def);
                let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
                let room = b.info.rooms.iter().find(|r| r.rect.contains(x, y));
                let room = room.unwrap_or_else(|| panic!("{} seed {seed}: a set piece part outside every room", m.id));
                assert!(
                    room.rect.contains(x + i32::from(d.w) - 1, y + i32::from(d.h) - 1),
                    "{} seed {seed}: {} runs out of {}",
                    m.id,
                    d.id,
                    m.nodes[room.node].id
                );
                if d.solid {
                    for j in 0..i32::from(d.h) {
                        for i in 0..i32::from(d.w) {
                            let k = ((y + j) * bp.tiles.w() as i32 + x + i) as usize;
                            assert_eq!(solid[k], 1, "{} seed {seed}: {} stands on something solid", m.id, d.id);
                        }
                    }
                }
            }
            for r in &b.info.rooms {
                if m.set_rooms.iter().any(|s| usize::from(s.node) == r.node) {
                    listed += 1;
                    let any = bp.props.iter().any(|p| {
                        matches!(p.key, jane_core::Key::Local(i) if bp.local_names[i as usize].contains("_set_"))
                            && r.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y))
                    });
                    furnished += i32::from(any);
                }
            }
        }
        assert!(furnished * 10 >= listed * 8, "{}: only {furnished} of {listed} listed rooms took a set piece", m.id);
    }
}
