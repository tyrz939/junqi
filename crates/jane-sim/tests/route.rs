//! The way to each quest step, read off the county (`jane_sim::route`), is true to the world
//! (QUEST-TREE.md §2): it leaves Castle by a road that is there, every fingerpost it names stands
//! by the way and says what the words say it says, the turns come in the order the walk meets
//! them along road she can walk, and it ends where the place is.

mod common;

use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, QuestId, ZoneId};
use jane_sim::Blueprints;
use jane_sim::route::{Leg, Roads, Route, place_of_step, road, sign};

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
        return bad;
    }
    // The way is road she can walk, a step at a time, from Castle.
    let p = &r.path;
    if p.is_empty()
        || !roads.town().contains(p[0].0, p[0].1)
        || !r.to.grow(4).contains(p[p.len() - 1].0, p[p.len() - 1].1)
    {
        bad.push(format!("{what}: does not start in Castle and end at the place"));
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
    for l in &r.legs {
        let at = match l {
            Leg::Out { at, .. } | Leg::Post { at, .. } | Leg::Off { at, .. } => *at,
            Leg::InTown { .. } => {
                bad.push(format!("{what}: in Castle and out of it"));
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
            if roads.town().contains(at.0, at.1) && i + 1 < p.len() {
                bad.push(format!("{what}: leaves Castle inside it"));
            }
            let j = (i + 40).min(p.len() - 1);
            let wind = jane_world::county::country::roads::compass(p[j].0 - p[i].0, p[j].1 - p[i].1).to_lowercase();
            if wind != *dir {
                bad.push(format!("{what}: out of Castle the way heads {wind}, not {dir}"));
            }
            // "Take the road": there is one, a few steps out.
            let on_foot = matches!(r.legs[..], [_, Leg::Off { metres: 0, .. }]);
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
        Some(Leg::Off { .. }) => {}
        _ => bad.push(format!("{what}: never leaves the road")),
    }
    bad
}

fn check(bps: &Blueprints, seed: u32, print: bool) -> (usize, Vec<String>) {
    let cat = jane_data::catalog();
    let county = bps.get(ZoneId::County);
    let solid = solid_props(county);
    let roads = Roads::new(county).expect("the county has Castle");
    let (mut n, mut bad) = (0, Vec::new());
    for (qi, q) in cat.story.quests.iter().enumerate() {
        for i in 0..q.requirements.len() {
            let Some(place) = place_of_step(bps, QuestId(qi as u16), i) else { continue };
            let what = format!("seed {seed}, {} step {i}", q.id);
            let Some(r) = roads.route(place.rect) else {
                bad.push(format!("{what}: no way to {:?}", place.rect));
                continue;
            };
            if print {
                println!("{what}: {} | {}", r.words(), r.short());
            }
            n += 1;
            bad.extend(faults(county, &solid, &roads, &r, &what));
        }
    }
    (n, bad)
}

/// Seed 7's county, every step the words give a place for.
#[test]
fn every_way_on_seed_7_is_true_to_the_county() {
    let (n, bad) = check(&common::bps(), common::SEED, false);
    assert!(n > 80, "most steps have a way ({n})");
    assert!(bad.is_empty(), "a way the county does not bear out: {bad:#?}");
}

/// Seeds 1 to 8.
#[test]
#[ignore = "slow: eight seeds built whole"]
fn every_way_on_seeds_1_to_8_is_true_to_the_county() {
    let mut all = Vec::new();
    for seed in 1..=8 {
        let bps = Blueprints::build(seed).expect("the seed builds");
        let (n, bad) = check(&bps, seed, false);
        assert!(n > 80, "seed {seed}: most steps have a way ({n})");
        all.extend(bad);
    }
    assert!(all.is_empty(), "a way the county does not bear out: {all:#?}");
}
