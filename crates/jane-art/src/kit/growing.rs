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
                for (x0, y0, x1, y1) in [
                    (cx - 6, foot - 22, cx - 10, foot - 30),
                    (cx + 6, foot - 24, cx + 11, foot - 31),
                    (cx, foot - 16, cx + 1, foot - 33),
                ] {
                    c.line((x0, y0), (x1, y1), k.trim.at(Tone::Mid), 1, 6);
                }
                return Some(Stand::Up(&[]));
            }
            let crown = Rect::new(cx - 13, (foot - 44).max(0), 26, 26);
            crown_volume(c, k, crown);
            // Apples, a few, lit; none on a tree she has picked (`open`) until it bears again.
            let apples = if state == State::Open { 0 } else { 6 };
            for i in 0..apples {
                let hh = parts::hash(k.seed, i, 33);
                let (x, y) = (
                    crown.x + 4 + (hh % (crown.w - 8) as u32) as i32,
                    crown.y + 8 + ((hh >> 8) % (crown.h - 12) as u32) as i32,
                );
                c.fill_rect(Rect::new(x, y, 2, 2), k.accent.at(Tone::Base), 12);
                c.dot(x, y, k.accent.at(Tone::Light), 12);
            }
            Stand::Up(&[])
        }
        "stump" => {
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 5);
            c.polygon_lit(
                &[(cx - 5, foot - 8), (cx + 4, foot - 8), (cx + 5, foot - 1), (cx - 6, foot - 1)],
                k.trim,
                110,
                Z::new(2, 4),
            );
            c.ellipse(Rect::new(cx - 5, foot - 11, 10, 5), k.body.at(Tone::Light), 5);
            c.ellipse(Rect::new(cx - 3, foot - 10, 6, 3), k.body.at(Tone::Base), 5);
            c.dot(cx - 1, foot - 9, k.body.at(Tone::Mid), 5);
            Stand::Up(&[])
        }
        "log" => {
            ao(c, 1, w - 2, foot, 4);
            c.polygon_lit(
                &[(3, foot - 9), (w - 4, foot - 9), (w - 4, foot - 1), (3, foot - 1)],
                k.trim,
                0,
                Z::new(2, 5),
            );
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
            // Stones, each cut into faces by `rock::stone`: a lit top, sides facing out along
            // their edges, a crack and moss on a big one; several lie back to front, a stone in
            // front standing proud of the ones behind it.
            let n = match k.look.shape {
                "rubble" => 6,
                "heap" => 5,
                "rockface" => 4,
                _ => 1,
            };
            ao(c, 1, w - 2, foot, 5);
            let mut stones: Vec<(Rect, u32)> = (0..n)
                .map(|i| {
                    let hh = parts::hash(k.seed, i, 55);
                    let (sw, sh) = if n == 1 {
                        (w - 3 - 2 * (hh >> 1 & 1) as i32, (h - 3).min(w - 4) - 2 * (hh >> 2 & 1) as i32)
                    } else {
                        (8 + (hh % 8) as i32, 7 + (hh >> 4 & 7) as i32)
                    };
                    let x =
                        if n == 1 { 1 + (hh & 1) as i32 } else { 1 + ((hh >> 8) % (w - sw - 1).max(1) as u32) as i32 };
                    let y = foot - sh - if n == 1 { 0 } else { ((hh >> 12) % (h / 3).max(1) as u32) as i32 };
                    (Rect::new(x, y.max(0), sw, sh), hh)
                })
                .collect();
            stones.sort_by_key(|(r, _)| (r.bottom(), r.x));
            for (i, (r, hh)) in stones.into_iter().enumerate() {
                let big = r.w >= 14;
                let dress = crate::rock::Dress { moss: big && hh >> 20 & 1 == 0, crack: big, edges: big };
                crate::rock::stone(c, r, k.body, hh, dress, 2 + 2 * i as u8);
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
            c.polyline(
                &[(1, foot - 3), (8, foot - 6), (16, foot - 4), (24, foot - 7), (w - 2, foot - 5)],
                k.body.at(Tone::Base),
                3,
                2,
            );
            c.polyline(
                &[(1, foot - 4), (8, foot - 7), (16, foot - 5), (24, foot - 8), (w - 2, foot - 6)],
                k.body.at(Tone::Light),
                1,
                3,
            );
            c.line((12, foot - 5), (14, foot - 1), k.body.at(Tone::Shade), 1, 2);
            Stand::Flat(3)
        }
        "den" | "bank" => {
            // An earth bank: a lit crest, a face of earth with stones and roots in it; a den
            // has the dark mouth of a burrow.
            ao(c, 1, w - 2, foot, 6);
            let top = foot - (h - 2).min(k.fh * 16 - 2);
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline_fill(
                &[(1, foot - 1), (3, top + 6), (w / 3, top + 1), (2 * w / 3, top), (w - 3, top + 5), (w - 2, foot - 1)],
                Ix::INK,
                1,
            );
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
            if car {
                wreck_car(c, k, x, bw, foot);
            } else {
                let face = 9;
                parts::box3(c, x, bw, foot - 4, face, 10, k.body, Some((3, k.seed)), 3);
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
    c.retone(
        k.body,
        [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Light],
    );
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

/// A car left where it stopped, seen from the side as a saloon of the thirties is: a long
/// bonnet with its grille and a dead lamp, a cab with a rounded roof over two windows (one
/// cracked, one gone), a sloped boot, mudguards arched over tyres gone flat, a running board;
/// the paint faded and rusted through in blooms, and grass up round the wheels.
fn wreck_car(c: &mut Canvas, k: &Kit, x: i32, bw: i32, foot: i32) {
    let body = k.body;
    let r = |dx: i32| x + dx * bw / 76;
    // The body, the bonnet and the boot in one silhouette, then the cab on it.
    let hull = [
        (r(1), foot - 8),
        (r(2), foot - 17),
        (r(8), foot - 21),
        (r(16), foot - 21),
        (r(18), foot - 17),
        (r(50), foot - 18),
        (r(68), foot - 17),
        (r(74), foot - 13),
        (r(75), foot - 8),
    ];
    c.polygon_lit(&hull, body, 90, Z::new(3, 6));
    let cab = [
        (r(16), foot - 17),
        (r(19), foot - 31),
        (r(24), foot - 34),
        (r(44), foot - 34),
        (r(48), foot - 31),
        (r(51), foot - 17),
    ];
    c.polygon_lit(&cab, body, 110, Z::new(6, 9));
    c.retone(body, super::HARD);
    // The roof's top catching the sky, the bonnet's crown, the waist line.
    c.hline(r(24), r(44), foot - 34, body.at(Tone::Light), 9);
    c.hline(r(19), r(67), foot - 17, body.at(Tone::Light), 6);
    c.hline(r(3), r(74), foot - 12, body.at(Tone::Shade), 6);
    // The windows: the near one cracked, the far one gone.
    let w1 = Rect::new(r(21), foot - 30, r(33) - r(21), 10);
    let w2 = Rect::new(r(35), foot - 30, r(46) - r(35), 10);
    parts::glass(c, w1, false, 9);
    c.line((w1.x + 2, w1.y + 1), (w1.x + 6, w1.bottom() - 2), Ramp::HairWhite.at(Tone::Light), 1, 10);
    c.line((w1.x + 6, w1.y + 3), (w1.right() - 2, w1.y + 2), Ramp::HairWhite.at(Tone::Light), 1, 10);
    c.fill_normal(w2, Ramp::ClothBlack.at(Tone::Deep), crate::canvas::FLAT, 9);
    c.hline(w2.x, w2.right() - 1, w2.bottom() - 2, Ramp::Leather.at(Tone::Shade), 9);
    // The door: its seam, a brass handle.
    c.vline(r(34), foot - 30, foot - 12, body.at(Tone::Deep), 9);
    c.dot(r(31), foot - 19, Ramp::Brass.at(Tone::Light), 9);
    // The grille and the lamp.
    for gx in [r(74), r(75) - 1] {
        c.vline(gx, foot - 15, foot - 9, Ramp::Iron.at(Tone::Light), 7);
    }
    c.disc_lit(r(70), foot - 19, 2, Ramp::Glass, Z::flat(8));
    // Mudguards over the wheels, the wheels flat to the ground, the running board between.
    for (wx, ww) in [(r(12), 12), (r(60), 12)] {
        c.ellipse(Rect::new(wx - 1, foot - 13, ww + 2, 8), body.at(Tone::Mid), 7);
        c.hline(wx, wx + ww - 1, foot - 13, body.at(Tone::Light), 7);
        c.ellipse(Rect::new(wx, foot - 9, ww, 9), Ramp::ClothBlack.at(Tone::Base), 8);
        c.hline(wx + 1, wx + ww - 2, foot - 1, Ramp::ClothBlack.at(Tone::Shade), 8);
        c.disc_lit(wx + ww / 2, foot - 5, 2, Ramp::Iron, Z::flat(9));
    }
    c.fill_rect(Rect::new(r(24), foot - 7, r(58) - r(24), 2), Ramp::Iron.at(Tone::Shade), 6);
    c.hline(r(24), r(58), foot - 7, Ramp::Iron.at(Tone::Light), 6);
    // Rust blooms through the paint, and grass up round the wheels.
    for i in 0..9 {
        let hh = parts::hash(k.seed, i, 70);
        let (rx, ry) = (r(4) + (hh % (bw as u32 - 12)) as i32, foot - 20 + ((hh >> 8) % 10) as i32);
        c.tint(rx, ry, body, Tone::Deep);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
            if matches!(Ramp::of(c.get(rx + dx, ry + dy)), Some((q, _)) if q == body) {
                c.put(
                    rx + dx,
                    ry + dy,
                    Ramp::Copper.at(if dx == 0 { Tone::Shade } else { Tone::Base }),
                    crate::canvas::FLAT,
                    7,
                );
            }
        }
    }
    for gx in [r(10), r(14), r(24), r(57), r(62), r(72)] {
        let hh = parts::hash(k.seed, gx, 71);
        let tall = 2 + (hh % 3) as i32;
        c.vline(gx, foot - tall, foot - 1, Ramp::Grass.at(Tone::Base), 10);
        c.dot(gx, foot - tall, Ramp::Grass.at(Tone::Light), 10);
    }
}
