//! The dungeons' set pieces (DUNGEONS.md §2.10, ART.md §2.3): the larger things a room is
//! furnished with, drawn under whichever family a look names when that family has no such shape
//! of its own. `carpet` (a patterned rug with a border, a medallion and fringes), `crate_stack`
//! and `crate_pile`, `junk_heap` (rubble, scrap, bones, brick or broken furniture by its
//! materials), `shelf_fallen` and `book_spill`, `tool_rack`, `candlestand`, `tomb_open`,
//! `lathe`, `parts_bin`, `stanchions`, `bust`, `blackboard`, `globe`, `lab_bench`, `stool`,
//! `pipe_stack`, `sandbags`, `fallen_trunk`, `mushrooms`.
//!
//! The same rules as the rest of the kit: clusters of two px or more, a lit top edge and a shaded
//! right, materials in their own few tones, every routine standing on its foot row and ending in
//! [`super::finish`]'s outline and heights.

use jane_core::grid::Rect;

use super::parts::{self, ao, band, box3, lid_height, planks, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, _state: State) -> Option<Stand> {
    let (w, foot) = (k.w, k.foot());
    Some(match k.look.shape {
        "carpet" => carpet(c, k),
        "crate_stack" => {
            // Two crates, the lower one wide and the upper one smaller and set back, a sack
            // slumped against their foot.
            let (top, _) = crate_box(c, k, 1, w - 2, foot, 14, 10, k.seed);
            let uw = w - 10;
            crate_box(c, k, 5 + (k.seed % 3) as i32, uw, top.bottom() - 2, 12, 8, k.seed ^ 5);
            Stand::Up(&[])
        }
        "crate_pile" => {
            // Two crates side by side, one lower than the other, a third across them, a plank
            // leant against the pile.
            let half = w / 2;
            let (ta, _) = crate_box(c, k, 1, half - 1, foot, 14, 10, k.seed);
            let (tb, _) = crate_box(c, k, half + 1, half - 3, foot, 11, 9, k.seed ^ 3);
            let y = ta.bottom().max(tb.bottom()) - 3;
            crate_box(c, k, half / 2 + 2, half, y, 12, 8, k.seed ^ 9);
            let x = w - 5;
            c.polygon_lit(
                &[(x - 7, foot - 26), (x - 4, foot - 27), (x + 2, foot), (x - 1, foot)],
                k.body,
                60,
                Z::new(2, 8),
            );
            c.retone(k.body, super::HARD);
            Stand::Up(&[])
        }
        "junk_heap" => junk_heap(c, k),
        "shelf_fallen" => shelf_fallen(c, k),
        "book_spill" => {
            // Books fallen open and shut across the floor, a few loose pages among them.
            ao(c, 2, w - 3, foot, 3);
            let n = (w / 7).max(3);
            for i in 0..n {
                let hh = parts::hash(k.seed, i, 41);
                let x = 1 + (i * (w - 8)) / n + (hh % 3) as i32;
                let y = foot - 10 + ((hh >> 4) % 6) as i32;
                book(c, x, y, BOOKS[(hh >> 8) as usize % BOOKS.len()], hh & 0x1000 == 0, k.seed ^ i as u32);
            }
            // A page or two, loose.
            for i in 0..2 {
                let hh = parts::hash(k.seed, i, 43);
                let (x, y) = (3 + (hh % (w - 10).max(1) as u32) as i32, foot - 4 - (hh >> 8) as i32 % 3);
                c.fill_normal(Rect::new(x, y, 5, 3), Ramp::ClothLinen.at(Tone::Light), FLAT, 2);
                c.hline(x + 1, x + 3, y + 1, Ramp::Slate.at(Tone::Light), 2);
            }
            Stand::Flat(3)
        }
        "tool_rack" => tool_rack(c, k),
        "candlestand" => {
            // An iron pricket stand: three splayed feet, a shaft, a drip pan, three candles of
            // three heights, wax run down them and pooled on the floor. Unlit.
            let cx = w / 2;
            c.ellipse(Rect::new(cx - 5, foot - 3, 10, 3), Ramp::ClothCream.at(Tone::Mid), 1);
            ao(c, cx - 5, cx + 4, foot, 4);
            for (dx, dy) in [(-6, 0), (5, 0), (0, -2)] {
                c.line((cx, foot - 6), (cx + dx, foot + dy), k.body.at(Tone::Base), 2, 2);
            }
            post(c, cx - 1, foot - 25, foot - 5, 3, k.body, 3);
            band(c, cx - 3, cx + 3, foot - 16, k.body, 4);
            let pan = Rect::new(cx - 7, foot - 27, 14, 3);
            c.fill_normal(pan, k.body.at(Tone::Base), FLAT, 5);
            c.hline(pan.x, pan.right() - 1, pan.y, k.body.at(Tone::Light), 5);
            c.hline(pan.x + 1, pan.right() - 2, pan.bottom(), k.body.at(Tone::Shade), 5);
            for (i, (dx, hgt)) in [(-5, 7), (-1, 11), (3, 5)].into_iter().enumerate() {
                let x = cx + dx;
                let r = Rect::new(x, pan.y - hgt, 3, hgt);
                c.fill_normal(r, k.accent.at(Tone::Light), parts::south(), 6);
                c.vline(x + 2, r.y + 1, r.bottom() - 1, k.accent.at(Tone::Base), 6);
                c.dot(x, r.y, k.accent.at(Tone::High), 6);
                c.dot(x + 1, r.y - 1, Ix::SEAM, 6);
                // A run of wax down one side.
                if i != 1 {
                    c.vline(x + if i == 0 { 0 } else { 2 }, r.y + 2, r.y + 2 + hgt / 2, k.accent.at(Tone::High), 7);
                }
            }
            Stand::Up(&[])
        }
        "tomb_open" => tomb_open(c, k),
        "lathe" => lathe(c, k),
        "parts_bin" => {
            // A low bin of boards, full of cogs, bolts and a spanner.
            let (x, bw) = (2, w - 4);
            ao(c, x, x + bw - 1, foot, 5);
            let (top, front) = box3(c, x, bw, foot, 9, 7, k.body, Some((2, k.seed)), 3);
            let inside = Rect::new(top.x + 2, top.y + 1, top.w - 4, top.h - 1);
            c.fill_normal(inside, k.body.at(Tone::Deep), FLAT, 4);
            for i in 0..(bw / 4) {
                let hh = parts::hash(k.seed, i, 61);
                let (px, py) = (inside.x + 1 + (hh % (inside.w - 3) as u32) as i32, inside.y + (hh >> 8) as i32 % 3);
                if hh & 0x3000 == 0 {
                    cog(c, px, py, k.trim, 6);
                } else {
                    c.fill_rect(Rect::new(px, py, 2, 2), k.trim.at(Tone::Light), 6);
                    c.dot(px + 1, py + 1, k.trim.at(Tone::Shade), 6);
                }
            }
            c.line((inside.right() - 7, inside.y), (inside.right() - 1, inside.y - 3), k.trim.at(Tone::Light), 2, 7);
            band(c, front.x, front.right() - 1, front.y + 3, k.trim, 5);
            Stand::Up(&[])
        }
        "stanchions" => {
            // Two brass posts on weighted feet with a velvet rope slung between them: the line
            // the public keeps behind.
            let (x0, x1) = (4, w - 7);
            for x in [x0, x1] {
                ao(c, x - 3, x + 5, foot, 4);
                c.ellipse_lit(Rect::new(x - 3, foot - 4, 9, 4), k.trim, Z::flat(2));
                post(c, x, foot - 22, foot - 3, 3, k.trim, 3);
                c.ellipse_lit(Rect::new(x - 1, foot - 26, 5, 5), k.trim, Z::flat(4));
            }
            c.retone(k.trim, super::HARD);
            // The rope's sag, a curve two px thick.
            let (ya, span) = (foot - 20, x1 - x0 - 3);
            let mut prev = (x0 + 3, ya);
            for i in 1..=span {
                let t = i * 2 - span;
                let sag = 7 - (7 * t * t) / (span * span).max(1);
                let p = (x0 + 3 + i, ya + sag);
                c.line(prev, p, k.body.at(Tone::Base), 2, 5);
                c.dot(p.0, p.1, k.body.at(Tone::Light), 5);
                prev = p;
            }
            Stand::Up(&[])
        }
        "bust" => {
            // A bust on a fluted pedestal: the column, its cap, a head and shoulders in marble.
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 4);
            c.fill_normal(Rect::new(cx - 6, foot - 3, 12, 3), k.trim.at(Tone::Base), parts::south(), 2);
            c.hline(cx - 6, cx + 5, foot - 3, k.trim.at(Tone::Light), 2);
            c.polygon_lit(
                &[(cx - 4, foot - 20), (cx + 3, foot - 20), (cx + 3, foot - 3), (cx - 4, foot - 3)],
                k.trim,
                100,
                Z::new(2, 4),
            );
            c.retone(k.trim, super::HARD);
            for fx in [cx - 2, cx, cx + 2] {
                c.vline(fx, foot - 18, foot - 5, k.trim.at(Tone::Shade), 4);
            }
            let cap = Rect::new(cx - 6, foot - 23, 12, 3);
            c.fill_normal(cap, k.trim.at(Tone::Light), FLAT, 5);
            c.hline(cap.x, cap.right() - 1, cap.bottom() - 1, k.trim.at(Tone::Shade), 5);
            // Shoulders, a neck, a head turned a little to the light.
            c.polygon_lit(
                &[(cx - 7, foot - 24), (cx + 6, foot - 24), (cx + 4, foot - 30), (cx - 5, foot - 30)],
                k.body,
                90,
                Z::new(6, 7),
            );
            c.fill_rect(Rect::new(cx - 2, foot - 33, 4, 3), k.body.at(Tone::Base), 7);
            c.ellipse_lit(Rect::new(cx - 4, foot - 41, 8, 9), k.body, Z::flat(8));
            c.retone(k.body, super::HARD);
            c.dot(cx - 2, foot - 37, k.body.at(Tone::Shade), 9);
            c.dot(cx + 1, foot - 37, k.body.at(Tone::Shade), 9);
            c.vline(cx, foot - 37, foot - 35, k.body.at(Tone::Light), 9);
            c.hline(cx - 3, cx + 2, foot - 41, k.body.at(Tone::Light), 9);
            Stand::Up(&[])
        }
        "blackboard" => {
            // A blackboard on its easel: an A of two legs, the board in its frame, a sum and a
            // diagram in chalk, the chalk and the duster on the ledge.
            let (x, bw) = (5, w - 10);
            let (by, bh) = (foot - 38, 22);
            ao(c, 3, w - 4, foot, 4);
            for (a, b) in [((x + 2, by - 4), (x - 2, foot)), ((x + bw - 3, by - 4), (x + bw + 1, foot))] {
                c.line(a, b, k.trim.at(Tone::Base), 2, 2);
            }
            c.line((x + 4, by - 4), (x + bw / 2, foot - 2), k.trim.at(Tone::Shade), 2, 1);
            let board = Rect::new(x, by, bw, bh);
            c.rect_bevel(board, k.trim, 2, Z::new(3, 4));
            let slate = Rect::new(x + 2, by + 2, bw - 4, bh - 4);
            c.fill_normal(slate, k.body.at(Tone::Shade), parts::south(), 4);
            c.fill_rect(Rect::new(slate.x, slate.y, slate.w, 1), k.body.at(Tone::Base), 4);
            let chalk = Ramp::ClothLinen.at(Tone::Base);
            parts::writing(c, Rect::new(slate.x + 2, slate.y + 2, slate.w / 2, slate.h - 4), 4, chalk, k.seed, 5);
            let (ox, oy) = (slate.x + slate.w * 3 / 4, slate.y + slate.h / 2);
            c.ellipse(Rect::new(ox - 5, oy - 5, 11, 10), chalk, 5);
            c.ellipse(Rect::new(ox - 4, oy - 4, 9, 8), k.body.at(Tone::Shade), 5);
            c.line((ox - 5, oy + 4), (ox + 5, oy - 4), chalk, 1, 5);
            let ledge = Rect::new(x - 1, by + bh, bw + 2, 2);
            c.fill_normal(ledge, k.trim.at(Tone::Light), FLAT, 5);
            c.fill_rect(Rect::new(x + 4, by + bh - 1, 3, 1), Ramp::ClothLinen.at(Tone::High), 6);
            c.fill_rect(Rect::new(x + bw - 9, by + bh - 2, 5, 2), Ramp::ClothGrey.at(Tone::Base), 6);
            Stand::Up(&[])
        }
        "globe" => {
            // A globe on a turned stand: the sea, the land in blots, a brass meridian round it.
            let cx = w / 2;
            ao(c, cx - 5, cx + 4, foot, 4);
            for (dx, dy) in [(-5, 0), (4, 0), (0, -2)] {
                c.line((cx, foot - 7), (cx + dx, foot + dy), k.trim.at(Tone::Base), 2, 2);
            }
            post(c, cx - 1, foot - 12, foot - 6, 3, k.trim, 3);
            let g = Rect::new(cx - 7, foot - 27, 14, 14);
            // The brass ring round it, drawn first so the globe stands inside it.
            c.ellipse(Rect::new(g.x - 1, g.y - 2, g.w + 2, g.h + 3), k.trim.at(Tone::Light), 4);
            c.ellipse(Rect::new(g.x, g.y - 1, g.w, g.h + 1), k.trim.at(Tone::Shade), 4);
            c.ellipse_lit(g, k.body, Z::flat(5));
            c.retone(k.body, super::HARD);
            for i in 0..4 {
                let hh = parts::hash(k.seed, i, 71);
                let (lx, ly) = (g.x + 3 + (hh % 7) as i32, g.y + 3 + (hh >> 4) as i32 % 7);
                c.dye_ellipse(Rect::new(lx, ly, 3 + (hh >> 8) as i32 % 3, 2 + (hh >> 12) as i32 % 2), k.body, k.accent);
            }
            Stand::Up(&[])
        }
        "lab_bench" => {
            // A laboratory bench: a dark top over cupboards, a retort on its stand, a flask of
            // something green, a rack of tubes.
            let (x, tw) = (2, w - 4);
            ao(c, x, x + tw - 1, foot, 6);
            let depth = (k.fh * 16 - 14).clamp(8, 18);
            let (top, front) = box3(c, x, tw, foot, 12, depth, k.body, None, 3);
            c.retone(k.body, super::HARD);
            for dx in [front.x + 2, front.x + tw / 2 + 1] {
                c.fill_normal(
                    Rect::new(dx, front.y + 2, tw / 2 - 4, front.h - 4),
                    k.trim.at(Tone::Base),
                    parts::south(),
                    4,
                );
                c.hline(dx, dx + tw / 2 - 5, front.y + 2, k.trim.at(Tone::Light), 4);
                c.dot(dx + tw / 4 - 2, front.y + 5, Ramp::Brass.at(Tone::Light), 5);
            }
            // The stand and the retort.
            let sx = top.x + 5;
            c.fill_rect(Rect::new(sx - 2, top.bottom() - 3, 6, 2), Ramp::Iron.at(Tone::Base), 6);
            c.vline(sx, top.bottom() - 20, top.bottom() - 3, Ramp::Iron.at(Tone::Light), 6);
            c.hline(sx, sx + 5, top.bottom() - 14, Ramp::Iron.at(Tone::Base), 6);
            let f = Rect::new(sx + 3, top.bottom() - 14, 7, 6);
            c.ellipse_lit(f, Ramp::Glass, Z::flat(7));
            c.line((f.right() - 1, f.y + 2), (f.right() + 5, f.y + 6), Ramp::Glass.at(Tone::Light), 1, 7);
            // The flask.
            let fx = top.x + tw / 2 - 2;
            c.ellipse_lit(Rect::new(fx - 4, top.bottom() - 10, 9, 8), k.accent, Z::flat(7));
            c.fill_rect(Rect::new(fx - 1, top.bottom() - 14, 3, 4), Ramp::Glass.at(Tone::Light), 7);
            c.dot(fx - 2, top.bottom() - 8, Ramp::Glass.at(Tone::High), 8);
            // The rack of tubes.
            let rx = top.right() - 14;
            c.fill_rect(Rect::new(rx, top.bottom() - 5, 11, 3), k.trim.at(Tone::Light), 6);
            for (i, t) in [Ramp::ClothRed, Ramp::ClothBlue, Ramp::Glass, k.accent].into_iter().enumerate() {
                let tx = rx + 1 + i as i32 * 3;
                c.fill_rect(Rect::new(tx, top.bottom() - 11, 2, 7), Ramp::Glass.at(Tone::Light), 7);
                c.fill_rect(Rect::new(tx, top.bottom() - 7, 2, 3), t.at(Tone::Base), 7);
            }
            Stand::Tops([
                (Rect::new(top.x, top.y, 3, top.h), lid_height(12)),
                (Rect::new(top.x, top.bottom() - 2, top.w, 2), lid_height(12)),
            ])
        }
        "stool" => {
            // A three-legged stool, its round seat worn pale in the middle.
            let cx = w / 2;
            ao(c, cx - 5, cx + 4, foot, 4);
            for (a, b) in [
                ((cx - 3, foot - 8), (cx - 6, foot)),
                ((cx + 2, foot - 8), (cx + 5, foot)),
                ((cx, foot - 8), (cx, foot - 2)),
            ] {
                c.line(a, b, k.body.at(Tone::Shade), 2, 2);
            }
            let seat = Rect::new(cx - 6, foot - 12, 12, 5);
            c.ellipse_lit(seat, k.body, Z::new(4, 5));
            c.retone(k.body, super::HARD);
            c.hline(cx - 2, cx + 1, foot - 10, k.body.at(Tone::Light), 6);
            Stand::Up(&[])
        }
        "pipe_stack" => {
            // Lengths of pipe racked on two chocks, three, two and one, their ends open to us.
            let len = w - 6;
            ao(c, 1, w - 2, foot, 4);
            for cx in [6, w - 9] {
                c.fill_normal(Rect::new(cx, foot - 3, 4, 3), Ramp::WoodDark.at(Tone::Base), parts::south(), 2);
            }
            for (row, n) in [(0, 3), (1, 2), (2, 1)] {
                for i in 0..n {
                    let d = 6;
                    let x0 = 2 + row * 3 + i * (d + 1) / 3;
                    let y = foot - 3 - (row + 1) * (d - 1) + i % 2;
                    pipe_length(c, x0 + i * 2, y, len - row * 4 - i * 2, d, k.body, 3 + (row * 2 + i) as u8);
                }
            }
            Stand::Up(&[])
        }
        "sandbags" => {
            // A low wall of sandbags, a course of four and a course of three over the joints.
            ao(c, 1, w - 2, foot, 5);
            let bw = (w - 2) / 4;
            for (row, n, off) in [(0, 4, 1), (1, 3, 1 + bw / 2)] {
                for i in 0..n {
                    let x = off + i * bw;
                    let y = foot - 7 - row * 6;
                    let mut m = Canvas::new(c.w(), c.h());
                    m.ellipse(Rect::new(x, y, bw + 1, 8), Ix::INK, 1);
                    c.inflate(&m, k.body, 3, Z::new(2 + row as u8 * 3, 5 + row as u8 * 3));
                    c.retone(k.body, super::HARD);
                    c.vline(x + 2, y + 2, y + 5, k.trim.at(Tone::Base), 6 + row as u8 * 3);
                }
            }
            Stand::Up(&[])
        }
        "fallen_trunk" => {
            // A tree down across the glade: its root plate torn up at the left, the trunk lying
            // along the ground, two broken limbs, moss on its back, a sawn end at the right
            // showing its rings, bracket fungus on its side.
            let (x0, x1) = (10, w - 3);
            let d = 13;
            let y = foot - d - 2;
            ao(c, x0, x1, foot, 6);
            // Limbs behind the trunk.
            for (bx, dx, dy) in [(x0 + 20, 6, -10), (x0 + 44, -5, -9)] {
                c.line((bx, y + 3), (bx + dx, y + dy), k.body.at(Tone::Shade), 3, 4);
                c.line((bx, y + 2), (bx + dx, y + dy - 1), k.body.at(Tone::Base), 1, 5);
            }
            let mut m = Canvas::new(c.w(), c.h());
            m.fill_rect(Rect::new(x0, y, x1 - x0, d), Ix::INK, 1);
            c.inflate(&m, k.body, d / 2, Z::new(3, 9));
            c.retone(k.body, super::HARD);
            // Bark: furrows along it.
            for i in 0..(x1 - x0) / 5 {
                let hh = parts::hash(k.seed, i, 91);
                let bx = x0 + 3 + i * 5 + (hh % 3) as i32;
                let by = y + 3 + (hh >> 4) as i32 % (d - 6);
                c.hline(bx, bx + 3, by, k.body.at(Tone::Deep), 10);
            }
            // Moss along its back.
            for i in 0..6 {
                let hh = parts::hash(k.seed, i, 93);
                let mx = x0 + 6 + (hh % (x1 - x0 - 14) as u32) as i32;
                c.fill_rect(Rect::new(mx, y + 1, 4 + (hh >> 8) as i32 % 4, 2), k.accent.at(Tone::Base), 11);
                c.dot(mx + 1, y + 1, k.accent.at(Tone::Light), 11);
            }
            // The sawn end: rings.
            let end = Rect::new(x1 - 5, y, 7, d);
            c.ellipse(end, k.trim.at(Tone::Light), 11);
            c.ellipse(Rect::new(end.x + 1, end.y + 2, 5, d - 4), k.trim.at(Tone::Base), 11);
            c.ellipse(Rect::new(end.x + 2, end.y + 4, 3, d - 8), k.trim.at(Tone::Mid), 11);
            c.dot(end.x + 3, end.y + d / 2, k.body.at(Tone::Shade), 11);
            // The root plate, torn up with earth in it, roots trailing.
            let mut rp = Canvas::new(c.w(), c.h());
            rp.ellipse(Rect::new(1, y - 9, 14, d + 12), Ix::INK, 1);
            c.inflate(&rp, Ramp::Soil, 3, Z::new(4, 14));
            c.retone(Ramp::Soil, super::HARD);
            for (a, b) in [
                ((4, y - 6), (1, y - 12)),
                ((10, y - 7), (13, y - 13)),
                ((3, y + d), (0, foot - 1)),
                ((12, y + 2), (16, y - 4)),
            ] {
                c.line(a, b, k.body.at(Tone::Base), 2, 12);
            }
            // Bracket fungus on its flank.
            for fx in [x0 + 30, x0 + 36] {
                c.ellipse_lit(Rect::new(fx, y + d - 5, 6, 3), Ramp::ClothOchre, Z::flat(10));
            }
            Stand::Up(&[])
        }
        "mushrooms" => {
            // Toadstools in a clump: pale stems, caps of the accent, a few spots.
            for i in 0..5 {
                let hh = parts::hash(k.seed, i, 81);
                let x = 2 + (hh % (w as u32 - 8)) as i32;
                let hgt = 3 + (hh >> 4) as i32 % 5;
                let y = foot - 2 - (hh >> 8) as i32 % 5;
                let cw = 4 + (hh >> 12) as i32 % 4;
                c.fill_rect(Rect::new(x + cw / 2 - 1, y - hgt, 2, hgt), Ramp::Bone.at(Tone::Light), 2);
                c.ellipse_lit(Rect::new(x, y - hgt - 3, cw, 4), k.accent, Z::flat(3));
                if cw > 5 {
                    c.dot(x + 1, y - hgt - 2, Ramp::Bone.at(Tone::High), 4);
                }
            }
            Stand::Flat(6)
        }
        _ => return None,
    })
}

