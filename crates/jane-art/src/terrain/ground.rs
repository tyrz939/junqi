//! Passes 1 to 3: the open ground's base tones, its edges, and its calm, clustered detail.
//!
//! The craft rules (ART.md §3.1) this pass holds to: texture is clusters of 2 px or more with a
//! lit edge and a shadow edge, never per-pixel speckle; calm ground is left calm; tone changes
//! over many cells (lush and dry grass, worn earth) with meandering, clustered boundaries and no
//! checker; every transition has a 1 to 2 px rim; decals (tufts, flowers, fallen leaves,
//! pebbles, cracks) are scattered by hash in clusters. Everything is a function of world px and
//! the seed, and detail is drawn from the cells one beyond the chunk too, so nothing is cut at
//! a seam.

use jane_core::Tile;
use jane_data::TilePattern as P;

use super::{CELL, CHUNK_CELLS, CHUNK_PX, MM, NONE, Painter, fast, salt};
use crate::canvas::{FLAT, Normal, UNIT, normal};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone, letter};

/// The ground's broad patches: a lattice every 64 px, a finer one every 16, and lush against dry
/// every 256.
const PATCH_SHIFT: u32 = 6;
const FINE_SHIFT: u32 = 4;
const LUSH_SHIFT: u32 = 8;

pub(super) fn paint(p: &mut Painter, x0: i32, y0: i32, seed: u32) {
    p.s.ly.clear();
    // Over the surface map (the chunk and a cell round it), so the margin's detail agrees with the
    // neighbouring chunk's.
    let (mx, my) = (x0 * CELL - CELL, y0 * CELL - CELL);
    p.s.patch.fill(mx, my, MM, MM, PATCH_SHIFT, seed ^ salt::PATCH);
    p.s.fine.fill(mx, my, MM, MM, FINE_SHIFT, (seed ^ salt::PATCH).rotate_left(7));
    p.s.lush.fill(mx, my, MM, MM, LUSH_SHIFT, (seed ^ salt::PATCH).rotate_left(13));
    base(p, x0 * CELL, y0 * CELL, seed);
    edges(p);
    for cy in -1..=CHUNK_CELLS {
        for cx in -1..=CHUNK_CELLS {
            detail(p, cx, cy, x0 + cx, y0 + cy, seed);
        }
    }
}

/// A tone in `Mid, Base, Lift` from a patch value 0..=255 and its meander: `Base` between `lo`
/// and `hi`.
#[inline]
fn patch_tone(v: i32, m: i32, lo: i32, hi: i32) -> Tone {
    let c = (v - 128) * 2 + 128 + m;
    if c < lo {
        Tone::Mid
    } else if c > hi {
        Tone::Lift
    } else {
        Tone::Base
    }
}

