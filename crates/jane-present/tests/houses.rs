//! Houses with owners (ART-PLAN Q2 and M7; §7 rule 1, "no stamp twice in view"): in the town of
//! every seed tried, each house has a seed of its own, and no two houses a frame can hold at once
//! share three or more of roof, wall, door, window rhythm, boundary and hero detail.

use std::collections::BTreeSet;

use jane_art::terrain::TileMap;
use jane_art::terrain::houses::{House, in_view};
use jane_core::Key;

/// The county of `seed` as built, its houses found and seeded as the renderer finds them.
fn county(seed: u32) -> (TileMap, (i32, i32)) {
    let sk = jane_world::county::county_skeleton(seed, 0).expect("a county");
    let mut c = jane_world::county::County::new(&sk, 0);
    for (_, stage) in jane_world::county::STAGES {
        stage(&mut c);
    }
    let bp = c.done();
    let square = jane_data::catalog().name_id("town_square").expect("the square's name");
    let m = bp.marks.get(&Key::Name(square)).expect("the town has its square");
    (TileMap::from_blueprint_seeded(&bp, seed), (i32::from(m.cell.x), i32::from(m.cell.y)))
}

#[test]
fn no_two_houses_in_view_share_three_of_six_on_seeds_1_to_8() {
    for seed in 1..=8u32 {
        let (map, (sx, sy)) = county(seed);
        // The town: every house within its box of the square.
        let town: Vec<&House> =
            map.houses.list().iter().filter(|h| (h.rect.x - sx).abs() < 60 && (h.rect.y - sy).abs() < 40).collect();
        assert!(town.len() >= 20, "seed {seed}: {} houses in the town", town.len());
        let seeds: BTreeSet<u32> = town.iter().map(|h| h.seed).collect();
        assert_eq!(seeds.len(), town.len(), "seed {seed}: every house a seed of its own");
        let doors = town.iter().filter(|h| h.door.is_some()).count();
        assert!(doors * 4 >= town.len() * 3, "seed {seed}: {doors} of {} houses found their door", town.len());
        let gardens = town.iter().filter(|h| h.plot.h > 0).count();
        assert!(gardens >= 6, "seed {seed}: {gardens} front gardens");
        let looks: Vec<_> = town.iter().map(|h| h.look()).collect();
        let roofs: BTreeSet<_> = looks.iter().map(|l| (l.roof as u8, l.age as u8)).collect();
        let walls: BTreeSet<_> = looks.iter().map(|l| format!("{:?}", l.wall)).collect();
        let doors: BTreeSet<_> = looks.iter().map(jane_art::terrain::houses::Look::door_variant).collect();
        assert!(roofs.len() >= 5 && walls.len() >= 4 && doors.len() >= 6, "seed {seed}: {roofs:?} {walls:?} {doors:?}");
        for (i, a) in town.iter().enumerate() {
            for b in &town[..i] {
                if in_view(a.rect, b.rect) {
                    let n = a.look().shared(&b.look());
                    assert!(n < 3, "seed {seed}: the houses at {:?} and {:?} share {n}", a.rect, b.rect);
                }
            }
        }
    }
}
