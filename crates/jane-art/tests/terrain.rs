//! The chunk painter's acceptance tests (ART.md §5, PRESENTATION.md §1.6): goldens, determinism,
//! a tile change reaching only the chunks `chunks_touched` names, the layer contract on the ground
//! and in the strips, and every style resolving.
//!
//! Re-bless the goldens after an intended change with either
//!
//! ```text
//! cargo run -p jane-cli -- sheet --bless
//! JANE_BLESS=1 cargo test -p jane-art --test terrain
//! ```

use std::collections::BTreeMap;

use jane_art::canvas::decode;
use jane_art::terrain::sheet::{golden_file, sample_county, sample_indoor};
use jane_art::terrain::{CHUNK_CELLS, CHUNK_PX, Chunk, Painter, Styles, TileMap, TileSource, chunks_touched, mask};
use jane_core::Tile;
use jane_core::grid::Rect;

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/terrain_golden.txt");

fn parse(s: &str) -> BTreeMap<String, String> {
    s.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once(' ').map(|(k, v)| (k.to_string(), v.trim().to_string())))
        .collect()
}

#[test]
fn every_chunk_and_sprite_matches_its_golden() {
    let now = golden_file(&mut Painter::new());
    if std::env::var_os("JANE_BLESS").is_some() {
        std::fs::write(PATH, &now).unwrap();
        return;
    }
    let want =
        parse(&std::fs::read_to_string(PATH).expect("tests/terrain_golden.txt: bless it (see this file's header)"));
    let got = parse(&now);
    let mut diff: Vec<String> = got
        .iter()
        .filter(|(k, v)| want.get(*k) != Some(v))
        .map(|(k, v)| format!("{k}: {} in the golden, now {v}", want.get(k).map_or("nothing", String::as_str)))
        .collect();
    diff.extend(want.keys().filter(|k| !got.contains_key(*k)).map(|k| format!("{k}: no longer painted")));
    assert!(diff.is_empty(), "terrain drifted; re-bless if this is intended:\n{}", diff.join("\n"));
}

fn hashes(p: &mut Painter, map: &TileMap, seed: u32) -> Vec<((i32, i32), u32)> {
    let (w, h) = map.size();
    let mut out = Vec::new();
    let mut c = Chunk::new();
    for cy in 0..(h + CHUNK_CELLS - 1) / CHUNK_CELLS {
        for cx in 0..(w + CHUNK_CELLS - 1) / CHUNK_CELLS {
            p.paint(map, seed, cx, cy, &mut c);
            out.push(((cx, cy), c.hash()));
        }
    }
    out
}

#[test]
fn the_same_seed_and_chunk_paint_the_same_bytes_in_any_order() {
    let map = sample_county();
    let mut p = Painter::new();
    let first = hashes(&mut p, &map, 3);
    // Again with a painter that has painted something else first, chunk by chunk backwards.
    let mut q = Painter::new();
    hashes(&mut q, &sample_indoor(), 9);
    let mut c = Chunk::new();
    for &((cx, cy), want) in first.iter().rev() {
        q.paint(&map, 3, cx, cy, &mut c);
        assert_eq!(c.hash(), want, "chunk ({cx}, {cy})");
    }
    let other = hashes(&mut p, &map, 4);
    assert_ne!(first, other, "the seed varies the ground");
}

#[test]
fn a_tile_change_reaches_only_the_chunks_it_touches() {
    let mut map = sample_county();
    let mut p = Painter::new();
    let before = hashes(&mut p, &map, 5);
    // Changes of every kind: a tree into water, a house wall into grass, a road into a cliff.
    for (r, t) in [
        (Rect::new(47, 5, 2, 2), Tile::Water),
        (Rect::new(28, 9, 1, 1), Tile::Grass),
        (Rect::new(31, 22, 3, 2), Tile::Cliff),
    ] {
        map.tiles.fill_rect(r, t);
        let after = hashes(&mut p, &map, 5);
        let reach = chunks_touched(r);
        for ((id, a), (_, b)) in before.iter().zip(&after) {
            if a != b {
                assert!(reach.contains(id.0, id.1), "{r:?} changed chunk {id:?}, outside {reach:?}");
            }
        }
        assert!(before != after, "{r:?} changed nothing");
        map = sample_county();
    }
}

