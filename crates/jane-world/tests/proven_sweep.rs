//! Grok #1: a zone the solver refused is never handed out. Over many seeds no test chose (New
//! Game seeds from the clock), every zone either proves (and the solver, asked again, agrees)
//! or the seed is refused with `ZoneError::Unproven`, never a blueprint nobody proved.

use jane_core::ZoneId;
use jane_world::solve::{ZoneRules, validate};
use jane_world::{ZoneError, build_zone_with, interiors};

/// Seeds spread over the whole `u32` range, the same every run.
fn random_seeds(n: usize) -> Vec<u32> {
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (x >> 32) as u32
        })
        .collect()
}

/// One seed's verdict on one zone: `Ok(true)` proven and re-proven, `Ok(false)` refused.
fn judge(zone: ZoneId, seed: u32) -> Result<bool, String> {
    match build_zone_with(zone, seed, &mut |_| {}) {
        Ok(bp) => {
            if zone == ZoneId::County || interiors::is_interior(zone) {
                let v = validate(&bp, &ZoneRules::for_zone(zone));
                if !v.ok() {
                    return Err(format!("{} seed {seed}: handed out but the solver refuses it", zone.name()));
                }
            } else if !jane_world::dungeon::build(zone, seed).info.errors.is_empty() {
                return Err(format!("{} seed {seed}: handed out with errors", zone.name()));
            }
            Ok(true)
        }
        Err(ZoneError::Unproven(z)) if z == zone => Ok(false),
        Err(e) => Err(format!("{} seed {seed}: {e}", zone.name())),
    }
}

#[test]
#[ignore = "slow: every zone over 24 random seeds (release: cargo test --release -- --ignored)"]
fn every_zone_proves_or_the_seed_is_refused() {
    let seeds = random_seeds(24);
    let jobs: Vec<(ZoneId, u32)> = seeds
        .iter()
        .flat_map(|&s| ZoneId::ALL.iter().filter(|&&z| jane_world::builds(z)).map(move |&z| (z, s)))
        .collect();
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    let verdicts: Vec<(ZoneId, u32, Result<bool, String>)> = std::thread::scope(|s| {
        let chunks: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(threads))
            .map(|c| s.spawn(move || c.iter().map(|&(z, seed)| (z, seed, judge(z, seed))).collect::<Vec<_>>()))
            .collect();
        chunks.into_iter().flat_map(|h| h.join().expect("a sweep thread")).collect()
    });
    let mut faults = Vec::new();
    let mut refused = Vec::new();
    for (z, seed, v) in &verdicts {
        match v {
            Ok(true) => {}
            Ok(false) => refused.push(format!("{} {seed}", z.name())),
            Err(e) => faults.push(e.clone()),
        }
    }
    assert!(faults.is_empty(), "{faults:#?}");
    // A refusal is allowed (New Game re-rolls), but if many seeds were refused New Game would
    // re-roll often: hold it to fewer than one seed in eight.
    let refused_seeds = seeds.iter().filter(|s| refused.iter().any(|r| r.ends_with(&format!(" {s}")))).count();
    assert!(refused_seeds * 8 < seeds.len(), "{refused_seeds} of {} seeds refused: {refused:?}", seeds.len());
    eprintln!("{} zone builds, {} refused: {refused:?}", verdicts.len(), refused.len());
}
