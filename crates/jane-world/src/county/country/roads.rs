//! The roads' furniture (`bridges`, `forks`, `lamps`, `milestones` in `country.ts`): lamps at an
//! even step on one side of each lit road, a lamp at each end of a bridge, a fingerpost at every
//! fork naming where each way goes and how far, and milestones with the distance to Castle. No
//! dice: where a road runs decides all of it.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::action::Action;
use jane_core::{DialogueId, Grid, Key, PropDefId, Rect, Tile};

use super::defs::defs;
use super::{County, Normal, macro_of, near_chunk, step_milli};
use crate::county::centre;
use crate::kit::js_round;
use crate::skeleton::roads::{FAR, road_distances};
use crate::skeleton::{PlacedSite, ROAD, RoadEnd, SKEL_H, SKEL_W};

/// Lamps along a lit road, cells apart. *Tuning.*
pub const LAMP_STEP: i64 = 22;
/// From the road's centre line to a lamp: past the metal (1) and the verge (2). *Tuning.*
pub const LAMP_OFF: i32 = 3;
/// Milestones, cells apart along a road. *Tuning.*
pub const MILE_STEP: i64 = 230;
/// A place a fingerpost names is at least this far by road, in tenths of a metre. *Tuning.*
const SIGN_NEAREST: u32 = 600;
/// A milestone stands only this far from Castle or further, in tenths of a metre. *Tuning.*
const MILE_NEAREST: u32 = 2500;

/// Which way `(dx, dy)` points, as a fingerpost says it: a plain wind when one axis is more than
/// 2.2 times the other, else the two together (`NORTH-EAST`).
pub fn compass(dx: i32, dy: i32) -> &'static str {
    let (ax, ay) = (i64::from(dx.abs()), i64::from(dy.abs()));
    let north = dy < 0;
    let west = dx < 0;
    if 5 * ax > 11 * ay {
        return if west { "WEST" } else { "EAST" };
    }
    if 5 * ay > 11 * ax {
        return if north { "NORTH" } else { "SOUTH" };
    }
    match (north, west) {
        (true, true) => "NORTH-WEST",
        (true, false) => "NORTH-EAST",
        (false, true) => "SOUTH-WEST",
        (false, false) => "SOUTH-EAST",
    }
}

/// A distance as a sign says it, from tenths of a metre: to the nearest 50 m (never less than
/// 50), and from a kilometre on in tenths of one.
pub fn distance_words(tenths: i64) -> String {
    if tenths < 10_000 {
        format!("{} m", (js_round(tenths, 500) * 50).max(50))
    } else {
        let t = js_round(tenths, 1000);
        format!("{}.{} km", t / 10, t % 10)
    }
}

/// Open, dry, unclaimed ground off the metal for a small thing by the road.
fn placeable(c: &County<'_>, x: i32, y: i32, w: i32, h: i32) -> bool {
    (y..y + h).all(|j| {
        (x..x + w).all(|i| {
            let t = c.k.get(i, j);
            !matches!(t, Tile::Road | Tile::Boardwalk | Tile::Water) && !c.k.solid(i, j) && !c.k.is_claimed(i, j)
        })
    })
}

/// No lamp within `apart` cells.
fn lamp_free(c: &County<'_>, x: i32, y: i32, apart: i32) -> bool {
    c.country.lamp_at.iter().all(|&(lx, ly)| {
        let (dx, dy) = (lx - x, ly - y);
        dx * dx + dy * dy >= apart * apart
    })
}

/// The east road (`county.furnishing.east_road`, from `tuning/country.json`) and its index in the
/// skeleton's roads: the road that runs to its site.
fn east_road(c: &County<'_>) -> Option<(usize, jane_data::EastRoad)> {
    let e = jane_data::catalog().county.furnishing.east_road?;
    let n = c.sk.roads.iter().position(|r| r.to == RoadEnd::Site(u16::from(e.to)))?;
    Some((n, e))
}

/// A notice that answers with `tree`, labelled `label`, beside point `i` of `line` on `side`,
/// three to six cells out, on open ground. True if it went down.
fn notice_beside(c: &mut County<'_>, line: &[(i32, i32)], i: usize, side: i32, tree: DialogueId, label: &str) -> bool {
    let nrm = Normal::of(line, i);
    for off in LAMP_OFF..LAMP_OFF + 4 {
        let (x, y) = nrm.cells(line[i], off * side);
        if near_chunk(c, x, y, 2) || !placeable(c, x, y, 2, 1) {
            continue;
        }
        let label = c.k.text(label);
        let p = c.k.prop(None, defs().p.sign, x, y);
        p.talk = Some(tree);
        p.label = Some(label);
        return true;
    }
    false
}

