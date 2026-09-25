//! Kept `then`s: an `if` whose conditions could not hold when its list ran, in a list that can
//! run again (a lever, a `while` row), runs its `then` as soon as they can.

use super::Changed;
use crate::solve::model::Solve;

pub fn run(s: &mut Solve<'_>) -> Changed {
    let mut changed = false;
    let mut n = s.deferred.len();
    while n > 0 {
        n -= 1;
        let d = s.deferred[n];
        if !s.conds(d.when).iter().all(|c| s.holds_in(c, d.layer)) {
            continue;
        }
        s.deferred.remove(n);
        s.apply_actions(s.list(d.then), d.layer, true, d.control);
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use jane_core::action::{Cond, Condition, FlagOp, FlagTest};
    use jane_core::{Action, ZoneId};

    use super::*;
    use crate::solve::model::Deferred;
    use crate::solve::sketch::Sketch;

    #[test]
    fn a_kept_then_runs_once_its_flag_is_set() {
        let mut k = Sketch::new(ZoneId::Arms, &["####", "#..#", "####"]);
        k.mark("start", 1, 1);
        let power = k.flag("test_power");
        let lit = k.flag("test_lit");
        let when = k.conds(vec![Cond { not: false, c: Condition::Flag { key: power, test: FlagTest::Min(2) } }]);
        let then = k.list(vec![Action::Flag { key: lit, op: FlagOp::Set(1) }]);
        k.with(|s| {
            s.deferred.push(Deferred { when, then, layer: 0, control: false });
            assert!(!run(s));
            s.flags.insert(power, 1);
            assert!(!run(s), "1 is not at least 2");
            s.flags.insert(power, 2);
            assert!(run(s));
            assert_eq!(s.flags.get(&lit), Some(&1));
            assert!(s.deferred.is_empty());
        });
    }
}
