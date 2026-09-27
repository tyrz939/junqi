//! `plant` (cactus, flower, the great flower, pumpkin): ART.md §2.2. What should have stayed in
//! the ground and has not: roots at the foot that shuffle it along, a stem or a body that sways a
//! beat behind them, and a head that is the thing's face. The cactus is a ribbed column with two
//! arms, its spines lit, a bloom on its crown and two dark holes and a split for a face. The
//! flower is a stem and two leaves under a ring of petals round a mouth of teeth. The great
//! flower is that at half as large again, two rings of petals, a maw, thorned vines hanging from
//! it and blades of leaves. The pumpkin is a ribbed gourd with a carved face that glows, its stem
//! and a curled vine on top, walking on its tendrils with a hop.

use jane_core::grid::Rect;
use jane_data::Anatomy;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const ROOT: Z = Z::new(1, 2);
    pub const STEM: Z = Z::new(3, 5);
    pub const BODY: Z = Z::new(4, 8);
    pub const HEAD: Z = Z::new(8, 11);
}

/// One beat: the sway of the top (px east), the bob, the roots' shuffle, the head's lean in,
/// and whether the mouth is open.
fn pose(beat: Beat) -> (i32, i32, i32, i32, bool) {
    match beat {
        Beat::Walk(k) => {
            let k = usize::from(k % 6);
            ([0, 1, 1, 0, -1, -1][k], [0, 1, 0, 0, 1, 0][k], [0, 1, 2, 0, -1, -2][k], 0, false)
        }
        Beat::Breathe => (0, 1, 0, 0, false),
        Beat::Idle(0) => (-1, 0, 0, 0, false),
        Beat::Idle(_) => (1, 0, 0, 0, true),
        Beat::Hurt => (-2, 1, 0, -1, true),
        Beat::Attack(0) => (-2, -1, 0, -2, false),
        Beat::Attack(1) => (3, 1, 0, 3, true),
        Beat::Attack(_) => (1, 0, 0, 1, false),
        Beat::Dead => (0, 0, 0, 0, false),
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    if beat == Beat::Dead {
        dead(c, k);
        return;
    }
    let p = pose(beat);
    match k.look.anatomy {
        Anatomy::Cactus => cactus(c, k, facing, p),
        Anatomy::Pumpkin => pumpkin(c, k, facing, p),
        _ => flower(c, k, facing, p),
    }
}

/// Roots spread at the foot, shuffling.
fn roots(c: &mut Canvas, k: &Coat, ax: i32, ay: i32, spread: i32, shuffle: i32) {
    let root = Ramp::Bark;
    for (i, dx) in [-spread, -spread / 2, spread / 2, spread].into_iter().enumerate() {
        let s = if i % 2 == 0 { shuffle } else { -shuffle };
        let lift = i32::from(s > 1);
        c.line((ax + dx / 3, ay - 3), (ax + dx + s, ay - 1 - lift), root.at(Tone::Base), 2, relief::ROOT.hi);
        c.dot(ax + dx / 3, ay - 3, root.at(Tone::Light), relief::ROOT.hi);
    }
    let _ = k;
}

fn eyes_glow(c: &mut Canvas, k: &Coat, pts: &[(i32, i32)], z: u8) {
    c.set_emitting(k.eye_emits);
    let ix = if k.eye_emits { Ramp::Ember.at(Tone::High) } else { Ix::INK };
    for &(x, y) in pts {
        c.fill_rect(Rect::new(x, y, 2, 2), ix, z);
    }
    c.set_emitting(false);
}

fn cactus(c: &mut Canvas, k: &Coat, facing: Facing, (sway, bob, shuffle, lean, open): (i32, i32, i32, i32, bool)) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    roots(c, k, ax, ay, 7, shuffle);
    let top = ay - 30 + bob;
    let x = ax + sway + lean;
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(&[(ax - 5, ay - 2), (ax + 4, ay - 2), (x + 5, top + 5), (x + 3, top), (x - 4, top), (x - 6, top + 5)], Ix::INK, 1);
    // The arms: out and up.
    let (ly, ry) = (ay - 17 + bob, ay - 21 + bob);
    m.polyline_fill(&[(ax - 5, ly), (ax - 10, ly), (ax - 12 + sway, ly - 8), (ax - 9 + sway, ly - 9), (ax - 8, ly - 3), (ax - 5, ly - 3)], Ix::INK, 1);
    m.polyline_fill(&[(ax + 4, ry), (ax + 9, ry), (ax + 11 + sway, ry - 7), (ax + 8 + sway, ry - 8), (ax + 7, ry - 3), (ax + 4, ry - 3)], Ix::INK, 1);
    c.inflate(&m, k.body, 3, relief::BODY);
    // Ribs down the column, spines lit along them.
    for dx in [-3, 0, 3] {
        for y in (top + 3..ay - 3).step_by(1) {
            let t = (y - top) as i64;
            let rx = ax + dx + ((x - ax) as i64 * (ay as i64 - y as i64) / (ay as i64 - top as i64).max(1)) as i32;
            c.tint(rx, y, k.body, Tone::Shade);
            if t % 4 == 1 {
                c.tint(rx - 1, y, k.body, Tone::High);
            }
        }
    }
    // The bloom on its crown.
    c.disc_lit(x, top, 2, k.mark, Z::flat(relief::HEAD.lo));
    c.dot(x, top, k.belly.at(Tone::Light), relief::HEAD.lo);
    if facing != Facing::Up {
        let fy = top + 9;
        let fx = if facing == Facing::Side { x + 1 } else { x - 3 };
        let eyes: &[(i32, i32)] = if facing == Facing::Side { &[(fx + 2, fy)] } else { &[(fx, fy), (fx + 4, fy)] };
        eyes_glow(c, k, eyes, relief::BODY.hi + 1);
        let mw = if open { 3 } else { 1 };
        c.fill_rect(Rect::new(fx + if facing == Facing::Side { 2 } else { 1 }, fy + 4, 4 - i32::from(facing == Facing::Side), mw), k.body.at(Tone::Deep), relief::BODY.hi + 1);
    }
}