/// Spines of books in the cloths the library keeps.
const BOOKS: [Ramp; 6] =
    [Ramp::ClothRed, Ramp::ClothGreen, Ramp::ClothNavy, Ramp::ClothMustard, Ramp::Leather, Ramp::ClothBrick];

/// A book lying on the floor at `(x, y)`: shut (a cover and its page edges) or open (two pages).
fn book(c: &mut Canvas, x: i32, y: i32, cover: Ramp, open: bool, seed: u32) {
    if open {
        let r = Rect::new(x, y, 9, 5);
        c.fill_normal(Rect::new(x - 1, y + 1, 11, 5), cover.at(Tone::Shade), FLAT, 2);
        c.fill_normal(r, Ramp::ClothLinen.at(Tone::Light), FLAT, 3);
        c.vline(x + 4, y, y + 4, Ramp::ClothLinen.at(Tone::Mid), 3);
        parts::writing(c, Rect::new(x + 1, y + 1, 3, 3), 2, Ramp::Slate.at(Tone::Light), seed, 3);
        parts::writing(c, Rect::new(x + 5, y + 1, 3, 3), 2, Ramp::Slate.at(Tone::Light), seed ^ 7, 3);
    } else {
        c.fill_normal(Rect::new(x, y, 7, 5), cover.at(Tone::Base), FLAT, 2);
        c.hline(x, x + 6, y, cover.at(Tone::Light), 3);
        c.fill_rect(Rect::new(x + 1, y + 5, 6, 1), Ramp::ClothCream.at(Tone::Light), 2);
        c.vline(x, y + 1, y + 4, cover.at(Tone::Light), 3);
    }
}

