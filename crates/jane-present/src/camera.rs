//! The camera (PRESENTATION.md §1.10): advanced per tick in sim units (`Fx`, 1/256 sim px),
//! `cam += (target - cam) * 0.18`, clamped to the zone, a zone narrower than the view centred;
//! a frame interpolates between the last two tick positions by `alpha` and shifts to canvas px
//! last. Integer throughout, so two machines at the same tick draw the same frame.

use jane_core::Rect;
use jane_core::num::CELL_FX;

use crate::frame::FX_TO_CANVAS;

/// The follow rate, 0.18 in 1/256ths.
const FOLLOW_Q8: i64 = 46;
/// The shake's decay per tick, 0.86 in 1/256ths.
const SHAKE_DECAY_Q8: i32 = 220;
/// Shake is capped at this many canvas px.
const SHAKE_MAX: i32 = 6;
/// Below this (canvas px in 1/256ths, 0.2 px) the shake is still.
const SHAKE_FLOOR: i32 = 51;
/// She is framed a little below centre: her middle, not her feet (10 sim px up).
const LOOK_UP_FX: i32 = 10 * 256;
/// A locked room keeps this much round it in view, sim px.
const LOCK_PAD_FX: i32 = 24 * 256;
/// One period of a sine, 16 steps, in 1/127ths: the shake's wobble.
const SINE16: [i32; 16] = [0, 49, 90, 117, 127, 117, 90, 49, 0, -49, -90, -117, -127, -117, -90, -49];

/// The camera: the top-left of the view in the zone.
#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    /// This tick's top-left, `Fx`.
    pub pos: (i32, i32),
    /// Last tick's, `Fx`.
    pub prev: (i32, i32),
    ready: bool,
    /// Held on a room (cells), from `Event::Camera`.
    pub lock: Option<Rect>,
    /// Canvas px in 1/256ths.
    shake_q8: i32,
    /// This tick's shake, canvas px.
    shake_off: (i32, i32),
}

/// The target along one axis: `want` (the view's top-left on her), held on a locked span, then
/// clamped to the zone or centred on a zone narrower than the view. All `Fx`.
fn axis(want: i32, view: i32, zone: i32, lock: Option<(i32, i32)>) -> i32 {
    let mut t = want;
    if let Some((l0, len)) = lock {
        t = if len + 2 * LOCK_PAD_FX <= view {
            l0 + len / 2 - view / 2
        } else {
            t.clamp(l0 - LOCK_PAD_FX, (l0 + len + LOCK_PAD_FX - view).max(l0 - LOCK_PAD_FX))
        };
    }
    if zone <= view { (zone - view) / 2 } else { t.clamp(0, zone - view) }
}

impl Camera {
    /// Forget where it was: the next tick snaps (a new zone).
    pub fn reset(&mut self) {
        self.ready = false;
        self.lock = None;
        self.shake_q8 = 0;
        self.shake_off = (0, 0);
    }

    /// Adds a shake of `amount` canvas px (`Event::Shake`), capped.
    pub fn shake(&mut self, amount: u8) {
        self.shake_q8 = (self.shake_q8 + i32::from(amount) * 256).min(SHAKE_MAX * 256);
    }

    /// One tick: follow `her` (feet, `Fx`) in a zone of `zone` cells seen through a canvas of
    /// `canvas` px. `tick` drives the shake's wobble.
    pub fn tick(&mut self, her: (i32, i32), zone: (u32, u32), canvas: (u16, u16), tick: u32) {
        // A canvas px is 128 Fx.
        let (vw, vh) = (i32::from(canvas.0) << FX_TO_CANVAS, i32::from(canvas.1) << FX_TO_CANVAS);
        let (zw, zh) = (zone.0 as i32 * CELL_FX, zone.1 as i32 * CELL_FX);
        let lx = self.lock.map(|r| (r.x * CELL_FX, r.w * CELL_FX));
        let ly = self.lock.map(|r| (r.y * CELL_FX, r.h * CELL_FX));
        let target = (axis(her.0 - vw / 2, vw, zw, lx), axis(her.1 - LOOK_UP_FX - vh / 2, vh, zh, ly));
        if self.ready {
            self.prev = self.pos;
            let step = |c: i32, t: i32| c + ((i64::from(t - c) * FOLLOW_Q8 + 128) >> 8) as i32;
            self.pos = (step(self.pos.0, target.0), step(self.pos.1, target.1));
        } else {
            self.pos = target;
            self.prev = target;
            self.ready = true;
        }
        self.shake_q8 = (self.shake_q8 * SHAKE_DECAY_Q8) >> 8;
        self.shake_off = if self.shake_q8 > SHAKE_FLOOR {
            let (a, b) = ((tick * 7) as usize & 15, (tick * 11 + 4) as usize & 15);
            ((SINE16[a] * self.shake_q8) / (127 * 256), (SINE16[b] * self.shake_q8) / (127 * 256))
        } else {
            (0, 0)
        };
    }

