//! The softlock pass on the Factory, the Museum, the Library and the Pipes (DUNGEONS.md §3.2 to
//! §3.4, "Can she get stuck?"), as `dungeon_states.rs` does it for the Gold Mine: no state she can
//! get each of them into leaves it unfinishable, on seeds 1 to 16.
//!
//! - On the room graph (`dungeon::checks::states`): every order of opening locks, spending
//!   materials on any verb prop she likes, and carrying keys out to spend on locks in other zones,
//!   searched whole; from every state reached, the mission can still be finished.
//! - On the cell grid: every plate's pushables, pushed and pulled every way they go, can still be
//!   brought back onto the plate from wherever they were left.
//!
//! The key scan and the grid search are the mine's, kept in a file of their own so the passes on
//! the other dungeons merge without touching each other.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use jane_core::grid::Rect;
use jane_core::tile::{F_NOPUSH, F_SOLID};
use jane_core::{Blueprint, Key, NameId, Tile, ZoneId};
use jane_data::{MissionEdgeKind, catalog};
use jane_world::dungeon::checks::states;
use jane_world::dungeon::{Built, build};

const WORKS: [ZoneId; 4] = [ZoneId::Factory, ZoneId::Museum, ZoneId::Library, ZoneId::Pipes];

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
fn no_state_of_the_factory_the_museum_the_library_or_the_pipes_strands_her() {
    for zone in WORKS {
        let mut most = 0;
        for seed in 1..=16 {
            let b = build(zone, seed);
            assert!(b.info.errors.is_empty(), "{zone:?} seed {seed}: {:?}", b.info.errors);
            let v = verdict(zone, &b);
            most = most.max(v.states);
            assert!(v.stranded.is_empty(), "{zone:?} seed {seed}, {} states:\n{}", v.states, v.stranded.join("\n"));
        }
        // The Library has no lock at all: one state is the whole of it.
        assert!(most > 1 || zone == ZoneId::Library, "{zone:?}: only {most} states searched");
    }
}

#[test]
fn their_keys_fit_nothing_outside_them() {
    // A key whose tag also fits a lock in another zone can be carried out and spent there, and the
    // dungeon is one key short (the Factory's plain keys could, as they were).
    for zone in WORKS {
        assert_eq!(outside(zone), Vec::<NameId>::new(), "{zone:?}");
    }
}

#[test]
fn the_factorys_gates_take_its_own_key_and_the_search_would_catch_a_plain_one() {
    let cat = catalog();
    let shop = cat.name_id("shop").expect("the shop tag");
    let m = cat.dungeons.mission_of(ZoneId::Factory).expect("the factory");
    let tags: Vec<NameId> = m
        .edges
        .iter()
        .filter_map(|e| match e.kind {
            MissionEdgeKind::Key { tag, .. } => Some(tag),
            _ => None,
        })
        .collect();
    assert_eq!(tags.iter().filter(|&&t| t == shop).count(), 2, "No. 1 LINE and the PRESS HALL");
    // As it was: a key that fits a lock outside, carried out and spent, strands her.
    let b = build(ZoneId::Factory, 1);
    let v = states::search(&b.blueprint, b.info.mission, &[], &[shop]);
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

/// Every plate of `zone` on `seed`, proven: (plates, states searched).
fn plates_hold(zone: ZoneId, seed: u32) -> (usize, usize) {
    let cat = catalog();
    let foot = |p: &jane_core::blueprint::PropSpawn| {
        let d = cat.story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h))
    };
    let (mut plates, mut searched) = (0, 0);
    let b = build(zone, seed);
    let bp = &b.blueprint;
    for plate in bp.props.iter().filter(|p| cat.story.prop(p.def).plate && !p.hidden) {
        let pr = foot(plate);
        let room = b.info.rooms.iter().find(|r| r.rect.contains(pr.x, pr.y)).expect("a plate stands in a room");
        let pushables: Vec<_> = bp
            .props
            .iter()
            .filter(|p| {
                cat.story.prop(p.def).push && !p.hidden && room.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y))
            })
            .collect();
        assert!(!pushables.is_empty(), "{zone:?} seed {seed}: nothing to push onto a plate");
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
            assert!(n > 1, "{zone:?} seed {seed}: {:?} cannot be moved at all", pushed.key);
            // Every pushable that can reach the plate at all can do so from anywhere it goes.
            let reaches = stuck.len() < n;
            assert!(
                !reaches || stuck.is_empty(),
                "{zone:?} seed {seed}: {:?} of {n} states strand the pushable {:?}:\n{}",
                stuck.len(),
                pushed.key,
                stuck.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
            );
            held |= reaches;
            searched += n;
        }
        assert!(held, "{zone:?} seed {seed}: no pushable reaches the plate at {pr:?}");
        plates += 1;
    }
    (plates, searched)
}

#[test]
fn every_plate_of_the_museum_the_library_and_the_pipes_can_be_held_again_from_wherever_its_pushable_was_left() {
    for zone in WORKS {
        let (mut plates, mut searched) = (0, 0);
        for seed in 1..=16 {
            let (p, n) = plates_hold(zone, seed);
            plates += p;
            searched += n;
        }
        // The Factory has no plate; the Museum's two are in a side room a seed may leave out.
        if zone != ZoneId::Factory {
            assert!(plates >= 16, "{zone:?}: only {plates} plates proven");
            assert!(searched > 16 * 100, "{zone:?}: only {searched} states searched");
        }
    }
}
