//! The save's form over real states (ARCHITECTURE.md §3.5, §3.6): a zone's units and props are
//! saved as what differs from its blueprint's spawn, and wherever a bot session has taken the
//! world (props used, opened and looted; units killed, stood up again by the ecology, made by a
//! consequence or the console; a zone left and entered again; a guest's parked body) the save
//! decodes to the state field for field, and the state decoded encodes to the same bytes.

mod common;

use common::*;
use jane_bot::crawl::Crawl;
use jane_bot::{Bot, Model, Plan};
use jane_core::action::School;
use jane_core::{Milli, ZoneId};
use jane_sim::input::DevOp;
use jane_sim::replay::diff_states;
use jane_sim::save::{Form, SpawnCounts, ZoneForm, decode_state, hash_of};
use jane_sim::state::LootState;
use jane_sim::{ClientToken, Command, Hit, Seat, Sim, StampedCommand, StepInput};

/// Save `sim`, decode it over the same blueprints and hold the state to the original, field for
/// field; and the form of the state decoded hashes as the original (the encoding is canonical).
/// A solo world also loads (`from_save`) to the same hash. Returns the save's size.
fn round_trip(sim: &Sim, what: &str) -> usize {
    let bytes = sim.save();
    let back = decode_state(&bytes, sim.blueprints()).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(back == *sim.state(), "{what}: the state decoded differs: {:?}", diff_states(&back, sim.state()));
    assert_eq!(hash_of(&Form::of(&back, sim.blueprints())), sim.hash(), "{what}: the form is canonical");
    if sim.state().party_size() == 1 && !sim.state().open {
        let loaded = Sim::from_save_with(&bytes, sim.blueprints().clone()).expect("loads");
        assert_eq!(loaded.hash(), sim.hash(), "{what}: loading is invisible to the hash");
    }
    bytes.len()
}

/// What zone `z`'s form keeps of its spawns.
fn counts(sim: &Sim, z: ZoneId) -> SpawnCounts {
    let s = sim.state();
    ZoneForm::of(s.zone(z).expect("the zone is made"), sim.blueprint(z), &s.syms).counts()
}

fn tp(sim: &mut Sim, zone: ZoneId) {
    let mark = jane_sim::sym::of_name(jane_data::catalog().name_id("start").unwrap());
    command(sim, Some(Seat(0)), Command::Dev(DevOp::Tp { zone, mark }));
    sim.step(&StepInput::IDLE);
    assert_eq!(sim.state().players[0].zone, zone);
}

fn command(sim: &mut Sim, seat: Option<Seat>, cmd: Command) {
    let cmds = [StampedCommand { seat, seq: 0, cmd }];
    sim.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
}

fn idle(sim: &mut Sim, n: u32) {
    for _ in 0..n {
        sim.step(&StepInput::IDLE);
    }
}

/// A fresh New Game stores none of the county's thousands of spawns but the few the ring woke
/// round her (`awake` is saved), and her body (made at runtime); the name tail is one run.
#[test]
fn a_new_game_saves_next_to_nothing_and_round_trips() {
    for seed in SEEDS {
        let sim = new_game(seed);
        let n = round_trip(&sim, "new game");
        let c = counts(&sim, ZoneId::County);
        let z = sim.state().zone(ZoneId::County).unwrap();
        println!("seed {seed} new game: {n} bytes; county {} units, {} props: {c:?}", z.units.len(), z.props.len());
        let awake = z.units.iter().filter(|u| u.awake).count() as u32;
        assert_eq!(c, SpawnCounts { units_stored: awake, ..SpawnCounts::default() }, "her body and who is awake");
        assert!(awake <= 8, "{awake} awake");
        // Before spawns were saved as deltas this was about 350 KB (SAVE_VERSION 7).
        assert!(n < 2 * 1024, "seed {seed}: {n} bytes");
    }
}

