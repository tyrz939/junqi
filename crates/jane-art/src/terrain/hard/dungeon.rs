//! The dungeon-dressing kit (ART-PLAN M5 and B2, ART.md §2.6.1): a generated dungeon framed and
//! paid off over what the hard painters laid, every choice from its theme (a data row) and its
//! room graph, none from its name.
//!
//! - **Faces two cells tall.** The solid cell over a wall's face (a wall or the void, solid either
//!   way, so this is drawing only) takes the face's upper half under the theme's head trim: a
//!   timber cap, dentils, a gilt frieze, a cornice, an iron beam, a string course or a rail.
//! - **Doorways framed.** A door in a north wall stands between two pilasters under a lintel,
//!   carved as the theme says (an angel, a lamp, fluting); a boss's door is two cells a side,
//!   fluted, under a keystone. Doors in the other walls have their posts capped.
//! - **Floors bordered.** An inset band a cell in from every wall of a room, corners turned, in
//!   the theme's border; corridors have none.
//! - **Worn lanes** from door to door, read off the room graph (`dungeon::Dungeon`).
//! - **The theme's motifs in every room**, on its faces (lamps on hooks, candle niches, ivy,
//!   paintings, signs, valves, high windows, radiators, coat pegs) and its floors (rails along the
//!   lanes, a channel with grates, a belt, a gantry's shadow, a carpet, moonlight from the high
//!   windows, bones heaped, plinths, stanchions, chalk).
//! - **The boss's room**: a floor laid concentric to its middle in the theme's inlay, four
//!   braziers, a dais, and the theme's emblem over it; **the set room** (the hub): a medallion and
//!   two braziers, lit as a stage.
//!
//! Every px is a pure function of its world px, the tiles about it and the dungeon's rooms, so a
//! chunk's seams never show; anything a cell draws stays inside its own cell.

use jane_core::Tile;
use jane_core::angle::iatan2;
use jane_data::{
    FloorMotifKind as F, ThemeBorder, ThemeCarving, ThemeEmblem, ThemeTrim, ThemeUpper, TilePattern as P,
    WallMotifKind as W,
};

use super::super::dungeon::{
    DRoom, Door, Dungeon, FRAMED_BORDER, FRAMED_MOTIF, FRAMED_TALL, LANE_HALF, Role, Side, Theme,
};
use super::super::{CELL, CHUNK_CELLS, Layers, Painter, Style, fast};
use super::{Cell, FACE, face_z, nst, put};
use crate::canvas::{FLAT, Normal, normal};
use crate::palette::{Ix, Ramp, Tone};

/// A painted px: its colour, normal (flat keeps what is under it on a floor), height over its
/// ground and whether it shines.
#[derive(Clone, Copy)]
struct Px {
    ix: Ix,
    n: Normal,
    dz: i32,
    glow: bool,
}

const fn px(ix: Ix) -> Px {
    Px { ix, n: FLAT, dz: 0, glow: false }
}

const fn lit(ix: Ix) -> Px {
    Px { ix, n: FLAT, dz: 0, glow: true }
}

const fn raised(ix: Ix, dz: i32) -> Px {
    Px { ix, n: FLAT, dz, glow: false }
}

/// A tone step on what is there already, not a colour: `dz` steps.
const fn shift(steps: i32) -> Px {
    Px { ix: Ix::CLEAR, n: FLAT, dz: steps, glow: false }
}

/// The start of a chunk in a dungeon: its framing marks cleared.
pub(super) fn begin(p: &mut Painter, dg: &Dungeon) {
    p.s.framed.clear();
    p.s.framed.resize(dg.rooms.len(), 0);
}

fn mark(p: &mut Painter, room: Option<u8>, bit: u8) {
    if let Some(f) = room.and_then(|r| p.s.framed.get_mut(usize::from(r))) {
        *f |= bit;
    }
}

/// Whether chunk-local cell `(cx, cy)` is a wall's face: a wall (not the void) with open floor
/// to its south.
fn is_face(p: &Painter, cx: i32, cy: i32) -> bool {
    let st = p.style_k(Painter::at(cx, cy));
    st.row.wall_like && st.tile != Tile::Void && !p.style_k(Painter::at(cx, cy + 1)).row.wall_like
}

/// Whether chunk-local cell `(cx, cy)` is the upper half of a face: solid over a face.
fn is_upper(p: &Painter, cx: i32, cy: i32) -> bool {
    p.style_k(Painter::at(cx, cy)).row.wall_like && is_face(p, cx, cy + 1)
}

/// Paint cell `c` as the upper half of the face under it, if it is one.
pub(super) fn upper(p: &mut Painter, c: &Cell, dg: &Dungeon) -> bool {
    if !is_upper(p, c.cx, c.cy) {
        return false;
    }
    let below = nst(p, c, 0, 1);
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (ix, n) = upper_px(&dg.theme, &below, wx, wy, y);
            put(p, c, x, y, ix, n, face_z(y, 1));
            if y <= 2 {
                if let Some(e) = catch(ix) {
                    p.s.ly.glow(c.px + x, c.py + y, e);
                }
            }
        }
    }
    // Its ends, where the face turns away: the material's darkest edge.
    for (dx, x) in [(-1, 0), (1, CELL - 1)] {
        if !is_upper(p, c.cx + dx, c.cy) {
            for y in 1..CELL {
                put(p, c, x, y, below.ramp.at(Tone::Deep), FLAT, face_z(y, 1));
            }
        }
    }
    mark(p, dg.room_id(c.wx, c.wy + 2), FRAMED_TALL);
    true
}

/// What a lit edge of the framing (the head trim's top, an emblem's lit side) catches of the
/// room's lamps: a faint glow three tones under its own, so the framing reads at 1x in a dark
/// dungeon (it is lit by what is near; this keeps its line where nothing is). `None` for a tone
/// under `Lift`, and for anything off a ramp.
fn catch(ix: Ix) -> Option<Ix> {
    let (r, t) = Ramp::of(ix)?;
    (t >= Tone::Lift).then(|| r.at(t.step(-3)))
}

/// The upper half's px: the head trim over rows 1 to 4 under a lit top edge, then the walling
/// carried up (or the shelves).
fn upper_px(th: &Theme, below: &Style, wx: i32, wy: i32, y: i32) -> (Ix, Normal) {
    let (r, acc) = (below.ramp, below.accent);
    if y == 0 {
        return (th.trim_ramp.unwrap_or(r).at(Tone::Light), normal(0, -60));
    }
    if y <= 4 {
        return trim(th, below, wx, y);
    }
    if th.row.upper == ThemeUpper::Shelves {
        return shelves(wx, y);
    }
    let nf = normal(0, FACE);
    match below.row.pattern {
        P::Timbered => {
            let wood = acc.unwrap_or(Ramp::WoodDark);
            let set = wx.rem_euclid(3 * CELL);
            if set < 5 {
                let t = [Tone::Light, Tone::Lift, Tone::Base, Tone::Base, Tone::Shade][set as usize];
                (wood.at(t), normal([-60, -30, 0, 20, 60][set as usize], FACE))
            } else {
                match fast(wx.div_euclid(2) as u32, wy.div_euclid(2) as u32, 0x0e4) % 197 {
                    0 => (Ramp::Brass.at(Tone::High), normal(-30, FACE - 30)),
                    1 => (Ramp::Brass.at(Tone::Base), nf),
                    _ => super::rock_face(r, wx, wy),
                }
            }
        }
        P::Rock => super::rock_face(r, wx, wy),
        P::Ironwork => {
            // The brick carried up, sooted darker toward the head.
            let (ix, n) = brick(acc.unwrap_or(Ramp::Brick), r, wx, wy);
            match Ramp::of(ix) {
                Some((rr, t)) if y < 9 => (rr.at(t.step(-1)), n),
                _ => (ix, n),
            }
        }
        P::Panelled => {
            let t = if y == 5 {
                Tone::Mid
            } else if wx.rem_euclid(8) == 0 {
                Tone::Lift
            } else {
                Tone::Base
            };
            (r.at(t), nf)
        }
        P::Pipework => {
            // A smaller main than the one below, flanged.
            let flange = wx.rem_euclid(24) < 2;
            match y {
                8 if flange => (Ramp::Iron.at(Tone::Light), nf),
                9 => (Ramp::Iron.at(if flange { Tone::High } else { Tone::Light }), normal(0, FACE - 50)),
                10 => (Ramp::Iron.at(if flange { Tone::Light } else { Tone::Base }), nf),
                11 => (Ramp::Iron.at(Tone::Shade), normal(0, FACE + 30)),
                12 if flange => (Ramp::Iron.at(Tone::Shade), nf),
                _ => walling(r, wx, wy),
            }
        }
        P::Wainscot => (r.at(if y == 5 { Tone::Mid } else { Tone::Base }), nf),
        _ => ashlar(r, wx, wy),
    }
}

