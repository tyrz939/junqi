//! The lesson's share of the UI (PRESENTATION.md §3.2): the card a learned spell brings up and
//! the icon's flight down to the bar, the words a jar or a page leaves, and the hush's darkened
//! edges. It draws from the presenter's moment (`crate::lesson`) and the view buffers, over the
//! HUD and under a conversation; it takes no input and asks for nothing.
//!
//! The card is a plaque centred over her (under her when there is no room above): the spell's
//! icon at twice its size in a well, on a halo of the school's colour; its name in the Head
//! face; a gold rule; what it does in the Fine face (as many of its sentences as fit three
//! lines). The first spell's card stays up longer and writes one more line, slowly, under a
//! rule of the school's colour. As the card goes, the icon lifts off it and flies to the bar
//! slot the sim bound the spell to, and the slot glints.

use jane_art::font::Face;
use jane_art::palette::{self, Ix, Ramp, Tone};
use jane_core::action::Stat;

use crate::frame::Src;
use crate::lesson::{Gift, Lessons, Moment};
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui, line_h, text_w, wrap_lines};
use crate::ui::hud;
use crate::ui::style::{self, argb};
use crate::view::ViewBuffers;

/// The UI picture the halo is painted into (the title's backdrop is 0, the loading card 1, the
/// map's chart 2).
pub const HALO: u16 = 3;
/// The halo's size, px.
const HALO_PX: u16 = 92;
/// The card's width, px: it sits between the vitals' chips and the tracker on a 768 canvas.
pub const CARD_W: i32 = 320;
/// The icon's well and the icon drawn in it, px.
const WELL: i32 = 76;
const ICON: i32 = 64;

/// Draws the moment under way, if there is one. `window_open`: the vitals are stepped aside, so
/// no gauge glints.
pub fn draw(ui: &mut Ui, l: &Lessons, b: &ViewBuffers, window_open: bool) {
    let Some(&m) = l.moment() else { return };
    let (cw, ch) = ui.canvas;
    let feet = l.screen().unwrap_or((cw / 2, ch / 2));
    edges(ui, l.hush(), cw, ch);
    match m.gift {
        Gift::Spell { spell, .. } => card(ui, m, spell, feet, b),
        Gift::Growth(stat) => words(ui, m, stat, feet, window_open),
    }
}

/// The hush at the edges: the corners of the world darken a little toward the middle.
fn edges(ui: &mut Ui, hush: u8, cw: i32, ch: i32) {
    if hush == 0 {
        return;
    }
    let a = (u32::from(hush) * 30 / 255) as u8;
    for k in 0..6 {
        let d = 10 + k * 14;
        ui.fill(Rect::new(0, 0, cw, d), argb(style::INK, a));
        ui.fill(Rect::new(0, ch - d, cw, d), argb(style::INK, a));
        ui.fill(Rect::new(0, d, d, ch - 2 * d), argb(style::INK, a / 2));
        ui.fill(Rect::new(cw - d, d, d, ch - 2 * d), argb(style::INK, a / 2));
    }
}

/// `t` ticks into a `n`-tick fade, 0 to 255.
fn ramp_in(t: i64, n: i64) -> u8 {
    (t.clamp(0, n.max(1)) * 255 / n.max(1)) as u8
}

/// Where the card stands: centred across the canvas, over her head, or under her feet when
/// there is no room above.
pub fn card_rect(canvas: (i32, i32), feet: (i32, i32), h: i32) -> Rect {
    let (cw, ch) = canvas;
    let x = (cw - CARD_W) / 2;
    let above = feet.1 - 52 - h;
    let y = if above >= 8 { above } else { (feet.1 + 16).min(ch - 60 - h).max(8) };
    Rect::new(x, y, CARD_W, h)
}

