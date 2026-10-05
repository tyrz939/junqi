//! The way to each quest step, read off the county (`jane_sim::route`), is true to the world
//! (QUEST-TREE.md §2): it leaves Castle by a road that is there, every fingerpost it names stands
//! by the way and says what the words say it says, the turns come in the order the walk meets
//! them along road she can walk, and it ends where the place is.

mod common;

use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, QuestId, ZoneId};
use jane_sim::Blueprints;
use jane_sim::route::{Leg, Roads, Route, Site, Start, place_of_step, road, sign};

/// Can her feet stand on `(x, y)`: no terrain, no solid prop's cells.
fn open(bp: &Blueprint, solid: &[bool], x: i32, y: i32) -> bool {
    let w = bp.tiles.w() as i32;
    bp.tiles.inside(x, y)
        && bp.tiles.read(x, y, jane_core::Tile::Void).flags() & F_SOLID == 0
        && !solid[(y * w + x) as usize]
}

fn solid_props(bp: &Blueprint) -> Vec<bool> {
    let cat = jane_data::catalog();
    let w = bp.tiles.w() as i32;
    let mut out = vec![false; (bp.tiles.w() * bp.tiles.h()) as usize];
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        if d.solid && !p.hidden && (!d.gate || p.locked) {
            for (x, y) in d.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).cells() {
                if bp.tiles.inside(x, y) {
                    out[(y * w + x) as usize] = true;
                }
            }
        }
    }
    out
}

