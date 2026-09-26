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
