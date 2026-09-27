//! Playing together (PRESENTATION.md §3.2 "Host, Join"; ARCHITECTURE.md §7): the title's Host
//! and Join screens, and what the HUD says of the table: who sits at it, in their coats, and
//! whom it waits for.
//!
//! The screens know nothing of the network: the app hands them rows and words and acts on the
//! intents they send (`AppIntent::Host`, `Join`, `Back`). They are built from the menus'
//! pieces: the window panel, the gold heading, tab buttons for a row's choices, wells for the
//! list, menu rows for the verbs.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};

use crate::input::UiAction;
use crate::ui::cmd::Rect;
use crate::ui::core::{AppIntent, ButtonKind, Ink, PanelStyle, Ui, text_w, wid};
use crate::ui::menus::{MenuState, dim, heading};
use crate::ui::style::{self, argb};

/// The seats' coats, seat by seat (ART.md: the look's own plum, then `person::SEAT_COATS`).
pub const COATS: [Ramp; 4] = [Ramp::ClothPlum, Ramp::ClothTeal, Ramp::ClothMoss, Ramp::ClothOchre];

/// What the Host screen asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostChoice {
    /// The world: a save slot, or `None` for a new one.
    pub slot: Option<u8>,
    /// Newcomers may sit down.
    pub open: bool,
    /// At most this many at the table, the host included.
    pub seats: u8,
    /// Frames of input delay.
    pub delay: u8,
    /// Wait for a stalled player however long (else she is got up after 10 s).
    pub wait: bool,
}

impl Default for HostChoice {
    fn default() -> Self {
        HostChoice { slot: None, open: true, seats: 4, delay: 3, wait: false }
    }
}

/// The Host screen's own state.
#[derive(Clone, Debug, Default)]
pub struct HostState {
    pub menu: MenuState,
    pub choice: HostChoice,
}

/// What the Host screen shows beside its choices.
#[derive(Clone, Debug, Default)]
pub struct HostInfo<'a> {
    /// Each slot's line ("The Lowfields · Day 3, 21:00"), `None` for an empty slot.
    pub slots: &'a [Option<String>],
    /// The port others dial.
    pub port: u16,
}

/// One host on the network, as the Join list shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanRow {
    /// "Tess's world".
    pub name: String,
    /// "192.168.1.20:7777": what Join dials.
    pub addr: String,
    /// "2 of 4".
    pub seats: String,
    /// This build can join it (same content, same build, a seat free).
    pub ok: bool,
}

/// The Join screen's own state.
#[derive(Clone, Debug, Default)]
pub struct JoinState {
    pub menu: MenuState,
    /// The address field.
    pub addr: String,
}

/// What the Join screen shows.
#[derive(Clone, Debug, Default)]
pub struct JoinInfo<'a> {
    pub found: &'a [LanRow],
    /// A join under way ("Building the county of seed 7"), or why the last one ended; `bad` for
    /// a refusal, which names both sides' content.
    pub status: Option<(&'a str, bool)>,
    /// A join is under way: Join is Cancel.
    pub joining: bool,
}

/// A row's label on the left of a Host row.
fn label(ui: &mut Ui, x: i32, y: i32, s: &str, lit: bool) {
    ui.text(x, y + 4, s, Ink::small(if lit { style::text_bright() } else { style::text() }).shadow());
}

/// Choices as tab buttons from `x`, each wide enough for the longest label; returns the one
/// clicked, and the right edge of the last.
fn tabs(ui: &mut Ui, name: &str, x: i32, y: i32, labels: &[&str], on: usize, enabled: &[bool]) -> (Option<usize>, i32) {
    let w = labels.iter().map(|l| text_w(Face::Small, l)).max().unwrap_or(0) + 20;
    let mut picked = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::new(x + i as i32 * (w + 6), y, w, 22);
        let en = enabled.get(i).copied().unwrap_or(true);
        if ui.button(wid(name, i as u32), r, l, ButtonKind::Tab { on: on == i }, en, false) {
            picked = Some(i);
        }
    }
    (picked, x + labels.len() as i32 * (w + 6) - 6)
}

