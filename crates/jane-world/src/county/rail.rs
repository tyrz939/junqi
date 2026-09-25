//! The railway in cells (`layRailway`, `railCurves`): one width of sleepers and rails, walkable,
//! with a shoulder of ballast either side and the growth cut back a cell beyond that. Where a road
//! crosses it the road keeps its metal and a sign stands at the crossing; where it crosses water it
//! goes over on a trestle (the track, a deck of planks either side, a rail of fence at the edges).
//! At each end, where it leaves the county through the trees, a fence crosses it: the rails run on
//! under it and out of sight. After the roads, so a road keeps its metal at a level crossing, and
//! after the chunks, so the halt's own platform rails stay as they were drawn.
//!
//! No dice: the line is the skeleton's, its bends are geometry.

use jane_core::action::Action;
use jane_core::{Key, Rect, Tile};

use super::County;
use super::land::EDGE;
use crate::kit::js_round;
use crate::skeleton::{COUNTY_H, COUNTY_W, MACRO, Region, SKEL_H, SKEL_W, Skeleton, Water};

/// Up the west fence the line runs at this column: the halt's platform rails are at 4 to 6.
pub const RAIL_WEST_X: i32 = 5;
/// The widest a bend of the line is drawn: cells from the corner to where the curve begins.
pub const BEND: i32 = 32;

/// The line's centre, in order from the south edge to the east (or north) edge, four-connected:
/// every sleeper has a neighbour along the way the line runs. Empty with no line.
pub fn rail_line(sk: &Skeleton, bend: i32) -> Vec<(i32, i32)> {
    if sk.rail.len() < 2 {
        return Vec::new();
    }
    let mut pts: Vec<(i32, i32)> = sk
        .rail
        .iter()
        .map(|&(mx, my)| (if mx == 0 { RAIL_WEST_X } else { mx * MACRO + MACRO / 2 }, my * MACRO + MACRO / 2))
        .collect();
    // Out to the map's own edge at both ends.
    let reach = |(x, y): (i32, i32)| {
        if y >= COUNTY_H - MACRO {
            (x, COUNTY_H - 1)
        } else if y < MACRO {
            (x, 0)
        } else if x >= COUNTY_W - MACRO {
            (COUNTY_W - 1, y)
        } else {
            (0, y)
        }
    };
    pts.insert(0, reach(pts[0]));
    pts.push(reach(pts[pts.len() - 1]));
    let path = rail_curves(&pts, sk, bend);
    let mut line = vec![path[0]];
    for w in path.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let steps = (bx - ax).abs().max((by - ay).abs());
        for s in 1..=steps {
            let x = ax + js_round(i64::from(bx - ax) * i64::from(s), i64::from(steps)) as i32;
            let y = ay + js_round(i64::from(by - ay) * i64::from(s), i64::from(steps)) as i32;
            let (px, py) = line[line.len() - 1];
            // A diagonal step becomes two square ones.
            if x != px && y != py {
                line.push((x, py));
            }
            line.push((x, y));
        }
    }
    line
}

/// The skeleton's line is square to the grid: straights and right-angle corners. Keep only the
/// corners, and round each into a curve (a quadratic from where the bend begins, through the
/// corner's pull, to where it ends) as wide as the straights either side allow. A bend that would
/// swing over the lake is drawn tighter.
fn rail_curves(pts: &[(i32, i32)], sk: &Skeleton, bend: i32) -> Vec<(i32, i32)> {
    let heading = |a: (i32, i32), b: (i32, i32)| ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    let mut corners = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        if heading(pts[i - 1], pts[i]) != heading(pts[i], pts[i + 1]) {
            corners.push(pts[i]);
        }
    }
    corners.push(pts[pts.len() - 1]);
    let len = |a: (i32, i32), b: (i32, i32)| (b.0 - a.0).abs() + (b.1 - a.1).abs();
    let lake = |(x, y): (i32, i32)| {
        sk.terrain.water.read((x >> 4).min(SKEL_W - 1), (y >> 4).min(SKEL_H - 1), Water::Dry) == Water::Lake
    };
    let mut out = vec![corners[0]];
    for i in 1..corners.len() - 1 {
        let c = corners[i];
        let (a, b) = (corners[i - 1], corners[i + 1]);
        let ia = heading(a, c);
        let ob = heading(c, b);
        // B(t) = (1-t)^2 P0 + 2t(1-t) C + t^2 P2, with P0 = C - r*in and P2 = C + r*out, at
        // t = s / 2r: the corner's own term cancels, leaving two squares over (2r)^2.
        let curve = |r: i32| -> Vec<(i32, i32)> {
            let span = 2 * r;
            let d = i64::from(span * span);
            (0..=span)
                .map(|s| {
                    let (u, v) = (i64::from(span - s), i64::from(s));
                    let at = |c: i32, i: i32, o: i32| {
                        c + js_round(-u * u * i64::from(r * i) + v * v * i64::from(r * o), d) as i32
                    };
                    (at(c.0, ia.0, ob.0), at(c.1, ia.1, ob.1))
                })
                .collect()
        };
        let mut r = bend.min(len(a, c) / 2).min(len(c, b) / 2);
        while r > 2 && curve(r).into_iter().any(lake) {
            r >>= 1;
        }
        if r > 0 {
            out.extend(curve(r));
        } else {
            out.push(c);
        }
    }
    out.push(corners[corners.len() - 1]);
    out
}

