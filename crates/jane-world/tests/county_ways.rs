//! The ways walk clear (`county::ways`): over a sweep of seeds, every footpath, lane and link lane
//! is clear of growth, fences and solid props (a prop's feet as `data/prop_feet.json` has them: a
//! tree's trunk on a way still stops her); no way runs through a fence without a gate or ends
//! against a fence, a wall or a building's blind side; a lane laid for a door begins at its step;
//! and every door keeps the cells in front of it clear. Seeds 1 to 8 in the fast tier, 1 to 48 in
//! the slow one. The table prints with `-- --nocapture`.

use jane_world::county::ways::{Audit, audit};
use jane_world::county::{County, STAGES, county_skeleton};

/// How many of each seed's complaints to print: `WAYS_NOTES` (12 by default).
#[allow(clippy::disallowed_methods)]
fn notes() -> usize {
    std::env::var("WAYS_NOTES").ok().and_then(|s| s.parse().ok()).unwrap_or(12)
}

fn audit_seed(seed: u32) -> Audit {
    let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
    let mut c = County::new(&sk, 0);
    for (_, stage) in STAGES {
        stage(&mut c);
    }
    audit(&c)
}

fn sweep(seeds: std::ops::RangeInclusive<u32>) {
    let seeds: Vec<u32> = seeds.collect();
    let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
    let per = seeds.len().div_ceil(threads).max(1);
    let all: Vec<(u32, Audit)> = std::thread::scope(|s| {
        let jobs: Vec<_> = seeds
            .chunks(per)
            .map(|part| s.spawn(move || part.iter().map(|&seed| (seed, audit_seed(seed))).collect::<Vec<_>>()))
            .collect();
        jobs.into_iter().flat_map(|j| j.join().expect("a county builds")).collect()
    });
    println!("seed  tile  prop  fence  blind  short  door");
    let mut bad = Vec::new();
    for (seed, a) in &all {
        println!(
            "{seed:>4}  {:>4}  {:>4}  {:>5}  {:>5}  {:>5}  {:>4}",
            a.tile_blocked, a.prop_blocked, a.fence_crossed, a.blind_end, a.short_of_door, a.door_blocked
        );
        bad.extend(a.notes.iter().take(notes()).map(|n| format!("seed {seed}: {n}")));
    }
    let total: u32 = all.iter().map(|(_, a)| a.total()).sum();
    assert!(total == 0, "{total} faults on the ways:\n{}", bad.join("\n"));
}

#[test]
fn every_way_is_clear_and_meets_its_place() {
    sweep(1..=8);
}

#[test]
#[ignore = "slow: forty-eight counties"]
fn every_way_is_clear_and_meets_its_place_on_many_seeds() {
    sweep(1..=48);
}
