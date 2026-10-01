//! PORT.md §6.m stage 7, the country furnishers: the roads' furniture, the relay runs, the small
//! places and their signposts, the country's places, and its life. Carries `density.test.ts` and
//! the static half of `harshness.test.ts` as floors, and the country parts of `county.test.ts` and
//! `town.test.ts` that need no placement rows or stories. Over `SEEDS` seeds (64 by default), every
//! seed's county built once, on every core:
//!
//! - lamps stand along the lit roads, and the county has plenty of them;
//! - every fingerpost at a fork, every signpost and every milestone reads, and names real places,
//!   winds and distances;
//! - nothing the country put down stands on a road or in a set place's box, and no unit on solid
//!   ground;
//! - nothing that bites stands within the first walk's safe band, in a haven, or off its threat;
//! - the density rule, per screen of `jane_core::view`: nearly every screen of reachable land has
//!   something on it, every screen of the first walk does, and something is always in view;
//! - the harshness floors: every threat band is populated, the Works thicker than the Lowfields,
//!   a road thinner than the field beside it;
//! - the Factory's relay runs, the small places' marks and made ground, the places' footprints;
//! - the same seed builds the same county.
//!
//! The tables are printed (`cargo test -- --nocapture`); the expectations under them are floors.

mod common;

use std::sync::OnceLock;

use jane_core::action::Action;
use jane_core::search::{Conn, Reach, UNREACHED, flood};
use jane_core::tile::F_SOLID;
use jane_core::view::{VIEW_H_CELLS, VIEW_W_CELLS};
use jane_core::{Blueprint, Grid, Key, ListRef, TextRef, Tile};
use jane_data::Faction;
use jane_world::county::country::{FIRST_CLEAR, Kind, dist};
use jane_world::county::{County, STAGES, build_county_on, county_skeleton};
use jane_world::skeleton::{MACRO, ROAD, SKEL_H, SKEL_W, Skeleton, Water};

/// Cells from a road's line that count as its edge, where the night shift walks.
const NIGHT_EDGE: u8 = 16;
const SW: i32 = VIEW_W_CELLS as i32;
const SH: i32 = VIEW_H_CELLS as i32;
const MINOR: [&str; 4] = ["herb", "rock", "lamp_post", "lamp_run"];
const WINDS: [&str; 8] = ["NORTH", "SOUTH", "EAST", "WEST", "NORTH-EAST", "NORTH-WEST", "SOUTH-EAST", "SOUTH-WEST"];

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

/// What one seed's county came to: the numbers the floors are asked of, and each check's
/// complaints.
#[derive(Clone, Debug, Default)]
struct Survey {
    seed: u32,
    land: u32,
    empty: u32,
    first_screens: u32,
    first_empty: u32,
    stretches: u32,
    in_view: u32,
    props: usize,
    hostile: u32,
    folk: u32,
    lamps: u32,
    forks: u32,
    places: usize,
    /// Macro cells (dry) and wildlife, by threat.
    macros: [u32; 7],
    wild: [u32; 7],
    road_macros: u32,
    on_road: u32,
    /// The night shift (`nightOnly` rows): all of it, and what stands at a road's edge.
    night: u32,
    night_edge: u32,
    missing: Vec<&'static str>,
    kinds: Vec<(Kind, u32)>,
    bad: Vec<String>,
}

fn surveys() -> &'static [Survey] {
    static ALL: OnceLock<Vec<Survey>> = OnceLock::new();
    ALL.get_or_init(|| {
        let seeds = seed_list();
        let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let per = seeds.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            let jobs: Vec<_> = seeds
                .chunks(per)
                .map(|part| s.spawn(move || part.iter().map(|&seed| survey(seed)).collect::<Vec<_>>()))
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a county builds")).collect()
        })
    })
}

fn text_of(bp: &Blueprint, t: TextRef) -> String {
    match t {
        TextRef::Text(id) => jane_data::catalog().text(id).to_owned(),
        TextRef::Local(_) => bp.text(t).unwrap_or_default().to_owned(),
    }
}

/// The words a prop reads out, if it reads.
fn words(bp: &Blueprint, list: Option<ListRef>) -> Option<String> {
    let ListRef::Blueprint(i) = list? else { return None };
    bp.lists[usize::from(i)].iter().find_map(|a| match a {
        Action::Read(t) => Some(text_of(bp, *t)),
        _ => None,
    })
}

fn is_distance(s: &str) -> bool {
    if let Some(m) = s.strip_suffix(" m") {
        return m.parse::<u32>().is_ok_and(|m| m >= 50 && m % 50 == 0);
    }
    s.strip_suffix(" km").is_some_and(|k| {
        let mut p = k.split('.');
        matches!((p.next(), p.next(), p.next()), (Some(a), Some(b), None) if a.parse::<u32>().is_ok() && b.len() == 1)
    })
}

