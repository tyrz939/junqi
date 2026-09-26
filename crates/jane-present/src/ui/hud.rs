//! The HUD (PRESENTATION.md §3.2): vitals top-left with their status chips, the target frame
//! top-centre, the zone, the clock and the sky top-right, the quest tracker down the right, the
//! bar at the bottom with the prompt above it and the toasts above that, the chrome buttons
//! bottom-left, the zone banner, and the death veil. Every piece anchors to an edge, so a wider
//! canvas spreads the HUD and keeps the middle clear.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::tuning::BAR_SLOTS;

use crate::input::{Action, Bindings, PadInput, pad};
use crate::text::Tone as Say;
use crate::ui::art::Mark;
use crate::ui::cmd::Rect;
use crate::ui::core::{
    AppIntent, ButtonKind, DragPayload, DropTarget, Ink, PanelStyle, SlotView, Ui, advance, fmt_u32, line_h, text_w,
    wid,
};
use crate::ui::style::{self, argb, fade};
use crate::view::{BANNER_TICKS, Gauge, TOAST_FADE, TOAST_TICKS, ViewBuffers};

/// The bar's slot size and the gap between slots, px.
pub const SLOT: i32 = 36;
pub const GAP: i32 = 4;
/// Keys the bar's slots show, from the bindings table.
const BAR_ACTIONS: [Action; 8] = [
    Action::Bar(0),
    Action::Bar(1),
    Action::Bar(2),
    Action::Bar(3),
    Action::Bar(4),
    Action::Bar(5),
    Action::Bar(6),
    Action::Bar(7),
];

/// What the HUD needs besides the buffers.
#[derive(Clone, Copy, Debug)]
pub struct HudCtx<'a> {
    pub bindings: &'a Bindings,
    /// The last device was a pad: pad glyphs in the hints.
    pub pad: bool,
    /// A window is open: the bar takes drops from it.
    pub window_open: bool,
}

/// The bar's rect on this canvas.
pub fn bar_rect(canvas: (i32, i32)) -> Rect {
    let w = BAR_SLOTS as i32 * SLOT + (BAR_SLOTS as i32 - 1) * GAP + 16;
    Rect::new((canvas.0 - w) / 2, canvas.1 - SLOT - 18, w, SLOT + 12)
}

/// Draws the HUD. Pointer presses on the bar and the chrome buttons become `UiOut`s.
pub fn draw(ui: &mut Ui, b: &ViewBuffers, cx: HudCtx<'_>) {
    let (cw, ch) = ui.canvas;
    // Under an open window the plates it covers step aside; the bar stays, a place to drop.
    if !cx.window_open {
        vitals(ui, b);
        target(ui, b, cw);
        sky(ui, b, cw);
        tracker(ui, b, cw);
    }
    let bar = bar_rect((cw, ch));
    bar_slots(ui, b, bar, cx);
    let top = prompt(ui, b, bar, cx);
    toasts(ui, b, top);
    buttons(ui, ch);
    banner(ui, b, cw, ch);
    if b.me.dead {
        veil(ui, b, cw, ch);
    }
}

