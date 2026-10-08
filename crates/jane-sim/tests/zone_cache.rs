//! The blueprint cache (PORT.md §13.13, `jane_sim::zone_cache`): what it reads back is what was
//! built, field for field and hash for hash, and a file damaged or stale in any way is refused
//! and the zone rebuilt.

use jane_core::ZoneId;
use jane_sim::Blueprints;
use jane_sim::zone_cache::{MemStore, Tally, ZoneCache, file_name};
use jane_world::hash::hash_packed;

/// Every zone of `seed` built packed, kept, then read back by a cache that builds nothing.
fn round_trip(seed: u32) {
    let built = Blueprints::build_packed_with(seed, &mut |_| {}).expect("builds");
    let mut cache = ZoneCache::new(MemStore::default());
    let (cold, t) = cache.blueprints(seed, &mut |_| {}).expect("builds");
    assert_eq!(t, Tally { read: 0, built: 13, unwritten: 0 });
    let (warm, t) = cache.blueprints(seed, &mut |_| panic!("a kept seed builds nothing")).expect("reads");
    assert_eq!(t, Tally { read: 13, built: 0, unwritten: 0 });
    for z in ZoneId::ALL {
        let (b, c, w) = (built.get(z), cold.get(z), warm.get(z));
        assert!(**c == **b, "seed {seed}: {} built through the cache differs", z.name());
        assert!(**w == **b, "seed {seed}: {} read back differs", z.name());
        assert_eq!(hash_packed(w), hash_packed(b), "seed {seed}: {} hash", z.name());
    }
    let bytes: usize = cache.store.files.values().map(Vec::len).sum();
    println!(
        "seed {seed}: {bytes} bytes kept, the county {}",
        cache.store.files[&file_name(seed, ZoneId::County)].len()
    );
}

#[test]
fn every_zone_reads_back_as_built() {
    round_trip(1);
}

#[test]
#[ignore = "slow: three more seeds' counties built twice in a dev build"]
fn every_zone_reads_back_as_built_on_more_seeds() {
    for seed in [2, 3, 4] {
        round_trip(seed);
    }
}

/// A file changed by `damage` is refused: the zone is not read back.
fn refused(cache: &mut ZoneCache<MemStore>, seed: u32, zone: ZoneId, damage: impl Fn(&mut Vec<u8>)) {
    let name = file_name(seed, zone);
    let good = cache.store.files[&name].clone();
    let mut bad = good.clone();
    damage(&mut bad);
    cache.store.files.insert(name.clone(), bad);
    assert!(cache.load_zone(seed, zone).is_none(), "{} damaged was read back", zone.name());
    cache.store.files.insert(name, good);
    assert!(cache.load_zone(seed, zone).is_some(), "{} restored reads back", zone.name());
}

#[test]
fn a_damaged_or_stale_file_is_refused() {
    let seed = 1;
    let mut cache = ZoneCache::new(MemStore::default());
    for z in [ZoneId::House, ZoneId::Church] {
        let bp = jane_sim::blueprints::build_one_packed_with(z, seed, &mut |_| {}).expect("builds");
        assert!(cache.store_zone(seed, &bp));
        assert!(cache.load_zone(seed, z).is_some_and(|r| r == bp));
    }
    let z = ZoneId::House;
    let flip = |at: usize| move |b: &mut Vec<u8>| b[at] ^= 0x10;
    refused(&mut cache, seed, z, flip(0)); // the magic
    refused(&mut cache, seed, z, flip(4)); // the format
    refused(&mut cache, seed, z, flip(6)); // the zone
    refused(&mut cache, seed, z, flip(8)); // the seed
    refused(&mut cache, seed, z, flip(12)); // the worldgen's stamp
    refused(&mut cache, seed, z, flip(20)); // the content hash
    refused(&mut cache, seed, z, flip(28)); // the blueprint's hash
    refused(&mut cache, seed, z, flip(36)); // the body's length
    refused(&mut cache, seed, z, flip(40)); // the body's sum
    refused(&mut cache, seed, z, |b| {
        let at = b.len() / 2;
        b[at] ^= 1;
    }); // the body
    refused(&mut cache, seed, z, |b| b.truncate(b.len() - 1));
    refused(&mut cache, seed, z, |b| b.truncate(20));
    refused(&mut cache, seed, z, Vec::clear);
    // Another zone's file under this zone's name.
    let church = cache.store.files[&file_name(seed, ZoneId::Church)].clone();
    refused(&mut cache, seed, z, |b| b.clone_from(&church));
    // A body that sums but decodes to another blueprint: the hash refuses it. (A body re-summed
    // after a change: the stale file a careless writer might leave.)
    refused(&mut cache, seed, z, |b| {
        let body = &mut b[48..];
        let at = body.len() - 1;
        body[at] ^= 1;
        let sum = xxhash_rust::xxh3::xxh3_64(body);
        b[40..48].copy_from_slice(&sum.to_le_bytes());
    });
    // Another seed's file under this seed's name.
    let other = jane_sim::blueprints::build_one_packed_with(z, 2, &mut |_| {}).expect("builds");
    let mut two = ZoneCache::new(MemStore::default());
    assert!(two.store_zone(2, &other));
    let theirs = two.store.files[&file_name(2, z)].clone();
    refused(&mut cache, seed, z, |b| b.clone_from(&theirs));
}

#[test]
fn a_refused_zone_is_rebuilt_and_kept_again() {
    let seed = 1;
    let mut cache = ZoneCache::new(MemStore::default());
    let (built, _) = cache.blueprints(seed, &mut |_| {}).expect("builds");
    let name = file_name(seed, ZoneId::Museum);
    let f = cache.store.files.get_mut(&name).expect("kept");
    let at = f.len() - 3;
    f[at] ^= 0xff;
    let (again, t) = cache.blueprints(seed, &mut |_| {}).expect("builds");
    assert_eq!(t, Tally { read: 12, built: 1, unwritten: 0 });
    for z in ZoneId::ALL {
        assert!(**again.get(z) == **built.get(z), "{}", z.name());
    }
    let (_, t) = cache.blueprints(seed, &mut |_| {}).expect("reads");
    assert_eq!(t, Tally { read: 13, built: 0, unwritten: 0 }, "the rebuilt zone was kept");
}

#[test]
fn a_built_blueprint_is_not_kept() {
    // Only the console form is kept; PC's grids are not this cache's.
    let bp = jane_sim::blueprints::build_one(ZoneId::House, 1).expect("builds");
    let mut cache = ZoneCache::new(MemStore::default());
    assert!(!cache.store_zone(1, &bp));
    assert!(cache.store.files.is_empty());
}
