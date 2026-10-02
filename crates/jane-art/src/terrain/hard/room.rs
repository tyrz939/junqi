//! A room seen from inside (ART-PLAN M4): its back wall drawn two cells tall over the cells the
//! wall already fills (drawing only: they were solid), papered or limewashed by whose room it is
//! (Julie's faded sprigs or stripes, the Arms' oxblood flock over dark panelling, St Anne's
//! limewash over ashlar), a cornice at its head, a dado rail, boards or panels under it and a
//! skirting at its foot; hung with its things (`houses::Decor`): windows that are the sky by day
//! and dark at night, a picture, a calendar, a mirror, a plate rack, the Arms' dartboard and its
//! sign's twin, St Anne's hymn board, and Julie's clock, stopped at five to nine under a
//! cobweb. By day each window lays its light on the floor under it, sheared as the sun comes in.

use jane_core::tile::F_SOLID;

use super::super::houses::{Decor, Room, RoomKind};
use super::super::{CELL, CHUNK_CELLS, Painter, fast};
use super::{Cell, FACE, face_z, put};
use crate::canvas::{FLAT, Normal, normal};
use crate::palette::{Ix, Ramp, Tone};

/// The face's rows, top to bottom over its two cells: the cornice, the field, the dado rail, the
/// panelling, the skirting.
const DADO: i32 = 19;
const SKIRT: i32 = 29;

/// Paint wall cell `c` of `room` if it is a face: the back wall's two cells, or a partition's one.
/// False if it is no face (a wall's top), for the caller to draw.
pub(super) fn wall(p: &mut Painter, c: &Cell, room: &Room, south_open: bool, daylight: bool) -> bool {
    let foot = room.face_foot(c.wx);
    let (k, tall) = match foot {
        Some(f) if c.wy == f - 1 => (0, true),
        Some(f) if c.wy == f - 2 => (1, true),
        _ if south_open => (0, false),
        _ => return false,
    };
    let decor = if tall { room.decor_at(c.wx) } else { Decor::None };
    for y in 0..CELL {
        for x in 0..CELL {
            let fy = if tall { (1 - k) * CELL + y } else { CELL + y };
            let (wx, wy) = c.w(x, y);
            let (ix, n) = match hung(decor, room, x, fy, daylight) {
                Some(px) => px,
                None => face(room, wx, wy, fy),
            };
            put(p, c, x, y, ix, n, face_z(y, k));
        }
    }
    // Its ends, where the wall turns away: a dark edge.
    let open = |p: &Painter, dx: i32| p.s.raw[Painter::at(c.cx + dx, c.cy)].flags() & F_SOLID == 0;
    let ends = |p: &Painter, dx: i32| room.face_foot(c.wx + dx).is_none() && !open(p, dx);
    for (dx, x) in [(-1, 0), (1, CELL - 1)] {
        if (tall && ends(p, dx)) || (!tall && open(p, dx)) {
            for y in 0..CELL {
                put(p, c, x, y, Ramp::WoodDark.at(Tone::Deep), FLAT, face_z(y, k));
            }
        }
    }
    true
}

