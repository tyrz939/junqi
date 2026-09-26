//! `vegetation` and `debris` (ART.md §2.3), the county's first: what grows in the draw list
//! (an apple tree, a stump, a log, flowers, herbs, a white rose, crops, a dead tree) and what
//! lies about (a boulder, a rock, rubble, a rock face, a slag heap, bones). The rest of both
//! families are step 7's.
//!
//! A crown is one soft volume with its leaves laid over it in clusters of three to five px,
//! each lit on its top-left and shaded under it, so a tree is a lit mass and not a flat blob.

use jane_core::grid::Rect;

use super::parts::{self, ao, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let _ = state;
    let (w, h, foot) = (k.w, k.h, k.foot());
    Some(match k.look.shape {
        "apple_tree" | "dry_tree" => {
            let dry = k.look.shape == "dry_tree";
            let cx = w / 2;
            ao(c, cx - 8, cx + 7, foot, 6);
            // The trunk, forking into two limbs under the crown.
            post(c, cx - 2, foot - 16, foot - 1, 4, k.trim, 4);
            c.line((cx - 1, foot - 14), (cx - 6, foot - 22), k.trim.at(Tone::Base), 2, 5);
            c.line((cx + 1, foot - 14), (cx + 6, foot - 24), k.trim.at(Tone::Base), 2, 5);
            if dry {
                for (x0, y0, x1, y1) in [(cx - 6, foot - 22, cx - 10, foot - 30), (cx + 6, foot - 24, cx + 11, foot - 31), (cx, foot - 16, cx + 1, foot - 33)] {
                    c.line((x0, y0), (x1, y1), k.trim.at(Tone::Mid), 1, 6);
                }
                return Some(Stand::Up(&[]));
            }
            let crown = Rect::new(cx - 13, (foot - 44).max(0), 26, 26);
            crown_volume(c, k, crown);
            // Apples, a few, lit.
            for i in 0..6 {
                let hh = parts::hash(k.seed, i, 33);
                let (x, y) = (crown.x + 4 + (hh % (crown.w - 8) as u32) as i32, crown.y + 8 + ((hh >> 8) % (crown.h - 12) as u32) as i32);
                c.fill_rect(Rect::new(x, y, 2, 2), k.accent.at(Tone::Base), 12);
                c.dot(x, y, k.accent.at(Tone::Light), 12);
            }
            Stand::Up(&[])
        }
        "stump" => {
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 5);
            c.polygon_lit(&[(cx - 5, foot - 8), (cx + 4, foot - 8), (cx + 5, foot - 1), (cx - 6, foot - 1)], k.trim, 110, Z::new(2, 4));
            c.ellipse(Rect::new(cx - 5, foot - 11, 10, 5), k.body.at(Tone::Light), 5);
            c.ellipse(Rect::new(cx - 3, foot - 10, 6, 3), k.body.at(Tone::Base), 5);
            c.dot(cx - 1, foot - 9, k.body.at(Tone::Mid), 5);
            Stand::Up(&[])
        }
        "log" => {
            ao(c, 1, w - 2, foot, 4);
            c.polygon_lit(&[(3, foot - 9), (w - 4, foot - 9), (w - 4, foot - 1), (3, foot - 1)], k.trim, 0, Z::new(2, 5));
            for y in [foot - 7, foot - 4] {
                c.hline(5, w - 6, y, k.trim.at(Tone::Shade), 5);
            }
            c.ellipse(Rect::new(w - 7, foot - 9, 5, 9), k.body.at(Tone::Light), 6);
            c.dot(w - 5, foot - 5, k.body.at(Tone::Mid), 6);
            Stand::Up(&[])
        }
        "flowers" | "flowerbed" | "herb" | "rose" | "crop" => {
            // A bed or a patch: leaves in clusters, the blooms or heads over them.
            let bed = k.look.shape == "flowerbed";
            let r = Rect::new(1, foot - (h - 2).min(k.fh * 16 - 2), w - 2, (h - 2).min(k.fh * 16 - 2));
            if bed {
                c.fill_normal(r, Ramp::Bark.at(Tone::Shade), FLAT, 1);
                c.hline(r.x, r.right() - 1, r.y, Ramp::Stone.at(Tone::Light), 2);
                c.hline(r.x, r.right() - 1, r.bottom() - 1, Ramp::Stone.at(Tone::Mid), 2);
            }
            let step = if k.look.shape == "crop" { 4 } else { 5 };
            let mut i = 0;
            for y in (r.y + 3..r.bottom() - 1).step_by(step as usize) {
                for x in (r.x + 2..r.right() - 2).step_by(step as usize) {
                    let hh = parts::hash(k.seed, i, 44);
                    i += 1;
                    let (x, y) = (x + (hh & 1) as i32, y + (hh >> 1 & 1) as i32);
                    // A plant: a little lit mound of leaves.
                    let mut m = Canvas::new(c.w(), c.h());
                    m.ellipse(Rect::new(x - 2, y - 1, 5, 4), Ix::INK, 1);
                    c.inflate(&m, k.body, 1, Z::new(2, 3));
                    c.dot(x - 1, y - 1, k.body.at(Tone::Light), 3);
                    if k.look.shape != "crop" && k.look.shape != "herb" && hh >> 4 & 1 == 0 {
                        c.dot(x, y - 2, k.accent.at(Tone::Light), 4);
                        c.dot(x + 1, y - 2, k.accent.at(Tone::Base), 4);
                        c.dot(x, y - 3, k.accent.at(Tone::High), 4);
                    }
                }
            }
            Stand::Flat(4)
        }
        "boulder" | "rock" | "rubble" | "rockface" | "heap" => {
            // Stones: each one soft volume, a crack or a chip on some, moss at the foot.
            let n = match k.look.shape {
                "rubble" => 6,
                "heap" => 5,
                "rockface" => 4,
                _ => 1,
            };
            ao(c, 1, w - 2, foot, 5);
            for i in 0..n {
                let hh = parts::hash(k.seed, i, 55);
                let (sw, sh) = if n == 1 { (w - 4, (h - 4).min(w - 2)) } else { (8 + (hh % 8) as i32, 7 + (hh >> 4 & 7) as i32) };
                let x = if n == 1 { 2 } else { 1 + ((hh >> 8) % (w - sw - 1).max(1) as u32) as i32 };
                let y = foot - sh - if n == 1 { 0 } else { ((hh >> 12) % (h / 3).max(1) as u32) as i32 };
                let mut m = Canvas::new(c.w(), c.h());
                m.polyline_fill(&[(x + 2, y), (x + sw - 3, y + 1), (x + sw - 1, y + sh / 2), (x + sw - 2, y + sh - 1), (x + 1, y + sh - 1), (x, y + sh / 3)], Ix::INK, 1);
                c.inflate(&m, k.body, (sw.min(sh) / 3).max(2), Z::new(2 + i as u8, 6 + i as u8));
                c.retone(k.body, super::HARD);
                if hh >> 16 & 1 == 0 {
                    c.line((x + sw / 2, y + 2), (x + sw / 2 + 1, y + sh / 2), Ix::SEAM, 1, 7 + i as u8);
                }
            }
            Stand::Up(&[])
        }
        "bones" => {
            // A few bones on the ground, a skull among them.
            let b = Ramp::Bone;
            c.line((2, foot - 3), (9, foot - 6), b.at(Tone::Light), 2, 1);
            c.line((6, foot - 2), (13, foot - 3), b.at(Tone::Base), 2, 1);
            c.ellipse_lit(Rect::new(8, foot - 10, 6, 5), b, Z::flat(2));
            c.dot(10, foot - 8, Ix::SEAM, 2);
            Stand::Flat(2)
        }
        _ => return None,
    })
}

