//! The determinism gates of ARCHITECTURE.md §8 for the sim's foundation. Carried from
//! `jane/test/sim.test.ts` "determinism" (same seed and inputs, same hash tick for tick; save,
//! load, continue equals never having saved) with a tape of movement, joins, leaves and zone
//! changes in place of the TS's casts. Run under `--profile checked` too (`overflow_free`).

mod common;

use common::{Tape, bps, new_game, run};
use jane_sim::{Seat, Sim, StepInput};

/// `same_tape_same_hash`: two sims on one seeded tape agree every 120 frames.
#[test]
fn same_tape_same_hash() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (Tape::new(11), Tape::new(11));
    let mut guests = 0;
    let mut zones = std::collections::BTreeSet::new();
    for chunk in 0..30 {
        run(&mut a, &mut ta, chunk * 120, (chunk + 1) * 120);
        run(&mut b, &mut tb, chunk * 120, (chunk + 1) * 120);
        assert_eq!(a.hash(), b.hash(), "frame {}", (chunk + 1) * 120);
        assert_eq!(a.zone_hashes(), b.zone_hashes());
        guests = guests.max(a.state().party_size());
        for p in a.state().connected() {
            zones.insert(p.zone);
        }
    }
    assert_eq!(a.state().frame, 3600);
    // The tape did what it says: company came, and the party went places.
    assert!(guests >= 2, "no guest ever sat down");
    assert!(zones.len() >= 3, "only {zones:?}");
}

#[test]
fn different_tapes_diverge_and_the_hash_sees_one_step() {
    let mut a = new_game();
    let mut b = new_game();
    assert_eq!(a.hash(), b.hash());
    let (mut ta, mut tb) = (Tape::new(1), Tape::new(2));
    run(&mut a, &mut ta, 0, 60);
    run(&mut b, &mut tb, 0, 60);
    assert_ne!(a.hash(), b.hash());
    let mut c = new_game();
    let h = c.hash();
    c.step(&StepInput::IDLE);
    assert_ne!(c.hash(), h, "the tick is in the hash");
}

/// `save_load_continue`: save at frames 131 and 257 of 520, load, continue; equal to the run
/// that never saved. Solo: a save is the host's world and parks every guest.
#[test]
fn save_load_continue() {
    let solo = || {
        let mut t = Tape::new(21);
        t.guests = false;
        t
    };
    let mut straight = new_game();
    let mut ts = solo();
    run(&mut straight, &mut ts, 0, 520);

    for at in [131, 257] {
        let mut first = new_game();
        let mut t = solo();
        run(&mut first, &mut t, 0, at);
        let bytes = first.save();
        let mut resumed = Sim::from_save_with(&bytes, bps()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash(), "loading changes nothing (frame {at})");
        run(&mut resumed, &mut t, at, 520);
        assert_eq!(resumed.hash(), straight.hash(), "saved at {at}");
        assert_eq!(resumed.state(), straight.state());
    }
}

/// `runtime_rebuild_is_invisible`: every runtime thrown away and rebuilt, every few frames;
/// the hash stream does not move.
#[test]
fn runtime_rebuild_is_invisible() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (Tape::new(33), Tape::new(33));
    for f in 0..1500 {
        run(&mut a, &mut ta, f, f + 1);
        if f % 97 == 13 {
            b.rebuild_runtimes();
        }
        run(&mut b, &mut tb, f, f + 1);
        if f % 60 == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(a.state(), b.state());
}

/// A runtime kept up tick by tick is the one a fresh build makes: occupancy, unit blocks,
/// awake sets, prop flags.
#[test]
fn a_live_runtime_equals_a_fresh_build() {
    let mut a = new_game();
    let mut t = Tape::new(44);
    t.guests = false;
    for chunk in 0..10 {
        run(&mut a, &mut t, chunk * 150, (chunk + 1) * 150);
        let fresh = Sim::from_save_with(&a.save(), bps()).unwrap();
        let host = a.state().players[0].zone;
        let (x, y) = (a.runtime(host).unwrap(), fresh.runtime(host).unwrap());
        assert_eq!(x.awake_units, y.awake_units);
        assert_eq!(x.awake_prop_list, y.awake_prop_list);
        assert_eq!(x.unit_blocks.len(), y.unit_blocks.len());
        assert_eq!(x.grid.occupied_cells(), y.grid.occupied_cells());
        let (w, h) = (x.grid.w() as i32, x.grid.h() as i32);
        let mut diffs = 0;
        for cy in 0..h {
            for cx in 0..w {
                diffs += u32::from(x.grid.flags_at(cx, cy) != y.grid.flags_at(cx, cy));
            }
        }
        assert_eq!(diffs, 0, "cells differ in {host:?}");
    }
}

/// The world clock: a game hour is a real minute (3600 ticks at 60 a second), a day is 24 real
/// minutes (WORLD.md §2.1), and the day rolls at midnight.
#[test]
fn the_clock_turns() {
    assert_eq!(jane_sim::tuning::TICKS_PER_HOUR, 60 * 60, "a game hour is one real minute");
    assert_eq!(jane_sim::tuning::TICKS_PER_DAY, 24 * 60 * 60, "a day is 24 real minutes");
    let mut s = new_game();
    let (clock0, day0) = (s.state().clock, s.state().day);
    assert_eq!(clock0, 17 * 3600);
    for _ in 0..7 * 3600 {
        s.step(&StepInput::IDLE);
    }
    assert_eq!(s.state().day, day0 + 1);
    assert_eq!(s.state().clock, 0);
    assert_eq!(s.state().tick.0, 7 * 3600);
    assert_eq!(s.view(Seat(0)).unwrap().clock(), (0, 1));
}
