//! The volume row of the Controls screen (PRESENTATION.md §5): master, music and effects, each a
//! minus, its value and a plus, in steps of ten. The keys work it too: on the row, left and right
//! turn the lit one down and up, and confirm moves the light to the next.

use alloc::string::ToString;
use jane_art::font::Face;

use crate::audio::Volumes;
use crate::input::UiAction;
use crate::ui::cmd::Rect;
use crate::ui::core::{ButtonKind, Ink, Ui, line_h, text_w, wid};
use crate::ui::style;

/// The three the row sets, in order.
pub const CHANNELS: [&str; 3] = ["Master", "Music", "Effects"];
/// A press moves a volume this far.
pub const STEP: u8 = 10;

fn get(v: Volumes, ch: u8) -> u8 {
    match ch {
        0 => v.master,
        1 => v.music,
        _ => v.sfx,
    }
}

/// `v` with channel `ch` moved by `by` steps, kept to 0..=100.
pub fn nudge(v: Volumes, ch: u8, by: i32) -> Volumes {
    let n = (i32::from(get(v, ch)) + by * i32::from(STEP)).clamp(0, 100) as u8;
    let mut out = v;
    match ch {
        0 => out.master = n,
        1 => out.music = n,
        _ => out.sfx = n,
    }
    out
}

/// The row at `(x, y)`; `lit` is the channel the keys are on when the row has the focus.
/// Returns the volumes when they changed this frame.
pub fn row(ui: &mut Ui, x: i32, y: i32, v: Volumes, lit: Option<u8>) -> Option<Volumes> {
    let mut out = None;
    ui.text(
        x + 20,
        y + 3,
        "Volume",
        Ink::small(if lit.is_some() { style::text_bright() } else { style::text() }).shadow(),
    );
    if ui.interactive
        && let Some(ch) = lit
    {
        for a in ui.input.actions.clone() {
            match a {
                UiAction::Left => out = Some(nudge(out.unwrap_or(v), ch, -1)),
                UiAction::Right => out = Some(nudge(out.unwrap_or(v), ch, 1)),
                _ => {}
            }
        }
    }
    let shown = out.unwrap_or(v);
    for (i, label) in CHANNELS.iter().enumerate() {
        let ch = i as u8;
        let gx = x + 200 + i as i32 * 132;
        let on = lit == Some(ch);
        ui.text(
            gx,
            y - line_h(Face::Fine) + 1,
            label,
            Ink::fine(if on { style::gold() } else { style::quiet() }).shadow(),
        );
        let minus = Rect::new(gx, y, 22, 22);
        let plus = Rect::new(gx + 74, y, 22, 22);
        if ui.button(wid("vol-down", u32::from(ch)), minus, "-", ButtonKind::Tab { on: false }, true, false) {
            out = Some(nudge(out.unwrap_or(v), ch, -1));
        }
        let n = get(shown, ch).to_string();
        let tx = gx + 48 - text_w(Face::Small, &n) / 2;
        ui.text(tx, y + 3, &n, Ink::small(if on { style::text_bright() } else { style::text() }).shadow());
        if on {
            ui.focus_ring(Rect::new(gx + 24, y, 48, 22));
        }
        if ui.button(wid("vol-up", u32::from(ch)), plus, "+", ButtonKind::Tab { on: false }, true, false) {
            out = Some(nudge(out.unwrap_or(v), ch, 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nudge_moves_one_channel_a_step_and_stops_at_the_ends() {
        let v = Volumes { master: 80, music: 70, sfx: 100 };
        assert_eq!(nudge(v, 1, -1), Volumes { music: 60, ..v });
        assert_eq!(nudge(v, 2, 1), v, "full stays full");
        assert_eq!(nudge(Volumes { master: 5, ..v }, 0, -1).master, 0);
    }
}