/// Five minutes of the story on each seed, saved every minute: doors opened, chests looted,
/// things used, the county's creatures woken round her; every save round-trips.
#[test]
fn a_story_session_round_trips_at_every_minute() {
    for seed in SEEDS {
        let mut sim = new_game(seed);
        let mut bot = Bot::story(Model::Reader);
        let mut size = 0;
        for m in 1..=5 {
            bot.play(&mut sim, MINUTE);
            size = round_trip(&sim, &format!("seed {seed} minute {m}"));
        }
        let z = sim.state().zone(ZoneId::County).unwrap();
        let used = z.props.iter().filter(|p| p.used).count();
        let looted = z.props.iter().filter(|p| p.loot != LootState::AsSpawned).count();
        let made = z.props.iter().filter(|p| p.spawn.is_none()).count();
        let c = counts(&sim, ZoneId::County);
        println!(
            "seed {seed} reader 5 min: {size} bytes; props used {used}, looted {looted}, made {made}; county {c:?}"
        );
        assert!(used + looted > 0, "seed {seed}: she touched nothing");
        assert!(c.units_stored > 1 && c.props_stored > 0, "{c:?}");
        // What was touched is stored; what was not is not.
        assert!((c.props_stored as usize) < z.props.len() / 10, "{c:?}");
        // Saving every spawn whole, this was about 360 KB (SAVE_VERSION 7); now 7 to 11 KB.
        assert!(size < 24 * 1024, "seed {seed}: {size} bytes after five minutes");
    }
}

/// A dungeon crawled from its door: creatures killed (corpses kept, runtime spawns come and
/// gone), its props used; saved every minute and at the end.
#[test]
fn a_crawl_round_trips_with_its_dead() {
    let mut sim = new_game(1);
    let mut bot = Bot::new(Model::Rusher, Plan::Crawl(Crawl::new(ZoneId::Cellar)));
    bot.setup = jane_bot::crawl::setup(sim.blueprints(), ZoneId::Cellar);
    let mut size = 0;
    for m in 1..=8 {
        bot.play(&mut sim, MINUTE);
        size = round_trip(&sim, &format!("crawl minute {m}"));
        if bot.done() {
            break;
        }
    }
    let z = sim.state().zone(ZoneId::Cellar).expect("she went in");
    let dead = z.units.iter().filter(|u| !u.alive).count();
    let spawned = sim.blueprint(ZoneId::Cellar).units.len();
    let c = counts(&sim, ZoneId::Cellar);
    println!("crawl of the cellar: {size} bytes; {dead} dead of {} ({spawned} spawned); cellar {c:?}", z.units.len());
    assert!(dead > 0 || c.units_removed > 0, "nothing died in the crawl");
    // About 350 KB with every spawn whole; now about 1.5 KB.
    assert!(size < 8 * 1024, "{size} bytes after a dungeon");
}