fn base(p: &mut Painter, wx0: i32, wy0: i32, seed: u32) {
    // The fields over the chunk, a row at a time: patches (two parts broad to one fine), the
    // meander's smooth part and the lush drifts.
    for buf in [&mut p.s.pv, &mut p.s.mv, &mut p.s.lv] {
        buf.fill(0);
    }
    p.s.patch.add_box(wx0, wy0, CHUNK_PX, CHUNK_PX, 2, &mut p.s.pv);
    p.s.fine.add_box(wx0, wy0, CHUNK_PX, CHUNK_PX, 1, &mut p.s.pv);
    p.s.wob_x.add_box(wx0, wy0, CHUNK_PX, CHUNK_PX, 1, &mut p.s.mv);
    p.s.lush.add_box(wx0, wy0, CHUNK_PX, CHUNK_PX, 1, &mut p.s.lv);
    for y in 0..CHUNK_PX {
        for x in 0..CHUNK_PX {
            let g = p.surf_px(x, y);
            if g == NONE {
                continue;
            }
            let st = *p.styles.id(g);
            let (wx, wy) = (wx0 + x, wy0 + y);
            let i = (y * CHUNK_PX + x) as usize;
            let v = p.s.pv[i] / 3;
            let cluster = below(fast((wx >> 1) as u32, (wy >> 1) as u32, seed ^ salt::PATCH), 13) as i32 - 6;
            let m = (p.s.mv[i] - 128) / 4 + cluster;
            // Dry grass by broad drifts, their edge wandering a little; the green just short of it
            // lightens toward it, so the one grass turns to the other without a seam.
            let drift = p.s.lv[i] + m / 6 - 176;
            let dry = drift > 0;
            let mut r = st.ramp;
            let z = i32::from(st.row.rise).max(1);
            let (tone, n) = match st.row.pattern {
                P::Water => (water_tone(p, x, y, v, m), FLAT),
                P::Ice => (if v + m / 2 > 150 { Tone::High } else { Tone::Light }, FLAT),
                P::Setts => setts(wx, wy, seed, (v - 128) * 2 + 128 + m),
                P::Soil => soil(wy, v, m),
                P::Marsh => {
                    let t = patch_tone(v, m, 84, 196);
                    (if v + m / 2 < 86 { Tone::Shade } else { t }, FLAT)
                }
                // A meadow is calm: its tones change over wide drifts, and the dry grass keeps
                // closer to its middle tone than the green does.
                P::Turf if dry => {
                    r = Ramp::TurfDry;
                    (patch_tone(v, m, -1000, 236), FLAT)
                }
                P::Turf if drift > -12 => (patch_tone(v, m, 24, 224).step(1).min(Tone::Lift), FLAT),
                P::Turf => (patch_tone(v, m, 24, 224), FLAT),
                _ => (patch_tone(v, m, 18, 188), FLAT),
            };
            p.s.ly.put(x, y, r.at(tone), n, z);
        }
    }
}

/// Water's tone by depth: pale in the shallows, deepening with the px to the bank and the cells
/// to the shore, the bands' edges wandering in clusters.
fn water_tone(p: &Painter, x: i32, y: i32, v: i32, m: i32) -> Tone {
    let shore = p.shore_px(x, y);
    // The cells' depth, bilinear between cell centres so the deep has no cell edges in it.
    let (gx, gy) = (x - CELL / 2, y - CELL / 2);
    let (cx, cy) = (gx.div_euclid(CELL), gy.div_euclid(CELL));
    let (fx, fy) = (gx.rem_euclid(CELL), gy.rem_euclid(CELL));
    let dep = |a: i32, b: i32| i32::from(p.s.depth[Painter::at(a, b)]);
    let top = dep(cx, cy) * (CELL - fx) + dep(cx + 1, cy) * fx;
    let bottom = dep(cx, cy + 1) * (CELL - fx) + dep(cx + 1, cy + 1) * fx;
    let cell10 = (top * (CELL - fy) + bottom * fy) * 10 / (CELL * CELL);
    let d = if shore < 16 { shore } else { 16 + (cell10 - 10).max(0) };
    // Near the bank the bands wander in clusters; out in the deep only broad slow swells move them.
    let d = if d < 22 { d + (v - 128) / 10 + m / 6 } else { d + (v - 128) / 6 };
    match d {
        d if d < 4 => Tone::Light,
        d if d < 10 => Tone::Lift,
        d if d < 22 => Tone::Base,
        d if d < 46 => Tone::Mid,
        _ => Tone::Shade,
    }
}