/// Every claim a route makes, checked against the county; the faults.
fn faults(bp: &Blueprint, solid: &[bool], roads: &Roads, r: &Route, what: &str) -> Vec<String> {
    let cat = jane_data::catalog();
    let fingerpost = cat.story.prop_id("fingerpost").unwrap();
    let mut bad = Vec::new();
    let words = r.words();
    for bit in ["Objective", "objective", "Go to the", "quest"] {
        if words.contains(bit) {
            bad.push(format!("{what}: says {bit:?}: {words}"));
        }
    }
    if let [Leg::InTown { from, .. }] = r.legs[..] {
        if !roads.town().contains(from.0, from.1) {
            bad.push(format!("{what}: the square is not in Castle"));
        }
        if !roads.town().contains(r.from.at().0, r.from.at().1) {
            bad.push(format!("{what}: in Castle, from {:?} outside it", r.from));
        }
        return bad;
    }
    // It says where it starts from, and starts there.
    let from = format!("from {}", r.from.said()).replace("from the edge of ", "from ");
    if !words.to_lowercase().replace("from the edge of ", "from ").contains(&from.to_lowercase()) {
        bad.push(format!("{what}: does not say {from:?}: {words}"));
    }
    if let Some(g) = r.from.ground()
        && !g.grow(3).contains(r.from.at().0, r.from.at().1)
    {
        bad.push(format!("{what}: {:?} starts away from its ground", r.from));
    }
    // The way is road she can walk, a step at a time, from the start.
    let p = &r.path;
    if p.is_empty() || p[0] != r.from.at() || !r.to.grow(4).contains(p[p.len() - 1].0, p[p.len() - 1].1) {
        bad.push(format!("{what}: does not start at {:?} and end at the place", r.from));
    }
    for (i, &(x, y)) in p.iter().enumerate() {
        if !open(bp, solid, x, y) {
            bad.push(format!("{what}: ({x}, {y}) on the way is not open ground"));
            break;
        }
        if i > 0 {
            let (dx, dy) = (x - p[i - 1].0, y - p[i - 1].1);
            let corner = dx != 0
                && dy != 0
                && !open(bp, solid, p[i - 1].0 + dx, p[i - 1].1)
                && !open(bp, solid, p[i - 1].0, p[i - 1].1 + dy);
            if dx.abs() > 1 || dy.abs() > 1 || corner {
                bad.push(format!("{what}: the way jumps at ({x}, {y})"));
                break;
            }
        }
    }
    // The legs in the order the walk meets them; each post where it is said to be.
    let mut last = 0;
    for (k, l) in r.legs.iter().enumerate() {
        let at = match l {
            Leg::Out { at, .. } | Leg::Post { at, .. } | Leg::Off { at, .. } => *at,
            Leg::InTown { from, .. } => {
                // Last, where the way comes into Castle.
                if k + 1 != r.legs.len() || !roads.town().contains(from.0, from.1) {
                    bad.push(format!("{what}: in Castle before the way is done"));
                }
                if !p.iter().skip(last).any(|c| roads.town().contains(c.0, c.1)) {
                    bad.push(format!("{what}: says Castle, and the way never comes into it"));
                }
                continue;
            }
        };
        let Some(i) = p.iter().position(|&c| c == at) else {
            bad.push(format!("{what}: {l:?} is not on the way"));
            continue;
        };
        if i < last {
            bad.push(format!("{what}: {l:?} comes before what was said before it"));
        }
        last = i;
        if let Leg::Out { at, dir, road: by_road } = l {
            match r.from.ground() {
                Some(g) if g.contains(at.0, at.1) && i + 1 < p.len() => {
                    bad.push(format!("{what}: leaves {} inside it", r.from.said()));
                }
                Some(g) if p[..i].iter().any(|c| !g.contains(c.0, c.1)) => {
                    bad.push(format!("{what}: out of {} before it says so", r.from.said()));
                }
                None if i != 0 => bad.push(format!("{what}: does not set out from {:?}", r.from)),
                _ => {}
            }
            let j = (i + 40).min(p.len() - 1);
            let wind = jane_world::county::country::roads::compass(p[j].0 - p[i].0, p[j].1 - p[i].1).to_lowercase();
            if wind != *dir {
                bad.push(format!("{what}: out of Castle the way heads {wind}, not {dir}"));
            }
            // "Take the road": there is one, a few steps out.
            let on_foot = matches!(
                r.legs[..],
                [_, Leg::Off { metres: 0, .. }] | [Leg::Out { road: false, .. }, Leg::InTown { .. }]
            );
            let ahead = if *by_road { &p[i..=j] } else { &p[i..] };
            if !on_foot && !ahead.iter().any(|c| road(bp.tiles.read(c.0, c.1, jane_core::Tile::Void))) {
                bad.push(format!("{what}: no road out of Castle at {at:?}"));
            }
        }
        if let Leg::Post { post, dir, sign: s, .. } = l {
            let there =
                bp.props.iter().find(|q| q.def == fingerpost && (i32::from(q.cell.x), i32::from(q.cell.y)) == *post);
            let Some(there) = there else {
                bad.push(format!("{what}: no fingerpost at {post:?}"));
                continue;
            };
            if (post.0 - at.0).abs() > 8 || (post.1 - at.1).abs() > 8 {
                bad.push(format!("{what}: the fingerpost at {post:?} is not by the way"));
            }
            let read = there.use_list.and_then(|l| bp.list(l)).and_then(|l| {
                l.iter().find_map(|a| match a {
                    jane_core::action::Action::Read(t) => bp.text(*t).map(str::to_owned),
                    _ => None,
                })
            });
            if let Some(s) = s
                && read.as_deref().and_then(|w| sign(w, dir)).as_deref() != Some(s.as_str())
            {
                bad.push(format!("{what}: the post at {post:?} does not say {s} to the {dir}: {read:?}"));
            }
            // The turn is the way's own: it heads that way from the post.
            let j = (i + 40).min(p.len() - 1);
            let wind = jane_world::county::country::roads::compass(p[j].0 - p[i].0, p[j].1 - p[i].1).to_lowercase();
            if wind != *dir {
                bad.push(format!("{what}: at {post:?} the way heads {wind}, not {dir}"));
            }
        }
    }
    // It leaves the road where there is road, and says so last.
    match r.legs.last() {
        Some(Leg::Off { at, metres, .. })
            if *metres > 0
                && !road(bp.tiles.read(at.0, at.1, jane_core::Tile::Void))
                && !roads.town().contains(at.0, at.1) =>
        {
            bad.push(format!("{what}: leaves the road at {at:?}, where there is none"));
        }
        Some(Leg::Off { .. } | Leg::InTown { .. }) => {}
        _ => bad.push(format!("{what}: never leaves the road")),
    }
    bad
}

