//! The solver's portals across height (MAP.md §4.7; R2): the flood stays the fast word-parallel
//! fill (`jane_core::search::fill`), and the ways it cannot take as steps are portals between
//! fills, as gates are:
//!
//! 1. Fill from the starts with ledge cells solid and decks absent (a span's end rows are solid
//!    to the ground under it; the ground under a deck is ground).
//! 2. For every ledge whose top is reached and whose landing is open, seed the landing; for every
//!    whole span with a cell of one end reached, seed its other end and mark its deck. Fill again,
//!    until nothing more is seeded.
//! 3. **No pits:** the same fill backwards (a ledge's landing reaches its top), from the entrance:
//!    every cell the forward fill reaches must be in it, so wherever she can stand she can walk
//!    back ([`no_pit`]).
//!
//! What is open is the caller's (`open(x, y)`: the tiles, or a night stage's, or a layer's with
//! its props); the portals are the blueprint's ledges and spans.

use alloc::vec::Vec;

use jane_core::Blueprint;
use jane_core::search::{Fill, fill};
use jane_core::tile::F_SOLID;

/// The most cells a ledge's face is deep (the sim's `tuning::LEDGE_FACE_MAX`).
const LEDGE_FACE_MAX: i32 = 6;

/// What a fill across the portals reached: the ground, and which spans' decks (a bit each).
#[derive(Debug)]
pub struct Reach {
    pub ground: Fill,
    pub decks: u64,
}

impl Reach {
    pub fn reached(&self, x: i32, y: i32) -> bool {
        self.ground.reached(x, y)
    }
}

/// A ledge's one-way portal: from its top cell to its landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ledge {
    pub top: (i32, i32),
    pub landing: (i32, i32),
}

/// Every ledge of `bp` with ground on both sides: the cell before its face, the first cell past it.
pub fn ledges(bp: &Blueprint, open: &impl Fn(i32, i32) -> bool) -> Vec<Ledge> {
    let mut out = Vec::new();
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && (x as u32) < bp.w() && (y as u32) < bp.h();
    for y in 0..bp.h() as i32 {
        for x in 0..bp.w() as i32 {
            let t = bp.tile(x, y);
            let Some((dx, dy)) = t.ledge_dir() else { continue };
            let top = (x - dx, y - dy);
            if !inside(top.0, top.1) || bp.tile(top.0, top.1) == t || !open(top.0, top.1) {
                continue;
            }
            let (mut cx, mut cy, mut faces) = (x, y, 0);
            while inside(cx, cy) && bp.tile(cx, cy) == t && faces < LEDGE_FACE_MAX {
                cx += dx;
                cy += dy;
                faces += 1;
            }
            if inside(cx, cy) && open(cx, cy) {
                out.push(Ledge { top, landing: (cx, cy) });
            }
        }
    }
    out
}

/// The tiles' own ground: open where the tile is not solid.
pub fn tiles_open(bp: &Blueprint) -> impl Fn(i32, i32) -> bool + '_ {
    move |x, y| x >= 0 && y >= 0 && (x as u32) < bp.w() && (y as u32) < bp.h() && bp.tile(x, y).flags() & F_SOLID == 0
}

/// Fill from `starts` across the ledges (one way: `backward` takes each from its landing to its
/// top) and the whole spans of `bp`, each span whole as `broken` (a bit each) does not say.
pub fn reach(
    bp: &Blueprint,
    open: &impl Fn(i32, i32) -> bool,
    starts: &[(i32, i32)],
    broken: u64,
    backward: bool,
) -> Reach {
    let w = bp.w();
    let ledges = ledges(bp, open);
    let mut seeds: Vec<(i32, i32)> = starts.to_vec();
    let mut decks = 0u64;
    let mut ground = Fill::bits_only();
    let cell_open = |i: usize| {
        let (x, y) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
        open(x, y)
    };
    loop {
        fill(w, bp.h(), &seeds, cell_open, &mut ground);
        let before = seeds.len();
        for l in &ledges {
            let (from, to) = if backward { (l.landing, l.top) } else { (l.top, l.landing) };
            if ground.reached(from.0, from.1) && !ground.reached(to.0, to.1) {
                seeds.push(to);
            }
        }
        for (i, s) in bp.spans.iter().enumerate() {
            if broken >> i & 1 != 0 {
                continue;
            }
            let [a, b] = s.ends();
            let near = |e: jane_core::Rect| e.cells().any(|(x, y)| ground.reached(x, y));
            let (ra, rb) = (near(a), near(b));
            if ra || rb {
                decks |= 1 << i;
            }
            for (far, from) in [(b, ra), (a, rb)] {
                if from {
                    for (x, y) in far.cells() {
                        if open(x, y) && !ground.reached(x, y) {
                            seeds.push((x, y));
                        }
                    }
                }
            }
        }
        if seeds.len() == before {
            return Reach { ground, decks };
        }
    }
}

