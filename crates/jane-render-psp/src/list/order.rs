//! Draws grouped by texture (PLAN-PSP-SPEED.md Q6a): inside a pass whose quads only blend each
//! px with what is under it, two quads whose px do not overlap may be drawn in either order and
//! the frame is the same. The sprites are laid by depth, so their pages interleave (a lamp, a
//! cobble, a sign, another lamp) and each change is a bind and a CLUT load on the GE. Here a quad
//! joins the last run of its own texture and mode before it when no quad between them touches
//! its px; every pair of quads that do touch keeps its order. Pure and integer, so a PC test
//! holds it to that.

use alloc::vec::Vec;

use super::{Mode, Quad, Tex};
#[cfg(test)]
use crate::list::guard::same_batch;

/// Runs looked back over for a quad's own: the cost is a quad times this, at most.
const WINDOW: usize = 8;

/// The px quad `q` may touch, `(x0, y0, x1, y1)` half-open, when it is a quad that blends each
/// px alone (no stencil, no target, no read-back) and so may move past what it does not touch;
/// `None` for one that stays where it is (and nothing moves past it).
pub fn reach(q: &Quad) -> Option<(i16, i16, i16, i16)> {
    let blends =
        matches!(q.mode, Mode::Alpha | Mode::Add | Mode::Multiply | Mode::Multiply2 | Mode::AddGlow | Mode::Halo);
    if !blends {
        return None;
    }
    match q.tex {
        // A rect of px (a sprite's, a patch's, its glow or relief, a flat quad, a stretched disc).
        Tex::Page(_) | Tex::Patch(_) | Tex::Normal(..) | Tex::Glow(_) | Tex::None | Tex::Spot | Tex::Disc => {
            (q.x0 < q.x1 && q.y0 < q.y1).then_some((q.x0, q.y0, q.x1, q.y1))
        }
        // A line from `(x0, y0)` to `(x1, y1)`, both ends drawn.
        Tex::Line(_) => Some((q.x0.min(q.x1), q.y0.min(q.y1), q.x0.max(q.x1) + 1, q.y0.max(q.y1) + 1)),
        _ => None,
    }
}

