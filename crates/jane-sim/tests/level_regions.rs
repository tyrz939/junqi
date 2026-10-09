//! MAP R2's hierarchy for paths across height (`jane_sim::regions`): on random ground of random
//! levels (plateaus and hollows, cliffs, joins, ledges, and with walls or without), the level
//! regions say a goal can be reached exactly when a brute-force walk of the search's own rules
//! (eight ways, no corner cut, ledges hopped one way) reaches it, where nothing but height divides
//! the ground; with walls they never say no to a goal that can be reached. And the search steered
//! by them finds a path exactly when the walk does, of a cost the walk's own Dijkstra says is the
//! least or close to it.

use std::collections::BinaryHeap;

use jane_core::grid::Grid;
use jane_core::search::PathEnd;
use jane_core::tile::BLOCK_MOVE;
use jane_core::{Plane, Rect, Sfc32, Tile};
use jane_sim::grid::ZoneGrid;
use jane_sim::path::{PathAsk, PathScratch, cost_of_cells};
use jane_sim::regions::{FAR, Route};

const W: u32 = 48;
const H: u32 = 32;

/// Random ground: level 1 with a few rects raised to 2 and sunk to 0; each cell at a higher level
/// than a neighbour four ways is the higher ground's edge: mostly a cliff, now and then a join
/// (stair), now and then a ledge hopped toward the lower side; walls sprinkled if `walls`.
fn ground(rng: &mut Sfc32, walls: bool) -> ZoneGrid {
    let (w, h) = (W as i32, H as i32);
    let mut lv = vec![1u8; (W * H) as usize];
    for _ in 0..2 + rng.below(4) {
        let (rw, rh) = (4 + rng.below(16) as i32, 4 + rng.below(12) as i32);
        let r = Rect::new(rng.below(W) as i32 - 2, rng.below(H) as i32 - 2, rw, rh);
        let l = if rng.below(3) == 0 { 0 } else { 2 };
        for (x, y) in r.cells().filter(|&(x, y)| x >= 0 && y >= 0 && x < w && y < h) {
            lv[(y * w + x) as usize] = l;
        }
    }
    let at = |x: i32, y: i32| lv[(y * w + x) as usize];
    let mut tiles = Grid::new(W, H, Tile::Grass);
    for y in 0..h {
        for x in 0..w {
            let lower = [(0, 1), (1, 0), (0, -1), (-1, 0)].into_iter().find(|&(dx, dy)| {
                x + dx >= 0 && y + dy >= 0 && x + dx < w && y + dy < h && at(x + dx, y + dy) < at(x, y)
            });
            let t = match lower {
                Some(d) => match rng.below(20) {
                    0 if rng.below(3) == 0 => Tile::Stair,
                    1 => match d {
                        (0, 1) => Tile::LedgeS,
                        (1, 0) => Tile::LedgeE,
                        (0, -1) => Tile::LedgeN,
                        _ => Tile::LedgeW,
                    },
                    _ => Tile::Cliff,
                },
                None if walls && rng.below(9) == 0 => Tile::Wall,
                None => Tile::Grass,
            };
            tiles.set(x, y, t);
        }
    }
    ZoneGrid::with_levels(tiles, Plane::pack(W, H, &lv))
}

/// Every cell reached from `start` by the search's own rules, with the least cost to each (10
/// straight, 14 diagonal, a ledge's hop as the search costs it): a plain Dijkstra.
fn walk(g: &ZoneGrid, start: (i32, i32)) -> Vec<u32> {
    let w = W as i32;
    let open = |x: i32, y: i32| g.inside(x, y) && g.flags_at(x, y) & BLOCK_MOVE == 0;
    let mut best = vec![u32::MAX; (W * H) as usize];
    let mut heap = BinaryHeap::new();
    best[(start.1 * w + start.0) as usize] = 0;
    heap.push(std::cmp::Reverse((0u32, start)));
    while let Some(std::cmp::Reverse((d, (x, y)))) = heap.pop() {
        if d > best[(y * w + x) as usize] {
            continue;
        }
        let mut go = |(nx, ny): (i32, i32), c: u32, best: &mut Vec<u32>| {
            let i = (ny * w + nx) as usize;
            if d + c < best[i] {
                best[i] = d + c;
                heap.push(std::cmp::Reverse((d + c, (nx, ny))));
            }
        };
        for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if open(nx, ny) {
                go((nx, ny), 10, &mut best);
            } else if let Some(((lx, ly), faces)) = jane_sim::height::ledge_landing(g, (nx, ny), (dx, dy)) {
                go((lx, ly), (faces as u32 + 1) * 10 + 20, &mut best);
            }
        }
        for (dx, dy) in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if open(nx, ny) && open(nx, y) && open(x, ny) {
                go((nx, ny), 14, &mut best);
            }
        }
    }
    best
}

