//! The Controls screen (PRESENTATION.md §3.2, §4): one row per action with its two keys, its
//! mouse button and its pad input. Press-to-rebind: pick a cell, press what it should be.
//! Conflicts are shown (in red, with who else has it), never refused. Reset puts every row back
//! to `data/bindings.json`. The aim assist, the backend and the volumes are rows beneath; the app
//! keeps all of it in `config.json`.
//!
//! A second page, Display (PRESENTATION.md §1.3), has a toggle for each `Features` row the tier
//! draws (`Features::rows`): a switch flips, a count steps down by halves and back; each says
//! whether it shows now or on the next start, and the backend is there too (next start).

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::input::AssistProfile;

use crate::audio::Volumes;
use crate::frame::{Features, Tier};
use crate::input::{
    ACTIONS, Action, BINDINGS, Bindings, MouseButton, PadInput, UiAction, action_label, key_name, mouse_name, pad_name,
    sc,
};
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, line_h, wid};
use crate::ui::menus::{dim, heading};
use crate::ui::style::{self, argb};

/// Columns: key, key, mouse, pad.
pub const COLS: [&str; 4] = ["Key", "Key", "Mouse", "Pad"];
const ROW_H: i32 = 17;
/// The volume row's height, with its labels above it.
const VOLUME_ROW_H: i32 = 34;

/// The screen's own state.
#[derive(Clone, Debug, Default)]
pub struct ControlsState {
    pub row: usize,
    pub col: u8,
    /// A cell waiting for its press.
    pub capture: Option<(usize, u8)>,
    pub scroll: usize,
    /// The row the keys light below the table: 0 the table, 1 assist, 2 backend, 3 volume,
    /// 4 reset, 5 back.
    pub foot: u8,
    /// The volume the keys turn on the volume row: 0 master, 1 music, 2 effects.
    pub vol: u8,
    /// 0 the bindings, 1 Display (the `Features` rows).
    pub page: u8,
    /// The row the keys light on the Display page: the tier's rows, then the backend, then Back.
    pub drow: usize,
}

/// What the screen shows beside the bindings.
#[derive(Clone, Copy, Debug)]
pub struct ControlsInfo<'a> {
    /// `None` follows the device.
    pub assist: Option<AssistProfile>,
    /// `auto`, `soft` or `wgpu` (takes effect on the next start).
    pub backend: &'a str,
    pub volumes: Volumes,
    /// The `Features` rows in force, and the tier drawing them.
    pub rows: Features,
    pub tier: Tier,
}

/// What the player changed this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlsOut {
    /// The bindings changed: save them.
    pub bindings: bool,
    pub assist: Option<Option<AssistProfile>>,
    pub backend: Option<&'static str>,
    /// The volumes changed: hear them now and save them.
    pub volumes: Option<Volumes>,
    /// A `Features` row was turned: the rows now, and its key (to keep in `config.json`).
    pub rows: Option<(Features, &'static str)>,
}

/// The Display page's row height.
const DISPLAY_ROW_H: i32 = 20;

/// A row's value as the page shows it.
fn row_text(rows: &Features, key: &str) -> String {
    match rows.get(key).as_deref() {
        Some("on") => "On".to_owned(),
        Some("off") => "Off".to_owned(),
        Some("0") if key == "shadows" => "Off".to_owned(),
        Some(n) => n.to_owned(),
        None => String::new(),
    }
}

