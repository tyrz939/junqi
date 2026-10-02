//! A fire she makes (`lamp` shapes `pit` and `grate`; ART.md §2.3, 2026-10-02): the fieldstone
//! ring out in the county, and the old iron grate in its worn stone surround that only the Fire
//! spell lights. Each has four frames ([`super::frame_ids`]), one seed through all of them so
//! the stones never move:
//!
//! ```text
//! Base   cold   old char in the hollow, a scorch on the ground round it
//! Open2  laid   two or three sticks of grey deadwood crosswise on it, unlit
//! On     lit    the sticks burning over a bed of embers, flames standing up (emissive)
//! Open   ash    a soft grey bed, a few charred stubs, one or two last embers (emissive, dim)
//! ```
//!
//! The presenter picks them by `jane_present::props::FireState`.

use jane_core::angle::{Angle, cos_q15, sin_q15};
use jane_core::grid::Rect;

use super::parts::{self, hash};
use super::{HARD, Kit, Stand, State};
use crate::canvas::{Canvas, FLAT, Z};
use crate::palette::{Ix, Ramp, Tone};

/// Deadwood: grey-brown, weathered.
const WOOD: Ramp = Ramp::Bark;
/// Char and soot.
const CHAR: Ramp = Ramp::WoodDark;
/// Ash: a cool soft grey.
const ASH: Ramp = Ramp::ClothGrey;
/// A broken end, paler than the bark.
const SPLINTER: Ramp = Ramp::WoodPale;

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    match k.look.shape {
        "pit" => Some(pit(c, k, state)),
        "grate" => Some(grate(c, k, state)),
        _ => None,
    }
}

/// `(x, y)` in the ellipse of radii `rx, ry` centred between pixels `cx - 1` and `cx` (and rows
/// `cy - 1` and `cy`), in 256ths of the way to its edge (squared).
fn ell(x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32) -> i32 {
    let dx = (2 * x + 1 - 2 * cx) * 16 / (2 * rx).max(1);
    let dy = (2 * y + 1 - 2 * cy) * 16 / (2 * ry).max(1);
    dx * dx + dy * dy
}

/// The ground the fires have scorched: contact shade (it darkens, it does not paint) in a
/// ragged oval, its rim broken into 2 px clusters.
fn scorch(c: &mut Canvas, k: &Kit, cx: i32, cy: i32, rx: i32, ry: i32) {
    for y in cy - ry..cy + ry {
        for x in cx - rx..cx + rx {
            let d = ell(x, y, cx, cy, rx, ry);
            let keep = d < 150 || (d < 256 && hash(k.seed, x >> 1, (y >> 1) + 70) % 3 != 0);
            if keep && c.get(x, y) == Ix::CLEAR {
                c.put(x, y, Ix::AO, FLAT, 0);
            }
        }
    }
}

/// One stone of the ring.
#[derive(Clone, Copy, Debug)]
struct Stone {
    r: Rect,
    ramp: Ramp,
    /// Behind the hollow: its face to the fire is the one the viewer sees.
    back: bool,
}

/// `n` fieldstones round `(cx, cy)`, each its own size and a few of slate, back to front.
fn ring(k: &Kit, cx: i32, cy: i32, rx: i32, ry: i32, n: i32) -> Vec<Stone> {
    let mut v: Vec<Stone> = (0..n)
        .map(|i| {
            let h = hash(k.seed, i, 50);
            let a = Angle((i * 65536 / n + (h & 0x7ff) as i32 - 0x400) as u16);
            let (s, co) = (sin_q15(a).0, cos_q15(a).0);
            let (x, y) = (cx + ((co * rx) >> 15), cy + ((s * ry) >> 15));
            // Nearer stones are bigger (the 3/4 view), and each one its own size.
            let (near, far) = (i32::from(s > 9000), i32::from(s < -9000));
            let w = 5 + near - far + (h >> 12 & 1) as i32;
            let hh = 4 + near - far;
            let ramp = if (h >> 16) % 4 == 0 { Ramp::Slate } else { k.body };
            Stone { r: Rect::new(x - w / 2, y - hh / 2 - 1, w, hh), ramp, back: s < 0 }
        })
        .collect();
    v.sort_by_key(|s| s.r.y);
    v
}