fn survey(seed: u32) -> Survey {
    let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
    let mut c = County::new(&sk, 0);
    let (mut first_prop, mut first_unit) = (0, 0);
    for (name, stage) in STAGES {
        if *name == "road_furniture" {
            first_prop = c.k.blueprint().props.len();
            first_unit = c.k.blueprint().units.len();
        }
        stage(&mut c);
    }
    let mut s = Survey { seed, ..Survey::default() };
    measure(&sk, &c, &mut s);
    lamps_line_the_lit_roads(&sk, &c, &mut s);
    signs_read(&sk, &c, first_prop, &mut s);
    nothing_on_a_road_or_in_a_box(&sk, &c, first_prop, first_unit, &mut s);
    the_first_walk_is_safe(&sk, &c, &mut s);
    relays_light_their_runs(&sk, &c, &mut s);
    small_places_stand(&sk, &c, &mut s);
    places_do_not_overlap(&sk, &c, &mut s);
    s
}

// --- the density rule and the harshness floors ------------------------------------------------

fn measure(sk: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let (gw, gh) = (bp.w() as i32 / SW, bp.h() as i32 / SH);
    let mut things = Grid::new(gw as u32, gh as u32, 0u32);
    let mut spots = Vec::new();
    let screen = |x: i32, y: i32| (x / SW, y / SH);
    // Land she can reach: a screen a quarter of which she can walk to from the platform, four
    // ways, over tiles alone.
    let start = Key::Name(cat.name_id("start").expect("a start name"));
    let m = bp.marks.get(&start).expect("a start mark");
    let mut reach = Reach::new();
    flood(
        bp.w(),
        bp.h(),
        &[(i32::from(m.cell.x), i32::from(m.cell.y))],
        Conn::Four,
        u32::MAX,
        |x, y| bp.tiles.read(x, y, Tile::Void).flags() & F_SOLID == 0,
        &mut reach,
    );
    for p in &bp.props {
        if MINOR.contains(&cat.story.prop(p.def).id) {
            continue;
        }
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        let (sx, sy) = screen(x, y);
        things.set(sx, sy, things.read(sx, sy, 0) + 1);
        spots.push((x, y));
    }
    // Wildlife nobody can reach is dropped by stage 9 (`drop_unreachable`): leave it out here.
    let alive = |u: &&jane_core::blueprint::UnitSpawn| {
        u.phase == 0 || reach.dist(i32::from(u.cell.x), i32::from(u.cell.y)) != UNREACHED
    };
    // The night shift is not in the world from six to nine: it fills no screen by day, and it is
    // counted on its own (`after_the_bell_the_night_shift_is_out`).
    let by_day =
        |u: &&jane_core::blueprint::UnitSpawn| !(cat.combat.unit(u.def).night_only && matches!(u.key, Key::Local(_)));
    for u in bp.units.iter().filter(alive).filter(|u| !by_day(u)) {
        s.night += 1;
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        if dist(&c.country.d_road, x, y) <= NIGHT_EDGE {
            s.night_edge += 1;
        }
        let def = cat.combat.unit(u.def);
        if !def.shuns_light {
            s.bad.push(format!("seed {}: a {} out at night that walks into the lamps", s.seed, def.id));
        }
    }
    for u in bp.units.iter().filter(alive).filter(by_day) {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        let (sx, sy) = screen(x, y);
        things.set(sx, sy, things.read(sx, sy, 0) + 1);
        spots.push((x, y));
        if cat.combat.unit(u.def).faction == Faction::Friendly {
            s.folk += 1;
        } else {
            s.hostile += 1;
        }
    }
    s.props = bp.props.len();
    // A small place the story needs gets its things from the placement rows, which are another
    // stage's: until they land, its mark stands in for them.
    for a in &sk.anchors {
        if let Some(m) = bp.marks.get(&Key::Name(cat.county.anchor(a.row).id)) {
            let (x, y) = (i32::from(m.cell.x), i32::from(m.cell.y));
            let (sx, sy) = screen(x, y);
            things.set(sx, sy, things.read(sx, sy, 0) + 1);
            spots.push((x, y));
        }
    }
    for sy in 0..gh {
        for sx in 0..gw {
            let mut n = 0;
            for y in (sy * SH..sy * SH + SH).step_by(3) {
                for x in (sx * SW..sx * SW + SW).step_by(3) {
                    n += u32::from(reach.dist(x, y) != UNREACHED);
                }
            }
            // A quarter of the samples: (SW / 3) * (SH / 3) / 4.
            if 4 * n < ((SW / 3) * (SH / 3)) as u32 {
                continue;
            }
            s.land += 1;
            if things.read(sx, sy, 0) == 0 {
                s.empty += 1;
            }
        }
    }
    // The first walk: every screen its two roads cross, and whether something stands within a
    // screen of each stretch. A screen that is mostly a set place is the place's own business.
    let first = sk.named.first_walk();
    let mut walk_screens: Vec<(i32, i32)> = Vec::new();
    for r in sk.roads.iter().filter(|r| first.contains(&(r.from, r.to))) {
        for &(mx, my) in &r.cells {
            let (x, y) = (mx * MACRO + MACRO / 2, my * MACRO + MACRO / 2);
            if !walk_screens.contains(&screen(x, y)) {
                walk_screens.push(screen(x, y));
            }
            s.stretches += 1;
            if spots.iter().any(|&(px, py)| (px - x).abs() <= SW / 2 + 6 && 2 * (py - y).abs() <= SH + 8) {
                s.in_view += 1;
            }
        }
    }
    let in_site = |(sx, sy): (i32, i32)| {
        let (x, y) = (sx * SW + SW / 2, sy * SH + SH / 2);
        bp.rects.iter().any(|(k, r)| c.k.local_name(*k).is_some_and(|n| n.starts_with("site_")) && r.contains(x, y))
    };
    s.first_screens = walk_screens.len() as u32;
    s.first_empty = walk_screens
        .iter()
        .filter(|&&(sx, sy)| sx < gw && sy < gh && things.read(sx, sy, 0) == 0 && !in_site((sx, sy)))
        .count() as u32;
    // Wildlife by threat: a spawn with a phase is one the ground placed.
    for my in 0..SKEL_H {
        for mx in 0..SKEL_W {
            if sk.terrain.water.read(mx, my, Water::Dry) != Water::Dry {
                continue;
            }
            let t = sk.threat.read(mx, my, 0);
            s.macros[usize::from(t)] += 1;
            if t > 0 && sk.road.read(mx, my, 0) & ROAD != 0 {
                s.road_macros += 1;
            }
        }
    }
    for u in bp.units.iter().filter(|u| u.phase > 0).filter(alive).filter(by_day) {
        s.wild[usize::from(u.phase.min(6))] += 1;
        let (mx, my) = ((i32::from(u.cell.x) >> 4).min(SKEL_W - 1), (i32::from(u.cell.y) >> 4).min(SKEL_H - 1));
        if sk.road.read(mx, my, 0) & ROAD != 0 {
            s.on_road += 1;
        }
    }
    for def in ["cottage_thatch", "farmhouse", "barn", "well", "fingerpost", "milestone", "tent", "den"] {
        let id = cat.story.prop_id(def).expect("a prop row");
        if !bp.props.iter().any(|p| p.def == id) {
            s.missing.push(def);
        }
    }
    s.places = c.places.len();
    for p in &c.places {
        match s.kinds.iter_mut().find(|(k, _)| *k == p.kind) {
            Some((_, n)) => *n += 1,
            None => s.kinds.push((p.kind, 1)),
        }
    }
}

