//! The carve's tests (PLAN-map-r4 section 12): the county carves clean and alike, the packed levels match a
//! byte reference, one crafted failure for each check, and the sketch tools.

use super::blocks::*;
use super::joins::JoinKind;
use super::*;
use crate::county::terraced::macro_plan::{Over, Reg};
use jane_core::Tile;

// --- the acceptance tests (PLAN-map-r4 section 12, WP2) ---------------------------------------------

use std::sync::OnceLock;

/// The county's carve, from its source, made once: what the byte-reference and the crafted-failure tests read.
fn first() -> &'static [Carved] {
    static CARVES: OnceLock<Vec<Carved>> = OnceLock::new();
    CARVES.get_or_init(|| {
        let src = crate::county::terraced::source::county();
        vec![carve_proven(src.plan, src.salt).unwrap_or_else(|e| panic!("the county: {e:?}"))]
    })
}

fn issues_of(c: &Carved, code: &str) -> Vec<String> {
    check::check(c).into_iter().filter(|i| i.code == code).map(|i| i.text).collect()
}

/// The county carves, the checks hold on it, and two carves of it are one carve.
#[test]
fn the_county_carves_clean_and_alike() {
    let c = &first()[0];
    let issues = check::check(c);
    assert!(issues.is_empty(), "{issues:?}");
    assert!(c.spans.len() <= jane_core::blueprint::SPANS_MAX, "{} spans", c.spans.len());
    let again = carve_proven(c.plan.clone(), c.salt).expect("a second carve");
    assert!(*c == again, "two carves differ");
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
        .flat_map(|by| (40..BW - 40).map(move |bx| (bx, by)))
        .find_map(|(bx, by)| {
            let (a, b2) = (c.blocks.district[Blocks::ix(bx, by)], c.blocks.district[Blocks::ix(bx + 1, by)]);
            let (ra, rb) = (c.plan.districts[usize::from(a)].reg, c.plan.districts[usize::from(b2)].reg);
            (ra != rb && ra != Reg::Works && rb != Reg::Works).then_some((bx * B + 3, by * B))
        })
        .expect("a border between the Lowfields and the Waters");
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

/// The county's river passes C-3, and the channel bends (its path is at least a tenth longer than the
/// chords of its 192-cell windows).
#[test]
fn the_river_meanders() {
    {
        let c = &first()[0];
        assert!(issues_of(c, "water").is_empty(), "{:?}", issues_of(c, "water"));
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
        assert!(chord > 0, "no 192-cell window");
        assert!(path * 10 >= chord * 11, "path {path}, chords {chord}");
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
    let c = &first()[0];
    let (_, mut b, _) = ground(&c.plan, c.salt).expect("the ground of a carved plan");
    let before = b.clone();
    blocks::tidy(&mut b, &c.plan, c.salt).expect("tidy again");
    assert!(b == before);
}
