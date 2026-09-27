//! The dungeons' own materials (ART.md §2.6, §8 step 7): each interior's walls and floors in
//! its own language, so a frame from inside says which building it is before anything stands
//! in it. Walls (the face a cell shows where its south is open): a mine gallery's rock with a
//! timber set every third cell and gold in the rock; crypt ashlar with a burial niche every
//! other cell; a works' sooted brick with iron pilasters and a beam; a museum's painted wall
//! over a picture rail and moulded panels; a sewer's walling with an iron main and a copper run;
//! a school's painted wall over a dado and boards. Floors: parquet, riveted iron plate, great
//! flags with a carved ledger stone among them, and a wet brick invert with a drain grate.
//!
//! Pure functions of the world px and the style's ramps, so a seam never shows and a chunk is
//! the same painted twice; `hard` calls them per pixel and keeps its own face lighting (the lit
//! top edge, the darkening foot) over them.

use jane_data::TilePattern as P;

use super::{CELL, fast};
use crate::canvas::{FLAT, Normal, UNIT, normal};
use crate::palette::{Ix, Ramp, Tone};

/// A face's normal: south, up a little (as `hard`'s).
const FACE: i32 = UNIT * 8 / 10;

/// A wall face's pixel for the interior patterns: `(colour, normal)`, or `None` for a pattern
/// that is not one of them. `y` is the row within the cell's face (0 at its top).
pub(super) fn face(pat: P, r: Ramp, accent: Option<Ramp>, wx: i32, wy: i32, y: i32) -> Option<(Ix, Normal)> {
    let flat_face = normal(0, FACE);
    Some(match pat {
        P::Timbered => {
            let wood = accent.unwrap_or(Ramp::WoodDark);
            let set = wx.rem_euclid(3 * CELL);
            let (post, cap) = (set < 5, (3..=5).contains(&y));
            if cap {
                // The cap: a squared timber along the top of the gallery.
                let t = [Tone::Light, Tone::Base, Tone::Shade][(y - 3) as usize];
                let t = if post && y == 4 && set == 2 { Tone::Deep } else { t };
                (wood.at(t), normal(0, if y == 3 { FACE - 50 } else { FACE }))
            } else if post {
                // A post under it: lit on its left, a grain streak, shaded on its right.
                let grain = set == 2 && (wy + set * 7).rem_euclid(9) < 2;
                let t = match set {
                    0 => Tone::Light,
                    1 => Tone::Lift,
                    4 => Tone::Shade,
                    _ if grain => Tone::Mid,
                    _ => Tone::Base,
                };
                (wood.at(t), normal([-60, -30, 0, 20, 60][set as usize], FACE))
            } else {
                // Rock, with the gold the company came for glinting in it.
                let g = fast(wx.div_euclid(2) as u32, wy.div_euclid(2) as u32, 0x0e3);
                match g % 61 {
                    0 => (Ramp::Brass.at(Tone::High), normal(-30, FACE - 30)),
                    1 => (Ramp::Brass.at(Tone::Base), flat_face),
                    _ => super::hard::rock_face(r, wx, wy),
                }
            }
        }
        P::Crypt => {
            let bone = accent.unwrap_or(Ramp::Bone);
            let nx = wx.rem_euclid(2 * CELL);
            // Its arched top: the first row only across the middle.
            let in_niche = (6..=13).contains(&nx) && (5..=12).contains(&y) && (y != 5 || (8..=11).contains(&nx));
            if in_niche {
                // A loculus: dark, and what was put in it.
                let k = fast(wx.div_euclid(2 * CELL) as u32, wy.div_euclid(CELL) as u32, 0xc4b7);
                let skull = k % 3 != 0;
                let t = match (nx, y) {
                    (9..=11, 9) if skull => bone.at(Tone::Light),
                    (9 | 11, 10) if skull => r.at(Tone::Deep),
                    (10, 10) if skull => bone.at(Tone::Base),
                    (9..=11, 11) if skull => bone.at(Tone::Shade),
                    (7..=12, 12) => bone.at(if nx == 7 { Tone::Light } else { Tone::Mid }),
                    (6, _) | (_, 6) => r.at(Tone::Deep),
                    _ => Ix::SEAM,
                };
                (t, normal(0, FACE - 20))
            } else if (5..=14).contains(&nx) && y == 13 {
                (r.at(Tone::Light), normal(0, FACE - 60))
            } else {
                // Ashlar in deep courses, long blocks staggered, each its own tone.
                let course = wy.div_euclid(8);
                let (yy, xs) = (wy.rem_euclid(8), wx + (course & 1) * 12);
                let lx = xs.rem_euclid(24);
                if yy == 7 || lx == 23 {
                    (r.at(Tone::Deep), normal(0, FACE + 10))
                } else {
                    let t = match fast(xs.div_euclid(24) as u32, course as u32, 0xc4b8) % 7 {
                        0 | 1 => Tone::Mid,
                        6 => Tone::Lift,
                        _ => Tone::Base,
                    };
                    let t = if yy == 0 {
                        t.step(1)
                    } else if yy == 6 {
                        t.step(-1)
                    } else {
                        t
                    };
                    (r.at(t), normal(if lx == 0 { -30 } else { 0 }, FACE))
                }
            }
        }
        P::Ironwork => {
            let brick = accent.unwrap_or(Ramp::Brick);
            let iron = Ramp::Iron;
            let col = wx.rem_euclid(4 * CELL);
            if col < 5 {
                // An iron pilaster, riveted down its middle.
                let t = if col == 2 && wy.rem_euclid(4) == 1 {
                    Tone::High
                } else {
                    [Tone::Light, Tone::Lift, Tone::Base, Tone::Base, Tone::Shade][col as usize]
                };
                (iron.at(t), normal([-60, -30, 0, 20, 60][col as usize], FACE))
            } else if (2..=4).contains(&y) {
                // The beam along the top, riveted.
                let t = if y == 3 && wx.rem_euclid(6) == 0 {
                    Tone::High
                } else {
                    [Tone::Light, Tone::Base, Tone::Shade][(y - 2) as usize]
                };
                (iron.at(t), normal(0, if y == 2 { FACE - 50 } else { FACE }))
            } else {
                // Brick in stretcher bond, sooted darker toward the top.
                let course = wy.div_euclid(4);
                let (yy, lx) = (wy.rem_euclid(4), (wx + (course & 1) * 4).rem_euclid(8));
                if yy == 3 || lx == 7 {
                    (r.at(Tone::Shade), normal(0, FACE + 10))
                } else {
                    let t = match fast((wx + (course & 1) * 4).div_euclid(8) as u32, course as u32, 0x1e0) % 6 {
                        0 => Tone::Shade,
                        1 | 2 => Tone::Base,
                        _ => Tone::Mid,
                    };
                    let t = if y < 8 { t.step(-1) } else { t };
                    (brick.at(if yy == 0 { t.step(1) } else { t }), flat_face)
                }
            }
        }
        P::Panelled => {
            let wood = accent.unwrap_or(Ramp::WoodDark);
            match y {
                // Flock paper: a stripe every eight px.
                0..=6 => (r.at(if wx.rem_euclid(8) == 0 { Tone::Lift } else { Tone::Base }), flat_face),
                7 => (wood.at(Tone::Light), normal(0, FACE - 60)),
                8 => (wood.at(Tone::Shade), normal(0, FACE + 20)),
                9..=14 => {
                    // Moulded panels, a stile between each: the moulding lit on its top and
                    // left, in shade on its foot and right.
                    let lx = wx.rem_euclid(CELL);
                    let t = match (lx, y) {
                        (0 | 15, _) => Tone::Mid,
                        (1, _) | (_, 9) => Tone::Light,
                        (14, _) | (_, 14) => Tone::Deep,
                        (2, _) | (_, 10) => Tone::Shade,
                        _ => Tone::Base,
                    };
                    (wood.at(t), flat_face)
                }
                _ => (wood.at(Tone::Deep), flat_face),
            }
        }
        P::Pipework => {
            let (iron, copper) = (Ramp::Iron, accent.unwrap_or(Ramp::Copper));
            let lx = wx.rem_euclid(2 * CELL);
            let flange = lx < 2;
            if (4..=8).contains(&y) || (flange && (3..=9).contains(&y)) {
                // The main: lit along its top, a flange at each joint.
                let t = match y {
                    3 | 4 => Tone::Light,
                    5 => Tone::Lift,
                    6 => Tone::Base,
                    7 => Tone::Shade,
                    _ => Tone::Deep,
                };
                let t = if flange { t.step(1) } else { t };
                (
                    iron.at(t),
                    normal(
                        0,
                        if y <= 4 {
                            FACE - 60
                        } else if y >= 7 {
                            FACE + 30
                        } else {
                            FACE
                        },
                    ),
                )
            } else if (11..=12).contains(&y) {
                // A copper run below it on brackets.
                if lx == 16 {
                    (iron.at(Tone::Shade), flat_face)
                } else {
                    (copper.at(if y == 11 { Tone::Light } else { Tone::Shade }), flat_face)
                }
            } else {
                // Coursed walling, a stain run down from each flange.
                let course = wy.div_euclid(5);
                let (yy, bx) = (wy.rem_euclid(5), (wx + (course & 1) * 5).rem_euclid(10));
                let stain = flange && y > 8;
                let t = if yy == 4 || bx == 9 {
                    Tone::Deep
                } else if stain {
                    Tone::Shade
                } else {
                    match fast((wx + (course & 1) * 5).div_euclid(10) as u32, course as u32, 0x919e) % 5 {
                        0 => Tone::Mid,
                        4 => Tone::Lift,
                        _ => Tone::Base,
                    }
                };
                (r.at(t), flat_face)
            }
        }
        P::Wainscot => {
            let wood = accent.unwrap_or(Ramp::WoodOak);
            match y {
                0..=7 => (r.at(if y == 7 { Tone::Mid } else { Tone::Base }), flat_face),
                8 => (wood.at(Tone::Light), normal(0, FACE - 60)),
                9 => (wood.at(Tone::Shade), normal(0, FACE + 20)),
                10..=14 => {
                    let t = match wx.rem_euclid(4) {
                        0 => Tone::Shade,
                        1 => Tone::Lift,
                        _ => Tone::Base,
                    };
                    (wood.at(t), normal(if wx.rem_euclid(4) == 1 { -30 } else { 0 }, FACE))
                }
                _ => (wood.at(Tone::Deep), flat_face),
            }
        }
        _ => return None,
    })
}

