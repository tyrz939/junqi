//! Layer 1 of the skeleton: the land. Height, the river and its lake, the three regions,
//! biomes, how wet and how rough the ground is. A pure function of `(seed, attempt)`, in
//! integers: noise is `Q16`, distances are compared squared, the only root is `isqrt`
//! (PORT.md §6.b, §6.c). Carries `jane/src/world/skeleton/terrain.ts`.
//!
//! The county's shape is fixed where the story needs it and free everywhere else: the Works
//! are the north (the hill; the School on its crown, seen from the station); the river runs
//! north to south somewhere east of the middle, the Waters beyond it; the Lowfields are the
//! south-west, with foothills along their southern edge. Where the river bends, where the
//! borders wobble, how high the hill is: the seed's.

use jane_core::grid::Grid;
use jane_core::noise::fbm;
use jane_core::num::{Q16, Q16_ONE, isqrt, mul_div_floor};
use jane_core::search::chamfer;
use jane_core::{Sfc32, ZoneId};

use super::types::{Biome, Region, SKEL_H, SKEL_W, Water, inside};
use crate::steps::{Step, dice};

/// A fraction in thousandths as `Q16`, for thresholds written as decimals in the TypeScript.
const fn q(permille: i32) -> i32 {
    permille * Q16_ONE / 1000
}

/// `(v - 0.5) * amp` in `Q16`: a noise value centred on zero and scaled.
fn centred(v: Q16, amp: i32) -> i32 {
    ((i64::from(v.0 - Q16_ONE / 2) * i64::from(amp)) as i32).clamp(i32::MIN / 2, i32::MAX / 2)
}

/// `Q16` to the nearest integer, halves up.
const fn round_q16(v: i64) -> i64 {
    (v + (Q16_ONE as i64 / 2)) >> 16
}

/// The noise salts, one per field, drawn in this order from the terrain's own noise dice.
#[derive(Clone, Copy, Debug)]
struct Salts {
    height: u32,
    river_slow: u32,
    river_fast: u32,
    works_edge: u32,
    foot_edge: u32,
    lake_edge: u32,
    patch: u32,
    big: u32,
    rough: u32,
}

impl Salts {
    fn draw(r: &mut Sfc32) -> Self {
        Salts {
            height: r.next_u32(),
            river_slow: r.next_u32(),
            river_fast: r.next_u32(),
            works_edge: r.next_u32(),
            foot_edge: r.next_u32(),
            lake_edge: r.next_u32(),
            patch: r.next_u32(),
            big: r.next_u32(),
            rough: r.next_u32(),
        }
    }
}

/// The lake, in macro cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lake {
    pub mx: i32,
    pub my: i32,
    pub r: i32,
}

/// The land of one seed.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// 0..=255.
    pub height: Grid<u8>,
    pub water: Grid<Water>,
    pub region: Grid<Region>,
    pub biome: Grid<Biome>,
    /// Distance to the nearest water, in tenths of a macro cell (10 straight, 14 diagonal).
    pub wet: Grid<u16>,
    /// Steepest height step to any of the eight neighbours. Asked of every cell by every row, so worked out once.
    pub slope: Grid<u8>,
    /// Cost of laying a road across the cell, Q8 (256 = 1.0), about 179 to 1100.
    pub rough: Grid<u16>,
    /// The river's column at each row: the Waters lie east of it.
    pub river_x: Vec<i32>,
    /// The crown of the hill, where the School goes.
    pub crown: (i32, i32),
    pub lake: Lake,
}

