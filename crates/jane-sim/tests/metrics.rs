//! `Sim::metrics` (ARCHITECTURE.md §11): what the last step cost is presentation only. A sim lent
//! a wall clock and one without step the same tape to the same hash, frame for frame; the counts
//! are filled either way, the times only with a clock, and the shape holds one time per phase.

mod common;

use common::{Tape, new_game, run};
use jane_sim::Phase;

/// A clock that moves a fixed amount every read: deterministic, and never zero.
fn fake_clock() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static T: AtomicU64 = AtomicU64::new(0);
    T.fetch_add(1000, Ordering::Relaxed)
}

#[test]
fn a_wall_clock_never_moves_the_state() {
    let (mut a, mut b) = (new_game(), new_game());
    b.set_wall_clock(Some(fake_clock));
    let (mut ta, mut tb) = (Tape::new(3), Tape::new(3));
    for f in 0..1200 {
        run(&mut a, &mut ta, f, f + 1);
        run(&mut b, &mut tb, f, f + 1);
        if f % 60 == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(a.hash(), b.hash());
}

#[test]
fn the_counts_are_kept_and_the_times_need_a_clock() {
    let mut s = new_game();
    let mut t = Tape::new(4);
    t.travel = false;
    t.guests = false;
    run(&mut s, &mut t, 0, 120);
    let m = s.metrics();
    assert!(m.ran);
    assert_eq!(m.frame + 1, s.state().frame, "the step it measured is the last one");
    assert!(m.zones_live >= 1);
    assert!(m.units_total >= m.units_awake && m.units_total > 0, "{m:?}");
    assert_eq!(m.step_ns, 0, "no clock, no times");
    assert!(m.phase_ns.iter().all(|&t| t == 0));

    s.set_wall_clock(Some(fake_clock));
    run(&mut s, &mut t, 120, 121);
    let m = s.metrics();
    assert!(m.step_ns > 0, "{m:?}");
    let phases: u64 = m.phase_ns.iter().map(|&t| u64::from(t)).sum();
    assert!(phases > 0 && phases <= u64::from(m.step_ns), "the phases sum inside the step: {m:?}");
    assert_eq!(Phase::ALL.len(), jane_sim::metrics::PHASES);
    for (i, p) in Phase::ALL.iter().enumerate() {
        assert_eq!(p.index(), i);
        assert!(!p.name().is_empty());
    }
}