/// Setts: courses 7 px tall of stones 7 to 12 px wide, a joint round each, each stone a tone of
/// its own, lit along its top and left edges, shaded along its bottom, domed for the light pass.
fn setts(wx: i32, wy: i32, seed: u32, patch: i32) -> (Tone, Normal) {
    const H: i32 = 7;
    let course = wy.div_euclid(H);
    let yy = wy.rem_euclid(H);
    let xs = wx + (fast(course as u32, 0, seed ^ salt::CELL) % 23) as i32;
    let block = xs.div_euclid(36);
    let lx = xs.rem_euclid(36);
    let h = fast(block as u32, course as u32, seed ^ salt::CELL);
    let c1 = 7 + (h % 6) as i32;
    let c2 = c1 + 7 + (h >> 4) as i32 % 6;
    let c3 = c2 + 7 + (h >> 8) as i32 % 6;
    let (start, end, k) = if lx < c1 {
        (0, c1, 0)
    } else if lx < c2 {
        (c1, c2, 1)
    } else if lx < c3 {
        (c2, c3, 2)
    } else {
        (c3, 36, 3)
    };
    if yy == H - 1 || lx == end - 1 {
        return (Tone::Shade, normal(0, 0));
    }
    let body = match (h >> (12 + k * 3)) & 7 {
        0 => Tone::Mid,
        1 | 2 => Tone::Lift,
        _ => Tone::Base,
    };
    // Broad wear over many stones: damp and dark in the hollows, bleached on the crowns.
    let body = if patch < 46 {
        body.step(-1)
    } else if patch > 210 {
        body.step(1)
    } else {
        body
    };
    // The stone's dome: normals from its middle, in 1/127ths.
    let w = end - 1 - start;
    let nx = ((2 * (lx - start) + 1 - w) * 36 / w.max(1)).clamp(-36, 36);
    let ny = ((2 * yy + 1 - (H - 1)) * 36 / (H - 1)).clamp(-36, 36);
    let t = if yy == 0 || lx == start {
        body.step(1)
    } else if yy == H - 2 || lx == end - 2 {
        body.step(-1)
    } else {
        body
    };
    (t, normal(nx, ny))
}

/// Dug soil in rows 6 px apart: each ridge lit along its top, its furrow in shade.
fn soil(wy: i32, v: i32, m: i32) -> (Tone, Normal) {
    match wy.rem_euclid(6) {
        5 => (Tone::Shade, normal(0, -50)),
        4 => (Tone::Mid, normal(0, -20)),
        0 => (Tone::Lift, normal(0, 40)),
        _ => (patch_tone(v, m, 50, 210), FLAT),
    }
}

/// Edges, read off the surface map. Water: a dark band under its bank, a pale lip everywhere
/// else. Land: a dark wet band where it meets water; a shadow under a higher neighbour to the
/// north; a rim where it drops to lower ground, lit where it faces north and dark where it faces
/// south, its normal tilted over the drop.
fn edges(p: &mut Painter) {
    let water = |p: &Painter, g: u8| g != NONE && p.styles.id(g).is_water();
    let rise = |p: &Painter, g: u8| i32::from(p.styles.id(g).row.rise);
    for y in 0..CHUNK_PX {
        for x in 0..CHUNK_PX {
            // No edge falls in a cell whose eight neighbours are all its own surface.
            if !p.s.mixed[Painter::at(x / CELL, y / CELL)] {
                continue;
            }
            let m = p.surf_px(x, y);
            if m == NONE {
                continue;
            }
            let (up, dn, lf, rt) = (p.surf_px(x, y - 1), p.surf_px(x, y + 1), p.surf_px(x - 1, y), p.surf_px(x + 1, y));
            if water(p, m) {
                let (up2, up3) = (p.surf_px(x, y - 2), p.surf_px(x, y - 3));
                if !water(p, up) {
                    set_tone(p, x, y, Tone::Deep);
                } else if !water(p, up2) {
                    set_tone(p, x, y, Tone::Shade);
                } else if !water(p, up3) {
                    p.s.ly.step(x, y, -1);
                } else if [dn, lf, rt].iter().any(|&n| n != NONE && !water(p, n)) {
                    set_tone(p, x, y, Tone::High);
                } else {
                    let (dn2, lf2, rt2) = (p.surf_px(x, y + 2), p.surf_px(x - 2, y), p.surf_px(x + 2, y));
                    if [dn2, lf2, rt2].iter().any(|&n| n != NONE && !water(p, n)) {
                        set_tone(p, x, y, Tone::Light);
                    }
                }
                continue;
            }
            // Land by water: the wet band, two px dark then one.
            let near =
                |d: i32| [(0, d), (0, -d), (d, 0), (-d, 0)].iter().any(|&(dx, dy)| water(p, p.surf_px(x + dx, y + dy)));
            if near(1) {
                p.s.ly.step(x, y, -2);
                continue;
            }
            if near(2) {
                p.s.ly.step(x, y, -1);
                continue;
            }
            let rm = rise(p, m);
            let up2 = p.surf_px(x, y - 2);
            if up != NONE && up != m && rise(p, up) > rm {
                p.s.ly.step(x, y, -2);
            } else if up2 != NONE && up2 != m && rise(p, up2) > rm && up != NONE {
                p.s.ly.step(x, y, -1);
            } else if dn != NONE && rise(p, dn) < rm {
                p.s.ly.step(x, y, -2);
                p.s.ly.tilt(x, y, normal(0, UNIT * 7 / 10));
            } else if up != NONE && rise(p, up) < rm {
                p.s.ly.step(x, y, 2);
                p.s.ly.tilt(x, y, normal(0, -UNIT * 7 / 10));
            } else if lf != NONE && rise(p, lf) < rm {
                p.s.ly.step(x, y, 1);
                p.s.ly.tilt(x, y, normal(-UNIT * 7 / 10, 0));
            } else if rt != NONE && rise(p, rt) < rm {
                p.s.ly.step(x, y, -1);
                p.s.ly.tilt(x, y, normal(UNIT * 7 / 10, 0));
            } else {
                let dn2 = p.surf_px(x, y + 2);
                if dn2 != NONE && dn != NONE && rise(p, dn2) < rm {
                    p.s.ly.step(x, y, -1);
                } else if up2 != NONE && up != NONE && rise(p, up2) < rm {
                    p.s.ly.step(x, y, 1);
                }
            }
        }
    }
}

