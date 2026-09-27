//! Stones (ART.md §2.3, §2.6): one generator for every rock that stands on the ground, the flora
//! bank's boulders and piles (the lone cliff cells and the rubble) and the kit's boulder, rock,
//! rubble, rock face and heap.
//!
//! A stone is cut, not blown up: its silhouette is a ring of seven to ten hashed vertices round
//! an ellipse, each at its own distance, the bottom sliced flat where it sits on the ground; a
//! top face is the same ring shrunk toward a point up and a little left of the middle, and the
//! band between the two rings is cut into facets, one per edge. Every face is one flat plane
//! with its own normal (the top tipped back to the sky, each side facing out along its edge),
//! toned by the baked light and handed to the light pass as that plane, so a lamp or the sun
//! lights a stone facet by facet. Then the painter's marks: the top's rim catching the light on
//! its lit edges, a crease where a lit face turns into a dark one, a crack with its lit lip,
//! lichen and moss in clusters where the rain sits, a darker band where it meets the ground, and
//! (for a boulder on its own) a pebble or two and a tuft at its foot. Integer only; seeded by
//! stable numbers.

use jane_core::angle::{Angle, cos_q15, sin_q15};
use jane_core::grid::Rect;
use jane_core::num::isqrt;

use crate::canvas::{BAKE_LIGHT, Canvas, UNIT, bresenham, normal};
use crate::hash::{below, h32};
use crate::palette::{Ramp, Tone};

/// Hash salt: stones.
const SALT: u32 = 0x524f_434b;

/// What a stone carries beyond its planes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dress {
    /// Lichen and moss on its top and its lit shoulder.
    pub moss: bool,
    /// A crack (by the seed, one stone in two).
    pub crack: bool,
    /// Creases along the edges between facets, and the top's lit rim: a stone big enough to
    /// carry them (a boulder); small stones keep their planes plain.
    pub edges: bool,
}

/// A stream of hashed numbers from one seed.
struct Dice {
    seed: u32,
    n: u32,
}

impl Dice {
    fn next(&mut self) -> u32 {
        self.n += 1;
        h32(self.seed, self.n, SALT)
    }

    /// `lo..=hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + below(self.next(), (hi - lo + 1).max(1) as u32) as i32
    }
}

/// Twice the signed area of the triangle `a, b, p`: which side of `a -> b` the point lies.
fn side(a: (i32, i32), b: (i32, i32), p: (i32, i32)) -> i64 {
    i64::from(b.0 - a.0) * i64::from(p.1 - a.1) - i64::from(b.1 - a.1) * i64::from(p.0 - a.0)
}

/// Whether pixel centre `p` (in doubled px) is inside the convex polygon `pts` (doubled px),
/// either winding.
fn inside(pts: &[(i32, i32)], p: (i32, i32)) -> bool {
    let (mut pos, mut neg) = (false, false);
    for k in 0..pts.len() {
        let s = side(pts[k], pts[(k + 1) % pts.len()], p);
        pos |= s > 0;
        neg |= s < 0;
    }
    !(pos && neg)
}

/// The tone the baked light gives a plane facing `(nx, ny)` (1/127ths): lit high, dark low.
fn plane_tone(nx: i32, ny: i32) -> Tone {
    let nz = isqrt((UNIT * UNIT - (nx * nx + ny * ny).min(UNIT * UNIT)) as u64) as i32;
    let l = (nx * BAKE_LIGHT[0] + ny * BAKE_LIGHT[1] + nz * BAKE_LIGHT[2]) / UNIT;
    match l {
        l if l > 116 => Tone::High,
        l if l > 100 => Tone::Light,
        l if l > 82 => Tone::Lift,
        l if l > 50 => Tone::Base,
        l if l > 24 => Tone::Mid,
        l if l > -12 => Tone::Shade,
        _ => Tone::Deep,
    }
}

