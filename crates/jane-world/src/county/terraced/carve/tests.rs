//! The carve's tests (PLAN-map-r4 section 12): every seed carves clean and alike, the packed levels match a
//! byte reference, one crafted failure for each check, and the sketch tools.

use super::blocks::*;
use super::joins::JoinKind;
use super::*;
use crate::county::terraced::macro_plan::{Over, Reg};
use jane_core::Tile;

pub(super) fn level_rgb(l: u8) -> [u8; 3] {
    match l {
        0 => [96, 140, 120],
        1 => [124, 176, 92],
        2 => [176, 160, 96],
        _ => [226, 212, 190],
    }
}

pub(super) fn tile_rgb(t: Option<Tile>, level: u8) -> [u8; 3] {
    match t {
        Some(Tile::Cliff) => [64, 48, 36],
        Some(Tile::Water) => [50, 90, 170],
        Some(Tile::Waterfall) => [255, 255, 255],
        Some(Tile::Stair) => [250, 250, 150],
        Some(Tile::Road) => [210, 170, 120],
        Some(Tile::LedgeS | Tile::LedgeE | Tile::LedgeW | Tile::LedgeN) => [255, 120, 30],
        Some(Tile::Stepping) => [200, 200, 200],
        Some(Tile::Boardwalk) => [150, 100, 60],
        Some(Tile::Track) => [90, 90, 90],
        _ => level_rgb(level),
    }
}

/// A tint by region so the borders read: Lowfields green, Waters blue, Works red-brown.
pub(super) fn region_tint(c: [u8; 3], r: Reg, flags: u8) -> [u8; 3] {
    let t: [i32; 3] = match r {
        Reg::Lowfields => [-10, 20, -10],
        Reg::Waters => [-20, 0, 30],
        Reg::Works => [30, -10, -20],
    };
    let mut o = [0u8; 3];
    for k in 0..3 {
        o[k] = (i32::from(c[k]) + t[k] + if flags & VALLEY != 0 { -12 } else { 0 }).clamp(0, 255) as u8;
    }
    o
}

/// The carve at `scale` cells to a pixel (a made cell in the pixel shows over the ground).
pub(super) fn dump(c: &Carved, scale: i32, path: &str) {
    dump_crop(c, scale, (0, 0, W), path);
}

pub(super) fn priority(t: Option<Tile>) -> u8 {
    match t {
        Some(Tile::Waterfall) => 6,
        Some(Tile::Stair | Tile::LedgeS | Tile::LedgeE | Tile::LedgeW | Tile::LedgeN | Tile::Stepping) => 5,
        Some(Tile::Road | Tile::Boardwalk | Tile::Track) => 4,
        Some(Tile::Cliff) => 3,
        Some(Tile::Water) => 2,
        Some(_) => 1,
        None => 0,
    }
}

/// A square crop `(x0, y0, size)` of cells at `scale` cells to a pixel (scale 1: a pixel a cell).
pub(super) fn dump_crop(c: &Carved, scale: i32, crop: (i32, i32, i32), path: &str) {
    let (n, px) = render_crop(c, scale, crop);
    write_png(n, &px, path);
}

pub(super) fn write_png(n: usize, px: &[u8], path: &str) {
    let zoom = if n <= 128 {
        6
    } else if n <= 256 {
        3
    } else {
        1
    };
    if zoom > 1 {
        let m = n * zoom;
        let mut big = vec![255u8; m * m * 4];
        for y in 0..m {
            for x in 0..m {
                let o = (y * m + x) * 4;
                let so = ((y / zoom) * n + x / zoom) * 4;
                big[o..o + 4].copy_from_slice(&px[so..so + 4]);
            }
        }
        std::fs::write(path, jane_art::sheet::png(m as u32, m as u32, &big)).unwrap();
    } else {
        std::fs::write(path, jane_art::sheet::png(n as u32, n as u32, px)).unwrap();
    }
}

