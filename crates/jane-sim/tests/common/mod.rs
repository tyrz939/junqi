//! What the sim's rule tests share: one seed's blueprints, built once per test binary, and a
//! seeded random tape of movement, joins, leaves and zone changes.

#![allow(dead_code)]

use std::sync::OnceLock;

use jane_core::{Angle, Sfc32, ZoneId};
use jane_sim::{Blueprints, ClientToken, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput};

pub const SEED: u32 = 7;

/// The blueprints of [`SEED`], built once.
pub fn bps() -> Blueprints {
    static B: OnceLock<Blueprints> = OnceLock::new();
    B.get_or_init(|| Blueprints::build(SEED).expect("seed 7 builds")).clone()
}

pub fn new_game() -> Sim {
    Sim::new_game_with(bps(), "Jane")
}

/// A seeded tape: every frame each seat's stick, sometimes a sprint; now and then a guest
/// sits down or gets up, the host opens the world, or someone is sent to another zone.
#[derive(Debug)]
pub struct Tape {
    rng: Sfc32,
    held: [InputFrame; 4],
    pub cmds: Vec<StampedCommand>,
    seq: u16,
    /// Zone changes on the tape (off for a solo tape that should stay put).
    pub travel: bool,
    /// Guests on the tape.
    pub guests: bool,
}

impl Tape {
    pub fn new(seed: u32) -> Self {
        Self {
            rng: Sfc32::seeded(seed, 9),
            held: [InputFrame::IDLE; 4],
            cmds: Vec::new(),
            seq: 0,
            travel: true,
            guests: true,
        }
    }

    fn cmd(&mut self, seat: Option<Seat>, cmd: Command) {
        self.seq = self.seq.wrapping_add(1);
        self.cmds.push(StampedCommand { seat, seq: self.seq, cmd });
    }

    /// Frame `f`'s input (commands in `self.cmds`, sorted).
    pub fn frame(&mut self, f: u32) -> StepInput<'_> {
        self.cmds.clear();
        for s in 0..4 {
            // A stick held for a while, then another.
            if self.rng.below(40) == 0 {
                let dir = Angle(self.rng.next_u32() as u16);
                let mag = if self.rng.below(5) == 0 { 0 } else { 40 + self.rng.below(88) as u8 };
                self.held[s] =
                    InputFrame { mv_dir: dir, mv_mag: mag, sprint: self.rng.below(3) == 0, ..InputFrame::IDLE };
            }
        }
        if f == 5 && self.guests {
            self.cmd(Some(Seat(0)), Command::Open(true));
        }
        if self.guests && self.rng.below(150) == 0 {
            let who = ClientToken(1 + u64::from(self.rng.below(4)));
            self.cmd(None, Command::Join { who });
        }
        if self.guests && self.rng.below(400) == 0 {
            let seat = Seat(1 + self.rng.below(3) as u8);
            self.cmd(Some(seat), Command::Leave);
        }
        if self.travel && self.rng.below(300) == 0 {
            let seat = Seat(self.rng.below(4) as u8);
            let zone = ZoneId::ALL[self.rng.below(ZoneId::ALL.len() as u32) as usize];
            let mark = jane_sim::sym::of_name(jane_data::catalog().name_id("start").unwrap());
            self.cmd(Some(seat), Command::Dev(DevOp::Tp { zone, mark }));
        }
        self.cmds.sort_by_key(|c| (c.seat, c.seq));
        StepInput { frames: self.held, commands: &self.cmds }
    }
}

/// Run frames `from..to` of `tape` on `sim`.
pub fn run(sim: &mut Sim, tape: &mut Tape, from: u32, to: u32) {
    for f in from..to {
        let input = tape.frame(f);
        sim.step(&input);
    }
}