/// The head trim, rows 1 to 4.
fn trim(th: &Theme, below: &Style, wx: i32, y: i32) -> (Ix, Normal) {
    let r = th.trim_ramp.unwrap_or(below.ramp);
    let n = [normal(0, FACE - 50), normal(0, FACE), normal(0, FACE), normal(0, FACE + 30)][(y - 1) as usize];
    let four = |a: Tone, b: Tone, c: Tone, d: Tone| (r.at([a, b, c, d][(y - 1) as usize]), n);
    match th.row.trim {
        ThemeTrim::Plain => four(Tone::Light, Tone::Base, Tone::Shade, Tone::Deep),
        ThemeTrim::Cap => {
            // A squared timber, the heads of the posts under it lit where the walling has posts.
            let post = below.row.pattern == P::Timbered && wx.rem_euclid(3 * CELL) < 5;
            let t = [Tone::Light, Tone::Base, Tone::Shade, Tone::Deep][(y - 1) as usize];
            (r.at(if post && y < 4 { t.step(1) } else { t }), n)
        }
        ThemeTrim::Dentil => match y {
            2 => (r.at(if wx.rem_euclid(4) < 2 { Tone::Light } else { Tone::Shade }), n),
            _ => four(Tone::Lift, Tone::Base, Tone::Mid, Tone::Deep),
        },
        ThemeTrim::Frieze => match y {
            2 => (r.at(if wx.rem_euclid(3) == 0 { Tone::Shade } else { Tone::Lift }), n),
            4 => (below.accent.unwrap_or(Ramp::WoodDark).at(Tone::Deep), n),
            _ => four(Tone::Light, Tone::Base, Tone::Mid, Tone::Deep),
        },
        ThemeTrim::Cornice => match y {
            2 => (r.at(if wx.rem_euclid(3) == 0 { Tone::Deep } else { Tone::Base }), n),
            _ => four(Tone::Light, Tone::Base, Tone::Shade, Tone::Deep),
        },
        ThemeTrim::Beam => match y {
            2 if wx.rem_euclid(6) == 3 => (r.at(Tone::High), normal(-30, FACE - 30)),
            _ => four(Tone::Light, Tone::Base, Tone::Shade, Tone::Deep),
        },
        ThemeTrim::Course => {
            if wx.rem_euclid(2 * CELL) < 3 && y >= 3 {
                (r.at(Tone::Lift), n)
            } else {
                four(Tone::Light, Tone::Base, Tone::Shade, Tone::Deep)
            }
        }
        ThemeTrim::Rail => match y {
            3 => (below.accent.unwrap_or(Ramp::WoodOak).at(Tone::Base), n),
            _ => four(Tone::Light, Tone::Lift, Tone::Base, Tone::Shade),
        },
    }
}

/// Ashlar in deep courses, long blocks staggered, each its own tone.
fn ashlar(r: Ramp, wx: i32, wy: i32) -> (Ix, Normal) {
    let course = wy.div_euclid(8);
    let (yy, xs) = (wy.rem_euclid(8), wx + (course & 1) * 12);
    let lx = xs.rem_euclid(24);
    if yy == 7 || lx == 23 {
        return (r.at(Tone::Deep), normal(0, FACE + 10));
    }
    let t = match fast(xs.div_euclid(24) as u32, course as u32, 0xc4b8) % 7 {
        0 | 1 => Tone::Mid,
        6 => Tone::Lift,
        _ => Tone::Base,
    };
    let t = match yy {
        0 => t.step(1),
        6 => t.step(-1),
        _ => t,
    };
    (r.at(t), normal(if lx == 0 { -30 } else { 0 }, FACE))
}

/// Brick in stretcher bond.
fn brick(brick: Ramp, mortar: Ramp, wx: i32, wy: i32) -> (Ix, Normal) {
    let course = wy.div_euclid(4);
    let (yy, lx) = (wy.rem_euclid(4), (wx + (course & 1) * 4).rem_euclid(8));
    if yy == 3 || lx == 7 {
        return (mortar.at(Tone::Shade), normal(0, FACE + 10));
    }
    let t = match fast((wx + (course & 1) * 4).div_euclid(8) as u32, course as u32, 0x1e0) % 6 {
        0 => Tone::Shade,
        1 | 2 => Tone::Base,
        _ => Tone::Mid,
    };
    (brick.at(if yy == 0 { t.step(1) } else { t }), normal(0, FACE))
}

/// Coursed walling.
fn walling(r: Ramp, wx: i32, wy: i32) -> (Ix, Normal) {
    let course = wy.div_euclid(5);
    let (yy, bx) = (wy.rem_euclid(5), (wx + (course & 1) * 5).rem_euclid(10));
    let t = if yy == 4 || bx == 9 {
        Tone::Deep
    } else {
        match fast((wx + (course & 1) * 5).div_euclid(10) as u32, course as u32, 0x919e) % 5 {
            0 => Tone::Mid,
            4 => Tone::Lift,
            _ => Tone::Base,
        }
    };
    (r.at(t), normal(0, FACE))
}

/// Book spines' cloths.
const SPINES: [Ramp; 9] = [
    Ramp::ClothRed,
    Ramp::ClothNavy,
    Ramp::RacingGreen,
    Ramp::Oxblood,
    Ramp::ClothMustard,
    Ramp::Leather,
    Ramp::ClothBrown,
    Ramp::ClothTeal,
    Ramp::ClothCream,
];

/// A wall as its shelves, `fy` rows down the face's two cells: an upright every cell and a half,
/// boards every nine rows, books of every height and cloth between, a gap now and then.
fn shelves(wx: i32, fy: i32) -> (Ix, Normal) {
    let wood = Ramp::WoodDark;
    let nf = normal(0, FACE);
    match wx.rem_euclid(24) {
        0 => return (wood.at(Tone::Light), normal(-40, FACE)),
        1 => return (wood.at(Tone::Shade), normal(30, FACE)),
        _ => {}
    }
    match fy {
        5 => return (wood.at(Tone::Deep), nf),
        13 | 22 => return (wood.at(Tone::Light), normal(0, FACE - 50)),
        14 | 23 | 30 | 31 => return (wood.at(Tone::Shade), nf),
        29 => return (wood.at(Tone::Lift), normal(0, FACE - 40)),
        _ => {}
    }
    let (shelf, top) = match fy {
        6..=12 => (0, 6),
        15..=21 => (1, 15),
        24..=28 => (2, 24),
        _ => return (wood.at(Tone::Deep), nf),
    };
    // A book is two px wide, its own cloth and height.
    let h = fast(wx.div_euclid(2) as u32, shelf as u32, 0xb00c);
    let rise = 1 + (h >> 8) as i32 % 3;
    if h % 13 == 0 || fy < top + rise - 1 {
        return (wood.at(Tone::Deep), nf);
    }
    if fy == top + rise + 1 && (h >> 16) % 3 == 0 {
        return (Ramp::Brass.at(Tone::Light), nf);
    }
    let cloth = SPINES[(h >> 12) as usize % SPINES.len()];
    let left = wx.rem_euclid(2) == 0;
    (cloth.at(if left { Tone::Lift } else { Tone::Shade }), normal(if left { -30 } else { 30 }, FACE))
}

// --- the dress pass ------------------------------------------------------------------------

/// The dress pass over a painted chunk of a dungeon.
pub(super) fn dress(p: &mut Painter, dg: &Dungeon, x0: i32, y0: i32, seed: u32) {
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let (wx, wy) = (x0 + cx, y0 + cy);
            if is_upper(p, cx, cy) {
                face(p, dg, cx, cy, wx, wy, 0);
            } else if is_face(p, cx, cy) {
                face(p, dg, cx, cy, wx, wy, CELL);
            } else if p.style_k(Painter::at(cx, cy)).row.wall_like {
                post(p, dg, cx, cy, wx, wy);
            } else if let Some(i) = dg.room_id(wx, wy) {
                floor(p, dg, usize::from(i), cx, cy, wx, wy, seed);
            } else {
                lintel(p, dg, cx, cy, wx, wy);
            }
        }
    }
}

