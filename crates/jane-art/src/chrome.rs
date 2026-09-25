//! Chrome (ART.md §7): panels, buttons, bars and slots, generated at 1x on the view grid at the
//! size they are shown (768 x 432 and wider), never scaled. Chrome is flat: albedo only, and the
//! light pass does not touch it. The panel fill and the cooldown veil are translucent through
//! [`crate::palette::alpha`]; everything else is opaque.
//!
//! All sizes are view px. The renderer caches a piece by `(w, h, style)`.

use jane_core::angle::{Angle, iatan2};
use jane_core::grid::Rect;

use crate::canvas::{Canvas, Dir, FLAT};
use crate::font::{Face, Font, Style};
use crate::palette::{Ix, Ramp, Tone};

/// A cut corner removes this many px diagonally.
const CUT: i32 = 2;
/// The bevel is this many px wide inside the outline.
const BEVEL: i32 = 2;

/// A button's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    /// At rest.
    Idle,
    /// Under the pointer or focused: the fill a tone lighter.
    Hover,
    /// Held: the face one px down and the bevel inverted.
    Pressed,
    /// Not available: a darker fill and grey text.
    Disabled,
}

/// The framed box every piece stands on: `fill`, a 1-px `k` outline with cut corners, and a
/// 2-px bevel inside it, `W` top and left and `G` bottom and right when `raised`, the other way
/// round when sunk.
fn frame(w: i32, h: i32, fill: Ix, raised: bool) -> Canvas {
    let mut c = Canvas::flat(w, h);
    c.fill_rect(Rect::new(0, 0, w, h), fill, 0);
    for y in 0..h {
        for x in 0..w {
            let (dt, dl, db, dr) = (y, x, h - 1 - y, w - 1 - x);
            let d = dt.min(dl).min(db).min(dr);
            if dl.min(dr) + dt.min(db) < CUT {
                c.clear_px(x, y);
            } else if (1..=BEVEL).contains(&d) {
                let lit = dt == d || dl == d;
                let ix = if lit == raised { Ix::BEVEL_LIGHT } else { Ix::BEVEL_SHADE };
                c.put(x, y, ix, FLAT, 0);
            }
        }
    }
    c
}

/// A panel `w x h` px: the 85 % `ui_panel` fill, a `k` outline with cut corners, a raised
/// 2-px bevel.
pub fn panel(w: i32, h: i32) -> Canvas {
    let mut c = frame(w, h, Ramp::UiPanel.at(Tone::Base), true);
    c.outline();
    c
}

/// A button `w x h` px in `state` with `label` centred in `face`: a panel whose fill follows the
/// state, pressed one px down with its bevel inverted.
pub fn button(w: i32, h: i32, state: ButtonState, label: &str, font: &Font, face: Face) -> Canvas {
    let fill = Ramp::UiPanel.at(match state {
        ButtonState::Idle => Tone::Base,
        ButtonState::Hover => Tone::Lift,
        ButtonState::Pressed => Tone::Mid,
        ButtonState::Disabled => Tone::Shade,
    });
    let mut c = frame(w, h, fill, state != ButtonState::Pressed);
    let ink = if state == ButtonState::Disabled { Ix::BEVEL_SHADE } else { Ramp::UiInk.at(Tone::Light) };
    let (tw, th) = (Font::measure(face, label), face.cell().1);
    let down = i32::from(state == ButtonState::Pressed);
    font.draw(&mut c, (w - tw) / 2, (h - th) / 2 + down, label, Style { shadow: true, ..Style::plain(face, ink) });
    c.outline();
    c
}

