//! VERIFICATION.md §4.1, the first hour, on seeds 1 to 3: each row that gives a number, measured
//! on the model's own trace (L4). Every row is printed; the rows in [`ENFORCED`] held on every
//! seed of the 28 September sweep (seeds 1 to 8) and are asserted here; the rest are recorded
//! (§6: a band is enforced only after it is recorded and read by the owner).
//!
//! Also held: the Explorer's first hour is not empty (§4.1 "nothing-to-see stretches": none over
//! two minutes, at most three over one), recorded.

mod common;

use common::*;
use jane_bot::experience::{self, Experience, FIRST_HOUR};
use jane_bot::run::Session;
use jane_bot::{Bot, Model};

/// §4.1 rows asserted, by claim and model.
const ENFORCED: [(&str, &str); 4] = [
    ("Julie's gate reached by the station road", "reader"),
    ("The house key from the dog, the kitchen entered", "reader"),
    ("Icebolt learned in the kitchen", "reader"),
    ("Time lost, first hour", "reader"),
];

/// The first hour of `model` on `seed`, traced, measured.
fn first_hour(seed: u32, model: Model) -> Experience {
    let mut sim = new_game(seed);
    let mut s = Session::new(Bot::story(model), &sim, 60);
    for _ in 0..60 * MINUTE {
        if s.bot.done() {
            break;
        }
        s.step(&mut sim);
    }
    let (_, t) = s.finish(&sim);
    experience::measure(&t, 0)
}

#[test]
fn the_first_hour_holds_its_bands() {
    let models = [Model::Reader, Model::Rusher, Model::Lost, Model::Explorer];
    let runs: Vec<Experience> = std::thread::scope(|sc| {
        let hs: Vec<_> = SEEDS
            .iter()
            .flat_map(|&seed| models.iter().map(move |&m| (seed, m)))
            .map(|(seed, m)| sc.spawn(move || first_hour(seed, m)))
            .collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    let mut bad = Vec::new();
    for b in &FIRST_HOUR {
        for &model in b.models {
            let mut cells = Vec::new();
            for x in runs.iter().filter(|x| x.model == model) {
                let v = experience::first_hour_value(x, b);
                let ok = experience::in_band(b, v);
                cells.push(format!(
                    "s{} {}{}",
                    x.seed,
                    v.map_or("never".into(), Experience::min),
                    if ok { "" } else { " OUT" }
                ));
                if !ok && ENFORCED.contains(&(b.claim, model)) {
                    bad.push(format!(
                        "seed {} {model}: {}: {}",
                        x.seed,
                        b.claim,
                        v.map_or("never".into(), Experience::min)
                    ));
                }
            }
            println!("{:<48} {model:<8} {}", b.claim, cells.join("  "));
        }
    }
    for x in runs.iter().filter(|x| x.model == "explorer") {
        println!(
            "explorer, seed {}: new ground with no new landmark, {} over 1 min, {} over 2 min (band: 3, 0)",
            x.seed,
            x.plain_over(60).count(),
            x.plain_over(120).count()
        );
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
