//! The sim diet's memory gate (PORT.md §13.2, §13.3; slow tier): `jane bench heap` on seed 1, and
//! the county build's peak, the thirteen blueprints and the sim at New Game held under what was
//! measured when the diet last moved, about 5% over. A change that costs memory fails here; one
//! that saves it should ratchet these down. Requested bytes (`cap`), 64-bit host; a 32-bit target
//! asks for a little less.

use serde_json::Value;

/// `(key, bytes at most)`. Measured 2026-10-08 after diet phase 1: build peak 15 295 371 (from
/// 51.8 MB), blueprints 9 953 711 (from 11.5 MB). After phase 2: the sim at New Game 16 171 574
/// (from 21.0 MB), and the console form (`Blueprints::packed`): blueprints 5 306 350, the sim over
/// them 11 524 453 at New Game and 11 559 387 at its peak. After phase 3 (the console form now
/// built packed, `build_one_packed_with`): build peak 8 261 221, blueprints 3 582 874, the sim
/// 6 677 025 at New Game and 6 732 673 at its peak; PC's build peak 11 851 515, blueprints
/// 8 211 051, sim 11 318 194. Zones built on demand (the console form, the county alone at New
/// Game): the sim 6 179 825 at New Game, a zone's entry peaking at 8 332 606 (the museum's build),
/// and back in the county after all twelve were entered and let go 6 954 764.
const BUDGETS: &[(&str, u64)] = &[
    ("build_peak", 12_450_000),
    ("blueprints_retained", 8_620_000),
    ("build_resident", 8_640_000),
    ("sim_new_game", 11_890_000),
    ("packed_build_peak", 8_680_000),
    ("packed_blueprints", 3_770_000),
    ("packed_sim_new_game", 7_010_000),
    ("packed_sim_peak", 7_070_000),
    ("on_demand_sim_new_game", 6_490_000),
    ("on_demand_entry_peak", 8_750_000),
    ("on_demand_back_most", 7_300_000),
];

/// PORT.md §13.2's PSP-1000 targets for the console form, asserted: the world's build peak and the
/// sim at New Game. (The Dreamcast's 6 MB sim is still a goal, not a gate.)
const BUILD_TARGET: u64 = 9_000_000;
const SIM_TARGET: u64 = 7_000_000;

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
    let peak = j["packed_build_peak"].as_u64().unwrap_or(u64::MAX);
    let sim = j["packed_sim_new_game"].as_u64().unwrap_or(u64::MAX);
    eprintln!(
        "console build peak {peak} bytes (target {BUILD_TARGET}); packed sim at New Game {sim} bytes (target {SIM_TARGET})"
    );
    assert!(peak <= BUILD_TARGET, "the console build peaks over PORT.md 13.2's {BUILD_TARGET} bytes: {peak}");
    assert!(sim <= SIM_TARGET, "the console sim at New Game is over PORT.md 13.2's {SIM_TARGET} bytes: {sim}");
    let entry = j["on_demand_entry_peak"].as_u64().unwrap_or(u64::MAX);
    assert!(entry <= BUILD_TARGET, "a zone's entry on demand peaks over the build's {BUILD_TARGET} bytes: {entry}");
    assert!(over.is_empty(), "over budget:\n{}", over.join("\n"));
}
