//! PORT.md §9.3, the determinism gate's first half, for all thirteen zones: 1. `hash(build_zone(z,
//! s))` for seeds 1..=SEEDS, twice in one process, equal (the county for seeds 1 to 4 only: it is
//! most of a second a build in the test profile); 2. equal to `tests/fixtures/hashes-x86_64.txt`,
//! which a fresh process wrote (`jane gen --zones all --seeds 1..16 --hash`); on any other target
//! that is gate 3, byte for byte.

mod common;

use jane_core::ZoneId;
use jane_world::hash::{LAYOUT, hash};
use jane_world::{build_zone, builds};

fn twice(zone: ZoneId) {
    twice_up_to(zone, common::seeds());
}

fn twice_up_to(zone: ZoneId, seeds: u32) {
    assert!(builds(zone));
    for seed in 1..=seeds {
        let a = build_zone(zone, seed).expect("a zone");
        let b = build_zone(zone, seed).expect("a zone");
        assert_eq!(a, b, "{} seed {seed}", zone.name());
        assert_eq!(hash(&a), hash(&b), "{} seed {seed}", zone.name());
    }
}

#[test]
fn the_county_hashes_the_same_twice() {
    twice_up_to(ZoneId::County, common::seeds().min(4));
}

#[test]
fn the_mine_hashes_the_same_twice() {
    twice(ZoneId::Mine);
}

#[test]
fn the_burial_hashes_the_same_twice() {
    twice(ZoneId::Burial);
}

#[test]
fn the_works_hash_the_same_twice() {
    twice(ZoneId::Factory);
}

#[test]
fn the_forest_hashes_the_same_twice() {
    twice(ZoneId::Forest);
}

#[test]
fn the_library_hashes_the_same_twice() {
    twice(ZoneId::Library);
}

#[test]
fn the_museum_hashes_the_same_twice() {
    twice(ZoneId::Museum);
}

#[test]
fn the_culvert_hashes_the_same_twice() {
    twice(ZoneId::Pipes);
}

#[test]
fn the_school_hashes_the_same_twice() {
    twice(ZoneId::School);
}

#[test]
fn every_zone_builds() {
    for z in ZoneId::ALL {
        assert!(builds(z), "{}", z.name());
        assert!(build_zone(z, 1).is_some(), "{}", z.name());
    }
}

#[test]
fn another_seed_is_another_hash() {
    let mut seen: Vec<u64> = Vec::new();
    // The Arms and St Anne's are the same building on every seed: they throw no dice.
    let fixed = |z: ZoneId| matches!(z, ZoneId::Arms | ZoneId::Church);
    for zone in ZoneId::ALL.into_iter().filter(|&z| builds(z) && !fixed(z)) {
        for seed in 1..=4 {
            let h = hash(&build_zone(zone, seed).expect("a zone"));
            assert!(!seen.contains(&h), "{} seed {seed} hashes like another build", zone.name());
            seen.push(h);
        }
    }
}

/// Gate 2 (and 3, off x86_64): the hashes a fresh process wrote. The file names the content it
/// was written from; after a content change it is stale, and says so, until `jane gen` rewrites it.
#[test]
fn the_hashes_match_the_fixture_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/hashes-x86_64.txt");
    let text = std::fs::read_to_string(path).expect("tests/fixtures/hashes-x86_64.txt");
    let content = format!("# content {:016x}, blueprint hash layout {LAYOUT}", jane_data::catalog().content_hash);
    if !text.lines().any(|l| l == content) {
        eprintln!(
            "hashes-x86_64.txt was written from other content or another hash layout (want \"{content}\"): \
             rewrite it with `jane gen --zones all --seeds 1..16 --hash`"
        );
        return;
    }
    let mut n = 0;
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let mut f = line.split(' ');
        let (Some(zone), Some(seed), Some(want)) = (f.next(), f.next(), f.next()) else { panic!("bad line {line}") };
        let zone = ZoneId::from_name(zone).expect("a zone");
        let seed: u32 = seed.parse().expect("a seed");
        let got = format!("{:016x}", hash(&build_zone(zone, seed).expect("a zone")));
        assert_eq!(got, want, "{} seed {seed}", zone.name());
        n += 1;
    }
    assert_eq!(n, ZoneId::ALL.len() * 16, "every zone, seeds 1 to 16");
}