/// Build the land for `(seed, attempt)`.
pub fn build_terrain(seed: u32, attempt: u8) -> Terrain {
    let mut rng = dice(seed, ZoneId::County, Step::SkelTerrain, attempt, 0, 0);
    let s = Salts::draw(&mut dice(seed, ZoneId::County, Step::SkelTerrainNoise, attempt, 0, 0));
    let (w, h) = (SKEL_W as u32, SKEL_H as u32);

    // The river: a wandering column, top to bottom, 58 to 66 % of the way across, drifting east
    // or west as it goes south, never out of the middle half.
    let base_tenths = rng.range(SKEL_W * 58 / 10, SKEL_W * 66 / 10);
    let lean = rng.range(-8, 8);
    let lo = i64::from(SKEL_W) * i64::from(Q16_ONE) / 2;
    let hi = i64::from(SKEL_W) * 74 * i64::from(Q16_ONE) / 100;
    let river_x: Vec<i32> = (0..SKEL_H)
        .map(|y| {
            let t = Q16::ratio(y, SKEL_H - 1).0 - Q16_ONE / 2;
            let wander = centred(fbm(0, y, 38, s.river_slow, 2, 1), 36) + centred(fbm(0, y, 11, s.river_fast, 1, 1), 8);
            let x =
                i64::from(base_tenths) * i64::from(Q16_ONE) / 10 + i64::from(lean) * i64::from(t) + i64::from(wander);
            round_q16(x.clamp(lo, hi)) as i32
        })
        .collect();

    // The hill: one crown in the north, a little west of the river so it stands over the town.
    let crown = (rng.range(SKEL_W * 3 / 10 + 1, SKEL_W * 48 / 100), rng.range(9, 17));
    let hill_r = rng.range(38, 48);

    // The Works' southern border wobbles around 34 % of the height; the foothills run along the
    // south-west around 80 %. Both depend on the column only.
    let works_edge: Vec<i32> = (0..SKEL_W)
        .map(|x| mul_div_floor(SKEL_H, 34 * Q16_ONE, 100) + centred(fbm(x, 0, 44, s.works_edge, 3, 1), 44))
        .collect();
    let foot_edge: Vec<i32> = (0..SKEL_W)
        .map(|x| mul_div_floor(SKEL_H, 80 * Q16_ONE, 100) + centred(fbm(x, 0, 30, s.foot_edge, 3, 1), 34))
        .collect();
    // Above zero: south of the foothill line.
    let foot = |x: i32, y: i32| (y << 16) - foot_edge[x as usize];

    let mut height = Grid::new(w, h, 0u8);
    let mut region = Grid::new(w, h, Region::Lowfields);
    let hill_r_q16 = i64::from(hill_r) << 16;
    for y in 0..SKEL_H {
        let rx = river_x[y as usize];
        for x in 0..SKEL_W {
            let mut hq: i64 = (60 << 16) + i64::from(fbm(x, y, 48, s.height, 4, 1).0) * 50;
            // Distance to the crown with the hill squashed north-south (dy * 5 / 4), in Q16.
            let dx4 = i64::from(x - crown.0) * 4;
            let dy5 = i64::from(y - crown.1) * 5;
            let d = i64::from(isqrt(((dx4 * dx4 + dy5 * dy5) as u64) << 32)) / 4;
            if d < hill_r_q16 {
                let t = Q16((i64::from(Q16_ONE) - d / i64::from(hill_r)) as i32);
                hq += i64::from(Q16::smooth(t).0) * 120;
            }
            let f = foot(x, y);
            if f > 0 && x < rx - 6 {
                hq += i64::from(f).saturating_mul(5).min(70 << 16);
            }
            let river = (x - rx).abs();
            if river < 14 {
                hq -= i64::from(Q16::smooth(Q16(Q16_ONE - Q16::ratio(river, 14).0)).0) * 38;
            }
            height.set(x, y, round_q16(hq).clamp(0, 255) as u8);
            let r = if (y << 16) < works_edge[x as usize] {
                Region::Works
            } else if x > rx {
                Region::Waters
            } else {
                Region::Lowfields
            };
            region.set(x, y, r);
        }
    }

    // Water: the river, two cells wide and three in the south, kept continuous where the column
    // jumps sideways between rows; then the lake in the Waters.
    let mut water = Grid::new(w, h, Water::Dry);
    for y in 0..SKEL_H {
        let rx = river_x[y as usize];
        let half = i32::from(y * 100 > SKEL_H * 55);
        for x in rx - 1..=rx + half {
            water.set(x, y, Water::River);
        }
        if y > 0 {
            let prev = river_x[y as usize - 1];
            for x in prev.min(rx)..=prev.max(rx) {
                water.set(x, y, Water::River);
            }
        }
    }
    // Clear of the east fence by its own width.
    let lake = Lake {
        mx: rng.range(SKEL_W * 78 / 100 + 1, SKEL_W * 86 / 100),
        my: rng.range(SKEL_H / 2 + 1, SKEL_H * 78 / 100),
        r: rng.range(7, 10),
    };
    for y in lake.my - lake.r - 3..=lake.my + lake.r + 3 {
        for x in lake.mx - lake.r - 3..=lake.mx + lake.r + 3 {
            // The lake is the Waters': it may meet the river's east bank, never cross it.
            if !inside(x, y) || x <= river_x[y as usize] {
                continue;
            }
            // Inside when dx² + (1.3 dy)² < edge², all times 100 and in Q16².
            let dx = i64::from(x - lake.mx);
            let dy = i64::from(y - lake.my);
            let edge = i64::from((lake.r << 16) + centred(fbm(x, y, 6, s.lake_edge, 2, 1), 5));
            if edge > 0 && (100 * dx * dx + 169 * dy * dy) << 32 < 100 * edge * edge {
                water.set(x, y, Water::Lake);
            }
        }
    }

    // How wet the ground is: chamfer distance to water.
    let wet = chamfer(w, h, |x, y| water.read(x, y, Water::Dry) != Water::Dry);

    let mut biome = Grid::new(w, h, Biome::Field);
    let mut rough = Grid::new(w, h, 0u16);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let patch = fbm(x, y, 14, s.patch, 3, 1).0;
            let big = fbm(x, y, 34, s.big, 2, 1).0;
            let wet10 = wet.read(x, y, u16::MAX);
            let b = match region.read(x, y, Region::Lowfields) {
                Region::Lowfields => {
                    if foot(x, y) > 0 {
                        if patch > q(620) { Biome::Wood } else { Biome::Foothill }
                    } else if big > q(600) {
                        Biome::Wood
                    } else if patch > q(520) {
                        Biome::Hedge
                    } else {
                        Biome::Field
                    }
                }
                Region::Waters => {
                    if wet10 < 30 {
                        Biome::Reed
                    } else if wet10 < 70 && patch > q(450) {
                        Biome::Marsh
                    } else if big > q(580) {
                        Biome::WetWood
                    } else if patch > q(600) {
                        Biome::Garden
                    } else {
                        Biome::Marsh
                    }
                }
                Region::Works => {
                    if height.read(x, y, 0) > 150 {
                        Biome::Hill
                    } else if big > q(550) {
                        Biome::Yard
                    } else {
                        Biome::Slag
                    }
                }
            };
            biome.set(x, y, b);
            // Without roughness every road is a ruler line with one bend; with it they find their way
            // round woods and wet ground and wander about a quarter further than the crow.
            let ground: i32 = match b {
                Biome::WetWood => 307,
                Biome::Reed => 256,
                Biome::Wood => 230,
                Biome::Marsh => 179,
                Biome::Hill => 128,
                _ => 0,
            };
            let noise = (fbm(x, y, 9, s.rough, 2, 1).0 * 666) >> 16;
            rough.set(x, y, (179 + noise + ground) as u16);
        }
    }

    let mut slope = Grid::new(w, h, 0u8);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let here = height.read(x, y, 0);
            let mut worst = 0u8;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    if inside(x + ox, y + oy) {
                        worst = worst.max(here.abs_diff(height.read(x + ox, y + oy, 0)));
                    }
                }
            }
            slope.set(x, y, worst);
        }
    }

    Terrain { height, water, region, biome, wet, slope, rough, river_x, crown, lake }
}