/// Set a pixel to `tone` of its own ramp.
fn set_tone(p: &mut Painter, x: i32, y: i32, tone: Tone) {
    if let Some((r, _)) = p.s.ly.tone(x, y) {
        p.s.ly.ink(x, y, r.at(tone));
    }
}

/// Detail on one ground cell, `(cx, cy)` chunk-local (it may be one outside the chunk: what it
/// draws inside the chunk is drawn the same from both sides of the seam), `(wx, wy)` world.
fn detail(p: &mut Painter, cx: i32, cy: i32, wx: i32, wy: i32, seed: u32) {
    let k = Painter::at(cx, cy);
    let g = p.s.surf[k];
    if g == NONE {
        return;
    }
    let st = *p.styles.id(g);
    let h = h32(wx as u32, wy as u32, seed ^ salt::CELL);
    let (px, py) = (cx * CELL, cy * CELL);
    let z = i32::from(st.row.rise).max(1);
    let r = st.ramp;
    let cluster = p.s.fine.at(wx * CELL + 8, wy * CELL + 8);
    match st.row.pattern {
        P::Turf => turf(p, cx, cy, wx, wy, g, h, z, cluster, seed),
        P::Earth | P::Gravel => {
            // Pebbles in clusters; now and then a crack in the dry of a lane.
            let (odds, n) = if st.row.pattern == P::Gravel { (2, 4) } else { (5, 3) };
            if h % odds == 0 {
                let (ax, ay) =
                    (px + 3 + below(h.rotate_right(4), 10) as i32, py + 3 + below(h.rotate_right(8), 10) as i32);
                for s in 0..2 + below(h.rotate_right(12), n) as i32 {
                    let hs = h32(h, s as u32, 1);
                    let (x, y) = (ax + below(hs, 7) as i32 - 3, ay + below(hs.rotate_right(8), 5) as i32 - 2);
                    pebble(p, x, y, g, hs >> 16 & 3 == 0, z);
                }
            }
            if st.row.pattern == P::Earth && (h >> 20) % 11 == 3 {
                crack(
                    p,
                    px + 2 + below(h.rotate_right(5), 12) as i32,
                    py + 3 + below(h.rotate_right(9), 10) as i32,
                    g,
                    h,
                    6,
                );
            }
        }
        P::Sand => {
            // A wind ripple: a lit crest over its shade, a run of 6 to 11 px.
            if h & 3 != 0 {
                let (x, y) = (px + below(h.rotate_right(4), 8) as i32, py + 3 + below(h.rotate_right(8), 10) as i32);
                let len = 6 + (h >> 12) as i32 % 6;
                for i in 0..len {
                    let yy = y + i32::from((i + (h >> 16) as i32 % 3) % 6 == 0) - i32::from(i > len - 3);
                    if own(p, x + i, yy, g) && own(p, x + i, yy + 1, g) {
                        p.s.ly.step(x + i, yy, 1);
                        p.s.ly.step(x + i, yy + 1, -1);
                    }
                }
            }
            if (h >> 20) % 23 == 0 {
                pebble(p, px + 5 + (h >> 3) as i32 % 6, py + 6, g, true, z);
            }
        }
        P::Marsh => {
            if h & 3 == 0 {
                puddle(p, px + 2 + (h >> 4) as i32 % 8, py + 3 + (h >> 8) as i32 % 9, g, h);
            } else if h & 7 == 5 || cluster > 170 && h & 3 == 1 {
                tuft(p, px + 4 + (h >> 5) as i32 % 8, py + 10 + (h >> 9) as i32 % 5, g, Ramp::Reed, 4, h >> 11, z, 5);
            }
        }
        P::Cracked => {
            for c in 0..2 {
                let hc = h32(h, c, 4);
                crack(p, px + below(hc, 16) as i32, py + below(hc.rotate_right(4), 16) as i32, g, hc, 10);
            }
        }
        P::Soil => {
            // Seedlings along the rows, every other cell.
            if (wx + wy) & 1 == 0 {
                let leaf = st.accent.unwrap_or(Ramp::Crop);
                for s in 0..2 {
                    let x = px + 2 + s * 7 + (h >> (s * 3)) as i32 % 4;
                    let y = py + 3 + 6 * s;
                    sprout(p, x, y, g, leaf, z);
                }
            }
        }
        P::Setts => {
            // Weeds in the joints: along the square's edge where the grass creeps in, and now and
            // then a tuft far out on it.
            let edge =
                [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| p.s.surf[Painter::at(cx + dx, cy + dy)] != g);
            if (edge && h % 3 == 1) || h % 73 == 1 {
                let (x, y) = (px + 1 + (h >> 3) as i32 % 13, py + 1 + (h >> 7) as i32 % 13);
                for (dx, dy, t) in [(0, 0, Tone::Shade), (1, 0, Tone::Mid), (0, -1, Tone::Base), (1, -1, Tone::Light)] {
                    if own(p, x + dx, y + dy, g) {
                        p.s.ly.put(x + dx, y + dy, Ramp::Turf.at(t), FLAT, z + 1);
                    }
                }
            }
        }
        P::Water => {
            // Ripples on open water, in trains where the wind catches it: a lit crest over the
            // shade behind it, two or three in a train, each a little shorter.
            if cluster > 150 && h % 3 == 0 && p.s.depth[k] >= 2 {
                let (x, y) = (px + 1 + (h >> 3) as i32 % 8, py + 2 + (h >> 7) as i32 % 8);
                for r in 0..2 + (h >> 20) as i32 % 2 {
                    let len = 5 - r + (h >> (11 + r)) as i32 % 3;
                    let (rx, ry) = (x + r * 2 + (h >> (14 + r)) as i32 % 2, y + r * 3);
                    for i in 0..len {
                        if own(p, rx + i, ry, g) && own(p, rx + i, ry + 1, g) {
                            p.s.ly.step(rx + i, ry, if i == len / 2 { 3 } else { 2 });
                            p.s.ly.step(rx + i, ry + 1, -1);
                        }
                    }
                }
            }
        }
        P::Ice => {
            if h & 3 == 0 {
                let (x, y) = (px + (h >> 3) as i32 % 10, py + 2 + (h >> 7) as i32 % 10);
                for i in 0..5 {
                    if own(p, x + i, y - i / 2, g) {
                        p.s.ly.step(x + i, y - i / 2, 2);
                    }
                }
            }
        }
        _ => {}
    }
    let _ = r;
}