pub(super) fn render_crop(c: &Carved, scale: i32, crop: (i32, i32, i32)) -> (usize, Vec<u8>) {
    let (cx0, cy0, size) = crop;
    let n = (size / scale) as usize;
    let mut px = vec![255u8; n * n * 4];
    let mut row = vec![None; W as usize];
    for y in 0..size / scale {
        let mut t_row: Vec<Option<Tile>> = vec![None; n];
        for dy in 0..scale {
            row.fill(None);
            c.tile_row(cy0 + y * scale + dy, &mut row);
            for x in 0..size / scale {
                for dx in 0..scale {
                    let t = row[(cx0 + x * scale + dx) as usize];
                    let cur = &mut t_row[x as usize];
                    if priority(t) > priority(*cur) {
                        *cur = t;
                    }
                }
            }
        }
        for x in 0..size / scale {
            let (px_x, px_y) = (cx0 + x * scale, cy0 + y * scale);
            let bi = Blocks::ix(px_x / 4, px_y / 4);
            let mut col = tile_rgb(t_row[x as usize], c.level_at(px_x, px_y));
            if c.spans
                .iter()
                .any(|sp| (0..scale).any(|d| sp.rect.contains(px_x + d, px_y) || sp.rect.contains(px_x, px_y + d)))
            {
                col = [200, 60, 200];
            }
            if t_row[x as usize].is_none() {
                col = region_tint(col, c.plan.districts[usize::from(c.blocks.district[bi])].reg, c.blocks.flags[bi]);
            }
            let o = (y as usize * n + x as usize) * 4;
            px[o..o + 3].copy_from_slice(&col);
        }
    }
    if scale >= 2 {
        // Glyphs so the made things read on an overview: each join a 5 px square, each span a
        // magenta bar, each plank brown.
        let mut put = |cx: i32, cy: i32, r: i32, col: [u8; 3]| {
            let (px_x, px_y) = ((cx - cx0) / scale, (cy - cy0) / scale);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (px_x + dx, px_y + dy);
                    if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                        let o = (y as usize * n + x as usize) * 4;
                        px[o..o + 3].copy_from_slice(&col);
                    }
                }
            }
        };
        for j in &c.joins {
            let col = match j.kind {
                JoinKind::Stair => [255, 255, 255],
                JoinKind::Ramp => [255, 235, 0],
                JoinKind::Ledge => [255, 110, 0],
                JoinKind::Closed(_) => [150, 150, 150],
                _ => [0, 0, 0],
            };
            let (cx, cy) = j.rect.centre();
            put(cx, cy, 2, col);
        }
        for sp in &c.spans {
            let (cx, cy) = sp.rect.centre();
            let n8 = (sp.length() / 2 / scale).max(1);
            for k in -n8..=n8 {
                if sp.along_x {
                    put(cx + k * scale, cy, 1, [230, 40, 230]);
                } else {
                    put(cx, cy + k * scale, 1, [230, 40, 230]);
                }
            }
        }
        for r in &c.planks {
            let (cx, cy) = r.centre();
            put(cx, cy, 1, [140, 80, 30]);
        }
    }
    (n, px)
}

// --- the acceptance tests (PLAN-map-r4 section 12, WP2) ---------------------------------------------

use std::sync::OnceLock;

