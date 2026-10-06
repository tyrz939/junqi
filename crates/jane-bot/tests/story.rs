//! The whole story, played (STORY.md §4, §10; VERIFICATION.md §2 L3 and §6): the Reader from New
//! Game through every act, dungeon, revelation and choice to one of the three endings, on seeds 1
//! to 8, each ending on at least one of them; every run a tape that replays to its hashes; and
//! nothing the story needs set aside for longer than [`STUCK_BUDGET`].
//!
//! **One sweep, many tests.** The eight runs are played once per test process (`story/sweep.rs`)
//! and every whole-story property is asserted from that one recording: the ending and the spine
//! (here), the Waters and Works quests taken on her way (`regions.rs`'s, here), no place crowded
//! with "!" (was `quest_marks.rs`), and the growth curve (`story/growth.rs`). Each used to play
//! all eight stories again in its own binary.
//!
//! The endings are chosen by the bot's choice policy (`Ctx::ending`, `jane play --ending`): the
//! ring on the study desk, the seam at the back of the mine's vault, or the Sunday train.
//!
//! Tiers (VERIFICATION.md §6): the sweep's tests are the slow core (`cargo test --release -p
//! jane-bot --test story -- --ignored --skip full_`); the other models' and the hostile runs are
//! the full tier (`full_` names, `-- --ignored`). The three endings played from the choice are
//! quick, and run always.

mod common;

#[path = "story/growth.rs"]
mod growth;
#[path = "story/sweep.rs"]
mod sweep;

use common::{NEW_QUESTS, clock, par_map};
use jane_bot::{Bot, Ending, Mark, Model};
use jane_sim::{Blueprints, Seat, Sim};
use sweep::{Played, SPINE, STORY_FRAMES, ending_for, play, play_with, readers};

/// Game minutes a step of the spine may stay set aside before it counts as stuck for good: a
/// day and a half (a step set aside for the night, or for a door's hours, comes back to).
const STUCK_BUDGET: u32 = 36 * 60;

fn bps(seed: u32) -> Blueprints {
    Blueprints::build(seed).expect("the seed builds")
}

/// Spine steps set aside, with how long each stayed so (game minutes; `None`: never undone).
fn stuck(r: &Played) -> Vec<(String, Option<u32>)> {
    let cat = jane_data::catalog();
    let mut out = Vec::new();
    for m in &r.log {
        let Mark::Stuck(why) = &m.mark else { continue };
        let Some(q) = SPINE.iter().find(|q| why.starts_with(&format!("{q} step"))) else { continue };
        let id = cat.story.quest_id(q).expect("a spine quest");
        let after = r.log.iter().find(|n| n.tick >= m.tick && n.mark == Mark::QuestDone(id)).map(|n| n.tick);
        out.push((why.clone(), after.map(|t| (t - m.tick) / 3600)));
    }
    out
}

fn row(r: &Played) -> String {
    let spine = SPINE.iter().filter(|q| r.done.iter().any(|d| d == *q)).count();
    format!(
        "seed {} {} {:<5} the_end {} | {} real ({} ticks) | spine {spine}/{} | quests {} | deaths {} | stuck {:?}",
        r.seed,
        r.model.name(),
        r.ending.name(),
        r.the_end,
        clock(Some(r.frames)),
        r.ticks,
        SPINE.len(),
        r.done.len(),
        r.death_lines.len(),
        stuck(r),
    )
}

