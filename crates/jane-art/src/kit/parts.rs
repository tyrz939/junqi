//! The kit's shared parts (ART.md §2.3): planks, posts, stone blocks, iron bands, glass and
//! flame, each drawn once so every family lays wood, stone and iron the same way and in the
//! same clusters. Every part writes its own normals (a face looks south, a top looks up, a
//! post turns like a cylinder) and a relief `z` the outline reads for seams; [`super::finish`]
//! stands the heights up after.
//!
//! Texture is clusters, never speckle: a plank's grain is two or three streaks of two to four
//! px in its shade, a knot is a two-px dark with a lit px over it, a stone block has a lit top
//! edge and one chip of shade.

use jane_core::grid::Rect;

use crate::canvas::{Canvas, Z, normal};
use crate::hash::{below, h32, salt};
use crate::palette::{Ix, Ramp, Tone};

/// The normal of a face looking south and a little up: a crate's front, a sign's board.
pub(crate) fn south() -> [u8; 2] {
    normal(0, 84)
}

/// A hash of `(a, b)` under the kit's seed.
pub(crate) fn hash(seed: u32, a: i32, b: i32) -> u32 {
    h32(seed ^ a as u32, b as u32, salt::STROKES ^ 0x4b49_5400)
}

/// Fill `r` flat in `ix` with the normal `n` at relief `z`, as one part.
pub(crate) fn fill(c: &mut Canvas, r: Rect, ix: Ix, n: [u8; 2], z: u8) {
    c.fill_normal(r, ix, n, z);
}

/// Planks across a face or a top, `n` of them in `r`: `across` runs them left to right (a
/// crate's side), else top to bottom (a door, a fence board). Each plank's leading edge is lit
/// and its trailing edge a seam in shade; the grain is a few short streaks of shade, and one
/// plank in four has a knot. `top` faces the sky (a lid, a table), else south.
#[allow(clippy::too_many_arguments)]
pub(crate) fn planks(c: &mut Canvas, r: Rect, ramp: Ramp, n: i32, across: bool, top: bool, seed: u32, z: u8) {
    let nrm = if top { crate::canvas::FLAT } else { south() };
    let base = if top { Tone::Lift } else { Tone::Base };
    fill(c, r, ramp.at(base), nrm, z);
    let n = n.max(1);
    let len = if across { r.h } else { r.w };
    for k in 0..n {
        // This plank's span across the boards.
        let (a, b) = (k * len / n, (k + 1) * len / n - 1);
        let tone = |t: Tone| ramp.at(t);
        // Each plank a touch lighter or darker than the next: boards cut from different trees.
        let shift = [0, 1, 0, -1][(hash(seed, k, 7) % 4) as usize];
        let body = tone(base.step(shift));
        for i in a..=b {
            for j in 0..if across { r.w } else { r.h } {
                let (x, y) = if across { (r.x + j, r.y + i) } else { (r.x + i, r.y + j) };
                let ix = if i == a && k > 0 {
                    tone(base.step(2))
                } else if i == b && k < n - 1 {
                    tone(Tone::Shade)
                } else {
                    body
                };
                c.put(x, y, ix, nrm, z);
            }
        }
        // Grain: two short streaks along the plank, in its mid tone, never on an edge.
        let span = b - a + 1;
        if span >= 3 {
            for s in 0..2 {
                let h = hash(seed, k, s);
                let along = if across { r.w } else { r.h };
                let at = a + 1 + below(h, (span - 2).max(1) as u32) as i32;
                let from = below(h >> 8, (along - 3).max(1) as u32) as i32;
                let l = 2 + (h >> 16 & 1) as i32;
                for d in 0..l {
                    let (x, y) = if across { (r.x + from + d, r.y + at) } else { (r.x + at, r.y + from + d) };
                    c.tint(x, y, ramp, base.step(-1).min(Tone::Mid));
                }
            }
            if hash(seed, k, 9) % 4 == 0 {
                let h = hash(seed, k, 10);
                let along = if across { r.w } else { r.h };
                let at = a + span / 2;
                let o = 1 + below(h, (along - 3).max(1) as u32) as i32;
                let (x, y) = if across { (r.x + o, r.y + at) } else { (r.x + at, r.y + o) };
                c.tint(x, y, ramp, Tone::Deep);
                let (x2, y2) = if across { (x + 1, y) } else { (x, y + 1) };
                c.tint(x2, y2, ramp, Tone::Shade);
            }
        }
    }
}

