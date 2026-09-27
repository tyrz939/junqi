//! `machine` and `small_thing` (ART.md §2.3): the Works' and the School's boards, boxes and
//! sockets, and the small things the tales leave about. `on` lights a machine's lamp or its
//! socket (emissive).
//!
//! Machine shapes: `socket`, `fuse_box`, `relay_box`, `call_box`, `breaker`, `generator`,
//! `valve`, `air_pump`, `clock`, `locker`. Small shapes: `bedroll`, `glove`, `veil`, `slippers`,
//! `boots`, `key`, `diving_helmet`, `hose`, `scarf`, `tangle`, `washing`, `candles`, `dinner`,
//! `tortoise`.

use jane_core::grid::Rect;

use super::parts::{ao, band, box3, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let on = state == State::On;
    let (w, foot) = (k.w, k.foot());
    let cx = w / 2;
    Some(match k.look.shape {
        "socket" => {
            // A socket on a post: a plate with its two pins' holes, a lamp over it that lights.
            ao(c, cx - 3, cx + 2, foot, 4);
            post(c, cx - 1, foot - 14, foot - 1, 3, k.trim, 3);
            c.rect_bevel(Rect::new(cx - 5, foot - 22, 10, 9), k.body, 1, Z::new(4, 6));
            for x in [cx - 3, cx + 1] {
                c.fill_rect(Rect::new(x, foot - 18, 2, 2), Ix::SEAM, 7);
            }
            pilot(c, k, cx - 1, foot - 21, on);
            Stand::Up(&[])
        }
        "fuse_box" | "relay_box" | "breaker" | "call_box" => {
            // A box on a post: an iron case with a lit bevel, its door and what is on it.
            ao(c, cx - 3, cx + 2, foot, 4);
            post(c, cx - 1, foot - 10, foot - 1, 3, k.trim, 3);
            let r = Rect::new(cx - 6, foot - 26, 12, 16);
            c.rect_bevel(r, k.body, 1, Z::new(4, 6));
            c.hline(r.x + 2, r.right() - 3, r.y + 2, k.body.at(Tone::Light), 7);
            match k.look.shape {
                "fuse_box" => {
                    for (i, y) in [r.y + 4, r.y + 8].into_iter().enumerate() {
                        for x in [r.x + 3, r.x + 6] {
                            c.fill_rect(Rect::new(x, y, 2, 3), Ramp::ClothLinen.at(Tone::Base), 7);
                            c.dot(x, y + if i == 0 { 0 } else { 2 }, Ramp::ClothLinen.at(Tone::Light), 7);
                        }
                    }
                    pilot(c, k, r.right() - 4, r.y + 3, on);
                }
                "relay_box" => {
                    c.fill_rect(Rect::new(r.x + 3, r.y + 5, 6, 6), k.body.at(Tone::Shade), 7);
                    c.disc_lit(r.x + 6, r.y + 8, 2, k.accent, Z::flat(8));
                    pilot(c, k, r.x + 5, r.y + 12, on);
                }
                "breaker" => {
                    c.fill_rect(Rect::new(r.x + 4, r.y + 4, 4, 9), k.body.at(Tone::Deep), 7);
                    let y = if on { r.y + 5 } else { r.y + 10 };
                    c.fill_rect(Rect::new(r.x + 3, y, 6, 2), k.accent.at(Tone::Base), 8);
                    c.dot(r.x + 3, y, k.accent.at(Tone::Light), 8);
                    pilot(c, k, r.right() - 3, r.y + 3, on);
                }
                _ => {
                    // A call box: the handset on its hook, a dial, a bell.
                    c.fill_rect(Rect::new(r.x + 2, r.y + 4, 3, 8), k.accent.at(Tone::Base), 7);
                    c.vline(r.x + 2, r.y + 4, r.y + 11, k.accent.at(Tone::Light), 7);
                    c.disc_lit(r.x + 8, r.y + 6, 2, Ramp::ClothLinen, Z::flat(7));
                    c.disc_lit(r.x + 8, r.y + 11, 1, Ramp::Brass, Z::flat(7));
                }
            }
            Stand::Up(&[])
        }
        "generator" => {
            // The Works' generator: an iron housing with its flywheel, a gauge, cables; cold,
            // its gauge dark; running, its lamp lit and its gauge glowing.
            let (x, gw) = (3, w - 6);
            ao(c, x, x + gw - 1, foot, 8);
            let (top, front) = box3(c, x, gw, foot, 20, 12, k.body, None, 3);
            c.retone(k.body, super::HARD);
            for y in (front.y + 3..front.bottom() - 2).step_by(4) {
                c.hline(front.x + 2, front.right() - 3, y, k.body.at(Tone::Shade), 4);
            }
            band(c, front.x, front.right() - 1, front.y, k.trim, 5);
            // The flywheel on its east end.
            c.disc_lit(front.right() - 7, front.y + 9, 6, k.trim, Z::new(5, 7));
            c.disc_lit(front.right() - 7, front.y + 9, 2, k.body, Z::flat(8));
            // The gauge and the lamp on the top.
            c.disc_lit(top.x + 8, top.y + top.h / 2, 3, Ramp::ClothLinen, Z::flat(9));
            c.line((top.x + 8, top.y + top.h / 2), (top.x + 9 + i32::from(on) * 2, top.y + top.h / 2 - 2), Ix::SEAM, 1, 10);
            pilot(c, k, top.x + 16, top.y + 3, on);
            c.line((front.x + 2, foot - 1), (front.x - 2, foot - 1), Ramp::ClothBlack.at(Tone::Base), 2, 2);
            Stand::Tops([(top, super::parts::lid_height(20)), (Rect::default(), 0)])
        }
        "valve" => {
            // A valve on a pipe from the ground: a hand wheel with its spokes, red.
            ao(c, cx - 5, cx + 4, foot, 4);
            c.polygon_lit(&[(cx - 2, foot - 12), (cx + 1, foot - 12), (cx + 1, foot - 1), (cx - 2, foot - 1)], k.trim, 90, Z::flat(3));
            c.hline(cx - 6, cx + 5, foot - 4, k.trim.at(Tone::Base), 3);
            let r = Rect::new(cx - 6, foot - 22, 12, 11);
            c.ellipse(r, k.accent.at(Tone::Base), 5);
            c.ellipse(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), Ix::CLEAR, 0);
            c.line((r.x + 2, r.y + 5), (r.right() - 3, r.y + 5), k.accent.at(Tone::Shade), 1, 5);
            c.line((cx - 1, r.y + 1), (cx - 1, r.bottom() - 2), k.accent.at(Tone::Light), 1, 5);
            c.disc_lit(cx - 1, r.y + 5, 1, k.trim, Z::flat(6));
            Stand::Up(&[])
        }
        "air_pump" => {
            // A diver's air pump: a wooden box with a great wheel on its side and a handle.
            let (x, pw) = (3, w - 12);
            ao(c, x, x + pw + 6, foot, 5);
            box3(c, x, pw, foot, 11, 6, k.body, Some((2, k.seed)), 3);
            c.ellipse(Rect::new(x + pw - 2, foot - 20, 13, 13), k.trim.at(Tone::Base), 5);
            c.ellipse(Rect::new(x + pw, foot - 18, 9, 9), Ix::CLEAR, 0);
            c.line((x + pw + 4, foot - 14), (x + pw + 8, foot - 22), k.trim.at(Tone::Light), 2, 6);
            c.disc_lit(x + pw + 4, foot - 14, 1, Ramp::Brass, Z::flat(7));
            c.fill_rect(Rect::new(x + 3, foot - 9, 5, 4), Ramp::Brass.at(Tone::Base), 5);
            Stand::Up(&[])
        }
        "clock" => {
            // The School's clock: a round face in a dark wood case, stopped at nine.
            let r = Rect::new(cx - 7, foot - 22, 14, 14);
            post(c, cx - 1, foot - 9, foot - 1, 3, k.trim, 3);
            c.ellipse_lit(Rect::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2), k.trim, Z::flat(5));
            c.ellipse(r, Ramp::ClothLinen.at(Tone::Light), 6);
            for (dx, dy) in [(0, -5), (5, 0), (0, 5), (-5, 0)] {
                c.dot(cx - 1 + dx, r.y + 7 + dy, k.trim.at(Tone::Shade), 6);
            }
            c.line((cx - 1, r.y + 7), (cx - 1, r.y + 3), Ix::SEAM, 1, 7);
            c.line((cx - 1, r.y + 7), (cx - 5, r.y + 7), Ix::SEAM, 1, 7);
            ao(c, cx - 3, cx + 2, foot, 3);
            Stand::Up(&[])
        }
        "locker" => {
            // A lamp locker: an iron cabinet with louvres and a padlock.
            let (x, lw) = (4, w - 8);
            ao(c, x, x + lw - 1, foot, 5);
            let (_, front) = box3(c, x, lw, foot, 22, 5, k.body, None, 3);
            c.retone(k.body, super::HARD);
            c.vline(front.x + lw / 2, front.y + 1, front.bottom() - 2, k.body.at(Tone::Deep), 4);
            for y in (front.y + 3..front.y + 9).step_by(2) {
                for x0 in [front.x + 2, front.x + lw / 2 + 2] {
                    c.hline(x0, x0 + lw / 2 - 5, y, k.body.at(Tone::Shade), 4);
                }
            }
            c.fill_rect(Rect::new(front.x + lw / 2 - 1, front.y + 12, 3, 3), Ramp::Brass.at(Tone::Base), 5);
            c.dot(front.x + lw / 2 - 1, front.y + 12, Ramp::Brass.at(Tone::High), 5);
            Stand::Up(&[])
        }
        _ => return small(c, k, on),
    })
}