/// A crate: planked, framed in battens, braced across; its top and front.
#[allow(clippy::too_many_arguments)]
fn crate_box(c: &mut Canvas, k: &Kit, x: i32, bw: i32, foot: i32, face: i32, depth: i32, seed: u32) -> (Rect, Rect) {
    ao(c, x, x + bw - 1, foot, 4);
    let (top, front) = box3(c, x, bw, foot, face, depth, k.body, Some(((bw / 7).max(2), seed)), 3);
    let batten = k.body.at(Tone::Light);
    for r in [top, front] {
        c.vline(r.x, r.y, r.bottom() - 1, batten, 5);
        c.vline(r.right() - 1, r.y, r.bottom() - 1, k.body.at(Tone::Shade), 5);
    }
    c.hline(front.x, front.right() - 1, front.y, batten, 5);
    c.line((front.x + 2, front.bottom() - 2), (front.right() - 3, front.y + 2), k.trim.at(Tone::Base), 2, 5);
    // A stencilled mark on one crate in two.
    if seed % 2 == 0 && front.w > 14 {
        let (sx, sy) = (front.x + 3, front.y + 3);
        c.fill_rect(Rect::new(sx, sy, 4, 1), k.body.at(Tone::Deep), 6);
        c.fill_rect(Rect::new(sx + 1, sy + 2, 2, 1), k.body.at(Tone::Deep), 6);
    }
    (top, front)
}