/// The face's own px at world px `(wx, wy)`, `fy` rows down its two cells.
fn face(room: &Room, wx: i32, wy: i32, fy: i32) -> (Ix, Normal) {
    let n = normal(0, FACE);
    let (dado, panel) = match room.kind {
        RoomKind::Julie => (Ramp::WoodOak, Ramp::Limewash),
        RoomKind::Inn => (Ramp::WoodDark, Ramp::WoodDark),
        RoomKind::Church => (Ramp::Stone, Ramp::Stone),
    };
    let ix = match fy {
        // The cornice: a shadow line where the face meets its head, a lit moulding under it.
        0 => Ramp::WoodDark.at(Tone::Deep),
        1 => paper(room, wx, wy).0.at(Tone::Light),
        2 => paper(room, wx, wy).0.at(Tone::Shade),
        // A picture rail in a papered room.
        5 if room.kind != RoomKind::Church => dado.at(Tone::Mid),
        _ if fy < DADO => {
            let (r, t) = paper(room, wx, wy);
            // The paper darkens a little toward the rail, in the light of the room.
            r.at(if fy > DADO - 3 { t.step(-1) } else { t })
        }
        DADO => dado.at(Tone::Light),
        20 => dado.at(Tone::Shade),
        _ if fy < SKIRT => match room.kind {
            // Julie's: painted tongue-and-groove boards.
            RoomKind::Julie => panel.at(if wx.rem_euclid(4) == 0 { Tone::Mid } else { Tone::Base }),
            // The Arms': dark fielded panels, lit on their top and left inside.
            RoomKind::Inn => {
                let (u, v) = (wx.rem_euclid(12), fy - 21);
                let t = if u == 0 || v == 0 || v == 7 {
                    Tone::Shade
                } else if u == 1 || v == 1 {
                    Tone::Lift
                } else if u == 11 || v == 6 {
                    Tone::Mid
                } else {
                    Tone::Base
                };
                panel.at(t)
            }
            // St Anne's: ashlar.
            RoomKind::Church => {
                let course = (fy - 21) / 4;
                let xs = wx + (course & 1) * 6;
                if (fy - 21) % 4 == 3 || xs.rem_euclid(12) == 0 {
                    panel.at(Tone::Mid)
                } else if (fy - 21) % 4 == 0 {
                    panel.at(Tone::Lift)
                } else {
                    panel.at(Tone::Base)
                }
            }
        },
        // The skirting.
        29 => Ramp::WoodDark.at(Tone::Lift),
        30 => Ramp::WoodDark.at(Tone::Base),
        _ => Ramp::WoodDark.at(Tone::Deep),
    };
    (ix, n)
}

/// The paper or the wash on a room's field at world px `(wx, wy)`: its ramp and tone.
fn paper(room: &Room, wx: i32, wy: i32) -> (Ramp, Tone) {
    match room.kind {
        RoomKind::Julie if room.seed & 1 == 0 => {
            // Faded sprigs on cream: a rosebud on a diamond lattice, a leaf by it.
            let (u, v) = ((wx + (wy.div_euclid(6) & 1) * 4).rem_euclid(8), wy.rem_euclid(6));
            match (u, v) {
                (3, 2) => (Ramp::ClothRose, Tone::Base),
                (4, 2) | (3, 1) => (Ramp::ClothRose, Tone::Lift),
                (4, 3) => (Ramp::LeafOlive, Tone::Lift),
                _ => (Ramp::ClothCream, Tone::Lift),
            }
        }
        RoomKind::Julie => {
            // Faded stripes: a broad one and a narrow, sky on cream.
            let u = wx.rem_euclid(10);
            match u {
                0..=2 => (Ramp::ClothSky, Tone::Light),
                5 => (Ramp::ClothSky, Tone::High),
                _ => (Ramp::ClothCream, Tone::Lift),
            }
        }
        RoomKind::Inn => {
            // Oxblood flock: a raised stripe of figure every 6 px.
            let (u, v) = (wx.rem_euclid(6), wy.rem_euclid(4));
            if u == 2 && v != 3 || u == 3 && v == 1 { (Ramp::Oxblood, Tone::Lift) } else { (Ramp::Oxblood, Tone::Base) }
        }
        RoomKind::Church => {
            // Limewash, its brush in long soft runs.
            let run = fast(wx.div_euclid(5) as u32, wy.div_euclid(9) as u32, 0x11e3) % 5;
            (Ramp::Limewash, if run == 0 { Tone::Base } else { Tone::Lift })
        }
    }
}

