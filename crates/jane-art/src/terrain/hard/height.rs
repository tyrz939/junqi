//! Height's painters (MAP.md §6.1, ART.md §3.1): a plateau's south face by region (the
//! Lowfields' sandstone in courses under an overhang of turf, roots and ivy, scree and nettles at
//! the foot; the Waters' mud and slate banks stained at the waterline, reeds at the foot; the
//! Works' brick retaining walls with buttresses, drain spouts and rust weeps, or slag banks of
//! clinker and glassy lumps), a ledge's turf bank, a waterfall, a stair through a face (stone,
//! timber or iron by region), a ladder, and after the ground: a rim's lit lip and dark drop line, a
//! ramp's road climbing its face between cut walls, the shade by a deck.
//!
//! A face is drawn in the solid cells at a plateau's south edge, two cells a level, its heights
//! climbing from its foot (`face_z`); its top rows are the plateau's turf breaking over the lip.
//! Every px is a pure function of the world px and the cell's height read (`levels::HCell`), so two
//! chunks agree on a seam.

use jane_core::Tile;

use super::{Cell, FACE, face_z, fast, nb, put, step, voronoi};
use crate::canvas::{FLAT, Normal, height_of_rows, normal};
use crate::palette::{Ix, Ramp, Tone};
use crate::terrain::levels::{self, EAST, HCell, Kind, NORTH, WEST};
use crate::terrain::{CELL, CHUNK_CELLS, Painter};

/// Cell `(dx, dy)` from `c` as height reads it.
fn hk(p: &Painter, c: &Cell, dx: i32, dy: i32) -> HCell {
    p.s.hk[Painter::at(c.cx + dx, c.cy + dy)]
}

/// The region `c` is drawn in: 0 the Lowfields, 1 the Waters, 2 the Works.
fn region(p: &Painter, c: &Cell) -> u8 {
    let r = p.s.region[Painter::at(c.cx, c.cy)];
    if r > 2 { 0 } else { r }
}

/// The face's own salt, so its features never line up with the ground's.
const SALT: u32 = 0x4641_4345;

/// Draws cell `c` if height has a painter for it; whether it did.
pub(super) fn paint(p: &mut Painter, c: &Cell) -> bool {
    let h = hk(p, c, 0, 0);
    match (h.kind, nb(p, c, 0, 0)) {
        (Kind::Face, Tile::Waterfall) => waterfall(p, c, h),
        (Kind::Face, Tile::LedgeS) => ledge_face(p, c, h),
        (Kind::Face, _) => face(p, c, h),
        (Kind::Join, Tile::Stair) => stair(p, c, h),
        (Kind::Join, Tile::Ladder) => ladder(p, c, h),
        (Kind::NorthFlight, Tile::Stair) => north_flight(p, c, h),
        _ => return false,
    }
    true
}

/// A px of a face: its colour, its normal, and whether it is the plateau's own lip (standing at
/// the top of the face, not on it).
type Px = (Ix, Normal, bool);

/// The turf breaking over a face's lip: `lip` rows, lit along the top, a ragged fringe under it.
fn turf_lip(wx: i32, fy: i32) -> Option<Px> {
    let ragged = (fast((wx >> 1) as u32, 0x11, SALT) % 4) as i32;
    let lip = 3 + ragged.min(2);
    if fy < lip {
        let t = match fy {
            0 => Tone::High,
            1 => Tone::Light,
            _ if (fast(wx as u32, fy as u32, SALT) % 3) == 0 => Tone::Base,
            _ => Tone::Lift,
        };
        return Some((Ramp::Turf.at(t), normal(0, -50), true));
    }
    // A blade or two hangs over the face; under the turf, its shadow.
    let blade = fast(wx as u32, 0x12, SALT) % 4 == 0;
    let hang = lip + 1 + (fast(wx as u32, 0x13, SALT) % 3) as i32;
    if blade && fy < hang {
        return Some((Ramp::Turf.at(if fy == hang - 1 { Tone::Mid } else { Tone::Base }), normal(0, FACE), false));
    }
    None
}

/// Is the px `fy` rows into a face just under its lip (the turf's shadow on the stone)?
fn under_lip(wx: i32, fy: i32) -> bool {
    let ragged = (fast((wx >> 1) as u32, 0x11, SALT) % 4) as i32;
    let lip = 3 + ragged.min(2);
    fy == lip || fy == lip + 1 && fast(wx as u32, 0x14, SALT) % 2 == 0
}

