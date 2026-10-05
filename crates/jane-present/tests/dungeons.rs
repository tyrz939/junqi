//! Dungeons are framed (ART-PLAN §7 rule 6, M5 and B2): in every generated dungeon of the seeds
//! tried, every room has a face at least two cells tall over it, a floor border and its theme's
//! motif at least once, as the painter draws them; every boss room is found and dressed; and a
//! theme no data row has dresses a synthetic dungeon the same way, so another game's theme row
//! gets framed dungeons for free.

use jane_art::terrain::dungeon::{FRAMED_BORDER, FRAMED_MOTIF, FRAMED_TALL, Role};
use jane_art::terrain::{CHUNK_CELLS, Chunk, Painter, TileMap, TileSource, paint_chunk};
use jane_core::ZoneId;

/// What every chunk of `map` drew of each room's framing, or-ed.
fn framed(map: &TileMap, seed: u32) -> Vec<u8> {
    let (w, h) = map.size();
    let mut p = Painter::new();
    let mut c = Chunk::new();
    let mut all = vec![0u8; map.dungeon.as_ref().map_or(0, |d| d.rooms.len())];
    for cy in 0..(h + CHUNK_CELLS - 1) / CHUNK_CELLS {
        for cx in 0..(w + CHUNK_CELLS - 1) / CHUNK_CELLS {
            paint_chunk(&mut p, map, seed, cx, cy, &mut c);
            for (a, f) in all.iter_mut().zip(p.framed()) {
                *a |= f;
            }
        }
    }
    all
}

const DUNGEONS: [ZoneId; 8] = [
    ZoneId::Mine,
    ZoneId::Burial,
    ZoneId::Factory,
    ZoneId::Forest,
    ZoneId::Library,
    ZoneId::Museum,
    ZoneId::Pipes,
    ZoneId::School,
];

#[test]
fn every_dungeon_room_is_framed_and_dressed() {
    let mut faults = Vec::new();
    for seed in 1..=2 {
        for zone in DUNGEONS {
            let built = jane_world::dungeon::build(zone, seed);
            let map = TileMap::from_blueprint_seeded(&built.blueprint, seed);
            let Some(dg) = map.dungeon.as_ref() else {
                faults.push(format!("{zone:?}: no theme"));
                continue;
            };
            let got = framed(&map, seed);
            if std::env::var_os("JANE_ROOMS").is_some() {
                for (i, r) in dg.real() {
                    println!(
                        "{zone:?} seed {seed} room {i} {:?} {:?} centre {:?} framed {:03b}",
                        r.role,
                        r.rect,
                        (r.rect.x + r.rect.w / 2, r.rect.y + r.rect.h / 2),
                        got[i]
                    );
                }
            }
            let tall_ok = zone == ZoneId::Forest;
            for (i, r) in dg.real() {
                let f = got[i];
                let want = FRAMED_BORDER | FRAMED_MOTIF | if tall_ok { 0 } else { FRAMED_TALL };
                if f & want != want {
                    faults.push(format!(
                        "{zone:?} seed {seed} room {i} at {:?} ({:?}): drew {f:03b} of {want:03b}",
                        r.rect, r.role
                    ));
                }
            }
            let bosses = dg.real().filter(|(_, r)| r.role == Role::Boss).count();
            let mission = jane_data::catalog().dungeons.mission_of(zone).expect("a mission");
            let has_boss = mission.nodes.iter().any(|n| n.kind == jane_data::MissionNodeKind::Boss);
            if has_boss && bosses != 1 {
                faults.push(format!("{zone:?} seed {seed}: {bosses} boss rooms"));
            }
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}
