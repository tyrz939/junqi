//! `structure` (ART.md §2.3): anything too small to be a building and too big to be furniture.
//! Shapes: `well`, `trough`, `fountain`, `pillar`, `chimney`, `coop`, `woodpile`,
//! `washing_line`, `cart`, `minecart`, `trolley`, `anvil`, `bale`, `pillar_box`, `phone_box`,
//! `ticket_window`, `shelter`, `stall`, `pump`, `beehive`, `scarecrow`, `tent`, `hoist`.

use jane_core::grid::Rect;

use super::parts::{self, ao, band, blocks, box3, glass, lid_height, planks, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z, normal};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let _ = state;
    let (w, h, foot) = (k.w, k.h, k.foot());
    Some(match k.look.shape {
        "well" => {
            // A round well of coursed stone, its dark water, two posts and a roof over the
            // winch with its rope and bucket.
            let (x, ww) = (3, w - 6);
            ao(c, x, x + ww - 1, foot, 6);
            let ring = Rect::new(x, foot - 20, ww, 10);
            blocks(c, Rect::new(x, foot - 11, ww, 11), k.body, 3, 5, false, k.seed, 3);
            c.ellipse(ring, k.body.at(Tone::Light), 5);
            c.ellipse(Rect::new(x + 3, ring.y + 2, ww - 6, 6), Ramp::Water.at(Tone::Deep), 5);
            c.hline(x + 6, x + ww - 8, ring.y + 4, Ramp::Water.at(Tone::Shade), 5);
            let top = foot - (h - 2).min(40);
            for px in [x, x + ww - 3] {
                post(c, px, top + 6, foot - 12, 3, k.trim, 6);
            }
            c.line((x + 2, top + 10), (x + ww - 3, top + 10), k.trim.at(Tone::Base), 2, 7);
            c.vline(w / 2, top + 11, ring.y + 3, Ramp::Reed.at(Tone::Base), 7);
            c.rect_bevel(Rect::new(w / 2 - 2, ring.y + 1, 4, 3), k.trim, 1, Z::new(7, 8));
            roof(c, k, x - 2, ww + 4, top, 7);
            Stand::Tops([(ring, lid_height(11)), (Rect::default(), 0)])
        }
        "trough" => {
            // A stone trough, water to its lip.
            let (x, tw) = (1, w - 2);
            ao(c, x, x + tw - 1, foot, 5);
            let (top, _) = box3(c, x, tw, foot, 8, 7, k.body, None, 3);
            c.retone(k.body, super::HARD);
            c.fill_normal(Rect::new(top.x + 2, top.y + 2, top.w - 4, top.h - 3), Ramp::Water.at(Tone::Base), FLAT, 5);
            c.hline(top.x + 3, top.x + top.w / 2, top.y + 3, Ramp::Water.at(Tone::Light), 5);
            Stand::Tops([(top, lid_height(8)), (Rect::default(), 0)])
        }
        "fountain" => {
            // A basin of dressed stone, a column with a bowl, water falling into the pool.
            let (x, fw) = (2, w - 4);
            ao(c, x, x + fw - 1, foot, 8);
            let basin = Rect::new(x, foot - 24, fw, 18);
            blocks(c, Rect::new(x, foot - 8, fw, 8), k.body, 4, 8, false, k.seed, 3);
            c.ellipse(basin, k.body.at(Tone::Light), 5);
            c.ellipse(Rect::new(x + 3, basin.y + 2, fw - 6, basin.h - 4), Ramp::Water.at(Tone::Base), 5);
            c.ellipse(Rect::new(x + 8, basin.y + 5, fw - 16, basin.h - 10), Ramp::Water.at(Tone::Light), 5);
            let cx = w / 2;
            c.polygon_lit(&[(cx - 2, foot - 36), (cx + 1, foot - 36), (cx + 2, foot - 16), (cx - 3, foot - 16)], k.body, 90, Z::flat(7));
            c.ellipse(Rect::new(cx - 7, foot - 40, 14, 5), k.body.at(Tone::Light), 8);
            for dx in [-6, 5] {
                c.vline(cx + dx, foot - 36, foot - 20, Ramp::Water.at(Tone::High), 8);
            }
            c.retone(k.body, super::HARD);
            Stand::Tops([(basin, lid_height(6)), (Rect::default(), 0)])
        }
        "pillar" => {
            // A stone column with a base and a capital.
            let cx = w / 2;
            let top = foot - (h - 2).min(44);
            ao(c, cx - 8, cx + 7, foot, 6);
            c.rect_bevel(Rect::new(cx - 8, foot - 5, 16, 5), k.body, 1, Z::new(2, 4));
            c.polygon_lit(&[(cx - 5, top + 5), (cx + 4, top + 5), (cx + 4, foot - 5), (cx - 5, foot - 5)], k.body, 110, Z::new(4, 6));
            c.retone(k.body, super::HARD);
            for x in [cx - 3, cx, cx + 2] {
                c.vline(x, top + 6, foot - 6, k.body.at(Tone::Mid), 6);
            }
            c.rect_bevel(Rect::new(cx - 8, top, 16, 5), k.body, 1, Z::new(6, 7));
            Stand::Up(&[])
        }
        "chimney" => {
            // A brick stack with a stone cap and two pots: the chimney over a roof.
            let cx = w / 2;
            let top = foot - (h - 2).min(30);
            blocks(c, Rect::new(cx - 6, top + 6, 12, foot - top - 6), k.body, 3, 4, false, k.seed, 3);
            c.rect_bevel(Rect::new(cx - 7, top + 4, 14, 3), Ramp::Stone, 1, Z::new(4, 5));
            for px in [cx - 5, cx + 1] {
                c.polygon_lit(&[(px, top), (px + 3, top), (px + 3, top + 4), (px, top + 4)], k.trim, 90, Z::flat(6));
                c.hline(px, px + 3, top, Ix::SEAM, 6);
            }
            Stand::Up(&[])
        }
        "coop" => {
            // A hen house: a planked box on legs, a pitched felt roof, a ramp to its pop hole.
            let (x, cw) = (2, w - 4);
            ao(c, x, x + cw - 1, foot, 6);
            for lx in [x + 1, x + cw - 4] {
                post(c, lx, foot - 6, foot - 1, 3, k.trim, 2);
            }
            let body = Rect::new(x, foot - 18, cw, 12);
            planks(c, body, k.body, (cw / 5).max(3), false, false, k.seed, 4);
            c.fill_normal(Rect::new(x + cw / 2 - 2, body.bottom() - 6, 4, 5), k.body.at(Tone::Deep), parts::south(), 5);
            c.line((x + cw / 2 - 1, body.bottom()), (x + cw / 2 + 5, foot), k.trim.at(Tone::Light), 2, 4);
            roof(c, k, x - 2, cw + 4, body.y - 8, 6);
            Stand::Up(&[])
        }
        "woodpile" => {
            // Logs stacked, their cut ends to the viewer: rings, bark round them.
            let rows = 3;
            ao(c, 1, w - 2, foot, 5);
            for r in 0..rows {
                let y = foot - 5 - r * 5;
                let off = if r % 2 == 1 { 3 } else { 0 };
                let mut x = 1 + off + r;
                while x + 5 <= w - 1 - r {
                    c.disc_lit(x + 2, y + 2, 2, k.body, Z::new(3 + r as u8, 4 + r as u8));
                    c.dot(x + 2, y + 2, k.trim.at(Tone::Shade), 5 + r as u8);
                    c.dot(x + 1, y + 1, k.trim.at(Tone::Light), 5 + r as u8);
                    x += 5;
                }
            }
            c.retone(k.body, super::HARD);
            Stand::Up(&[])
        }
        "washing_line" => {
            // Two props and a line between them, the washing pegged out: shirts, a sheet.
            let top = foot - 24;
            ao(c, 1, w - 2, foot, 3);
            for px in [1, w - 3] {
                post(c, px, top, foot - 1, 2, k.trim, 3);
            }
            c.line((2, top + 1), (w - 3, top + 1), Ramp::HairWhite.at(Tone::Base), 1, 5);
            let items = [Ramp::ClothLinen, Ramp::ClothSky, Ramp::ClothRose, Ramp::ClothLinen, Ramp::ClothCream];
            let mut x = 5;
            let mut i = 0;
            while x + 7 < w - 3 {
                let ramp = items[(parts::hash(k.seed, i, 60) % 5) as usize];
                let (iw, ih) = if i % 3 == 2 { (10, 12) } else { (6, 8) };
                c.polygon_cloth(&[(x, top + 2), (x + iw - 1, top + 2), (x + iw, top + 2 + ih), (x - 1, top + 2 + ih)], ramp, 50, Z::flat(5));
                c.hline(x, x + iw - 1, top + 2, ramp.at(Tone::Light), 5);
                c.dot(x + 1, top + 1, Ramp::WoodPale.at(Tone::Light), 6);
                x += iw + 3;
                i += 1;
            }
            Stand::Up(&[])
        }
        "cart" | "minecart" | "trolley" => {
            // A cart on two wheels (a trolley on four small ones, a minecart an iron tub on
            // flanged wheels).
            let (x, cw) = (2, w - 4);
            ao(c, x, x + cw - 1, foot, 5);
            let mine = k.look.shape == "minecart";
            let bed_h = if k.look.shape == "trolley" { 4 } else { 10 };
            let (top, _) = box3(c, x, cw, foot - 5, bed_h, 8, k.body, if mine { None } else { Some((3, k.seed)) }, 3);
            if mine {
                c.retone(k.body, super::HARD);
                band(c, x, x + cw - 1, top.bottom(), k.trim, 5);
                c.fill_normal(Rect::new(top.x + 2, top.y + 2, top.w - 4, top.h - 3), Ramp::Stone.at(Tone::Mid), FLAT, 5);
            }
            let wheels: &[i32] = if k.look.shape == "trolley" { &[x + 2, x + cw - 6] } else { &[x + 3, x + cw - 9] };
            let rad = if k.look.shape == "trolley" { 2 } else { 3 };
            for &wx in wheels {
                c.disc_lit(wx + rad, foot - rad - 1, rad, k.trim, Z::new(3, 5));
                c.dot(wx + rad, foot - rad - 1, k.trim.at(Tone::Deep), 6);
            }
            if k.look.shape == "cart" {
                c.line((x + cw - 1, foot - 10), (w - 1, foot - 14), k.body.at(Tone::Base), 2, 4);
                c.fill_normal(Rect::new(top.x + 3, top.y + 1, top.w - 6, 3), k.accent.at(Tone::Light), FLAT, 6);
            }
            Stand::Tops([(top, lid_height(bed_h + 5)), (Rect::default(), 0)])
        }
        "anvil" => {
            // An anvil on a block of elm.
            let cx = w / 2;
            ao(c, cx - 8, cx + 7, foot, 5);
            box3(c, cx - 5, 10, foot, 7, 3, k.trim, Some((1, k.seed)), 3);
            let top = foot - 15;
            c.polyline_fill(&[(cx - 11, top), (cx + 8, top), (cx + 6, top + 3), (cx + 3, top + 4), (cx + 3, top + 6), (cx - 4, top + 6), (cx - 4, top + 4), (cx - 8, top + 3)], k.body.at(Tone::Base), 5);
            c.hline(cx - 10, cx + 7, top, k.body.at(Tone::High), 6);
            c.hline(cx - 9, cx + 6, top + 1, k.body.at(Tone::Light), 6);
            Stand::Up(&[])
        }
        "bale" => {
            // A bale of hay: a rounded block of straw, bound twice with twine.
            let (x, bw) = (2, w - 4);
            ao(c, x, x + bw - 1, foot, 6);
            let mut m = Canvas::new(c.w(), c.h());
            m.fill_rect(Rect::new(x, foot - 22, bw, 22), Ix::INK, 1);
            c.inflate(&m, k.body, 5, Z::new(2, 9));
            c.retone(k.body, super::HARD);
            c.strokes(Rect::new(x + 1, foot - 21, bw - 2, 20), k.body, crate::canvas::StrokeKind::Grass, 10, k.seed);
            for bx in [x + bw / 3, x + 2 * bw / 3] {
                c.vline(bx, foot - 22, foot - 1, k.trim.at(Tone::Shade), 10);
            }
            Stand::Up(&[])
        }
        "pillar_box" | "phone_box" => {
            // A pillar box (a red drum with a cap and a slot) or a telephone box (red, glazed in
            // small panes, its crown lit).
            let cx = w / 2;
            if k.look.shape == "pillar_box" {
                ao(c, cx - 5, cx + 4, foot, 4);
                c.polygon_lit(&[(cx - 5, foot - 20), (cx + 4, foot - 20), (cx + 4, foot - 1), (cx - 5, foot - 1)], k.body, 110, Z::new(2, 5));
                c.ellipse_lit(Rect::new(cx - 6, foot - 24, 12, 6), k.body, Z::new(5, 7));
                c.retone(k.body, super::HARD);
                c.hline(cx - 3, cx + 2, foot - 16, Ix::SEAM, 6);
                c.fill_rect(Rect::new(cx - 2, foot - 13, 4, 3), Ramp::ClothLinen.at(Tone::Base), 6);
            } else {
                let (x, bw) = (w / 2 - 9, 18);
                ao(c, x, x + bw - 1, foot, 5);
                let (top, front) = box3(c, x, bw, foot, 32, 6, k.body, None, 3);
                c.retone(k.body, super::HARD);
                glass(c, Rect::new(front.x + 3, front.y + 6, front.w - 6, front.h - 12), false, 5);
                for gy in (front.y + 9..front.bottom() - 6).step_by(4) {
                    c.hline(front.x + 3, front.right() - 4, gy, k.body.at(Tone::Base), 6);
                }
                c.vline(front.x + front.w / 2, front.y + 6, front.bottom() - 7, k.body.at(Tone::Base), 6);
                c.fill_rect(Rect::new(front.x + 3, front.y + 1, front.w - 6, 3), Ramp::ClothLinen.at(Tone::Light), 6);
                c.hline(top.x, top.right() - 1, top.y, k.body.at(Tone::High), 6);
            }
            Stand::Up(&[])
        }
        "ticket_window" | "shelter" | "stall" => {
            // A booth with a counter and a window, a platform shelter with a bench under its
            // canopy, or a market stall under a striped awning with its goods out.
            let (x, bw) = (1, w - 2);
            ao(c, x, x + bw - 1, foot, 6);
            let tall = (h - 4).min(40);
            let canopy_y = foot - tall;
            match k.look.shape {
                "ticket_window" => {
                    let (_, front) = box3(c, x, bw, foot, tall - 6, 6, k.body, Some((3, k.seed)), 3);
                    glass(c, Rect::new(front.x + 5, front.y + 5, front.w - 10, 8), false, 5);
                    c.fill_normal(Rect::new(front.x + 3, front.y + 14, front.w - 6, 3), k.trim.at(Tone::Light), FLAT, 6);
                    parts::writing(c, Rect::new(front.x + 4, front.y + 1, front.w - 8, 3), 1, k.accent.at(Tone::Light), k.seed, 6);
                }
                "shelter" => {
                    for px in [x + 1, x + bw - 4] {
                        post(c, px, canopy_y + 4, foot - 1, 3, k.trim, 3);
                    }
                    planks(c, Rect::new(x + 2, canopy_y + 6, bw - 4, tall - 16), k.body, 4, true, false, k.seed, 2);
                    let (seat, _) = box3(c, x + 5, bw - 10, foot - 3, 3, 4, k.trim, Some((2, k.seed ^ 3)), 4);
                    let _ = seat;
                }
                _ => {
                    for px in [x + 1, x + bw - 3] {
                        post(c, px, canopy_y + 4, foot - 1, 2, k.trim, 3);
                    }
                    let (top, _) = box3(c, x + 2, bw - 4, foot, 10, 8, k.trim, Some((2, k.seed)), 3);
                    goods(c, k, top);
                }
            }
            awning(c, k, x - 1, bw + 2, canopy_y, k.look.shape == "stall");
            Stand::Up(&[])
        }
        "pump" => {
            // A village pump: an iron barrel on a stone base, its handle, its spout.
            let cx = w / 2;
            ao(c, cx - 4, cx + 3, foot, 4);
            box3(c, cx - 5, 10, foot, 4, 3, Ramp::Stone, None, 2);
            c.polygon_lit(&[(cx - 3, foot - 22), (cx + 2, foot - 22), (cx + 2, foot - 5), (cx - 3, foot - 5)], k.body, 100, Z::new(3, 5));
            c.retone(k.body, super::HARD);
            c.disc_lit(cx, foot - 23, 2, k.body, Z::flat(6));
            c.line((cx - 3, foot - 20), (cx - 8, foot - 14), k.body.at(Tone::Base), 1, 6);
            c.fill_rect(Rect::new(cx + 2, foot - 13, 3, 2), k.body.at(Tone::Mid), 6);
            Stand::Up(&[])
        }
        "beehive" => {
            // A skep: a dome of coiled straw on a board, its doorway dark.
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 4);
            let mut m = Canvas::new(c.w(), c.h());
            m.ellipse(Rect::new(cx - 6, foot - 16, 12, 16), Ix::INK, 1);
            m.fill_rect(Rect::new(cx - 6, foot - 8, 12, 8), Ix::INK, 1);
            c.inflate(&m, k.body, 4, Z::new(2, 7));
            c.retone(k.body, super::HARD);
            for y in (foot - 14..foot - 1).step_by(3) {
                c.hline(cx - 5, cx + 4, y, k.body.at(Tone::Mid), 8);
            }
            c.fill_rect(Rect::new(cx - 1, foot - 3, 3, 2), Ix::SEAM, 8);
            Stand::Up(&[])
        }
        "scarecrow" => {
            // A scarecrow on its cross: a sacking head under a hat, a coat stuffed with straw.
            let cx = w / 2;
            let top = foot - (h - 2).min(36);
            ao(c, cx - 3, cx + 2, foot, 3);
            post(c, cx - 1, top + 8, foot - 1, 2, k.trim, 3);
            c.line((cx - 10, top + 13), (cx + 9, top + 13), k.trim.at(Tone::Base), 2, 4);
            c.polygon_cloth(&[(cx - 9, top + 12), (cx + 8, top + 12), (cx + 5, top + 26), (cx - 6, top + 26)], k.body, 60, Z::flat(5));
            for (sx, sy) in [(cx - 10, top + 15), (cx + 9, top + 15), (cx - 5, top + 27)] {
                c.fill_rect(Rect::new(sx, sy, 2, 2), Ramp::Reed.at(Tone::Light), 6);
            }
            c.ellipse_lit(Rect::new(cx - 4, top + 3, 8, 9), Ramp::ClothCream, Z::flat(6));
            c.dot(cx - 2, top + 7, Ix::SEAM, 7);
            c.dot(cx + 1, top + 7, Ix::SEAM, 7);
            c.ellipse_lit(Rect::new(cx - 7, top + 2, 14, 3), k.accent, Z::flat(8));
            c.ellipse_lit(Rect::new(cx - 4, top - 2, 8, 5), k.accent, Z::flat(8));
            Stand::Up(&[])
        }
        "tent" => {
            // A ridge tent: two pitched faces of canvas, the dark of its door, a guy line.
            let (x, tw) = (1, w - 2);
            ao(c, x, x + tw - 1, foot, 5);
            let top = foot - (h - 3).min(24);
            c.polygon_lit(&[(x + tw / 2, top), (x + tw - 1, foot - 1), (x, foot - 1)], k.body, 120, Z::new(2, 8));
            c.retone(k.body, super::HARD);
            c.polyline_fill(&[(x + tw / 2, top + 6), (x + tw / 2 + 4, foot - 1), (x + tw / 2 - 4, foot - 1)], Ix::SEAM, 9);
            c.line((x + tw / 2, top), (x + tw - 1, top - 2), k.trim.at(Tone::Base), 1, 9);
            Stand::Up(&[])
        }
        "footbridge" => {
            // A footbridge over a ditch, broken: planks across two stringers, the middle ones
            // gone and one hanging, the handrail snapped.
            let top = foot - 20;
            ao(c, 1, w - 2, foot, 5);
            for (i, x) in (2..w - 4).step_by(5).enumerate() {
                if i == 3 || i == 4 {
                    continue;
                }
                planks(c, Rect::new(x, top + 6, 4, 12), k.body, 1, false, true, k.seed ^ i as u32, 3);
            }
            c.line((17, top + 8), (20, top + 18), k.body.at(Tone::Shade), 3, 2);
            for y in [top + 6, top + 16] {
                c.hline(1, 14, y, k.trim.at(Tone::Base), 4);
                c.hline(w - 15, w - 2, y, k.trim.at(Tone::Base), 4);
            }
            for x in [2, 13, w - 14, w - 3] {
                post(c, x, top, top + 7, 2, k.trim, 5);
            }
            c.line((3, top), (13, top + 2), k.trim.at(Tone::Light), 1, 6);
            c.line((w - 13, top), (w - 3, top), k.trim.at(Tone::Light), 1, 6);
            Stand::Flat(8)
        }
        "planter" => {
            // A dry planter: a stone trough of cracked earth and dead stems.
            let (x, pw) = (2, w - 4);
            ao(c, x, x + pw - 1, foot, 5);
            let (top, _) = box3(c, x, pw, foot, 10, 10, k.body, None, 3);
            c.retone(k.body, super::HARD);
            c.fill_normal(Rect::new(top.x + 2, top.y + 2, top.w - 4, top.h - 3), Ramp::Bark.at(Tone::Mid), FLAT, 5);
            c.line((top.x + 5, top.y + 3), (top.x + 9, top.y + 6), Ramp::Bark.at(Tone::Deep), 1, 5);
            for (sx, sy) in [(top.x + 6, top.y + 4), (top.x + 14, top.y + 5), (top.x + 20, top.y + 3)] {
                c.line((sx, sy), (sx + 1, sy - 9), Ramp::Reed.at(Tone::Mid), 1, 6);
                c.dot(sx + 2, sy - 8, Ramp::Reed.at(Tone::Light), 6);
            }
            Stand::Tops([(top, lid_height(10)), (Rect::default(), 0)])
        }
        "bell" => {
            // The School's bell in its timber frame: a bronze bell lit down one flank, its
            // clapper under it, the headstock on the beam.
            let top = foot - (h - 2).min(40);
            ao(c, 1, w - 2, foot, 6);
            for x in [2, w - 6] {
                post(c, x, top, foot - 1, 4, k.trim, 3);
            }
            c.fill_normal(Rect::new(2, top, w - 4, 4), k.trim.at(Tone::Base), parts::south(), 4);
            c.hline(2, w - 3, top, k.trim.at(Tone::Light), 4);
            let cx = w / 2;
            c.polygon_lit(&[(cx - 4, top + 5), (cx + 3, top + 5), (cx + 6, top + 18), (cx + 8, top + 21), (cx - 9, top + 21), (cx - 7, top + 18)], k.body, 110, Z::new(6, 9));
            c.retone(k.body, super::HARD);
            c.hline(cx - 9, cx + 8, top + 21, k.body.at(Tone::Shade), 9);
            c.disc_lit(cx - 1, top + 23, 2, k.body, Z::flat(9));
            Stand::Up(&[])
        }
        "rope" | "pole" => {
            // A bell rope from the ceiling with its striped sally, or a plain pole in the ground.
            let cx = w / 2;
            let top = foot - (h - 2).min(40);
            ao(c, cx - 3, cx + 2, foot, 3);
            if k.look.shape == "rope" {
                c.line((cx - 1, top), (cx - 1, foot - 3), k.body.at(Tone::Base), 2, 4);
                c.vline(cx - 1, top, foot - 3, k.body.at(Tone::Light), 4);
                c.polygon_lit(&[(cx - 3, foot - 18), (cx + 2, foot - 18), (cx + 2, foot - 8), (cx - 3, foot - 8)], k.accent, 90, Z::flat(5));
                for y in [foot - 16, foot - 12] {
                    c.hline(cx - 3, cx + 2, y, Ramp::ClothLinen.at(Tone::Base), 5);
                }
            } else {
                post(c, cx - 1, top, foot - 1, 3, k.body, 4);
            }
            Stand::Up(&[])
        }
        "cage" => {
            // Cage traps: two wire boxes stacked, a door propped on one.
            let cx = w / 2;
            ao(c, cx - 7, cx + 6, foot, 4);
            for (y, x0) in [(foot - 7, cx - 7), (foot - 13, cx - 5)] {
                let r = Rect::new(x0, y, 12, 6);
                c.rect_bevel(r, k.body, 1, Z::new(3, 4));
                c.fill_rect(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), Ramp::ClothBlack.at(Tone::Deep), 4);
                for x in (r.x + 2..r.right() - 1).step_by(2) {
                    c.vline(x, r.y + 1, r.bottom() - 2, k.body.at(Tone::Light), 5);
                }
            }
            Stand::Up(&[])
        }
        "armour" => {
            // A suit of armour on its stand (or on its rail): a helm with a slit, a breastplate
            // lit on its left, tassets, greaves, sabatons on a plinth.
            let edge_on = k.fh > k.fw;
            let cx = w / 2;
            let top = foot - (h - 2).min(42);
            ao(c, cx - 7, cx + 6, foot, 5);
            if !edge_on && k.fw >= 3 {
                c.fill_normal(Rect::new(1, foot - 6, w - 2, 2), k.trim.at(Tone::Base), parts::south(), 2);
                c.hline(1, w - 2, foot - 6, k.trim.at(Tone::Light), 2);
            }
            c.rect_bevel(Rect::new(cx - 6, foot - 4, 12, 4), k.trim, 1, Z::new(2, 3));
            for x in [cx - 4, cx + 1] {
                c.polygon_lit(&[(x, foot - 16), (x + 3, foot - 16), (x + 3, foot - 4), (x, foot - 4)], k.body, 90, Z::flat(4));
            }
            c.polygon_lit(&[(cx - 6, top + 11), (cx + 5, top + 11), (cx + 4, top + 22), (cx + 3, foot - 15), (cx - 4, foot - 15), (cx - 5, top + 22)], k.body, 110, Z::new(5, 8));
            for x in [cx - 9, cx + 6] {
                c.polygon_lit(&[(x, top + 11), (x + 3, top + 11), (x + 3, top + 22), (x, top + 22)], k.body, 90, Z::flat(7));
                c.disc_lit(x + 1, top + 11, 2, k.body, Z::flat(9));
            }
            c.ellipse_lit(Rect::new(cx - 4, top, 8, 11), k.body, Z::new(8, 10));
            c.retone(k.body, super::HARD);
            c.hline(cx - 3, cx + 2, top + 5, Ix::SEAM, 11);
            c.fill_rect(Rect::new(cx - 1, top - 3, 2, 3), k.accent.at(Tone::Base), 11);
            Stand::Up(&[])
        }
        "figure" => {
            // A waxwork on its plinth: a figure in a grey coat, too still, its face too pale.
            let cx = w / 2;
            let top = foot - (h - 2).min(40);
            ao(c, cx - 8, cx + 7, foot, 5);
            box3(c, cx - 9, 18, foot, 5, 6, k.trim, None, 2);
            c.polygon_cloth(&[(cx - 5, top + 10), (cx + 4, top + 10), (cx + 6, foot - 8), (cx - 7, foot - 8)], k.accent, 80, Z::new(4, 6));
            c.ellipse_lit(Rect::new(cx - 4, top, 8, 10), k.body, Z::new(6, 8));
            c.dot(cx - 2, top + 5, Ix::SEAM, 9);
            c.dot(cx + 1, top + 5, Ix::SEAM, 9);
            c.retone(k.body, super::HARD);
            Stand::Up(&[])
        }
        "mounted_fox" => {
            // The fox, mounted: the museum's fox on a plinth with a brass plate.
            ao(c, 2, w - 3, foot, 5);
            let (top, front) = box3(c, 3, w - 6, foot, 8, 6, k.trim, Some((2, k.seed)), 2);
            c.fill_rect(Rect::new(front.x + front.w / 2 - 3, front.y + 3, 6, 2), Ramp::Brass.at(Tone::Light), 3);
            if let Some((id, jane_data::Look::Creature(fox))) = crate::looks::find("museum_fox") {
                let _ = id;
                let fox = jane_data::CreatureLook { emits: &[], ..*fox };
                if let Ok(set) = crate::creature::render(&fox, crate::creature::seed("museum_fox"), false) {
                    if let Some(f) = set.frame(crate::sprite::FrameId::Side) {
                        let mut f = f.clone();
                        f.remap(|ix| ix);
                        c.stamp(&f, (w - f.w()) / 2, top.y + top.h / 2 - f.h() + 4);
                    }
                }
            }
            Stand::Up(&[])
        }
        "hoist" => {
            // A mine's hoist frame: two legs, a head beam, a wheel and a hanging chain; broken,
            // the wheel off and the chain slack on the ground.
            let top = foot - (h - 2).min(40);
            ao(c, 1, w - 2, foot, 5);
            for (x0, x1) in [(2, 6), (w - 3, w - 7)] {
                c.line((x0, foot - 1), (x1, top + 3), k.body.at(Tone::Base), 3, 4);
            }
            c.fill_normal(Rect::new(3, top, w - 6, 4), k.body.at(Tone::Base), parts::south(), 5);
            c.hline(3, w - 4, top, k.body.at(Tone::Light), 5);
            c.disc_lit(w / 2 + 4, foot - 5, 4, k.trim, Z::flat(4));
            c.dot(w / 2 + 4, foot - 5, k.trim.at(Tone::Deep), 5);
            let _ = normal;
            Stand::Up(&[])
        }
        _ => return None,
    })
}

