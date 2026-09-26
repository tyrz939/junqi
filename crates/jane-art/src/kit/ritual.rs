//! `ritual` (ART.md §2.3): the things a player works. `on` is a lever thrown, a plate pressed,
//! an orb or a stone lit (emitting). Shapes: `hatch`, `stairs`, `plate`, `lever`, `orb`,
//! `manhole`, `way_in`, `ladder`, `summon_stone`, `shrine`.

use jane_core::grid::Rect;

use super::parts::{self, ao, blocks, planks, post};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z, normal};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let on = state == State::On;
    let (w, h, foot) = (k.w, k.h, k.foot());
    Some(match k.look.shape {
        "hatch" => {
            // A cellar hatch in the floor: two planked leaves in an iron frame, a ring to lift.
            let r = Rect::new(3, foot - (h - 4).min(24), w - 6, (h - 4).min(24));
            c.fill_normal(Rect::new(r.x - 2, r.y - 1, r.w + 4, r.h + 2), k.trim.at(Tone::Base), FLAT, 1);
            c.hline(r.x - 2, r.right() + 1, r.y - 1, k.trim.at(Tone::Light), 1);
            planks(c, Rect::new(r.x, r.y, r.w / 2, r.h), k.body, 3, false, true, k.seed, 2);
            planks(c, Rect::new(r.x + r.w / 2, r.y, r.w - r.w / 2, r.h), k.body, 3, false, true, k.seed ^ 1, 2);
            c.vline(r.x + r.w / 2, r.y, r.bottom() - 1, k.body.at(Tone::Deep), 2);
            for y in [r.y + 3, r.bottom() - 4] {
                c.hline(r.x + 1, r.right() - 2, y, k.trim.at(Tone::Mid), 3);
            }
            c.ellipse(Rect::new(r.x + r.w / 2 - 2, r.y + r.h / 2 - 1, 4, 3), k.accent.at(Tone::Light), 3);
            c.dot(r.x + r.w / 2 - 1, r.y + r.h / 2, k.body.at(Tone::Shade), 3);
            Stand::Flat(2)
        }
        "stairs" | "way_in" => {
            // Steps going down into the dark: each tread lit on its nose, each riser deeper.
            let r = Rect::new(2, foot - (h - 2).min(28), w - 4, (h - 2).min(28));
            c.fill_normal(Rect::new(r.x - 2, r.y - 2, r.w + 4, r.h + 3), k.trim.at(Tone::Mid), FLAT, 2);
            c.hline(r.x - 2, r.right() + 1, r.y - 2, k.trim.at(Tone::Light), 2);
            let n = (r.h / 5).max(3);
            for i in 0..n {
                let y = r.y + i * r.h / n;
                let t = Tone::ALL[(5 - i).clamp(0, 5) as usize];
                c.fill_normal(Rect::new(r.x, y, r.w, r.h / n), k.body.at(t), normal(0, -60), 1);
                c.hline(r.x, r.right() - 1, y, k.body.at(t.step(1)), 1);
            }
            Stand::Flat(1)
        }
        "plate" => {
            // A pressure plate: a slab in a sunk frame; pressed, it sits lower and its seam dark.
            let r = Rect::new(4, foot - 22, w - 8, 18);
            c.fill_normal(Rect::new(r.x - 2, r.y - 2, r.w + 4, r.h + 4), k.trim.at(Tone::Shade), FLAT, 1);
            let d = i32::from(on);
            c.rect_bevel(Rect::new(r.x, r.y + d, r.w, r.h - d), k.body, 2, Z::new(2, if on { 2 } else { 3 }));
            for i in 0..3 {
                c.hline(r.x + 3, r.right() - 4, r.y + 5 + i * 4 + d, k.body.at(Tone::Mid), 3);
            }
            Stand::Flat(3)
        }
        "lever" => {
            // A lever in an iron box: thrown back, or forward when on.
            let cx = w / 2;
            ao(c, cx - 5, cx + 4, foot, 4);
            let base = Rect::new(cx - 5, foot - 6, 10, 6);
            c.rect_bevel(base, k.trim, 1, Z::new(2, 4));
            c.fill_rect(Rect::new(cx - 3, base.y + 1, 6, 1), Ix::SEAM, 5);
            let tip = if on { (cx + 4, foot - 16) } else { (cx - 4, foot - 16) };
            c.line((cx, base.y + 1), tip, k.body.at(Tone::Base), 2, 6);
            c.disc_lit(tip.0, tip.1, 2, k.accent, Z::flat(7));
            Stand::Up(&[])
        }
        "orb" => {
            // An orb on a turned stand: glass with a light in it; on, it shines in its colour.
            let cx = w / 2;
            ao(c, cx - 6, cx + 5, foot, 5);
            c.rect_bevel(Rect::new(cx - 5, foot - 4, 10, 4), k.trim, 1, Z::new(2, 4));
            post(c, cx - 1, foot - 12, foot - 4, 3, k.trim, 4);
            c.polyline_fill(&[(cx - 4, foot - 13), (cx + 3, foot - 13), (cx + 1, foot - 11), (cx - 2, foot - 11)], k.trim.at(Tone::Base), 5);
            let orb = Rect::new(cx - 6, foot - 25, 12, 12);
            c.set_emitting(on);
            c.soft_ellipse(orb, k.accent, Z::new(6, 12));
            c.set_emitting(false);
            if !on {
                c.retone(k.accent, [Tone::Deep, Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Light, Tone::Light]);
            }
            c.dot(cx - 3, orb.y + 3, Ramp::HairWhite.at(Tone::High), 12);
            Stand::Up(&[])
        }
        "manhole" => {
            // An iron cover in its ring, cast with a lattice.
            let r = Rect::new(w / 2 - 11, foot - 20, 22, 18);
            c.ellipse(r, k.trim.at(Tone::Shade), 1);
            c.ellipse(Rect::new(r.x + 2, r.y + 1, r.w - 4, r.h - 3), k.body.at(Tone::Base), 2);
            for i in 0..4 {
                c.hline(r.x + 4, r.right() - 5, r.y + 4 + i * 3, k.body.at(Tone::Mid), 2);
            }
            c.dot(r.x + 6, r.y + 2, k.body.at(Tone::Light), 2);
            Stand::Flat(2)
        }
        "ladder" => {
            // A ladder leaning back against what is behind it; broken, a rung gone and a rail
            // snapped short.
            let broken = k.fw > k.fh;
            let (x0, top) = (w / 2 - 5, foot - (h - 2).min(40));
            ao(c, x0, x0 + 9, foot, 4);
            let snap = if broken { top + (foot - top) / 2 } else { top };
            c.line((x0, foot - 1), (x0 + 2, snap), k.body.at(Tone::Light), 2, 5);
            c.line((x0 + 9, foot - 1), (x0 + 11, top), k.body.at(Tone::Base), 2, 5);
            for (i, y) in (top + 3..foot - 2).step_by(5).enumerate() {
                if broken && (i == 1 || y < snap) {
                    continue;
                }
                c.hline(x0 + 1, x0 + 9, y, k.body.at(Tone::Base), 6);
            }
            Stand::Up(&[])
        }
        "summon_stone" => {
            // A low round stone with a ring of cut marks that glow when it wakes.
            let r = Rect::new(3, foot - 16, w - 6, 15);
            ao(c, r.x, r.right() - 1, foot, 5);
            let mut m = Canvas::new(c.w(), c.h());
            m.ellipse(r, Ix::INK, 1);
            c.inflate(&m, k.body, 4, Z::new(2, 8));
            c.retone(k.body, super::HARD);
            c.set_emitting(on);
            for i in 0..6 {
                let a = jane_core::angle::Angle((i * 65536 / 6) as u16);
                let (s, co) = (jane_core::angle::sin_q15(a).0, jane_core::angle::cos_q15(a).0);
                let (x, y) = (r.x + r.w / 2 + ((co * (r.w / 3)) >> 15), r.y + r.h / 2 + ((s * (r.h / 3)) >> 15));
                let ix = if on { k.accent.at(Tone::High) } else { k.body.at(Tone::Deep) };
                c.fill_rect(Rect::new(x, y, 2, 1), ix, 9);
            }
            c.set_emitting(false);
            Stand::Up(&[])
        }
        "shrine" => {
            // A wayside shrine: a little roofed box on a post, a candle and a figure inside.
            let cx = w / 2;
            ao(c, cx - 4, cx + 3, foot, 4);
            post(c, cx - 2, foot - 16, foot - 1, 4, k.trim, 3);
            let bx = Rect::new(cx - 7, foot - 30, 14, 14);
            c.fill_normal(bx, k.body.at(Tone::Base), parts::south(), 5);
            c.fill_normal(Rect::new(bx.x + 3, bx.y + 3, bx.w - 6, bx.h - 4), k.body.at(Tone::Deep), parts::south(), 5);
            c.fill_rect(Rect::new(cx - 1, bx.y + 6, 2, 6), Ramp::Stone.at(Tone::Light), 6);
            c.dot(cx - 1, bx.y + 5, Ramp::Stone.at(Tone::High), 6);
            c.polyline_fill(&[(bx.x - 2, bx.y), (cx - 1, bx.y - 6), (cx, bx.y - 6), (bx.right() + 1, bx.y)], k.accent.at(Tone::Base), 7);
            c.line((bx.x - 2, bx.y), (cx - 1, bx.y - 6), k.accent.at(Tone::Light), 1, 7);
            blocks(c, Rect::new(cx - 5, foot - 3, 10, 3), Ramp::Stone, 3, 5, false, k.seed, 2);
            Stand::Up(&[])
        }
        _ => return None,
    })
}
