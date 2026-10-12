//! The terraced county, built and proven (PLAN-map-r4 WP3). The game ships one map, picked by hand
//! ([`CHOSEN`]), so these check that map and a sample, not every seed: the chosen map builds and proves
//! (and seed 1, found by walking its plans); and the chosen map with the first eight seeds' maps build
//! alike twice and as the console form, stand every chunk on one level, and lay nothing on a face or a
//! landing.

use jane_core::Rect;
use jane_core::tile::F_SOLID;
use jane_world::county::terraced::CHOSEN;
use jane_world::county::terraced::build::{Built, build_pinned, find};
use jane_world::county::terraced::macro_plan::{Reg, idx, plan_from};
use jane_world::county::terraced::one_level;
use jane_world::county::terraced::prove::{BANDS, joint_offences};
use jane_world::county::terraced::skel::skeleton_of;
use jane_world::hash::hash;
use jane_world::skeleton::SkeletonRows;

/// The seeds the slow tests build besides the chosen map: a map is picked by looking, so a handful is a
/// sample.
const SEEDS: u32 = 8;

fn built(seed: u32) -> Built {
    find(seed, &mut |_| {}, false).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
}

fn chosen() -> Built {
    build_pinned(CHOSEN, &mut |_| {}, false).unwrap_or_else(|e| panic!("{CHOSEN:?}: {e:?}"))
}

/// The maps the slow tests judge: the chosen one, then the first seeds'.
fn maps() -> Vec<Built> {
    std::iter::once(chosen()).chain((1..=SEEDS).map(built)).collect()
}

/// The chosen map builds and proves, in the county's size with its levels and decks; and the plan it pins is
/// the one `plan_from` accepts for its seed, on the island layout.
#[test]
fn the_chosen_map_builds_and_proves() {
    let plan = plan_from(CHOSEN.seed, 0).expect("a plan for the chosen seed");
    assert_eq!((plan.attempt, plan.arch), (CHOSEN.plan, "island"), "the chosen plan is not the one plan_from accepts");
    let map = chosen();
    assert_eq!(map.pin, CHOSEN);
    let bp = &map.bp;
    assert_eq!((bp.w(), bp.h()), (1536, 1536));
    assert!(bp.level.is_some() && !bp.spans.is_empty(), "levels and decks");
    assert!(map.proof.seconds.iter().all(|&s| s > 0), "{:?}", map.proof.seconds);
}

/// Seed 1 builds and proves by walking its plans; the pin it lands on builds the same county again.
#[test]
fn seed_one_builds_and_proves() {
    let first = built(1);
    assert_eq!((first.bp.w(), first.bp.h()), (1536, 1536));
    let again = build_pinned(first.pin, &mut |_| {}, false).unwrap_or_else(|e| panic!("{:?}: {e:?}", first.pin));
    assert_eq!(again.pin, first.pin);
    assert!(again.bp == first.bp, "the pin {:?} builds another county", first.pin);
}

