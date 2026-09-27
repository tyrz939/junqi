//! The whole story, played (STORY.md §4, §10; VERIFICATION.md §2 L3): the Reader from New Game
//! through every act, dungeon, revelation and choice to one of the three endings, on seeds 1 to
//! 5, each ending on at least one of them; every run a tape that replays to its hashes; and
//! nothing the story needs set aside for longer than [`STUCK_BUDGET`].
//!
//! The endings are chosen by the bot's choice policy (`Ctx::ending`, `jane play --ending`): the
//! ring on the study desk, the seam at the back of the mine's vault, or the Sunday train.
//!
//! A whole story is a minute or more of release time per seed, so those runs are `#[ignore]`d:
//! CI runs them with `cargo test --release -p jane-bot --test story -- --ignored`. The three
//! endings played from the choice (the console putting a new game at Yours to Say) are quick,
//! and run always.

mod common;

use common::clock;
use jane_bot::{Bot, Ending, Mark, Model};
use jane_sim::replay::{Recorder, verify_tape};
use jane_sim::{Blueprints, Seat, Sim};

/// Frames a story may take before the test calls it stuck: forty game hours of play (the
/// Reader's whole stories run 800 to 1,200 minutes of the log's clock on the seeds that end,
/// deaths and all), and the nights slept away do not count against it (they are not frames).
const STORY_FRAMES: u32 = 40 * 60 * 60 * 60;

/// Game minutes a step of the spine may stay set aside before it counts as stuck for good: a
/// day and a half (a step set aside for the night, or for a door's hours, comes back to).
const STUCK_BUDGET: u32 = 36 * 60;

/// The spine, in the order the dog gives it.
const SPINE: [&str; 12] = [
    "the_letter",
    "defeat_skeleton",
    "see_the_kitchen",
    "stock_the_bench",
    "rats_below",
    "the_mine",
    "the_museum",
    "the_forest",
    "the_factory",
    "the_burial",
    "the_school",
    "the_choice",
];

fn bps(seed: u32) -> Blueprints {
    Blueprints::build(seed).expect("the seed builds")
}

/// The ending a seed's run chooses: the three in turn, so seeds 1 to 3 reach all three.
fn ending_for(seed: u32) -> Ending {
    [Ending::Hold, Ending::Hill, Ending::Train][(seed as usize + 2) % 3]
}

/// What the town notices of each act (STORY.md §11): the consequence rows each act's boss
/// fires, which must all have fired by the end of a story.
const NOTICES: [&str; 6] = ["mine_quiet", "wing_lit", "forest_quiet", "works_dark", "burial_quiet", "bell_stopped"];

/// What a whole run came to.
struct Run {
    seed: u32,
    ending: Ending,
    the_end: u8,
    frames: u32,
    ticks: u32,
    deaths: usize,
    done: Vec<String>,
    /// Spine steps set aside, with how long each stayed so (game minutes; `None`: never undone).
    stuck: Vec<(String, Option<u32>)>,
    log: Vec<String>,
    /// Every death, as the bot recorded it.
    death_lines: Vec<String>,
    /// §11's rows that had not fired by the end.
    unnoticed: Vec<&'static str>,
}

/// The Reader from New Game to the end, recorded, and its tape verified against its hashes.
fn play(seed: u32, ending: Ending) -> Run {
    let b = bps(seed);
    let mut rec = Recorder::new(Sim::new_game_with(b.clone(), "Jane"));
    let mut bot = Bot::story(Model::Reader);
    bot.ctx.ending = Some(ending);
    let frames = bot.play(&mut rec, STORY_FRAMES);
    let (sim, tape) = rec.finish();
    let v = sim.view(Seat(0)).expect("seat 0");
    let cat = jane_data::catalog();
    let done: Vec<String> = v.quests_done().iter().map(|&q| cat.story.quest(q).id.to_owned()).collect();
    // A step of the spine given up on: how long until its quest was done.
    let mut stuck = Vec::new();
    for m in &bot.log {
        let Mark::Stuck(why) = &m.mark else { continue };
        let Some(q) = SPINE.iter().find(|q| why.starts_with(&format!("{q} step"))) else { continue };
        let id = cat.story.quest_id(q).expect("a spine quest");
        let after = bot.log.iter().find(|n| n.tick >= m.tick && n.mark == Mark::QuestDone(id)).map(|n| n.tick);
        stuck.push((why.clone(), after.map(|t| (t - m.tick) / 3600)));
    }
    let rows = jane_data::catalog().living.consequences;
    let unnoticed = NOTICES
        .into_iter()
        .filter(|n| {
            let i = rows.iter().position(|r| r.id == *n).unwrap_or_else(|| panic!("no consequence {n}"));
            !sim.state().consequences_done.get(i as u32)
        })
        .collect();
    let verified = verify_tape(&tape, b).expect("the story's tape replays to every hash");
    assert_eq!(verified.final_hash, sim.hash(), "seed {seed}: the tape ends where the run did");
    Run {
        seed,
        ending,
        the_end: v.the_end(),
        frames,
        ticks: sim.state().tick.0,
        deaths: bot.log.iter().filter(|m| m.mark == Mark::Died).count(),
        done,
        stuck,
        log: bot.log.iter().map(jane_bot::Milestone::line).collect(),
        death_lines: bot.deaths.iter().map(jane_bot::Death::line).collect(),
        unnoticed,
    }
}

