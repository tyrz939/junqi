//! A house's front garden (ART-PLAN M7): the rows of grass in front of its wall
//! (`houses::House::plot`), laid out by its look's recipe. A path from the door to a gate in the
//! boundary along the garden's front; beside the path a cottage garden's flowers in drifts, a
//! veg patch's cabbages in rows, a tidy lawn mown in stripes, or a neglected one's long grass;
//! and standing in it what the recipe has (hollyhocks, a water butt and bean canes, a bird bath
//! and a sundial, a rusted bike). The ground is painted here; what stands is a `garden` sprite.
//! Drawing only: the sim's cells are the grass they were.

use super::super::hard::voronoi;
use super::super::houses::{Garden, House, Look};
use super::{CELL, Kind, Painter, Thing, contact, fast, tall_tuft};
use crate::canvas::normal;
use crate::garden::Piece;
use crate::hash::h32;
use crate::palette::{Ramp, Tone};

/// The house whose garden holds chunk-local cell `(cx, cy)`, with the cell's world cell.
fn house(p: &Painter, cx: i32, cy: i32) -> Option<(House, i32, i32)> {
    let h = p.s.house[Painter::at(cx, cy)]?;
    let (wx, wy) = (cx + p.x0c, cy + p.y0c);
    h.plot.contains(wx, wy).then_some((h, wx, wy))
}

/// The gate's west cell: under the door, else the middle of the garden.
fn gate_x(h: &House) -> i32 {
    h.door.unwrap_or(h.plot.x + h.plot.w / 2 - 1).clamp(h.plot.x, h.plot.right() - 2)
}

/// What stands on a garden's cell: `None` if the cell is no garden's, else what (if anything) it
/// shows in place of what the tile would have stood there.
// None: no garden here, the tile stands as it would; Some(None): a garden that shows nothing.
#[allow(clippy::option_option)]
pub(super) fn thing(p: &Painter, cx: i32, cy: i32) -> Option<Option<Thing>> {
    let (h, wx, wy) = house(p, cx, cy)?;
    let l = h.look();
    let gx = gate_x(&h);
    let hh = h32(wx as u32, wy as u32, h.seed ^ 0x9a2d);
    let piece = |pc: Piece, ox: i32, oy: i32| {
        Some(Some(Thing {
            pick: p.bank.pick(Kind::Garden(pc), hh),
            ox,
            oy,
            canopy: false,
            tree: false,
            shade: !pc.is_boundary(),
        }))
    };
    if wy == h.plot.bottom() - 1 {
        // The boundary along the garden's front, its gate under the door: a cottage garden's
        // gate is a rose arch.
        return if wx == gx {
            piece(if l.garden == Garden::Cottage { Piece::RoseArch } else { Piece::of(l.boundary, true, false) }, 8, 0)
        } else if wx == gx + 1 {
            Some(None)
        } else {
            piece(Piece::of(l.boundary, false, l.garden == Garden::Neglect), 0, 0)
        };
    }
    if wx == gx || wx == gx + 1 {
        return Some(None);
    }
    let rel = wx - h.plot.x;
    let back = wy == h.plot.y;
    let (west_mid, east_mid) = ((gx - h.plot.x) / 2, gx + 2 + (h.plot.right() - gx - 2) / 2 - h.plot.x);
    match l.garden {
        Garden::Cottage if back && (rel == 0 || rel == h.plot.w - 1 || hh % 3 == 0) => piece(Piece::Hollyhocks, 0, -1),
        Garden::Veg if back && rel == 0 => piece(Piece::WaterButt, -1, -2),
        Garden::Veg if rel == east_mid => piece(Piece::Canes, 0, 0),
        Garden::Tidy if rel == west_mid => piece(Piece::BirdBath, 0, 0),
        Garden::Tidy if rel == east_mid => piece(Piece::Sundial, 0, 0),
        Garden::Neglect if back && rel == 1 => piece(Piece::Bike, 0, -1),
        Garden::Neglect if hh % 3 == 0 => Some(Some(Thing {
            pick: p.bank.pick(Kind::Grass, hh >> 4),
            ox: (hh >> 8) as i32 % 9 - 4,
            oy: 0,
            canopy: false,
            tree: false,
            shade: false,
        })),
        _ => Some(None),
    }
}