fn vitals(ui: &mut Ui, b: &ViewBuffers) {
    let h = &b.hud;
    let (x, y, w) = (8, 8, 212);
    ui.panel(Rect::new(x, y, w, 58), PanelStyle::Hud);
    // Her name, and the day's number beside it quietly.
    ui.text(x + 8, y + 5, &b.heroine, Ink::small(style::text_bright()).shadow());
    let low = h.hp.max > 0 && h.hp.frac < 250;
    gauge_row(ui, Rect::new(x + 8, y + 26, w - 16, 10), &h.hp, Ramp::ClothRed, low);
    gauge_row(ui, Rect::new(x + 8, y + 39, w - 16, 7), &h.mp, Ramp::ClothBlue, false);
    gauge_row(ui, Rect::new(x + 8, y + 49, w - 16, 5), &h.en, Ramp::ClothMustard, false);
    // HP as numbers at the name's right.
    let mut buf = [0u8; 12];
    let n = fmt_u32(h.hp.now.max(0) as u32, &mut buf).to_owned();
    let m = fmt_u32(h.hp.max.max(0) as u32, &mut buf).to_owned();
    let right = x + w - 8;
    ui.text_right(
        right - text_w(Face::Fine, &m) - 8,
        y + 9,
        &n,
        Ink::fine(if low { style::bad() } else { style::text() }).shadow(),
    );
    ui.text_right(right - text_w(Face::Fine, &m), y + 9, "/", Ink::fine(style::quiet()).shadow());
    ui.text_right(right, y + 9, &m, Ink::fine(style::quiet()).shadow());
    // Status chips under the plate.
    for (i, s) in h.statuses.iter().enumerate() {
        let r = Rect::new(x + i as i32 * 24, y + 62, 20, 20);
        let left = (s.ticks_left.min(s.total) * 1000 / s.total.max(1)) as u16;
        let v = SlotView { icon: Some(s.icon), usable: true, cooldown: 1000 - left, ..SlotView::default() };
        ui.slot(wid("chip", i as u32), r, &v, None, DropTarget::Window, false);
        if s.harmful {
            ui.fill(Rect::new(i32::from(r.x) + 2, i32::from(r.y) + 18, 16, 1), argb(style::bad(), 200));
        }
        let name = crate::text::text(s.name);
        let secs = s.ticks_left.div_ceil(60);
        ui.tip(wid("chip-tip", i as u32), r, |t| {
            use std::fmt::Write as _;
            let _ = write!(t, "{name}\n{secs} s left");
        });
    }
}

/// A gauge with its lag, and a slow pulse on the leading edge when `warn`.
fn gauge_row(ui: &mut Ui, r: Rect, g: &Gauge, ramp: Ramp, warn: bool) {
    let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
    ui.bar(r, g.frac, g.lag, ramp);
    if warn {
        let k = (ui.tick % 60) as i32;
        let a = (40 + 3 * (30 - (k - 30).abs())).clamp(0, 255) as u8;
        ui.fill(
            Rect::new(x + 1, y + 1, (w - 2) * i32::from(g.frac) / 1000, h - 2),
            argb(Ramp::ClothRed.at(Tone::Glint), a),
        );
    }
}

fn target(ui: &mut Ui, b: &ViewBuffers, cw: i32) {
    let Some(t) = &b.hud.target else { return };
    let w = 240;
    let x = (cw - w) / 2;
    let y = 8;
    ui.panel(Rect::new(x, y, w, 38), PanelStyle::Hud);
    let ink = if t.hostile { Ramp::ClothRed.at(Tone::High) } else { style::good() };
    if t.boss {
        ui.mark(Mark::Skull, x + 6, y + 4, 255);
    }
    ui.text_in(Rect::new(x, y + 3, w, 18), &t.name, Ink::small(ink).shadow());
    ui.bar(Rect::new(x + 10, y + 24, w - 20, 8), t.hp.frac, t.hp.lag, Ramp::ClothRed);
}

fn sky(ui: &mut Ui, b: &ViewBuffers, cw: i32) {
    let h = &b.hud;
    let w = (text_w(Face::Small, h.zone_name) + 42).max(176);
    let x = cw - w - 8;
    let y = 8;
    ui.panel(Rect::new(x, y, w, 44), PanelStyle::Hud);
    // The sun or the moon, rising and setting along the plate's top edge through the day.
    let mark = if h.night { Mark::Moon } else { Mark::Sun };
    ui.mark(mark, x + 8, y + 6, 255);
    ui.text(x + 30, y + 5, h.zone_name, Ink::small(style::text_bright()).shadow());
    let mut day = String::with_capacity(12);
    day.push_str("Day ");
    let mut buf = [0u8; 8];
    day.push_str(fmt_u32(h.day, &mut buf));
    ui.text(x + 30, y + 26, &h.clock, Ink::fine(style::gold()).shadow());
    ui.text_right(x + w - 10, y + 26, &day, Ink::fine(style::quiet()).shadow());
    // The hour as a thin arc of the day under the words.
    let frac = (h.clock_ticks as i64 * i64::from(w - 20) / (24 * 7200)) as i32;
    ui.fill(Rect::new(x + 10, y + 39, w - 20, 1), argb(style::INK, 140));
    ui.fill(
        Rect::new(x + 10 + frac, y + 38, 2, 3),
        argb(if h.night { Ramp::Bone.at(Tone::Light) } else { Ramp::GlassLit.at(Tone::High) }, 230),
    );
}

