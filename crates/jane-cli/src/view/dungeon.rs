//! `jane view --dungeon <id|all> [--seeds A..B | --seed N] [--out DIR] [--no-png]`: each seed's
//! generated dungeon as a PNG (a colour per tile family, props as dots, units as diamonds, marks
//! as crosses), and a line per build with its attempts after validation (the solver and checks
//! C1 to C13), whether it fell back, and its time; each refused attempt with its first reason.

use std::path::Path;
use std::time::Instant;

use jane_art::sheet::Image;
use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::tile::{F_BLOCK_LOS, F_SOLID, F_WATER};
use jane_core::{Blueprint, Key, Tile, ZoneId};
use jane_data::{MissionDef, catalog};
use jane_world::dungeon::{Built, build};

const SCALE: u32 = 4;

/// A unit's mark, around its cell's centre pixel.
const DIAMOND: [(i32, i32); 13] =
    [(0, -2), (-1, -1), (0, -1), (1, -1), (-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)];

fn tile_rgb(m: &MissionDef, t: Tile) -> [u8; 3] {
    if t == m.wall {
        return [40, 38, 44];
    }
    if t == m.floor {
        return [150, 140, 122];
    }
    match t {
        Tile::Sill => [196, 176, 110],
        Tile::Track => [120, 92, 60],
        Tile::Glass => [150, 210, 225],
        Tile::Rubble => [96, 84, 70],
        Tile::Void => [0, 0, 0],
        _ => {
            let f = t.flags();
            if f & F_WATER != 0 {
                [60, 100, 170]
            } else if f & F_SOLID != 0 && f & F_BLOCK_LOS != 0 {
                [70, 60, 76]
            } else if f & F_SOLID != 0 {
                [110, 130, 150]
            } else {
                // Another floor: a shade of its own, from its id.
                let h = u32::from(t.id()).wrapping_mul(2_654_435_761);
                [120 + (h >> 24) as u8 % 60, 120 + (h >> 16) as u8 % 60, 100 + (h >> 8) as u8 % 60]
            }
        }
    }
}

fn name_of(bp: &Blueprint, k: Key) -> String {
    match k {
        Key::Name(n) => catalog().name(n).to_owned(),
        Key::Local(i) => bp.local_names.get(i as usize).map(String::from).unwrap_or_default(),
    }
}

/// One dungeon blueprint as an image.
pub fn dungeon_image(built: &Built) -> Image {
    let bp = &built.blueprint;
    let m = built.info.mission;
    let c = catalog();
    let mut img = Image::new(bp.w() * SCALE, bp.h() * SCALE, [0, 0, 0, 255]);
    for y in 0..bp.h() as i32 {
        for x in 0..bp.w() as i32 {
            let [r, g, b] = tile_rgb(m, bp.tiles.read(x, y, Tile::Void));
            img.block(x as u32, y as u32, SCALE, [r, g, b, 255]);
        }
    }
    let lamp_rows = m.lights.map(|l| l.family).unwrap_or_default();
    let gate_rows: Vec<_> = m.edges.iter().filter_map(|e| e.gate).flat_map(|g| g.rows).collect();
    for p in &bp.props {
        let def = c.story.prop(p.def);
        let colour = if m.lights.is_some() && lamp_rows.contains(&p.def) {
            [250, 226, 90, 255]
        } else if gate_rows.contains(&p.def) || def.gate {
            [220, 60, 50, 255]
        } else if p.hidden {
            [170, 90, 220, 255]
        } else if p.locked {
            [240, 140, 40, 255]
        } else if def.push {
            [120, 200, 120, 255]
        } else {
            [245, 245, 245, 255]
        };
        for dy in 0..u32::from(def.h) {
            for dx in 0..u32::from(def.w) {
                let (cx, cy) = (u32::from(p.cell.x) + dx, u32::from(p.cell.y) + dy);
                for (ox, oy) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
                    img.set(cx * SCALE + ox, cy * SCALE + oy, colour);
                }
            }
        }
    }
    for u in &bp.units {
        let named = matches!(u.key, Key::Name(_)) && !name_of(bp, u.key).contains("_spawn_");
        let colour = if named { [240, 80, 220, 255] } else { [230, 40, 40, 255] };
        let (cx, cy) = (u32::from(u.cell.x) * SCALE + 2, u32::from(u.cell.y) * SCALE + 2);
        for (ox, oy) in DIAMOND {
            img.set(cx.wrapping_add_signed(ox), cy.wrapping_add_signed(oy), colour);
        }
    }
    for mk in bp.marks.values() {
        let (cx, cy) = (u32::from(mk.cell.x) * SCALE + 2, u32::from(mk.cell.y) * SCALE + 2);
        for d in -3i32..=3 {
            img.set(cx.wrapping_add_signed(d), cy, [60, 230, 250, 255]);
            img.set(cx, cy.wrapping_add_signed(d), [60, 230, 250, 255]);
        }
    }
    img
}

