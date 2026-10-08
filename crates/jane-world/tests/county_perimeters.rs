//! Every named patch has an edge (`county::perimeter`): over seeds 1 to 16, each placed patch's
//! hedge, wall, reed edge or slag bank stands along most of its ring, with gaps; nothing it was
//! laid across was a way (every footpath, lane and road cell inside the patch's box is still
//! walkable at the end); and the ground just inside the edge is reached from the platform. The
//! table prints with `-- --nocapture`.

use jane_core::search::{Fill, fill};
use jane_core::tile::F_SOLID;
use jane_core::{Key, Tile};
use jane_world::county::finish::blocked_by_props;
use jane_world::county::perimeter::Perimeter;
use jane_world::county::{County, STAGES, county_skeleton};

/// Sectors the ring is judged in: a few cells of edge each.
const SECTORS: usize = 256;
/// The share of sectors, in percent, every edge stands in: roads, places, woods, water, crag and
/// its gateways may take the rest (a patch on the bank, or one a road runs through, keeps less).
const COVER_MIN: usize = 15;
/// The share every seed's edges stand in on average.
const COVER_MEAN: usize = 50;

struct Row {
    name: String,
    p: Perimeter,
    inner: u32,
    reached: u32,
    gaps: usize,
    cover: usize,
}

struct Seen {
    seed: u32,
    rows: Vec<Row>,
    bad: Vec<String>,
}

fn check(seed: u32) -> Seen {
    let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
    let mut c = County::new(&sk, 0);
    for (_, stage) in STAGES {
        stage(&mut c);
    }
    let cat = jane_data::catalog();
    let k = &c.k;
    let (w, h) = (k.w(), k.h());
    let blocked = blocked_by_props(k);
    let start = cat
        .name_id("start")
        .and_then(|s| k.blueprint().marks.get(&Key::Name(s)))
        .map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)))
        .expect("the county has a start");
    let with_tiles = k.blueprint_with_tiles();
    let tiles = with_tiles.tiles.as_slice();
    let mut reach = Fill::new();
    fill(w as u32, h as u32, &[start], |i| tiles[i].flags() & F_SOLID == 0 && !blocked[i], &mut reach);
    let mut out = Seen { seed, rows: Vec::new(), bad: Vec::new() };
    assert_eq!(c.perimeters.len(), sk.areas.len(), "one edge a patch");
    for (p, a) in c.perimeters.iter().zip(&sk.areas) {
        let name = cat.name(a.def.id).to_string();
        if p.gave_up {
            out.bad.push(format!("{name}: shut something in and was taken up"));
        }
        // Walk the core's cells by angle: cover is the turns of the ring it stands in, and a gap is
        // two sectors running with none.
        let mut sectors = [false; SECTORS];
        for &(x, y) in &p.core {
            let (dx, dy) = (x - p.centre.0, y - p.centre.1);
            sectors[sector(dx, dy)] = true;
        }
        let cover = sectors.iter().filter(|&&s| s).count() * 100 / SECTORS;
        if cover < COVER_MIN {
            out.bad.push(format!("{name}: stands round {cover}% of its ring"));
        }
        let gaps =
            (0..SECTORS).filter(|&i| sectors[i] && !sectors[(i + 1) % SECTORS] && !sectors[(i + 2) % SECTORS]).count();
        if gaps < 2 {
            out.bad.push(format!("{name}: {gaps} gaps"));
        }
        // Inside the edge, and outside it, reached from the platform.
        let (mut inner, mut inner_reached) = (0u32, 0u32);
        let reach_r = p.radius * 86 / 100;
        let r_in = (reach_r * 70 / 100) as i64;
        for y in (p.centre.1 - reach_r).max(0)..(p.centre.1 + reach_r).min(h) {
            for x in (p.centre.0 - reach_r).max(0)..(p.centre.0 + reach_r).min(w) {
                let (dx, dy) = (i64::from(x - p.centre.0), i64::from(y - p.centre.1));
                if dx * dx + dy * dy > r_in * r_in {
                    continue;
                }
                let i = (y * w + x) as usize;
                if tiles[i].flags() & F_SOLID != 0 || blocked[i] {
                    continue;
                }
                inner += 1;
                inner_reached += u32::from(reach.reached(x, y));
            }
        }
        if inner > 50 && inner_reached == 0 {
            out.bad.push(format!("{name}: nothing inside the edge is reached"));
        }
        // No way runs into the edge: every way cell round the patch walks.
        let mut ways_shut = 0usize;
        let r = p.radius + 4;
        for y in (p.centre.1 - r).max(0)..(p.centre.1 + r).min(h) {
            for x in (p.centre.0 - r).max(0)..(p.centre.0 + r).min(w) {
                let i = (y * w + x) as usize;
                let way = c.trodden[i] || matches!(tiles[i], Tile::Road | Tile::GrownPath | Tile::Boardwalk);
                if way && p.core.contains(&(x, y)) {
                    ways_shut += 1;
                }
            }
        }
        if ways_shut > 0 {
            out.bad.push(format!("{name}: the edge stands on {ways_shut} cells of way"));
        }
        out.rows.push(Row { name, p: p.clone(), inner, reached: inner_reached, gaps, cover });
    }
    out
}

/// One of [`SECTORS`] round a centre, by the diamond angle.
fn sector(dx: i32, dy: i32) -> usize {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax + ay == 0 {
        return 0;
    }
    let q = SECTORS as i32 / 4;
    let p = ay * q / (ax + ay);
    let s = match (dx >= 0, dy >= 0) {
        (true, true) => p,
        (false, true) => 2 * q - p,
        (false, false) => 2 * q + p,
        (true, false) => 4 * q - p,
    };
    (s as usize) % SECTORS
}

#[test]
fn every_patch_has_an_edge_with_gaps_on_seeds_1_to_16() {
    let seeds: Vec<u32> = (1..=16).collect();
    let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
    let per = seeds.len().div_ceil(threads).max(1);
    let all: Vec<Seen> = std::thread::scope(|s| {
        let jobs: Vec<_> =
            seeds.chunks(per).map(|part| s.spawn(move || part.iter().map(|&n| check(n)).collect::<Vec<_>>())).collect();
        jobs.into_iter().flat_map(|j| j.join().expect("a county builds")).collect()
    });
    let mut bad = Vec::new();
    for s in &all {
        for r in &s.rows {
            println!(
                "{:>3} {:<16} {:<5} r{:<4} at {:?} core {:<5} cover {:<3}% gaps {:<3} knocked {:<3} inside {}/{}",
                s.seed,
                r.name,
                format!("{:?}", r.p.edge),
                r.p.radius,
                r.p.centre,
                r.p.core.len(),
                r.cover,
                r.gaps,
                r.p.knocked,
                r.reached,
                r.inner
            );
        }
        let mean = s.rows.iter().map(|r| r.cover).sum::<usize>() / s.rows.len().max(1);
        if mean < COVER_MEAN {
            bad.push(format!("seed {}: its edges stand round {mean}% of their rings on average", s.seed));
        }
        bad.extend(s.bad.iter().map(|b| format!("seed {}: {b}", s.seed)));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
