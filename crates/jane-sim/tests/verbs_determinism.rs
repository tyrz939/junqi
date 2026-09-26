//! The determinism gates of ARCHITECTURE.md §8 with the world verbs on the tape: USE (talk,
//! loot, lift, push), conversations advanced, chosen and closed, bags moved, crafting, items,
//! gifts and quests from the console. `same_tape_same_hash`, `runtime_rebuild_is_invisible` and
//! `save_load_continue` over it; run under `--profile checked` too.

mod common;

use common::bps;
use jane_core::{Angle, Sfc32, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

/// A seeded tape for one seat at Julie's door, the dog on the step: a stick held a while, USE held now and then,
/// and a command most frames.
struct VerbTape {
    rng: Sfc32,
    held: InputFrame,
    cmds: Vec<StampedCommand>,
    seq: u16,
}

impl VerbTape {
    fn new(seed: u32) -> Self {
        Self { rng: Sfc32::seeded(seed, 3), held: InputFrame::IDLE, cmds: Vec::new(), seq: 0 }
    }

    fn cmd(&mut self, cmd: Command) {
        self.seq = self.seq.wrapping_add(1);
        self.cmds.push(StampedCommand { seat: Some(Seat(0)), seq: self.seq, cmd });
    }

    fn frame(&mut self, f: u32) -> StepInput<'_> {
        let cat = jane_data::catalog();
        self.cmds.clear();
        if f == 0 {
            let gate = jane_sim::sym::of_name(cat.name_id("house_front").unwrap());
            self.cmd(Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
            let q = cat.story.quest_id("defeat_skeleton").unwrap();
            self.cmd(Command::Dev(DevOp::Quest(q)));
        }
        if self.rng.below(30) == 0 {
            let dir = Angle(self.rng.next_u32() as u16);
            let mag = if self.rng.below(4) == 0 { 0 } else { 60 + self.rng.below(68) as u8 };
            self.held = InputFrame { mv_dir: dir, mv_mag: mag, use_held: self.rng.below(4) == 0, ..InputFrame::IDLE };
        }
        match self.rng.below(24) {
            0 | 1 => self.cmd(Command::Use),
            2 => self.cmd(Command::Advance),
            3 => {
                let option = self.rng.below(2) as u8;
                self.cmd(Command::Choose { option });
            }
            4 => self.cmd(Command::CloseDialogue),
            5 => {
                let (from, to) = (self.rng.below(24) as u8, self.rng.below(24) as u8);
                self.cmd(Command::BagMove { from, to });
            }
            6 => {
                let (bag, slot) = (self.rng.below(24) as u8, self.rng.below(3) as u8);
                self.cmd(Command::CraftPut { bag, slot });
            }
            7 => self.cmd(Command::CraftTake),
            8 => self.cmd(Command::CraftClearAll),
            9 => self.cmd(Command::Item(cat.combat.item_id("apple").unwrap())),
            10 => {
                let item = cat
                    .combat
                    .item_id(["gold_dust", "pansy", "small_water", "rock"][self.rng.below(4) as usize])
                    .unwrap();
                self.cmd(Command::Dev(DevOp::Give { item, qty: 1 }));
            }
            12 => self.cmd(Command::Item(cat.combat.item_id("julies_letter").unwrap())),
            11 if self.rng.below(8) == 0 => {
                let slot = self.rng.below(24) as u8;
                self.cmd(Command::BagDestroy { slot });
            }
            _ => {}
        }
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = self.held;
        StepInput { frames, commands: &self.cmds }
    }
}

fn run(sim: &mut Sim, tape: &mut VerbTape, from: u32, to: u32) -> (u32, u32) {
    let (mut talks, mut bags) = (0, 0);
    for f in from..to {
        let input = tape.frame(f);
        sim.step(&input);
        for e in sim.drain_events() {
            match e.kind {
                EventKind::Dialogue => talks += 1,
                EventKind::Bag => bags += 1,
                _ => {}
            }
        }
    }
    (talks, bags)
}

fn new_game() -> Sim {
    common::new_game()
}

#[test]
fn same_verb_tape_same_hash() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (VerbTape::new(5), VerbTape::new(5));
    let (mut talks, mut bags) = (0, 0);
    for chunk in 0..15 {
        let (t, g) = run(&mut a, &mut ta, chunk * 120, (chunk + 1) * 120);
        run(&mut b, &mut tb, chunk * 120, (chunk + 1) * 120);
        talks += t;
        bags += g;
        assert_eq!(a.hash(), b.hash(), "frame {}", (chunk + 1) * 120);
    }
    assert_eq!(a.state(), b.state());
    // The tape did what it says.
    assert!(talks > 0 && bags > 0, "talks {talks}, bags {bags}");
}

#[test]
fn a_runtime_rebuilt_mid_verbs_is_invisible() {
    let mut a = new_game();
    let mut b = new_game();
    let (mut ta, mut tb) = (VerbTape::new(8), VerbTape::new(8));
    for f in 0..1500 {
        run(&mut a, &mut ta, f, f + 1);
        if f % 53 == 7 {
            b.rebuild_runtimes();
        }
        run(&mut b, &mut tb, f, f + 1);
        if f % 60 == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(a.state(), b.state());
}

#[test]
fn save_load_continue_with_verbs_on_the_tape() {
    let mut straight = new_game();
    let mut ts = VerbTape::new(13);
    run(&mut straight, &mut ts, 0, 900);
    for at in [131, 257, 611] {
        let mut first = new_game();
        let mut t = VerbTape::new(13);
        run(&mut first, &mut t, 0, at);
        let bytes = first.save();
        let mut resumed = Sim::from_save_with(&bytes, bps()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash(), "loading changes nothing (frame {at})");
        run(&mut resumed, &mut t, at, 900);
        assert_eq!(resumed.hash(), straight.hash(), "saved at {at}");
        assert_eq!(resumed.state(), straight.state());
    }
}
