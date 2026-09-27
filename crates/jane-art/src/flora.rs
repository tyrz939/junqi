//! Flora (ART.md §2.6): the things that stand up out of the ground and are drawn bigger than their
//! one sim cell, the way a 2D Zelda draws them: broadleaf trees, conifers, dead trees, shrubs,
//! stone piles and boulders. The sim cell stays one cell; the sprite stands on it with its crown
//! over the neighbours, and the chunk painter stamps it into the chunk's strips so it y-sorts with
//! units (PRESENTATION.md §1.6).
//!
//! A crown is layered leaf masses: a darker back layer, the main ring and a front layer, each mass
//! a lit sphere with a crescent of shade under it where the mass in front overlaps it, the whole
//! lit from the top-left with a dark core under it; leaves are clusters of two or three px, lit
//! on the lit side and dark on the shadow side, and the silhouette is ragged with leaf tips and
//! notches, never a lollipop disc (ART.md §3.1). Normals are the masses' spheres and height is
//! px above the foot, so the lit tiers see the crown as a volume. Seeded by stable numbers, never
//! the sim's dice; integer only.
//!
//! Units: px at 16 a cell. A sprite's foot `(ax, ay)` is the pixel that stands on the cell's
//! bottom-centre; its height layer is px above that foot.

use jane_core::angle::{Angle, cos_q15, iatan2, sin_q15};
use jane_core::grid::Rect;
use jane_core::num::isqrt;

use crate::canvas::{BAKE_LIGHT, Canvas, UNIT, height_of_rows, normal};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone, letter};
use crate::rock::Dress;

/// Hash salt: flora shapes.
const SALT: u32 = 0x464c_4f52;

/// A flora sprite and its foot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    /// All four layers.
    pub canvas: Canvas,
    /// The foot's x, px from the left.
    pub ax: i32,
    /// The foot's y, px from the top.
    pub ay: i32,
}

/// A stream of hashed numbers from one seed.
struct Dice {
    seed: u32,
    n: u32,
}

impl Dice {
    fn new(seed: u32) -> Dice {
        Dice { seed, n: 0 }
    }

    fn next(&mut self) -> u32 {
        self.n += 1;
        h32(self.seed, self.n, SALT)
    }

    /// `lo..=hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + below(self.next(), (hi - lo + 1).max(1) as u32) as i32
    }
}

/// A tone from a shading value in 1/127ths: lit high, dark low.
fn shade_tone(v: i32) -> Tone {
    match v {
        v if v > 80 => Tone::High,
        v if v > 52 => Tone::Light,
        v if v > 24 => Tone::Lift,
        v if v > -4 => Tone::Base,
        v if v > -36 => Tone::Mid,
        v if v > -66 => Tone::Shade,
        _ => Tone::Deep,
    }
}

/// One leaf mass: centre and radius in 1/8 px, how far back it sits (darker the further), and its
/// lobes: `lobes` bumps round its edge, `phase` where the first one is, so a mass's outline is a
/// scallop of leaf clusters and not a circle.
#[derive(Clone, Copy)]
struct Puff {
    x: i32,
    y: i32,
    r: i32,
    back: i32,
    lobes: i32,
    phase: i32,
}

impl Puff {
    /// A mass with lobes by `d`.
    fn lobed(x: i32, y: i32, r: i32, back: i32, d: &mut Dice) -> Puff {
        Puff { x, y, r, back, lobes: d.range(5, 7), phase: d.range(0, 65535) }
    }

    /// The mass's radius toward `(dx, dy)` (1/8 px), its lobes in it.
    fn reach(&self, dx: i32, dy: i32) -> i32 {
        let a = i32::from(iatan2(dy, dx).0);
        let bump = sin_q15(Angle((a * self.lobes + self.phase) as u16)).0;
        self.r + ((self.r * LOBE / 100 * bump) >> 15)
    }
}

/// How deep a mass's lobes are, per cent of its radius.
const LOBE: i32 = 11;