fn open_cells(g: &ZoneGrid) -> Vec<(i32, i32)> {
    (0..H as i32)
        .flat_map(|y| (0..W as i32).map(move |x| (x, y)))
        .filter(|&(x, y)| g.flags_at(x, y) & BLOCK_MOVE == 0)
        .collect()
}

#[test]
fn the_regions_reach_what_a_walk_reaches() {
    let mut rng = Sfc32::seeded(0x12e9, 7);
    let mut route = Route::default();
    let (mut yes, mut no) = (0, 0);
    for map in 0..60 {
        let walls = map % 2 == 1;
        let g = ground(&mut rng, walls);
        let r = g.regions().expect("regions on ground with levels");
        let cells = open_cells(&g);
        for _ in 0..40 {
            let s = cells[rng.below(cells.len() as u32) as usize];
            let reach = walk(&g, s);
            for _ in 0..10 {
                let t = cells[rng.below(cells.len() as u32) as usize];
                let (Some(rs), Some(rt)) = (r.region(s.0, s.1), r.region(t.0, t.1)) else { continue };
                route.prepare(r, &g, (s, t), rt, Some((s, 10_000)));
                let said = route.bound(r, rs, s) < FAR;
                let walked = reach[(t.1 * W as i32 + t.0) as usize] != u32::MAX;
                if walls {
                    assert!(said || !walked, "map {map}: {s:?} -> {t:?} walked but the regions said no");
                } else {
                    assert_eq!(said, walked, "map {map}: {s:?} -> {t:?}");
                }
                if walked { yes += 1 } else { no += 1 }
            }
        }
    }
    assert!(yes > 1000 && no > 1000, "both kinds tried: {yes} reached, {no} not");
}

#[test]
fn the_steered_search_finds_what_a_walk_finds() {
    let mut rng = Sfc32::seeded(0x5ea2, 3);
    let mut p = PathScratch::new();
    let (mut found, mut longer, mut cost_sum, mut best_sum) = (0, 0, 0u64, 0u64);
    for map in 0..40 {
        let g = ground(&mut rng, map % 2 == 1);
        let cells = open_cells(&g);
        for _ in 0..30 {
            let s = cells[rng.below(cells.len() as u32) as usize];
            let reach = walk(&g, s);
            for _ in 0..8 {
                let t = cells[rng.below(cells.len() as u32) as usize];
                let ask = PathAsk {
                    ledges: Some((s, 10_000)),
                    budget: 1_000_000,
                    ..PathAsk::new(s, t, cost_of_cells(20_000))
                };
                let end = p.find(&g, ask);
                let least = reach[(t.1 * W as i32 + t.0) as usize];
                assert_eq!(end == Some(PathEnd::Found), least != u32::MAX, "map {map}: {s:?} -> {t:?}");
                if least == u32::MAX || s == t {
                    continue;
                }
                // The path is walkable by the same rules, and costs what it says.
                let mut cost = 0u64;
                let mut prev = s;
                for &c in &p.out {
                    let (dx, dy) = (c.0 - prev.0, c.1 - prev.1);
                    cost += match (dx.abs(), dy.abs()) {
                        (1, 0) | (0, 1) => 10,
                        (1, 1) => 14,
                        _ => {
                            let dir = (dx.signum(), dy.signum());
                            let landing = jane_sim::height::ledge_landing(&g, (prev.0 + dir.0, prev.1 + dir.1), dir);
                            assert_eq!(
                                landing.map(|l| l.0),
                                Some(c),
                                "map {map}: a jump is a hop: {s:?} -> {t:?} at {prev:?} -> {c:?} path {:?} tiles {:?} {:?}",
                                p.out,
                                g.tile_at(prev.0 + dir.0, prev.1 + dir.1),
                                g.tile_at(c.0, c.1)
                            );
                            (dx.abs().max(dy.abs()) as u64) * 10 + 20
                        }
                    };
                    prev = c;
                }
                assert_eq!(prev, t);
                found += 1;
                longer += usize::from(cost > u64::from(least));
                cost_sum += cost;
                best_sum += u64::from(least);
            }
        }
    }
    assert!(found > 2000, "{found}");
    // Leg by leg it may take a portal that is not the very best for the whole way; never by much.
    assert!(cost_sum * 100 <= best_sum * 105, "{cost_sum} against the least {best_sum} ({longer} longer of {found})");
}

#[test]
fn a_flat_grid_has_no_regions_and_its_paths_are_not_steered() {
    let g = ZoneGrid::new(Grid::new(40, 30, Tile::Grass));
    assert!(g.regions().is_none());
    let mut p = PathScratch::new();
    assert_eq!(p.find(&g, PathAsk::new((1, 1), (38, 28), cost_of_cells(200))), Some(PathEnd::Found));
    assert_eq!((p.stats.steered, p.stats.unreachable), (0, 0));
}