fn stone(c: &mut Canvas, s: &Stone) {
    c.ellipse_lit(s.r, s.ramp, Z::new(2, 4));
    if s.back {
        // Its face to the fire, the one the viewer sees, is sooted.
        c.shade(Rect::new(s.r.x, s.r.y + s.r.h - 2, s.r.w, 3), s.ramp, 2);
    }
}

/// How a stick looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wood {
    /// Grey deadwood, its broken ends pale.
    Dead,
    /// Burning: glowing where the flames are, black at its ends.
    Burning,
    /// A charred stub, its end grey with ash.
    Charred,
}

/// A stick from `a` to `b`, two px thick: lit along its top (or its left, if it runs steep), its
/// underside in shade.
fn stick(c: &mut Canvas, k: &Kit, a: (i32, i32), b: (i32, i32), wood: Wood, z: u8) {
    let steep = (b.1 - a.1).abs() > (b.0 - a.0).abs();
    let off = if steep { (1, 0) } else { (0, 1) };
    let under = |p: (i32, i32)| (p.0 + off.0, p.1 + off.1);
    let (top, low) = match wood {
        Wood::Dead => (WOOD.at(Tone::Light), WOOD.at(Tone::Base)),
        Wood::Burning | Wood::Charred => (CHAR.at(Tone::Mid), CHAR.at(Tone::Deep)),
    };
    // A dead stick is three px: its lit top, its body, its underside in shade.
    if wood == Wood::Dead {
        let below = |p: (i32, i32)| (p.0 + 2 * off.0, p.1 + 2 * off.1);
        c.line(below(a), below(b), WOOD.at(Tone::Deep), 1, z);
    }
    c.line(under(a), under(b), low, 1, z);
    c.line(a, b, top, 1, z + 1);
    match wood {
        Wood::Dead => {
            // Weathered silver along its top in a run or two; a knot; pale broken ends.
            let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
            let h = hash(k.seed, a.0 * 7 + b.0, a.1 * 5 + b.1);
            let at = |t: i32| (a.0 + (b.0 - a.0) * t / n, a.1 + (b.1 - a.1) * t / n);
            let from = 2 + (h % 3) as i32;
            for t in from..(from + 3).min(n - 1) {
                let p = at(t);
                c.dot(p.0, p.1, ASH.at(Tone::Light), z + 1);
            }
            let knot = at(n / 2 + 1 + ((h >> 4) & 1) as i32);
            let knot = under(knot);
            c.dot(knot.0, knot.1, WOOD.at(Tone::Deep), z);
            for p in [a, b] {
                c.dot(p.0, p.1, SPLINTER.at(Tone::Light), z + 1);
                let q = under(p);
                c.dot(q.0, q.1, SPLINTER.at(Tone::Base), z);
            }
        }
        Wood::Burning => {
            // The middle, in the flames, glows through its cracks; the ends are black.
            let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
            c.set_emitting(true);
            for t in n / 4..=n - n / 4 {
                let p = (a.0 + (b.0 - a.0) * t / n, a.1 + (b.1 - a.1) * t / n);
                let hot = t > n / 3 && t < n - n / 3;
                let q = under(p);
                c.dot(q.0, q.1, Ramp::Ember.at(if hot { Tone::Light } else { Tone::Base }), z);
                if hot || t % 3 != 0 {
                    c.dot(p.0, p.1, Ramp::Ember.at(if hot { Tone::High } else { Tone::Base }), z + 1);
                }
            }
            c.set_emitting(false);
        }
        Wood::Charred => {
            c.dot(b.0, b.1, ASH.at(Tone::Light), z + 1);
            let q = under(b);
            c.dot(q.0, q.1, ASH.at(Tone::Base), z);
        }
    }
}