fn tracker(ui: &mut Ui, b: &ViewBuffers, cw: i32) {
    let lines = &b.hud.tracker;
    if lines.is_empty() {
        return;
    }
    let w = 200;
    let x = cw - w - 8;
    let mut y = 60;
    let fw = advance(Face::Fine);
    let cols = ((w - 20) / fw) as usize;
    for q in lines {
        // A soft dark backing so the words read over any ground, no frame.
        let steps: Vec<&str> = crate::ui::core::wrap_lines(&q.step, cols).collect();
        let hgt = 14 + steps.len() as i32 * line_h(Face::Fine) + 4;
        ui.fill(Rect::new(x, y, w, hgt), argb(style::INK, 110));
        ui.fill(Rect::new(x, y, 2, hgt), argb(if q.ready { style::gold() } else { style::gold_deep() }, 220));
        ui.text(x + 8, y + 2, &q.title, Ink::fine(if q.ready { style::gold() } else { style::text_bright() }).shadow());
        for (i, s) in steps.iter().enumerate() {
            let ink = if q.ready { style::gold() } else { style::quiet() };
            ui.text(x + 12, y + 14 + i as i32 * line_h(Face::Fine), s, Ink::fine(ink).shadow());
        }
        y += hgt + 4;
    }
}

fn bar_slots(ui: &mut Ui, b: &ViewBuffers, r: Rect, cx: HudCtx<'_>) {
    ui.panel(r, PanelStyle::Hud);
    ui.drop_area(r, DropTarget::BarBackground);
    let (x0, y0) = (i32::from(r.x) + 8, i32::from(r.y) + 6);
    for (i, s) in b.hud.bar.iter().enumerate() {
        let sr = Rect::new(x0 + i as i32 * (SLOT + GAP), y0, SLOT, SLOT);
        let label = if cx.pad {
            match cx.bindings.pad(BAR_ACTIONS[i]) {
                Some(p) => Some(crate::input::pad_name(p)),
                None => Some(cx.bindings.key(BAR_ACTIONS[i])),
            }
        } else {
            Some(cx.bindings.key(BAR_ACTIONS[i]))
        };
        let v = SlotView {
            icon: s.icon,
            count: if s.item.is_some() { s.count } else { 0 },
            item: s.item,
            spell: s.spell,
            cooldown: s.cooldown,
            gcd: s.gcd,
            usable: s.usable || s.icon.is_none(),
            label,
            flash: s.flash,
            flash_bad: s.flash_bad,
        };
        let payload = (s.icon.is_some() && cx.window_open).then_some(DragPayload::Bar(i as u8));
        let out = ui.slot(wid("bar", i as u32), sr, &v, payload, DropTarget::Bar(i as u8), false);
        if out.clicked && !cx.window_open {
            ui.command(jane_sim::input::Command::Bar { slot: i as u8, on: None });
        }
        if out.right_clicked && s.icon.is_some() {
            ui.command(jane_sim::input::Command::Unbind { slot: i as u8 });
        }
        if s.icon.is_some() {
            let (item, spell) = (s.item, s.spell);
            ui.tip(wid("bar-tip", i as u32), sr, |t| slot_tip(t, item, spell));
        }
    }
}

/// A slot's tooltip: the name, then what it does.
pub fn slot_tip(t: &mut String, item: Option<jane_core::ItemId>, spell: Option<jane_core::SpellId>) {
    use std::fmt::Write as _;
    let cat = jane_data::catalog();
    if let Some(s) = spell {
        let d = cat.combat.spell(s);
        let _ = writeln!(t, "{}", crate::text::text(d.name));
        let mut cost = String::new();
        if d.mp.points() > 0 {
            let _ = write!(cost, "{} mana", d.mp.points());
        }
        if d.energy.points() > 0 {
            if !cost.is_empty() {
                cost.push_str(", ");
            }
            let _ = write!(cost, "{} energy", d.energy.points());
        }
        if !cost.is_empty() {
            let _ = write!(t, "{cost}. ");
        }
        if d.cooldown.0 > 0 {
            let mut c = String::new();
            crate::text::span(d.cooldown.0, &mut c);
            let _ = write!(t, "Again after {c}. ");
        }
        t.push('\n');
        t.push_str(crate::text::text(d.description));
    } else if let Some(i) = item {
        let d = cat.combat.item(i);
        let _ = writeln!(t, "{}", crate::text::text(d.name));
        t.push_str(crate::text::text(d.description));
    }
}

