//! `furniture` (ART.md §2.3): what a room is furnished with. Shapes: `table`, `workbench`,
//! `desk`, `counter`, `altar`, `shelf`, `cabinet`, `case`, `bed`, `stove`, `bench`, `pew`,
//! `chair`, `pegs`, `register`, `plinth`; the stores she keeps things in: `cupboard`, `locker`
//! (a left-luggage locker, the cupboard in railway paint) and `dresser`; and the dungeons' own
//! (§8 step 7): `bookcase` (the library), `school_desk` (the School), `tomb` (the Burial). A
//! stove's `on` is its firebox alight.

use jane_core::grid::Rect;

use super::parts::{self, ao, box3, glass, lid_height, planks, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let on = state == State::On;
    let (w, h, foot) = (k.w, k.h, k.foot());
    Some(match k.look.shape {
        "table" | "desk" | "workbench" => {
            // A table top of planks over four legs (the far two in its shade), what lies on it.
            let (x, tw) = (2, w - 4);
            let leg = 9;
            let depth = (k.fh * 16 - 14).clamp(8, 18);
            ao(c, x, x + tw - 1, foot, 6);
            for lx in [x + 1, x + tw - 4] {
                post(c, lx, foot - leg - depth + 2, foot - leg, 3, k.trim, 2);
                c.shade(Rect::new(lx - 1, foot - leg - depth, 5, depth + 2), k.trim, 1);
            }
            if k.look.shape == "desk" {
                // A desk is closed down to the floor at the front: drawers with brass pulls.
                let (_, front) = box3(c, x, tw, foot, leg + 1, 0, k.body, Some((2, k.seed)), 3);
                for dx in [x + 4, x + tw - 10] {
                    c.hline(dx, dx + 5, front.y + 4, k.body.at(Tone::Shade), 4);
                    c.dot(dx + 2, front.y + 2, k.accent.at(Tone::Light), 4);
                }
            } else {
                for lx in [x + 1, x + tw - 4] {
                    post(c, lx, foot - leg, foot - 1, 3, k.trim, 3);
                }
            }
            let (top, _) = box3(c, x, tw, foot - leg + 2, 3, depth, k.body, Some((3, k.seed)), 4);
            match k.look.shape {
                "workbench" => {
                    // Tools and a vice: a hammer, a saw's blade, a jar of nails.
                    c.fill_rect(Rect::new(top.x + 3, top.y + 3, 6, 2), Ramp::Iron.at(Tone::Light), 6);
                    c.line((top.x + 5, top.y + 5), (top.x + 8, top.y + 8), k.trim.at(Tone::Base), 1, 6);
                    c.fill_normal(
                        Rect::new(top.right() - 8, top.y + 2, 5, 4),
                        Ramp::Glass.at(Tone::Base),
                        parts::south(),
                        6,
                    );
                    c.hline(top.right() - 8, top.right() - 4, top.y + 2, Ramp::Glass.at(Tone::Light), 6);
                    c.rect_bevel(Rect::new(top.x + tw / 2 - 3, top.bottom() - 4, 6, 4), Ramp::Iron, 1, Z::new(6, 7));
                }
                "desk" => {
                    // A ledger open on it and a lamp's brass.
                    let b = Rect::new(top.x + tw / 2 - 5, top.y + 2, 10, 6);
                    c.fill_normal(b, Ramp::ClothLinen.at(Tone::Light), FLAT, 6);
                    c.vline(b.x + 5, b.y, b.bottom() - 1, Ramp::ClothLinen.at(Tone::Mid), 6);
                    parts::writing(c, Rect::new(b.x + 1, b.y + 1, 3, 4), 2, Ramp::Slate.at(Tone::Mid), k.seed, 6);
                }
                _ => {
                    // A cloth runner down the middle.
                    if k.accent != Ramp::Brass {
                        c.fill_normal(Rect::new(top.x + tw / 2 - 3, top.y, 6, top.h), k.accent.at(Tone::Base), FLAT, 6);
                        c.vline(top.x + tw / 2 - 3, top.y, top.bottom() - 1, k.accent.at(Tone::Light), 6);
                    }
                }
            }
            Stand::Tops([(top, lid_height(leg)), (Rect::default(), 0)])
        }
        "counter" | "altar" => {
            // A counter or an altar: a panelled front to the floor, a top that overhangs it, a
            // cloth over an altar.
            let face = if k.look.shape == "altar" { 14 } else { 16 };
            let depth = (k.fh * 16 - face).clamp(6, 16);
            ao(c, 1, w - 2, foot, 6);
            let (top, front) = box3(c, 1, w - 2, foot, face, depth, k.body, Some(((w / 12).max(2), k.seed)), 3);
            for px in (front.x + 3..front.right() - 4).step_by(10) {
                c.fill_normal(Rect::new(px, front.y + 3, 7, front.h - 6), k.body.at(Tone::Shade), parts::south(), 4);
                c.hline(px, px + 6, front.bottom() - 4, k.body.at(Tone::Light), 4);
            }
            if k.look.shape == "altar" {
                c.fill_normal(Rect::new(top.x + 4, top.y, top.w - 8, top.h + 5), k.accent.at(Tone::Light), FLAT, 5);
                c.vline(top.x + 4, top.y, top.bottom() + 4, k.accent.at(Tone::High), 5);
                c.fill_rect(Rect::new(top.x + top.w / 2 - 1, top.bottom() - 1, 2, 5), Ramp::Brass.at(Tone::Light), 6);
                for cx in [top.x + 6, top.right() - 7] {
                    c.fill_rect(Rect::new(cx, top.y + 1, 2, 4), Ramp::ClothLinen.at(Tone::Light), 6);
                }
            }
            Stand::Tops([(top, lid_height(face)), (Rect::default(), 0)])
        }
        "shelf" | "cabinet" => {
            // Shelves against the wall: a carcass, two or three boards, jars and books on them;
            // a broken cabinet has a door off and a board down.
            let broken = k.look.shape == "cabinet";
            let sh = (h - 3).min(30);
            let r = Rect::new(1, foot - sh + 1, w - 2, sh);
            ao(c, r.x, r.right() - 1, foot, 4);
            c.fill_normal(r, k.body.at(Tone::Deep), parts::south(), 3);
            c.rect_bevel(Rect::new(r.x, r.y, r.w, 3), k.body, 1, Z::new(4, 5));
            c.fill_normal(Rect::new(r.x, r.y, 2, r.h), k.body.at(Tone::Base), parts::south(), 4);
            c.fill_normal(Rect::new(r.right() - 2, r.y, 2, r.h), k.body.at(Tone::Mid), parts::south(), 4);
            let boards = (sh / 9).max(2);
            for i in 1..=boards {
                let y = r.y + i * sh / (boards + 1) + 2;
                if broken && i == boards {
                    c.line((r.x + 2, y - 3), (r.right() - 3, y), k.body.at(Tone::Base), 2, 4);
                    continue;
                }
                c.fill_normal(Rect::new(r.x + 2, y, r.w - 4, 2), k.body.at(Tone::Light), parts::south(), 4);
                wares(c, k, r.x + 3, y - 1, r.w - 6, i);
            }
            Stand::Up(&[])
        }
        "bookcase" => {
            // A bookcase to the ceiling: a carcass under a cornice, every board full of spines
            // of their own colours and heights, one leaning, a gap here and there.
            let sh = (h - 3).min(34);
            let r = Rect::new(1, foot - sh + 1, w - 2, sh);
            ao(c, r.x, r.right() - 1, foot, 4);
            c.fill_normal(r, k.body.at(Tone::Deep), parts::south(), 3);
            c.fill_normal(Rect::new(r.x, r.y, 2, r.h), k.body.at(Tone::Base), parts::south(), 4);
            c.fill_normal(Rect::new(r.right() - 2, r.y, 2, r.h), k.body.at(Tone::Mid), parts::south(), 4);
            let spines = [
                Ramp::ClothRed,
                Ramp::ClothGreen,
                Ramp::ClothNavy,
                Ramp::ClothMustard,
                Ramp::Leather,
                Ramp::ClothBrick,
                Ramp::ClothBlack,
                Ramp::ClothTweed,
            ];
            let boards = (sh / 9).max(2);
            let mut prev = r.y + 3;
            for i in 1..=boards + 1 {
                let y = if i > boards { r.bottom() - 2 } else { r.y + i * sh / (boards + 1) + 1 };
                let mut x = r.x + 3;
                let mut n = 0u32;
                while x < r.right() - 3 {
                    let hsh = parts::hash(k.seed, x, i);
                    let bw = 1 + (hsh % 3) as i32;
                    if hsh % 13 == 0 {
                        x += 3;
                        continue;
                    }
                    let tall = (y - prev - 1) - (hsh >> 4) as i32 % 3;
                    let sp = spines[(hsh >> 8) as usize % spines.len()];
                    let bw = bw.min(r.right() - 3 - x);
                    if n == 4 && hsh % 3 == 0 && bw > 0 {
                        c.line((x, y - 1), (x + 3, y - tall + 1), sp.at(Tone::Base), 2, 5);
                        x += 4;
                    } else {
                        c.fill_normal(Rect::new(x, y - tall, bw, tall), sp.at(Tone::Base), parts::south(), 5);
                        c.vline(x, y - tall, y - 1, sp.at(Tone::Light), 5);
                        c.dot(x + bw - 1, y - tall + 2, Ramp::Brass.at(Tone::Light), 5);
                        x += bw;
                    }
                    n += 1;
                }
                if i <= boards {
                    c.fill_normal(Rect::new(r.x + 2, y, r.w - 4, 2), k.body.at(Tone::Light), parts::south(), 4);
                }
                prev = y + 2;
            }
            c.rect_bevel(Rect::new(r.x - 1, r.y - 1, r.w + 2, 4), k.body, 1, Z::new(5, 6));
            Stand::Up(&[])
        }
        "school_desk" => {
            // A double school desk: a sloped lid with two inkwells and a pen groove, on an
            // iron frame, its bench in front; initials cut in the lid.
            let (x, tw) = (3, w - 6);
            ao(c, x, x + tw - 1, foot, 6);
            for lx in [x + 2, x + tw - 5] {
                post(c, lx, foot - 16, foot - 1, 2, k.trim, 2);
            }
            let (seat, _) = box3(c, x + 2, tw - 4, foot, 4, 4, k.body, Some((1, k.seed)), 3);
            let (top, front) =
                box3(c, x, tw, foot - 9, 7, (k.fh * 16 - 20).clamp(8, 14), k.body, Some((2, k.seed ^ 3)), 4);
            c.hline(front.x, front.right() - 1, front.y, k.body.at(Tone::Deep), 5);
            c.hline(top.x + 2, top.right() - 3, top.y + 2, k.body.at(Tone::Shade), 6);
            for ix in [top.x + 5, top.right() - 7] {
                c.fill_rect(Rect::new(ix, top.y + 1, 3, 2), k.accent.at(Tone::Light), 6);
                c.dot(ix + 1, top.y + 1, Ix::SEAM, 7);
            }
            parts::writing(c, Rect::new(top.x + tw / 2 - 4, top.y + 5, 6, 2), 1, k.body.at(Tone::Shade), k.seed, 6);
            Stand::Tops([(top, lid_height(16)), (seat, lid_height(4))])
        }
        "tomb" => {
            // A chest tomb: a moulded lid over a panelled chest, the lid carved with a figure
            // lying with its hands together, a quatrefoil on the chest's face.
            let (x, tw) = (2, w - 4);
            let depth = (k.fh * 16 - 14).clamp(10, 34);
            ao(c, x, x + tw - 1, foot, 6);
            let (top, front) = box3(c, x, tw, foot, 10, depth, k.body, None, 3);
            c.retone(k.body, super::HARD);
            c.rect_bevel(Rect::new(top.x - 1, top.y - 1, top.w + 2, top.h + 2), k.body, 1, Z::new(5, 6));
            let cx = top.x + top.w / 2;
            let fig = |c: &mut Canvas, dy: i32| {
                c.ellipse_lit(Rect::new(cx - 3, top.y + 2 + dy, 6, 5), k.body, Z::flat(7));
                c.polygon_lit(
                    &[
                        (cx - 4, top.y + 7 + dy),
                        (cx + 3, top.y + 7 + dy),
                        (cx + 2, top.bottom() - 3),
                        (cx - 3, top.bottom() - 3),
                    ],
                    k.body,
                    90,
                    Z::flat(7),
                );
            };
            fig(c, 0);
            c.retone(k.body, super::HARD);
            c.vline(cx, top.y + 10, top.y + 13, k.body.at(Tone::Shade), 8);
            c.dot(cx - 1, top.y + 9, k.body.at(Tone::Light), 8);
            let q = (cx - 3, front.y + 2);
            for (dx, dy) in [(2, 0), (0, 2), (4, 2), (2, 4)] {
                c.fill_rect(Rect::new(q.0 + dx, q.1 + dy, 2, 2), k.body.at(Tone::Shade), 4);
            }
            c.dot(q.0 + 2, q.1 + 2, k.accent.at(Tone::Light), 4);
            Stand::Tops([(top, lid_height(10)), (Rect::default(), 0)])
        }
        "case" => {
            // A glass case on a wooden stand: its panes reflecting, what is inside dim.
            let r = Rect::new(3, foot - 26, w - 6, 16);
            ao(c, 2, w - 3, foot, 5);
            box3(c, 2, w - 4, foot, 10, 4, k.body, Some((2, k.seed)), 3);
            glass(c, r, false, 6);
            c.fill_normal(
                Rect::new(r.x + r.w / 2 - 3, r.bottom() - 6, 6, 5),
                k.accent.at(Tone::Base),
                parts::south(),
                5,
            );
            c.rect_bevel(Rect::new(r.x - 1, r.y - 2, r.w + 2, 3), k.body, 1, Z::new(7, 8));
            Stand::Up(&[])
        }
        "bed" => {
            // A bed from above: a headboard, a pillow, a folded blanket over the foot; a sickbay
            // bed is iron and white.
            let r = Rect::new(2, foot - (h - 4).min(k.fh * 16 - 2), w - 4, (h - 4).min(k.fh * 16 - 2));
            ao(c, r.x, r.right() - 1, foot, 5);
            box3(c, r.x, r.w, foot, 5, 0, k.trim, Some((1, k.seed)), 2);
            c.rect_bevel(Rect::new(r.x - 1, r.y - 6, r.w + 2, 7), k.trim, 1, Z::new(4, 8));
            let mut m = Canvas::new(c.w(), c.h());
            m.fill_rect(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 6), Ix::INK, 1);
            c.inflate(&m, Ramp::ClothLinen, 3, Z::new(4, 6));
            c.rect_round(Rect::new(r.x + 3, r.y + 2, r.w - 6, 7), Ramp::ClothLinen, 1, 2, Z::new(6, 7));
            let b = Rect::new(r.x + 1, r.y + r.h / 3 + 2, r.w - 2, r.h - r.h / 3 - 7);
            let mut bm = Canvas::new(c.w(), c.h());
            bm.fill_rect(b, Ix::INK, 1);
            c.inflate(&bm, k.body, 2, Z::new(6, 8));
            c.retone(k.body, super::HARD);
            c.hline(b.x, b.right() - 1, b.y, k.body.at(Tone::Light), 8);
            c.hline(b.x, b.right() - 1, b.y + 2, k.body.at(Tone::Mid), 8);
            Stand::Tops([(Rect::new(r.x, r.y, r.w, r.h - 5), 6), (Rect::new(r.x - 1, r.y - 6, r.w + 2, 7), 12)])
        }
        "stove" => {
            // A kitchen range: an iron block with a hob, a firebox door with its grille alight
            // when the fire is in, a pipe up to the flue.
            let (x, sw) = (3, w - 6);
            ao(c, x, x + sw - 1, foot, 5);
            let (top, front) = box3(c, x, sw, foot, 14, 8, k.body, None, 3);
            c.retone(k.body, super::HARD);
            for hx in [top.x + 3, top.x + sw / 2 + 1] {
                c.ellipse(Rect::new(hx, top.y + 2, 7, 4), k.body.at(Tone::Shade), 5);
                c.ellipse(Rect::new(hx + 1, top.y + 2, 5, 3), k.body.at(Tone::Base), 5);
            }
            let fb = Rect::new(front.x + 3, front.y + 3, 9, 7);
            c.rect_bevel(fb, k.trim, 1, Z::new(4, 5));
            c.set_emitting(on);
            for gx in (fb.x + 2..fb.right() - 1).step_by(2) {
                let ix = if on { Ramp::Ember.at(Tone::High) } else { Ix::SEAM };
                c.vline(gx, fb.y + 2, fb.bottom() - 3, ix, 6);
            }
            c.set_emitting(false);
            c.fill_rect(Rect::new(front.right() - 7, front.y + 4, 4, 2), k.trim.at(Tone::Light), 5);
            post(c, top.right() - 5, top.y - 14, top.y + 2, 3, k.body, 6);
            Stand::Tops([(top, lid_height(14)), (Rect::default(), 0)])
        }
        "bench" | "pew" => {
            // A bench: a seat of boards on legs, a back of two rails; a pew's back is a panel.
            let (x, bw) = (1, w - 2);
            ao(c, x, x + bw - 1, foot, 4);
            let back_top = foot - 20;
            for lx in [x + 2, x + bw - 5] {
                post(c, lx, back_top, foot - 1, 3, k.trim, 3);
            }
            if k.look.shape == "pew" {
                planks(c, Rect::new(x, back_top, bw, 9), k.body, 2, true, false, k.seed, 4);
            } else {
                for y in [back_top + 1, back_top + 5] {
                    c.fill_normal(Rect::new(x, y, bw, 3), k.body.at(Tone::Base), parts::south(), 4);
                    c.hline(x, x + bw - 1, y, k.body.at(Tone::Light), 4);
                }
            }
            let (top, _) = box3(c, x, bw, foot - 5, 2, 5, k.body, Some((2, k.seed)), 5);
            Stand::Tops([(top, lid_height(7)), (Rect::default(), 0)])
        }
        "chair" => {
            // A kitchen chair: a rush seat, a ladder back, turned legs.
            let (x, cw) = (w / 2 - 5, 10);
            ao(c, x, x + cw - 1, foot, 4);
            for lx in [x, x + cw - 2] {
                post(c, lx, foot - 22, foot - 1, 2, k.body, 3);
            }
            for y in [foot - 21, foot - 17] {
                c.hline(x, x + cw - 1, y, k.body.at(Tone::Light), 4);
                c.hline(x, x + cw - 1, y + 1, k.body.at(Tone::Mid), 4);
            }
            let (top, _) = box3(c, x, cw, foot - 6, 2, 5, k.accent, None, 5);
            Stand::Tops([(top, lid_height(8)), (Rect::default(), 0)])
        }
        "pegs" => {
            // A row of pegs on a board, coats hanging from them.
            let r = Rect::new(1, foot - 26, w - 2, 4);
            c.fill_normal(r, k.body.at(Tone::Base), parts::south(), 4);
            c.hline(r.x, r.right() - 1, r.y, k.body.at(Tone::Light), 4);
            for (i, px) in (4..w - 3).step_by(6).enumerate() {
                c.dot(px, r.y + 1, k.trim.at(Tone::High), 5);
                let coat = [Ramp::ClothNavy, Ramp::ClothGrey, Ramp::ClothBrown][i % 3];
                c.polygon_cloth(
                    &[(px - 1, r.bottom()), (px + 1, r.bottom()), (px + 3, r.bottom() + 14), (px - 3, r.bottom() + 14)],
                    coat,
                    60,
                    Z::flat(5),
                );
            }
            Stand::Up(&[])
        }
        "register" => {
            // A great book open on a sloped stand.
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 4);
            post(c, cx - 1, foot - 12, foot - 1, 3, k.trim, 3);
            let b = Rect::new(cx - 9, foot - 18, 18, 7);
            c.fill_normal(Rect::new(b.x - 1, b.y + 1, b.w + 2, b.h), k.body.at(Tone::Base), parts::south(), 4);
            c.fill_normal(b, Ramp::ClothLinen.at(Tone::Light), crate::canvas::normal(0, -40), 5);
            c.vline(cx, b.y, b.bottom() - 1, Ramp::ClothLinen.at(Tone::Mid), 5);
            parts::writing(c, Rect::new(b.x + 1, b.y + 1, 7, 5), 3, Ramp::Slate.at(Tone::Mid), k.seed, 5);
            parts::writing(c, Rect::new(cx + 2, b.y + 1, 7, 5), 3, Ramp::Slate.at(Tone::Mid), k.seed ^ 3, 5);
            Stand::Up(&[])
        }
        "cupboard" | "locker" => cupboard(c, k),
        "dresser" => dresser(c, k),
        "plinth" => {
            // A low stone plinth with a bevelled top: an empty place for an exhibit.
            let r = Rect::new(4, foot - 20, w - 8, 18);
            c.rect_bevel(r, k.body, 3, Z::new(1, 3));
            Stand::Flat(3)
        }
        _ => return None,
    })
}