/// A bar `w x h` px: a `G` track, the fill as a dithered gradient of `ramp` from its light tone
/// at the top to its shade at the bottom, a damage-lag tail in `ui_lag` from the fill to `lag`,
/// and ticks in `K` at every quarter. `fill` and `lag` are permille of the track.
pub fn bar(w: i32, h: i32, fill: i32, lag: i32, ramp: Ramp) -> Canvas {
    let mut c = Canvas::flat(w, h);
    c.fill_rect(Rect::new(0, 0, w, h), Ix::BEVEL_SHADE, 0);
    let inner = Rect::new(1, 1, w - 2, h - 2);
    let fw = inner.w * fill.clamp(0, 1000) / 1000;
    let lw = inner.w * lag.clamp(0, 1000) / 1000;
    if lw > fw {
        c.fill_rect(Rect::new(inner.x + fw, inner.y, lw - fw, inner.h), Ramp::UiLag.at(Tone::Base), 0);
    }
    if fw > 0 {
        c.fill_rect(Rect::new(inner.x, inner.y, fw, inner.h), ramp.at(Tone::Base), 0);
        c.gradient(Rect::new(inner.x, inner.y, fw, inner.h), ramp, Dir::Down, Tone::High, Tone::Shade, true);
    }
    for q in 1..4 {
        let x = inner.x + inner.w * q / 4;
        c.vline(x, inner.y, inner.bottom() - 1, Ix::SEAM, 0);
    }
    c.outline();
    c
}

/// A slot `size` px square (36 in the bag, 20 as a chip): a sunk dark well. The icon goes in
/// it, and the [`sweep`] over the icon.
pub fn slot(size: i32) -> Canvas {
    let mut c = frame(size, size, Ramp::UiSlot.at(Tone::Base), false);
    c.outline();
    c
}

/// The cooldown and GCD overlay for a `size` px slot: the part of a turn still to run (`left`,
/// permille, clockwise from twelve o'clock) inside the slot's bevel, in `W` at 30 %; clear
/// elsewhere. Drawn over the slot's icon.
pub fn sweep(size: i32, left: i32) -> Canvas {
    let mut c = Canvas::flat(size, size);
    let turn = left.clamp(0, 1000) * 65536 / 1000;
    for (i, a) in angle_table(size).iter().enumerate() {
        let (x, y) = (i as i32 % size, i as i32 / size);
        let inside = x > BEVEL && y > BEVEL && x < size - 1 - BEVEL && y < size - 1 - BEVEL;
        if inside && i32::from(*a) < turn {
            c.put(x, y, Ramp::UiVeil.at(Tone::Base), FLAT, 0);
        }
    }
    c
}

/// The conic sweep's angle table for a `size` px slot: per pixel, the clockwise angle from
/// twelve o'clock about the centre, in jane-core [`Angle`] units. Built once per slot size.
pub fn angle_table(size: i32) -> Vec<u16> {
    let mut t = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let a = iatan2(2 * y + 1 - size, 2 * x + 1 - size);
            t.push(a.0.wrapping_sub(Angle::NORTH.0));
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_is_flat_and_closed() {
        let font = Font::build();
        for c in [
            panel(64, 32),
            button(48, 20, ButtonState::Pressed, "Take", &font, Face::Fine),
            bar(60, 8, 600, 800, Ramp::ClothRed),
            slot(36),
        ] {
            c.validate().unwrap();
            assert!(c.is_flat());
            assert!(matches!(c.get(0, 0), Ix::CLEAR | Ix::INK), "a cut or outlined corner");
        }
        let s = sweep(36, 250);
        s.validate().unwrap();
        assert_eq!(s.get(24, 8), Ramp::UiVeil.at(Tone::Base), "the first quarter is veiled");
        assert_eq!(s.get(10, 26), Ix::CLEAR, "the rest is not");
    }

    #[test]
    fn a_pressed_button_inverts_its_bevel() {
        let font = Font::build();
        let up = button(40, 16, ButtonState::Idle, "", &font, Face::Fine);
        let down = button(40, 16, ButtonState::Pressed, "", &font, Face::Fine);
        assert_eq!(up.get(20, 1), Ix::BEVEL_LIGHT);
        assert_eq!(down.get(20, 1), Ix::BEVEL_SHADE);
    }

    #[test]
    fn the_sweep_starts_at_twelve_and_runs_clockwise() {
        let t = angle_table(36);
        let at = |x: i32, y: i32| t[(y * 36 + x) as usize];
        assert!(at(18, 2) < 2000, "twelve o'clock is near zero");
        assert!(at(33, 18) > 15000 && at(33, 18) < 18000, "three o'clock is a quarter turn");
        assert!(at(2, 18) > 47000, "nine o'clock is three quarters");
    }
}
