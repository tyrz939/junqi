//! Every cell's ground (`paintLand`, `ground`), and the tree line round the edge.
//!
//! The skeleton's macro cells are 16 m squares; looked up through a smooth warp they stop being
//! squares, and water is read as a smooth field so the river has banks instead of steps. Four
//! million cells, so the three smooth fields (warp x, warp y, clumps) are bilinear lattices read
//! as per-row `Q16` gradients: inside one macro block each is a straight line along a row, and a
//! cell costs three additions (PORT.md §6.b). Pines are `Material::Pine` over `Tile::Tree`, laid
//! as rects of runs (PORT.md §6.i).
//!
//! Refines skeleton data, so it throws the skeleton's attempt (PORT.md §6.a).

use alloc::vec;
use alloc::vec::Vec;
use jane_core::hash::mix32;
use jane_core::noise::lattice;
use jane_core::num::Q16_ONE;
use jane_core::{Material, Rect, Tile};

use super::County;
use crate::skeleton::{Biome, COUNTY_H, COUNTY_W, MACRO, SKEL_H, SKEL_W, Water};
use crate::steps::Step;

/// How far the warp may push a lookup, in cells, end to end.
const WARP: i32 = 20;
/// The tree line round the county is this deep: the county is a bowl, not a plane.
pub const EDGE: i32 = 4;

/// A fraction in thousandths as `Q16`.
const fn q(permille: i32) -> i32 {
    permille * Q16_ONE / 1000
}

/// A cell's own value in `0..Q16_ONE`: cheap, because every cell of the county asks.
#[inline]
fn cell_q16(salt: u32, x: i32, y: i32) -> i32 {
    (mix32(salt ^ (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1)) >> 16) as i32
}

/// The salts the land's fields are drawn from.
#[derive(Clone, Copy, Debug)]
struct Salts {
    warp_x: u32,
    warp_y: u32,
    clumps: u32,
    cell: u32,
    hedge: u32,
    yard: u32,
}

/// Every cell's ground.
pub fn paint_land(c: &mut County<'_>) {
    let sk = c.sk;
    let t = &sk.terrain;
    let mut rng = c.k.dice_at(Step::CountyLand, sk.attempt, 0, 0);
    let s = Salts {
        warp_x: rng.next_u32(),
        warp_y: rng.next_u32(),
        clumps: rng.next_u32(),
        cell: rng.next_u32(),
        hedge: rng.next_u32(),
        yard: rng.next_u32(),
    };
    let (sw, sh) = (SKEL_W as usize, SKEL_H as usize);
    let lw = sw + 1;
    let field = |salt: u32| -> Vec<i32> {
        (0..=SKEL_H).flat_map(|y| (0..=SKEL_W).map(move |x| lattice(x, y, salt).0)).collect()
    };
    let (warp_x, warp_y, clumps) = (field(s.warp_x), field(s.warp_y), field(s.clumps));
    // Water as 0 or 1 a macro cell, and which blocks have water within two macro cells: only
    // those read the water field at all.
    let mut wet = vec![0i64; sw * sh];
    let mut near_water = vec![false; sw * sh];
    for my in 0..SKEL_H {
        for mx in 0..SKEL_W {
            if t.water.read(mx, my, Water::Dry) == Water::Dry {
                continue;
            }
            wet[my as usize * sw + mx as usize] = 1;
            for oy in -2..=2 {
                for ox in -2..=2 {
                    let (x, y) = (mx + ox, my + oy);
                    if x >= 0 && y >= 0 && x < SKEL_W && y < SKEL_H {
                        near_water[y as usize * sw + x as usize] = true;
                    }
                }
            }
        }
    }
    let biomes = t.biome.as_slice();
    let (u_max, v_max) = ((SKEL_W - 1) << 16, (SKEL_H - 1) << 16);
    let mut pines = Pines::default();
    let tiles = c.k.tiles_mut().as_mut_slice();
    for y in 0..COUNTY_H {
        let my = (y / MACRO) as usize;
        let j = y % MACRO;
        let mut i_row = y as usize * COUNTY_W as usize;
        for mx in 0..sw {
            let n = my * lw + mx;
            // A field's value down the block's left and right edges at this row, in Q20 (Q16 x 16,
            // so `j / 16` is exact), and its step along the row in Q24.
            let edge = |f: &[i32], k: usize| f[k] * MACRO + (f[k + lw] - f[k]) * j;
            let row = |f: &[i32]| {
                let (l, r) = (edge(f, n), edge(f, n + 1));
                (l * MACRO, r - l)
            };
            let (mut wx, dwx) = row(&warp_x);
            let (mut wy, dwy) = row(&warp_y);
            let (mut cl, dcl) = row(&clumps);
            let watery = near_water[my * sw + mx];
            let x0 = mx as i32 * MACRO;
            for i in 0..MACRO {
                let x = x0 + i;
                // The lookup, pushed by the warp: (x + (w - 0.5) * WARP) / MACRO - 0.5, in Q16.
                let off_x = ((wx - (1 << 23)) * WARP) >> 8;
                let off_y = ((wy - (1 << 23)) * WARP) >> 8;
                let u = ((((x << 16) + off_x) >> 4) - (Q16_ONE / 2)).clamp(0, u_max);
                let v = ((((y << 16) + off_y) >> 4) - (Q16_ONE / 2)).clamp(0, v_max);
                wx += dwx;
                wy += dwy;
                let clump24 = cl;
                cl += dcl;
                if watery {
                    let u0 = (u >> 16).min(SKEL_W - 2);
                    let v0 = (v >> 16).min(SKEL_H - 2);
                    let tx = i64::from(u - (u0 << 16));
                    let tv = i64::from(v - (v0 << 16));
                    let one = i64::from(Q16_ONE);
                    let k = v0 as usize * sw + u0 as usize;
                    // Bilinear, in Q32: at least 0.4 is the bank, at least 0.5 the water.
                    let water = wet[k] * (one - tx) * (one - tv)
                        + wet[k + 1] * tx * (one - tv)
                        + wet[k + sw] * (one - tx) * tv
                        + wet[k + sw + 1] * tx * tv;
                    if water * 5 >= 2 << 32 {
                        tiles[i_row] = if water * 2 >= 1 << 32 { Tile::Water } else { Tile::Sand };
                        i_row += 1;
                        continue;
                    }
                }
                let r = cell_q16(s.cell, x, y);
                let biome = biomes[((v + Q16_ONE / 2) >> 16) as usize * sw + ((u + Q16_ONE / 2) >> 16) as usize];
                // A little of the per-cell value on the smooth one (about 0.12 of its spread), so
                // thickets have ragged edges.
                let clump = (clump24 >> 8) + (((r - Q16_ONE / 2) * 31) >> 8);
                let (tile, pine) = ground(biome, clump, r, x, y, &s);
                tiles[i_row] = tile;
                c.wild_earth[i_row] = tile == Tile::Dirt;
                if pine {
                    pines.cell(x);
                }
                i_row += 1;
            }
        }
        pines.end_row(y);
    }
    let rects = pines.finish();
    c.k.paint_all(rects, Material::Pine);
}

