//! Triggers, once any cell of their rect is reached. Only the "everything is dead" half of a
//! lock-in matters to reachability: the solver assumes fights are won, so a `while` row fires
//! once its conditions can hold (a flag some fired list set, a unit that was killed), and an
//! `enter` row's locks are skipped. What an `enter` row stands up is still there to be killed.

use jane_core::Action;
use jane_core::blueprint::TriggerMode;

use super::Changed;
use crate::solve::model::Solve;
use crate::solve::rows;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    for n in 0..s.triggers.len() {
        let row = s.triggers[n];
        if row.fired {
            continue;
        }
        let Some(&r) = s.bp.rects.get(&row.t.rect) else { continue };
        if r.w <= 0 || r.h <= 0 || !s.layers.touches_any((r.x, r.y, r.right() - 1, r.bottom() - 1)) {
            continue;
        }
        match row.t.mode {
            TriggerMode::While => {
                let when = row.t.when.map_or(&[][..], |w| s.conds(w));
                if !when.iter().all(|c| s.holds(c)) {
                    continue;
                }
                s.triggers[n].fired = true;
                s.apply_actions(s.list(row.t.actions), 0, !row.t.once, false);
                changed = true;
            }
            TriggerMode::Enter => {
                s.triggers[n].fired = true;
                let before = s.waiting.len();
                let mut spawns = Vec::new();
                rows::each_action(s.bp, s.cat, row.t.actions, &mut |a| {
                    if matches!(a, Action::Spawn { .. }) {
                        spawns.push(*a);
                    }
                });
                for a in &spawns {
                    s.apply_action(a, false);
                }
                changed |= s.waiting.len() > before;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;
    use jane_core::action::{Cond, Condition, FlagOp, FlagTest};
    use jane_core::blueprint::Trigger;

    use super::*;
    use crate::solve::sketch::Sketch;

    #[test]
    fn an_enter_row_stands_its_units_up_and_skips_its_locks() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "######"]);
        k.mark("start", 1, 1);
        let at = k.mark("lockin_a", 4, 1);
        let room = k.rect("arena", 3, 1, 2, 1);
        let g = k.prop("g", "gate_v", 2, 0);
        let gate = k.bp.props[g].key;
        let unit = k.name("lock_in_1");
        let def = k.cat.combat.unit_id("skeleton").unwrap();
        let l = k.list(vec![Action::Lock(gate), Action::Spawn { key: unit, def, at }]);
        let name = k.name("seal");
        k.bp.triggers.insert(
            name,
            Trigger { rect: room, mode: TriggerMode::Enter, once: true, when: None, actions: l, reset: None },
        );
        k.with(|s| {
            s.flood_all();
            assert!(run(s));
            assert_eq!(s.waiting.len(), 1);
            assert!(!s.bp.props[g].locked && !s.locked_in(g, 0));
            assert!(!run(s), "fired once");
        });
    }

    #[test]
    fn a_while_row_waits_for_its_conditions() {
        let mut k = Sketch::new(ZoneId::Arms, &["######", "#....#", "######"]);
        k.mark("start", 1, 1);
        let room = k.rect("arena", 3, 1, 2, 1);
        let boss = k.name("boss");
        let done = k.flag("test_done");
        let when = k.conds(vec![Cond { not: false, c: Condition::Dead(boss) }]);
        let l = k.list(vec![Action::Flag { key: done, op: FlagOp::Set(1) }]);
        let name = k.name("cleared");
        k.bp.triggers.insert(
            name,
            Trigger { rect: room, mode: TriggerMode::While, once: true, when: Some(when), actions: l, reset: None },
        );
        // A negated condition never blocks: it held earlier if at all.
        let never = k.flag("test_never");
        let not = k.conds(vec![Cond { not: true, c: Condition::Flag { key: never, test: FlagTest::NonZero } }]);
        let l2 = k.list(vec![Action::Flag { key: never, op: FlagOp::Set(1) }]);
        let name2 = k.name("not_yet");
        k.bp.triggers.insert(
            name2,
            Trigger { rect: room, mode: TriggerMode::While, once: true, when: Some(not), actions: l2, reset: None },
        );
        k.with(|s| {
            s.flood_all();
            assert!(run(s), "the negated row fires");
            assert_eq!(s.flags.get(&never), Some(&1));
            assert_eq!(s.flags.get(&done), None);
            s.dead.insert(boss);
            assert!(run(s));
            assert_eq!(s.flags.get(&done), Some(&1));
        });
    }
}
