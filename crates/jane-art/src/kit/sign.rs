//! `sign` (ART.md §2.3): boards on posts, hanging signs, fingerposts, name boards, notice
//! boards, and the stones the county writes on. Writing is `text_rows` of illegible strokes;
//! the words are in the dialogue, never on the sprite.
//!
//! Shapes: `post`, `hanging`, `fingerpost`, `name_board`, `notice`, `timetable`, `milestone`,
//! `gravestone`, `headstone`, `standing_stone`, `memorial`, `chalk`, `note`.

use jane_core::grid::Rect;

use super::parts::{self, ao, band, blocks, planks, post, writing};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let _ = state;
    let (w, foot) = (k.w, k.foot());
    let rows = i32::from(k.look.text_rows);
    Some(match k.look.shape {
        "post" => {
            // A board across one stout post, its top edge lit, the writing burnt in.
            let (bw, bh) = ((w - 6).min(26), 11);
            let bx = (w - bw) / 2;
            let by = (foot - 13 - bh).max(1);
            ao(c, w / 2 - 3, w / 2 + 2, foot - 2, 4);
            post(c, w / 2 - 2, by + 2, foot - 3, 4, k.trim, 3);
            board(c, k, Rect::new(bx, by, bw, bh), rows.max(2));
            Stand::Up(&[])
        }
        "hanging" => {
            // A post, an arm off it, and a painted board hung on two chains, swinging a little
            // toward the viewer.
            let top = foot - (k.h - 3).min(40);
            ao(c, 2, 7, foot - 2, 4);
            post(c, 3, top, foot - 3, 4, k.trim, 3);
            c.fill_normal(Rect::new(3, top + 2, w - 5, 2), k.trim.at(Tone::Base), parts::south(), 4);
            c.hline(4, w - 3, top + 2, k.trim.at(Tone::Light), 4);
            c.dot(w - 3, top + 4, k.trim.at(Tone::Mid), 4);
            let (bx, bw, by, bh) = (8, w - 11, top + 7, 12);
            for x in [bx + 2, bx + bw - 3] {
                c.vline(x, top + 4, by - 1, Ramp::Iron.at(Tone::Shade), 5);
            }
            board(c, k, Rect::new(bx, by, bw, bh), 0);
            emblem(c, k, Rect::new(bx + 3, by + 2, bw - 6, bh - 4));
            Stand::Up(&[])
        }
        "fingerpost" => {
            // A white post with two arms pointing opposite ways at different heights.
            let top = foot - (k.h - 4).min(34);
            ao(c, w / 2 - 3, w / 2 + 2, foot - 2, 4);
            post(c, w / 2 - 2, top, foot - 3, 3, k.trim, 3);
            c.disc_lit(w / 2 - 1, top, 2, k.trim, Z::flat(4));
            arrow(c, k, w / 2 + 1, top + 4, w / 2 - 2, true);
            arrow(c, k, 1, top + 11, w / 2 - 3, false);
            Stand::Up(&[])
        }
        "name_board" => {
            // A long board on two posts: the station's name, the Halt's, the county's.
            let bh = 12;
            let by = foot - 6 - bh - (k.h - 16 - bh).clamp(4, 10);
            ao(c, 2, w - 3, foot - 2, 4);
            for x in [3, w - 7] {
                post(c, x, by + 3, foot - 3, 4, k.trim, 3);
            }
            board(c, k, Rect::new(1, by, w - 2, bh), rows.max(1));
            Stand::Up(&[])
        }
        "notice" | "timetable" => {
            // A framed board under a little roof, with papers pinned to it.
            let bh = (k.h - 22).clamp(12, 20);
            let by = foot - 6 - bh;
            ao(c, 2, w - 3, foot - 2, 4);
            for x in [2, w - 6] {
                post(c, x, by - 2, foot - 3, 4, k.trim, 3);
            }
            planks(c, Rect::new(1, by, w - 2, bh), k.body, (bh / 5).max(2), true, false, k.seed, 5);
            frame(c, Rect::new(1, by, w - 2, bh), k.trim, 6);
            // The roof over it: a slate strip lit on its front edge.
            c.fill_normal(Rect::new(0, by - 4, w, 3), Ramp::Slate.at(Tone::Base), crate::canvas::normal(0, -70), 7);
            c.hline(0, w - 1, by - 2, Ramp::Slate.at(Tone::Light), 7);
            papers(c, k, Rect::new(3, by + 2, w - 6, bh - 4), k.look.shape == "timetable");
            Stand::Up(&[])
        }
        "milestone" => {
            // A squat stone with a rounded head and a carved number.
            let top = foot - 14;
            ao(c, 2, w - 3, foot - 1, 5);
            stone(c, k, Rect::new(3, top, w - 6, foot - top), 3);
            writing(c, Rect::new(5, top + 5, w - 10, 6), 2, k.body.at(Tone::Deep), k.seed, 6);
            Stand::Up(&[])
        }
        "gravestone" | "headstone" => {
            // A slab with a rounded head, leaning a px, carved, and moss at its foot.
            let top = foot - (k.h - 4).min(22);
            ao(c, 1, w - 2, foot - 1, 5);
            stone(c, k, Rect::new(2, top, w - 4, foot - top - 1), 5);
            if k.look.shape == "headstone" {
                // A cross cut in its head.
                c.vline(w / 2 - 1, top + 3, top + 8, k.body.at(Tone::Deep), 6);
                c.hline(w / 2 - 3, w / 2 + 1, top + 5, k.body.at(Tone::Deep), 6);
            }
            writing(c, Rect::new(4, top + 10, w - 8, 8), rows.max(2), k.body.at(Tone::Deep), k.seed, 6);
            moss(c, k, Rect::new(2, foot - 5, w - 4, 4));
            Stand::Up(&[])
        }
        "standing_stone" => {
            // A rough monolith: a tall irregular volume, lichen in clusters, a scratch or two.
            let top = foot - (k.h - 2).min(36);
            ao(c, 0, w - 1, foot - 1, 6);
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline_fill(
                &[(3, foot - 1), (2, top + 8), (5, top + 1), (9, top), (12, top + 5), (13, foot - 2)],
                Ix::INK,
                1,
            );
            c.inflate(&m, k.body, 3, Z::new(2, 6));
            c.retone(k.body, super::HARD);
            for (i, (x, y)) in [(5, top + 9), (9, top + 16), (6, top + 22)].into_iter().enumerate() {
                if parts::hash(k.seed, i as i32, 4) % 2 == 0 {
                    lichen(c, x, y, 5);
                }
            }
            writing(c, Rect::new(5, top + 12, 6, 6), rows, k.body.at(Tone::Deep), k.seed, 6);
            moss(c, k, Rect::new(3, foot - 4, w - 6, 3));
            Stand::Up(&[])
        }
        "memorial" => {
            // A stepped plinth, a tapering shaft with a cross over it, the names cut in its face.
            ao(c, 1, w - 2, foot - 1, 6);
            blocks(c, Rect::new(2, foot - 6, w - 4, 6), k.body, 3, 7, false, k.seed, 3);
            c.fill_normal(Rect::new(2, foot - 8, w - 4, 2), k.body.at(Tone::Light), crate::canvas::FLAT, 4);
            blocks(c, Rect::new(6, foot - 12, w - 12, 4), k.body, 4, 10, false, k.seed ^ 1, 5);
            let (sx, sw) = (w / 2 - 4, 8);
            let top = foot - (k.h - 8).min(40);
            c.polygon_lit(
                &[(sx + 1, top + 6), (sx + sw - 2, top + 6), (sx + sw - 1, foot - 12), (sx, foot - 12)],
                k.body,
                80,
                Z::flat(6),
            );
            c.retone(k.body, super::HARD);
            // The cross.
            c.fill_normal(Rect::new(w / 2 - 1, top, 2, 7), k.body.at(Tone::Base), parts::south(), 7);
            c.fill_normal(Rect::new(w / 2 - 3, top + 2, 6, 2), k.body.at(Tone::Base), parts::south(), 7);
            c.vline(w / 2 - 1, top, top + 6, k.body.at(Tone::Light), 7);
            writing(
                c,
                Rect::new(sx + 2, top + 10, sw - 4, foot - 14 - top - 10),
                rows.max(4),
                k.body.at(Tone::Deep),
                k.seed,
                7,
            );
            Stand::Up(&[])
        }
        "portrait" => {
            // A portrait in a gilt frame on an easel: a figure in black, the face scratched out.
            let cx = w / 2;
            ao(c, cx - 8, cx + 7, foot, 4);
            for (x0, x1) in [(cx - 7, cx - 10), (cx + 6, cx + 9)] {
                c.line((x0, foot - 26), (x1, foot - 1), k.trim.at(Tone::Base), 2, 3);
            }
            let r = Rect::new(cx - 9, foot - 30, 18, 20);
            c.rect_bevel(r, k.accent, 2, Z::new(5, 7));
            let canvas = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
            c.fill_normal(canvas, Ramp::ClothGreen.at(Tone::Shade), parts::south(), 6);
            c.polygon_cloth(
                &[
                    (cx - 4, canvas.y + 9),
                    (cx + 3, canvas.y + 9),
                    (cx + 5, canvas.bottom() - 1),
                    (cx - 6, canvas.bottom() - 1),
                ],
                Ramp::ClothBlack,
                60,
                Z::flat(7),
            );
            c.ellipse(Rect::new(cx - 3, canvas.y + 2, 6, 7), Ramp::Skin.at(Tone::Mid), 7);
            for i in 0..3 {
                c.line((cx - 3 + i, canvas.y + 2), (cx + 1 + i, canvas.y + 8), Ramp::ClothLinen.at(Tone::Light), 1, 8);
            }
            Stand::Up(&[])
        }
        "chalk" => {
            // Chalk on the flags: a mark and an arrow, rubbed (or pencil, in the accent).
            let y = foot - 7;
            let chalk = if k.look.materials.accent.is_some() {
                k.accent.at(Tone::Base)
            } else {
                Ramp::HairWhite.at(Tone::Light)
            };
            c.line((3, y + 3), (8, y - 1), chalk, 2, 1);
            c.line((8, y - 1), (12, y + 3), chalk, 2, 1);
            c.line((4, y + 5), (11, y + 5), Ramp::HairWhite.at(Tone::Base), 2, 1);
            Stand::Flat(1)
        }
        "note" => {
            // A folded page on the ground, its corner lifted, its lines in pencil.
            let r = Rect::new(3, foot - 11, w - 6, 9);
            c.ao_contact(Rect::new(r.x - 1, r.y + 1, r.w + 2, r.h + 1), 1);
            c.fill_normal(r, k.body.at(Tone::Light), crate::canvas::FLAT, 1);
            c.hline(r.x, r.right() - 1, r.y, k.body.at(Tone::High), 1);
            c.vline(r.x + r.w / 2, r.y + 1, r.bottom() - 1, k.body.at(Tone::Base), 1);
            c.fill_normal(Rect::new(r.right() - 2, r.y, 2, 2), k.body.at(Tone::Mid), crate::canvas::normal(40, -40), 2);
            writing(c, Rect::new(r.x + 1, r.y + 2, r.w / 2 - 2, r.h - 3), 3, Ramp::Slate.at(Tone::Mid), k.seed, 1);
            writing(
                c,
                Rect::new(r.x + r.w / 2 + 1, r.y + 2, r.w / 2 - 2, r.h - 3),
                3,
                Ramp::Slate.at(Tone::Mid),
                k.seed ^ 5,
                1,
            );
            Stand::Flat(2)
        }
        _ => return None,
    })
}

