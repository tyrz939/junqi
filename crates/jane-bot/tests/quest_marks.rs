//! The quest marks as a player meets them (PLAY-PLAN.md 0.4): on seeds 1 to 8, all through a
//! Reader's story, no place shows her more than three "!" at once (forty cells round any one of
//! them), whatever she has taken and left. Checked every ten game minutes and at each morning.
//!
//! A whole story per seed: `#[ignore]`d, the slow tier
//! (`cargo test --release -p jane-bot --test quest_marks -- --ignored`).

use jane_bot::{Bot, Model};
use jane_sim::{Blueprints, Seat, Sim};

/// "A place": forty cells round any "!".
const CROWD_CELLS: i32 = 40;
/// The most a place may show at once.
const CROWD_MAX: usize = 3;
/// As the story suite: forty game hours of frames.
const STORY_FRAMES: u32 = 40 * 60 * 60 * 60;
/// Ticks between looks: ten game minutes.
const EVERY: u32 = 10 * 3600;

/// A run's worst crowd: how many, at which tick, and where.
type Worst = (usize, u32, Vec<(i32, i32)>);

/// The worst crowd of a seed's run.
fn worst(seed: u32) -> Worst {
    let mut sim = Sim::new_game_with(Blueprints::build(seed).expect("the seed builds"), "Jane");
    let mut bot = Bot::story(Model::Reader);
    let mut worst = (0, 0, Vec::new());
    let (mut next, mut hour) = (0, u8::MAX);
    for _ in 0..STORY_FRAMES {
        if bot.done() {
            break;
        }
        bot.step(&mut sim);
        let v = sim.view(Seat(0)).expect("seat 0");
        let tick = sim.state().tick.0;
        // Every ten minutes, and the moment an hour turns (a night slept is no frames).
        if tick < next && v.hour() == hour {
            continue;
        }
        (next, hour) = (tick + EVERY, v.hour());
        let c = v.offer_crowd(CROWD_CELLS);
        if c.len() > worst.0 {
            worst = (c.len(), tick, c);
        }
    }
    worst
}

#[test]
#[ignore = "slow: a whole story on eight seeds, a minute or more each in release"]
fn no_place_shows_more_than_three_offers_through_a_reader_s_story() {
    let runs: Vec<(u32, Worst)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=8).map(|s| sc.spawn(move || (s, worst(s)))).collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    let mut bad = Vec::new();
    for (seed, (n, tick, at)) in &runs {
        println!("seed {seed}: at most {n} \"!\" in a place (tick {tick}) {at:?}");
        if *n > CROWD_MAX {
            bad.push(format!("seed {seed}: {n} at tick {tick}: {at:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