/// A fielded panel let into a door: a groove round it (in shade on the top and left, where the
/// frame cuts the light off, lit on the bottom and right) and the raised field inside it.
fn panel(c: &mut Canvas, r: Rect, ramp: Ramp, z: u8) {
    c.fill_normal(r, ramp.at(Tone::Mid), parts::south(), z);
    c.hline(r.x, r.right() - 1, r.y, ramp.at(Tone::Deep), z);
    c.vline(r.x, r.y, r.bottom() - 1, ramp.at(Tone::Shade), z);
    c.hline(r.x + 1, r.right() - 1, r.bottom() - 1, ramp.at(Tone::Light), z);
    c.vline(r.right() - 1, r.y + 1, r.bottom() - 1, ramp.at(Tone::Lift), z);
    if r.w >= 6 && r.h >= 6 {
        c.rect_bevel(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), ramp, 1, Z::new(z, z + 1));
    }
}

/// A door `r` hung in a carcass: its board (grain in wood), the gap round it in shade with its
/// lit leading edges, and two fielded panels; or, a locker's, louvres over a plain panel.
fn door(c: &mut Canvas, k: &Kit, r: Rect, locker: bool, seed: u32) {
    planks(c, r, k.body, 1, false, false, seed, 4);
    c.hline(r.x, r.right() - 1, r.y, k.body.at(Tone::Light), 4);
    c.vline(r.x, r.y, r.bottom() - 1, k.body.at(Tone::Lift), 4);
    c.vline(r.right(), r.y, r.bottom(), k.body.at(Tone::Deep), 4);
    c.hline(r.x, r.right(), r.bottom(), k.body.at(Tone::Deep), 4);
    let inner = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
    let upper = inner.h * 3 / 5;
    if locker {
        // Louvres to air what is left in it: slits in shade, each with the lit lip under it.
        for y in (inner.y + 1..inner.y + upper - 1).step_by(3) {
            c.hline(inner.x + 1, inner.right() - 2, y, k.body.at(Tone::Deep), 5);
            c.hline(inner.x + 1, inner.right() - 2, y + 1, k.body.at(Tone::Light), 5);
        }
        panel(c, Rect::new(inner.x, inner.y + upper + 1, inner.w, inner.h - upper - 1), k.body, 5);
    } else {
        panel(c, Rect::new(inner.x, inner.y, inner.w, upper), k.body, 5);
        panel(c, Rect::new(inner.x, inner.y + upper + 2, inner.w, inner.h - upper - 2), k.body, 5);
    }
}