/// A crown of leaf masses inside the ellipse centred `(cx8, cy8)` with radii `(rx8, ry8)`, all in
/// 1/8 px: a back layer across its top, `n` masses round the middle of mixed sizes and uneven
/// spacing (a big mass beside a small one, a gap where a limb shows), and a front layer low in
/// it. Each pixel belongs to the front-most mass over it and takes its tone from where it sits in
/// that mass (a lit sphere, a crescent of shade under it), in the whole crown (lit at the top and
/// left, a dark core under it) and how far back the mass is. Each mass in front of the back layer
/// has a lit band along its upper-left edge following its lobes, the way a painter lights a
/// clump; dark leaf clusters lie in the shade; the silhouette gets leaf tips and notches. `grow` is
/// the masses' size in 1/16ths.
#[allow(clippy::too_many_arguments)]
fn crown(
    c: &mut Canvas,
    (cx8, cy8): (i32, i32),
    (rx8, ry8): (i32, i32),
    n: i32,
    ramp: Ramp,
    d: &mut Dice,
    foot_y: i32,
    grow: i32,
) {
    let small = rx8.min(ry8);
    let mut puffs: Vec<Puff> = Vec::new();
    // The back layer: small masses along the crown's top, drawn first and darker.
    for i in 0..=(n / 2) {
        let t = i * 2 * 16384 / (n / 2 + 1).max(1) + 49152 - 16384 + d.range(-2400, 2400);
        let a = Angle(t as u16);
        let (ca, sa) = (cos_q15(a).0, sin_q15(a).0);
        let p = Puff::lobed(
            cx8 + ((ca * rx8) >> 15) * 62 / 100,
            cy8 + ((sa * ry8) >> 15) * 70 / 100,
            small * (30 + d.range(0, 14)) / 100 * grow / 16,
            2,
            d,
        );
        puffs.push(p);
    }
    // The main masses: one big one off the middle toward the light, then the ring, its masses
    // big and small by turns and spaced unevenly, pushed out or pulled in.
    let p = Puff::lobed(cx8 - rx8 / 8 + d.range(-4, 4), cy8 - ry8 * 18 / 100, small * 56 / 100 * grow / 16, 1, d);
    puffs.push(p);
    let a0 = d.range(0, 65535);
    let big = d.range(0, 1);
    for i in 0..n {
        let a = Angle((a0 + i * 65536 / n + d.range(-5200, 5200)) as u16);
        let k = 50 + d.range(0, 22);
        let size = if i % 2 == big { 42 + d.range(0, 12) } else { 30 + d.range(0, 8) };
        let (ca, sa) = (cos_q15(a).0, sin_q15(a).0);
        let p = Puff::lobed(
            cx8 + ((ca * rx8) >> 15) * k / 100,
            cy8 + ((sa * ry8) >> 15) * k / 100,
            small * size / 100 * grow / 16,
            1,
            d,
        );
        puffs.push(p);
    }
    // The front layer: two or three masses low in it, in front of everything, off the middle.
    let side = if d.range(0, 1) == 0 { -1 } else { 1 };
    for j in 0..2 + d.range(0, 1) {
        let p = Puff::lobed(
            cx8 + side * (j * 2 - 1) * rx8 * d.range(12, 36) / 100,
            cy8 + ry8 * (22 + d.range(0, 18)) / 100,
            small * (28 + d.range(0, 10)) / 100 * grow / 16,
            0,
            d,
        );
        puffs.push(p);
    }
    // Back to front, then top to bottom within a layer; ties by index, a total key.
    let mut order: Vec<usize> = (0..puffs.len()).collect();
    order.sort_by_key(|&i| (-puffs[i].back, puffs[i].y, i));
    let (w, h) = (c.w(), c.h());
    let mut owner = vec![-1i32; (w * h) as usize];
    for &i in &order {
        let q = puffs[i];
        let reach = q.r * (100 + LOBE) / 100;
        for y in ((q.y - reach) >> 3) - 1..=((q.y + reach) >> 3) + 1 {
            for x in ((q.x - reach) >> 3) - 1..=((q.x + reach) >> 3) + 1 {
                if x < 0 || y < 0 || x >= w || y >= h {
                    continue;
                }
                let (dx, dy) = (x * 8 + 4 - q.x, y * 8 + 4 - q.y);
                let r = q.reach(dx, dy);
                if dx * dx + dy * dy <= r * r {
                    owner[(y * w + x) as usize] = i as i32;
                }
            }
        }
    }
    let seed = d.next();
    let mut vals = vec![0i32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = owner[(y * w + x) as usize];
            if i < 0 {
                continue;
            }
            let q = puffs[i as usize];
            let (dx, dy) = (x * 8 + 4 - q.x, y * 8 + 4 - q.y);
            let r = q.reach(dx, dy).max(1);
            let (lx, ly) = (dx * UNIT / r, dy * UNIT / r);
            let s = (lx * lx + ly * ly).min(UNIT * UNIT);
            let nz = isqrt((UNIT * UNIT - s) as u64) as i32;
            let lam = (lx * BAKE_LIGHT[0] + ly * BAKE_LIGHT[1] + nz * BAKE_LIGHT[2]) / UNIT;
            let (gx, gy) = ((x * 8 + 4 - cx8) * UNIT / rx8.max(1), (y * 8 + 4 - cy8) * UNIT / ry8.max(1));
            let global = -gy * 40 / 100 - gx * 22 / 100;
            // The dark core: under the middle of the crown, where no light gets in.
            let core = {
                let (dx, dy) = (gx, gy - UNIT * 35 / 100);
                let dd = dx * dx + dy * dy * 2;
                if dd < UNIT * UNIT / 5 {
                    -28
                } else if dd < UNIT * UNIT * 2 / 5 {
                    -12
                } else {
                    0
                }
            };
            let jitter = below(h32((x >> 1) as u32, (y >> 1) as u32, seed), 9) as i32 - 4;
            let mut v = lam * 42 / 100 + global + core - q.back * 18 + jitter + 4;
            // The crescent under each mass is what makes a crown read as masses and not a disc.
            if s > UNIT * UNIT * 64 / 100 && ly > UNIT / 6 {
                v -= 40;
            }
            // The lit band: along the mass's upper-left edge, inside its lobes, where the crown
            // is not in its own shade.
            let toward = -(lx * 6 + ly * 8) / 10;
            if q.back < 2 && s > UNIT * UNIT * 30 / 100 && s < UNIT * UNIT * 82 / 100 && toward > UNIT * 45 / 100 {
                v += if v > 10 { 16 } else { 10 };
            }
            vals[(y * w + x) as usize] = v;
            let z = (foot_y - y).max(1) + nz * r / (UNIT * 16);
            // The normal: the whole crown's dome, the mass's own sphere a little over it, so the lit tiers
            // light the crown as one volume with its masses in it, never as a heap of balls.
            let (cxn, cyn) = (gx.clamp(-UNIT, UNIT), gy.clamp(-UNIT, UNIT));
            c.put(
                x,
                y,
                ramp.at(shade_tone(v)),
                normal((cxn * 6 + lx * 4) / 10, (cyn * 6 + ly * 4) / 10),
                z.clamp(1, 255) as u8,
            );
        }
    }
    // Leaves: a jittered 3 px lattice over the crown; at the silhouette a tip reaching out on the
    // lit side or a notch below; inside, a dark cluster here and there in the shade.
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && owner[(y * w + x) as usize] >= 0;
    let lseed = d.next();
    for gy in 0..=(h / 3) {
        for gx in 0..=(w / 3) {
            let hl = h32(gx as u32, gy as u32, lseed);
            let (x, y) = (gx * 3 + (hl % 3) as i32, gy * 3 + ((hl >> 2) % 3) as i32);
            if !inside(x, y) {
                continue;
            }
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| !inside(x + dx, y + dy));
            let v = vals[(y * w + x) as usize];
            let base = shade_tone(v);
            let z = c.height_at(x, y);
            if edge {
                // Out along the outward side: a tip on the lit side of the crown, a notch below.
                let (ox, oy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .copied()
                    .find(|&(dx, dy)| !inside(x + dx, y + dy))
                    .unwrap_or((0, -1));
                match (hl >> 5) % 4 {
                    0 | 1 if oy <= 0 => {
                        let t = base.step(i32::from(ox + oy < 0));
                        c.put(x + ox, y + oy, ramp.at(t), normal(ox * 80, oy * 80), z);
                        if (hl >> 8) & 1 == 0 {
                            c.put(
                                x + ox + oy.abs() * ox.signum(),
                                y + oy - ox.abs(),
                                ramp.at(t),
                                normal(ox * 80, oy * 80),
                                z,
                            );
                        }
                    }
                    2 if oy >= 0 && v < 0 => c.clear_px(x, y),
                    _ => {}
                }
                continue;
            }
            if (hl >> 5) % 4 != 0 || v >= -30 {
                continue;
            }
            // Dark leaves in a mass's shade.
            let lam = {
                let [nx, ny, nz] = crate::canvas::decode(c.normal_at(x, y));
                (nx * BAKE_LIGHT[0] + ny * BAKE_LIGHT[1] + nz * BAKE_LIGHT[2]) / UNIT
            };
            if lam >= 0 {
                continue;
            }
            for (dx, dy) in [(0, 0), (1, 1)] {
                if inside(x + dx, y + dy) {
                    let n = c.normal_at(x + dx, y + dy);
                    c.put(x + dx, y + dy, ramp.at(base.step(-1)), n, z.saturating_add(1));
                }
            }
        }
    }
}