/// A machine's pilot lamp: dark glass off, a warm point lit and emitting on.
fn pilot(c: &mut Canvas, k: &Kit, x: i32, y: i32, on: bool) {
    if on {
        c.set_emitting(true);
        c.fill_rect(Rect::new(x, y, 2, 2), k.accent.at(Tone::High), 9);
        c.dot(x, y, k.accent.at(Tone::Glint), 9);
        c.set_emitting(false);
    } else {
        c.fill_rect(Rect::new(x, y, 2, 2), k.accent.at(Tone::Deep), 9);
        c.dot(x, y, k.accent.at(Tone::Mid), 9);
    }
}

/// The small things at prop scale (the tales' and the county's): lying on the ground a few
/// px thick, or standing a little.
fn small(c: &mut Canvas, k: &Kit, on: bool) -> Option<Stand> {
    let (w, foot) = (k.w, k.foot());
    let cx = w / 2;
    let b = k.body;
    let soft = |c: &mut Canvas, pts: &[(i32, i32)], ramp: Ramp, rad: i32, z: Z| {
        let mut m = Canvas::new(c.w(), c.h());
        m.polyline_fill(pts, Ix::INK, 1);
        c.inflate(&m, ramp, rad, z);
    };
    Some(match k.look.shape {
        "bedroll" => {
            // A bedroll: a blanket rolled, tied twice, lying on the ground.
            c.ao_contact(Rect::new(3, foot - 8, w - 6, 8), 1);
            soft(c, &[(3, foot - 9), (w - 5, foot - 9), (w - 3, foot - 5), (w - 5, foot - 1), (3, foot - 1), (2, foot - 5)], b, 3, Z::new(1, 4));
            for x in [w / 3, 2 * w / 3] {
                c.vline(x, foot - 9, foot - 1, k.trim.at(Tone::Base), 5);
            }
            c.ellipse(Rect::new(w - 7, foot - 8, 4, 7), b.at(Tone::Mid), 5);
            c.dot(w - 6, foot - 5, b.at(Tone::Deep), 5);
            Stand::Flat(4)
        }
        "glove" => {
            // A glove on a folded cloth: the Shot-Firer's, singed.
            box3(c, cx - 11, 22, foot, 4, 10, Ramp::ClothLinen, None, 2);
            soft(c, &[(cx - 5, foot - 6), (cx - 5, foot - 12), (cx - 3, foot - 15), (cx - 1, foot - 12), (cx + 1, foot - 16), (cx + 3, foot - 12), (cx + 6, foot - 13), (cx + 4, foot - 7), (cx + 3, foot - 5)], b, 2, Z::new(4, 6));
            c.fill_rect(Rect::new(cx - 5, foot - 7, 9, 2), k.trim.at(Tone::Base), 6);
            c.dot(cx + 2, foot - 11, Ramp::ClothBlack.at(Tone::Base), 6);
            Stand::Flat(6)
        }
        "veil" => {
            // A beekeeper's hat and veil, laid down: a brim and the net's pale folds.
            c.ao_contact(Rect::new(cx - 7, foot - 5, 14, 5), 1);
            c.ellipse_lit(Rect::new(cx - 7, foot - 8, 14, 6), b, Z::new(2, 3));
            c.ellipse_lit(Rect::new(cx - 4, foot - 12, 8, 6), b, Z::new(3, 5));
            for x in [cx - 5, cx - 2, cx + 1, cx + 4] {
                c.vline(x, foot - 7, foot - 3, Ramp::HairWhite.at(Tone::Light), 4);
            }
            Stand::Flat(5)
        }
        "slippers" | "boots" => {
            // A pair: side by side, toes to the viewer, the near one lit.
            let boots = k.look.shape == "boots";
            c.ao_contact(Rect::new(cx - 7, foot - 5, 14, 5), 1);
            for (i, x) in [(0, cx - 6), (1, cx + 1)] {
                let top = if boots { foot - 12 } else { foot - 5 };
                soft(c, &[(x, top), (x + 4, top), (x + 5, foot - 3), (x + 5, foot - 1), (x, foot - 1)], b, 2, Z::new(2, if boots { 8 } else { 3 }));
                if boots {
                    c.hline(x, x + 4, top, b.at(Tone::Light), 9);
                    c.hline(x, x + 5, foot - 2, k.trim.at(Tone::Shade), 3);
                } else {
                    c.fill_rect(Rect::new(x + 1, foot - 4, 3, 1), k.trim.at(Tone::Light), 4);
                }
                let _ = i;
            }
            Stand::Flat(if boots { 10 } else { 3 })
        }
        "key" => {
            // A key on the ground, its bow lit, its shadow under it.
            c.ao_contact(Rect::new(cx - 6, foot - 5, 12, 4), 1);
            c.ellipse(Rect::new(cx - 6, foot - 9, 5, 5), b.at(Tone::Base), 2);
            c.dot(cx - 4, foot - 7, Ix::CLEAR, 0);
            c.line((cx - 2, foot - 6), (cx + 5, foot - 4), b.at(Tone::Light), 2, 2);
            c.fill_rect(Rect::new(cx + 3, foot - 4, 2, 3), b.at(Tone::Base), 2);
            Stand::Flat(2)
        }
        "diving_helmet" => {
            // A diving helmet: a brass dome, a round glass port, a collar of bolts.
            ao(c, cx - 7, cx + 6, foot, 4);
            c.ellipse_lit(Rect::new(cx - 7, foot - 16, 14, 14), b, Z::new(2, 10));
            c.retone(b, super::HARD);
            c.ellipse(Rect::new(cx - 4, foot - 12, 7, 6), Ramp::Glass.at(Tone::Shade), 11);
            c.dot(cx - 3, foot - 11, Ramp::Glass.at(Tone::Light), 11);
            c.fill_rect(Rect::new(cx - 7, foot - 4, 14, 3), b.at(Tone::Mid), 3);
            for x in [cx - 6, cx - 2, cx + 2, cx + 5] {
                c.dot(x, foot - 3, b.at(Tone::High), 4);
            }
            Stand::Up(&[])
        }
        "hose" => {
            // A hose coiled on the ground, its end trailing off.
            c.ao_contact(Rect::new(2, foot - 10, w - 4, 10), 1);
            for r in [9, 6, 3] {
                let e = Rect::new(8 - r / 2, foot - 8 - r / 2 + 3, r * 2, r + 1);
                c.ellipse(e, b.at(Tone::Base), 2);
                c.ellipse(Rect::new(e.x + 2, e.y + 2, e.w - 4, e.h - 4), Ix::CLEAR, 0);
            }
            c.line((16, foot - 6), (w - 3, foot - 3), b.at(Tone::Light), 2, 2);
            Stand::Flat(2)
        }
        "scarf" | "tangle" | "washing" => {
            // Wool on the ground: a scarf's length with its fringe, a pulled tangle, a heap of
            // washing.
            c.ao_contact(Rect::new(1, foot - 8, w - 2, 8), 1);
            match k.look.shape {
                "scarf" => {
                    soft(c, &[(1, foot - 7), (w - 3, foot - 9), (w - 2, foot - 5), (2, foot - 3)], b, 2, Z::new(1, 2));
                    for x in (4..w - 3).step_by(6) {
                        c.vline(x, foot - 8, foot - 4, k.trim.at(Tone::Base), 3);
                    }
                }
                "tangle" => {
                    for i in 0..5 {
                        let a = jane_core::angle::Angle((i * 13107) as u16);
                        let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
                        c.line((cx + ((co * 5) >> 15), foot - 6 + ((s * 3) >> 15)), (cx - ((co * 5) >> 15), foot - 6 - ((s * 3) >> 15)), b.at(if i % 2 == 0 { Tone::Light } else { Tone::Base }), 2, 2);
                    }
                }
                _ => {
                    for (x, y, r) in [(cx - 6, foot - 8, Ramp::ClothLinen), (cx - 1, foot - 10, Ramp::ClothSky), (cx + 1, foot - 7, Ramp::ClothRose)] {
                        soft(c, &[(x, y), (x + 7, y + 1), (x + 6, y + 6), (x - 1, y + 5)], r, 2, Z::new(1, 4));
                    }
                }
            }
            Stand::Flat(4)
        }
        "candles" => {
            // Candle ends on the ground: three stubs in a pool of wax.
            c.ellipse(Rect::new(cx - 6, foot - 6, 12, 5), b.at(Tone::Light), 1);
            for (x, h) in [(cx - 4, 4), (cx - 1, 6), (cx + 2, 3)] {
                c.fill_rect(Rect::new(x, foot - 3 - h, 2, h), b.at(Tone::Base), 3);
                c.dot(x, foot - 3 - h, b.at(Tone::High), 3);
                c.dot(x, foot - 4 - h, Ix::SEAM, 3);
            }
            Stand::Flat(6)
        }
        "dinner" => {
            // A plate with a dinner on it: a slice, a heap of potatoes, greens.
            c.ao_contact(Rect::new(cx - 7, foot - 6, 14, 6), 1);
            c.ellipse(Rect::new(cx - 7, foot - 10, 14, 8), Ramp::ClothLinen.at(Tone::Light), 2);
            c.ellipse(Rect::new(cx - 5, foot - 9, 10, 6), Ramp::ClothLinen.at(Tone::Base), 2);
            c.fill_rect(Rect::new(cx - 4, foot - 8, 4, 3), Ramp::ClothBrick.at(Tone::Base), 3);
            c.disc_lit(cx + 2, foot - 7, 1, Ramp::WoodPale, Z::flat(3));
            c.fill_rect(Rect::new(cx - 1, foot - 5, 3, 1), Ramp::Leaf.at(Tone::Base), 3);
            Stand::Flat(3)
        }
        "tortoise" => {
            // A tortoise: a domed shell in plates, its head out to the east.
            ao(c, cx - 6, cx + 6, foot, 4);
            c.ellipse_lit(Rect::new(cx + 3, foot - 6, 4, 4), Ramp::Leaf, Z::flat(2));
            c.ellipse_lit(Rect::new(cx - 6, foot - 9, 11, 8), b, Z::new(2, 6));
            c.retone(b, super::HARD);
            for (x, y) in [(cx - 3, foot - 7), (cx, foot - 7), (cx - 2, foot - 4)] {
                c.hline(x, x + 2, y, b.at(Tone::Shade), 7);
            }
            Stand::Up(&[])
        }
        "scroll" => {
            // A burnt scroll on a lectern's foot: the paper curled, its edge charred, embers
            // along the burn when it is lit.
            ao(c, cx - 8, cx + 7, foot, 4);
            box3(c, cx - 8, 16, foot, 5, 6, Ramp::WoodDark, Some((2, k.seed)), 2);
            let r = Rect::new(cx - 7, foot - 13, 14, 6);
            c.fill_normal(r, b.at(Tone::Light), FLAT, 5);
            c.ellipse_lit(Rect::new(r.x - 1, r.y - 1, 3, r.h + 2), b, Z::flat(6));
            c.ellipse_lit(Rect::new(r.right() - 2, r.y - 1, 3, r.h + 2), b, Z::flat(6));
            c.hline(r.x + 2, r.right() - 3, r.y + 2, b.at(Tone::Mid), 5);
            c.hline(r.x + 2, r.right() - 5, r.y + 4, b.at(Tone::Mid), 5);
            c.hline(r.right() - 5, r.right() - 2, r.bottom() - 1, Ramp::ClothBlack.at(Tone::Base), 6);
            if on {
                c.set_emitting(true);
                for x in [r.right() - 5, r.right() - 3] {
                    c.dot(x, r.bottom() - 1, Ramp::Ember.at(Tone::High), 7);
                }
                c.set_emitting(false);
            }
            Stand::Up(&[])
        }
        _ => return None,
    })
}
