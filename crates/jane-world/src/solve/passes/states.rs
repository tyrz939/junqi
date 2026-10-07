//! THE STATEFUL FLOOD (DUNGEONS.md §2.6, ENGINE.md "The stateful flood"). A building with a
//! breaker is two buildings. The flood's nodes are (cell, layer): one layer of "seen" for every
//! combination of the zone's state flags, at most eight. Walking keeps the layer. A CONTROL (a
//! prop whose `use` sets a state flag; never a plate, which is a moment, not a state) reached in
//! one layer is an edge to the layer its list leads to, and that layer's flood starts AT THE
//! CONTROL: she is where she stood when she pulled it.
//!
//! What controls lock, unlock, show and hide is how the layers differ, worked out on the flags
//! alone before anyone walks ([`prepare`]); a layer that looks different depending on how it was
//! reached is refused by name. Keys, loot, kills, spells and every other list stay monotone and
//! shared between layers.
//!
//! This is not a sweep pass: [`prepare`] runs once before the first flood, and [`edges`] runs
//! inside every flood (`flood.rs`), after each layer is flooded.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use jane_core::action::{Action, FlagOp};

use crate::solve::model::{Governed, Solve};
use crate::solve::report::SolveError;
use crate::solve::rows::{self, MAX_DEPTH};

/// Find the controls and work out every layer a pull can lead to from how the zone starts.
pub(crate) fn prepare(s: &mut Solve<'_>) -> Vec<SolveError> {
    let n = s.bp.props.len();
    s.states.governed = vec![None; s.layers.count];
    s.states.governed[0] = Some(vec![Governed::default(); n]);
    if s.states.flags.is_empty() {
        return Vec::new();
    }
    for i in 0..n {
        let p = &s.bp.props[i];
        let (Some(l), true, false) = (p.use_list, p.loot.is_empty(), s.def(i).plate) else { continue };
        let mut sets = false;
        rows::each_action(s.bp, s.cat, l, &mut |a| {
            if let Action::Flag { key, .. } = *a {
                sets |= s.states.bit(key).is_some();
            }
        });
        if sets {
            s.states.controls.push(i);
            s.props[i].control = true;
        }
    }
    let mut errors = Vec::new();
    let mut todo = VecDeque::from([0usize]);
    while let Some(from) = todo.pop_front() {
        for c in s.states.controls.clone() {
            let mut next = s.states.governed[from].clone().unwrap_or_default();
            let to = pull(s, s.use_of(c), from, Some(&mut next));
            if to == from {
                continue;
            }
            let Some(there) = &s.states.governed[to] else {
                s.states.governed[to] = Some(next);
                todo.push_back(to);
                continue;
            };
            // A state must look the same however it was reached, or "lit" means nothing. That
            // includes the state the zone starts in: two pulls must leave the building as it was made.
            for i in 0..n {
                let made = (to != 0).then_some(there.as_slice());
                if standing(s, Some(&next), i) != standing(s, made, i) {
                    let e = SolveError::StateLeftDifferently { prop: s.bp.props[i].key, control: s.bp.props[c].key };
                    if !errors.contains(&e) {
                        errors.push(e);
                    }
                }
            }
        }
    }
    errors
}

/// How a prop stands in a layer: what the lists left it as, over how the blueprint made it.
fn standing(s: &Solve<'_>, g: Option<&[Governed]>, i: usize) -> (bool, bool) {
    let p = &s.bp.props[i];
    let g = g.map(|g| g[i]).unwrap_or_default();
    (g.locked.unwrap_or(p.locked), g.hidden.unwrap_or(p.hidden))
}

/// Run a control's list on the flags alone: the layer it leads to from `from`, and (given a
/// map) what it does to props on the way. Conditions about a state are asked of the state as
/// the list began; the rest are assumed.
pub(crate) fn pull(s: &Solve<'_>, list: &[Action], from: usize, mut into: Option<&mut Vec<Governed>>) -> usize {
    let mut to = from;
    walk(s, list, from, &mut to, &mut into, 0);
    to
}

fn walk(s: &Solve<'_>, list: &[Action], from: usize, to: &mut usize, into: &mut Option<&mut Vec<Governed>>, depth: u8) {
    if depth > MAX_DEPTH {
        return;
    }
    for a in list {
        match *a {
            Action::If { when, then, els } => {
                if s.conds(when).iter().all(|c| s.state_holds(c, from).unwrap_or(true)) {
                    walk(s, s.list(then), from, to, into, depth + 1);
                } else if let Some(e) = els {
                    walk(s, s.list(e), from, to, into, depth + 1);
                }
            }
            Action::Flag { key, op } => {
                let Some(bit) = s.states.bit(key) else { continue };
                let v = match op {
                    FlagOp::Add(x) => ((*to >> bit) & 1) as i32 + x != 0,
                    FlagOp::Set(x) => x != 0,
                };
                *to = if v { *to | (1 << bit) } else { *to & !(1 << bit) };
            }
            Action::Lock(k) | Action::Unlock(k) | Action::Show(k) | Action::Hide(k) => {
                let (Some(g), Some(&i)) = (into.as_deref_mut(), s.prop_ix.get(&k)) else { continue };
                match *a {
                    Action::Lock(_) => g[i].locked = Some(true),
                    Action::Unlock(_) => g[i].locked = Some(false),
                    Action::Show(_) => g[i].hidden = Some(false),
                    _ => g[i].hidden = Some(true),
                }
            }
            _ => {}
        }
    }
}