/// A face cell's dress: `top` is 0 for the upper half, a cell for the lower. Its room is the one
/// its foot opens onto.
fn face(p: &mut Painter, dg: &Dungeon, cx: i32, cy: i32, wx: i32, wy: i32, top: i32) {
    let th = &dg.theme;
    let foot = if top == 0 { wy + 2 } else { wy + 1 };
    let room_id = dg.room_id(wx, foot);
    let tall = top == 0 || is_upper(p, cx, cy - 1);
    let k = i32::from(top == 0);
    let below = *p.style_k(Painter::at(cx, cy + k));
    let room = room_id.map(|i| &dg.rooms[usize::from(i)]);
    let jamb = dg.jamb(wx, foot - 1, 1).filter(|(_, d, _)| d.side == Side::N);
    let (bx, by) = (cx * CELL, cy * CELL);
    let mut motif = false;
    for y in 0..CELL {
        for x in 0..CELL {
            let fy = top + y;
            let wpx = wx * CELL + x;
            let got = if let Some((_, d, along)) = jamb { jamb_px(th, &d, along, x, fy, tall) } else { None };
            let mut shown = false;
            let got = got.or_else(|| {
                let e = room.filter(|r| r.role == Role::Boss).and_then(|r| emblem(th, r, wpx, fy, tall));
                shown = e.is_some();
                e
            });
            let got = got.or_else(|| {
                let d = deco(th, wpx, wy * CELL + y, fy, tall);
                motif |= d.is_some();
                d
            });
            let got = got.or_else(|| {
                (th.row.upper == ThemeUpper::Shelves && top == CELL).then(|| {
                    let (ix, n) = shelves(wpx, fy);
                    Px { ix, n, dz: 0, glow: false }
                })
            });
            if let Some(g) = got {
                let (lx, ly) = (bx + x, by + y);
                let n = if g.n == FLAT { normal(0, FACE) } else { g.n };
                p.s.ly.put(lx, ly, g.ix, n, face_z(y, k) + g.dz);
                if g.glow {
                    p.s.ly.glow(lx, ly, g.ix);
                } else if let Some(e) = catch(g.ix).filter(|_| shown) {
                    p.s.ly.glow(lx, ly, e);
                }
            }
        }
    }
    // Shelves, and a walling with posts, are a motif wherever the faces are.
    let everywhere = th.row.upper == ThemeUpper::Shelves
        || (th.row.trim == ThemeTrim::Cap && below.row.pattern == P::Timbered && tall);
    if motif || everywhere {
        mark(p, room_id, FRAMED_MOTIF);
    }
}

/// A theme's motif hung on a face at world px `wpx` (`wpy`), `fy` rows down its two cells.
fn deco(th: &Theme, wpx: i32, wpy: i32, fy: i32, tall: bool) -> Option<Px> {
    th.row.wall.iter().find_map(|m| {
        let period = i32::from(m.every.max(1)) * CELL;
        let u = (wpx - i32::from(m.at)).rem_euclid(period);
        hung(m.kind, u, period, wpx, wpy, fy, tall)
    })
}

/// One wall motif's px, `u` px into its period from where it starts.
fn hung(kind: W, u: i32, period: i32, wpx: i32, wpy: i32, fy: i32, tall: bool) -> Option<Px> {
    let nf = normal(0, FACE);
    match kind {
        W::Lamp => {
            // A lamp on a hook from a post, lit.
            if !tall {
                return None;
            }
            let (iron, brass) = (Ramp::Iron, Ramp::Brass);
            match (u, fy) {
                (0..=3, 6) => Some(Px { ix: iron.at(Tone::Light), n: normal(0, FACE - 40), dz: 1, glow: false }),
                (3, 7 | 8) => Some(raised(iron.at(Tone::Shade), 1)),
                (2..=4, 9) => Some(raised(brass.at(Tone::Light), 1)),
                (2 | 4, 10 | 11) => Some(raised(brass.at(if u == 2 { Tone::Base } else { Tone::Shade }), 1)),
                (3, 10) => Some(lit(Ramp::Ember.at(Tone::Glint))),
                (3, 11) => Some(lit(Ramp::Ember.at(Tone::High))),
                (2..=4, 12) => Some(raised(brass.at(Tone::Shade), 1)),
                _ => None,
            }
        }
        W::Niche => {
            // A candle in an arched niche, its flame lit.
            if !tall || !(0..=7).contains(&u) || !(5..=13).contains(&fy) || (fy == 5 && !(2..=5).contains(&u)) {
                return None;
            }
            let stone = Ramp::Stone;
            Some(match (u, fy) {
                (0 | 7, _) | (_, 5) => px(stone.at(Tone::Deep)),
                (_, 13) => px(stone.at(Tone::Light)),
                (3, 8) => lit(Ramp::Ember.at(Tone::Glint)),
                (3, 9) => lit(Ramp::Ember.at(Tone::High)),
                (2 | 4, 9) => lit(Ramp::Ember.at(Tone::Light)),
                (3 | 4, 10..=12) => px(Ramp::Bone.at(if u == 3 { Tone::Light } else { Tone::Base })),
                (1, 6..=12) => px(stone.at(Tone::Shade)),
                _ => px(Ix::SEAM),
            })
        }
        W::Ivy => {
            // Ivy hung from the head down the face, in some periods, in two-px clumps.
            let b = (wpx - u).div_euclid(period);
            let hb = fast(b as u32, wpy.div_euclid(2 * CELL) as u32, 0x1e7);
            if hb % 3 != 0 {
                return None;
            }
            let (from, w) = (4 + (hb >> 4) as i32 % 10, 10 + (hb >> 8) as i32 % 12);
            if u < from || u >= from + w {
                return None;
            }
            let len = 6 + fast(b as u32, ((u - from) / 2) as u32, 0x1e8) as i32 % 22;
            let start = if tall { 4 } else { CELL };
            if fy < start || fy > start + len {
                return None;
            }
            let clump = fast(wpx.div_euclid(2) as u32, fy.div_euclid(2) as u32, 0x1e9);
            if clump % 3 == 0 && fy > start + 2 {
                return None;
            }
            let t = match clump % 5 {
                0 => Tone::Light,
                1 | 2 => Tone::Base,
                _ => Tone::Shade,
            };
            let leaf = if fy > start + len - 3 { Ramp::LeafDeep } else { Ramp::Leaf };
            Some(Px { ix: leaf.at(t), n: normal(-20, FACE - 20), dz: 1, glow: false })
        }
        W::Painting => {
            // A painting in a gilt frame, a size and a subject of its own: a portrait or a view.
            if !tall {
                return None;
            }
            let h = fast((wpx - u).div_euclid(period) as u32, 0, 0x7a1);
            let w = (16 + (h % 4) as i32 * 4).min(period - 4);
            let hh = 10 + (h >> 4) as i32 % 5;
            let (u, v) = (u - (period - w) / 2, fy - 7);
            if u < 0 || u >= w || v < 0 || v >= hh {
                return None;
            }
            let gold = Ramp::Brass;
            let edge = u == 0 || v == 0 || u == w - 1 || v == hh - 1;
            let inner = u == 1 || v == 1 || u == w - 2 || v == hh - 2;
            let ix = if edge {
                gold.at(if u == 0 || v == 0 { Tone::Light } else { Tone::Shade })
            } else if inner {
                gold.at(if u == 1 || v == 1 { Tone::Shade } else { Tone::Base })
            } else if (h >> 8) % 3 == 0 {
                let (fx, fy2) = (u - w / 2, v - hh / 2 + 1);
                if fx * fx * 4 + fy2 * fy2 * 3 < 18 {
                    Ramp::SkinPale.at(if fx < 0 { Tone::Light } else { Tone::Base })
                } else if fy2 > 2 {
                    Ramp::ClothBlack.at(Tone::Base)
                } else {
                    Ramp::WoodDark.at(Tone::Deep)
                }
            } else {
                let hill = hh / 2 + ((u * 3 + (h >> 12) as i32) % 7 - 3) / 3;
                match v.cmp(&hill) {
                    std::cmp::Ordering::Less => Ramp::ClothSky.at(if v < 3 { Tone::Base } else { Tone::Lift }),
                    std::cmp::Ordering::Equal => Ramp::LeafOlive.at(Tone::Light),
                    std::cmp::Ordering::Greater => {
                        (if (u + v) % 6 < 3 { Ramp::LeafOlive } else { Ramp::Earth }).at(Tone::Base)
                    }
                }
            };
            Some(Px { ix, n: nf, dz: 1, glow: false })
        }
        W::Sign => {
            // A warning sign: a yellow triangle, its black edge and its stroke.
            let v = fy - 18;
            if !(0..12).contains(&u) || !(0..10).contains(&v) {
                return None;
            }
            let half = v * 6 / 9;
            let d = (2 * u - 11).abs();
            if d > 2 * half + 1 {
                return None;
            }
            let ix = if d >= 2 * half - 1 || v == 9 {
                Ramp::ClothBlack.at(Tone::Base)
            } else if (u == 5 || u == 6) && ((3..=6).contains(&v) || v == 8) {
                Ramp::ClothBlack.at(Tone::Deep)
            } else {
                Ramp::ClothMustard.at(if v < 4 { Tone::Light } else { Tone::Base })
            };
            Some(Px { ix, n: nf, dz: 1, glow: false })
        }
        W::Valve => {
            // A valve wheel on the small main.
            if !tall {
                return None;
            }
            let (dx, dy) = (u - 4, fy - 10);
            let d2 = dx * dx + dy * dy;
            if d2 > 20 {
                return None;
            }
            let red = Ramp::ClothRed;
            let ix = if d2 <= 1 {
                Ramp::Iron.at(Tone::Light)
            } else if d2 >= 10 {
                red.at(if dy < 0 {
                    Tone::Light
                } else if dy > 1 {
                    Tone::Shade
                } else {
                    Tone::Base
                })
            } else if dx == 0 || dy == 0 {
                red.at(Tone::Shade)
            } else {
                return None;
            };
            Some(Px { ix, n: normal(-dx * 10, FACE - dy * 10), dz: 1, glow: false })
        }
        W::Window => {
            // A high window, moonlit: a painted frame, a mullion and a transom, the panes lit.
            if !tall || !(0..16).contains(&u) || !(3..=14).contains(&fy) {
                return None;
            }
            let frame = Ramp::Limewash;
            if u == 0 || u == 15 || fy == 3 || fy == 14 {
                let t = if u == 0 || fy == 3 { Tone::Light } else { Tone::Base };
                return Some(Px { ix: frame.at(t), n: nf, dz: 1, glow: false });
            }
            if u == 7 || u == 8 || fy == 8 {
                return Some(Px { ix: frame.at(Tone::Lift), n: nf, dz: 1, glow: false });
            }
            let t = if (u + fy) % 7 == 0 {
                Tone::High
            } else if fy < 8 {
                Tone::Light
            } else {
                Tone::Lift
            };
            Some(lit(Ramp::ClothSky.at(t)))
        }
        W::Radiator => {
            // A cast-iron radiator, its fins lit on their left.
            if !(0..14).contains(&u) || !(21..=28).contains(&fy) {
                return None;
            }
            let t = if fy == 21 {
                Tone::Light
            } else if fy == 28 {
                if u == 1 || u == 12 { Tone::Shade } else { return None }
            } else if u % 2 == 0 {
                Tone::Lift
            } else {
                Tone::Shade
            };
            Some(Px { ix: Ramp::Iron.at(t), n: normal(if u % 2 == 0 { -40 } else { 40 }, FACE), dz: 1, glow: false })
        }
        W::Pegs => {
            // Coat pegs over the dado, a coat on some.
            if !(0..14).contains(&u) {
                return None;
            }
            let peg = u % 8 == 3;
            match fy {
                22 if peg => Some(px(Ramp::Brass.at(Tone::Light))),
                23 if peg => Some(px(Ramp::Brass.at(Tone::Shade))),
                24..=29 => {
                    let pg = (u - 3).div_euclid(8) * 8 + 3;
                    let h = fast((wpx - u + pg) as u32, 0, 0xc0a7);
                    let half = 1 + (fy - 23) / 3;
                    let d = u - pg;
                    if h % 3 != 0 || d.abs() > half {
                        return None;
                    }
                    let cloth =
                        [Ramp::ClothNavy, Ramp::ClothGrey, Ramp::ClothRed, Ramp::ClothBrown][(h >> 4) as usize % 4];
                    let t = match d.signum() {
                        -1 => Tone::Lift,
                        1 => Tone::Shade,
                        _ => Tone::Base,
                    };
                    Some(Px { ix: cloth.at(t), n: normal(d * 20, FACE), dz: 2, glow: false })
                }
                _ => None,
            }
        }
    }
}

