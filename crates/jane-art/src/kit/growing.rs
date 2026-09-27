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
                // One stone: its breadth and its place a little different by the seed.
                let (sw, sh) = if n == 1 {
                    (w - 4 - 2 * (hh >> 1 & 1) as i32, (h - 4).min(w - 2) - 2 * (hh >> 2 & 1) as i32)
                } else {
                    (8 + (hh % 8) as i32, 7 + (hh >> 4 & 7) as i32)
                };
                let x = if n == 1 { 2 + (hh & 1) as i32 } else { 1 + ((hh >> 8) % (w - sw - 1).max(1) as u32) as i32 };
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
        "bud" | "seed" => {
            // A bud rising from the ground (it glows when it wakes), or a seed just sprouting.
            let cx = w / 2;
            c.ellipse(Rect::new(cx - 4, foot - 4, 8, 3), Ramp::Bark.at(Tone::Shade), 1);
            c.line((cx, foot - 3), (cx, foot - 7), Ramp::Leaf.at(Tone::Base), 2, 2);
            c.ellipse_lit(Rect::new(cx - 4, foot - 7, 4, 3), Ramp::Leaf, Z::flat(2));
            if k.look.shape == "bud" {
                c.set_emitting(state == State::On);
                c.ellipse_lit(Rect::new(cx - 2, foot - 13, 6, 7), k.accent, Z::flat(3));
                c.set_emitting(false);
            }
            Stand::Flat(4)
        }
        "vine" => {
            // A root across the ground, knotted, lit along its top.
            c.ao_contact(Rect::new(1, foot - 7, w - 2, 8), 1);
            c.polyline(&[(1, foot - 3), (8, foot - 6), (16, foot - 4), (24, foot - 7), (w - 2, foot - 5)], k.body.at(Tone::Base), 3, 2);
            c.polyline(&[(1, foot - 4), (8, foot - 7), (16, foot - 5), (24, foot - 8), (w - 2, foot - 6)], k.body.at(Tone::Light), 1, 3);
            c.line((12, foot - 5), (14, foot - 1), k.body.at(Tone::Shade), 1, 2);
            Stand::Flat(3)
        }
        "den" | "bank" => {
            // An earth bank: a lit crest, a face of earth with stones and roots in it; a den
            // has the dark mouth of a burrow.
            ao(c, 1, w - 2, foot, 6);
            let top = foot - (h - 2).min(k.fh * 16 - 2);
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline_fill(&[(1, foot - 1), (3, top + 6), (w / 3, top + 1), (2 * w / 3, top), (w - 3, top + 5), (w - 2, foot - 1)], Ix::INK, 1);
            c.inflate(&m, k.body, 5, Z::new(2, 8));
            c.retone(k.body, super::HARD);
            c.strokes(Rect::new(2, top, w - 4, 6), Ramp::Grass, crate::canvas::StrokeKind::Grass, 6, k.seed);
            if k.look.shape == "den" {
                c.ellipse(Rect::new(w / 2 - 7, foot - 10, 14, 9), Ramp::ClothBlack.at(Tone::Deep), 9);
                c.hline(w / 2 - 5, w / 2 + 4, foot - 10, k.body.at(Tone::Shade), 9);
            } else {
                for (i, x) in (6..w - 6).step_by(9).enumerate() {
                    c.line((x, foot - 8 - (i as i32 % 2) * 3), (x + 4, foot - 4), k.body.at(Tone::Deep), 1, 9);
                }
            }
            Stand::Up(&[])
        }
        "dole" | "slab" | "flat_stone" => {
            // Dressed stone: a dole stone with its hollows for the coins, a squared slab, a
            // flat stepping stone.
            ao(c, 1, w - 2, foot, 4);
            let (face, depth) = match k.look.shape {
                "slab" => (6, 6),
                "flat_stone" => (3, 7),
                _ => (6, 7),
            };
            let (top, _) = parts::box3(c, 2, w - 4, foot, face, depth, k.body, None, 2);
            c.retone(k.body, super::HARD);
            if k.look.shape == "dole" {
                for x in [top.x + 4, top.x + top.w / 2, top.right() - 7] {
                    c.ellipse(Rect::new(x, top.y + 2, 4, 3), k.body.at(Tone::Shade), 3);
                    c.dot(x + 1, top.y + 3, k.body.at(Tone::Deep), 3);
                }
            }
            Stand::Tops([(top, parts::lid_height(face)), (Rect::default(), 0)])
        }
        "hollow" | "gap" => {
            // Bare earth where something was: a patch of turned soil (and, at a wall's end,
            // two fallen stones).
            c.ellipse(Rect::new(2, foot - 10, w - 4, 9), Ramp::Bark.at(Tone::Shade), 1);
            c.ellipse(Rect::new(4, foot - 8, w - 8, 5), Ramp::Bark.at(Tone::Mid), 1);
            if k.look.shape == "gap" {
                c.ellipse_lit(Rect::new(3, foot - 7, 5, 4), Ramp::Stone, Z::flat(2));
                c.ellipse_lit(Rect::new(w - 8, foot - 5, 4, 3), Ramp::Stone, Z::flat(2));
            }
            Stand::Flat(2)
        }
        "wreck_car" | "wreck_cart" => {
            // A wreck: a rusted body on its flat tyres, a cracked screen (a car), or a cart on
            // its side with a wheel off.
            let car = k.look.shape == "wreck_car";
            let (x, bw) = (2, w - 4);
            ao(c, x, x + bw - 1, foot, 8);
            let face = if car { 14 } else { 9 };
            let (top, front) = parts::box3(c, x, bw, foot - 4, face, if car { 16 } else { 10 }, k.body, if car { None } else { Some((3, k.seed)) }, 3);
            if car {
                c.retone(k.body, super::HARD);
                // Its panels: a chrome strip along the side, the doors' seams, a dead headlamp,
                // the bonnet lit along its crown.
                c.hline(front.x + 1, front.right() - 2, front.y + 4, Ramp::Iron.at(Tone::High), 4);
                c.hline(front.x + 1, front.right() - 2, front.y + 5, Ramp::Iron.at(Tone::Shade), 4);
                for sx in [front.x + front.w / 3, front.x + 2 * front.w / 3] {
                    c.vline(sx, front.y + 1, front.bottom() - 2, k.body.at(Tone::Deep), 4);
                }
                c.disc_lit(front.right() - 5, front.y + 8, 2, Ramp::Glass, Z::flat(5));
                c.hline(top.x + 2, top.right() - 3, top.y + 3, k.body.at(Tone::Light), 4);
                c.hline(top.x + 2, top.right() - 3, top.bottom() - 3, k.body.at(Tone::Mid), 4);
                // The cab, its screen cracked, rust in blooms down the panels.
                let cab = Rect::new(x + bw / 4, top.y - 9, bw / 2, 10);
                c.rect_round(cab, k.body, 1, 3, Z::new(6, 8));
                parts::glass(c, Rect::new(cab.x + 3, cab.y + 2, cab.w - 6, 5), false, 9);
                c.line((cab.x + 4, cab.y + 2), (cab.x + 8, cab.y + 6), Ramp::HairWhite.at(Tone::Light), 1, 9);
                for i in 0..6 {
                    let hh = parts::hash(k.seed, i, 70);
                    let (rx, ry) = (front.x + 2 + (hh % (front.w - 6) as u32) as i32, front.y + 2 + ((hh >> 8) % (front.h - 4) as u32) as i32);
                    c.fill_rect(Rect::new(rx, ry, 3, 2), Ramp::Copper.at(Tone::Shade), 4);
                    c.dot(rx, ry, Ramp::Copper.at(Tone::Base), 4);
                }
                for fx in [x + 6, x + bw - 12] {
                    c.ellipse(Rect::new(fx, foot - 7, 7, 6), Ramp::ClothBlack.at(Tone::Base), 2);
                    c.dot(fx + 3, foot - 5, Ramp::Iron.at(Tone::Light), 2);
                }
            } else {
                c.ellipse(Rect::new(x + bw - 12, foot - 10, 11, 9), k.trim.at(Tone::Base), 2);
                c.ellipse(Rect::new(x + bw - 9, foot - 8, 5, 5), Ix::CLEAR, 0);
                c.line((x + bw - 7, foot - 6), (x + bw - 7, foot - 9), k.trim.at(Tone::Light), 1, 3);
                c.fill_rect(Rect::new(x + 2, foot - 4, 3, 4), k.trim.at(Tone::Shade), 2);
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
