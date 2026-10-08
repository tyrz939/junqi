//! `jane sheet terrain`, `jane sheet flora` and `jane sheet county <seed> --full` (ART.md §5,
//! PRESENTATION.md §6): the chunk painter's sheets, and the paint time per chunk on this machine.

use std::path::Path;
use std::time::Instant;

use jane_art::Font;
use jane_art::sheet::Image;
use jane_art::terrain::{self, CHUNK_CELLS, Chunk, Painter, TileMap};
use jane_core::grid::Rect;

pub const USAGE: &str = "  sheet terrain [--tile NAME | --sample [--zoom Z]] [--out DIR]
                                      every tile in its sixteen neighbour contexts, dry over wet, one sheet a group;
                                      --sample: the made-up county and interior with every tile, dry, by day, by night
  sheet flora [--out DIR]             every tree, shrub and stone the chunk painter stamps, and their layers
  sheet county <seed> --full [--at MARK|X,Y] [--radius CELLS] [--zoom Z] [--out DIR]
                                      the chunk painter over a window of a built county: its four layers side by
                                      side, its albedo, and the window lit at five in the afternoon and at night;
                                      prints the paint time per chunk";

fn write(out: &Path, name: &str, img: &Image) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join(format!("{name}.png"));
    std::fs::write(&path, img.png()).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

fn get<'a>(args: &'a [String], k: &str) -> Option<&'a String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1))
}

/// `sheet --bless`, the terrain's half: rewrites `crates/jane-art/tests/terrain_golden.txt`.
pub fn bless() -> Result<(), String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../jane-art/tests/terrain_golden.txt");
    std::fs::write(&path, terrain::sheet::golden_file(&mut Painter::new()))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("blessed {}", path.display());
    Ok(())
}

pub fn terrain(args: &[String], out: &Path, font: &Font) -> Result<(), String> {
    let mut p = Painter::new();
    if args.iter().any(|a| a == "--sample") {
        let zoom: u32 = get(args, "--zoom").map_or(Ok(1), |s| s.parse().map_err(|_| format!("bad --zoom {s}")))?;
        for (name, img) in terrain::sheet::samples(&mut p) {
            write(out, &name, &scale(&img, zoom))?;
        }
        return Ok(());
    }
    let sheets = terrain::sheet::terrain(&mut p, font, get(args, "--tile").map(String::as_str));
    if sheets.is_empty() {
        return Err("no tile by that name; the names are data/looks/tiles.json's keys".into());
    }
    for (name, img) in sheets {
        write(out, &name, &img)?;
    }
    Ok(())
}

pub fn flora(out: &Path, font: &Font) -> Result<(), String> {
    let p = Painter::new();
    write(out, "flora", &terrain::sheet::flora(p.bank(), font))
}

/// The cell a mark names, by its name in the catalog or the blueprint.
fn mark(bp: &jane_core::Blueprint, name: &str) -> Option<(i32, i32)> {
    let cat = jane_data::catalog();
    bp.marks.iter().find_map(|(k, m)| {
        let n = match k {
            jane_core::ids::Key::Name(n) => cat.name(*n),
            jane_core::ids::Key::Local(i) => bp.local_names.get(*i as usize).unwrap_or(""),
        };
        (n == name).then(|| (i32::from(m.cell.x), i32::from(m.cell.y)))
    })
}