/// Every step's way from Castle; and, from every `every`th place the steps go to, the way there
/// from each named place, from a point on a road and from where she might have been asked. The
/// count of ways and the faults.
fn check(bps: &Blueprints, seed: u32, print: bool, every: usize) -> (usize, Vec<String>) {
    let cat = jane_data::catalog();
    let county = bps.get(ZoneId::County);
    let solid = solid_props(county);
    let roads = Roads::new(county).expect("the county has Castle");
    let (mut n, mut bad) = (0, Vec::new());
    let mut one = |r: Route, what: &str, bad: &mut Vec<String>| {
        if print {
            println!("{what}: {} | {}", r.words(), r.short());
        }
        n += 1;
        bad.extend(faults(county, &solid, &roads, &r, what));
    };
    let mut places: Vec<(String, Route)> = Vec::new();
    for (qi, q) in cat.story.quests.iter().enumerate() {
        for i in 0..q.requirements.len() {
            let Some(place) = place_of_step(bps, QuestId(qi as u16), i) else { continue };
            let what = format!("seed {seed}, {} step {i}", q.id);
            let Some(r) = roads.route(place.rect) else {
                bad.push(format!("{what}: no way to {:?}", place.rect));
                continue;
            };
            if !places.iter().any(|(_, p)| p.to == r.to) {
                places.push((what.clone(), r.clone()));
            }
            one(r, &what, &mut bad);
        }
    }
    let m = places.len();
    for k in (0..m).step_by(every) {
        let (what, from_castle) = &places[k];
        let to = from_castle.to;
        let mut starts: Vec<Start> = roads
            .sites()
            .iter()
            .filter(|s| !s.start.ground().is_some_and(|g| g.grow(6).overlaps(to)))
            .map(|s| s.start.clone())
            .collect();
        // Halfway along another place's way, by the road; and where a third's ends.
        let other = &places[(k + 1) % m].1.path;
        if let Some(&mid) = other.get(other.len() / 2)
            && roads.near_road(mid)
        {
            starts.push(Start::Here(mid));
        }
        if let Some(&end) = places[(k + 2) % m].1.path.last() {
            starts.push(Start::Given(end));
        }
        for s in starts {
            let what = format!("{what}, from {s:?}");
            match roads.route_from(&s, to) {
                Some(r) => one(r, &what, &mut bad),
                None => bad.push(format!("{what}: no way to {to:?}")),
            }
        }
    }
    (n, bad)
}

/// Seed 7's county, every step the words give a place for, from Castle; and from the other
/// starts to every third place.
#[test]
fn every_way_on_seed_7_is_true_to_the_county() {
    let (n, bad) = check(&common::bps(), common::SEED, std::env::var_os("JANE_ROUTE_PRINT").is_some(), 3);
    assert!(n > 150, "most steps have a way ({n})");
    assert!(bad.is_empty(), "a way the county does not bear out: {bad:#?}");
}