/// A painted board: planks across in the body's ramp, a frame of the trim, the writing burnt in.
fn board(c: &mut Canvas, k: &Kit, r: Rect, rows: i32) {
    planks(c, r, k.body, (r.h / 4).max(2), true, false, k.seed, 5);
    frame(c, r, k.trim, 6);
    if rows > 0 {
        // Letters painted pale on a dark board, burnt dark into a pale one.
        let pale = crate::palette::luma(k.accent.at(Tone::Base)) > crate::palette::luma(k.body.at(Tone::Base));
        let ink = k.accent.at(if pale { Tone::Light } else { Tone::Shade });
        writing(c, Rect::new(r.x + 3, r.y + 2, r.w - 6, r.h - 4), rows, ink, k.seed, 6);
    }
}

/// A frame round `r` in `ramp`: lit on its top and left, shaded on its bottom and right.
fn frame(c: &mut Canvas, r: Rect, ramp: Ramp, z: u8) {
    c.hline(r.x, r.right() - 1, r.y, ramp.at(Tone::Light), z);
    c.vline(r.x, r.y, r.bottom() - 1, ramp.at(Tone::Base), z);
    c.hline(r.x, r.right() - 1, r.bottom() - 1, ramp.at(Tone::Shade), z);
    c.vline(r.right() - 1, r.y, r.bottom() - 1, ramp.at(Tone::Mid), z);
}

