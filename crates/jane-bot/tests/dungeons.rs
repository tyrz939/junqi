//! PORT.md §7 P4's gate: the bot plays every dungeon on three seeds (`dungeons.test.ts`, played
//! rather than scripted): set down at the dungeon's county door by the console with the kit a
//! player would carry there (`crawl::setup`: its given verbs and keys, the county door's key,
//! the growth the dungeons and errands before it offer (`crawl::growth_before`), apples and three
//! potions, ten in the morning; inside, where the other way in arrives, when the door's key is
//! found inside), it goes in, works the dungeon's locks, chests, plates and verbs, fights what it
//! meets, the boss last, and walks out.
//!
//! What each run came to is printed, one row per seed: when it got in, the bosses it put down,
//! when it got out, how often it died, and where it stopped and why. A dungeon the crawl cannot
//! finish is a finding about the bot or the game, reported here as it stands, not hidden: the
//! test holds every run to getting in and to ending in a known state (out, or stopped with a
//! reason), and the two dungeons without a boss that it finishes (the cellar, the library) to
//! finishing.

mod common;

use common::*;
use jane_bot::crawl::{self, Stage};
use jane_bot::{Mark, Model, Plan};
use jane_core::ZoneId;

/// Twenty game minutes a dungeon.
const FRAMES: u32 = 20 * MINUTE;

fn play(z: ZoneId, must_finish: bool) {
    let cat = jane_data::catalog();
    let mut problems = Vec::new();
    for seed in SEEDS {
        let (rec, bot) = common::crawl(seed, Model::Reader, z, FRAMES);
        let Plan::Crawl(c) = &bot.plan else { unreachable!() };
        let bosses: Vec<String> =
            c.bosses.iter().map(|&(d, t)| format!("{} {}", cat.combat.unit(d).id, clock(Some(t)))).collect();
        let boss = crawl::boss_of(z).map(|b| cat.combat.unit(b).id);
        let won = crawl::boss_of(z).is_none_or(|b| c.bosses.iter().any(|&(d, _)| d == b));
        let left = c.stage == Stage::Done && c.left.is_some();
        let why = match (&c.stuck, c.stage) {
            (Some(s), _) => s.clone(),
            (None, Stage::Done) => String::new(),
            (None, _) => {
                let v = rec.sim().view(jane_sim::Seat(0)).expect("seat 0");
                format!(
                    "out of time ({}): {}",
                    clock(Some(rec.sim().state().tick.0)),
                    if v.zone() == z { c.why_stuck(&v) } else { String::new() }
                )
            }
        };
        println!(
            "{:<8} seed {seed}: in {} | boss {} down: {:?} | out {} | deaths {} | {}{}",
            z.name(),
            clock(c.entered),
            boss.unwrap_or("none"),
            bosses,
            clock(c.left),
            bot.log.iter().filter(|m| m.mark == Mark::Died).count(),
            if won && left { "FINISHED" } else { "stopped: " },
            if won && left { "" } else { why.as_str() },
        );
        if c.entered.is_none() {
            problems.push(format!("seed {seed}: never got in ({why})"));
        }
        if c.stage != Stage::Done && c.stuck.is_none() && why.is_empty() {
            problems.push(format!("seed {seed}: ended in no known state"));
        }
        if must_finish && !(won && left) {
            problems.push(format!("seed {seed}: did not finish ({why})"));
        }
    }
    assert!(problems.is_empty(), "{}: {}", z.name(), problems.join("; "));
}

#[test]
fn julies_cellar() {
    play(ZoneId::Cellar, true);
}

#[test]
fn the_library() {
    play(ZoneId::Library, true);
}

#[test]
fn the_gold_mine() {
    play(ZoneId::Mine, false);
}

#[test]
fn the_museum() {
    play(ZoneId::Museum, false);
}

#[test]
fn butterfly_forest() {
    play(ZoneId::Forest, false);
}

#[test]
fn the_pipes() {
    play(ZoneId::Pipes, true);
}

#[test]
fn the_factory() {
    play(ZoneId::Factory, true);
}

#[test]
fn the_burial_chamber() {
    play(ZoneId::Burial, false);
}

#[test]
fn the_school() {
    play(ZoneId::School, false);
}
