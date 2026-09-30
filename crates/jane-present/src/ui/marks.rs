//! The quest marks over heads (PRESENTATION.md §3.8): the one marker the world has, and only on
//! people. A small gold glyph in the stroke font over whoever has a quest to give her, and over
//! whoever takes one back once it is ready; a gentle bob; a soft glow round it after dark.
//!
//! Which mark is the sim's word (`View::quest_mark`, per seat), gathered by the presenter with
//! where each head is ([`Present::marks`](crate::Present::marks)): so each machine at a table
//! shows its own, and none over someone she is talking to.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::view::QuestMark;

use crate::present::QuestMarker;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui, advance, line_h};
use crate::ui::style::{self, argb};

/// The glyph over someone with a quest to give her. The owner's choice (2026-10-01): the
/// question is theirs to ask. WoW has it the other way round (`!` to take a quest, `?` to hand
/// one in): swap these two to flip it.
pub const OFFER_GLYPH: &str = "?";
/// The glyph over someone a ready quest goes back to.
pub const HAND_IN_GLYPH: &str = "!";

/// Ticks for one bob, up and down, and how many rows it rises.
pub const BOB_TICKS: u32 = 72;
pub const BOB_ROWS: i32 = 2;

/// The glyph for a mark.
pub fn glyph(m: QuestMark) -> &'static str {
    match m {
        QuestMark::Offer => OFFER_GLYPH,
        QuestMark::HandIn => HAND_IN_GLYPH,
    }
}

/// Rows the mark stands up at `tick`, 0 to `BOB_ROWS`: a slow triangle, each person on their own
/// phase so a crowd does not bob as one.
pub fn bob(tick: u32, id: u32) -> i32 {
    let t = (tick.wrapping_add(id.wrapping_mul(29))) % BOB_TICKS;
    let half = BOB_TICKS / 2;
    let up = if t < half { t } else { BOB_TICKS - t };
    (up as i32 * (2 * BOB_ROWS + 1) / BOB_TICKS as i32).min(BOB_ROWS)
}

/// The glyph's box over a head at canvas `(x, y)` (its foot), risen `bob` rows.
pub fn rect(m: &QuestMarker, bob: i32) -> Rect {
    let (w, h) = (advance(Face::Small), line_h(Face::Small));
    Rect::new(m.x - w / 2, m.y - h - bob, w, h)
}

/// Draws every mark: under the rest of the UI, over the world. `dark`: they glow.
pub fn draw(ui: &mut Ui, marks: &[QuestMarker], dark: bool, tick: u32) {
    for m in marks {
        let r = rect(m, bob(tick, m.id));
        if dark {
            let (cx, cy) = (i32::from(r.x) + i32::from(r.w) / 2, i32::from(r.y) + i32::from(r.h) / 2);
            glow(ui, (cx, cy), 10, argb(Ramp::UiGold.at(Tone::Light), 62));
            glow(ui, (cx, cy), 6, argb(Ramp::UiGold.at(Tone::High), 96));
        }
        let ink = Ink::small(style::gold()).outline();
        ui.text(i32::from(r.x), i32::from(r.y), glyph(m.mark), ink);
    }
}

/// A soft disc of `c` round `(cx, cy)`, radius `r`, a row at a time.
fn glow(ui: &mut Ui, (cx, cy): (i32, i32), r: i32, c: u32) {
    for dy in -r..=r {
        let w = isqrt(r * r - dy * dy);
        if w > 0 {
            ui.fill(Rect::new(cx - w, cy + dy, 2 * w + 1, 1), c);
        }
    }
}

fn isqrt(n: i32) -> i32 {
    let mut x = 0;
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owner_s_way_round_and_one_constant_flips_it() {
        assert_eq!(glyph(QuestMark::Offer), "?");
        assert_eq!(glyph(QuestMark::HandIn), "!");
    }

    #[test]
    fn a_bob_is_gentle_and_each_head_keeps_its_own_time() {
        let rows: Vec<i32> = (0..BOB_TICKS).map(|t| bob(t, 0)).collect();
        assert!(rows.iter().all(|&r| (0..=BOB_ROWS).contains(&r)));
        assert!(rows.windows(2).all(|w| (w[0] - w[1]).abs() <= 1), "never jumps: {rows:?}");
        assert_eq!(*rows.iter().max().unwrap(), BOB_ROWS);
        assert!((0..BOB_TICKS).any(|t| bob(t, 0) != bob(t, 1)), "two heads, two phases");
    }
}