/// A hanging sign's painted device in the accent: a round mark with its light and a lit rim.
fn emblem(c: &mut Canvas, k: &Kit, r: Rect) {
    let d = r.h.min(r.w);
    c.ellipse_lit(Rect::new(r.x + (r.w - d) / 2, r.y, d, d), k.accent, Z::flat(7));
    c.retone(k.accent, super::HARD);
}

/// A fingerpost's arm from `x`, `len` px long, pointing right or left, its tip cut to a point.
fn arrow(c: &mut Canvas, k: &Kit, x: i32, y: i32, len: i32, right: bool) {
    let r = Rect::new(x, y, len, 4);
    planks(c, r, k.body, 1, true, false, k.seed, 5);
    let tip = if right { r.right() } else { r.x - 1 };
    let dir = if right { 1 } else { -1 };
    c.fill_normal(Rect::new(tip.min(tip + dir), y + 1, 1, 2), k.body.at(Tone::Base), parts::south(), 5);
    c.hline(r.x, r.right() - 1, y, k.body.at(Tone::Light), 5);
    writing(c, Rect::new(r.x + 2, y + 1, len - 4, 2), 1, k.accent.at(Tone::Shade), k.seed ^ x as u32, 5);
}

/// Papers pinned to a notice board: two or three sheets askew, each with its lines, one with a
/// grid (a timetable).
fn papers(c: &mut Canvas, k: &Kit, r: Rect, grid: bool) {
    let paper = Ramp::ClothLinen;
    let n = (r.w / 12).clamp(1, 3);
    for i in 0..n {
        let h = parts::hash(k.seed, i, 21);
        let pw = (r.w / n - 2).min(12);
        let x = r.x + i * (r.w / n) + (h % 2) as i32;
        let ph = (r.h - (h >> 2 & 1) as i32).max(6);
        let y = r.y + (h >> 3 & 1) as i32;
        let pr = Rect::new(x, y, pw, ph);
        c.fill_normal(pr, paper.at(Tone::Base), parts::south(), 7);
        c.hline(x, x + pw - 1, y, paper.at(Tone::Light), 7);
        c.dot(x + pw / 2, y, Ramp::Brass.at(Tone::Light), 8);
        if grid && i == 0 {
            for gy in (y + 3..y + ph - 1).step_by(2) {
                c.hline(x + 1, x + pw - 2, gy, paper.at(Tone::Mid), 7);
            }
            c.vline(x + pw / 3, y + 2, y + ph - 2, paper.at(Tone::Mid), 7);
        } else {
            writing(c, Rect::new(x + 1, y + 2, pw - 2, ph - 3), (ph - 3) / 2, Ramp::Slate.at(Tone::Shade), h, 7);
        }
    }
    let _ = band;
}

