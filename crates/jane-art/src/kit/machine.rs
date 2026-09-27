//! `machine` and `small_thing` (ART.md §2.3): the Works' and the School's boards, boxes and
//! sockets, and the small things the tales leave about. `on` lights a machine's lamp or its
//! socket (emissive).
//!
//! Machine shapes: `socket`, `fuse_box`, `relay_box`, `call_box`, `breaker`, `generator`,
//! `valve`, `air_pump`, `clock`, `locker`. Small shapes: `bedroll`, `glove`, `veil`, `slippers`,
//! `boots`, `key`, `diving_helmet`, `hose`, `scarf`, `tangle`, `washing`, `candles`, `dinner`,
//! `tortoise`.

use jane_core::grid::Rect;

use super::parts::{self, ao, band, box3, post, writing};
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
        // --- The scatter (a dungeon's floor marks and wall hangings, `scatter_*`) ---------------
        "stain" => {
            // A stain soaked into the floor: overlapping blots of the body's dark, a wet sheen
            // on one edge.
            // Scaled to its footprint: a spill a cell across, a pool two.
            let (fw, fh) = (w, k.fh * 16);
            let h = parts::hash(k.seed, 1, 90);
            for i in 0..3 + fw / 16 {
                let hh = parts::hash(k.seed, i, 91);
                let (bw, bh) = (fw * 3 / 8 + (hh >> 12) as i32 % (fw / 4).max(1), fh / 4 + (hh >> 16) as i32 % (fh / 6).max(1));
                let (x, y) = (2 + (hh % (fw - bw - 3).max(1) as u32) as i32, foot - fh * 2 / 3 + ((hh >> 8) % (fh / 3).max(1) as u32) as i32);
                c.ellipse(Rect::new(x, y, bw, bh), b.at(if i == 0 { Tone::Shade } else { Tone::Deep }), 1);
            }
            c.fill_rect(Rect::new(4 + (h % (fw as u32 / 2)) as i32, foot - fh / 2, 2, 1), b.at(Tone::Light), 1);
            Stand::Flat(1)
        }
        "papers" => {
            // Loose sheets dropped on the floor, written on, one screwed into a ball.
            for i in 0..2 {
                let hh = parts::hash(k.seed, i, 92);
                let (x, y) = (1 + i * 6 + (hh % 2) as i32, foot - 11 + (hh >> 4) as i32 % 3);
                let lean = if hh & 1 == 0 { 1 } else { -1 };
                c.polyline_fill(&[(x, y + 1), (x + 6, y), (x + 7 + lean, y + 7), (x + 1 + lean, y + 8)], Ramp::ClothLinen.at(Tone::Light), 2);
                for r in 0..3 {
                    c.hline(x + 2, x + 5, y + 2 + 2 * r, Ramp::ClothLinen.at(Tone::Mid), 2);
                }
                c.dot(x + 6, y, Ramp::ClothLinen.at(Tone::High), 2);
            }
            c.disc_lit(12, foot - 3, 1, Ramp::ClothLinen, Z::flat(3));
            Stand::Flat(3)
        }
        "books" => {
            // Books fallen from a shelf: two lying open-flat and one on its spine.
            let spines = [Ramp::ClothRed, Ramp::ClothGreen, Ramp::ClothNavy, Ramp::Leather];
            for i in 0..3 {
                let hh = parts::hash(k.seed, i, 93);
                let r = spines[(hh % 4) as usize];
                let (x, y) = (1 + i * 4 + (hh >> 4) as i32 % 2, foot - 5 - i * 2 - (hh >> 6) as i32 % 2);
                c.fill_rect(Rect::new(x, y, 6, 3), r.at(Tone::Base), 2 + i as u8);
                c.hline(x, x + 5, y, r.at(Tone::Light), 2 + i as u8);
                c.vline(x + 5, y + 1, y + 2, Ramp::ClothLinen.at(Tone::Light), 2 + i as u8);
            }
            Stand::Flat(4)
        }
        "shards" => {
            // Broken glass from a case: splinters catching the light.
            for i in 0..5 {
                let hh = parts::hash(k.seed, i, 94);
                let (x, y) = (2 + (hh % 11) as i32, foot - 9 + ((hh >> 8) % 7) as i32);
                c.polyline_fill(&[(x, y), (x + 2, y + 1), (x, y + 2)], Ramp::Glass.at(Tone::Light), 1);
                c.dot(x, y, Ramp::Glass.at(Tone::High), 1);
            }
            Stand::Flat(1)
        }
        "leaves" => {
            // Leaves blown in or silt left by the water: small ovals in two or three colours.
            let r = [b, k.accent, k.trim];
            let (fw, fh) = (w, k.fh * 16);
            for i in 0..(6 * fw * fh / 256) {
                let hh = parts::hash(k.seed, i, 95);
                let (x, y) = (1 + (hh % (fw - 4) as u32) as i32, foot - fh + 5 + ((hh >> 8) % (fh - 6) as u32) as i32);
                let ramp = r[(hh >> 16) as usize % 3];
                c.fill_rect(Rect::new(x, y, 3, 2), ramp.at(Tone::Base), 1);
                c.dot(x, y, ramp.at(Tone::Light), 1);
            }
            Stand::Flat(1)
        }
        "cogs" => {
            // A dropped cog and bolts, in oil.
            c.ellipse(Rect::new(3, foot - 7, 10, 4), Ramp::ClothBlack.at(Tone::Shade), 1);
            let (gx, gy) = (7, foot - 8);
            c.disc_lit(gx, gy, 3, b, Z::flat(2));
            for (dx, dy) in [(0, -4), (4, 0), (0, 4), (-4, 0), (3, -3), (3, 3), (-3, 3), (-3, -3)] {
                c.dot(gx + dx, gy + dy, b.at(Tone::Base), 2);
            }
            c.dot(gx, gy, b.at(Tone::Deep), 3);
            for (x, y) in [(12, foot - 4), (13, foot - 9)] {
                c.fill_rect(Rect::new(x, y, 2, 1), b.at(Tone::Light), 2);
            }
            Stand::Flat(3)
        }
        "rug" => {
            // A rug on the floor, worn: a field in the body's colour, a border in the accent,
            // a pattern down its middle, fringes at the ends, a corner turned up.
            let r = Rect::new(1, foot - k.fh * 16 + 5, w - 2, k.fh * 16 - 7);
            c.fill_normal(r, b.at(Tone::Base), FLAT, 1);
            c.shade(Rect::new(r.x + r.w / 2, r.y, r.w / 2, r.h), b, 1);
            for (x0, y0, x1, y1) in [(r.x + 1, r.y + 1, r.right() - 2, r.y + 1), (r.x + 1, r.bottom() - 2, r.right() - 2, r.bottom() - 2)] {
                c.hline(x0, x1, y0.min(y1), k.accent.at(Tone::Base), 1);
            }
            c.vline(r.x + 1, r.y + 1, r.bottom() - 2, k.accent.at(Tone::Base), 1);
            c.vline(r.right() - 2, r.y + 1, r.bottom() - 2, k.accent.at(Tone::Shade), 1);
            let cy = r.y + r.h / 2;
            for x in (r.x + 4..r.right() - 4).step_by(4) {
                c.fill_rect(Rect::new(x, cy - 1, 2, 2), k.accent.at(Tone::Light), 1);
            }
            for x in (r.x..r.right()).step_by(2) {
                c.dot(x, r.y - 1, k.accent.at(Tone::Light), 1);
                c.dot(x, r.bottom(), k.accent.at(Tone::Light), 1);
            }
            c.polyline_fill(&[(r.right() - 5, r.bottom() - 1), (r.right() - 1, r.bottom() - 1), (r.right() - 1, r.bottom() - 5)], b.at(Tone::Light), 2);
            Stand::Flat(2)
        }
        "bonepile" => {
            // Bones gone down in a heap: long bones crossed, ribs, a skull on top.
            let bone = Ramp::Bone;
            let (fw, fh) = (w, k.fh * 16);
            for i in 0..(4 * fw / 16) {
                let hh = parts::hash(k.seed, i, 98);
                let (x, y) = (2 + (hh % (fw - 10) as u32) as i32, foot - fh / 2 + ((hh >> 8) % (fh / 3) as u32) as i32);
                let (dx, dy) = if hh & 0x100 == 0 { (6, 2) } else { (5, -2) };
                c.line((x, y), (x + dx, y + dy), bone.at(Tone::Light), 2, 2);
                c.dot(x, y, bone.at(Tone::High), 2);
                c.dot(x + dx, y + dy, bone.at(Tone::Base), 2);
            }
            let (sx, sy) = (fw / 2 - 3, foot - fh / 2 - 2);
            c.ellipse_lit(Rect::new(sx, sy, 6, 5), bone, Z::flat(4));
            c.dot(sx + 1, sy + 2, Ix::SEAM, 4);
            c.dot(sx + 3, sy + 2, Ix::SEAM, 4);
            c.hline(sx + 1, sx + 4, sy + 4, bone.at(Tone::Shade), 4);
            Stand::Flat(4)
        }
        "chains" => {
            // Chains down the wall from an iron ring, a hook at the end of one.
            let iron = Ramp::Iron;
            for (x, len) in [(5, 18), (10, 13)] {
                c.fill_rect(Rect::new(x - 1, 1, 3, 2), iron.at(Tone::Light), 4);
                for y in (3..3 + len).step_by(2) {
                    let t = if (y / 2) % 2 == 0 { Tone::Light } else { Tone::Shade };
                    c.fill_rect(Rect::new(x, y, 1 + (y / 2) % 2, 2), iron.at(t), 4);
                }
            }
            c.line((10, 16), (12, 18), iron.at(Tone::Light), 2, 4);
            Stand::Up(&[])
        }
        "cobweb" => {
            // A web in the wall's corner over this cell: threads out from the corner, three
            // arcs across them, a strand hanging.
            let right = k.seed & 1 == 1;
            let (ox, oy) = (if right { w - 1 } else { 0 }, 1);
            let d = if right { -1 } else { 1 };
            let silk = Ramp::HairWhite;
            let ends = [(ox + d * 13, oy), (ox + d * 11, oy + 7), (ox + d * 6, oy + 12), (ox, oy + 14)];
            for &e in &ends {
                crate::creature::stair(c, (ox, oy), e, silk.at(Tone::Base), 4);
            }
            for r in [4, 8, 11] {
                let pts: Vec<(i32, i32)> = ends.iter().map(|&(ex, ey)| (ox + (ex - ox) * r / 13, oy + (ey - oy) * r / 13)).collect();
                for p in pts.windows(2) {
                    crate::creature::stair(c, p[0], p[1], silk.at(Tone::Light), 4);
                }
            }
            crate::creature::stair(c, ends[1], (ends[1].0, ends[1].1 + 8), silk.at(Tone::Mid), 4);
            Stand::Up(&[])
        }
        "poster" => {
            // A notice pinned on the wall over this cell: a sheet, curled at a corner, written
            // on, a brass pin at the top.
            let (x, y) = (3, 2);
            let r = Rect::new(x, y, 10, 12);
            c.fill_normal(r, b.at(Tone::Light), parts::south(), 3);
            c.hline(r.x, r.right() - 1, r.y, b.at(Tone::High), 3);
            writing(c, Rect::new(r.x + 2, r.y + 3, r.w - 4, r.h - 5), 4, k.accent.at(Tone::Deep), k.seed, 4);
            c.fill_rect(Rect::new(r.right() - 3, r.bottom() - 3, 3, 3), b.at(Tone::Shade), 4);
            c.dot(r.x + r.w / 2, r.y + 1, Ramp::Brass.at(Tone::Light), 5);
            Stand::Up(&[])
        }
        "frame" => {
            // A small portrait hung on the wall over this cell: a gilt frame, a dark ground,
            // a pale face looking out.
            let r = Rect::new(3, 1, 10, 13);
            c.rect_bevel(r, Ramp::Brass, 1, Z::new(3, 4));
            let inner = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
            c.fill_normal(inner, b.at(Tone::Shade), parts::south(), 4);
            c.ellipse(Rect::new(inner.x + 2, inner.y + 1, 3, 4), Ramp::Skin.at(Tone::Mid), 5);
            c.fill_rect(Rect::new(inner.x + 1, inner.bottom() - 3, 5, 3), Ramp::ClothBlack.at(Tone::Base), 5);
            Stand::Up(&[])
        }
        "moss" => {
            // Damp down the wall over this cell: a dark wet run, green along it, a green clump
            // where it meets the floor.
            for (x, t) in [(6, Tone::Deep), (7, Tone::Shade), (8, Tone::Deep)] {
                c.vline(x, 2, 15, b.at(t), 3);
            }
            for i in 0..5 {
                let hh = parts::hash(k.seed, i, 97);
                let y = 3 + (hh % 11) as i32;
                c.fill_rect(Rect::new(5 + (hh >> 8) as i32 % 3, y, 2, 2), k.accent.at(if i % 2 == 0 { Tone::Base } else { Tone::Light }), 4);
            }
            c.ellipse(Rect::new(3, foot - 5, 9, 4), k.accent.at(Tone::Base), 3);
            c.hline(4, 10, foot - 5, k.accent.at(Tone::Light), 3);
            Stand::Up(&[])
        }
        "tools" => {
            // A pick and a shovel leant against the wall: their hafts up the face, the heads
            // on the floor and at the top.
            ao(c, 3, 12, foot, 3);
            c.line((4, foot - 1), (7, 3), k.trim.at(Tone::Base), 2, 4);
            c.line((7, 3), (4, 2), b.at(Tone::Light), 2, 5);
            c.line((7, 3), (11, 5), b.at(Tone::Base), 2, 5);
            c.line((11, foot - 3), (10, 6), k.trim.at(Tone::Light), 1, 4);
            c.line((12, foot - 3), (11, 6), k.trim.at(Tone::Base), 1, 4);
            c.fill_rect(Rect::new(9, foot - 5, 5, 4), b.at(Tone::Base), 5);
            c.hline(9, 13, foot - 5, b.at(Tone::Light), 5);
            Stand::Up(&[])
        }
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
