//! The softlock pass on Butterfly Forest, the Burial Chamber and the School (DUNGEONS.md §3.3,
//! §3.5 and §3.6, "Can she get stuck?"): no state she can get them into leaves them unfinishable,
//! on seeds 1 to 16.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use jane_core::action::{Action, Condition};
use jane_core::grid::Rect;
use jane_core::tile::{F_NOPUSH, F_SOLID};
use jane_core::{Blueprint, Key, NameId, Tile, ZoneId};
use jane_data::{MissionEdgeKind, catalog};
use jane_world::dungeon::checks::states;
use jane_world::dungeon::{Built, build};

const LATE: [ZoneId; 3] = [ZoneId::Forest, ZoneId::Burial, ZoneId::School];

/// Every key tag a lock in `zone` takes (as `dungeon_states.rs` reads them).
fn tags_in(zone: ZoneId) -> BTreeSet<NameId> {
    let mut out = BTreeSet::new();
    if let Some(m) = catalog().dungeons.mission_of(zone) {
        for e in m.edges {
            for k in std::iter::once(&e.kind).chain(e.also) {
                if let MissionEdgeKind::Key { tag, .. } = *k {
                    out.insert(tag);
                }
            }
        }
        out.extend(m.nodes.iter().flat_map(|n| n.holds).filter_map(|h| h.key_tag));
        return out;
    }
    if let Some(bp) = jane_world::build_zone(zone, 1) {
        out.extend(bp.props.iter().filter_map(|p| match p.key_tag {
            Some(Key::Name(t)) if p.locked => Some(t),
            _ => None,
        }));
    }
    out
}

fn all_tags() -> &'static BTreeMap<ZoneId, BTreeSet<NameId>> {
    static TAGS: OnceLock<BTreeMap<ZoneId, BTreeSet<NameId>>> = OnceLock::new();
    TAGS.get_or_init(|| ZoneId::ALL.iter().map(|&z| (z, tags_in(z))).collect())
}

/// The tags this zone's locks take that also fit a lock in another zone.
fn outside(zone: ZoneId) -> Vec<NameId> {
    let all = all_tags();
    let here = &all[&zone];
    here.iter().copied().filter(|t| all.iter().any(|(&z, tags)| z != zone && tags.contains(t))).collect()
}

fn verdict(zone: ZoneId, b: &Built) -> states::Verdict {
    let m = b.info.mission;
    let dropped = b.info.layout.as_ref().map_or(&[][..], |l| l.dropped.as_slice());
    states::search_gated(&b.blueprint, m, dropped, &outside(zone))
}

#[test]
fn no_state_of_the_forest_the_burial_or_the_school_strands_her() {
    for zone in LATE {
        for seed in 1..=16 {
            let b = build(zone, seed);
            assert!(b.info.errors.is_empty(), "{zone:?} seed {seed}: {:?}", b.info.errors);
            let v = verdict(zone, &b);
            eprintln!("{zone:?} seed {seed}: {} states", v.states);
            assert!(v.states > 10, "{zone:?} seed {seed}: only {} states searched", v.states);
            assert!(v.stranded.is_empty(), "{zone:?} seed {seed}, {} states:\n{}", v.states, v.stranded.join("\n"));
        }
    }
}

#[test]
fn the_gated_search_holds_back_what_a_verb_or_a_condition_has_not_yet_given() {
    // Without Electric the physics clock is never shown, so the rope never opens the tower: every
    // state is stranded short of it. The plain search would have had the tower on reaching the hall.
    let b = build(ZoneId::School, 1);
    let spark = catalog().combat.spell_id("spark").expect("Electric");
    let mut m = *b.info.mission;
    let verbs: Vec<_> = m.given_verbs.iter().copied().filter(|&v| v != spark).collect();
    m.given_verbs = Box::leak(verbs.into_boxed_slice());
    let v = states::search_gated(&b.blueprint, &m, &[], &[]);
    assert!(!v.stranded.is_empty() && v.stranded.len() == v.states, "{} of {}", v.stranded.len(), v.states);
    assert!(v.stranded.iter().all(|s| s.contains("tower")), "{:?}", v.stranded.first());
    assert!(states::search(&b.blueprint, &m, &[], &[]).stranded.is_empty());
}

