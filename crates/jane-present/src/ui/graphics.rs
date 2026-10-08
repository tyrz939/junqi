//! A console's Graphics page (Pause > Graphics, PORT.md §13.13): the preset, the frame rate and
//! each costly effect, turned with the d-pad's left and right; every turn takes at once and is
//! kept (`gfx_psp::Graphics` in `settings.txt`). Never drawn on PC.

use alloc::format;
use jane_art::palette::{Ramp, Tone};

use crate::gfx_psp::{Effect, FrameRate, Graphics, Preset};
use crate::input::UiAction;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, text_w, wid};
use crate::ui::menus::{dim, heading};
use crate::ui::style::{self, argb};

/// The page's lit row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraphicsState {
    pub row: u8,
}

/// Rows: the preset, the frame rate, each effect, Back.
const ROWS: u8 = 2 + Effect::ALL.len() as u8 + 1;
const BACK: u8 = ROWS - 1;

/// Draws the page over `g`; the settings when a row turned them.
pub fn draw_console(ui: &mut Ui, st: &mut GraphicsState, g: Graphics) -> Option<Graphics> {
    let mut now = g;
    if ui.interactive {
        for a in ui.input.actions.clone() {
            match a {
                UiAction::Up => st.row = st.row.saturating_sub(1),
                UiAction::Down => st.row = (st.row + 1).min(BACK),
                UiAction::Left | UiAction::Right | UiAction::Confirm if st.row < BACK => {
                    let by: i32 = if a == UiAction::Left { -1 } else { 1 };
                    let step = |i: usize, n: usize| (i as i32 + by).rem_euclid(n as i32) as usize;
                    match st.row {
                        0 => {
                            let i = now.preset().map_or(0, |p| Preset::ALL.iter().position(|q| *q == p).unwrap_or(0));
                            let p = Preset::ALL[if now.preset().is_some() { step(i, 3) } else { 0 }];
                            now.on = p.graphics().on;
                        }
                        1 => {
                            let i = FrameRate::ALL.iter().position(|r| *r == now.rate).unwrap_or(1);
                            now.rate = FrameRate::ALL[step(i, 3)];
                        }
                        r => {
                            let e = Effect::ALL[usize::from(r - 2)];
                            now.set(e, !now.has(e));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let (cw, ch) = ui.canvas;
    dim(ui, 190);
    let (w, h) = (330, ch - 12);
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 6, "Graphics");
    let lh = 15;
    let (rx, rw) = (x + 16, w - 32);
    let mut ry = y + 30;
    let row = st.row;
    let line = |ui: &mut Ui, k: u8, label: &str, value: &str, ry: &mut i32| {
        let lit = row == k;
        if lit {
            let rr = Rect::new(rx, *ry - 2, rw, lh);
            ui.fill(rr, argb(Ramp::UiPanel.at(Tone::Light), 45));
            ui.focus_ring(rr);
        }
        let ink = if lit { style::text_bright() } else { style::text() };
        ui.text(rx + 4, *ry + 2, label, Ink::fine(ink).shadow());
        let v: alloc::string::String = if lit { format!("< {value} >") } else { value.into() };
        ui.text_right(
            rx + rw - 6,
            *ry + 2,
            &v,
            Ink::fine(if lit { style::gold() } else { style::text_bright() }).shadow(),
        );
        *ry += lh;
    };
    line(ui, 0, "Preset", now.preset().map_or("Custom", Preset::label), &mut ry);
    line(ui, 1, "Frame rate", now.rate.label(), &mut ry);
    ry += 4;
    for (k, e) in Effect::ALL.into_iter().enumerate() {
        line(ui, 2 + k as u8, e.label(), if now.has(e) { "On" } else { "Off" }, &mut ry);
    }
    let note = "L + R + SELECT twice: each pass's cost";
    ui.text(cw / 2 - text_w(jane_art::font::Face::Fine, note) / 2, ry + 3, note, Ink::fine(style::quiet()).shadow());
    let back = Rect::new(cw / 2 - 52, ry + 18, 104, 22);
    if ui.button(wid("graphics-back", 2), back, "Back", ButtonKind::Menu, true, row == BACK) {
        ui.intent(AppIntent::Back);
    }
    (now != g).then_some(now)
}