/// Whether chunk-local px `(x, y)` is on surface `g`.
#[inline]
fn own(p: &Painter, x: i32, y: i32, g: u8) -> bool {
    p.surf_px(x, y) == g
}

/// Grass: calm, with tufts in clusters where it grows long, flowers in the meadows, fallen
/// leaves near trees, and blades hanging over the lip where a lane cuts in.
#[allow(clippy::too_many_arguments)]
fn turf(p: &mut Painter, cx: i32, cy: i32, wx: i32, wy: i32, g: u8, h: u32, z: i32, cluster: i32, seed: u32) {
    let (px, py) = (cx * CELL, cy * CELL);
    // Tufts: none where the grass is short, up to two where it grows long, now and then one.
    let n = ((cluster - 140) / 40).clamp(0, 2) + i32::from(h % 9 == 0);
    for t in 0..n as u32 {
        let ht = h32(h, t, 8);
        let (x, y) = (px + 2 + below(ht, 12) as i32, py + 5 + below(ht.rotate_right(8), 10) as i32);
        let r = p.s.ly.tone(x, y).map_or(Ramp::Turf, |(r, _)| r);
        tuft(
            p,
            x,
            y,
            g,
            r,
            3 + below(ht.rotate_right(16), 3) as i32,
            ht >> 20,
            z,
            3 + below(ht.rotate_right(24), 2) as i32,
        );
    }
    // Meadow patches where the flowers are thick, one colour to a patch.
    let meadow = h32((wx >> 3) as u32, (wy >> 3) as u32, seed ^ salt::CELL ^ 0x0a0a) & 255;
    if meadow < 30 && h % 3 == 0 {
        let colour = FLOWERS[(meadow % 5) as usize];
        let (ax, ay) = (px + 3 + below(h.rotate_right(5), 10) as i32, py + 3 + below(h.rotate_right(9), 10) as i32);
        for f in 0..3 + below(h.rotate_right(13), 3) {
            let hf = h32(h, f, 9);
            flower(p, ax + below(hf, 7) as i32 - 3, ay + below(hf.rotate_right(8), 5) as i32 - 2, g, colour, z);
        }
    } else if h % 97 == 11 {
        flower(p, px + 4 + (h >> 3) as i32 % 8, py + 6, g, FLOWERS[((h >> 1) % 5) as usize], z);
    }
    // Fallen leaves, the more the nearer the trees.
    let trees = (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| p.s.raw[Painter::at(cx + dx, cy + dy)] == Tile::Tree)
        .count() as u32;
    if trees > 0 {
        let leaves = trees / 3 + u32::from(h % 5 < trees.min(4));
        for l in 0..leaves {
            let hl = h32(h, l, 10);
            leaf(p, px + below(hl, 15) as i32, py + below(hl.rotate_right(8), 15) as i32, g, hl >> 16, z);
        }
    }
    // Blades over the lip where lower ground cuts in: south and east, and a few to the north.
    for (dx, dy) in [(0, 1), (1, 0), (-1, 0), (0, -1)] {
        let n = p.s.surf[Painter::at(cx + dx, cy + dy)];
        if n == g || n == NONE || i32::from(p.styles.id(n).row.rise) >= i32::from(p.styles.id(g).row.rise) {
            continue;
        }
        let ho = h32(h, (dx + 2 * dy + 3) as u32, 11);
        for b in 0..=below(ho, 2) {
            let hb = h32(ho, b, 12);
            let along = 2 + below(hb, 12) as i32;
            let (x, y) = match (dx, dy) {
                (0, 1) => (px + along, py + CELL - 1),
                (0, -1) => (px + along, py + 1),
                (1, 0) => (px + CELL - 1, py + along),
                _ => (px, py + along),
            };
            if own(p, x, y, g) {
                let r = p.s.ly.tone(x, y).map_or(Ramp::Turf, |(r, _)| r);
                overhang(p, x, y, (dx, dy), r, hb >> 8, z);
            }
        }
    }
}

