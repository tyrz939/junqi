//! PORT.md §6.m stage 2: the land. Carries the terrain parts of `jane/test/skeleton.test.ts`
//! as invariants over seeds: the river touches north and south, the hill's crown is in the Works,
//! one lake east of the river, the regions within their bands, the same seed the same land.

mod common;

use jane_world::skeleton::{Biome, Region, SKEL_H, SKEL_W, Terrain, Water, build_terrain};

fn count(t: &Terrain, r: Region) -> u32 {
    t.region.as_slice().iter().filter(|&&x| x == r).count() as u32
}

#[test]
fn the_river_runs_the_whole_county_north_to_south() {
    for seed in 1..=common::seeds() {
        let t = build_terrain(seed, 0);
        assert!(t.water.read(t.river_x[0], 0, Water::Dry) == Water::River, "seed {seed}: no river at the north edge");
        let last = (SKEL_H - 1) as usize;
        assert!(t.water.read(t.river_x[last], SKEL_H - 1, Water::Dry) == Water::River, "seed {seed}: south edge");
        for y in 1..SKEL_H as usize {
            // Continuous: each row's river touches the row above's.
            let (a, b) = (t.river_x[y - 1], t.river_x[y]);
            assert!(
                (a.min(b)..=a.max(b)).all(|x| t.water.read(x, y as i32, Water::Dry) != Water::Dry),
                "seed {seed} row {y}"
            );
        }
        for &x in &t.river_x {
            assert!((SKEL_W / 2..=SKEL_W * 3 / 4).contains(&x), "seed {seed}: river at column {x}");
        }
    }
}

#[test]
fn the_hill_crowns_the_works() {
    for seed in 1..=common::seeds() {
        let t = build_terrain(seed, 0);
        let (cx, cy) = t.crown;
        assert_eq!(t.region.read(cx, cy, Region::Lowfields), Region::Works, "seed {seed}: crown not in the Works");
        // The highest ground of the county is the hill, near its crown.
        let (mut best, mut at) = (0u8, (0, 0));
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let h = t.height.read(x, y, 0);
                if h > best {
                    best = h;
                    at = (x, y);
                }
            }
        }
        assert_eq!(t.region.read(at.0, at.1, Region::Lowfields), Region::Works, "seed {seed}: highest point {at:?}");
        assert!(best > 180, "seed {seed}: the hill tops out at {best}");
        assert!(t.biome.as_slice().contains(&Biome::Hill), "seed {seed}: no hill biome");
    }
}

#[test]
fn one_lake_in_the_waters() {
    for seed in 1..=common::seeds() {
        let t = build_terrain(seed, 0);
        let lake: Vec<(i32, i32)> = (0..SKEL_H)
            .flat_map(|y| (0..SKEL_W).map(move |x| (x, y)))
            .filter(|&(x, y)| t.water.read(x, y, Water::Dry) == Water::Lake)
            .collect();
        assert!(lake.len() > 60, "seed {seed}: lake of {} cells", lake.len());
        // Its centre is in the Waters, well east of the river; it may meet the river's east bank
        // (the river feeds it) but never cross it.
        let (mx, my) = (t.lake.mx, t.lake.my);
        assert!(mx > t.river_x[my as usize] + 3, "seed {seed}: lake centre ({mx},{my}) not east of the river");
        for &(x, y) in &lake {
            assert!(x > t.river_x[y as usize], "seed {seed}: lake at ({x},{y}) west of the river");
            assert!(x < SKEL_W - 1, "seed {seed}: the lake runs off the east edge");
        }
        // One body: every lake cell reaches every other through lake cells.
        let mut reach = jane_core::search::Reach::new();
        jane_core::search::flood(
            SKEL_W as u32,
            SKEL_H as u32,
            &lake[..1],
            jane_core::search::Conn::Four,
            u32::MAX,
            |x, y| t.water.read(x, y, Water::Dry) == Water::Lake,
            &mut reach,
        );
        assert_eq!(reach.count() as usize, lake.len(), "seed {seed}: the lake is in pieces");
    }
}

#[test]
fn regions_hold_their_bands() {
    let total = (SKEL_W * SKEL_H) as u32;
    for seed in 1..=common::seeds() {
        let t = build_terrain(seed, 0);
        let (lf, wa, wo) = (count(&t, Region::Lowfields), count(&t, Region::Waters), count(&t, Region::Works));
        assert_eq!(lf + wa + wo, total);
        // The Lowfields hold the town and every story place: the biggest share. Bands are wide on purpose.
        assert!((30..=60).contains(&(lf * 100 / total)), "seed {seed}: lowfields {}%", lf * 100 / total);
        assert!((12..=35).contains(&(wa * 100 / total)), "seed {seed}: waters {}%", wa * 100 / total);
        assert!((18..=45).contains(&(wo * 100 / total)), "seed {seed}: works {}%", wo * 100 / total);
        // Every Waters cell is east of the river; every Lowfields cell west of it or on it.
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                match t.region.read(x, y, Region::Works) {
                    Region::Waters => assert!(x > t.river_x[y as usize]),
                    Region::Lowfields => assert!(x <= t.river_x[y as usize]),
                    Region::Works => {}
                }
            }
        }
    }
}

#[test]
fn biomes_belong_to_their_regions_and_roughness_is_bounded() {
    for seed in 1..=common::seeds().min(16) {
        let t = build_terrain(seed, 0);
        for y in 0..SKEL_H {
            for x in 0..SKEL_W {
                let b = t.biome.read(x, y, Biome::Field);
                let ok = match t.region.read(x, y, Region::Works) {
                    Region::Lowfields => matches!(b, Biome::Field | Biome::Hedge | Biome::Wood | Biome::Foothill),
                    Region::Waters => matches!(b, Biome::Reed | Biome::Marsh | Biome::WetWood | Biome::Garden),
                    Region::Works => matches!(b, Biome::Hill | Biome::Yard | Biome::Slag),
                };
                assert!(ok, "seed {seed}: {b:?} at ({x},{y})");
                let r = t.rough.read(x, y, 0);
                assert!((179..=179 + 666 + 307).contains(&r), "seed {seed}: rough {r}");
            }
        }
    }
}

#[test]
fn the_same_seed_is_the_same_land_and_seeds_differ() {
    let a = build_terrain(7, 0);
    let b = build_terrain(7, 0);
    assert_eq!(a.height, b.height);
    assert_eq!(a.water, b.water);
    assert_eq!(a.biome, b.biome);
    assert_eq!(a.rough, b.rough);
    let c = build_terrain(8, 0);
    assert_ne!(a.height, c.height);
    let d = build_terrain(7, 1);
    assert_ne!(a.height, d.height, "an attempt re-rolls the land");
}
