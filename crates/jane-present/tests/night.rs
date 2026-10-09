//! The night's look (NIGHT.md §4, §9 R3): the albedo LUT and the overlays go in in the turn's
//! dark second on every tier and the console alike; the PC's chunks are painted again under it;
//! the overlays are a pure function of the seed and the chunk, never the same boards on two
//! windows side by side, and a console draws no more of them than its preset allows.

use jane_art::night::Look;
use jane_art::terrain::{Chunk, Opening};
use jane_present::night::{Overlay, Sockets, place};
use jane_present::{Pass, Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::tuning::TICKS_PER_HOUR;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const S: u32 = jane_core::num::TICK_RATE;

/// Seed 1 at the stile on the farm's hedge (the Lowfields: the night's intensity is the stage),
/// the clock a second short of nine.
fn before_nine(mark: &str) -> Sim {
    let mut sim = Sim::new_game(1, "Jane");
    let m = sim.view(Seat(0)).and_then(|v| v.sym(mark)).expect("the mark");
    let tp = [StampedCommand {
        seat: Some(Seat(0)),
        seq: 1,
        cmd: Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::County, mark: m }),
    }];
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &tp });
    for _ in 0..30 {
        sim.step(&StepInput::IDLE);
    }
    sim.state_mut().clock = 21 * TICKS_PER_HOUR - S;
    sim
}

fn step(sim: &mut Sim, ps: &mut [&mut Present]) {
    sim.step(&StepInput::IDLE);
    let events = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    for p in ps {
        p.tick(&v, &events);
    }
}

#[test]
fn the_night_goes_in_in_the_dark_second_on_every_tier_and_the_chunks_are_painted_again() {
    let mut sim = before_nine("hedge_stile_farm");
    let mut t0 = Present::new(Tier::T0);
    let mut t2 = Present::new(Tier::T2);
    let mut c2 = Present::from_tables_console(Tier::T0, &t0.tables(), 12).unwrap();
    let canvas = (480, 272);
    let mut before = None;
    let mut went_in = None;
    for t in 0..(6 * S) {
        step(&mut sim, &mut [&mut t0, &mut t2, &mut c2]);
        let v = sim.view(Seat(0)).unwrap();
        let night = v.night();
        let since = v.tick().0.wrapping_sub(night.turned_at.0);
        let a = t0.draw(0, canvas).night;
        let first = t0.frame().chunks.first().map(|c| (c.id, c.generation));
        if before.is_none() {
            before = first;
        }
        assert_eq!(a, t2.draw(0, canvas).night, "tick {t}: T2's night");
        assert_eq!(a, c2.draw(0, canvas).night, "tick {t}: the console's night");
        if night.stage == 0 || since < 2 * S / 5 {
            assert_eq!(a, 0, "tick {t}: nothing of the night before the gutter's floor");
        } else if went_in.is_none() {
            assert_eq!(a, 1, "tick {t}: in at the held dark, the fields' intensity at N1");
            went_in = Some(t);
        }
    }
    assert!(went_in.is_some(), "the night went in");
    // The chunk under the view was painted again in the night's materials.
    let (id, gen0) = before.expect("chunks in view");
    let gen1 = t0.chunk(id).expect("still held").1;
    assert_ne!(gen0, gen1, "painted again under the turn");
}

#[test]
fn a_console_draws_its_overlays_from_its_night_page_under_its_cap() {
    // The square at N4 (the town at 3): windows boarded, chalk on the setts.
    let mut sim = before_nine("town_square");
    // As `jane sheet scene --flag` sets them: the flag, and the spine's consequence of the name.
    for f in ["mine_quiet", "works_dark", "burial_quiet"] {
        if let Some(c) = jane_data::catalog().living.consequence_id(f) {
            sim.state_mut().consequences_done.set(u32::from(c.0), true);
        }
        if let Some(k) = sim.state().syms.find(f) {
            sim.state_mut().flags.insert(jane_sim::state::FlagKey::Named(k), 1);
        }
    }
    let t0 = Present::new(Tier::T0);
    let mut c2 = Present::from_tables_console(Tier::T0, &t0.tables(), 12).unwrap();
    // Painted as a console paints: by jobs (its worker's), landed as they come back.
    for _ in 0..(10 * S) {
        step(&mut sim, &mut [&mut c2]);
        let v = sim.view(Seat(0)).unwrap();
        if let Some(mut job) = c2.take_paint_job(&v) {
            job.run();
            c2.land(job);
        }
    }
    let canvas = (480, 272);
    let count = |p: &mut Present| {
        let f = p.draw(0, canvas);
        f.passes
            .iter()
            .filter_map(|p| match p {
                Pass::Sprites { layer: jane_present::Depth::Ground, cmds } => Some(cmds.len),
                _ => None,
            })
            .sum::<u32>()
    };
    c2.set_night_cap(0);
    let none = count(&mut c2);
    c2.set_night_cap(5);
    let five = count(&mut c2);
    c2.set_night_cap(48);
    let more = count(&mut c2);
    assert!(five > none && five - none <= 5, "{none} {five}");
    assert!(more >= five, "{five} {more}");
}

#[test]
fn the_overlays_are_the_seeds_and_two_windows_side_by_side_never_take_the_same_boards() {
    let mut c = Chunk::new();
    // A long front: a casement in every cell of a row, and an upper floor over it.
    for x in 0..16 {
        c.openings.push(Opening { x: x * 16 + 3, y: 3 * 16 + 3, w: 10, h: 10, lit: x % 3 != 0, upper: false });
        c.openings.push(Opening { x: x * 16 + 4, y: 2 * 16 + 7, w: 8, h: 8, lit: false, upper: true });
    }
    let looks = jane_art::night::all();
    let mut s = Sockets::new();
    s.copy_from(&c);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for seed in 1..40 {
        place(&s, (3, 5), seed, 4, &|_, _| 4, &looks, &mut a);
        place(&s, (3, 5), seed, 4, &|_, _| 4, &looks, &mut b);
        assert_eq!(a, b, "seed {seed}: the same overlays twice");
        let boards: Vec<&Overlay> =
            a.iter().filter(|o| matches!(looks[usize::from(o.look)], Look::Boards { .. })).collect();
        for p in &boards {
            for q in &boards {
                // Along one front, up to four cells apart.
                let near = p.y == q.y && p.x != q.x && (i32::from(p.x) - i32::from(q.x)).abs() <= 4 * 16;
                if near {
                    assert!(
                        (p.look, p.mirror) != (q.look, q.mirror),
                        "seed {seed}: the same boards at {:?} and {:?}",
                        (p.x, p.y),
                        (q.x, q.y)
                    );
                }
            }
        }
    }
    // By day, nothing.
    place(&s, (3, 5), 1, 0, &|_, _| 0, &looks, &mut a);
    assert!(a.is_empty());
}