/// The Display page: a toggle per row the tier draws, the backend, Back.
fn display(ui: &mut Ui, st: &mut ControlsState, info: ControlsInfo<'_>, r: Rect, out: &mut ControlsOut) {
    let (x, y, w) = (i32::from(r.x), i32::from(r.y), i32::from(r.w));
    let rows: Vec<_> = Features::rows(info.tier).collect();
    let n = rows.len();
    // Keys: up and down the rows, then the backend, then Back; left or right turns one, and
    // confirm presses the lit button (the row's own, or Back).
    let turn = |st: &ControlsState, out: &mut ControlsOut, rows_now: &mut Features| {
        if let Some(row) = rows.get(st.drow) {
            rows_now.cycle(info.tier, row.key);
            out.rows = Some((*rows_now, row.key));
        }
    };
    let mut now = info.rows;
    if ui.interactive {
        for a in ui.input.actions.clone() {
            match a {
                UiAction::Up => st.drow = st.drow.saturating_sub(1),
                UiAction::Down => st.drow = (st.drow + 1).min(n + 1),
                UiAction::Left | UiAction::Right if st.drow < n => turn(st, out, &mut now),
                _ => {}
            }
        }
    }
    let tier = match info.tier {
        Tier::T0 => "T0, drawn by soft",
        Tier::T1 => "T1, drawn by gl2",
        Tier::T2 => "T2, drawn by wgpu",
    };
    let ty = y + 46;
    ui.text(x + 20, ty, tier, Ink::fine(style::gold()).shadow());
    // One column where the rows fit above the backend and Back, else two side by side (the
    // 640 x 360 canvas): each row its label and its button; one that shows only after a
    // restart is marked, and the mark said in the corner.
    let h = i32::from(r.h);
    let room = (y + h - 76) - (ty + 16);
    let cols = if n as i32 * DISPLAY_ROW_H <= room { 1 } else { 2 };
    let per = n.div_ceil(cols);
    let col_w = (w - 24) / cols as i32;
    let bw = if cols == 1 { 110 } else { 76 };
    for (k, row) in rows.iter().enumerate() {
        let (c, i) = ((k / per) as i32, (k % per) as i32);
        let (cx0, ry) = (x + 12 + c * col_w, ty + 16 + i * DISPLAY_ROW_H);
        let lit = st.drow == k;
        if i % 2 == 1 {
            ui.fill(Rect::new(cx0, ry - 2, col_w - 4, DISPLAY_ROW_H), argb(style::INK, 40));
        }
        ui.text(
            cx0 + 8,
            ry + 3,
            row.label,
            Ink::small(if lit { style::text_bright() } else { style::text() }).shadow(),
        );
        let bx = if cols == 1 { x + 200 } else { cx0 + col_w - 12 - bw };
        let br = Rect::new(bx, ry, bw, DISPLAY_ROW_H - 3);
        let text = row_text(&now, row.key);
        if ui.button(wid("feature", k as u32), br, &text, ButtonKind::Tab { on: lit }, true, lit) {
            st.drow = k;
            turn(st, out, &mut now);
        }
        if !row.live {
            ui.text(bx + bw + 3, ry + 3, "*", Ink::fine(style::quiet()).shadow());
        }
    }
    if rows.iter().any(|r| !r.live) {
        ui.text_right(x + w - 20, ty, "* next start", Ink::fine(style::quiet()).shadow());
    }
    let by = ty + 16 + per as i32 * DISPLAY_ROW_H + 8;
    ui.text(
        x + 20,
        by + 3,
        "Backend",
        Ink::small(if st.drow == n { style::text_bright() } else { style::text() }).shadow(),
    );
    for (i, label) in ["auto", "soft", "gl2", "wgpu"].iter().enumerate() {
        let br = Rect::new(x + 200 + i as i32 * 84, by, 78, 22);
        if ui.button(
            wid("display-backend", i as u32),
            br,
            label,
            ButtonKind::Tab { on: info.backend == *label },
            true,
            false,
        ) {
            out.backend = Some(label);
        }
    }
    ui.text(x + 200 + 4 * 84 + 6, by + 6, "next start", Ink::fine(style::quiet()).shadow());
    let back = Rect::new(x + w / 2 - 80, by + 34, 160, 24);
    if ui.button(wid("controls-back", 1), back, "Back", ButtonKind::Menu, true, st.drow == n + 1) {
        ui.intent(AppIntent::Back);
    }
}

fn cell_text(b: &Bindings, a: Action, col: u8) -> &'static str {
    let Some(r) = b.row(a) else { return "" };
    match col {
        0 | 1 => match r.keys[usize::from(col)] {
            0 => "",
            k => key_name(k),
        },
        2 => r.mouse.map_or("", mouse_name),
        _ => r.pad.map_or("", pad_name),
    }
}