/// Where the way starts: where she is, by a road; else the nearest place she knows, never the
/// one she is going to; else where she was asked; else Castle.
#[test]
fn the_way_starts_from_the_most_use_of_what_she_knows() {
    let bps = common::bps();
    let roads = Roads::new(bps.get(ZoneId::County)).expect("the county has Castle");
    let site = |id: &str| roads.sites().iter().find(|s| s.id == id).expect("the site is built");
    let to = site("gold_mine").start.ground().unwrap();
    let on_road = site("station").start.at();
    assert!(roads.is_road(on_road), "the Halt's start is on its road");
    let (all, none) = (|_: &Site| true, |_: &Site| false);
    assert_eq!(roads.start(Some(on_road), None, &none, None, to), Start::Here(on_road));
    let w = bps.get(ZoneId::County).tiles.w() as i32;
    let wild = (0..w * w)
        .map(|i| (i % w, i / w))
        .find(|&(x, y)| {
            (x - on_road.0).abs() < 300 && (y - on_road.1).abs() < 300 && roads.open(x, y) && !roads.near_road((x, y))
        })
        .expect("open ground off the roads near the Halt");
    let s = roads.start(Some(wild), None, &all, None, to);
    assert!(matches!(s, Start::Place { .. }), "off the road she starts from a place she knows: {s:?}");
    assert!(!s.ground().unwrap().grow(6).overlaps(to), "not the place she is going to");
    assert_eq!(roads.start(Some(wild), None, &none, Some(on_road), to), Start::Given(on_road));
    assert_eq!(roads.start(Some(wild), None, &none, None, to), roads.castle());
    let r = roads.route_from(&Start::Here(on_road), to).expect("a way from the Halt");
    assert!(r.words().starts_with("From here, "), "{}", r.words());
    assert!(r.short().contains(" from here"), "{}", r.short());
}

/// A way set where she stood, walked partway: "here" is where she stands now. It says the road
/// out from there, and the first turn's metres count from there; the turns ahead keep their words.
#[test]
fn a_way_walked_partway_is_said_from_where_she_stands() {
    let bps = common::bps();
    let county = bps.get(ZoneId::County);
    let solid = solid_props(county);
    let roads = Roads::new(county).expect("the county has Castle");
    let on_road = roads.sites().iter().find(|s| s.id == "station").expect("the Halt").start.at();
    let mut checked = 0;
    for s in roads.sites().iter().filter(|s| s.id != "station") {
        let Some(to) = s.start.ground() else { continue };
        let Some(r) = roads.route_from(&Start::Here(on_road), to) else { continue };
        let first_turn = r.legs.iter().skip(1).find_map(|l| match l {
            Leg::Post { at, metres, .. } => Some((r.path.iter().position(|c| c == at).unwrap(), *metres)),
            _ => None,
        });
        let Some((k, m)) = first_turn else { continue };
        if k < 40 {
            continue;
        }
        let i = k / 2;
        let w = roads.walked(&r, i).expect("a way walked as far as its middle is said again");
        assert_eq!(w.from, Start::Here(r.path[i]), "from where she stands");
        assert!(w.words().starts_with("From here, "), "{}", w.words());
        let Leg::Post { metres, .. } = w.legs[1] else { panic!("the first turn is still the post: {:?}", w.legs) };
        assert!(metres < m, "the post is nearer than it was ({metres} m, was {m} m)");
        assert_eq!(w.legs.len(), r.legs.len(), "the turns ahead are the same turns");
        let bad = faults(county, &solid, &roads, &w, "walked");
        assert!(bad.is_empty(), "the walked way is true to the county: {bad:#?}");
        assert!(roads.walked(&r, k + 1).is_none(), "past the first turn the way is worked out again");
        checked += 1;
    }
    assert!(checked >= 2, "some ways from the Halt turn at a post far enough on ({checked})");
}

/// Seeds 1 to 8, from every start to every place.
#[test]
#[ignore = "slow: eight seeds built whole"]
fn every_way_on_seeds_1_to_8_is_true_to_the_county() {
    let mut all = Vec::new();
    for seed in 1..=8 {
        let bps = Blueprints::build(seed).expect("the seed builds");
        let (n, bad) = check(&bps, seed, false, 1);
        assert!(n > 300, "seed {seed}: most steps have a way ({n})");
        all.extend(bad);
    }
    assert!(all.is_empty(), "a way the county does not bear out: {all:#?}");
}
