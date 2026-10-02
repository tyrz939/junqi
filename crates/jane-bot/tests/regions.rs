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
//! The slow tier plays the Reader's whole story on seeds 1 to 8 and counts the new quests she was
//! given and finished on her way (`cargo test --release -p jane-bot --test regions -- --ignored`).

use jane_bot::audit::{self, Verdict};
use jane_bot::{Bot, Mark, Model};
use jane_sim::{Blueprints, Sim};

/// Every quest of the Waters and Works side content, and the cross-region round.
const NEW: [&str; 33] = [
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
                        .filter(|a| NEW.contains(&a.quest) && !held(a.quest, &a.verdict))
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

/// As the story suite: forty game hours of frames.
const STORY_FRAMES: u32 = 40 * 60 * 60 * 60;

#[test]
#[ignore = "slow: a whole story on eight seeds, a minute or more each in release"]
fn the_reader_takes_the_new_quests_on_her_way() {
    let cat = jane_data::catalog();
    let ids: Vec<_> = NEW.iter().map(|q| cat.story.quest_id(q).expect("a quest")).collect();
    let runs: Vec<(u32, usize, usize, bool)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=8u32)
            .map(|seed| {
                let ids = ids.clone();
                sc.spawn(move || {
                    let mut sim = Sim::new_game_with(Blueprints::build(seed).expect("the seed builds"), "Jane");
                    let mut bot = Bot::story(Model::Reader);
                    bot.play(&mut sim, STORY_FRAMES);
                    let given =
                        bot.log.iter().filter(|m| matches!(m.mark, Mark::QuestGiven(q) if ids.contains(&q))).count();
                    let done =
                        bot.log.iter().filter(|m| matches!(m.mark, Mark::QuestDone(q) if ids.contains(&q))).count();
                    let v = sim.view(jane_sim::Seat(0)).expect("seat 0");
                    (seed, given, done, v.the_end() != 0)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    let mut all_done = 0;
    for &(seed, given, done, end) in &runs {
        println!("seed {seed}: {given} of the new quests given, {done} done; the end {end}");
        all_done += done;
        assert!(end, "seed {seed}: the Reader did not reach an ending with the new quests in the county");
    }
    assert!(all_done >= 8, "the Reader finished only {all_done} of the new quests over eight seeds");
}