/// A floor pixel for the interior patterns: `(colour, normal, height over the floor)`, or
/// `None` for a pattern that is not one of them. `wear` is the painter's broad wear field
/// (low: trodden or stained).
pub(super) fn floor(pat: P, r: Ramp, accent: Option<Ramp>, wx: i32, wy: i32, wear: i32) -> Option<(Ix, Normal, i32)> {
    let worn = |t: Tone| {
        if wear < super::hard::WEAR_DARK {
            t.step(-1)
        } else if wear > super::hard::WEAR_LIGHT {
            t.step(1)
        } else {
            t
        }
    };
    Some(match pat {
        P::Parquet => {
            // Two staves to an 8 px square, each square turned from its neighbours and a tone of
            // its own; the staves' joint a px of mid, the squares' a px of shade.
            let joint = accent.unwrap_or(Ramp::WoodDark);
            let (sx, sy) = (wx.div_euclid(8), wy.div_euclid(8));
            let (lx, ly) = (wx.rem_euclid(8), wy.rem_euclid(8));
            if lx == 0 || ly == 0 {
                return Some((joint.at(Tone::Shade), FLAT, 0));
            }
            let across = (sx + sy) & 1 == 0;
            let body = match fast(sx as u32, sy as u32, 0x9a7) % 5 {
                0 => Tone::Mid,
                1 => Tone::Lift,
                _ => Tone::Base,
            };
            let (seam, lit) = if across { (ly == 4, ly == 1 || ly == 5) } else { (lx == 4, lx == 1 || lx == 5) };
            let t = if seam {
                Tone::Mid
            } else if lit {
                body.step(1)
            } else {
                body
            };
            (r.at(worn(t)), FLAT, 0)
        }
        P::Plates => {
            // Riveted plates two cells by one in a running bond, a faint chequer of lugs,
            // oil in the trodden places.
            let (pw, row) = (2 * CELL, wy.div_euclid(CELL));
            let xs = wx + (row & 1) * CELL;
            let (lx, ly) = (xs.rem_euclid(pw), wy.rem_euclid(CELL));
            let body = match fast(xs.div_euclid(pw) as u32, row as u32, 0x91a7) % 5 {
                0 => Tone::Mid,
                _ => Tone::Base,
            };
            // A row of rivets along each plate's top and left edge, a px in.
            let rivet = (ly == 2 && lx % 4 == 2) || (lx == 2 && ly % 4 == 2 && ly > 2);
            let lug = (lx + 2 * ly).rem_euclid(6) == 0 && ly % 3 == 1;
            let (t, n, dz) = if lx == 0 || ly == 0 {
                (Tone::Deep, FLAT, 0)
            } else if rivet {
                (Tone::High, normal(-40, -40), 1)
            } else if lx == 1 || ly == 1 {
                (Tone::Lift, FLAT, 0)
            } else if lx == pw - 1 || ly == CELL - 1 {
                (Tone::Shade, FLAT, 0)
            } else if lug {
                (body.step(1), normal(-30, -30), 0)
            } else {
                (body, FLAT, 0)
            };
            // Oil: only the most trodden places, a tone down.
            let t = if wear < 44 && !rivet { t.step(-1) } else { t };
            (r.at(t), n, dz)
        }
        P::Flags => {
            // Great flags in a running bond; one in eleven a ledger stone, carved.
            let row = wy.div_euclid(12);
            let xs = wx + (row & 1) * 8;
            let (lx, ly) = (xs.rem_euclid(16), wy.rem_euclid(12));
            let k = fast(xs.div_euclid(16) as u32, row as u32, 0xf1a6);
            if lx == 0 || ly == 0 {
                return Some((r.at(Tone::Shade), FLAT, 0));
            }
            let body = match k % 7 {
                0 | 1 => Tone::Mid,
                6 => Tone::Lift,
                _ => Tone::Base,
            };
            let ledger = k % 11 == 0;
            let carved = ledger
                && ((lx == 2 || lx == 13) && (2..=9).contains(&ly)
                    || (ly == 2 || ly == 9) && (2..=13).contains(&lx)
                    || (ly == 4 || ly == 6) && (4..=4 + (k >> 8) as i32 % 8).contains(&lx)
                    || lx == 7 && ly == 8);
            let t = if carved {
                Tone::Deep
            } else if lx == 1 || ly == 1 {
                body.step(1)
            } else {
                body
            };
            (r.at(worn(t)), FLAT, 0)
        }
        P::Grating => {
            // A wet brick invert; a drain grate in one cell in thirty; standing water in the low places.
            let (cx, cy) = (wx.div_euclid(CELL), wy.div_euclid(CELL));
            let (lx, ly) = (wx.rem_euclid(CELL), wy.rem_euclid(CELL));
            if fast(cx as u32, cy as u32, 0x94a7) % 30 == 0 && (4..12).contains(&lx) && (4..12).contains(&ly) {
                let iron = Ramp::Iron;
                let t = if lx == 4 || ly == 4 {
                    Tone::Light
                } else if lx == 11 || ly == 11 {
                    Tone::Shade
                } else if lx % 2 == 0 {
                    Tone::Base
                } else {
                    return Some((Ix::SEAM, FLAT, 0));
                };
                return Some((iron.at(t), FLAT, 0));
            }
            if wear < 40 {
                // Standing water: dark, the odd px of glint on it.
                let glint = (wx * 7 + wy * 13).rem_euclid(29) == 0;
                return Some((Ramp::Water.at(if glint { Tone::Light } else { Tone::Deep }), FLAT, 0));
            }
            // The invert is brick, 12 by 6 in stretcher bond.
            let course = wy.div_euclid(6);
            let xs = wx + (course & 1) * 6;
            let (fx, fy) = (xs.rem_euclid(12), wy.rem_euclid(6));
            let t = if fx == 0 || fy == 0 {
                Tone::Shade
            } else if fy == 1 {
                Tone::Lift
            } else {
                match fast(xs.div_euclid(12) as u32, course as u32, 0x94a8) % 4 {
                    0 => Tone::Mid,
                    _ => Tone::Base,
                }
            };
            (r.at(worn(t)), FLAT, 0)
        }
        _ => return None,
    })
}