/// Flower colours: white, yellow, red, violet, orange, from the TS build's base colours.
const FLOWERS: [char; 5] = ['w', 'y', 'r', 'p', 'o'];

/// A tuft: `n` blades from one root, fanning out, the middle ones tallest, dark at the root and
/// lit at the tip; `len` px at the most. Only rooted on the surface `g`.
#[allow(clippy::too_many_arguments)]
fn tuft(p: &mut Painter, x: i32, y: i32, g: u8, r: Ramp, n: i32, h: u32, z: i32, len: i32) {
    if !own(p, x, y, g) {
        return;
    }
    for b in 0..n {
        let spread = b * 2 - n + 1;
        let hb = h32(h, b as u32, 7);
        let l = (len - spread.abs() / 2 + (hb % 2) as i32).max(2);
        let lean = spread.signum() * i32::from(spread.abs() > 1 || hb & 2 == 0);
        for i in 0..l {
            let bx = x + spread / 2 + lean * (i * 2 / l);
            let by = y - i;
            // Dry blades' tips stop short of the ramp's palest, which reads as litter on the grass.
            let tone = if i == l - 1 {
                if r == Ramp::TurfDry { Tone::Lift } else { Tone::High }
            } else if i == l - 2 {
                Tone::Light
            } else if i == 0 {
                Tone::Shade
            } else {
                Tone::Mid
            };
            p.s.ly.put(bx, by, r.at(tone), normal(lean * 40, -40), z + 1);
        }
    }
    // The shade at its root.
    for dx in -1..=1 {
        p.s.ly.step(x + dx, y + 1, -1);
    }
}

