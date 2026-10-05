//! The memory budgets (PLAY-PLAN.md §7, slow tier): `jane bench --mem` on `soft`, seed 1, a
//! tour of 12 hops, and each system held under its budget, so a regression shows. The budgets
//! sit a little over what was measured on 2026-10-05; the target is 64 MB of heap on `soft`
//! (then 32), and the test says how far off it is.

// Megabytes for a person to read.
#![allow(clippy::cast_precision_loss)]

use serde_json::Value;

const MB: u64 = 1024 * 1024;

/// `(row of "built", MB at most)`.
const BUILT: &[(&str, u64)] = &[
    ("blueprint: county", 10),
    ("blueprints: the other 12", 3),
    ("sim state and runtime grids", 11),
    ("atlas px held: albedo (u16 CLUT)", 36),
    ("atlas px held: lit layers", 0),
    ("terrain chunk cache", 9),
    ("presenter: the rest (looks, tables)", 8),
    ("ui", 1),
    ("renderer t0 (heap)", 1),
    ("audio engine", 7),
];

/// `(top-level number, MB at most)`.
const TOTALS: &[(&str, u64)] =
    &[("heap_halt", 90), ("heap_walk", 93), ("heap_peak", 116), ("worldgen_scratch", 52), ("rss_peak", 125)];

/// The goal for the heap at the Halt on `soft`.
const TARGET: u64 = 64;

#[test]
#[ignore = "slow tier: plays a seed and tours the county (about a minute in release)"]
fn soft_keeps_to_its_memory_budgets() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jane"))
        .args(["bench", "--mem", "--backend", "soft", "--seed", "1", "--hops", "12", "--json"])
        .output()
        .expect("jane runs");
    assert!(out.status.success(), "jane bench --mem: {}", String::from_utf8_lossy(&out.stderr));
    let j: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let mut over = Vec::new();
    for &(row, budget) in BUILT {
        let b = j["built"][row].as_u64().unwrap_or_else(|| panic!("no row {row}"));
        if b > budget * MB {
            over.push(format!("{row}: {:.1} MB, budget {budget}", b as f64 / MB as f64));
        }
    }
    for &(key, budget) in TOTALS {
        let b = j[key].as_u64().unwrap_or_else(|| panic!("no {key}"));
        if b > budget * MB {
            over.push(format!("{key}: {:.1} MB, budget {budget}", b as f64 / MB as f64));
        }
    }
    // After the walk: the chunk cache at most its 48 slots (a low sun's long band wants them).
    let chunks = j["sized_walk"]["terrain chunk cache"].as_u64().unwrap_or(u64::MAX);
    if chunks > 17 * MB {
        over.push(format!("terrain chunk cache after the walk: {:.1} MB, budget 17", chunks as f64 / MB as f64));
    }
    let halt = j["heap_halt"].as_u64().unwrap_or(0);
    eprintln!(
        "soft: heap at the Halt {:.1} MB, {:.1} MB over the {TARGET} MB target; peak {:.1} MB, resident peak {:.1} MB",
        halt as f64 / MB as f64,
        halt.saturating_sub(TARGET * MB) as f64 / MB as f64,
        j["heap_peak"].as_u64().unwrap_or(0) as f64 / MB as f64,
        j["rss_peak"].as_u64().unwrap_or(0) as f64 / MB as f64,
    );
    assert!(over.is_empty(), "over budget:\n{}", over.join("\n"));
}
