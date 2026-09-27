//! A lesson's moment (PRESENTATION.md §2.1), played through the real sim from New Game: it never
//! touches the world (the state hash is the same with a presenter watching or not), two spells
//! learned at once queue rather than overlap, the first is the first for every seat at a table,
//! and with company nothing is ever held.

use jane_present::audio::{NullBus, Soundtrack};
use jane_present::lesson::{BREATH, FIRST, Gift, SPELL};
use jane_present::view::ViewBuffers;
use jane_present::{Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{ClientToken, Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

fn spell(name: &str) -> jane_core::SpellId {
    jane_data::catalog().combat.spell_id(name).expect("a spell")
}

/// The commands of tick `t`: she learns icebolt and fireball in the same tick, and walks a
/// little either side of it.
fn tape(t: u32) -> (Vec<StampedCommand>, InputFrame) {
    let dev = |seq: u16, op| StampedCommand { seat: Some(Seat(0)), seq, cmd: Command::Dev(op) };
    let cmds = if t == 5 {
        vec![dev(1, DevOp::God(true)), dev(2, DevOp::Learn(spell("icebolt"))), dev(3, DevOp::Learn(spell("fireball")))]
    } else {
        Vec::new()
    };
    let walk = if (20..80).contains(&t) || (400..460).contains(&t) {
        InputFrame { mv_dir: jane_core::Angle::EAST, mv_mag: 127, ..InputFrame::IDLE }
    } else {
        InputFrame::IDLE
    };
    (cmds, walk)
}

#[test]
fn a_moment_never_touches_the_world_and_two_queue_one_after_the_other() {
    let ticks = FIRST.end + BREATH + SPELL.end + 40;
    let mut watched = Sim::new_game(1, "Jane");
    let mut alone = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let mut bufs = ViewBuffers::new();
    let mut sound = Soundtrack::new();
    let mut began = Vec::new();
    let mut under_way = 0;
    for t in 0..ticks {
        let (cmds, walk) = tape(t);
        for s in [&mut watched, &mut alone] {
            s.step(&StepInput { frames: [walk; 4], commands: &cmds });
        }
        let events = watched.drain_events().to_vec();
        alone.drain_events();
        let v = watched.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
        bufs.tick(&v, &events);
        sound.tick(&v, &events, &mut NullBus);
        sound.lesson(p.lessons(), &mut NullBus);
        if t % 3 == 0 {
            p.draw(128, CANVAS);
        }
        if let Some(g) = p.lessons().began() {
            began.push((t, g));
        }
        under_way += u32::from(p.lessons().moment().is_some());
        // The learned toast is the moment's to say, not a toast.
        assert!(bufs.hud.toasts.iter().all(|x| !x.text.starts_with("Learned")), "tick {t}");
        assert_eq!(watched.hash(), alone.hash(), "tick {t}: the presenter changed the world");
    }
    assert_eq!(began.len(), 2, "{began:?}");
    let (t0, g0) = began[0];
    let (t1, g1) = began[1];
    assert!(matches!(g0, Gift::Spell { first: true, .. }), "icebolt is her first: {g0:?}");
    assert!(matches!(g1, Gift::Spell { first: false, .. }), "fireball is not: {g1:?}");
    // The second waits for the first to end and a breath after it: never two at once.
    assert_eq!(t1 - t0, FIRST.end + BREATH, "queued, not overlapped");
    assert_eq!(under_way, FIRST.end + SPELL.end, "one moment at a time, each its whole length");
    assert!(p.lessons().moment().is_none() && p.lessons().waiting() == 0);
}

#[test]
fn at_a_table_each_seat_has_its_first_and_nothing_is_held() {
    let mut s = Sim::new_game(1, "Jane");
    let host = |cmd| StampedCommand { seat: Some(Seat(0)), seq: 1, cmd };
    s.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[host(Command::Open(true))] });
    let join = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(11) } }];
    s.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &join });
    assert_eq!(s.state().party_size(), 2);
    // The guest learns it; the table knows it; each machine plays its own seat's first.
    let learn = [StampedCommand { seat: Some(Seat(1)), seq: 2, cmd: Command::Dev(DevOp::Learn(spell("icebolt"))) }];
    let mut ps = [Box::new(Present::new(Tier::T0)), Box::new(Present::new(Tier::T0))];
    let mut firsts = [false; 2];
    for t in 0..FIRST.end + 10 {
        let cmds: &[StampedCommand] = if t == 0 { &learn } else { &[] };
        s.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
        let events = s.drain_events().to_vec();
        for (seat, p) in ps.iter_mut().enumerate() {
            let v = s.view(Seat(seat as u8)).expect("both seated");
            p.tick(&v, &events);
            if let Some(Gift::Spell { first, .. }) = p.lessons().began() {
                firsts[seat] = first;
            }
            // With company the world never holds, whatever the moment; alone it would.
            assert!(!p.lessons().holds_world(false), "tick {t}: held with company");
            if t < FIRST.hold && p.lessons().moment().is_some() {
                assert!(p.lessons().holds_world(true), "tick {t}: alone, the first holds");
            }
        }
    }
    assert_eq!(firsts, [true, true], "the first spell is the first for each seat");
}