/// A wooden post or a pole, `w` px wide, from row `y0` down to `y1`: lit down its left, shaded
/// down its right, a cylinder's normals.
pub(crate) fn post(c: &mut Canvas, x: i32, y0: i32, y1: i32, w: i32, ramp: Ramp, z: u8) {
    c.polygon_lit(&[(x, y0), (x + w - 1, y0), (x + w - 1, y1), (x, y1)], ramp, 90, Z::flat(z));
    if w >= 3 {
        for y in y0 + 1..y1 {
            c.tint(x, y, ramp, Tone::Light);
        }
    }
}

/// Stone laid in courses over `r`: blocks `bh` rows high, joints staggered course to course,
/// mortar in the stone's shade, each block's top edge lit and one chip of shade on some.
/// `top` faces the sky, else south.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blocks(c: &mut Canvas, r: Rect, ramp: Ramp, bh: i32, bw: i32, top: bool, seed: u32, z: u8) {
    let nrm = if top { crate::canvas::FLAT } else { south() };
    let bh = bh.max(2);
    fill(c, r, ramp.at(Tone::Base), nrm, z);
    for y in r.y..r.bottom() {
        let course = (y - r.y) / bh;
        let row = (y - r.y) % bh;
        let off = if course % 2 == 1 { bw / 2 } else { 0 };
        for x in r.x..r.right() {
            let col = (x - r.x + off) % bw.max(2);
            let block = (x - r.x + off) / bw.max(2);
            let shift = [0, 1, 0, -1, 0][(hash(seed, block, course) % 5) as usize];
            let base = if top { Tone::Lift } else { Tone::Base }.step(shift);
            let ix = if row == bh - 1 || col == bw - 1 {
                ramp.at(Tone::Shade)
            } else if row == 0 {
                ramp.at(base.step(1))
            } else {
                ramp.at(base)
            };
            c.put(x, y, ix, nrm, z);
        }
    }
    // A chip in some blocks: two px of shade.
    for (i, y) in (r.y + 1..r.bottom() - 1).step_by(bh as usize).enumerate() {
        let h = hash(seed, i as i32, 31);
        if h % 3 == 0 && r.w > 4 {
            let x = r.x + 1 + below(h >> 4, (r.w - 3) as u32) as i32;
            c.tint(x, y + 1, ramp, Tone::Mid);
            c.tint(x + 1, y + 1, ramp, Tone::Mid);
        }
    }
}

/// An iron band across `x0..=x1` on row `y`, two px, lit on top, with a rivet at each end.
pub(crate) fn band(c: &mut Canvas, x0: i32, x1: i32, y: i32, ramp: Ramp, z: u8) {
    c.fill_rect(Rect::new(x0, y, x1 - x0 + 1, 1), ramp.at(Tone::Light), z);
    c.fill_rect(Rect::new(x0, y + 1, x1 - x0 + 1, 1), ramp.at(Tone::Mid), z);
    for x in [x0 + 1, x1 - 1] {
        c.dot(x, y, ramp.at(Tone::High), z);
    }
}

/// A pane of glass over `r`: cool and dark with a streak of reflected sky when unlit; warm and
/// emitting when lit, its core brightest and its edge a tone down, so the lamp reads as light
/// and not as paint.
pub(crate) fn glass(c: &mut Canvas, r: Rect, lit: bool, z: u8) {
    if lit {
        c.set_emitting(true);
        let mut m = Canvas::new(c.w(), c.h());
        m.fill_rect(r, Ix::INK, 1);
        c.inflate(&m, Ramp::GlassLit, (r.w.min(r.h) / 2).max(1), Z::flat(z));
        c.retone(
            Ramp::GlassLit,
            [Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::High, Tone::High, Tone::Glint, Tone::Glint],
        );
        c.set_emitting(false);
    } else {
        fill(c, r, Ramp::Glass.at(Tone::Shade), south(), z);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if x - r.x == y - r.y + 1 || x - r.x == y - r.y + 2 {
                    c.tint(x, y, Ramp::Glass, Tone::Light);
                }
            }
        }
        c.tint(r.x, r.y, Ramp::Glass, Tone::Base);
    }
}