// --- the roads --------------------------------------------------------------------------------

/// Every lit stretch of a road (outside the set places' ground) has a lamp near it: at least one
/// lamp for every three lamp steps of lit road, standing within eight cells of the lit line.
fn lamps_line_the_lit_roads(sk: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let lamp = cat.story.prop_id("lamp_post").expect("a lamp row");
    // The east road's lamps are rows of their own (`tuning/country.json`), and lamps all the same.
    let east = cat.county.furnishing.east_road.map_or([lamp; 2], |e| [e.lamp, e.bridge_lamp]);
    let lamps_of = [lamp, east[0], east[1]];
    let lamps: Vec<(i32, i32)> = bp
        .props
        .iter()
        .filter(|p| lamps_of.contains(&p.def))
        .map(|p| (i32::from(p.cell.x), i32::from(p.cell.y)))
        .collect();
    s.lamps = lamps.len() as u32;
    for n in 0..sk.roads.len() {
        let (line, lit) = (&c.lines[n], &c.lit[n]);
        let open: Vec<(i32, i32)> = line
            .iter()
            .zip(lit)
            .filter(|&(&(x, y), &l)| l && !c.chunks.iter().any(|ch| ch.bounds.grow(12).contains(x, y)))
            .map(|(&p, _)| p)
            .collect();
        let near = lamps
            .iter()
            .filter(|&&(lx, ly)| open.iter().any(|&(x, y)| (lx - x).pow(2) + (ly - y).pow(2) <= 64))
            .count();
        let want = open.len() / (3 * jane_world::county::country::roads::LAMP_STEP as usize);
        if near < want {
            s.bad.push(format!("seed {}: road {n} has {} lit points and {near} lamps by them", s.seed, open.len()));
        }
    }
}