#[test]
fn no_key_of_the_forest_the_burial_or_the_school_fits_a_lock_outside_it() {
    // The School's front-door key fits the county's front door, which is what it is for: no lock
    // in the School takes it.
    for zone in LATE {
        assert_eq!(outside(zone), Vec::<NameId>::new(), "{zone:?}");
    }
}

#[test]
fn every_door_that_drops_behind_her_remembers_it_was_lifted() {
    // A trigger that locks a way and undoes itself at a death re-arms at every death after it
    // fired. Unless it asks whether it was cleared (a flag, or its keeper dead), a death anywhere
    // after the clear stands the room up again, and the clear, which fires once, never lifts it
    // (the Snake's lock-in room, DUNGEONS.md §3.5).
    let cat = catalog();
    for zone in LATE {
        let b = build(zone, 1);
        let bp = &b.blueprint;
        let story = cat.story.triggers_in(zone).map(|(_, d)| (d.id.to_owned(), &d.trigger));
        let own = bp.triggers.iter().map(|(k, t)| (format!("{k:?}"), t));
        for (id, t) in own.chain(story) {
            let list = |r| jane_world::solve::rows::list(bp, cat, r).unwrap_or(&[]);
            if t.reset.is_none() || !list(t.actions).iter().any(|a| matches!(a, Action::Lock(_))) {
                continue;
            }
            let when = t.when.and_then(|w| jane_world::solve::rows::conds(bp, cat, w)).unwrap_or(&[]);
            let remembers = when.iter().any(|c| c.not && matches!(c.c, Condition::Flag { .. } | Condition::Dead(_)));
            assert!(remembers, "{zone:?}: {id} locks again after any death once it has been cleared");
        }
    }
}

/// Floor cells reachable from `from` over the tiles alone, with `solid` cells shut as well.
fn flood(bp: &Blueprint, from: (i32, i32), solid: &BTreeSet<(i32, i32)>) -> BTreeSet<(i32, i32)> {
    let mut seen = BTreeSet::from([from]);
    let mut q = vec![from];
    while let Some((x, y)) = q.pop() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let c = (x + dx, y + dy);
            if !bp.tiles.inside(c.0, c.1) || solid.contains(&c) || seen.contains(&c) {
                continue;
            }
            if bp.tiles.read(c.0, c.1, Tile::Void).flags() & F_SOLID != 0 {
                continue;
            }
            seen.insert(c);
            q.push(c);
        }
    }
    seen
}

#[test]
fn growing_every_hedge_she_can_grow_walls_her_off_from_nothing() {
    // Grow closes as well as opens (DUNGEONS.md §3.3): every hedge seed grown, and every bud in the
    // belfry, every room she could reach over the tiles she can still reach.
    let cat = catalog();
    for zone in LATE {
        for seed in 1..=16 {
            let b = build(zone, seed);
            let bp = &b.blueprint;
            let mut shut = BTreeSet::new();
            for p in &bp.props {
                let Some(l) = p.use_list.and_then(|r| jane_world::solve::rows::list(bp, cat, r)) else { continue };
                for a in l {
                    if let Action::Fill { rect, tile } = *a
                        && tile.flags() & F_SOLID != 0
                    {
                        let r = bp.rects.get(&rect).expect("a fill's rect");
                        shut.extend(r.cells());
                    }
                }
            }
            if shut.is_empty() {
                continue;
            }
            let entry = bp.marks.get(&Key::Name(cat.name_id("entry").expect("entry"))).expect("an entry mark");
            let from = (i32::from(entry.cell.x), i32::from(entry.cell.y));
            let before = flood(bp, from, &BTreeSet::new());
            let after = flood(bp, from, &shut);
            for r in &b.info.rooms {
                let reach = |s: &BTreeSet<(i32, i32)>| r.rect.cells().any(|c| s.contains(&c));
                assert!(
                    !reach(&before) || reach(&after),
                    "{zone:?} seed {seed}: growing every hedge shuts her out of {}",
                    b.info.mission.nodes[r.node].id
                );
            }
        }
    }
}