/// The nearest plain lamp within eight cells of `at` becomes a lamp of row `def`, switched on.
fn bridge_lamp_near(c: &mut County<'_>, at: (i32, i32), def: PropDefId) {
    let plain = defs().p.lamp_post;
    let d2 = |p: &jane_core::blueprint::PropSpawn| {
        let (dx, dy) = (i32::from(p.cell.x) - at.0, i32::from(p.cell.y) - at.1);
        dx * dx + dy * dy
    };
    let near = c.k.props_mut().iter_mut().filter(|p| p.def == plain && d2(p) <= 64).min_by_key(|p| (d2(p), p.cell));
    if let Some(p) = near {
        p.def = def;
        p.on = true;
    }
}

/// A lamp beside the road at point `i` of `line`, on `side` (1: the right of travel), three to five
/// cells off the centre line; none nearer than `apart` to another. True if one went down.
fn lamp_beside(c: &mut County<'_>, line: &[(i32, i32)], i: usize, side: i32, apart: i32) -> bool {
    lamp_of(c, line, i, side, apart, defs().p.lamp_post)
}

/// [`lamp_beside`] with a row of its own: a lamp row lit only while switched on (`lightWhenOn`,
/// the east road's) is stood switched on.
fn lamp_of(c: &mut County<'_>, line: &[(i32, i32)], i: usize, side: i32, apart: i32, def: PropDefId) -> bool {
    let nrm = Normal::of(line, i);
    for off in [LAMP_OFF, LAMP_OFF + 1, LAMP_OFF + 2] {
        let (x, y) = nrm.cells(line[i], off * side);
        let t = c.k.get(x, y);
        if matches!(t, Tile::Road | Tile::Boardwalk | Tile::Water)
            || c.k.solid(x, y)
            || c.k.is_claimed(x, y)
            || near_chunk(c, x, y, 2)
        {
            continue;
        }
        if !lamp_free(c, x, y, apart) {
            return false;
        }
        let lit_when_on = jane_data::catalog().story.prop(def).light_when_on;
        c.k.prop(None, def, x, y).on = lit_when_on;
        c.country.lamp_at.push((x, y));
        return true;
    }
    false
}

/// A lamp at each end of a bridge, on the road's lamp side. Bridges are lit whatever the road is.
/// The east road's river bridge (its longest crossing) is the rect its row names, its two lamps
/// are its `bridge_lamp` row, and its toll board stands at the town end, across the road.
pub fn bridges(c: &mut County<'_>) {
    let east = east_road(c);
    for n in 0..c.sk.roads.len() {
        let line = c.lines[n].clone();
        let wet: Vec<bool> = line.iter().map(|&(x, y)| c.was_water(x, y)).collect();
        // Each crossing as (its first wet point, the first dry point after it or the line's end).
        let mut crossings: Vec<(usize, usize)> = Vec::new();
        let mut from = None;
        for (i, &now) in wet.iter().chain(core::iter::once(&false)).enumerate() {
            match (now, from) {
                (true, None) => from = Some(i),
                (false, Some(f)) => {
                    crossings.push((f, i));
                    from = None;
                }
                _ => {}
            }
        }
        let river = east.filter(|&(e, _)| e == n).and_then(|(_, e)| {
            let longest = crossings.iter().enumerate().max_by_key(|&(k, &(a, b))| (b - a, core::cmp::Reverse(k)));
            longest.map(|(k, _)| (k, e))
        });
        for (k, &(a, b)) in crossings.iter().enumerate() {
            let this = river.filter(|&(r, _)| r == k).map(|(_, e)| e);
            let def = this.map_or(defs().p.lamp_post, |e| e.bridge_lamp);
            // The last dry point before the water, and the first one after it (none when the
            // road ends in the water).
            let before_at = a.saturating_sub(4);
            let after_at = (b < line.len()).then(|| (b + 3).min(line.len() - 1));
            for at in core::iter::once(before_at).chain(after_at) {
                if !lamp_of(c, &line, at, 1, 6, def) && this.is_some() {
                    // Another road's lamp already stands at this end of the bridge (roads share
                    // their crossings): it is the bridge's lamp all the same.
                    bridge_lamp_near(c, line[at], def);
                }
            }
            if let Some(e) = this {
                let (x0, y0, x1, y1) =
                    line[a..b].iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |r, &(x, y)| {
                        (r.0.min(x), r.1.min(y), r.2.max(x), r.3.max(y))
                    });
                c.k.rect(Key::Name(e.bridge), Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1).grow(3));
                // At the town end, across the road; else a little further back toward the town.
                let _ = [-1, 1].iter().any(|&side| {
                    (0..8).any(|d| {
                        d * 2 <= before_at
                            && notice_beside(c, &line, before_at - d * 2, side, e.toll_board, "A toll board")
                    })
                });
            }
        }
    }
}