/// After layer `s` is flooded: every control she can work in it, standing beside it, seeds the
/// layer it leads to with the cells round it that she reached and that are still floor there.
pub(crate) fn edges(sv: &mut Solve<'_>, s: usize, seeds: &mut [Vec<(i32, i32)>], dirty: &mut VecDeque<usize>) {
    for ci in 0..sv.states.controls.len() {
        let c = sv.states.controls[ci];
        if !sv.can_work(c, s) || !sv.touches_in(c, s) {
            continue;
        }
        let pass = sv.pass;
        if let Some(t) = sv.trail.as_mut() {
            t.edges.entry(c).or_insert(pass + 1);
        }
        let to = pull(sv, sv.use_of(c), s, None);
        if to == s || sv.states.governed[to].is_none() {
            continue;
        }
        let (x0, y0, x1, y1) = sv.ring(c);
        let l = &sv.layers;
        let mut any = false;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !l.inside(x, y) {
                    continue;
                }
                let i = l.ix(x, y);
                if l.seen[s][i] == 0 || l.seen[to][i] != 0 || l.blocked[to][i] != 0 {
                    continue;
                }
                seeds[to].push((x, y));
                any = true;
            }
        }
        if any && !dirty.contains(&to) {
            dirty.push_back(to);
        }
    }
}

#[cfg(test)]
mod tests {
    use jane_core::action::{Cond, Condition, FlagTest};
    use jane_core::{Key, ZoneId};

    use super::*;
    use crate::solve::sketch::Sketch;

    /// A breaker: one `if` on the flag, both branches set it and swing the shutter.
    fn breaker(k: &mut Sketch, shutter: Key, lopsided: bool) -> usize {
        let lit = k.flag("test_lit");
        let when = k.conds(vec![Cond { not: false, c: Condition::Flag { key: lit, test: FlagTest::NonZero } }]);
        let off = if lopsided {
            vec![Action::Flag { key: lit, op: FlagOp::Set(0) }]
        } else {
            vec![Action::Flag { key: lit, op: FlagOp::Set(0) }, Action::Lock(shutter)]
        };
        let then = k.list(off);
        let els = k.list(vec![Action::Flag { key: lit, op: FlagOp::Set(1) }, Action::Unlock(shutter)]);
        let l = k.list(vec![Action::If { when, then, els: Some(els) }]);
        let b = k.prop("breaker", "lever", 1, 2);
        k.bp.props[b].use_list = Some(l);
        k.rules.states = vec![lit];
        b
    }

    #[test]
    fn prepare_finds_the_control_and_both_layers() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "#....#", "######"]);
        k.mark("start", 1, 1);
        let g = k.prop("shutter", "gate_v", 3, 0);
        k.bp.props[g].locked = true;
        let shutter = k.bp.props[g].key;
        let b = breaker(&mut k, shutter, false);
        k.with(|s| {
            assert_eq!(s.states.controls, vec![b]);
            assert!(s.props[b].control);
            let lit = s.states.governed[1].as_ref().expect("lit is reachable");
            assert_eq!(lit[g], Governed { locked: Some(false), hidden: None });
            assert!(s.locked_in(g, 0) && !s.locked_in(g, 1));
            assert_eq!(pull(s, s.use_of(b), 1, None), 0);
        });
    }

    #[test]
    fn a_state_that_looks_different_depending_on_how_it_was_reached_is_refused_by_name() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "#....#", "######"]);
        k.mark("start", 1, 1);
        let g = k.prop("shutter", "gate_v", 3, 0);
        k.bp.props[g].locked = true;
        let shutter = k.bp.props[g].key;
        let b = breaker(&mut k, shutter, true);
        let r = k.validate();
        assert_eq!(r.errors, vec![SolveError::StateLeftDifferently { prop: shutter, control: k.bp.props[b].key }]);
        assert!(r.lines(&k.bp)[0].contains("\"shutter\" is left differently"));
    }

    #[test]
    fn a_pull_seeds_the_next_layer_at_the_control() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "#....#", "######"]);
        k.mark("start", 1, 1);
        let g = k.prop("shutter", "gate_v", 3, 0);
        k.bp.props[g].locked = true;
        let shutter = k.bp.props[g].key;
        breaker(&mut k, shutter, false);
        k.with(|s| {
            s.flood_all();
            assert!(s.layers.reached[0] && s.layers.reached[1]);
            assert!(s.layers.seen[0][s.layers.ix(4, 1)] == 0, "shut in the dark");
            assert!(s.layers.seen[1][s.layers.ix(4, 1)] != 0, "open with the lights on");
        });
    }
}
