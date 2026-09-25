//! Mechanisms: levers, plates, switches, controls. A prop with a `use` list and no loot that she
//! can stand beside is worked: once, or (a control) once in every layer she reaches it in. A
//! plate is worked like a lever: the solver assumes a barrel reaches it (check C11 proves one
//! can). A lever that works only once (`once`) gets no second pull: a `then` it could not run
//! yet is not kept.

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    for i in 0..s.bp.props.len() {
        let p = &s.bp.props[i];
        if s.props[i].withheld || p.use_list.is_none() || !p.loot.is_empty() || s.def(i).answers.is_some() {
            continue;
        }
        let Some(at) = s.at(i) else { continue };
        if s.held_shut(i, at) {
            continue;
        }
        changed |= s.work(i, at);
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::action::{Cond, Condition, FlagOp, FlagTest};
    use jane_core::{Action, ZoneId};

    use super::*;
    use crate::solve::sketch::Sketch;

    /// `verbs2.test.ts` "the solver reads both branches: a `then` waits for its flag, and a lever
    /// that works once does not get a second pull", at the pass: the lever keeps its `then`.
    #[test]
    fn a_lever_runs_its_list_once_and_keeps_a_then_that_cannot_hold_yet() {
        for (def, kept) in [("lever", 1), ("hoist_lever", 0)] {
            let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "######"]);
            k.mark("start", 1, 1);
            let g = k.prop("g", "gate_v", 4, 0);
            k.bp.props[g].locked = true;
            let gate = k.bp.props[g].key;
            let power = k.flag("test_power");
            let when = k.conds(vec![Cond { not: false, c: Condition::Flag { key: power, test: FlagTest::NonZero } }]);
            let then = k.list(vec![Action::Unlock(gate)]);
            let pulled = k.flag("test_pulled");
            let l =
                k.list(vec![Action::Flag { key: pulled, op: FlagOp::Add(1) }, Action::If { when, then, els: None }]);
            let sw = k.prop("sw", def, 2, 1);
            k.bp.props[sw].use_list = Some(l);
            k.with(|s| {
                s.flood_all();
                assert!(run(s));
                assert_eq!(s.flags.get(&pulled), Some(&1));
                assert!(!s.props[g].open);
                assert_eq!(s.deferred.len(), kept, "{def}");
                assert!(!run(s), "worked once");
                assert_eq!(s.flags.get(&pulled), Some(&1));
            });
        }
    }

    #[test]
    fn a_plate_is_worked_like_a_lever_and_a_barrel_is_no_wall() {
        let mut k = Sketch::new(ZoneId::Arms, &["########", "#......#", "#......#", "########"]);
        k.mark("start", 1, 1);
        let g = k.prop("g", "gate_v", 6, 0);
        k.bp.props[g].locked = true;
        let gate = k.bp.props[g].key;
        let l = k.list(vec![Action::Unlock(gate)]);
        k.prop("barrel", "barrel", 2, 1);
        let plate = k.prop("plate", "plate", 4, 1);
        k.bp.props[plate].use_list = Some(l);
        k.with(|s| {
            s.flood_all();
            assert!(s.layers.seen_any(5, 1), "a barrel is pushed, not walked round");
            assert!(run(s));
            assert!(s.props[g].open && s.opened);
        });
    }

    #[test]
    fn a_withheld_prop_is_never_worked() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "######"]);
        k.mark("start", 1, 1);
        let flag = k.flag("test_power");
        let l = k.list(vec![Action::Flag { key: flag, op: FlagOp::Set(1) }]);
        let sw = k.prop("sw", "lever", 2, 1);
        k.bp.props[sw].use_list = Some(l);
        let key = k.bp.props[sw].key;
        k.opts.withhold.props.push(key);
        k.with(|s| {
            s.flood_all();
            assert!(!run(s));
            assert_eq!(s.flags.get(&flag), None);
        });
    }
}