/// A pitched roof over `x..x + w` whose ridge is on row `top`: slates in the accent's ramp.
fn roof(c: &mut Canvas, k: &Kit, x: i32, w: i32, top: i32, z: u8) {
    let ramp = k.accent;
    c.polyline_fill(&[(x + w / 2 - 1, top), (x + w / 2, top), (x + w - 1, top + 7), (x, top + 7)], ramp.at(Tone::Base), z);
    for y in (top + 2..top + 8).step_by(2) {
        c.hline(x + 1, x + w - 2, y, ramp.at(Tone::Mid), z);
    }
    c.line((x + w / 2 - 1, top), (x, top + 7), ramp.at(Tone::Light), 1, z);
    c.hline(x, x + w - 1, top + 7, ramp.at(Tone::Shade), z);
}

/// An awning over a booth: canvas in the accent's stripes (or plain), its scalloped edge.
fn awning(c: &mut Canvas, k: &Kit, x: i32, w: i32, y: i32, stripes: bool) {
    let r = Rect::new(x, y, w, 7);
    c.fill_normal(r, k.accent.at(Tone::Base), normal(0, -50), 8);
    if stripes {
        for sx in (x..x + w).step_by(6) {
            c.fill_rect(Rect::new(sx, y, 3, 7), Ramp::ClothLinen.at(Tone::Base), 8);
        }
    }
    c.hline(x, x + w - 1, y, k.accent.at(Tone::Light), 8);
    for sx in (x..x + w).step_by(3) {
        c.dot(sx + 1, y + 7, k.accent.at(Tone::Shade), 8);
    }
}

/// A stall's goods on its counter: bunches, loaves or packets, by the seed.
fn goods(c: &mut Canvas, k: &Kit, top: Rect) {
    let ramps = [k.body, Ramp::Leaf, Ramp::ClothMustard, Ramp::WoodPale];
    let mut x = top.x + 2;
    let mut i = 0;
    while x + 4 < top.right() {
        let r = ramps[(parts::hash(k.seed, i, 90) % 4) as usize];
        c.ellipse_lit(Rect::new(x, top.y + 1, 5, 4), r, Z::flat(7));
        x += 6;
        i += 1;
    }
}
