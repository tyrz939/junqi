//! Three rats in Julie's cellar, fought every way a player might (the owner's PSP freeze in
//! v0.5.0, "froze fighting 3 rats in the basement", seed 555721351): from the corridor, the rat
//! room's spur and doorway and the room itself; walking at them, away, round, standing in the way;
//! a target held or let go; the bar, melee and the bolts; Use; mortal and god; hurt to one HP and
//! killed. Three chasers in a small room blocking each other is the lean chase's edge
//! (`ai::chase`, the zone's search budget and `ZoneState::path_queue`, a held step's wait and its
//! bounded look round, the sight cache).
//!
//! The blueprints are the console's (packed, built on demand and let go), and the zones the doors
//! near her lead to are handed in as the PSP's shell builds them ahead.
//!
//! What must hold: every step returns (a step not back in [`STALL_SECS`] fails the test rather
//! than hanging it), the refused-search queue never holds more than the zone's units (a queue
//! that never drains grows), and the fights are real (the rats noticed her and most died).
//! `JANE_CELLAR_VARIANTS` runs more variants than the gate's (hundreds, for a hunt).

use std::sync::{Arc, mpsc};
use std::time::Duration;

use jane_core::{Angle, Sfc32, SpellId, Vec2, ZoneId};
use jane_sim::blueprints::{Build, build_one_packed_with};
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput, TargetRef, UnitId};

/// The owner's seed.
const SEED: u32 = 555_721_351;
/// Variants the gate plays.
const VARIANTS: u32 = 32;
/// Seconds a step may take before the sim counts as hung.
const STALL_SECS: u64 = 20;
/// Where she is stood: the corridor under the rat room, its spur and doorway, inside it, and
/// further along the corridor.
const SPOTS: [(i32, i32); 8] = [(14, 36), (15, 33), (13, 31), (12, 25), (10, 20), (20, 28), (8, 38), (30, 38)];
/// How near a door the shell builds its zone ahead, in cells (`ahead.rs`'s `NEAR`, and more: the
/// cellar's stairs are far from the rats).
const AHEAD_CELLS: i32 = 48;
const SPELLS: [&str; 3] = ["icebolt", "fireball", "poisonbolt"];

fn step(s: &mut Sim, f: InputFrame, c: Option<Command>) {
    let cmds: Vec<StampedCommand> =
        c.into_iter().map(|cmd| StampedCommand { seat: Some(Seat(0)), seq: 0, cmd }).collect();
    let mut frames = [InputFrame::IDLE; 4];
    frames[0] = f;
    s.step(&StepInput { frames, commands: &cmds });
}

/// What one variant came to.
#[derive(Default)]
struct Tally {
    fought: bool,
    killed: usize,
}

