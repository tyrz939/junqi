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

use jane_core::blueprint::Span;
use jane_core::search::{Fill, fill_words};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Rect};

use crate::bits::Bits;

/// The most cells a ledge's face is deep (the sim's `tuning::LEDGE_FACE_MAX`).
const LEDGE_FACE_MAX: i32 = 6;

/// What a fill across the portals reached: the ground, and which spans' decks (a bit each).
#[derive(Debug)]
pub struct Reach {
    pub ground: Bits,
    pub decks: u64,
    w: u32,
    h: u32,
}

impl Reach {
    /// Outside the grid is unreached.
    pub fn reached(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as u32) < self.w
            && (y as u32) < self.h
            && self.ground[y as usize * self.w as usize + x as usize]
    }
}

/// A ledge's one-way portal: from its top cell to its landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ledge {
    pub top: (i32, i32),
    pub landing: (i32, i32),
}

/// The ways a fill leaves the ground it cannot take as steps: every ledge (one way) and every span
/// (both ways, whole unless `broken` says not). Without either the fills are the plain fill, so a
/// zone with no levels and no spans (every county, dungeon and interior so far) is unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Portals {
    pub ledges: Vec<Ledge>,
    pub spans: Vec<Span>,
}

impl Portals {
    /// Every ledge of `bp` with ground on both sides ([`ledges`]) and its spans.
    pub fn of(bp: &Blueprint, open: &impl Fn(i32, i32) -> bool) -> Self {
        Self { ledges: ledges(bp, open), spans: bp.spans.clone() }
    }

    /// The portals of a zone that has levels or spans, and `None` for one that has neither (so
    /// every fill over it is the plain fill).
    pub fn for_zone(bp: &Blueprint) -> Option<Self> {
        (bp.level.is_some() || !bp.spans.is_empty()).then(|| Self::of(bp, &tiles_open(bp)))
    }

    /// Neither a ledge nor a span.
    pub fn is_empty(&self) -> bool {
        self.ledges.is_empty() && self.spans.is_empty()
    }

    /// The spans as they are built broken, a bit each.
    pub fn built_broken(&self) -> u64 {
        self.spans.iter().enumerate().fold(0, |b, (i, s)| b | u64::from(s.broken) << i)
    }
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

/// What `open(x, y)` says of every cell of `bp`, a bit a cell (`y * w + x`), as [`reach_words`]
/// takes it.
pub fn open_words(bp: &Blueprint, open: &impl Fn(i32, i32) -> bool) -> Vec<u64> {
    let (w, h) = (bp.w() as usize, bp.h() as usize);
    let mut words = alloc::vec![0u64; (w * h).div_ceil(64)];
    for y in 0..h {
        for x in 0..w {
            if open(x as i32, y as i32) {
                let i = y * w + x;
                words[i >> 6] |= 1 << (i & 63);
            }
        }
    }
    words
}

/// How a fill crosses the portals: which, which spans are down, and which way (`backward` takes
/// each ledge from its landing to its top, for the no-pit proof).
#[derive(Clone, Copy, Debug)]
pub struct Crossing<'a> {
    pub portals: &'a Portals,
    pub broken: u64,
    pub backward: bool,
}

/// Fill from `starts` across the portals, word-parallel: `fill_words` over what is open and not yet
/// reached, from the seeds of each round, ORed into the cells reached; then every ledge whose top
/// is reached and every whole span with an end reached seeds what lies across, until nothing more
/// is seeded. `open(k)` is word `k` of what is open (the plain fill's), asked again each round, so
/// it must not change while this runs. Returns the cells reached, and which spans' decks (a bit
/// each) carried a fill.
pub fn spread(
    w: u32,
    h: u32,
    open: &mut impl FnMut(usize) -> u64,
    cross: &Crossing<'_>,
    starts: &[(i32, i32)],
    fill: &mut Fill,
) -> (Bits, u64) {
    let (wu, n) = (w as usize, w as usize * h as usize);
    let mut acc = Bits::new(n, false);
    let mut decks = 0u64;
    let mut seeds: Vec<(i32, i32)> = starts.to_vec();
    let ix = |x: i32, y: i32| y as usize * wu + x as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h;
    loop {
        seeds.retain(|&(x, y)| inside(x, y) && !acc[ix(x, y)]);
        if !seeds.is_empty() {
            let reached = &acc;
            fill_words(w, h, &seeds, |k| open(k) & !reached.word(k), fill);
            acc.or_words(fill.seen_words());
        }
        seeds.clear();
        let mut is_open = |x: i32, y: i32| {
            let i = ix(x, y);
            open(i >> 6) >> (i & 63) & 1 != 0
        };
        for l in &cross.portals.ledges {
            let (from, to) = if cross.backward { (l.landing, l.top) } else { (l.top, l.landing) };
            if inside(from.0, from.1)
                && inside(to.0, to.1)
                && acc[ix(from.0, from.1)]
                && !acc[ix(to.0, to.1)]
                && is_open(to.0, to.1)
            {
                seeds.push(to);
            }
        }
        for (i, s) in cross.portals.spans.iter().enumerate() {
            if cross.broken >> i & 1 != 0 {
                continue;
            }
            let [a, b]: [Rect; 2] = s.ends();
            let near = |e: Rect| e.cells().any(|(x, y)| inside(x, y) && acc[ix(x, y)]);
            let (ra, rb) = (near(a), near(b));
            if ra || rb {
                decks |= 1 << i;
            }
            for (far, from) in [(b, ra), (a, rb)] {
                if from {
                    for (x, y) in far.cells() {
                        if inside(x, y) && !acc[ix(x, y)] && is_open(x, y) {
                            seeds.push((x, y));
                        }
                    }
                }
            }
        }
        if seeds.is_empty() {
            return (acc, decks);
        }
    }
}

