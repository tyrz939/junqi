//! What is not flesh and cloth (ART.md §2.1 `skin`, `ghost`, `extras`; §8 step 7): the bee
//! veil and the diver's helmet, the small extras (a watch chain, a ring of keys, a bell on an
//! ankle, iron over the fists, a soaking), and the passes that make a person out of another
//! material: plate armour in lames, a statue cracked and grown with lichen, a waxwork's sheen
//! and its drip, a gilt man's glint, and a shade that is cold and comes apart below the waist.

use jane_core::grid::Rect;
use jane_data::{Extra, Hat, Skin};

use super::draw::{CX, Rig, relief};
use super::pose::Facing;
use super::{AY, Dress};
use crate::canvas::{Canvas, Z, bresenham};
use crate::palette::{Ix, Ramp, Tone};

/// A bee veil or a diver's helmet, from whichever way the frame faces.
pub(super) fn hood(c: &mut Canvas, d: &Dress, r: &Rig) {
    match d.look.head.hat {
        Hat::Veil => veil(c, d, r),
        Hat::Diving => diving(c, d, r),
        _ => {}
    }
}

/// A bee veil: a pale brimmed hat, and a black net from its brim to the collar with the face a
/// shadow behind it.
fn veil(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    let side = r.facing == Facing::Side;
    let (x0, x1) = (s.x - 1 + i32::from(side), s.right());
    let (top, bot) = (s.y + 2, r.top);
    // What is behind the net sits in its shade.
    c.shade(Rect::new(x0, top, x1 - x0 + 1, bot - top + 2), d.skin, 1);
    c.shade(Rect::new(x0, top, x1 - x0 + 1, bot - top + 2), d.hair, 1);
    let net = Ramp::ClothBlack;
    // The net hangs a px wider at the collar; its lines every other column, a band at the hem.
    for y in top..=bot {
        let flare = i32::from(y > (top + bot) / 2);
        for x in (x0 - flare..=x1 + flare).filter(|x| (x - x0).rem_euclid(3) == 0) {
            c.dot(x, y, net.at(Tone::Light), z.lo);
        }
        c.dot(x0 - flare, y, net.at(Tone::Base), z.lo);
        c.dot(x1 + flare, y, net.at(Tone::Base), z.lo);
    }
    c.hline(x0 - 1, x1 + 1, bot, net.at(Tone::Base), z.lo);
    c.hline(x0 - 1, x1 + 1, bot + 1, net.at(Tone::Shade), z.lo);
    // The hat: a crown and a wide brim, lit on its top.
    let lean = i32::from(side);
    c.ellipse_lit(Rect::new(s.x + 1 + lean, s.y - 5, s.w - 2, 8), h, z);
    c.hline(s.x + 1 + lean, s.right() - 2 + lean, s.y + 1, h.at(Tone::Shade), z.hi);
    c.ellipse_lit(Rect::new(s.x - 4, s.y + 1, s.w + 8, 3), h, Z::new(z.hi, z.hi + 1));
}

