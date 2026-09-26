//! `jane hash --seed N --frames K [--every F] [--party] [--time]`: a new game on seed N, stepped
//! K frames on a built-in tape, printing the state hash (`Sim::hash`, xxh3-64 of the save's
//! encoding, ARCHITECTURE.md §3.6) every F frames and at the end. The same seed and frame count
//! print the same lines in every process and on every target: the sim's half of the
//! cross-target gate (§8 `cross_target_hash`).
//!
//! The tape is fixed by the seed: the host walks the county, a new heading every two to four
//! seconds, sprinting a quarter of the time. With `--party` guests sit down and get up and the
//! party is sent between zones. With `--time` the tick's median, p99 and worst (µs, release
//! builds mean something) and the save's size go to stderr.

use std::time::Instant;

use jane_core::{Angle, Sfc32, ZoneId};
use jane_sim::{ClientToken, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput};

pub const USAGE: &str = "  hash --seed N --frames K [--every F] [--party] [--time]
                                      a new game stepped K frames on a built-in tape; print `frame hash`
                                      every F frames (default 600) and at the end (--time: tick µs to stderr)";

/// The built-in tape.
struct Tape {
    rng: Sfc32,
    held: [InputFrame; 4],
    next_turn: [u32; 4],
    party: bool,
    cmds: Vec<StampedCommand>,
    start: jane_core::Sym,
}

impl Tape {
    fn new(seed: u32, party: bool) -> Self {
        let start = jane_sim::sym::of_name(jane_data::catalog().name_id("start").expect("\"start\" is a name"));
        Self {
            rng: Sfc32::seeded(seed, 0x7a9e),
            held: [InputFrame::IDLE; 4],
            next_turn: [0; 4],
            party,
            cmds: Vec::new(),
            start,
        }
    }

    fn frame(&mut self, f: u32) -> StepInput<'_> {
        self.cmds.clear();
        for s in 0..4 {
            if f >= self.next_turn[s] {
                self.next_turn[s] = f + 120 + self.rng.below(120);
                let dir = Angle(self.rng.next_u32() as u16);
                self.held[s] = InputFrame { sprint: self.rng.below(4) == 0, ..InputFrame::walk(dir) };
            }
        }
        if self.party {
            let mut seq = 0u16;
            let mut push = |seat: Option<Seat>, cmd| {
                seq += 1;
                self.cmds.push(StampedCommand { seat, seq, cmd });
            };
            if f == 30 {
                push(Some(Seat(0)), Command::Open(true));
            }
            if self.rng.below(600) == 0 {
                push(None, Command::Join { who: ClientToken(1 + u64::from(self.rng.below(3))) });
            }
            if self.rng.below(1500) == 0 {
                push(Some(Seat(1 + self.rng.below(3) as u8)), Command::Leave);
            }
            if self.rng.below(1200) == 0 {
                let seat = Seat(self.rng.below(4) as u8);
                let zone = ZoneId::ALL[self.rng.below(ZoneId::ALL.len() as u32) as usize];
                push(Some(seat), Command::Dev(DevOp::Tp { zone, mark: self.start }));
            }
            self.cmds.sort_by_key(|c| (c.seat, c.seq));
        }
        StepInput { frames: self.held, commands: &self.cmds }
    }
}

fn arg<T: std::str::FromStr>(args: &[String], k: &str) -> Result<Option<T>, String> {
    match args.iter().position(|a| a == k) {
        None => Ok(None),
        Some(i) => {
            let v = args.get(i + 1).ok_or_else(|| format!("{k} needs a value"))?;
            v.parse().map(Some).map_err(|_| format!("bad {k} {v}"))
        }
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let seed: u32 = arg(args, "--seed")?.unwrap_or(1);
    let frames: u32 = arg(args, "--frames")?.unwrap_or(3600);
    let every: u32 = arg(args, "--every")?.unwrap_or(600).max(1);
    let party = args.iter().any(|a| a == "--party");
    let time = args.iter().any(|a| a == "--time");

    let t0 = Instant::now();
    let mut sim = Sim::new_game(seed, "Jane");
    let built_ms = t0.elapsed().as_millis();
    println!("# jane hash --seed {seed} --frames {frames}{}", if party { " --party" } else { "" });
    println!("# content {:016x}, save version {}", jane_data::catalog().content_hash, jane_sim::state::SAVE_VERSION);
    println!("# frame hash (xxh3-64 of the state's postcard encoding)");
    println!("{:>7} {:016x}", 0, sim.hash());

    let mut tape = Tape::new(seed, party);
    let mut ticks_ns: Vec<u64> = Vec::with_capacity(frames as usize);
    for f in 0..frames {
        let input = tape.frame(f);
        let t = Instant::now();
        sim.step(&input);
        ticks_ns.push(t.elapsed().as_nanos() as u64);
        let _ = sim.drain_events();
        if (f + 1) % every == 0 || f + 1 == frames {
            println!("{:>7} {:016x}", f + 1, sim.hash());
        }
    }
    if time {
        ticks_ns.sort_unstable();
        let pick = |q: usize| ticks_ns.get((ticks_ns.len().saturating_sub(1)) * q / 100).copied().unwrap_or(0);
        let t = Instant::now();
        let h = sim.hash();
        let hash_us = t.elapsed().as_micros();
        let t = Instant::now();
        let save = sim.save();
        let save_us = t.elapsed().as_micros();
        let v = sim.view(Seat(0)).map(|v| (v.zone(), v.body().pos.cell()));
        eprintln!(
            "new game {built_ms} ms; tick over {frames} frames: median {} µs, p99 {} µs, worst {} µs; \
             hash {hash_us} µs ({h:016x}); save {} bytes in {save_us} µs; seat 0 at {v:?}",
            pick(50) / 1000,
            pick(99) / 1000,
            ticks_ns.last().copied().unwrap_or(0) / 1000,
            save.len(),
        );
        eprintln!("tick ns: median {} p99 {} worst {}", pick(50), pick(99), ticks_ns.last().copied().unwrap_or(0));
    }
    Ok(())
}