/// Walking distances along the roads from each site, worked out when first asked for.
struct FromSite<'s> {
    sites: &'s [PlacedSite],
    road: &'s Grid<u8>,
    got: Vec<Option<Grid<u32>>>,
}

impl FromSite<'_> {
    fn of(&mut self, row: u8) -> &Grid<u32> {
        let (sites, road) = (self.sites, self.road);
        self.got[usize::from(row)].get_or_insert_with(|| {
            let s = &sites[usize::from(row)];
            road_distances(road, s.mx, s.my)
        })
    }
}

/// Forks: where a road leaves the network that was there before it. Each gets a fingerpost off the
/// corner, naming the places each way goes and how far by road, and a lamp on the other corner.
pub fn forks(c: &mut County<'_>) {
    let sk = c.sk;
    let named: Vec<u8> =
        sk.sites.iter().filter(|s| s.def.on_road && s.row != sk.named.julie_house).map(|s| s.row).collect();
    let mut from = FromSite { sites: &sk.sites, road: &sk.road, got: vec![None; sk.sites.len()] };
    let mut seen = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    let mut older = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    for (n, r) in sk.roads.iter().enumerate() {
        for i in 1..r.cells.len() {
            let (a, b) = (r.cells[i - 1], r.cells[i]);
            let was = older.read(a.0, a.1, false);
            let now = older.read(b.0, b.1, false);
            if was == now || n == 0 {
                continue;
            }
            let fork = if was { a } else { b };
            if seen.read(fork.0, fork.1, false) {
                continue;
            }
            seen.set(fork.0, fork.1, true);
            sign_fork(c, fork, &named, &mut from, n);
        }
        for &(x, y) in &r.cells {
            older.set(x, y, true);
        }
    }
}

/// One way out of a fork: the neighbouring road cell it leaves by, its wind, and the places it
/// leads to with their distances by road.
struct Way {
    step: (i32, i32),
    dir: &'static str,
    list: Vec<(String, u32)>,
}

fn sign_fork(c: &mut County<'_>, fork: (i32, i32), named: &[u8], from: &mut FromSite<'_>, n: usize) {
    let sk = c.sk;
    let cat = jane_data::catalog();
    let (fx, fy) = fork;
    let on_road = |x: i32, y: i32| sk.road.read(x, y, 0) & ROAD != 0;
    // For each place, the way it lies is the neighbouring road cell nearest it.
    let mut ways: Vec<Way> = Vec::new();
    for &row in named {
        let d = from.of(row);
        let here = d.read(fx, fy, FAR);
        if here == FAR || here < SIGN_NEAREST {
            continue;
        }
        let mut best = here;
        let mut step = None;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (x, y) = (fx + ox, fy + oy);
                if (ox, oy) == (0, 0) || !on_road(x, y) || d.read(x, y, FAR) >= best {
                    continue;
                }
                best = d.read(x, y, FAR);
                step = Some((x, y));
            }
        }
        let Some(step) = step else { continue };
        // Look a few cells down that way, so a wiggle at the fork does not name the wrong wind.
        let mut cur = step;
        for _ in 0..5 {
            let mut nb = d.read(cur.0, cur.1, FAR);
            let mut next = cur;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let (x, y) = (cur.0 + ox, cur.1 + oy);
                    if on_road(x, y) && d.read(x, y, FAR) < nb {
                        nb = d.read(x, y, FAR);
                        next = (x, y);
                    }
                }
            }
            if next == cur {
                break;
            }
            cur = next;
        }
        let dir = compass(cur.0 - fx, cur.1 - fy);
        let name = cat.text(sk.site(row).def.name).to_uppercase();
        match ways.iter_mut().find(|w| w.step == step) {
            Some(w) => w.list.push((name, here)),
            None => ways.push(Way { step, dir, list: vec![(name, here)] }),
        }
    }
    if ways.len() < 2 {
        return;
    }
    let mut parts: Vec<String> = ways
        .iter_mut()
        .map(|w| {
            w.list.sort_by_key(|&(_, m)| m);
            let named: Vec<String> =
                w.list.iter().take(2).map(|(name, m)| format!("{name}, {}", distance_words(i64::from(*m)))).collect();
            format!("{}: {}", w.dir, named.join("; "))
        })
        .collect();
    parts.sort();
    let text = format!("{}.", parts.join(". "));
    // The post stands off the corner, on open ground beside the verge.
    let line = c.lines[n].clone();
    let (cx, cy) = (centre(fx), centre(fy));
    let near = line
        .iter()
        .enumerate()
        .min_by_key(|&(i, &(x, y))| ((x - cx) * (x - cx) + (y - cy) * (y - cy), i))
        .map_or(0, |(i, _)| i);
    let post = defs().p.fingerpost;
    let row = cat.story.prop(post);
    let (pw, ph) = (i32::from(row.w), i32::from(row.h));
    let nrm = Normal::of(&line, near);
    for side in [-1, 1] {
        for off in [4, 5, 6] {
            let (x, y) = nrm.cells(line[near], off * side);
            if near_chunk(c, x, y, 4) || !placeable(c, x, y, pw, ph) {
                continue;
            }
            let key = read_sign(c, post, x, y, "A fingerpost", &text);
            c.fork_posts.push((key, (x, y), text));
            // And a lamp on the other corner, so a fork can be found after dark.
            lamp_beside(c, &line, near, -side, 8);
            return;
        }
    }
}