/// A cog, five px across, lit on its upper left.
fn cog(c: &mut Canvas, x: i32, y: i32, ramp: Ramp, z: u8) {
    c.fill_rect(Rect::new(x, y + 1, 5, 3), ramp.at(Tone::Base), z);
    c.fill_rect(Rect::new(x + 1, y, 3, 5), ramp.at(Tone::Base), z);
    c.dot(x + 1, y + 1, ramp.at(Tone::Light), z);
    c.dot(x + 2, y + 2, ramp.at(Tone::Deep), z);
}

/// A length of pipe lying along the rack: a lit upper half, a shaded lower, its open end a
/// dark ring on the left.
fn pipe_length(c: &mut Canvas, x: i32, y: i32, len: i32, d: i32, ramp: Ramp, z: u8) {
    let mut m = Canvas::new(c.w(), c.h());
    m.fill_rect(Rect::new(x + 2, y, len - 2, d), Ix::INK, 1);
    c.inflate(&m, ramp, d / 2, Z::flat(z));
    c.retone(ramp, super::HARD);
    c.ellipse_lit(Rect::new(x, y, 5, d), ramp, Z::flat(z));
    c.ellipse(Rect::new(x + 1, y + 1, 3, d - 2), ramp.at(Tone::Deep), z);
    c.hline(x + 5, x + len - 2, y + 1, ramp.at(Tone::High), z);
}