/// Seeds a sweep runs: `SEEDS` (64 by default; CI's checked profile asks for fewer).
#[allow(clippy::disallowed_methods)]
fn seeds() -> u32 {
    std::env::var("SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(64)
}

/// The carves of seeds 1 to 4, built once: what the byte-reference and the crafted-failure tests read.
fn first() -> &'static [Carved] {
    static CARVES: OnceLock<Vec<Carved>> = OnceLock::new();
    CARVES
        .get_or_init(|| (1..=4).map(|seed| carve_seed(seed).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"))).collect())
}

fn issues_of(c: &Carved, code: &str) -> Vec<String> {
    check::check(c).into_iter().filter(|i| i.code == code).map(|i| i.text).collect()
}

/// Every seed carves, the checks hold on it, and two carves of it are one carve.
#[test]
fn every_seed_carves_clean_and_alike() {
    for seed in 1..=seeds() {
        let c = carve_seed(seed).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        let issues = check::check(&c);
        assert!(issues.is_empty(), "seed {seed}: {issues:?}");
        // The carve's own claims: a stair or ramp for each plan gate it laid, a span for each crossing.
        assert!(c.spans.len() <= jane_core::blueprint::SPANS_MAX, "seed {seed}: {} spans", c.spans.len());
        if seed <= 4 {
            let again = carve_seed(seed).expect("a second carve");
            assert!(c == again, "seed {seed}: two carves differ");
        }
    }
}

/// The packed level plane, read cell by cell, is the byte grid the blocks and the joins' lower halves
/// make (4 seeds).
#[test]
fn the_packed_levels_equal_a_byte_reference() {
    for c in first() {
        let w = W as usize;
        let mut reference = vec![0u8; w * w];
        for y in 0..W {
            for x in 0..W {
                reference[y as usize * w + x as usize] = c.blocks.level[Blocks::ix(x / B, y / B)];
            }
        }
        for j in &c.joins {
            if let Some(r) = j.lower {
                for (x, y) in r.cells() {
                    reference[y as usize * w + x as usize] = j.lo;
                }
            }
        }
        let mut bad = 0;
        for y in 0..W {
            for x in 0..W {
                if c.level_at(x, y) != reference[y as usize * w + x as usize] {
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0, "seed {}: {bad} cells differ from the byte reference", c.plan.seed);
    }
}

/// The sketch is a county blueprint of the carve's tiles, levels and spans, with the Halt's start mark.
#[test]
fn the_sketch_holds_the_carve() {
    let c = &first()[0];
    let bp = c.sketch();
    assert_eq!((bp.tiles.w(), bp.tiles.h()), (W as u32, W as u32));
    assert_eq!(bp.spans, c.spans);
    assert_eq!(bp.level.as_ref(), Some(&c.level));
    assert_eq!(bp.marks.len(), 1);
    let mut row = vec![None; W as usize];
    for y in [0, 400, 801, 1535] {
        row.fill(None);
        c.tile_row(y, &mut row);
        for (x, t) in row.iter().enumerate() {
            let want = t.unwrap_or(Tile::Grass);
            assert_eq!(*bp.tiles.get(x as i32, y).unwrap(), want, "({x}, {y})");
        }
    }
}

// --- one crafted failure for each check -------------------------------------------------------------

/// A block of ground nothing has touched: its neighbours all one level, no flags in sight.
fn quiet_block(c: &Carved) -> (i32, i32) {
    for by in 40..BW - 40 {
        for bx in 40..BW - 40 {
            let l = c.blocks.level[Blocks::ix(bx, by)];
            let calm = l < 3
                && (-3..=3).all(|dy| {
                    (-3..=3).all(|dx| {
                        let i = Blocks::ix(bx + dx, by + dy);
                        c.blocks.level[i] == l
                            && c.blocks.flags[i] == 0
                            && c.blocks.district[i] == c.blocks.district[Blocks::ix(bx, by)]
                    })
                });
            if calm && !c.water.bits.get(((by * B) * W + bx * B) as usize) {
                return (bx, by);
            }
        }
    }
    panic!("no quiet ground");
}

/// C-1: a block standing a level above both its neighbours north and south is a spike.
#[test]
fn c1_a_spike_is_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "shape").is_empty());
    let (bx, by) = quiet_block(&c);
    let i = Blocks::ix(bx, by);
    c.blocks.level[i] = (c.blocks.level[i] + 1).min(3);
    let found = issues_of(&c, "shape");
    assert!(found.iter().any(|t| t.starts_with("thin")), "{found:?}");
}

/// `spans::well_formed` holds for every span the carve laid, and not for one of 44.
#[test]
#[allow(clippy::disallowed_types)]
fn well_formed_agrees_with_the_carve() {
    let mut seen = 0;
    for c in first() {
        // Rows of tiles as they are asked for.
        let rows: std::cell::RefCell<std::collections::BTreeMap<i32, Vec<Option<Tile>>>> =
            std::cell::RefCell::new(std::collections::BTreeMap::new());
        let solid = |x: i32, y: i32| {
            let mut rows = rows.borrow_mut();
            let row = rows.entry(y).or_insert_with(|| {
                let mut r = vec![None; W as usize];
                c.tile_row(y, &mut r);
                r
            });
            row[x as usize].is_some_and(|t| t.flags() & jane_core::tile::F_SOLID != 0)
        };
        let level = |x: i32, y: i32| c.level_at(x, y);
        for s in &c.spans {
            assert!(spans::well_formed(&level, &solid, s), "seed {}: {s:?}", c.plan.seed);
            let mut long = *s;
            if long.along_x {
                long.rect.w = 44;
            } else {
                long.rect.h = 44;
            }
            assert!(!spans::well_formed(&level, &solid, &long));
            seen += 1;
        }
    }
    assert!(seen > 0);
}

/// C-2: a span of 44 is longer than any deck.
#[test]
fn c2_a_forty_four_long_span_is_caught() {
    let mut c = first().iter().find(|c| !c.spans.is_empty()).expect("a carve with a span").clone();
    assert!(issues_of(&c, "spans").is_empty());
    let s = &mut c.spans[0];
    if s.along_x {
        s.rect.w = 44;
    } else {
        s.rect.h = 44;
    }
    let found = issues_of(&c, "spans");
    assert!(found.iter().any(|t| t.contains("44 long")), "{found:?}");
}

/// C-3: a channel that runs straight for 60 cells, and one that lies on a region's border.
#[test]
fn c3_a_straight_channel_and_a_hugging_one_are_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "water").is_empty());
    // No crossing on the river: no stretch of it is straight by design.
    for cr in &mut c.plan.crossings {
        cr.over = Over::Valley;
    }
    // 60 cells of the outflow, laid dead straight (a step of 4 cells a point, the line the carve uses).
    let reach: Vec<usize> = (0..c.water.channel.len()).filter(|&k| c.water.channel[k].reach == 1).collect();
    let mid = reach[reach.len() / 2];
    let (x0, y0) = (c.water.channel[mid].x, c.water.channel[mid].y.min(W - 100));
    for k in 0..20 {
        let p = &mut c.water.channel[mid + k];
        p.x = x0;
        p.y = y0 + 4 * k as i32;
    }
    let found = issues_of(&c, "water");
    assert!(found.iter().any(|t| t.contains("runs straight for 64 cells")), "{found:?}");
    // A canal: a long, gentle swing (it strays three cells from every chord, so it is not straight, but it is
    // under a tenth longer than its chord over 192 cells).
    let mut c = first()[0].clone();
    for cr in &mut c.plan.crossings {
        cr.over = Over::Valley;
    }
    c.water.channel.retain(|p| p.reach == 0);
    let pt = |k: i32| {
        let swing = [0, 1, 2, 3, 3, 2, 1, 0, -1, -2, -3, -3, -2, -1][(k as usize / 2) % 14];
        water::RiverPt { x: 100 + 4 * k, y: 700 + swing, half: 2, level: 0, reach: 1 }
    };
    c.water.channel.extend((0..120).map(pt));
    let found = issues_of(&c, "water");
    assert!(found.iter().any(|t| t.contains("canal")), "{found:?}");
    // Every point on the first border cell found: the river on the border.
    let mut c = first()[0].clone();
    let at = (40..BW - 40)
        .find_map(|bx| {
            let by = BW / 2;
            let (a, b2) = (c.blocks.district[Blocks::ix(bx, by)], c.blocks.district[Blocks::ix(bx + 1, by)]);
            let (ra, rb) = (c.plan.districts[usize::from(a)].reg, c.plan.districts[usize::from(b2)].reg);
            (ra != rb && ra != Reg::Works && rb != Reg::Works).then_some((bx * B + 3, by * B))
        })
        .expect("a border on the middle row");
    for p in &mut c.water.channel {
        p.x = at.0;
        p.y = at.1;
    }
    let found = issues_of(&c, "water");
    assert!(found.iter().any(|t| t.contains("hug a border")), "{found:?}");
}

