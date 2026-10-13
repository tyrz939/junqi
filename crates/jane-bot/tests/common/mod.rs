//! What the bot's gate tests share: blueprints built once per seed per test binary, and runs.

#![allow(dead_code)]

use std::sync::OnceLock;

use jane_bot::crawl::Crawl;
use jane_bot::{Bot, Mark, Model, Plan};
use jane_core::{QuestId, ZoneId};
use jane_sim::replay::{Recorder, Tape};
use jane_sim::{Blueprints, Sim};

/// The seeds the gate plays (PORT.md §7 P4: "on 3 seeds").
pub const SEEDS: [u32; 3] = [1, 2, 3];
pub const MODELS: [Model; 2] = [Model::Reader, Model::Rusher];

/// A game minute's frames (60 frames a second; the clock does not stop but for talk).
pub const MINUTE: u32 = 60 * 60;

/// The blueprints of a seed of the gate's, built once per process (a county is a third of a
/// second).
pub fn bps(seed: u32) -> Blueprints {
    static B: [OnceLock<Blueprints>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let i = SEEDS.iter().position(|&s| s == seed).expect("one of the gate's seeds");
    B[i].get_or_init(|| Blueprints::build(seed).expect("the seed builds")).clone()
}

pub fn new_game(seed: u32) -> Sim {
    Sim::new_game_with(bps(seed), "Jane")
}

/// The story from New Game, recorded, for `frames` (or until the bot stops).
pub fn story(seed: u32, model: Model, frames: u32) -> (Recorder, Bot) {
    let mut rec = Recorder::new(new_game(seed));
    let mut bot = Bot::story(model);
    bot.play(&mut rec, frames);
    (rec, bot)
}

/// A crawl of dungeon `z` from its door with the kit a player would carry there, recorded.
pub fn crawl(seed: u32, model: Model, z: ZoneId, frames: u32) -> (Recorder, Bot) {
    let sim = new_game(seed);
    let setup = jane_bot::crawl::setup(sim.blueprints(), z);
    let mut rec = Recorder::new(sim);
    let mut bot = Bot::new(model, Plan::Crawl(Crawl::new(z)));
    bot.setup = setup;
    bot.play(&mut rec, frames);
    (rec, bot)
}

/// When a quest was handed in, in ticks.
pub fn done_at(bot: &Bot, quest: &str) -> Option<u32> {
    let q = quest_id(quest);
    bot.log.iter().find(|m| m.mark == Mark::QuestDone(q)).map(|m| m.tick)
}

pub fn given_at(bot: &Bot, quest: &str) -> Option<u32> {
    let q = quest_id(quest);
    bot.log.iter().find(|m| m.mark == Mark::QuestGiven(q)).map(|m| m.tick)
}

pub fn entered_at(bot: &Bot, z: ZoneId) -> Option<u32> {
    bot.log.iter().find(|m| m.mark == Mark::Zone(z)).map(|m| m.tick)
}

pub fn learned_at(bot: &Bot, spell: &str) -> Option<u32> {
    let s = jane_data::catalog().combat.spell_id(spell).expect("a spell");
    bot.log.iter().find(|m| m.mark == Mark::Learned(s)).map(|m| m.tick)
}

pub fn deaths(bot: &Bot) -> usize {
    bot.log.iter().filter(|m| m.mark == Mark::Died).count()
}

pub fn quest_id(q: &str) -> QuestId {
    jane_data::catalog().story.quest_id(q).unwrap_or_else(|| panic!("no quest {q}"))
}

/// `mm:ss` of a tick, or `-`.
pub fn clock(t: Option<u32>) -> String {
    t.map_or_else(|| "-".to_owned(), |t| format!("{:02}:{:02}", t / 3600, t / 60 % 60))
}

/// Games played at once in this test process, by every test in it together: the cores, or
/// `JANE_TEST_JOBS`. A test binary's tests run side by side, and each used to spawn a thread a
/// seed, so three tests of eight seeds were twenty-four games in memory at once.
#[allow(clippy::disallowed_methods)]
fn jobs() -> usize {
    std::env::var("JANE_TEST_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, usize::from))
        .max(1)
}

/// The process's one pool of game slots (see [`jobs`]).
static SLOTS: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// A slot held for one job.
struct Slot;

impl Slot {
    fn take() -> Slot {
        let (busy, freed) = &SLOTS;
        let mut n = busy.lock().expect("the pool");
        while *n >= jobs() {
            n = freed.wait(n).expect("the pool");
        }
        *n += 1;
        Slot
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let (busy, freed) = &SLOTS;
        *busy.lock().expect("the pool") -= 1;
        freed.notify_one();
    }
}

/// `f` over `items` in parallel, in order, no more at once than the process's pool allows.
pub fn par_map<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let f = &f;
    std::thread::scope(|sc| {
        let hs: Vec<_> = items
            .into_iter()
            .map(|x| {
                sc.spawn(move || {
                    let _slot = Slot::take();
                    f(x)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a job")).collect()
    })
}

/// The tape's bytes, decoded again: what `jane replay verify` reads.
pub fn round_trip(tape: &Tape) -> Tape {
    Tape::decode(&tape.encode()).expect("a tape decodes")
}

/// Every quest of the Waters and Works side content, and the cross-region round (`regions.rs`, and the story sweep).
pub const NEW_QUESTS: [&str; 33] = [
    "the_toll",
    "three_crossings",
    "the_keepers_tally",
    "cutting_order",
    "what_the_eels_eat",
    "one_hook_empty",
    "the_planting_book",
    "dead_heading",
    "mother",
    "overdue",
    "road_closed",
    "the_diversion",
    "nobody_has_drowned",
    "the_statue_faces",
    "loveday_landing",
    "tuesday_reed_end",
    "tuesday_round",
    "shift_rota",
    "dinners_out",
    "relieve_the_night_shift",
    "signals_at_danger",
    "the_foremans_diary",
    "last_wagon",
    "register_of_burials",
    "flowers_for_the_rise",
    "goldskins_four",
    "what_the_pipes_say",
    "the_mere",
    "the_bell_rope",
    "fourteen_again",
    "the_adit",
    "breen_coal",
    "tuesday_hut_two",
];
