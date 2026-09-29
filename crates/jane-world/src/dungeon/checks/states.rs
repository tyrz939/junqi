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
use jane_core::action::{Action, Cond, Condition, FlagKey, FlagTest, ListRef};
use jane_core::ids::{ItemId, Key, NameId, SpellId};
use jane_data::{Answers, MissionDef, MissionEdgeKind, MissionNodeKind, MissionProp, catalog};

use super::{Gains, answers_of, gains_of_holding};
use crate::solve::rows;

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
    search_with(bp, m, dropped, outside, false)
}

/// [`search`], with what a prop gives held back until she can have it ([`Gated`]): a verb that
/// answers it known, whatever shows or unlocks it done, the conditions of an `if` met. A building
/// whose way on is a condition (the Burial's seal, the School's bell rope) is only searched truly
/// this way; without it, what the rope gives when six clocks are stopped is had on reaching the
/// hall.
pub fn search_gated(bp: &Blueprint, m: &MissionDef, dropped: &[u8], outside: &[NameId]) -> Verdict {
    search_with(bp, m, dropped, outside, true)
}

fn search_with(bp: &Blueprint, m: &MissionDef, dropped: &[u8], outside: &[NameId], gated: bool) -> Verdict {
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
                if !(gated && h.unit.is_none()) {
                    merge(&mut gains[n], &g);
                }
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
    let model = gated.then(|| Gated::of(bp, m, &nodes, &gains, &sinks));
    let s = Search {
        m,
        nodes: &nodes,
        locks: &locks,
        gains: &gains,
        sinks: &sinks,
        entrance,
        outside,
        bound: &bound,
        gated: model.as_ref(),
    };
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
    /// [`search_gated`]'s model; `None`, a free holding gives on reaching its room.
    gated: Option<&'a Gated>,
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
            if let Some(g) = self.gated {
                g.settle(self, s, &reached, &mut had);
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

// --- the gated model ---------------------------------------------------------------------------

/// Where a thing that shows or unlocks a prop comes from.
#[derive(Clone, Copy, Debug)]
enum Src {
    Entry(usize),
    Sink(usize),
    Trigger(usize),
}

/// A free prop's gift, or one branch of it: what it gives, once her verbs, whatever shows and
/// unlocks it, and the conditions of the `if` it sits in allow.
#[derive(Clone, Debug)]
struct Entry {
    node: usize,
    answers: Option<Answers>,
    /// Any one of these showing it will do; empty, it stands shown.
    shown_by: Vec<Src>,
    unlocked_by: Vec<Src>,
    when: Vec<Cond>,
    gives: Gains,
}

/// A trigger of the zone's (the mission's, a lock-in's, the story's), with no rect asked: it
/// runs once its conditions hold.
#[derive(Clone, Debug)]
struct TriggerGift {
    when: Vec<Cond>,
    gives: Gains,
}

/// [`search_gated`]'s model of the zone's free props and triggers.
#[derive(Clone, Debug, Default)]
pub struct Gated {
    entries: Vec<Entry>,
    triggers: Vec<TriggerGift>,
    /// Every flag anything here sets: a condition on any other is not the model's to judge.
    known: BTreeSet<FlagKey>,
}

/// What a list gives and what it shows and unlocks, split by the `if`s it sits in.
#[derive(Default)]
struct Branch {
    when: Vec<Cond>,
    gives: Gains,
    shows: Vec<Key>,
    unlocks: Vec<Key>,
}

fn branches(bp: &Blueprint, list: Option<ListRef>, when: &[Cond], out: &mut Vec<Branch>, depth: u8) {
    let cat = catalog();
    let Some(l) = list.and_then(|r| rows::list(bp, cat, r)) else { return };
    if depth > 8 {
        return;
    }
    let at = out.len();
    out.push(Branch { when: when.to_vec(), ..Branch::default() });
    for a in l {
        match *a {
            Action::Give(s) => out[at].gives.item(cat, s.item, s.qty),
            Action::Learn(sp) => out[at].gives.verbs.push(sp),
            Action::Flag { key, .. } => out[at].gives.flags.push(key),
            Action::Show(k) => out[at].shows.push(k),
            Action::Unlock(k) => out[at].unlocks.push(k),
            Action::If { when: w, then, els } => {
                let mut inner = when.to_vec();
                inner.extend(rows::conds(bp, cat, w).unwrap_or(&[]).iter().copied());
                branches(bp, Some(then), &inner, out, depth + 1);
                // What happens otherwise is had as if it always could: the model's optimism.
                branches(bp, els, when, out, depth + 1);
            }
            Action::Send { then, .. } => branches(bp, then, when, out, depth + 1),
            _ => {}
        }
    }
}

impl Gated {
    fn of(bp: &Blueprint, m: &MissionDef, nodes: &[usize], gains: &[Gains], sinks: &[Sink]) -> Gated {
        let cat = catalog();
        let mut g = Gated::default();
        let mut shows: Vec<(Src, Key)> = Vec::new();
        let mut unlocks: Vec<(Src, Key)> = Vec::new();
        // Props: the free holdings she can reach, and what each branch of their lists gives.
        let mut placed: Vec<(usize, &jane_core::blueprint::PropSpawn)> = Vec::new();
        let mut sink_at = 0;
        for &n in nodes {
            for h in m.nodes[n].holds {
                let prop = bp.props.iter().find(|p| p.key == Key::Name(h.key));
                if !h.needs.is_empty() {
                    // The sinks, in `search`'s order: what their lists show and unlock.
                    let mut bs = Vec::new();
                    branches(bp, prop.and_then(|p| p.use_list), &[], &mut bs, 0);
                    for b in bs {
                        shows.extend(b.shows.iter().map(|&k| (Src::Sink(sink_at), k)));
                        unlocks.extend(b.unlocks.iter().map(|&k| (Src::Sink(sink_at), k)));
                    }
                    sink_at += 1;
                    continue;
                }
                if h.unit.is_some() {
                    continue;
                }
                let bare = Entry {
                    node: n,
                    answers: None,
                    shown_by: Vec::new(),
                    unlocked_by: Vec::new(),
                    when: Vec::new(),
                    gives: Gains::default(),
                };
                let Some(p) = prop else {
                    // Not placed as a prop: had on reaching the room, as `search` has it.
                    g.entries.push(Entry { gives: gains_of_holding(bp, h), ..bare });
                    continue;
                };
                // The talk, the loot and a control's states, had whole.
                let mut base = Gains::default();
                for s in &p.loot {
                    base.item(cat, s.item, s.qty);
                }
                if let Some(tree) = p.talk.and_then(|t| cat.story.dialogue.get(t.index())) {
                    for d in tree.nodes {
                        base.actions(bp, cat, d.actions);
                        for o in d.options {
                            base.actions(bp, cat, o.actions);
                        }
                    }
                }
                if let Some(state) = h.controls {
                    base.states.push(state);
                    for &l in &h.becomes {
                        base.actions(bp, cat, l);
                    }
                }
                let first = g.entries.len();
                let answers = cat.story.prop(p.def).answers.filter(|&a| a != Answers::Physical);
                g.entries.push(Entry { answers, gives: base, ..bare });
                let mut bs = Vec::new();
                branches(bp, p.use_list, &[], &mut bs, 0);
                for b in bs {
                    let i = g.entries.len();
                    shows.extend(b.shows.iter().map(|&k| (Src::Entry(i), k)));
                    unlocks.extend(b.unlocks.iter().map(|&k| (Src::Entry(i), k)));
                    g.entries.push(Entry { when: b.when, gives: b.gives, ..g.entries[first].clone() });
                }
                placed.extend((first..g.entries.len()).map(|i| (i, p)));
            }
        }
        // Triggers: the blueprint's (the mission's own, the lock-ins') and the story's for the zone.
        let story = cat.story.triggers_in(m.zone).map(|(_, d)| &d.trigger);
        for t in bp.triggers.values().chain(story) {
            let when = t.when.and_then(|w| rows::conds(bp, cat, w)).unwrap_or(&[]).to_vec();
            let mut bs = Vec::new();
            branches(bp, Some(t.actions), &when, &mut bs, 0);
            for b in bs {
                let i = g.triggers.len();
                shows.extend(b.shows.iter().map(|&k| (Src::Trigger(i), k)));
                unlocks.extend(b.unlocks.iter().map(|&k| (Src::Trigger(i), k)));
                g.triggers.push(TriggerGift { when: b.when, gives: b.gives });
            }
        }
        // Hidden and locked props wait on whatever shows and unlocks them. One that nothing here
        // shows or unlocks is had as it stands (something the model does not see sees to it).
        for (i, p) in placed {
            if p.hidden {
                g.entries[i].shown_by = shows.iter().filter(|s| s.1 == p.key).map(|s| s.0).collect();
            }
            if p.locked && p.key_tag.is_none() {
                g.entries[i].unlocked_by = unlocks.iter().filter(|s| s.1 == p.key).map(|s| s.0).collect();
            }
        }
        let all = g.entries.iter().map(|e| &e.gives).chain(g.triggers.iter().map(|t| &t.gives));
        let all = all.chain(gains).chain(sinks.iter().map(|k| &k.gives));
        g.known = all.flat_map(|x| x.flags.iter().copied()).collect();
        g
    }

    fn met(&self, search: &Search<'_>, s: &State, had: &Gains, when: &[Cond]) -> bool {
        let cat = catalog();
        when.iter().all(|c| {
            if c.not {
                return true;
            }
            match c.c {
                Condition::Flag { key, test } if self.known.contains(&key) => {
                    let v = had.flags.iter().filter(|&&f| f == key).count() as i32;
                    match test {
                        FlagTest::Eq(n) => v == n,
                        FlagTest::Min(n) => v >= n,
                        FlagTest::NonZero => v != 0,
                    }
                }
                Condition::HasItem(st) => match cat.combat.items.get(st.item.index()).and_then(|d| d.opens) {
                    Some(tag) => {
                        let have = had.keys.get(&tag).copied().unwrap_or(0)
                            + if search.m.given_keys.contains(&tag) { 99 } else { 0 }
                            - s.keys_spent.get(&tag).copied().unwrap_or(0);
                        have >= i32::from(st.qty.max(1))
                    }
                    None => true,
                },
                _ => true,
            }
        })
    }

    /// Everything she can have in a state, given the rooms she reached: to a fixed point.
    fn settle(&self, search: &Search<'_>, s: &State, reached: &[bool], had: &mut Gains) {
        let mut entry = vec![false; self.entries.len()];
        let mut trig = vec![false; self.triggers.len()];
        let on = |src: &Src, entry: &[bool], trig: &[bool]| match *src {
            Src::Entry(i) => entry[i],
            Src::Sink(i) => s.used >> i & 1 == 1,
            Src::Trigger(i) => trig[i],
        };
        loop {
            let mut moved = false;
            for (i, t) in self.triggers.iter().enumerate() {
                if !trig[i] && self.met(search, s, had, &t.when) {
                    trig[i] = true;
                    merge(had, &t.gives);
                    moved = true;
                }
            }
            for (i, e) in self.entries.iter().enumerate() {
                if entry[i] || !reached[e.node] {
                    continue;
                }
                let verb = e
                    .answers
                    .is_none_or(|a| search.m.given_verbs.iter().chain(&had.verbs).any(|&v| answers_of(v) == Some(a)));
                let shown = e.shown_by.is_empty() || e.shown_by.iter().any(|x| on(x, &entry, &trig));
                let open = e.unlocked_by.is_empty() || e.unlocked_by.iter().any(|x| on(x, &entry, &trig));
                if verb && shown && open && self.met(search, s, had, &e.when) {
                    entry[i] = true;
                    merge(had, &e.gives);
                    moved = true;
                }
            }
            if !moved {
                return;
            }
        }
    }
}
