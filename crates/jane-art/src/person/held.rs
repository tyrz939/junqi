//! Held things (ART.md §2.1 `held`, §8 step 7): drawn by the composer at the hand each frame
//! puts where it is, so a lantern swings with the walk and a hammer comes up with the wind-up.
//! The near hand holds (the screen-left hand facing the viewer or away); west mirrors it, the
//! hand with it. A lantern emits when the look says its held thing does; a pipe is at the
//! mouth, not the hand.

use jane_core::grid::Rect;
use jane_data::{EmitRole, HeldItem};

use super::Dress;
use super::draw::Rig;
use super::pose::Facing;
use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};

/// Relief: a held thing stands in front of the arm that holds it.
const Z_HELD: u8 = 9;

/// The held thing in the hand whose top-left is `(hx, hy)` (a 4 x 3 mitt).
pub(super) fn draw(c: &mut Canvas, d: &Dress, r: &Rig, hx: i32, hy: i32) {
    let z = Z_HELD;
    // On a diagonal a hafted thing leans the way she faces, as from the side.
    let side = r.facing == Facing::Side || r.facing.diagonal();
    // Facing away, a thing in her hand is partly behind her: drawn a relief lower.
    let z = if r.facing.back() { z - 4 } else { z };
    let wood = Ramp::WoodOak;
    match d.look.held {
        HeldItem::None | HeldItem::Pipe => {}
        HeldItem::Lantern => {
            // A hand lantern: a bail from the fist, a cap, four panes about a flame.
            let (x, y) = (hx + 1 - i32::from(r.pose.phase >= 32768), (hy + 3).min(super::AY - 9));
            c.vline(x + 1, hy + 1, y, Ramp::Iron.at(Tone::Shade), z);
            c.fill_rect(Rect::new(x - 1, y, 5, 2), Ramp::Iron.at(Tone::Base), z);
            c.dot(x - 1, y, Ramp::Iron.at(Tone::Light), z);
            let lit = d.look.emits.contains(&EmitRole::Held);
            c.set_emitting(lit);
            let glass = if lit { Ramp::GlassLit } else { Ramp::Glass };
            c.fill_rect(Rect::new(x - 1, y + 2, 5, 4), glass.at(Tone::Light), z);
            c.fill_rect(Rect::new(x, y + 3, 3, 2), glass.at(Tone::High), z);
            c.set_emitting(false);
            for dx in [-1, 3] {
                c.vline(x + dx, y + 2, y + 5, Ramp::Iron.at(Tone::Shade), z + 1);
            }
            c.hline(x - 1, x + 3, y + 6, Ramp::Iron.at(Tone::Shade), z);
        }
        HeldItem::Hammer => {
            // A hammer: the haft up from the fist, the head across its top.
            let (tx, ty) = if side { (hx + 5, hy - 6) } else { (hx + 1, hy - 8) };
            c.line((hx + 1, hy + 1), (tx, ty), wood.at(Tone::Light), 2, z);
            c.rect_bevel(Rect::new(tx - 3, ty - 2, 6, 3), Ramp::Iron, 1, Z::new(z, z + 1));
        }
        HeldItem::Net => {
            // A butterfly net: a cane held at a slope, a wire hoop at its tip, the gauze bag
            // hanging from the hoop and swinging a px with the stride.
            let (tx, ty) = if side { (hx + 8, hy - 20) } else { (hx - 2, hy - 22) };
            c.line((hx + 1, hy + 2), (tx, ty + 3), wood.at(Tone::Light), 1, z);
            // The hoop is a ring seen a little from above; the bag hangs from its near rim.
            let (x, y) = (tx - 4, ty - 2);
            let gauze = Ramp::ClothLinen;
            let sway = i32::from(r.pose.phase >= 32768);
            c.polyline_fill(
                &[(x + 1, y + 3), (x + 7, y + 3), (x + 5 + sway, y + 9), (x + 3 + sway, y + 9)],
                gauze.at(Tone::Base),
                z - 1,
            );
            c.vline(x + 5 + sway, y + 5, y + 8, gauze.at(Tone::Shade), z - 1);
            c.hline(x + 2, x + 6, y + 4, gauze.at(Tone::Light), z - 1);
            let wire = Ramp::Iron.at(Tone::Light);
            c.hline(x + 2, x + 6, y, wire, z);
            c.hline(x + 2, x + 6, y + 3, wire, z);
            c.vline(x, y + 1, y + 2, wire, z);
            c.vline(x + 8, y + 1, y + 2, wire, z);
            for (px, py) in [(x + 1, y), (x + 7, y), (x + 1, y + 3), (x + 7, y + 3)] {
                c.dot(px, py, wire, z);
            }
        }
        HeldItem::Pole | HeldItem::Broom => {
            // A long pole held upright, its foot on the ground; a broom's head of bristles.
            let x = hx + 1;
            let foot = super::AY - 1;
            c.line((x, hy - 12), (x + i32::from(side), foot - 1), wood.at(Tone::Light), 2, z);
            c.vline(x + 1, hy - 12, foot - 2, wood.at(Tone::Shade), z);
            if d.look.held == HeldItem::Broom {
                let bx = x + i32::from(side) - 2;
                c.polyline_fill(
                    &[(bx, foot - 5), (bx + 5, foot - 5), (bx + 7, foot), (bx - 2, foot)],
                    Ramp::Reed.at(Tone::Base),
                    z,
                );
                for k in 0..3 {
                    c.vline(bx + 1 + 2 * k, foot - 3, foot - 1, Ramp::Reed.at(Tone::Shade), z);
                }
                c.hline(bx, bx + 4, foot - 5, Ramp::Leather.at(Tone::Base), z);
            }
        }
        HeldItem::Suitcase => {
            // A suitcase at her side: leather over a frame, a brass clasp, its handle in her fist.
            // It swings a px with the stride, a beat behind.
            let (x, y) = (hx - 2, (hy + 3).min(super::AY - 9) + i32::from(r.pose.phase >= 32768));
            c.rect_round(Rect::new(x, y, 9, 7), Ramp::Leather, 1, 1, Z::new(z - 1, z));
            c.hline(x + 1, x + 7, y + 1, Ramp::Leather.at(Tone::Light), z);
            c.fill_rect(Rect::new(x + 3, y, 3, 1), Ramp::Brass.at(Tone::Light), z + 1);
            c.hline(x + 1, x + 2, hy + 2, Ramp::Leather.at(Tone::Shade), z);
        }
        HeldItem::Dish => {
            // A dish carried in front: a white rim and what is in it.
            let (x, y) = (hx - 2, hy - 1);
            c.ellipse(Rect::new(x, y, 8, 3), Ramp::ClothLinen.at(Tone::Light), z);
            c.hline(x + 2, x + 5, y + 1, Ramp::ClothBrick.at(Tone::Base), z);
            c.dot(x + 1, y + 2, Ramp::ClothLinen.at(Tone::Shade), z);
        }
        HeldItem::Bell => {
            // A handbell: the handle in her fist, the bell under it.
            let (x, y) = (hx, hy + 2);
            c.polyline_fill(&[(x + 1, y), (x + 3, y), (x + 4, y + 4), (x, y + 4)], Ramp::Brass.at(Tone::Base), z);
            c.dot(x + 1, y + 1, Ramp::Brass.at(Tone::High), z);
            c.hline(x - 1, x + 5, y + 4, Ramp::Brass.at(Tone::Shade), z);
            c.dot(x + 2, y + 5, Ramp::Iron.at(Tone::Shade), z);
        }
        HeldItem::Billhook => {
            // A billhook: a short handle and a curved blade hooked at its tip.
            let (tx, ty) = if side { (hx + 5, hy - 4) } else { (hx + 1, hy - 6) };
            c.line((hx + 1, hy + 1), (tx - 1, ty + 2), wood.at(Tone::Base), 2, z);
            c.polyline_fill(
                &[(tx - 1, ty + 2), (tx + 1, ty - 2), (tx + 3, ty - 3), (tx + 3, ty - 1), (tx + 1, ty + 1)],
                Ramp::Iron.at(Tone::Light),
                z + 1,
            );
            c.dot(tx + 3, ty - 3, Ramp::Iron.at(Tone::High), z + 1);
        }
        HeldItem::Book => {
            // A book under her hand: its cover, its pages' edge pale.
            let (x, y) = (hx - 1, hy - 2);
            c.fill_rect(Rect::new(x, y, 5, 6), Ramp::ClothRed.at(Tone::Base), z);
            c.vline(x, y, y + 5, Ramp::ClothRed.at(Tone::Light), z);
            c.vline(x + 4, y + 1, y + 5, Ramp::ClothLinen.at(Tone::Light), z);
        }
    }
}

/// What is held at the face, drawn over the head: a pipe.
pub(super) fn at_face(c: &mut Canvas, d: &Dress, r: &Rig) {
    if d.look.held != HeldItem::Pipe || r.facing.back() {
        return;
    }
    let s = r.skull;
    let my = r.eye_y() + 5;
    let (x, dir) = if r.facing == Facing::Side { (s.right() - 2, 1) } else { (r.cx(), 1) };
    c.hline(x, x + 3 * dir, my, Ramp::WoodDark.at(Tone::Base), Z_HELD);
    c.fill_rect(Rect::new(x + 3 * dir, my - 2, 2, 3), Ramp::WoodDark.at(Tone::Light), Z_HELD);
    c.dot(x + 3 * dir, my - 3, Ix::SEAM, Z_HELD);
}