/// The Lowfields' face: sandstone in courses of five and seven px, each course its own blocks,
/// an iron-stained bed now and then; roots under the turf, ivy hung in patches; scree and
/// nettles at the foot.
fn sandstone(wx: i32, wy: i32, fy: i32, n_px: i32) -> Px {
    if let Some(px) = turf_lip(wx, fy) {
        return px;
    }
    let rock = Ramp::Rock;
    if under_lip(wx, fy) {
        return (Ramp::RockFace.at(Tone::Deep), normal(0, FACE + 20), false);
    }
    // Roots: one column in seventeen, down 4 to 10 px under the turf, wandering a px.
    let rc = wx.div_euclid(3);
    let rh = fast(rc as u32, 0x21, SALT);
    if rh % 17 == 0 {
        let len = 4 + (rh >> 8) as i32 % 7;
        let col = rc * 3 + (((fy >> 2) + (rh >> 12) as i32) & 1);
        if wx == col && fy < 5 + len {
            return (Ramp::Bark.at(if fy & 3 == 0 { Tone::Mid } else { Tone::Shade }), FLAT, false);
        }
    }
    // Ivy: patches by a slow field along the face, hung from the top to a ragged depth.
    let patch = fast((wx >> 5) as u32, 0x22, SALT);
    if patch % 3 == 0 {
        let reach = 8 + (patch >> 8) as i32 % (n_px / 2).max(1) + (fast((wx >> 2) as u32, 0x23, SALT) % 6) as i32;
        let in_patch = (wx & 31) > 4 && (wx & 31) < 28;
        if in_patch && fy < reach {
            let v = voronoi(wx, wy, 3, 0x1717);
            if v.id % 4 != 0 {
                let s = -(v.dx * 2 + v.dy * 3);
                let t = if v.edge {
                    Tone::Shade
                } else if s > 3 {
                    Tone::Light
                } else if s > -2 {
                    Tone::Base
                } else {
                    Tone::Mid
                };
                return (Ramp::Leaf.at(t), normal(v.dx * 15, FACE - 30), false);
            }
        }
    }
    // The foot: scree over the last five rows, nettles in clumps.
    let from_foot = n_px - 1 - fy;
    if from_foot < 6 {
        let nettle = fast((wx >> 2) as u32, 0x24, SALT);
        let tall = 2 + (fast(wx as u32, 0x25, SALT) % 5) as i32;
        if nettle % 3 == 0 && from_foot < tall && (wx & 1 == 0 || from_foot < tall - 1) {
            let t = if from_foot == tall - 1 { Tone::Light } else { Tone::Base };
            return (Ramp::Leaf.at(t), normal(0, 30), false);
        }
        if from_foot < 4 {
            let v = voronoi(wx, wy, 3, 0x5c);
            let t = if v.edge {
                Tone::Shade
            } else if v.dy < 0 {
                Tone::Lift
            } else {
                Tone::Mid
            };
            return (rock.at(t), normal(v.dx * 20, 20), false);
        }
    }
    // Beds of the rock, three to eight px thick by a hash of each, their seams wandering a px or
    // two along the face (a step every few px, never ruled), split into blocks by joints that lean.
    let wob = ((fast((wx >> 2) as u32, 0x26, SALT) % 5) as i32 - 2) / 2
        + ((fast((wx >> 4) as u32, 0x2b, SALT) % 3) as i32 - 1);
    let yy = wy + wob;
    let (bed, ly, th) = bed_of(yy);
    let bh0 = fast(bed as u32, 0x27, SALT);
    let len = 10 + (bh0 % 13) as i32;
    // A joint leans a px every three rows, its way by the bed.
    let lean = if bh0 & 0x100 == 0 { ly / 3 } else { -(ly / 3) };
    let xs = wx + (bh0 >> 9) as i32 % len + lean;
    let (block, lx) = (xs.div_euclid(len), xs.rem_euclid(len));
    let bh = fast(block as u32, bed as u32, SALT);
    // An iron-stained bed now and then, in earth's ramp; the rest the rock's, a shade off its face's.
    let ramp = if bh0 % 6 == 1 { Ramp::Earth } else { rock };
    let body = match bh % 16 {
        5..=6 => Tone::Base,
        7 => Tone::Lift,
        8..=9 => Tone::Shade,
        _ => Tone::Mid,
    };
    // Damp low down: the lowest third a tone darker, block by block.
    let body = if fy > n_px * 2 / 3 && bh % 2 == 0 { body.step(-1) } else { body };
    // A fissure down the face every few cells: a dark crack wandering down two thirds of it.
    let fc = wx.div_euclid(37);
    let fh = fast(fc as u32, 0x29, SALT);
    let fx = fc * 37 + (fh % 30) as i32 + (((fy >> 2) + (fh >> 8) as i32) & 1);
    if fh % 3 != 0 && wx == fx && fy < n_px * 2 / 3 + (fh >> 12) as i32 % 6 {
        return (Ramp::RockFace.at(Tone::Deep), normal(-30, FACE), false);
    }
    if ly == th - 1 {
        return (Ramp::RockFace.at(if bh % 3 == 0 { Tone::Deep } else { Tone::Shade }), normal(0, FACE + 25), false);
    }
    if lx == 0 {
        return (Ramp::RockFace.at(Tone::Deep), normal(-20, FACE), false);
    }
    // Lichen in pale rosettes on a few blocks' upper edges; a pit or two where it weathers.
    let lichen = bh >> 20 & 15 == 0 && ly < 2 && (2..6).contains(&lx);
    let pit = (bh >> 12) % 7 == 0 && lx == 3 + (bh >> 16) as i32 % 4 && ly == 1 + (bh >> 24) as i32 % 2;
    let (r, t, n) = if lichen {
        (Ramp::Bone, if ly == 0 { Tone::Base } else { Tone::Mid }, normal(0, FACE - 30))
    } else if pit {
        (ramp, body.step(-2), normal(0, FACE))
    } else if ly == 0 {
        (ramp, body.step(1), normal(0, FACE - 50))
    } else if lx == 1 {
        (ramp, body.step(1), normal(-40, FACE))
    } else if lx == len - 1 || ly == th - 2 {
        (ramp, body.step(-1), normal(25, FACE + 15))
    } else {
        (ramp, body, normal(0, FACE))
    };
    (r.at(t), n, false)
}

/// The bed of the rock at world row `y` (beds three to eight px thick, by the hash of each block
/// of 32 rows), the row within it, and its thickness.
fn bed_of(y: i32) -> (i32, i32, i32) {
    let blk = y.div_euclid(32);
    let mut top = blk * 32;
    let mut i = 0;
    loop {
        let th = 3 + (fast(blk as u32, i, SALT ^ 0xbed) % 6) as i32;
        if y < top + th || top + th >= (blk + 1) * 32 {
            let th = th.min((blk + 1) * 32 - top);
            return (blk * 16 + i as i32, y - top, th.max(1));
        }
        top += th;
        i += 1;
    }
}

