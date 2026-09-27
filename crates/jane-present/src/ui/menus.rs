//! The menus (PRESENTATION.md §3.2): pause, the save and load slots, and the list widget they
//! share. A menu is a column of rows; up and down move the light, confirm or a click picks.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};

use crate::input::UiAction;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, text_w, wid};
use crate::ui::style::{self, argb};

/// A menu's own state: which row is lit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub focus: u8,
}

impl MenuState {
    /// Moves the light by this frame's up and down, skipping disabled rows.
    pub fn nav(&mut self, ui: &Ui, enabled: &[bool]) {
        if !ui.interactive || enabled.is_empty() {
            return;
        }
        let n = enabled.len() as i32;
        for a in &ui.input.actions {
            let d = match a {
                UiAction::Up => -1,
                UiAction::Down => 1,
                _ => continue,
            };
            let mut f = i32::from(self.focus);
            for _ in 0..n {
                f = (f + d).rem_euclid(n);
                if enabled[f as usize] {
                    break;
                }
            }
            self.focus = f as u8;
        }
        if !enabled.get(usize::from(self.focus)).copied().unwrap_or(false) {
            self.focus = enabled.iter().position(|&e| e).unwrap_or(0) as u8;
        }
    }
}

/// Dims the world under a menu.
pub fn dim(ui: &mut Ui, a: u8) {
    let (cw, ch) = ui.canvas;
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ramp::UiSlot.at(Tone::Deep), a));
}

/// A heading in the Head face with gold rules either side, centred on `cx`.
pub fn heading(ui: &mut Ui, cx: i32, y: i32, s: &str) {
    let w = text_w(Face::Head, s);
    ui.text(cx - w / 2, y, s, Ink::head(style::gold()).shadow());
    ui.rule(cx - w / 2 - 44, cx - w / 2 - 8, y + 13, style::gold_deep());
    ui.rule(cx + w / 2 + 8, cx + w / 2 + 44, y + 13, style::gold_deep());
}

/// A column of menu rows in `r`, `row_h` each; returns the row picked this frame.
pub fn rows(
    ui: &mut Ui,
    st: &mut MenuState,
    name: &str,
    r: Rect,
    row_h: i32,
    labels: &[&str],
    enabled: &[bool],
) -> Option<usize> {
    st.nav(ui, enabled);
    let mut picked = None;
    for (i, l) in labels.iter().enumerate() {
        let rr = Rect::new(i32::from(r.x), i32::from(r.y) + i as i32 * row_h, i32::from(r.w), row_h - 2);
        let on = enabled.get(i).copied().unwrap_or(true);
        if ui.hover(rr) && on && ui.input.pointer.is_some() {
            st.focus = i as u8;
        }
        if ui.button(wid(name, i as u32), rr, l, ButtonKind::Menu, on, st.focus == i as u8) {
            picked = Some(i);
        }
    }
    picked
}

/// What the pause menu shows beside its rows.
#[derive(Clone, Debug, Default)]
pub struct PauseInfo<'a> {
    /// Near a bed or a fire.
    pub can_save: bool,
    /// "Day 2, 21:14", "The Lowfields".
    pub when: &'a str,
    pub zone: &'a str,
    /// Anyone else sitting down: the world does not stop.
    pub company: bool,
}

/// The pause menu: Resume, Save, Load, Controls, Quit to Title.
pub fn pause(ui: &mut Ui, st: &mut MenuState, info: &PauseInfo<'_>) {
    let (cw, ch) = ui.canvas;
    dim(ui, 150);
    let (w, h) = (300, 268);
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, "Paused");
    let labels = ["Resume", "Save", "Load", "Controls", "Quit to Title"];
    let enabled = [true, info.can_save, true, true, true];
    let picked = rows(ui, st, "pause", Rect::new(x + 20, y + 52, w - 40, 0), 30, &labels, &enabled);
    match picked {
        Some(0) => ui.intent(AppIntent::Resume),
        Some(1) => ui.intent(AppIntent::SaveMenu),
        Some(2) => ui.intent(AppIntent::LoadMenu),
        Some(3) => ui.intent(AppIntent::Controls),
        Some(4) => ui.intent(AppIntent::ToTitle),
        _ => {}
    }
    // Why Save is grey, under the rows while it is.
    let foot = y + h - 44;
    if info.can_save {
        let s = "A bed or a fire is in reach";
        ui.text(cw / 2 - text_w(Face::Fine, s) / 2, foot, s, Ink::fine(style::good()).shadow());
    } else {
        let s = "Save by a bed or a fire";
        ui.text(cw / 2 - text_w(Face::Fine, s) / 2, foot, s, Ink::fine(style::quiet()).shadow());
    }
    let mut line = String::with_capacity(48);
    line.push_str(info.zone);
    if !info.when.is_empty() {
        line.push_str(" · ");
        line.push_str(info.when);
    }
    ui.text(cw / 2 - text_w(Face::Fine, &line) / 2, foot + 16, &line, Ink::fine(style::dim()).shadow());
    if info.company {
        let s = "The world does not stop with company";
        ui.text(cw / 2 - text_w(Face::Fine, s) / 2, y + h + 6, s, Ink::fine(style::warn()).shadow());
    }
}

/// One save slot as the list shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotRow {
    /// Empty (or unreadable): nothing to load.
    pub empty: bool,
    /// "The Lowfields", "Day 3, 21:00", "HP 34 of 40", "2 min ago".
    pub zone: String,
    pub when: String,
    pub hp: String,
    pub age: String,
    /// The last one written: what Continue loads.
    pub latest: bool,
}