/// Left and right on the lit row step its choice.
fn step(ui: &Ui, st: MenuState, row: u8, at: usize, n: usize) -> Option<usize> {
    if !ui.interactive || st.focus != row {
        return None;
    }
    let mut at = at as i32;
    let mut moved = false;
    for a in &ui.input.actions {
        match a {
            UiAction::Left => at -= 1,
            UiAction::Right => at += 1,
            _ => continue,
        }
        moved = true;
    }
    moved.then(|| at.clamp(0, n as i32 - 1) as usize)
}

/// Title → Host: the world, the door, the seats, the delay, the wait; Host and Back.
pub fn host(ui: &mut Ui, st: &mut HostState, info: &HostInfo<'_>) {
    let (cw, ch) = ui.canvas;
    dim(ui, 150);
    let (w, h) = (600, 350);
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, "Host");
    let sub = "You play; others on this network join you.";
    ui.text(cw / 2 - text_w(Face::Fine, sub) / 2, y + 40, sub, Ink::fine(style::quiet()).shadow());

    // Rows 0..=4 choose; 5 hosts; 6 backs out.
    let enabled = [true; 7];
    st.menu.nav(ui, &enabled);
    let c = &mut st.choice;
    let (lx, cx) = (x + 28, x + 198);
    let note = |ui: &mut Ui, x: i32, y: i32, s: &str| {
        ui.text(x, y, s, Ink::fine(style::quiet()).shadow());
    };
    let mut ry = y + 66;
    // The world.
    label(ui, lx, ry, "World", st.menu.focus == 0);
    let mut worlds = vec!["New"];
    let mut en = vec![true];
    for (i, s) in info.slots.iter().enumerate() {
        worlds.push(["Slot 1", "Slot 2", "Slot 3"].get(i).copied().unwrap_or("Slot"));
        en.push(s.is_some());
    }
    let on = c.slot.map_or(0, |n| usize::from(n) + 1);
    let (pick, _) = tabs(ui, "host-world", cx, ry, &worlds, on, &en);
    if let Some(k) = pick.or_else(|| step(ui, st.menu, 0, on, worlds.len()).filter(|&k| en[k])) {
        c.slot = k.checked_sub(1).map(|n| n as u8);
    }
    let what = match c.slot.and_then(|n| info.slots.get(usize::from(n))).cloned().flatten() {
        Some(s) => s,
        None => "A new county, built now".to_owned(),
    };
    note(ui, cx + 2, ry + 27, &what);
    ry += 50;
    // The door.
    label(ui, lx, ry, "Door", st.menu.focus == 1);
    let on = usize::from(!c.open);
    let (pick, _) = tabs(ui, "host-door", cx, ry, &["Open", "Closed"], on, &[]);
    if let Some(k) = pick.or_else(|| step(ui, st.menu, 1, on, 2)) {
        c.open = k == 0;
    }
    note(ui, cx + 2, ry + 27, if c.open { "Anyone here may sit down" } else { "Only you, until you open it" });
    ry += 50;
    // Seats.
    label(ui, lx, ry, "Seats", st.menu.focus == 2);
    let on = usize::from(c.seats.clamp(2, 4) - 2);
    let (pick, end) = tabs(ui, "host-seats", cx, ry, &["2", "3", "4"], on, &[]);
    if let Some(k) = pick.or_else(|| step(ui, st.menu, 2, on, 3)) {
        c.seats = k as u8 + 2;
    }
    // The coats, as many lit as there are seats.
    for (i, ramp) in COATS.iter().enumerate() {
        coat(ui, end + 16 + i as i32 * 18, ry + 5, *ramp, (i as u8) < c.seats);
    }
    ry += 36;
    // Delay.
    label(ui, lx, ry, "Input delay", st.menu.focus == 3);
    let on = usize::from(c.delay.clamp(2, 6) - 2);
    let (pick, end) = tabs(ui, "host-delay", cx, ry, &["2", "3", "4", "5", "6"], on, &[]);
    if let Some(k) = pick.or_else(|| step(ui, st.menu, 3, on, 5)) {
        c.delay = k as u8 + 2;
    }
    note(ui, end + 12, ry + 6, &format!("{} ms", u32::from(c.delay) * 50 / 3));
    ry += 36;
    // A stalled player.
    label(ui, lx, ry, "If one stalls", st.menu.focus == 4);
    let on = usize::from(c.wait);
    let (pick, _) = tabs(ui, "host-wait", cx, ry, &["Drop at 10 s", "Wait"], on, &[]);
    if let Some(k) = pick.or_else(|| step(ui, st.menu, 4, on, 2)) {
        c.wait = k == 1;
    }
    ry += 42;
    // Where others dial.
    let port = format!("Others join this machine's address, port {}", info.port);
    ui.rule(x + 28, x + w - 28, ry - 6, style::gold_deep());
    ui.text(cw / 2 - text_w(Face::Fine, &port) / 2, ry + 2, &port, Ink::fine(style::text()).shadow());
    // The verbs.
    let by = y + h - 40;
    let go = Rect::new(x + w / 2 - 170, by, 160, 26);
    let back = Rect::new(x + w / 2 + 10, by, 160, 26);
    for (i, rr) in [(5u8, go), (6, back)] {
        if ui.hover(rr) && ui.input.pointer.is_some() {
            st.menu.focus = i;
        }
    }
    if ui.button(wid("host-go", 0), go, "Host", ButtonKind::Menu, true, st.menu.focus == 5) {
        ui.intent(AppIntent::Host(st.choice));
    }
    if ui.button(wid("host-back", 0), back, "Back", ButtonKind::Menu, true, st.menu.focus == 6)
        || (ui.interactive && ui.input.has(UiAction::Cancel))
    {
        ui.intent(AppIntent::Back);
    }
    ui.claim(r);
}