/// A card's lines: its name's face, what it does wrapped, and the first spell's line wrapped.
fn layout(m: Moment, spell: jane_core::SpellId) -> (Face, String, Vec<String>, Vec<String>) {
    let d = jane_data::catalog().combat.spell(spell);
    let name = crate::text::text(d.name).to_owned();
    let text_col = CARD_W - WELL - 38;
    let face = if text_w(Face::Head, &name) <= text_col { Face::Head } else { Face::Small };
    let cols = (text_col / 8) as usize;
    let what = crate::text::card_line(crate::text::text(d.description), cols, 3);
    let lines = wrap_lines(what, cols).map(str::to_owned).collect();
    let first = if m.beats().line > 0 {
        wrap_lines(crate::text::FIRST_SPELL, ((CARD_W - 28) / 8) as usize).map(str::to_owned).collect()
    } else {
        Vec::new()
    };
    (face, name, lines, first)
}

fn card(ui: &mut Ui, m: Moment, spell: jane_core::SpellId, feet: (i32, i32), b: &ViewBuffers) {
    let beats = m.beats();
    let age = i64::from(m.age);
    let bloom = i64::from(beats.bloom);
    if age < bloom {
        return;
    }
    let t = age - bloom;
    let (face, name, lines, first) = layout(m, spell);
    let lh = line_h(Face::Fine);
    let body = (14 + line_h(face) + 10 + lines.len() as i32 * lh + 12).max(WELL + 20);
    let h = if first.is_empty() { body } else { body + 12 + first.len() as i32 * lh + 10 };
    let rise = (8 - t as i32 / 3).max(0);
    let r = card_rect(ui.canvas, feet, h);
    let (x, y, w) = (i32::from(r.x), i32::from(r.y) + rise, CARD_W);
    // The card goes as the icon leaves it (or, with no slot to fly to, at the fly's time).
    let fly = i64::from(beats.fly);
    let going = fly - 24;
    let out = if age >= going { 255 - ramp_in(age - going, 36) } else { 255 };
    let a = ramp_in(t, 24).min(out);
    if a == 0 && age < fly {
        return;
    }
    let ramp = m.gift.ramp();
    let lit = ramp.at(Tone::Light);
    if a > 0 {
        let k = |v: u8| ((u32::from(v) * u32::from(a)) / 255) as u8;
        // The plate: a banded body, a dark rim, a lit top edge, a gold hairline.
        ui.fill(Rect::new(x - 1, y - 1, w + 2, h + 2), argb(style::INK, k(200)));
        ui.fill(Rect::new(x, y, w, h), argb(style::panel_bottom(), k(236)));
        ui.fill(Rect::new(x, y, w, h / 2), argb(style::panel_top(), k(120)));
        ui.fill(Rect::new(x + 1, y + 1, w - 2, 1), argb(style::panel_lit(), k(150)));
        hairline(ui, Rect::new(x + 3, y + 3, w - 6, h - 6), style::gold_deep(), k(150));
        // The well, and the halo of the school's light behind the icon: it swells as the card
        // comes up and breathes while it stays.
        let well = Rect::new(x + 12, y + (body - WELL) / 2, WELL, WELL);
        ui.fill(well, argb(style::well(), k(230)));
        hairline(ui, well, lit, k(110));
        let breathe = 200 + (40 * tri(t as u32, 90)) / 255;
        let swell = if t < 20 { 255 } else { breathe };
        halo(ui, ramp, well, k(swell.min(255) as u8));
        if age < fly {
            let src = ui_icon(ui, spell);
            ui.sprite(
                src,
                Rect::new(i32::from(well.x) + (WELL - ICON) / 2, i32::from(well.y) + (WELL - ICON) / 2, ICON, ICON),
                0,
                k(ramp_in(t - 6, 18)),
            );
        }
        // The name, the rule growing out under it, what it does.
        let tx = x + 12 + WELL + 14;
        let ty = y + 14;
        ui.text(tx, ty, &name, Ink::new(face, style::text_bright()).shadow().alpha(k(ramp_in(t - 10, 20))));
        let rule_w = ((w - (tx - x) - 14) as i64 * i64::from(ramp_in(t - 16, 24)) / 255) as i32;
        let ry = ty + line_h(face) + 4;
        ui.rule(tx, tx + rule_w, ry, style::gold());
        for (i, s) in lines.iter().enumerate() {
            let ink = Ink::fine(style::text()).shadow().alpha(k(ramp_in(t - 28 - i as i64 * 6, 22)));
            ui.text(tx, ry + 6 + i as i32 * lh, s, ink);
        }
        // The first spell's line, written out slowly under a rule of her new colour.
        if !first.is_empty() {
            let lt = age - i64::from(beats.line);
            if lt >= 0 {
                let fy = y + body;
                let rw = ((w - 28) as i64 * i64::from(ramp_in(lt, 30)) / 255) as i32;
                ui.fill(Rect::new(x + (w - rw) / 2, fy, rw, 1), argb(lit, k(160)));
                let total: usize = first.iter().map(|s| s.chars().count() + 1).sum();
                let mut left = (lt.max(0) as usize / 2).min(total);
                for (i, s) in first.iter().enumerate() {
                    let n = s.chars().count();
                    let shown: String = s.chars().take(left).collect();
                    left = left.saturating_sub(n + 1);
                    let sx = x + (w - text_w(Face::Fine, s)) / 2;
                    ui.text(sx, fy + 8 + i as i32 * lh, &shown, Ink::fine(style::gold()).shadow().alpha(a));
                }
            }
        }
    }
    // The flight to the bar, and the glint where it lands.
    let Some(slot) = b.hud.bar.iter().position(|s| s.spell == Some(spell)) else { return };
    let to = hud::bar_slot(ui.canvas, slot);
    let land = i64::from(beats.land);
    let from = (x + 12 + WELL / 2, y + (body - WELL) / 2 + WELL / 2);
    let dest = (i32::from(to.x) + i32::from(to.w) / 2, i32::from(to.y) + i32::from(to.h) / 2);
    if (fly..land).contains(&age) {
        let n = land - fly;
        let src = ui_icon(ui, spell);
        // A few ghosts behind it, the school's colour thinning out.
        for back in (0..4).rev() {
            let s = (age - fly - back * 2).max(0);
            let e = i64::from(crate::lesson::ease(s as u32, n as u32));
            let px = from.0 + ((dest.0 - from.0) as i64 * e / 256) as i32;
            // An arc: up and over, then down onto the slot.
            let lift = (e * (256 - e) / 256 * 90 / 256) as i32;
            let py = from.1 + ((dest.1 - from.1) as i64 * e / 256) as i32 - lift;
            let size = ICON - ((ICON - 32) as i64 * e / 256) as i32;
            if back == 0 {
                ui.sprite(src, Rect::new(px - size / 2, py - size / 2, size, size), 0, 255);
            } else {
                let g = (120 - back * 30) as u8;
                ui.fill(Rect::new(px - size / 4, py - size / 4, size / 2, size / 2), argb(lit, g));
            }
        }
    } else if age >= land {
        let g = age - land;
        let span = i64::from(beats.end.saturating_sub(beats.land)).max(1);
        let a = 255 - ramp_in(g, span);
        let grow = (g * 12 / span) as i32;
        ui.fill(to.inset(2), argb(Ramp::UiGold.at(Tone::Glint), (u32::from(a) * 150 / 255) as u8));
        hairline(
            ui,
            Rect::new(
                i32::from(to.x) - grow,
                i32::from(to.y) - grow,
                i32::from(to.w) + 2 * grow,
                i32::from(to.h) + 2 * grow,
            ),
            lit,
            a,
        );
        hairline(ui, to, style::gold(), a);
    }
}