/// A door's frame on a north face: a pilaster each side of the opening (two cells of them for a
/// boss's door, fluted), its capital under the head and its base at the foot, carved as the
/// theme says.
fn jamb_px(th: &Theme, d: &Door, along: i32, x: i32, fy: i32, tall: bool) -> Option<Px> {
    let stone = th.frame;
    let top = if tall { 5 } else { CELL + 1 };
    if fy < top {
        return None;
    }
    // How far the px is from the opening, and so where it is across the pilaster from its left.
    let width = if d.boss { 11 + CELL } else { 11 };
    let dist = if along < 0 { (CELL - x) + (-1 - along) * CELL } else { (x + 1) + (along - 1) * CELL };
    if dist > width {
        return None;
    }
    let u = if along < 0 { width - dist } else { dist - 1 };
    let (inner, outer) = (u == 0, u == width - 1);
    let (inner, outer) = if along < 0 { (outer, inner) } else { (inner, outer) };
    let cap = fy < top + 3;
    let base = fy >= 2 * CELL - 3;
    let t = if cap || base {
        if fy == top || fy == 2 * CELL - 3 {
            Tone::Light
        } else if fy == top + 2 || fy == 2 * CELL - 1 {
            Tone::Shade
        } else {
            Tone::Base
        }
    } else if outer || inner {
        Tone::Deep
    } else if u == 1 {
        Tone::Light
    } else if u == width - 2 {
        Tone::Shade
    } else {
        Tone::Base
    };
    let mut ix = stone.at(t);
    let (cu, cv) = (u - width / 2, fy - top - 4);
    if !(cap || base) {
        match th.row.carving {
            ThemeCarving::Angel if tall && !d.boss => {
                if let Some(a) = angel(cu, cv, 1) {
                    ix = a;
                }
            }
            ThemeCarving::Lamp if tall && cu.abs() <= 1 && (3..=6).contains(&cv) => {
                return Some(lit(Ramp::Ember.at(if cv < 5 { Tone::Glint } else { Tone::High })));
            }
            _ => {}
        }
        let fluted = d.boss || th.row.carving == ThemeCarving::Fluted;
        if fluted && u > 1 && u < width - 2 && (u - 2).rem_euclid(3) == 0 && ix == stone.at(Tone::Base) {
            ix = stone.at(Tone::Shade);
        }
        if d.boss && th.row.carving == ThemeCarving::Angel && tall {
            if let Some(a) = angel(cu, cv, 1) {
                ix = a;
            }
        }
    }
    let n = normal(
        if u == 0 {
            -50
        } else if u == width - 1 {
            50
        } else {
            0
        },
        FACE,
    );
    Some(Px { ix, n, dz: 1, glow: false })
}

/// A carved angel, `(u, v)` from the top of her head's middle, `s` her scale: a halo, a head,
/// wings raised from her shoulders and a robe to her feet. In bone, lit from the upper left.
fn angel(u: i32, v: i32, s: i32) -> Option<Ix> {
    let s = s.max(1);
    let (u, v) = (u / s, v / s);
    let bone = Ramp::Bone;
    let hd = u * u + (v - 2) * (v - 2);
    if hd <= 3 {
        return Some(bone.at(if u <= 0 && v <= 2 { Tone::Light } else { Tone::Base }));
    }
    if (hd == 5 || hd == 8) && v <= 1 {
        return Some(Ramp::Brass.at(Tone::Light));
    }
    // Wings: from the shoulders up and out, feathered.
    let au = u.abs();
    if (2..=5).contains(&au) && v >= 7 - au && v <= 14 - au {
        return Some(bone.at(if u < 0 { Tone::Lift } else { Tone::Shade }));
    }
    // The robe, widening to her feet.
    if (5..=18).contains(&v) && au <= 1 + (v - 5) / 5 {
        let t = match u.signum() {
            -1 => Tone::Light,
            1 => Tone::Mid,
            _ => Tone::Base,
        };
        return Some(bone.at(t));
    }
    None
}

