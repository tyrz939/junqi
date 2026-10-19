//! THROWAWAY (PLAN-PSP-SPEED.md P0e): dump one painted console-form chunk as a 256x256 T8 index
//! plane (palette sorted by frequency, as chunkcodec.rs), 65,536 bytes, for the PSP micro-bench
//! (spikes/psp-game/src/micro.rs). To run: copy to crates/jane-cli/examples/, then
//! cargo run --release -p jane-cli --example microchunk -- OUT_RAW [KX KY]
//! and compress with tools/psp-measure/microchunk.py.
use jane_art::terrain::{self, CHUNK_CELLS, Chunk, Painter, TileMap, TileSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (kx, ky) = (args.get(1).map_or(24, |s| s.parse().unwrap()), args.get(2).map_or(24, |s| s.parse().unwrap()));
    let bytes = std::fs::read("assets/world/county.bp").expect("county.bp");
    let bp = jane_sim::baked::decode(&bytes, jane_core::ZoneId::County, false).expect("decode");
    let seed = 0x5eed_u32;
    let map = TileMap::from_blueprint_seeded(&bp, seed);
    let (w, h) = map.size();
    assert!(kx < w / CHUNK_CELLS && ky < h / CHUNK_CELLS);
    let mut p = Painter::new();
    p.drop_normals();
    let mut chunk = Chunk::new();
    chunk.drop_normals();
    terrain::paint_chunk(&mut p, &map, seed, kx, ky, &mut chunk);
    let mut cnt: std::collections::HashMap<u32, u32> = Default::default();
    for &c in &chunk.layers.albedo {
        *cnt.entry(c).or_insert(0) += 1;
    }
    let mut pal: Vec<(u32, u32)> = cnt.into_iter().collect();
    pal.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let ix: std::collections::HashMap<u32, u8> = pal.iter().enumerate().map(|(i, e)| (e.0, i as u8)).collect();
    let idx: Vec<u8> = chunk.layers.albedo.iter().map(|c| ix[c]).collect();
    println!("{} px, {} colours", idx.len(), pal.len());
    std::fs::write(&args[0], idx).unwrap();
}
