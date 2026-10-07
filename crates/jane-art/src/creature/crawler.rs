//! `crawler` (the lurker): ART.md §2.2, §8 step 7. What the data calls it: the Burial's snake
//! that spits from far off, the shape on the lake's bank, the thing under the vermin man's hatch,
//! and in DUNGEONS.md the measure of a thing too strong to fight. Drawn as what lives in the
//! dark water under all three: a cave salamander grown huge, pale as a drowned hand. A long
//! body in a slow S, a flat wide head with a lipless mouth and two small eyes that shine, pink
//! gills in feathery tufts behind the jaw, four stubby splayed legs, a flattened tail with a fin.
//!
//! It walks in a swim: the S travels down the body from the head, the legs paddle in diagonal
//! pairs, the gills stream. The idle pair lifts the head and flares the gills; it strikes with
//! the jaw dropped on a pink mouth. Dead, it lies on its back, the paler belly up, legs in the air.

use alloc::vec::Vec;
use jane_core::grid::Rect;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 2);
    pub const BODY: Z = Z::new(2, 6);
    pub const HEAD: Z = Z::new(5, 8);
}

/// The S's offset at body point `i` (0 the head) on beat `k`: a table, no floats.
fn wave(i: i32, k: i32) -> i32 {
    const W: [i32; 12] = [0, 1, 2, 2, 1, 0, 0, -1, -2, -2, -1, 0];
    W[((i + 12 - k * 2) % 12) as usize]
}

