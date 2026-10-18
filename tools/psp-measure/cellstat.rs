//! THROWAWAY measurement for progress/PLAN-psp-streaming.md (not committed). Paints chunks of the
//! baked county on the host and counts distinct 16x16 cells, contexts, colours and compressed sizes.
//! cargo run --release -p jane-cli --example cellstat -- [x0 y0 w h] (chunk coords; default all)
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::Instant;

use jane_art::terrain::{self, CHUNK_CELLS, CHUNK_PX, Chunk, Painter, TileMap, TileSource};

fn h64<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn main() {
    let a: Vec<i32> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let bytes = std::fs::read("assets/world/county.bp").expect("county.bp");
    let bp = jane_sim::baked::decode(&bytes, jane_core::ZoneId::County, false).expect("decode");
    let seed = 0x5eed_u32;
    let map = TileMap::from_blueprint_seeded(&bp, seed);
    let (w, h) = map.size();
    let (kw, kh) = (w / CHUNK_CELLS, h / CHUNK_CELLS);
    let (x0, y0, nx, ny) = if a.len() == 4 { (a[0], a[1], a[2], a[3]) } else { (0, 0, kw, kh) };
    println!("county {w}x{h} cells, {kw}x{kh} chunks; painting {nx}x{ny} from {x0},{y0}");
    let mut p = Painter::new();
    p.drop_normals();
    let mut chunk = Chunk::new();
    chunk.drop_normals();
    let side = CHUNK_PX as usize;
    let cp = terrain::CELL as usize;
    let mut cells_alb: HashMap<u64, u32> = HashMap::new();
    let mut cells_alb_h: HashSet<u64> = HashSet::new();
    let mut blocks8: HashSet<u64> = HashSet::new();
    let mut ctx: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut all_colours: HashSet<u32> = HashSet::new();
    let mut times = Vec::new();
    let (mut n_cells, mut lz_t8, mut lz_h, mut lz_hrel_zero, mut over256) = (0u64, 0usize, 0usize, 0usize, 0u32);
    let (mut placed, mut water, mut casters, mut fences) = (0usize, 0usize, 0usize, 0usize);
    let mut colours_per_chunk = Vec::new();
    let mut flat_h_cells = 0u64;
    let (mut dec_ns, mut ent_bytes) = (0u128, 0f64);
    for ky in y0..y0 + ny {
        for kx in x0..x0 + nx {
            let t = Instant::now();
            terrain::paint_chunk(&mut p, &map, seed, kx, ky, &mut chunk);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            let l = &chunk.layers;
            placed += chunk.placed.len();
            water += chunk.water.len();
            casters += chunk.casters.len();
            fences += chunk.fences.len();
            // T8 of the chunk
            let mut clut: HashMap<u32, u8> = HashMap::new();
            let mut idx = vec![0u8; side * side];
            for (k, &c) in l.albedo.iter().enumerate() {
                all_colours.insert(c);
                let n = clut.len();
                let e = *clut.entry(c).or_insert(n.min(255) as u8);
                idx[k] = e;
            }
            if clut.len() > 256 {
                over256 += 1;
            }
            colours_per_chunk.push(clut.len());
            let comp = lz4_flex::block::compress(&idx);
            lz_t8 += comp.len() + clut.len().min(256) * 3;
            let td = Instant::now();
            let back = lz4_flex::block::decompress(&comp, idx.len()).unwrap();
            dec_ns += td.elapsed().as_nanos();
            assert_eq!(back.len(), idx.len());
            // order-1 entropy (context: left px index, and up px index) lower bound in bits
            let mut c1: HashMap<(u8, u8), HashMap<u8, u32>> = HashMap::new();
            for k in 0..idx.len() {
                let left = if k % side > 0 { idx[k - 1] } else { 255 };
                let up = if k >= side { idx[k - side] } else { 255 };
                *c1.entry((left, up)).or_default().entry(idx[k]).or_insert(0) += 1;
            }
            let mut bits = 0f64;
            for m in c1.values() {
                let t: u32 = m.values().sum();
                for &v in m.values() {
                    bits -= f64::from(v) * (f64::from(v) / f64::from(t)).log2();
                }
            }
            ent_bytes += bits / 8.0 + clut.len() as f64 * 3.0;
            lz_h += lz4_flex::block::compress(&l.height).len();
            let all1 = l.height.iter().all(|&v| v <= 1);
            if all1 {
                lz_hrel_zero += 1;
            }
            for cy in 0..CHUNK_CELLS as usize {
                for cx in 0..CHUNK_CELLS as usize {
                    let mut px = Vec::with_capacity(cp * cp);
                    let mut hh = Vec::with_capacity(cp * cp);
                    for y in 0..cp {
                        let r = (cy * cp + y) * side + cx * cp;
                        px.extend_from_slice(&l.albedo[r..r + cp]);
                        hh.extend_from_slice(&l.height[r..r + cp]);
                    }
                    if hh.iter().all(|&v| v == hh[0]) {
                        flat_h_cells += 1;
                    }
                    let ha = h64(&px[..]);
                    *cells_alb.entry(ha).or_insert(0) += 1;
                    cells_alb_h.insert(h64(&(ha, &hh[..])));
                    for by in 0..2 {
                        for bx in 0..2 {
                            let b: Vec<u32> = (0..8).flat_map(|y| px[(by * 8 + y) * cp + bx * 8..][..8].to_vec()).collect();
                            blocks8.insert(h64(&b[..]));
                        }
                    }
                    // context: 3x3 tiles + material + region of the centre
                    let (wx, wy) = (kx * CHUNK_CELLS + cx as i32, ky * CHUNK_CELLS + cy as i32);
                    let mut k = Vec::with_capacity(12);
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            k.push(map.tile(wx + dx, wy + dy) as u8);
                        }
                    }
                    k.push(map.material(wx, wy).map_or(0, |m| m as u8 + 1));
                    k.push(map.region(wx, wy));
                    ctx.entry(h64(&k[..])).or_default().insert(ha);
                    n_cells += 1;
                }
            }
        }
    }
    times.sort_by(f64::total_cmp);
    let n = times.len();
    let mean = times.iter().sum::<f64>() / n as f64;
    colours_per_chunk.sort();
    let singles = cells_alb.values().filter(|&&v| v == 1).count();
    let ctx_n = ctx.len();
    let ctx_cells: usize = ctx.values().map(HashSet::len).sum();
    let mut ctx_sizes: Vec<usize> = ctx.values().map(HashSet::len).collect();
    ctx_sizes.sort();
    println!("chunks {n}: host paint mean {mean:.2} ms, p50 {:.2}, p95 {:.2}, max {:.2}", times[n / 2], times[n * 95 / 100], times[n - 1]);
    println!("cells {n_cells}; distinct albedo cells {} ({:.1}%), seen once {singles}; distinct albedo+height {}", cells_alb.len(), 100.0 * cells_alb.len() as f64 / n_cells as f64, cells_alb_h.len());
    println!("distinct 8x8 albedo blocks {} of {}", blocks8.len(), n_cells * 4);
    println!("cells with flat height {flat_h_cells}; chunks with all height<=1 {lz_hrel_zero}");
    println!("3x3-context classes {ctx_n}; distinct cells over classes {ctx_cells}; cells/class p50 {} p90 {} max {}", ctx_sizes[ctx_n / 2], ctx_sizes[ctx_n * 9 / 10], ctx_sizes[ctx_n - 1]);
    println!("colours: county-wide {}, per chunk p50 {} p95 {} max {}, chunks over 256: {over256}", all_colours.len(), colours_per_chunk[n / 2], colours_per_chunk[n * 95 / 100], colours_per_chunk[n - 1]);
    println!("lz4 per chunk: T8+clut mean {:.1} KB, height mean {:.1} KB; total {:.1} MB + {:.1} MB", lz_t8 as f64 / n as f64 / 1024.0, lz_h as f64 / n as f64 / 1024.0, lz_t8 as f64 / 1048576.0, lz_h as f64 / 1048576.0);
    println!("lz4 decode host mean {:.1} us/chunk; order-1 (left,up) entropy bound mean {:.1} KB/chunk", dec_ns as f64 / n as f64 / 1000.0, ent_bytes / n as f64 / 1024.0);
    println!("per chunk: placed {:.1}, water cells {:.1}, casters {:.1}, fence parts {:.1}", placed as f64 / n as f64, water as f64 / n as f64, casters as f64 / n as f64, fences as f64 / n as f64);
}