/// Worked stone: a slab with a rounded head, lit on its left, shaded on its right, a chip or
/// two.
fn stone(c: &mut Canvas, k: &Kit, r: Rect, round: i32) {
    let mut m = Canvas::new(c.w(), c.h());
    m.fill_rect(Rect::new(r.x, r.y + round, r.w, r.h - round), Ix::INK, 1);
    m.ellipse(Rect::new(r.x, r.y, r.w, 2 * round + 1), Ix::INK, 1);
    c.inflate(&m, k.body, 2, Z::new(2, 5));
    c.retone(k.body, super::HARD);
    // A face flat toward the viewer between the rounded edges: the carving reads on it.
    for y in r.y + round + 1..r.bottom() - 1 {
        for x in r.x + 2..r.right() - 2 {
            c.tint(x, y, k.body, Tone::Base);
        }
    }
    let h = parts::hash(k.seed, r.x, r.y);
    if h % 2 == 0 {
        c.tint(r.right() - 3, r.y + round + 2, k.body, Tone::Mid);
        c.tint(r.right() - 3, r.y + round + 3, k.body, Tone::Mid);
    }
}

/// Moss at a stone's foot: clusters of two or three in the leaf's greens.
fn moss(c: &mut Canvas, k: &Kit, r: Rect) {
    for i in 0..(r.w / 3).max(1) {
        let h = parts::hash(k.seed, i, 40);
        if h % 3 == 0 {
            continue;
        }
        let x = r.x + i * 3 + (h >> 2 & 1) as i32;
        let y = r.bottom() - 1 - (h >> 3 & 1) as i32;
        for (dx, dy, t) in [(0, 0, Tone::Base), (1, 0, Tone::Shade), (0, -1, Tone::Light)] {
            if c.get(x + dx, y + dy).is_opaque() {
                c.dot(x + dx, y + dy, Ramp::Leaf.at(t), 6);
            }
        }
    }
}

/// A patch of lichen at `(x, y)`: a pale crust of three px.
fn lichen(c: &mut Canvas, x: i32, y: i32, z: u8) {
    for (dx, dy, t) in [(0, 0, Tone::Light), (1, 0, Tone::Base), (0, 1, Tone::Base)] {
        if c.get(x + dx, y + dy).is_opaque() {
            c.dot(x + dx, y + dy, Ramp::Reed.at(t), z);
        }
    }
}