/// A jar or a page: the words above her between two short rules, and the gauge it grew glints.
fn words(ui: &mut Ui, m: Moment, stat: Stat, feet: (i32, i32), window_open: bool) {
    let b = m.beats();
    let age = i64::from(m.age);
    let bloom = i64::from(b.bloom);
    if age < bloom {
        return;
    }
    let t = age - bloom;
    let out = i64::from(b.lift) + 20;
    let a = ramp_in(t, 20).min(if age >= out { 255 - ramp_in(age - out, 30) } else { 255 });
    let s = crate::text::grew(stat);
    let lit = m.gift.ramp().at(Tone::Light);
    let (cw, _) = ui.canvas;
    let wd = text_w(Face::Small, s);
    let x = ((feet.0 - wd / 2).max(8)).min(cw - wd - 8);
    let y = (feet.1 - 78 - (t as i32 / 6).min(6)).max(64);
    let grow = (t.min(30) as i32) * 24 / 30;
    ui.fill(
        Rect::new(x - 10, y - 2, wd + 20, line_h(Face::Small) + 4),
        argb(style::INK, (u32::from(a) * 100 / 255) as u8),
    );
    ui.fill(Rect::new(x - 16 - grow, y + 9, grow + 6, 1), argb(lit, a));
    ui.fill(Rect::new(x + wd + 10, y + 9, grow + 6, 1), argb(lit, a));
    ui.text(x, y, s, Ink::small(style::text_bright()).shadow().alpha(a));
    // The gauge it grew: a light runs along it, then it glows a moment.
    if window_open {
        return;
    }
    let g = hud::VITALS_GAUGES[usize::from(stat != Stat::Strength)];
    let (gx, gw) = (i32::from(g.x), i32::from(g.w));
    if t < 30 {
        let at = gx + (gw as i64 * t / 30) as i32;
        ui.fill(Rect::new(at - 6, i32::from(g.y) - 1, 12, i32::from(g.h) + 2), argb(Ramp::UiGold.at(Tone::Glint), 200));
    } else if t < 70 {
        let a = 255 - ramp_in(t - 30, 40);
        hairline(ui, g.inset(-2), lit, a);
    }
}