/// A diver's helmet: a brass sphere on a breastplate, bolted, with a round glass port on the
/// face (lit from inside when the look says its glass emits) and one on the near side.
fn diving(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    let ey = r.eye_y();
    let lit = d.look.emits.contains(&jane_data::EmitRole::Glass);
    // The breastplate over the shoulders, its rim of bolts.
    let (bx, bw) = (CX - 7 + r.lean, 14);
    c.rect_round(Rect::new(bx, r.top - 2, bw, 4), h, 1, 2, Z::new(z.lo, z.lo + 1));
    for k in 0..4 {
        c.dot(bx + 2 + 3 * k, r.top, h.at(Tone::Shade), z.lo + 1);
    }
    // The sphere, a px wider than the head all round.
    let ball = Rect::new(s.x - 2, s.y - 3, s.w + 4, s.h + 3);
    c.ellipse_lit(ball, h, z);
    c.dot(ball.x + 3, ball.y + 2, h.at(Tone::Glint), z.hi);
    let port = |c: &mut Canvas, x: i32, y: i32, w: i32| {
        c.ellipse(Rect::new(x - 1, y - 1, w + 2, 6), h.at(Tone::Light), z.hi);
        c.hline(x, x + w - 1, y + 4, h.at(Tone::Shade), z.hi);
        c.set_emitting(lit);
        let glass = if lit { Ramp::GlassLit } else { Ramp::Glass };
        c.ellipse(Rect::new(x, y, w, 4), glass.at(if lit { Tone::Light } else { Tone::Shade }), z.hi + 1);
        c.dot(x + 1, y + 1, glass.at(Tone::High), z.hi + 1);
        c.set_emitting(false);
    };
    let cx = r.cx();
    match r.facing {
        Facing::Down | Facing::DownRight => {
            port(c, cx - 3, ey - 2, 6);
            for (x, y) in [(cx - 5, ey - 2), (cx + 4, ey - 2), (cx - 5, ey + 2), (cx + 4, ey + 2)] {
                c.dot(x, y, h.at(Tone::Deep), z.hi);
            }
        }
        Facing::Side => {
            port(c, s.right() - 3, ey - 2, 4);
            port(c, s.x + 2, ey - 1, 3);
        }
        Facing::Up | Facing::UpRight => {
            // The air line's union at the back, and a valve on the crown.
            c.disc_lit(cx - 1, ey + 1, 1, Ramp::Iron, Z::flat(z.hi + 1));
            c.dot(cx - 1, ball.y, Ramp::Iron.at(Tone::Light), z.hi + 1);
        }
    }
}

/// The extras drawn over a finished body: a watch chain, a ring of keys, a bell on the ankle.
pub(super) fn extras(c: &mut Canvas, d: &Dress, r: &Rig) {
    let x = &d.look.extras;
    let (wy, hip) = (r.waist, r.hip);
    let z = relief::FRONT + 2;
    let brass = Ramp::Brass;
    if x.contains(&Extra::WatchChain) {
        let cx = r.cx();
        match r.facing {
            Facing::Down | Facing::DownRight => {
                for (px, py) in [(cx - 1, wy - 1), (cx, wy), (cx + 1, wy), (cx + 2, wy), (cx + 3, wy - 1)] {
                    c.dot(px, py, brass.at(Tone::Light), z);
                }
                c.dot(cx + 1, wy, brass.at(Tone::High), z);
            }
            Facing::Side => {
                let fx = CX + r.side_w() / 2 - 2 + r.lean;
                c.dot(fx - 1, wy - 1, brass.at(Tone::Light), z);
                c.dot(fx, wy, brass.at(Tone::High), z);
            }
            Facing::Up | Facing::UpRight => {}
        }
    }
    if x.contains(&Extra::Keys) {
        // A ring on the belt and three keys hanging from it, a beat behind the stride.
        let kx = match r.facing {
            Facing::Down | Facing::DownRight => CX + 4,
            Facing::Side => CX - 2 + r.lean,
            Facing::Up | Facing::UpRight => CX - 6,
        };
        let ky = hip - 2;
        let iron = Ramp::Iron;
        for (px, py) in [(kx, ky), (kx + 1, ky - 1), (kx + 2, ky), (kx + 1, ky + 1)] {
            c.dot(px, py, iron.at(Tone::Light), z);
        }
        let sway = i32::from(r.pose.phase >= 32768);
        for k in 0..3 {
            let (px, py) = (kx + k + sway - 1 + i32::from(k == 0), ky + 2);
            c.vline(px, py, py + 1 + i32::from(k == 1), iron.at(if k == 1 { Tone::High } else { Tone::Base }), z);
        }
    }
    if x.contains(&Extra::BellAnkle) {
        // A little brass bell tied at the near ankle: it goes where the leg goes.
        let rows = if d.look.body.boots == jane_data::Boots::Boots { 3 } else { 2 };
        let (bx, foot) = r.near_foot();
        let by = foot - rows - 2;
        c.fill_rect(Rect::new(bx, by, 2, 2), brass.at(Tone::Base), relief::BOOT.hi + 1);
        c.dot(bx, by, brass.at(Tone::High), relief::BOOT.hi + 1);
        c.dot(bx + 1, by + 1, brass.at(Tone::Shade), relief::BOOT.hi + 1);
    }
}