    /// The view's top-left in canvas px at `alpha` (0 last tick, 255 this tick).
    pub fn at(&self, alpha: u8) -> (i32, i32) {
        let a = alpha_256(alpha);
        let lerp = |p: i32, c: i32| p + ((i64::from(c - p) * i64::from(a)) >> 8) as i32;
        let (x, y) = (lerp(self.prev.0, self.pos.0), lerp(self.prev.1, self.pos.1));
        ((x >> FX_TO_CANVAS) + self.shake_off.0, (y >> FX_TO_CANVAS) + self.shake_off.1)
    }
}

/// `alpha` 0..=255 as a weight 0..=256, so 255 lands exactly on this tick.
pub fn alpha_256(alpha: u8) -> i32 {
    i32::from(alpha) + i32::from(alpha >> 7)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: (u16, u16) = (768, 432);
    /// 768 canvas px is 384 sim px, 48 cells.
    const VIEW_W_FX: i32 = 768 << 7;

    fn fx(cells: i32) -> i32 {
        cells * CELL_FX
    }

    #[test]
    fn it_snaps_first_then_follows_at_018() {
        let mut c = Camera::default();
        c.tick((fx(100), fx(100)), (200, 200), CANVAS, 0);
        assert_eq!(c.pos.0, fx(100) - VIEW_W_FX / 2);
        assert_eq!(c.prev, c.pos);
        let start = c.pos.0;
        c.tick((fx(110), fx(100)), (200, 200), CANVAS, 1);
        // Ten cells east: the camera covers 0.18 of it this tick.
        assert_eq!(c.pos.0 - start, (fx(10) * 46 + 128) >> 8);
        // And converges.
        for t in 2..200 {
            c.tick((fx(110), fx(100)), (200, 200), CANVAS, t);
        }
        assert!((c.pos.0 - (fx(110) - VIEW_W_FX / 2)).abs() <= 2);
    }

    #[test]
    fn it_clamps_to_the_zone_and_centres_a_small_one() {
        let mut c = Camera::default();
        // In the top-left corner of a big zone: held at 0, 0.
        c.tick((fx(2), fx(2)), (200, 200), CANVAS, 0);
        assert_eq!(c.pos, (0, 0));
        // In the far corner: held so the view ends at the zone's edge.
        let mut c = Camera::default();
        c.tick((fx(199), fx(199)), (200, 200), CANVAS, 0);
        assert_eq!(c.pos.0, fx(200) - VIEW_W_FX);
        assert_eq!(c.at(255).1 + 432, 200 * 16);
        // A room of 20 x 10 cells in a 48 x 27 view: centred, wherever she stands in it.
        for her in [(fx(1), fx(1)), (fx(19), fx(9))] {
            let mut c = Camera::default();
            c.tick(her, (20, 10), CANVAS, 0);
            let (x, y) = c.at(255);
            assert_eq!((x, y), ((20 * 16 - 768) / 2, (10 * 16 - 432) / 2));
        }
    }

    #[test]
    fn a_frame_interpolates_between_the_last_two_ticks() {
        let mut c = Camera::default();
        c.tick((fx(100), fx(100)), (400, 400), CANVAS, 0);
        c.tick((fx(120), fx(100)), (400, 400), CANVAS, 1);
        let (a, b) = (c.at(0).0, c.at(255).0);
        assert_eq!(a, c.prev.0 >> 7);
        assert_eq!(b, c.pos.0 >> 7);
        let mid = c.at(128).0;
        assert!(a < mid && mid < b, "{a} {mid} {b}");
        // Monotone in alpha.
        let xs: Vec<i32> = (0..=255u8).map(|al| c.at(al).0).collect();
        assert!(xs.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn a_lock_holds_a_small_room_whole() {
        let mut c = Camera { lock: Some(Rect::new(50, 50, 10, 8)), ..Camera::default() };
        c.tick((fx(52), fx(52)), (400, 400), CANVAS, 0);
        let (x, y) = c.at(255);
        assert_eq!((x, y), (55 * 16 - 384, 54 * 16 - 216));
    }

    #[test]
    fn a_shake_wobbles_then_dies() {
        let mut c = Camera::default();
        c.tick((fx(100), fx(100)), (400, 400), CANVAS, 0);
        let still = c.at(255);
        c.shake(4);
        let moved = (1..8).any(|t| {
            c.tick((fx(100), fx(100)), (400, 400), CANVAS, t);
            c.at(255) != still
        });
        assert!(moved);
        for t in 8..120 {
            c.tick((fx(100), fx(100)), (400, 400), CANVAS, t);
        }
        assert_eq!(c.at(255), still);
    }
}