/// Every fingerpost, signpost and milestone reads, and what it says is real: a site's name, a wind,
/// a distance. A fork's post names two ways or more.
fn signs_read(sk: &Skeleton, c: &County<'_>, first_prop: usize, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let names: Vec<String> = sk.sites.iter().map(|x| cat.text(x.def.name).to_uppercase()).collect();
    let id = |d: &str| cat.story.prop_id(d).expect("a prop row");
    let (finger, sign, mile) = (id("fingerpost"), id("signpost"), id("milestone"));
    // The builder's own signs, named by where they stand; a placement row's sign says what its
    // row says.
    // A story's footpath's post names the story's place (`county::stories`).
    let places: Vec<String> =
        cat.county.stories.iter().map(|x| jane_world::names::story_name(sk.seed, x.id).to_uppercase()).collect();
    // A place's arm on a post (`county::stories::posts`): a story's place, a site, or one of the
    // places the quests name that is no site (the Hoar Stone over the burial, the Company's grate);
    // which way, how far.
    let arm = |a: &str| {
        let bits: Vec<&str> = a.split(", ").collect();
        let named = |n: &str| {
            places.iter().chain(&names).any(|m| m == n) || ["THE HOAR STONE", "THE COMPANY'S GRATE"].contains(&n)
        };
        bits.len() == 3 && named(bits[0]) && WINDS.contains(&bits[1]) && is_distance(bits[2])
    };
    for p in bp.props[first_prop..].iter().filter(|p| matches!(p.key, Key::Local(_))) {
        if c.k.local_name(p.key).is_some_and(|n| n.starts_with("story_post_")) {
            let text = words(bp, p.use_list).unwrap_or_default();
            // "FOOTPATH. NAME, WIND, 150 m."
            let ok = text.strip_prefix("FOOTPATH. ").and_then(|t| t.strip_suffix('.')).is_some_and(|t| {
                let bits: Vec<&str> = t.split(", ").collect();
                bits.len() == 3
                    && places.iter().any(|m| m == bits[0])
                    && WINDS.contains(&bits[1])
                    && is_distance(bits[2])
            });
            if !ok {
                s.bad.push(format!("seed {}: a story's fingerpost reads {text:?}", s.seed));
            }
            continue;
        }
        // A post to a place (`county::stories`): for each place it names (one to three), the
        // place's name, which way, how far.
        if c.k.local_name(p.key).is_some_and(|n| n.starts_with("place_post_")) {
            let text = words(bp, p.use_list).unwrap_or_default();
            let arms: Vec<&str> = text.strip_suffix('.').unwrap_or(&text).split(". ").collect();
            let ok = (1..=3).contains(&arms.len()) && arms.iter().all(|a| arm(a));
            if !ok {
                s.bad.push(format!("seed {}: a post to a place reads {text:?}", s.seed));
            }
            continue;
        }
        // The lighting notice at Pell's stone: each numbered lamp, which way, how far (to ten metres).
        if c.k.local_name(p.key).is_some_and(|n| n.starts_with("lighting_notice_")) {
            let text = words(bp, p.use_list).unwrap_or_default();
            let body = text
                .strip_prefix("COUNTY LIGHTING. ")
                .and_then(|t| t.strip_suffix(" THERE IS NO LAMP 14 ON THIS ROAD."));
            let ok = body.is_some_and(|b| {
                let lamps: Vec<&str> = b.strip_suffix('.').unwrap_or(b).split(". ").collect();
                lamps.len() == 3
                    && lamps.iter().zip(["LAMP 12", "LAMP 13", "LAMP 15"]).all(|(l, n)| {
                        let bits: Vec<&str> = l.split(", ").collect();
                        bits.len() == 3
                            && bits[0] == n
                            && WINDS.contains(&bits[1])
                            && bits[2]
                                .strip_suffix(" m")
                                .and_then(|m| m.parse::<u32>().ok())
                                .is_some_and(|m| m >= 10 && m % 10 == 0)
                    })
            });
            if !ok {
                s.bad.push(format!("seed {}: the lighting notice reads {text:?}", s.seed));
            }
            continue;
        }
        let what = if p.def == finger {
            "fingerpost"
        } else if p.def == sign {
            "signpost"
        } else if p.def == mile {
            "milestone"
        } else {
            continue;
        };
        let at = (p.cell.x, p.cell.y);
        let Some(text) = words(bp, p.use_list) else {
            s.bad.push(format!("seed {}: a {what} at {at:?} says nothing", s.seed));
            continue;
        };
        let body = text.strip_suffix('.').unwrap_or(&text);
        let ok = match what {
            "milestone" => body.strip_prefix("CASTLE ").is_some_and(is_distance),
            "signpost" => body.split(". ").all(|part| {
                let bits: Vec<&str> = part.split(", ").collect();
                bits.len() == 3
                    && names.iter().any(|n| n == bits[0])
                    && WINDS.contains(&bits[1])
                    && is_distance(bits[2])
            }),
            _ => {
                s.forks += 1;
                let ways: Vec<&str> = body.split(". ").collect();
                // Its ways, then an arm for a place or two nearby (`county::stories::posts`).
                let roads = ways.iter().take_while(|w| w.split_once(": ").is_some_and(|(d, _)| WINDS.contains(&d)));
                roads.clone().count() >= 2
                    && ways.iter().skip(roads.clone().count()).all(|a| arm(a))
                    && roads.clone().all(|w| {
                        let Some((wind, list)) = w.split_once(": ") else { return false };
                        WINDS.contains(&wind)
                            && list.split("; ").all(|x| {
                                x.rsplit_once(", ").is_some_and(|(n, d)| names.iter().any(|m| m == n) && is_distance(d))
                            })
                    })
            }
        };
        if !ok {
            s.bad.push(format!("seed {}: the {what} at {at:?} reads {text:?}", s.seed));
        }
    }
}