#[test]
fn chunks_keep_the_layer_contract() {
    let mut p = Painter::new();
    for map in [sample_county(), sample_indoor()] {
        let (w, h) = map.size();
        let mut c = Chunk::new();
        for cy in 0..(h + CHUNK_CELLS - 1) / CHUNK_CELLS {
            for cx in 0..(w + CHUNK_CELLS - 1) / CHUNK_CELLS {
                p.paint(&map, 1, cx, cy, &mut c);
                let l = &c.layers;
                let n = (CHUNK_PX * CHUNK_PX) as usize;
                assert_eq!([l.albedo.len(), l.normal.len(), l.emissive.len(), l.height.len()], [n; 4]);
                for i in 0..n {
                    assert_eq!(l.albedo[i] >> 24, 0xff, "the ground is opaque");
                    assert!(l.height[i] >= 1, "height at least 1 on the ground");
                    let [x, y, z] = decode(l.normal[i]);
                    let len = jane_core::num::isqrt(((x * x + y * y + z * z) * 255 * 255 / (127 * 127)) as u64);
                    assert!(len.abs_diff(255) <= 2, "a normal of length {len}");
                }
                for s in c.strips() {
                    let k = usize::from(s.w) * usize::from(s.h);
                    assert_eq!([s.albedo.len(), s.normal.len(), s.height.len(), s.mask.len()], [k; 4]);
                    for i in 0..k {
                        let drawn = s.albedo[i] != 0;
                        assert_eq!(drawn, s.height[i] >= 1, "height marks what is drawn");
                        assert_eq!(drawn, s.mask[i] != mask::CLEAR, "the mask marks what is drawn");
                    }
                }
                for w in &c.water {
                    assert!(w.phase < 64 && i32::from(w.x) < CHUNK_CELLS && i32::from(w.y) < CHUNK_CELLS);
                }
                for s in &c.casters {
                    assert!(s.height > 0 && (s.a.0 == s.b.0 || s.a.1 == s.b.1), "{s:?}");
                }
            }
        }
    }
}

#[test]
fn the_county_sample_has_canopy_water_casters_and_lit_windows() {
    let map = sample_county();
    let mut p = Painter::new();
    let mut c = Chunk::new();
    let (mut canopy, mut water, mut casters, mut glow) = (0, 0, 0, 0);
    for cy in 0..3 {
        for cx in 0..4 {
            p.paint(&map, 1, cx, cy, &mut c);
            canopy += c.strips().iter().flat_map(|s| &s.mask).filter(|&&m| m == mask::CANOPY).count();
            water += c.water.len();
            casters += c.casters.len();
            glow += c.layers.emissive.iter().filter(|e| e.0 != 0).count();
            let bank = p.bank().all().len();
            assert!(c.placed.iter().all(|t| usize::from(t.sprite) < bank && c.strips().iter().any(|s| s.row == t.row)));
        }
    }
    assert!(canopy > 1000 && water > 50 && casters > 20 && glow > 0, "{canopy} {water} {casters} {glow}");
}

#[test]
fn every_style_resolves_its_ramps() {
    Styles::from_looks(&jane_data::tile_looks()).unwrap();
}

#[test]
fn a_placed_fence_marks_its_px_stands_them_off_the_ground_and_gives_its_posts_and_rails() {
    // The sample's fences: a run across cells 2..14 of row 2 and down columns 2 and 13 to row 9
    // (PRESENTATION.md §1.7, the fence rule).
    let map = sample_county();
    let mut p = Painter::new();
    p.set_standing(jane_art::terrain::Standing::Placed);
    let mut c = Chunk::new();
    p.paint(&map, 1, 0, 0, &mut c);
    let fence_px = (0..CHUNK_PX * CHUNK_PX).filter(|&k| c.is_fence(k % CHUNK_PX, k / CHUNK_PX)).count();
    // Twelve cells across, eight down each side: posts and rails, a few hundred px a cell.
    assert!(fence_px > 20 * 60, "{fence_px} px marked a fence's");
    for k in 0..CHUNK_PX * CHUNK_PX {
        if c.is_fence(k % CHUNK_PX, k / CHUNK_PX) {
            assert!(c.layers.height[k as usize] >= jane_art::terrain::FENCE_FLOOR, "a fence px on the ground at {k}");
        }
    }
    // A post a fence cell from the ground to 28 px; every other part floats (a rail).
    let posts = c.fences.iter().filter(|f| f.lo == 0).count();
    assert_eq!(posts, 12 + 2 * 7, "{:?}", c.fences);
    assert!(c.fences.iter().all(|f| f.lo == 0 && f.hi == 28 || f.lo > 0 && f.hi > f.lo));
    // Each post stands on its cell's foot row, under its drawn px.
    for f in c.fences.iter().filter(|f| f.lo == 0) {
        let (x, y) = (i32::from(f.x0) + 2, i32::from(f.y1) - 1);
        assert!(c.is_fence(x, y) && c.is_fence(x, y - 20), "a post's part off its drawing at ({x}, {y})");
    }
}