/// Whether a tile is growth the line cuts back.
fn growth(t: Tile) -> bool {
    matches!(
        t,
        Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Cliff | Tile::Rubble | Tile::Hedge | Tile::Fence | Tile::Wall
    )
}

/// The railway, rastered: see the module docs.
pub fn lay_railway(c: &mut County<'_>) {
    let sk = c.sk;
    let line = rail_line(sk, BEND);
    if line.is_empty() {
        return;
    }
    let w = COUNTY_W;
    let mut centre: Vec<i32> = line.iter().map(|&(x, y)| y * w + x).collect();
    centre.sort();
    centre.dedup();
    let on_line = |x: i32, y: i32| centre.binary_search(&(y * w + x)).is_ok();
    let inner = |x: i32, y: i32| x >= EDGE && y >= EDGE && x < COUNTY_W - EDGE && y < COUNTY_H - EDGE;
    // Anything small already standing where the line goes gives way to it (a scatter from a
    // patch's dressing). Nothing with a name is ever this near the line.
    let mut anon: Vec<Key> =
        c.k.blueprint()
            .props
            .iter()
            .map(|p| p.key)
            .filter(|&k| c.k.local_name(k).is_some_and(|n| n.starts_with("county_")))
            .collect();
    anon.sort();
    c.k.retain_props(|p| {
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        anon.binary_search(&p.key).is_err() || !(-1..=2).any(|oy| (-1..=2).any(|ox| on_line(x + ox, y + oy)))
    });
    let sign = jane_data::catalog().story.prop_id("sign").expect("a sign row");
    let label = c.k.text("A crossing sign");
    let words = c.k.text("RAILWAY CROSSING. STOP, LOOK AND LISTEN.");
    let read = c.k.list(vec![Action::Read(words)]);
    let mut crossing = false;
    let mut signs = 0;
    for (i, &(x, y)) in line.iter().enumerate() {
        let here = c.k.get(x, y);
        // A level crossing: the road keeps its metal, and a sign stands at the first crossing of
        // each road.
        if here == Tile::Road || here == Tile::Boardwalk {
            if !crossing {
                let (px, py) = line[i.saturating_sub(3)];
                for (ox, oy) in [(2, 0), (-3, 0), (0, 2), (0, -2)] {
                    if !c.k.fits(px + ox, py + oy, 2, 1, 0) || on_line(px + ox, py + oy) {
                        continue;
                    }
                    let key = c.k.local(&format!("rail_crossing_{signs}"));
                    signs += 1;
                    let p = c.k.prop(Some(key), sign, px + ox, py + oy);
                    p.label = Some(label);
                    p.use_list = Some(read);
                    break;
                }
            }
            crossing = true;
            continue;
        }
        crossing = false;
        if here == Tile::Rail {
            continue; // the halt's own platform rails
        }
        let wet = here == Tile::Water;
        c.k.set(x, y, Tile::Track);
        for oy in -2..=2i32 {
            for ox in -2..=2i32 {
                let (cx, cy) = (x + ox, y + oy);
                if (ox == 0 && oy == 0) || !inner(cx, cy) || on_line(cx, cy) {
                    continue;
                }
                let t = c.k.get(cx, cy);
                if matches!(t, Tile::Road | Tile::Boardwalk | Tile::Track | Tile::Rail) {
                    continue;
                }
                let near = ox.abs().max(oy.abs()) == 1;
                if t == Tile::Water {
                    // The trestle: planks either side of the rails, and a rail of fence along its edges.
                    if near || wet {
                        c.k.set(cx, cy, if near { Tile::Boardwalk } else { Tile::Fence });
                    }
                } else if near {
                    c.k.set(cx, cy, Tile::Dirt);
                } else if growth(t) {
                    let works = sk.region_at(cx >> 4, cy >> 4) == Region::Works;
                    c.k.set(cx, cy, if works { Tile::Dirt } else { Tile::Grass });
                }
            }
        }
    }
    // Where it leaves: a fence across the cutting at the inner edge of the trees, the rails
    // running on beyond it.
    for end in [line[0], line[line.len() - 1]] {
        let vertical = end.1 == 0 || end.1 == COUNTY_H - 1;
        // `d` counts in from the map's edge: the trees are 0 to EDGE - 1, and the fence stands on
        // the last of them.
        let cell = |d: i32, o: i32| {
            if vertical {
                (end.0 + o, if end.1 == 0 { d } else { COUNTY_H - 1 - d })
            } else {
                (if end.0 == 0 { d } else { COUNTY_W - 1 - d }, end.1 + o)
            }
        };
        for o in -3..=3 {
            let (x, y) = cell(EDGE - 1, o);
            c.k.set(x, y, Tile::Fence);
        }
        // Beyond the fence the trees stand back from the line, so the rails are seen to go on.
        for d in 0..EDGE - 1 {
            for (o, t) in [(0, Tile::Track), (-1, Tile::Dirt), (1, Tile::Dirt)] {
                let (x, y) = cell(d, o);
                c.k.set(x, y, t);
            }
        }
    }
    // The line is the line: nothing the country builds later stands on it.
    for &(x, y) in &line {
        c.k.claim(Rect::new(x - 2, y - 2, 5, 5));
    }
}