/// Nothing the country put down stands on a road's metal, a bridge or in a set place's box, and no
/// unit of its stands on solid ground.
fn nothing_on_a_road_or_in_a_box(_: &Skeleton, c: &County<'_>, first_prop: usize, first_unit: usize, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let in_box =
        |x: i32, y: i32| c.chunks.iter().find(|ch| ch.bounds.contains(x, y)).map(jane_world::county::Chunk::id);
    // What the builder put down, named by where it stands: a placement row stands where its row says.
    for p in bp.props[first_prop..].iter().filter(|p| matches!(p.key, Key::Local(_))) {
        let row = cat.story.prop(p.def);
        let (x0, y0) = (i32::from(p.cell.x), i32::from(p.cell.y));
        for y in y0..y0 + i32::from(row.h) {
            for x in x0..x0 + i32::from(row.w) {
                let t = bp.tiles.read(x, y, Tile::Void);
                if matches!(t, Tile::Road | Tile::Boardwalk) {
                    s.bad.push(format!("seed {}: a {} at ({x0}, {y0}) stands on {t:?}", s.seed, row.id));
                }
                if let Some(ch) = in_box(x, y) {
                    s.bad.push(format!("seed {}: a {} at ({x0}, {y0}) stands in {ch}'s box", s.seed, row.id));
                }
            }
        }
    }
    for u in bp.units[first_unit..].iter().filter(|u| matches!(u.key, Key::Local(_))) {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        let id = cat.combat.unit(u.def).id;
        if bp.tiles.read(x, y, Tile::Void).flags() & F_SOLID != 0 {
            s.bad.push(format!("seed {}: a {id} at ({x}, {y}) stands on solid ground", s.seed));
        }
        if let Some(ch) = in_box(x, y) {
            s.bad.push(format!("seed {}: a {id} at ({x}, {y}) stands in {ch}'s box", s.seed));
        }
    }
}

/// Nothing that bites within the first walk's safe band or in a haven, and every one of them
/// plays at the threat of the ground it stands on.
fn the_first_walk_is_safe(sk: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let bp = c.k.blueprint();
    // The ground's own, named by where they stand: a placement row's unit is its row's business.
    for u in bp.units.iter().filter(|u| u.phase > 0 && matches!(u.key, Key::Local(_))) {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        let t = sk.threat.read(x >> 4, y >> 4, 0);
        if t == 0 || u.phase != t {
            s.bad.push(format!("seed {}: phase {} at ({x}, {y}) on threat {t}", s.seed, u.phase));
        }
        let d = dist(&c.country.d_first, x, y);
        if d < FIRST_CLEAR {
            s.bad.push(format!("seed {}: something that bites {d} cells from the first walk at ({x}, {y})", s.seed));
        }
    }
    // Julie's yard (WORLD.md §4.1): seven short-sighted bones, the two in the yard and five outside
    // the fence, none on a road's metal.
    let yard = jane_data::catalog().combat.unit_id("yard_bones").expect("the yard's row");
    let bones: Vec<_> = bp.units.iter().filter(|u| u.def == yard).collect();
    if bones.len() != 7 {
        s.bad.push(format!("seed {}: {} of the yard's bones stand, not 7", s.seed, bones.len()));
    }
    for u in bones {
        let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
        let in_yard = c.chunks.iter().any(|ch| ch.bounds.contains(x, y));
        if !in_yard && dist(&c.country.d_road, x, y) < 8 {
            s.bad.push(format!("seed {}: the yard's bones by a road at ({x}, {y})", s.seed));
        }
    }
}