/// The prompt above the bar; returns the y above it (where the toasts stack).
fn prompt(ui: &mut Ui, b: &ViewBuffers, bar: Rect, cx: HudCtx<'_>) -> i32 {
    let top = i32::from(bar.y) - 8;
    let Some(p) = &b.hud.prompt else { return top };
    if b.dialogue.is_some() || b.me.dead {
        return top;
    }
    let (cw, _) = ui.canvas;
    let mut words = String::with_capacity(48);
    words.push_str(p.verb);
    if !p.label.is_empty() {
        words.push_str(": ");
        words.push_str(&p.label);
    }
    if p.hold && p.verb != "Hold to push" {
        words.push_str(" (hold to push)");
    }
    let tw = text_w(Face::Small, &words);
    let cap = key_cap_w(cx, Action::Use);
    let w = cap + 8 + tw;
    let x = (cw - w) / 2;
    let y = top - 22;
    // Rises a few px as it appears for the first time is out of scope; a steady breathing lift.
    ui.fill(Rect::new(x - 12, y - 3, w + 24, 24), argb(style::INK, 120));
    ui.rule(x - 30, x + w + 30, y - 4, style::gold_deep());
    key_cap(ui, x, y, cx, Action::Use);
    ui.text(x + cap + 8, y + 1, &words, Ink::small(style::text_bright()).shadow());
    y - 8
}

/// The width of the cap [`key_cap`] draws.
pub fn key_cap_w(cx: HudCtx<'_>, a: Action) -> i32 {
    if cx.pad && cx.bindings.pad(a).is_some() {
        return 16;
    }
    text_w(Face::Fine, cx.bindings.key(a)).max(8) + 10
}

/// A key cap (or the pad's button) for `a` from the bindings table, at `(x, y)`, 18 px tall.
pub fn key_cap(ui: &mut Ui, x: i32, y: i32, cx: HudCtx<'_>, a: Action) {
    if cx.pad
        && let Some(p) = cx.bindings.pad(a)
    {
        let (m, l) = match p {
            PadInput::Button(pad::A) => (Mark::PadA, "A"),
            PadInput::Button(pad::B) => (Mark::PadB, "B"),
            PadInput::Button(pad::X) => (Mark::PadX, "X"),
            PadInput::Button(pad::Y) => (Mark::PadY, "Y"),
            _ => (Mark::PadShoulder, crate::input::pad_name(p)),
        };
        ui.mark(m, x, y + 1, 255);
        ui.text(x + 8 - text_w(Face::Fine, l) / 2, y + 4, l, Ink::fine(style::text_bright()).outline());
        return;
    }
    let k = cx.bindings.key(a);
    let w = text_w(Face::Fine, k).max(8) + 10;
    let r = Rect::new(x, y, w, 18);
    ui.fill(Rect::new(x + 1, y, w - 2, 18), argb(style::INK, 255));
    ui.fill(Rect::new(x, y + 1, w, 16), argb(style::INK, 255));
    ui.fill(r.inset(1), argb(Ramp::UiInk.at(Tone::Light), 255));
    ui.fill(Rect::new(x + 1, y + 14, w - 2, 3), argb(Ramp::UiInk.at(Tone::Mid), 255));
    ui.fill(Rect::new(x + 2, y + 1, w - 4, 1), argb(Ramp::UiInk.at(Tone::Glint), 255));
    ui.text(x + (w - text_w(Face::Fine, k)) / 2, y + 2, k, Ink::fine(style::INK));
}