/// Title → Join: the hosts the network answers with, an address field, Join and Back.
pub fn join(ui: &mut Ui, st: &mut JoinState, info: &JoinInfo<'_>) {
    let (cw, ch) = ui.canvas;
    dim(ui, 150);
    let (w, h) = (500, 356);
    let r = Rect::new((cw - w) / 2, (ch - h) / 2, w, h);
    ui.panel(r, PanelStyle::Window);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    heading(ui, cw / 2, y + 14, "Join");

    // The list: at most four hosts, each a well.
    let list_y = y + 50;
    ui.text(x + 24, list_y, "On this network", Ink::fine(style::gold()).shadow());
    let rows = info.found.len().min(4);
    let n_rows = rows as u8;
    // Focus: the rows, the field, Join, Back.
    let mut enabled: Vec<bool> = info.found.iter().take(4).map(|f| f.ok).collect();
    enabled.extend([true, true, true]);
    st.menu.nav(ui, &enabled);
    let row_h = 38;
    for (i, f) in info.found.iter().take(4).enumerate() {
        let rr = Rect::new(x + 20, list_y + 16 + i as i32 * row_h, w - 40, row_h - 4);
        let over = f.ok && ui.hover(rr);
        if over && ui.input.pointer.is_some() {
            st.menu.focus = i as u8;
        }
        let lit = st.menu.focus == i as u8;
        ui.well(rr, lit);
        if lit {
            ui.fill(rr.inset(2), argb(Ramp::UiPanel.at(Tone::Light), 60));
            ui.focus_ring(rr);
        }
        let (rx, ry) = (i32::from(rr.x), i32::from(rr.y));
        let name_ink = if f.ok { style::text_bright() } else { style::dim() };
        ui.text(rx + 10, ry + 3, &f.name, Ink::small(name_ink).shadow());
        ui.text(rx + 10, ry + 18, &f.addr, Ink::fine(style::quiet()).shadow());
        let right = rx + i32::from(rr.w) - 10;
        ui.text_right(right, ry + 4, &f.seats, Ink::fine(style::text()).shadow());
        if !f.ok {
            ui.text_right(right, ry + 18, "another build, or full", Ink::fine(style::bad()).shadow());
        }
        let clicked = over && ui.input.released;
        let confirmed = lit && ui.interactive && ui.input.has(UiAction::Confirm);
        if f.ok && (clicked || confirmed) {
            st.addr.clone_from(&f.addr);
            ui.intent(AppIntent::Join(f.addr.clone()));
        }
        ui.claim(rr);
    }
    if rows == 0 {
        let s = "Nobody hosting answers yet. Asking every second...";
        ui.text(x + 30, list_y + 24, s, Ink::fine(style::dim()).shadow());
    }

    // The address field.
    let fy = list_y + 16 + 4 * row_h + 6;
    ui.text(
        x + 24,
        fy + 6,
        "Or an address",
        Ink::small(if st.menu.focus == n_rows { style::text_bright() } else { style::text() }).shadow(),
    );
    let field = Rect::new(x + 196, fy, w - 220, 26);
    if ui.hover(field) && ui.input.pointer.is_some() && ui.input.pressed {
        st.menu.focus = n_rows;
    }
    let out = ui.text_field(field, &mut st.addr, 40, Ink::small(style::text_bright()).shadow());
    ui.text(x + 198, fy + 30, "192.168.1.20, or 192.168.1.20:7777", Ink::fine(style::dim()).shadow());

    // What is happening, or what went wrong.
    if let Some((s, bad)) = info.status {
        let ink = if bad { style::bad() } else { style::gold() };
        ui.wrapped(Rect::new(x + 24, fy + 48, w - 48, 30), s, Ink::fine(ink).shadow());
    }

    let by = y + h - 40;
    let go = Rect::new(x + w / 2 - 170, by, 160, 26);
    let back = Rect::new(x + w / 2 + 10, by, 160, 26);
    for (i, rr) in [(n_rows + 1, go), (n_rows + 2, back)] {
        if ui.hover(rr) && ui.input.pointer.is_some() {
            st.menu.focus = i;
        }
    }
    let verb = if info.joining { "Cancel" } else { "Join" };
    let can = info.joining || !st.addr.trim().is_empty();
    if (ui.button(wid("join-go", 0), go, verb, ButtonKind::Menu, can, st.menu.focus == n_rows + 1)
        || (out.submitted && !info.joining))
        && can
    {
        ui.intent(if info.joining { AppIntent::Back } else { AppIntent::Join(st.addr.trim().to_owned()) });
    }
    if ui.button(wid("join-back", 0), back, "Back", ButtonKind::Menu, true, st.menu.focus == n_rows + 2) {
        ui.intent(AppIntent::Back);
    }
    ui.claim(r);
}