/// A tall two-door cupboard against the wall, or a left-luggage locker in railway paint: a
/// moulded cornice, two panelled doors meeting at a seam with their brass knobs and a keyhole,
/// a plinth in the doors' shadow. A locker's doors are louvred and carry numbered brass tags.
fn cupboard(c: &mut Canvas, k: &Kit) -> Stand {
    let locker = k.look.shape == "locker";
    let (w, h, foot) = (k.w, k.h, k.foot());
    let (x, cw) = (1, w - 2);
    let depth = 4;
    let face = (h - 3 - depth).min(40);
    ao(c, x, x + cw - 1, foot, 6);
    let (top, front) = box3(c, x, cw, foot, face, depth, k.body, Some((2, k.seed)), 3);
    // The plinth, set back, in the shadow the doors throw down it.
    c.fill_normal(Rect::new(x + 1, foot - 2, cw - 2, 3), k.body.at(Tone::Shade), parts::south(), 3);
    c.hline(x + 1, x + cw - 2, foot - 2, k.body.at(Tone::Deep), 3);
    // The doors, meeting in the middle.
    let (d0, d1) = (front.y + 5, foot - 4);
    let mid = x + cw / 2;
    let left = Rect::new(x + 2, d0, mid - x - 3, d1 - d0);
    let right = Rect::new(mid, d0, x + cw - 3 - mid, d1 - d0);
    door(c, k, left, locker, k.seed ^ 0x0b);
    door(c, k, right, locker, k.seed ^ 0x0d);
    // The cornice over them, overhanging a px each side, and its shadow on the doors' heads.
    c.rect_bevel(Rect::new(x - 1, front.y, cw + 2, 4), k.trim, 1, Z::new(5, 6));
    c.hline(x, x + cw - 1, front.y + 4, k.body.at(Tone::Deep), 5);
    let ky = d0 + (d1 - d0) / 2;
    if locker {
        // A bar handle each, and a brass number on each door.
        for hx in [mid - 3, mid + 2] {
            c.fill_rect(Rect::new(hx, ky - 2, 1, 5), k.trim.at(Tone::Light), 6);
            c.dot(hx, ky + 2, k.trim.at(Tone::Shade), 6);
        }
        for d in [left, right] {
            let t = Rect::new(d.x + d.w / 2 - 3, d.bottom() - 7, 6, 4);
            c.rect_bevel(t, k.accent, 1, Z::new(6, 7));
            c.hline(t.x + 2, t.x + 3, t.y + 2, k.accent.at(Tone::Deep), 7);
        }
    } else {
        // Brass knobs either side of the seam, a keyhole under the right one.
        for kx in [mid - 3, mid + 1] {
            c.fill_rect(Rect::new(kx, ky, 2, 2), k.accent.at(Tone::Base), 6);
            c.dot(kx, ky, k.accent.at(Tone::High), 7);
            c.dot(kx + 1, ky + 1, k.accent.at(Tone::Shade), 6);
        }
        c.fill_rect(Rect::new(mid + 1, ky + 4, 2, 3), k.accent.at(Tone::Light), 6);
        c.dot(mid + 1, ky + 5, Ix::SEAM, 7);
    }
    Stand::Tops([(top, lid_height(face)), (Rect::default(), 0)])
}

