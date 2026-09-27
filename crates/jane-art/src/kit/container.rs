//! `container` (ART.md §2.3): what holds things. `open` is the lid up (a chest looted, a crate
//! prised). Shapes: `chest`, `trunk`, `crate`, `barrel`, `sack`, `jar`, `churn`, `coffin`,
//! `tin`, `bowl`, `bottles`, `parcel`, `mug`.

use jane_core::grid::Rect;

use super::parts::{self, ao, band, box3, lid_height, planks};
use super::{Kit, Stand, State};
use crate::canvas::{Canvas, Z, normal};
use crate::palette::{Ix, Ramp, Tone};

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let open = state == State::Open;
    let (w, foot) = (k.w, k.foot());
    Some(match k.look.shape {
        "chest" | "trunk" => {
            // A chest: a planked body, a domed lid bound in iron, a lock plate; open, the lid
            // stands back on its hinges and the dark inside shows.
            let (bw, face, depth) = ((w - 6).min(26), 10, 6);
            let x = (w - bw) / 2;
            ao(c, x, x + bw - 1, foot, 5);
            let (top, front) = box3(c, x, bw, foot, face, depth, k.body, Some((3, k.seed)), 3);
            let bands = [x + 3, x + bw - 5];
            if open {
                // The inside, dark, and the lid standing up behind it, its underside in shade.
                c.fill_normal(Rect::new(top.x + 1, top.y + 1, top.w - 2, top.h - 1), k.body.at(Tone::Deep), crate::canvas::FLAT, 4);
                let lid = Rect::new(x, top.y - 9, bw, 9);
                planks(c, lid, k.body, 3, true, false, k.seed ^ 3, 2);
                c.hline(lid.x, lid.right() - 1, lid.bottom() - 1, k.body.at(Tone::Shade), 2);
                if k.accent == Ramp::Brass {
                    c.fill_rect(Rect::new(top.x + 4, top.bottom() - 3, 4, 2), Ramp::Brass.at(Tone::Light), 5);
                    c.dot(top.x + 5, top.bottom() - 3, Ramp::Brass.at(Tone::Glint), 5);
                }
                for bx in bands {
                    c.fill_rect(Rect::new(bx, lid.y, 2, lid.h), k.trim.at(Tone::Mid), 3);
                }
            } else {
                // The lid, a barrel vault overhanging the body a px each side: seen from above
                // and in front, its crown catches the light in a band, its front turns down to
                // the viewer, its ends turn away (the west lit, the east in shade); the staves
                // run along it, the lip throws a line of shadow on the body under it.
                let (lx, lw) = (x - 1, bw + 2);
                let (ly, lip) = (top.y - 5, front.y + 1);
                let lh = lip - ly;
                for y in ly..lip {
                    let t = (y - ly) * 100 / lh.max(1);
                    let (tone, ny) = match t {
                        0..=14 => (Tone::Base, -100),
                        15..=39 => (Tone::Light, -60),
                        40..=54 => (Tone::Lift, -10),
                        55..=79 => (Tone::Base, 40),
                        _ => (Tone::Mid, 80),
                    };
                    // Round the vault's back corners.
                    let cut = if y == ly { 2 } else { i32::from(y == ly + 1) };
                    for xx in lx + cut..lx + lw - cut {
                        let end = if xx < lx + 2 { 1 } else if xx >= lx + lw - 2 { -2 } else { 0 };
                        c.put(xx, y, k.body.at(tone.step(end)), crate::canvas::normal(end * -40, ny), 6);
                    }
                }
                for y in [ly + lh * 3 / 10, ly + lh * 6 / 10] {
                    c.hline(lx + 1, lx + lw - 2, y, k.body.at(Tone::Shade), 6);
                }
                c.hline(lx, lx + lw - 1, lip - 1, k.body.at(Tone::Deep), 6);
                c.shade(Rect::new(x - 2, lip - 1, bw + 4, 3), k.body, 1);
                for bx in bands {
                    for y in ly + 1..lip {
                        let t = (y - ly) * 100 / lh.max(1);
                        let tone = if (15..40).contains(&t) { Tone::High } else if t < 55 { Tone::Light } else { Tone::Base };
                        c.put(bx, y, k.trim.at(tone), crate::canvas::normal(0, 0), 7);
                        c.put(bx + 1, y, k.trim.at(tone.step(-1)), crate::canvas::normal(0, 0), 7);
                    }
                }
                // The hasp: a brass plate hanging from the lip over the body, a keyhole in it.
                let hx = x + bw / 2 - 2;
                c.fill_rect(Rect::new(hx, lip - 2, 4, 5), k.accent.at(Tone::Base), 8);
                c.hline(hx, hx + 3, lip - 2, k.accent.at(Tone::High), 8);
                c.vline(hx + 3, lip - 1, lip + 2, k.accent.at(Tone::Shade), 8);
                c.dot(hx + 1, lip, Ix::SEAM, 8);
                c.dot(hx + 1, lip + 1, Ix::SEAM, 8);
                let _ = Z::flat(0);
            }
            for bx in bands {
                c.fill_rect(Rect::new(bx, front.y, 2, front.h), k.trim.at(Tone::Mid), 4);
                c.dot(bx, front.y + 2, k.trim.at(Tone::High), 4);
            }
            Stand::Tops([(Rect::new(x, 0, bw, front.y), lid_height(face)), (Rect::default(), 0)])
        }
        "crate" => {
            // A crate: planked faces in a frame of battens, a brace across the front.
            let (bw, face, depth) = ((w - 4).min(26), 14, 10);
            let x = (w - bw) / 2;
            ao(c, x, x + bw - 1, foot, 5);
            let (top, front) = box3(c, x, bw, foot, face, depth, k.body, Some((4, k.seed)), 3);
            if open {
                c.fill_normal(Rect::new(top.x + 2, top.y + 2, top.w - 4, top.h - 3), k.body.at(Tone::Deep), crate::canvas::FLAT, 5);
                c.hline(top.x + 2, top.right() - 3, top.y + 2, Ramp::Reed.at(Tone::Light), 5);
            }
            batten(c, k, top, true);
            batten(c, k, front, false);
            c.line((front.x + 2, front.bottom() - 3), (front.right() - 3, front.y + 2), k.trim.at(Tone::Base), 2, 5);
            Stand::Tops([(top, lid_height(face)), (Rect::default(), 0)])
        }
        "barrel" => {
            // A barrel: staves round a bellied body, two iron hoops, the head's planks on top.
            let bw = (w - 8).min(20);
            let x = (w - bw) / 2;
            let (h, top_y) = (18, foot - 18);
            ao(c, x, x + bw - 1, foot, 5);
            c.polygon_lit(&[(x + 1, top_y + 3), (x + bw - 2, top_y + 3), (x + bw - 1, top_y + h / 2), (x + bw - 2, foot), (x + 1, foot), (x, top_y + h / 2)], k.body, 110, Z::new(2, 4));
            c.retone(k.body, super::HARD);
            for sx in (x + 3..x + bw - 2).step_by(4) {
                c.vline(sx, top_y + 4, foot - 1, k.body.at(Tone::Shade), 4);
            }
            for y in [top_y + 5, foot - 4] {
                band(c, x, x + bw - 1, y, k.trim, 5);
            }
            let head = Rect::new(x + 1, top_y, bw - 2, 5);
            c.ellipse(head, k.body.at(Tone::Light), 6);
            c.ellipse(Rect::new(head.x + 1, head.y + 1, head.w - 2, head.h - 1), k.body.at(Tone::Base), 6);
            c.hline(head.x + 3, head.right() - 4, head.y + 2, k.body.at(Tone::Mid), 6);
            Stand::Tops([(Rect::new(x, 0, bw, top_y + 4), lid_height(h - 3)), (Rect::default(), 0)])
        }
        "sack" => {
            // Sacks slumped against each other, tied at the neck, their weave in soft folds.
            let n = (w / 12).max(1);
            ao(c, 1, w - 2, foot, 5);
            for i in 0..n {
                let sw = w / n - 1;
                let sx = i * (w / n);
                let top = foot - 12 + (i % 2);
                let mut m = Canvas::new(c.w(), c.h());
                m.ellipse(Rect::new(sx, top + 3, sw, foot - top - 2), Ix::INK, 1);
                m.fill_rect(Rect::new(sx + sw / 2 - 2, top, 4, 4), Ix::INK, 1);
                c.inflate(&m, k.body, 3, Z::new(2, 5));
                c.retone(k.body, super::HARD);
                c.hline(sx + sw / 2 - 2, sx + sw / 2 + 1, top + 3, k.trim.at(Tone::Base), 6);
                c.vline(sx + sw / 2 + 1, top + 6, foot - 3, k.body.at(Tone::Mid), 5);
            }
            Stand::Up(&[])
        }
        "jar" => {
            // A clay jar: a round belly, a narrow neck, a lip lit on its rim.
            let bw = (w - 4).min(24);
            let x = (w - bw) / 2;
            let bh = bw.min(20);
            ao(c, x + 1, x + bw - 2, foot, 4);
            let mut m = Canvas::new(c.w(), c.h());
            m.ellipse(Rect::new(x, foot - bh + 2, bw, bh - 2), Ix::INK, 1);
            let nw = bw / 2;
            m.fill_rect(Rect::new(x + (bw - nw) / 2, foot - bh - 1, nw, 4), Ix::INK, 1);
            c.inflate(&m, k.body, bw / 4, Z::new(2, 5));
            c.retone(k.body, super::HARD);
            let lip = Rect::new(x + (bw - nw) / 2 - 1, foot - bh - 2, nw + 2, 2);
            c.fill_normal(lip, k.body.at(Tone::Light), crate::canvas::FLAT, 6);
            c.hline(lip.x + 1, lip.right() - 2, lip.bottom(), Ix::SEAM, 6);
            // A painted band round the belly, set by the seed; one jar in two is chipped.
            let by = foot - bh / 2 + (k.seed % 3) as i32 - 1;
            c.hline(x + 2, x + bw - 3, by, k.accent.at(Tone::Base), 6);
            if k.seed % 2 == 1 {
                c.dot(x + bw - 3, foot - bh + 4, Ix::SEAM, 6);
                c.dot(x + bw - 4, foot - bh + 5, k.body.at(Tone::Light), 6);
            }
            Stand::Up(&[])
        }
        "churn" => {
            // A milk churn: a tin body, a shoulder, a neck, a lid with its handle.
            let (x, bw) = (w / 2 - 5, 10);
            ao(c, x, x + bw - 1, foot, 4);
            c.polygon_lit(&[(x, foot - 13), (x + bw - 1, foot - 13), (x + bw - 1, foot), (x, foot)], k.body, 100, Z::new(2, 4));
            c.polygon_lit(&[(x + 1, foot - 16), (x + bw - 2, foot - 16), (x + bw - 1, foot - 13), (x, foot - 13)], k.body, 100, Z::flat(5));
            c.polygon_lit(&[(x + 3, foot - 20), (x + bw - 4, foot - 20), (x + bw - 4, foot - 16), (x + 3, foot - 16)], k.body, 100, Z::flat(6));
            c.retone(k.body, super::HARD);
            band(c, x, x + bw - 1, foot - 10, k.body, 5);
            c.hline(x + 2, x + bw - 3, foot - 21, k.body.at(Tone::Light), 7);
            Stand::Up(&[])
        }
        "coffin" => {
            // A coffin lid seen from above: the six-sided board, its planks lengthways, brass
            // handles on the sides; open, the lid askew and the dark inside.
            let (cx, top, len) = (w / 2, foot - (k.h - 4).min(44), (k.h - 4).min(44));
            let pts = |dx: i32, y0: i32| {
                vec![
                    (cx - 5 + dx, y0),
                    (cx + 4 + dx, y0),
                    (cx + 8 + dx, y0 + len / 4),
                    (cx + 5 + dx, y0 + len - 1),
                    (cx - 6 + dx, y0 + len - 1),
                    (cx - 9 + dx, y0 + len / 4),
                ]
            };
            ao(c, cx - 9, cx + 8, foot, 5);
            let body = pts(0, top);
            c.polygon_lit(&body, k.body, 60, Z::new(2, 6));
            c.retone(k.body, super::HARD);
            if open {
                let mut m = Canvas::new(c.w(), c.h());
                m.polyline_fill(&pts(0, top), Ix::INK, 1);
                let mut inner = Canvas::new(c.w(), c.h());
                inner.polyline_fill(&pts(0, top), Ix::INK, 1);
                for y in top + 2..top + len - 2 {
                    for x in cx - 7..cx + 7 {
                        if m.get(x - 1, y).is_opaque() && m.get(x + 1, y).is_opaque() && m.get(x, y - 1).is_opaque() {
                            c.put(x, y, k.body.at(Tone::Deep), crate::canvas::FLAT, 3);
                        }
                    }
                }
                c.polygon_lit(&pts(6, top + 4)[..], k.body, 60, Z::new(6, 8));
            } else {
                for x in [cx - 3, cx + 2] {
                    c.vline(x, top + 2, top + len - 3, k.body.at(Tone::Shade), 7);
                }
                c.fill_rect(Rect::new(cx - 2, top + len / 4, 3, 1), k.accent.at(Tone::Light), 7);
                c.fill_rect(Rect::new(cx - 1, top + len / 4 - 1, 1, 3), k.accent.at(Tone::Light), 7);
            }
            Stand::Flat(6)
        }
        "tin" => {
            // A tin dish on the ground, its rim lit and its inside in shade (the dog's bowl).
            let r = Rect::new(w / 2 - 5, foot - 7, 10, 6);
            c.ao_contact(Rect::new(r.x - 1, r.y + 2, r.w + 2, r.h), 1);
            c.ellipse(r, k.body.at(Tone::Light), 2);
            c.ellipse(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), k.body.at(Tone::Shade), 2);
            c.hline(r.x + 2, r.right() - 3, r.y + 3, k.body.at(Tone::Mid), 2);
            c.dot(r.x + 2, r.y, k.body.at(Tone::High), 2);
            Stand::Flat(2)
        }
        "bowl" => {
            // A wooden bowl heaped with apples.
            let (x, bw) = (w / 2 - 9, 18);
            ao(c, x, x + bw - 1, foot, 4);
            c.polygon_lit(&[(x, foot - 7), (x + bw - 1, foot - 7), (x + bw - 4, foot - 1), (x + 3, foot - 1)], k.body, 100, Z::new(2, 4));
            c.retone(k.body, super::HARD);
            c.hline(x, x + bw - 1, foot - 7, k.body.at(Tone::Light), 5);
            for (i, (ax, ay)) in [(x + 3, foot - 11), (x + 8, foot - 12), (x + 12, foot - 10), (x + 6, foot - 9)].into_iter().enumerate() {
                let ramp = if i % 3 == 1 { Ramp::Leaf } else { k.accent };
                c.disc_lit(ax + 1, ay + 1, 2, ramp, Z::flat(6));
            }
            c.retone(k.accent, super::HARD);
            Stand::Up(&[])
        }
        "bottles" | "mug" | "parcel" => {
            small(c, k);
            Stand::Up(&[])
        }
        _ => return None,
    })
}