/// Variant `v`: a new game, sent down to the cellar and stood by the rats, then played at random
/// for a minute or so. `beat` hears every step before it runs.
fn variant(bps: &Blueprints, v: u32, beat: &mpsc::Sender<(u32, u32)>) -> Tally {
    let mut rng = Sfc32::seeded(SEED, v);
    let mut s = Sim::new_game_with(bps.clone(), "Jane");
    let spells: Vec<SpellId> = SPELLS.iter().filter_map(|n| jane_data::catalog().combat.spell_id(n)).collect();
    if v % 4 != 3 {
        for &id in &spells {
            step(&mut s, InputFrame::IDLE, Some(Command::Dev(DevOp::Learn(id))));
        }
    }
    let god = v % 3 != 0;
    if god {
        step(&mut s, InputFrame::IDLE, Some(Command::Dev(DevOp::God(true))));
    }
    let mark = s.view(Seat(0)).and_then(|vw| vw.sym("stair_a")).expect("the cellar's stair");
    step(&mut s, InputFrame::IDLE, Some(Command::Dev(DevOp::Tp { zone: ZoneId::Cellar, mark })));
    for _ in 0..3 {
        step(&mut s, InputFrame::IDLE, None);
    }
    let her = s.state().players[0].unit;
    assert_eq!(s.state().players[0].zone, ZoneId::Cellar, "variant {v}: she is in the cellar");
    let (x, y) = SPOTS[rng.below(SPOTS.len() as u32) as usize];
    if let Some(u) = s.state_mut().zone_mut(ZoneId::Cellar).and_then(|z| z.unit_mut(her)) {
        u.pos = Vec2::centre(x, y);
    }
    s.rebuild_runtimes();
    let ticks = 1200 + rng.below(2400);
    let (mut dir, mut mag, mut seek) = (Angle::EAST, 0u8, false);
    let mut tally = Tally::default();
    for t in 0..ticks {
        let Some(z) = s.state().zone(ZoneId::Cellar) else { break };
        assert!(
            z.path_queue.len() <= z.units.len(),
            "variant {v} tick {t}: {} walkers queued for a search in a zone of {} units",
            z.path_queue.len(),
            z.units.len()
        );
        let rats: Vec<UnitId> = z.units.iter().filter(|u| u.alive && u.id != her).map(|u| u.id).collect();
        tally.fought |= z.units.iter().any(|u| u.id != her && u.combat == CombatState::Combat);
        if rng.one_in(20) {
            mag = [0, 0, 40, 127, 127, 127][rng.below(6) as usize];
            seek = !rng.one_in(3);
            dir = Angle(rng.below(65_536) as u16);
        }
        if seek {
            let at = |id: UnitId| z.unit(id).map(|u| u.pos);
            if let (Some(a), Some(b)) = (at(her), rats.first().and_then(|&r| at(r))) {
                dir = jane_core::angle::bearing(a, b);
            }
        }
        let on = (!rats.is_empty() && !rng.one_in(4)).then(|| rats[rng.below(rats.len() as u32) as usize]);
        let f = InputFrame {
            mv_dir: dir,
            mv_mag: mag,
            target: on.map(TargetRef::Unit),
            sprint: rng.one_in(3),
            ..InputFrame::IDLE
        };
        let c = if !god && t == ticks / 3 {
            Some(Command::Dev(DevOp::Hp(1)))
        } else if s.frozen() {
            Some(Command::CloseDialogue)
        } else if rng.one_in(12) {
            Some(Command::Use)
        } else if rng.one_in(10) {
            Some(Command::Cast { spell: spells[rng.below(spells.len() as u32) as usize], on })
        } else if rng.one_in(15) {
            Some(Command::Bar { slot: rng.below(4) as u8, on })
        } else {
            None
        };
        // As the console's shell builds ahead (`spikes/psp-game/src/ahead.rs`): every half
        // second, a zone a door near her leads to and not held is handed in, then let go again.
        if t % 30 == 0 {
            for z in s.zones_ahead(Seat(0), AHEAD_CELLS) {
                if s.blueprints().held_now(z).is_none() {
                    let bp = build_one_packed_with(z, SEED, &mut |_| {}).expect("a zone ahead builds");
                    s.offer_blueprint(Arc::new(bp));
                }
            }
        }
        let _ = beat.send((v, t));
        step(&mut s, f, c);
    }
    if let Some(z) = s.state().zone(ZoneId::Cellar) {
        tally.killed = z.units.iter().filter(|u| u.id != her && !u.alive).count();
    }
    tally
}

#[test]
fn three_rats_in_the_cellar_never_hang_the_sim() {
    #[allow(clippy::disallowed_methods)]
    let n = std::env::var("JANE_CELLAR_VARIANTS").ok().and_then(|s| s.parse().ok()).unwrap_or(VARIANTS);
    let (beat, heard) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        // The console's blueprints: packed, built on demand and let go (PORT.md §13.3).
        let source = Arc::new(Build { packed: true, load: None });
        let bps = Blueprints::on_demand_with(SEED, source, &mut |_| {}).expect("the owner's seed builds");
        (0..n).map(|v| variant(&bps, v, &beat)).collect::<Vec<_>>()
    });
    // A step not back within STALL_SECS is a hang: the test fails (the worker is left spinning
    // and goes with the process).
    let mut last = None;
    loop {
        match heard.recv_timeout(Duration::from_secs(STALL_SECS)) {
            Ok(at) => last = Some(at),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the sim hung: (variant, tick) {last:?} has not come back in {STALL_SECS} s")
            }
        }
    }
    let tallies = worker.join().expect("every variant played");
    let fought = tallies.iter().filter(|t| t.fought).count();
    let killed: usize = tallies.iter().map(|t| t.killed).sum();
    println!("{n} variants: {fought} fights, {killed} rats killed");
    assert!(fought * 4 >= tallies.len() * 3, "the rats fought her in most variants: {fought} of {}", tallies.len());
    assert!(killed >= tallies.len(), "rats died: {killed} in {} variants", tallies.len());
}