/// One tongue of flame standing on `(x, y)`: `h` rows tall, `w` px across at its widest (a
/// quarter of the way up), its tip curling `lean` px aside. White-yellow at its heart low down,
/// orange through its body, red at its edges and its tip; all of it emits.
fn tongue(c: &mut Canvas, (x, y): (i32, i32), h: i32, w: i32, lean: i32) {
    c.set_emitting(true);
    c.begin();
    for r in 0..h {
        let t = r * 256 / h.max(1);
        // Widest a quarter of the way up, a point at the top.
        let prof = if t < 64 { 176 + t + t / 4 } else { (256 - t) * 256 / 192 };
        let half = (w * prof + 255) / 512;
        let mid = x + lean * t * t / 65536;
        for dx in -half..=half {
            let off = dx.abs() * 256 / (half + 1);
            let ix = if t > 215 {
                Ramp::Ember.at(Tone::Shade)
            } else if off > 190 {
                Ramp::Ember.at(Tone::Mid)
            } else if t < 140 && off < 80 {
                Ramp::GlassLit.at(Tone::High)
            } else if t < 180 && off < 140 {
                Ramp::GlassLit.at(Tone::Base)
            } else {
                Ramp::Ember.at(Tone::Base)
            };
            c.put(mid + dx, y - r, ix, FLAT, 7);
        }
    }
    c.set_emitting(false);
}

/// Flames: tongues `(x, y, h, w, lean)`, the back ones first.
fn flames(c: &mut Canvas, tongues: &[(i32, i32, i32, i32, i32)]) {
    for &(x, y, h, w, lean) in tongues {
        tongue(c, (x, y), h, w, lean);
    }
}

/// Old char in a hollow `r`: black, with grey drifts of what ash the rain left.
fn char_bed(c: &mut Canvas, k: &Kit, r: Rect) {
    let coal = Ramp::Slate.at(Tone::Deep);
    c.ellipse(r, coal, 1);
    for i in 0..7 {
        let h = hash(k.seed, i, 60);
        let x = r.x + 2 + (h % (r.w - 5).max(1) as u32) as i32;
        let y = r.y + 1 + ((h >> 8) % (r.h - 3).max(1) as u32) as i32;
        let w = 2 + (h >> 16 & 1) as i32;
        // Most are grey drifts of ash, a few brown-black lumps of what did not burn.
        let lump = i % 3 == 2;
        for dx in 0..w {
            if c.get(x + dx, y) == coal {
                let t = if dx == 0 && h >> 18 & 1 == 0 { Tone::Mid } else { Tone::Shade };
                c.dot(x + dx, y, if lump { CHAR.at(Tone::Shade) } else { ASH.at(t) }, 1);
            }
        }
    }
}

/// A bed of embers filling the ellipse in `r`: white-hot at its heart, cooling to dull red,
/// broken coals dark in its outer part.
fn ember_bed(c: &mut Canvas, k: &Kit, r: Rect) {
    let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
    let (rx, ry) = (r.w / 2, r.h / 2);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let d = ell(x, y, cx, cy, rx, ry);
            if d >= 256 {
                continue;
            }
            let coal = d > 110 && hash(k.seed, x >> 1, (y >> 1) + 90) % 3 == 0;
            if coal {
                c.dot(x, y, CHAR.at(if y < cy { Tone::Shade } else { Tone::Deep }), 2);
                continue;
            }
            let t = if d < 70 {
                Tone::High
            } else if d < 160 {
                Tone::Base
            } else {
                Tone::Shade
            };
            c.set_emitting(true);
            c.dot(x, y, Ramp::Ember.at(t), 2);
            c.set_emitting(false);
        }
    }
}

/// Soft grey ash in a low mound filling `r`.
fn ash_bed(c: &mut Canvas, r: Rect) {
    let mut a = Canvas::new(c.w(), c.h());
    a.ellipse_lit(r, ASH, Z::new(1, 2));
    a.retone(ASH, [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::Light]);
    c.stamp(&a, 0, 0);
}

/// A last ember: two px, dull red, glowing faintly.
fn last_ember(c: &mut Canvas, x: i32, y: i32) {
    c.set_emitting(true);
    c.dot(x, y, Ramp::Ember.at(Tone::Shade), 3);
    c.dot(x + 1, y, Ramp::Ember.at(Tone::Deep), 3);
    c.set_emitting(false);
}

/// Fieldstones' own tones: weathered, a little darker than cut stone, a high only on a crown.
const FIELD: [Tone; 8] =
    [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::High];