/// A coat's swatch: the seat's cloth in a small lit square, dark when the seat is empty.
fn coat(ui: &mut Ui, x: i32, y: i32, ramp: Ramp, lit: bool) {
    let r = Rect::new(x, y, 12, 12);
    ui.fill(r, argb(style::INK, 230));
    if lit {
        ui.fill(r.inset(1), argb(ramp.at(Tone::Mid), 255));
        ui.fill(Rect::new(x + 1, y + 1, 10, 3), argb(ramp.at(Tone::Light), 255));
        ui.fill(Rect::new(x + 1, y + 9, 10, 2), argb(ramp.at(Tone::Shade), 255));
    } else {
        ui.fill(r.inset(1), argb(style::well(), 230));
    }
}

/// Who sits at the table, under the vitals and their chips: a coat per seat, lit when taken,
/// hers underlined in gold, and whether this machine hosts. `seats` has a bit per connected
/// seat; `chips` is how many status chips the vitals show (the table sits below them).
pub fn table(ui: &mut Ui, seats: u8, me: u8, hosting: bool, chips: bool) {
    let (x, y) = (8, if chips { 94 } else { 70 });
    let words = if hosting { "hosting" } else { "joined" };
    let w = 12 + 4 * 16 + 6 + text_w(Face::Fine, words) + 8;
    ui.panel(Rect::new(x, y, w, 24), PanelStyle::Hud);
    for (i, ramp) in COATS.iter().enumerate() {
        let (cx, cy) = (x + 6 + i as i32 * 16, y + 5);
        coat(ui, cx, cy, *ramp, seats & (1 << i) != 0);
        if i as u8 == me {
            ui.rule(cx - 1, cx + 13, cy + 14, style::gold());
        }
    }
    ui.text(x + 12 + 4 * 16, y + 6, words, Ink::fine(style::quiet()).shadow());
}

