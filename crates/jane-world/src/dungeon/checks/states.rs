//! No reachable state strands her (DUNGEONS.md §2.6, "the softlock pass"). C3 searches the
//! orders of spending plain keys; this searches everything she can spend or waste, in every
//! order, and asks of **every** state she can get into whether the mission can still be finished
//! from it. Not a generation check (it would re-roll nothing C1 to C12 do not), but a proof the
//! tests ask of a dungeon: `jane-world/tests/dungeon_states.rs`.
//!
//! On the room graph of the nodes this seed placed. A state is what she has done that cannot be
//! undone: the locks opened (a key spent, a verb's materials spent), the verb props she used
//! wherever she liked (a cabinet mended with the steps' wood), and keys carried out of the zone and
//! spent on a lock there. Everything else is not a choice and is had the moment it can be: rooms
//! walked into, what they give (keys, materials, verbs, flags), gates that open on a flag or a
//! state. What she can do in a state:
//!
//! - open a key lock she can reach with a key that fits (spent, unless the key is bound);
//! - open a verb lock she can reach, knowing the verb, with its materials (spent);
//! - use a verb prop in a room she reached, with its materials (spent), for what it gives;
//! - carry a key whose tag also fits a lock in another zone (`outside`) out, and spend it there.
//!
//! What is left out, and why it cannot strand her: dying keeps her bag and a lock-in's reset opens
//! its gate again (`triggers::reset_on_death`); leaving and coming back keeps the zone as it was
//! (`ZoneState` is saved whole); a dropped key never ages out (`loot::step_drops`: what opens is
//! story) and Destroy refuses it; a pushed thing is moved back by pulling (`dungeon_states.rs`
//! proves each plate's pushable on the cell grid).

use std::collections::{BTreeMap, BTreeSet};

use jane_core::Blueprint;
use jane_core::action::FlagKey;
use jane_core::ids::{ItemId, Key, NameId, SpellId};
use jane_data::{MissionDef, MissionEdgeKind, MissionNodeKind, MissionProp, catalog};

use super::{Gains, answers_of, gains_of_holding};

/// A spend she may make: what it takes and what it gives.
#[derive(Clone, Debug)]
struct Sink {
    node: usize,
    name: String,
    needs: Vec<(ItemId, i32)>,
    /// A verb that answers the prop must be known (`None`: none asked).
    answers: Option<jane_data::Answers>,
    gives: Gains,
}

#[derive(Clone, Debug)]
struct Lock {
    a: usize,
    b: usize,
    kinds: Vec<MissionEdgeKind>,
}

/// What cannot be undone. Everything else is worked out from it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
struct State {
    open: u64,
    used: u64,
    keys_spent: BTreeMap<NameId, i32>,
    items_spent: BTreeMap<ItemId, i32>,
}

/// What the search found.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// States she can get into.
    pub states: usize,
    /// Of those, the ones she can no longer finish from, each with a way into it.
    pub stranded: Vec<String>,
}

/// Every state she can reach in mission `m` as built in `bp` (with the nodes the layout dropped
/// left out), and whether each can still be finished. `outside`: key tags that also fit a lock in
/// another zone.
pub fn search(bp: &Blueprint, m: &MissionDef, dropped: &[u8], outside: &[NameId]) -> Verdict {
    let cat = catalog();
    let gone = |n: usize| dropped.iter().any(|&d| usize::from(d) == n);
    let nodes: Vec<usize> = (0..m.nodes.len()).filter(|&n| !gone(n)).collect();
    let mut locks = Vec::new();
    for e in m.edges {
        let (a, b) = (usize::from(e.from), usize::from(e.to));
        if matches!(e.kind, MissionEdgeKind::Sight) || gone(a) || gone(b) {
            continue;
        }
        locks.push(Lock { a, b, kinds: std::iter::once(e.kind).chain(e.also.iter().copied()).collect() });
    }
    // A node's own gains, and its verb props (a holding that asks for materials) apart.
    let mut gains: Vec<Gains> = vec![Gains::default(); m.nodes.len()];
    let mut sinks = Vec::new();
    for &n in &nodes {
        for h in m.nodes[n].holds {
            let g = gains_of_holding(bp, h);
            if h.needs.is_empty() {
                merge(&mut gains[n], &g);
                continue;
            }
            let answers = match h.prop {
                Some(MissionProp::Row(d)) => cat.story.prop(d).answers,
                _ => None,
            };
            let needs = h.needs.iter().map(|s| (s.item, i32::from(s.qty))).collect();
            sinks.push(Sink { node: n, name: format!("{}:{}", m.nodes[n].id, h.socket), needs, answers, gives: g });
        }
    }
    let bound = |tag: NameId| cat.combat.items.iter().filter(|d| d.opens == Some(tag)).all(|d| d.bound);
    let Some(entrance) = nodes.iter().copied().find(|&n| m.nodes[n].kind == MissionNodeKind::Entrance) else {
        return Verdict::default();
    };
    let s = Search { m, nodes: &nodes, locks: &locks, gains: &gains, sinks: &sinks, entrance, outside, bound: &bound };
    s.run()
}

