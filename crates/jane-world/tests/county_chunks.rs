//! PORT.md §6.m stage 6: the set places stamped, the link lanes, the doors. Over `SEEDS` seeds (64
//! by default), every seed's county built once on every core and every check run on it:
//!
//! - every name a chunk exports (its marks, rects, keyed props and units) is in the county, and so
//!   is every mark and rect of the county's contract a chunk provides;
//! - every gate connects: a flood from `start` over open ground reaches every gate of every chunk;
//! - no link lane cuts into a box: every cell of a box is the chunk's own, but where the railway
//!   runs, and the halt keeps its platform rails;
//! - a fill's props lie whole inside its cells;
//! - every door row stands, leads where its row says, and the mark it leads to is one its zone
//!   provides; a door's return mark is in the county;
//! - the same seed builds the same county.

mod common;

use std::sync::OnceLock;

use jane_core::search::{Conn, Reach, flood};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Grid, Key, NameId, Tile, ZoneId};
use jane_data::{DoorAt, MissionNameWhat, NameKind};
use jane_world::county::rail::{BEND, rail_line};
use jane_world::county::{County, STAGES, build_county, county_skeleton};
use jane_world::skeleton::Skeleton;

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

type Check = fn(&Skeleton, &County<'_>, &mut Vec<String>);

const CHECKS: &[(&str, Check)] = &[
    ("exports", exports_are_there),
    ("gates", gates_connect),
    ("boxes", boxes_are_whole),
    ("fills", fills_lie_in_their_cells),
    ("doors", doors_stand),
];

/// What each check complained of, by check, in seed order.
fn verdicts() -> &'static [Vec<String>] {
    static ALL: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    ALL.get_or_init(|| {
        let seeds = seed_list();
        let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let per = seeds.len().div_ceil(threads).max(1);
        let parts: Vec<Vec<Vec<String>>> = std::thread::scope(|s| {
            let jobs: Vec<_> = seeds
                .chunks(per)
                .map(|part| {
                    s.spawn(move || {
                        let mut bad = vec![Vec::new(); CHECKS.len()];
                        for &seed in part {
                            let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
                            let mut c = County::new(&sk, 0);
                            for (_, stage) in STAGES {
                                stage(&mut c);
                            }
                            for (i, (_, check)) in CHECKS.iter().enumerate() {
                                check(&sk, &c, &mut bad[i]);
                            }
                        }
                        bad
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().expect("a county builds")).collect()
        });
        (0..CHECKS.len()).map(|i| parts.iter().flat_map(|p| p[i].iter().cloned()).collect()).collect()
    })
}

fn verdict(name: &str) {
    let i = CHECKS.iter().position(|(n, _)| *n == name).expect("a check of that name");
    let bad = &verdicts()[i];
    assert!(bad.is_empty(), "{} failures, the first: {:#?}", bad.len(), &bad[..bad.len().min(8)]);
}

fn has(bp: &Blueprint, kind: NameKind, n: NameId) -> bool {
    let k = Key::Name(n);
    match kind {
        NameKind::Mark => bp.marks.contains_key(&k),
        NameKind::Rect => bp.rects.contains_key(&k),
        NameKind::Prop => bp.props.iter().any(|p| p.key == k),
        NameKind::Unit => bp.units.iter().any(|u| u.key == k),
        NameKind::Area => true,
    }
}

#[test]
fn every_name_a_chunk_exports_is_in_the_county() {
    verdict("exports");
}

fn exports_are_there(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    if c.chunks.len() != cat.chunks.defs.len() {
        bad.push(format!("seed {}: {} of {} chunks stamped", sk.seed, c.chunks.len(), cat.chunks.defs.len()));
    }
    for (kind, n) in cat.chunks.exports() {
        if !has(bp, kind, n) {
            bad.push(format!("seed {}: no {kind:?} {}", sk.seed, cat.name(n)));
        }
    }
    // The contract's marks and rects that some chunk provides.
    let county = cat.county.zone(ZoneId::County);
    for (kind, n) in county.contract.names() {
        let provided = cat.chunks.exports().any(|e| e == (kind, n));
        if provided && matches!(kind, NameKind::Mark | NameKind::Rect) && !has(bp, kind, n) {
            bad.push(format!("seed {}: the contract's {kind:?} {} is missing", sk.seed, cat.name(n)));
        }
    }
    for ch in &c.chunks {
        let key = c.k.blueprint().local_names.iter().position(|s| *s == format!("site_{}", ch.id()));
        if key.is_none_or(|i| bp.rects.get(&Key::Local(i as u32)) != Some(&ch.bounds)) {
            bad.push(format!("seed {}: no rect site_{}", sk.seed, ch.id()));
        }
    }
}

/// Cells nobody can stand on: solid ground, water, and every solid prop's footprint.
fn blocked(bp: &Blueprint) -> Grid<bool> {
    let cat = jane_data::catalog();
    let mut g = Grid::new(bp.w(), bp.h(), false);
    for (i, t) in bp.tiles.as_slice().iter().enumerate() {
        g.as_mut_slice()[i] = t.flags() & F_SOLID != 0;
    }
    for p in &bp.props {
        let row = cat.story.prop(p.def);
        if row.solid {
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            g.fill_rect(jane_core::Rect::new(x, y, i32::from(row.w), i32::from(row.h)), true);
        }
    }
    g
}

#[test]
fn a_flood_from_the_start_reaches_every_gate() {
    verdict("gates");
}

fn gates_connect(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let start = cat.name_id("start").expect("a start name");
    let Some(m) = bp.marks.get(&Key::Name(start)) else {
        bad.push(format!("seed {}: no start mark", sk.seed));
        return;
    };
    let solid = blocked(bp);
    let mut reach = Reach::new();
    let from = [(i32::from(m.cell.x), i32::from(m.cell.y))];
    flood(bp.w(), bp.h(), &from, Conn::Four, u32::MAX, |x, y| !solid.read(x, y, true), &mut reach);
    for ch in &c.chunks {
        for &(x, y) in &ch.gates {
            if reach.dist(x, y) == jane_core::search::UNREACHED {
                bad.push(format!(
                    "seed {}: {}'s gate at ({x}, {y}) is not reached ({:?})",
                    sk.seed,
                    ch.id(),
                    c.k.get(x, y)
                ));
            }
        }
    }
}

#[test]
fn no_lane_cuts_into_a_box_and_the_halt_keeps_its_rails() {
    verdict("boxes");
}

fn boxes_are_whole(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let line = rail_line(sk, BEND);
    let mut rail: Vec<(i32, i32)> = line.clone();
    rail.sort();
    let near_rail =
        |x: i32, y: i32| (-2..=2).any(|oy| (-2..=2).any(|ox| rail.binary_search(&(x + ox, y + oy)).is_ok()));
    for ch in &c.chunks {
        let b = ch.bounds;
        let mut cut = 0;
        for y in 0..ch.def.h {
            for x in 0..ch.def.w {
                let (cx, cy) = (b.x + i32::from(x), b.y + i32::from(y));
                let (want, got) = (jane_world::county::chunks::stamped_tile(ch.def, x, y), c.k.get(cx, cy));
                if want == Tile::Rail && got != Tile::Rail {
                    bad.push(format!("seed {}: {} lost its rail at ({cx}, {cy}): {got:?}", sk.seed, ch.id()));
                }
                // A front garden's fence (the `gardens` stage) is the place's own: laid only on
                // its garden ground.
                let garden_fence = got == Tile::Fence && jane_core::garden::plot_ground(want);
                if want != got && !near_rail(cx, cy) && !garden_fence {
                    cut += 1;
                }
            }
        }
        if cut > 0 {
            bad.push(format!("seed {}: {} cells of {} are not its own", sk.seed, cut, ch.id()));
        }
    }
}

#[test]
fn a_fill_lies_in_its_cells() {
    verdict("fills");
}

fn fills_lie_in_their_cells(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    for ch in &c.chunks {
        for f in ch.def.fills {
            let row = cat.chunks.fill(f);
            let size = cat.story.prop(row.def);
            let inside = |x: i32, y: i32| f.cells.iter().any(|r| r.contains(x - ch.bounds.x, y - ch.bounds.y));
            let placed: Vec<_> = bp
                .props
                .iter()
                .filter(|p| p.def == row.def && ch.bounds.contains(i32::from(p.cell.x), i32::from(p.cell.y)))
                .collect();
            if placed.is_empty() || placed.len() > usize::from(row.count) {
                bad.push(format!("seed {}: {} of {} placed {}", sk.seed, row.id, ch.id(), placed.len()));
            }
            for p in placed {
                let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
                let whole = (0..i32::from(size.h)).all(|j| (0..i32::from(size.w)).all(|i| inside(x + i, y + j)));
                if !whole {
                    bad.push(format!("seed {}: a {} at ({x}, {y}) is outside its fill", sk.seed, row.id));
                }
            }
        }
    }
}

/// Does `zone` provide `mark`: its `zones.json` contract, or its mission's names.
fn zone_has_mark(zone: ZoneId, mark: NameId) -> bool {
    let cat = jane_data::catalog();
    cat.county.zone(zone).contract.marks.contains(&mark)
        || cat
            .dungeons
            .mission_of(zone)
            .is_some_and(|m| m.provides.iter().any(|n| n.name == mark && n.what == MissionNameWhat::Mark))
}

#[test]
fn every_door_stands_and_leads_to_a_mark_its_zone_provides() {
    verdict("doors");
}

fn doors_stand(sk: &Skeleton, c: &County<'_>, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let s = sk.seed;
    for d in cat.county.doors {
        let name = cat.name(d.key);
        let Some(p) = bp.props.iter().find(|p| p.key == Key::Name(d.key)) else {
            bad.push(format!("seed {s}: no door {name}"));
            continue;
        };
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        if p.def != d.def || p.locked != (d.key_tag.is_some() && !d.keyed) {
            bad.push(format!("seed {s}: {name} is not as its row says"));
        }
        match (d.to, p.to) {
            (None, None) => {}
            (Some(m), Some(to)) if to.zone == d.zone && to.mark == Key::Name(m) => {
                if !zone_has_mark(d.zone, m) {
                    bad.push(format!(
                        "seed {s}: {name} leads to {} in {:?}, which has no such mark",
                        cat.name(m),
                        d.zone
                    ));
                }
            }
            _ => bad.push(format!("seed {s}: {name} leads to {:?}, its row to {:?}", p.to, d.to)),
        }
        if let Some(m) = d.mark {
            let h = i32::from(cat.story.prop(d.def).h);
            let at = bp.marks.get(&Key::Name(m)).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)));
            if at != Some((x, y + h)) {
                bad.push(format!("seed {s}: {name}'s mark {} is at {at:?}", cat.name(m)));
            }
        }
        let site = match d.at {
            DoorAt::Chunk(i) | DoorAt::Near(i) => i,
        };
        let ch = c.chunks.iter().find(|ch| ch.site == site).expect("every site has its chunk");
        let ok = match d.at {
            DoorAt::Chunk(_) => ch.slot_named(&format!("{}_door", ch.id())) == Some((x, y)),
            DoorAt::Near(_) => ch.bounds.grow(14).contains(x, y),
        };
        if !ok {
            bad.push(format!("seed {s}: {name} at ({x}, {y}) is not where its row puts it"));
        }
    }
}

#[test]
fn chunks_and_doors_are_the_same_for_the_same_seed() {
    for seed in seed_list().into_iter().take(common::seeds().min(4) as usize) {
        let a = build_county(seed, 0).expect("builds");
        let b = build_county(seed, 0).expect("builds");
        assert!(a == b, "seed {seed}: two builds differ");
    }
}
