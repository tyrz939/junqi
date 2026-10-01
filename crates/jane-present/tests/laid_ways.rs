//! A laid way keeps its edge (ART.md §3.1, the ecotone): where wild grounds drift into each other
//! across their edge, no px inside a lane's, a walk's or a yard's cells is turned over, and no
//! drift of earth eats into the ground beside one unless the land's own open earth is near.

use jane_art::terrain::{CHUNK_CELLS, Chunk, Painter, TileMap, TileSource};
use jane_core::{Material, Tile};

/// Dirt the land did not lay: a way, a yard, a town's ground.
fn laid(m: &TileMap, x: i32, y: i32) -> bool {
    m.tile(x, y) == Tile::Dirt && m.material(x, y) != Some(Material::WildEarth)
}

fn wild_near(m: &TileMap, x: i32, y: i32, r: i32) -> bool {
    (-r..=r).any(|j| (-r..=r).any(|i| m.material(x + i, y + j) == Some(Material::WildEarth)))
}

#[test]
fn no_drift_turns_over_a_laid_way() {
    let dirt = Tile::Dirt.id();
    for seed in [1u32, 6] {
        let sk = jane_world::county::county_skeleton(seed, 0).expect("a county");
        let mut c = jane_world::county::County::new(&sk, 0);
        for (_, stage) in jane_world::county::STAGES {
            stage(&mut c);
        }
        let map = TileMap::from_blueprint(&c.done());
        let (w, h) = map.size();
        assert!(
            (0..h).any(|y| (0..w).any(|x| map.material(x, y) == Some(Material::WildEarth))),
            "seed {seed}: the land laid open earth"
        );
        // Every chunk with a laid way in it, a few hundred of them spread over the county.
        let mut chunks = Vec::new();
        for cy in 0..h / CHUNK_CELLS {
            for cx in 0..w / CHUNK_CELLS {
                let n = (0..CHUNK_CELLS)
                    .flat_map(|j| (0..CHUNK_CELLS).map(move |i| (i, j)))
                    .filter(|&(i, j)| laid(&map, cx * CHUNK_CELLS + i, cy * CHUNK_CELLS + j))
                    .count();
                if n > 8 {
                    chunks.push((cx, cy));
                }
            }
        }
        assert!(chunks.len() > 50, "seed {seed}: {} chunks with ways", chunks.len());
        let step = (chunks.len() / 300).max(1);
        let (mut p, mut out) = (Painter::new(), Chunk::new());
        let (mut drifted, mut checked) = (0, 0);
        for &(cx, cy) in chunks.iter().step_by(step) {
            p.paint(&map, seed, cx, cy, &mut out);
            checked += 1;
            for &(px, py, from, to) in p.drifts() {
                drifted += 1;
                let (x, y) = (cx * CHUNK_CELLS + i32::from(px) / 16, cy * CHUNK_CELLS + i32::from(py) / 16);
                assert!(!laid(&map, x, y), "seed {seed}: a drift turned over px ({px}, {py}) of laid cell ({x}, {y})");
                let beside = (-1..=1).any(|j| (-1..=1).any(|i| laid(&map, x + i, y + j)));
                assert!(
                    !(beside && (to == dirt || from == dirt) && !wild_near(&map, x, y, 3)),
                    "seed {seed}: a drift of earth at ({x}, {y}) beside a way, with no open earth near"
                );
            }
        }
        // The drifts still run where the wild grounds meet.
        assert!(checked > 50 && drifted > 0, "seed {seed}: {checked} chunks, {drifted} drifted px");
    }
}