/// Paint a garden's ground on chunk-local cell `(cx, cy)`: its path, its beds, its lawn; the
/// wall's shade kept over them, the boundary's laid along its foot.
pub(super) fn ground(p: &mut Painter, cx: i32, cy: i32, seed: u32) {
    let Some((h, _, wy)) = house(p, cx, cy) else { return };
    let l = h.look();
    let gx = gate_x(&h);
    let (path0, path1) = (gx * CELL + 9, gx * CELL + 23);
    let (px0, py0) = (cx * CELL, cy * CELL);
    let boundary = wy == h.plot.bottom() - 1;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wpx, wpy) = ((cx + p.x0c) * CELL + x, (cy + p.y0c) * CELL + y);
            let z = p.s.ly.z(px0 + x, py0 + y).clamp(1, 4);
            let on_path = (path0..path1).contains(&wpx);
            let edge = wpx == path0 - 1 || wpx == path1;
            let (ramp, t, dz) = if on_path {
                path(&l, wpx, wpy)
            } else if edge {
                (Ramp::Soil, Tone::Shade, 0)
            } else if boundary && y >= CELL - 4 {
                // A strip of grass along the boundary's foot.
                (Ramp::Turf, if (wpx + wpy) % 5 == 0 { Tone::Lift } else { Tone::Base }, 0)
            } else {
                bed(&l, wpx, wpy, h.seed)
            };
            p.s.ly.put(px0 + x, py0 + y, ramp.at(t), normal(0, -10), z + dz);
        }
    }
    if l.garden == Garden::Neglect {
        // Long grass in clumps through it.
        for k in 0..3 {
            let hk = h32((cx + p.x0c) as u32, (cy + p.y0c) as u32 * 3 + k, seed ^ 0x4e61);
            let (x, y) = (px0 + 1 + (hk % 14) as i32, py0 + 4 + (hk >> 8) as i32 % 10);
            if !(path0 - 1..=path1).contains(&((cx + p.x0c) * CELL + x - px0)) {
                let z = p.s.ly.z(x, y);
                tall_tuft(p, x, y, Ramp::TurfDry, 3 + (hk >> 16) as i32 % 2, hk >> 20, z);
            }
        }
    }
    // The wall's shade on the garden under it, as `hard::contact` laid it on the grass.
    if wy == h.plot.y {
        for x in 0..CELL {
            for (y, s) in [(0, -2), (1, -2), (2, -1), (3, -1)] {
                p.s.ly.step(px0 + x, py0 + y, s);
            }
            if fast((px0 + x) as u32 >> 1, wy as u32, seed) & 1 == 0 {
                p.s.ly.step(px0 + x, py0 + 4, -1);
            }
        }
    }
    if boundary {
        let world = (p.x0c * CELL, p.y0c * CELL);
        contact(p, px0 + 8, py0 + CELL - 2, 10, 2, world, seed);
    }
}