fn flower(c: &mut Canvas, k: &Coat, facing: Facing, (sway, bob, shuffle, lean, open): (i32, i32, i32, i32, bool)) {
    let great = k.look.anatomy == Anatomy::GreatFlower;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let s = if great { 3 } else { 2 };
    let sc = |v: i32| v * s / 2;
    roots(c, k, ax, ay, sc(8), shuffle);
    let stem = Ramp::Leaf;
    let (hx, hy) = (ax + sway * s / 2 + lean * s / 2, ay - sc(24) + bob);
    // The stem, curved a little with the sway, and its leaves.
    let mid = (ax + (hx - ax) / 3 - sc(2), (ay + hy) / 2);
    c.line((ax, ay - 2), mid, stem.at(Tone::Base), sc(2).max(2), relief::STEM.lo);
    c.line(mid, (hx, hy + sc(5)), stem.at(Tone::Base), sc(2).max(2), relief::STEM.lo);
    c.line((ax - 1, ay - 2), (mid.0 - 1, mid.1), stem.at(Tone::Light), 1, relief::STEM.hi);
    for side in [-1, 1] {
        let ly = ay - sc(8) - i32::from(side > 0) * sc(4);
        let mut m = Canvas::new(c.w(), c.h());
        let tip = (ax + side * sc(10) - sway, ly - sc(4) + i32::from(side < 0) * sc(1));
        m.polyline_fill(&[(ax, ly), (ax + side * sc(5), ly - sc(3)), tip, (ax + side * sc(6), ly)], Ix::INK, 1);
        c.inflate(&m, stem, 2, relief::STEM);
        c.line((ax, ly), tip, stem.at(Tone::Shade), 1, relief::STEM.hi);
        if great {
            // Thorns along the blade.
            c.dot(ax + side * sc(6), ly - sc(3) - 1, Ramp::Bone.at(Tone::Light), relief::STEM.hi);
        }
    }
    if great {
        // Vines hanging from under the bloom, thorned.
        for dx in [-sc(9), sc(8)] {
            let pts = [(hx + dx / 2, hy + sc(4)), (hx + dx, hy + sc(9)), (hx + dx - sway, hy + sc(15))];
            c.polyline(&pts, stem.at(Tone::Shade), 2, relief::STEM.hi);
            c.dot(pts[1].0 + 1, pts[1].1, Ramp::Bone.at(Tone::Light), relief::STEM.hi);
        }
    }
    // The bloom: petals round a mouth.
    let pr = sc(if great { 10 } else { 7 });
    let petals = if great { 9 } else { 7 };
    let side_on = facing == Facing::Side;
    let z = relief::HEAD;
    for ring in 0..(if great { 2 } else { 1 }) {
        let ramp = if ring == 0 { k.mark } else { k.belly };
        let r = pr - ring * sc(3);
        for i in 0..petals {
            // A petal: an ellipse out from the centre, spaced round (by a table: no floats).
            let (dx, dy) = ring_at(i, petals, r);
            let (dx, dy) = if side_on { (dx / 3, dy) } else { (dx, dy * 3 / 4) };
            let mut m = Canvas::new(c.w(), c.h());
            let pw = sc(4);
            m.ellipse(Rect::new(hx + dx - pw / 2, hy + dy - pw / 2, pw, pw), Ix::INK, 1);
            m.line((hx, hy), (hx + dx, hy + dy), Ix::INK, sc(2), 1);
            c.inflate(&m, ramp, 2, Z::new(z.lo + ring as u8, z.hi + ring as u8));
        }
    }
    if facing == Facing::Up {
        c.disc_lit(hx, hy, sc(2), stem, Z::flat(z.hi + 2));
    } else {
        // The mouth: dark, ringed with teeth; open wider when it bites.
        let mw = sc(if open { 6 } else { 4 });
        let mh = sc(if open { 5 } else { 3 });
        let mouth = Rect::new(hx - mw / 2 + i32::from(side_on) * sc(2), hy - mh / 2, mw, mh);
        c.ellipse(mouth, k.body.at(Tone::Deep), z.hi + 2);
        let teeth = Ramp::Bone;
        for x in (mouth.x + 1..mouth.right() - 1).step_by(2) {
            c.dot(x, mouth.y, teeth.at(Tone::High), z.hi + 2);
            c.dot(x + 1, mouth.bottom() - 1, teeth.at(Tone::Light), z.hi + 2);
        }
        if k.eye_emits {
            c.set_emitting(true);
            c.dot(hx, hy, Ramp::Ember.at(Tone::High), z.hi + 2);
            c.set_emitting(false);
        }
    }
}

