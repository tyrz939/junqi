//! The menus (PRESENTATION.md §3.2): pause, the save and load slots, and the list widget they
//! share. A menu is a column of rows; up and down move the light, confirm or a click picks.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};

use crate::input::UiAction;
use crate::ui::art::Mark;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, advance, line_h, text_w, wid, wrap_lines};
use crate::ui::style::{self, argb};

/// A menu's own state: which row is lit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub focus: u8,
}

/// Each open menu's own light, bottom to top (§3.1: the top layer eats input). One light shared
/// by every layer moved the pause menu's light under "Quit to the title?" as Yes and No were
/// chosen (2026-10-01); each layer keeps its own, and a layer under another is drawn with its
/// light where it was left.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuLights {
    states: Vec<MenuState>,
}

impl MenuLights {
    /// A menu opened on top, lit at `st`.
    pub fn push(&mut self, st: MenuState) {
        self.states.push(st);
    }

    /// The top menu closed: the one under it keeps its light.
    pub fn pop(&mut self) {
        self.states.pop();
    }

    pub fn clear(&mut self) {
        self.states.clear();
    }

    /// As many lights as `n` open menus: one closed or opened some other way loses or gains its
    /// light at the top.
    pub fn sync(&mut self, n: usize) {
        self.states.resize(n, MenuState::default());
    }