/// The theme's emblem writ large over the boss's room (B2), at world px `wpx`, `fy` rows down the
/// face, centred on the room.
fn emblem(th: &Theme, room: &DRoom, wpx: i32, fy: i32, tall: bool) -> Option<Px> {
    if !tall {
        return None;
    }
    // Over the middle of the north wall; where a door stands there, a pair, symmetric about it.
    let cx = room.centre().0;
    let mid = cx.div_euclid(CELL);
    let door = room.doors.iter().any(|d| d.side == Side::N && d.a <= mid + 2 && d.b >= mid - 2);
    let dx = if door {
        let off = (room.rect.w * CELL / 4).max(3 * CELL);
        let l = wpx - (cx - off);
        let r = wpx - (cx + off);
        if l.abs() < r.abs() { l } else { r }
    } else {
        wpx - cx
    };
    if dx.abs() > 26 {
        return None;
    }
    let nf = normal(0, FACE);
    let [ra, rb] = th.emblem_ramps;
    let wheel = |spokes: i32, r: i32| -> Option<Px> {
        let dy = fy - 16;
        let d2 = dx * dx + dy * dy;
        if d2 > r * r {
            return None;
        }
        let d = jane_core::num::isqrt(d2 as u64) as i32;
        if d >= r - 2 {
            let t = if dy < 0 { Tone::Light } else { Tone::Shade };
            return Some(Px { ix: ra.at(t), n: normal(dx * 4, FACE - dy * 4), dz: 2, glow: false });
        }
        if d <= 2 {
            let t = if dx + dy < 0 { Tone::Light } else { Tone::Base };
            return Some(Px { ix: rb.at(t), n: nf, dz: 3, glow: false });
        }
        let a = i32::from(iatan2(dy, dx).0);
        let sector = 65536 / spokes;
        let off = a.rem_euclid(sector);
        if i64::from(off.min(sector - off)) * i64::from(d) * 6283 / 65_536_000 < 1 {
            return Some(Px { ix: ra.at(Tone::Base), n: nf, dz: 2, glow: false });
        }
        None
    };
    match th.row.emblem {
        ThemeEmblem::Plain => None,
        ThemeEmblem::Wheel => wheel(6, 13),
        ThemeEmblem::Headframe => {
            if let Some(w) = wheel(8, 13) {
                return Some(w);
            }
            // Its legs, splayed to the foot.
            let leg = |x0: i32| (dx - (x0 + (fy - 16) * x0.signum() / 2)).abs() <= 1;
            (fy > 16 && (leg(-6) || leg(6))).then(|| {
                let t = if dx < 0 { Tone::Lift } else { Tone::Shade };
                Px { ix: Ramp::WoodDark.at(t), n: nf, dz: 1, glow: false }
            })
        }
        ThemeEmblem::Rose => {
            // A rose window: tracery round a hub, coloured glass between, lit.
            if let Some(w) = wheel(8, 13) {
                return Some(w);
            }
            let dy = fy - 16;
            if dx * dx + dy * dy > 121 {
                return None;
            }
            let a = i32::from(iatan2(dy, dx).0) / (65536 / 8);
            let glass = [Ramp::ClothRed, Ramp::ClothBlue, Ramp::ClothMustard, Ramp::ClothBlue][(a & 3) as usize];
            Some(lit(glass.at(if (dx + dy) % 3 == 0 { Tone::Light } else { Tone::Base })))
        }
        ThemeEmblem::Angel => angel(dx, fy - 3, 2).map(|ix| Px { ix, n: nf, dz: 2, glow: false }),
        ThemeEmblem::Furnace => {
            // A mouth in an arch, glowing from its throat, a grate across it.
            let dy = fy - 12;
            let inside = dx.abs() <= 12 && (dy >= 0 || dx * dx + dy * dy <= 144);
            let ring = dx.abs() <= 15 && (dy >= 0 || dx * dx + dy * dy <= 225);
            if inside {
                if dx.rem_euclid(5) == 0 && fy > 18 {
                    return Some(px(Ramp::Iron.at(Tone::Shade)));
                }
                let t = match fy {
                    0..=10 => Tone::Base,
                    11..=18 => Tone::Light,
                    19..=25 => Tone::High,
                    _ => Tone::Glint,
                };
                return Some(lit(rb.at(t)));
            }
            (ring && fy < 31).then(|| px(ra.at(if (fy + dx.abs()) % 3 == 0 { Tone::Shade } else { Tone::Light })))
        }
        ThemeEmblem::Clock => {
            // A clock stopped at five to nine.
            let dy = fy - 15;
            let d2 = dx * dx + dy * dy;
            if d2 > 196 {
                return None;
            }
            if d2 >= 160 {
                return Some(Px {
                    ix: ra.at(if dy < 0 { Tone::Light } else { Tone::Shade }),
                    n: nf,
                    dz: 2,
                    glow: false,
                });
            }
            let hour = dy == 0 && (-6..=0).contains(&dx);
            let minute = (-5..=0).contains(&dx) && dy == dx * 2;
            if hour || minute || d2 <= 1 {
                return Some(raised(Ramp::ClothBlack.at(Tone::Base), 1));
            }
            if d2 >= 120 && (dx == 0 || dy == 0) {
                return Some(px(Ramp::ClothBlack.at(Tone::Shade)));
            }
            Some(px(rb.at(if dx + dy < 0 { Tone::Light } else { Tone::Lift })))
        }
        ThemeEmblem::Portrait => {
            // A great portrait: a pale face in the dark, looking out.
            let (u, v) = (dx + 20, fy - 5);
            if !(0..41).contains(&u) || !(0..25).contains(&v) {
                return None;
            }
            if u < 2 || v < 2 || u > 38 || v > 22 {
                return Some(px(ra.at(if u < 2 || v < 2 { Tone::Light } else { Tone::Shade })));
            }
            let fy2 = v - 11;
            if dx * dx * 3 + fy2 * fy2 * 2 < 60 {
                if fy2 == -1 && dx.abs() == 2 {
                    return Some(px(Ramp::ClothBlack.at(Tone::Deep)));
                }
                return Some(px(rb.at(if dx < 0 { Tone::Light } else { Tone::Base })));
            }
            Some(px(if v > 16 { Ramp::ClothBlack.at(Tone::Base) } else { Ramp::WoodDark.at(Tone::Deep) }))
        }
    }
}

/// A wall cell beside a door in a west, east or south wall: its post capped.
fn post(p: &mut Painter, dg: &Dungeon, cx: i32, cy: i32, wx: i32, wy: i32) {
    let Some((_, d, _)) = dg.jamb(wx, wy, 1) else { return };
    if d.side == Side::N || p.style_k(Painter::at(cx, cy)).tile == Tile::Void {
        return;
    }
    let r = dg.theme.frame;
    let (bx, by) = (cx * CELL, cy * CELL);
    for y in 2..14 {
        for x in 2..14 {
            let t = if x == 2 || y == 2 {
                Tone::Light
            } else if x == 13 || y == 13 {
                Tone::Deep
            } else if (7..=8).contains(&x) && (7..=8).contains(&y) {
                Tone::Lift
            } else {
                Tone::Base
            };
            let z = p.s.ly.z(bx + x, by + y) + 2;
            p.s.ly.put(bx + x, by + y, r.at(t), FLAT, z);
        }
    }
}

/// A lintel over a north door's opening: the frame carried across it, a keystone at its middle;
/// a boss's deeper and carved.
fn lintel(p: &mut Painter, dg: &Dungeon, cx: i32, cy: i32, wx: i32, wy: i32) {
    let Some(i) = dg.room_id(wx, wy + 2) else { return };
    let door = dg.rooms[usize::from(i)]
        .doors
        .iter()
        .find(|d| d.side == Side::N && d.at == wy + 1 && (d.a..=d.b).contains(&wx));
    let Some(d) = door.copied() else { return };
    let r = dg.theme.frame;
    let deep = if d.boss { 9 } else { 6 };
    let mid = (d.a + d.b + 1) * CELL / 2;
    let key = 3 + i32::from(d.boss);
    let (bx, by) = (cx * CELL, cy * CELL);
    for y in 0..deep {
        for x in 0..CELL {
            let wpx = wx * CELL + x;
            let off = (wpx - mid).abs();
            let t = if y == 0 {
                Tone::Light
            } else if y == deep - 1 {
                Tone::Deep
            } else if off < key {
                Tone::Lift
            } else if off == key || (d.boss && y == deep / 2 && wpx.rem_euclid(4) == 0) {
                Tone::Shade
            } else {
                Tone::Base
            };
            let n = normal(0, if y == 0 { -50 } else { FACE });
            p.s.ly.put(bx + x, by + y, r.at(t), n, face_z(y, 1));
        }
    }
}

// --- floors ------------------------------------------------------------------------------------

/// Which of a room cell's sides and corners open onto what is not its room.
#[derive(Clone, Copy, Default)]
struct Open {
    n: bool,
    s: bool,
    w: bool,
    e: bool,
    nw: bool,
    ne: bool,
    sw: bool,
    se: bool,
}

/// Whether local px `(x, y)` of a room cell lies on its inset border band: `(across, along_x,
/// corner)`, `across` 0 at the band's wall side to 3.
fn band(o: Open, x: i32, y: i32) -> Option<(i32, bool, bool)> {
    let (b0, b1) = (11, 14);
    let (l0, l1) = (CELL - 1 - b1, CELL - 1 - b0);
    let span = |v: i32, lo_open: bool, hi_open: bool| {
        let lo = if lo_open { b0 } else { 0 };
        let hi = if hi_open { l1 } else { CELL - 1 };
        (lo..=hi).contains(&v)
    };
    let (in_x0, in_x1) = ((b0..=b1).contains(&x), (l0..=l1).contains(&x));
    let (in_y0, in_y1) = ((b0..=b1).contains(&y), (l0..=l1).contains(&y));
    let corner = (o.n && o.w && in_x0 && in_y0)
        || (o.n && o.e && in_x1 && in_y0)
        || (o.s && o.w && in_x0 && in_y1)
        || (o.s && o.e && in_x1 && in_y1);
    if o.n && in_y0 && span(x, o.w, o.e) {
        return Some((y - b0, true, corner));
    }
    if o.s && in_y1 && span(x, o.w, o.e) {
        return Some((l1 - y, true, corner));
    }
    if o.w && in_x0 && span(y, o.n, o.s) {
        return Some((x - b0, false, corner));
    }
    if o.e && in_x1 && span(y, o.n, o.s) {
        return Some((l1 - x, false, corner));
    }
    // An inside corner: the bands of the cells beside it turn here.
    let diag = |open: bool, a: bool, b: bool| open && !a && !b;
    if diag(o.nw, o.n, o.w) && ((in_x0 && y <= b1) || (in_y0 && x <= b1)) {
        return Some((if in_x0 { x - b0 } else { y - b0 }, !in_x0, in_x0 && in_y0));
    }
    if diag(o.ne, o.n, o.e) && ((in_x1 && y <= b1) || (in_y0 && x >= l0)) {
        return Some((if in_x1 { l1 - x } else { y - b0 }, !in_x1, in_x1 && in_y0));
    }
    if diag(o.sw, o.s, o.w) && ((in_x0 && y >= l0) || (in_y1 && x <= b1)) {
        return Some((if in_x0 { x - b0 } else { l1 - y }, !in_x0, in_x0 && in_y1));
    }
    if diag(o.se, o.s, o.e) && ((in_x1 && y >= l0) || (in_y1 && x >= l0)) {
        return Some((if in_x1 { l1 - x } else { l1 - y }, !in_x1, in_x1 && in_y1));
    }
    None
}

