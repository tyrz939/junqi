//! `barrier` (ART.md §2.3): what stands in the way. Shapes: `door`, `gate` (edge-on when its
//! footprint is taller than wide), `shutter`, `boards`, `drywall`, `crack`, `web`, `rails`,
//! `sleepers`, `roots`. An open gate is a doorway and is not drawn (the presenter skips it).

use jane_core::grid::Rect;

use super::parts::{self, ao, blocks, box3, lid_height, planks, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let _ = state;
    let (w, h, foot) = (k.w, k.h, k.foot());
    let edge_on = k.fh > k.fw;
    Some(match k.look.shape {
        "door" => {
            // A door in its frame: a stone or timber surround, a planked leaf with strap hinges
            // and a ring, a worn step before it.
            let (dw, dh) = ((w - 10).min(20), (h - 6).min(30));
            let x = (w - dw) / 2;
            let top = foot - 2 - dh;
            ao(c, x - 3, x + dw + 2, foot, 4);
            c.rect_bevel(Rect::new(x - 3, top - 3, dw + 6, dh + 4), k.trim, 2, Z::new(3, 5));
            planks(c, Rect::new(x, top, dw, dh), k.body, (dw / 4).max(2), false, false, k.seed, 6);
            c.hline(x, x + dw - 1, top, k.body.at(Tone::Shade), 6);
            for y in [top + 5, top + dh - 6] {
                c.fill_rect(Rect::new(x, y, dw * 2 / 3, 2), Ramp::Iron.at(Tone::Mid), 7);
                c.hline(x, x + dw * 2 / 3 - 1, y, Ramp::Iron.at(Tone::Light), 7);
            }
            c.disc_lit(x + dw - 4, top + dh / 2, 1, k.accent, Z::flat(8));
            c.fill_normal(Rect::new(x - 2, foot - 2, dw + 4, 2), k.trim.at(Tone::Light), FLAT, 2);
            Stand::Up(&[])
        }
        "door_broken" => {
            // A door off one hinge, hanging askew in its frame, the dark behind it.
            let (dw, dh) = ((w - 10).min(20), (h - 6).min(30));
            let x = (w - dw) / 2;
            let top = foot - 2 - dh;
            ao(c, x - 3, x + dw + 2, foot, 4);
            c.rect_bevel(Rect::new(x - 3, top - 3, dw + 6, dh + 4), k.trim, 2, Z::new(3, 5));
            c.fill_normal(Rect::new(x, top, dw, dh), Ramp::ClothBlack.at(Tone::Deep), parts::south(), 5);
            let pts = [(x + 1, top + 1), (x + dw - 4, top + 4), (x + dw - 2, foot - 2), (x + 3, foot - 4)];
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline_fill(&pts, Ix::INK, 1);
            c.inflate(&m, k.body, 2, Z::new(6, 7));
            c.retone(k.body, super::HARD);
            for y in [top + 8, top + 17] {
                c.line((x + 2, y), (x + dw - 3, y + 2), k.body.at(Tone::Shade), 1, 8);
            }
            c.line((x + 6, top + 10), (x + 9, top + 15), Ix::SEAM, 1, 8);
            Stand::Up(&[])
        }
        "barn_doors" => {
            // A barn's great doors: two planked leaves with their braces in a Z, strap hinges,
            // the gap where they meet dark.
            let top = foot - (h - 4).min(40);
            ao(c, 1, w - 2, foot, 6);
            c.rect_bevel(Rect::new(1, top - 3, w - 2, foot - top + 3), k.trim, 2, Z::new(3, 5));
            for (x0, x1) in [(4, w / 2 - 1), (w / 2 + 1, w - 5)] {
                let r = Rect::new(x0, top, x1 - x0 + 1, foot - top - 1);
                planks(c, r, k.body, (r.w / 4).max(3), false, false, k.seed ^ x0 as u32, 6);
                for y in [r.y + 2, r.bottom() - 4] {
                    c.fill_rect(Rect::new(r.x, y, r.w, 2), k.body.at(Tone::Light), 7);
                }
                c.line((r.x + 1, r.bottom() - 3), (r.right() - 2, r.y + 3), k.body.at(Tone::Light), 2, 7);
            }
            c.vline(w / 2, top, foot - 2, Ramp::ClothBlack.at(Tone::Deep), 7);
            Stand::Up(&[])
        }
        "arch" => {
            // An arch bricked up: a stone arch in the wall, the bricks laid in it newer and
            // redder than the stone round them.
            let top = foot - (h - 3).min(34);
            ao(c, 1, w - 2, foot, 6);
            blocks(c, Rect::new(0, top, w, foot - top + 1), k.body, 4, 8, false, k.seed, 3);
            let (ax0, ax1) = (w / 2 - 12, w / 2 + 11);
            let mut m = Canvas::new(c.w(), c.h());
            m.fill_rect(Rect::new(ax0, top + 12, ax1 - ax0 + 1, foot - top - 12), Ix::INK, 1);
            m.ellipse(Rect::new(ax0, top + 2, ax1 - ax0 + 1, 22), Ix::INK, 1);
            for y in 0..c.h() {
                for x in 0..c.w() {
                    if m.get(x, y).is_opaque() {
                        let course = (y - top) / 3;
                        let off = if course % 2 == 0 { 0 } else { 3 };
                        let t = if (y - top) % 3 == 2 || (x + off) % 6 == 5 { Tone::Shade } else if (y - top) % 3 == 0 { Tone::Light } else { Tone::Base };
                        c.put(x, y, k.accent.at(t), parts::south(), 4);
                    }
                }
            }
            for i in 0..9 {
                let a = jane_core::angle::Angle((32768 + i * 4096) as u16);
                let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
                let (bx, by) = (w / 2 + ((co * 13) >> 15), top + 13 + ((s * 11) >> 15));
                c.rect_bevel(Rect::new(bx - 2, by - 2, 5, 4), k.body, 1, Z::new(5, 6));
            }
            Stand::Up(&[])
        }
        "broken_rails" => {
            // A length of track, broken: a rail bent up out of its chairs, sleepers split, a gap.
            let (y0, y1) = (foot - 20, foot - 4);
            for x in (1..w - 2).step_by(6) {
                let split = parts::hash(k.seed, x, 90) % 3 == 0;
                c.fill_normal(Rect::new(x, y0, 3, y1 - y0 + 1), k.body.at(Tone::Base), FLAT, 1);
                c.vline(x, y0, y1, k.body.at(Tone::Light), 1);
                if split {
                    c.vline(x + 1, y0 + 4, y1 - 3, k.body.at(Tone::Deep), 1);
                }
            }
            c.fill_normal(Rect::new(0, y0 + 3, w / 2 - 4, 2), k.trim.at(Tone::Mid), FLAT, 2);
            c.hline(0, w / 2 - 5, y0 + 3, k.trim.at(Tone::High), 2);
            c.line((w / 2 + 2, y0 + 3), (w - 1, y0 - 4), k.trim.at(Tone::Light), 2, 6);
            c.fill_normal(Rect::new(0, y1 - 4, w, 2), k.trim.at(Tone::Mid), FLAT, 2);
            c.hline(0, w - 1, y1 - 4, k.trim.at(Tone::High), 2);
            Stand::Up(&[])
        }
        "gate" if edge_on => {
            // A gate seen along its length: its top rail running up the footprint between two
            // posts, the rails below it stacked into a lit edge and a shaded one.
            let cx = w / 2;
            ao(c, cx - 3, cx + 2, foot, 4);
            for py in [k.back() + 2, foot - 2] {
                post(c, cx - 2, py - 18, py, 4, k.trim, 4);
                c.fill_normal(Rect::new(cx - 2, py - 19, 4, 2), k.trim.at(Tone::Light), FLAT, 5);
            }
            c.fill_normal(Rect::new(cx - 1, k.back() - 12, 2, foot - k.back() + 8), k.body.at(Tone::Base), crate::canvas::normal(-40, 0), 5);
            c.vline(cx - 1, k.back() - 12, foot - 6, k.body.at(Tone::Light), 6);
            c.vline(cx, k.back() - 12, foot - 6, k.body.at(Tone::Mid), 6);
            Stand::Up(&[])
        }
        "gate" => {
            // A five-barred gate hung between two posts, its brace rising from the hinge.
            let top = foot - 18;
            ao(c, 1, w - 2, foot, 4);
            for x in [1, w - 5] {
                post(c, x, top - 3, foot - 1, 4, k.trim, 4);
                c.fill_normal(Rect::new(x, top - 4, 4, 2), k.trim.at(Tone::Light), FLAT, 5);
            }
            let (x0, x1) = (5, w - 6);
            for i in 0..5 {
                let y = top + i * 4;
                c.fill_normal(Rect::new(x0, y, x1 - x0 + 1, 2), k.body.at(Tone::Base), parts::south(), 5);
                c.hline(x0, x1, y, k.body.at(Tone::Light), 5);
            }
            for x in [x0 + 2, x1 - 2] {
                c.fill_normal(Rect::new(x, top, 2, 17), k.body.at(Tone::Mid), parts::south(), 6);
            }
            c.line((x0 + 3, top + 15), (x1 - 3, top + 1), k.body.at(Tone::Base), 2, 6);
            c.fill_rect(Rect::new(x0, top + 1, 3, 2), k.accent.at(Tone::Mid), 7);
            c.fill_rect(Rect::new(x0, top + 13, 3, 2), k.accent.at(Tone::Mid), 7);
            Stand::Up(&[])
        }
        "shutter" => {
            // A roller shutter: corrugated iron between two runners, the drum's housing over it.
            let (r, runner) = if edge_on {
                (Rect::new(w / 2 - 2, k.back() - 16, 4, h - k.back() + 14), false)
            } else {
                (Rect::new(2, foot - 24, w - 4, 24), true)
            };
            ao(c, r.x, r.right() - 1, foot, 4);
            c.fill_normal(r, k.body.at(Tone::Base), parts::south(), 4);
            if runner {
                for y in (r.y + 4..r.bottom()).step_by(3) {
                    c.hline(r.x, r.right() - 1, y, k.body.at(Tone::Light), 5);
                    c.hline(r.x, r.right() - 1, y + 1, k.body.at(Tone::Mid), 5);
                }
                c.fill_normal(Rect::new(r.x - 1, r.y - 1, r.w + 2, 4), k.trim.at(Tone::Base), parts::south(), 6);
                c.hline(r.x - 1, r.right(), r.y - 1, k.trim.at(Tone::Light), 6);
                for x in [r.x, r.right() - 2] {
                    c.fill_rect(Rect::new(x, r.y + 3, 2, r.h - 3), k.trim.at(Tone::Shade), 6);
                }
            } else {
                c.vline(r.x, r.y, r.bottom() - 1, k.body.at(Tone::Light), 5);
                c.vline(r.right() - 1, r.y, r.bottom() - 1, k.body.at(Tone::Shade), 5);
            }
            Stand::Up(&[])
        }
        "boards" => {
            // Planks nailed across a gap (or a window): two or three boards askew, nail heads.
            ao(c, 1, w - 2, foot, 4);
            let top = foot - (h - 4).min(22);
            for (i, (y, tilt)) in [(top + 2, 1), (top + 8, -1), (top + 14, 0)].into_iter().enumerate() {
                if y + 5 > foot {
                    break;
                }
                let r = Rect::new(1, y, w - 2, 5);
                planks(c, r, k.body, 1, true, false, k.seed ^ i as u32, 5 + i as u8);
                if tilt != 0 {
                    c.hline(1, w / 2, y + if tilt > 0 { 4 } else { 0 }, k.body.at(Tone::Shade), 6 + i as u8);
                }
                for x in [3, w - 5] {
                    c.dot(x, y + 2, Ramp::Iron.at(Tone::Light), 7 + i as u8);
                }
            }
            Stand::Up(&[])
        }
        "drywall" | "crack" => {
            // A wall of stone: dry laid with capstones, or dressed and cracked through.
            let (r, top) = if edge_on {
                let r = Rect::new(w / 2 - 5, k.back() - 8, 10, h - k.back() + 7);
                (r, Rect::new(r.x, r.y - 3, r.w, 3))
            } else {
                let r = Rect::new(0, foot - 14, w, 15);
                (r, Rect::new(0, r.y - 5, w, 5))
            };
            let (bh, bw) = if k.look.shape == "drywall" { (3, 5) } else { (4, 8) };
            blocks(c, r, k.body, bh, bw, false, k.seed, 4);
            blocks(c, top, k.body, top.h, bw + 2, true, k.seed ^ 7, 5);
            c.hline(top.x, top.right() - 1, top.bottom() - 1, k.body.at(Tone::Light), 5);
            if k.look.shape == "crack" {
                let (mut x, mut y) = (r.x + r.w / 2, r.y);
                let mut i = 0;
                while y < r.bottom() - 1 {
                    let h2 = parts::hash(k.seed, i, 70);
                    let nx = x + (h2 % 3) as i32 - 1;
                    c.line((x, y), (nx, y + 2), Ix::SEAM, 1, 6);
                    c.dot(nx + 1, y + 1, k.body.at(Tone::Light), 6);
                    x = nx;
                    y += 2;
                    i += 1;
                }
            }
            Stand::Tops([(top, lid_height(r.h)), (Rect::default(), 0)])
        }
        "web" => {
            // A web: radial threads and rings of silk, pale and fine, catching the light.
            let (cx, cy) = (w / 2, foot - (h - 2) / 2);
            let silk = Ramp::HairWhite;
            let rad = (w.min(h) / 2 - 1).max(4);
            for i in 0..8 {
                let a = jane_core::angle::Angle((i * 8192) as u16);
                let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
                c.line((cx, cy), (cx + ((co * rad) >> 15), cy + ((s * rad) >> 15)), silk.at(Tone::Mid), 1, 3);
            }
            for r in [rad / 3, 2 * rad / 3, rad - 1] {
                c.ellipse(Rect::new(cx - r, cy - r, 2 * r + 1, 2 * r + 1), silk.at(Tone::Light), 3);
                c.ellipse(Rect::new(cx - r + 1, cy - r + 1, 2 * r - 1, 2 * r - 1), Ix::CLEAR, 0);
            }
            for i in 0..8 {
                let a = jane_core::angle::Angle((i * 8192) as u16);
                let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
                c.line((cx, cy), (cx + ((co * rad) >> 15), cy + ((s * rad) >> 15)), silk.at(Tone::Base), 1, 3);
            }
            Stand::Flat(if k.look.rise > 0 { 20 } else { 1 })
        }
        "rails" => {
            // A length of track: sleepers across, two rails along, lit on their tops.
            let (y0, y1) = (foot - 13, foot - 2);
            for x in (1..w - 2).step_by(5) {
                c.fill_normal(Rect::new(x, y0, 3, y1 - y0 + 1), k.body.at(Tone::Base), FLAT, 1);
                c.vline(x, y0, y1, k.body.at(Tone::Light), 1);
            }
            for y in [y0 + 2, y1 - 3] {
                c.fill_normal(Rect::new(0, y, w, 2), k.trim.at(Tone::Mid), FLAT, 2);
                c.hline(0, w - 1, y, k.trim.at(Tone::High), 2);
            }
            Stand::Flat(2)
        }
        "sleepers" => {
            // Old sleepers stacked in a crib, their ends to the viewer.
            ao(c, 1, w - 2, foot, 4);
            let (bw, face) = (w - 2, 5);
            box3(c, 1, bw, foot, face, 3, k.body, Some((1, k.seed)), 3);
            box3(c, 3, bw - 4, foot - face, face, 3, k.body, Some((1, k.seed ^ 2)), 5);
            Stand::Up(&[])
        }
        "roots" => {
            // A wall of roots: thick tangled limbs lit on their tops, knotted, dark between.
            let top = foot - (h - 2).min(28);
            c.fill_normal(Rect::new(1, top + 2, w - 2, foot - top - 2), k.body.at(Tone::Deep), parts::south(), 2);
            for i in 0..7 {
                let hh = parts::hash(k.seed, i, 80);
                let y0 = top + (hh % (foot - top) as u32) as i32;
                let y1 = top + ((hh >> 8) % (foot - top) as u32) as i32;
                c.line((0, y0), (w - 1, y1), k.body.at(Tone::Base), 3, 4 + (i % 3) as u8);
                c.line((0, y0 - 1), (w - 1, y1 - 1), k.body.at(Tone::Light), 1, 4 + (i % 3) as u8);
            }
            Stand::Up(&[])
        }
        _ => return None,
    })
}
