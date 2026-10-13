//! The terraced county, built from its source and proven (PLAN-map-r4 WP3, PLAN-world-as-data): it
//! builds and proves, builds alike twice and as the console form, stands every chunk on one level, and lays
//! nothing on a face or a landing.

use jane_core::Rect;
use jane_core::tile::F_SOLID;
use jane_world::county::terraced::build::{Built, build};
use jane_world::county::terraced::macro_plan::idx;
use jane_world::county::terraced::one_level;
use jane_world::county::terraced::prove::{BANDS, joint_offences};
use jane_world::county::terraced::skel::skeleton_from;
use jane_world::county::terraced::source::county;
use jane_world::hash::hash;
use jane_world::skeleton::SkeletonRows;

fn built(pack: bool) -> Built {
    build(&county(), &mut |_| {}, pack).unwrap_or_else(|e| panic!("the county: {e:?}"))
}

/// The county builds and proves, in its size with its levels and decks.
#[test]
fn the_county_builds_and_proves() {
    let map = built(false);
    let bp = &map.bp;
    assert_eq!((bp.w(), bp.h()), (1536, 1536));
    assert!(bp.level.is_some() && !bp.spans.is_empty(), "levels and decks");
    assert!(map.proof.seconds.iter().all(|&s| s > 0), "{:?}", map.proof.seconds);
}

/// The county is one county: built twice it hashes alike, and the console form (packed as it is laid) is
/// the PC form packed.
#[test]
fn the_county_builds_alike_twice_and_the_console_form_is_the_pc_form_packed() {
    let (a, b) = (built(false), built(false));
    assert_eq!(hash(&a.bp), hash(&b.bp), "two builds differ");
    let mut pc = a.bp;
    pc.shrink_to_fit();
    pc.pack();
    assert!(built(true).bp == pc, "the console form is not the PC form packed");
}

/// Every chunk's box stands on one level, and nothing stands on a face, a rim, a stair, a ledge, a fall or
/// a landing: no prop, no unit, no tile of the carve laid over, no landing or end of a deck shut.
#[test]
fn every_chunk_stands_on_one_level_and_nothing_stands_on_a_face_or_a_landing() {
    let b = built(false);
    let level = b.bp.level.as_ref().expect("levels");
    for pad in &b.carved.pads {
        assert!(one_level(level, pad.boxr), "the {} chunk's box crosses a face", pad.site);
    }
    let offences = joint_offences(&b.bp, &b.carved, 5);
    assert!(offences.is_empty(), "{offences:?}");
    let open = |r: Rect| r.cells().all(|(x, y)| b.bp.tile(x, y).flags() & F_SOLID == 0);
    for j in &b.carved.joins {
        for (k, l) in j.landings.iter().enumerate() {
            assert!(open(*l), "landing {k} of the {:?} at {:?} is shut", j.kind, j.rect);
        }
    }
    for s in &b.bp.spans {
        assert!(s.ends().into_iter().all(open), "an end of the deck at {:?} is shut", s.rect);
    }
}

/// The proofs on the county, in words, with the build's time and where it goes. Printed for the report:
/// `cargo test --release -p jane-world --test terraced_county the_county_report -- --ignored --nocapture`.
#[test]
#[ignore = "builds the county five times"]
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
fn the_county_report() {
    use std::time::Instant;
    const RUNS: usize = 5;
    let src = county();
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
        let map = build(&src, &mut say, false).unwrap_or_else(|e| panic!("the county: {e:?}"));
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

/// P-1 to P-8 on the built county, as the report prints them.
fn print_proofs(map: &Built) {
    let src = county();
    let sk = skeleton_from(&map.carved, &SkeletonRows::catalog(), &src.places, &src.routes.lit).expect("the skeleton");
    let plan = &map.carved.plan;
    println!("the county: {} layout, salt {}", plan.arch, map.carved.salt);
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