fn row(r: &Run) -> String {
    let spine = SPINE.iter().filter(|q| r.done.iter().any(|d| d == *q)).count();
    format!(
        "seed {} {:<5} the_end {} | {} real ({} ticks) | spine {spine}/{} | quests {} | deaths {} | stuck {:?}",
        r.seed,
        r.ending.name(),
        r.the_end,
        clock(Some(r.frames)),
        r.ticks,
        SPINE.len(),
        r.done.len(),
        r.deaths,
        r.stuck,
    )
}

/// Seeds 1 to 5, the endings in turn: every run reaches the ending it chose, through every act
/// of the spine, and nothing the spine needs stays set aside past the budget.
#[test]
#[ignore = "a whole story per seed: cargo test --release -p jane-bot --test story -- --ignored"]
fn the_reader_reaches_an_ending_on_seeds_1_to_5() {
    // The five in parallel: each is its own sim, and a whole story is minutes of release time.
    let runs: Vec<Run> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=5).map(|s| sc.spawn(move || play(s, ending_for(s)))).collect();
        hs.into_iter().map(|h| h.join().expect("a story run")).collect()
    });
    let mut problems = Vec::new();
    for r in &runs {
        println!("{}", row(r));
        for d in &r.death_lines {
            println!("    death {d}");
        }
        if r.the_end != r.ending.the_end() {
            problems.push(format!("seed {}: ended {} not {}", r.seed, r.the_end, r.ending.the_end()));
            for l in r.log.iter().filter(|l| !l.contains("killed")) {
                println!("    {l}");
            }
        }
        for q in SPINE {
            if !r.done.iter().any(|d| d == q) {
                problems.push(format!("seed {}: {q} not done", r.seed));
            }
        }
        if !r.unnoticed.is_empty() {
            problems.push(format!("seed {}: the town never noticed {:?}", r.seed, r.unnoticed));
        }
        for (why, took) in &r.stuck {
            if took.is_none_or(|m| m > STUCK_BUDGET) {
                problems.push(format!("seed {}: stuck {took:?} min: {why}", r.seed));
            }
        }
    }
    // Each of the three is reached on at least one seed.
    for e in [Ending::Hold, Ending::Hill, Ending::Train] {
        if !runs.iter().any(|r| r.the_end == e.the_end()) {
            problems.push(format!("no seed reached the {} ending", e.name()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// From Yours to Say (the console putting a new game there, the Ball in her bag): each of the
/// three is played where STORY.md §10 puts it, closes the game with its own `the_end`, and takes
/// the Ball; the train waits for a Sunday it stops on.
#[test]
fn the_three_endings_are_played_from_the_choice() {
    for ending in [Ending::Hold, Ending::Hill, Ending::Train] {
        let mut sim = Sim::new_game_with(bps(7), "Jane");
        let setup = jane_bot::console::start_at(&mut sim, "choice").expect("the act");
        let mut bot = Bot::story(Model::Reader);
        bot.setup = setup;
        bot.ctx.ending = Some(ending);
        let frames = bot.play(&mut sim, 2 * 60 * 60 * 60);
        let v = sim.view(Seat(0)).expect("seat 0");
        println!("{:<5} the_end {} in {} real, day {}", ending.name(), v.the_end(), clock(Some(frames)), v.clock().1);
        assert_eq!(v.the_end(), ending.the_end(), "{}: {:?}", ending.name(), bot.stuck());
        let ball = jane_data::catalog().combat.item_id("the_ball").expect("the Ball");
        assert_eq!(jane_bot::sense::holds(&v, ball), 0, "the Ball is where she set it down");
        let choice = jane_data::catalog().story.quest_id("the_choice").expect("Yours to Say");
        assert!(v.quests_done().contains(&choice));
        assert!(bot.done(), "the bot stops at the end");
    }
}
