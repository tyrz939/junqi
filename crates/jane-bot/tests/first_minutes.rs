//! PORT.md §7 P4's gate: the bot plays the first minutes on three seeds, both models, from New
//! Game on the station platform (`first_walk.rs` scripts the same walk from Julie's gate; this
//! plays it): the station road, Julie's stoop (the letter), the dog and its quest, the yard
//! skeleton, the house key, the kitchen, the note and the potion, the orb, and down to the cellar
//! for the dog's rats (VERIFICATION.md §4.1's first-hour rows).
//!
//! Each run is twelve game minutes, the time the first hour's bands give the Reader to reach the
//! orb; what each run reached, and when, is printed as a table. What every run must reach is
//! asserted: the letter handed in at the stoop and the kitchen stood in.

mod common;

use common::*;
use jane_bot::Model;
use jane_core::ZoneId;

/// Twelve game minutes.
const FRAMES: u32 = 12 * MINUTE;

fn play(model: Model) {
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for seed in SEEDS {
        let (rec, bot) = story(seed, model, FRAMES);
        let row = format!(
            "{seed} {:<6} stoop {} dog {} skeleton {} kitchen {} note {} potion {} icebolt {} rats {} cellar {} deaths {} ticks {}",
            model.name(),
            clock(done_at(&bot, "the_letter")),
            clock(given_at(&bot, "defeat_skeleton")),
            clock(done_at(&bot, "defeat_skeleton")),
            clock(done_at(&bot, "see_the_kitchen")),
            clock(given_at(&bot, "stock_the_bench")),
            clock(done_at(&bot, "stock_the_bench")),
            clock(learned_at(&bot, "icebolt")),
            clock(given_at(&bot, "rats_below")),
            clock(entered_at(&bot, ZoneId::Cellar)),
            deaths(&bot),
            rec.sim().state().tick.0,
        );
        println!("{row}");
        if done_at(&bot, "the_letter").is_none() || done_at(&bot, "see_the_kitchen").is_none() {
            failures.push(row.clone());
            for m in &bot.log {
                println!("    {}", m.line());
            }
        }
        rows.push(row);
    }
    assert!(failures.is_empty(), "short of the kitchen:\n{}", failures.join("\n"));
}

#[test]
fn the_reader_plays_the_first_minutes_on_three_seeds() {
    play(Model::Reader);
}

#[test]
fn the_rusher_plays_the_first_minutes_on_three_seeds() {
    play(Model::Rusher);
}