/// A Welsh dresser against the wall: a base of three drawers over two doors and a panel, a
/// worktop with a jug on it, and a rack of two shelves above, the plates stood on edge behind
/// their rails and cups hung under the lower shelf.
fn dresser(c: &mut Canvas, k: &Kit) -> Stand {
    let (w, foot) = (k.w, k.foot());
    let (x, dw) = (1, w - 2);
    ao(c, x, x + dw - 1, foot, 6);
    // The rack first: it stands behind the worktop.
    let (r0, r1) = (4, foot - 20);
    planks(c, Rect::new(x + 3, r0, dw - 6, r1 - r0), k.trim, 6, false, false, k.seed ^ 5, 2);
    c.shade(Rect::new(x + 1, r0 - 2, dw - 2, 8), k.trim, 1);
    for px in [x, x + dw - 3] {
        post(c, px, r0, r1 + 1, 3, k.body, 4);
    }
    let shelves = [r0 + (r1 - r0) / 2 - 1, r1 - 4];
    let n = (dw - 8) / 7;
    let off = x + 4 + (dw - 8 - n * 7) / 2;
    for (i, &sy) in shelves.iter().enumerate() {
        // Plates stood on edge on the shelf: a willow-blue rim, the white well, a blue middle.
        for p in 0..n {
            let px = off + p * 7;
            let py = sy - 7;
            if parts::hash(k.seed, p, i as i32) % 6 == 0 {
                // A gap where one is out in use: a bowl lying there instead.
                c.fill_normal(Rect::new(px + 1, sy - 3, 5, 3), k.accent.at(Tone::Base), parts::south(), 5);
                c.hline(px + 1, px + 5, sy - 3, k.accent.at(Tone::Light), 5);
                c.hline(px + 2, px + 4, sy - 1, k.accent.at(Tone::Shade), 5);
                continue;
            }
            c.ellipse_lit(Rect::new(px, py, 7, 7), k.accent, Z::new(5, 6));
            c.ellipse(Rect::new(px + 1, py + 1, 5, 5), Ramp::ClothLinen.at(Tone::Light), 6);
            c.dot(px + 3, py + 3, k.accent.at(Tone::Light), 6);
            c.dot(px + 1, py + 2, Ramp::ClothLinen.at(Tone::High), 6);
            c.dot(px + 4, py + 5, Ramp::ClothLinen.at(Tone::Mid), 6);
        }
        c.fill_normal(Rect::new(x + 3, sy, dw - 6, 2), k.body.at(Tone::Light), parts::south(), 5);
        c.hline(x + 3, x + dw - 4, sy + 1, k.body.at(Tone::Mid), 5);
        c.hline(x + 3, x + dw - 4, sy + 2, k.trim.at(Tone::Deep), 3);
        // The rail in front of the plates.
        c.hline(x + 3, x + dw - 4, sy - 3, k.body.at(Tone::Lift), 6);
    }
    // Cups on hooks under the lower shelf.
    let hy = shelves[1] + 2;
    for (i, cx) in (x + 7..x + dw - 8).step_by(9).enumerate() {
        if i == 2 {
            continue;
        }
        c.dot(cx + 1, hy, k.body.at(Tone::Shade), 5);
        c.fill_normal(Rect::new(cx, hy + 1, 3, 2), Ramp::ClothLinen.at(Tone::Light), parts::south(), 6);
        c.dot(cx + 3, hy + 1, Ramp::ClothLinen.at(Tone::Mid), 6);
        c.hline(cx, cx + 2, hy + 2, k.accent.at(Tone::Base), 6);
    }
    c.rect_bevel(Rect::new(x - 1, r0 - 3, dw + 2, 4), k.body, 1, Z::new(6, 7));
    c.hline(x + 1, x + dw - 2, r0 + 1, k.trim.at(Tone::Deep), 3);
    // The base: worktop, three drawers, two doors either side of a panel, the plinth.
    let face = 15;
    let (top, front) = box3(c, x, dw, foot, face, 5, k.body, Some((3, k.seed)), 3);
    let dy = front.y + 1;
    let third = (dw - 4) / 3;
    for i in 0..3 {
        let r = Rect::new(x + 2 + i * third, dy, third - 1, 4);
        c.rect_bevel(r, k.body, 1, Z::new(4, 5));
        c.hline(r.x + r.w / 2 - 2, r.x + r.w / 2 + 1, r.y + 2, k.body.at(Tone::Deep), 6);
        c.dot(r.x + r.w / 2 - 1, r.y + 1, Ramp::Brass.at(Tone::Light), 6);
        c.dot(r.x + r.w / 2, r.y + 1, Ramp::Brass.at(Tone::Light), 6);
    }
    let (d0, d1) = (dy + 6, foot - 3);
    for i in 0..3 {
        let r = Rect::new(x + 2 + i * third, d0, third - 1, d1 - d0);
        if i == 1 {
            panel(c, r, k.body, 4);
            continue;
        }
        door(c, k, r, false, k.seed ^ (i as u32 + 17));
        let kx = if i == 0 { r.right() - 3 } else { r.x + 2 };
        c.fill_rect(Rect::new(kx, r.y + r.h / 2 - 1, 2, 2), Ramp::Brass.at(Tone::Base), 6);
        c.dot(kx, r.y + r.h / 2 - 1, Ramp::Brass.at(Tone::High), 7);
    }
    c.fill_normal(Rect::new(x + 1, foot - 1, dw - 2, 2), k.body.at(Tone::Shade), parts::south(), 3);
    c.hline(x + 1, x + dw - 2, foot - 2, k.body.at(Tone::Deep), 3);
    // A jug on the worktop, at the right end.
    let jx = x + dw - 12;
    c.ellipse_lit(Rect::new(jx, top.y - 5, 6, 8), Ramp::ClothLinen, Z::new(8, 10));
    c.hline(jx + 1, jx + 4, top.y - 1, k.accent.at(Tone::Base), 10);
    c.fill_rect(Rect::new(jx + 1, top.y - 6, 4, 1), Ramp::ClothLinen.at(Tone::Light), 10);
    c.vline(jx + 6, top.y - 4, top.y - 1, Ramp::ClothLinen.at(Tone::Mid), 9);
    Stand::Tops([(top, lid_height(face)), (Rect::default(), 0)])
}

