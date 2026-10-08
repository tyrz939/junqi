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

/// One store every `CacheSource` call opens, as a console's directory is .
#[derive(Debug)]
struct Shared;

// A test's stand-in for a directory a console opens by path: one map, behind a lock.
#[allow(clippy::disallowed_types)]
static FILES: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

impl jane_sim::zone_cache::Store for Shared {
    fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        FILES.lock().unwrap().get(name).cloned()
    }

    fn write(&mut self, name: &str, fill: &mut dyn FnMut(&mut dyn jane_sim::zone_cache::Sink) -> bool) -> bool {
        let mut m = MemStore::default();
        let ok = m.write(name, fill);
        if ok {
            FILES.lock().unwrap().extend(m.files);
        }
        ok
    }

    fn remove(&mut self, name: &str) {
        FILES.lock().unwrap().remove(name);
    }
}

/// On demand over the cache (PORT.md §13.3): New Game cold builds and keeps the county alone; a
/// zone entered is read if kept, else built and kept; New Game warm reads the county and builds
/// nothing; and the sim over it hashes as the sim over every zone built.
#[test]
fn on_demand_reads_and_keeps_each_zone() {
    use jane_sim::input::{Command, DevOp, StampedCommand};
    use jane_sim::zone_cache::CacheSource;
    use jane_sim::{InputFrame, Seat, Sim, StepInput};
    use std::sync::Arc;
    let seed = 1;
    let source = Arc::new(CacheSource::new((|| Shared) as fn() -> Shared));
    let (cold, t) = ZoneCache::new(Shared).on_demand(seed, &source, &mut |_| {}).expect("builds");
    assert_eq!(t, Tally { read: 0, built: 1, unwritten: 0 });
    assert_eq!(cold.held_count(), 1);
    let (warm, t) = ZoneCache::new(Shared)
        .on_demand(seed, &source, &mut |_| panic!("a kept county builds nothing"))
        .expect("reads");
    assert_eq!(t, Tally { read: 1, built: 0, unwritten: 0 });
    let all = Blueprints::build_packed_with(seed, &mut |_| {}).expect("builds");
    let (mut a, mut b) = (Sim::new_game_with(all, "Jane"), Sim::new_game_with(warm, "Jane"));
    let start = jane_sim::sym::of_name(jane_data::catalog().name_id("start").unwrap());
    let frames = [InputFrame::IDLE; 4];
    for (i, z) in [ZoneId::Burial, ZoneId::County, ZoneId::Burial, ZoneId::County].into_iter().enumerate() {
        let cmd = StampedCommand {
            seat: Some(Seat(0)),
            seq: i as u16 + 1,
            cmd: Command::Dev(DevOp::Tp { zone: z, mark: start }),
        };
        for s in [&mut a, &mut b] {
            s.step(&StepInput { frames, commands: &[cmd] });
            for _ in 0..30 {
                s.step(&StepInput { frames, commands: &[] });
            }
        }
        assert_eq!(a.hash(), b.hash(), "after travel {i} to {}", z.name());
        assert_eq!(a.save(), b.save());
    }
    // The Burial was built on the first entry and kept; the second read it back.
    assert_eq!(source.tally(), Tally { read: 2, built: 2, unwritten: 0 });
    assert!(FILES.lock().unwrap().contains_key(&file_name(seed, ZoneId::Burial)));
    drop(cold);
}

/// A zone entered mid-game is the same zone however it came (PORT.md §13.3): built on her step
/// (on demand, the cache empty), read back from the cache, or built with all thirteen at New
/// Game. A dev travel into each zone at tick 150 and back at 300, at 22:00: every state hash the
/// same, step for step.
#[test]
fn a_zone_entered_mid_game_is_the_same_built_read_or_held() {
    use jane_sim::blueprints::Build;
    use jane_sim::input::{Command, DevOp, StampedCommand};
    use jane_sim::zone_cache::CacheSource;
    use jane_sim::{InputFrame, Seat, Sim, StepInput};
    use std::sync::{Arc, OnceLock};
    static KEPT: OnceLock<MemStore> = OnceLock::new();
    let seed = 1;
    let all = Blueprints::build_packed_with(seed, &mut |_| {}).expect("builds");
    KEPT.get_or_init(|| {
        let mut c = ZoneCache::new(MemStore::default());
        for z in ZoneId::ALL {
            assert!(c.store_zone(seed, all.get(z)));
        }
        c.store
    });
    let kept = Arc::new(CacheSource::new((|| KEPT.get().expect("kept").clone()) as fn() -> MemStore));
    let cat = jane_data::catalog();
    let sym = |n: &str| jane_sim::sym::of_name(cat.name_id(n).unwrap_or_else(|| panic!("{n}")));
    for z in [ZoneId::Burial, ZoneId::Arms, ZoneId::House, ZoneId::Mine, ZoneId::Church, ZoneId::Cellar] {
        let built = Blueprints::on_demand_with(seed, Arc::new(Build { packed: true, load: None }), &mut |_| {})
            .expect("builds");
        let read = Blueprints::on_demand_with(seed, kept.clone(), &mut |_| {}).expect("reads");
        let mut sims = [all.clone(), built, read].map(|b| Sim::new_game_with(b, "Jane"));
        let frames = [InputFrame::IDLE; 4];
        let mut seq = 0u16;
        for t in 0..420u32 {
            let ops: Vec<DevOp> = match t {
                0 => vec![DevOp::Tp { zone: ZoneId::County, mark: sym("town_square") }, DevOp::Time { hour: 22 }],
                150 => vec![DevOp::Tp { zone: z, mark: sym("entry") }],
                300 => vec![DevOp::Tp { zone: ZoneId::County, mark: sym("town_square") }],
                _ => Vec::new(),
            };
            let commands: Vec<StampedCommand> = ops
                .into_iter()
                .map(|op| {
                    seq += 1;
                    StampedCommand { seat: Some(Seat(0)), seq, cmd: Command::Dev(op) }
                })
                .collect();
            for s in &mut sims {
                s.step(&StepInput { frames, commands: &commands });
            }
            let h = sims.each_ref().map(Sim::hash);
            assert!(
                h[0] == h[1] && h[0] == h[2],
                "{}: tick {t}: all {:016x} built {:016x} read {:016x}",
                z.name(),
                h[0],
                h[1],
                h[2]
            );
        }
        println!("{}: tick 420 {:016x}", z.name(), sims[0].hash());
    }
}
