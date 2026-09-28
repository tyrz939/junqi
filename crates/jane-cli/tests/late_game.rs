//! The late game is no slower than the early game (PORT.md §9.4's tick rows, `jane bench sim`):
//! the Reader plays seed 7 for two hours from New Game, and the last half hour's step p50 and p99
//! and whole frame (step, bot and the tape's hash) are held to `tools/perf/thresholds.json`'s
//! `sim` rows. Wall time, and a minute or so of it in release, so ignored by default and run in
//! release, by hand before a gate and by the `bench` job when its runners exist:
//! `cargo test --release -p jane-cli --test late_game -- --ignored`.

use std::process::Command;

#[test]
#[ignore = "slow: a two-hour session timed against the tick budget, run in release"]
fn the_late_game_keeps_the_tick_budget() {
    let out = Command::new(env!("CARGO_BIN_EXE_jane"))
        .args(["bench", "sim", "--model", "reader", "--seeds", "7", "--minutes", "120", "--gate"])
        .output()
        .expect("jane bench sim runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "jane bench sim --gate failed:\n{text}\n{err}");
}
