//! Zones built on demand (PORT.md §13.3): New Game builds the county alone, a zone is built as a
//! seat walks in and let go once nobody is in it, and nothing the sim steps, saves or hashes can
//! tell. Held to the bot fixture's file, to a story run's every hash and save, and to a zone built
//! again being the very blueprint.

mod common;

use std::sync::Arc;

use common::*;
use jane_bot::{Model, fixture};
use jane_core::ZoneId;
use jane_sim::blueprints::{Build, ZoneSource};
use jane_sim::replay::{input_at, verify_tape};
use jane_sim::{Blueprints, Seat, Sim, StepInput};

fn on_demand(seed: u32, packed: bool) -> Blueprints {
    Blueprints::on_demand_with(seed, Arc::new(Build { packed, load: None }), &mut |_| {}).expect("the county builds")
}

#[test]
fn a_zone_built_again_is_the_same_blueprint() {
    let seed = 1;
    let all = bps(seed);
    let src = Build { packed: false, load: None };
    let packed = Build { packed: true, load: None };
    for z in ZoneId::ALL.into_iter().filter(|&z| z != ZoneId::County) {
        let a = src.zone(seed, z, &mut |_| {}).expect("builds");
        let b = src.zone(seed, z, &mut |_| {}).expect("builds");
        let h = jane_world::hash::hash(&a);
        assert_eq!(h, jane_world::hash::hash(&b), "{z:?} built twice");
        assert_eq!(h, jane_world::hash::hash(all.get(z)), "{z:?} on demand and up front");
        // Only the county has areas or regions: what lets the world's rolls and skies run the
        // same over a zone not held (`Blueprints::hold`).
        assert!(a.areas.is_empty() && a.regions.is_empty(), "{z:?}");
        let p = packed.zone(seed, z, &mut |_| {}).expect("builds");
        assert_eq!(p, packed.zone(seed, z, &mut |_| {}).expect("builds"), "{z:?} packed, built twice");
        let mut q = a.clone();
        q.pack();
        assert_eq!(p, q, "{z:?}: built packed is the PC build packed");
    }
}

#[test]
fn the_bot_fixture_hashes_the_same_with_zones_built_on_demand() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/bot-hash-x86_64.txt");
    let text = std::fs::read_to_string(path).expect("tests/fixtures/bot-hash-x86_64.txt");
    let want: Vec<&str> = text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
    let mut got = Vec::new();
    for seed in fixture::SEEDS {
        for model in fixture::MODELS {
            let tape = fixture::session(on_demand(seed, seed == 2), model);
            got.extend(fixture::lines(seed, model, &tape));
        }
    }
    assert_eq!(got, want);
}

/// A story run recorded over every blueprint held, replayed over blueprints built on demand: each
/// step, the hash; whenever a zone has a state but no blueprint held, the save's bytes and each
/// zone's hash; at the end, loaded from its save over the on-demand set, the same hash.
#[test]
fn a_story_run_hashes_and_saves_the_same_with_zones_let_go() {
    for (seed, model, packed) in [(1, Model::Reader, false), (2, Model::Rusher, true)] {
        let (rec, _) = story(seed, model, 30 * MINUTE);
        let (sim, tape) = rec.finish();
        let mut a = tape.new_game_with(if packed { bps(seed).packed() } else { bps(seed) });
        let mut b = tape.new_game_with(on_demand(seed, packed));
        assert_eq!(b.blueprints().held_count(), 1, "New Game holds the county alone");
        let (mut let_go, mut most, mut offered) = (0, 1, 0);
        let src = Build { packed, load: None };
        for f in 0..tape.frames {
            let (frames, commands) = input_at(&tape, f).expect("a frame of the tape");
            let input = StepInput { frames, commands };
            // Half the time, the zones behind the doors near her are built ahead and handed in, as
            // a shell's loader thread would: invisible to every hash.
            if f % 240 == 0 && f / 240 % 2 == 0 {
                for z in b.zones_ahead(Seat(0), 24) {
                    b.offer_blueprint(Arc::new(src.zone(seed, z, &mut |_| {}).expect("builds")));
                    offered += 1;
                }
            }
            a.step(&input);
            b.step(&input);
            let held = b.blueprints().held_count();
            most = most.max(held);
            let gone = ZoneId::ALL.iter().any(|&z| b.state().zone(z).is_some() && b.blueprints().held_now(z).is_none());
            if gone && (f % 600 == 0 || f + 1 == tape.frames) {
                let_go += 1;
                assert_eq!(a.save(), b.save(), "seed {seed} frame {f}: the save");
                assert_eq!(a.zone_hashes(), b.zone_hashes(), "seed {seed} frame {f}");
            }
            if f % 60 == 0 {
                assert_eq!(a.hash(), b.hash(), "seed {seed} frame {f}");
            }
        }
        assert_eq!(a.hash(), sim.hash());
        assert_eq!(b.hash(), sim.hash());
        let visited: Vec<ZoneId> = ZoneId::ALL.into_iter().filter(|&z| b.state().zone(z).is_some()).collect();
        println!(
            "seed {seed}: zones made {visited:?}, at most {most} held, {let_go} checks with a zone let go, {offered} offered"
        );
        assert!(let_go > 0, "seed {seed}: no zone was let go in the run, so nothing was proved");
        let loaded = Sim::from_save_with(&b.save(), on_demand(seed, packed)).expect("loads");
        let back = Sim::from_save_with(&a.save(), if packed { bps(seed).packed() } else { bps(seed) }).expect("loads");
        assert_eq!(loaded.hash(), back.hash(), "seed {seed}: loaded");
        assert!(loaded.blueprints().held_count() <= 2, "seed {seed}: a load lets go of the zones nobody is in");
        // The tape, replayed on its own over the on-demand set, lands on every hash.
        verify_tape(&tape, on_demand(seed, packed)).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    }
}

/// What lets the save's name runs skip a zone never built (`save::sym_runs`), and an on-demand New
/// Game prove its county alone: each zone's generated names hold one no other zone has (and no
/// name the sim interns at run time, a zone's own or a made thing's `def#id`), so a zone never
/// interned can never match the table; and every zone of every seed tried builds.
#[test]
fn every_zone_builds_and_names_one_name_its_own() {
    let src = Build { packed: false, load: None };
    for seed in 1..=16 {
        let zones: Vec<_> = ZoneId::ALL
            .into_iter()
            .filter(|&z| z != ZoneId::County)
            .map(|z| src.zone(seed, z, &mut |_| {}).unwrap_or_else(|e| panic!("seed {seed}: {e}")))
            .collect();
        if seed > 3 {
            continue;
        }
        let mut all: Vec<String> = bps(seed).get(ZoneId::County).local_names.iter().map(str::to_owned).collect();
        for bp in &zones {
            all.extend(bp.local_names.iter().map(str::to_owned));
        }
        for bp in &zones {
            if bp.local_names.is_empty() {
                continue;
            }
            let own = bp.local_names.iter().any(|n| {
                all.iter().filter(|m| m.as_str() == n).count() == 1
                    && !n.contains('#')
                    && ZoneId::ALL.iter().all(|z| z.name() != n)
            });
            assert!(own, "seed {seed}: zone {:?} has no name of its own", bp.zone);
        }
    }
}
