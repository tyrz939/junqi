//! `jane gen --zones <dungeons|all|id,id,..> [--seeds A..B | --seed N] [--hash]`: build each zone
//! for each seed through `jane_world::build_zone` (every candidate proven by the solver and, for a
//! dungeon, checks C1 to C12) and print a line per build.
//!
//! With `--hash` the line is `<zone> <seed> <hash>`: the blueprint's 64-bit hash
//! (`jane_world::hash`, FNV-1a 64 over explicit little-endian bytes) as 16 hex digits, after a
//! `#` header naming the content hash and the hash layout. That is the determinism gate's first
//! half (PORT.md §9.3 1-2): the same seed hashes the same in one process, in a fresh one, and on
//! every target, so `tests/fixtures/hashes-<target>.txt` diff byte for byte. Without `--hash` the
//! line gives attempts, rooms, props, units and the build time.
//!
//! Seeds are inclusive here: `--seeds 1..64` is sixty-four seeds. `--zones all` is all thirteen
//! zones in tick order, the county first (proven and re-rolled like any other: its line's
//! `attempts` says how many candidates it took); `--zones county` builds it alone. A zone with no
//! builder would be named on stderr and skipped; today there is none.

use std::time::Instant;

use jane_core::ZoneId;

pub const USAGE: &str = "  gen --zones <dungeons|all|id,id..> [--seeds A..B | --seed N] [--hash]
                                      build each zone for each seed (A..B inclusive), proven; with --hash
                                      print `zone seed hash` lines for the cross-target determinism gate";

pub(crate) fn zones(which: &str) -> Result<Vec<ZoneId>, String> {
    let dungeons = || {
        ZoneId::ALL.into_iter().filter(|&z| jane_data::catalog().dungeons.mission_of(z).is_some()).collect::<Vec<_>>()
    };
    match which {
        "all" => Ok(ZoneId::ALL.to_vec()),
        "dungeons" => Ok(dungeons()),
        list => list.split(',').map(|s| ZoneId::from_name(s).ok_or_else(|| format!("no zone \"{s}\""))).collect(),
    }
}

pub(crate) fn seeds(args: &[String]) -> Result<std::ops::RangeInclusive<u32>, String> {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1));
    if let Some(s) = get("--seed") {
        let n: u32 = s.parse().map_err(|_| format!("bad seed {s}"))?;
        return Ok(n..=n);
    }
    let s = get("--seeds").map_or("1..64", String::as_str);
    let (a, b) = s.split_once("..").ok_or_else(|| format!("bad range {s}"))?;
    let bad = || format!("bad range {s}");
    Ok(a.parse().map_err(|_| bad())?..=b.trim_start_matches('=').parse().map_err(|_| bad())?)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let which =
        args.iter().position(|a| a == "--zones").and_then(|i| args.get(i + 1)).map_or("dungeons", String::as_str);
    let hash = args.iter().any(|a| a == "--hash");
    let zones = zones(which)?;
    let range = seeds(args)?;
    if hash {
        println!("# jane gen --zones {which} --seeds {}..{} --hash", range.start(), range.end());
        println!(
            "# content {:016x}, blueprint hash layout {}",
            jane_data::catalog().content_hash,
            jane_world::hash::LAYOUT
        );
        println!("# zone seed hash (FNV-1a 64 over little-endian bytes)");
    }
    for zone in zones {
        if !jane_world::builds(zone) {
            eprintln!("jane gen: {}: no builder yet, skipped", zone.name());
            continue;
        }
        for seed in range.clone() {
            let t0 = Instant::now();
            let Some(bp) = jane_world::build_zone(zone, seed) else { continue };
            let us = t0.elapsed().as_micros();
            if hash {
                println!("{} {seed} {:016x}", zone.name(), jane_world::hash::hash(&bp));
            } else {
                println!(
                    "{} {seed}: attempts {}, {} props, {} units, {}.{:03} ms",
                    zone.name(),
                    bp.attempts,
                    bp.props.len(),
                    bp.units.len(),
                    us / 1000,
                    us % 1000
                );
            }
        }
    }
    Ok(())
}