fn overlaps(a: (i16, i16, i16, i16), b: (i16, i16, i16, i16)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

fn union(a: (i16, i16, i16, i16), b: (i16, i16, i16, i16)) -> (i16, i16, i16, i16) {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

/// Quad `q`'s texture and mode as one word, for a quad that may move ([`reach`]): equal words are
/// [`crate::list::guard::same_batch`]. 0 for one that stays put.
fn key(q: &Quad) -> u32 {
    let mode = match q.mode {
        Mode::Alpha => 1,
        Mode::Add => 2,
        Mode::Multiply => 3,
        Mode::Multiply2 => 4,
        Mode::AddGlow => 5,
        Mode::Halo => 6,
        _ => return 0,
    };
    let (kind, v) = match q.tex {
        Tex::Page(p) => (1, u32::from(p)),
        Tex::Patch(p) => (2, u32::from(p)),
        // A lamp's relief CLUT is under `LAMP_RELIEFS`, the sun's `u16::MAX`: one byte names it.
        Tex::Normal(p, c) => (3, u32::from(p) << 8 | u32::from(c.min(0xff))),
        Tex::Glow(p) => (4, u32::from(p)),
        Tex::None => (5, 0),
        Tex::Spot => (6, 0),
        Tex::Disc => (7, 0),
        Tex::Line(fade) => (8, u32::from(fade)),
        _ => return 0,
    };
    kind << 28 | mode << 24 | v
}

/// Bounds no quad passes.
const ALL: (i16, i16, i16, i16) = (i16::MIN, i16::MIN, i16::MAX, i16::MAX);

/// A run being gathered: its texture and mode ([`key`], 0 for what stays put), the bounds of
/// every quad in it, and its quads as a chain through [`Order::next`].
#[derive(Clone, Copy, Debug)]
struct Run {
    key: u32,
    bounds: (i16, i16, i16, i16),
    head: u32,
    tail: u32,
}

/// The scratch of [`Order::by_texture`], kept between frames.
#[derive(Debug, Default)]
pub struct Order {
    runs: Vec<Run>,
    next: Vec<u32>,
    out: Vec<Quad>,
}

const END: u32 = u32::MAX;

impl Order {
    /// `quads` (one pass's) reordered so that each quad joins the latest run of its texture and
    /// mode it can reach without passing a quad whose px it touches (or one that stays put), at
    /// most [`WINDOW`] runs back. Returns the runs: the batches the GE draws them in at least.
    pub fn by_texture(&mut self, quads: &mut [Quad]) -> usize {
        self.runs.clear();
        self.next.clear();
        self.next.resize(quads.len(), END);
        let mut moved = false;
        for (i, q) in quads.iter().enumerate() {
            let (key, r) = match (key(q), reach(q)) {
                (k, Some(r)) if k != 0 => (k, r),
                _ => (0, ALL),
            };
            let mut join = None;
            if key != 0 {
                let lo = self.runs.len().saturating_sub(WINDOW);
                for k in (lo..self.runs.len()).rev() {
                    let run = &self.runs[k];
                    if run.key == key {
                        join = Some(k);
                        break;
                    }
                    if overlaps(run.bounds, r) {
                        break;
                    }
                }
            }
            let i = i as u32;
            match join {
                Some(k) => {
                    moved |= k + 1 != self.runs.len();
                    let run = &mut self.runs[k];
                    self.next[run.tail as usize] = i;
                    run.tail = i;
                    run.bounds = union(run.bounds, r);
                }
                // A run of its own, last. Two runs of one texture side by side (the one between
                // was a barrier) still draw as one batch.
                None => self.runs.push(Run { key, bounds: r, head: i, tail: i }),
            }
        }
        // Every quad joined the run before it or began one: the order is as it was.
        if !moved {
            return self.runs.len();
        }
        self.out.clear();
        for run in &self.runs {
            let mut k = run.head;
            while k != END {
                self.out.push(quads[k as usize]);
                k = self.next[k as usize];
            }
        }
        quads.copy_from_slice(&self.out);
        self.runs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(tex: Tex, mode: Mode, (x0, y0, x1, y1): (i16, i16, i16, i16), colour: u32) -> Quad {
        Quad { tex, mode, colour, x0, y0, x1, y1, u0: 0, v0: 0, u1: 0, v1: 0 }
    }

    /// Every pair of quads whose px touch (or either of which stays put) is in the same order in
    /// `b` as in `a`; `b` is a permutation of `a` (quads told apart by their colour).
    fn same_frame(a: &[Quad], b: &[Quad]) {
        assert_eq!(a.len(), b.len());
        let pos = |c: u32| b.iter().position(|q| q.colour == c).expect("every quad kept");
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                let touch = match (reach(&a[i]), reach(&a[j])) {
                    (Some(x), Some(y)) => overlaps(x, y),
                    _ => true,
                };
                if touch {
                    assert!(pos(a[i].colour) < pos(a[j].colour), "quads {i} and {j} swapped");
                }
            }
        }
    }

    fn batches(qs: &[Quad]) -> usize {
        1 + qs.windows(2).filter(|w| !same_batch(&w[0], &w[1])).count()
    }

    #[test]
    fn sprites_apart_join_their_pages_and_sprites_over_each_other_keep_their_order() {
        // Lamps on page 1 along a street, signs on page 2 between them, a cart (page 1) over a
        // sign: the cart may not pass the sign under it.
        let a = [
            q(Tex::Page(1), Mode::Alpha, (0, 0, 10, 20), 1),
            q(Tex::Page(2), Mode::Alpha, (20, 0, 30, 10), 2),
            q(Tex::Page(1), Mode::Alpha, (40, 0, 50, 20), 3),
            q(Tex::Page(2), Mode::Alpha, (60, 0, 70, 10), 4),
            q(Tex::Page(1), Mode::Alpha, (62, 5, 80, 15), 5),
            q(Tex::Page(1), Mode::Add, (90, 0, 99, 9), 6),
        ];
        let mut b = a;
        let runs = Order::default().by_texture(&mut b);
        same_frame(&a, &b);
        assert_eq!(runs, 4);
        assert_eq!(batches(&b), 4, "{b:?}");
        assert_eq!(batches(&a), 6);
    }

    #[test]
    fn a_key_is_the_batch() {
        let texs = [
            Tex::Page(0),
            Tex::Page(1),
            Tex::Page(256),
            Tex::Patch(1),
            Tex::Normal(1, u16::MAX),
            Tex::Normal(1, 0),
            Tex::Normal(1, 95),
            Tex::Normal(2, 0),
            Tex::Glow(1),
            Tex::None,
            Tex::Spot,
            Tex::Disc,
            Tex::Line(true),
            Tex::Line(false),
        ];
        let modes = [Mode::Alpha, Mode::Add, Mode::Multiply, Mode::Multiply2, Mode::AddGlow, Mode::Halo];
        let all: Vec<Quad> = texs.iter().flat_map(|&t| modes.iter().map(move |&m| q(t, m, (0, 0, 1, 1), 0))).collect();
        for a in &all {
            assert_ne!(key(a), 0);
            for b in &all {
                assert_eq!(key(a) == key(b), same_batch(a, b), "{a:?} {b:?}");
            }
        }
        assert_eq!(key(&q(Tex::Page(1), Mode::Lut(0), (0, 0, 1, 1), 0)), 0);
        assert_eq!(key(&q(Tex::Chunk(1), Mode::Alpha, (0, 0, 1, 1), 0)), 0);
    }

    #[test]
    fn what_stays_put_is_passed_by_nothing() {
        let a = [
            q(Tex::Page(1), Mode::Alpha, (0, 0, 10, 10), 1),
            q(Tex::Frame(0, 0), Mode::Lut(0), (100, 100, 110, 110), 2),
            q(Tex::Page(1), Mode::Alpha, (50, 50, 60, 60), 3),
            q(Tex::Page(3), Mode::StencilMark, (200, 0, 210, 10), 4),
            q(Tex::Page(1), Mode::Alpha, (70, 70, 80, 80), 5),
        ];
        let mut b = a;
        Order::default().by_texture(&mut b);
        assert_eq!(b, a);
    }

    #[test]
    fn a_line_reaches_both_its_ends() {
        let l = q(Tex::Line(true), Mode::Alpha, (10, 20, 4, 8), 1);
        assert_eq!(reach(&l), Some((4, 8, 11, 21)));
        // A spot on the line's last px may not pass it.
        let a = [q(Tex::Spot, Mode::Alpha, (0, 0, 2, 2), 1), l, q(Tex::Spot, Mode::Alpha, (10, 20, 12, 22), 2)];
        let mut b = a;
        Order::default().by_texture(&mut b);
        same_frame(&a, &b);
        assert_eq!(b, a);
    }

    /// A crowd of a few hundred sprites over a few pages, laid by depth: the order keeps every
    /// pair that touches, and the batches fall.
    #[test]
    fn a_crowd_keeps_every_pair_that_touches_and_draws_in_fewer_batches() {
        let mut s = 0x1234_5678u32;
        let mut rnd = |n: u32| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s % n
        };
        let mut a = Vec::new();
        for k in 0..300u32 {
            let (x, y) = (rnd(470) as i16, (k * 270 / 300) as i16);
            let (w, h) = (2 + rnd(30) as i16, 2 + rnd(40) as i16);
            let tex = match rnd(9) {
                0 => Tex::Patch(k as u16),
                1 => Tex::Line(true),
                2 => Tex::Spot,
                3 => Tex::Frame(0, 0),
                p => Tex::Page(p as u16),
            };
            let mode = if rnd(10) == 0 { Mode::Add } else { Mode::Alpha };
            let mode = if tex == Tex::Frame(0, 0) { Mode::Lut(0) } else { mode };
            a.push(q(tex, mode, (x, y, x + w, y + h), k + 1));
        }
        let mut b = a.clone();
        let mut o = Order::default();
        o.by_texture(&mut b);
        same_frame(&a, &b);
        assert!(batches(&b) * 100 < batches(&a) * 85, "{} -> {}", batches(&a), batches(&b));
        // Again over its own output: nothing more to gain, nothing broken.
        let mut c = b.clone();
        o.by_texture(&mut c);
        same_frame(&b, &c);
    }
}