/// Iron over the fists: a band across the knuckles, lit on top, with its rivets.
pub(super) fn knuckles(c: &mut Canvas, d: &Dress, x: i32, y: i32, z: u8) {
    if !d.look.extras.contains(&Extra::Knuckles) {
        return;
    }
    let iron = Ramp::Iron;
    c.fill_rect(Rect::new(x, y, 4, 2), iron.at(Tone::Base), z + 1);
    c.hline(x, x + 3, y, iron.at(Tone::Light), z + 1);
    c.dot(x + 1, y, iron.at(Tone::Glint), z + 1);
    c.dot(x + 3, y + 1, iron.at(Tone::Shade), z + 1);
}

/// Soaked to the waist: the cloth and the skin below the hip a tone darker.
pub(super) fn wet(c: &mut Canvas, d: &Dress, r: &Rig) {
    if !d.look.extras.contains(&Extra::Wet) {
        return;
    }
    for y in r.hip..AY {
        for x in 0..c.w() {
            if let Some((ramp, t)) = Ramp::of(c.get(x, y)) {
                if [d.coat, d.legs, d.skin, d.front].contains(&ramp) {
                    c.recolour(x, y, ramp.at(t.step(-1)));
                }
            }
        }
    }
}

/// `ramp`'s pixels on the line through `pts` to `tone`, and the px left of each to `lip`: a
/// crack with its lit edge, a seam between plates.
fn score(c: &mut Canvas, ramp: Ramp, pts: &[(i32, i32)], tone: Tone, lip: Option<Tone>) {
    for w in pts.windows(2) {
        bresenham(w[0].0, w[0].1, w[1].0, w[1].1, |x, y| {
            c.tint(x, y, ramp, tone);
            if let Some(l) = lip {
                c.tint(x - 1, y, ramp, l);
            }
        });
    }
}

/// The material passes, after the cloth and skin are toned and before the outline: plate in
/// lames, stone cracked and grown with lichen, wax with its sheen and a drip, gilt's glint.
pub(super) fn material(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let ey = r.eye_y();
    let front = !r.facing.back();
    let t = i32::from(r.facing == Facing::DownRight);
    if d.coat == Ramp::Iron || d.look.head.skin == Skin::Metal {
        // Plate: a lame every four rows, the gap in shade and the next plate's top edge in
        // lift, a rivet at the lit end of it.
        let iron = Ramp::Iron;
        for y in (r.top + 2..AY - 3).filter(|y| (y - r.top) % 4 == 2) {
            let row: Vec<i32> =
                (0..c.w()).filter(|&x| matches!(Ramp::of(c.get(x, y)), Some((q, _)) if q == iron)).collect();
            let (Some(&a), Some(&b)) = (row.first(), row.last()) else { continue };
            for &x in &row {
                c.tint(x, y, iron, Tone::Shade);
                c.tint(x, y + 1, iron, Tone::Lift);
            }
            if b - a > 5 {
                c.tint(a + 1, y + 1, iron, Tone::High);
            }
        }
    }
    if d.look.head.skin == Skin::Metal && front {
        // A closed helm: the sight a dark slit across the face (the eyes' light inside it) and
        // a lit ridge down its middle.
        let (x0, x1) =
            if r.facing == Facing::Side { (s.right() - 6, s.right()) } else { (s.x + 1 + t, s.right() - 2 + t) };
        for y in [ey, ey + 1] {
            for x in x0..=x1 {
                if c.emissive_at(x, y) == Ix::CLEAR {
                    c.tint(x, y, Ramp::Iron, Tone::Deep);
                }
            }
        }
        let rx = if r.facing == Facing::Side { s.right() - 1 } else { r.cx() - 1 };
        for y in ey + 2..s.bottom() {
            c.tint(rx, y, Ramp::Iron, Tone::Light);
        }
        c.tint(rx, ey - 1, Ramp::Iron, Tone::Light);
    }
    match d.look.head.skin {
        Skin::Stone => {
            let st = Ramp::Stone;
            let l = r.lean;
            score(
                c,
                st,
                &[(CX + 2 + l, r.top + 2), (CX + 3 + l, r.top + 5), (CX + 2 + l, r.top + 7), (CX + 4 + l, r.top + 10)],
                Tone::Deep,
                Some(Tone::Light),
            );
            score(
                c,
                st,
                &[(CX - 4, r.hip + 3), (CX - 2, r.hip + 6), (CX - 3, r.hip + 9)],
                Tone::Deep,
                Some(Tone::Light),
            );
            score(c, st, &[(s.x + 2, s.y + 1), (s.x + 3, s.y + 3)], Tone::Shade, None);
            // Lichen where the rain sits: on the shoulders and along the hem.
            let lichen = Ramp::LeafOlive;
            for (x, y, w) in [(CX - 6 + l, r.top, 3), (CX + 2 + l, AY - 5, 3), (CX - 3, AY - 4, 2)] {
                for (k, px) in (x..x + w).enumerate() {
                    for py in [y, y + 1] {
                        if matches!(Ramp::of(c.get(px, py)), Some((q, _)) if q == st) {
                            c.recolour(
                                px,
                                py,
                                lichen.at(if (k + (py - y) as usize) % 2 == 0 { Tone::Base } else { Tone::Shade }),
                            );
                        }
                    }
                }
            }
        }
        Skin::Wax => {
            // A sheen on the brow and the near cheek: skin that has never been warm.
            let w = Ramp::Plaster;
            if front {
                let (fx, cx) =
                    if r.facing == Facing::Side { (s.right() - 4, s.right() - 3) } else { (s.x + 3 + t, s.x + 2 + t) };
                c.tint(fx, s.y + 2, w, Tone::High);
                c.tint(fx + 1, s.y + 2, w, Tone::High);
                c.tint(cx, ey + 2, w, Tone::Glint);
            }
        }
        Skin::Gilt => {
            // Gold is bright: skin's four tones each a step up, a glint on the brow.
            let g = Ramp::Brass;
            for y in s.y - 1..AY {
                for x in 0..c.w() {
                    if let Some((q, t)) = Ramp::of(c.get(x, y)) {
                        if q == g && c.emissive_at(x, y) == Ix::CLEAR && d.skin == g {
                            c.recolour(x, y, g.at(t.step(1)));
                        }
                    }
                }
            }
            if front {
                c.tint(s.x + 3, s.y + 2, g, Tone::Glint);
            }
        }
        _ => {}
    }
}

