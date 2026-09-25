//! `jane view`: the seed viewer as PNGs (PORT.md §5.6). Draws the skeleton: the land, the patches
//! (outlined in their threat's colour), the roads (lit ones brighter), bridges, the railway, the
//! small places, the anchors and the story's sites. Each later worldgen stage adds its layer as it
//! lands. Beside each sheet a `.txt` names the sites by colour and lists the checks.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use jane_art::sheet::Image;
use jane_world::skeleton::{
    Biome, MACRO, ROAD, ROAD_BRIDGE, ROAD_LIT, SKEL_H, SKEL_W, Skeleton, Terrain, Water, build_terrain, skeleton,
};

mod dungeon;

/// Pixels per macro cell.
const SCALE: u32 = 6;

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

/// Threat 0 (a haven) to 6, green through amber to red and purple.
fn threat_rgb(t: u8) -> [u8; 3] {
    match t {
        0 => [80, 200, 120],
        1 => [170, 210, 90],
        2 => [230, 210, 70],
        3 => [240, 160, 50],
        4 => [230, 100, 40],
        5 => [200, 40, 40],
        _ => [150, 30, 150],
    }
}

/// Sixteen colours a site is told apart by, in row order; the `.txt` beside the sheet names them.
const SITE_COLOURS: [([u8; 3], &str); 16] = [
    ([255, 255, 255], "white"),
    ([255, 120, 200], "pink"),
    ([255, 220, 0], "yellow"),
    ([120, 60, 20], "brown"),
    ([255, 140, 0], "orange"),
    ([0, 200, 255], "cyan"),
    ([0, 90, 255], "blue"),
    ([160, 90, 255], "violet"),
    ([0, 255, 120], "green"),
    ([0, 255, 255], "aqua"),
    ([90, 90, 90], "grey"),
    ([255, 0, 0], "red"),
    ([255, 200, 150], "peach"),
    ([0, 0, 0], "black"),
    ([200, 255, 0], "lime"),
    ([128, 0, 0], "maroon"),
];

fn rgba([r, g, b]: [u8; 3]) -> [u8; 4] {
    [r, g, b, 255]
}

/// The centre pixel of a macro cell.
fn centre(x: i32, y: i32) -> (i32, i32) {
    (x * SCALE as i32 + SCALE as i32 / 2, y * SCALE as i32 + SCALE as i32 / 2)
}

fn dot(img: &mut Image, (px, py): (i32, i32), r: i32, c: [u8; 4]) {
    for y in py - r..=py + r {
        for x in px - r..=px + r {
            if x >= 0 && y >= 0 {
                img.set(x as u32, y as u32, c);
            }
        }
    }
}

/// A line `2r + 1` pixels thick between two pixels (Bresenham).
fn line(img: &mut Image, (x0, y0): (i32, i32), (x1, y1): (i32, i32), r: i32, c: [u8; 4]) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        dot(img, (x, y), r, c);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// A ring of `radius` pixels, two thick.
fn ring(img: &mut Image, (cx, cy): (i32, i32), radius: i32, c: [u8; 4]) {
    let (outer, inner) = (radius * radius, (radius - 2).max(0).pow(2));
    for y in cy - radius..=cy + radius {
        for x in cx - radius..=cx + radius {
            let d = (x - cx).pow(2) + (y - cy).pow(2);
            if d <= outer && d > inner && x >= 0 && y >= 0 {
                img.set(x as u32, y as u32, c);
            }
        }
    }
}

/// The land of one seed as an image: biome colour lit by height (or the threat, with `threat`), water over it.
fn land_image(t: &Terrain, threat: Option<&jane_core::Grid<u8>>) -> Image {
    let mut img = Image::new(SKEL_W as u32 * SCALE, SKEL_H as u32 * SCALE, [0, 0, 0, 255]);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let rgb = match t.water.read(x, y, Water::Dry) {
                Water::River => [70, 120, 190],
                Water::Lake => [50, 96, 170],
                Water::Dry => {
                    let [r, g, b] = match threat {
                        Some(g) => threat_rgb(g.read(x, y, 0)),
                        None => biome_rgb(t.biome.read(x, y, Biome::Field)),
                    };
                    // Height as light: 0 darkens by a quarter, 255 lightens by a quarter.
                    let h = i32::from(t.height.read(x, y, 0));
                    let k = |c: u8| (i32::from(c) * (192 + h / 2) / 256).clamp(0, 255) as u8;
                    [k(r), k(g), k(b)]
                }
            };
            img.block(x as u32, y as u32, SCALE, rgba(rgb));
        }
    }
    img
}

