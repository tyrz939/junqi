//! The county's whole-county floods (what she can walk to from the platform), in one place so they
//! can cross height. With no portals (`County::portals` is `None`: the plain county) a flood is
//! [`fill_words`] and nothing else, so its cells are the cells it always reached; with portals
//! (the terraced county's ledges and spans) it is [`spread`], which takes the same fill across them.

use alloc::vec::Vec;

use jane_core::search::{Fill, fill_words};

use crate::bits::Bits;
use crate::solve::portals::{Crossing, Portals, spread};

/// The scratch a flood reuses, so a stage that floods many times (`cut_through`) allocates once.
#[derive(Debug)]
pub struct Scratch {
    fill: Fill,
    across: Vec<u64>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

impl Scratch {
    /// A scratch for floods that keep only the cells reached.
    pub fn new() -> Self {
        Self { fill: Fill::direct(), across: Vec::new() }
    }
}

/// The cells reached from `starts` over what `open` says is open (word `k` has bit `j` set when
/// cell `64 k + j`, `y * w + x`, is open; asked again for each round, so it must not change while
/// this runs), as a bitset of words, cell `i` at bit `i & 63` of word `i >> 6`. Borrowed from
/// `scratch` until the next flood.
///
/// Without `portals` it is [`fill_words`], byte for byte. With them the fill goes on across every
/// ledge whose top it reached and every span (as built) with an end it reached.
pub fn flood<'s>(
    portals: Option<&Portals>,
    w: u32,
    h: u32,
    starts: &[(i32, i32)],
    mut open: impl FnMut(usize) -> u64,
    scratch: &'s mut Scratch,
) -> &'s [u64] {
    match portals {
        None => {
            fill_words(w, h, starts, open, &mut scratch.fill);
            scratch.fill.seen_words()
        }
        Some(p) => {
            let cross = Crossing { portals: p, broken: p.built_broken(), backward: false };
            let (reached, _) = spread(w, h, &mut open, &cross, starts, &mut scratch.fill);
            scratch.across.clear();
            scratch.across.extend((0..(w as usize * h as usize).div_ceil(64)).map(|k| reached.word(k)));
            &scratch.across
        }
    }
}

/// [`flood`] once, as an owned bitset of `w * h` cells. Without portals the fill's own words are
/// handed over, not copied.
pub fn flood_bits(
    portals: Option<&Portals>,
    w: u32,
    h: u32,
    starts: &[(i32, i32)],
    mut open: impl FnMut(usize) -> u64,
) -> Bits {
    let n = w as usize * h as usize;
    let mut fill = Fill::direct();
    match portals {
        None => {
            fill_words(w, h, starts, open, &mut fill);
            Bits::from_words(fill.into_seen(), n)
        }
        Some(p) => {
            let cross = Crossing { portals: p, broken: p.built_broken(), backward: false };
            spread(w, h, &mut open, &cross, starts, &mut fill).0
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use jane_core::Rect;
    use jane_core::blueprint::{Span, SpanKind};

    use super::*;
    use crate::solve::portals::Ledge;

    /// 20 x 6: a wall column at x 10 with the doorway shut, so two rooms.
    fn open_words(w: u32, h: u32) -> Vec<u64> {
        let n = (w * h) as usize;
        let mut words = vec![0u64; n.div_ceil(64)];
        for i in 0..n {
            if (i as u32 % w) != 10 {
                words[i >> 6] |= 1 << (i & 63);
            }
        }
        words
    }

    #[test]
    fn without_portals_a_flood_is_the_plain_fill_and_the_wall_stops_it() {
        let (w, h) = (20u32, 6u32);
        let words = open_words(w, h);
        let mut scratch = Scratch::new();
        let got = flood(None, w, h, &[(2, 2)], |k| words[k], &mut scratch).to_vec();
        let mut want = Fill::direct();
        fill_words(w, h, &[(2, 2)], |k| words[k], &mut want);
        assert_eq!(got, want.seen_words());
        let bits = flood_bits(None, w, h, &[(2, 2)], |k| words[k]);
        assert!(bits[(2 * w + 3) as usize] && !bits[(2 * w + 15) as usize]);
    }

    #[test]
    fn a_span_or_a_ledge_carries_a_flood_over_the_wall_and_a_broken_span_does_not() {
        let (w, h) = (20u32, 6u32);
        let words = open_words(w, h);
        let span = |broken| Span {
            rect: Rect::new(10, 1, 1, 3),
            along_x: true,
            deck_level: 1,
            broken,
            kind: SpanKind::Bridge,
        };
        let over = Portals { ledges: Vec::new(), spans: vec![span(false)] };
        let bits = flood_bits(Some(&over), w, h, &[(2, 2)], |k| words[k]);
        assert!(bits[(2 * w + 15) as usize], "over the span");
        assert!(bits[(5 * w + 15) as usize], "the far room is one fill");
        let down = Portals { ledges: vec![Ledge { top: (9, 4), landing: (11, 4) }], spans: Vec::new() };
        assert!(flood_bits(Some(&down), w, h, &[(2, 2)], |k| words[k])[(4 * w + 18) as usize], "down the ledge");
        // A ledge only goes one way.
        assert!(!flood_bits(Some(&down), w, h, &[(18, 4)], |k| words[k])[(4 * w + 2) as usize]);
        let broken = Portals { ledges: Vec::new(), spans: vec![span(true)] };
        assert!(!flood_bits(Some(&broken), w, h, &[(2, 2)], |k| words[k])[(2 * w + 15) as usize]);
    }
}