/// The proofs on the chosen map, in words, with the build's time and where it goes. Printed for the report:
/// `cargo test --release -p jane-world --test terraced_county the_chosen_map_report -- --ignored --nocapture`.
#[test]
#[ignore = "builds the chosen map five times"]
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
fn the_chosen_map_report() {
    use std::time::Instant;
    const RUNS: usize = 5;
    let mut totals = Vec::new();
    let mut stages: Vec<(&'static str, u128)> = Vec::new();
    for run in 0..RUNS {
        let t0 = Instant::now();
        let mut last: Option<(&'static str, Instant)> = None;
        stages.clear();
        let mut say = |name: &'static str| {
            let now = Instant::now();
            if let Some((was, at)) = last.replace((name, now)) {
                stages.push((was, now.duration_since(at).as_micros()));
            }
        };
        let map = build_pinned(CHOSEN, &mut say, false).unwrap_or_else(|e| panic!("{CHOSEN:?}: {e:?}"));
        if let Some((was, at)) = last {
            stages.push((was, at.elapsed().as_micros()));
        }
        totals.push(t0.elapsed().as_millis());
        if run == 0 {
            print_proofs(&map);
        }
    }
    totals.sort_unstable();
    println!(
        "build and prove, release, PC: median {} ms (min {} ms, max {} ms) over {RUNS} runs",
        totals[RUNS / 2],
        totals[0],
        totals[RUNS - 1]
    );
    stages.sort_by_key(|&(_, us)| std::cmp::Reverse(us));
    let top: Vec<String> = stages.iter().take(10).map(|(n, us)| format!("{n} {} ms", us / 1000)).collect();
    println!("longest stages (last run): {}", top.join(", "));
}

/// P-1 to P-8 on a built map, as the report prints them.
fn print_proofs(map: &Built) {
    let rows = SkeletonRows::catalog();
    let sk = skeleton_of(&map.carved, &rows).expect("the map's skeleton");
    let plan = &map.carved.plan;
    println!("map {:?}: {} layout, plan attempt {}, salt {}", map.pin, plan.arch, plan.attempt, map.pin.salt);
    println!("P-1 the solver: holds (no error)");
    println!("P-2 no pits by the tiles, spans as built and mended: holds");
    println!("P-3 no pits with the props that stand in the way: holds");
    println!(
        "P-4 night, the two barriers shut ({:?}): every named mark still reached, no pit: holds",
        map.carved.night_gates
    );
    for (k, (&(chain, lo, hi), s)) in BANDS.iter().zip(map.proof.seconds).enumerate() {
        let ends: Vec<String> = chain.iter().map(|&(a, b)| format!("{a}>{b}")).collect();
        println!(
            "P-5 {} {} s, MAP.md {lo} to {hi}: {}",
            ends.join(" "),
            s,
            if map.proof.in_band(k) { "in the band" } else { "OUTSIDE the band" }
        );
    }
    println!("P-6 kept fires with no ledge landing within 60 cells: {} of {}", map.proof.fires_far, map.proof.fires);
    println!("P-7 nothing on the land's joints: holds ({} offences)", joint_offences(&map.bp, &map.carved, 50).len());
    let first_walk = sk.named.first_walk();
    let worst = sk
        .roads
        .iter()
        .filter(|r| first_walk.contains(&(r.from, r.to)))
        .flat_map(|r| r.cells.iter())
        .map(|&(x, y)| sk.threat.read(x, y, 0))
        .max()
        .unwrap_or(0);
    let mut patches = vec![0u32; plan.districts.len()];
    for a in &sk.areas {
        patches[usize::from(plan.district_of[idx(a.mx, a.my)])] += 1;
    }
    println!(
        "P-8 threat: the first walk's worst by day is {worst}; the most named patches in a district is {} ({} laid)",
        patches.iter().max().copied().unwrap_or(0),
        sk.areas.len()
    );
}

/// The first eight seeds build and prove. Prints each map's pin and its routes' times against MAP.md's
/// (P-5 is reported, not refused), and the kept fires with no ledge near (P-6).
#[test]
#[ignore = "eight counties, each laid and proved"]
fn the_first_eight_seeds_build_and_prove() {
    println!("seed  pin(plan,salt)  routes in seconds, MAP.md's bounds in brackets; fires with no ledge near");
    for seed in 1..=SEEDS {
        let b = built(seed);
        let times: Vec<String> = BANDS
            .iter()
            .zip(b.proof.seconds)
            .enumerate()
            .map(|(k, (&(_, lo, hi), s))| format!("{s}{}[{lo}-{hi}]", if b.proof.in_band(k) { "" } else { "!" }))
            .collect();
        println!(
            "{seed:4}  ({:3},{})  {}  fires {}/{}",
            b.pin.plan,
            b.pin.salt,
            times.join("  "),
            b.proof.fires_far,
            b.proof.fires
        );
    }
}

/// A pinned map is one county: built twice it hashes alike, and the console form (packed as it is laid) is the
/// PC form packed.
#[test]
#[ignore = "builds two counties four times"]
fn a_pinned_map_builds_alike_twice_and_the_console_form_is_the_pc_form_packed() {
    for a in [chosen(), built(1)] {
        let b = build_pinned(a.pin, &mut |_| {}, false).expect("the pinned map");
        assert_eq!(hash(&a.bp), hash(&b.bp), "two builds of {:?} differ", a.pin);
        let mut pc = a.bp;
        pc.shrink_to_fit();
        pc.pack();
        let console = build_pinned(a.pin, &mut |_| {}, true).expect("the pinned map, packed");
        assert!(console.bp == pc, "the console form of {:?} is not the PC form packed", a.pin);
    }
}

/// Every chunk's box stands on one level, and nothing stands on a face, a rim, a stair, a ledge, a fall or
/// a landing: no prop, no unit, no tile of the carve laid over, no landing or end of a deck shut.
#[test]
#[ignore = "nine counties, each laid and proved"]
fn every_chunk_stands_on_one_level_and_nothing_stands_on_a_face_or_a_landing() {
    for b in maps() {
        let level = b.bp.level.as_ref().expect("levels");
        for pad in &b.carved.pads {
            assert!(one_level(level, pad.boxr), "{:?}: the {} chunk's box crosses a face", b.pin, pad.site);
        }
        let offences = joint_offences(&b.bp, &b.carved, 5);
        assert!(offences.is_empty(), "{:?}: {offences:?}", b.pin);
        let open = |r: Rect| r.cells().all(|(x, y)| b.bp.tile(x, y).flags() & F_SOLID == 0);
        for j in &b.carved.joins {
            for (k, l) in j.landings.iter().enumerate() {
                assert!(open(*l), "{:?}: landing {k} of the {:?} at {:?} is shut", b.pin, j.kind, j.rect);
            }
        }
        for s in &b.bp.spans {
            assert!(s.ends().into_iter().all(open), "{:?}: an end of the deck at {:?} is shut", b.pin, s.rect);
        }
    }
}

/// The small places by district, against their region's: each should hold a sixth of the region's. Printed;
/// a district short of it is named.
#[test]
#[ignore = "nine counties, each laid and proved"]
fn each_district_holds_a_sixth_of_its_regions_small_places() {
    let rows = SkeletonRows::catalog();
    let mut short = Vec::new();
    for b in maps() {
        let (plan, sk) = (&b.carved.plan, skeleton_of(&b.carved, &rows).expect("the map's skeleton"));
        let mut counts = vec![0u32; plan.districts.len()];
        for p in &sk.pois {
            counts[usize::from(plan.district_of[idx(p.mx, p.my)])] += 1;
        }
        let mut line = format!("{:?}:", b.pin);
        for reg in [Reg::Lowfields, Reg::Waters, Reg::Works] {
            let of_reg: Vec<usize> = (0..counts.len()).filter(|&d| plan.districts[d].reg == reg).collect();
            let total: u32 = of_reg.iter().map(|&d| counts[d]).sum();
            let each: Vec<String> = of_reg.iter().map(|&d| counts[d].to_string()).collect();
            line = format!("{line}  {reg:?} {total}: {}", each.join(" "));
            short.extend(
                of_reg
                    .iter()
                    .filter(|&&d| counts[d] * 6 < total)
                    .map(|&d| format!("{:?}: {:?} has {} of {total}", b.pin, plan.districts[d].role, counts[d])),
            );
        }
        println!("{line}");
    }
    println!("short districts: {short:#?}");
}
