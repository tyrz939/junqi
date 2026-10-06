//! PORT.md §9.3 gates 4 and 5 over bot sessions (ARCHITECTURE.md §8 `replay_roundtrip`,
//! `save_load_continue`): a bot's run recorded and played back from its bytes lands on every
//! hash on the tape; saved in the middle, loaded, and played on by the same bot, it lands on the
//! same hashes as the run that was never saved; a crawl whose setup is the console's replays too.

mod common;

use common::*;
use jane_bot::{Bot, Model};
use jane_core::ZoneId;
use jane_sim::replay::{HASH_EVERY, verify_tape};
use jane_sim::{Seat, Sim};

#[test]
fn a_recorded_session_replays_from_its_bytes_to_every_hash() {
    for (seed, model) in [(1, Model::Reader), (2, Model::Rusher)] {
        let (rec, bot) = story(seed, model, 5 * MINUTE);
        let (sim, tape) = rec.finish();
        assert!(bot.log.iter().any(|m| m.mark == jane_bot::Mark::QuestDone(quest_id("the_letter"))));
        assert_eq!(tape.final_hash(), Some(sim.hash()));
        // A hash lands on every 600-tick mark a frame steps onto; a skip (a night's sleep, a
        // crawl's set-up) steps over marks, so the tape is held to half the frames' worth, and to
        // no long stretch of frames without one: hashing that stops fails either way.
        let gap = tape.hashes.windows(2).map(|w| w[1].frame - w[0].frame).max().unwrap_or(u32::MAX);
        println!("seed {seed}: {} hashes, the longest gap {gap} frames", tape.hashes.len());
        assert!(tape.hashes.len() >= tape.frames as usize / HASH_EVERY as usize / 2, "{} hashes", tape.hashes.len());
        assert!(gap <= 6 * HASH_EVERY, "{gap} frames without a hash");
        // A walk's stick turns a little every frame; stops, talk and fights hold it.
        assert!(tape.runs.len() < tape.frames as usize, "{} runs for {} frames", tape.runs.len(), tape.frames);
        let back = round_trip(&tape);
        let v = verify_tape(&back, bps(seed)).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert_eq!(v.final_hash, sim.hash());
        assert_eq!(v.frames, tape.frames);
        println!(
            "seed {seed} {}: {} frames, {} runs, {} bytes",
            model.name(),
            tape.frames,
            tape.runs.len(),
            tape.encode().len()
        );
    }
}

/// Hashes at every `HASH_EVERY` ticks and the frame they were taken on.
fn hashes_every(sim: &Sim, out: &mut Vec<(u32, u64)>) {
    let t = sim.state().tick.0;
    if t % HASH_EVERY == 0 && out.last().is_none_or(|&(tt, _)| tt != t) {
        out.push((t, sim.hash()));
    }
}

#[test]
fn saved_in_the_middle_loaded_and_played_on_it_is_the_same_run() {
    let (seed, model) = (1, Model::Rusher);
    let (half, rest) = (100 * 60 + 17, 3 * MINUTE);
    // Two bots, the same from New Game: one plays on unbroken, the other through a save.
    let mut a = new_game(seed);
    let mut b = new_game(seed);
    let mut bot_a = Bot::story(model);
    let mut bot_b = Bot::story(model);
    for _ in 0..half {
        bot_a.step(&mut a);
        bot_b.step(&mut b);
    }
    assert_eq!(a.hash(), b.hash());
    let bytes = b.save();
    let mut b = Sim::from_save_with(&bytes, bps(seed)).expect("the save loads");
    assert_eq!(b.hash(), a.hash(), "loading is invisible to the hash");
    let (mut ha, mut hb) = (Vec::new(), Vec::new());
    for _ in 0..rest {
        bot_a.step(&mut a);
        bot_b.step(&mut b);
        hashes_every(&a, &mut ha);
        hashes_every(&b, &mut hb);
    }
    assert!(ha.len() >= 3, "{}", ha.len());
    assert_eq!(ha, hb);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(bot_a.log, bot_b.log);
    // And she got somewhere in it.
    assert!(a.view(Seat(0)).unwrap().quests_done().len() >= 2);
}

#[test]
fn a_crawl_set_up_by_the_console_replays_to_every_hash() {
    let (rec, bot) = crawl(1, Model::Rusher, ZoneId::Cellar, 4 * MINUTE);
    assert!(bot.log.iter().any(|m| m.mark == jane_bot::Mark::Zone(ZoneId::Cellar)));
    let (sim, tape) = rec.finish();
    let v = verify_tape(&round_trip(&tape), bps(1)).expect("the crawl replays");
    assert_eq!(v.final_hash, sim.hash());
    // The console's setup is in the tape: the first frames carry its commands.
    assert!(tape.runs.iter().take(12).any(|r| r.commands.iter().any(|c| matches!(c.cmd, jane_sim::Command::Dev(_)))));
}