/// Save or load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotMode {
    Save,
    Load,
}

/// The slot list: three rows and Back. Saving over a slot, and loading, are one pick.
pub fn slots(ui: &mut Ui, st: &mut MenuState, mode: SlotMode, rows_in: &[SlotRow]) {
    let (cw, ch) = ui.canvas;
    dim(ui, 170);
    let (w, row_h) = (420, 54);
    let h = 70 + rows_in.len() as i32 * row_h + 44;
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, if mode == SlotMode::Save { "Save" } else { "Load" });
    let n = rows_in.len();
    let mut enabled: Vec<bool> = rows_in.iter().map(|s| mode == SlotMode::Save || !s.empty).collect();
    enabled.push(true);
    st.nav(ui, &enabled);
    for (i, s) in rows_in.iter().enumerate() {
        let rr = Rect::new(x + 16, y + 52 + i as i32 * row_h, w - 32, row_h - 6);
        let on = enabled[i];
        let over = on && ui.hover(rr);
        if over && ui.input.pointer.is_some() {
            st.focus = i as u8;
        }
        let lit = st.focus == i as u8;
        ui.well(rr, lit);
        if lit {
            ui.fill(rr.inset(2), argb(Ramp::UiPanel.at(Tone::Light), 60));
            ui.focus_ring(rr);
        }
        let (rx, ry) = (i32::from(rr.x), i32::from(rr.y));
        let mut num = String::from("Slot ");
        num.push(char::from(b'1' + i as u8));
        ui.text(rx + 10, ry + 6, &num, Ink::fine(if lit { style::gold() } else { style::quiet() }).shadow());
        if s.latest {
            ui.text(rx + 10, ry + 30, "latest", Ink::fine(style::gold_deep()).shadow());
        }
        if s.empty {
            ui.text(rx + 70, ry + 14, "Empty", Ink::small(style::dim()).shadow());
        } else {
            ui.text(
                rx + 70,
                ry + 5,
                &s.zone,
                Ink::small(if on { style::text_bright() } else { style::dim() }).shadow(),
            );
            ui.text(rx + 70, ry + 27, &s.when, Ink::fine(style::text()).shadow());
            ui.text_right(rx + i32::from(rr.w) - 10, ry + 8, &s.hp, Ink::fine(style::quiet()).shadow());
            ui.text_right(rx + i32::from(rr.w) - 10, ry + 27, &s.age, Ink::fine(style::quiet()).shadow());
        }
        let clicked = over && ui.input.released;
        let confirmed = lit && ui.interactive && ui.input.has(UiAction::Confirm);
        if on && (clicked || confirmed) {
            ui.intent(if mode == SlotMode::Save { AppIntent::Save(i as u8) } else { AppIntent::Load(i as u8) });
        }
        ui.claim(rr);
    }
    let back = Rect::new(x + w / 2 - 70, y + h - 38, 140, 26);
    if back_button(ui, st, back, n as u8) {
        ui.intent(AppIntent::Back);
    }
}

/// The Back row at the foot of a list, lit as row `index`.
pub fn back_button(ui: &mut Ui, st: &mut MenuState, r: Rect, index: u8) -> bool {
    if ui.hover(r) && ui.input.pointer.is_some() {
        st.focus = index;
    }
    ui.button(wid("back", u32::from(index)), r, "Back", ButtonKind::Menu, true, st.focus == index)
}

/// A yes-or-no over everything: "Destroy the brass key?".
pub fn confirm(ui: &mut Ui, st: &mut MenuState, question: &str) -> Option<bool> {
    let (cw, ch) = ui.canvas;
    dim(ui, 120);
    let w = (text_w(Face::Small, question) + 60).max(260);
    let h = 100;
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    ui.text_in(Rect::new(x, y + 14, w, 20), question, Ink::small(style::text_bright()).shadow());
    let picked = rows(ui, st, "confirm", Rect::new(x + w / 2 - 60, y + 42, 120, 0), 26, &["Yes", "No"], &[true, true]);
    if ui.interactive && ui.input.has(UiAction::Cancel) {
        return Some(false);
    }
    picked.map(|i| i == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::core::UiInput;
    use crate::ui::{UiArt, UiOut};

    #[test]
    fn the_light_skips_a_disabled_row() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = MenuState::default();
        ui.begin(UiInput { actions: vec![UiAction::Down], ..UiInput::default() }, 1, (768, 432));
        st.nav(&ui, &[true, false, true]);
        assert_eq!(st.focus, 2);
        ui.begin(UiInput { actions: vec![UiAction::Down], ..UiInput::default() }, 2, (768, 432));
        st.nav(&ui, &[true, false, true]);
        assert_eq!(st.focus, 0, "wraps");
    }

    #[test]
    fn pause_saves_only_in_reach_of_rest() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = MenuState { focus: 1 };
        let go = || UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() };
        ui.begin(go(), 1, (768, 432));
        pause(&mut ui, &mut st, &PauseInfo { can_save: false, ..PauseInfo::default() });
        assert!(!ui.out.contains(&UiOut::Intent(AppIntent::SaveMenu)));
        let mut st = MenuState { focus: 1 };
        ui.begin(go(), 2, (768, 432));
        pause(&mut ui, &mut st, &PauseInfo { can_save: true, ..PauseInfo::default() });
        assert!(ui.out.contains(&UiOut::Intent(AppIntent::SaveMenu)));
    }
}