/// The table waits for someone: a band across the top third, the waited-for coats, how long,
/// and what happens at ten seconds.
pub fn stall(ui: &mut Ui, seats: u8, waited_ms: u64, wait: bool) {
    let (cw, ch) = ui.canvas;
    let (w, h) = (440, 50);
    let r = Rect::new((cw - w) / 2, ch * 3 / 8 - h / 2, w, h);
    ui.panel(r, PanelStyle::Hud);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    let pulse = (ui.tick / 4 % 16) as u8;
    ui.fill(Rect::new(x + 2, y + 2, w - 4, 1), argb(style::warn(), 120 + pulse * 6));
    let mut who = String::from("Waiting for the ");
    let mut first = true;
    let mut cx = x + 12;
    for (i, ramp) in COATS.iter().enumerate() {
        if seats & (1 << i) == 0 {
            continue;
        }
        coat(ui, cx, y + 9, *ramp, true);
        cx += 16;
        if !first {
            who.push_str(" and ");
        }
        first = false;
        who.push_str(["plum", "teal", "moss", "ochre"][i]);
    }
    who.push_str(if seats.count_ones() > 1 { " coats" } else { " coat" });
    let secs = waited_ms / 1000;
    ui.text(cx + 6, y + 7, &who, Ink::small(style::text_bright()).shadow());
    let foot = if wait {
        format!("{secs} s. The host waits however long.")
    } else {
        format!("{secs} s. At 10 s they get up; they can come back.")
    };
    ui.text(x + 12, y + 29, &foot, Ink::fine(style::quiet()).shadow());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::core::UiInput;
    use crate::ui::{UiArt, UiOut};

    fn ui() -> Ui {
        Ui::new(UiArt::build(1).0)
    }

    #[test]
    fn the_host_screen_hosts_what_was_chosen() {
        let mut ui = ui();
        let mut st = HostState::default();
        // Down to the seats row, left twice: two seats.
        let acts = vec![UiAction::Down, UiAction::Down, UiAction::Left, UiAction::Left];
        ui.begin(UiInput { actions: acts, ..UiInput::default() }, 1, (768, 432));
        host(&mut ui, &mut st, &HostInfo { slots: &[None, Some("The Lowfields".into()), None], port: 7777 });
        assert_eq!(st.choice.seats, 2);
        // An empty slot cannot be picked.
        st.menu.focus = 0;
        ui.begin(UiInput { actions: vec![UiAction::Right], ..UiInput::default() }, 2, (768, 432));
        host(&mut ui, &mut st, &HostInfo { slots: &[None, Some("The Lowfields".into()), None], port: 7777 });
        assert_eq!(st.choice.slot, None, "slot 1 is empty");
        st.menu.focus = 5;
        ui.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 3, (768, 432));
        host(&mut ui, &mut st, &HostInfo { slots: &[None, None, None], port: 7777 });
        assert!(ui.out.contains(&UiOut::Intent(AppIntent::Host(HostChoice { seats: 2, ..HostChoice::default() }))));
    }

    #[test]
    fn the_join_screen_joins_a_listed_host_or_an_address() {
        let mut ui = ui();
        let mut st = JoinState::default();
        let found = [
            LanRow { name: "Tess's world".into(), addr: "10.0.0.2:7777".into(), seats: "1 of 4".into(), ok: true },
            LanRow { name: "Old".into(), addr: "10.0.0.3:7777".into(), seats: "1 of 4".into(), ok: false },
        ];
        ui.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 1, (768, 432));
        join(&mut ui, &mut st, &JoinInfo { found: &found, ..JoinInfo::default() });
        assert!(ui.out.contains(&UiOut::Intent(AppIntent::Join("10.0.0.2:7777".into()))));
        // Typed, then Return.
        let mut st = JoinState { menu: MenuState { focus: 0 }, addr: String::new() };
        ui.begin(UiInput { typed: "pi:7800".into(), ..UiInput::default() }, 2, (768, 432));
        join(&mut ui, &mut st, &JoinInfo::default());
        assert_eq!(st.addr, "pi:7800");
    }
}