/// A trunk from row `top` to row `bottom` (its foot), `w` px wide round column `cx`, flaring
/// into roots over its bottom four rows; lit on the left with a cylinder's normals, bark in grain
/// runs down it, the crown's shade on its top rows, and two limbs forking up into the crown.
fn trunk(c: &mut Canvas, cx: i32, top: i32, bottom: i32, w: i32, bark: Ramp, seed: u32) {
    let half = w / 2;
    for y in top..=bottom {
        let flare = ((y - (bottom - 4)).max(0) * 3 + 1) / 2;
        let (x0, x1) = (cx - half - flare, cx + (w - half) + flare);
        for x in x0..x1 {
            let lx = ((2 * (x - x0) + 1 - (x1 - x0)) * UNIT / (x1 - x0).max(1)).clamp(-UNIT, UNIT);
            let mut tone = if lx < -60 {
                Tone::Light
            } else if lx < -20 {
                Tone::Lift
            } else if lx < 30 {
                Tone::Base
            } else if lx < 80 {
                Tone::Mid
            } else {
                Tone::Shade
            };
            // Grain: runs of 3 to 7 px down a column, darker or lighter.
            let run = h32(x as u32, (y + (h32(x as u32, 1, seed) % 7) as i32).div_euclid(7) as u32, seed);
            if run % 4 == 0 {
                tone = tone.step(-1);
            } else if run % 9 == 1 && lx < 20 {
                tone = tone.step(1);
            }
            if y < top + 5 {
                tone = tone.step(if y < top + 3 { -2 } else { -1 });
            }
            // A root: a lit bump each side of the flare.
            if flare > 0 && (x == x0 || x == x1 - 1) && y == bottom {
                tone = Tone::Mid;
            }
            c.put(x, y, bark.at(tone), normal(lx * 9 / 10, 0), (bottom - y + 1).max(1) as u8);
        }
    }
    for side in [-1, 1] {
        let (mut x, mut y) = (cx + side * (half - 1), top + 2);
        for i in 0..6 {
            c.put(
                x,
                y,
                bark.at(if side < 0 { Tone::Lift } else { Tone::Mid }),
                normal(side * 60, -40),
                (bottom - y + 1) as u8,
            );
            c.put(x + side, y, bark.at(Tone::Shade), normal(side * 60, -40), (bottom - y + 1) as u8);
            y -= 1;
            x += side * i32::from(i % 2 == 1);
        }
    }
}