/// The skeleton of one seed: land, patches, roads, rail, small places, sites.
pub fn skeleton_image(s: &Skeleton, threat: bool) -> Image {
    let t = &s.terrain;
    let mut img = land_image(t, threat.then_some(&s.threat));
    let (cx, cy) = t.crown;
    dot(&mut img, centre(cx, cy), 1, [250, 230, 90, 255]);

    // Patches: a ring at their radius in their threat's colour, a dark one inside it so it reads on any ground.
    for a in &s.areas {
        let r = i32::from(a.def.radius) * SCALE as i32 / MACRO;
        ring(&mut img, centre(a.mx, a.my), r + 1, [20, 20, 20, 255]);
        ring(&mut img, centre(a.mx, a.my), r, rgba(threat_rgb(a.def.threat)));
    }

    // Roads: unlit first, lit over them, then the bridges.
    let (dark, lit) = ([96, 84, 70, 255], [255, 236, 150, 255]);
    for pass in [false, true] {
        for r in &s.roads {
            for w in r.cells.windows(2) {
                let is_lit = |(x, y): (i32, i32)| s.road.read(x, y, 0) & ROAD_LIT != 0;
                let on = is_lit(w[0]) && is_lit(w[1]);
                if on == pass {
                    line(&mut img, centre(w[0].0, w[0].1), centre(w[1].0, w[1].1), 1, if on { lit } else { dark });
                }
            }
        }
    }
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let bits = s.road.read(x, y, 0);
            if bits & ROAD_BRIDGE != 0 {
                dot(&mut img, centre(x, y), 2, [150, 80, 30, 255]);
            } else if bits & ROAD != 0 && bits & ROAD_LIT != 0 {
                dot(&mut img, centre(x, y), 0, [255, 255, 220, 255]);
            }
        }
    }

    // The railway: dark ballast, light sleepers every other cell.
    for w in s.rail.windows(2) {
        line(&mut img, centre(w[0].0, w[0].1), centre(w[1].0, w[1].1), 1, [30, 30, 36, 255]);
    }
    for (k, &(x, y)) in s.rail.iter().enumerate() {
        if k % 2 == 0 {
            dot(&mut img, centre(x, y), 0, [190, 190, 200, 255]);
        }
    }

    // Small places: a pale dot; the story's anchors: magenta.
    for p in &s.pois {
        let c = if p.anchor.is_some() { [230, 0, 230, 255] } else { [245, 245, 235, 255] };
        dot(&mut img, centre(p.mx, p.my), 1, [0, 0, 0, 255]);
        dot(&mut img, centre(p.mx, p.my), 0, c);
    }

    // Sites: a block of their colour, framed black (red for a dungeon mouth).
    for x in &s.sites {
        let (px, py) = centre(x.mx, x.my);
        let frame = if x.def.dungeon { [220, 0, 0, 255] } else { [0, 0, 0, 255] };
        dot(&mut img, (px, py), 6, frame);
        dot(&mut img, (px, py), 4, rgba(SITE_COLOURS[usize::from(x.row) % SITE_COLOURS.len()].0));
    }
    img
}