/// A join that lays one tile over a rect: walls and ledges for the crafted pits and cuts.
fn wall(rect: Rect, tile: Tile) -> super::joins::Join {
    super::joins::Join {
        kind: JoinKind::Stair,
        gate: None,
        rect,
        tile,
        up: (0, 0),
        lo: 0,
        hi: 0,
        foot: rect.centre(),
        top: rect.centre(),
        landings: [Rect::new(0, 0, 0, 0); 2],
        lower: None,
    }
}

/// C-4: a ledge hopped over a wall into a closed pocket is a pit: she gets in and cannot get out.
#[test]
fn c4_a_ledge_into_a_pit_is_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "pit").is_empty());
    let (bx, by) = quiet_block(&c);
    let (cx, cy) = (bx * B, by * B);
    // A pocket five cells square, its west wall holding the ledge (a hop east), walled all round.
    for r in [
        Rect::new(cx - 3, cy - 3, 7, 1),
        Rect::new(cx - 3, cy + 3, 7, 1),
        Rect::new(cx + 3, cy - 2, 1, 5),
        Rect::new(cx - 3, cy - 2, 1, 2),
        Rect::new(cx - 3, cy + 1, 1, 2),
    ] {
        c.joins.push(wall(r, Tile::Cliff));
    }
    let mut hop = wall(Rect::new(cx - 3, cy, 1, 1), Tile::LedgeE);
    hop.kind = JoinKind::Ledge;
    hop.up = (1, 0);
    c.joins.push(hop);
    let found = issues_of(&c, "pit");
    assert!(found.iter().any(|t| t.contains("can be reached but not left")), "{found:?}");
}