/// The Waters' face: banks of mud in soft bands with plates of slate through them, dipping a
/// little; a crust line where the water stood and green-black stain under it; reeds at the foot.
fn mudbank(wx: i32, wy: i32, fy: i32, n_px: i32) -> Px {
    if let Some(px) = turf_lip(wx, fy) {
        return px;
    }
    if under_lip(wx, fy) {
        return (Ramp::Mud.at(Tone::Deep), normal(0, FACE + 20), false);
    }
    let from_foot = n_px - 1 - fy;
    // Reeds at the foot: clumps, each blade a px, its tip lit.
    let clump = fast((wx >> 2) as u32, 0x31, SALT);
    let tall = 4 + (fast(wx as u32, 0x32, SALT) % 8) as i32;
    if clump % 5 < 3 && (wx & 1 == 0 || clump % 2 == 0) && from_foot < tall {
        let t = if from_foot >= tall - 2 { Tone::Light } else { Tone::Base };
        return (Ramp::Reed.at(t), normal(0, 30), false);
    }
    // The waterline: a pale crust, the stain under it.
    let wl = n_px * 2 / 3 + ((fast((wx >> 3) as u32, 0x33, SALT) % 3) as i32 - 1);
    if fy == wl {
        return (Ramp::Mud.at(Tone::High), normal(0, FACE - 40), false);
    }
    if fy > wl {
        // Drips of stain down from the line, a darker green-black toward the foot.
        let drip = fast(wx as u32, 0x34, SALT) % 4 == 0;
        let t = if drip || fy > wl + 4 { Tone::Shade } else { Tone::Mid };
        return (Ramp::Marsh.at(t), normal(0, FACE), false);
    }
    // Slate plates: a band in five, two px, dipping a px every twelve along, broken here and there.
    let yy = wy + wx.div_euclid(12);
    let band = yy.div_euclid(4);
    let bh = fast(band as u32, 0x35, SALT);
    if bh % 5 == 0 {
        let ly = yy.rem_euclid(4);
        let gap = fast((wx >> 2) as u32, band as u32, SALT) % 5 == 0;
        if !gap && ly < 2 {
            let t = if ly == 0 { Tone::Light } else { Tone::Mid };
            return (Ramp::Slate.at(t), normal(0, FACE - 30 * i32::from(ly == 0)), false);
        }
        if !gap && ly == 2 {
            return (Ramp::Slate.at(Tone::Deep), normal(0, FACE + 20), false);
        }
    }
    // Mud in soft bands, a clod lit on its top now and then.
    let v = voronoi(wx, wy * 2, 5, 0x3d);
    let t = match bh % 4 {
        0 => Tone::Mid,
        1 => Tone::Lift,
        _ => Tone::Base,
    };
    let t = if v.edge {
        t.step(-1)
    } else if v.dy < -3 && v.id % 3 == 0 {
        t.step(1)
    } else {
        t
    };
    (Ramp::Mud.at(t), normal(v.dx * 10, FACE), false)
}

/// The Works' face: brick retaining walls in stretcher bond under a stone coping, a buttress every
/// ten cells, drain spouts with rust run from them; or, a run in five, a bank of slag: clinker in
/// lumps lit on top, glassy black, rust.
fn works_face(wx: i32, wy: i32, fy: i32, n_px: i32) -> Px {
    let seg = fast((wx.div_euclid(16 * 6)) as u32, 0x41, SALT);
    if seg % 5 < 2 {
        return slag(wx, wy, fy, n_px);
    }
    // The coping: three rows of dressed stone, its shadow under it.
    if fy < 3 {
        let t = [Tone::Light, Tone::Base, Tone::Mid][fy as usize];
        let joint = wx.rem_euclid(14) == 0;
        return (
            Ramp::Stone.at(if joint { Tone::Shade } else { t }),
            normal(0, if fy == 0 { -50 } else { 20 }),
            fy == 0,
        );
    }
    if fy == 3 {
        return (Ramp::Brick.at(Tone::Deep), normal(0, FACE + 20), false);
    }
    // A buttress: six px stepped out, lit west and shaded east, its own coping.
    let bx = wx.rem_euclid(16 * 10);
    let buttress = bx < 7;
    let by = fy - 4;
    let (course, ly) = (by.div_euclid(4), by.rem_euclid(4));
    let xs = wx + (course & 1) * 4;
    let (block, lx) = (xs.div_euclid(8), xs.rem_euclid(8));
    let bh = fast(block as u32, course as u32, SALT ^ 0x6b);
    // A drain spout, a cell in five, at two thirds down, its rust run under it.
    let spout_x = wx.rem_euclid(80);
    let spout_y = n_px * 3 / 5;
    if (38..41).contains(&spout_x) && (spout_y..spout_y + 2).contains(&fy) {
        let t = if spout_x == 38 { Tone::Lift } else { Tone::Shade };
        return (Ramp::Iron.at(if fy == spout_y { t } else { Tone::Deep }), normal(0, -20), false);
    }
    let run = (37..42).contains(&spout_x) && fy > spout_y + 1 && fy < spout_y + 18 - (spout_x - 39).abs() * 4;
    if run && (fy + spout_x) % 4 != 0 {
        let t = if fy < spout_y + 6 { Tone::Mid } else { Tone::Shade };
        return (Ramp::Copper.at(t), normal(0, FACE), false);
    }
    // Rust weeps: a column in nineteen, from a third down, four to ten px.
    let weep = fast(wx as u32, 0x42, SALT);
    if weep % 19 == 0 && fy > n_px / 3 && fy < n_px / 3 + 4 + (weep >> 8) as i32 % 7 {
        return (Ramp::Copper.at(Tone::Mid), normal(0, FACE), false);
    }
    let mortar = ly == 3 || lx == 7;
    // Sooted from the top, damp from the foot: the brick runs dark, the odd one paler.
    let soot = fy < n_px / 2 && bh % 2 == 0;
    let body = match bh % 11 {
        0..=2 => Tone::Shade,
        3 => Tone::Base,
        4 => Tone::Lift,
        _ => Tone::Mid,
    };
    let body = if soot { body.step(-1) } else { body };
    if buttress {
        let t = match bx {
            0 | 6 => Tone::Deep,
            1 => body.step(2),
            5 => body.step(-1),
            _ if mortar => Tone::Shade,
            _ => body.step(1),
        };
        let nx = match bx {
            0 | 1 => -60,
            5 | 6 => 60,
            _ => 0,
        };
        return (Ramp::Brick.at(t), normal(nx, FACE - 10), false);
    }
    if mortar {
        return (Ramp::Brick.at(Tone::Deep), normal(0, FACE + 10), false);
    }
    let t = if ly == 0 { body.step(1) } else { body };
    let from_foot = n_px - 1 - fy;
    let t = if from_foot < 3 { t.step(-1) } else { t };
    (Ramp::Brick.at(t), normal(if lx == 0 { -25 } else { 0 }, if ly == 0 { FACE - 40 } else { FACE }), false)
}