/// The border's px, `across` 0 to 3 over the band, `along` world px along it.
fn inlay(th: &Theme, floor: Ramp, across: i32, along: i32, corner: bool) -> Ix {
    let [a, b] = th.border_ramps;
    let (a, b) = (a.unwrap_or(floor), b.unwrap_or(floor));
    if corner {
        // A square knot at a corner, lit on its top and left.
        let t = match across {
            0 => Tone::Light,
            3 => Tone::Shade,
            _ => Tone::Lift,
        };
        return a.at(t);
    }
    match th.row.border {
        ThemeBorder::Edging => match across {
            0 => a.at(Tone::Light),
            3 => a.at(Tone::Deep),
            _ if along.rem_euclid(12) == 5 => b.at(Tone::Light),
            _ => a.at(if along.rem_euclid(12) == 0 { Tone::Shade } else { Tone::Base }),
        },
        ThemeBorder::Lozenge => {
            let k = (along.rem_euclid(8) - 4).abs() + (across * 2 - 3).abs() / 2;
            match across {
                0 | 3 => floor.at(Tone::Deep),
                _ if k <= 1 => a.at(if across == 1 { Tone::Light } else { Tone::Base }),
                _ => floor.at(Tone::Shade),
            }
        }
        ThemeBorder::Inlay => match across {
            0 => a.at(Tone::Light),
            3 => a.at(Tone::Base),
            _ => b.at(if along.rem_euclid(6) < 3 { Tone::Shade } else { Tone::Mid }),
        },
        ThemeBorder::Stripes => {
            if (along + across).rem_euclid(8) < 4 {
                a.at(if across == 0 { Tone::Light } else { Tone::Base })
            } else {
                b.at(Tone::Base)
            }
        }
        ThemeBorder::Kerb => match across {
            0 => floor.at(Tone::Light),
            3 => floor.at(Tone::Deep),
            _ if along.rem_euclid(16) < 3 => floor.at(Tone::Deep),
            _ => floor.at(Tone::Lift),
        },
        ThemeBorder::Line => match across {
            1 | 2 => a.at(if along.rem_euclid(24) == 0 { Tone::Base } else { Tone::Light }),
            _ => floor.at(Tone::Base),
        },
        ThemeBorder::Pebbles => match across {
            0 | 3 => floor.at(Tone::Shade),
            _ => a.at(if along.rem_euclid(5) < 2 { Tone::Light } else { Tone::Base }),
        },
    }
}

/// A room's floor cell dressed: its lane worn, its motifs laid, its border inset, its payoff.
#[allow(clippy::too_many_arguments)]
fn floor(p: &mut Painter, dg: &Dungeon, i: usize, cx: i32, cy: i32, wx: i32, wy: i32, seed: u32) {
    let th = &dg.theme;
    let room = &dg.rooms[i];
    let id = Some(i as u8);
    let other = |dx: i32, dy: i32| dg.room_id(wx + dx, wy + dy) != id;
    let o = Open {
        n: other(0, -1),
        s: other(0, 1),
        w: other(-1, 0),
        e: other(1, 0),
        nw: other(-1, -1),
        ne: other(1, -1),
        sw: other(-1, 1),
        se: other(1, 1),
    };
    let ring = o.n || o.s || o.w || o.e;
    let (bx, by) = (cx * CELL, cy * CELL);
    let floor_ramp = p.style_k(Painter::at(cx, cy)).ramp;
    let busy = dg.busy(wx, wy);
    let (rcx, rcy) = room.centre();
    let carpet = th.floor(F::Carpet).filter(|_| (-2..=2).all(|dy| (-2..=2).all(|dx| !other(dx, dy))));
    let window = th.row.wall.iter().find(|m| m.kind == W::Window);
    let wear = i32::from(th.row.lane_wear);
    let mut motif = false;
    let mut bordered = false;
    for y in 0..CELL {
        for x in 0..CELL {
            let (lx, ly) = (bx + x, by + y);
            let (wpx, wpy) = (wx * CELL + x, wy * CELL + y);
            let lane = dg.lane_dist(i, wpx, wpy);
            let mut laid: Option<Px> = None;
            let mut lay = |g: Option<Px>, motif: &mut bool, is_motif: bool| {
                if let Some(g) = g {
                    laid = Some(g);
                    *motif |= is_motif;
                }
            };
            for m in th.row.floor {
                let g = match m.kind {
                    F::Rails => rails(room, wpx, wpy),
                    F::Channel if !ring => channel(room, lane, wpx, wpy),
                    F::Belt if !ring && !busy => belt(room, lane, wpx, wpy),
                    F::Gantry if !ring => gantry(room, i32::from(m.every.max(1)), wpx, wpy),
                    F::Carpet if carpet.is_some() => Some(carpet_px(dg, id, wx, wy, x, y, wpx, wpy, m.ramp)),
                    F::Moon => {
                        window.and_then(|w| moon(dg, room, i32::from(w.every.max(1)), i32::from(w.at), wpx, wpy))
                    }
                    F::Heaps => heap(room, wpx, wpy),
                    F::Toadstools => toadstools(room, wpx, wpy),
                    _ => None,
                };
                if g.is_some() {
                    lay(g, &mut motif, true);
                    break;
                }
            }
            // The lane, worn: whole in its middle, its edge in two-px clusters.
            if laid.is_none() && wear != 0 && lane < LANE_HALF {
                let k = fast((wpx >> 1) as u32, (wpy >> 1) as u32, 0x1a7e) % 8;
                if lane < 6 || (k as i32) < (LANE_HALF - lane) * 2 {
                    laid = Some(shift(wear));
                }
            }
            // The payoff floors.
            if matches!(room.role, Role::Boss | Role::Set) && !busy {
                if let Some(g) = payoff(th, room, floor_ramp, wpx - rcx, wpy - rcy) {
                    laid = Some(match laid {
                        // A tone step and a payoff's step add.
                        Some(l) if l.ix == Ix::CLEAR && g.ix == Ix::CLEAR => shift(l.dz + g.dz),
                        _ => g,
                    });
                    motif = true;
                }
            }
            // The border.
            if let Some((across, along_x, corner)) = band(o, x, y) {
                let along = if along_x { wpx } else { wpy };
                laid = Some(px(inlay(th, floor_ramp, across, along, corner)));
                bordered = true;
            }
            if let Some(g) = laid {
                lay_px(&mut p.s.ly, lx, ly, g);
            }
        }
    }
    // Things that stand in a cell.
    if !busy {
        for m in th.row.floor {
            let drew = match m.kind {
                F::Plinths => ring && plinth(p, i32::from(m.every.max(1)), o, bx, by, wx, wy),
                F::Stanchions => o.n && stanchions(p, i32::from(m.every.max(1)), bx, by, wx),
                F::Chalk => chalk(p, room, bx, by, wx, wy, seed),
                _ => false,
            };
            motif |= drew;
        }
        if matches!(room.role, Role::Boss | Role::Set) && brazier(p, room, th.row.braziers, bx, by, wx, wy) {
            motif = true;
        }
    }
    if bordered {
        mark(p, id, FRAMED_BORDER);
    }
    if motif {
        mark(p, id, FRAMED_MOTIF);
    }
}

/// Lay `g` at chunk-local px `(x, y)`: a colour stood `dz` over the floor (its normal kept when
/// flat), or a tone step on what is there.
fn lay_px(ly: &mut Layers, x: i32, y: i32, g: Px) {
    if g.ix == Ix::CLEAR {
        ly.step(x, y, g.dz);
        return;
    }
    let z = ly.z(x, y);
    let n = if g.n == FLAT { Layers::i(x, y).map_or(FLAT, |i| ly.normal[i]) } else { g.n };
    ly.put(x, y, g.ix, n, z + g.dz);
    if g.glow {
        ly.glow(x, y, g.ix);
    }
}

/// Rails along the lanes: two iron rails on timber sleepers (every lane is square to the walls).
fn rails(room: &DRoom, wpx: i32, wpy: i32) -> Option<Px> {
    for &(ax, ay, bx, by) in &room.lanes {
        let (along, across, lo, hi) =
            if ay == by { (wpx, wpy - ay, ax.min(bx), ax.max(bx)) } else { (wpy, wpx - ax, ay.min(by), ay.max(by)) };
        if along < lo - 5 || along > hi + 5 || across.abs() > 5 {
            continue;
        }
        let iron = Ramp::Iron;
        if across.abs() == 3 {
            return Some(raised(iron.at(if across < 0 { Tone::Light } else { Tone::Lift }), 1));
        }
        if across == 4 {
            return Some(px(iron.at(Tone::Shade)));
        }
        if along.rem_euclid(6) < 2 {
            return Some(px(Ramp::WoodDark.at(if along.rem_euclid(6) == 0 { Tone::Base } else { Tone::Shade })));
        }
    }
    None
}