/// The longest dark roads get a relay box and a run of dead lamps, and sparking the box lights
/// them: every lamp it switches is out there, dark, and it sets its `lamps_<n>` flag.
fn relays_light_their_runs(_: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let id = |d: &str| cat.story.prop_id(d).expect("a prop row");
    let (relay, run) = (id("relay_box"), id("lamp_run"));
    let boxes: Vec<_> = bp.props.iter().filter(|p| p.def == relay).collect();
    let lamps = bp.props.iter().filter(|p| p.def == run).count();
    if boxes.is_empty() || lamps < 3 * boxes.len() {
        s.bad.push(format!("seed {}: {} relay boxes and {lamps} dead lamps", s.seed, boxes.len()));
    }
    for b in boxes {
        let Some(ListRef::Blueprint(i)) = b.use_list else {
            s.bad.push(format!("seed {}: a relay box does nothing", s.seed));
            continue;
        };
        let acts = &bp.lists[usize::from(i)];
        let switched: Vec<Key> = acts
            .iter()
            .filter_map(|a| match a {
                Action::Switch { prop, .. } => Some(*prop),
                _ => None,
            })
            .collect();
        let flags = acts.iter().any(|a| {
            matches!(a, Action::Flag { key: jane_core::FlagKey::Named(k), .. } if c.k.local_name(*k).is_some_and(|n| n.starts_with("lamps_")))
        });
        if switched.len() < 3 || !flags {
            s.bad.push(format!("seed {}: a relay box switches {} lamps, flag {flags}", s.seed, switched.len()));
        }
        for k in switched {
            if !bp.props.iter().any(|p| p.key == k && p.def == run && !p.on) {
                s.bad.push(format!(
                    "seed {}: a relay box switches {:?}, which is not a dark lamp",
                    s.seed,
                    c.k.local_name(k)
                ));
            }
        }
    }
}

/// Every small place the story needs has its mark and its rect, and the four the side quests name
/// stand on made ground, not a prop in a field.
fn small_places_stand(sk: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    for a in &sk.anchors {
        let key = Key::Name(cat.county.anchor(a.row).id);
        if !bp.marks.contains_key(&key) || !bp.rects.contains_key(&key) {
            s.bad.push(format!(
                "seed {}: anchor {} has no mark or rect",
                s.seed,
                cat.name(cat.county.anchor(a.row).id)
            ));
        }
    }
    let made = [Tile::Cobble, Tile::Dirt, Tile::Garden, Tile::Track, Tile::FloorWood, Tile::Rubble];
    for id in ["halt_well", "halt_signpost", "halt_cart", "carters_cart"] {
        let Some(name) = cat.name_id(id) else { continue };
        let Some(m) = bp.marks.get(&Key::Name(name)) else {
            s.bad.push(format!("seed {}: no {id}", s.seed));
            continue;
        };
        let (mx, my) = (i32::from(m.cell.x), i32::from(m.cell.y));
        let ground = (-7..=4)
            .flat_map(|j| (-6..=6).map(move |i| (mx + i, my + j)))
            .filter(|&(x, y)| made.contains(&bp.tiles.read(x, y, Tile::Void)))
            .count();
        // The TypeScript's floor is 24 cells, on two seeds; a road or water through the pad takes
        // a few more on a seed in a couple of hundred.
        if ground <= 16 {
            s.bad.push(format!("seed {}: {id} is on {ground} cells of made ground", s.seed));
        }
    }
}

/// No two of the country's places overlap, and each one's footprint is claimed.
fn places_do_not_overlap(_: &Skeleton, c: &County<'_>, s: &mut Survey) {
    let mut boxes: Vec<_> = c.places.iter().map(|p| p.bounds).collect();
    boxes.sort_by_key(|b| (b.x, b.y));
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            if b.x >= a.x + a.w {
                break;
            }
            if a.overlaps(*b) {
                s.bad.push(format!("seed {}: places {a:?} and {b:?} overlap", s.seed));
            }
        }
        if !c.k.is_claimed(a.x, a.y) || !c.k.is_claimed(a.x + a.w - 1, a.y + a.h - 1) {
            s.bad.push(format!("seed {}: place {a:?} is not claimed", s.seed));
        }
    }
}

// --- the tests --------------------------------------------------------------------------------