/// A prop at `(x, y)` with a label and words to read, keyed by where it stands.
fn read_sign(c: &mut County<'_>, def: jane_core::PropDefId, x: i32, y: i32, label: &str, words: &str) -> Key {
    let label = c.k.text(label);
    let words = c.k.text(words);
    let read = c.k.list(vec![Action::Read(words)]);
    let p = c.k.prop(None, def, x, y);
    p.label = Some(label);
    p.use_list = Some(read);
    p.key
}

/// Lamps along every lit stretch: one side of each road (its right), an even step, set off the
/// verge.
pub fn lamps(c: &mut County<'_>) {
    let east = east_road(c);
    for n in 0..c.sk.roads.len() {
        let line = c.lines[n].clone();
        let lit = c.lit[n].clone();
        let this = east.filter(|&(e, _)| e == n).map(|(_, e)| e);
        let def = this.map_or(defs().p.lamp_post, |e| e.lamp);
        let mut stood = Vec::new();
        let mut walked = 0;
        // The first lamp six tenths of a step in.
        let mut next = LAMP_STEP * 600;
        for i in 6..line.len().saturating_sub(6) {
            walked += step_milli(line[i - 1], line[i]);
            if walked < next || !lit.get(i).copied().unwrap_or(false) {
                continue;
            }
            if lamp_of(c, &line, i, 1, (LAMP_STEP - 6) as i32, def) {
                next = walked + LAMP_STEP * 1000;
                stood.push(i);
            }
        }
        // The east road's lighting notice: across the road from its first lamp, else from the
        // next that has room, else beside one.
        if let Some(e) = this {
            let _ = [-1, 1]
                .iter()
                .any(|&side| stood.iter().any(|&i| notice_beside(c, &line, i, side, e.notice, "A notice")));
        }
    }
}

/// Milestones: the distance to Castle by road, cut in stone, on the side the lamps are not.
pub fn milestones(c: &mut County<'_>) {
    let sk = c.sk;
    let town = sk.site(sk.named.town);
    let d = road_distances(&sk.road, town.mx, town.my);
    let stone = defs().p.milestone;
    for n in 0..sk.roads.len() {
        let line = c.lines[n].clone();
        let mut walked = 0;
        let mut next = MILE_STEP * 500;
        for i in 6..line.len().saturating_sub(6) {
            walked += step_milli(line[i - 1], line[i]);
            if walked < next {
                continue;
            }
            let (mx, my) = macro_of(line[i].0, line[i].1);
            let m = d.read(mx, my, FAR);
            if m == FAR || m < MILE_NEAREST {
                continue;
            }
            let (x, y) = Normal::of(&line, i).cells(line[i], -LAMP_OFF);
            if near_chunk(c, x, y, 4) || !placeable(c, x, y, 1, 1) {
                continue;
            }
            read_sign(c, stone, x, y, "A milestone", &format!("CASTLE {}", distance_words(i64::from(m))));
            next = walked + MILE_STEP * 1000;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerpost_says_which_way_and_how_far() {
        assert_eq!(compass(10, 1), "EAST");
        assert_eq!(compass(-1, -10), "NORTH");
        assert_eq!(compass(5, 5), "SOUTH-EAST");
        assert_eq!(compass(-12, 5), "WEST");
        assert_eq!(compass(-11, 5), "SOUTH-WEST");
        assert_eq!(compass(-11, 6), "SOUTH-WEST");
        assert_eq!(distance_words(120), "50 m");
        assert_eq!(distance_words(3740), "350 m");
        assert_eq!(distance_words(9990), "1000 m");
        assert_eq!(distance_words(10_000), "1.0 km");
        assert_eq!(distance_words(12_540), "1.3 km");
    }
}