fn carpet(c: &mut Canvas, k: &Kit) -> Stand {
    // A patterned rug: a field, a border of the accent between two guard lines, a medallion
    // in the middle and a quarter of one in each corner, small figures over the field, a path
    // worn across it, fringes at the short ends.
    let (w, foot) = (k.w, k.foot());
    let wide = k.fw >= k.fh;
    let r = if wide {
        Rect::new(3, foot - k.fh * 16 + 3, w - 6, k.fh * 16 - 5)
    } else {
        Rect::new(2, foot - k.fh * 16 + 4, w - 4, k.fh * 16 - 7)
    };
    let (b, a) = (k.body, k.accent);
    c.fill_normal(r, b.at(Tone::Base), FLAT, 1);
    // The border.
    let bd = 4;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let e = (x - r.x).min(r.right() - 1 - x).min(y - r.y).min(r.bottom() - 1 - y);
            let ix = match e {
                0 => a.at(Tone::Shade),
                1 | 2 => {
                    // A running pattern in the border: a dot every four px.
                    let along = if y - r.y <= 2 || r.bottom() - 1 - y <= 2 { x - r.x } else { y - r.y };
                    if e == 1 && along % 4 == 1 { b.at(Tone::Light) } else { a.at(Tone::Base) }
                }
                3 => a.at(Tone::Light),
                4 => b.at(Tone::Shade),
                _ => continue,
            };
            c.put(x, y, ix, FLAT, 1);
        }
    }
    // The medallion: a lozenge of the accent, a darker heart, a light eye.
    let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
    let (rx, ry) = ((r.w / 2 - bd - 3).max(3), (r.h / 2 - bd - 2).max(2));
    let lozenge = |c: &mut Canvas, sx: i32, sy: i32, ix: Ix| {
        c.polyline_fill(&[(cx - sx, cy), (cx, cy - sy), (cx + sx, cy), (cx, cy + sy)], ix, 1);
    };
    lozenge(c, rx, ry, a.at(Tone::Shade));
    lozenge(c, rx - 1, ry - 1, a.at(Tone::Base));
    lozenge(c, (rx * 2 / 3).max(2), (ry * 2 / 3).max(1), b.at(Tone::Deep));
    lozenge(c, (rx / 3).max(1), (ry / 3).max(1), a.at(Tone::Light));
    c.dot(cx, cy, k.trim.at(Tone::High), 1);
    // A quarter of a lozenge in each corner of the field.
    let q = 4;
    for (x0, y0, sx, sy) in [
        (r.x + bd + 1, r.y + bd + 1, 1, 1),
        (r.right() - bd - 2, r.y + bd + 1, -1, 1),
        (r.x + bd + 1, r.bottom() - bd - 2, 1, -1),
        (r.right() - bd - 2, r.bottom() - bd - 2, -1, -1),
    ] {
        c.polyline_fill(&[(x0, y0), (x0 + sx * q, y0), (x0, y0 + sy * q)], a.at(Tone::Base), 1);
        c.dot(x0, y0, a.at(Tone::Light), 1);
    }
    // Small figures over the field, where the medallion is not.
    for i in 0..(r.w * r.h / 90) {
        let hh = parts::hash(k.seed, i, 21);
        let x = r.x + bd + 2 + (hh % (r.w - 2 * bd - 4).max(1) as u32) as i32;
        let y = r.y + bd + 2 + ((hh >> 8) % (r.h - 2 * bd - 4).max(1) as u32) as i32;
        let (dx, dy) = ((x - cx).abs() * ry, (y - cy).abs() * rx);
        if dx + dy <= (rx + 2) * (ry + 2) {
            continue;
        }
        c.dot(x, y, b.at(Tone::Light), 1);
        c.dot(x + 1, y, b.at(Tone::Light), 1);
        c.dot(x, y + 1, b.at(Tone::Mid), 1);
    }
    // Worn where feet have crossed it: the pile gone pale in a soft band.
    for i in 0..(r.w / 3) {
        let hh = parts::hash(k.seed, i, 23);
        let x = r.x + r.w / 3 + (hh % (r.w / 2).max(1) as u32) as i32;
        let y = r.y + r.h / 2 + (hh >> 8) as i32 % 5 - 2;
        c.tint(x, y, b, Tone::Lift);
        c.tint(x + 1, y, b, Tone::Lift);
    }
    // Fringes at the short ends.
    let fringe = k.trim.at(Tone::Light);
    if wide {
        for y in (r.y + 1..r.bottom() - 1).step_by(2) {
            c.hline(r.x - 2, r.x - 1, y, fringe, 1);
            c.hline(r.right(), r.right() + 1, y, fringe, 1);
        }
    } else {
        for x in (r.x + 1..r.right() - 1).step_by(2) {
            c.vline(x, r.y - 2, r.y - 1, fringe, 1);
            c.vline(x, r.bottom(), r.bottom() + 1, fringe, 1);
        }
    }
    // A corner turned up, its underside showing.
    let (tx, ty) = (r.right() - 1, r.bottom() - 1);
    c.polyline_fill(&[(tx - 6, ty), (tx, ty), (tx, ty - 6)], b.at(Tone::Deep), 2);
    c.polyline_fill(&[(tx - 5, ty - 1), (tx - 1, ty - 1), (tx - 1, ty - 5)], k.trim.at(Tone::Base), 2);
    Stand::Flat(1)
}

