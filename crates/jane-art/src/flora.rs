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

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::angle::{Angle, cos_q15, iatan2, sin_q15};
use jane_core::grid::Rect;
use jane_core::num::isqrt;

use crate::canvas::{BAKE_LIGHT, Canvas, FLAT, UNIT, height_of_rows, normal};
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

/// How a crown is leafed (ART-PLAN Q1): its main ramp, a second ramp that some of its masses
/// take (`share` in 16: an autumn crown is gold beside russet beside a mass still green), and
/// whether its lit rim turns first (`fringe`: a green crown edged in gold).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leafing {
    /// Most of the crown.
    pub main: Ramp,
    /// The other masses, or the rim.
    pub second: Ramp,
    /// Masses in 16 that take `second`.
    pub share: u32,
    /// Whether the lit rim takes `second`.
    pub fringe: bool,
}

impl Leafing {
    /// One ramp all through.
    pub const fn plain(r: Ramp) -> Leafing {
        Leafing { main: r, second: r, share: 0, fringe: false }
    }

    /// `main` with `share` masses in 16 of `second`.
    pub const fn mixed(main: Ramp, second: Ramp, share: u32) -> Leafing {
        Leafing { main, second, share, fringe: false }
    }

    /// `main` with its lit rim in `second`.
    pub const fn fringed(main: Ramp, second: Ramp) -> Leafing {
        Leafing { main, second, share: 0, fringe: true }
    }
}

/// A crown of leaf masses inside the ellipse centred `(cx8, cy8)` with radii `(rx8, ry8)`, all in
/// 1/8 px: a back layer across its top, `n` masses round the middle of mixed sizes and uneven
/// spacing (a big mass beside a small one, a gap where a limb shows), and a front layer low in
/// it. Each pixel belongs to the front-most mass over it and takes its tone from where it sits in
/// that mass (a lit sphere, a crescent of shade under it), in the whole crown (lit at the top and
/// left, a dark core under it) and how far back the mass is. Each mass in front of the back layer
/// has a lit band along its upper-left edge following its lobes, the way a painter lights a
/// clump; dark leaf clusters lie in the shade; the silhouette gets leaf tips and notches. `grow` is
/// the masses' size in 1/16ths. Each mass is leafed by `leaf`: its own ramp, its lit rim turned.
#[allow(clippy::too_many_arguments)]
fn crown(
    c: &mut Canvas,
    (cx8, cy8): (i32, i32),
    (rx8, ry8): (i32, i32),
    n: i32,
    leaf: Leafing,
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
    // Each mass's ramp: the second ramp's share of them, by the mass's own lobes (no dice, so a
    // plain crown draws as it always did).
    let ramps: Vec<Ramp> = puffs
        .iter()
        .enumerate()
        .map(|(i, q)| {
            if leaf.share > 0 && below(h32(i as u32, q.phase as u32, SALT ^ 0x7e), 16) < leaf.share {
                leaf.second
            } else {
                leaf.main
            }
        })
        .collect();
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
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && owner[(y * w + x) as usize] >= 0;
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
            // A fringed crown turns first along its lit rim: within two px of the sky on the
            // crown's upper-left, in clusters of two (never a ruled line).
            let rim = leaf.fringe
                && gx * 3 + gy * 4 < UNIT * 2
                && below(h32((x >> 1) as u32, (y >> 1) as u32, seed ^ 0x51), 4) > 0
                && [(-2, 0), (0, -2), (-1, -1), (2, 0), (0, 2), (-3, 0), (0, -3)]
                    .iter()
                    .any(|&(ex, ey)| !inside(x + ex, y + ey));
            let ramp = if rim { leaf.second } else { ramps[i as usize] };
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
    let lseed = d.next();
    for gy in 0..=(h / 3) {
        for gx in 0..=(w / 3) {
            let hl = h32(gx as u32, gy as u32, lseed);
            let (x, y) = (gx * 3 + (hl % 3) as i32, gy * 3 + ((hl >> 2) % 3) as i32);
            if !inside(x, y) {
                continue;
            }
            let ramp = Ramp::of(c.get(x, y)).map_or(leaf.main, |(r, _)| r);
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
                    let r = Ramp::of(c.get(x + dx, y + dy)).map_or(ramp, |(r, _)| r);
                    c.put(x + dx, y + dy, r.at(base.step(-1)), n, z.saturating_add(1));
                }
            }
        }
    }
}

/// Gaps in a crown where the ground under it shows through: `per_mille` of its leaf px, cut as
/// holes of two to four px well inside its edge, so an airy crown (an ash, a birch) reads as leaf
/// masses with light between them and not as a solid ball. The outline then rims each hole in
/// the crown's own dark: a hole with depth.
fn gaps(c: &mut Canvas, per_mille: i32, seed: u32) {
    if per_mille <= 0 {
        return;
    }
    let (w, h) = (c.w(), c.h());
    let leafy = |c: &Canvas, x: i32, y: i32| Ramp::of(c.get(x, y)).is_some_and(|(r, _)| !is_bark(r));
    let area = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| leafy(c, x, y)).count() as i32;
    let holes = area * per_mille / 1000 / 3;
    let mut made = 0;
    for k in 0..holes * 8 {
        if made >= holes {
            break;
        }
        let hk = h32(k as u32, 0x6a9, seed);
        let (x, y) = (below(hk, w as u32) as i32, below(hk >> 12, h as u32) as i32);
        // Deep inside the leaf, never low in it (where the trunk rises into it).
        let deep = (-3..=3).all(|e: i32| leafy(c, x + e, y) && leafy(c, x, y + e));
        if !deep || y > h * 3 / 5 {
            continue;
        }
        made += 1;
        let shape: &[(i32, i32)] = match (hk >> 28) & 3 {
            0 => &[(0, 0), (1, 0)],
            1 => &[(0, 0), (1, 0), (0, 1)],
            2 => &[(0, 0), (1, 0), (2, 0), (1, 1)],
            _ => &[(0, 0), (0, 1), (1, 1)],
        };
        for &(dx, dy) in shape {
            c.clear_px(x + dx, y + dy);
        }
    }
}