/// Units killed by a blow, standing up again at the ecology's mark; a consequence that takes
/// spawns away (tombstones) and makes others; one made by the console. Each state round-trips.
#[test]
fn deaths_respawns_and_runtime_spawns_round_trip() {
    let cat = jane_data::catalog();
    let mut sim = new_game(1);
    let mut bot = Bot::story(Model::Reader);
    bot.play(&mut sim, MINUTE);
    let prefix = |sim: &Sim, p: &str| {
        let s = sim.state();
        let z = s.zone(ZoneId::County).unwrap();
        z.units
            .iter()
            .filter(|u| u.key.is_some_and(|k| s.syms.name(k).starts_with(p)))
            .map(|u| u.id)
            .collect::<Vec<_>>()
    };
    let alive = |sim: &Sim, id| sim.state().zone(ZoneId::County).unwrap().unit(id).is_some_and(|u| u.alive);

    // One rat killed: a corpse (stored whole), up again at the next ten-minute mark.
    let rats = prefix(&sim, "rat_allotment_");
    assert_eq!(rats.len(), 4);
    let hit = Hit {
        to: rats[0],
        amount: Milli(1_000_000_000),
        school: School::Physical,
        from: None,
        crit: false,
        status: None,
    };
    sim.queue_hit(ZoneId::County, hit);
    idle(&mut sim, 1);
    assert!(!alive(&sim, rats[0]));
    round_trip(&sim, "a rat dead");
    let mut n = 0;
    while !alive(&sim, rats[0]) {
        idle(&mut sim, 1);
        n += 1;
        assert!(n < jane_sim::tuning::TICKS_PER_HOUR, "the rat stands up within the hour");
    }
    round_trip(&sim, "the rat stood up again");

    // The console makes one at her feet.
    let rat = cat.combat.unit_id("rat").unwrap();
    let before = sim.state().zone(ZoneId::County).unwrap().units.len();
    command(&mut sim, Some(Seat(0)), Command::Dev(DevOp::Spawn(rat)));
    idle(&mut sim, 1);
    assert_eq!(sim.state().zone(ZoneId::County).unwrap().units.len(), before + 1);
    round_trip(&sim, "a rat from the console");

    // A consequence in her zone: the allotment's rats go (their rows are tombstones) and the
    // sheds' come (made at runtime).
    let q = cat.story.quest_id("rats_in_the_sheds").unwrap();
    sim.state_mut().quests.done.push(q);
    idle(&mut sim, 1);
    assert!(prefix(&sim, "rat_allotment_").is_empty());
    assert_eq!(prefix(&sim, "rat_shed_").len(), 4);
    let c = counts(&sim, ZoneId::County);
    assert!(c.units_removed >= 4, "{c:?}");
    round_trip(&sim, "a consequence took rats and made rats");
    bot.play(&mut sim, MINUTE);
    round_trip(&sim, "a minute on");
}

/// A dungeon made and lived in, left (its runtime dropped), saved; entered again, saved.
#[test]
fn a_zone_left_and_entered_again_round_trips() {
    let mut sim = new_game(2);
    let mut bot = Bot::story(Model::Rusher);
    bot.play(&mut sim, MINUTE);
    tp(&mut sim, ZoneId::Mine);
    idle(&mut sim, 600);
    round_trip(&sim, "live in the mine");
    tp(&mut sim, ZoneId::County);
    idle(&mut sim, 60);
    assert!(sim.runtime(ZoneId::Mine).is_none(), "the mine is unloaded");
    round_trip(&sim, "the mine unloaded");
    tp(&mut sim, ZoneId::Mine);
    idle(&mut sim, 60);
    assert!(sim.runtime(ZoneId::Mine).is_some());
    round_trip(&sim, "the mine again");
    bot.play(&mut sim, MINUTE);
    round_trip(&sim, "and a minute of the story");
}

/// A guest sits down beside the story, walks, and gets up: her body is parked on her seat. The
/// world with her in it, and with her parked, decodes as it was; a load parks a guest still sat.
#[test]
fn parked_seats_round_trip() {
    let mut sim = new_game(3);
    let mut bot = Bot::story(Model::Reader);
    bot.play(&mut sim, MINUTE / 2);
    command(&mut sim, Some(Seat(0)), Command::Open(true));
    command(&mut sim, None, Command::Join { who: ClientToken(9) });
    assert_eq!(sim.state().party_size(), 2);
    bot.play(&mut sim, MINUTE / 2);
    round_trip(&sim, "two at the table");
    let guest = sim.state().players[1].unit;
    let loaded = Sim::from_save_with(&sim.save(), sim.blueprints().clone()).unwrap();
    let parked = loaded.state().players[1].parked.as_deref().expect("a load parks the guest");
    let zone = sim.state().players[1].zone;
    assert_eq!(Some(parked.id), Some(guest));
    assert_eq!(parked.pos, sim.state().zone(zone).unwrap().unit(guest).unwrap().pos);
    round_trip(&loaded, "loaded with the guest parked");

    command(&mut sim, Some(Seat(1)), Command::Leave);
    assert!(sim.state().players[1].parked.is_some());
    bot.play(&mut sim, MINUTE / 2);
    round_trip(&sim, "the guest got up");
    command(&mut sim, None, Command::Join { who: ClientToken(9) });
    assert!(sim.state().players[1].connected);
    idle(&mut sim, 30);
    round_trip(&sim, "the guest sat down again");
}