/// [`spread`] over a bitset of what is open (a bit a cell, `y * w + x`): the cells reached from
/// `starts`.
pub fn reach_words(
    w: u32,
    h: u32,
    open: &[u64],
    p: &Portals,
    starts: &[(i32, i32)],
    broken: u64,
    backward: bool,
) -> Bits {
    reach_decks(w, h, open, p, starts, broken, backward).0
}

fn reach_decks(
    w: u32,
    h: u32,
    open: &[u64],
    p: &Portals,
    starts: &[(i32, i32)],
    broken: u64,
    backward: bool,
) -> (Bits, u64) {
    let mut word = |k: usize| open.get(k).copied().unwrap_or(0);
    let cross = Crossing { portals: p, broken, backward };
    spread(w, h, &mut word, &cross, starts, &mut Fill::direct())
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
    let (w, h) = (bp.w(), bp.h());
    let (ground, decks) = reach_decks(w, h, &open_words(bp, open), &Portals::of(bp, open), starts, broken, backward);
    Reach { ground, decks, w, h }
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
    let (w, h) = (bp.w(), bp.h());
    let (words, portals) = (open_words(bp, open), Portals::of(bp, open));
    let out = reach_words(w, h, &words, &portals, &[entrance], broken, false);
    let back = reach_words(w, h, &words, &portals, &[entrance], broken, true);
    (0..out.len().div_ceil(64)).find_map(|k| {
        let m = out.word(k) & !back.word(k);
        (m != 0).then(|| {
            let i = (k << 6) + m.trailing_zeros() as usize;
            ((i % w as usize) as i32, (i / w as usize) as i32)
        })
    })
}