/// A channel of water down the middle of a room along its longer side, between lit kerbs, an iron
/// grate across it where a lane crosses and every four cells.
fn channel(room: &DRoom, lane: i32, wpx: i32, wpy: i32) -> Option<Px> {
    let (cx, cy) = room.centre();
    let (across, along) = if room.rect.w >= room.rect.h { (wpy - cy, wpx) } else { (wpx - cx, wpy) };
    let a = across.abs();
    if a > 7 {
        return None;
    }
    if a >= 6 {
        return Some(raised(Ramp::Stone.at(if a == 6 { Tone::Light } else { Tone::Shade }), 1));
    }
    if lane < LANE_HALF || along.rem_euclid(4 * CELL) < 6 {
        let bar = along.rem_euclid(3) == 0;
        return Some(raised(Ramp::Iron.at(if bar { Tone::Light } else { Tone::Deep }), 1));
    }
    let t = if (along * 7 + across * 13).rem_euclid(23) == 0 {
        Tone::Light
    } else if a >= 4 {
        Tone::Shade
    } else {
        Tone::Base
    };
    Some(raised(Ramp::Water.at(t), -1))
}

/// A belt along a room, two cells off its middle line, its rollers ticking, broken where a lane
/// crosses it and short of the walls.
fn belt(room: &DRoom, lane: i32, wpx: i32, wpy: i32) -> Option<Px> {
    let (cx, cy) = room.centre();
    let wide = room.rect.w >= room.rect.h;
    let (across, along, len) = if wide { (wpy - cy, wpx - cx, room.rect.w) } else { (wpx - cx, wpy - cy, room.rect.h) };
    let b = across - 2 * CELL;
    if !(-5..=5).contains(&b) || lane < 2 * CELL || along.abs() >= len * CELL / 2 - 3 * CELL {
        return None;
    }
    let iron = Ramp::Iron;
    let ix = match b {
        -5 => iron.at(Tone::Light),
        5 => iron.at(Tone::Deep),
        -4 | 4 => iron.at(if along.rem_euclid(4) == 0 { Tone::High } else { Tone::Shade }),
        _ => Ramp::ClothBlack.at(if (along + b).rem_euclid(5) == 0 { Tone::Lift } else { Tone::Base }),
    };
    Some(raised(ix, 2))
}

/// A gantry's shadow across the floor every `every` cells, a lattice of light through it.
fn gantry(room: &DRoom, every: i32, wpx: i32, wpy: i32) -> Option<Px> {
    let (cx, cy) = room.centre();
    let (across, along) = if room.rect.w >= room.rect.h { (wpy - cy, wpx - cx) } else { (wpx - cx, wpy - cy) };
    let g = along.rem_euclid(every * CELL);
    if g >= 10 {
        return None;
    }
    let lattice = (g + across).rem_euclid(10) < 2 || (g - across).rem_euclid(10) < 2;
    (!lattice || g == 0 || g == 9).then_some(shift(-1))
}

/// A carpet two cells in from the walls: a field of small lozenges inside a figured border.
#[allow(clippy::too_many_arguments)]
fn carpet_px(dg: &Dungeon, id: Option<u8>, wx: i32, wy: i32, x: i32, y: i32, wpx: i32, wpy: i32, ramp: &str) -> Px {
    let field = Ramp::by_name(ramp).unwrap_or(Ramp::Oxblood);
    let edge = |dx: i32, dy: i32| !(-2..=2).all(|ey| (-2..=2).all(|ex| dg.room_id(wx + dx + ex, wy + dy + ey) == id));
    let (n, s, w, e) = (edge(0, -1), edge(0, 1), edge(-1, 0), edge(1, 0));
    let d = [(n, y), (s, CELL - 1 - y), (w, x), (e, CELL - 1 - x)]
        .into_iter()
        .filter(|(o, _)| *o)
        .map(|(_, d)| d)
        .min()
        .unwrap_or(99);
    let gold = Ramp::ClothMustard;
    let ix = match d {
        0 => gold.at(Tone::Shade),
        1 | 4 => gold.at(Tone::Base),
        2 | 3 => {
            let a = if n || s { wpx } else { wpy };
            if a.rem_euclid(6) < 3 { field.at(Tone::Shade) } else { gold.at(Tone::Lift) }
        }
        _ => {
            let (u, v) = (wpx.rem_euclid(10) - 5, wpy.rem_euclid(10) - 5);
            match u.abs() + v.abs() {
                3 => Ramp::ClothNavy.at(Tone::Base),
                0 | 1 => gold.at(Tone::Base),
                _ => field.at(Tone::Base),
            }
        }
    };
    px(ix)
}

/// Moonlight on a room's floor from the high windows of its north wall (every `every` cells,
/// `at` px into the period): each window's light sheared as it falls, its mullion and transom
/// bars of shade across it.
fn moon(dg: &Dungeon, room: &DRoom, every: i32, at: i32, wpx: i32, wpy: i32) -> Option<Px> {
    let dy = wpy - room.rect.y * CELL;
    if !(6..46).contains(&dy) {
        return None;
    }
    let sx = wpx - dy / 3;
    let u = (sx - at).rem_euclid(every * CELL);
    if !(1..15).contains(&u) || (7..=8).contains(&u) || (24..27).contains(&dy) {
        return None;
    }
    // Only under a window that is there: a face two cells tall at that column.
    let col = sx.div_euclid(CELL);
    let wall = |y: i32| dg.room_id(col, y).is_none();
    (wall(room.rect.y - 1) && wall(room.rect.y - 2) && dg.room_id(col, room.rect.y).is_some()).then_some(shift(1))
}

/// A fairy ring: toadstools in a ring two cells across, red caps flecked white, each lit on its
/// top and casting its own small shade, the grass inside a tone darker.
fn toadstools(room: &DRoom, wpx: i32, wpy: i32) -> Option<Px> {
    for &(hx, hy) in &room.spots {
        let (dx, dy) = (wpx - (hx * CELL + CELL / 2), wpy - (hy * CELL + CELL / 2));
        let d2 = dx * dx + dy * dy;
        if d2 > 22 * 22 {
            continue;
        }
        // Twelve toadstools round the ring, each its own size.
        let a = i32::from(iatan2(dy, dx).0);
        let k = (a + 2731) / 5461;
        let ca = k * 5461;
        let s = jane_core::angle::sin_q15(jane_core::Angle(ca as u16));
        let c = jane_core::angle::cos_q15(jane_core::Angle(ca as u16));
        let (tx, ty) = (c.0 * 18 / 32768, s.0 * 18 / 32768);
        let (u, v) = (dx - tx, dy - ty);
        let big = 1 + i32::from(fast(k as u32, (hx * 31 + hy) as u32, 0x70ad) % 3 == 0);
        if u.abs() <= big && (-big - 1..=0).contains(&v) {
            let t = if v == -big - 1 || u < 0 { Tone::Light } else { Tone::Base };
            let fleck = (u + v) % 3 == 0 && v < 0;
            let ix = if fleck { Ramp::ClothCream.at(Tone::Light) } else { Ramp::ClothRed.at(t) };
            return Some(raised(ix, 3 - v));
        }
        if u.abs() <= 0 && (1..=2).contains(&v) {
            return Some(raised(Ramp::ClothCream.at(Tone::Base), 2 - v));
        }
        if (1..=3).contains(&u) && v == 2 {
            return Some(shift(-1));
        }
        if d2 < 15 * 15 && fast((wpx >> 1) as u32, (wpy >> 1) as u32, 0x70ae) % 4 == 0 {
            return Some(shift(-1));
        }
        return None;
    }
    None
}

/// Bones heaped: a mound of long bones crossed every way, a skull on it, its foot in shade.
fn heap(room: &DRoom, wpx: i32, wpy: i32) -> Option<Px> {
    for (k, &(hx, hy)) in room.spots.iter().enumerate() {
        let (dx, dy) = (wpx - (hx * CELL + CELL / 2), (wpy - (hy * CELL + CELL / 2)) * 3 / 2);
        let d2 = dx * dx + dy * dy;
        if d2 > 110 {
            continue;
        }
        let bone = Ramp::Bone;
        let h = fast((wpx >> 1) as u32, (wpy >> 1) as u32, 0xb0e5 + k as u32);
        if d2 > 80 {
            // Its foot: a few strays and the shade it throws.
            return Some(if h % 3 == 0 { px(bone.at(Tone::Mid)) } else { shift(-1) });
        }
        // A skull near its top.
        let (sx, sy) = (dx + 2 - (k as i32 % 2) * 4, dy + 6);
        if sx * sx + sy * sy <= 4 {
            if sy == 1 && sx.abs() == 1 {
                return Some(raised(Ramp::ClothBlack.at(Tone::Deep), 5));
            }
            let t = if sx + sy < 0 { Tone::Light } else { Tone::Base };
            return Some(Px { ix: bone.at(t), n: normal(-30, -30), dz: 5, glow: false });
        }
        let t = match h % 6 {
            0 => Tone::Shade,
            1 | 2 => Tone::Base,
            3 | 4 => Tone::Lift,
            _ => Tone::Light,
        };
        let t = if dx + dy < 0 { t.step(1) } else { t };
        return Some(Px { ix: bone.at(t), n: normal(dx * 4, dy * 4), dz: (110 - d2) / 25, glow: false });
    }
    None
}

