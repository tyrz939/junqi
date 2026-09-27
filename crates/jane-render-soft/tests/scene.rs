//! A whole frame through the presenter and `soft`, from a real New Game (PRESENTATION.md §1.3's
//! tier test, §1.10's camera, §1.11's purity).

use jane_core::Angle;
use jane_present::{Backend, Depth, Pass, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

/// Steps the sim once with seat 0 on `input` (and `cmd`, if any) and ticks the presenter.
fn step(sim: &mut Sim, p: &mut Present, input: InputFrame, cmd: Option<Command>) {
    let mut frames = [InputFrame::IDLE; 4];
    frames[0] = input;
    let cmds: Vec<StampedCommand> =
        cmd.into_iter().map(|cmd| StampedCommand { seat: Some(Seat(0)), seq: 1, cmd }).collect();
    sim.step(&StepInput { frames, commands: &cmds });
    let events = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    p.tick(&v, &events);
}

fn at_hour(seed: u32, hour: u8, tier: Tier) -> (Sim, Present) {
    let mut sim = Sim::new_game(seed, "Jane");
    let mut p = Present::new(tier);
    step(&mut sim, &mut p, InputFrame::IDLE, Some(Command::Dev(DevOp::Time { hour })));
    for _ in 0..30 {
        step(&mut sim, &mut p, InputFrame::IDLE, None);
    }
    (sim, p)
}

#[test]
fn a_frame_holds_no_pass_above_its_tier_and_soft_draws_every_pass() {
    let (sim, _) = at_hour(1, 22, Tier::T0);
    for tier in [Tier::T0, Tier::T1, Tier::T2] {
        let mut p = Present::new(tier);
        let v = sim.view(Seat(0)).unwrap();
        for _ in 0..3 {
            p.tick(&v, &[]);
        }
        let mut soft = Soft::new();
        soft.upload_atlas(p.atlas());
        let frame = p.draw(200, CANVAS);
        assert_eq!(frame.tier, tier);
        for pass in &frame.passes {
            assert!(pass.needs() <= tier, "{pass:?} needs {:?} in a {tier:?} frame", pass.needs());
        }
        soft.draw(frame);
        let (px, w, h) = soft.pixels();
        assert_eq!((w, h), CANVAS);
        assert_eq!(px.len(), 768 * 432);
    }
}

#[test]
fn a_town_night_has_every_pass_and_noon_needs_no_light() {
    let (_, mut night) = at_hour(1, 22, Tier::T0);
    let f = night.draw(255, CANVAS);
    let kinds: Vec<&str> = f
        .passes
        .iter()
        .map(|p| match p {
            Pass::Terrain { .. } => "terrain",
            Pass::Sprites { layer: Depth::Ground, .. } => "ground",
            Pass::Sprites { layer: Depth::Standing, .. } => "standing",
            Pass::Sprites { .. } => "other sprites",
            Pass::Silhouettes { .. } => "silhouettes",
            Pass::Lights { .. } => "lights",
            Pass::Post(_) => "post",
            Pass::Sky(_) => "sky",
            Pass::Parallax { .. } => "far",
            Pass::Water { .. } => "water",
            Pass::Weather(_) => "weather",
            Pass::Fog { .. } => "fog",
            Pass::Rays { .. } => "rays",
            Pass::Particles { .. } => "particles",
        })
        .collect();
    // Out of doors the sky and its far things come first; the moon is up at 22:00, so its
    // silhouettes lie under the standing things; the weather, clear on the first walk, before
    // the light; the grade last (every tier draws it, 2026-09-27).
    assert_eq!(
        kinds,
        ["sky", "far", "far", "terrain", "ground", "silhouettes", "standing", "weather", "lights", "post"]
    );
    assert!(!f.chunks.is_empty() && !f.sprites.is_empty());
    let (_, mut noon) = at_hour(1, 12, Tier::T0);
    assert!(!noon.draw(255, CANVAS).passes.iter().any(|p| matches!(p, Pass::Lights { .. })));
}

#[test]
fn night_is_darker_than_noon() {
    let mean = |hour| {
        let (_, mut p) = at_hour(1, hour, Tier::T0);
        let mut soft = Soft::new();
        soft.upload_atlas(p.atlas());
        soft.draw(p.draw(255, CANVAS));
        let (px, _, _) = soft.pixels();
        px.iter().map(|&c| u64::from((c >> 16) & 0xff) + u64::from((c >> 8) & 0xff) + u64::from(c & 0xff)).sum::<u64>()
            / px.len() as u64
    };
    let (noon, night) = (mean(12), mean(23));
    // T2 is the reference look, and its night keeps its value (ART.md §3.1, "night is
    // beautiful, not dark"): about two thirds of noon's mean, lamps and all.
    assert!(night * 4 < noon * 3, "noon {noon}, night {night}");
}

#[test]
fn the_camera_frames_her_and_drawing_is_pure_in_tick_and_alpha() {
    let (mut sim, mut p) = at_hour(1, 12, Tier::T0);
    // She walks east for a second: the camera trails her, then settles on her when she stops.
    for _ in 0..60 {
        step(&mut sim, &mut p, InputFrame::walk(Angle::EAST), None);
    }
    let draw = |p: &mut Present, alpha: u8| {
        let mut soft = Soft::new();
        soft.upload_atlas(p.atlas());
        soft.draw(p.draw(alpha, CANVAS));
        soft.pixels().0.to_vec()
    };
    // Mid-walk, a frame between ticks is neither tick, and the same alpha twice is the same frame.
    let (a0, a1, again) = (draw(&mut p, 0), draw(&mut p, 255), draw(&mut p, 0));
    assert_ne!(a0, a1);
    assert_eq!(a0, again);
    for _ in 0..240 {
        step(&mut sim, &mut p, InputFrame::IDLE, None);
    }
    let v = sim.view(Seat(0)).unwrap();
    let her = (v.body().pos.x.0 >> 7, v.body().pos.y.0 >> 7);
    let zone = (v.size().0 as i32 * 16, v.size().1 as i32 * 16);
    let (cx, cy) = p.camera().at(255);
    // Her middle (feet less 20 canvas px) at the canvas centre, a px either way, unless that
    // would show past the zone's edge.
    let want = ((her.0 - 384).clamp(0, zone.0 - 768), (her.1 - 20 - 216).clamp(0, zone.1 - 432));
    assert!((cx - want.0).abs() <= 1 && (cy - want.1).abs() <= 1, "camera {:?}, want {want:?}", (cx, cy));
    // Somewhere she stands away from the edges, so the centring is what was tested.
    assert!(her.0 - 384 > 0 || her.1 - 236 > 0, "her {her:?}");
}