fn all_bad() -> Vec<&'static String> {
    surveys().iter().flat_map(|s| &s.bad).collect()
}

fn assert_clean(what: &str) {
    let bad: Vec<_> = all_bad().into_iter().filter(|b| b.contains(what)).collect();
    assert!(bad.is_empty(), "{} failures, the first: {:#?}", bad.len(), &bad[..bad.len().min(8)]);
}

#[test]
fn lamps_stand_along_the_lit_roads() {
    assert_clean("lamps by them");
    for s in surveys() {
        assert!(s.lamps > 60, "seed {}: {} lamps", s.seed, s.lamps);
    }
}

#[test]
fn every_sign_reads_and_names_real_places() {
    for what in ["says nothing", "reads"] {
        assert_clean(what);
    }
    let forks: u32 = surveys().iter().map(|s| s.forks).sum();
    assert!(forks as usize >= surveys().len(), "{forks} fork fingerposts over {} seeds", surveys().len());
}

#[test]
fn nothing_stands_on_a_road_or_in_a_box() {
    for what in ["stands on", "'s box", "solid ground"] {
        assert_clean(what);
    }
}

#[test]
fn nothing_bites_near_the_first_walk_or_in_a_haven() {
    for what in ["from the first walk", "phase", "the yard's bones"] {
        assert_clean(what);
    }
}

/// After the bell (WORLD.md §4.3): on every seed the night shift is out on the unlit roads' edges
/// and the rough ground, and nowhere by day; every one of it shuns the lamps, so a lit road is
/// the safe line; none of it is near the first walk (`nothing_bites_near_the_first_walk...`
/// holds it to that with the rest).
#[test]
fn after_the_bell_the_night_shift_is_out() {
    assert_clean("walks into the lamps");
    let rows: Vec<String> =
        surveys().iter().map(|s| format!("{:>11}{:>7}{:>7}", s.seed, s.night, s.night_edge)).collect();
    println!("{:>11}{:>7}{:>7}\n{}", "seed", "night", "edge", rows.join("\n"));
    for s in surveys() {
        assert!(s.night >= 40, "seed {}: {} out after the bell", s.seed, s.night);
        assert!(s.night_edge >= 5, "seed {}: {} of {} at a road's edge", s.seed, s.night_edge, s.night);
        let wild: u32 = s.wild.iter().sum();
        assert!(10 * s.night < 3 * wild, "seed {}: {} at night against {wild} by day", s.seed, s.night);
    }
}

#[test]
fn the_longest_dark_roads_get_relay_runs() {
    assert_clean("relay box");
}

#[test]
fn small_places_have_their_marks_and_their_ground() {
    for what in ["anchor", "made ground", ": no "] {
        assert_clean(what);
    }
}

#[test]
fn places_never_overlap() {
    assert_clean("overlap");
    assert_clean("is not claimed");
}

fn pct(n: u32, d: u32) -> u32 {
    100 * n / d.max(1)
}

#[test]
fn the_county_is_full() {
    let mut lines = vec![format!(
        "{:>11}{:>8}{:>6}{:>7}{:>9}{:>12}{:>10}{:>8}{:>9}{:>7}{:>8}",
        "seed", "land", "empty", "empty%", "walk scr", "walk empty", "in view%", "props", "hostile", "folk", "places"
    )];
    for s in surveys() {
        lines.push(format!(
            "{:>11}{:>8}{:>6}{:>7}{:>9}{:>12}{:>10}{:>8}{:>9}{:>7}{:>8}",
            s.seed,
            s.land,
            s.empty,
            pct(s.empty, s.land),
            s.first_screens,
            s.first_empty,
            pct(s.in_view, s.stretches),
            s.props,
            s.hostile,
            s.folk,
            s.places
        ));
    }
    println!("per-screen content, {SW} x {SH} cells a screen\n{}", lines.join("\n"));
    if let Some(s) = surveys().first() {
        let kinds: Vec<String> = s.kinds.iter().map(|(k, n)| format!("{} {n}", k.name())).collect();
        println!("seed {}, places by kind: {}", s.seed, kinds.join(", "));
    }
    // The TypeScript held "in view" above 95 % on each of its two seeds; its own county comes to
    // 98 % on seed 11. Over the sweep: above 95 % in all, and above 90 % on every seed.
    let (seen, stretches): (u32, u32) = surveys().iter().fold((0, 0), |(a, b), s| (a + s.in_view, b + s.stretches));
    assert!(100 * seen > 95 * stretches, "{seen} of {stretches} stretches of the first walk in view");
    let (hostile, land): (u32, u32) = surveys().iter().fold((0, 0), |(a, b), s| (a + s.hostile, b + s.land));
    assert!(10 * hostile > 7 * land, "{hostile} things that bite on {land} screens of land");
    for s in surveys() {
        assert!(10 * s.empty < s.land, "seed {}: {} of {} reachable land screens are empty", s.seed, s.empty, s.land);
        assert_eq!(s.first_empty, 0, "seed {}: screens of the first walk with nothing on them", s.seed);
        assert!(
            100 * s.in_view > 90 * s.stretches,
            "seed {}: {} of {} stretches in view",
            s.seed,
            s.in_view,
            s.stretches
        );
        assert!(s.folk > 150, "seed {}: {} folk", s.seed, s.folk);
        // Per screen of land: more than 0.6 things that bite on every seed (0.7 over the sweep).
        assert!(10 * s.hostile > 6 * s.land, "seed {}: {} hostile on {} screens", s.seed, s.hostile, s.land);
    }
    // Every seed has a cottage, a farm, a barn, a well, a fingerpost, a milestone, a tent and a den,
    // but one in a couple of hundred whose Lowfields are nearly all wood and foothill, with no
    // open roadside a farm fits (seed 1022145915 has none).
    let short: Vec<_> = surveys().iter().filter(|s| !s.missing.is_empty()).map(|s| (s.seed, &s.missing)).collect();
    assert!(100 * short.len() <= surveys().len(), "{short:?}");
}