/// What the sheet shows, in words: the sites by colour, the attempt, the checks.
pub fn legend(s: &Skeleton) -> String {
    let cat = jane_data::catalog();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "seed {} attempt {} ({} tried, {} steps back) {}",
        s.seed,
        s.attempt,
        s.stats.attempts,
        s.stats.backjumps,
        if s.ok { "ok" } else { "FAILED" }
    );
    for x in &s.sites {
        let colour = SITE_COLOURS[usize::from(x.row) % SITE_COLOURS.len()].1;
        let _ = writeln!(out, "  {colour:7} {:18} ({:3},{:3}) {}", x.def.id, x.mx, x.my, cat.text(x.def.name));
    }
    for a in &s.areas {
        let _ = writeln!(out, "  area    {:18} ({:3},{:3}) threat {}", cat.name(a.def.id), a.mx, a.my, a.def.threat);
    }
    for c in &s.checks {
        let _ = writeln!(out, "  [{}] {}: {}", if c.ok { "ok" } else { "!!" }, c.rule, c.detail);
    }
    out
}

/// The land alone, for a seed whose skeleton cannot be built.
pub fn terrain_image(t: &Terrain) -> Image {
    land_image(t, None)
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
    if args.iter().any(|a| a == "--county") {
        return run_county(args);
    }
    let range = seeds(args)?;
    let out = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map_or("sheets", String::as_str);
    let threat = args.iter().any(|a| a == "--threat");
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
    let write = |path: &Path, bytes: &[u8]| std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()));
    for seed in range {
        let path = Path::new(out).join(format!("county-{seed}.png"));
        match skeleton(seed) {
            Ok(s) => {
                write(&path, &skeleton_image(&s, threat).png())?;
                write(&path.with_extension("txt"), legend(&s).as_bytes())?;
                println!("{} attempt {}{}", path.display(), s.attempt, if s.ok { "" } else { " FAILED" });
            }
            Err(e) => {
                write(&path, &terrain_image(&build_terrain(seed, 0)).png())?;
                println!("{}: land only: {e}", path.display());
            }
        }
    }
    Ok(())
}

// --- the county in cells -------------------------------------------------------------------

/// A tile's colour on the county sheet.
fn tile_rgb(t: jane_core::Tile) -> [u8; 3] {
    use jane_core::Tile as T;
    match t {
        T::Void => [0, 0, 0],
        T::Grass => [118, 164, 84],
        T::GrassTall => [92, 140, 66],
        T::Dirt => [150, 122, 84],
        T::Road => [96, 92, 90],
        T::Water => [52, 96, 170],
        T::Sand => [206, 190, 140],
        T::Bush => [60, 110, 50],
        T::Tree => [34, 84, 40],
        T::Fence => [120, 84, 52],
        T::HouseWall => [150, 110, 90],
        T::HouseRoof => [130, 60, 50],
        T::Floor | T::FloorWood => [170, 140, 100],
        T::Wall | T::WallTop | T::StoneWall => [110, 106, 100],
        T::Moss => [104, 140, 80],
        T::DryBed => [160, 148, 110],
        T::Garden => [120, 100, 60],
        T::Rubble => [120, 110, 100],
        T::Track => [70, 56, 44],
        T::GrownPath => [130, 140, 90],
        T::Cobble => [150, 146, 136],
        T::Rail => [40, 40, 44],
        T::Cliff => [96, 90, 84],
        T::Hedge => [46, 96, 46],
        T::Boardwalk => [176, 130, 70],
        T::DeadTree => [90, 80, 60],
        T::Glass => [170, 200, 220],
        T::FlowerBed => [200, 120, 160],
        T::Stepping => [140, 140, 150],
        T::Crops => [190, 170, 80],
        T::Ice => [210, 230, 240],
        _ => [255, 0, 255],
    }
}

/// A pine: a tree drawn darker and bluer.
const PINE_RGB: [u8; 3] = [24, 62, 44];

