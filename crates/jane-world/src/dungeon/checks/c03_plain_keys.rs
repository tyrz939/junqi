//! C3: no order of spending plain keys strands her (`spendingOrders`), on the room graph of the
//! nodes this seed placed. A plain key fits more than one lock, so she chooses where it goes:
//! search every order of opening the plain locks she can reach; whenever she runs out of
//! choices, every critical node must have been reached. Everything that is not a choice (a
//! named key, a verb, a flag, a state) is opened the moment it can be.
//!
//! The build runs the same search on the whole mission (`jane-schema`'s mission lint); this one
//! leaves out the side rooms the layout dropped, whose keys she then never finds.

use std::collections::{BTreeMap, BTreeSet};

use jane_core::Blueprint;
use jane_core::action::FlagKey;
use jane_core::ids::{Key, NameId, SpellId};
use jane_data::{MissionDef, MissionEdgeKind, MissionNodeKind};

use super::{Check, Ctx, Fault, Gains, gains_of};

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let dropped = c.info.layout.as_ref().map_or(&[][..], |l| l.dropped.as_slice());
    spending_orders(c.bp, c.m, dropped).into_iter().map(|e| Fault::new(Check::C3, e)).collect()
}

struct GraphLock {
    edge: usize,
    a: usize,
    b: usize,
    kinds: Vec<MissionEdgeKind>,
}

/// Every order of spending plain keys, over the nodes not in `dropped` (node indices).
pub fn spending_orders(bp: &Blueprint, m: &MissionDef, dropped: &[u8]) -> Vec<String> {
    let gone = |n: usize| dropped.iter().any(|&d| usize::from(d) == n);
    let nodes: Vec<usize> = (0..m.nodes.len()).filter(|&n| !gone(n)).collect();
    let mut locks = Vec::new();
    for (i, e) in m.edges.iter().enumerate() {
        let (a, b) = (usize::from(e.from), usize::from(e.to));
        if matches!(e.kind, MissionEdgeKind::Sight) || gone(a) || gone(b) {
            continue;
        }
        locks.push(GraphLock { edge: i, a, b, kinds: std::iter::once(e.kind).chain(e.also.iter().copied()).collect() });
    }
    let mut tag_uses: BTreeMap<NameId, u32> = BTreeMap::new();
    for l in &locks {
        for k in &l.kinds {
            if let MissionEdgeKind::Key { tag, .. } = k {
                *tag_uses.entry(*tag).or_insert(0) += 1;
            }
        }
    }
    let plain: Vec<Option<NameId>> = locks
        .iter()
        .map(|l| {
            l.kinds.iter().find_map(|k| match *k {
                MissionEdgeKind::Key { tag, .. } if tag_uses.get(&tag).copied().unwrap_or(0) > 1 => Some(tag),
                _ => None,
            })
        })
        .collect();
    let gains: Vec<Gains> = m.nodes.iter().map(|n| gains_of(bp, n)).collect();
    let Some(entrance) = nodes.iter().copied().find(|&n| m.nodes[n].kind == MissionNodeKind::Entrance) else {
        return Vec::new();
    };
    let mut s = Search {
        m,
        nodes: &nodes,
        locks: &locks,
        plain: &plain,
        gains: &gains,
        entrance,
        errors: Vec::new(),
        seen: BTreeSet::new(),
    };
    s.explore(Vec::new(), &BTreeMap::new(), &[]);
    s.errors
}

struct Search<'a> {
    m: &'a MissionDef,
    nodes: &'a [usize],
    locks: &'a [GraphLock],
    /// Per lock: the plain key tag it takes, if it takes one.
    plain: &'a [Option<NameId>],
    gains: &'a [Gains],
    entrance: usize,
    errors: Vec<String>,
    seen: BTreeSet<Vec<usize>>,
}