/// A flame standing on `(x, y)`, `h` px tall: an ember teardrop with a bright core, emitting.
pub(crate) fn flame(c: &mut Canvas, x: i32, y: i32, h: i32, lean: i32, z: u8) {
    c.set_emitting(true);
    let w = (h / 2).max(2);
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(
        &[(x - w / 2, y), (x + (w - 1) / 2 + 1, y), (x + (w - 1) / 2, y - h / 2), (x + lean, y - h + 1)],
        Ix::INK,
        1,
    );
    m.ellipse(Rect::new(x - w / 2, y - h / 2, w + 1, h / 2 + 1), Ix::INK, 1);
    c.inflate(&m, Ramp::Ember, 1, Z::flat(z));
    c.retone(
        Ramp::Ember,
        [Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::Light, Tone::High, Tone::Glint, Tone::Glint],
    );
    if h >= 4 {
        c.fill_rect(Rect::new(x, y - h / 2, 1, h / 2), Ramp::GlassLit.at(Tone::Glint), z);
    }
    c.set_emitting(false);
}

/// A box seen in the 3/4 view: its front face `face` rows tall standing on `foot` across
/// `x..x + w`, and its top `depth` rows deep above it. Returns `(top, front)`; the caller lids
/// the top at `lid_height(face)`. Planks when `wood`, else a plain lit block.
#[allow(clippy::too_many_arguments)]
pub(crate) fn box3(
    c: &mut Canvas,
    x: i32,
    w: i32,
    foot: i32,
    face: i32,
    depth: i32,
    ramp: Ramp,
    wood: Option<(i32, u32)>,
    z: u8,
) -> (Rect, Rect) {
    let front = Rect::new(x, foot - face + 1, w, face);
    let top = Rect::new(x, front.y - depth, w, depth);
    match wood {
        Some((n, seed)) => {
            planks(c, top, ramp, n, false, true, seed, z + 1);
            planks(c, front, ramp, (face / 4).max(1), true, false, seed ^ 1, z);
        }
        None => {
            fill(c, top, ramp.at(Tone::Lift), crate::canvas::FLAT, z + 1);
            fill(c, front, ramp.at(Tone::Mid), south(), z);
        }
    }
    // The top's front edge catches the light; the face's foot sits in its shade.
    c.hline(top.x, top.right() - 1, top.bottom() - 1, ramp.at(Tone::Light), z + 1);
    c.hline(front.x, front.right() - 1, front.bottom() - 1, ramp.at(Tone::Shade), z);
    (top, front)
}

/// How high a top stands over a face `face` rows tall: its height above the foot, as a person's
/// rows stand ([`Canvas::upright`]).
pub(crate) fn lid_height(face: i32) -> u8 {
    (face * 5 / 4).clamp(1, 255) as u8
}

/// The contact shadow under a thing standing on the foot row across `x0..=x1`.
pub(crate) fn ao(c: &mut Canvas, x0: i32, x1: i32, foot: i32, deep: i32) {
    c.ao_contact(Rect::new(x0 - 1, foot - deep / 2, x1 - x0 + 3, deep), 1);
}

/// Illegible writing: `rows` rows of short strokes in `ix` across `r`, broken into words by
/// the seed. The words are in the dialogue, never on the sprite (ART.md §2.3).
pub(crate) fn writing(c: &mut Canvas, r: Rect, rows: i32, ix: Ix, seed: u32, z: u8) {
    let rows = rows.max(1);
    let pitch = (r.h / rows).max(2);
    for k in 0..rows {
        let y = r.y + k * pitch + pitch / 2 - 1;
        if y >= r.bottom() {
            break;
        }
        let mut x = r.x + i32::from(k == 0 && r.w > 8);
        let end = r.right() - 1 - (hash(seed, k, 3) % 3) as i32;
        while x < end {
            let word = 2 + (hash(seed, k, x) % 4) as i32;
            let to = (x + word - 1).min(end - 1);
            c.hline(x, to, y, ix, z);
            x = to + 3;
        }
    }
}
