//! The owner's playtest: "a rat spawned inside the chest in the basement so it couldn't run
//! anywhere". No unit of any built zone starts in terrain or in a solid prop's cells, and every
//! one has an open cell beside it to move to.

use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, ZoneId};
use jane_data::catalog;
use jane_world::build_zone;

/// Is `(x, y)` shut to a body at the start: terrain, the edge, or a solid prop standing there
/// (a gate while it is locked; a hidden prop too, since it may show).
fn shut(bp: &Blueprint, x: i32, y: i32) -> bool {
    if !bp.tiles.inside(x, y) || bp.tiles.read(x, y, jane_core::Tile::Void).flags() & F_SOLID != 0 {
        return true;
    }
    bp.props.iter().any(|p| {
        let def = catalog().story.prop(p.def);
        def.solid && (!def.gate || p.locked) && def.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).contains(x, y)
    })
}

fn caught(bp: &Blueprint, out: &mut Vec<String>, zone: ZoneId, seed: u32) {
    for u in &bp.units {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        let id = catalog().combat.unit(u.def).id;
        if shut(bp, x, y) {
            out.push(format!("{zone:?} seed {seed}: {id} at ({x}, {y}) stands in something solid"));
        } else if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dy)| shut(bp, x + dx, y + dy)) {
            out.push(format!("{zone:?} seed {seed}: {id} at ({x}, {y}) is boxed in"));
        }
    }
}

#[test]
fn no_unit_starts_in_a_solid_prop_or_boxed_in_on_seeds_1_to_8() {
    let mut out = Vec::new();
    let mut n = 0;
    for zone in ZoneId::ALL.iter().copied().filter(|&z| z != ZoneId::County) {
        for seed in 1..=8 {
            let bp = build_zone(zone, seed).unwrap_or_else(|| panic!("{zone:?} builds on seed {seed}"));
            n += bp.units.len();
            caught(&bp, &mut out, zone, seed);
        }
    }
    assert!(n > 500, "the zones have their units ({n})");
    // The cellar's rats on the seeds the rat room's chest once stood on one (18, 93, 135).
    for seed in [18, 93, 135, 199, 200] {
        caught(&build_zone(ZoneId::Cellar, seed).expect("the cellar builds"), &mut out, ZoneId::Cellar, seed);
    }
    assert!(out.is_empty(), "units started where they cannot move: {out:#?}");
}

/// The county's own, on a few seeds (it is the slowest build).
#[test]
fn no_county_unit_starts_in_a_solid_prop_or_boxed_in() {
    let mut out = Vec::new();
    for seed in 1..=3 {
        let bp = build_zone(ZoneId::County, seed).expect("the county builds");
        caught(&bp, &mut out, ZoneId::County, seed);
    }
    assert!(out.is_empty(), "units started where they cannot move: {out:#?}");
}