/// What hangs at `(u, fy)` of a cell of the back wall (`u` its px across, `fy` down the face),
/// if anything does there.
fn hung(d: Decor, room: &Room, u: i32, fy: i32, daylight: bool) -> Option<(Ix, Normal)> {
    let n = normal(0, FACE - 20);
    let ix = match d {
        Decor::None => return None,
        Decor::Window => window(room, u, fy, daylight)?,
        Decor::Picture => {
            // A landscape in a gilt frame on a cord from a nail.
            let (x0, x1, y0, y1) = (2, 13, 7, 15);
            if fy == 5 && u == 7 || fy == 6 && (u == 6 || u == 8) {
                return Some((Ramp::Iron.at(Tone::Shade), n));
            }
            if !(x0..=x1).contains(&u) || !(y0..=y1).contains(&fy) {
                return None;
            }
            if u == x0 || fy == y0 {
                Ramp::Brass.at(Tone::Light)
            } else if u == x1 || fy == y1 {
                Ramp::Brass.at(Tone::Shade)
            } else if u == x0 + 1 || fy == y0 + 1 || u == x1 - 1 || fy == y1 - 1 {
                Ramp::Brass.at(Tone::Base)
            } else {
                let hill = 11 + ((u - 7) * (u - 7)) / 12;
                if fy < 10 {
                    Ramp::Sky.at(if fy == 9 { Tone::High } else { Tone::Light })
                } else if fy >= hill {
                    Ramp::Leaf.at(if fy == hill { Tone::Lift } else { Tone::Base })
                } else {
                    Ramp::LeafOlive.at(Tone::Lift)
                }
            }
        }
        Decor::Clock => clock(room, u, fy)?,
        Decor::Calendar => {
            let (x0, x1, y0, y1) = (5, 10, 7, 15);
            if fy == 6 && u == 7 {
                return Some((Ramp::Brass.at(Tone::Light), n));
            }
            if !(x0..=x1).contains(&u) || !(y0..=y1).contains(&fy) {
                return None;
            }
            if fy <= y0 + 1 {
                Ramp::ClothRed.at(if fy == y0 { Tone::Light } else { Tone::Base })
            } else if u == x1 || fy == y1 {
                Ramp::ClothLinen.at(Tone::Mid)
            } else if (u - x0) % 2 == 1 && (fy - y0) % 2 == 1 && fy > y0 + 2 {
                Ramp::ClothBlack.at(Tone::Lift)
            } else {
                Ramp::ClothLinen.at(Tone::Light)
            }
        }
        Decor::Mirror => {
            // An oval glass in a dark frame, the room's light in it on a diagonal.
            let (cx, cy) = (8, 10);
            let e = (u - cx) * (u - cx) * 36 + (fy - cy) * (fy - cy) * 16;
            if e > 36 * 16 + 40 {
                return None;
            }
            if e > 36 * 16 - 100 {
                Ramp::WoodDark.at(if u < cx { Tone::Lift } else { Tone::Shade })
            } else if (u - cx) + (fy - cy) == -2 || (u - cx) + (fy - cy) == -3 {
                Ramp::Glass.at(Tone::High)
            } else {
                Ramp::Glass.at(if fy < cy { Tone::Lift } else { Tone::Base })
            }
        }
        Decor::PlateRack => {
            // Three plates, blue on white, on a rack.
            if fy == 15 || fy == 6 {
                return Some((Ramp::WoodOak.at(if fy == 6 { Tone::Light } else { Tone::Base }), n));
            }
            if fy == 16 {
                return Some((Ramp::WoodOak.at(Tone::Shade), n));
            }
            let plate = [3, 8, 13].into_iter().find(|&px| (u - px).abs() <= 2 && (fy - 11).abs() <= 3)?;
            let (dx, dy) = (u - plate, fy - 11);
            if dx * dx + dy * dy > 6 {
                return None;
            }
            if dx * dx + dy * dy >= 5 {
                Ramp::ClothBlue.at(Tone::Base)
            } else if dx == 0 && dy == 0 {
                Ramp::ClothBlue.at(Tone::Lift)
            } else {
                Ramp::Limewash.at(if dx + dy < 0 { Tone::High } else { Tone::Light })
            }
        }
        Decor::Dartboard => {
            // The board on its cabinet's back, rings and sectors, the bull.
            let (cx, cy) = (8, 11);
            let (dx, dy) = (u - cx, fy - cy);
            let r2 = dx * dx + dy * dy;
            if r2 > 42 {
                return None;
            }
            let sector = (u32::from(jane_core::angle::iatan2(dy, dx).0) * 20 / 65536) % 2 == 0;
            if r2 > 30 {
                Ramp::ClothBlack.at(Tone::Base)
            } else if r2 > 20 {
                if sector { Ramp::ClothRed.at(Tone::Base) } else { Ramp::ClothGreen.at(Tone::Base) }
            } else if r2 <= 1 {
                Ramp::ClothRed.at(Tone::Light)
            } else if r2 <= 3 {
                Ramp::ClothGreen.at(Tone::Base)
            } else if sector {
                Ramp::ClothCream.at(Tone::Lift)
            } else {
                Ramp::ClothBlack.at(Tone::Lift)
            }
        }
        Decor::Sign => {
            // The inn sign's twin: oxblood board, gilt border, a white castle on it.
            let (x0, x1, y0, y1) = (1, 14, 6, 16);
            if !(x0..=x1).contains(&u) || !(y0..=y1).contains(&fy) {
                return None;
            }
            let castle = (5..=10).contains(&u) && (10..=14).contains(&fy)
                || fy == 9 && (u == 5 || u == 7 || u == 8 || u == 10)
                || (6..=9).contains(&u) && fy == 8 && u % 3 == 0;
            if u == x0 || u == x1 || fy == y0 || fy == y1 {
                Ramp::Brass.at(if u == x0 || fy == y0 { Tone::Light } else { Tone::Shade })
            } else if castle {
                Ramp::Limewash.at(if u < 8 { Tone::Light } else { Tone::Base })
            } else {
                Ramp::Oxblood.at(Tone::Base)
            }
        }
        Decor::HymnBoard => {
            // Dark oak with a pointed head, four hymns' numbers in white on it.
            let (x0, x1, y0, y1) = (3, 12, 4, 18);
            let head = (y0 + 2 - fy).max(0);
            if !(x0 + head..=x1 - head).contains(&u) || !(y0..=y1).contains(&fy) {
                return None;
            }
            if u == x0 + head || fy == y0 {
                Ramp::WoodDark.at(Tone::Light)
            } else if u == x1 - head || fy == y1 {
                Ramp::WoodDark.at(Tone::Deep)
            } else if [7, 10, 13, 16].contains(&fy) && (5..=10).contains(&u) && u != 8 {
                Ramp::Limewash.at(Tone::Light)
            } else {
                Ramp::WoodDark.at(Tone::Shade)
            }
        }
    };
    Some((ix, n))
}

