//! The tier gate (PRESENTATION.md §1.3, §1.12): the same `Frame` on T0 and T1 differs only in
//! the `Features` rows, so T1's albedo pass, everything before the light, is `soft`'s frame
//! before its lightmap, pixel for pixel. One test in this file: SDL lives once in a process.

use jane_present::{Backend, Pass, Present, Tier};
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
    for (seed, hour) in [(1, 22), (1, 17), (2, 12), (3, 19)] {
        let mut p = at_hour(seed, hour);
        gl.upload_atlas(p.atlas());
        soft.upload_atlas(p.atlas());
        let frame = p.draw(200, CANVAS);
        assert_eq!(frame.tier, Tier::T1);
        assert!(frame.passes.iter().any(|q| matches!(q, Pass::Lights { .. })));
        silhouettes |= frame.passes.iter().any(|q| matches!(q, Pass::Silhouettes { .. }));
        for exact in [true, false] {
            gl.set_rows(Rows { exact, ..Rows::T1 });
            gl.draw(p.frame());
            let mut albedo = Vec::new();
            let (w, h) = gl.read_albedo(&mut albedo);
            assert_eq!((w, h), CANVAS);
            // soft draws the same frame without its light pass: its albedo pass alone. The
            // atmosphere's passes go too, since gl2 does not draw them yet (PRESENTATION.md §1.3).
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
                )
            };
            let lights: Vec<Pass> = f.passes.iter().copied().filter(|q| off(q)).collect();
            f.passes.retain(|q| !off(q));
            soft.draw(p.frame());
            p.frame_mut().passes.extend(lights);
            let (px, sw, sh) = soft.pixels();
            assert_eq!((sw, sh), CANVAS);
            let (n, most, first) = diff(px, &albedo, usize::from(w));
            eprintln!("seed {seed} {hour:02}:00 exact {exact}: {n} px differ, by at most {most}; first {first:x?}");
            if exact {
                assert_eq!(n, 0, "seed {seed} at {hour}: the exact albedo is soft's; first {first:x?}");
            } else {
                // The fast mode blends where soft floors, and lays the contact shadows under the
                // pass: never more than a few levels, and on few px.
                assert!(n < px.len() / 20 && most <= 16, "seed {seed} at {hour}: {n} px differ in the fast mode");
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
}