/// Blades hanging over the lip toward `(dx, dy)`: rooted one px inside, reaching 1 to 3 px over.
fn overhang(p: &mut Painter, x: i32, y: i32, (dx, dy): (i32, i32), r: Ramp, h: u32, z: i32) {
    for b in 0..3i32 {
        let reach = 1 + below(h32(h, b as u32, 13), 3) as i32;
        let side = b - 1;
        for i in 0..=reach {
            let (bx, by) = if dx == 0 { (x + side, y + dy * i) } else { (x + dx * i, y + side) };
            let tone = if i == reach {
                Tone::Light
            } else if i == 0 {
                Tone::Mid
            } else {
                Tone::Base
            };
            p.s.ly.put(bx, by, r.at(tone), normal(dx * 40, dy * 40), z + 1);
        }
    }
}

/// A flower: four petals round a yellow eye and a leaf under it; its shade to the right below.
fn flower(p: &mut Painter, x: i32, y: i32, g: u8, c: char, z: i32) {
    if !own(p, x, y, g) || !own(p, x, y + 2, g) {
        return;
    }
    let petal = letter(c).unwrap_or(Ix::INK);
    let eye = letter(if c == 'y' { 'o' } else { 'y' }).unwrap_or(Ix::INK);
    let leaf = Ramp::Shrub;
    p.s.ly.step(x + 1, y + 2, -1);
    p.s.ly.step(x + 2, y + 1, -1);
    for (dx, dy, ix, n) in [
        (0, 2, leaf.at(Tone::Mid), FLAT),
        (-1, 2, leaf.at(Tone::Base), FLAT),
        (-1, 0, petal, normal(-50, 0)),
        (1, 0, petal, normal(50, 0)),
        (0, -1, petal, normal(0, -50)),
        (0, 1, petal, normal(0, 50)),
        (0, 0, eye, FLAT),
    ] {
        p.s.ly.put(x + dx, y + dy, ix, n, z + 2);
    }
}

/// A fallen leaf: three px of autumn, lit on one side.
fn leaf(p: &mut Painter, x: i32, y: i32, g: u8, h: u32, z: i32) {
    if !own(p, x, y, g) || !own(p, x + 1, y + 1, g) {
        return;
    }
    let (lit, dark) = match h % 4 {
        0 => ('o', 'Y'),
        1 => ('y', 'Y'),
        2 => ('m', 'T'),
        _ => ('o', 'T'),
    };
    let (lit, dark) = (letter(lit).unwrap_or(Ix::INK), letter(dark).unwrap_or(Ix::INK));
    let turn = (h >> 2) & 1 == 1;
    let (a, b) = if turn { ((1, 0), (0, 1)) } else { ((0, 0), (1, 1)) };
    p.s.ly.put(x + a.0, y + a.1, lit, normal(-30, -30), z + 1);
    p.s.ly.put(x + b.0, y + b.1, dark, normal(30, 30), z + 1);
    p.s.ly.put(x + i32::from(!turn), y, lit, FLAT, z + 1);
    p.s.ly.step(x + b.0 + 1, y + b.1 + 1, -1);
}