/// Point `i` of `n` round a circle of radius `r`, from a table of sixteen directions.
fn ring_at(i: i32, n: i32, r: i32) -> (i32, i32) {
    const DIRS: [(i32, i32); 16] = [
        (0, -100), (38, -92), (71, -71), (92, -38), (100, 0), (92, 38), (71, 71), (38, 92),
        (0, 100), (-38, 92), (-71, 71), (-92, 38), (-100, 0), (-92, -38), (-71, -71), (-38, -92),
    ];
    let (x, y) = DIRS[((i * 16 / n) % 16) as usize];
    (x * r / 100, y * r / 100)
}

fn pumpkin(c: &mut Canvas, k: &Coat, facing: Facing, (sway, bob, shuffle, lean, open): (i32, i32, i32, i32, bool)) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    // It hops on its tendrils: up on the walk's passing beats.
    roots(c, k, ax, ay, 8, shuffle);
    let hop = bob * 2;
    let g = Rect::new(ax - 9 + sway + lean, ay - 18 - hop, 18, 14);
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(g, Ix::INK, 1);
    c.inflate(&m, k.body, 4, relief::BODY);
    // Ribs.
    for dx in [-5, -2, 2, 5] {
        let x = g.x + g.w / 2 + dx;
        for y in g.y + 2..g.bottom() - 2 {
            c.tint(x, y, k.body, Tone::Shade);
        }
        c.tint(x - 1, g.y + 3, k.body, Tone::Light);
    }
    // The stem and a curled vine on top.
    let (sx, sy) = (g.x + g.w / 2, g.y);
    c.fill_rect(Rect::new(sx - 1, sy - 4, 3, 5), Ramp::WoodDark.at(Tone::Base), relief::HEAD.lo);
    c.vline(sx - 1, sy - 4, sy, Ramp::WoodDark.at(Tone::Light), relief::HEAD.lo);
    c.polyline(&[(sx + 1, sy - 3), (sx + 5, sy - 5), (sx + 6, sy - 2), (sx + 4, sy - 1)], Ramp::Leaf.at(Tone::Base), 1, relief::HEAD.lo);
    if facing == Facing::Up {
        return;
    }
    // The carved face: two triangles for eyes, a jagged mouth, lit from inside.
    let z = relief::BODY.hi + 1;
    let fx = g.x + g.w / 2 + if facing == Facing::Side { 3 } else { 0 };
    let ey = g.y + 4;
    let eyes: &[(i32, i32)] = if facing == Facing::Side { &[(fx + 1, ey)] } else { &[(fx - 4, ey), (fx + 2, ey)] };
    eyes_glow(c, k, eyes, z);
    let my = g.y + 9 + i32::from(open);
    let (m0, m1) = if facing == Facing::Side { (fx, fx + 4) } else { (fx - 4, fx + 3) };
    for x in m0..=m1 {
        let tooth = (x - m0) % 3 == 1;
        c.put(x, my, Ramp::Ember.at(if tooth { Tone::Base } else { Tone::Shade }), crate::canvas::FLAT, z);
        if !tooth {
            c.put(x, my + 1, Ramp::Ember.at(Tone::Deep), crate::canvas::FLAT, z);
        }
    }
}