/// The path from the door to the gate: a cottage's crazy paving with grass in its joints, a veg
/// patch's gravel, a tidy garden's flags, a neglected one's paving gone under the grass.
fn path(l: &Look, wx: i32, wy: i32) -> (Ramp, Tone, i32) {
    match l.garden {
        Garden::Veg => {
            let g = fast(wx.div_euclid(2) as u32, wy.div_euclid(2) as u32, 0x6a7e);
            (Ramp::Gravel, [Tone::Base, Tone::Lift, Tone::Base, Tone::Mid][(g % 4) as usize], 1)
        }
        Garden::Tidy => {
            let (u, v) = (wx.rem_euclid(7), (wy + (wx.div_euclid(7) & 1) * 3).rem_euclid(7));
            let t = if u == 0 || v == 0 {
                Tone::Mid
            } else if u == 1 || v == 1 {
                Tone::Light
            } else {
                Tone::Lift
            };
            (Ramp::Stone, t, 1)
        }
        Garden::Cottage | Garden::Neglect => {
            let v = voronoi(wx, wy, 5, 0xc4a2);
            if v.edge || (l.garden == Garden::Neglect && v.id % 3 == 0) {
                let r = if l.garden == Garden::Neglect { Ramp::TurfDry } else { Ramp::Turf };
                return (r, if v.edge { Tone::Mid } else { Tone::Base }, 0);
            }
            let body = match v.id % 4 {
                0 => Tone::Mid,
                1 => Tone::Light,
                _ => Tone::Lift,
            };
            let t = if v.dy < -1 {
                body.step(1)
            } else if v.dy > 1 {
                body.step(-1)
            } else {
                body
            };
            (Ramp::Stone, t, 1)
        }
    }
}

/// A bed, a lawn, a patch: what the garden grows either side of its path.
fn bed(l: &Look, wx: i32, wy: i32, seed: u32) -> (Ramp, Tone, i32) {
    match l.garden {
        Garden::Cottage => {
            // Flowers in drifts of one colour over a mat of leaf: a clump a bloom in two.
            let v = voronoi(wx, wy, 3, seed ^ 0xf10e);
            let drift = super::super::slow(wx.div_euclid(6), wy.div_euclid(4), seed ^ 0xd81f);
            let bloom = [Ramp::Bloom, Ramp::Wisteria, Ramp::ClothMustard, Ramp::ClothLinen, Ramp::ClothRed]
                [(drift as usize * 5 / 256).min(4)];
            if v.edge {
                return (Ramp::Shrub, Tone::Shade, 1);
            }
            let s = -(v.dx * 2 + v.dy * 3);
            if v.id % 2 == 0 && v.dx.abs() <= 1 && v.dy.abs() <= 1 {
                return (bloom, if s > 0 { Tone::Light } else { Tone::Base }, 3);
            }
            let leaf = if v.id % 5 == 1 { Ramp::LeafOlive } else { Ramp::Shrub };
            (
                leaf,
                if s > 2 {
                    Tone::Lift
                } else if s > -2 {
                    Tone::Base
                } else {
                    Tone::Mid
                },
                2,
            )
        }
        Garden::Veg => {
            // Dug soil with a furrow every 8 px, a cabbage on each row every 6.
            let (u, v) = (wx.rem_euclid(6), wy.rem_euclid(8));
            let (dx, dy) = (u - 3, v - 4);
            if dx * dx * 4 + dy * dy * 9 <= 25 {
                let s = -(dx * 2 + dy * 3);
                let t = if s > 3 {
                    Tone::Light
                } else if s > 0 {
                    Tone::Lift
                } else if dx * dx * 4 + dy * dy * 9 > 16 {
                    Tone::Shade
                } else {
                    Tone::Base
                };
                return (Ramp::LeafDeep, t, 3);
            }
            (
                Ramp::Soil,
                if v == 0 {
                    Tone::Shade
                } else if v == 1 {
                    Tone::Lift
                } else {
                    Tone::Base
                },
                0,
            )
        }
        Garden::Tidy => {
            // A lawn mown in stripes.
            let stripe = wx.div_euclid(6) & 1 == 0;
            let fleck = fast(wx as u32, wy as u32, 0x1a37) % 23 == 0;
            (
                Ramp::Turf,
                if fleck {
                    Tone::Mid
                } else if stripe {
                    Tone::Lift
                } else {
                    Tone::Base
                },
                0,
            )
        }
        Garden::Neglect => {
            let f = fast(wx.div_euclid(2) as u32, wy.div_euclid(2) as u32, 0x0e61) % 7;
            (
                Ramp::TurfDry,
                if f == 0 {
                    Tone::Mid
                } else if f == 1 {
                    Tone::Lift
                } else {
                    Tone::Base
                },
                0,
            )
        }
    }
}
