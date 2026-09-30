//! The tier gate (PRESENTATION.md §1.3, §1.12): the same `Frame` on T0 and T1 differs only in
//! the `Features` rows, so T1's albedo pass, everything before the light, is `soft`'s frame
//! before its lightmap, pixel for pixel. One test in this file: SDL lives once in a process.
//! Four scenes drawn twice on two APIs and read back are half a minute of a dev build, so it is
//! the slow tier (VERIFICATION.md §6): `cargo test --release -p jane-render-gl2 -- --ignored`.

use jane_core::Angle;
use jane_present::{Backend, Pass, Present, Tier, WeatherKind};
use jane_render_gl2::{Api, Gl2, Rows};
use jane_render_soft::Soft;
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

/// A new game at `hour`, the presenter at T1 ticked beside it.
fn at_hour(seed: u32, hour: u8) -> Present {
    let mut sim = Sim::new_game(seed, "Jane");
    let mut p = Present::new(Tier::T1);
    p.set_canvas(CANVAS);
    let cmd = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour }) }];
    for k in 0..40 {
        let cmds: &[StampedCommand] = if k == 0 { &cmd } else { &[] };
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
    }
    p
}

/// She walks round Julie's house and in under its eaves at 11:00 (PRESENTATION.md §1.6): its
/// roof stands in front of her, and she is seen through it on the canvas's checker.
fn behind_house() -> Present {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T1);
    p.set_canvas(CANVAS);
    let mark = sim.state().syms.find("house_front").expect("Julie's house has a front");
    let seat = Some(Seat(0));
    let cmds = [
        StampedCommand { seat, seq: 1, cmd: Command::Dev(DevOp::God(true)) },
        StampedCommand { seat, seq: 2, cmd: Command::Dev(DevOp::Time { hour: 11 }) },
        StampedCommand { seat, seq: 3, cmd: Command::Dev(DevOp::Tp { zone: jane_core::ids::ZoneId::County, mark }) },
    ];
    let legs = [(Angle::WEST, 90), (Angle::NORTH, 140), (Angle::EAST, 75), (Angle::SOUTH, 60)];
    let steps = std::iter::repeat_n(InputFrame::IDLE, 61)
        .chain(legs.iter().flat_map(|&(dir, n)| std::iter::repeat_n(InputFrame::walk(dir), n)));
    for (k, frame) in steps.enumerate() {
        let cmds: &[StampedCommand] = if k == 0 { &cmds } else { &[] };
        let idle = InputFrame::IDLE;
        sim.step(&StepInput { frames: [frame, idle, idle, idle], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
    }
    p
}

/// The first differing px: where, and the two colours.
type First = Option<(usize, usize, u32, u32)>;

/// `(pixels that differ, the largest channel difference, the first differing px)`.
fn diff(a: &[u32], b: &[u32], w: usize) -> (usize, u32, First) {
    let mut n = 0;
    let mut most = 0;
    let mut first = None;
    for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
        if x != y {
            n += 1;
            let d = (0..3).map(|k| ((x >> (k * 8)) & 0xff).abs_diff((y >> (k * 8)) & 0xff)).max().unwrap_or(0);
            most = most.max(d);
            first.get_or_insert((i % w, i / w, x, y));
        }
    }
    (n, most, first)
}

#[test]
#[ignore = "slow: four scenes on two GL APIs against soft, about half a minute in a dev build"]
fn the_albedo_pass_is_softs_pixel_for_pixel() {
    // GLSL 1.20 on desktop GL, then GLSL ES 1.00 where the driver makes a GLES context.
    for api in [Api::Desktop, Api::Es] {
        match Gl2::headless(api) {
            Ok(gl) => both_modes_match(gl),
            Err(e) => eprintln!("no {api:?} context: skipped ({e})"),
        }
    }
}

fn both_modes_match(mut gl: Gl2) {
    eprintln!("{}", gl.describe());
    let mut soft = Soft::new();
    let mut silhouettes = false;
    let scenes =
        [(1, 22), (1, 17), (2, 12), (3, 19)].map(|(seed, hour)| (format!("seed {seed} {hour:02}:00"), seed, hour));
    for (label, seed, hour) in scenes.into_iter().chain([("behind Julie's house".to_string(), 1, 11)]) {
        let mut p = if label.starts_with("behind") { behind_house() } else { at_hour(seed, hour) };
        gl.upload_atlas(p.atlas());
        soft.upload_atlas(p.atlas());
        let frame = p.draw(200, CANVAS);
        assert_eq!(frame.tier, Tier::T1);
        assert!(frame.passes.iter().any(|q| matches!(q, Pass::Lights { .. })));
        silhouettes |= frame.passes.iter().any(|q| matches!(q, Pass::Silhouettes { .. }));
        if label.starts_with("behind") {
            assert!(frame.sprites.iter().any(|s| s.foot.is_some_and(|f| f.see)), "{label}: she stands behind the roof");
        }
        for exact in [true, false] {
            gl.set_rows(Rows { exact, ..Rows::T1 });
            gl.draw(p.frame());
            let mut albedo = Vec::new();
            let (w, h) = gl.read_albedo(&mut albedo);
            assert_eq!((w, h), CANVAS);
            // soft draws the same frame without its light pass: its albedo pass alone. The
            // atmosphere's passes go too: gl2 draws them after its albedo, over the lit canvas
            // (the sky and the water in its compose, the particles and the fog over it), and the
            // grade, which both draw last.
            let f = p.frame_mut();
            let off = |q: &Pass| {
                matches!(
                    q,
                    Pass::Lights { .. }
                        | Pass::Sky(_)
                        | Pass::Parallax { .. }
                        | Pass::Water { .. }
                        | Pass::Weather(_)
                        | Pass::Fog { .. }
                        | Pass::Rays { .. }
                        | Pass::Particles { .. }
                        | Pass::Post(_)
                )
            };
            let lights: Vec<Pass> = f.passes.iter().copied().filter(|q| off(q)).collect();
            f.passes.retain(|q| !off(q));
            soft.draw(p.frame());
            p.frame_mut().passes.extend(lights);
            let (px, sw, sh) = soft.pixels();
            assert_eq!((sw, sh), CANVAS);
            let (n, most, first) = diff(px, &albedo, usize::from(w));
            eprintln!("{label} exact {exact}: {n} px differ, by at most {most}; first {first:x?}");
            if exact {
                assert_eq!(n, 0, "{label}: the exact albedo is soft's; first {first:x?}");
            } else {
                // The fast mode blends where soft floors, and lays the contact shadows under the
                // pass: never more than a few levels, and on few px.
                assert!(n < px.len() / 20 && most <= 16, "{label}: {n} px differ in the fast mode");
            }
        }
        // The whole frame is lit: the light changes the albedo, and the canvas reads back.
        let mut out = Vec::new();
        assert_eq!(gl.read_back(&mut out), CANVAS);
        assert!(out.iter().any(|&c| c != out[0]));
        let s = gl.stats().expect("gl2 measures its frames");
        assert!(s.frames > 0 || s.draw_calls > 0);
    }
    assert!(silhouettes, "some frame had the sun's silhouettes");
    the_atmosphere_is_drawn(&mut gl);
    more_quads_than_the_indices_name(&mut gl);
}