    /// Layer `k`'s light (0 at the bottom).
    pub fn layer(&mut self, k: usize) -> &mut MenuState {
        if self.states.len() <= k {
            self.states.resize(k + 1, MenuState::default());
        }
        &mut self.states[k]
    }
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
    /// The LAN row (P8): its words and whether it can be picked ("Open to LAN" alone; "Hosting
    /// on port 7777" greyed while hosting; nothing when joined).
    pub lan: Option<(&'a str, bool)>,
    /// Joined to another's table: the world is the host's to save.
    pub guest: bool,
}

/// Why the pause menu's Save is grey, where it is: the words under the rows and the F5 refusal.
pub const REST_TO_SAVE: &str = "Rest at a bed or fire to save";
/// A guest's Save: the world is not hers to write.
pub const HOSTS_TO_SAVE: &str = "The host's world: anyone's rest saves it there";

/// Why Save cannot be picked now; `None` when it can.
pub fn save_reason(info: &PauseInfo<'_>) -> Option<&'static str> {
    if info.guest {
        Some(HOSTS_TO_SAVE)
    } else if !info.can_save {
        Some(REST_TO_SAVE)
    } else {
        None
    }
}

/// The pause menu: Resume, Save, Load, Open to LAN, Controls, Quit to Title.
pub fn pause(ui: &mut Ui, st: &mut MenuState, info: &PauseInfo<'_>) {
    let (cw, ch) = ui.canvas;
    dim(ui, 150);
    let extra = if info.lan.is_some() { 30 } else { 0 };
    let (w, h) = (300, 268 + extra);
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, "Paused");
    let mut labels = vec!["Resume", "Save", "Load"];
    let mut enabled = vec![true, info.can_save, true];
    if let Some((l, on)) = info.lan {
        labels.push(l);
        enabled.push(on);
    }
    labels.extend(["Controls", "Quit to Title"]);
    enabled.extend([true, true]);
    let picked = rows(ui, st, "pause", Rect::new(x + 20, y + 52, w - 40, 0), 30, &labels, &enabled);
    let lan = usize::from(info.lan.is_some());
    match picked {
        Some(0) => ui.intent(AppIntent::Resume),
        Some(1) => ui.intent(AppIntent::SaveMenu),
        Some(2) => ui.intent(AppIntent::LoadMenu),
        Some(3) if lan == 1 => ui.intent(AppIntent::OpenToLan),
        Some(k) if k == 3 + lan => ui.intent(AppIntent::Controls),
        Some(k) if k == 4 + lan => ui.intent(AppIntent::ToTitle),
        _ => {}
    }
    // Why Save is grey: a lock beside the grey word, and the reason under the rows.
    let foot = y + h - 44;
    if let Some(why) = save_reason(info) {
        let row = y + 52 + 30;
        ui.mark_ink(Mark::Lock, cw / 2 + text_w(Face::Small, "Save") / 2 + 6, row + 6, style::dim(), 200);
        let tw = text_w(Face::Fine, why) + 20;
        let lx = cw / 2 - tw / 2;
        ui.mark_ink(Mark::Lock, lx, foot - 3, style::quiet(), 230);
        ui.text(lx + 20, foot, why, Ink::fine(style::quiet()).shadow());
    } else {
        let s = "A bed or a fire is in reach";
        ui.text(cw / 2 - text_w(Face::Fine, s) / 2, foot, s, Ink::fine(style::good()).shadow());
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
        ui.text(cw / 2 - text_w(Face::Fine, s) / 2, y - 16, s, Ink::fine(style::warn()).shadow());
    }
}

/// One save slot as the list shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotRow {
    /// Empty (or unreadable): nothing to load.
    pub empty: bool,
    /// Where she stood: "The Lowfields".
    pub zone: String,
    /// "Day 3 · 21:14" (the hour alone, "Day 3 · 21:00", for a save without its note).
    pub when: String,
    /// It was night: the moon beside the time, else the sun.
    pub night: bool,
    /// The story where she was, when the slot's note says: the quest she followed first and its
    /// open step ("A Letter from Julie", "Auntie Julie's house").
    pub quest: String,
    pub step: String,
    /// "34 of 40".
    pub hp: String,
    /// When it was written, in real time: "5 min ago".
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

/// What picking slot `i` does: an empty slot saves at once, a used one asks first
/// ([`AppIntent::Overwrite`]); a load loads.
pub fn slot_intent(mode: SlotMode, i: u8, row: &SlotRow) -> AppIntent {
    match (mode, row.empty) {
        (SlotMode::Save, true) => AppIntent::Save(i),
        (SlotMode::Save, false) => AppIntent::Overwrite(i),
        (SlotMode::Load, _) => AppIntent::Load(i),
    }
}

/// What the overwrite question says is lost: "The Lowfields, Day 3 · 21:14, saved 5 min ago.
/// Saving here replaces it."
pub fn overwrite_detail(row: &SlotRow) -> String {
    let mut s = format!("{}, {}", row.zone, row.when);
    if !row.age.is_empty() {
        s.push_str(", saved ");
        s.push_str(&row.age);
    }
    s.push_str(". Saving here replaces it.");
    s
}

/// `s` cut to `w` px in `face`, with "..." where it was cut.
pub fn fit(face: Face, s: &str, w: i32) -> String {
    let cols = (w / advance(face)).max(0) as usize;
    if s.chars().count() <= cols {
        return s.to_owned();
    }
    let keep = cols.saturating_sub(3);
    let mut out: String = s.chars().take(keep).collect();
    // Cut at a word's end when one is near, not through a word.
    if s.chars().nth(keep).is_some_and(|c| c != ' ')
        && let Some(sp) = out.rfind(' ').filter(|&sp| out.len() - sp <= 8)
    {
        out.truncate(sp);
    }
    out.truncate(out.trim_end_matches([' ', ',', ';', ':', '.', '-']).len());
    out.push_str("...");
    out
}

/// The picker: one card a slot, then a line saying what the lit one does, then Back. Saving
/// into an empty slot is one pick; over a used one asks first (the app's confirmation).
pub fn slots(ui: &mut Ui, st: &mut MenuState, mode: SlotMode, rows_in: &[SlotRow]) {
    let (cw, ch) = ui.canvas;
    dim(ui, 170);
    let (w, row_h) = (480.min(cw - 24), 68);
    let n = rows_in.len();
    let h = 56 + n as i32 * row_h + 62;
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, if mode == SlotMode::Save { "Save" } else { "Load" });
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
        slot_card(ui, rr, i as u8, s, on, lit);
        let clicked = over && ui.input.released;
        let confirmed = lit && ui.interactive && ui.input.has(UiAction::Confirm);
        if on && (clicked || confirmed) {
            ui.intent(slot_intent(mode, i as u8, s));
        }
        ui.claim(rr);
    }
    // What the lit card does, said before it is done.
    let hint = match rows_in.get(usize::from(st.focus)) {
        Some(s) if mode == SlotMode::Save && s.empty => "An empty slot: saves here at once",
        Some(_) if mode == SlotMode::Save => "Holds a save: asks before writing over it",
        Some(_) => "Loads the world as it was saved",
        None => "",
    };
    let hy = y + 52 + n as i32 * row_h + 2;
    let hint = fit(Face::Fine, hint, w - 40);
    ui.text(cw / 2 - text_w(Face::Fine, &hint) / 2, hy, &hint, Ink::fine(style::quiet()).shadow());
    let back = Rect::new(x + w / 2 - 70, y + h - 36, 140, 26);
    if back_button(ui, st, back, n as u8) {
        ui.intent(AppIntent::Back);
    }
}