fn zones(which: &str) -> Result<Vec<ZoneId>, String> {
    let all: Vec<ZoneId> = catalog().dungeons.missions.iter().map(|m| m.zone).collect();
    if which == "all" {
        return Ok(all);
    }
    ZoneId::from_name(which).filter(|z| all.contains(z)).map(|z| vec![z]).ok_or_else(|| {
        let ids: Vec<&str> = all.iter().map(|z| z.name()).collect();
        format!("no generated dungeon \"{which}\" (one of: all {})", ids.join(" "))
    })
}

/// `--dungeon`: draws each seed's dungeon, and a summary line per dungeon.
pub fn run(args: &[String], which: &str) -> Result<(), String> {
    let range = super::seeds(args)?;
    let out = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map_or("sheets", String::as_str);
    let draw = !args.iter().any(|a| a == "--no-png");
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
    for zone in zones(which)? {
        let (mut n, mut total_us, mut worst_us, mut worst_attempts, mut fallbacks) = (0u32, 0u128, 0u128, 0u8, 0u32);
        let mut attempts_sum = 0u32;
        for seed in range.clone() {
            let t0 = Instant::now();
            let built = build(zone, seed);
            let us = t0.elapsed().as_micros();
            let bp = &built.blueprint;
            let fallback = built.info.layout.as_ref().is_some_and(|l| l.fallback);
            n += 1;
            total_us += us;
            worst_us = worst_us.max(us);
            worst_attempts = worst_attempts.max(bp.attempts);
            attempts_sum += u32::from(bp.attempts);
            fallbacks += u32::from(fallback);
            if draw {
                let path = Path::new(out).join(format!("dungeon-{}-{seed}.png", zone.name()));
                std::fs::write(&path, dungeon_image(&built).png()).map_err(|e| format!("{}: {e}", path.display()))?;
                println!(
                    "{}  attempts {}{}  {} rooms  {} props  {} units  {}.{:03} ms",
                    path.display(),
                    bp.attempts,
                    if fallback { " (fallback)" } else { "" },
                    built.info.rooms.len(),
                    bp.props.len(),
                    bp.units.len(),
                    us / 1000,
                    us % 1000
                );
            }
            for (attempt, why) in &built.info.rejected {
                println!("  seed {seed}: attempt {} refused: {why}", attempt + 1);
            }
            for e in &built.info.errors {
                println!("  seed {seed}: {e}");
            }
        }
        if n > 0 {
            let mean = total_us / u128::from(n);
            // Mean attempts in hundredths, as integers.
            let att = attempts_sum * 100 / n;
            println!(
                "{}: {n} seeds, mean {}.{:03} ms, worst {}.{:03} ms, attempts mean {}.{:02} worst {worst_attempts}/{ZONE_ATTEMPTS}, fallback {fallbacks}",
                zone.name(),
                mean / 1000,
                mean % 1000,
                worst_us / 1000,
                worst_us % 1000,
                att / 100,
                att % 100
            );
        }
    }
    Ok(())
}