/// A shade (`ghost`): every ramp to a cold blue-grey (`slate`), tone for tone, no contact shadow, and below the hip it comes apart: a 50 % checker into tongues
/// of different lengths, then nothing. Its eyes keep their light. Its heights halve, so its
/// shadow is faint. Run last.
pub(super) fn ghost(c: &mut Canvas, r: &Rig) {
    let mist = Ramp::Slate;
    for y in 0..c.h() {
        for x in 0..c.w() {
            let ix = c.get(x, y);
            if ix == Ix::AO {
                c.clear_px(x, y);
                continue;
            }
            if !ix.is_opaque() || c.emissive_at(x, y) != Ix::CLEAR {
                continue;
            }
            let t = Ramp::of(ix).map_or(Tone::Deep, |(_, t)| t);
            c.recolour(x, y, mist.at(t));
        }
    }
    let wy = r.hip;
    for x in 0..c.w() {
        // Each column's tongue: its own length, fixed to the body so it does not crawl.
        let k = (x - r.lean).rem_euclid(7);
        let cut = wy + 5 + (k * 5) % 7;
        for y in wy + 1..c.h() {
            let keep = y < cut && (y < wy + 3 || (x + y) % 2 == 0);
            if !keep && c.emissive_at(x, y) == Ix::CLEAR {
                c.clear_px(x, y);
            }
        }
    }
    c.scale_heights(1, 2);
}

/// A seated person's frame (`extras: seated`): facing out of the chair (from the side too: she
/// turns in it to look), or its back; the body a seat lower, the feet together, the hands in
/// the lap. The walk's beat rocks the chair: `(pose, the back's lean)`.
pub(super) fn seat(facing: Facing, pose: super::Pose) -> (Facing, super::Pose, i32) {
    let k = usize::from(pose.phase) * 6 / 65536;
    let (dy, lean) = if pose.breathe { (0, 0) } else { [(0, 0), (0, 1), (0, 2), (0, -1), (0, -2), (0, -3)][k % 6] };
    let facing = if facing.back() { Facing::Up } else { Facing::Down };
    let p = super::Pose {
        bob: 2 + dy,
        arm: [0, 0],
        leg: [0, 0],
        lift: [0, 0],
        lean: 0,
        splay: [0, 0],
        raise: [3, 3],
        ..pose
    };
    (facing, p, lean)
}

