//! `jane view`: the seed viewer as PNGs (PORT.md §5.6). Today it draws the skeleton's land;
//! each worldgen stage adds its layer as it lands.

use std::path::Path;

use jane_art::sheet::Image;
use jane_world::skeleton::{Biome, SKEL_H, SKEL_W, Terrain, Water, build_terrain};

mod dungeon;

const SCALE: u32 = 4;

fn biome_rgb(b: Biome) -> [u8; 3] {
    match b {
        Biome::Field => [150, 178, 96],
        Biome::Hedge => [112, 150, 80],
        Biome::Wood => [58, 102, 56],
        Biome::Foothill => [150, 150, 104],
        Biome::Reed => [140, 160, 110],
        Biome::Marsh => [104, 128, 96],
        Biome::WetWood => [66, 96, 72],
        Biome::Garden => [168, 160, 110],
        Biome::Slag => [110, 104, 100],
        Biome::Yard => [140, 128, 112],
        Biome::Hill => [128, 118, 96],
        Biome::Town => [180, 150, 120],
    }
}

/// The land of one seed as an image: biome colour lit by height, water over it.
pub fn terrain_image(t: &Terrain) -> Image {
    let mut img = Image::new(SKEL_W as u32 * SCALE, SKEL_H as u32 * SCALE, [0, 0, 0, 255]);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let rgb = match t.water.read(x, y, Water::Dry) {
                Water::River => [70, 120, 190],
                Water::Lake => [50, 96, 170],
                Water::Dry => {
                    let [r, g, b] = biome_rgb(t.biome.read(x, y, Biome::Field));
                    // Height as light: 0 darkens by a third, 255 lightens by a third.
                    let h = i32::from(t.height.read(x, y, 0));
                    let k = |c: u8| (i32::from(c) * (192 + h / 2) / 256).clamp(0, 255) as u8;
                    [k(r), k(g), k(b)]
                }
            };
            img.block(x as u32, y as u32, SCALE, [rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    let (cx, cy) = t.crown;
    img.block(cx as u32, cy as u32, SCALE, [250, 230, 90, 255]);
    img
}

/// `--seeds A..B` (inclusive of A, exclusive of B, like Rust), or `--seed N`.
fn seeds(args: &[String]) -> Result<std::ops::Range<u32>, String> {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1));
    if let Some(s) = get("--seed") {
        let n: u32 = s.parse().map_err(|_| format!("bad seed {s}"))?;
        return Ok(n..n + 1);
    }
    let s = get("--seeds").map_or("1..25", String::as_str);
    let (a, b) = s.split_once("..").ok_or_else(|| format!("bad range {s}"))?;
    Ok(a.parse().map_err(|_| format!("bad range {s}"))?..b.parse().map_err(|_| format!("bad range {s}"))?)
}

pub fn run(args: &[String]) -> Result<(), String> {
    if let Some(which) = args.iter().position(|a| a == "--dungeon").and_then(|i| args.get(i + 1)) {
        return dungeon::run(args, which);
    }
    let range = seeds(args)?;
    let out = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map_or("sheets", String::as_str);
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
    for seed in range {
        let t = build_terrain(seed, 0);
        let path = Path::new(out).join(format!("county-{seed}.png"));
        std::fs::write(&path, terrain_image(&t).png()).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}
