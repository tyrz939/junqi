//! No allocation after the second frame (PRESENTATION.md §1.1, §1.12), in tree as far as the
//! lints let it be: a `#[global_allocator]` is an `unsafe impl`, which `unsafe_code = "forbid"`
//! refuses in every workspace crate and its tests (ARCHITECTURE.md §9 has the same note for the
//! sim), so the counting allocator runs out of tree until the one-exception crate of PORT.md §3.4
//! lands. Here, the proxy: every buffer the frame and the framebuffer are built in keeps its
//! address and capacity from the third frame on, so none of them grew or was replaced.

use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::{InputFrame, Seat, Sim, StepInput};

/// `(address, capacity)` of each of the frame's lists and of the framebuffer.
fn marks(p: &mut Present, soft: &mut Soft, alpha: u8) -> Vec<(usize, usize)> {
    let f = p.draw(alpha, (768, 432));
    let mut m = vec![
        (f.passes.as_ptr() as usize, f.passes.capacity()),
        (f.chunks.as_ptr() as usize, f.chunks.capacity()),
        (f.sprites.as_ptr() as usize, f.sprites.capacity()),
        (f.layers.as_ptr() as usize, f.layers.capacity()),
        (f.lights.as_ptr() as usize, f.lights.capacity()),
        (f.casters.as_ptr() as usize, f.casters.capacity()),
        (f.blocks.as_ptr() as usize, f.blocks.capacity()),
    ];
    m.extend(f.layers.iter().map(|l| (l.albedo.as_ptr() as usize, l.albedo.capacity())));
    soft.draw(f);
    m.push((soft.pixels().0.as_ptr() as usize, soft.pixels().0.len()));
    m
}

#[test]
fn the_frame_buffers_hold_still_after_the_second_frame() {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    let mut soft = Soft::new();
    soft.upload_atlas(p.atlas());
    let input = StepInput { frames: [InputFrame::IDLE; 4], commands: &[] };
    let mut first = None;
    for frame in 0..240u32 {
        sim.step(&input);
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
        let m = marks(&mut p, &mut soft, (frame * 37 % 256) as u8);
        match &first {
            None if frame >= 2 => first = Some(m),
            Some(f) => assert_eq!(f, &m, "a buffer moved or grew at frame {frame}"),
            None => {}
        }
    }
}

/// Ticks in the square before the clock is set a second short of nine.
const WARM: u32 = 240;

/// The night's turn (NIGHT.md §2.2, §9 R2): the gutter, the held dark and the band of light
/// through the lightmap's rows allocate nothing either, from a second before nine to five after.
#[test]
fn the_turn_holds_the_frame_buffers_still() {
    use jane_sim::input::DevOp;
    use jane_sim::{Command, StampedCommand};
    let mut sim = Sim::new_game(1, "Jane");
    let mark = sim.view(Seat(0)).and_then(|v| v.sym("town_square")).expect("the square");
    let tp = [StampedCommand {
        seat: Some(Seat(0)),
        seq: 1,
        cmd: Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::County, mark }),
    }];
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &tp });
    let mut p = Present::new(Tier::T0);
    let mut soft = Soft::new();
    soft.upload_atlas(p.atlas());
    let input = StepInput { frames: [InputFrame::IDLE; 4], commands: &[] };
    let mut first = None;
    let mut turned = false;
    // Four seconds in the square first (her arrival painted), then the clock a second short of
    // nine: from there to five seconds after, nothing moves or grows.
    for frame in 0..WARM + 360 {
        if frame == WARM {
            sim.state_mut().clock = 21 * jane_sim::tuning::TICKS_PER_HOUR - 60;
        }
        sim.step(&input);
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
        turned |= !p.turn_now().is_none();
        let m = marks(&mut p, &mut soft, (frame * 37 % 256) as u8);
        match &first {
            None if frame >= WARM => first = Some(m),
            Some(f) => assert_eq!(f, &m, "a buffer moved or grew at frame {frame}"),
            None => {}
        }
    }
    assert!(turned, "the turn ran");
}