/// Sets `(a, col)` from this frame's press, if there was one: a key for the key columns, a
/// button for the mouse, a pad input for the pad. Backspace or Delete clears the cell.
fn capture(ui: &Ui, b: &mut Bindings, a: Action, col: u8) -> Option<bool> {
    let Some(r) = b.rows.iter_mut().find(|r| r.action == a) else { return Some(false) };
    let keys = ui.input.keys;
    if keys.has(sc::BACKSPACE) || keys.has(76) {
        match col {
            0 | 1 => r.keys[usize::from(col)] = 0,
            2 => r.mouse = None,
            _ => r.pad = None,
        }
        return Some(true);
    }
    match col {
        0 | 1 => {
            let k = keys.iter().find(|&k| k != sc::ESCAPE)?;
            r.keys[usize::from(col)] = k;
            Some(true)
        }
        2 => {
            let m = if ui.input.pressed {
                MouseButton::Left
            } else if ui.input.right_pressed {
                MouseButton::Right
            } else if ui.input.middle_pressed {
                MouseButton::Middle
            } else {
                return None;
            };
            r.mouse = Some(m);
            Some(true)
        }
        _ => {
            let p = ui.input.pad_pressed;
            let got = if p.lt {
                PadInput::LeftTrigger
            } else if p.rt {
                PadInput::RightTrigger
            } else if p.buttons != 0 {
                PadInput::Button(p.buttons.trailing_zeros() as u8)
            } else {
                return None;
            };
            r.pad = Some(got);
            Some(true)
        }
    }
}