/// C-5: a border with two gates built is short of three.
#[test]
fn c5_a_border_with_two_gates_is_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "gates").is_empty());
    let on_border = |gi: usize| {
        let g = &c.plan.gates[gi];
        let (a, b) = (c.plan.districts[usize::from(g.da)].reg, c.plan.districts[usize::from(g.db)].reg);
        (a == Reg::Lowfields && b == Reg::Works) || (a == Reg::Works && b == Reg::Lowfields)
    };
    let gates: Vec<u8> = (0..c.plan.gates.len()).filter(|&gi| on_border(gi)).map(|gi| gi as u8).collect();
    // Keep the first two gates that have a way, strip the others' joins and spans.
    let mut kept = 0;
    let mut strip: Vec<u8> = Vec::new();
    for &g in &gates {
        let has = c.joins.iter().any(|j| j.gate == Some(g) && matches!(j.kind, JoinKind::Stair | JoinKind::Ramp));
        if has && kept < 2 {
            kept += 1;
        } else {
            strip.push(g);
        }
    }
    c.joins.retain(|j| !j.gate.is_some_and(|g| strip.contains(&g)));
    let crossings: Vec<u8> = c
        .plan
        .crossings
        .iter()
        .enumerate()
        .filter(|(_, x)| x.gate.is_some_and(|g| strip.contains(&g)))
        .map(|(i, _)| i as u8)
        .collect();
    let keep: Vec<bool> = c.span_meta.iter().map(|m| !crossings.contains(&m.crossing)).collect();
    let mut k = 0;
    c.spans.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    let mut k = 0;
    c.span_meta.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    let found = issues_of(&c, "gates");
    assert!(found.iter().any(|t| t.starts_with("escarpment: 2 ways across")), "{found:?}");
}

