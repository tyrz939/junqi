//! Work (ART-PLAN Q4): what a townsperson does while standing at it, a loop of up to four beats
//! drawn by the composer for a look that names a `task` and played by the presenter while the
//! schedule row is the one the work belongs to. Presentation only.
//!
//! - `sweep`: from the side, both hands on the broom's haft, leaning into the stroke, its head
//!   pushed out along the ground and drawn back ([`super::held`] draws the broom where the beat
//!   puts it).
//! - `read`: a newspaper held open at the chest, the eyes lowered to it; the right hand takes
//!   the page up, stands it on the fold and lays it over.
//! - `bottles`: a milk bottle by its neck, bent to the step, set down, and up again with the
//!   bottle left on the step.
//! - `knit`: in her rocking chair, the chair rocking and the needles working
//!   ([`super::special::chair_front`] draws the needles by the beat).

use jane_core::grid::Rect;
use jane_data::Task;

use super::draw::{Rig, hand, hand_front, relief};
use super::pose::{Facing, Pose, STAND};
use super::{AY, Dress};
use crate::canvas::Canvas;
use crate::palette::{Ramp, Tone};
use crate::sprite::FrameId;

/// Newsprint and milk: a warm off-white that is no one's coat.
const PAPER: Ramp = Ramp::Plaster;

/// The beats of a look's work, with their facing and pose: none for no task.
pub(crate) fn frames(task: Task) -> Vec<(FrameId, Facing, Pose)> {
    use FrameId as F;
    let down = [F::Task1, F::Task2, F::Task3, F::Task4];
    let beats = |ids: [FrameId; 4], facing: Facing, poses: [Pose; 4]| -> Vec<(FrameId, Facing, Pose)> {
        ids.into_iter().zip(poses).enumerate().map(|(k, (f, p))| (f, facing, Pose { task: k as u8 + 1, ..p })).collect()
    };
    match task {
        Task::None => Vec::new(),
        Task::Sweep => {
            // Both hands on the haft at the waist, the near one forward of the far; the stroke
            // goes out with a lean and comes back. Nothing trails: she is not walking.
            let stroke = |a: i32, lean: i32| Pose { arm: [a, a - 3], raise: [3, 1], lean, lag: (0, lean), ..STAND };
            beats(
                [F::TaskSide1, F::TaskSide2, F::TaskSide3, F::TaskSide4],
                Facing::Side,
                [stroke(1, 1), stroke(3, 2), stroke(5, 2), stroke(3, 1)],
            )
        }
        Task::Read => {
            // The paper at the chest between the hands, the eyes lowered to it; the right hand
            // lifts the page and carries it over to the left.
            let open = Pose { raise: [4, 4], spread: [-3, -3], shut: true, ..STAND };
            let lift = Pose { raise: [4, 6], spread: [-3, -4], ..open };
            let over = Pose { raise: [4, 7], spread: [-3, -7], ..open };
            beats(down, Facing::Down, [open, lift, over, open])
        }
        Task::Bottles => {
            // Down on his knees' bend, the feet set apart, the near hand down to the step (the
            // bottle's foot on the ground on the third beat).
            let bend = |bob: i32, a: i32| Pose { bob, arm: [a, 0], splay: [1, 1], lag: (bob, 0), ..STAND };
            beats(down, Facing::Down, [STAND, bend(2, 0), bend(3, -2), STAND])
        }
        Task::Knit => {
            // The rock: forward, back, forward, back past upright (`special::seat` reads the
            // walk's phase for the rock: sixths of a turn).
            let rock = |k: u32| Pose { phase: (k * 65536 / 6 + 1) as u16, ..STAND };
            beats(down, Facing::Down, [rock(0), rock(1), rock(0), rock(3)])
        }
    }
}

/// What the work puts in the hands this beat, over the finished body: the newspaper, the milk
/// bottle.
pub(super) fn draw(c: &mut Canvas, d: &Dress, r: &Rig) {
    let beat = r.pose.task;
    if beat == 0 {
        return;
    }
    match d.look.task {
        Task::Read => paper(c, d, r, beat),
        Task::Bottles => {
            let (hx, hy) = hand_front(r, 0);
            // In the fist by its neck, out past his side so it shows against the coat, until it
            // stands on the step; it stays there as he rises.
            let x = hx - 2;
            if beat <= 2 {
                bottle(c, x, hy + 2);
                hand(c, d, hx, hy, relief::ARM.hi);
            } else {
                bottle(c, x, AY - 5);
                if beat == 3 {
                    hand(c, d, hx, hy, relief::ARM.hi);
                }
            }
        }
        Task::None | Task::Sweep | Task::Knit => {}
    }
}

