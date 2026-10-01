//! The places the Lost could not find from the words (the sweep of 1 October 2026): Farrant's
//! ring, Denny's light, Rendle's coat, Number Fourteen's lamps, the library and the gate of
//! Butterfly Forest, and the first doors of the mine, the Burial, the School and the Factory.
//! Each is found by reading the county: a fingerpost by a road names it, the forks on the way name
//! it from further off, and the words name something she can walk to.
//!
//! The fast tier holds the signs on the gate's seeds; the slow tier plays the Lost to an ending on
//! seeds 1 to 8 and holds her to finding each place from the words on seven of them:
//! `cargo test --release -p jane-bot --test clarity -- --ignored`.

mod common;

use common::*;
use jane_bot::lost::{landmark_words, names, way_words, words};
use jane_bot::{Bot, Mark, Model};
use jane_core::Rect;
use jane_core::action::Action;

/// The steps (quest, step from 0) the clarity pass was for.
const HARD: [(&str, usize); 12] = [
    ("farrant_ring", 0),
    ("denny_light", 0),
    ("rendle_coat", 0),
    ("number_fourteen", 0),
    ("number_fourteen", 1),
    ("number_fourteen", 2),
    ("the_forest", 1),
    ("the_mine", 0),
    ("the_burial", 0),
    ("the_school", 0),
    ("the_factory", 0),
    ("the_factory", 1),
];

/// Steps whose place a sign by a road names: all but the lamps, which are counted from Pell's
/// stone (the words name it, and the stone is where she was given them).
const POSTED: [(&str, usize); 8] = [
    ("farrant_ring", 0),
    ("denny_light", 0),
    ("rendle_coat", 0),
    ("the_forest", 1),
    ("the_mine", 0),
    ("the_burial", 0),
    ("the_school", 0),
    ("the_factory", 0),
];

/// Every sign in the county with words, and where it stands.
fn signs(v: &jane_sim::View<'_>) -> Vec<((i32, i32), String)> {
    let (w, h) = v.size();
    let mut out = Vec::new();
    for p in v.props_in(Rect::new(0, 0, w as i32, h as i32)) {
        let Some(l) = v.prop_spawn(p).and_then(|s| s.use_list) else { continue };
        let mut text = String::new();
        jane_bot::sense::visit(&|l| v.list(l), l, &mut |a| {
            if let Action::Read(t) = *a {
                text.push_str(v.text(t));
                text.push(' ');
            }
        });
        if !text.is_empty() {
            out.push(((i32::from(p.cell.x), i32::from(p.cell.y)), text));
        }
    }
    out
}

/// Each hard place a sign names, on two posts at least, one of them far enough off to be on the
/// way there (not only at it); and the lamps' words name the stone they are counted from.
#[test]
fn the_hard_places_are_named_by_the_roads() {
    let cat = jane_data::catalog();
    let mut bad = Vec::new();
    for seed in SEEDS {
        let sim = new_game(seed);
        let v = sim.view(jane_sim::Seat(0)).expect("a seat");
        let all = signs(&v);
        for (quest, i) in POSTED {
            let q = cat.story.quest_id(quest).expect("a quest");
            let want = landmark_words(&v, q, Some(i));
            let named: Vec<((i32, i32), i32)> = all
                .iter()
                .flat_map(|(at, text)| way_words(text).into_iter().map(move |w| (*at, w)))
                .filter(|(_, w)| names(&words(&w.name), &want))
                .map(|(at, w)| (at, w.metres))
                .collect();
            let far = named.iter().filter(|&&(_, m)| m >= 150).count();
            println!("seed {seed} {quest} step {}: {} signs name it, {far} from 150 m or more", i + 1, named.len());
            if named.len() < 2 || far == 0 {
                bad.push(format!("seed {seed}: {quest} step {}: {} signs, {far} far ({want:?})", i + 1, named.len()));
            }
        }
        let q = cat.story.quest_id("number_fourteen").expect("a quest");
        for i in 0..3 {
            let want = landmark_words(&v, q, Some(i));
            if !(want.iter().any(|w| w == "pell") && want.iter().any(|w| w == "stone")) {
                bad.push(format!("seed {seed}: number_fourteen step {}: the words do not name Pell's stone", i + 1));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The ending a seed's story run chooses (the sweep's rule: the three in turn).
fn ending_for(seed: u32) -> jane_bot::Ending {
    [jane_bot::Ending::Hold, jane_bot::Ending::Hill, jane_bot::Ending::Train][(seed as usize + 2) % 3]
}

/// What the log says she gave up looking for ("farrant_ring step 1").
fn given_up(bot: &Bot) -> Vec<String> {
    bot.log
        .iter()
        .filter_map(|l| match &l.mark {
            Mark::Note(n) => {
                n.strip_prefix("lost: gave up looking for ").map(|r| r.trim_end_matches(" (told where)").to_owned())
            }
            _ => None,
        })
        .collect()
}

/// The steps the Lost gave up looking for on `seed`, playing from New Game until every hard step
/// is done or given up, the story ends, or 2400 minutes (the sweep's cap).
fn gave_up(seed: u32) -> Vec<String> {
    let cat = jane_data::catalog();
    let bps = jane_sim::Blueprints::build(seed).expect("the seed builds");
    let mut sim = jane_sim::Sim::new_game_with(bps, "Jane");
    let mut bot = Bot::story(Model::Lost);
    bot.ctx.ending = Some(ending_for(seed));
    for _ in 0..2400 {
        bot.play(&mut sim, MINUTE);
        if bot.done() {
            break;
        }
        let gave = given_up(&bot);
        let v = sim.view(jane_sim::Seat(0)).expect("a seat");
        let resolved = HARD.iter().all(|&(quest, i)| {
            let q = cat.story.quest_id(quest).expect("a quest");
            v.quests_done().contains(&q)
                || v.quests().any(|qv| qv.quest == q && qv.count(i) >= cat.story.quest(q).requirements[i].qty)
                || gave.contains(&format!("{quest} step {}", i + 1))
        });
        if resolved {
            break;
        }
    }
    given_up(&bot)
}

#[test]
#[ignore = "slow: the Lost on eight seeds until the hard steps are done, up to an hour of a release build"]
fn the_lost_finds_the_hard_places_on_seven_seeds_of_eight() {
    let seeds: Vec<u32> = (1..=8).collect();
    let runs: Vec<(u32, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = seeds.iter().map(|&s| sc.spawn(move || (s, gave_up(s)))).collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    let mut bad = Vec::new();
    for (quest, i) in HARD {
        let name = format!("{quest} step {}", i + 1);
        let lost: Vec<u32> = runs.iter().filter(|(_, g)| g.contains(&name)).map(|&(s, _)| s).collect();
        println!("{name}: found on {} of {} (gave up on {lost:?})", seeds.len() - lost.len(), seeds.len());
        if lost.len() > 1 {
            bad.push(format!("{name}: gave up on seeds {lost:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