fn toasts(ui: &mut Ui, b: &ViewBuffers, bottom: i32) {
    let (cw, _) = ui.canvas;
    let now = b.tick;
    let lh = 20;
    let mut y = bottom - lh;
    for t in b.hud.toasts.iter().rev() {
        let age = now.wrapping_sub(t.born);
        let fade_in = (age * 40).min(255);
        let left = TOAST_TICKS.saturating_sub(age);
        let fade_out = if left < TOAST_FADE { left * 255 / TOAST_FADE } else { 255 };
        let a = fade_in.min(fade_out) as u8;
        // It rises into place over its first ticks.
        let rise = (6i32 - age as i32).max(0);
        let ink = match t.tone {
            Say::Plain => style::text_bright(),
            Say::Good => style::gold(),
            Say::Refused => Ramp::ClothRed.at(Tone::High),
        };
        let mut words = String::with_capacity(t.text.len() + 6);
        words.push_str(&t.text);
        if t.count > 1 {
            let mut buf = [0u8; 6];
            words.push_str("  x");
            words.push_str(fmt_u32(u32::from(t.count), &mut buf));
        }
        let w = text_w(Face::Small, &words);
        let x = (cw - w) / 2;
        ui.fill(Rect::new(x - 10, y + rise - 1, w + 20, lh), fade(argb(style::INK, 110), a));
        ui.text(x, y + rise + 1, &words, Ink::small(ink).shadow().alpha(a));
        y -= lh + 2;
    }
}

fn buttons(ui: &mut Ui, ch: i32) {
    let labels = [("Bag", 0u8), ("Book", 1), ("Log", 2), ("Map", 3)];
    let (w, h) = (40, 18);
    for (i, (l, tab)) in labels.iter().enumerate() {
        let r = Rect::new(8 + i as i32 * (w + 4), ch - h - 8, w, h);
        if ui.button(wid("hud-btn", i as u32), r, l, ButtonKind::Chip, true, false) {
            ui.intent(AppIntent::OpenWindow(*tab));
        }
    }
    let r = Rect::new(8 + 4 * (w + 4), ch - h - 8, w, h);
    if ui.button(wid("hud-btn", 9), r, "Menu", ButtonKind::Chip, true, false) {
        ui.intent(AppIntent::Pause);
    }
}

fn banner(ui: &mut Ui, b: &ViewBuffers, cw: i32, ch: i32) {
    let Some((name, born)) = b.hud.banner else { return };
    let age = b.tick.wrapping_sub(born);
    let a = if age < 30 {
        age * 255 / 30
    } else if age > BANNER_TICKS - 50 {
        (BANNER_TICKS - age) * 255 / 50
    } else {
        255
    };
    let a = a.min(255) as u8;
    let w = text_w(Face::Head, name);
    let y = ch / 4 - 14;
    let x = (cw - w) / 2;
    // The rules grow out from the name as it arrives.
    let grow = (age.min(40) as i32) * 3;
    ui.rule(x - 30 - grow, x - 10, y + 13, style::gold());
    ui.rule(x + w + 10, x + w + 30 + grow, y + 13, style::gold());
    ui.text(x, y, name, Ink::head(style::text_bright()).shadow().alpha(a));
    let sub = if b.hud.night { "Night" } else { "" };
    if !sub.is_empty() {
        ui.text((cw - text_w(Face::Fine, sub)) / 2, y + 30, sub, Ink::fine(style::quiet()).shadow().alpha(a));
    }
}

fn veil(ui: &mut Ui, b: &ViewBuffers, cw: i32, ch: i32) {
    // A cold violet veil, deepening toward the edges.
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ramp::ClothPlum.at(Tone::Deep), 120));
    for k in 0..6 {
        let d = 6 + k * 10;
        let a = (70 - k * 11).max(0) as u8;
        ui.fill(Rect::new(0, 0, cw, d), argb(style::INK, a));
        ui.fill(Rect::new(0, ch - d, cw, d), argb(style::INK, a));
    }
    let y = ch / 2 - 30;
    let s = format!("{} fell", b.heroine);
    let s = s.as_str();
    ui.text((cw - text_w(Face::Head, s)) / 2, y, s, Ink::head(Ramp::Bone.at(Tone::Light)).shadow());
    let secs = b.me.respawn_ticks.div_ceil(60);
    let mut buf = [0u8; 8];
    let mut line = String::from("Waking in ");
    line.push_str(fmt_u32(secs, &mut buf));
    let w = text_w(Face::Small, &line);
    ui.mark(Mark::Skull, (cw - w) / 2 - 22, y + 34, 220);
    ui.text((cw - w) / 2, y + 35, &line, Ink::small(style::quiet()).shadow());
}