pub fn county(args: &[String], out: &Path, font: &Font) -> Result<(), String> {
    if !args.iter().any(|a| a == "--full") {
        return Err("sheet county draws the chunk painter with --full; the skeleton card is `jane view`".into());
    }
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).ok_or("sheet county <seed> --full")?;
    let radius: i32 = get(args, "--radius").map_or(Ok(24), |s| s.parse().map_err(|_| format!("bad --radius {s}")))?;
    let t0 = Instant::now();
    let sk = jane_world::county::county_skeleton(seed, 0).map_err(|e| format!("seed {seed}: {e}"))?;
    let mut c = jane_world::county::County::new(&sk, 0);
    for (_, stage) in jane_world::county::STAGES {
        stage(&mut c);
    }
    let bp = c.done();
    println!("county {seed} built in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let at = get(args, "--at").map_or("town_square", String::as_str);
    let (cx, cy) = match mark(&bp, at) {
        Some(c) => c,
        None if at.contains(',') => {
            let (a, b) = at.split_once(',').unwrap_or(("0", "0"));
            (
                a.trim().parse().map_err(|_| format!("bad --at {at}"))?,
                b.trim().parse().map_err(|_| format!("bad --at {at}"))?,
            )
        }
        None => {
            let cat = jane_data::catalog();
            let mut names: Vec<String> = bp
                .marks
                .keys()
                .map(|k| match k {
                    jane_core::ids::Key::Name(n) => cat.name(*n).to_string(),
                    jane_core::ids::Key::Local(i) => {
                        bp.local_names.get(*i as usize).map(String::from).unwrap_or_default()
                    }
                })
                .collect();
            names.sort();
            names.dedup();
            return Err(format!("no mark \"{at}\" (or give X,Y); marks: {}", names.join(", ")));
        }
    };
    let cells = Rect::new(cx - radius, cy - radius, 2 * radius, 2 * radius);
    let mut p = Painter::new();
    // The paint time per chunk over the window, measured apart from the composing.
    let map = TileMap::from_blueprint(&bp);
    let mut chunk = Chunk::new();
    let (x0, y0) = (cells.x.div_euclid(CHUNK_CELLS), cells.y.div_euclid(CHUNK_CELLS));
    let (x1, y1) = ((cells.right() - 1).div_euclid(CHUNK_CELLS), (cells.bottom() - 1).div_euclid(CHUNK_CELLS));
    let mut times: Vec<f64> = Vec::new();
    let mut strip_kb = 0;
    for _ in 0..2 {
        times.clear();
        for ky in y0..=y1 {
            for kx in x0..=x1 {
                let t = Instant::now();
                terrain::paint_chunk(&mut p, &map, seed, kx, ky, &mut chunk);
                times.push(t.elapsed().as_secs_f64() * 1000.0);
                let bytes: usize = chunk.strips().iter().map(|s| usize::from(s.w) * usize::from(s.h) * 8).sum();
                strip_kb = strip_kb.max(bytes / 1024);
            }
        }
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / f64::from(u32::try_from(times.len().max(1)).unwrap_or(1));
    println!(
        "paint: {} chunks, {:.2} ms a chunk mean, {:.2} median, {:.2} max; strips up to {strip_kb} KB a chunk",
        times.len(),
        mean,
        times[times.len() / 2],
        times.last().copied().unwrap_or(0.0)
    );
    let sheets = terrain::sheet::county(&mut p, &bp, seed, cells, font);
    let stem = format!("county-{seed}-{at}").replace(',', "_");
    let zoom: u32 = get(args, "--zoom").map_or(Ok(1), |s| s.parse().map_err(|_| format!("bad --zoom {s}")))?;
    write(out, &format!("{stem}-layers"), &sheets.layers)?;
    write(out, &format!("{stem}-albedo"), &scale(&sheets.albedo, zoom))?;
    write(out, &format!("{stem}-day"), &scale(&sheets.day, zoom))?;
    write(out, &format!("{stem}-night"), &scale(&sheets.night, zoom))?;
    Ok(())
}

/// `img` with each pixel drawn `z` pixels square.
fn scale(img: &Image, z: u32) -> Image {
    if z <= 1 {
        return img.clone();
    }
    let mut out = Image::new(img.w * z, img.h * z, [0, 0, 0, 255]);
    for y in 0..img.h {
        for x in 0..img.w {
            out.block(x, y, z, img.get(x, y));
        }
    }
    out
}
