//! The slow tier's one story sweep (VERIFICATION.md §6): the Reader from New Game to her ending on
//! seeds 1 to 8, each played **once** per test process, recorded, its tape verified, and what every
//! whole-story test asks of it kept: the bot's log and deaths, what the town noticed, the worst
//! crowd of "!" she saw, and her growth hour by hour. The tests that read it (`story.rs`, and the
//! growth curve) assert their own properties of the same eight runs, where each used to play
//! all eight again itself.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use jane_bot::run::Session;
use jane_bot::{Bot, Ending, Milestone, Model};
use jane_core::{SpellId, ZoneId};
use jane_sim::replay::{Recorder, verify_tape};
use jane_sim::trace::{Ev, Kind};
use jane_sim::{Blueprints, Seat, Sim};

use crate::common::par_map;

/// Frames an hour of play.
pub const HOUR: u32 = 60 * 60 * 60;

/// Frames a story may take before the test calls it stuck: forty game hours of play (the
/// Reader's whole stories run 800 to 1,200 minutes of the log's clock on the seeds that end,
/// deaths and all), and the nights slept away do not count against it (they are not frames).
pub const STORY_FRAMES: u32 = 40 * HOUR;

/// The seeds the sweep plays.
pub const SWEEP: std::ops::RangeInclusive<u32> = 1..=8;

/// "A place" for the quest marks: forty cells round any "!".
pub const CROWD_CELLS: i32 = 40;
/// Ticks between looks at the quest marks: ten game minutes.
const CROWD_EVERY: u32 = 10 * 3600;

/// What the town notices of each act (STORY.md §11): the consequence rows each act's boss
/// fires, which must all have fired by the end of a story.
pub const NOTICES: [&str; 6] = ["mine_quiet", "wing_lit", "forest_quiet", "works_dark", "burial_quiet", "bell_stopped"];

/// The ending a seed's run chooses: the three in turn, so seeds 1 to 3 reach all three.
pub fn ending_for(seed: u32) -> Ending {
    [Ending::Hold, Ending::Hill, Ending::Train][(seed as usize + 2) % 3]
}

/// Her stats at a moment.
#[derive(Clone, Debug, Default)]
pub struct Her {
    pub strength: u16,
    pub spirit: u16,
    pub learned: Vec<SpellId>,
}

/// An hour of play: her at its end, the place she spent most of it in (a dungeon's index, or 100
/// + the county's region), and what hurt her in it.
#[derive(Clone)]
pub struct Hour {
    pub her: Her,
    pub zone: u16,
    pub deaths: u32,
    pub hurt: i64,
    pub blows: u32,
}

/// One whole story, as every test of it needs it.
pub struct Played {
    pub seed: u32,
    pub model: Model,
    pub ending: Ending,
    pub the_end: u8,
    pub frames: u32,
    pub ticks: u32,
    /// The bot's log: quests given and done, zones, deaths, steps set aside, notes.
    pub log: Vec<Milestone>,
    /// Every death, as the bot recorded it.
    pub death_lines: Vec<String>,
    /// Quests done at the end, by id.
    pub done: Vec<String>,
    /// §11's rows that had not fired by the end.
    pub unnoticed: Vec<&'static str>,
    /// The worst crowd of "!" she was shown: how many, at which tick, and where.
    pub crowd: (usize, u32, Vec<(i32, i32)>),
    /// Her at New Game, every hour of play, and on first coming into each place.
    pub start: Her,
    pub hours: Vec<Hour>,
    pub arrivals: Vec<(u16, u32, Her)>,
    /// Why its tape did not replay to every hash, if it did not.
    pub tape_fault: Option<String>,
}

fn her_of(sim: &Sim) -> Her {
    let v = sim.view(Seat(0)).expect("seat 0");
    let b = v.body();
    Her { strength: b.strength, spirit: b.spirit, learned: v.learned().to_vec() }
}

fn place_of(sim: &Sim) -> u16 {
    let v = sim.view(Seat(0)).expect("seat 0");
    match v.zone() {
        ZoneId::County => 100 + v.region() as u16,
        z => z.index() as u16,
    }
}

/// `model` from New Game to the end on `seed`, choosing `ending`: recorded and traced, its tape
/// verified against its hashes, and everything [`Played`] holds taken on the way.
pub fn play(model: Model, seed: u32, ending: Ending) -> Played {
    play_with(model, seed, ending, false)
}