impl Search<'_> {
    fn explore(&mut self, opened: Vec<usize>, spent: &BTreeMap<NameId, i32>, order: &[String]) {
        let mut id = opened.clone();
        id.sort();
        if !self.errors.is_empty() || !self.seen.insert(id) {
            return;
        }
        let m = self.m;
        // Settle: reach, collect, open what is not a choice, until nothing moves.
        let mut open = opened;
        for _guard in 0..64 {
            let mut reached = vec![self.entrance];
            let mut q = 0;
            while q < reached.len() {
                for l in self.locks {
                    let passable = open.contains(&l.edge)
                        || l.kinds.iter().all(|k| matches!(k, MissionEdgeKind::Open | MissionEdgeKind::Lockin { .. }));
                    if !passable {
                        continue;
                    }
                    let next = if l.a == reached[q] {
                        Some(l.b)
                    } else if l.b == reached[q] {
                        Some(l.a)
                    } else {
                        None
                    };
                    if let Some(nx) = next.filter(|nx| !reached.contains(nx)) {
                        reached.push(nx);
                    }
                }
                q += 1;
            }
            let mut verbs: Vec<SpellId> = m.given_verbs.to_vec();
            let mut flags: Vec<FlagKey> = Vec::new();
            let mut controlled: Vec<u8> = Vec::new();
            let mut keys: BTreeMap<NameId, i32> = m.given_keys.iter().map(|&t| (t, 99)).collect();
            for &n in self.nodes {
                if !reached.contains(&n) {
                    continue;
                }
                let g = &self.gains[n];
                verbs.extend(&g.verbs);
                flags.extend(&g.flags);
                controlled.extend(&g.states);
                for (&tag, &k) in &g.keys {
                    *keys.entry(tag).or_insert(0) += k;
                }
            }
            let state_open = |var: u8, is: u8| m.states[usize::from(var)].initial == is || controlled.contains(&var);
            let mut moved = false;
            for (i, l) in self.locks.iter().enumerate() {
                if open.contains(&l.edge)
                    || self.plain[i].is_some()
                    || (!reached.contains(&l.a) && !reached.contains(&l.b))
                {
                    continue;
                }
                let can = l.kinds.iter().all(|k| match *k {
                    MissionEdgeKind::Key { tag, .. } => keys.get(&tag).copied().unwrap_or(0) > 0,
                    MissionEdgeKind::Verb { verb, .. } => verbs.contains(&verb),
                    MissionEdgeKind::Oneway { flag, .. } => flags.contains(&FlagKey::Named(Key::Name(flag))),
                    MissionEdgeKind::State { var, is, .. } => state_open(var, is),
                    _ => true,
                });
                if can {
                    open.push(l.edge);
                    moved = true;
                }
            }
            if moved {
                continue;
            }
            // Her choices now: each plain lock in reach that a key in hand fits.
            let choices: Vec<(usize, NameId)> = self
                .locks
                .iter()
                .enumerate()
                .filter_map(|(i, l)| {
                    let tag = self.plain[i]?;
                    let left = keys.get(&tag).copied().unwrap_or(0) - spent.get(&tag).copied().unwrap_or(0);
                    let fits =
                        !open.contains(&l.edge) && (reached.contains(&l.a) || reached.contains(&l.b)) && left > 0;
                    fits.then_some((i, tag))
                })
                .collect();
            if choices.is_empty() {
                let lost: Vec<&str> = self
                    .nodes
                    .iter()
                    .filter(|&&n| m.nodes[n].critical && !reached.contains(&n))
                    .map(|&n| m.nodes[n].id)
                    .collect();
                if !lost.is_empty() {
                    self.errors.push(format!(
                        "opening plain locks in the order [{}] strands her short of {}",
                        order.join(", "),
                        lost.join(", ")
                    ));
                }
                return;
            }
            for (i, tag) in choices {
                let l = &self.locks[i];
                let mut next_open = open.clone();
                next_open.push(l.edge);
                let mut next_spent = spent.clone();
                *next_spent.entry(tag).or_insert(0) += 1;
                let mut next_order = order.to_vec();
                next_order.push(format!("{}-{}", m.nodes[l.a].id, m.nodes[l.b].id));
                self.explore(next_open, &next_spent, &next_order);
            }
            return;
        }
    }
}