/// Whether `r` is one of the barks a trunk is drawn in (gaps are cut in leaf only).
fn is_bark(r: Ramp) -> bool {
    matches!(r, Ramp::Bark | Ramp::Stone | Ramp::HairWhite | Ramp::HairBlack | Ramp::WoodDark | Ramp::Deadwood)
}

/// How a trunk's bark reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BarkLook {
    /// Ridged: grain runs down it.
    Ridged,
    /// A beech's: smooth, a dark run now and then.
    Smooth,
    /// A silver birch's: white, barred in black.
    Birch,
}

/// A trunk from row `top` to row `bottom` (its foot), `w` px wide round column `cx` at the foot
/// and leaning `lean` px over at the top, flaring into roots over its bottom four rows (`flare`
/// in halves of a px a row) with a root or two running out over the ground; lit on the left with a
/// cylinder's normals, bark by `look`, the crown's shade on its top rows, and two limbs forking up
/// into the crown.
#[allow(clippy::too_many_arguments)]
fn trunk(
    c: &mut Canvas,
    cx: i32,
    (top, bottom): (i32, i32),
    w: i32,
    bark: Ramp,
    seed: u32,
    (lean, flare): (i32, i32),
    look: BarkLook,
) {
    let half = w / 2;
    let span = (bottom - top).max(1);
    let off = |y: i32| lean * (bottom - y) / span;
    for y in top..=bottom {
        let fl = ((y - (bottom - 4)).max(0) * flare + 1) / 2;
        let (x0, x1) = (cx + off(y) - half - fl, cx + off(y) + (w - half) + fl);
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
            let mut ramp = bark;
            match look {
                BarkLook::Ridged => {
                    // Grain: runs of 3 to 7 px down a column, darker or lighter.
                    let run = h32(x as u32, (y + (h32(x as u32, 1, seed) % 7) as i32).div_euclid(7) as u32, seed);
                    if run % 4 == 0 {
                        tone = tone.step(-1);
                    } else if run % 9 == 1 && lx < 20 {
                        tone = tone.step(1);
                    }
                }
                BarkLook::Smooth => {
                    if h32(x as u32, y.div_euclid(9) as u32, seed) % 13 == 0 {
                        tone = tone.step(-1);
                    }
                }
                BarkLook::Birch => {
                    // Black bars across the white, two to four px, every few rows.
                    let band = h32(y.div_euclid(3) as u32, 7, seed);
                    let (bx, bw) = (below(band, w.max(1) as u32) as i32 - half, 2 + below(band >> 8, 3) as i32);
                    let rel = x - (cx + off(y));
                    if band % 3 == 0 && y % 3 == 0 && rel >= bx && rel < bx + bw {
                        ramp = Ramp::HairBlack;
                        tone = if lx < 0 { Tone::Lift } else { Tone::Base };
                    }
                }
            }
            if y < top + 5 {
                tone = tone.step(if y < top + 3 { -2 } else { -1 });
            }
            // A root: a lit bump each side of the flare.
            if fl > 0 && (x == x0 || x == x1 - 1) && y == bottom {
                tone = Tone::Mid;
            }
            c.put(x, y, ramp.at(tone), normal(lx * 9 / 10, 0), (bottom - y + 1).max(1) as u8);
        }
    }
    // Roots running out over the ground from the flare: lit on top, a px or two high.
    if flare > 0 {
        let fl = (4 * flare + 1) / 2;
        for side in [-1i32, 1] {
            let hr = h32((side + 2) as u32, 0x2007, seed);
            let len = 1 + below(hr, 3) as i32;
            let start = if side < 0 { cx - half - fl - 1 } else { cx + (w - half) + fl };
            let t = if side < 0 { Tone::Lift } else { Tone::Mid };
            for i in 0..len {
                let x = start + side * i;
                c.put(x, bottom, bark.at(t), normal(side * 40, -60), 1);
            }
            c.put(start, bottom - 1, bark.at(t.step(1)), normal(side * 40, -60), 2);
        }
    }
    for side in [-1, 1] {
        let (mut x, mut y) = (cx + off(top + 2) + side * (half - 1), top + 2);
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

/// A broadleaf species (ART-PLAN M3): what a tree is drawn as over the one sim tile, chosen by
/// the chunk painter from the biome and what is near (`terrain::standing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Species {
    /// Wide and low, a heavy trunk with a big root flare.
    Oak,
    /// Tall and round, smooth grey bark.
    Beech,
    /// Airy: gaps in the crown, a slender leaning trunk.
    Ash,
    /// Silver birch: narrow and light, a white trunk barred in black.
    Birch,
    /// Field maple: a small round tree of the hedgerows.
    Maple,
    /// Weeping willow, by water: a curtain of leaf to the ground.
    Willow,
    /// Small and wind-bent, in haws.
    Hawthorn,
    /// The churchyard's: dense, near black, low on a red trunk.
    Yew,
    /// Dark and glossy, a cone, red berries.
    Holly,
}

