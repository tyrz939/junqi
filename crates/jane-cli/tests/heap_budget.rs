//! The sim diet's memory gate (PORT.md §13.2, §13.3; slow tier): `jane bench heap` on seed 1, and
//! the county build's peak, the thirteen blueprints and the sim at New Game held under what was
//! measured when the diet last moved, about 5% over. A change that costs memory fails here; one
//! that saves it should ratchet these down. Requested bytes (`cap`), 64-bit host; a 32-bit target
//! asks for a little less.

use serde_json::Value;

/// `(key, bytes at most)`. Measured 2026-10-08 after diet phase 1: build peak 15 295 371 (from
/// 51.8 MB), blueprints 9 953 711 (from 11.5 MB). After phase 2: the sim at New Game 16 171 574
/// (from 21.0 MB), and the console form (`Blueprints::packed`): blueprints 5 306 350, the sim over
/// them 11 524 453 at New Game and 11 559 387 at its peak.
const BUDGETS: &[(&str, u64)] = &[
    ("build_peak", 16_100_000),
    ("blueprints_retained", 10_450_000),
    ("build_resident", 10_470_000),
    ("sim_new_game", 16_980_000),
    ("packed_blueprints", 5_570_000),
    ("packed_sim_new_game", 12_100_000),
    ("packed_sim_peak", 12_140_000),
];

/// PORT.md §13.2's targets, reported (not yet asserted for the sim).
const BUILD_TARGET: u64 = 20_000_000;
const SIM_TARGET: u64 = 6_000_000;

#[test]
#[ignore = "slow tier (release): builds seed 1's thirteen zones"]
fn the_build_and_the_sim_keep_to_their_heap() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jane"))
        .args(["bench", "heap", "--seed", "1", "--ticks", "60", "--json"])
        .output()
        .expect("jane runs");
    assert!(out.status.success(), "jane bench heap: {}", String::from_utf8_lossy(&out.stderr));
    let j: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let mut over = Vec::new();
    for &(key, budget) in BUDGETS {
        let b = j[key].as_u64().unwrap_or_else(|| panic!("no {key}"));
        if b > budget {
            over.push(format!("{key}: {b} bytes, budget {budget}"));
        }
    }
    let peak = j["build_peak"].as_u64().unwrap_or(0);
    let sim = j["packed_sim_new_game"].as_u64().unwrap_or(0);
    eprintln!(
        "county build peak {peak} bytes (target {BUILD_TARGET}); packed sim at New Game {sim} bytes (target {SIM_TARGET})"
    );
    assert!(peak <= BUILD_TARGET, "the county build peaks over PORT.md 13.2's {BUILD_TARGET} bytes: {peak}");
    assert!(over.is_empty(), "over budget:\n{}", over.join("\n"));
}
