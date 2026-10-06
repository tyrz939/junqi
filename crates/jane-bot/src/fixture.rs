//! The bot-session hash fixture (PORT.md §9.3 gate 4; ARCHITECTURE.md §8 `cross_target_hash`):
//! each model plays the first five minutes of seeds 1 to 3 from New Game, recorded, and the
//! tape's hash stream (a hash every 600 ticks, and the last state's) is written one line per
//! hash. `jane play --fixture` writes `tests/fixtures/bot-hash-<target>.txt`; the bot's
//! `fixture` test holds this build to it, and every target's file must be byte-equal.

use jane_sim::replay::{Recorder, Tape};
use jane_sim::{Blueprints, Sim};

use crate::{Bot, Model};

pub const SEEDS: [u32; 3] = [1, 2, 3];
pub const MODELS: [Model; 2] = [Model::Reader, Model::Rusher];
/// Five real minutes of frames.
pub const FRAMES: u32 = 5 * 60 * 60;

/// One session: `model` on a new game of `bps`'s seed for [`FRAMES`] frames, recorded.
pub fn session(bps: Blueprints, model: Model) -> Tape {
    let mut rec = Recorder::new(Sim::new_game_with(bps, "Jane"));
    let mut bot = Bot::story(model);
    for _ in 0..FRAMES {
        bot.step(&mut rec);
    }
    rec.finish().1
}

/// The header line naming what the file was written from; a file with another is stale.
pub fn content_line() -> String {
    format!(
        "# content {:016x}, save version {}, replay version {}",
        jane_data::catalog().content_hash,
        jane_sim::state::SAVE_VERSION,
        jane_sim::replay::REPLAY_VERSION
    )
}

/// A session's lines: `seed model frame tick hash`.
pub fn lines(seed: u32, model: Model, tape: &Tape) -> Vec<String> {
    tape.hashes.iter().map(|h| format!("{seed} {} {} {} {:016x}", model.name(), h.frame, h.tick, h.hash)).collect()
}

/// The whole file, built with `bps(seed)` for each seed.
pub fn text(mut bps: impl FnMut(u32) -> Blueprints) -> String {
    let mut out = String::new();
    out += "# jane play --fixture: the first five minutes of seeds 1 to 3, reader and rusher\n";
    out += &content_line();
    out += "\n# seed model frame tick hash (xxh3-64 of the state, after the step of that frame)\n";
    for seed in SEEDS {
        let b = bps(seed);
        for model in MODELS {
            let tape = session(b.clone(), model);
            for l in lines(seed, model, &tape) {
                out += &l;
                out.push('\n');
            }
        }
    }
    out
}