/// A one-pixel frame round `r`.
fn hairline(ui: &mut Ui, r: Rect, ix: Ix, a: u8) {
    let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
    let c = argb(ix, a);
    ui.fill(Rect::new(x, y, w, 1), c);
    ui.fill(Rect::new(x, y + h - 1, w, 1), c);
    ui.fill(Rect::new(x, y + 1, 1, h - 2), c);
    ui.fill(Rect::new(x + w - 1, y + 1, 1, h - 2), c);
}

/// A triangle wave over `n` ticks, 0 to 255.
fn tri(t: u32, n: u32) -> u32 {
    let p = t % n.max(1);
    let half = n / 2;
    if p < half { p * 255 / half.max(1) } else { (n - p) * 255 / half.max(1) }
}

/// The spell's icon on the UI's page.
fn ui_icon(ui: &Ui, spell: jane_core::SpellId) -> Src {
    ui.art.icon(jane_data::catalog().combat.spell(spell).icon)
}

/// The halo behind the icon: a soft disc of `ramp`'s light, painted once per colour.
fn halo(ui: &mut Ui, ramp: Ramp, well: Rect, a: u8) {
    let [r, g, b] = palette::rgb(ramp.at(Tone::Light));
    let want = 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
    let n = HALO_PX;
    let img = ui.image_mut(HALO, n, n);
    let mid = usize::from(n / 2) * usize::from(n) + usize::from(n / 2);
    if img.argb.get(mid).is_none_or(|&c| c | 0xff00_0000 != want) {
        let c = i32::from(n) / 2;
        for y in 0..i32::from(n) {
            for x in 0..i32::from(n) {
                let d2 = (x - c) * (x - c) + (y - c) * (y - c);
                let k = (255 - d2 * 255 / (c * c)).max(0) as u32;
                let k = k * k / 255;
                img.argb[(y * i32::from(n) + x) as usize] = k << 24 | (want & 0x00ff_ffff);
            }
        }
        ui.image_changed(HALO);
    }
    let (cx, cy) = (i32::from(well.x) + i32::from(well.w) / 2, i32::from(well.y) + i32::from(well.h) / 2);
    let s = i32::from(n);
    ui.image(HALO, Src { x: 0, y: 0, w: n, h: n }, Rect::new(cx - s / 2, cy - s / 2, s, s), a);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_keeps_clear_of_her_and_on_the_canvas() {
        let canvas = (768, 432);
        // She stands in the middle: the card is over her head.
        let r = card_rect(canvas, (384, 216), 100);
        assert!(r.bottom() < 216 - 44, "{r:?} over her head");
        // Near the top of a small room: under her feet instead, still on the canvas.
        let r = card_rect(canvas, (384, 90), 100);
        assert!(i32::from(r.y) > 90 && r.bottom() < 432 - 100, "{r:?}");
        // Centred, between the vitals' chips and the tracker.
        assert_eq!(i32::from(r.x) + CARD_W / 2, 384);
        assert!(i32::from(r.x) > 220 && r.right() < 768 - 208);
    }
}