/// A slag bank: lumps of clinker lit on their top, glassy black ones glinting, rust here and there;
/// a lip of cinder and slag grass.
fn slag(wx: i32, wy: i32, fy: i32, n_px: i32) -> Px {
    if fy < 3 {
        let grass = fast(wx as u32, 0x51, SALT) % 3 == 0;
        let t = if fy == 0 { Tone::Light } else { Tone::Base };
        return if grass {
            (Ramp::Turf.at(t), normal(0, -50), true)
        } else {
            (Ramp::Ballast.at(t.step(1)), normal(0, -50), true)
        };
    }
    let v = voronoi(wx, wy, 5, 0x5a6);
    let s = -(v.dx * 2 + v.dy * 3);
    if v.id % 11 == 0 {
        // Glassy slag: near black, a glint on its upper left.
        let t = if s > 6 {
            Tone::Light
        } else if s > 2 {
            Tone::Mid
        } else {
            Tone::Deep
        };
        return (Ramp::WallDark.at(t), normal(v.dx * 20, FACE - 20), false);
    }
    let ramp = if v.id % 13 == 0 { Ramp::Copper } else { Ramp::Works };
    let body = match v.id % 5 {
        0 => Tone::Mid,
        1 => Tone::Lift,
        _ => Tone::Base,
    };
    let t = if v.edge {
        Tone::Deep
    } else if s > 4 {
        body.step(1)
    } else if s < -5 {
        body.step(-1)
    } else {
        body
    };
    let t = if n_px - 1 - fy < 3 { t.step(-1) } else { t };
    (ramp.at(t), normal((v.dx * 18).clamp(-70, 70), FACE - v.dy * 6), false)
}

/// A face px in region `reg`.
fn face_px(reg: u8, wx: i32, wy: i32, fy: i32, n_px: i32) -> Px {
    match reg {
        1 => mudbank(wx, wy, fy, n_px),
        2 => works_face(wx, wy, fy, n_px),
        _ => sandstone(wx, wy, fy, n_px),
    }
}

/// Px from the face's top of row 0 of a face or join cell.
fn top_row(h: HCell) -> i32 {
    (i32::from(h.n) - 1 - i32::from(h.k)) * CELL
}

/// The height of the face's top over its foot.
fn top_z(h: HCell) -> i32 {
    height_of_rows(i32::from(h.n) * CELL)
}

/// A plateau's south face.
fn face(p: &mut Painter, c: &Cell, h: HCell) {
    let reg = region(p, c);
    let n_px = i32::from(h.n) * CELL;
    let row0 = top_row(h);
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (ix, n, lip) = face_px(reg, wx, wy, row0 + y, n_px);
            let z = if lip { top_z(h) } else { face_z(y, i32::from(h.k)) };
            put(p, c, x, y, ix, n, z);
        }
    }
    ends(p, c, h);
}

/// A face's ends: where lower ground lies beside it, the face turns away (lit west, dark east);
/// where a join lies beside it, its cut wall; where a waterfall does, the rock is wet.
fn ends(p: &mut Painter, c: &Cell, h: HCell) {
    let side = |p: &Painter, dx: i32| {
        let n = hk(p, c, dx, 0);
        let t = nb(p, c, dx, 0);
        (n.kind == Kind::Join || t == Tile::Stair || t == Tile::Ladder, t == Tile::Waterfall)
    };
    for (dx, x_edge, x_in, open_bit) in [(-1, 0, 1, WEST), (1, CELL - 1, CELL - 2, EAST)] {
        let (join, fall) = side(p, dx);
        let lit = dx < 0;
        if h.side & open_bit != 0 || join {
            for y in 0..CELL {
                let z = p.s.ly.z(c.px + x_in, c.py + y);
                if let Some((r, _)) = p.s.ly.tone(c.px + x_edge, c.py + y) {
                    put(p, c, x_edge, y, r.at(Tone::Deep), FLAT, z);
                }
                step(p, c, x_in, y, if lit { 1 } else { -1 });
                if join {
                    // The cut wall's face, a px further in, a tone off the face's.
                    step(p, c, x_in + if lit { 1 } else { -1 }, y, if lit { 1 } else { -1 });
                }
            }
        }
        if fall {
            for y in 0..CELL {
                for d in 0..3 {
                    let x = if dx < 0 { d } else { CELL - 1 - d };
                    if (y + d + c.wy) % 3 != 0 || d == 0 {
                        step(p, c, x, y, -1);
                    }
                }
            }
        }
    }
}