fn junk_heap(c: &mut Canvas, k: &Kit) -> Stand {
    // A heap against a wall: a low lumpy mound of the body's stuff with pieces of the trim's
    // sticking out of it and a few fallen from it. By the body: rock and timber (the mine), scrap
    // iron (the works), bones and skulls (the crypt), brick and a pipe (the sewer), broken
    // chairs (anything of wood, over a bed of its own splinters).
    let (w, foot) = (k.w, k.foot());
    let rise = i32::from(k.look.rise);
    let top = foot - (k.fh * 16 - 8) - rise;
    let tall = foot - top;
    ao(c, 1, w - 2, foot, 6);
    let bones = k.body == Ramp::Bone;
    let wood = matches!(k.body, Ramp::WoodOak | Ramp::WoodDark | Ramp::WoodPale);
    // The mound: a wide low dome and two humps on it, one to each side of the middle.
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(Rect::new(1, foot - tall * 2 / 3, w - 2, tall * 2 / 3 + 1), Ix::INK, 1);
    let lean = if k.seed % 2 == 0 { 0 } else { w / 8 };
    m.ellipse(Rect::new(w / 6 + lean, top, w / 2, tall * 3 / 4), Ix::INK, 1);
    m.ellipse(Rect::new(w / 2 - 2 + lean / 2, top + tall / 4, w / 3 + 2, tall * 2 / 3), Ix::INK, 1);
    let bed = if wood { k.trim } else { k.body };
    c.inflate(&m, bed, 6, Z::new(2, 8));
    c.retone(bed, super::HARD);
    let on = |x: i32, y: i32| m.get(x, y).is_opaque() && m.get(x, y - 2).is_opaque();
    // What the mound is made of, lying on it: stones, bones, planks, by the body.
    let n = w * tall / 40;
    for i in 0..n {
        let hh = parts::hash(k.seed, i, 31);
        let x = 2 + (hh % (w as u32 - 8)) as i32;
        let y = top + 2 + ((hh >> 8) % (tall - 3).max(1) as u32) as i32;
        if !on(x, y) {
            continue;
        }
        let z = Z::new(6 + (i % 3) as u8, 8 + (i % 3) as u8);
        if bones {
            if hh & 0x7000 == 0 {
                skull(c, x - 3, y - 3, k.body, 9);
            } else {
                let (dx, dy) = [(6, 2), (5, -2), (2, 5), (7, 0)][(hh >> 12) as usize % 4];
                c.line((x, y), (x + dx, y + dy), k.body.at(Tone::Light), 2, 8);
                c.dot(x, y, k.body.at(Tone::High), 8);
                c.dot(x + dx, y + dy, k.body.at(Tone::High), 8);
                c.dot(x + dx / 2, y + dy / 2 + 1, k.body.at(Tone::Shade), 8);
            }
        } else if wood {
            let (dx, dy) = [(9, 3), (8, -3), (5, 5), (10, 0)][(hh >> 12) as usize % 4];
            c.line((x, y), (x + dx, y + dy), k.body.at(Tone::Shade), 3, 7);
            c.line((x, y - 1), (x + dx, y + dy - 1), k.body.at(Tone::Light), 1, 8);
        } else {
            let (sw, sh) = (4 + (hh >> 12) as i32 % 4, 3 + (hh >> 16) as i32 % 3);
            let mut st = Canvas::new(c.w(), c.h());
            st.polyline_fill(&[(x + 1, y), (x + sw - 1, y), (x + sw, y + sh), (x, y + sh)], Ix::INK, 1);
            c.inflate(&st, k.body, 2, z);
            c.retone(k.body, super::HARD);
            c.dot(x + 1, y, k.body.at(Tone::High), z.hi);
        }
    }
    if wood {
        // A chair's back and two of its legs, thrown on top.
        let (cx, cy) = (w / 2 - 6 + lean, top + 3);
        let t = k.body;
        for (a, b) in [
            ((cx, cy), (cx + 9, cy + 2)),
            ((cx, cy + 5), (cx + 9, cy + 7)),
            ((cx, cy), (cx - 1, cy + 11)),
            ((cx + 9, cy + 2), (cx + 8, cy + 13)),
        ] {
            c.line(a, b, t.at(Tone::Base), 2, 11);
            c.dot(a.0, a.1, t.at(Tone::Light), 11);
        }
    } else if !bones {
        // What sticks out of it: timbers, a pipe, a rail, by the trim.
        for i in 0..2 {
            let hh = parts::hash(k.seed, i, 33);
            let x = w / 3 + (hh % (w as u32 / 3)) as i32;
            let y = top + tall / 3 + (hh >> 8) as i32 % 4;
            let (dx, dy) = [(-9, -7), (10, -5)][i as usize];
            let t = k.trim;
            c.line((x, y), (x + dx, y + dy), t.at(Tone::Shade), 3, 9);
            c.line((x, y - 1), (x + dx, y + dy - 1), t.at(Tone::Light), 1, 10);
            c.dot(x + dx, y + dy, t.at(Tone::High), 10);
        }
    } else {
        skull(c, w / 2 - 3 + lean, top + 1, k.body, 11);
    }
    // A few pieces fallen from it at its foot.
    for i in 0..5 {
        let hh = parts::hash(k.seed, i, 35);
        let x = 2 + (hh % (w as u32 - 5)) as i32;
        let y = foot - 1 - (hh >> 12) as i32 % 3;
        let ramp = if wood { k.body } else { bed };
        c.fill_rect(Rect::new(x, y - 1, 2 + (hh >> 8) as i32 % 2, 2), ramp.at(Tone::Base), 3);
        c.dot(x, y - 1, ramp.at(Tone::Light), 3);
    }
    Stand::Up(&[])
}

// A skull, six px across, lit on its upper left, its sockets dark.
fn skull(c: &mut Canvas, x: i32, y: i32, ramp: Ramp, z: u8) {
    c.ellipse_lit(Rect::new(x, y, 7, 6), ramp, Z::flat(z));
    c.fill_rect(Rect::new(x + 1, y + 5, 5, 2), ramp.at(Tone::Base), z);
    c.dot(x + 2, y + 3, Ix::SEAM, z);
    c.dot(x + 4, y + 3, Ix::SEAM, z);
    c.dot(x + 3, y + 5, ramp.at(Tone::Shade), z);
}

fn shelf_fallen(c: &mut Canvas, k: &Kit) -> Stand {
    // A bookcase gone over on its face, its back to the ceiling, its top end still up on the
    // books that went down under it; the rest of them spilled across the floor in front.
    let (w, foot) = (k.w, k.foot());
    let (x0, x1) = (2, w - 3);
    let (lo, hi) = (foot - 9, foot - 19);
    ao(c, x0, x1, foot - 4, 6);
    // The back of the carcass, planked, slanting from the floor on the left up to the right.
    let pts = [(x0, lo - 4), (x1, hi - 4), (x1, hi + 8), (x0, lo + 6)];
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(&pts, Ix::INK, 1);
    c.inflate(&m, k.body, 1, Z::new(3, 6));
    c.retone(
        k.body,
        [Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::Light],
    );
    for i in 1..4 {
        let t = i * (x1 - x0) / 4;
        c.line(
            (x0 + t, lo - 4 - (lo - hi) * t / (x1 - x0)),
            (x0 + t, lo + 6 - (lo - hi) * t / (x1 - x0) - 2),
            k.body.at(Tone::Shade),
            1,
            6,
        );
    }
    // Its end, standing up at the right: the side board and a shelf's edge.
    c.fill_normal(Rect::new(x1 - 3, hi - 4, 4, 13), k.body.at(Tone::Mid), parts::south(), 5);
    c.vline(x1 - 3, hi - 4, hi + 8, k.body.at(Tone::Light), 5);
    // Books under its high end and spilled in front.
    for i in 0..(w / 6) {
        let hh = parts::hash(k.seed, i, 51);
        let x = 2 + i * 6 + (hh % 3) as i32;
        let y = foot - 5 - (hh >> 4) as i32 % 3 + if x > w / 2 { -3 } else { 0 };
        book(c, x, y.min(foot - 5), BOOKS[(hh >> 8) as usize % BOOKS.len()], hh & 0x3000 == 0, k.seed ^ i as u32);
    }
    Stand::Up(&[])
}