/// The rocking chair's back: two posts, a curved top rail, spindles between, the back leaning
/// with the rock. Behind her facing the viewer; over her from behind.
pub(super) fn chair_back(c: &mut Canvas, r: &Rig, lean: i32) {
    let wood = Ramp::WoodDark;
    let top = r.top - 9;
    let seat = r.hip + 1;
    let z = Z::new(1, 3);
    for x in [CX - 9, CX + 7] {
        c.line((x + lean, top), (x, seat), wood.at(Tone::Base), 2, z.hi);
        c.dot(x + lean, top, wood.at(Tone::Light), z.hi);
    }
    // The top rail, bowed: its ends a px lower than its middle.
    c.line((CX - 9 + lean, top + 1), (CX - 3 + lean, top - 1), wood.at(Tone::Light), 2, z.hi);
    c.line((CX - 3 + lean, top - 1), (CX + 2 + lean, top - 1), wood.at(Tone::Light), 2, z.hi);
    c.line((CX + 2 + lean, top - 1), (CX + 8 + lean, top + 1), wood.at(Tone::Base), 2, z.hi);
    for x in [CX - 5, CX - 2, CX + 1, CX + 4] {
        c.line((x + lean, top + 1), (x, seat), wood.at(Tone::Shade), 1, z.lo);
        c.dot(x + lean, top + 2, wood.at(Tone::Base), z.lo);
    }
}

/// The rockers: a bow along the floor under the chair, rising at the ends; drawn before her, so
/// her feet stand in front of them.
pub(super) fn chair_rockers(c: &mut Canvas) {
    let rock = Ramp::WoodDark;
    c.line((CX - 12, AY - 5), (CX - 9, AY - 3), rock.at(Tone::Base), 2, relief::FAR.lo);
    c.line((CX - 9, AY - 3), (CX + 8, AY - 3), rock.at(Tone::Base), 2, relief::FAR.lo);
    c.line((CX + 8, AY - 3), (CX + 11, AY - 5), rock.at(Tone::Base), 2, relief::FAR.lo);
    c.hline(CX - 9, CX + 8, AY - 3, rock.at(Tone::Light), relief::FAR.lo);
}

/// The chair's arms either side of her and its front legs; facing the viewer, the knitting in
/// her lap: two needles and a ball of the wool.
pub(super) fn chair_front(c: &mut Canvas, d: &Dress, r: &Rig, facing_us: bool) {
    let wood = Ramp::WoodDark;
    let z = relief::ARM.hi + 1;
    let arm_y = r.waist + 1;
    for (x, lit) in [(CX - 11, true), (CX + 9, false)] {
        c.fill_rect(Rect::new(x, arm_y, 3, 2), wood.at(if lit { Tone::Light } else { Tone::Base }), z);
        c.line((x + 1, arm_y + 2), (x + 1, AY - 4), wood.at(Tone::Shade), 2, z);
    }
    if facing_us {
        // The knitting: a ball of wool in the lap and the needles crossed over it.
        let (kx, ky) = (CX + 2, r.hip - 1);
        let _ = d;
        c.disc_lit(kx, ky, 2, Ramp::ClothRose, Z::flat(relief::FRONT + 3));
        // Knitting (ART-PLAN Q4): on the off beats the near needle's tip comes up a px and the
        // far one's in, a stitch being made.
        let w = i32::from(r.pose.task != 0 && r.pose.task % 2 == 0);
        c.line((kx - 5, ky - 3 - w), (kx - 1, ky + 1), Ramp::Iron.at(Tone::Light), 1, relief::FRONT + 4);
        c.line((kx - 5 + w, ky + 1), (kx - 1, ky - 3), Ramp::Iron.at(Tone::Light), 1, relief::FRONT + 4);
    }
}