/// A ledge's face (MAP.md §2.4): a bank, not a cliff. Its crown rounds over and is lit (it reads
/// "hop me" from above); the bank falls away from the sky in clumps, each lit on its upper left and
/// a tone darker the lower it lies; a ragged foot of bare ground. By region: the Lowfields' turf,
/// the Waters' reed lip over a muddy bank, the Works' broken kerb over a bank of cinders. From
/// below it is plainly a bank too high to climb.
fn ledge_face(p: &mut Painter, c: &Cell, h: HCell) {
    let reg = region(p, c);
    let n_px = i32::from(h.n) * CELL;
    let row0 = top_row(h);
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (ix, n) = ledge_px(reg, wx, wy, row0 + y, n_px);
            let z = if row0 + y < 4 { top_z(h) } else { face_z(y, i32::from(h.k)) };
            put(p, c, x, y, ix, n, z);
        }
    }
    ends(p, c, h);
}

/// A px of a ledge's bank in region `reg`, `fy` rows down a bank `n_px` tall.
fn ledge_px(reg: u8, wx: i32, wy: i32, fy: i32, n_px: i32) -> (Ix, Normal) {
    let from_foot = n_px - 1 - fy;
    let (bank, foot_ramp) = match reg {
        2 => (Ramp::Ballast, Ramp::Works),
        _ => (Ramp::Turf, Ramp::Earth),
    };
    // The crown.
    if fy < 4 {
        return match reg {
            1 => {
                // A lip of reeds: their tips lit, gaps of the dark between them.
                let blade = fast(wx as u32, 0x63, SALT) % 3 != 0;
                let t = [Tone::High, Tone::Light, Tone::Base, Tone::Mid][fy as usize];
                if blade { (Ramp::Reed.at(t), normal(0, -60)) } else { (Ramp::Turf.at(t.step(-1)), normal(0, -40)) }
            }
            2 => {
                // A kerb of setts, broken: a stone gone every so often, cinders in the gap.
                let stone = wx.div_euclid(7);
                let gone = fast(stone as u32, 0x64, SALT) % 5 == 0;
                if gone {
                    (Ramp::Ballast.at([Tone::Base, Tone::Mid, Tone::Shade, Tone::Shade][fy as usize]), FLAT)
                } else {
                    let t = if wx.rem_euclid(7) == 0 {
                        Tone::Shade
                    } else {
                        [Tone::Light, Tone::Base, Tone::Mid, Tone::Deep][fy as usize]
                    };
                    (Ramp::Setts.at(t), normal(0, -50 + fy * 30))
                }
            }
            _ => {
                let t = [Tone::Lift, Tone::High, Tone::Light, Tone::Base][fy as usize];
                (Ramp::Turf.at(t), normal(0, -70 + fy * 30))
            }
        };
    }
    // The foot: bare ground, ragged.
    let soil = 2 + (fast((wx >> 1) as u32, 0x62, SALT) % 3) as i32;
    if from_foot < soil {
        let v = voronoi(wx, wy, 3, 0x51);
        let t = if from_foot == 0 {
            Tone::Deep
        } else if v.edge {
            Tone::Shade
        } else {
            Tone::Mid
        };
        return (foot_ramp.at(t), normal(0, FACE));
    }
    // The bank in clumps, squashed across: each clump's tone by how low it lies (with a jitter of
    // its own, so no row is ruled), lit on its upper left.
    let v = voronoi(wx, wy * 2, 5, 0x6c);
    let span = (n_px - 4 - soil).max(1);
    let low = ((fy - 4) * 6 / span + (v.id % 3) as i32 - 1).clamp(0, 5);
    let body = [Tone::Light, Tone::Base, Tone::Base, Tone::Mid, Tone::Mid, Tone::Shade][low as usize];
    let s = -(v.dx * 2 + v.dy * 3);
    let t = if v.edge {
        body.step(-1)
    } else if s > 7 {
        body.step(1)
    } else {
        body
    };
    // A reed or a blade combed down here and there.
    let blade = fast(wx as u32, (wy >> 2) as u32, SALT ^ 0x6b) % 23 == 0;
    let (r, t) = match (reg, blade) {
        (1, true) => (Ramp::Reed, Tone::Base),
        (0, true) => (Ramp::Turf, t.step(2)),
        _ => (bank, t),
    };
    (r.at(t), normal((v.dx * 12).clamp(-50, 50), FACE - 35))
}