fn tool_rack(c: &mut Canvas, k: &Kit) -> Stand {
    // A dark board of planks on two feet against the wall, tools hung from its pegs, bright
    // against it: a pick, a shovel, a saw, a coil of rope, a hammer and a lamp, by the seed.
    let (w, foot) = (k.w, k.foot());
    let (x, bw) = (1, w - 2);
    let (top, bh) = (foot - 34, 26);
    ao(c, x, x + bw - 1, foot, 4);
    for px in [x + 1, x + bw - 4] {
        post(c, px, top, foot - 1, 3, k.body, 2);
    }
    planks(c, Rect::new(x, top, bw, bh), k.body, 3, true, false, k.seed, 3);
    c.shade(Rect::new(x - 4, top + 2, bw + 8, bh * 2), k.body, 1);
    c.hline(x, x + bw - 1, top, k.body.at(Tone::Light), 3);
    c.hline(x, x + bw - 1, top + bh - 1, k.body.at(Tone::Deep), 3);
    let slots = (bw - 2) / 11;
    let pitch = (bw - 2) / slots.max(1);
    let (t, wd) = (k.trim, k.accent);
    for i in 0..slots {
        let sx = x + 3 + i * pitch;
        c.fill_rect(Rect::new(sx + 3, top + 3, 2, 2), Ramp::Brass.at(Tone::Light), 5);
        let which = parts::hash(k.seed, i, 53).wrapping_add(i as u32 * 7) % 6;
        match which {
            0 => {
                // A pick: the helve hanging down, the head a curved bar across its top.
                c.line((sx + 4, top + 5), (sx + 4, top + bh + 2), wd.at(Tone::Base), 2, 6);
                c.vline(sx + 4, top + 6, top + bh + 1, wd.at(Tone::Light), 6);
                c.polyline_fill(
                    &[
                        (sx - 1, top + 9),
                        (sx + 4, top + 4),
                        (sx + 10, top + 9),
                        (sx + 9, top + 10),
                        (sx + 4, top + 7),
                        (sx, top + 10),
                    ],
                    t.at(Tone::Base),
                    7,
                );
                c.line((sx, top + 9), (sx + 4, top + 5), t.at(Tone::High), 1, 8);
            }
            1 => {
                // A shovel: a handle with its grip, a spade's blade at the foot, lit down its left.
                c.fill_rect(Rect::new(sx + 1, top + 3, 7, 2), wd.at(Tone::Light), 6);
                c.line((sx + 4, top + 5), (sx + 4, top + bh - 8), wd.at(Tone::Base), 2, 6);
                c.polyline_fill(
                    &[
                        (sx, top + bh - 9),
                        (sx + 8, top + bh - 9),
                        (sx + 8, top + bh),
                        (sx + 4, top + bh + 3),
                        (sx, top + bh),
                    ],
                    t.at(Tone::Base),
                    7,
                );
                c.vline(sx, top + bh - 9, top + bh, t.at(Tone::High), 8);
                c.hline(sx, sx + 8, top + bh - 9, t.at(Tone::Light), 8);
            }
            2 => {
                // A saw: the blade's teeth down its front edge, the handle up top.
                c.polyline_fill(
                    &[(sx + 1, top + 8), (sx + 8, top + 8), (sx + 7, top + bh + 1), (sx + 1, top + bh - 2)],
                    t.at(Tone::Light),
                    6,
                );
                for y in (top + 9..top + bh - 1).step_by(2) {
                    c.dot(sx + 1, y, t.at(Tone::Deep), 7);
                }
                c.vline(sx + 7, top + 9, top + bh - 1, t.at(Tone::Base), 7);
                c.fill_rect(Rect::new(sx + 1, top + 3, 7, 6), wd.at(Tone::Base), 7);
                c.fill_rect(Rect::new(sx + 3, top + 5, 3, 2), k.body.at(Tone::Deep), 8);
            }
            3 => {
                // A coil of rope.
                c.ellipse(Rect::new(sx, top + 5, 10, 12), Ramp::Reed.at(Tone::Light), 6);
                c.ellipse(Rect::new(sx + 2, top + 7, 6, 8), Ramp::Reed.at(Tone::Shade), 6);
                c.ellipse(Rect::new(sx + 3, top + 8, 4, 6), k.body.at(Tone::Deep), 6);
                c.line((sx + 6, top + 16), (sx + 7, top + bh + 1), Ramp::Reed.at(Tone::Base), 2, 6);
            }
            4 => {
                // A hammer and a spanner.
                c.line((sx + 2, top + 6), (sx + 2, top + 20), wd.at(Tone::Base), 2, 6);
                c.fill_rect(Rect::new(sx - 1, top + 4, 7, 4), t.at(Tone::Base), 7);
                c.hline(sx - 1, sx + 5, top + 4, t.at(Tone::High), 7);
                c.line((sx + 7, top + 7), (sx + 7, top + 22), t.at(Tone::Light), 2, 6);
                c.fill_rect(Rect::new(sx + 5, top + 21, 6, 4), t.at(Tone::Base), 7);
                c.fill_rect(Rect::new(sx + 7, top + 23, 2, 2), k.body.at(Tone::Deep), 8);
            }
            _ => {
                // A miner's lamp on its hook, cold.
                c.fill_rect(Rect::new(sx + 1, top + 6, 8, 11), t.at(Tone::Base), 6);
                c.fill_rect(Rect::new(sx + 2, top + 8, 6, 6), Ramp::Glass.at(Tone::Shade), 7);
                c.dot(sx + 3, top + 9, Ramp::Glass.at(Tone::Light), 7);
                c.dot(sx + 4, top + 10, Ramp::Glass.at(Tone::Light), 7);
                c.hline(sx + 1, sx + 8, top + 6, t.at(Tone::High), 7);
                c.line((sx + 5, top + 5), (sx + 4, top + 3), t.at(Tone::Light), 1, 7);
            }
        }
    }
    Stand::Up(&[])
}