/// The county's cells, one pixel a cell, or the mean of `scale x scale` cells a pixel. Pine paint
/// is drawn over trees; marks are white dots, props yellow.
pub fn county_image(bp: &jane_core::Blueprint, scale: u32) -> Image {
    let (w, h) = (bp.w(), bp.h());
    let mut rgb: Vec<[u8; 3]> = bp.tiles.as_slice().iter().map(|&t| tile_rgb(t)).collect();
    for &(r, m) in &bp.paint {
        if m != jane_core::Material::Pine {
            continue;
        }
        for (x, y) in r.cells() {
            if bp.tiles.read(x, y, jane_core::Tile::Void) == jane_core::Tile::Tree {
                rgb[(y as u32 * w + x as u32) as usize] = PINE_RGB;
            }
        }
    }
    for p in &bp.props {
        rgb[(u32::from(p.cell.y) * w + u32::from(p.cell.x)) as usize] = [255, 220, 0];
    }
    let s = scale.max(1);
    let mut img = Image::new(w / s, h / s, [0, 0, 0, 255]);
    for py in 0..h / s {
        for px in 0..w / s {
            let mut sum = [0u32; 3];
            for y in py * s..py * s + s {
                for x in px * s..px * s + s {
                    let c = rgb[(y * w + x) as usize];
                    for k in 0..3 {
                        sum[k] += u32::from(c[k]);
                    }
                }
            }
            let n = s * s;
            img.set(px, py, [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]);
        }
    }
    for m in bp.marks.values() {
        let (x, y) = (i32::from(m.cell.x) / s as i32, i32::from(m.cell.y) / s as i32);
        dot(&mut img, (x, y), 1, [255, 255, 255, 255]);
    }
    img
}

/// `jane view --county [--seeds A..B | --seed N] [--scale S] [--out DIR] [--x X --y Y --size N --zoom Z]`:
/// the county's cells as a PNG per seed (`county-cells-<seed>.png`), one pixel a cell unless
/// `--scale` averages; `--x`, `--y`, `--size` crop a square of cells instead, `--zoom` pixels a
/// cell. Prints each stage's wall time.
fn run_county(args: &[String]) -> Result<(), String> {
    let range = seeds(args)?;
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1));
    let num = |k: &str, d: u32| get(k).map_or(Ok(d), |s| s.parse::<u32>().map_err(|_| format!("bad {k} {s}")));
    let out = get("--out").map_or("sheets", String::as_str);
    let scale = num("--scale", 1)?;
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
    for seed in range {
        let t0 = Instant::now();
        let sk = jane_world::county::county_skeleton(seed, 0).map_err(|e| format!("seed {seed}: {e}"))?;
        let mut line = format!("seed {seed}: skeleton {:.1} ms", ms(t0));
        let mut c = jane_world::county::County::new(&sk, 0);
        let mut total = 0.0;
        for (name, stage) in jane_world::county::STAGES {
            let t = Instant::now();
            stage(&mut c);
            let d = ms(t);
            total += d;
            if d >= 0.05 {
                let _ = write!(line, ", {name} {d:.1}");
            }
        }
        let bp = c.done();
        let _ = write!(
            line,
            "; county {total:.1} ms, {} paint rects, {} props, {} marks",
            bp.paint.len(),
            bp.props.len(),
            bp.marks.len()
        );
        println!("{line}");
        let img = if let Some(x) = get("--x") {
            let parse = |s: &String| s.parse::<i32>().map_err(|_| format!("bad number {s}"));
            let (x, y, size) = (parse(x)?, get("--y").map_or(Ok(0), parse)?, num("--size", 400)? as i32);
            crop(&county_image(&bp, 1), x, y, size, num("--zoom", 1)?.max(1))
        } else {
            county_image(&bp, scale)
        };
        let path = Path::new(out).join(format!("county-cells-{seed}.png"));
        std::fs::write(&path, img.png()).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// A `size x size` square of `img` from `(x, y)`, each pixel drawn `zoom` pixels square.
fn crop(img: &Image, x: i32, y: i32, size: i32, zoom: u32) -> Image {
    let mut out = Image::new(size as u32 * zoom, size as u32 * zoom, [0, 0, 0, 255]);
    for j in 0..size {
        for i in 0..size {
            let (sx, sy) = (x + i, y + j);
            if sx >= 0 && sy >= 0 && (sx as u32) < img.w && (sy as u32) < img.h {
                out.block(i as u32, j as u32, zoom, img.get(sx as u32, sy as u32));
            }
        }
    }
    out
}