/// A waterfall (MAP.md §2.6): water sheeting over the lip in streaks down wet dark rock, a curl of
/// white at the top, foam churning at the foot. Still in the chunk; the presenter sets streaks
/// and spray moving over it.
fn waterfall(p: &mut Painter, c: &Cell, h: HCell) {
    let n_px = i32::from(h.n) * CELL;
    let row0 = top_row(h);
    let w = |dx: i32| nb(p, c, dx, 0) == Tile::Waterfall;
    let (west, east) = (w(-1), w(1));
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let fy = row0 + y;
            let from_foot = n_px - 1 - fy;
            // The sheet's own ragged edges where the fall ends.
            let edge = (!west && x < 2 + (fast(wy as u32 >> 2, 0x71, SALT) % 2) as i32)
                || (!east && x > CELL - 3 - (fast(wy as u32 >> 2, 0x72, SALT) % 2) as i32);
            let (ix, n) = if edge {
                let v = voronoi(wx, wy, 4, 0x7a);
                (Ramp::RockFace.at(if v.edge { Tone::Deep } else { Tone::Shade }), normal(0, FACE))
            } else if fy < 3 {
                // Over the lip: a glassy curl, white at its crest.
                let t = [Tone::Light, Tone::Glint, Tone::High][fy as usize];
                (Ramp::Water.at(t), normal(0, -40))
            } else if from_foot < 5 {
                // Foam at the foot, churning in blobs.
                let v = voronoi(wx, wy, 3, 0x7f);
                let t = if v.edge {
                    Tone::Light
                } else if v.dy < 0 {
                    Tone::Glint
                } else {
                    Tone::High
                };
                (Ramp::Water.at(t), normal(v.dx * 20, 30))
            } else {
                // Streaks: each column its own phase and length, lit streaks over a body of the
                // water's mid tones, a darker vein now and then.
                let sh = fast(wx as u32, 0x73, SALT);
                let len = 5 + (sh % 7) as i32;
                let along = (wy + (sh >> 4) as i32 % 32).rem_euclid(len + 2);
                let t = match (along, sh % 5) {
                    (0, _) => Tone::High,
                    (1, _) => Tone::Light,
                    (_, 0) => Tone::Mid,
                    (_, 1) => Tone::Base,
                    _ => Tone::Lift,
                };
                (Ramp::Water.at(t), normal(0, FACE - 20))
            };
            let z = if fy < 2 { top_z(h) } else { face_z(y, i32::from(h.k)) };
            put(p, c, x, y, ix, n, z);
        }
    }
}

/// Where cell `c` lies across its flight: its column from the flight's west end and the flight's
/// width in cells (six cells looked each way).
fn across(p: &Painter, c: &Cell, t: Tile) -> (i32, i32) {
    let mut w = 0;
    while w < 6 && nb(p, c, -(w + 1), 0) == t {
        w += 1;
    }
    let mut e = 0;
    while e < 6 && nb(p, c, e + 1, 0) == t {
        e += 1;
    }
    (w, w + e + 1)
}

/// A stair through a face (MAP.md §2.3): eight treads a level, a nosing lit along each, the riser
/// under it in the tread's shade; worn pale up its middle. The Lowfields' and the Waters' quays
/// in stone with cheek walls where the face is beside them, a rail on a flight of two levels; the
/// Waters' timber treads on piles; the Works' iron, chequer plate on stringers.
fn stair(p: &mut Painter, c: &Cell, h: HCell) {
    let reg = region(p, c);
    let row0 = top_row(h);
    let n_px = i32::from(h.n) * CELL;
    let (col, cols) = across(p, c, Tile::Stair);
    let wide = cols * CELL;
    let cheek_w = matches!(nb(p, c, -1, 0), t if levels::cliffy(t));
    let cheek_e = matches!(nb(p, c, 1, 0), t if levels::cliffy(t));
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let fy = row0 + y;
            let ax = col * CELL + x;
            let pitch = 4;
            let (tread, r) = (fy.div_euclid(pitch), fy.rem_euclid(pitch));
            // Each tread's top stands at its own height: the flight casts in steps.
            let z = height_of_rows(n_px - tread * pitch).max(1);
            // Worn in the middle of the flight: the middle third a tone paler on the tread.
            let mid = (ax - wide / 3).clamp(-1, wide / 3 + 1);
            let worn = mid >= 0 && mid <= wide / 3 && r < 2;
            let (ramp, t, n) = match reg {
                2 => {
                    // Iron: chequer plate on the tread, a riser of plate with a rivet line.
                    let chequer = (wx + wy).rem_euclid(4) == 0 && r == 1;
                    let t = match r {
                        0 => Tone::Light,
                        1 if chequer => Tone::High,
                        1 => Tone::Base,
                        2 => Tone::Deep,
                        _ if wx.rem_euclid(6) == 2 => Tone::Lift,
                        _ => Tone::Mid,
                    };
                    (Ramp::Iron, t, normal(0, if r < 2 { -50 } else { FACE }))
                }
                1 => {
                    // Timber treads, open risers: the dark under each board, the boards' joints.
                    let joint = wx.rem_euclid(11) == 0;
                    let t = match r {
                        0 => Tone::Light,
                        1 if joint => Tone::Shade,
                        1 => Tone::Lift,
                        2 => Tone::Mid,
                        _ => Tone::Deep,
                    };
                    let t = if worn && r < 2 { t.step(1) } else { t };
                    (Ramp::WoodOak, t, normal(0, if r < 2 { -50 } else { FACE }))
                }
                _ => {
                    // Stone: a nosing, the tread, the shadow under the nosing, the riser.
                    let slab = fast((wx.div_euclid(10)) as u32, tread as u32, SALT) % 5;
                    let body = if slab == 0 { Tone::Mid } else { Tone::Base };
                    let t = match r {
                        0 => Tone::Light,
                        1 => body.step(1),
                        2 => Tone::Shade,
                        _ => body,
                    };
                    let t = if worn { t.step(1) } else { t };
                    let joint = wx.rem_euclid(10) == 0 && r != 0;
                    (Ramp::Stone, if joint { Tone::Shade } else { t }, normal(0, if r < 2 { -55 } else { FACE }))
                }
            };
            put(p, c, x, y, ramp.at(t), n, z);
            // Moss in the Lowfields' treads' corners.
            if reg == 0 && r == 1 && (ax < 2 || ax >= wide - 2) && fast(wx as u32, wy as u32, SALT) % 3 == 0 {
                put(p, c, x, y, Ramp::Turf.at(Tone::Mid), FLAT, z);
            }
        }
    }
    // Cheek walls where the face is beside the flight, a rail on the steep ones.
    for (on, x0, lit) in [(cheek_w, 0, true), (cheek_e, CELL - 3, false)] {
        if !on {
            continue;
        }
        let ramp = match reg {
            2 => Ramp::Iron,
            1 => Ramp::WoodDark,
            _ => Ramp::Stone,
        };
        for y in 0..CELL {
            let fy = row0 + y;
            let z = height_of_rows(n_px - fy + 4).max(2);
            for d in 0..3 {
                let t = match (d, lit) {
                    (0, true) | (2, false) => Tone::Deep,
                    (1, true) => Tone::Light,
                    (1, false) => Tone::Mid,
                    _ => Tone::Base,
                };
                let t = if y % 8 == 0 && d == 1 { t.step(-1) } else { t };
                put(p, c, x0 + d, y, ramp.at(t), normal(if lit { -50 } else { 50 }, -30), z);
            }
        }
    }
    if h.n >= 4 && reg == 0 && cheek_e {
        for y in 0..CELL {
            let fy = row0 + y;
            let z = height_of_rows(n_px - fy + 14).max(2);
            put(p, c, CELL - 5, y, Ramp::Iron.at(Tone::Shade), FLAT, z);
            if fy % 8 == 0 {
                put(p, c, CELL - 5, y, Ramp::Iron.at(Tone::Light), FLAT, z);
            }
        }
    }
}

