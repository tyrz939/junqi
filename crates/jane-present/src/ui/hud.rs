//! The HUD (PRESENTATION.md §3.2): vitals top-left with their status chips, the target frame
//! top-centre, the zone, the clock and the sky top-right, the quest tracker down the right, the
//! bar at the bottom with the prompt above it and the toasts above that, the chrome buttons
//! bottom-left, the zone banner, and the death veil. Every piece anchors to an edge, so a wider
//! canvas spreads the HUD and keeps the middle clear.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::tuning::BAR_SLOTS;

use crate::input::{Action, Bindings, PadInput, pad};
use crate::text::Tone as Say;
use crate::ui::art::Mark;
use crate::ui::cmd::Rect;
use crate::ui::core::{
    AppIntent, ButtonKind, DragPayload, DropTarget, Ink, PanelStyle, SlotView, Ui, advance, fmt_u32, line_h, text_w,
    wid, wrap_lines,
};
use crate::ui::style::{self, argb, fade};
use crate::view::{Gauge, QuestLine, TOAST_FADE, TOAST_TICKS, ViewBuffers};

/// Columns a line of world text (a toast, the prompt) wraps at in the Small face: the
/// tooltip's rule (§3.2). A mine chest's "has no keyhole" ran off both edges of the canvas.
pub const TEXT_COLS: i32 = 40;
/// Px kept clear between world text and the canvas's edges.
pub const TEXT_EDGE: i32 = 12;

/// The columns world text in `face` wraps at on a canvas `cw` wide: [`TEXT_COLS`], fewer where
/// the canvas is too narrow for them.
pub fn text_cols(cw: i32, face: Face) -> usize {
    TEXT_COLS.min((cw - 2 * TEXT_EDGE - 20) / advance(face)).max(8) as usize
}

/// Lines a toast shows at most, and the prompt: what runs longer ends in "...".
pub const TOAST_LINES: usize = 4;
pub const PROMPT_LINES: usize = 3;

/// `s` wrapped at `cols`, at most `most` lines, the last cut short with "..." if it ran on.
pub fn wrapped(s: &str, cols: usize, most: usize) -> Vec<String> {
    let mut lines: Vec<String> = wrap_lines(s, cols).map(str::to_owned).collect();
    if lines.len() > most {
        lines.truncate(most.max(1));
        if let Some(last) = lines.last_mut() {
            while last.chars().count() + 3 > cols && last.pop().is_some() {}
            last.push_str("...");
        }
    }
    lines
}

/// Where a line `w` px wide starts, centred on a canvas `cw` wide and kept on it.
pub fn centred_x(cw: i32, w: i32) -> i32 {
    ((cw - w) / 2).clamp(TEXT_EDGE, (cw - w - TEXT_EDGE).max(TEXT_EDGE))
}

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

/// Bar slot `i`'s well on this canvas (a lesson's icon flies to it, `ui::lesson`).
pub fn bar_slot(canvas: (i32, i32), i: usize) -> Rect {
    let r = bar_rect(canvas);
    Rect::new(i32::from(r.x) + 8 + i as i32 * (SLOT + GAP), i32::from(r.y) + 6, SLOT, SLOT)
}

/// The vitals plate's width, px: a little over a quarter of the 640 canvas.
pub const VITALS_W: i32 = 172;
/// The vitals' three gauges: health, mana, energy (a jar or a page glints the one it grew).
pub const VITALS_GAUGES: [Rect; 3] =
    [Rect::new(16, 34, VITALS_W - 16, 10), Rect::new(16, 47, VITALS_W - 16, 7), Rect::new(16, 57, VITALS_W - 16, 5)];
/// The target frame's width at most, px; narrower where the sky plate's name leaves less.
pub const TARGET_W: i32 = 184;

/// Draws the HUD. Pointer presses on the bar and the chrome buttons become `UiOut`s.
pub fn draw(ui: &mut Ui, b: &ViewBuffers, cx: HudCtx<'_>) {
    let (cw, ch) = ui.canvas;
    // Under an open window the plates it covers step aside; the bar stays, a place to drop.
    if !cx.window_open {
        vitals(ui, b);
        let sky_x = sky(ui, b, cw);
        if !b.me.dead {
            target(ui, b, cw, sky_x);
        }
        tracker(ui, b, cw);
    }
    let bar = bar_rect((cw, ch));
    bar_slots(ui, b, bar, cx);
    let top = prompt(ui, b, bar, cx);
    toasts(ui, b, top);
    buttons(ui, ch, i32::from(bar.x));
    if !b.me.dead {
        banner(ui, b, cw, ch);
    }
    if b.me.dead {
        veil(ui, b, cw, ch);
    }
}