/// The indices are 16-bit and name 16383 quads; a list longer than that (F2 and F3 up at dusk
/// by Julie's step, the shadow spans and the UI both past it) is drawn whole, not a panic. The
/// owner's first crash (September 2026).
fn more_quads_than_the_indices_name(gl: &mut Gl2) {
    use jane_present::ui::{Rect, UiCmd};
    gl.set_rows(Rows::T1);
    let mut p = at_hour(1, 12);
    gl.upload_atlas(p.atlas());
    p.draw(200, CANVAS);
    let colour = |i: i32| 0xff00_0000 | (i as u32).wrapping_mul(2_654_435_761) & 0x00ff_ffff;
    let w = i32::from(CANVAS.0);
    let n = 40_000;
    p.frame_mut().ui.clear();
    for i in 0..n {
        p.frame_mut().ui.push(UiCmd::Fill { dst: Rect::new(i % w, i / w, 1, 1), argb: colour(i) });
    }
    gl.draw(p.frame());
    let mut out = Vec::new();
    gl.read_back(&mut out);
    for i in (0..n).step_by(97) {
        let got = out[i as usize] & 0x00ff_ffff;
        assert_eq!(got, colour(i) & 0x00ff_ffff, "fill {i} of {n} is drawn");
    }
}

/// The atmosphere's passes on T1 (PRESENTATION.md §1.3): rain and mist at night draw, and leave
/// the albedo pass as it was.
fn the_atmosphere_is_drawn(gl: &mut Gl2) {
    gl.set_rows(Rows::T1);
    for kind in [WeatherKind::Rain, WeatherKind::Mist] {
        let mut sim = Sim::new_game(1, "Jane");
        let mut p = Present::new(Tier::T1);
        p.set_canvas(CANVAS);
        p.atmos_mut().force(Some((kind, if kind == WeatherKind::Rain { 255 } else { 0 })));
        let cmd = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour: 22 }) }];
        for k in 0..240 {
            let cmds: &[StampedCommand] = if k == 0 { &cmd } else { &[] };
            sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
            let events = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0 plays");
            p.tick(&v, &events);
        }
        gl.upload_atlas(p.atlas());
        let frame = p.draw(200, CANVAS);
        let want = |q: &Pass| match kind {
            WeatherKind::Rain => matches!(q, Pass::Particles { .. }),
            _ => matches!(q, Pass::Fog { .. }),
        };
        assert!(frame.passes.iter().any(want), "{kind:?} at night puts its pass in the frame");
        gl.draw(p.frame());
        let (mut with, mut albedo) = (Vec::new(), Vec::new());
        gl.read_back(&mut with);
        gl.read_albedo(&mut albedo);
        let f = p.frame_mut();
        let atmos: Vec<Pass> = f.passes.iter().copied().filter(is_atmosphere).collect();
        f.passes.retain(|q| !is_atmosphere(q));
        gl.draw(p.frame());
        p.frame_mut().passes.extend(atmos);
        let (mut without, mut albedo2) = (Vec::new(), Vec::new());
        gl.read_back(&mut without);
        gl.read_albedo(&mut albedo2);
        let (n, most, _) = diff(&with, &without, usize::from(CANVAS.0));
        eprintln!("{kind:?} at 22:00: the atmosphere changes {n} px, by at most {most}");
        assert!(n > 2000, "{kind:?}: gl2 draws its atmosphere ({n} px changed)");
        assert_eq!(albedo, albedo2, "{kind:?}: the atmosphere never touches the albedo pass");
    }
}

fn is_atmosphere(q: &Pass) -> bool {
    matches!(
        q,
        Pass::Sky(_)
            | Pass::Parallax { .. }
            | Pass::Water { .. }
            | Pass::Weather(_)
            | Pass::Fog { .. }
            | Pass::Particles { .. }
    )
}