/// A species' shape: its canvas, its crown (centre row, radii, masses, how big the masses are and
/// how much sky shows through), its trunk (top row, width, lean, root flare) and its bark.
#[derive(Clone, Copy, Debug)]
struct Profile {
    w: i32,
    h: i32,
    crown_y: i32,
    rx: i32,
    ry: i32,
    masses: i32,
    grow: i32,
    gaps: i32,
    trunk_top: i32,
    trunk_w: i32,
    lean: i32,
    flare: i32,
    bark: Ramp,
    look: BarkLook,
}

impl Species {
    fn profile(self) -> Profile {
        let p = Profile {
            w: 64,
            h: 80,
            crown_y: 31,
            rx: 27,
            ry: 24,
            masses: 9,
            grow: 16,
            gaps: 0,
            trunk_top: 42,
            trunk_w: 10,
            lean: 0,
            flare: 3,
            bark: Ramp::Bark,
            look: BarkLook::Ridged,
        };
        match self {
            Species::Oak => Profile {
                w: 76,
                crown_y: 34,
                rx: 33,
                ry: 22,
                masses: 11,
                gaps: 20,
                trunk_top: 44,
                trunk_w: 12,
                flare: 4,
                ..p
            },
            Species::Beech => Profile {
                h: 86,
                ry: 27,
                crown_y: 34,
                trunk_top: 50,
                trunk_w: 10,
                lean: 1,
                bark: Ramp::Stone,
                look: BarkLook::Smooth,
                ..p
            },
            Species::Ash => Profile {
                w: 60,
                h: 86,
                crown_y: 30,
                rx: 23,
                ry: 26,
                masses: 9,
                grow: 13,
                gaps: 110,
                trunk_top: 44,
                trunk_w: 7,
                lean: 2,
                flare: 2,
                ..p
            },
            Species::Birch => Profile {
                w: 44,
                h: 82,
                crown_y: 28,
                rx: 16,
                ry: 25,
                masses: 8,
                grow: 16,
                gaps: 35,
                trunk_top: 36,
                trunk_w: 5,
                lean: 3,
                flare: 1,
                bark: Ramp::HairWhite,
                look: BarkLook::Birch,
            },
            Species::Maple => Profile {
                w: 50,
                h: 64,
                crown_y: 24,
                rx: 21,
                ry: 18,
                masses: 7,
                gaps: 15,
                trunk_top: 32,
                trunk_w: 8,
                lean: 1,
                flare: 2,
                ..p
            },
            Species::Willow => Profile {
                w: 70,
                h: 78,
                crown_y: 26,
                rx: 30,
                ry: 19,
                masses: 9,
                grow: 15,
                trunk_top: 38,
                trunk_w: 10,
                lean: 2,
                flare: 3,
                ..p
            },
            Species::Hawthorn => Profile {
                w: 48,
                h: 52,
                crown_y: 20,
                rx: 19,
                ry: 13,
                masses: 7,
                grow: 15,
                gaps: 40,
                trunk_top: 26,
                trunk_w: 6,
                lean: 5,
                flare: 2,
                ..p
            },
            Species::Yew => Profile {
                w: 60,
                h: 66,
                crown_y: 28,
                rx: 26,
                ry: 21,
                masses: 10,
                grow: 17,
                trunk_top: 46,
                trunk_w: 10,
                flare: 3,
                bark: Ramp::WoodDark,
                ..p
            },
            Species::Holly => Profile {
                w: 40,
                h: 64,
                crown_y: 30,
                rx: 15,
                ry: 23,
                masses: 8,
                grow: 15,
                trunk_top: 50,
                trunk_w: 5,
                flare: 1,
                ..p
            },
        }
    }

    /// The name sheets and the presenter know it by.
    pub fn name(self) -> &'static str {
        match self {
            Species::Oak => "oak",
            Species::Beech => "beech",
            Species::Ash => "ash",
            Species::Birch => "birch",
            Species::Maple => "maple",
            Species::Willow => "willow",
            Species::Hawthorn => "hawthorn",
            Species::Yew => "yew",
            Species::Holly => "holly",
        }
    }
}

/// Red beads on the lit side of a crown: haws, holly berries. Each lit on top.
fn beads(c: &mut Canvas, n: i32, d: &mut Dice) {
    let (red, dark) = (letter('r').unwrap_or(Ix::INK), letter('R').unwrap_or(Ix::INK));
    let (w, h) = (c.w(), c.h());
    let mut placed = 0;
    for _ in 0..n * 6 {
        if placed >= n {
            break;
        }
        let (x, y) = (d.range(2, w - 3), d.range(2, h * 2 / 3));
        let leaf = |x: i32, y: i32| Ramp::of(c.get(x, y)).is_some_and(|(r, _)| !is_bark(r));
        if !(leaf(x, y) && leaf(x, y + 1) && leaf(x - 1, y) && leaf(x + 1, y + 1)) {
            continue;
        }
        let lit = Ramp::of(c.get(x, y)).is_some_and(|(_, t)| t >= Tone::Base);
        if !lit && d.range(0, 2) > 0 {
            continue;
        }
        let z = c.height_at(x, y).saturating_add(1);
        c.dot(x, y, red, z);
        c.dot(x, y + 1, dark, z);
        if d.range(0, 2) == 0 {
            c.dot(x + 1, y + 1, red, z);
        }
        placed += 1;
    }
}