/// One slot's card in `rr`: a numbered plate on the left, then where she was and when, the
/// story's step, her health and how long ago in real time.
fn slot_card(ui: &mut Ui, rr: Rect, i: u8, s: &SlotRow, on: bool, lit: bool) {
    let (x, y, w, h) = (i32::from(rr.x), i32::from(rr.y), i32::from(rr.w), i32::from(rr.h));
    ui.well(rr, lit);
    if lit {
        ui.fill(rr.inset(2), argb(Ramp::UiPanel.at(Tone::Light), 56));
        ui.rule(x + 52, x + w - 8, y + 2, style::gold_deep());
    }
    // The plate: a dark tablet with gold rules, the slot's number cut into it.
    let (px, py, pw, ph) = (x + 6, y + 6, 40, h - 12);
    ui.fill(Rect::new(px, py, pw, ph), argb(Ramp::UiSlot.at(Tone::Deep), 220));
    ui.fill(Rect::new(px + 1, py + 1, pw - 2, ph / 2), argb(Ramp::UiPanel.at(Tone::Base), 90));
    let rim = if s.empty { style::dim() } else { style::gold_deep() };
    ui.rule(px + 2, px + pw - 2, py, rim);
    ui.rule(px + 2, px + pw - 2, py + ph - 1, rim);
    let num = [b'1' + i];
    let num = std::str::from_utf8(&num).unwrap_or("?");
    let ink = if s.empty {
        style::dim()
    } else if lit {
        Ramp::UiGold.at(Tone::High)
    } else {
        style::gold()
    };
    ui.text_in(Rect::new(px, py + 1, pw, ph), num, Ink::head(ink).shadow());
    if s.latest {
        ui.mark(Mark::Diamond, px + pw / 2 - 3, py + ph - 4, 255);
    }
    if lit {
        ui.focus_ring(rr);
    }
    let (tx, right) = (x + 56, x + w - 10);
    if s.empty {
        let ty = y + (h - line_h(Face::Small)) / 2;
        ui.text(tx, ty, "Empty", Ink::small(if on { style::quiet() } else { style::dim() }).shadow());
        return;
    }
    let bright = if on { style::text_bright() } else { style::dim() };
    // Line one: the place, and the hour with the sun or the moon.
    let ww = text_w(Face::Fine, &s.when);
    ui.text_right(right, y + 8, &s.when, Ink::fine(if lit { style::gold() } else { style::text() }).shadow());
    ui.mark(if s.night { Mark::Moon } else { Mark::Sun }, right - ww - 20, y + 3, 255);
    let place = fit(Face::Small, &s.zone, right - ww - 28 - tx);
    ui.text(tx, y + 5, &place, Ink::small(bright).shadow());
    // Line two: the story, the quest in gold and its open step after it.
    let sy = y + 26;
    if !s.quest.is_empty() {
        let q = fit(Face::Fine, &s.quest, right - tx);
        let after = ui.text(tx, sy, &q, Ink::fine(style::gold_deep()).shadow());
        let left = right - after - text_w(Face::Fine, " · ");
        if !s.step.is_empty() && left > advance(Face::Fine) * 6 {
            let step = fit(Face::Fine, &s.step, left);
            let x2 = ui.text(after, sy, " · ", Ink::fine(style::dim()).shadow());
            ui.text(x2, sy, &step, Ink::fine(style::text()).shadow());
        }
    }
    // Line three: her health, the real time since, and the latest mark.
    let ly = y + h - 17;
    if !s.hp.is_empty() {
        ui.mark(Mark::Heart, tx - 2, ly - 3, 230);
        ui.text(tx + 16, ly, &s.hp, Ink::fine(style::quiet()).shadow());
    }
    ui.text_right(right, ly, &s.age, Ink::fine(style::quiet()).shadow());
    if s.latest {
        let l = "latest";
        let lx = right - text_w(Face::Fine, &s.age) - text_w(Face::Fine, l) - 14;
        ui.text(lx, ly, l, Ink::fine(style::gold_deep()).shadow());
    }
}