/// C-6: no ledge near a hub, and a ledge whose walk back is too short.
#[test]
fn c6_a_hub_with_no_ledge_is_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "ledges").is_empty());
    c.joins.retain(|j| j.kind != JoinKind::Ledge);
    let found = issues_of(&c, "ledges");
    assert!(found.iter().any(|t| t.starts_with("no ledge lands within 60 cells of")), "{found:?}");
}

/// C-7: the Halt walled in leaves Julie's and the Castle off its level-one ground.
#[test]
fn c7_a_cut_first_walk_is_caught() {
    let mut c = first()[0].clone();
    assert!(issues_of(&c, "first_walk").is_empty());
    // The Halt walled in: a ring of cliff round its pad's centre.
    let (sx, sy) = c.pads.iter().find(|p| p.site == "station").expect("the Halt").boxr.centre();
    for r in [
        Rect::new(sx - 10, sy - 10, 21, 1),
        Rect::new(sx - 10, sy + 10, 21, 1),
        Rect::new(sx - 10, sy - 9, 1, 19),
        Rect::new(sx + 10, sy - 9, 1, 19),
    ] {
        c.joins.push(wall(r, Tile::Cliff));
    }
    let found = issues_of(&c, "first_walk");
    assert!(found.iter().any(|t| t.contains("is not on the Halt's level ground")), "{found:?}");
}

/// Seeds 3 and 7, where R0's river ran along the border: here they carve, pass C-3, and the channel
/// bends (its path is at least a tenth longer than the chords of its 192-cell windows).
#[test]
fn seeds_three_and_seven_meander() {
    for seed in [3, 7] {
        let c = carve_seed(seed).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        assert!(issues_of(&c, "water").is_empty(), "seed {seed}: {:?}", issues_of(&c, "water"));
        let (mut path, mut chord) = (0i64, 0i64);
        for r in 0..2u8 {
            let pts: Vec<_> = c.water.channel.iter().filter(|p| p.reach == r).collect();
            for win in pts.windows(49) {
                for pair in win.windows(2) {
                    path += water::isqrt(i64::from((pair[0].x - pair[1].x).pow(2) + (pair[0].y - pair[1].y).pow(2)));
                }
                chord += water::isqrt(i64::from((win[0].x - win[48].x).pow(2) + (win[0].y - win[48].y).pow(2)));
            }
        }
        assert!(chord > 0, "seed {seed}: no 192-cell window");
        assert!(path * 10 >= chord * 11, "seed {seed}: path {path}, chords {chord}");
    }
}

/// The opening: a ribbon two blocks wide of another level goes, a plateau five blocks wide stays, and
/// the blocks tidy never changes are left be.
#[test]
fn open_thin_removes_ribbons_and_keeps_plateaus() {
    let w = BW as usize;
    let mut b = Blocks::new();
    b.level.fill(1);
    for by in 100..160 {
        for bx in 100..102 {
            b.level[Blocks::ix(bx, by)] = 2; // a ribbon of two blocks
        }
        for bx in 200..205 {
            b.level[Blocks::ix(bx, by)] = 2; // a plateau of five
        }
    }
    b.flags[Blocks::ix(100, 120)] = PAD; // held
    blocks::open_thin(&mut b, 5);
    assert!((100..160).all(|by| b.level[Blocks::ix(101, by)] == 1), "the ribbon is gone");
    assert_eq!(b.level[Blocks::ix(100, 120)], 2, "a held block stays");
    assert!((100..160).all(|by| (200..205).all(|bx| b.level[Blocks::ix(bx, by)] == 2)), "the plateau stays");
    let twos = (0..b.level.len()).filter(|&i| b.level[i] == 2).count();
    assert_eq!(twos, 60 * 5 + 1, "nothing else moved (of {w} blocks across)");
    // A second opening finds nothing more.
    let once = b.clone();
    blocks::open_thin(&mut b, 5);
    assert!(b == once);
}

// --- the carve's parts ---------------------------------------------------------------------------------