/// A sash window in the back wall: by day the sky over the gardens behind the house, its top
/// panes palest; at night dark glass; a painted frame, a sill, and in Julie's a curtain either
/// side.
fn window(room: &Room, u: i32, fy: i32, daylight: bool) -> Option<Ix> {
    let (x0, x1, y0, y1) = (3, 12, 4, 17);
    if room.kind == RoomKind::Julie && ((1..=2).contains(&u) || (13..=14).contains(&u)) && (3..=19).contains(&fy) {
        // The curtains, tied back, their folds lit on the left.
        let fold = (u + fy / 3) % 2 == 0;
        return Some(Ramp::ClothRose.at(if fold { Tone::Lift } else { Tone::Mid }));
    }
    if fy == y1 + 1 && (x0 - 1..=x1 + 1).contains(&u) {
        return Some(Ramp::Stone.at(Tone::High));
    }
    if !(x0..=x1).contains(&u) || !(y0..=y1).contains(&fy) {
        return None;
    }
    let mid = (y0 + y1) / 2;
    let frame = u == x0 || u == x1 || fy == y0 || fy == y1 || fy == mid || u == (x0 + x1) / 2 + 1;
    if frame {
        return Some(Ramp::Limewash.at(if u == x0 || fy == y0 { Tone::Light } else { Tone::Base }));
    }
    Some(if daylight {
        // The sky, palest at its top, a hedge's green along the bottom panes.
        if fy > y1 - 3 {
            Ramp::Leaf.at(if fy == y1 - 2 { Tone::Lift } else { Tone::Base })
        } else {
            Ramp::Sky.at(if fy < y0 + 4 { Tone::High } else { Tone::Light })
        }
    } else if (u + fy) % 7 == 0 {
        Ramp::Glass.at(Tone::Mid)
    } else {
        Ramp::Glass.at(Tone::Deep)
    })
}