#[test]
fn the_county_is_populated_by_threat() {
    // Densities per square kilometre: a macro cell is 16 m square, 256 m².
    let per_km2 = |n: u32, macros: u32| u64::from(n) * 1_000_000 / (u64::from(macros.max(1)) * 256);
    let mut lines = vec![format!("{:>11}{:>7}{:>8}{:>10}{:>9}", "seed", "threat", "macros", "creatures", "per km2")];
    for s in surveys() {
        for t in 1..7 {
            if s.macros[t] > 0 {
                lines.push(format!(
                    "{:>11}{t:>7}{:>8}{:>10}{:>9}",
                    s.seed,
                    s.macros[t],
                    s.wild[t],
                    per_km2(s.wild[t], s.macros[t])
                ));
            }
        }
    }
    println!("wildlife by threat\n{}", lines.join("\n"));
    // The TypeScript's floors, held on its two seeds (the Works 1.6 times as thick as the
    // Lowfields, and under 900 a km²), are held over the whole sweep; each seed is held to a
    // looser pair, because the TypeScript's own county breaks the tight one on seeds it never
    // tested (944 a km² on seed 6, 928 on seed 7).
    let sum = |f: fn(&Survey) -> u32| surveys().iter().map(f).sum::<u32>();
    let near = per_km2(sum(|s| s.wild[1]), sum(|s| s.macros[1]));
    let far = per_km2(sum(|s| s.wild[4] + s.wild[5] + s.wild[6]), sum(|s| s.macros[4] + s.macros[5] + s.macros[6]));
    println!("over the sweep: {near} a km² at threat 1, {far} at threat 4 to 6");
    assert!(10 * far > 16 * near, "the Works ({far}) no thicker than the Lowfields ({near})");
    assert!(far < 900, "{far} a km² at threat 4 to 6");
    for s in surveys() {
        let wild: u32 = s.wild.iter().sum();
        assert!((2000..5000).contains(&wild), "seed {}: {wild} wild", s.seed);
        for t in 1..7 {
            assert!(s.macros[t] == 0 || s.wild[t] > 0, "seed {} threat {t} is empty", s.seed);
        }
        // The ground under a road carries well under the wildlife the ground off it does.
        let off_macros: u32 = s.macros[1..].iter().sum::<u32>() - s.road_macros;
        let road = u64::from(s.on_road) * u64::from(off_macros);
        let off = u64::from(wild - s.on_road) * u64::from(s.road_macros);
        assert!(10 * road < 6 * off, "seed {}: a road is no safer than the field", s.seed);
        let near = per_km2(s.wild[1], s.macros[1]);
        let far = per_km2(s.wild[4] + s.wild[5] + s.wild[6], s.macros[4] + s.macros[5] + s.macros[6]);
        assert!(10 * far > 14 * near, "seed {}: the Works ({far}) no thicker than the Lowfields ({near})", s.seed);
        assert!(near > 40, "seed {}: {near} a km² at threat 1", s.seed);
        assert!(far < 1000, "seed {}: {far} a km² at threat 4 to 6", s.seed);
    }
}

#[test]
fn the_same_seed_builds_the_same_county() {
    for seed in seed_list().into_iter().take(2) {
        let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
        assert!(build_county_on(&sk, 0) == build_county_on(&sk, 0), "seed {seed}");
    }
}