/// A tree's crown in `r`: one soft volume in the body's greens, then leaf clusters over it, lit
/// on their top-left and shaded under them, denser where the light falls.
fn crown_volume(c: &mut Canvas, k: &Kit, r: Rect) {
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), Ix::INK, 1);
    // Lobes round the rim, so the crown's edge is leafy and not a circle.
    for i in 0..9 {
        let a = jane_core::angle::Angle((i * 65536 / 9 + (k.seed & 0xfff) as i32) as u16);
        let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
        let (cx, cy) = (r.x + r.w / 2 + ((co * (r.w / 2 - 5)) >> 15), r.y + r.h / 2 + ((s * (r.h / 2 - 5)) >> 15));
        m.ellipse(Rect::new(cx - 5, cy - 4, 10, 9), Ix::INK, 1);
    }
    c.inflate(&m, k.body, 7, Z::new(8, 14));
    c.retone(k.body, [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Light]);
    for i in 0..(r.w * r.h / 18) {
        let hh = parts::hash(k.seed, i, 66);
        let (x, y) = (r.x + (hh % r.w as u32) as i32, r.y + ((hh >> 8) % r.h as u32) as i32);
        let Some((_, t)) = Ramp::of(c.get(x, y)).filter(|(rr, _)| *rr == k.body) else { continue };
        // A cluster: a lit cap over the leaf's own tone and a shade under it.
        let up = t.step(1).min(Tone::High);
        for (dx, dy, tone) in [(0, 0, up), (1, 0, up), (-1, 1, t), (0, 1, t), (1, 1, t), (0, 2, t.step(-2))] {
            c.tint(x + dx, y + dy, k.body, tone);
        }
    }
}
