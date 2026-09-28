//! The softlock pass (DUNGEONS.md §3.1, "Can she get stuck?"): no state she can get the Gold
//! Mine into leaves it unfinishable, on seeds 1 to 16.
//!
//! - On the room graph (`dungeon::checks::states`): every order of opening locks, spending
//!   materials on any verb prop she likes, and carrying keys out to spend on locks in other zones,
//!   searched whole; from every state reached, the mission can still be finished.
//! - On the cell grid: every plate's pushables, pushed and pulled every way they go, can still be
//!   brought back onto the plate from wherever they were left.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use jane_core::grid::Rect;
use jane_core::tile::{F_NOPUSH, F_SOLID};
use jane_core::{Blueprint, Key, NameId, Tile, ZoneId};
use jane_data::{MissionEdgeKind, catalog};
use jane_world::dungeon::checks::states;
use jane_world::dungeon::{Built, build};

/// Every key tag a lock in `zone` takes, when that zone is built (seed 1: the tags on a zone's
/// locks are its data's, whatever the seed).
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

/// Key tags that fit a lock somewhere, by zone, built once.
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
    states::search(&b.blueprint, m, dropped, &outside(zone))
}

#[test]
fn no_state_of_the_gold_mine_strands_her() {
    for seed in 1..=16 {
        let b = build(ZoneId::Mine, seed);
        assert!(b.info.errors.is_empty(), "seed {seed}: {:?}", b.info.errors);
        let v = verdict(ZoneId::Mine, &b);
        assert!(v.states > 10, "seed {seed}: only {} states searched", v.states);
        assert!(v.stranded.is_empty(), "seed {seed}, {} states:\n{}", v.states, v.stranded.join("\n"));
    }
}

#[test]
fn the_gold_mines_plain_keys_fit_nothing_outside_it() {
    // A plain key that fits a lock in the county can be carried out and spent there, and the mine
    // is one key short (the softlock pass, DUNGEONS.md §3.1).
    assert_eq!(outside(ZoneId::Mine), Vec::<NameId>::new());
    // And the scan sees such locks where they are: the plain key's, in the cellar and the county.
    let generic = catalog().name_id("generic").expect("the plain key's tag");
    let with: Vec<ZoneId> = all_tags().iter().filter(|(_, t)| t.contains(&generic)).map(|(&z, _)| z).collect();
    assert!(with.len() >= 2, "plain locks found only in {with:?}");
}

#[test]
fn a_plain_key_that_fits_outside_is_caught() {
    // The search finds the leak when there is one: the mine as it was, its two plain locks on the
    // key every county shed takes.
    let b = build(ZoneId::Mine, 1);
    let pit = catalog().name_id("pit").expect("the pit tag");
    let v = states::search(&b.blueprint, b.info.mission, &[], &[pit]);
    assert!(!v.stranded.is_empty(), "carrying a pit key out strands nothing?");
    assert!(v.stranded.iter().any(|s| s.contains("outside")), "{:?}", v.stranded);
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

    /// She can stand here with the pushable's top-left at `b`.
    fn walk(&self, x: i32, y: i32, b: (i32, i32)) -> bool {
        self.ix(x, y).is_some_and(|i| !self.blocked[i]) && self.tile(x, y) & F_SOLID == 0 && !self.under(x, y, b)
    }

    /// The pushable fits with its top-left at `b`.
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

    /// Which walkable cells join up with the pushable at `b`: a label a cell, 0 for none.
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

/// Pushed and pulled every way it goes, from where it stands with her anywhere she can walk to
/// from `start`: every state reached, and whether each can still bring it onto `plate`.
fn push_states(room: &Room<'_>, from: (i32, i32), start: (i32, i32), plate: Rect) -> (usize, Vec<String>) {
    type S = ((i32, i32), u16);
    let on = |b: (i32, i32)| {
        b.0 < plate.right() && b.0 + room.size.0 > plate.x && b.1 < plate.bottom() && b.1 + room.size.1 > plate.y
    };
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
        // Every cell beside the footprint, facing it.
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
    let mut can: BTreeSet<S> = seen.iter().copied().filter(|&(b, _)| on(b)).collect();
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
fn every_plate_of_the_gold_mine_can_be_held_again_from_wherever_its_pushable_was_left() {
    let cat = catalog();
    let foot = |p: &jane_core::blueprint::PropSpawn| {
        let d = cat.story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
    };
    let (mut plates, mut searched) = (0, 0);
    for seed in 1..=16 {
        let b = build(ZoneId::Mine, seed);
        let bp = &b.blueprint;
        for plate in bp.props.iter().filter(|p| cat.story.prop(p.def).plate && !p.hidden) {
            let pr = foot(plate);
            let room = b.info.rooms.iter().find(|r| r.rect.contains(pr.x, pr.y)).expect("a plate stands in a room");
            let pushables: Vec<_> = bp
                .props
                .iter()
                .filter(|p| {
                    cat.story.prop(p.def).push
                        && !p.hidden
                        && room.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y))
                })
                .collect();
            assert!(!pushables.is_empty(), "seed {seed}: nothing to push onto a plate");
            let mut held = false;
            for pushed in &pushables {
                let around = room.rect.grow(1);
                let mut blocked = vec![false; (around.w * around.h) as usize];
                for p in bp.props.iter().filter(|p| p.key != pushed.key && !p.hidden && cat.story.prop(p.def).solid) {
                    for (x, y) in foot(p).cells().filter(|&(x, y)| around.contains(x, y)) {
                        blocked[((y - around.y) * around.w + x - around.x) as usize] = true;
                    }
                }
                let f = foot(pushed);
                let r = Room { bp, rect: room.rect, around, blocked, size: (f.w, f.h) };
                let (n, stuck) = push_states(&r, (f.x, f.y), room.centre, pr);
                assert!(n > 1, "seed {seed}: {:?} cannot be moved at all", pushed.key);
                // Every pushable that can reach the plate at all can do so from anywhere it goes.
                let reaches = stuck.len() < n;
                assert!(
                    !reaches || stuck.is_empty(),
                    "seed {seed}: {:?} of {n} states strand the pushable {:?}:\n{}",
                    stuck.len(),
                    pushed.key,
                    stuck.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
                );
                held |= reaches;
                searched += n;
            }
            assert!(held, "seed {seed}: no pushable reaches the plate at {pr:?}");
            plates += 1;
        }
    }
    assert!(plates >= 16, "only {plates} plates proven");
    assert!(searched > 16 * 1000, "only {searched} states searched");
}
