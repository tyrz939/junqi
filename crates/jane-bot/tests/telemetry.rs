//! Play telemetry (PLAY-PLAN.md 0.1, `jane telemetry`): it only looks (a run with it steps to
//! the same hash as one without), two runs write the same bytes, and the first minutes' tables
//! hold what the first minutes are (the yard's bones killed, her blows taken, a minute a line).

mod common;

use common::*;
use jane_bot::telemetry::{SUMMARY_HEADER, Telemetry};
use jane_bot::{Bot, Model};

/// Four minutes: the letter, the yard's bones and the house.
const FRAMES: u32 = 4 * MINUTE;

fn run(seed: u32, watch: bool) -> (Bot, Option<Telemetry>, u64) {
    let mut sim = new_game(seed);
    let mut bot = Bot::story(Model::Reader);
    let mut t = watch.then(|| Telemetry::new("reader", seed));
    for _ in 0..FRAMES {
        bot.step(&mut sim);
        if let Some(t) = t.as_mut() {
            t.observe(&sim, bot.events());
        }
    }
    let hash = sim.hash();
    (bot, t, hash)
}

#[test]
fn telemetry_only_looks_and_writes_the_same_bytes_twice() {
    let (_, _, plain) = run(1, false);
    let (bot, t, watched) = run(1, true);
    assert_eq!(plain, watched, "a run with telemetry steps as one without");
    let t = t.expect("watched");
    let (bot2, t2, _) = run(1, true);
    let t2 = t2.expect("watched");
    let dir = std::env::temp_dir().join(format!("jane-telemetry-{}", std::process::id()));
    let (a, b) = (dir.join("a"), dir.join("b"));
    let row = t.write(&a, &bot).expect("written");
    let row2 = t2.write(&b, &bot2).expect("written");
    assert_eq!(row, row2);
    let mut names: Vec<String> = std::fs::read_dir(&a)
        .expect("the tables")
        .map(|e| e.expect("an entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for kind in ["blows", "chapters", "deaths", "heals", "kills", "milestones", "minutes", "rests", "summary"] {
        let n = format!("{kind}_reader_1.csv");
        assert!(names.contains(&n), "{n} written: {names:?}");
        let (x, y) = (std::fs::read(a.join(&n)).expect("read"), std::fs::read(b.join(&n)).expect("read"));
        assert!(x == y, "{n}: two runs wrote different bytes");
    }
    let _ = std::fs::remove_dir_all(&dir);
    // The first minutes as they are.
    assert_eq!(t.minutes.len(), 4, "a line a minute");
    assert!(t.kills.iter().any(|k| k.contains(",yard_bones,")), "the yard's bones are a kill: {:?}", t.kills);
    assert!(t.blows.iter().all(|b| b.split(',').count() == 11), "every blow row whole: {:?}", t.blows.first());
    assert_eq!(row.split(',').count(), SUMMARY_HEADER.split(',').count(), "{row}");
    println!("{SUMMARY_HEADER}\n{row}");
}