/// The ground of one cell: `clump` is smooth (thickets, outcrops, pools), `r` is the cell's own
/// (the odd tree, the long grass), both `Q16` in `0..1`. The flag says the tree is a pine.
fn ground(biome: Biome, clump: i32, r: i32, x: i32, y: i32, s: &Salts) -> (Tile, bool) {
    let plain = |t: Tile| (t, false);
    match biome {
        Biome::Field => plain(if r < q(4) {
            Tile::Tree
        } else if r < q(12) {
            Tile::Bush
        } else if r < q(100) {
            Tile::GrassTall
        } else {
            Tile::Grass
        }),
        Biome::Hedge => {
            // Hedged fields: lines of bush on a loose grid, with gaps where a gate once was.
            let line = x % 34 == 0 || y % 30 == 0;
            if line && cell_q16(s.hedge, x / 9, y / 9) > q(300) {
                return plain(Tile::Bush);
            }
            plain(if r < q(6) {
                Tile::Tree
            } else if r < q(160) {
                Tile::GrassTall
            } else {
                Tile::Grass
            })
        }
        Biome::Wood => {
            if clump > q(800) {
                (Tile::Tree, true)
            } else if clump > q(660) || r < q(50) {
                plain(Tile::Tree)
            } else if r < q(400) {
                plain(Tile::GrassTall)
            } else {
                plain(Tile::Grass)
            }
        }
        Biome::Foothill => {
            if clump > q(740) {
                plain(Tile::Cliff)
            } else if r < q(20) {
                (Tile::Tree, true)
            } else if clump < q(300) {
                plain(Tile::Dirt)
            } else if r < q(200) {
                plain(Tile::GrassTall)
            } else {
                plain(Tile::Grass)
            }
        }
        // Patches of reed and of moss, not a salt-and-pepper of both: the smooth field decides.
        Biome::Reed => plain(if clump > q(780) {
            Tile::Water
        } else if clump > q(470) {
            Tile::GrassTall
        } else {
            Tile::Moss
        }),
        Biome::Marsh => plain(if clump > q(760) {
            Tile::Water
        } else if clump < q(220) {
            Tile::DryBed
        } else if r < q(300) {
            Tile::GrassTall
        } else if r < q(340) {
            Tile::Bush
        } else {
            Tile::Moss
        }),
        Biome::WetWood => plain(if clump > q(680) || r < q(50) {
            Tile::Tree
        } else if r < q(500) {
            Tile::Moss
        } else {
            Tile::GrassTall
        }),
        Biome::Garden => plain(if clump > q(700) {
            Tile::Bush
        } else if r < q(30) {
            Tile::Rubble
        } else if clump > q(440) {
            Tile::Garden
        } else {
            Tile::Grass
        }),
        // Rubble is solid: a scatter of it is cover, a carpet of it is a maze.
        Biome::Slag => plain(if clump > q(780) {
            Tile::Cliff
        } else if r < q(50) {
            Tile::Rubble
        } else if clump < q(300) {
            Tile::DryBed
        } else {
            Tile::Dirt
        }),
        // The yards' old hardstandings, in strips.
        Biome::Yard => plain(if y % 46 < 2 && cell_q16(s.yard, x / 40, y / 46) > q(500) {
            Tile::Cobble
        } else if r < q(40) {
            Tile::Rubble
        } else if clump > q(600) {
            Tile::Cobble
        } else {
            Tile::Dirt
        }),
        Biome::Hill => plain(if clump > q(720) {
            Tile::Cliff
        } else if r < q(40) {
            Tile::Rubble
        } else if clump < q(350) {
            Tile::Grass
        } else {
            Tile::Dirt
        }),
        Biome::Town => plain(Tile::Grass),
    }
}