/// A glossy crown: a glint here and there on the lit leaves (holly).
fn gloss(c: &mut Canvas, seed: u32) {
    for y in 0..c.h() {
        for x in 0..c.w() {
            let Some((r, t)) = Ramp::of(c.get(x, y)) else { continue };
            if is_bark(r) || t < Tone::Lift || h32(x as u32, y as u32, seed) % 7 != 0 {
                continue;
            }
            let (n, z) = (c.normal_at(x, y), c.height_at(x, y));
            c.put(x, y, r.at(Tone::High), n, z);
        }
    }
}

/// A willow's curtain: strands hanging from the crown's lower edge toward the ground in pairs of
/// columns (a lit px and its shade beside it), of uneven length, a gap between some where the
/// trunk shows, each ending in a lighter tip a px wide; lit on the crown's left, darker on its
/// right.
fn curtain(c: &mut Canvas, (x0, x1): (i32, i32), lowest: i32, leaf: Leafing, seed: u32) {
    let mid = (x0 + x1) / 2;
    let leafy = |c: &Canvas, x: i32, y: i32| Ramp::of(c.get(x, y)).is_some_and(|(r, _)| !is_bark(r));
    let mut x = x0;
    while x < x1 {
        let hx = h32(x as u32, 0x3110, seed);
        let step = 2 + i32::from(hx % 5 == 0);
        if hx % 7 == 0 {
            x += step;
            continue;
        }
        // From the crown's lowest leaf in this column.
        let Some(start) = (0..lowest).rev().find(|&y| leafy(c, x, y)) else {
            x += step;
            continue;
        };
        let reach = (lowest - start) * (35 + below(hx >> 4, 55) as i32) / 100;
        let ramp = if leaf.share > 0 && below(hx >> 12, 16) < leaf.share { leaf.second } else { leaf.main };
        let lit = x < mid;
        for i in 1..=reach {
            let y = start + i;
            let tip = i == reach;
            let (a, b) = if lit { (Tone::Lift, Tone::Mid) } else { (Tone::Mid, Tone::Shade) };
            let a = if tip {
                Tone::Light
            } else if i % 4 == 0 {
                a.step(-1)
            } else {
                a
            };
            let n = normal(if lit { -30 } else { 30 }, 50);
            if !leafy(c, x, y) {
                c.put(x, y, ramp.at(a), n, 1);
            }
            if !tip && !leafy(c, x + 1, y) && i < reach - 1 {
                c.put(x + 1, y, ramp.at(b), n, 1);
            }
        }
        x += step;
    }
}

