//! PC side of the PSP sim spike (PORT.md §13.10).
//!
//! `record <out.jrp> [frames] [seed]`: a rusher bot plays a new game from New Game, recorded with a
//! hash every 60 ticks, and the tape is written. `verify <tape.jrp>`: re-simulate the tape against
//! its hash stream and print the line the PSP prints (`SIM hash=... ticks=...`) plus every hash.

use jane_bot::{Bot, Model};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use jane_sim::replay::{verify_tape, Recorder, Tape};
use jane_sim::{Blueprints, Sim};

const EVERY: u32 = 60;

/// The system allocator, counting live and peak requested bytes (as the PSP side does).
struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(n, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

fn live() -> usize {
    LIVE.load(Relaxed)
}
fn peak() -> usize {
    PEAK.load(Relaxed)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("record") => {
            let out = args.get(1).expect("record <out.jrp> [frames] [seed]");
            let frames: u32 = args.get(2).map_or(3600, |s| s.parse().expect("frames"));
            let seed: u32 = args.get(3).map_or(1, |s| s.parse().expect("seed"));
            let bps = Blueprints::build(seed).expect("blueprints");
            let mut rec = Recorder::with_period(Sim::new_game_with(bps, "Jane"), EVERY);
            let mut bot = Bot::story(Model::Rusher);
            for _ in 0..frames {
                bot.step(&mut rec);
            }
            for l in &bot.log {
                println!("bot: {}", l.line());
            }
            let (_, tape) = rec.finish();
            std::fs::write(out, tape.encode()).expect("write tape");
            println!("wrote {out}: {} bytes, {} frames, {} runs", tape.encode().len(), tape.frames, tape.runs.len());
        }
        Some("verify") => {
            let bytes = std::fs::read(args.get(1).expect("verify <tape.jrp>")).expect("read tape");
            let tape = Tape::decode(&bytes).expect("decode");
            for h in &tape.hashes {
                println!("hash frame={} tick={} {:016x}", h.frame, h.tick, h.hash);
            }
            println!("content={:016x} after decode live={} peak_heap={}", jane_data::catalog().content_hash, live(), peak());
            let mut zones = Vec::new();
            for z in jane_core::ZoneId::ALL {
                let bp = jane_sim::blueprints::build_one(z, tape.header.seed).expect("zone");
                println!(
                    "SIM zone {} bp={:016x} live={} peak_heap={}",
                    z.name(),
                    jane_world::hash::hash(&bp),
                    live(),
                    peak()
                );
                zones.push(std::sync::Arc::new(bp));
            }
            let bps = Blueprints::from_parts(tape.header.seed, zones.try_into().expect("13 zones"));
            // From here the peak is the running sim's: blueprints resident plus state and scratch.
            println!("build_peak={} resident={}", peak(), live());
            PEAK.store(live(), Relaxed);
            let v = verify_tape(&tape, bps).expect("verify");
            println!(
                "SIM hash={:016x} ticks={} frames={} hashes={} live={} peak_heap={}",
                v.final_hash,
                v.ticks,
                v.frames,
                v.hashes,
                live(),
                peak()
            );
        }
        _ => eprintln!("usage: psp-sim-host record <out.jrp> [frames] [seed] | verify <tape.jrp>"),
    }
}