/// Stands a plant's heights in the one projection (ART.md §1.1): its generators write a px `r`
/// rows over the foot row `ay` as about `r` px up (a crown's dome a few px over that); the px is
/// `height_of_rows(r)` up, exactly, so its every column lands on the foot row in a lit tier's
/// field and a crown floats over its trunk as high as it is drawn. The dome is not kept: it
/// stood the middle of a crown, the part over the trunk, a few rows in front of the rest, where
/// the trunk held it to the ground, and a low sun drew it as a streak beside the crown's shadow.
/// A crown's depth is its caster's.
///
/// It stands on its lowest drawn row ([`base`]), which is the foot row `ay` for a tree (its
/// trunk's root) and a little above it for a shrub (the crown's rim, over the contact shadow):
/// a shrub counted from its foot hovered a few px over the ground in the lit tiers, and the
/// ground drawn under its rim lay inside it, in its shadow.
fn stand(c: &mut Canvas, ay: i32) {
    let (w, h) = (c.w(), c.h());
    let b = base(c, ay);
    c.heights_by(Rect::new(0, 0, w, h), |_, y| height_of_rows((b - y).max(0)));
}

/// The lowest row of `c` at or above `ay` with a drawn px (the contact shadow is not drawn): what
/// a plant stands on.
pub fn base(c: &Canvas, ay: i32) -> i32 {
    (0..=ay.min(c.h() - 1)).rev().find(|&y| (0..c.w()).any(|x| c.get(x, y).is_opaque())).unwrap_or(ay)
}