/// A flight down a north rim: four treads falling away north inside the cell, each lit along its
/// edge with the next lower beyond a dark line.
fn north_flight(p: &mut Painter, c: &Cell, _h: HCell) {
    let reg = region(p, c);
    let ramp = match reg {
        2 => Ramp::Iron,
        1 => Ramp::WoodOak,
        _ => Ramp::Stone,
    };
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, _) = c.w(x, y);
            let r = y.rem_euclid(4);
            let tread = y / 4;
            let t = match r {
                0 => Tone::Deep,
                1 => Tone::Light,
                _ => Tone::Base,
            };
            let t = if tread == 0 && r == 0 { Tone::Shade } else { t };
            let joint = wx.rem_euclid(10) == 0 && r > 1;
            put(p, c, x, y, ramp.at(if joint { Tone::Mid } else { t }), normal(0, -30), (tread + 1).max(1));
        }
    }
}

/// A ladder up a face: the face behind it, two stiles and a rung every four rows, its shadow on
/// the stone to its right. Timber, iron in the Works.
fn ladder(p: &mut Painter, c: &Cell, h: HCell) {
    let reg = region(p, c);
    face(p, c, HCell { kind: Kind::Face, side: 0, ..h });
    let ramp = if reg == 2 { Ramp::Iron } else { Ramp::WoodOak };
    let row0 = top_row(h);
    for y in 0..CELL {
        let fy = row0 + y;
        let z = face_z(y, i32::from(h.k)) + 2;
        for (x, t) in [(3, Tone::Light), (4, Tone::Base), (11, Tone::Lift), (12, Tone::Mid)] {
            put(p, c, x, y, ramp.at(t), normal(if x == 3 || x == 11 { -40 } else { 20 }, FACE), z);
        }
        step(p, c, 5, y, -2);
        step(p, c, 13, y, -2);
        if fy.rem_euclid(4) == 1 {
            for x in 5..11 {
                put(p, c, x, y, ramp.at(Tone::Lift), normal(0, -30), z);
                step(p, c, x, y + 1, -2);
            }
        }
    }
}

/// After the ground: each rim's lip (a lit edge and the dark drop past it; a sliver of face on a
/// side, lit west, shaded east), a ledge's rim rounded over toward its drop, a ramp's road
/// climbing its face (heights and a shade toward the foot), the shade under and beside a deck.
pub(in crate::terrain) fn overlay(p: &mut Painter, x0: i32, y0: i32, seed: u32) {
    let _ = (x0, y0, seed);
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let k = Painter::at(cx, cy);
            let h = p.s.hk[k];
            let (px, py) = (cx * CELL, cy * CELL);
            let t = p.s.raw[k];
            if p.levels && h.kind == Kind::Rim {
                rim(p, (px, py), h, t, (x0 + cx, y0 + cy));
            }
            if p.levels && h.kind == Kind::Join && p.s.surf[k] != super::NONE {
                ramp(p, (px, py), h);
            }
            if p.levels && h.kind == Kind::Plain && p.s.hk[Painter::at(cx, cy - 1)].kind == Kind::Face {
                foot_shade(p, (px, py), (x0 + cx, y0 + cy));
            }
            let d = p.s.deck[k];
            if d != 0 {
                deck_shade(p, (px, py), d, h.base);
            }
        }
    }
}

