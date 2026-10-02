//! Houses with owners (ART-PLAN Q2 and M7; §7 rule 1, "no stamp twice in view"): in the town of
//! every seed tried, each house has a seed of its own, and no two houses a frame can hold at once
//! share three or more of roof, wall, door, window rhythm, boundary and hero detail. And every
//! garden boundary drawn stands on fence the sim has, its gate open ground she walks through.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use jane_art::terrain::TileMap;
use jane_art::terrain::houses::{House, in_view};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Key, Tile};

/// A county as built, its houses found and seeded as the renderer finds them, and its square.
type Built = (Blueprint, TileMap, (i32, i32));

/// The county of `seed` as built.
fn build(seed: u32) -> Built {
    let sk = jane_world::county::county_skeleton(seed, 0).expect("a county");
    let mut c = jane_world::county::County::new(&sk, 0);
    for (_, stage) in jane_world::county::STAGES {
        stage(&mut c);
    }
    let bp = c.done();
    let square = jane_data::catalog().name_id("town_square").expect("the square's name");
    let m = bp.marks.get(&Key::Name(square)).expect("the town has its square");
    let map = TileMap::from_blueprint_seeded(&bp, seed);
    let sq = (i32::from(m.cell.x), i32::from(m.cell.y));
    (bp, map, sq)
}

/// Seeds 1 to 8, built once for every test here.
fn counties() -> &'static [Built] {
    static C: OnceLock<Vec<Built>> = OnceLock::new();
    C.get_or_init(|| (1..=8).map(build).collect())
}

fn county(seed: u32) -> (&'static TileMap, (i32, i32)) {
    let (_, m, sq) = &counties()[seed as usize - 1];
    (m, *sq)
}

#[test]
fn every_garden_boundary_drawn_is_fence_to_the_sim_and_its_gate_lets_her_through_on_seeds_1_to_8() {
    let cat = jane_data::catalog();
    for seed in 1..=8u32 {
        let (bp, map, _) = &counties()[seed as usize - 1];
        let w = bp.tiles.w() as i32;
        let tile = |x: i32, y: i32| bp.tiles.read(x, y, Tile::Void);
        let mut stops = vec![false; bp.tiles.as_slice().len()];
        for p in &bp.props {
            let d = cat.story.prop(p.def);
            if d.solid && !d.gate && !p.hidden {
                for dy in 0..i32::from(d.h) {
                    for dx in 0..i32::from(d.w) {
                        let (x, y) = (i32::from(p.cell.x) + dx, i32::from(p.cell.y) + dy);
                        if bp.tiles.get(x, y).is_some() {
                            stops[(y * w + x) as usize] = true;
                        }
                    }
                }
            }
        }
        let walk = |x: i32, y: i32| {
            bp.tiles.get(x, y).is_some() && tile(x, y).flags() & F_SOLID == 0 && !stops[(y * w + x) as usize]
        };
        let mut fenced = 0;
        for h in map.houses.list().iter().filter(|h| h.fence != 0) {
            fenced += 1;
            let fy = h.plot.bottom() - 1;
            for x in (h.plot.x..h.plot.right()).filter(|&x| h.fenced(x, fy)) {
                let t = tile(x, fy);
                assert!(t.flags() & F_SOLID != 0, "seed {seed}: a boundary drawn at ({x}, {fy}) on {t:?}");
            }
            let Some(gx) = h.gate else { continue };
            assert!(walk(gx, fy) && walk(gx + 1, fy), "seed {seed}: the gate at ({gx}, {fy}) is shut");
            // From outside the gate, through it and up the garden alone, to the door's step.
            let Some(door) = h.door else { continue };
            let inside = |x: i32, y: i32| h.plot.contains(x, y) || (y == fy + 1 && (x == gx || x == gx + 1));
            let mut seen = BTreeSet::new();
            let mut todo: Vec<(i32, i32)> =
                [(gx, fy + 1), (gx + 1, fy + 1)].into_iter().filter(|&(x, y)| walk(x, y)).collect();
            assert!(!todo.is_empty(), "seed {seed}: the gate at ({gx}, {fy}) opens onto nothing");
            while let Some((x, y)) = todo.pop() {
                if !seen.insert((x, y)) {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if inside(nx, ny) && walk(nx, ny) {
                        todo.push((nx, ny));
                    }
                }
            }
            assert!(
                seen.contains(&(door, h.plot.y)) || seen.contains(&(door + 1, h.plot.y)),
                "seed {seed}: the door at {door} of {:?} is not reached through its gate",
                h.rect
            );
        }
        assert!(fenced >= 6, "seed {seed}: {fenced} gardens fenced");
    }
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
