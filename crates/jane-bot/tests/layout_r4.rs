//! `--layout r4` (MAP R4 WP5, lean): the chosen terraced county in the live game, behind a dev switch.
//! The sim builds on it and the Reader bot plays two minutes: no panic, not stuck, the clock ran.

use jane_bot::{Bot, Model};
use jane_core::ZoneId;
use jane_sim::replay::Recorder;
use jane_sim::{Blueprints, Sim};

#[test]
fn the_reader_plays_two_minutes_on_the_terraced_county() {
    // The only test in this binary: the switch is process-wide.
    jane_sim::blueprints::set_layout_r4(true);
    let bps = Blueprints::build(7).expect("seed 7 builds on layout r4");
    let county = bps.get(ZoneId::County);
    assert!(county.w() > 0 && county.h() > 0);
    let mut rec = Recorder::new(Sim::new_game_with(bps, "Jane"));
    let mut bot = Bot::story(Model::Reader);
    bot.play(&mut rec, 2 * 60 * 60);
    assert!(bot.stuck().is_none(), "stuck: {:?}", bot.stuck());
    assert!(rec.sim().state().tick.0 >= 2 * 60 * 60 - 60, "the clock ran: {}", rec.sim().state().tick.0);
}