/// Wilted, fallen, split.
fn dead(c: &mut Canvas, k: &Coat) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    match k.look.anatomy {
        Anatomy::Pumpkin => {
            for (dx, w) in [(-9, 9), (1, 8)] {
                let mut m = Canvas::new(c.w(), c.h());
                m.ellipse(Rect::new(ax + dx, ay - 7, w, 7), Ix::INK, 1);
                c.inflate(&m, k.body, 2, Z::new(2, 4));
            }
            c.hline(ax - 6, ax - 2, ay - 4, k.body.at(Tone::Deep), 4);
        }
        Anatomy::Cactus => {
            let mut m = Canvas::new(c.w(), c.h());
            m.ellipse(Rect::new(ax - 12, ay - 7, 24, 7), Ix::INK, 1);
            m.ellipse(Rect::new(ax - 4, ay - 11, 5, 6), Ix::INK, 1);
            c.inflate(&m, k.body, 3, Z::new(2, 4));
            c.hline(ax - 10, ax + 9, ay - 4, k.body.at(Tone::Shade), 4);
        }
        _ => {
            let great = k.look.anatomy == Anatomy::GreatFlower;
            let len = if great { 20 } else { 13 };
            c.line((ax - len, ay - 2), (ax + len / 3, ay - 3), Ramp::Leaf.at(Tone::Shade), 2, 2);
            let mut m = Canvas::new(c.w(), c.h());
            let r = if great { 9 } else { 6 };
            m.ellipse(Rect::new(ax + len / 3, ay - r, 2 * r, r), Ix::INK, 1);
            c.inflate(&m, k.mark, 2, Z::new(2, 4));
            c.ellipse(Rect::new(ax + len / 3 + r - 2, ay - r / 2 - 1, 4, 2), k.body.at(Tone::Deep), 4);
        }
    }
}