/// What is wrong with each run: an ending not its own, a spine quest not done, a notice never
/// fired, a step set aside past the budget, a tape that does not replay.
fn problems(runs: &[Played]) -> Vec<String> {
    let mut problems = Vec::new();
    for r in runs {
        println!("{}", row(r));
        for d in &r.death_lines {
            println!("    death {d}");
        }
        if let Some(f) = &r.tape_fault {
            problems.push(format!("seed {}: {f}", r.seed));
        }
        if r.the_end != r.ending.the_end() {
            problems.push(format!("seed {}: ended {} not {}", r.seed, r.the_end, r.ending.the_end()));
            for l in r.log.iter().map(jane_bot::Milestone::line).filter(|l| !l.contains("killed")) {
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
        for (why, took) in stuck(r) {
            if took.is_none_or(|m| m > STUCK_BUDGET) {
                problems.push(format!("seed {}: stuck {took:?} min: {why}", r.seed));
            }
        }
    }
    problems
}

/// Seeds 1 to 8, the endings in turn: every run reaches the ending it chose, through every act
/// of the spine, and nothing the spine needs stays set aside past the budget. Seed 6 stalled in
/// the Factory until 2026-10-02 (pressed into a bench's corner by the generator, the Charge Hand
/// a cell off across it: `tactics::works::steer` now gives up a way she cannot go).
#[test]
#[ignore = "slow: the story sweep, eight whole stories, a minute or more each in release"]
fn the_reader_reaches_an_ending_on_seeds_1_to_8() {
    let runs = readers();
    let mut problems = problems(runs);
    // Each of the three is reached on at least one seed.
    for e in [Ending::Hold, Ending::Hill, Ending::Train] {
        if !runs.iter().any(|r| r.the_end == e.the_end()) {
            problems.push(format!("no seed reached the {} ending", e.name()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// `regions.rs`'s whole-story check, on the sweep: she walks the spine through both regions and
/// is offered what stands by her way (at least eight of the Waters and Works quests given over
/// the eight seeds).
#[test]
#[ignore = "slow: the story sweep"]
fn the_reader_takes_the_new_quests_on_her_way() {
    let cat = jane_data::catalog();
    let ids: Vec<_> = NEW_QUESTS.iter().map(|q| cat.story.quest_id(q).expect("a quest")).collect();
    let mut all_given = 0;
    for r in readers() {
        let given = r.log.iter().filter(|m| matches!(m.mark, Mark::QuestGiven(q) if ids.contains(&q))).count();
        let done = r.log.iter().filter(|m| matches!(m.mark, Mark::QuestDone(q) if ids.contains(&q))).count();
        println!("seed {}: {given} of the new quests given, {done} done; the end {}", r.seed, r.the_end);
        all_given += given;
    }
    assert!(all_given >= 8, "the Reader was given only {all_given} of the new quests over eight seeds");
}

/// The most "!" a place may show at once.
const CROWD_MAX: usize = 3;

/// The quest marks as a player meets them (PLAY-PLAN.md 0.4): all through a Reader's story, no
/// place shows her more than three "!" at once (forty cells round any one of them), whatever she
/// has taken and left. Looked at every ten game minutes and at each turn of the hour.
#[test]
#[ignore = "slow: the story sweep"]
fn no_place_shows_more_than_three_offers_through_a_reader_s_story() {
    let mut bad = Vec::new();
    for r in readers() {
        let (n, tick, at) = &r.crowd;
        println!("seed {}: at most {n} \"!\" in a place (tick {tick}) {at:?}", r.seed);
        if *n > CROWD_MAX {
            bad.push(format!("seed {}: {n} at tick {tick}: {at:?}", r.seed));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// PLAY-PLAN.md §2.2's L3 proof for made fires: with them on and every match she holds thrown out
/// at the start of each chapter (each spine quest done), the Reader still finishes on seeds 1 to
/// 8: the kept fires carry her, and what she lights on the way is extra. `FIRES_MADE` is flipped
/// on only while this holds.
#[test]
#[ignore = "slow, full tier: a whole story on eight seeds with made fires"]
fn full_the_reader_reaches_an_ending_with_made_fires_and_no_matches_kept() {
    // The gate for flipping the flag: it holds the slow tier only once fires are on by default.
    // Until then the bots are still being taught (PLAY-PLAN.md Phase 2); run it by hand with
    // `JANE_FIRES_GATE=1` to see how far they have come.
    if !jane_sim::tuning::FIRES_MADE && std::env::var_os("JANE_FIRES_GATE").is_none() {
        eprintln!("skipped: FIRES_MADE is off (set JANE_FIRES_GATE=1 to run it anyway)");
        return;
    }
    let runs = par_map(sweep::SWEEP.collect(), |s| play_with(Model::Reader, s, ending_for(s), true));
    let problems = problems(&runs);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The two other stalls the overnight audit found (PLAY-PLAN.md 0.2): the Cautious on seed 6 (the
/// Reader's Factory corner) and the Rusher on seed 8, which came out of the Museum by a park wall
/// and planned over whole blocks a way that crossed it (`coarse.rs` plans over the pieces of a
/// block now). Both play to an ending.
#[test]
#[ignore = "slow, full tier: two whole stories of other models"]
fn full_the_cautious_on_seed_6_and_the_rusher_on_seed_8_reach_an_ending() {
    let runs = par_map(vec![(Model::Cautious, 6), (Model::Rusher, 8)], |(m, s)| play(m, s, ending_for(s)));
    let problems = problems(&runs);
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

/// Plays the bot's steps, and every [`Hostile::every`] frames sets aside every side quest in
/// the log, as a player who abandons everything she is given would.
struct Hostile {
    sim: Sim,
    frames: u32,
    every: u32,
    seq: u16,
    /// Side quests set aside, all told.
    aside: u32,
}

impl jane_bot::Host for Hostile {
    fn view(&self, seat: Seat) -> Option<jane_sim::View<'_>> {
        self.sim.view(seat)
    }

    fn step(&mut self, input: &jane_sim::StepInput<'_>) -> jane_sim::Stepped {
        self.frames += 1;
        let cat = jane_data::catalog();
        let side: Vec<_> =
            self.sim.state().quests.active.iter().map(|p| p.quest).filter(|&q| !cat.story.quest(q).main).collect();
        if self.frames % self.every == 0 && !side.is_empty() {
            let mut cmds = input.commands.to_vec();
            for q in side {
                self.seq = self.seq.wrapping_add(1);
                self.aside += 1;
                cmds.push(jane_sim::StampedCommand {
                    seat: Some(Seat(0)),
                    seq: self.seq,
                    cmd: jane_sim::Command::Abandon(q),
                });
            }
            cmds.sort_by_key(|c| (c.seat, c.seq));
            return self.sim.step(&jane_sim::StepInput { frames: input.frames, commands: &cmds });
        }
        self.sim.step(input)
    }

    fn drain_events(&mut self) -> &[jane_sim::Event] {
        self.sim.drain_events()
    }

    fn sim(&self) -> &Sim {
        &self.sim
    }
}

/// Abandoning can never make the main story incompletable: the Reader plays seed 2 with every
/// side quest she takes set aside again every half a minute, and still reaches her ending
/// through every act of the spine.
#[test]
#[ignore = "slow, full tier: a whole story, a minute or more in release"]
fn full_the_story_ends_with_every_side_quest_set_aside_as_it_is_taken() {
    let seed = 2;
    let ending = ending_for(seed);
    let mut host = Hostile { sim: Sim::new_game_with(bps(seed), "Jane"), frames: 0, every: 1800, seq: 40000, aside: 0 };
    let mut bot = Bot::story(Model::Reader);
    bot.ctx.ending = Some(ending);
    let frames = bot.play(&mut host, STORY_FRAMES);
    let v = host.sim.view(Seat(0)).expect("seat 0");
    let cat = jane_data::catalog();
    println!("seed {seed}: the_end {} in {}, {} side quests set aside", v.the_end(), clock(Some(frames)), host.aside);
    for q in SPINE {
        let id = cat.story.quest_id(q).expect("a spine quest");
        assert!(v.quests_done().contains(&id), "{q} done: {:?}", bot.stuck());
    }
    assert_eq!(v.the_end(), ending.the_end(), "{:?}", bot.stuck());
    assert!(host.aside > 0, "she was given side quests to set aside");
}