// --- pushables, on the cell grid ----------------------------------------------------------------

struct Room<'a> {
    bp: &'a Blueprint,
    rect: Rect,
    /// The rect and a cell round it: where she may stand (a doorway's sill is in the rim).
    around: Rect,
    /// Over `around`: a solid prop (not the pushable) stands here.
    blocked: Vec<bool>,
    size: (i32, i32),
}

impl Room<'_> {
    fn ix(&self, x: i32, y: i32) -> Option<usize> {
        self.around.contains(x, y).then(|| ((y - self.around.y) * self.around.w + x - self.around.x) as usize)
    }

    fn tile(&self, x: i32, y: i32) -> u8 {
        self.bp.tiles.read(x, y, Tile::Void).flags()
    }

    fn under(&self, x: i32, y: i32, b: (i32, i32)) -> bool {
        x >= b.0 && x < b.0 + self.size.0 && y >= b.1 && y < b.1 + self.size.1
    }

    fn walk(&self, x: i32, y: i32, b: (i32, i32)) -> bool {
        self.ix(x, y).is_some_and(|i| !self.blocked[i]) && self.tile(x, y) & F_SOLID == 0 && !self.under(x, y, b)
    }

    fn fits(&self, b: (i32, i32)) -> bool {
        (0..self.size.1).all(|j| {
            (0..self.size.0).all(|i| {
                let (x, y) = (b.0 + i, b.1 + j);
                self.rect.contains(x, y)
                    && self.tile(x, y) & (F_SOLID | F_NOPUSH) == 0
                    && self.ix(x, y).is_some_and(|i| !self.blocked[i])
            })
        })
    }

    fn labels(&self, b: (i32, i32)) -> Vec<u16> {
        let a = self.around;
        let mut out = vec![0u16; (a.w * a.h) as usize];
        let mut next = 0u16;
        for (x0, y0) in a.cells() {
            let i = self.ix(x0, y0).expect("in the rect");
            if out[i] != 0 || !self.walk(x0, y0, b) {
                continue;
            }
            next += 1;
            out[i] = next;
            let mut q = vec![(x0, y0)];
            while let Some((x, y)) = q.pop() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if !self.walk(nx, ny, b) {
                        continue;
                    }
                    let j = self.ix(nx, ny).expect("walkable is inside");
                    if out[j] == 0 {
                        out[j] = next;
                        q.push((nx, ny));
                    }
                }
            }
        }
        out
    }
}