fn tomb_open(c: &mut Canvas, k: &Kit) -> Stand {
    // A chest tomb robbed or woken: the lid pushed back and askew so the front of the hollow
    // shows, the dark in it, a skull looking out, a hand's bones over the rim.
    let (w, foot) = (k.w, k.foot());
    let (x, tw) = (2, w - 4);
    let depth = (k.fh * 16 - 14).clamp(10, 34);
    ao(c, x, x + tw - 1, foot, 6);
    let (top, front) = box3(c, x, tw, foot, 10, depth, k.body, None, 3);
    c.retone(k.body, super::HARD);
    // The hollow, dark, with what lies in it.
    let hollow = Rect::new(top.x + 2, top.y + 2, top.w - 4, top.h - 3);
    c.fill_normal(hollow, Ramp::Void.at(Tone::Deep), FLAT, 4);
    skull(c, hollow.x + 2, hollow.bottom() - 8, Ramp::Bone, 5);
    // The lid over the back of it, askew: its near edge a diagonal, lit, its shadow on the dark.
    let (l0, l1) = (top.y - 3, top.y + top.h * 11 / 20);
    let lid = [(top.x + 1, l0 + 1), (top.right(), l0 - 1), (top.right() - 1, l1 - 5), (top.x + 3, l1 + 1)];
    c.polyline_fill(&[lid[3], lid[2], (lid[2].0, lid[2].1 + 3), (lid[3].0, lid[3].1 + 3)], Ix::SEAM, 6);
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(&lid, Ix::INK, 1);
    c.inflate(&m, k.body, 1, Z::new(7, 8));
    c.retone(k.body, super::HARD);
    c.line(lid[3], lid[2], k.body.at(Tone::High), 1, 9);
    c.line((lid[3].0, lid[3].1 + 1), (lid[2].0, lid[2].1 + 1), k.body.at(Tone::Mid), 1, 9);
    // The worn figure on the lid, and a quatrefoil on the chest's face.
    let cx = (lid[0].0 + lid[1].0) / 2;
    c.ellipse_lit(Rect::new(cx - 3, l0 + 2, 6, 5), k.body, Z::flat(9));
    c.fill_rect(Rect::new(cx - 2, l0 + 7, 4, (l1 - l0 - 12).max(2)), k.body.at(Tone::Light), 9);
    let q = (front.x + tw / 2 - 4, front.y + 2);
    for (dx, dy) in [(2, 0), (0, 2), (4, 2), (2, 4)] {
        c.fill_rect(Rect::new(q.0 + dx, q.1 + dy, 2, 2), k.body.at(Tone::Shade), 4);
    }
    c.dot(q.0 + 2, q.1 + 2, k.accent.at(Tone::Light), 4);
    let bone = Ramp::Bone;
    for f in 0..3 {
        c.vline(hollow.right() - 3 - f * 2, hollow.bottom() - 2, hollow.bottom() + 2, bone.at(Tone::Light), 6);
    }
    c.hline(hollow.right() - 8, hollow.right() - 2, hollow.bottom() - 2, bone.at(Tone::Base), 6);
    Stand::Up(&[])
}

fn lathe(c: &mut Canvas, k: &Kit) -> Stand {
    // A lathe: a cast bed on two cabinet legs, the headstock on the left with its chuck and a
    // belt guard, the tailstock on the right, the tool post between, curls of swarf in the tray
    // and on the floor. The paint is the body's, the bright metal the trim's.
    let (w, foot) = (k.w, k.foot());
    let (x0, x1) = (2, w - 3);
    ao(c, x0, x1, foot, 6);
    // Legs.
    for (lx, lw) in [(x0 + 1, 11), (x1 - 10, 9)] {
        let (top, _) = box3(c, lx, lw, foot, 13, 3, k.body, None, 3);
        c.hline(top.x + 2, top.right() - 3, top.bottom() + 5, k.body.at(Tone::Shade), 3);
    }
    c.retone(k.body, super::HARD);
    // The tray between the legs.
    c.fill_normal(Rect::new(x0 + 12, foot - 14, x1 - x0 - 22, 3), k.trim.at(Tone::Shade), FLAT, 4);
    // The bed: two ways of bright metal on the paint.
    let bed = Rect::new(x0, foot - 20, x1 - x0 + 1, 6);
    c.fill_normal(bed, k.body.at(Tone::Base), parts::south(), 5);
    c.hline(bed.x, bed.right() - 1, bed.y, k.trim.at(Tone::High), 6);
    c.hline(bed.x, bed.right() - 1, bed.y + 2, k.trim.at(Tone::Light), 6);
    c.hline(bed.x, bed.right() - 1, bed.bottom() - 1, k.body.at(Tone::Shade), 5);
    // The headstock, its belt guard, the chuck.
    let hs = Rect::new(x0 + 1, foot - 33, 14, 14);
    c.rect_bevel(hs, k.body, 1, Z::new(6, 8));
    c.fill_normal(Rect::new(hs.x + 2, hs.y - 5, 9, 6), k.body.at(Tone::Mid), parts::south(), 7);
    c.hline(hs.x + 2, hs.x + 10, hs.y - 5, k.body.at(Tone::Light), 7);
    c.dot(hs.x + 3, hs.y + 3, Ramp::Brass.at(Tone::Light), 8);
    c.ellipse_lit(Rect::new(hs.right() - 1, hs.y + 3, 6, 8), k.trim, Z::flat(8));
    c.dot(hs.right() + 2, hs.y + 7, Ix::SEAM, 9);
    // The tailstock and its handwheel.
    let ts = Rect::new(x1 - 12, foot - 28, 9, 9);
    c.rect_bevel(ts, k.body, 1, Z::new(6, 7));
    c.fill_rect(Rect::new(ts.x - 6, ts.y + 3, 6, 2), k.trim.at(Tone::Light), 7);
    c.ellipse(Rect::new(ts.right() - 1, ts.y + 1, 4, 7), k.trim.at(Tone::Base), 7);
    // The tool post on its saddle.
    let tp = Rect::new(w / 2 - 3, foot - 26, 7, 6);
    c.rect_bevel(tp, k.trim, 1, Z::new(6, 7));
    c.line((tp.x + 1, tp.y), (tp.x - 3, tp.y - 2), k.trim.at(Tone::High), 1, 8);
    c.retone(k.trim, super::HARD);
    // Swarf: bright curls in the tray and on the floor.
    for i in 0..5 {
        let hh = parts::hash(k.seed, i, 63);
        let x = x0 + 8 + (hh % (x1 - x0 - 16) as u32) as i32;
        let y = if i < 3 { foot - 14 } else { foot - 2 - (hh >> 8) as i32 % 2 };
        c.dot(x, y, k.trim.at(Tone::High), 6);
        c.dot(x + 1, y - 1, k.trim.at(Tone::Light), 6);
        c.dot(x + 2, y, k.trim.at(Tone::High), 6);
    }
    Stand::Up(&[])
}