/// A pebble: 3 x 2 (or 4 x 3 when `big`) of stone, grey or the ground's own, lit on its top-left,
/// its underside in shade, its contact shade under it and to its right.
fn pebble(p: &mut Painter, x: i32, y: i32, g: u8, big: bool, z: i32) {
    let (w, h) = if big { (4, 3) } else { (3, 2) };
    if !(own(p, x, y, g) && own(p, x + w - 1, y + h - 1, g)) {
        return;
    }
    let grey = h32(x as u32, y as u32, 0x9e66) % 3 != 0;
    let stone = if grey { Ramp::Stone } else { p.s.ly.tone(x, y).map_or(Ramp::Stone, |(r, _)| r) };
    for j in 0..h {
        for i in 0..w {
            let corner = (i == 0 || i == w - 1) && (j == 0 || j == h - 1) && big;
            if corner {
                continue;
            }
            let t = if j == 0 && i < w - 1 {
                if i == 0 || !big { Tone::High } else { Tone::Light }
            } else if j == h - 1 || i == w - 1 {
                Tone::Mid
            } else {
                Tone::Lift
            };
            let t = if grey { t } else { t.step(1) };
            let (nx, ny) = ((2 * i + 1 - w) * 70 / w, (2 * j + 1 - h) * 70 / h);
            p.s.ly.put(x + i, y + j, stone.at(t), normal(nx, ny), z + 1 + i32::from(j < h - 1));
        }
        p.s.ly.step(x + w, y + j, -1);
    }
    for i in 0..w {
        p.s.ly.step(x + i + 1, y + h, -1);
    }
}

/// A crack wandering across, 1 px dark with its lip lit above it, turning now and then.
fn crack(p: &mut Painter, x: i32, y: i32, g: u8, h: u32, len: i32) {
    let (mut x, mut y) = (x, y);
    let mut dir = (h >> 8) & 3;
    for step in 0..len {
        if own(p, x, y, g) {
            p.s.ly.step(x, y, -2);
            if own(p, x, y - 1, g) && dir & 1 == 0 {
                p.s.ly.step(x, y - 1, 1);
            }
        }
        let hs = h32(h, step as u32, 5);
        if hs & 3 == 0 {
            dir = (dir + if hs & 4 == 0 { 1 } else { 3 }) & 3;
        }
        match dir {
            0 => x += 1,
            1 => y += 1,
            2 => x -= 1,
            _ => y -= 1,
        }
    }
}

/// A puddle of standing water in the marsh: a lens of water ramp, lit on its far lip.
fn puddle(p: &mut Painter, x: i32, y: i32, g: u8, h: u32) {
    let w = 5 + (h >> 12) as i32 % 3;
    let wr = Ramp::Water;
    for i in 0..w {
        for j in 0..2 {
            let corner = (i == 0 || i == w - 1) && j == 1;
            if corner || !own(p, x + i, y + j, g) {
                continue;
            }
            let t = if j == 0 && (i == 1 || i == 2) {
                Tone::Light
            } else if j == 0 {
                Tone::Base
            } else {
                Tone::Mid
            };
            p.s.ly.put(x + i, y + j, wr.at(t), FLAT, 1);
        }
    }
    for i in 0..w {
        p.s.ly.step(x + i, y - 1, -1);
    }
}

/// A seedling: a stem and two leaves.
fn sprout(p: &mut Painter, x: i32, y: i32, g: u8, leaf: Ramp, z: i32) {
    for (dx, dy, t) in
        [(0, 0, Tone::Shade), (0, -1, Tone::Base), (-1, -2, Tone::Light), (1, -2, Tone::Base), (0, -2, Tone::Lift)]
    {
        if own(p, x + dx, y + dy, g) {
            p.s.ly.put(x + dx, y + dy, leaf.at(t), normal(dx * 40, -30), z + 1 - dy);
        }
    }
}