/// One stone in the box `r` (its foot on `r.bottom() - 1`), in `ramp`, cut by `seed`, standing
/// `z` px proud (relief, for the seams between stones in a pile: a stone in front stands two
/// over the one behind). Returns the top face's centre, where a pile's next stone may rest.
pub fn stone(c: &mut Canvas, r: Rect, ramp: Ramp, seed: u32, dress: Dress, z: u8) -> (i32, i32) {
    let mut d = Dice { seed, n: 0 };
    let n = d.range(7, 10);
    let base = r.bottom() - 1;
    // The ellipse's centre and radii, in doubled px so an even box has a half-px centre.
    let (cx2, cy2) = (2 * r.x + r.w, 2 * r.y + r.h);
    let (rx2, ry2) = (r.w - 1, r.h - 1);
    let phase = d.range(0, 65535);
    // One vertex in a big stone is bitten in: a notch where a piece broke away, so the
    // outline is never a cut gem's.
    let notch = if r.w >= 14 { d.range(0, n - 1) } else { -1 };
    let mut outer: Vec<(i32, i32)> = Vec::with_capacity(n as usize);
    for i in 0..n {
        let a = Angle((phase + i * 65536 / n + d.range(-65536 / (5 * n), 65536 / (5 * n))) as u16);
        let (co, si) = (cos_q15(a).0, sin_q15(a).0);
        // Upper vertices reach further out and down ones less: a stone is broad at the
        // shoulder and settles into the ground; each vertex its own reach.
        let reach = if i == notch {
            d.range(70, 78)
        } else if si < 0 {
            d.range(84, 104)
        } else {
            d.range(90, 106)
        };
        let x = cx2 + ((co * rx2 * reach / 100) >> 15);
        // The lower half reaches past the foot and is sliced flat there: it sits on the ground.
        let down = if si > 0 { 118 } else { 100 };
        let y = (cy2 + ((si * ry2 * reach / 100 * down / 100) >> 15)).clamp(2 * r.y + 1, 2 * base + 1);
        outer.push((x, y));
    }
    // The top face's point: up and a little left of the middle, where the rain sits.
    let (tx2, ty2) = (cx2 - rx2 / 6 + d.range(-2, 2), cy2 - ry2 * d.range(30, 42) / 100);
    // A small stone is mostly its sides: its top is a smaller share of it.
    let k = if r.w < 16 { d.range(34, 42) } else { d.range(46, 58) };
    let top: Vec<(i32, i32)> =
        outer.iter().map(|&(x, y)| (tx2 + (x - tx2) * k / 100, ty2 + (y - ty2) * k / 100)).collect();
    // Each face's normal and tone: the top tipped back to the sky, each side facing out along
    // its edge, a little of each face's own so no two neighbours match.
    let tn = (d.range(-14, 4), -d.range(28, 42));
    let mut faces: Vec<((i32, i32), Tone)> = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let (a, b) = (outer[i], outer[(i + 1) % n as usize]);
        let (mx, my) = ((a.0 + b.0) / 2 - tx2, (a.1 + b.1) / 2 - ty2);
        let len = isqrt((i64::from(mx) * i64::from(mx) + i64::from(my) * i64::from(my)) as u64).max(1) as i32;
        // A face turned down the screen leans back less: its foot is on the ground.
        let (sx, sy) = (100, if my > 0 { 62 } else { 92 });
        let nx = mx * sx / len + d.range(-10, 10);
        let ny = my * sy / len + d.range(-10, 10);
        faces.push(((nx, ny), plane_tone(nx, ny)));
    }
    // A top a tone under what the baked light gives it: the sun adds its own on top, and a
    // stone's top is weathered, never white.
    let top_tone = plane_tone(tn.0, tn.1).step(-1);
    c.begin();
    let (x0, x1) = (r.x - 1, r.right() + 1);
    for y in r.y - 1..=base {
        for x in x0..=x1 {
            let p = (2 * x + 1, 2 * y + 1);
            if !inside(&outer, p) {
                continue;
            }
            let (nrm, mut tone) = if inside(&top, p) {
                (tn, top_tone)
            } else {
                // The facet whose wedge from the top's point holds the pixel.
                let f = (0..n as usize)
                    .find(|&i| {
                        let q = [outer[i], outer[(i + 1) % n as usize], top[(i + 1) % n as usize], top[i]];
                        inside(&q, p)
                    })
                    .unwrap_or_else(|| {
                        // A px on a seam between two wedges: the nearest edge's face.
                        (0..n as usize)
                            .min_by_key(|&i| {
                                let (a, b) = (outer[i], outer[(i + 1) % n as usize]);
                                let (mx, my) = ((a.0 + b.0) / 2 - p.0, (a.1 + b.1) / 2 - p.1);
                                mx * mx + my * my
                            })
                            .unwrap_or(0)
                    });
                faces[f]
            };
            // Where it meets the ground it is in its own shade.
            if y >= base - 1 && tone > Tone::Shade {
                tone = tone.step(-1);
            }
            c.put(x, y, ramp.at(tone), normal(nrm.0, nrm.1), z);
        }
    }
    let at = |p: (i32, i32)| (p.0.div_euclid(2), p.1.div_euclid(2));
    if dress.edges {
        // The top's rim catches the light on its lit edges; a crease runs down where a lit face
        // turns into a dark one.
        for i in 0..n as usize {
            let j = (i + 1) % n as usize;
            let ((nx, ny), t) = faces[i];
            let (a, b) = (at(top[i]), at(top[j]));
            if nx + ny < -30 {
                bresenham(a.0, a.1, b.0, b.1, |x, y| {
                    if matches!(Ramp::of(c.get(x, y)), Some((q, _)) if q == ramp) {
                        c.tint(x, y, ramp, top_tone.step(1).min(Tone::High));
                    }
                });
            }
            let (_, t2) = faces[j];
            // Only where a lit face turns into a dark one: a crease on every edge is a gem.
            if t.max(t2) >= Tone::Base && t.min(t2) <= Tone::Mid {
                let dark = t.min(t2).step(-1).max(Tone::Deep);
                let (a, b) = (at(top[j]), at(outer[j]));
                bresenham(a.0, a.1, b.0, b.1, |x, y| {
                    if y < base - 1 && matches!(Ramp::of(c.get(x, y)), Some((q, _)) if q == ramp) {
                        c.tint(x, y, ramp, dark);
                    }
                });
            }
        }
    }
    let (tcx, tcy) = at((tx2, ty2));
    if dress.edges {
        // A pit or two in the top, weathered out: a dark px with its lit lower lip.
        for _ in 0..d.range(1, 2) {
            let (x, y) = (tcx + d.range(-r.w / 5, r.w / 5), tcy + d.range(-1, r.h / 8));
            if matches!(Ramp::of(c.get(x, y)), Some((q, t)) if q == ramp && t == top_tone)
                && matches!(Ramp::of(c.get(x + 1, y + 1)), Some((q, t)) if q == ramp && t == top_tone)
            {
                c.tint(x, y, ramp, top_tone.step(-2));
                c.tint(x + 1, y, ramp, top_tone.step(-1));
                c.tint(x + 1, y + 1, ramp, top_tone.step(1).min(Tone::High));
            }
        }
    }
    if dress.crack && d.range(0, 1) == 0 {
        // A crack from the top's edge down a face, lit on its left lip.
        let mut x = tcx + d.range(-2, 4);
        let len = r.h * d.range(35, 55) / 100;
        for y in tcy + 1..(tcy + 1 + len).min(base - 1) {
            if c.get(x, y).is_opaque() && c.get(x - 1, y).is_opaque() && c.get(x + 1, y).is_opaque() {
                c.tint(x, y, ramp, Tone::Deep);
                c.tint(x - 1, y, ramp, Tone::Light);
            }
            x += d.range(-1, 1);
        }
    }
    if dress.moss {
        // Moss where the rain sits: one or two cushions on the top, over its lit edge, each a
        // lit crown row, a body and a shaded underside; and a rosette or two of pale lichen on
        // the lit side.
        let patches = if r.w >= 20 { d.range(1, 2) } else { 1 };
        for _ in 0..patches {
            // A cushion draped over the crown: across its width it hangs deepest in the middle
            // and thins to its ends, each column its own depth.
            let pw = (r.w / 3 + d.range(-1, 2)).max(4);
            let mx = tcx - pw / 2 + d.range(-r.w / 4, r.w / 10);
            let deep = (r.h / 4).max(2);
            for x in mx..mx + pw {
                let Some(y0) = (r.y - 1..base).find(|&y| c.get(x, y).is_opaque()) else { continue };
                let mid = pw / 2 - (x - mx - pw / 2).abs();
                let hang = (deep * (mid + 1) / (pw / 2 + 1) + d.range(0, 1)).max(1) + 1;
                for y in y0..y0 + hang {
                    if !matches!(Ramp::of(c.get(x, y)), Some((q, _)) if q == ramp) || y >= base - 2 {
                        continue;
                    }
                    let t = if y == y0 {
                        Tone::Light
                    } else if y == y0 + hang - 1 {
                        Tone::Shade
                    } else if x < mx + pw / 3 {
                        Tone::Lift
                    } else {
                        Tone::Base
                    };
                    c.recolour(x, y, Ramp::Marsh.at(t));
                }
            }
        }
        for _ in 0..d.range(0, 1) {
            let (lx, ly) = (r.x + r.w / 4 + d.range(-2, 2), tcy + r.h / 4 + d.range(-1, 2));
            for (dx, dy, t) in [(0, 0, Tone::Lift), (1, 0, Tone::Base), (0, 1, Tone::Mid)] {
                if matches!(Ramp::of(c.get(lx + dx, ly + dy)), Some((q, _)) if q == ramp) && ly + dy < base - 1 {
                    c.recolour(lx + dx, ly + dy, Ramp::LeafOlive.at(t));
                }
            }
        }
    }
    (tcx, tcy)
}

/// A pebble centred on `x` with its foot on row `y`: a stone five or six px across, what a
/// boulder sheds.
pub fn pebble(c: &mut Canvas, x: i32, y: i32, ramp: Ramp, seed: u32, z: u8) {
    let w = 5 + (seed & 1) as i32;
    stone(c, Rect::new(x - w / 2, y - 3, w, 4), ramp, seed, Dress { moss: false, crack: false, edges: false }, z);
}

/// A tuft of grass at `(x, y)` (its foot row): three blades, the middle one tallest.
pub fn tuft(c: &mut Canvas, x: i32, y: i32, z: u8) {
    let g = Ramp::Grass;
    c.begin();
    for (dx, tall, t) in [(-1, 2, Tone::Shade), (0, 4, Tone::Base), (1, 3, Tone::Mid)] {
        for k in 0..tall {
            let tone = if k == tall - 1 { t.step(2) } else { t };
            c.put(x + dx + i32::from(k == tall - 1 && dx != 0) * dx, y - k, g.at(tone), normal(0, -40), z);
        }
    }
}