/// The fieldstone ring.
fn pit(c: &mut Canvas, k: &Kit, state: State) -> Stand {
    // All of it on the footprint's front cell, the ground it blocks (`base` 1): the back cell is
    // the flames' and the air's, and she may stand in it.
    let (cx, cy) = (k.w / 2, k.foot() - 7);
    scorch(c, k, cx, cy, 15, 7);
    let hollow = Rect::new(cx - 9, cy - 4, 18, 7);
    let bed = Rect::new(cx - 8, cy - 4, 16, 6);
    char_bed(c, k, hollow);
    match state {
        State::Open => ash_bed(c, Rect::new(cx - 7, cy - 3, 14, 4)),
        State::On => ember_bed(c, k, bed),
        _ => {}
    }
    let stones = ring(k, cx, cy, 12, 5, 10);
    for s in stones.iter().filter(|s| s.back) {
        stone(c, s);
    }
    // Laid crosswise, their ends on the stones.
    let sticks =
        [((cx - 10, cy - 2), (cx + 6, cy - 6)), ((cx - 7, cy - 6), (cx + 9, cy - 2)), ((cx - 5, cy), (cx + 3, cy - 4))];
    match state {
        State::Laid => {
            for (a, b) in sticks {
                stick(c, k, a, b, Wood::Dead, 4);
            }
        }
        State::On => {
            for (a, b) in sticks {
                stick(c, k, a, b, Wood::Burning, 4);
            }
            flames(
                c,
                &[
                    (cx - 5, cy - 2, 8, 4, -2),
                    (cx + 5, cy - 2, 9, 4, 2),
                    (cx + 2, cy - 1, 12, 5, -1),
                    (cx - 2, cy, 16, 6, 1),
                ],
            );
        }
        State::Open => {
            stick(c, k, (cx - 7, cy - 3), (cx - 3, cy - 2), Wood::Charred, 3);
            stick(c, k, (cx + 6, cy - 4), (cx + 2, cy - 3), Wood::Charred, 3);
            last_ember(c, cx - 3, cy - 1);
            last_ember(c, cx + 1, cy - 2);
        }
        State::Base => {
            stick(c, k, (cx - 6, cy - 3), (cx - 2, cy - 2), Wood::Charred, 3);
            stick(c, k, (cx + 5, cy - 4), (cx + 2, cy - 3), Wood::Charred, 3);
        }
    }
    for s in stones.iter().filter(|s| !s.back) {
        stone(c, s);
    }
    c.retone(k.body, FIELD);
    c.retone(Ramp::Slate, FIELD);
    Stand::Hearth(4, cy + 1)
}

