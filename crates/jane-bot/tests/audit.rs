//! VERIFICATION.md §2 L5 on the built world, seeds 1 to 3: every quest step's thing stands
//! somewhere on the seed, and nothing a step or a "Back to ..." names by its own name is missing
//! from the county (the 2026 web build's bug: an anchor a quest named was never built). Steps whose
//! landmark is built but far from the thing, or whose thing is off the roads, are printed for the
//! owner (`jane audit --seed N`, the sweep's section 4): recorded, not enforced (§6).
//!
//! The omens: never two lethal ones of a region true on a seed, and every claim but those in
//! [`SAID_BY_THE_WORLD`] posted or said somewhere.

mod common;

use common::*;
use jane_bot::audit::{self, Verdict};

/// Omens whose claim no sign or person says on any seed: the world shows it and nothing tells
/// it (the sweep report lists them; a content decision, not a bug this test holds).
const SAID_BY_THE_WORLD: [&str; 1] = ["scarecrow_closer"];

#[test]
fn every_step_has_its_thing_and_every_name_it_says_is_built() {
    let mut bad = Vec::new();
    for seed in SEEDS {
        let steps = audit::steps(&bps(seed));
        for a in &steps {
            let step = if a.step == 255 { "back to".to_owned() } else { format!("step {}", a.step + 1) };
            match &a.verdict {
                Verdict::Nothing | Verdict::Unbuilt(_) => {
                    bad.push(format!("seed {seed}: {} {step}: \"{}\": {}", a.quest, a.text, a.verdict.word()));
                }
                v if v.bad() => println!("seed {seed}: {} {step}: \"{}\": {}", a.quest, a.text, v.word()),
                _ => {}
            }
        }
        println!(
            "seed {seed}: {} steps and hand-ins, {} with a finding",
            steps.len(),
            steps.iter().filter(|a| a.verdict.bad()).count()
        );
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_omens_keep_their_rules() {
    for seed in SEEDS {
        let os = audit::omens(&bps(seed));
        let problems: Vec<String> = audit::omen_problems(&os)
            .into_iter()
            .filter(|p| !SAID_BY_THE_WORLD.iter().any(|id| p.starts_with(&format!("omen {id}:"))))
            .collect();
        println!("seed {seed}: true {:?}", os.iter().filter(|o| o.true_here).map(|o| o.id).collect::<Vec<_>>());
        assert!(problems.is_empty(), "seed {seed}: {problems:?}");
    }
}

#[test]
fn the_audit_catches_what_it_is_for() {
    // A phrase is read the way the audit reads the words.
    let p = audit::phrases("A red glove, at the well on the station road");
    assert_eq!(p[0].0, "at");
    assert_eq!(p[0].1, ["well"]);
    assert_eq!(p[1].1, ["station", "road"]);
    let p = audit::phrases("The parcel, to the door facing the fire in Castle square");
    assert!(p.iter().any(|(prep, w, proper)| prep == "in" && w == &["castle", "square"] && proper == &["castle"]));
    // "by" is a short reach, "near" a longer one, a route's the longest.
    assert!(audit::reach("by") < audit::reach("near") && audit::reach("near") < audit::reach("from"));

    // The web build's bug, made on purpose: the well the book names is not built. The glove's
    // step, held on the real seed, fails on this one.
    let real = bps(1);
    let glove = |b: &jane_sim::Blueprints| {
        audit::steps(b)
            .into_iter()
            .find(|a| a.quest == "lost_property" && a.step == 0)
            .expect("the glove's step")
            .verdict
    };
    assert_eq!(glove(&real), Verdict::Ok, "the real seed builds the well by the glove");
    let cat = jane_data::catalog();
    let mut county = (**real.get(jane_core::ZoneId::County)).clone();
    county.props.retain(|p| cat.story.prop(p.def).id != "well_head");
    let mut zones: Vec<std::sync::Arc<jane_core::Blueprint>> =
        jane_core::ZoneId::ALL.iter().map(|&z| real.get(z).clone()).collect();
    zones[0] = std::sync::Arc::new(county);
    let broken = jane_sim::Blueprints::from_parts(1, zones.try_into().expect("thirteen"));
    let v = glove(&broken);
    println!("with no well built: {}", v.word());
    assert!(matches!(v, Verdict::Far(_) | Verdict::Unbuilt(_)), "{v:?}");
}
