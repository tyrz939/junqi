//! `jane sheet terrain`, `jane sheet flora` and `jane sheet county <seed> --full` (ART.md Â§5,
//! PRESENTATION.md Â§6): the chunk painter's sheets, and the paint time per chunk on this machine.

use std::path::Path;
use std::time::Instant;

use jane_art::Font;
use jane_art::sheet::{self, Image};
use jane_art::terrain::{self, CHUNK_CELLS, Chunk, Painter, TileMap};
use jane_core::grid::Rect;

pub const USAGE: &str = "  sheet terrain [--tile NAME | --sample [--zoom Z]] [--out DIR]
                                      every tile in its sixteen neighbour contexts, dry over wet, one sheet a group;
                                      --sample: the made-up county and interior with every tile, dry, by day, by night
  sheet flora [--out DIR]             every tree, shrub and stone the chunk painter stamps, and their layers
  sheet county <seed> --full [--at MARK|X,Y] [--radius CELLS] [--zoom Z] [--out DIR]
                                      the chunk painter over a window of a built county: its four layers side by
                                      side, its albedo, and the window lit at five in the afternoon and at night;
                                      prints the paint time per chunk
  sheet heights [--zoom Z] [--out DIR] the dev grounds (terraces, viaduct) painted whole: layers, albedo, day, night";

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

/// `jane sheet heights [--zoom Z]` (MAP.md Â§9 R3): the chunk painter over each dev ground whole
/// (`jane_sim::dev_ground`: the terraces and the viaduct), its layers, its albedo, and lit at five
/// in the afternoon and at night, through the art crate's light pass alone (no presenter).
pub fn heights(args: &[String], out: &Path, font: &Font) -> Result<(), String> {
    let zoom: u32 = get(args, "--zoom").map_or(Ok(1), |s| s.parse().map_err(|_| format!("bad --zoom {s}")))?;
    for g in [jane_sim::dev_ground::Ground::Terraces, jane_sim::dev_ground::Ground::Viaduct] {
        let bp = jane_sim::dev_ground::blueprint(g, jane_core::ZoneId::County);
        let cells = Rect::new(0, 0, bp.w() as i32, bp.h() as i32);
        let mut p = Painter::new();
        let t = Instant::now();
        let sheets = terrain::sheet::county(&mut p, &bp, 1, cells, font);
        println!("{}: painted in {:.0} ms", g.name(), t.elapsed().as_secs_f64() * 1000.0);
        let stem = format!("heights-{}", g.name());
        write(out, &format!("{stem}-layers"), &sheets.layers)?;
        write(out, &format!("{stem}-albedo"), &scale(&sheets.albedo, zoom))?;
        write(out, &format!("{stem}-day"), &scale(&sheets.day, zoom))?;
        write(out, &format!("{stem}-night"), &scale(&sheets.night, zoom))?;
    }
    write(out, "heights-looks", &looks(font))?;
    write(out, "heights-decks", &decks(font))?;
    Ok(())
}

/// One region's height in small: a plateau's face (rows 6 and 7) over the field, a ledge in it,
/// a stair, a ladder, a ramp's road, a waterfall from a stream into a pool, and a rise in the
/// field with its rims and its own face; every cell of region `region`.
fn sample(region: u8) -> jane_core::Blueprint {
    use jane_core::{Plane, Tile};
    let (w, h) = (44u32, 16u32);
    let mut bp = jane_core::Blueprint::new(jane_core::ZoneId::County, w, h, Tile::Grass);
    let mut lv = vec![1u8; (w * h) as usize];
    let mut level = |r: Rect, l: u8| {
        for (x, y) in r.cells() {
            lv[(y * w as i32 + x) as usize] = l;
        }
    };
    level(Rect::new(0, 0, w as i32, 8), 2);
    let fill = |bp: &mut jane_core::Blueprint, r: Rect, t: Tile| bp.tiles.fill_rect(r, t);
    fill(&mut bp, Rect::new(0, 6, w as i32, 2), Tile::Cliff);
    fill(&mut bp, Rect::new(3, 6, 6, 2), Tile::LedgeS);
    fill(&mut bp, Rect::new(11, 6, 4, 2), Tile::Stair);
    level(Rect::new(11, 7, 4, 1), 1);
    fill(&mut bp, Rect::new(18, 6, 1, 2), Tile::Ladder);
    level(Rect::new(18, 7, 1, 1), 1);
    fill(&mut bp, Rect::new(21, 2, 5, 10), Tile::Road);
    level(Rect::new(21, 7, 5, 1), 1);
    fill(&mut bp, Rect::new(29, 0, 3, 6), Tile::Water);
    fill(&mut bp, Rect::new(29, 6, 3, 2), Tile::Waterfall);
    fill(&mut bp, Rect::new(27, 8, 7, 3), Tile::Water);
    let rise = Rect::new(36, 9, 7, 6);
    level(rise, 2);
    fill(&mut bp, Rect::new(rise.x, rise.y, rise.w, 1), Tile::Cliff);
    fill(&mut bp, Rect::new(rise.x, rise.y, 1, rise.h), Tile::Cliff);
    fill(&mut bp, Rect::new(rise.right() - 1, rise.y, 1, rise.h), Tile::Cliff);
    fill(&mut bp, Rect::new(rise.x, rise.bottom() - 2, rise.w, 2), Tile::Cliff);
    fill(&mut bp, Rect::new(rise.right() - 1, rise.y + 1, 1, 2), Tile::LedgeE);
    for (x, y) in [(2, 2), (6, 3), (14, 1), (34, 2), (40, 4), (8, 12), (16, 13)] {
        fill(&mut bp, Rect::new(x, y, 1, 1), if (x + y) % 3 == 0 { Tile::Tree } else { Tile::Bush });
    }
    bp.level = Some(Plane::pack(w, h, &lv));
    bp.regions = jane_core::blueprint::RegionMap::new(16, 3, 1, region);
    bp
}