/// Battens round a crate's face or top: a frame of darker boards, lit on the light's edges.
fn batten(c: &mut Canvas, k: &Kit, r: Rect, top: bool) {
    let z = if top { 6 } else { 5 };
    let (lit, dark) = (k.trim.at(Tone::Light), k.trim.at(Tone::Shade));
    c.fill_normal(Rect::new(r.x, r.y, r.w, 2), k.trim.at(Tone::Base), if top { crate::canvas::FLAT } else { parts::south() }, z);
    c.hline(r.x, r.right() - 1, r.y, lit, z);
    c.fill_rect(Rect::new(r.x, r.bottom() - 2, r.w, 2), k.trim.at(Tone::Mid), z);
    c.fill_rect(Rect::new(r.x, r.y, 2, r.h), k.trim.at(Tone::Base), z);
    c.vline(r.x, r.y, r.bottom() - 1, lit, z);
    c.fill_rect(Rect::new(r.right() - 2, r.y, 2, r.h), dark, z);
    let _ = normal;
}

/// Small things at prop scale: a mug, bottles, a parcel.
fn small(c: &mut Canvas, k: &Kit) {
    let (w, foot) = (k.w, k.foot());
    let cx = w / 2;
    c.ao_contact(Rect::new(cx - 5, foot - 3, 10, 4), 1);
    match k.look.shape {
        "mug" => {
            c.polygon_lit(&[(cx - 3, foot - 7), (cx + 2, foot - 7), (cx + 2, foot - 1), (cx - 3, foot - 1)], k.body, 90, Z::new(2, 3));
            c.hline(cx - 3, cx + 2, foot - 7, k.body.at(Tone::Light), 4);
            c.line((cx + 3, foot - 6), (cx + 4, foot - 3), k.body.at(Tone::Base), 1, 3);
        }
        "bottles" => {
            for (i, x) in [cx - 4, cx, cx + 3].into_iter().enumerate() {
                let t = foot - 9 + (i as i32 % 2);
                c.polygon_lit(&[(x - 1, t), (x + 1, t), (x + 1, foot - 1), (x - 1, foot - 1)], k.body, 100, Z::new(2, 4));
                c.fill_rect(Rect::new(x - 1, t - 2, 2, 2), k.accent.at(Tone::Light), 5);
            }
        }
        _ => {
            let r = Rect::new(cx - 5, foot - 7, 10, 6);
            let (top, _) = box3(c, r.x, r.w, foot - 1, 4, 3, k.body, None, 2);
            c.vline(cx, top.y, foot - 1, k.trim.at(Tone::Base), 4);
            c.hline(r.x, r.right() - 1, top.y + 1, k.trim.at(Tone::Base), 4);
        }
    }
}