/// A broadleaf of `species`, leafed by `leaf`. Foot: the trunk's bottom centre.
pub fn tree(seed: u32, species: Species, leaf: Leafing) -> Sprite {
    let mut d = Dice::new(seed);
    let pr = species.profile();
    let (w, h) = (pr.w, pr.h);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (w / 2, h - 2);
    // A lean one way or the other: a hawthorn bent by the wind, an ash reaching for the light.
    let lean = if pr.lean == 0 { 0 } else { pr.lean * if d.range(0, 1) == 0 { -1 } else { 1 } };
    trunk(&mut c, ax, (pr.trunk_top, ay), pr.trunk_w, pr.bark, d.next(), (lean, pr.flare), pr.look);
    let cx8 = (ax + lean) * 8 + d.range(-12, 12);
    crown(&mut c, (cx8, pr.crown_y * 8), (pr.rx * 8, pr.ry * 8), pr.masses, leaf, &mut d, ay, pr.grow);
    gaps(&mut c, pr.gaps, d.next());
    match species {
        Species::Willow => {
            let (x0, x1) = (ax + lean - pr.rx + 3, ax + lean + pr.rx - 3);
            curtain(&mut c, (x0, x1), ay - 6, leaf, d.next());
        }
        Species::Hawthorn => beads(&mut c, 12, &mut d),
        Species::Holly => {
            gloss(&mut c, d.next());
            beads(&mut c, 6, &mut d);
        }
        _ => {}
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A broadleaf. Large: 64 x 80; medium: 48 x 64. Foot: the trunk's bottom centre. The plain
/// round tree the species grew from (kept for the tests of a crown's light).
pub fn broadleaf(seed: u32, large: bool, leaf: Ramp, bark: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = if large { (64, 80) } else { (48, 64) };
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (w / 2, h - 2);
    trunk(
        &mut c,
        ax,
        (if large { 42 } else { 32 }, ay),
        if large { 10 } else { 8 },
        bark,
        d.next(),
        (0, 3),
        BarkLook::Ridged,
    );
    let cx8 = ax * 8 + d.range(-12, 12);
    let (cy, rx, ry, n) = if large { (31, 27, 24, 9) } else { (24, 20, 18, 7) };
    crown(&mut c, (cx8, cy * 8), (rx * 8, ry * 8), n, Leafing::plain(leaf), &mut d, ay, 16);
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
    trunk(&mut c, ax, (62, ay), 6, bark, d.next(), (0, 3), BarkLook::Ridged);
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
    trunk(&mut c, ax, (14, ay), 6, wood, d.next(), (0, 3), BarkLook::Ridged);
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

/// A shrub on one cell, a little wider than it: 28 x 26, its foot a px below the cell, leafed by
/// `leaf` (bronze in October). Berries are a few red beads, each lit on top.
pub fn bush(seed: u32, leaf: Leafing, berries: bool) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (28, 26);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (14, 24);
    let cx8 = ax * 8 + d.range(-4, 4);
    crown(&mut c, (cx8, 13 * 8), (11 * 8, 10 * 8), 5, leaf, &mut d, ay, 20);
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

/// A stump on the wood's floor: a short cut trunk, its top a pale face of rings lit from the
/// top-left, its bark ridged, its roots flaring into the ground, moss on its shaded side. 22 x 18.
pub fn stump(seed: u32, bark: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (22, 18);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (11, 16);
    let tall = d.range(5, 8);
    trunk(&mut c, ax, (ay - tall, ay), 10, bark, d.next(), (0, 3), BarkLook::Ridged);
    // The cut face: an ellipse of pale wood with a ring or two and a crack, lit on its upper left.
    let (fx, fy, rx, ry) = (ax, ay - tall - 1, 5, 2 + d.range(0, 1));
    for y in fy - ry..=fy + ry {
        for x in fx - rx..=fx + rx {
            let (dx, dy) = (x - fx, y - fy);
            let e = dx * dx * 64 / (rx * rx) + dy * dy * 64 / (ry * ry);
            if e > 64 {
                continue;
            }
            let ring = (e / 20) % 2 == 1;
            let t = if e > 48 {
                Tone::Mid
            } else if ring {
                Tone::Base
            } else if dx + dy < 0 {
                Tone::Light
            } else {
                Tone::Lift
            };
            c.put(x, y, Ramp::WoodPale.at(t), normal(dx * 10, -100 + dy * 10), (tall + 2) as u8);
        }
    }
    let crack = d.range(-2, 2);
    c.put(fx + crack, fy, Ramp::WoodPale.at(Tone::Shade), FLAT, (tall + 2) as u8);
    c.put(fx + crack + 1, fy + 1, Ramp::WoodPale.at(Tone::Shade), FLAT, (tall + 2) as u8);
    // Moss down its shaded side.
    for y in fy + ry + 1..ay {
        if d.range(0, 2) > 0 {
            let x = ax + 4 + d.range(0, 1);
            if c.get(x, y).is_opaque() {
                c.put(x, y, Ramp::Marsh.at(if y % 2 == 0 { Tone::Base } else { Tone::Mid }), normal(60, 0), 2);
            }
        }
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A fallen limb lying along the ground: a lit cylinder of bark a few px thick, its broken end a
/// pale splintered face, a twig or two still on it, moss along its top. 34 x 14.
pub fn limb(seed: u32, bark: Ramp) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (34, 14);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (17, 12);
    let (x0, x1) = (3, 30);
    let thick = 4 + d.range(0, 1);
    let tilt = d.range(-1, 1);
    for x in x0..=x1 {
        let mid = ay - thick / 2 - 1 + tilt * (x - x0) / (x1 - x0);
        let th = thick - i32::from(x > x1 - 6);
        for k in 0..th {
            let y = mid - th / 2 + k;
            let ly = (2 * k + 1 - th) * UNIT / th;
            let t = if ly < -50 {
                Tone::Light
            } else if ly < 0 {
                Tone::Base
            } else if ly < 60 {
                Tone::Mid
            } else {
                Tone::Shade
            };
            let grain = h32(x.div_euclid(4) as u32, k as u32, seed) % 5 == 0;
            let t = if grain { t.step(-1) } else { t };
            c.put(x, y, bark.at(t), normal(0, ly * 9 / 10), (th - k + 1) as u8);
        }
        // Moss in tufts along its top.
        if h32(x as u32, 3, seed) % 5 < 2 && x > x0 + 3 {
            c.put(x, mid - th / 2, Ramp::Marsh.at(Tone::Lift), normal(0, -80), (th + 1) as u8);
        }
    }
    // The broken end: pale wood, splintered.
    let my = ay - thick / 2 - 1;
    for k in 0..thick {
        let y = my - thick / 2 + k;
        c.put(x0 - 1, y, Ramp::WoodPale.at(if k < thick / 2 { Tone::Light } else { Tone::Base }), normal(-80, 0), 3);
        if k % 2 == 0 {
            c.put(x0 - 2, y, Ramp::WoodPale.at(Tone::Mid), normal(-80, 0), 3);
        }
    }
    // A twig or two up off it.
    for _ in 0..=d.range(0, 1) {
        let x = d.range(x0 + 6, x1 - 6);
        let (mut px, mut py) = (x, my - thick / 2);
        let dir = if d.range(0, 1) == 0 { -1 } else { 1 };
        for i in 0..4 {
            c.put(
                px,
                py,
                bark.at(if i < 2 { Tone::Base } else { Tone::Lift }),
                normal(dir * 40, -60),
                (thick + i) as u8,
            );
            py -= 1;
            px += dir * i32::from(i % 2 == 0);
        }
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A clump of fronds from one crown: ferns, or bracken turned rust. Each frond arcs up and out
/// from the root as a band three px wide (so the outline leaves it a lit middle), its tip curling
/// over, lit along its upper side on the side to the light; the middle fronds tallest and drawn
/// last, in front. 24 x 18.
pub fn fern(seed: u32, leaf: Leafing) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = (24, 18);
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (12, 16);
    let n = d.range(5, 6);
    // Outer fronds first, so the middle ones lie over them.
    let mut order: Vec<i32> = (0..n).collect();
    order.sort_by_key(|&f| -(f * 2 - (n - 1)).abs());
    for f in order {
        let spread = f * 2 - (n - 1);
        let ramp =
            if leaf.share > 0 && below(h32(f as u32, 5, seed), 16) < leaf.share { leaf.second } else { leaf.main };
        let reach = 5 + (n - spread.abs()) + d.range(0, 2);
        // The frond's spine: out by `spread`, up, then over at the tip.
        let mut pts = Vec::new();
        let (mut x8, mut y8) = (ax * 8 + spread * 3, ay * 8);
        let (mut vx, mut vy) = (spread * 4, -(26 - spread.abs() * 3));
        for _ in 0..reach {
            pts.push((x8 >> 3, y8 >> 3));
            x8 += vx;
            y8 += vy;
            vy += 3;
            vx += spread.signum() * 2;
        }
        let z = |y: i32| (ay - y + 1).clamp(1, 255) as u8;
        for s in pts.windows(2) {
            c.line(s[0], s[1], ramp.at(Tone::Mid), 3, z(s[0].1));
        }
        // The lit rib along it, a px over the spine on the lit side; the leaflets' notches as
        // a darker px every other step underneath.
        let lit = spread <= 0;
        for (i, &(x, y)) in pts.iter().enumerate() {
            let t = if lit { Tone::Light } else { Tone::Base };
            c.put(x, y - 1, ramp.at(t), normal(spread * 12, -70), z(y));
            c.put(x, y, ramp.at(if lit { Tone::Lift } else { Tone::Mid }), normal(spread * 12, -40), z(y));
            if i % 2 == 1 {
                c.put(x, y + 1, ramp.at(Tone::Shade), normal(spread * 12, 60), z(y));
            }
        }
    }
    c.outline();
    stand(&mut c, ay);
    Sprite { canvas: c, ax, ay }
}

/// A stand of tall growth that sways (ART-PLAN M2): reeds with a bulrush or two, or long grass
/// gone to seed. Blades a px wide and two apart from one root, the middle ones tallest, dark at
/// the root and lit at the tip, a bulrush's head or a seed head on some. Not outlined, like the
/// tufts painted in the ground it stands among: an outline round a blade a px wide is all the
/// blade there is. 16 x 24 (reeds) or 16 x 16 (grass).
pub fn stand_of(seed: u32, ramp: Ramp, reeds: bool) -> Sprite {
    let mut d = Dice::new(seed);
    let (w, h) = if reeds { (16, 24) } else { (16, 16) };
    let mut c = Canvas::new(w, h);
    let (ax, ay) = (8, h - 2);
    let n = d.range(4, 5);
    for b in 0..n {
        let spread = b * 2 - (n - 1);
        let len = if reeds { 16 + d.range(0, 4) } else { 9 + d.range(0, 3) } - spread.abs() * 3 / 2;
        let lean = spread.signum();
        let root = ax + spread;
        let mut top = (root, ay);
        for i in 0..len {
            let x = root + lean * (i * i * 3) / (len * len).max(1);
            let y = ay - i;
            let t = if i == len - 1 {
                Tone::Light
            } else if i >= len - 3 {
                Tone::Lift
            } else if i < 2 {
                Tone::Deep
            } else if i < 5 {
                Tone::Shade
            } else if spread < 0 {
                Tone::Base
            } else {
                Tone::Mid
            };
            c.put(x, y, ramp.at(t), normal(lean * 40, -30), (i + 1) as u8);
            top = (x, y);
        }
        // A bulrush's head on a reed, a seed head on grass: two px wide, lit on its left.
        if (reeds && b % 2 == 1) || (!reeds && b % 2 == 0) {
            let (head, k) = if reeds { (Ramp::WoodDark, 4) } else { (Ramp::Thatch, 2) };
            for j in 0..k {
                let y = top.1 + 1 + j - k;
                c.put(
                    top.0,
                    y,
                    head.at(if j == 0 { Tone::Light } else { Tone::Base }),
                    normal(-40, -30),
                    (len + j) as u8,
                );
                c.put(top.0 + 1, y, head.at(Tone::Shade), normal(40, -30), (len + j) as u8);
            }
        }
    }
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

/// What a bank sprite is: the chunk painter asks for one of a kind, and a cell's hash picks
/// which of the kind's variants it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A broadleaf of a species in summer green (a few hold on into October).
    Green(Species),
    /// A broadleaf of a species turned (ART-PLAN Q1): oak russet, beech copper gold, maple and
    /// birch butter yellow, ash a green gone yellow at its edge.
    Turned(Species),
    /// A conifer.
    Pine,
    /// A dead tree.
    Dead,
    /// A shrub in green.
    Bush,
    /// A shrub gone olive.
    BushOlive,
    /// A shrub gone bronze with the autumn.
    BushBronze,
    /// A shrub in berry.
    Berry,
    /// A pile of stones.
    Rocks,
    /// A boulder by a crag.
    Boulder,
    /// A stump on the woodland floor (ART-PLAN M3).
    Stump,
    /// A fallen limb on the woodland floor.
    Limb,
    /// A fern.
    Fern,
    /// Bracken: a fern turned rust.
    Bracken,
    /// Reeds that sway and rustle (ART-PLAN M2).
    Reeds,
    /// Long grass that sways and rustles.
    Grass,
    /// A front garden's boundary, gate or ornament (ART-PLAN M7, `garden`).
    Garden(crate::garden::Piece),
}

impl Kind {
    /// Whether it is a tree (a trunk in the strip, a crown in the canopy, a caster round its foot).
    pub fn is_tree(self) -> bool {
        matches!(self, Kind::Green(_) | Kind::Turned(_) | Kind::Pine | Kind::Dead)
    }

    /// Whether a breeze moves it (ART-PLAN M2): crowns, shrubs, ferns and tall growth.
    pub fn sways(self) -> bool {
        !matches!(self, Kind::Dead | Kind::Rocks | Kind::Boulder | Kind::Stump | Kind::Limb | Kind::Garden(_))
    }
}

/// The broadleaf species, in bank order.
const SPECIES: [Species; 9] = [
    Species::Oak,
    Species::Beech,
    Species::Ash,
    Species::Birch,
    Species::Maple,
    Species::Willow,
    Species::Hawthorn,
    Species::Yew,
    Species::Holly,
];

/// Every flora sprite the chunk painter stamps, built once: about seventy (ART.md §2.6), each
/// kind a run of variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    sprites: Vec<(String, Sprite)>,
    kinds: Vec<(Kind, u16, u16)>,
}

impl Bank {
    /// Build every sprite from `r`.
    pub fn new(r: Ramps) -> Bank {
        let mut b = Bank { sprites: Vec::new(), kinds: Vec::new() };
        let (leaf, olive, wet) = (r.leaf, Ramp::LeafOlive, Ramp::LeafDeep);
        let (beech, oak, maple) = (Ramp::LeafBeech, Ramp::LeafOak, Ramp::LeafMaple);
        for (k, &sp) in SPECIES.iter().enumerate() {
            let seed = 1000 + k as u32 * 211;
            // Summer's leafings, and October's.
            let (green, turned): (&[Leafing], &[Leafing]) = match sp {
                Species::Oak => (
                    &[Leafing::plain(leaf), Leafing::mixed(leaf, olive, 5)],
                    &[Leafing::mixed(oak, beech, 4), Leafing::mixed(oak, olive, 3), Leafing::mixed(beech, oak, 6)],
                ),
                Species::Beech => (
                    &[Leafing::fringed(leaf, beech)],
                    &[Leafing::mixed(beech, oak, 3), Leafing::mixed(beech, maple, 4), Leafing::mixed(beech, oak, 7)],
                ),
                Species::Ash => (
                    &[Leafing::plain(leaf), Leafing::mixed(leaf, olive, 6)],
                    &[Leafing::fringed(olive, maple), Leafing::mixed(maple, olive, 6)],
                ),
                Species::Birch => {
                    (&[Leafing::plain(olive)], &[Leafing::mixed(maple, beech, 3), Leafing::fringed(maple, olive)])
                }
                Species::Maple => {
                    (&[Leafing::fringed(leaf, maple)], &[Leafing::mixed(maple, beech, 4), Leafing::plain(maple)])
                }
                Species::Willow => {
                    (&[Leafing::plain(olive), Leafing::mixed(olive, wet, 6)], &[Leafing::mixed(olive, maple, 5)])
                }
                Species::Hawthorn => {
                    (&[Leafing::mixed(wet, leaf, 6)], &[Leafing::mixed(oak, olive, 6), Leafing::mixed(olive, oak, 5)])
                }
                Species::Yew => (&[Leafing::plain(Ramp::LeafYew), Leafing::mixed(Ramp::LeafYew, r.needle, 4)], &[]),
                Species::Holly => (&[Leafing::plain(wet), Leafing::mixed(wet, Ramp::LeafYew, 5)], &[]),
            };
            b.run(Kind::Green(sp), &format!("tree_{}", sp.name()), green.len(), |i| {
                tree(seed + i * 37, sp, green[i as usize])
            });
            if !turned.is_empty() {
                b.run(Kind::Turned(sp), &format!("tree_{}_turned", sp.name()), turned.len(), |i| {
                    tree(seed + 101 + i * 41, sp, turned[i as usize])
                });
            }
        }
        b.run(Kind::Pine, "pine", 3, |i| pine(9000 + i * 53, r.needle, r.bark));
        b.run(Kind::Dead, "dead_tree", 2, |i| dead_tree(12000 + i * 71, r.dead));
        b.run(Kind::Bush, "bush_leaf", 4, |i| bush(15000 + i * 29, Leafing::plain(r.shrub), false));
        b.run(Kind::BushOlive, "bush_olive", 3, |i| bush(15013 + i * 29, Leafing::mixed(olive, r.shrub, 4), false));
        b.run(Kind::BushBronze, "bush_bronze", 3, |i| bush(16000 + i * 31, Leafing::mixed(oak, beech, 5), false));
        b.run(Kind::Berry, "bush_berry", 2, |i| bush(17000 + i * 31, Leafing::mixed(r.shrub, oak, 3), true));
        b.run(Kind::Rocks, "rocks", 4, |i| rocks(19000 + i * 43, r.stone));
        b.run(Kind::Boulder, "boulder", 6, |i| boulder(21000 + i * 59, r.stone));
        b.run(Kind::Stump, "stump", 2, |i| stump(23000 + i * 61, r.bark));
        b.run(Kind::Limb, "limb", 2, |i| limb(24000 + i * 67, r.bark));
        b.run(Kind::Fern, "fern", 3, |i| fern(25000 + i * 73, Leafing::mixed(r.shrub, leaf, 6)));
        b.run(Kind::Bracken, "bracken", 3, |i| fern(26000 + i * 79, Leafing::mixed(oak, beech, 6)));
        b.run(Kind::Reeds, "reeds", 3, |i| stand_of(27000 + i * 83, Ramp::Reed, true));
        b.run(Kind::Grass, "grass", 3, |i| {
            stand_of(28000 + i * 89, if i == 1 { Ramp::Turf } else { Ramp::TurfDry }, false)
        });
        for piece in crate::garden::Piece::ALL {
            use crate::garden::Piece as G;
            let n = match piece {
                G::RoseArch | G::Hollyhocks | G::Canes | G::Bike => 2,
                _ => 1,
            };
            b.run(Kind::Garden(piece), piece.name(), n, |i| crate::garden::sprite(piece, 29000 + i * 97));
        }
        b
    }

    fn run(&mut self, kind: Kind, name: &str, n: usize, make: impl Fn(u32) -> Sprite) {
        let start = self.sprites.len() as u16;
        for i in 0..n {
            self.sprites.push((format!("{name}_{i}"), make(i as u32)));
        }
        self.kinds.push((kind, start, n as u16));
    }

    /// The sprite of `kind` hash `h` picks, as an index into [`Bank::all`]; a kind the bank has
    /// none of (a yew has no October) falls back to its green.
    pub fn pick(&self, kind: Kind, h: u32) -> u16 {
        let found = self.kinds.iter().find(|k| k.0 == kind).or_else(|| match kind {
            Kind::Turned(sp) => self.kinds.iter().find(|k| k.0 == Kind::Green(sp)),
            _ => None,
        });
        found.map_or(0, |&(_, start, n)| start + below(h, u32::from(n.max(1))) as u16)
    }

    /// The kind of sprite `i`.
    pub fn kind_of(&self, i: u16) -> Kind {
        self.kinds.iter().find(|&&(_, s, n)| i >= s && i < s + n).map_or(Kind::Rocks, |k| k.0)
    }

    /// Sprite `i`.
    pub fn get(&self, i: u16) -> &Sprite {
        &self.sprites[usize::from(i)].1
    }

    /// Every sprite with a name, in bank order: for sheets, goldens and the presenter's atlas.
    pub fn all(&self) -> Vec<(String, &Sprite)> {
        self.sprites.iter().map(|(n, s)| (n.clone(), s)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bank_is_valid_sprites_with_their_feet_inside() {
        let b = Bank::new(Ramps::default());
        let all = b.all();
        // About seventy plants, and the gardens' twenty pieces (ART-PLAN M7).
        assert!((60..=110).contains(&all.len()), "{}", all.len());
        for (name, s) in &all {
            s.canvas.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(s.ax >= 0 && s.ax < s.canvas.w() && s.ay >= 0 && s.ay < s.canvas.h(), "{name}");
            let near = (-2..=2).any(|dx| (0..=4).any(|dy| s.canvas.get(s.ax + dx, s.ay - dy).is_opaque()));
            assert!(near, "{name}: it stands on its foot");
        }
        assert_eq!(b, Bank::new(Ramps::default()), "same ramps, same bytes");
        for (i, (name, _)) in all.iter().enumerate() {
            assert_eq!(
                b.kind_of(i as u16).is_tree(),
                name.starts_with("tree") || name.starts_with("pine") || name.starts_with("dead"),
                "{name}"
            );
        }
    }

    #[test]
    fn every_kind_picks_its_own_and_a_yew_keeps_its_green() {
        let b = Bank::new(Ramps::default());
        for &sp in &SPECIES {
            let i = b.pick(Kind::Green(sp), 7);
            assert_eq!(b.kind_of(i), Kind::Green(sp));
        }
        assert_eq!(b.kind_of(b.pick(Kind::Turned(Species::Yew), 3)), Kind::Green(Species::Yew));
        assert_eq!(b.kind_of(b.pick(Kind::Turned(Species::Oak), 3)), Kind::Turned(Species::Oak));
    }

    #[test]
    fn species_differ_in_silhouette() {
        // Width over height of what is drawn: an oak is wide, a birch and a holly narrow.
        let ratio = |s: Sprite| {
            let r = s.canvas.bounds().expect("drawn");
            r.w * 100 / r.h
        };
        let oak = ratio(tree(1, Species::Oak, Leafing::plain(Ramp::Leaf)));
        let birch = ratio(tree(1, Species::Birch, Leafing::plain(Ramp::Leaf)));
        let holly = ratio(tree(1, Species::Holly, Leafing::plain(Ramp::Leaf)));
        assert!(oak > birch + 30 && oak > holly + 20, "oak {oak}, birch {birch}, holly {holly}");
    }

    #[test]
    fn an_autumn_crown_is_its_ramps() {
        let s = tree(3, Species::Oak, Leafing::mixed(Ramp::LeafOak, Ramp::LeafBeech, 6));
        let count = |r: Ramp| s.canvas.albedo().iter().filter(|&&ix| Ramp::of(ix).is_some_and(|(q, _)| q == r)).count();
        assert!(
            count(Ramp::LeafOak) > 200 && count(Ramp::LeafBeech) > 40,
            "{} {}",
            count(Ramp::LeafOak),
            count(Ramp::LeafBeech)
        );
        assert_eq!(count(Ramp::Leaf), 0);
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
            tree(4, Species::Willow, Leafing::plain(Ramp::LeafOlive)),
            tree(5, Species::Hawthorn, Leafing::plain(Ramp::LeafOlive)),
            pine(1, Ramp::Leaf, Ramp::Bark),
            dead_tree(1, Ramp::Bark),
            bush(1, Leafing::plain(Ramp::Leaf), false),
            stump(1, Ramp::Bark),
            fern(1, Leafing::plain(Ramp::Leaf)),
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