/// `heights-looks`: each region's sample (`sample`) painted, its albedo over its light at five in
/// the afternoon, at 1x and its face at 3x: the faces, rims, lips, ledges, stairs, ladders, ramps
/// and waterfalls of MAP.md Â§6.1.
fn looks(font: &Font) -> Image {
    let names = ["Lowfields", "Waters", "Works"];
    let (w, h) = (44 * 16, 16 * 16);
    let rows: Vec<(Image, Image)> = (0..3u8)
        .map(|r| {
            let bp = sample(r);
            let mut p = Painter::new();
            let s = terrain::sheet::county(&mut p, &bp, 3, Rect::new(0, 0, 44, 16), font);
            (s.albedo, s.day)
        })
        .collect();
    let zoom = 3u32;
    let close = (w as u32 / 2, 64u32);
    let mut img = Image::new(w as u32 * 2 + 24, 3 * (h as u32 + close.1 * zoom + 40) + 10, [34, 32, 40, 255]);
    for (i, (albedo, day)) in rows.iter().enumerate() {
        let y0 = 10 + i as u32 * (h as u32 + close.1 * zoom + 40);
        sheet::label(&mut img, font, 8, y0, names[i], jane_art::Face::Small, [230, 226, 214]);
        for (k, src) in [albedo, day].iter().enumerate() {
            let x0 = 8 + k as u32 * (w as u32 + 8);
            for y in 0..h as u32 {
                for x in 0..w as u32 {
                    img.set(x0 + x, y0 + 14 + y, src.get(x, y));
                }
            }
        }
        // The face from the ledge to the waterfall at 3x.
        let (cx, cy) = (40u32, 80u32);
        for y in 0..close.1 * zoom {
            for x in 0..close.0 * zoom {
                img.set(8 + x, y0 + 20 + h as u32 + y, albedo.get(cx + x / zoom, cy + y / zoom));
            }
        }
    }
    img
}

/// `heights-decks`: every piece of every kind of deck (`jane_art::deck`) at 3x, a kind a row, on
/// the towpath's sand.
fn decks(font: &Font) -> Image {
    use jane_art::deck::{Kind, Piece, render};
    let zoom = 3u32;
    let cell = 30 * zoom;
    let mut img =
        Image::new(90 + Piece::ALL.len() as u32 * cell, Kind::ALL.len() as u32 * (52 * zoom) + 20, [34, 32, 40, 255]);
    for (r, &k) in Kind::ALL.iter().enumerate() {
        let y0 = 10 + r as u32 * 52 * zoom;
        sheet::label(&mut img, font, 6, y0 + 20, k.name(), jane_art::Face::Small, [230, 226, 214]);
        for (i, &piece) in Piece::ALL.iter().enumerate() {
            let c = render(k, piece);
            let x0 = 90 + i as u32 * cell;
            img.fill(x0, y0, c.w() as u32 * zoom + 4, c.h() as u32 * zoom + 4, [170, 150, 112]);
            sheet::put_albedo(&mut img, &c, x0 + 2, y0 + 2, zoom);
        }
    }
    img
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