/// A rim's edges on the ground the rim was drawn as.
fn rim(p: &mut Painter, (px, py): (i32, i32), h: HCell, t: Tile, (wx, wy): (i32, i32)) {
    let ly = &mut p.s.ly;
    let ledge = t.ledge_dir();
    let z = |ly: &super::super::Layers, x: i32, y: i32| ly.z(px + x, py + y);
    if h.side & NORTH != 0 {
        if ledge == Some((0, -1)) {
            // A ledge hopped north: its lip rounded over, three rows, lit on the crown.
            for x in 0..CELL {
                for (y, tone) in [(0, Tone::Shade), (1, Tone::Light), (2, Tone::High), (3, Tone::Lift)] {
                    let zz = z(ly, x, y).max(3);
                    ly.put(px + x, py + y, Ramp::Turf.at(tone), normal(0, -60), zz);
                }
            }
        } else {
            for x in 0..CELL {
                let wob = (fast((wx * CELL + x) as u32 >> 1, wy as u32, SALT) % 3 == 0) as i32;
                // The drop line, the lit lip, the ground a tone up just in from it.
                let zz = z(ly, x, 2).max(2);
                if let Some((r, _)) = ly.tone(px + x, py + 2) {
                    ly.put(px + x, py, r.at(Tone::Deep), FLAT, zz);
                    ly.put(px + x, py + 1, r.at(if wob == 1 { Tone::Light } else { Tone::High }), normal(0, -70), zz);
                }
                ly.step(px + x, py + 2 + wob, 1);
            }
        }
    }
    for (bit, dir, x_edge, x_in, lit) in [(WEST, (-1, 0), 0, 1, true), (EAST, (1, 0), CELL - 1, CELL - 2, false)] {
        if h.side & bit == 0 {
            continue;
        }
        let x_lip = if lit { x_in + 1 } else { x_in - 1 };
        for y in 0..CELL {
            let zz = z(ly, x_lip, y).max(2);
            if ledge == Some(dir) {
                // A ledge hopped this way: a rounded turf lip, its crown lit, notched every few
                // rows as the turf rolls over.
                let notch = (wy * CELL + y).rem_euclid(6) == 0;
                ly.put(px + x_edge, py + y, Ramp::Turf.at(Tone::Shade), normal(0, 0), zz);
                ly.put(
                    px + x_in,
                    py + y,
                    Ramp::Turf.at(if notch { Tone::Base } else { Tone::High }),
                    normal(0, -40),
                    zz,
                );
                ly.put(px + x_lip, py + y, Ramp::Turf.at(Tone::Light), normal(0, -40), zz);
            } else if let Some((r, _)) = ly.tone(px + x_lip, py + y) {
                // The face seen edge-on: a sliver of stone, lit west and dark east.
                let rock = Ramp::Rock;
                ly.put(px + x_edge, py + y, rock.at(if lit { Tone::Base } else { Tone::Deep }), FLAT, zz);
                ly.put(px + x_in, py + y, if lit { r.at(Tone::High) } else { rock.at(Tone::Shade) }, FLAT, zz);
                ly.step(px + x_lip, py + y, if lit { 1 } else { -1 });
            }
        }
    }
}

/// The ground at a face's foot: past the contact shade's four rows, four more a tone down, the
/// last in 2 px clusters (the ambient light a face two cells tall keeps off the ground at its
/// foot; on a console, whose sun shadows lay no face, the face's only shade).
fn foot_shade(p: &mut Painter, (px, py): (i32, i32), (wx, _wy): (i32, i32)) {
    for y in 2..8 {
        for x in 0..CELL {
            if y == 7 && ((wx * CELL + x) >> 1) & 1 == 0 {
                continue;
            }
            p.s.ly.step(px + x, py + y, -1);
        }
    }
}

/// A ramp's road through a face: its heights climb from the foot like the face beside it, and it
/// darkens a little toward the foot (a slope turned from the sky).
fn ramp(p: &mut Painter, (px, py): (i32, i32), h: HCell) {
    for y in 0..CELL {
        let zz = face_z(y, i32::from(h.k));
        // The slope's lower half turned from the sky, a tone down; its foot's last rows two.
        let shade = if h.k == 0 { i32::from(y > CELL / 2) + i32::from(y > CELL - 3) } else { 0 };
        for x in 0..CELL {
            if let Some(i) = super::super::Layers::i(px + x, py + y) {
                let rel = i32::from(p.s.ly.height[i]).min(4);
                p.s.ly.height[i] = (zz + rel).clamp(1, 255) as u8;
            }
            if shade > 0 {
                p.s.ly.step(px + x, py + y, -shade);
            }
        }
    }
}

/// The shade under and beside a deck: the ground under it two tones down, and four px of ground
/// beside it a tone down, the two nearest two.
fn deck_shade(p: &mut Painter, (px, py): (i32, i32), d: u8, base: u8) {
    let ly = &mut p.s.ly;
    if d & levels::DECK_UNDER != 0 {
        // The deck stands over this cell at its own level (MAP.md §6.2): its heights are the
        // deck's, so every tier casts the deck's shadow on the ground beside it as a slab's, and
        // what is under it is under it.
        let z = (i32::from(levels::deck_level(d)) - i32::from(base)) * levels::LEVEL_PX + 3;
        for y in 0..CELL {
            for x in 0..CELL {
                ly.step(px + x, py + y, -2);
                if let Some(i) = super::super::Layers::i(px + x, py + y) {
                    ly.height[i] = ly.height[i].max(z.clamp(1, 255) as u8);
                }
            }
        }
        return;
    }
    // Darker the nearer the edge the deck lies along: three px two tones, five more one, the
    // step between them in 2 px clusters along it.
    let near = |e: i32, along: i32| -> i32 {
        if e < 3 || e == 3 && along & 2 == 0 {
            -2
        } else if e < 8 || e == 8 && along & 2 == 0 {
            -1
        } else {
            0
        }
    };
    for y in 0..CELL {
        for x in 0..CELL {
            let mut s = 0;
            for (bit, e, along) in [
                (levels::DECK_E, CELL - 1 - x, y),
                (levels::DECK_W, x, y),
                (levels::DECK_S, CELL - 1 - y, x),
                (levels::DECK_N, y, x),
            ] {
                if d & bit != 0 {
                    s = s.min(near(e, along));
                }
            }
            if s != 0 {
                ly.step(px + x, py + y, s);
            }
        }
    }
}
