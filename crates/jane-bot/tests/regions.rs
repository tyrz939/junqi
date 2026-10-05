//! The Waters and the Works side content, held to QUEST-TREE.md's audit on seeds 1 to 8: every
//! step and every "Back to ..." of the new quests has its thing on the seed, the words it says
//! name something built near it, the place's own name is written up where she arrives, and it can
//! be seen from a road or a track (the audit's `ok`, or a dungeon the words name). Stricter than
//! `audit.rs`, which holds the whole tree only to "built".
//!
//! One documented exception: No. 14, Again counts its lamps from Pell's stone, as No. 14 does;
//! the stone is not lettered at every lamp (the lighting notice by the stone says where each
//! stands: `clarity.rs`).
//!
//! The slow tier's story sweep (`story.rs`) counts the new quests the Reader was given and
//! finished on her way through the whole story; this file's slow test is the side-quest run, put
//! at the Burial's act with every new quest in her log.

mod common;

use common::{NEW_QUESTS, par_map};
use jane_bot::audit::{self, Verdict};
use jane_bot::{Bot, Model};
use jane_sim::{Blueprints, Sim};

fn held(quest: &str, v: &Verdict) -> bool {
    match v {
        Verdict::Ok | Verdict::Elsewhere { named: true, .. } => true,
        Verdict::Unposted(w) => quest == "fourteen_again" && w.contains("pell"),
        _ => false,
    }
}

#[test]
fn the_new_quests_pass_the_audit_on_seeds_1_to_8() {
    let runs: Vec<(u32, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=8u32)
            .map(|seed| {
                sc.spawn(move || {
                    let bps = Blueprints::build(seed).expect("the seed builds");
                    let bad = audit::steps(&bps)
                        .into_iter()
                        .filter(|a| NEW_QUESTS.contains(&a.quest) && !held(a.quest, &a.verdict))
                        .map(|a| {
                            let step =
                                if a.step == 255 { "back to".to_owned() } else { format!("step {}", a.step + 1) };
                            format!("seed {seed}: {} {step}: \"{}\": {}", a.quest, a.text, a.verdict.word())
                        })
                        .collect();
                    (seed, bad)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a seed")).collect()
    });
    let bad: Vec<String> = runs.into_iter().flat_map(|(_, b)| b).collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Game hours the side-quest run plays.
const ERRAND_HOURS: u32 = 16;

/// The side-quest run: a Reader put at the Burial's act (the Works open, the verbs to that act
/// learned, her growth what the dungeons before it hold) with every new quest of the Waters and
/// the Works in her log and what their givers hand over in her bag, playing sixteen game hours.
/// What she finishes is what the words and the world let a player who reads finish.
#[test]
#[ignore = "slow: sixteen game hours on three seeds"]
fn a_reader_given_the_new_quests_finishes_them() {
    let cat = jane_data::catalog();
    let ids: Vec<_> = NEW_QUESTS.iter().map(|q| cat.story.quest_id(q).expect("a quest")).collect();
    let runs: Vec<(u32, Vec<&'static str>)> = par_map(vec![1, 2, 3], |seed| {
        let mut sim = Sim::new_game_with(Blueprints::build(seed).expect("the seed builds"), "Jane");
        let mut setup = jane_bot::console::start_at(&mut sim, "burial").expect("the act");
        for &q in &ids {
            setup.push(jane_sim::Command::Dev(jane_sim::DevOp::Quest(q)));
        }
        for (item, qty) in [("signal_dinner", 3), ("nurses_bag", 1)] {
            let item = cat.combat.item_id(item).expect("an item");
            setup.push(jane_sim::Command::Dev(jane_sim::DevOp::Give { item, qty }));
        }
        let mut bot = Bot::story(Model::Reader);
        bot.setup = setup;
        bot.play(&mut sim, ERRAND_HOURS * 60 * 60 * 60);
        let v = sim.view(jane_sim::Seat(0)).expect("seat 0");
        let done: Vec<&'static str> =
            ids.iter().filter(|q| v.quests_done().contains(q)).map(|&q| cat.story.quest(q).id).collect();
        (seed, done)
    });
    let mut all = 0;
    for (seed, done) in &runs {
        println!("seed {seed}: {} of {} done: {}", done.len(), NEW_QUESTS.len(), done.join(", "));
        all += done.len();
    }
    assert!(all >= MIN_DONE, "the Reader finished {all} of the new quests over three seeds");
}

/// The least the side-quest run finishes over its three seeds (measured 2 October 2026: see
/// QUEST-TREE.md §11: 20, 16 and 20 of 33 after the merge of 3 October).
const MIN_DONE: usize = 50;
