//! PORT.md §6.m stage 5: the county's land, roads, footpaths, railway raster and fingerposts,
//! rasterised from the skeleton. Carries the land and road parts of `jane/test/county.test.ts`
//! (the size, the first walk being a road all the way, the same county for the same seed) and
//! stage 5's own floors: a road within 3 cells of every skeleton road cell's centre, water under a
//! road is a bridge of planks, the rail raster is 4-connected edge to edge, and the tree line
//! holds. A set place is drawn over whatever road ran into it (stage 6), so the road and bridge
//! checks leave every chunk's box out. `SEEDS` seeds (64 by default); a county's land is a tenth of a second in a dev build.
//! A sheet of the first seeds is drawn to `$CARGO_TARGET_TMPDIR/sheets/` (never asserted).

mod common;

use std::sync::OnceLock;

use jane_core::{Blueprint, Grid, Tile};
use jane_world::county::land::EDGE;
use jane_world::county::rail::{BEND, rail_line};
use jane_world::county::{County, STAGES, build_county, county_skeleton};
use jane_world::skeleton::{COUNTY_H, COUNTY_W, MACRO, SKEL_H, SKEL_W, Skeleton};

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

type Check = fn(&Skeleton, &County<'_>, &mut Vec<String>);

/// The checks run on every seed's county, each named for the test that reports it.
const CHECKS: &[(&str, Check)] = &[
    ("roads", roads_follow_the_skeleton),
    ("first_walk", first_walk_is_road),
    ("bridges", bridges_are_planks),
    ("rail", rail_is_connected),
    ("tree_line", tree_line_holds),
    ("path_ends", path_ends_are_marked),
];

/// Every seed's county built once (on as many threads as there are cores) and every check run on
/// it before it is finished, while the ground as it was before the roads is still there to read.
/// What each check complained of, by check, in seed order.
fn verdicts() -> &'static [Vec<String>] {
    static ALL: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    ALL.get_or_init(|| {
        let seeds = seed_list();
        let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let per = seeds.len().div_ceil(threads).max(1);
        let parts: Vec<Vec<Vec<String>>> = std::thread::scope(|s| {
            let jobs: Vec<_> = seeds
                .chunks(per)
                .map(|chunk| {
                    s.spawn(move || {
                        let mut bad = vec![Vec::new(); CHECKS.len()];
                        for &seed in chunk {
                            let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
                            let mut c = County::new(&sk, 0);
                            for (_, stage) in STAGES {
                                stage(&mut c);
                            }
                            for (i, (_, check)) in CHECKS.iter().enumerate() {
                                check(&sk, &c, &mut bad[i]);
                            }
                        }
                        bad
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().expect("a county builds")).collect()
        });
        (0..CHECKS.len()).map(|i| parts.iter().flat_map(|p| p[i].iter().cloned()).collect()).collect()
    })
}

fn verdict(name: &str) {
    let i = CHECKS.iter().position(|(n, _)| *n == name).expect("a check of that name");
    let bad = &verdicts()[i];
    assert!(bad.is_empty(), "{} failures, the first: {:#?}", bad.len(), &bad[..bad.len().min(8)]);
}

fn tile(c: &County<'_>, x: i32, y: i32) -> Tile {
    c.k.get(x, y)
}

/// Whether `(x, y)` is within `margin` cells of a chunk's box: the stamp drew over what was there.
fn in_chunk(c: &County<'_>, x: i32, y: i32, margin: i32) -> bool {
    c.chunks.iter().any(|ch| ch.bounds.grow(margin).contains(x, y))
}

#[test]
fn is_two_thousand_cells_square() {
    let bp = build_county(seed_list()[0], 0).expect("builds");
    assert_eq!((bp.w(), bp.h()), (COUNTY_W as u32, COUNTY_H as u32));
    assert_eq!((COUNTY_W, COUNTY_H), (2000, 2000));
    assert_eq!(bp.attempts, 1);
    assert_eq!(bp.text(bp.name), Some("Castle"));
}

/// ARCHITECTURE.md §4.6.b: the county carries the skeleton's region of every macro cell, so the
/// sky that rains on a cell is its region's; every cell of the county answers.
#[test]
fn carries_the_skeletons_regions() {
    let seed = seed_list()[0];
    let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
    let bp = jane_world::county::build_county_on(&sk, 0);
    let m = &bp.regions;
    assert_eq!((i32::from(m.scale), i32::from(m.w), i32::from(m.h)), (MACRO, SKEL_W, SKEL_H));
    for my in 0..SKEL_H {
        for mx in 0..SKEL_W {
            let want = sk.region_at(mx, my) as u8;
            for (dx, dy) in [(0, 0), (MACRO - 1, MACRO - 1), (MACRO / 2, 3)] {
                assert_eq!(bp.region_at(mx * MACRO + dx, my * MACRO + dy), Some(want), "macro ({mx}, {my})");
            }
        }
    }
    assert_eq!(bp.region_at(COUNTY_W, 0), None);
    let used: std::collections::BTreeSet<u8> = m.cells.iter().copied().collect();
    assert_eq!(used.into_iter().collect::<Vec<_>>(), [0, 1, 2], "all three regions");
}

#[test]
fn a_road_runs_within_three_cells_of_every_skeleton_road_cell() {
    verdict("roads");
}

fn roads_follow_the_skeleton(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    for (n, r) in sk.roads.iter().enumerate() {
        for &(mx, my) in &r.cells {
            let (cx, cy) = (mx * MACRO + MACRO / 2, my * MACRO + MACRO / 2);
            if in_chunk(c, cx, cy, 3) {
                continue;
            }
            let found = (-3..=3).any(|oy: i32| {
                (-3..=3).any(|ox: i32| {
                    ox * ox + oy * oy <= 9 && matches!(tile(c, cx + ox, cy + oy), Tile::Road | Tile::Boardwalk)
                })
            });
            if !found {
                bad.push(format!("seed {}: road {n} has no road near ({cx}, {cy})", sk.seed));
            }
        }
    }
}

#[test]
fn the_first_walk_is_a_road_all_the_way() {
    verdict("first_walk");
}

fn first_walk_is_road(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    // county.test.ts: at least 85% of the station road's macro cells have road within 6 cells.
    let station = jane_world::skeleton::RoadEnd::Site(u16::from(sk.named.station));
    let first = sk.roads.iter().find(|r| r.from == station).expect("a road from the station");
    let road = first
        .cells
        .iter()
        .filter(|&&(mx, my)| {
            let (x, y) = (mx * MACRO + MACRO / 2, my * MACRO + MACRO / 2);
            (-6..=6).any(|oy| (-6..=6).any(|ox| tile(c, x + ox, y + oy) == Tile::Road))
        })
        .count();
    if road * 100 <= first.cells.len() * 85 {
        bad.push(format!("seed {}: {road} of {} station road cells have road", sk.seed, first.cells.len()));
    }
}

#[test]
fn water_under_a_road_is_boardwalk() {
    verdict("bridges");
}

fn bridges_are_planks(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let before: &Grid<Tile> = c.before.as_ref().expect("the roads keep the ground they were laid on");
    let mut planks = 0;
    for y in 0..COUNTY_H {
        for x in 0..COUNTY_W {
            let (was, now) = (before.read(x, y, Tile::Void), tile(c, x, y));
            if was == Tile::Water && now == Tile::Road && !in_chunk(c, x, y, 0) {
                bad.push(format!("seed {}: road on water at ({x}, {y})", sk.seed));
            }
            planks += u32::from(now == Tile::Boardwalk);
        }
    }
    if sk.bridges() > 0 && planks == 0 {
        bad.push(format!("seed {}: {} bridges and not a plank", sk.seed, sk.bridges()));
    }
}

#[test]
fn the_railway_is_four_connected_from_edge_to_edge() {
    verdict("rail");
}

fn rail_is_connected(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let on_edge = |(x, y): (i32, i32)| x == 0 || y == 0 || x == COUNTY_W - 1 || y == COUNTY_H - 1;
    let line = rail_line(sk, BEND);
    let s = sk.seed;
    if line.len() < 2 {
        bad.push(format!("seed {s}: no railway"));
        return;
    }
    let (a, b) = (line[0], line[line.len() - 1]);
    if !on_edge(a) || !on_edge(b) || a == b {
        bad.push(format!("seed {s}: the line runs {a:?} to {b:?}, not edge to edge"));
    }
    for w in line.windows(2) {
        if (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs() != 1 {
            bad.push(format!("seed {s}: the line jumps {:?} to {:?}", w[0], w[1]));
        }
    }
    let in_trees = |x: i32, y: i32| x < EDGE || y < EDGE || x >= COUNTY_W - EDGE || y >= COUNTY_H - EDGE;
    for &(x, y) in &line {
        let t = tile(c, x, y);
        // Track, or a road it crosses on the level (a road bridge where it crosses one); in
        // the trees at each end, the fence the rails run on under.
        let fence = t == Tile::Fence && in_trees(x, y);
        if !fence && !matches!(t, Tile::Track | Tile::Road | Tile::Boardwalk | Tile::Rail) {
            bad.push(format!("seed {s}: {t:?} on the line at ({x}, {y})"));
        }
    }
}

#[test]
fn the_tree_line_holds_but_where_the_rails_run_out() {
    verdict("tree_line");
}

fn tree_line_holds(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let line = rail_line(sk, BEND);
    let ends: Vec<(i32, i32)> = [line.first(), line.last()].into_iter().flatten().copied().collect();
    let by_rail = |x: i32, y: i32| ends.iter().any(|&(ex, ey)| (x - ex).abs() <= 3 && (y - ey).abs() <= 3);
    for y in 0..COUNTY_H {
        for x in 0..COUNTY_W {
            let border = x < EDGE || y < EDGE || x >= COUNTY_W - EDGE || y >= COUNTY_H - EDGE;
            if border && !by_rail(x, y) && tile(c, x, y) != Tile::Tree {
                bad.push(format!("seed {}: {:?} in the tree line at ({x}, {y})", sk.seed, tile(c, x, y)));
            }
        }
    }
}

#[test]
fn every_footpath_end_has_its_mark() {
    verdict("path_ends");
}

fn path_ends_are_marked(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    for fp in &c.footpaths {
        for name in cat.county.paths[fp.row].marks {
            if !bp.marks.contains_key(&jane_core::Key::Name(name)) {
                bad.push(format!("seed {}: no mark {}", sk.seed, cat.name(name)));
            }
        }
    }
    if c.footpaths.len() != cat.county.paths.len() {
        bad.push(format!("seed {}: {} of {} footpaths laid", sk.seed, c.footpaths.len(), cat.county.paths.len()));
    }
}

/// A county attempt stands on its own skeleton: attempt 1 is a different county. (The same seed
/// building the same county is `determinism.rs`'s.)
#[test]
fn another_attempt_is_another_county() {
    let seed = seed_list()[0];
    let (a, b) = (build_county(seed, 0).expect("builds"), build_county(seed, 1).expect("builds"));
    assert_eq!(b.attempts, 2);
    assert!(a.tiles != b.tiles);
}

/// A tile's colour on the sheet: enough to tell ground, water, roads and the line apart.
fn rgb(t: Tile) -> [u8; 3] {
    match t {
        Tile::Grass => [118, 164, 84],
        Tile::GrassTall | Tile::Moss => [92, 140, 66],
        Tile::Tree | Tile::Bush | Tile::Hedge => [34, 84, 40],
        Tile::Water => [52, 96, 170],
        Tile::Sand => [206, 190, 140],
        Tile::Road => [60, 58, 58],
        Tile::Boardwalk => [190, 130, 60],
        Tile::Track => [120, 30, 30],
        Tile::Fence => [240, 240, 240],
        Tile::Cliff | Tile::Rubble => [96, 90, 84],
        _ => [150, 122, 84],
    }
}

fn sheet(bp: &Blueprint, scale: u32) -> Vec<u8> {
    let (w, h) = (bp.w() / scale, bp.h() / scale);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            // The most telling cell of the block: a road, the line or water over the ground.
            let cells = (0..scale).flat_map(|j| (0..scale).map(move |i| (x * scale + i, y * scale + j)));
            let t = cells
                .map(|(i, j)| bp.tiles.read(i as i32, j as i32, Tile::Void))
                .max_by_key(|&t| match t {
                    Tile::Track | Tile::Fence => 4,
                    Tile::Boardwalk => 3,
                    Tile::Road => 2,
                    Tile::Water => 1,
                    _ => 0,
                })
                .unwrap_or(Tile::Void);
            let [r, g, b] = rgb(t);
            px.extend_from_slice(&[r, g, b, 255]);
        }
    }
    jane_art::sheet::png(w, h, &px)
}

#[test]
#[ignore = "tool: writes county sheets to look at; asserts nothing of the county"]
fn draws_a_sheet() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("sheets");
    std::fs::create_dir_all(&dir).expect("a sheets dir");
    for seed in seed_list().into_iter().take(2) {
        let bp = build_county(seed, 0).expect("builds");
        let png = sheet(&bp, 2);
        assert_eq!(&png[1..4], b"PNG");
        std::fs::write(dir.join(format!("county-{seed}.png")), png).expect("written");
    }
}