fn vitals(ui: &mut Ui, b: &ViewBuffers) {
    let h = &b.hud;
    let (x, y, w) = (8, 8, VITALS_W);
    ui.panel(Rect::new(x, y, w, 58), PanelStyle::Hud);
    // Her name, and the day's number beside it quietly.
    ui.text(x + 8, y + 5, &b.heroine, Ink::small(style::text_bright()).shadow());
    let low = h.hp.max > 0 && h.hp.frac < 250;
    let [hp, mp, en] = VITALS_GAUGES;
    gauge_row(ui, hp, &h.hp, Ramp::ClothRed, low);
    gauge_row(ui, mp, &h.mp, Ramp::ClothBlue, false);
    gauge_row(ui, en, &h.en, Ramp::ClothMustard, false);
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
        let v = SlotView { icon: Some(s.icon), usable: true, gcd: 1000 - left, ..SlotView::default() };
        ui.slot(wid("chip", i as u32), r, &v, None, DropTarget::Window, false);
        if s.harmful {
            ui.fill(Rect::new(i32::from(r.x) + 2, i32::from(r.y) + 18, 16, 1), argb(style::bad(), 200));
        }
        let name = crate::text::text(s.name);
        let secs = s.ticks_left.div_ceil(60);
        ui.tip(wid("chip-tip", i as u32), r, |t| {
            use core::fmt::Write as _;
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

/// The target frame, top-centre between the vitals and the sky plate (which starts at `sky_x`).
fn target(ui: &mut Ui, b: &ViewBuffers, cw: i32, sky_x: i32) {
    let Some(t) = &b.hud.target else { return };
    let room = (cw / 2 - (VITALS_W + 16)).min(sky_x - 8 - cw / 2);
    let w = TARGET_W.min(2 * room).max(96);
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

/// The zone, the clock and the sky, top-right; returns the plate's left edge.
fn sky(ui: &mut Ui, b: &ViewBuffers, cw: i32) -> i32 {
    let h = &b.hud;
    let w = (text_w(Face::Small, h.zone_name) + 42).max(150);
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
    let frac = (h.clock_ticks as i64 * i64::from(w - 20) / (24 * i64::from(jane_core::num::TICKS_PER_HOUR))) as i32;
    ui.fill(Rect::new(x + 10, y + 39, w - 20, 1), argb(style::INK, 140));
    ui.fill(
        Rect::new(x + 10 + frac, y + 38, 2, 3),
        argb(if h.night { Ramp::Bone.at(Tone::Light) } else { Ramp::GlassLit.at(Tone::High) }, 230),
    );
    x
}

/// The tracker's width, px: under a third of the 640 canvas, wide enough for a quest's name.
pub const TRACKER_W: i32 = 192;
/// Its top: under the sky plate.
pub const TRACKER_TOP: i32 = 60;
/// Px kept clear under it: the save card, the toasts and the prompt live there.
pub const TRACKER_FOOT: i32 = 150;
/// Lines the top quest's step shows at most; what runs longer ends in "...". Every other tracked
/// quest is its name and one line of its step (decided 2026-10-07: compact on 640 x 360).
pub const TRACKER_STEP_LINES: usize = 2;
/// A band's name row, px.
const TRACKER_TITLE_H: i32 = 13;

/// One tracked quest's band, laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerBand {
    pub rect: Rect,
    pub title: String,
    pub steps: Vec<String>,
    /// The way and the bearing (`QuestLine::way`, `bearing`), two lines each at most.
    pub way: Vec<String>,
    pub ready: bool,
    pub main: bool,
}

/// The tracker laid out on a canvas: the bands that fit down the right edge between the sky
/// plate and [`TRACKER_FOOT`], each step wrapped at the band's width and cut at
/// [`TRACKER_STEP_LINES`]; the rest counted (`more`, said under the last band as "N more in
/// the Log").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackerLayout {
    pub bands: Vec<TrackerBand>,
    pub more: usize,
    /// Where the "more" line goes.
    pub more_y: i32,
}

pub fn tracker_layout(lines: &[QuestLine], canvas: (i32, i32)) -> TrackerLayout {
    let (cw, ch) = canvas;
    let w = TRACKER_W.min(cw - 16);
    let x = cw - w - 8;
    let bottom = (ch - TRACKER_FOOT).max(TRACKER_TOP + 40);
    let fw = advance(Face::Fine);
    let lh = line_h(Face::Fine);
    let cols = ((w - 24) / fw).max(8) as usize;
    let mut out = TrackerLayout { more_y: TRACKER_TOP, ..TrackerLayout::default() };
    let mut y = TRACKER_TOP;
    for (n, q) in lines.iter().enumerate() {
        let title = wrapped(&q.title, ((w - 22) / fw).max(8) as usize, 1).pop().unwrap_or_default();
        // The top quest opens out (its step on two lines, the way and the bearing on one each);
        // the rest are a name and one line.
        let top = n == 0;
        let steps = wrapped(&q.step, cols, if top { TRACKER_STEP_LINES } else { 1 });
        let mut way: Vec<String> = if top {
            [&q.way, &q.bearing].into_iter().filter(|s| !s.is_empty()).flat_map(|s| wrapped(s, cols, 1)).collect()
        } else {
            Vec::new()
        };
        // Room for this band, and for the "more" line if any come after it; on a short canvas
        // the way gives up its lines before the quest gives up its band.
        let after = if n + 1 < lines.len() { lh + 2 } else { 0 };
        let height = |way: &[String]| TRACKER_TITLE_H + (steps.len() + way.len()) as i32 * lh + 3;
        while !way.is_empty() && y + height(&way) + after > bottom {
            way.pop();
        }
        let h = height(&way);
        if y + h + after > bottom {
            out.more = lines.len() - n;
            break;
        }
        out.bands.push(TrackerBand { rect: Rect::new(x, y, w, h), title, steps, way, ready: q.ready, main: q.main });
        y += h + 3;
    }
    out.more_y = y;
    out
}

/// The tracker (§3.2): the quests this seat tracks, the main line first, each on a dark band
/// with a gold edge: its name (a diamond for the story's own), then its open step ("Back to ..."
/// in gold when it is ready). It keeps down the right edge, clear of the sky plate above and the
/// save card and toasts below; what does not fit is counted.
fn tracker(ui: &mut Ui, b: &ViewBuffers, cw: i32) {
    let lines = &b.hud.tracker;
    if lines.is_empty() {
        return;
    }
    let lay = tracker_layout(lines, (cw, ui.canvas.1));
    let lh = line_h(Face::Fine);
    for band in &lay.bands {
        let (x, y, w, h) =
            (i32::from(band.rect.x), i32::from(band.rect.y), i32::from(band.rect.w), i32::from(band.rect.h));
        // A soft dark backing so the words read over any ground: deeper at the edge, fading in.
        ui.fill(Rect::new(x, y, w, h), argb(style::INK, 104));
        ui.fill(Rect::new(x, y, 24, h), argb(style::INK, 40));
        let edge = if band.ready { style::gold() } else { style::gold_deep() };
        ui.fill(Rect::new(x, y, 2, h), argb(edge, 225));
        ui.fill(Rect::new(x + 2, y, w - 2, 1), argb(style::gold_deep(), 50));
        let mut tx = x + 8;
        if band.main {
            ui.mark(Mark::Diamond, x + 7, y + 3, 255);
            tx += 10;
        }
        let ink = if band.ready { style::gold() } else { style::text_bright() };
        ui.text(tx, y + 2, &band.title, Ink::fine(ink).shadow());
        let step_ink = if band.ready { style::gold() } else { style::quiet() };
        for (i, s) in band.steps.iter().enumerate() {
            ui.text(x + 14, y + TRACKER_TITLE_H + i as i32 * lh, s, Ink::fine(step_ink).shadow());
        }
        let wy = y + TRACKER_TITLE_H + band.steps.len() as i32 * lh;
        for (i, s) in band.way.iter().enumerate() {
            ui.text(x + 14, wy + i as i32 * lh, s, Ink::fine(style::dim()).shadow());
        }
    }
    if lay.more > 0 {
        let x = cw - TRACKER_W.min(cw - 16) - 8;
        let more = format!("{} more in the Log", lay.more);
        ui.fill(Rect::new(x, lay.more_y, 2, lh), argb(style::gold_deep(), 120));
        ui.text(x + 8, lay.more_y, &more, Ink::fine(style::dim()).shadow());
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
    use core::fmt::Write as _;
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
    let cap = key_cap_w(cx, Action::Use);
    // Wrapped to the canvas beside the key cap, the block centred and kept on the canvas.
    let cols = text_cols(cw - cap - 8, Face::Small);
    let lines = wrapped(&words, cols, PROMPT_LINES);
    let tw = lines.iter().map(|l| text_w(Face::Small, l)).max().unwrap_or(0);
    let step = line_h(Face::Small) + 2;
    let more = (lines.len().max(1) as i32 - 1) * step;
    let w = cap + 8 + tw;
    let x = centred_x(cw, w);
    let y = top - 22 - more;
    ui.fill(Rect::new(x - 12, y - 3, w + 24, 24 + more), argb(style::INK, 120));
    ui.rule((x - 30).max(0), (x + w + 30).min(cw), y - 4, style::gold_deep());
    key_cap(ui, x, y, cx, Action::Use);
    for (i, l) in lines.iter().enumerate() {
        ui.text(x + cap + 8, y + 1 + i as i32 * step, l, Ink::small(style::text_bright()).shadow());
    }
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
    cap(ui, x, y, cx.bindings.key(a), None);
}

/// A key cap with `key` on it at `(x, y)`, 18 px tall, or with `pad` the pad's button (its mark,
/// `key` printed over it: "A", "LB"); returns its width.
pub fn cap(ui: &mut Ui, x: i32, y: i32, key: &str, pad: Option<Mark>) -> i32 {
    if let Some(m) = pad {
        if m == Mark::PadShoulder {
            // A shoulder: a small dark lozenge with its name.
            let w = text_w(Face::Fine, key) + 8;
            ui.fill(Rect::new(x + 1, y + 2, w - 2, 14), argb(style::INK, 255));
            ui.fill(Rect::new(x, y + 3, w, 12), argb(style::INK, 255));
            ui.fill(Rect::new(x + 1, y + 3, w - 2, 1), argb(Ramp::UiInk.at(Tone::Mid), 255));
            ui.text(x + 4, y + 4, key, Ink::fine(style::text_bright()));
            return w;
        }
        ui.mark(m, x, y + 1, 255);
        ui.text(x + 8 - text_w(Face::Fine, key) / 2, y + 4, key, Ink::fine(style::text_bright()).outline());
        return 16;
    }
    let w = text_w(Face::Fine, key).max(8) + 10;
    let r = Rect::new(x, y, w, 18);
    ui.fill(Rect::new(x + 1, y, w - 2, 18), argb(style::INK, 255));
    ui.fill(Rect::new(x, y + 1, w, 16), argb(style::INK, 255));
    ui.fill(r.inset(1), argb(Ramp::UiInk.at(Tone::Light), 255));
    ui.fill(Rect::new(x + 1, y + 14, w - 2, 3), argb(Ramp::UiInk.at(Tone::Mid), 255));
    ui.fill(Rect::new(x + 2, y + 1, w - 4, 1), argb(Ramp::UiInk.at(Tone::Glint), 255));
    ui.text(x + (w - text_w(Face::Fine, key)) / 2, y + 2, key, Ink::fine(style::INK));
    w
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
        // Wrapped at the tooltip's width, each line centred, the block kept on the canvas.
        let lines = wrapped(&words, text_cols(cw, Face::Small), TOAST_LINES);
        let step = line_h(Face::Small) + 2;
        let more = (lines.len().max(1) as i32 - 1) * step;
        let top = y - more;
        // An older toast with no room left above the newer ones is not drawn.
        if top < TEXT_EDGE {
            break;
        }
        let w = lines.iter().map(|l| text_w(Face::Small, l)).max().unwrap_or(0);
        let bx = centred_x(cw, w);
        ui.fill(Rect::new(bx - 10, top + rise - 1, w + 20, lh + more), fade(argb(style::INK, 110), a));
        for (i, l) in lines.iter().enumerate() {
            let x = centred_x(cw, text_w(Face::Small, l));
            ui.text(x, top + rise + 1 + i as i32 * step, l, Ink::small(ink).shadow().alpha(a));
        }
        y = top - lh - 2;
    }
}

/// The chrome buttons bottom-left: in a row where they clear the bar (which starts at `bar_x`),
/// else Bag, Book and Log along the bottom with Map and Menu over them.
fn buttons(ui: &mut Ui, ch: i32, bar_x: i32) {
    let labels = [("Bag", Some(0u8)), ("Book", Some(1)), ("Log", Some(2)), ("Map", Some(3)), ("Menu", None)];
    let (w, h) = (40, 18);
    let one_row = 8 + 5 * (w + 4) <= bar_x - 4;
    for (i, (l, tab)) in labels.iter().enumerate() {
        let (col, row) = if one_row { (i as i32, 0) } else { (i as i32 % 3, i as i32 / 3) };
        let r = Rect::new(8 + col * (w + 4), ch - h - 8 - row * (h + 4), w, h);
        let id = if tab.is_some() { i as u32 } else { 9 };
        if ui.button(wid("hud-btn", id), r, l, ButtonKind::Chip, true, false) {
            ui.intent(match tab {
                Some(t) => AppIntent::OpenWindow(*t),
                None => AppIntent::Pause,
            });
        }
    }
}

fn banner(ui: &mut Ui, b: &ViewBuffers, cw: i32, ch: i32) {
    let Some(bn) = &b.hud.banner else { return };
    let (name, total) = (bn.text.as_str(), bn.ticks());
    let age = b.tick.wrapping_sub(bn.born);
    let a = if age < 30 {
        age * 255 / 30
    } else if age > total - 50 {
        (total - age) * 255 / 50
    } else {
        255
    };
    let a = a.min(255) as u8;
    if bn.place {
        // A named place she has crossed into: smaller than a zone's, its rules short and still,
        // a diamond at each end, as a map's lettering sits between two flourishes.
        let w = text_w(Face::Small, name);
        let y = ch / 4 - 6;
        let x = centred_x(cw, w);
        let grow = (age.min(30) as i32) * 2 / 3;
        let ra = argb(style::gold(), a);
        let ry = y + 6;
        ui.fill(Rect::new(x - 12 - grow, ry, 8 + grow, 1), ra);
        ui.fill(Rect::new(x + w + 4, ry, 8 + grow, 1), ra);
        ui.fill(Rect::new(x - 14 - grow, ry - 1, 1, 3), ra);
        ui.fill(Rect::new(x - 15 - grow, ry, 3, 1), ra);
        ui.fill(Rect::new(x + w + 13 + grow, ry - 1, 1, 3), ra);
        ui.fill(Rect::new(x + w + 12 + grow, ry, 3, 1), ra);
        ui.text(x, y, name, Ink::small(style::text_bright()).shadow().alpha(a));
        return;
    }
    let w = text_w(Face::Head, name);
    let y = ch / 4 - 14;
    let x = centred_x(cw, w);
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
    ui.fill(Rect::new(0, 0, cw, ch), argb(Ramp::ClothPlum.at(Tone::Deep), 170));
    for k in 0..8 {
        let d = 6 + k * 12;
        let a = (96 - k * 12).max(0) as u8;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UiArt;
    use crate::ui::cmd::UiCmd;
    use crate::ui::core::UiInput;
    use crate::view::Prompt;

    /// The strings the game says, as a toast and as the prompt's label, on the narrowest, the
    /// usual and a 21:9 canvas: every glyph lands inside the canvas. A mine chest's "has no
    /// keyhole" ran off both edges (2026-10-01). The catalog's longest lines are the worst case.
    #[test]
    fn every_text_stays_on_the_canvas_as_a_toast_and_as_the_prompt() {
        let cat = jane_data::catalog();
        let mut ui = Ui::new(UiArt::build(1).0);
        let bind = Bindings::default();
        let cx = HudCtx { bindings: &bind, pad: false, window_open: true };
        let mut longest = cat.texts.to_vec();
        longest.sort_by_key(|s| core::cmp::Reverse(s.len()));
        let mut all = vec!["Chest has no keyhole. Something under the floor holds the lid down"];
        all.extend(longest.into_iter().filter(|s| !s.contains('\n')).take(60));
        all.extend(cat.texts.iter().copied().filter(|s| s.len() > 30 && s.len() < 120 && !s.contains('\n')));
        for canvas in [(640u16, 360u16), (768, 432), (1008, 432)] {
            let (cw, ch) = (i32::from(canvas.0), i32::from(canvas.1));
            for s in &all {
                let mut b = ViewBuffers::new();
                b.push_toast(s, Say::Refused);
                b.hud.prompt = Some(Prompt { verb: "Read", label: (*s).to_owned(), hold: false });
                b.tick += 20;
                ui.begin(UiInput::default(), b.tick, canvas);
                draw(&mut ui, &b, cx);
                for c in &ui.cmds {
                    if let UiCmd::Sprite { dst, ink, .. } = c
                        && *ink != 0
                    {
                        assert!(
                            dst.x >= 0 && dst.right() <= cw && dst.y >= 0 && dst.bottom() <= ch,
                            "a glyph of {s:?} at {dst:?} leaves the {cw}x{ch} canvas"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_tracker_stays_on_screen_with_long_words_and_counts_the_rest() {
        let long = "A very long step that goes on and on about the well by the station road and the lamp \
                    posts and the hedge and the gate and the dog and the fire at the halt, and then some more";
        let lines: Vec<QuestLine> = (0..8)
            .map(|i| QuestLine {
                quest: None,
                title: format!("Quest number {i} with a name far too long for the tracker's band to hold"),
                step: long.into(),
                ready: i == 2,
                main: i == 0,
                way: "North out of Castle, then east at the fingerpost, the long way round".into(),
                bearing: "North-east of you, about 400 m".into(),
            })
            .collect();
        for canvas in [(640, 360), (768, 432), (1024, 432), (560, 432), (768, 300)] {
            let lay = tracker_layout(&lines, canvas);
            assert!(!lay.bands.is_empty(), "{canvas:?}: at least one shows");
            assert_eq!(lay.bands.len() + lay.more, lines.len(), "{canvas:?}: every quest shown or counted");
            let fw = advance(Face::Fine);
            for b in &lay.bands {
                let r = b.rect;
                assert!(
                    r.x >= 0 && r.right() <= canvas.0 && i32::from(r.y) >= TRACKER_TOP,
                    "{canvas:?}: {r:?} on screen"
                );
                assert!(r.bottom() <= canvas.1 - TRACKER_FOOT, "{canvas:?}: {r:?} clear of the save card and toasts");
                assert!(b.steps.len() <= TRACKER_STEP_LINES);
                assert!(b.way.len() <= 2, "{canvas:?}: the way and the bearing, a line each at most");
                if canvas.1 >= 360 {
                    assert_eq!(lay.bands[0].way.len(), 2, "{canvas:?}: the first band has room for both");
                }
                for s in b.steps.iter().chain(&b.way).chain(core::iter::once(&b.title)) {
                    assert!(14 + s.chars().count() as i32 * fw <= i32::from(r.w), "{canvas:?}: {s:?} fits its band");
                }
            }
            if lay.more > 0 {
                assert!(lay.more_y + line_h(Face::Fine) <= canvas.1 - TRACKER_FOOT);
            }
            // The save card sits below all of it.
            let card = crate::view::SavedCard { text: "Saved to slot 3 by the ochre coat".into(), ok: true, born: 0 };
            let sc = crate::ui::saved::rect(canvas, &card);
            assert!(lay.bands.iter().all(|b| b.rect.bottom() < i32::from(sc.y)), "{canvas:?}");
        }
        // One quest: one band.
        let one = tracker_layout(&lines[..1], (768, 432));
        assert_eq!((one.bands.len(), one.more), (1, 0));
        // Compact on 640 x 360: only the top quest opens out; the rest are a name and one line,
        // and three tracked quests fit in 140 px.
        let lay = tracker_layout(&lines[..3], (640, 360));
        assert_eq!(lay.bands.len(), 3);
        assert_eq!((lay.bands[0].steps.len(), lay.bands[0].way.len()), (2, 2));
        assert!(lay.bands[1..].iter().all(|b| b.steps.len() == 1 && b.way.is_empty()));
        let used = lay.bands.last().unwrap().rect.bottom() - TRACKER_TOP;
        assert!(used <= 140, "{used} px");
        assert!(lay.bands.iter().all(|b| i32::from(b.rect.w) < 640 / 3));
    }
}