/// Tidy leaves levels it has tidied as they are.
#[test]
fn tidy_is_idempotent() {
    let c = &first()[1];
    let (_, mut b, _) = ground(&c.plan, c.salt).expect("the ground of a carved plan");
    let before = b.clone();
    blocks::tidy(&mut b, &c.plan, c.salt).expect("tidy again");
    assert!(b == before);
}

// --- tools (run by hand) ---------------------------------------------------------------------------

/// The sweep of section 12: 256 seeds, how many carves are refused, how many re-rolls the others took, and
/// how long the accepted carve takes. `cargo test --release -p jane-world carve::tests::sweep -- --ignored --nocapture`.
#[test]
#[ignore = "256 seeds: minutes"]
#[allow(clippy::disallowed_methods, clippy::disallowed_types)]
fn sweep() {
    use std::time::Instant;
    let n: u32 = std::env::var("SWEEP").ok().and_then(|s| s.parse().ok()).unwrap_or(256);
    let (mut refused, mut plans_hist, mut salts_hist) = (Vec::new(), [0u32; 8], [0u32; 8]);
    let mut times: Vec<u128> = Vec::new();
    let mut full: Vec<u128> = Vec::new();
    for seed in 1..=n {
        let t_all = Instant::now();
        let (r, t) = carve_seed_stats(seed);
        full.push(t_all.elapsed().as_micros());
        match r {
            Ok(c) => {
                plans_hist[(t.plans as usize).min(7)] += 1;
                salts_hist[(t.salts as usize).min(7)] += 1;
                // The accepted carve alone, with its checks, timed.
                let t0 = Instant::now();
                let again = carve_proven(c.plan.clone(), c.salt).expect("the same carve again");
                times.push(t0.elapsed().as_micros());
                assert!(again == c, "seed {seed}: the carve is not repeatable");
            }
            Err(e) => refused.push((seed, e)),
        }
    }
    times.sort();
    let med = times.get(times.len() / 2).copied().unwrap_or(0);
    let p90 = times.get(times.len() * 9 / 10).copied().unwrap_or(0);
    println!("{n} seeds: {} refused {:?}", refused.len(), refused);
    println!("plans tried (1..7+): {:?}; salts tried (1..7+): {:?}", &plans_hist[1..], &salts_hist[1..]);
    println!(
        "carve + checks of the accepted plan: median {med} us, p90 {p90} us, max {} us",
        times.last().copied().unwrap_or(0)
    );
    // The whole of `carve_seed`: every plan the macro plan builds and refuses on the way (each a build and a
    // macro check, several ms) and every re-roll.
    full.sort();
    println!(
        "carve_seed whole: median {} us, p90 {} us, max {} us",
        full[full.len() / 2],
        full[full.len() * 9 / 10],
        full.last().copied().unwrap_or(0)
    );
    assert!(refused.len() * 50 < n as usize, "{} of {n} seeds refused", refused.len());
    if !cfg!(debug_assertions) {
        assert!(med <= 40_000, "median carve {med} us");
        assert!(times.last().copied().unwrap_or(0) <= 150_000, "slowest carve {:?} us", times.last());
    }
}

/// The tiles-only sketches of seeds 1 to 8 (768 square, two cells a pixel) and a contact sheet of them,
/// into `CARVE_OUT` (`target/carve` by default).
#[test]
#[ignore = "writes the sketches"]
#[allow(clippy::disallowed_methods)]
fn sketches() {
    let dir = std::env::var("CARVE_OUT").unwrap_or_else(|_| "../../target/carve".to_string());
    std::fs::create_dir_all(&dir).unwrap();
    for seed in 1..=8u32 {
        let c = carve_seed(seed).unwrap_or_else(|e| panic!("seed {seed}: {e:?}"));
        dump(&c, 2, &format!("{dir}/carve-{seed:02}.png"));
        println!(
            "seed {seed}: plan attempt {} salt {} joins {} spans {} planks {}",
            c.plan.attempt,
            c.salt,
            c.joins.len(),
            c.spans.len(),
            c.planks.len()
        );
    }
}
