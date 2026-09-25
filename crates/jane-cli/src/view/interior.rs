//! `jane view --interior <house|cellar|arms|church|all> [--seeds A..B | --seed N] [--out DIR]`:
//! each seed's hand-built interior as a PNG (a colour per tile, props as their footprint, doors
//! and gates marked, units as diamonds, marks as crosses), and a line per build with its
//! attempts, what the solver said and its time.

use std::path::Path;
use std::time::Instant;

use jane_art::sheet::Image;
use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Tile, ZoneId};
use jane_data::catalog;
use jane_world::interiors::{build_interior, is_interior};
use jane_world::solve::{ZoneRules, validate};

const SCALE: u32 = 8;

fn tile_rgb(t: Tile) -> [u8; 3] {
    match t {
        Tile::Wall => [52, 46, 44],
        Tile::TempleWall => [70, 66, 80],
        Tile::Floor => [128, 120, 108],
        Tile::FloorWood => [168, 124, 80],
        Tile::TempleFloor => [176, 170, 156],
        Tile::Garden => [88, 120, 60],
        Tile::Void => [0, 0, 0],
        _ if t.flags() & F_SOLID != 0 => [90, 70, 90],
        _ => [255, 0, 255],
    }
}

/// One interior as an image.
pub fn interior_image(bp: &Blueprint) -> Image {
    let c = catalog();
    let mut img = Image::new(bp.w() * SCALE, bp.h() * SCALE, [0, 0, 0, 255]);
    for y in 0..bp.h() as i32 {
        for x in 0..bp.w() as i32 {
            let [r, g, b] = tile_rgb(bp.tiles.read(x, y, Tile::Void));
            img.block(x as u32, y as u32, SCALE, [r, g, b, 255]);
        }
    }
    for p in &bp.props {
        let def = c.story.prop(p.def);
        let colour = if p.to.is_some() {
            [60, 230, 250, 255]
        } else if p.locked {
            [230, 70, 50, 255]
        } else if !p.loot.is_empty() {
            [250, 210, 70, 255]
        } else if p.talk.is_some() {
            [150, 230, 120, 255]
        } else if def.id == "torch" {
            [255, 160, 40, 255]
        } else if def.solid {
            [96, 64, 40, 255]
        } else {
            [230, 230, 230, 255]
        };
        let (px, py) = (u32::from(p.cell.x) * SCALE, u32::from(p.cell.y) * SCALE);
        let (w, h) = (u32::from(def.w) * SCALE, u32::from(def.h) * SCALE);
        for dy in 1..h - 1 {
            for dx in 1..w - 1 {
                img.set(px + dx, py + dy, colour);
            }
        }
    }
    for u in &bp.units {
        let (cx, cy) = (i32::from(u.cell.x) * SCALE as i32 + 4, i32::from(u.cell.y) * SCALE as i32 + 4);
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                if dx.abs() + dy.abs() <= 3 {
                    img.set((cx + dx) as u32, (cy + dy) as u32, [240, 60, 200, 255]);
                }
            }
        }
    }
    for m in bp.marks.values() {
        let (cx, cy) = (i32::from(m.cell.x) * SCALE as i32 + 4, i32::from(m.cell.y) * SCALE as i32 + 4);
        for d in -4i32..=4 {
            img.set((cx + d) as u32, cy as u32, [255, 255, 255, 255]);
            img.set(cx as u32, (cy + d) as u32, [255, 255, 255, 255]);
        }
    }
    img
}

fn zones(which: &str) -> Result<Vec<ZoneId>, String> {
    let all: Vec<ZoneId> = ZoneId::ALL.into_iter().filter(|&z| is_interior(z)).collect();
    if which == "all" {
        return Ok(all);
    }
    ZoneId::from_name(which).filter(|z| all.contains(z)).map(|z| vec![z]).ok_or_else(|| {
        let ids: Vec<&str> = all.iter().map(|z| z.name()).collect();
        format!("no interior \"{which}\" (one of: all {})", ids.join(" "))
    })
}

/// `--interior`: draws each seed's interior and says how it went.
pub fn run(args: &[String], which: &str) -> Result<(), String> {
    let range = super::seeds(args)?;
    let out = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map_or("sheets", String::as_str);
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
    for zone in zones(which)? {
        let rules = ZoneRules::for_zone(zone);
        for seed in range.clone() {
            let t0 = Instant::now();
            let bp = build_interior(zone, seed).ok_or_else(|| format!("{} is not an interior", zone.name()))?;
            let us = t0.elapsed().as_micros();
            let report = validate(&bp, &rules);
            let path = Path::new(out).join(format!("interior-{}-{seed}.png", zone.name()));
            std::fs::write(&path, interior_image(&bp).png()).map_err(|e| format!("{}: {e}", path.display()))?;
            println!(
                "{}  attempts {}/{ZONE_ATTEMPTS}  {} props  {} units  {}  {}.{:03} ms",
                path.display(),
                bp.attempts,
                bp.props.len(),
                bp.units.len(),
                if report.ok() { "solved" } else { "REFUSED" },
                us / 1000,
                us % 1000
            );
            for line in report.lines(&bp) {
                println!("  {line}");
            }
        }
    }
    Ok(())
}