/// Pine cells gathered into rects: a run along a row, grown down while the next row has the same
/// run under it.
#[derive(Debug, Default)]
struct Pines {
    rects: Vec<Rect>,
    /// This row's runs so far, `(x, w)`.
    row: Vec<(i32, i32)>,
    /// Last row's runs, `(x, w, rect)`, by `x`.
    open: Vec<(i32, i32, usize)>,
}

impl Pines {
    fn cell(&mut self, x: i32) {
        match self.row.last_mut() {
            Some((x0, w)) if *x0 + *w == x => *w += 1,
            _ => self.row.push((x, 1)),
        }
    }

    fn end_row(&mut self, y: i32) {
        let mut next = Vec::with_capacity(self.row.len());
        let mut o = 0;
        for &(x, w) in &self.row {
            while o < self.open.len() && self.open[o].0 < x {
                o += 1;
            }
            let rect = match self.open.get(o) {
                Some(&(ox, ow, k)) if ox == x && ow == w => {
                    self.rects[k].h += 1;
                    k
                }
                _ => {
                    self.rects.push(Rect::new(x, y, w, 1));
                    self.rects.len() - 1
                }
            };
            next.push((x, w, rect));
        }
        self.open = next;
        self.row.clear();
    }

    fn finish(self) -> Vec<Rect> {
        self.rects
    }
}

/// The open earth the land laid that is still dirt, off every way, door step and set place, as
/// rects: what drifts into the wild ground round it (`Material::WildEarth`). A lane, a yard or a
/// town's ground is dirt laid by hand and keeps its edge.
pub fn wild_earth(c: &County<'_>) -> Vec<Rect> {
    let (w, h) = (c.k.w(), c.k.h());
    let boxes: Vec<Rect> = c.chunks.iter().map(|ch| ch.bounds).collect();
    let mut runs = Pines::default();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if c.wild_earth[i]
                && !c.trodden[i]
                && c.k.get(x, y) == Tile::Dirt
                && !boxes.iter().any(|b| b.contains(x, y))
            {
                runs.cell(x);
            }
        }
        runs.end_row(y);
    }
    runs.finish()
}

/// The tree line round the edge, `EDGE` deep.
pub fn tree_line(c: &mut County<'_>) {
    let (w, h) = (c.k.w(), c.k.h());
    for r in [
        Rect::new(0, 0, w, EDGE),
        Rect::new(0, h - EDGE, w, EDGE),
        Rect::new(0, 0, EDGE, h),
        Rect::new(w - EDGE, 0, EDGE, h),
    ] {
        c.k.fill(r, Tile::Tree);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pines_merge_runs_down_the_rows() {
        let mut p = Pines::default();
        for y in 0..3 {
            for x in [2, 3, 4, 9] {
                p.cell(x);
            }
            if y == 1 {
                p.cell(20);
            }
            p.end_row(y);
        }
        assert_eq!(p.finish(), vec![Rect::new(2, 0, 3, 3), Rect::new(9, 0, 1, 3), Rect::new(20, 1, 1, 1)]);
    }

    #[test]
    fn thresholds_are_fractions_of_one() {
        assert_eq!(q(500), Q16_ONE / 2);
        assert_eq!(q(1000), Q16_ONE);
        for x in 0..50 {
            assert!((0..Q16_ONE).contains(&cell_q16(9, x, x * 7)));
        }
    }
}