/// A wall clock: a case, its dial, its pendulum's window. Julie's stopped at five to nine, its
/// pendulum hanging still, a cobweb from its corner.
fn clock(room: &Room, u: i32, fy: i32) -> Option<Ix> {
    let (x0, x1, y0, y1) = (3, 12, 3, 20);
    let julie = room.kind == RoomKind::Julie;
    // The cobweb: threads from the case's top corner up the wall and across.
    let thread = u - x1 == 8 - fy || u - x1 == (8 - fy) / 2 || fy == 3;
    if julie && u > x1 && u < 15 && fy < 8 && thread {
        return Some(Ramp::HairWhite.at(Tone::Base));
    }
    if !(x0..=x1).contains(&u) || !(y0..=y1).contains(&fy) {
        return None;
    }
    let (cx, cy) = (8, 8);
    let (dx, dy) = (u - cx, fy - cy);
    let r2 = dx * dx + dy * dy;
    if r2 <= 16 {
        // The dial in its brass bezel: the hour hand toward nine, the minute hand toward
        // eleven, a dot at each quarter.
        let hour = dy == 0 && (-3..=0).contains(&dx);
        let minute = (dx == -1 && (-3..=-1).contains(&dy)) || (dx == -2 && dy == -3);
        let quarter = r2 == 9 && (dx == 0 || dy == 0);
        return Some(if hour || minute {
            Ramp::ClothBlack.at(Tone::Base)
        } else if r2 >= 13 {
            Ramp::Brass.at(if dx + dy < 0 { Tone::Light } else { Tone::Shade })
        } else if quarter {
            Ramp::ClothBlack.at(Tone::Lift)
        } else {
            Ramp::Limewash.at(if dx + dy < -2 { Tone::High } else { Tone::Light })
        });
    }
    // The pendulum's window under the dial: the rod straight down, its brass bob; Julie's
    // dusty, the bob still.
    if (13..=18).contains(&fy) && (6..=10).contains(&u) {
        return Some(if u == 8 && fy < 17 {
            Ramp::Brass.at(Tone::Shade)
        } else if (7..=9).contains(&u) && fy >= 16 {
            Ramp::Brass.at(if u == 7 { Tone::Light } else { Tone::Base })
        } else if julie && (u + fy) % 3 == 0 {
            Ramp::Glass.at(Tone::Mid)
        } else {
            Ramp::Glass.at(Tone::Shade)
        });
    }
    let t = if u == x0 || fy == y0 {
        Tone::Light
    } else if u == x1 || fy == y1 {
        Tone::Shade
    } else {
        Tone::Base
    };
    Some(Ramp::WoodDark.at(t))
}

/// By day, each window of the back wall lays its light on the floor under it: a parallelogram
/// from the wall's foot, sheared east as it comes south (the sun in the top-left), its glazing
/// bars' shadows across it, a step brighter at its heart. The floor keeps its own ramp.
pub(super) fn daylight(p: &mut Painter, room: &Room, x0: i32, y0: i32) {
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let k = Painter::at(cx, cy);
            if p.s.raw[k].flags() & F_SOLID != 0 {
                continue;
            }
            let (wx, wy) = (x0 + cx, y0 + cy);
            for xw in wx - 4..=wx + 1 {
                if room.decor_at(xw) != Decor::Window {
                    continue;
                }
                let Some(f) = room.face_foot(xw) else { continue };
                // The window's light falls from its glass (14 px of it) onto the floor a few px
                // out from the wall's foot: a patch as wide as the glass and twice as deep, its
                // sides sheared a px east for every four rows south.
                for y in 0..CELL {
                    let dy = wy * CELL + y - f * CELL - 3;
                    if !(0..30).contains(&dy) {
                        continue;
                    }
                    let left = xw * CELL + 3 + dy / 4;
                    let right = left + 11;
                    let bar = left + 5;
                    for x in 0..CELL {
                        let px = wx * CELL + x;
                        if px < left || px >= right || px == bar || dy == 14 {
                            continue;
                        }
                        let rim = px == left || px == right - 1 || dy == 0 || dy == 29;
                        p.s.ly.step(cx * CELL + x, cy * CELL + y, if rim { 1 } else { 2 });
                    }
                }
            }
        }
    }
}