/// Draws the Controls screen over whatever is under it.
pub fn draw(ui: &mut Ui, st: &mut ControlsState, b: &mut Bindings, info: ControlsInfo<'_>) -> ControlsOut {
    let mut out = ControlsOut::default();
    let (cw, ch) = ui.canvas;
    dim(ui, 190);
    let (w, h) = (620.min(cw - 16), (ch - 16).min(416));
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 10, "Controls");
    // The pages: the bindings and Display; the tab keys go between them.
    let capturing = st.capture.is_some();
    if ui.interactive && !capturing {
        for a in ui.input.actions.clone() {
            if matches!(a, UiAction::TabLeft | UiAction::TabRight) {
                st.page ^= 1;
            }
        }
    }
    // Right of the heading's rule: the keys' page, then Display.
    for (i, (label, tx, tw)) in [("Keys", 170, 60), ("Display", 104, 92)].into_iter().enumerate() {
        let tr = Rect::new(x + w - tx, y + 12, tw, 20);
        if ui.button(
            wid("controls-page", i as u32),
            tr,
            label,
            ButtonKind::Tab { on: st.page == i as u8 },
            !capturing,
            false,
        ) {
            st.page = i as u8;
        }
    }
    if st.page == 1 {
        display(ui, st, info, r, &mut out);
        return out;
    }
    // The table.
    let col_x = [x + 200, x + 290, x + 380, x + 500];
    let col_w = [84, 84, 114, 80];
    let ty = y + 46;
    for (i, c) in COLS.iter().enumerate() {
        ui.text(col_x[i] + 4, ty, c, Ink::fine(style::gold()).shadow());
    }
    ui.text(x + 20, ty, "Action", Ink::fine(style::gold()).shadow());
    let visible = ((h - 170 - VOLUME_ROW_H) / ROW_H).max(4) as usize;
    let n = ACTIONS.len();
    // Keys: rows and columns, confirm to capture; the wheel scrolls.
    if ui.interactive && !capturing {
        for a in ui.input.actions.clone() {
            match a {
                UiAction::Up if st.foot == 0 => st.row = st.row.saturating_sub(1),
                UiAction::Up => st.foot -= 1,
                UiAction::Down if st.foot == 0 && st.row + 1 < n => st.row += 1,
                UiAction::Down => st.foot = (st.foot + 1).min(5),
                UiAction::Confirm if st.foot == 3 => st.vol = (st.vol + 1) % 3,
                UiAction::Left if st.foot == 0 => st.col = st.col.saturating_sub(1),
                UiAction::Right if st.foot == 0 => st.col = (st.col + 1).min(3),
                UiAction::Confirm if st.foot == 0 => st.capture = Some((st.row, st.col)),
                _ => {}
            }
        }
        if ui.input.wheel != 0 {
            st.scroll = (st.scroll as i32 - ui.input.wheel * 2).clamp(0, (n - visible) as i32) as usize;
        }
    }
    if st.row < st.scroll {
        st.scroll = st.row;
    }
    if st.row >= st.scroll + visible {
        st.scroll = st.row + 1 - visible;
    }
    for (k, (_, action, _)) in ACTIONS.iter().enumerate().skip(st.scroll).take(visible) {
        let ry = ty + 16 + (k - st.scroll) as i32 * ROW_H;
        let lit_row = st.foot == 0 && st.row == k;
        if k % 2 == 1 {
            ui.fill(Rect::new(x + 12, ry - 1, w - 24, ROW_H), argb(style::INK, 40));
        }
        if lit_row {
            ui.fill(Rect::new(x + 12, ry - 1, w - 24, ROW_H), argb(Ramp::UiPanel.at(Tone::Light), 45));
        }
        ui.text(
            x + 20,
            ry + 2,
            action_label(*action),
            Ink::fine(if lit_row { style::text_bright() } else { style::text() }).shadow(),
        );
        for c in 0..4u8 {
            let cr = Rect::new(col_x[usize::from(c)], ry, col_w[usize::from(c)], ROW_H - 2);
            let over = ui.hover(cr);
            if over && ui.input.pointer.is_some() && !capturing {
                st.row = k;
                st.col = c;
                st.foot = 0;
            }
            let this = st.capture == Some((k, c));
            let lit = lit_row && st.col == c;
            let conflict = b.conflicts(*action, c).next();
            let text = if this { "press..." } else { cell_text(b, *action, c) };
            if this {
                let pulse = 120 + (ui.tick % 30) as u8 * 4;
                ui.fill(cr, argb(style::gold_deep(), pulse));
            } else if lit {
                ui.fill(cr, argb(Ramp::UiSlot.at(Tone::Base), 255));
                ui.focus_ring(cr);
            } else {
                ui.fill(cr, argb(Ramp::UiSlot.at(Tone::Shade), 200));
            }
            let ink = if this {
                style::INK
            } else if conflict.is_some() {
                style::bad()
            } else if text.is_empty() {
                style::dim()
            } else {
                style::text_bright()
            };
            ui.text(i32::from(cr.x) + 5, ry + 2, if text.is_empty() && !this { "-" } else { text }, Ink::fine(ink));
            if over && ui.input.released && !capturing && !this {
                st.capture = Some((k, c));
            }
            ui.claim(cr);
        }
    }
    // A scroll mark when there is more.
    if n > visible {
        let track = Rect::new(x + w - 16, ty + 16, 3, visible as i32 * ROW_H);
        ui.fill(track, argb(style::INK, 180));
        let th = (visible * visible * ROW_H as usize / n) as i32;
        let tpos = (st.scroll * visible * ROW_H as usize / n) as i32;
        ui.fill(Rect::new(i32::from(track.x), i32::from(track.y) + tpos, 3, th.max(6)), argb(style::gold(), 220));
    }
    // What the lit cell clashes with.
    let foot_y = ty + 16 + visible as i32 * ROW_H + 6;
    if let Some((_, action, _)) = ACTIONS.get(st.row) {
        let clash: Vec<&str> = b.conflicts(*action, st.col).map(action_label).collect();
        if !clash.is_empty() {
            let s = format!("Also bound to: {}", clash.join(", "));
            ui.text(x + 20, foot_y, &s, Ink::fine(style::bad()).shadow());
        } else if capturing {
            let s = "Press the new one. Backspace clears it, Esc keeps it.";
            ui.text(x + 20, foot_y, s, Ink::fine(style::gold()).shadow());
        }
    }
    // The press for a cell waiting on one.
    if let Some((k, c)) = st.capture {
        if ui.input.keys.has(sc::ESCAPE) || ui.input.has(UiAction::Cancel) {
            st.capture = None;
        } else if let Some((_, action, _)) = ACTIONS.get(k)
            && let Some(changed) = capture(ui, b, *action, c)
        {
            st.capture = None;
            out.bindings = changed;
        }
    }

    // The rows beneath: aim assist, backend, reset, back.
    let fy = foot_y + 18;
    let lh = line_h(Face::Small) + 6;
    ui.text(
        x + 20,
        fy + 3,
        "Aim assist",
        Ink::small(if st.foot == 1 { style::text_bright() } else { style::text() }).shadow(),
    );
    let assists: [(&str, Option<AssistProfile>); 4] = [
        ("Auto", None),
        ("Off", Some(AssistProfile::Off)),
        ("Pad", Some(AssistProfile::Pad)),
        ("Mouse", Some(AssistProfile::Mouse)),
    ];
    for (i, (label, v)) in assists.iter().enumerate() {
        let br = Rect::new(x + 200 + i as i32 * 84, fy, 78, 22);
        if ui.button(wid("assist", i as u32), br, label, ButtonKind::Tab { on: info.assist == *v }, true, false) {
            out.assist = Some(*v);
        }
    }
    let by = fy + lh;
    ui.text(
        x + 20,
        by + 3,
        "Backend",
        Ink::small(if st.foot == 2 { style::text_bright() } else { style::text() }).shadow(),
    );
    for (i, label) in ["auto", "soft", "gl2", "wgpu"].iter().enumerate() {
        let br = Rect::new(x + 200 + i as i32 * 84, by, 78, 22);
        if ui.button(wid("backend", i as u32), br, label, ButtonKind::Tab { on: info.backend == *label }, true, false) {
            out.backend = Some(label);
        }
    }
    ui.text(x + 200 + 4 * 84 + 6, by + 6, "next start", Ink::fine(style::quiet()).shadow());
    let vy = by + lh + VOLUME_ROW_H - lh;
    out.volumes = crate::ui::volume::row(ui, x, vy, info.volumes, (st.foot == 3).then_some(st.vol));
    let ry = vy + lh + 6;
    let reset = Rect::new(x + w / 2 - 170, ry, 160, 24);
    let back = Rect::new(x + w / 2 + 10, ry, 160, 24);
    if ui.button(wid("controls-reset", 0), reset, "Reset all", ButtonKind::Menu, true, st.foot == 4) {
        b.rows = BINDINGS.to_vec();
        out.bindings = true;
    }
    if ui.button(wid("controls-back", 0), back, "Back", ButtonKind::Menu, true, st.foot == 5) {
        ui.intent(AppIntent::Back);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeySet;
    use crate::ui::UiArt;
    use crate::ui::core::UiInput;

    #[test]
    fn a_press_rebinds_and_a_clash_is_shown_not_refused() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut b = Bindings::default();
        let mut st = ControlsState::default();
        let info = ControlsInfo {
            assist: None,
            backend: "auto",
            volumes: Volumes::default(),
            rows: Features::of(Tier::T1),
            tier: Tier::T1,
        };
        // Row 0 (walk up), key 1: confirm to capture, then press E (Use's key).
        ui.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 1, (768, 432));
        draw(&mut ui, &mut st, &mut b, info);
        assert_eq!(st.capture, Some((0, 0)));
        let mut keys = KeySet::EMPTY;
        keys.set(sc::E, true);
        ui.begin(UiInput { keys, ..UiInput::default() }, 2, (768, 432));
        let out = draw(&mut ui, &mut st, &mut b, info);
        assert!(out.bindings);
        assert_eq!(b.row(Action::Up).unwrap().keys[0], sc::E);
        assert_eq!(b.conflicts(Action::Up, 0).collect::<Vec<_>>(), vec![Action::Use]);
    }

    #[test]
    fn the_display_page_turns_the_rows_the_tier_draws() {
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut b = Bindings::default();
        let mut st = ControlsState::default();
        let mut info = ControlsInfo {
            assist: None,
            backend: "auto",
            volumes: Volumes::default(),
            rows: Features::of(Tier::T1),
            tier: Tier::T1,
        };
        // The tab key goes to Display; confirm turns its first row (T1's N dot L) off.
        ui.begin(UiInput { actions: vec![UiAction::TabRight], ..UiInput::default() }, 1, (768, 432));
        draw(&mut ui, &mut st, &mut b, info);
        assert_eq!(st.page, 1);
        ui.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 2, (768, 432));
        let out = draw(&mut ui, &mut st, &mut b, info);
        let (rows, key) = out.rows.expect("a row turned");
        assert_eq!(key, "normal_light");
        assert!(!rows.normal_light);
        // Down to the lamp shadows: a count steps down by halves.
        info.rows = rows;
        ui.begin(UiInput { actions: vec![UiAction::Down, UiAction::Right], ..UiInput::default() }, 3, (768, 432));
        let out = draw(&mut ui, &mut st, &mut b, info);
        assert_eq!(out.rows.map(|(r, k)| (r.shadows, k)), Some((4, "shadows")));
        // T0 has no N dot L to turn, and its lamp shadows (2026-09-27); T2 has no silhouettes.
        let keys = |t| Features::rows(t).map(|r| r.key).collect::<Vec<_>>();
        assert!(!keys(Tier::T0).contains(&"normal_light") && keys(Tier::T0).contains(&"shadows"));
        assert!(!keys(Tier::T2).contains(&"silhouettes") && keys(Tier::T2).contains(&"bloom"));
    }
}
