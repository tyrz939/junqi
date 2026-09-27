//! VERIFICATION.md §3.1: a trace is an observer and a pure function of its inputs. A traced
//! session steps and hashes as an untraced one; the same seed, model and build write the same
//! bytes; the bytes read back as they were written; and the L4 table is computed from them alone.

mod common;

use common::*;
use jane_bot::run::Session;
use jane_bot::{Bot, Model};
use jane_sim::trace::{Kind, Trace};

fn traced(seed: u32, model: Model, frames: u32) -> (u64, Trace) {
    let mut sim = new_game(seed);
    let mut s = Session::new(Bot::story(model), &sim, frames / MINUTE);
    for _ in 0..frames {
        s.step(&mut sim);
    }
    let (_, t) = s.finish(&sim);
    (sim.hash(), t)
}

#[test]
fn a_traced_session_steps_and_hashes_as_an_untraced_one() {
    for model in [Model::Reader, Model::Lost] {
        let frames = 4 * MINUTE;
        let mut sim = new_game(1);
        let mut bot = Bot::story(model);
        for _ in 0..frames {
            bot.step(&mut sim);
        }
        let (hash, t) = traced(1, model, frames);
        assert_eq!(hash, sim.hash(), "{}: the trace changed the run", model.name());
        assert!(t.records.iter().any(|r| matches!(r.kind, Kind::Sample(_))));
        assert!(t.records.iter().any(|r| matches!(r.kind, Kind::Decision { .. })));
        assert!(t.records.iter().any(|r| matches!(r.kind, Kind::Hash(_))));
    }
}

#[test]
fn the_same_session_writes_the_same_trace_and_it_reads_back() {
    let (_, a) = traced(2, Model::Rusher, 3 * MINUTE);
    let (_, b) = traced(2, Model::Rusher, 3 * MINUTE);
    let (ea, eb) = (a.encode(), b.encode());
    assert_eq!(ea, eb, "same seed, model and build: the same bytes");
    let back = Trace::decode(&ea).expect("a trace reads back");
    assert_eq!(back, a);
    let x = jane_bot::experience::measure(&back, 0);
    assert_eq!(x, jane_bot::experience::measure(&a, 0), "L4 is a function of the bytes");
    println!("{} records, {} bytes; L4: {} quests given", a.records.len(), ea.len(), x.quests.len());
}