/// What stands on a shelf: jars, a row of books, a tin, by the seed.
fn wares(c: &mut Canvas, k: &Kit, x: i32, y: i32, w: i32, row: i32) {
    let mut at = x;
    let mut i = 0;
    while at < x + w - 3 {
        let hsh = parts::hash(k.seed, row, i);
        match hsh % 4 {
            0 | 1 => {
                // Books: three spines, a tone apart.
                let ramps = [Ramp::ClothRed, Ramp::ClothNavy, Ramp::ClothGreen, Ramp::ClothMustard];
                for b in 0..3 {
                    let r = ramps[((hsh >> 4) as usize + b) % 4];
                    c.fill_normal(
                        Rect::new(at + b as i32 * 2, y - 5 + (b as i32 % 2), 2, 5 - (b as i32 % 2)),
                        r.at(Tone::Base),
                        parts::south(),
                        5,
                    );
                    c.dot(at + b as i32 * 2, y - 5 + (b as i32 % 2), r.at(Tone::Light), 5);
                }
                at += 7;
            }
            2 => {
                c.fill_normal(Rect::new(at, y - 4, 3, 4), Ramp::Glass.at(Tone::Base), parts::south(), 5);
                c.dot(at, y - 4, Ramp::Glass.at(Tone::Light), 5);
                c.hline(at, at + 2, y - 5, k.accent.at(Tone::Base), 5);
                at += 5;
            }
            _ => at += 3,
        }
        i += 1;
    }
}
