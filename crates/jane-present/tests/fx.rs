//! A bolt's end (PRESENTATION.md §2): the sim says `Impact` wherever a bolt dies, on a body, on
//! a wall or where its range runs out, and the presenter bursts the spell's impact there, in the
//! frame's parts and with its light. Played through the real sim from New Game.

use jane_present::frame::FX_TO_CANVAS;
use jane_present::{Present, Tier};
use jane_sim::event::EventKind;
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

/// Where a bolt ended, zone canvas px, and how far from her it flew, px.
struct End {
    at: (i32, i32),
    flew: i32,
}

/// A new game, she learns icebolt (and, with `spawn`, a skeleton stands east of her), casts it
/// east, and the world runs until the bolt is gone; the presenter ticks beside it. Returns the
/// presenter just after the tick the impact landed on, and where it landed.
fn cast_east(spawn: bool) -> (Present, End) {
    let cat = jane_data::catalog();
    let spell = cat.combat.spell_id("icebolt").expect("icebolt is a spell");
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let seat = Seat(0);
    let dev = |seq: u16, op| StampedCommand { seat: Some(seat), seq, cmd: Command::Dev(op) };
    let mut script: Vec<Vec<StampedCommand>> =
        vec![vec![dev(1, DevOp::God(true)), dev(2, DevOp::Learn(spell)), dev(3, DevOp::Mp(9999))], Vec::new()];
    if spawn {
        let skel = cat.combat.unit_id("skeleton").expect("a skeleton");
        script.push(vec![dev(4, DevOp::Spawn(skel))]);
        script.push(Vec::new());
    }
    for cmds in &script {
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
        let events = sim.drain_events().to_vec();
        p.tick(&sim.view(seat).expect("seat 0 plays"), &events);
    }
    let from = sim.view(seat).expect("seat 0 plays").body().pos;
    let aim = InputFrame { aim: Some(jane_core::Angle::EAST), ..InputFrame::IDLE };
    let cast = [StampedCommand { seat: Some(seat), seq: 5, cmd: Command::Cast { spell, on: None } }];
    let mut flying = false;
    for k in 0..400 {
        let cmds: &[StampedCommand] = if k == 0 { &cast } else { &[] };
        sim.step(&StepInput { frames: [aim; 4], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(seat).expect("seat 0 plays");
        flying |= v.projectiles().iter().any(|b| b.spell == spell);
        p.tick(&v, &events);
        let end = events.iter().find_map(|e| match e.kind {
            EventKind::Impact { spell: s, at, .. } if s == spell => Some(at),
            _ => None,
        });
        if let Some(at) = end {
            assert!(flying, "the bolt was in the view before it ended");
            assert!(!v.projectiles().iter().any(|b| b.spell == spell), "an impact ends the bolt");
            let px = |f: jane_core::Fx| f.0 >> FX_TO_CANVAS;
            let at = (px(at.x), px(at.y));
            let flew = (at.0 - px(from.x)).abs().max((at.1 - px(from.y)).abs());
            return (p, End { at, flew });
        }
    }
    panic!("the bolt never ended");
}

/// The frame just after the impact holds its burst and its light at the point.
fn shows_the_impact(p: &mut Present, at: (i32, i32)) {
    let f = p.draw(255, CANVAS);
    let (x, y) = (at.0 - f.camera.0, at.1 - f.camera.1);
    let lit = f.lights.iter().any(|l| !l.casts && (l.pos.0 - x).abs() <= 1 && (l.pos.1 - y).abs() <= 1);
    assert!(lit, "the impact throws its light at ({x}, {y})");
    // Its sparks start 16 px up (the head's height) and its ring lies on the ground.
    let near = f.parts.iter().filter(|q| (i32::from(q.x) - x).abs() <= 24 && (i32::from(q.y) - y).abs() <= 32).count();
    eprintln!("impact at ({x}, {y}): {near} parts near it");
    assert!(near >= 8, "{near} parts of the burst near ({x}, {y})");
}

#[test]
fn a_bolt_at_the_end_of_its_range_lands_an_impact() {
    let (mut p, end) = cast_east(false);
    eprintln!("the bolt flew {} px", end.flew);
    // Nothing east of the start for its range: it flew the whole of it (15 cells, 240 px).
    assert!(end.flew >= 200, "it flew {} px", end.flew);
    shows_the_impact(&mut p, end.at);
}

#[test]
fn a_bolt_that_hits_a_body_lands_an_impact_on_it() {
    let (mut p, end) = cast_east(true);
    eprintln!("the bolt flew {} px", end.flew);
    // The skeleton stands three cells east: the bolt ends on it, well short of its range.
    assert!(end.flew < 120, "it flew {} px: it missed the skeleton", end.flew);
    shows_the_impact(&mut p, end.at);
}

/// Loot glints (the owner's first playtest: a corpse worth looting sparkles, WoW-style). A drop
/// on the ground throws a gold, glowing mote now and then; where nothing lies nothing glints;
/// taken, it glints no more.
#[test]
fn a_drop_glints_until_it_is_taken() {
    let seat = Seat(0);
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let run = |sim: &mut Sim, p: &mut Present, n: u32| {
        for _ in 0..n {
            sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
            let events = sim.drain_events().to_vec();
            p.tick(&sim.view(seat).expect("seat 0 plays"), &events);
        }
    };
    run(&mut sim, &mut p, 2);
    let (her, zone) = {
        let v = sim.view(seat).expect("seat 0 plays");
        (v.body().pos, v.zone())
    };
    // A rat's meat, lying a few cells east of her, where the rat fell.
    let at = jane_core::Vec2::new(her.x + jane_core::Fx::from_px(24), her.y);
    let item = jane_data::catalog().combat.item_id("rat_meat").expect("rat meat");
    let st = sim.state_mut();
    let id = st.next.drop();
    let born = st.tick;
    st.zone_mut(zone).expect("her zone").drops.push(jane_sim::state::Drop { id, item, qty: 1, pos: at, born });
    // Glowing parts over the drop, and glowing parts anywhere.
    let glints = |p: &mut Present| {
        let f = p.draw(128, CANVAS);
        let (x, y) = ((at.x.0 >> FX_TO_CANVAS) - f.camera.0, (at.y.0 >> FX_TO_CANVAS) - f.camera.1);
        let over = |q: &&jane_present::frame::Particle| {
            q.glow > 0 && (i32::from(q.x) - x).abs() <= 8 && (-24..=4).contains(&(i32::from(q.y) - y))
        };
        (f.parts.iter().filter(over).count(), f.parts.iter().filter(|q| q.glow > 0).count())
    };
    // Over two of its beats it has glinted, and nowhere else.
    let mut seen = 0;
    for _ in 0..140 {
        run(&mut sim, &mut p, 1);
        let (over, all) = glints(&mut p);
        assert_eq!(over, all, "nothing glints but the drop");
        seen += over;
    }
    assert!(seen > 0, "the drop glinted");
    // Taken: its last glint dies out and none come.
    sim.state_mut().zone_mut(zone).expect("her zone").drops.clear();
    run(&mut sim, &mut p, 60);
    for _ in 0..140 {
        run(&mut sim, &mut p, 1);
        assert_eq!(glints(&mut p).1, 0, "a drop taken glints no more");
    }
}