/// A broadleaf. Large: 64 x 80; medium: 48 x 64. Foot: the trunk's bottom centre.
pub fn broadleaf(seed: u32, large: bool, leaf: Ramp, bark: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = if large { (64, 80) } else { (48, 64) };
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (w / 2, h - 2);
    trunk(&mut c, ax, if large { 42 } else { 32 }, ay, if large { 10 } else { 8 }, bark, d.next());
    let cx8 = ax * 8 + d.range(-12, 12);
    let (cy, rx, ry, n) = if large { (31, 27, 24, 9) } else { (24, 20, 18, 7) };
    crown(&mut c, (cx8, cy * 8), (rx * 8, ry * 8), n, leaf, &mut d, ay, 16);
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A conifer: drooping tiers on a short trunk, none the same: each leans a px or so off the
/// stem, is wider on one side than the other, and has a ragged edge of branch ends that hang at
/// its skirt; the top is a thin leader. Lit on its left and dark on its right, the skirt of each
/// tier in the shade of the one above. 44 x 84.
pub fn pine(seed: u32, needle: Ramp, bark: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (44, 84);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (w / 2, h - 2);
    trunk(&mut c, ax, 62, ay, 6, bark, d.next());
    let seed2 = d.next();
    let tiers = [(3, 17, 7), (12, 20, 11), (24, 21, 14), (36, 22, 17), (49, 22, 20)];
    for (k, (top, tall, hm)) in tiers.into_iter().enumerate() {
        let top = top + d.range(-1, 1);
        let hm = hm + d.range(-1, 1);
        // Off the stem a little, and wider one side than the other.
        let lean = if k == 0 { 0 } else { d.range(-1, 1) };
        let (wl, wr) = (100 + d.range(-12, 12), 100 + d.range(-12, 12));
        for yy in 0..tall {
            let y = top + yy;
            let t = yy + 1;
            // An eased half-width: quick at the tip, slower toward the tier's skirt.
            let half = hm * t * (2 * tall - t) / (tall * tall);
            let f = t * UNIT / tall;
            // Branch ends: every other row pair a jag in or out, more toward the skirt.
            let jag = |side: u32| {
                let hj = h32((y >> 1) as u32, (k as u32) << 1 | side, seed2);
                let n = below(hj, 3) as i32 - 1;
                if yy > tall / 3 { n } else { n.min(0) }
            };
            let (l, r) = (half * wl / 100 + jag(0), half * wr / 100 + jag(1));
            let mid = ax + lean * yy / tall;
            for x in (mid - l - 1)..=(mid + r) {
                let span = if x < mid { l } else { r }.max(1);
                let lx = ((2 * (x - mid) + 1) * UNIT / (2 * span)).clamp(-UNIT - 1, UNIT + 1);
                // The skirt: needle tips hang below the tier's edge in 2 px teeth.
                let edge = lx.abs() > UNIT * 80 / 100;
                let tooth = h32((x >> 1) as u32, k as u32, seed2) % 3;
                if edge && yy > tall - 4 && tooth == 0 {
                    continue;
                }
                if lx.abs() > UNIT && !(yy > tall - 3 && tooth == 1) {
                    continue;
                }
                let droop = if f > UNIT * 80 / 100 { 56 } else { 0 };
                let jitter = below(h32((x >> 1) as u32, (y >> 1) as u32, seed2), 13) as i32 - 6;
                let v = -lx * 6 / 10 + (UNIT - f) * 4 / 10 - droop + jitter - 16 - k as i32 * 4;
                let n = normal(lx * 8 / 10, 30 + f * 50 / UNIT);
                c.put(x, y, needle.at(shade_tone(v)), n, (ay - y).clamp(1, 255) as u8);
            }
        }
        // Needle clusters: short strokes slanting down and out, lit on the left half.
        for s in 0..hm {
            let hs = h32(s as u32, k as u32, seed2 ^ 0x77);
            let yy = 4 + below(hs, (tall - 6).max(1) as u32) as i32;
            let half = hm * (yy + 1) * (2 * tall - yy - 1) / (tall * tall);
            if half < 2 {
                continue;
            }
            let x = ax - half + 1 + below(hs.rotate_right(8), (2 * half - 2).max(1) as u32) as i32;
            let side = if x < ax { -1 } else { 1 };
            let t = if side < 0 { Tone::Light } else { Tone::Shade };
            for i in 0..2 {
                let (px, py) = (x + side * i, top + yy + i);
                if c.get(px, py).is_opaque() {
                    let z = c.height_at(px, py);
                    c.put(px, py, needle.at(t), normal(side * 60, 40), z);
                }
            }
        }
    }
    // The leader: a thin spike above the top tier.
    for y in 0..4 {
        c.put(ax, y, needle.at(if y < 2 { Tone::Mid } else { Tone::Base }), normal(0, -40), (ay - y) as u8);
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A dead tree: a grey trunk and a few bare limbs. 44 x 68.
pub fn dead_tree(seed: u32, wood: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (44, 68);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (w / 2, h - 2);
    trunk(&mut c, ax, 14, ay, 6, wood, d.next());
    // Limbs: a walk up and out, the first steps thick.
    let mut limbs: Vec<(i32, i32, i32, i32, i32)> =
        vec![(ax - 2, 30, -1, 7, 2), (ax + 2, 24, 1, 7, 2), (ax, 16, 0, 6, 2), (ax - 2, 42, -1, 4, 1)];
    let mut k = 0;
    while k < limbs.len() && k < 12 {
        let (x0, y0, dir, len, pen) = limbs[k];
        let (mut fx, mut fy) = (x0 * 8, y0 * 8);
        let lean = if dir == 0 { d.range(-4, 4) } else { dir * 8 };
        for i in 0..len {
            let (px, py) = (fx >> 3, fy >> 3);
            fx += lean * d.range(7, 13) / 10;
            fy -= d.range(5, 11);
            let p = if i < 3 { pen } else { 1 };
            let tone = if i < 2 { Tone::Base } else { Tone::Lift };
            c.line((px, py), (fx >> 3, fy >> 3), wood.at(tone), p, (ay - (fy >> 3)).clamp(1, 255) as u8);
            if i > 2 && limbs.len() < 12 && d.range(0, 3) == 0 {
                limbs.push((fx >> 3, fy >> 3, -dir.signum(), 3, 1));
            }
        }
        k += 1;
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A shrub on one cell, a little wider than it: 28 x 26, its foot a px below the cell. Berries are
/// a few red beads, each lit on top.
pub fn bush(seed: u32, ramp: Ramp, berries: bool) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (28, 26);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (14, 24);
    let cx8 = ax * 8 + d.range(-4, 4);
    crown(&mut c, (cx8, 13 * 8), (11 * 8, 10 * 8), 5, ramp, &mut d, ay, 20);
    if berries {
        let (red, dark) = (letter('r').unwrap_or(Ix::INK), letter('R').unwrap_or(Ix::INK));
        for _ in 0..5 {
            let (x, y) = (d.range(6, 20), d.range(5, 16));
            if c.get(x, y).is_opaque() && c.get(x, y + 1).is_opaque() {
                let z = c.height_at(x, y).saturating_add(1);
                c.dot(x, y, red, z);
                c.dot(x, y + 1, dark, z);
            }
        }
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A pile of two or three stones on one cell: 26 x 22, each a faceted stone of its own
/// (`rock::stone`), the ones behind first, a front stone standing proud of them so the line
/// between them is a seam.
pub fn rocks(seed: u32, stone: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (26, 22);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (13, 20);
    let mut stones = vec![
        Rect::new(2 + d.range(0, 3), 8 + d.range(0, 2), 14 + d.range(-1, 1), 12),
        Rect::new(12 + d.range(0, 2), 11 + d.range(0, 1), 11 + d.range(-1, 1), 10),
    ];
    if d.range(0, 9) < 6 {
        stones.push(Rect::new(7 + d.range(0, 4), 3 + d.range(0, 2), 10 + d.range(-1, 1), 10));
    }
    // Back ones first: the higher a stone's foot, the further back it lies.
    stones.sort_by_key(|r| (r.bottom(), r.x));
    let small = crate::rock::Dress { moss: false, crack: false, edges: false };
    for (i, r) in stones.into_iter().enumerate() {
        let r = Rect::new(r.x, r.y, r.w, r.h.min(ay + 1 - r.y));
        crate::rock::stone(
            &mut c,
            r,
            stone,
            d.next(),
            Dress { moss: i == 0 && d.range(0, 2) == 0, ..small },
            2 + 2 * i as u8,
        );
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A boulder, standing on one cell and overhanging it: 34 x 30, cut into faces by
/// `rock::stone` (a lit top tipped to the sky, sides facing out along their edges, a crease
/// where a lit face turns dark, a crack, lichen and moss where the rain sits), sitting on a
/// flat foot with a pebble or two and a tuft beside it. No two are the same shape.
pub fn boulder(seed: u32, stone: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (34, 30);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (17, 28);
    let (bw, bh) = (d.range(26, 31), d.range(21, 26));
    let x = ax - bw / 2 + d.range(-1, 1);
    crate::rock::stone(
        &mut c,
        Rect::new(x, ay + 1 - bh, bw, bh),
        stone,
        d.next(),
        // Two boulders in three carry moss: a crag's fallen stones are not a matched set.
        Dress { moss: d.range(0, 2) > 0, crack: true, edges: true },
        6,
    );
    // What it shed and what grows at its foot: a pebble on one side, a tuft on the other.
    let left = d.range(0, 1) == 0;
    let (px, tx) = if left { (x - 2, x + bw - 2) } else { (x + bw + 1, x + 2) };
    if d.range(0, 3) > 0 {
        crate::rock::pebble(&mut c, px.clamp(3, w - 4), ay, stone, d.next(), 2);
    }
    if d.range(0, 2) > 0 {
        crate::rock::tuft(&mut c, tx.clamp(2, w - 3), ay, 3);
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// The ramps a bank is drawn in: from the tree, pine, bush, dead tree and rubble looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramps {
    /// An ordinary wood's leaf; the olive and the dark wet-wood greens go beside it.
    pub leaf: Ramp,
    /// Trunks.
    pub bark: Ramp,
    /// Conifers.
    pub needle: Ramp,
    /// Shrubs.
    pub shrub: Ramp,
    /// Dead trees.
    pub dead: Ramp,
    /// Stone piles and boulders.
    pub stone: Ramp,
}

impl Default for Ramps {
    fn default() -> Ramps {
        Ramps {
            leaf: Ramp::Leaf,
            bark: Ramp::Bark,
            needle: Ramp::Needle,
            shrub: Ramp::Shrub,
            dead: Ramp::Deadwood,
            stone: Ramp::Stone,
        }
    }
}

/// Every flora sprite the chunk painter stamps, built once: 46 of them (ART.md §2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    /// Large broadleaves by leaf (ordinary, olive, wet), four of each.
    pub large: [[Sprite; 4]; 3],
    /// Medium broadleaves by leaf, three of each.
    pub medium: [[Sprite; 3]; 3],
    /// Conifers.
    pub pines: [Sprite; 3],
    /// Dead trees.
    pub dead: [Sprite; 2],
    /// Shrubs by leaf (ordinary, olive), four of each.
    pub bushes: [[Sprite; 4]; 2],
    /// Shrubs in berry.
    pub berry: [Sprite; 2],
    /// Stone piles.
    pub rocks: [Sprite; 4],
    /// Boulders: six, so neighbours by a crag are seldom twins.
    pub boulders: [Sprite; 6],
}

impl Bank {
    /// Build every sprite from `r`.
    pub fn new(r: Ramps) -> Bank {
        let leaves = [r.leaf, Ramp::LeafOlive, Ramp::LeafDeep];
        let shrubs = [r.shrub, Ramp::LeafOlive];
        Bank {
            large: std::array::from_fn(|p| {
                std::array::from_fn(|i| broadleaf(1000 + p as u32 * 37 + i as u32 * 101, true, leaves[p], r.bark))
            }),
            medium: std::array::from_fn(|p| {
                std::array::from_fn(|i| broadleaf(5000 + p as u32 * 41 + i as u32 * 97, false, leaves[p], r.bark))
            }),
            pines: std::array::from_fn(|i| pine(9000 + i as u32 * 53, r.needle, r.bark)),
            dead: std::array::from_fn(|i| dead_tree(12000 + i as u32 * 71, r.dead)),
            bushes: std::array::from_fn(|p| {
                std::array::from_fn(|i| bush(15000 + p as u32 * 13 + i as u32 * 29, shrubs[p], false))
            }),
            berry: std::array::from_fn(|i| bush(17000 + i as u32 * 31, r.shrub, true)),
            rocks: std::array::from_fn(|i| rocks(19000 + i as u32 * 43, r.stone)),
            boulders: std::array::from_fn(|i| boulder(21000 + i as u32 * 59, r.stone)),
        }
    }

    /// Every sprite with a name, in bank order: for sheets and goldens.
    pub fn all(&self) -> Vec<(String, &Sprite)> {
        let mut out: Vec<(String, &Sprite)> = Vec::new();
        let leaf = ["leaf", "olive", "wet"];
        for (p, row) in self.large.iter().enumerate() {
            out.extend(row.iter().enumerate().map(|(i, s)| (format!("tree_large_{}_{i}", leaf[p]), s)));
        }
        for (p, row) in self.medium.iter().enumerate() {
            out.extend(row.iter().enumerate().map(|(i, s)| (format!("tree_medium_{}_{i}", leaf[p]), s)));
        }
        out.extend(self.pines.iter().enumerate().map(|(i, s)| (format!("pine_{i}"), s)));
        out.extend(self.dead.iter().enumerate().map(|(i, s)| (format!("dead_tree_{i}"), s)));
        for (p, row) in self.bushes.iter().enumerate() {
            out.extend(row.iter().enumerate().map(|(i, s)| (format!("bush_{}_{i}", leaf[p]), s)));
        }
        out.extend(self.berry.iter().enumerate().map(|(i, s)| (format!("bush_berry_{i}"), s)));
        out.extend(self.rocks.iter().enumerate().map(|(i, s)| (format!("rocks_{i}"), s)));
        out.extend(self.boulders.iter().enumerate().map(|(i, s)| (format!("boulder_{i}"), s)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bank_is_forty_six_valid_sprites_with_their_feet_inside() {
        let b = Bank::new(Ramps::default());
        let all = b.all();
        assert_eq!(all.len(), 46);
        for (name, s) in &all {
            s.canvas.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(s.ax >= 0 && s.ax < s.canvas.w() && s.ay >= 0 && s.ay < s.canvas.h(), "{name}");
            let near = (-2..=2).any(|dx| (0..=4).any(|dy| s.canvas.get(s.ax + dx, s.ay - dy).is_opaque()));
            assert!(near, "{name}: it stands on its foot");
        }
        assert_eq!(b, Bank::new(Ramps::default()), "same ramps, same bytes");
    }

    #[test]
    fn a_crown_is_lit_from_the_top_left_and_dark_in_its_core() {
        let s = broadleaf(1, true, Ramp::Leaf, Ramp::Bark);
        let tone = |x, y| Ramp::of(s.canvas.get(x, y)).map_or(0, |(_, t)| t as i32);
        let (mut tl, mut br) = (0, 0);
        for y in 10..22 {
            for x in 12..26 {
                tl += tone(x, y);
                br += tone(x + 26, y + 18);
            }
        }
        assert!(tl > br, "top-left {tl}, bottom-right {br}");
        assert!(s.canvas.height_at(32, 10) > s.canvas.height_at(32, 70));
    }

    #[test]
    fn a_plant_stands_in_the_one_projection() {
        use crate::canvas::rows_up;
        let plants = [
            broadleaf(1, true, Ramp::Leaf, Ramp::Bark),
            broadleaf(2, false, Ramp::Leaf, Ramp::Bark),
            pine(1, Ramp::Leaf, Ramp::Bark),
            dead_tree(1, Ramp::Bark),
            bush(1, Ramp::Leaf, false),
        ];
        for s in plants {
            // A tree stands on its foot row; a shrub on the rim of its crown, over its contact
            // shadow.
            let b = base(&s.canvas, s.ay);
            assert!(b <= s.ay && b >= s.ay - 6, "{b} for a foot on {}", s.ay);
            for y in 0..b {
                for x in 0..s.canvas.w() {
                    if !s.canvas.get(x, y).is_opaque() {
                        continue;
                    }
                    // Exactly its rows' true height over what it stands on: a lit tier stands
                    // every column of it on that row.
                    let h = i32::from(s.canvas.height_at(x, y));
                    assert_eq!(rows_up(h), b - y, "({x}, {y}): {h} px up, {} rows over its base", b - y);
                }
            }
        }
        assert_eq!(base(&broadleaf(1, true, Ramp::Leaf, Ramp::Bark).canvas, 78), 78);
        // A broadleaf's crown floats: the lowest px of a column clear of the trunk is well up.
        let s = broadleaf(1, true, Ramp::Leaf, Ramp::Bark);
        let low = (0..s.canvas.h()).rev().find(|&y| s.canvas.get(s.ax - 20, y).is_opaque()).unwrap_or(0);
        assert!(
            s.canvas.height_at(s.ax - 20, low) > 30,
            "the crown's edge is {} up",
            s.canvas.height_at(s.ax - 20, low)
        );
    }
}