/// The old grate: an iron grate over a sooted firebox, set in a kerb of worn slabs, an andiron
/// at each back corner and a little moss on the kerb where nobody sits.
fn grate(c: &mut Canvas, k: &Kit, state: State) -> Stand {
    let (w, foot) = (k.w, k.foot());
    let (x0, x1) = (2, w - 3);
    let kw = x1 - x0 + 1;
    let top = foot - 15;
    let face = foot - 4;
    let iron = k.trim;
    c.ao_contact(Rect::new(0, top, w, foot - top + 2), 1);
    // The kerb: its top in slabs, its front a course of blocks, its corners worn round.
    parts::blocks(c, Rect::new(x0, top, kw, face - top), k.body, 4, 7, true, k.seed, 3);
    parts::blocks(c, Rect::new(x0, face, kw, foot - face), k.body, 4, 9, false, k.seed ^ 7, 2);
    for (x, y) in [(x0, top), (x0 + 1, top), (x0, top + 1), (x1, top), (x1 - 1, top), (x1, top + 1)] {
        c.clear_px(x, y);
    }
    for (x, y) in [(x0, foot - 1), (x1, foot - 1)] {
        c.clear_px(x, y);
    }
    // A crack across a slab.
    c.line((x1 - 5, top + 1), (x1 - 3, top + 5), k.body.at(Tone::Shade), 1, 3);
    // The firebox: its back wall faces the viewer, sooted; its floor holds what is in it.
    let (ox, oy, ow, oh) = (x0 + 5, top + 3, kw - 10, 6);
    c.fill_rect(Rect::new(ox, oy, ow, 2), Ramp::Slate.at(Tone::Shade), 2);
    c.hline(ox, ox + ow - 1, oy, Ramp::Slate.at(Tone::Deep), 2);
    let floor = Rect::new(ox, oy + 2, ow, oh - 2);
    c.fill_rect(floor, Ramp::Slate.at(Tone::Deep), 1);
    match state {
        State::On => {
            c.set_emitting(true);
            // The back wall takes the glow.
            c.hline(ox + 2, ox + ow - 3, oy + 1, Ramp::Ember.at(Tone::Deep), 2);
            c.set_emitting(false);
            ember_bed(c, k, Rect::new(ox - 1, oy + 2, ow + 2, oh - 1));
        }
        State::Open => ash_bed(c, Rect::new(ox, oy + 2, ow, oh - 1)),
        _ => char_bed(c, k, floor),
    }
    c.vline(ox, oy, oy + oh - 1, Ramp::Slate.at(Tone::Deep), 2);
    // The grate: a back and a front rail, bars between, rust on some.
    let lit = state == State::On;
    let (bar, bar_lit) =
        if lit { (iron.at(Tone::Deep), iron.at(Tone::Shade)) } else { (iron.at(Tone::Base), iron.at(Tone::Light)) };
    c.hline(ox, ox + ow - 1, oy + 2, bar, 4);
    for i in 0..6 {
        let x = ox + 2 + i * 3;
        c.vline(x, oy + 2, oy + oh - 1, bar, 4);
        c.dot(x, oy + oh - 2, bar_lit, 5);
        let h = hash(k.seed, i, 40);
        if !lit && h % 3 == 0 {
            let y = oy + 3 + (h >> 4) as i32 % (oh - 4).max(1);
            c.vline(x, y, y + 1, Ramp::Copper.at(Tone::Shade), 4);
        }
    }
    c.hline(ox - 1, ox + ow, oy + oh, bar_lit, 5);
    c.hline(ox - 1, ox + ow, oy + oh + 1, if lit { iron.at(Tone::Deep) } else { iron.at(Tone::Shade) }, 5);
    // The andirons at the back corners: a square post, a knob.
    for x in [ox - 2, ox + ow] {
        c.vline(x, oy - 3, oy + 1, iron.at(Tone::Light), 5);
        c.vline(x + 1, oy - 3, oy + 1, iron.at(Tone::Shade), 5);
        c.fill_rect(Rect::new(x, oy - 5, 2, 2), iron.at(Tone::Base), 6);
        c.dot(x, oy - 5, iron.at(Tone::High), 6);
    }
    // A little moss on the kerb's side.
    c.ellipse_lit(Rect::new(x0, top + 6, 4, 3), Ramp::Leaf, Z::new(3, 4));
    c.retone(
        Ramp::Leaf,
        [Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Mid, Tone::Base, Tone::Base, Tone::Base],
    );
    let (mx, my) = (ox + ow / 2, oy + 4);
    let sticks =
        [((ox + 1, my), (ox + 17, my - 4)), ((ox + 1, my - 4), (ox + 17, my)), ((mx - 4, my + 1), (mx + 4, my - 3))];
    match state {
        State::Laid => {
            for (a, b) in sticks {
                stick(c, k, a, b, Wood::Dead, 6);
            }
        }
        State::On => {
            for (a, b) in sticks {
                stick(c, k, a, b, Wood::Burning, 6);
            }
            flames(
                c,
                &[
                    (mx - 6, my, 8, 4, -2),
                    (mx + 6, my, 9, 4, 2),
                    (mx + 2, my + 1, 12, 5, -1),
                    (mx - 2, my + 1, 16, 6, 1),
                ],
            );
        }
        State::Open => {
            stick(c, k, (ox + 2, my), (ox + 6, my - 1), Wood::Charred, 6);
            stick(c, k, (ox + ow - 3, my - 1), (ox + ow - 7, my), Wood::Charred, 6);
            last_ember(c, mx, oy + oh - 2);
        }
        State::Base => stick(c, k, (ox + 3, my), (ox + 7, my - 1), Wood::Charred, 6),
    }
    c.retone(iron, HARD);
    c.retone(k.body, HARD);
    Stand::Hearth(5, oy + oh)
}