/// The spine, in the order the dog gives it.
pub const SPINE: [&str; 12] = [
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

/// [`play`]; `fires_bare`: with made fires on (`jane_sim::fire`, PLAY-PLAN.md §2.2) and every
/// match she holds thrown out each time a spine quest is done (L3: the kept fires carry her).
pub fn play_with(model: Model, seed: u32, ending: Ending, fires_bare: bool) -> Played {
    let b = Blueprints::build(seed).expect("the seed builds");
    let mut rec = Recorder::new(Sim::new_game_with(b.clone(), "Jane"));
    let mut bot = Bot::story(model);
    bot.ctx.ending = Some(ending);
    let cat = jane_data::catalog();
    // Made fires on, and every match she holds thrown out each time a spine quest is done.
    let spine: Vec<_> = SPINE.iter().filter_map(|q| cat.story.quest_id(q)).collect();
    let a_match = cat.combat.item_id("match").expect("matches");
    let mut chapters = usize::MAX;
    if fires_bare {
        bot.setup.push(jane_sim::Command::Dev(jane_sim::DevOp::Fires(true)));
    }
    let start = her_of(rec.sim());
    let mut sess = Session::new(bot, rec.sim(), STORY_FRAMES / 3600);
    let (mut hours, mut arrivals, mut here) = (Vec::new(), Vec::new(), BTreeMap::<u16, u32>::new());
    let mut crowd = (0, 0, Vec::new());
    let (mut next_look, mut hour) = (0, u8::MAX);
    let mut frame = 0;
    while frame < STORY_FRAMES && !sess.bot.done() {
        sess.step(&mut rec);
        frame += 1;
        let sim = rec.sim();
        // The quest marks: every ten minutes, and the moment an hour turns (a night slept is no
        // frames).
        let tick = sim.state().tick.0;
        let v = sim.view(Seat(0)).expect("seat 0");
        if tick >= next_look || v.hour() != hour {
            (next_look, hour) = (tick + CROWD_EVERY, v.hour());
            let c = v.offer_crowd(CROWD_CELLS);
            if c.len() > crowd.0 {
                crowd = (c.len(), tick, c);
            }
        }
        if fires_bare && frame % 60 == 0 && sess.bot.setup.is_empty() {
            let done = spine.iter().filter(|q| v.quests_done().contains(q)).count();
            if done != chapters {
                chapters = done;
                for (slot, s) in v.me().bag.iter().enumerate() {
                    if s.is_some_and(|s| s.item == a_match) {
                        sess.bot.setup.push(jane_sim::Command::BagDestroy { slot: slot as u8 });
                    }
                }
            }
        }
        // Growth: where she is each second, her stats each hour.
        if frame % 60 == 0 {
            let p = place_of(sim);
            *here.entry(p).or_insert(0) += 1;
            if !arrivals.iter().any(|a: &(u16, u32, Her)| a.0 == p) {
                arrivals.push((p, frame / HOUR, her_of(sim)));
            }
        }
        if frame % HOUR == 0 {
            let zone = here.iter().max_by_key(|e| *e.1).map_or(100, |e| *e.0);
            here.clear();
            hours.push(Hour { her: her_of(sim), zone, deaths: 0, hurt: 0, blows: 0 });
        }
    }
    let zone = here.iter().max_by_key(|e| *e.1).map_or(100, |e| *e.0);
    hours.push(Hour { her: her_of(rec.sim()), zone, deaths: 0, hurt: 0, blows: 0 });
    let (bot, trace) = sess.finish(rec.sim());
    for r in trace.records.iter().filter(|r| r.seat == Some(0)) {
        let last = hours.len() - 1;
        let h = &mut hours[((r.frame / HOUR) as usize).min(last)];
        match r.kind {
            Kind::Event(Ev::Died { .. }) => h.deaths += 1,
            Kind::Event(Ev::Hurt { amount, .. }) => {
                h.hurt += i64::from(amount);
                h.blows += 1;
            }
            _ => {}
        }
    }
    drop(trace);
    let (sim, tape) = rec.finish();
    let v = sim.view(Seat(0)).expect("seat 0");
    let cat = jane_data::catalog();
    let done = v.quests_done().iter().map(|&q| cat.story.quest(q).id.to_owned()).collect();
    let rows = cat.living.consequences;
    let unnoticed = NOTICES
        .into_iter()
        .filter(|n| {
            let i = rows.iter().position(|r| r.id == *n).unwrap_or_else(|| panic!("no consequence {n}"));
            !sim.state().consequences_done.get(i as u32)
        })
        .collect();
    // Kept as data, not a panic: a panic here would leave the sweep unplayed for the next test
    // to play all over again.
    let tape_fault = match verify_tape(&tape, b) {
        Ok(v) if v.final_hash == sim.hash() => None,
        Ok(_) => Some("the tape does not end where the run did".to_owned()),
        Err(e) => Some(format!("the tape does not replay: {e}")),
    };
    Played {
        seed,
        model,
        ending,
        the_end: v.the_end(),
        frames: frame,
        ticks: sim.state().tick.0,
        death_lines: bot.deaths.iter().map(jane_bot::Death::line).collect(),
        log: bot.log,
        done,
        unnoticed,
        crowd,
        start,
        hours,
        arrivals,
        tape_fault,
    }
}

/// The Reader's sweep, seeds 1 to 8, played once per process (the first test to ask plays it,
/// the rest wait for it).
pub fn readers() -> &'static [Played] {
    static RUNS: OnceLock<Vec<Played>> = OnceLock::new();
    RUNS.get_or_init(|| par_map(SWEEP.collect(), |s| play(Model::Reader, s, ending_for(s))))
}