fn merge(into: &mut Gains, g: &Gains) {
    for (&k, &v) in &g.keys {
        *into.keys.entry(k).or_insert(0) += v;
    }
    for (&k, &v) in &g.items {
        *into.items.entry(k).or_insert(0) += v;
    }
    into.verbs.extend(&g.verbs);
    into.flags.extend(&g.flags);
    into.states.extend(&g.states);
}

struct Search<'a> {
    m: &'a MissionDef,
    nodes: &'a [usize],
    locks: &'a [Lock],
    gains: &'a [Gains],
    sinks: &'a [Sink],
    entrance: usize,
    outside: &'a [NameId],
    bound: &'a dyn Fn(NameId) -> bool,
}

/// A state worked out: where she can be and what she holds.
struct Seen {
    reached: Vec<bool>,
    had: Gains,
}

impl Search<'_> {
    fn passable(&self, s: &State, i: usize) -> bool {
        s.open >> i & 1 == 1
            || self.locks[i].kinds.iter().all(|k| matches!(k, MissionEdgeKind::Open | MissionEdgeKind::Lockin { .. }))
    }

    /// Reach, collect, and open what is not a choice, until nothing moves.
    fn settle(&self, s: &mut State) -> Seen {
        loop {
            let mut reached = vec![false; self.m.nodes.len()];
            reached[self.entrance] = true;
            let mut q = vec![self.entrance];
            while let Some(n) = q.pop() {
                for i in 0..self.locks.len() {
                    let l = &self.locks[i];
                    if !self.passable(s, i) {
                        continue;
                    }
                    for (x, y) in [(l.a, l.b), (l.b, l.a)] {
                        if x == n && !reached[y] {
                            reached[y] = true;
                            q.push(y);
                        }
                    }
                }
            }
            let mut had = Gains::default();
            for &n in self.nodes {
                if reached[n] {
                    merge(&mut had, &self.gains[n]);
                }
            }
            for (i, k) in self.sinks.iter().enumerate() {
                if s.used >> i & 1 == 1 {
                    merge(&mut had, &k.gives);
                }
            }
            let m = self.m;
            let mut moved = false;
            for i in 0..self.locks.len() {
                let l = &self.locks[i];
                if s.open >> i & 1 == 1 || !(reached[l.a] || reached[l.b]) {
                    continue;
                }
                let free = l.kinds.iter().all(|k| match *k {
                    MissionEdgeKind::Open | MissionEdgeKind::Lockin { .. } | MissionEdgeKind::Sight => true,
                    MissionEdgeKind::Oneway { flag, .. } => had.flags.contains(&FlagKey::Named(Key::Name(flag))),
                    MissionEdgeKind::State { var, is, .. } => {
                        m.states[usize::from(var)].initial == is || had.states.contains(&var)
                    }
                    MissionEdgeKind::Key { .. } | MissionEdgeKind::Verb { .. } => false,
                });
                let needs_nothing =
                    l.kinds.iter().all(|k| !matches!(k, MissionEdgeKind::Open | MissionEdgeKind::Sight));
                if free && needs_nothing {
                    s.open |= 1 << i;
                    moved = true;
                }
            }
            if !moved {
                return Seen { reached, had };
            }
        }
    }

    fn keys(&self, s: &State, seen: &Seen, tag: NameId) -> i32 {
        seen.had.keys.get(&tag).copied().unwrap_or(0) + if self.m.given_keys.contains(&tag) { 99 } else { 0 }
            - s.keys_spent.get(&tag).copied().unwrap_or(0)
    }

    fn items(s: &State, seen: &Seen, item: ItemId) -> i32 {
        seen.had.items.get(&item).copied().unwrap_or(0) - s.items_spent.get(&item).copied().unwrap_or(0)
    }

    fn knows(&self, seen: &Seen, v: SpellId) -> bool {
        self.m.given_verbs.contains(&v) || seen.had.verbs.contains(&v)
    }

    /// Every move from a settled state, named.
    fn moves(&self, s: &State, seen: &Seen) -> Vec<(State, String)> {
        let m = self.m;
        let mut out = Vec::new();
        let afford = |s: &State, needs: &[(ItemId, i32)]| needs.iter().all(|&(i, q)| Self::items(s, seen, i) >= q);
        let spend = |s: &mut State, needs: &[(ItemId, i32)]| {
            for &(i, q) in needs {
                *s.items_spent.entry(i).or_insert(0) += q;
            }
        };
        for (i, l) in self.locks.iter().enumerate() {
            if s.open >> i & 1 == 1 || !(seen.reached[l.a] || seen.reached[l.b]) {
                continue;
            }
            let mut next = s.clone();
            let mut ok = true;
            for k in &l.kinds {
                match *k {
                    MissionEdgeKind::Key { tag, .. } => {
                        ok &= self.keys(s, seen, tag) > 0;
                        if !(self.bound)(tag) {
                            *next.keys_spent.entry(tag).or_insert(0) += 1;
                        }
                    }
                    MissionEdgeKind::Verb { verb, needs, .. } => {
                        let needs: Vec<(ItemId, i32)> = needs.iter().map(|s| (s.item, i32::from(s.qty))).collect();
                        ok &= self.knows(seen, verb) && afford(s, &needs);
                        spend(&mut next, &needs);
                    }
                    MissionEdgeKind::Oneway { flag, .. } => {
                        ok &= seen.had.flags.contains(&FlagKey::Named(Key::Name(flag)));
                    }
                    MissionEdgeKind::State { var, is, .. } => {
                        ok &= m.states[usize::from(var)].initial == is || seen.had.states.contains(&var);
                    }
                    _ => {}
                }
            }
            if ok {
                next.open |= 1 << i;
                out.push((next, format!("open {}-{}", m.nodes[l.a].id, m.nodes[l.b].id)));
            }
        }
        for (i, k) in self.sinks.iter().enumerate() {
            if s.used >> i & 1 == 1 || !seen.reached[k.node] || !afford(s, &k.needs) {
                continue;
            }
            let verb = k
                .answers
                .is_none_or(|a| m.given_verbs.iter().chain(&seen.had.verbs).any(|&v| answers_of(v) == Some(a)));
            if !verb {
                continue;
            }
            let mut next = s.clone();
            spend(&mut next, &k.needs);
            next.used |= 1 << i;
            out.push((next, format!("use {}", k.name)));
        }
        for &tag in self.outside {
            if self.keys(s, seen, tag) > 0 && !(self.bound)(tag) {
                let mut next = s.clone();
                *next.keys_spent.entry(tag).or_insert(0) += 1;
                out.push((next, format!("spend a {} key outside", catalog().name(tag))));
            }
        }
        out
    }

    fn done(&self, seen: &Seen) -> bool {
        self.nodes.iter().all(|&n| !self.m.nodes[n].critical || seen.reached[n])
    }

    fn run(&self) -> Verdict {
        // Forward: every state she can get into, with the move that first got her there.
        let mut start = State::default();
        self.settle(&mut start);
        let mut how: BTreeMap<State, Option<(State, String)>> = BTreeMap::new();
        let mut next: BTreeMap<State, Vec<State>> = BTreeMap::new();
        let mut finished: BTreeSet<State> = BTreeSet::new();
        how.insert(start.clone(), None);
        let mut q = vec![start];
        while let Some(s) = q.pop() {
            let mut t = s.clone();
            let seen = self.settle(&mut t);
            if self.done(&seen) {
                finished.insert(s.clone());
            }
            let mut outs = Vec::new();
            for (mut n, why) in self.moves(&s, &seen) {
                self.settle(&mut n);
                outs.push(n.clone());
                if !how.contains_key(&n) {
                    how.insert(n.clone(), Some((s.clone(), why)));
                    q.push(n);
                }
            }
            next.insert(s, outs);
        }
        // Backward: the states from which a finished one can be reached. Every move spends or
        // opens something, so the graph has no cycles and a fixed point comes quickly.
        let mut can = finished;
        loop {
            let before = can.len();
            for (s, outs) in &next {
                if !can.contains(s) && outs.iter().any(|o| can.contains(o)) {
                    can.insert(s.clone());
                }
            }
            if can.len() == before {
                break;
            }
        }
        let mut stranded = Vec::new();
        for s in how.keys() {
            if can.contains(s) {
                continue;
            }
            let mut path = Vec::new();
            let mut at = s.clone();
            while let Some(Some((prev, why))) = how.get(&at) {
                path.push(why.clone());
                at = prev.clone();
            }
            path.reverse();
            let mut t = s.clone();
            let seen = self.settle(&mut t);
            let lost: Vec<&str> = self
                .nodes
                .iter()
                .filter(|&&n| self.m.nodes[n].critical && !seen.reached[n])
                .map(|&n| self.m.nodes[n].id)
                .collect();
            stranded.push(format!("[{}] strands her short of {}", path.join(", "), lost.join(", ")));
        }
        Verdict { states: how.len(), stranded }
    }
}
