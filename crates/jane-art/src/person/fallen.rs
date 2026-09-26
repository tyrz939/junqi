//! `fallen`, every person's dead frame (ART.md §4.1): a frame drawn in the fallen pose, turned a quarter
//! anticlockwise (so she lies on her back, head to the west, face up), repeated columns taken
//! out until the body is at most 28 px long, dropped so its lowest row is `ay - 2`, pallor
//! applied, and a muted pool under it at 38 % of its length from the head. Heights fall to the
//! body's thickness, so its cast shadow is a sliver. A dead frame never emits.

use jane_core::grid::Rect;

use super::{AX, AY, H, W};
use crate::canvas::Canvas;
use crate::palette::{Ix, Ramp, Tone, pallor};

/// The longest a fallen body lies, px.
pub const MAX_LEN: i32 = 28;

/// The dead frame made from `side`, a finished frame of the body in the fallen pose (32 x 40).
pub fn fallen(side: &Canvas, seed: u32) -> Canvas {
    let mut stood = side.clone();
    for y in 0..stood.h() {
        for x in 0..stood.w() {
            if stood.get(x, y) == Ix::AO {
                stood.clear_px(x, y);
            }
        }
    }
    let mut lying = stood.rotate_ccw();
    lying.shorten_to(MAX_LEN);
    lying.remap(pallor);
    lying.quench();
    lying.dome_heights(5);
    let Some(b) = lying.bounds() else { return Canvas::new(W, H) };
    // Where the body goes: centred on the anchor, its lowest row on ay - 2.
    let (x, y) = (AX - b.w / 2 - b.x, AY - 2 - (b.bottom() - 1));
    let mut out = Canvas::new(W, H);
    // The pool: under the body, 38 % of the way from the head, spreading toward the viewer;
    // lined in its own dark, then the body laid over it with the lines it stood up with.
    let px = AX - b.w / 2 + b.w * 38 / 100;
    let pw = 14 + (seed & 3) as i32;
    // Under the body's middle row, spreading a little further toward the viewer than away.
    let mid = y + b.y + b.h / 2;
    let pool = Rect::new(px - pw / 2, mid - 3, pw, (AY + 1 - (mid - 3)).max(9));
    out.ellipse_lit(pool, Ramp::Pool, crate::canvas::Z::flat(1));
    out.retone(
        Ramp::Pool,
        [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Mid, Tone::Base, Tone::Base, Tone::Base],
    );
    out.outline();
    out.ao_contact(Rect::new(AX - b.w / 2 - 1, AY - 4, b.w + 2, 4), 0);
    out.stamp(&lying, x, y);
    out
}