/// Pushed and pulled every way it goes from `from`, with her anywhere she can walk to from
/// `start`: every state reached, and those from which it can no longer be brought back to `from`
/// with her on the side she started (a reset: every door she could reach, she can reach again).
fn stranding_pushes(room: &Room<'_>, from: (i32, i32), start: (i32, i32)) -> (usize, Vec<String>) {
    type S = ((i32, i32), u16);
    let mut labels: BTreeMap<(i32, i32), Vec<u16>> = BTreeMap::new();
    let mut label = |b: (i32, i32), at: (i32, i32)| -> u16 {
        let l = labels.entry(b).or_insert_with(|| room.labels(b));
        room.ix(at.0, at.1).map_or(0, |i| l[i])
    };
    let s0 = (from, label(from, start));
    assert!(s0.1 != 0, "she cannot stand at {start:?}");
    let mut next: BTreeMap<S, Vec<S>> = BTreeMap::new();
    let mut q = vec![s0];
    let mut seen = BTreeSet::from([s0]);
    while let Some(s) = q.pop() {
        let (b, me) = s;
        let mut outs = Vec::new();
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let cells: Vec<(i32, i32)> = (0..room.size.1)
                .flat_map(|j| (0..room.size.0).map(move |i| (b.0 + i - dx, b.1 + j - dy)))
                .filter(|&(x, y)| !room.under(x, y, b))
                .collect();
            for (x, y) in cells {
                if label(b, (x, y)) != me {
                    continue;
                }
                let pushed = (b.0 + dx, b.1 + dy);
                if room.fits(pushed) {
                    outs.push((pushed, label(pushed, (x + dx, y + dy))));
                }
                let (pulled, back) = ((b.0 - dx, b.1 - dy), (x - dx, y - dy));
                if room.walk(back.0, back.1, b) && room.fits(pulled) {
                    outs.push((pulled, label(pulled, back)));
                }
            }
        }
        for &o in &outs {
            if seen.insert(o) {
                q.push(o);
            }
        }
        next.insert(s, outs);
    }
    let mut can: BTreeSet<S> = BTreeSet::from([s0]);
    loop {
        let before = can.len();
        for (s, outs) in &next {
            if !can.contains(s) && outs.iter().any(|o| can.contains(o)) {
                can.insert(*s);
            }
        }
        if can.len() == before {
            break;
        }
    }
    let stuck =
        seen.iter().filter(|s| !can.contains(s)).map(|(b, at)| format!("left at {b:?}, she at {at:?}")).collect();
    (seen.len(), stuck)
}

#[test]
fn every_pushable_of_the_forest_the_burial_and_the_school_can_be_put_back_from_wherever_it_was_left() {
    let cat = catalog();
    let foot = |p: &jane_core::blueprint::PropSpawn| {
        let d = cat.story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
    };
    let (mut pushables, mut searched) = (0, 0);
    for zone in LATE {
        for seed in 1..=16 {
            let b = build(zone, seed);
            let bp = &b.blueprint;
            for pushed in bp.props.iter().filter(|p| cat.story.prop(p.def).push && !p.hidden) {
                let f = foot(pushed);
                let Some(room) = b.info.rooms.iter().find(|r| r.rect.contains(f.x, f.y)) else { continue };
                let around = room.rect.grow(1);
                let mut blocked = vec![false; (around.w * around.h) as usize];
                for p in bp.props.iter().filter(|p| p.key != pushed.key && !p.hidden && cat.story.prop(p.def).solid) {
                    for (x, y) in foot(p).cells().filter(|&(x, y)| around.contains(x, y)) {
                        blocked[((y - around.y) * around.w + x - around.x) as usize] = true;
                    }
                }
                let r = Room { bp, rect: room.rect, around, blocked, size: (f.w, f.h) };
                // She starts on the floor nearest the middle (a pillar may stand on it).
                let c = room.centre;
                let start = room
                    .rect
                    .cells()
                    .filter(|&(x, y)| r.walk(x, y, (f.x, f.y)))
                    .min_by_key(|&(x, y)| ((x - c.0).pow(2) + (y - c.1).pow(2), y, x))
                    .expect("floor to stand on");
                let (n, stuck) = stranding_pushes(&r, (f.x, f.y), start);
                assert!(
                    stuck.is_empty(),
                    "{zone:?} seed {seed}: {} of {n} states strand {:?}:\n{}",
                    stuck.len(),
                    pushed.key,
                    stuck.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
                );
                pushables += 1;
                searched += n;
            }
        }
    }
    // The Burial's great torch on every seed, at least.
    assert!(pushables >= 16, "only {pushables} pushables proven");
    assert!(searched > 16 * 1000, "only {searched} states searched");
}

#[test]
fn the_mine_is_clean_under_the_gated_search_too() {
    for seed in 1..=16 {
        let b = build(ZoneId::Mine, seed);
        let v = verdict(ZoneId::Mine, &b);
        assert!(v.stranded.is_empty(), "seed {seed}, {} states:\n{}", v.states, v.stranded.join("\n"));
    }
}