/// The Back row at the foot of a list, lit as row `index`.
pub fn back_button(ui: &mut Ui, st: &mut MenuState, r: Rect, index: u8) -> bool {
    if ui.hover(r) && ui.input.pointer.is_some() {
        st.focus = index;
    }
    ui.button(wid("back", u32::from(index)), r, "Back", ButtonKind::Menu, true, st.focus == index)
}

/// A question over everything: its words, a line or two more, and the two answers.
#[derive(Clone, Copy, Debug)]
pub struct Ask<'a> {
    pub question: &'a str,
    /// Wrapped under the question; empty for none.
    pub detail: &'a str,
    pub yes: &'a str,
    pub no: &'a str,
}

/// A yes-or-no over everything: "Destroy the brass key?".
pub fn confirm(ui: &mut Ui, st: &mut MenuState, question: &str) -> Option<bool> {
    ask(ui, st, &Ask { question, detail: "", yes: "Yes", no: "No" })
}

/// [`confirm`] with more to say and its own answers: "Save over slot 2?", what is there now,
/// "Save over" and "Keep it". The app draws it as the top layer, so it owns every key.
pub fn ask(ui: &mut Ui, st: &mut MenuState, a: &Ask<'_>) -> Option<bool> {
    let (cw, ch) = ui.canvas;
    dim(ui, 120);
    let least = if a.detail.is_empty() { 260 } else { 340 };
    let w = (text_w(Face::Small, a.question) + 60).max(least).min(cw - 32);
    let cols = ((w - 40) / advance(Face::Fine)).max(1) as usize;
    let lines = if a.detail.is_empty() { 0 } else { wrap_lines(a.detail, cols).count() as i32 };
    let extra = if lines > 0 { lines * line_h(Face::Fine) + 8 } else { 0 };
    let bw = (text_w(Face::Small, a.yes).max(text_w(Face::Small, a.no)) + 48).max(120);
    let h = 100 + extra;
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    ui.text_in(Rect::new(x, y + 14, w, 20), a.question, Ink::small(style::text_bright()).shadow());
    if lines > 0 {
        let dr = Rect::new(x + 20, y + 40, w - 40, lines * line_h(Face::Fine));
        ui.wrapped(dr, a.detail, Ink::fine(style::quiet()).shadow());
    }
    let at = Rect::new(x + w / 2 - bw / 2, y + 42 + extra, bw, 0);
    let picked = rows(ui, st, "confirm", at, 26, &[a.yes, a.no], &[true, true]);
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
    fn a_confirmation_owns_the_keys_and_the_menu_under_it_keeps_its_light() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut lights = MenuLights::default();
        lights.push(MenuState::default());
        lights.push(MenuState { focus: 1 });
        let info = PauseInfo::default();
        for (t, a) in [UiAction::Up, UiAction::Down, UiAction::Up, UiAction::Confirm].into_iter().enumerate() {
            ui.begin(UiInput { actions: vec![a], ..UiInput::default() }, t as u32, (768, 432));
            // The app's loop: every layer drawn, only the top one interactive.
            ui.interactive = false;
            pause(&mut ui, lights.layer(0), &info);
            ui.interactive = true;
            let picked = confirm(&mut ui, lights.layer(1), "Quit to the title?");
            assert_eq!(lights.layer(0).focus, 0, "Resume stays lit under the question ({a:?})");
            if a == UiAction::Confirm {
                assert_eq!(picked, Some(true), "Up from No is Yes");
            }
            assert!(!ui.out.iter().any(|o| matches!(o, UiOut::Intent(_))), "the pause menu under it picks nothing");
        }
        assert_eq!(lights.layer(1).focus, 0);
        lights.pop();
        assert_eq!(lights.layer(0).focus, 0, "back to the pause menu as it was");
    }

    fn used() -> SlotRow {
        SlotRow {
            zone: "The Lowfields".into(),
            when: "Day 3 · 21:14".into(),
            age: "5 min ago".into(),
            ..SlotRow::default()
        }
    }

    #[test]
    fn saving_over_a_used_slot_asks_first_and_the_question_owns_the_keys() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let rows = [used(), SlotRow { empty: true, ..SlotRow::default() }, used()];
        let press = |a| UiInput { actions: vec![a], ..UiInput::default() };
        // An empty slot saves at once; a used one asks; a load loads.
        let mut st = MenuState { focus: 1 };
        ui.begin(press(UiAction::Confirm), 1, (768, 432));
        slots(&mut ui, &mut st, SlotMode::Save, &rows);
        assert!(ui.out.contains(&UiOut::Intent(AppIntent::Save(1))));
        let mut lights = MenuLights::default();
        lights.push(MenuState { focus: 0 });
        ui.begin(press(UiAction::Down), 2, (768, 432));
        slots(&mut ui, lights.layer(0), SlotMode::Save, &rows);
        ui.begin(press(UiAction::Down), 3, (768, 432));
        slots(&mut ui, lights.layer(0), SlotMode::Save, &rows);
        assert_eq!(lights.layer(0).focus, 2, "the keys walk the cards, the empty one too");
        ui.begin(press(UiAction::Confirm), 4, (768, 432));
        slots(&mut ui, lights.layer(0), SlotMode::Save, &rows);
        assert!(ui.out.contains(&UiOut::Intent(AppIntent::Overwrite(2))), "a used slot asks");
        assert!(!ui.out.contains(&UiOut::Intent(AppIntent::Save(2))), "and does not save yet");
        assert_eq!(slot_intent(SlotMode::Load, 2, &rows[2]), AppIntent::Load(2));
        // The app opens the question on top with "Keep it" lit; it owns every key.
        lights.push(MenuState { focus: 1 });
        let detail = overwrite_detail(&rows[2]);
        assert!(detail.contains("The Lowfields") && detail.contains("5 min ago"), "{detail}");
        let ask_ = Ask { question: "Save over slot 3?", detail: &detail, yes: "Save over", no: "Keep it" };
        let mut answer = None;
        for (t, a) in [UiAction::Up, UiAction::Down, UiAction::Up, UiAction::Confirm].into_iter().enumerate() {
            ui.begin(press(a), 10 + t as u32, (768, 432));
            ui.interactive = false;
            slots(&mut ui, lights.layer(0), SlotMode::Save, &rows);
            ui.interactive = true;
            answer = ask(&mut ui, lights.layer(1), &ask_);
            assert_eq!(lights.layer(0).focus, 2, "the card under the question keeps its light ({a:?})");
            assert!(!ui.out.iter().any(|o| matches!(o, UiOut::Intent(_))), "the picker under it picks nothing");
        }
        assert_eq!(answer, Some(true), "Up, Down, Up from Keep it is Save over");
        // Confirm at once on the question as it opens keeps the save.
        let mut st = MenuState { focus: 1 };
        ui.begin(press(UiAction::Confirm), 20, (768, 432));
        assert_eq!(ask(&mut ui, &mut st, &ask_), Some(false));
    }

    #[test]
    fn grey_save_says_why_and_where() {
        let info = PauseInfo::default();
        assert_eq!(save_reason(&info), Some("Rest at a bed or fire to save"));
        assert_eq!(save_reason(&PauseInfo { can_save: true, ..PauseInfo::default() }), None);
        assert_eq!(save_reason(&PauseInfo { guest: true, ..PauseInfo::default() }), Some(HOSTS_TO_SAVE));
        // Grey, not hidden: the light steps over it, and it stays on the menu.
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = MenuState::default();
        ui.begin(UiInput { actions: vec![UiAction::Down], ..UiInput::default() }, 1, (768, 432));
        pause(&mut ui, &mut st, &info);
        assert_eq!(st.focus, 2, "Down from Resume skips the grey Save to Load");
        assert!(text_w(Face::Fine, REST_TO_SAVE) + 20 < 300 - 16, "the reason fits the pause plate");
    }

    #[test]
    fn long_words_are_cut_to_fit() {
        assert_eq!(fit(Face::Fine, "short", 80), "short");
        let cut = fit(Face::Fine, "Auntie Julie's house, at the end of the station road", 8 * 22);
        assert_eq!(cut, "Auntie Julie's...", "cut at a word's end");
        assert!(text_w(Face::Fine, &cut) <= 8 * 22);
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