/// The spans `bp` builds broken, a bit each.
pub fn built_broken(bp: &Blueprint) -> u64 {
    bp.spans.iter().enumerate().fold(0, |b, (i, s)| b | u64::from(s.broken) << i)
}

/// No pits (MAP.md §4.7, tenet 4): every cell reached from `entrance` can walk back to it. The
/// first cell reached that cannot, if any.
pub fn first_pit(
    bp: &Blueprint,
    open: &impl Fn(i32, i32) -> bool,
    entrance: (i32, i32),
    broken: u64,
) -> Option<(i32, i32)> {
    let out = reach(bp, open, &[entrance], broken, false);
    let back = reach(bp, open, &[entrance], broken, true);
    (0..bp.h() as i32)
        .flat_map(|y| (0..bp.w() as i32).map(move |x| (x, y)))
        .find(|&(x, y)| out.reached(x, y) && !back.reached(x, y))
}

/// [`first_pit`] is none.
pub fn no_pit(bp: &Blueprint, open: &impl Fn(i32, i32) -> bool, entrance: (i32, i32), broken: u64) -> bool {
    first_pit(bp, open, entrance, broken).is_none()
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use jane_core::blueprint::Span;
    use jane_core::{Plane, Rect, Tile, ZoneId};

    use super::*;

    /// 40 x 30: a terrace at level 1 north (rows 1 to 9) and south (rows 21 to 28), a valley at
    /// level 0 between (rows 12 to 19, faces rows 10 and 11, rim row 20), a span along y over the
    /// valley at x 18 to 20 (rows 10 to 20), walls round the edge. No stair: the valley is reached
    /// only as the cases give.
    fn valley() -> Blueprint {
        let (w, h) = (40, 30);
        let mut bp = Blueprint::new(ZoneId::County, w, h, Tile::Grass);
        for r in [Rect::new(0, 0, 40, 1), Rect::new(0, 29, 40, 1), Rect::new(0, 0, 1, 30), Rect::new(39, 0, 1, 30)] {
            bp.tiles.fill_rect(r, Tile::Wall);
        }
        bp.tiles.fill_rect(Rect::new(1, 10, 38, 2), Tile::Cliff);
        bp.tiles.fill_rect(Rect::new(1, 20, 38, 1), Tile::Cliff);
        let mut lv = vec![1u8; (w * h) as usize];
        for y in 12..20 {
            for x in 0..w {
                lv[(y * w + x) as usize] = 0;
            }
        }
        bp.level = Some(Plane::pack(w, h, &lv));
        bp.spans = vec![Span { rect: Rect::new(18, 10, 3, 11), along_x: false, deck_level: 1, broken: false }];
        bp
    }

    #[test]
    fn a_span_carries_the_fill_over_and_the_ground_runs_under() {
        let bp = valley();
        let open = tiles_open(&bp);
        let r = reach(&bp, &open, &[(5, 5)], 0, false);
        assert!(r.reached(30, 25), "over the deck to the south terrace");
        assert_eq!(r.decks, 1);
        assert!(!r.reached(19, 15), "the valley under it is not reached from the deck");
        // From the valley: under the span, end to end, and never up onto it.
        let under = reach(&bp, &open, &[(2, 15)], 0, false);
        assert!(under.reached(37, 15), "under the span along the valley");
        assert!(!under.reached(5, 5) && under.decks == 0);
        // Broken, it carries nothing.
        let broken = reach(&bp, &open, &[(5, 5)], 1, false);
        assert!(!broken.reached(30, 25) && broken.decks == 0);
    }

    #[test]
    fn a_ledge_into_a_pit_fails_and_a_stair_out_of_it_mends_it() {
        let mut bp = valley();
        bp.spans.clear();
        // A ledge off the north terrace into the valley, hopped south.
        bp.tiles.fill_rect(Rect::new(5, 10, 3, 2), Tile::LedgeS);
        {
            let open = tiles_open(&bp);
            assert!(reach(&bp, &open, &[(6, 5)], 0, false).reached(6, 15), "down the ledge");
            assert!(!reach(&bp, &open, &[(6, 15)], 0, false).reached(6, 5), "never up it");
            assert_eq!(first_pit(&bp, &open, (6, 5), 0), Some((1, 12)), "the valley is a pit");
        }
        // A stair back up the north face: no pit.
        bp.tiles.fill_rect(Rect::new(30, 10, 3, 2), Tile::Stair);
        let open = tiles_open(&bp);
        assert!(no_pit(&bp, &open, (6, 5), 0));
    }
}