/// [`first_pit`] is none.
pub fn no_pit(bp: &Blueprint, open: &impl Fn(i32, i32) -> bool, entrance: (i32, i32), broken: u64) -> bool {
    first_pit(bp, open, entrance, broken).is_none()
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use jane_core::blueprint::{Span, SpanKind};
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
        bp.spans = vec![Span {
            rect: Rect::new(18, 10, 3, 11),
            along_x: false,
            deck_level: 1,
            broken: false,
            kind: SpanKind::Bridge,
        }];
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

    /// The fill as it was before the word-parallel one: from every seed again each round, over a
    /// closure, with the ground a [`Fill`]. What the new one must equal.
    fn oracle(
        bp: &Blueprint,
        open: &impl Fn(i32, i32) -> bool,
        starts: &[(i32, i32)],
        broken: u64,
        backward: bool,
    ) -> (Vec<bool>, u64) {
        use jane_core::search::fill;
        let w = bp.w();
        let ledges = ledges(bp, open);
        let mut seeds: Vec<(i32, i32)> = starts.to_vec();
        let mut decks = 0u64;
        let mut ground = Fill::bits_only();
        let cell_open = |i: usize| open((i as u32 % w) as i32, (i as u32 / w) as i32);
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
                let near = |e: Rect| e.cells().any(|(x, y)| ground.reached(x, y));
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
                let all = (0..bp.h() as i32).flat_map(|y| (0..w as i32).map(move |x| (x, y)));
                return (all.map(|(x, y)| ground.reached(x, y)).collect(), decks);
            }
        }
    }

    fn same_as_oracle(bp: &Blueprint, starts: &[(i32, i32)], broken: u64, what: &str) {
        let open = tiles_open(bp);
        for backward in [false, true] {
            let (want, decks) = oracle(bp, &open, starts, broken, backward);
            let got = reach(bp, &open, starts, broken, backward);
            let words =
                reach_words(bp.w(), bp.h(), &open_words(bp, &open), &Portals::of(bp, &open), starts, broken, backward);
            assert_eq!(got.decks, decks, "{what}: decks, backward {backward}");
            for (i, &r) in want.iter().enumerate() {
                assert_eq!(words[i], r, "{what}: cell {i}, backward {backward}");
                assert_eq!(got.reached((i as u32 % bp.w()) as i32, (i as u32 / bp.w()) as i32), r, "{what}: reach {i}");
            }
        }
    }

    #[test]
    fn the_word_fill_equals_the_old_fill_on_the_cases() {
        let mut bp = valley();
        for (starts, broken) in [(vec![(5, 5)], 0), (vec![(5, 5)], 1), (vec![(2, 15)], 0), (vec![(30, 25)], 0)] {
            same_as_oracle(&bp, &starts, broken, "valley");
        }
        bp.spans.clear();
        bp.tiles.fill_rect(Rect::new(5, 10, 3, 2), Tile::LedgeS);
        same_as_oracle(&bp, &[(6, 5)], 0, "ledge");
        same_as_oracle(&bp, &[(6, 15)], 0, "ledge from below");
        bp.tiles.fill_rect(Rect::new(30, 10, 3, 2), Tile::Stair);
        same_as_oracle(&bp, &[(6, 5)], 0, "stair");
    }

    /// A small xorshift, so the grids are the same on every machine.
    fn dice(state: &mut u32) -> u32 {
        *state ^= *state << 13;
        *state ^= *state >> 17;
        *state ^= *state << 5;
        *state
    }

    #[test]
    fn the_word_fill_equals_the_old_fill_on_200_random_grids() {
        let mut state = 0x2545_F491u32;
        let (mut ledged, mut decked) = (0usize, 0usize);
        for case in 0..200 {
            let (w, h) = (64u32, 48u32);
            let mut bp = Blueprint::new(ZoneId::County, w, h, Tile::Grass);
            for _ in 0..(w * h / 4) {
                let (x, y) = ((dice(&mut state) % w) as i32, (dice(&mut state) % h) as i32);
                bp.tiles.fill_rect(Rect::new(x, y, 1 + (dice(&mut state) % 3) as i32, 1), Tile::Wall);
            }
            for _ in 0..=(dice(&mut state) % 5) {
                let (x, y) = ((dice(&mut state) % (w - 4)) as i32, (dice(&mut state) % (h - 6)) as i32);
                let tile = [Tile::LedgeS, Tile::LedgeN, Tile::LedgeE, Tile::LedgeW][(dice(&mut state) % 4) as usize];
                let r = if matches!(tile, Tile::LedgeS | Tile::LedgeN) {
                    Rect::new(x, y, 1 + (dice(&mut state) % 4) as i32, 1 + (dice(&mut state) % 3) as i32)
                } else {
                    Rect::new(x, y, 1 + (dice(&mut state) % 3) as i32, 1 + (dice(&mut state) % 4) as i32)
                };
                bp.tiles.fill_rect(r, tile);
            }
            for _ in 0..(dice(&mut state) % 4) {
                let along_x = dice(&mut state) % 2 == 0;
                let (x, y) = (2 + (dice(&mut state) % (w - 14)) as i32, 2 + (dice(&mut state) % (h - 14)) as i32);
                let (len, wide) = (3 + (dice(&mut state) % 8) as i32, 1 + (dice(&mut state) % 3) as i32);
                let rect = if along_x { Rect::new(x, y, len, wide) } else { Rect::new(x, y, wide, len) };
                let broken = dice(&mut state) % 5 == 0;
                bp.spans.push(Span { rect, along_x, deck_level: 1, broken, kind: SpanKind::Bridge });
            }
            let starts: Vec<(i32, i32)> = (0..=dice(&mut state) % 3)
                .map(|_| ((dice(&mut state) % w) as i32, (dice(&mut state) % h) as i32))
                .collect();
            let broken = u64::from(dice(&mut state) % 4);
            same_as_oracle(&bp, &starts, broken, &alloc::format!("grid {case}"));
            let open = tiles_open(&bp);
            ledged += usize::from(!ledges(&bp, &open).is_empty());
            decked += usize::from(oracle(&bp, &open, &starts, broken, false).1 != 0);
        }
        assert!(ledged >= 40 && decked >= 20, "the grids use the portals: {ledged} with a ledge, {decked} with a deck");
    }

    #[test]
    fn first_pit_is_the_old_first_pit_on_the_cases() {
        let bp = valley();
        let open = tiles_open(&bp);
        let w = bp.w() as usize;
        for start in [(5, 5), (2, 15), (30, 25)] {
            for broken in [0u64, 1] {
                let out = oracle(&bp, &open, &[start], broken, false).0;
                let back = oracle(&bp, &open, &[start], broken, true).0;
                let want = (0..out.len()).find(|&i| out[i] && !back[i]).map(|i| ((i % w) as i32, (i / w) as i32));
                assert_eq!(first_pit(&bp, &open, start, broken), want, "{start:?} broken {broken}");
            }
        }
    }
}
