//! Her side of a fight, drawn over the world (PLAY-PLAN §2.1, PRESENTATION.md §3.9): the ring
//! under her target (red for a foe, gold for a prop), a foe's name and a slim health bar over it,
//! a slim cast bar under anyone whose cast is building, and a small mark where a click on open
//! ground sent her. Read from [`FightOverlay`], which the presenter gathers from the view; each
//! machine at a table draws its own target and everyone's bars.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};

use crate::present::{CastBar, FightOverlay, TargetMarker};
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui, line_h, text_w};
use crate::ui::style::{self, argb};

/// The bars' width and height, canvas px.
pub const BAR_W: i32 = 28;
pub const BAR_H: i32 = 3;
/// The ring is this much wider than it is tall.
const RING_FLAT: i32 = 3;

/// Draws all of it: under the rest of the UI, over the world.
pub fn draw(ui: &mut Ui, o: &FightOverlay) {
    if let Some((x, y)) = o.walk {
        walk_mark(ui, (x, y), o.tick);
    }
    if let Some(t) = o.target {
        target(ui, &t, o.tick);
    }
    for b in &o.bars {
        cast_bar(ui, b);
    }
}

/// The ring under her target, and for a foe its name and health over its head.
fn target(ui: &mut Ui, t: &TargetMarker, tick: u32) {
    let ix = if t.hostile { Ramp::ClothRed.at(Tone::Lift) } else { style::gold() };
    // A slow breath in the ring, so the eye finds it without it shouting.
    let breath = (tick % 90) as i32;
    let a = 200 + (if breath < 45 { breath } else { 90 - breath }) as u8;
    ring(ui, (t.x, t.y), t.half, argb(ix, a), argb(style::INK, 110));
    if !t.hostile && t.name.is_empty() {
        return;
    }
    let mut top = t.head - 3;
    if let Some((hp, max)) = t.hp {
        let w = BAR_W;
        let r = Rect::new(t.x - w / 2, top - BAR_H, w, BAR_H);
        bar(ui, r, hp.max(0) as i64 * 256 / i64::from(max.max(1)), argb(Ramp::ClothRed.at(Tone::Base), 250));
        top -= BAR_H + 2;
    }
    if !t.name.is_empty() {
        let w = text_w(Face::Fine, t.name);
        let ink = Ink::fine(if t.hostile { style::text_bright() } else { style::gold() }).outline();
        ui.text(t.x - w / 2, top - line_h(Face::Fine), t.name, ink);
    }
}

/// A slim bar under a casting body, in its school's colour, with a bright head.
fn cast_bar(ui: &mut Ui, b: &CastBar) {
    let ix = Ramp::at(jane_art::fx::school_ramp(b.school), Tone::Light);
    let r = Rect::new(b.x - BAR_W / 2, b.y + 5, BAR_W, BAR_H);
    let fill = bar(ui, r, i64::from(b.frac), argb(ix, 245));
    if fill > 0 && fill < BAR_W - 2 {
        let head = Ramp::at(jane_art::fx::school_ramp(b.school), Tone::High);
        ui.fill(Rect::new(r.x as i32 + 1 + fill - 1, r.y as i32 + 1, 1, BAR_H - 2), argb(head, 255));
    }
}

/// A framed bar `r` filled `frac` 256ths with `c`: returns the filled width inside the frame.
fn bar(ui: &mut Ui, r: Rect, frac: i64, c: u32) -> i32 {
    let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
    ui.fill(Rect::new(x - 1, y - 1, w + 2, h + 2), argb(style::INK, 200));
    ui.fill(Rect::new(x, y, w, h), argb(style::well(), 220));
    let inner = (frac.clamp(0, 256) * i64::from(w) / 256) as i32;
    if inner > 0 {
        ui.fill(Rect::new(x, y, inner, h), c);
    }
    inner
}

/// Where a click sent her: a small ring that closes in, and a dot.
fn walk_mark(ui: &mut Ui, (x, y): (i32, i32), tick: u32) {
    let k = (tick % 40) as i32;
    let half = 10 - k / 7;
    let c = argb(style::gold(), 150 + (40 - k) as u8 * 2);
    ring(ui, (x, y), half.max(4), c, argb(style::INK, 70));
    ui.fill(Rect::new(x, y - 1, 1, 1), argb(style::text_bright(), 230));
}

/// A flat ring of half-width `half` round `(cx, cy)`: `c` with a dark line under it.
fn ring(ui: &mut Ui, (cx, cy): (i32, i32), half: i32, c: u32, under: u32) {
    let rx = half.max(2);
    let ry = (rx / RING_FLAT).max(2);
    // Each row's half-width where the ellipse crosses the row's outer edge (half a row out), so
    // the rows join without gaps and the top and bottom are short runs, not a lid.
    let w = |dy: i32| -> i32 {
        let e = (2 * dy.abs() - 1).max(0);
        isqrt(rx * rx * (4 * ry * ry - e * e) / (4 * ry * ry))
    };
    for pass in 0..2 {
        let (oy, col) = if pass == 0 { (1, under) } else { (0, c) };
        for dy in -ry..=ry {
            let here = w(dy);
            let next = match dy.cmp(&0) {
                std::cmp::Ordering::Less => w(dy + 1),
                std::cmp::Ordering::Greater => w(dy - 1),
                std::cmp::Ordering::Equal => here,
            };
            let y = cy + dy + oy;
            if dy.abs() == ry {
                ui.fill(Rect::new(cx - here, y, 2 * here + 1, 1), col);
            }
            let (lo, hi) = (here.min(next), here.max(next));
            let span = hi - lo + 1;
            ui.fill(Rect::new(cx - hi, y, span, 1), col);
            ui.fill(Rect::new(cx + lo, y, span, 1), col);
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
    fn a_ring_is_closed_and_flat() {
        for half in [3, 9, 20] {
            let ry = (half / RING_FLAT).max(2);
            assert!(ry < half, "flatter than tall");
        }
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(81), 9);
        assert_eq!(isqrt(80), 8);
    }
}