/// A milk bottle with its cap on row `y`: a foil cap, then the glass full of milk, lit down its
/// left.
fn bottle(c: &mut Canvas, x: i32, y: i32) {
    let z = relief::FRONT + 3;
    c.fill_rect(Rect::new(x, y, 3, 1), Ramp::Iron.at(Tone::Light), z);
    c.fill_rect(Rect::new(x, y + 1, 3, 5), PAPER.at(Tone::Light), z);
    c.vline(x, y + 2, y + 4, PAPER.at(Tone::High), z);
    c.vline(x + 2, y + 1, y + 5, PAPER.at(Tone::Base), z);
}

/// The newspaper held open in both hands, the hands behind it and a thumb over each edge; on
/// beats two and three the right hand carries a page over, on four the left page is the fresh
/// one.
fn paper(c: &mut Canvas, d: &Dress, r: &Rig, beat: u8) {
    let ((lx, ly), (rx, ry)) = (hand_front(r, 0), hand_front(r, 1));
    let z = relief::FRONT + 3;
    // Open across both hands: the fold in the middle, the left page lit and the right a tone
    // down (turned a little from the light).
    let (x0, y0, h) = (lx, ly - 1, 8);
    let x1 = x0 + 13;
    let fold = x0 + 6;
    c.fill_rect(Rect::new(x0, y0, fold - x0, h), PAPER.at(Tone::Light), z);
    c.fill_rect(Rect::new(fold + 1, y0, x1 - fold, h), PAPER.at(Tone::Base), z);
    c.vline(fold, y0, y0 + h - 1, PAPER.at(Tone::Shade), z);
    // The print in words: a headline across the left page, then lines of type a tone into the
    // paper; the fresh page's lines fall differently.
    let (ink, type_l, type_r) = (PAPER.at(Tone::Shade), PAPER.at(Tone::Base), PAPER.at(Tone::Mid));
    let fresh = i32::from(beat == 4);
    c.hline(x0 + 1, x0 + 3, y0 + 2, ink, z);
    c.hline(x0 + 5 - fresh, fold - 1, y0 + 2, ink, z);
    for (k, y) in [y0 + 4, y0 + 6].into_iter().enumerate() {
        let cut = x0 + 2 + (k as i32 + fresh) % 2;
        c.hline(x0 + 1, cut, y, type_l, z);
        c.hline(cut + 2, fold - 1, y, type_l, z);
        c.hline(fold + 2, fold + 3 + k as i32, y, type_r, z);
        c.hline(fold + 5 + k as i32, x1 - 1, y, type_r, z);
    }
    c.hline(fold + 2, x1 - 1, y0 + 2, type_r, z);
    match beat {
        2 => {
            // The page's corner taken up: its outer edge comes in and up, lit as it rises, the
            // next page showing past it in shade.
            c.fill_rect(Rect::new(x1 - 2, y0, 3, h), PAPER.at(Tone::Shade), z + 1);
            c.fill_rect(Rect::new(fold + 1, y0 - 1, x1 - fold - 3, h), PAPER.at(Tone::Light), z + 1);
            c.vline(x1 - 3, y0 - 1, y0 + h - 2, PAPER.at(Tone::High), z + 1);
        }
        3 => {
            // The page stood up on the fold, edge on: a lit strip over the paper, the next page
            // showing beside it.
            c.fill_rect(Rect::new(fold + 1, y0, x1 - fold, h), PAPER.at(Tone::Base), z);
            c.fill_rect(Rect::new(fold, y0 - 4, 2, h + 3), PAPER.at(Tone::Light), z + 1);
            c.vline(fold, y0 - 4, y0 + h - 2, PAPER.at(Tone::High), z + 1);
        }
        _ => {}
    }
    // The thumbs over its edges (the right one on the page it carries).
    for (x, y) in [(lx + 1, ly), (rx + 1, ry)] {
        c.fill_rect(Rect::new(x, y, 2, 2), d.skin.at(Tone::Base), relief::ARM.hi + 3);
        c.dot(x, y, d.skin.at(Tone::Lift), relief::ARM.hi + 3);
    }
}