/// One beat: the walk phase, the head's lift, the jaw open, the gills flared.
fn pose(beat: Beat) -> (i32, i32, bool, bool) {
    match beat {
        Beat::Walk(k) => (i32::from(k), 0, false, false),
        Beat::Breathe => (0, 1, false, true),
        Beat::Idle(0) | Beat::Attack(0) => (0, 3, false, true),
        Beat::Idle(_) => (1, 2, true, true),
        Beat::Hurt => (3, -1, true, false),
        Beat::Attack(1) => (1, -1, true, true),
        Beat::Attack(_) => (2, 1, false, false),
        Beat::Dead => (0, 0, false, false),
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    if beat == Beat::Dead {
        dead(c, k, ax, ay);
        return;
    }
    let (phase, lift, open, flare) = pose(beat);
    let lunge = if beat == Beat::Attack(1) { 3 } else { 0 };
    match facing {
        Facing::Side => side(c, k, ax, ay, phase, lift, open, flare, lunge),
        Facing::Down | Facing::Up => top(c, k, ax, ay, facing == Facing::Down, phase, lift, open, flare, lunge),
        Facing::DownRight | Facing::UpRight => {
            turned(c, k, ax, ay, facing == Facing::DownRight, phase, lift, open, flare, lunge);
        }
    }
}

/// Turned an eighth (the diagonals): the head down and to the right (`toward`) or up and to the
/// right, the body running back the other way in its S (the wave across the line of it), the
/// near legs splayed toward us and the far ones over the back in shade, the gills streaming.
/// Toward, the face: both eyes, the far one closer to the snout, the mouth's line along it.
/// Away, the back of the head and the eye on its right.
#[allow(clippy::too_many_arguments)]
fn turned(
    c: &mut Canvas,
    k: &Coat,
    ax: i32,
    ay: i32,
    toward: bool,
    phase: i32,
    lift: i32,
    open: bool,
    flare: bool,
    lunge: i32,
) {
    // The line of the body from the head: back up (toward) or down (away) the screen and to the
    // left; across it (the wave) is (1, 1) facing down and (1, -1) facing up.
    let back = if toward { (-2, -2) } else { (-2, 2) };
    let across = if toward { (1, -1) } else { (1, 1) };
    let (hx, hy) = if toward { (ax + 9 + lunge, ay - 4 - lift) } else { (ax + 8 + lunge, ay - 19 - lift) };
    let pts: Vec<(i32, i32, i32)> = (0..9)
        .map(|i| {
            let w = wave(i, phase);
            let x = hx + back.0 * i + across.0 * w;
            // Away, the tail is nearest and lowest: the body rides so its tip keeps the ground.
            let ride = if toward { 0 } else { across.1 * wave(8, phase) };
            let y = hy + back.1 * i + across.1 * w - ride + if i < 2 && !toward { lift } else { 0 };
            (x, y, [3, 3, 3, 3, 3, 2, 2, 1, 1][i as usize])
        })
        .collect();
    // The far legs, over the back and in its shade.
    let far = if toward { (1, -1) } else { (-1, -1) };
    let near = (-far.0, -far.1);
    for (li, i) in [(0, 1), (1, 5)] {
        let (x, y, g) = pts[i as usize];
        let reach = if (phase + li) % 2 == 0 { 2 } else { -1 };
        let foot = (x + far.0 * (g + 4) + reach, (y + far.1 * (g + 2) + reach * back.1.signum() / 2).min(ay - 1));
        leg(c, k, (x + far.0 * g, y + far.1), foot, true);
    }
    let mut m = Canvas::new(c.w(), c.h());
    for w in pts.windows(2) {
        m.line((w[0].0, w[0].1), (w[1].0, w[1].1), Ix::INK, 2 * w[0].2, 1);
    }
    // The tail's fin along the last of it.
    let (tx, ty, _) = pts[8];
    let (fx, fy, _) = pts[6];
    m.polyline_fill(
        &[(fx, fy - 2), (tx + back.0, ty + back.1 - 1), (tx + back.0, ty + back.1 + 1), (fx, fy + 2)],
        Ix::INK,
        1,
    );
    // The head: flat and wide, turned with the body.
    m.ellipse(Rect::new(hx - 4, hy - 3, 9, 6), Ix::INK, 1);
    c.inflate(&m, k.body, 2, relief::BODY);
    let r = k.body;
    let zb = relief::BODY.hi;
    // The ridge of the back lit, a mottle now and then.
    for (i, &(x, y, g)) in pts.iter().enumerate().skip(1).take(7) {
        c.fill_rect(Rect::new(x - 1, y - 1, 2, 1), r.at(Tone::Light), zb);
        if g >= 2 {
            c.dot(x + 1, y + 1, r.at(Tone::Mid), zb);
        }
        if i % 3 == 1 {
            c.fill_rect(Rect::new(x - 2, y, 2, 2), r.at(Tone::Mid), zb);
        }
    }
    if toward && k.belly != k.body {
        // The paler flank toward us along the underside.
        for w in pts.windows(2).take(6) {
            c.dye_poly(
                &[
                    (w[0].0 - 1, w[0].1 + 1),
                    (w[1].0 - 1, w[1].1 + 1),
                    (w[1].0 - 2, w[1].1 + w[1].2),
                    (w[0].0 - 2, w[0].1 + w[0].2),
                ],
                k.body,
                k.belly,
            );
        }
    }
    // The near legs, splayed toward us.
    for (li, i) in [(1, 1), (0, 5)] {
        let (x, y, g) = pts[i as usize];
        let reach = if (phase + li) % 2 == 0 { 2 } else { -1 };
        let foot = (x + near.0 * (g + 3) + reach, (y + near.1 * (g + 3)).min(ay - 1));
        leg(c, k, (x + near.0 * (g - 1), y + near.1 * (g - 1)), foot, false);
    }
    let z = relief::HEAD.hi;
    let gdir = if toward { (-1, -1) } else { (-1, 1) };
    gills(c, k, hx - 4, hy - 1, gdir, flare, z);
    gills(c, k, hx + 2, hy - 3, (gdir.0, -1), flare, z);
    if toward {
        eye(c, k, hx - 2, hy - 1, z);
        eye(c, k, hx + 3, hy - 2, z);
        if open {
            c.polyline_fill(&[(hx - 2, hy + 1), (hx + 4, hy), (hx + 3, hy + 3)], Ramp::ClothRose.at(Tone::Base), z);
            c.hline(hx - 1, hx + 3, hy + 1, Ramp::Bone.at(Tone::Light), z);
        } else {
            c.line((hx - 3, hy + 1), (hx + 4, hy), k.body.at(Tone::Deep), 1, z);
        }
    } else {
        eye(c, k, hx + 4, hy - 1, z);
    }
}

/// A stubby leg: a thigh out from the body to an elbow, a forearm down to splayed toes.
fn leg(c: &mut Canvas, k: &Coat, from: (i32, i32), foot: (i32, i32), far: bool) {
    let z = if far { relief::FAR } else { relief::BODY };
    let r = k.body;
    c.line(from, foot, r.at(if far { Tone::Shade } else { Tone::Base }), 2, z.hi);
    for dx in [-1, 1] {
        c.dot(foot.0 + dx, foot.1 + 1, r.at(Tone::Light), z.hi);
    }
}

/// Feathery gills: three tufts off the neck on one side, streaming back.
fn gills(c: &mut Canvas, k: &Coat, x: i32, y: i32, dir: (i32, i32), flare: bool, z: u8) {
    let g = k.mark;
    let n = if flare { 3 } else { 2 };
    for i in 0..3 {
        let (tx, ty) = (x + dir.0 * (n + i % 2), y + dir.1 * (n + 1) + (i - 1) * 2);
        super::insect::stair(c, (x, y + (i - 1)), (tx, ty), g.at(Tone::Base), z);
        c.dot(tx, ty, g.at(Tone::Light), z);
        c.dot(x + dir.0, y + (i - 1), g.at(Tone::Shade), z);
    }
}

#[allow(clippy::too_many_arguments)]
fn side(c: &mut Canvas, k: &Coat, ax: i32, ay: i32, phase: i32, lift: i32, open: bool, flare: bool, lunge: i32) {
    // The body's spine from the head (east) to the tail (west): each point its girth.
    let mut m = Canvas::new(c.w(), c.h());
    let base = ay - 5;
    let pts: Vec<(i32, i32, i32)> = (0..10)
        .map(|i| {
            let x = ax + 12 + lunge - i * 3;
            let y = base - wave(i, phase) / 2 - if i < 2 { lift } else { 0 };
            let girth = [3, 3, 3, 3, 3, 3, 2, 2, 1, 1][i as usize];
            (x, y, girth)
        })
        .collect();
    // The far legs first, in the body's shade.
    for (li, i) in [(0, 2), (1, 6)] {
        let (x, y, _) = pts[i];
        let reach = if (phase + li) % 2 == 0 { 2 } else { -1 };
        leg(c, k, (x + 1, y), (x + 2 + reach, ay - 1), true);
    }
    for w in pts.windows(2) {
        m.line((w[0].0, w[0].1), (w[1].0, w[1].1), Ix::INK, 2 * w[0].2, 1);
    }
    // The tail's fin, above and below.
    let (tx, ty, _) = pts[9];
    m.polyline_fill(
        &[(pts[6].0, pts[6].1 - 2), (tx - 3, ty - 1), (tx - 3, ty + 1), (pts[6].0, pts[6].1 + 2)],
        Ix::INK,
        1,
    );
    // The head: flat and wide, a blunt snout.
    let (hx, hy, _) = pts[0];
    m.ellipse(Rect::new(hx - 3, hy - 3, 8, 5), Ix::INK, 1);
    c.inflate(&m, k.body, 2, relief::BODY);
    if k.belly != k.body {
        for w in pts.windows(2) {
            c.dye_poly(
                &[(w[0].0, w[0].1 + 1), (w[1].0, w[1].1 + 1), (w[1].0, w[1].1 + w[1].2), (w[0].0, w[0].1 + w[0].2)],
                k.body,
                k.belly,
            );
        }
    }
    // The near legs.
    for (li, i) in [(1, 2), (0, 6)] {
        let (x, y, _) = pts[i];
        let reach = if (phase + li) % 2 == 0 { 2 } else { -1 };
        leg(c, k, (x, y + 1), (x + reach, ay - 1), false);
    }
    let z = relief::HEAD.hi;
    // The mouth's line, dropped on a pink maw when it strikes, and the eye.
    if open {
        c.polyline_fill(&[(hx - 1, hy + 1), (hx + 5, hy), (hx + 4, hy + 3)], Ramp::ClothRose.at(Tone::Base), z);
    } else {
        c.hline(hx - 1, hx + 4, hy + 1, k.body.at(Tone::Deep), z);
    }
    eye(c, k, hx + 2, hy - 2, z);
    gills(c, k, hx - 3, hy - 1, (-1, -1), flare, z);
}

#[allow(clippy::too_many_arguments)]
fn top(
    c: &mut Canvas,
    k: &Coat,
    ax: i32,
    ay: i32,
    toward: bool,
    phase: i32,
    lift: i32,
    open: bool,
    flare: bool,
    lunge: i32,
) {
    // From above: the head toward the viewer (or away), the body running up the screen in an S.
    let dir = if toward { -1 } else { 1 };
    // From above the head keeps its row: a lift or a lunge shows in the jaw and the gills.
    let _ = (lift, lunge);
    let head_y = if toward { ay - 3 } else { ay - 25 };
    let pts: Vec<(i32, i32, i32)> = (0..9)
        .map(|i| {
            let y = head_y + dir * i * 3;
            let x = ax + wave(i, phase) * 3 / 2;
            let girth = [3, 3, 3, 3, 3, 2, 2, 1, 1][i as usize];
            (x, y, girth)
        })
        .collect();
    let mut m = Canvas::new(c.w(), c.h());
    for w in pts.windows(2) {
        m.line((w[0].0, w[0].1), (w[1].0, w[1].1), Ix::INK, 2 * w[0].2, 1);
    }
    let (hx, hy, _) = pts[0];
    m.ellipse(Rect::new(hx - 5, hy - 2, 11, 6), Ix::INK, 1);
    // The legs, out to the sides, paddling in diagonal pairs, splayed like a newt's.
    for (li, i) in [(0, 1), (1, 5)] {
        let (x, y, g) = pts[i];
        for side in [-1, 1] {
            let reach = if (phase + li + i32::from(side > 0)) % 2 == 0 { 2 } else { -1 };
            m.line((x + side * g, y), (x + side * (g + 5), y + reach * dir), Ix::INK, 2, 1);
        }
    }
    c.inflate(&m, k.body, 2, relief::BODY);
    let z = relief::HEAD.hi;
    // Seen from above it read as a flat pale cut-out (the whole-frame pass, 2026-09-27): the back
    // now carries its form. A lit ridge down the spine toward the light, the costal grooves
    // ticked in across each flank, and a few dim mottles on the back, a cave salamander's.
    let r = k.body;
    for (i, &(x, y, g)) in pts.iter().enumerate().skip(1).take(7) {
        let zb = relief::BODY.hi;
        c.fill_rect(Rect::new(x - 1, y, 1, 2), r.at(Tone::Light), zb);
        if g >= 2 {
            // The flank away from the light in shade, two px; the near one lit.
            c.fill_rect(Rect::new(x + g - 1, y, 1, 3), r.at(Tone::Shade), zb);
            c.fill_rect(Rect::new(x + g - 2, y + 1, 1, 2), r.at(Tone::Mid), zb);
            c.dot(x - g + 1, y + 1, r.at(Tone::Mid), zb);
        }
        if i % 3 == 1 {
            let off = if (i as i32 + phase) % 2 == 0 { 1 } else { -2 };
            c.fill_rect(Rect::new(x + off, y + 1, 2, 2), r.at(Tone::Mid), zb);
        }
    }
    for side in [-1, 1] {
        gills(c, k, hx + side * 4, hy, (side, dir), flare, z);
    }
    if toward {
        // The face: two small shining eyes wide apart, the lipless mouth across the snout.
        eye(c, k, hx - 3, hy - 1, z);
        eye(c, k, hx + 2, hy - 1, z);
        if open {
            c.fill_rect(Rect::new(hx - 2, hy + 1, 5, 2), Ramp::ClothRose.at(Tone::Base), z);
            c.hline(hx - 2, hx + 2, hy + 1, Ramp::Bone.at(Tone::Light), z);
        } else {
            c.hline(hx - 3, hx + 3, hy + 2, k.body.at(Tone::Deep), z);
        }
    }
}

fn eye(c: &mut Canvas, k: &Coat, x: i32, y: i32, z: u8) {
    c.set_emitting(k.eye_emits);
    c.dot(x, y, if k.eye_emits { Ramp::Ember.at(Tone::High) } else { Ix::INK }, z);
    c.set_emitting(false);
}

/// On its back: the paler belly up, the legs in the air, the tail slack.
fn dead(c: &mut Canvas, k: &Coat, ax: i32, ay: i32) {
    let mut m = Canvas::new(c.w(), c.h());
    let pts: Vec<(i32, i32)> = (0..9).map(|i| (ax + 12 - i * 3, ay - 3 + wave(i, 1) / 2)).collect();
    for w in pts.windows(2) {
        m.line(w[0], w[1], Ix::INK, 4, 1);
    }
    m.ellipse(Rect::new(pts[0].0 - 2, ay - 6, 7, 5), Ix::INK, 1);
    c.inflate(&m, k.belly, 2, Z::new(2, 4));
    for &i in &[1, 5] {
        let (x, y) = pts[i];
        c.line((x, y - 2), (x - 1, y - 4), k.body.at(Tone::Base), 2, 4);
        c.line((x + 2, y - 2), (x + 3, y - 4), k.body.at(Tone::Shade), 2, 4);
    }
    c.hline(pts[0].0, pts[0].0 + 3, ay - 3, k.body.at(Tone::Deep), 5);
}