/// The payoff floor of a boss's room (rings and spokes laid concentric to its middle in the
/// theme's inlay, the sectors between in a chequer) or the set room's (a medallion in a pool of
/// stage light).
fn payoff(th: &Theme, room: &DRoom, floor: Ramp, dx: i32, dy: i32) -> Option<Px> {
    let span = room.rect.w.min(room.rect.h) * CELL / 2;
    let boss = room.role == Role::Boss;
    let r = if boss { span - 2 * CELL } else { span / 2 };
    if r < 2 * CELL {
        return None;
    }
    let d2 = dx * dx + dy * dy;
    let d = jane_core::num::isqrt(d2 as u64) as i32;
    if d > r {
        // The stage's light past the medallion, fading in two-px clusters.
        let fade = fast((dx >> 1) as u32, (dy >> 1) as u32, 0x57a6) % (2 * CELL as u32);
        return (!boss && d < r + 2 * CELL && fade >= (d - r) as u32).then_some(shift(1));
    }
    let inlay = th.inlay;
    let spokes = i32::from(if boss { th.row.boss_spokes } else { th.row.set_spokes }).max(3);
    let a = i32::from(iatan2(dy, dx).0);
    let sector = 65536 / spokes;
    let off = a.rem_euclid(sector);
    let arc = (i64::from(off.min(sector - off)) * i64::from(d) * 6283 / 65_536_000) as i32;
    let ring = |at: i32| (d - at).abs() <= 1;
    // The boss's dais: the middle of its floor raised a step, round, its north lip lit and its
    // south riser in shade, the inlay's star on top.
    let dais = r / 4;
    if boss && d <= dais {
        let edge = d >= dais - 1;
        return Some(if d <= 5 {
            raised(inlay.at(if dx + dy < 0 { Tone::Light } else { Tone::Base }), 4)
        } else if edge && dy > 0 {
            Px { ix: floor.at(Tone::Shade), n: normal(dx * 3, dy * 3), dz: 2, glow: false }
        } else if edge {
            Px { ix: floor.at(Tone::Light), n: normal(dx * 3, dy * 3), dz: 3, glow: false }
        } else if arc < 1 {
            raised(inlay.at(Tone::Base), 4)
        } else {
            raised(floor.at(Tone::Lift), 4)
        });
    }
    Some(if d <= 5 {
        px(inlay.at(if dx + dy < 0 { Tone::Light } else { Tone::Base }))
    } else if ring(r - 2) || ring(r / 2) || (boss && ring(r * 3 / 4)) {
        px(inlay.at(if dy < 0 { Tone::Light } else { Tone::Base }))
    } else if arc < 1 && d > 7 {
        px(inlay.at(Tone::Shade))
    } else if boss {
        shift(if d > r / 2 && d < r * 3 / 4 && (a / sector) & 1 == 0 { -1 } else { 0 })
    } else {
        shift(1)
    })
}

/// Where a payoff room's braziers stand: the boss's four at the corners of a square about its
/// middle; the set room's two, either side of its middle against its north wall.
fn brazier_cells(room: &DRoom, count: u8) -> [(i32, i32); 4] {
    let (cx, cy) = (room.rect.x + room.rect.w / 2, room.rect.y + room.rect.h / 2);
    let (ox, oy) = ((room.rect.w / 3).max(2), (room.rect.h / 3).max(2));
    // Mirror about the room's middle: an even room's mirror cell is one less.
    let (ex, ey) = (1 - room.rect.w % 2, 1 - room.rect.h % 2);
    let none = (i32::MIN, 0);
    match (room.role, count) {
        (_, 0) => [none; 4],
        (Role::Boss, 4) => {
            [(cx - ox, cy - oy), (cx + ox - ex, cy - oy), (cx - ox, cy + oy - ey), (cx + ox - ex, cy + oy - ey)]
        }
        (Role::Boss, _) => [(cx - ox, cy), (cx + ox - ex, cy), none, none],
        _ => [(cx - ox, room.rect.y + 1), (cx + ox - ex, room.rect.y + 1), none, none],
    }
}

/// A brazier in cell `(wx, wy)` if one stands there: an iron bowl on legs, coals, a flame that
/// lights the room.
fn brazier(p: &mut Painter, room: &DRoom, count: u8, bx: i32, by: i32, wx: i32, wy: i32) -> bool {
    if !brazier_cells(room, count).contains(&(wx, wy)) {
        return false;
    }
    let (iron, fire) = (Ramp::Iron, Ramp::Ember);
    for y in 0..CELL {
        for x in 0..CELL {
            let u: i32 = x - 8;
            let g = match (u, y) {
                (-1..=0, 1) => lit(fire.at(Tone::Light)),
                (-2..=1, 2..=3) | (-3..=2, 4..=5) => {
                    lit(fire.at(if (-1..=0).contains(&u) { Tone::Glint } else { Tone::High }))
                }
                (-4..=3, 6) => lit(fire.at(Tone::Base)),
                (-5..=4, 7) => px(iron.at(Tone::Light)),
                (-5..=4, 8) => px(iron.at(if u < 0 { Tone::Lift } else { Tone::Base })),
                (-4..=3, 9) | (-3 | 2, 11..=13) | (-1, 11..=12) => px(iron.at(Tone::Shade)),
                (-3..=2, 10) => px(iron.at(Tone::Deep)),
                _ => continue,
            };
            lay_px(&mut p.s.ly, bx + x, by + y, Px { dz: 14 - y, ..g });
        }
    }
    true
}

/// A plinth against the north wall every `every` cells of its margin, a vase or a bust on it and
/// a label on its front.
fn plinth(p: &mut Painter, every: i32, o: Open, bx: i32, by: i32, wx: i32, wy: i32) -> bool {
    if !o.n || o.w || o.e || wx.rem_euclid(every) != every / 2 {
        return false;
    }
    let h = fast(wx as u32, wy as u32, 0x9117);
    let bust = h & 1 == 1;
    let thing = if bust { Ramp::Bone } else { [Ramp::ClothBlue, Ramp::Copper, Ramp::Brass][(h >> 4) as usize % 3] };
    for y in 0..11 {
        for x in 3..13 {
            let u: i32 = x - 8;
            let (ramp, t, dz) = match y {
                0..=5 => {
                    let shape = if bust {
                        (y < 3 && u.abs() <= 1) || (y >= 3 && u.abs() <= 2)
                    } else {
                        u.abs() <= 1 + y.min(5 - y) / 2
                    };
                    if !shape {
                        continue;
                    }
                    (thing, if u < 0 { Tone::Light } else { Tone::Base }, 14 - y)
                }
                6 => (Ramp::Limewash, Tone::Light, 8),
                7 => (Ramp::Limewash, Tone::Lift, 7),
                9 if (6..=9).contains(&x) => (Ramp::ClothCream, Tone::Light, 4),
                _ => (Ramp::Limewash, if x == 12 { Tone::Shade } else { Tone::Base }, 10 - y),
            };
            lay_px(&mut p.s.ly, bx + x, by + y, raised(ramp.at(t), dz));
        }
    }
    true
}

/// Brass stanchions on the north band every `every` cells, a red rope swagged between.
fn stanchions(p: &mut Painter, every: i32, bx: i32, by: i32, wx: i32) -> bool {
    let period = every * CELL;
    for x in 0..CELL {
        let u = (wx * CELL + x).rem_euclid(period);
        let (lx, base) = (bx + x, by + 12);
        if u < 2 {
            for k in 0..9 {
                let t = if k == 8 {
                    Tone::High
                } else if u == 0 {
                    Tone::Light
                } else {
                    Tone::Shade
                };
                lay_px(&mut p.s.ly, lx, base - k, raised(Ramp::Brass.at(t), k));
            }
        } else {
            let sag = (u * (period - u)) * 4 / (period * period / 4);
            let y = base - 7 + sag;
            lay_px(&mut p.s.ly, lx, y, raised(Ramp::ClothRed.at(Tone::Base), 7 - sag));
            lay_px(&mut p.s.ly, lx, y + 1, raised(Ramp::ClothRed.at(Tone::Deep), 6 - sag));
        }
    }
    true
}

/// A chalk drawing on a room's floor once a room: a hopscotch ladder three cells long.
fn chalk(p: &mut Painter, room: &DRoom, bx: i32, by: i32, wx: i32, wy: i32, seed: u32) -> bool {
    let h = fast(room.seed, seed, 0xc4a1);
    let hx = room.rect.x + 2 + (h % (room.rect.w.max(5) as u32 - 4)) as i32;
    let hy = room.rect.y + 2 + ((h >> 8) % (room.rect.h.max(6) as u32 - 5)) as i32;
    if wx != hx || !(hy..hy + 3).contains(&wy) {
        return false;
    }
    let chalk = Ramp::ClothCream.at(Tone::Light);
    for y in 0..CELL {
        for x in 2..=13 {
            let line = x == 2 || x == 13 || y == 1 || (wy == hy + 2 && y == 14);
            let mark = (7..=8).contains(&x) && (6..=9).contains(&y) && (x + y + wy) % 3 == 0;
            if (line && (x + y) % 5 != 0) || mark {
                p.s.ly.ink(bx + x, by + y, chalk);
            }
        }
    }
    true
}
