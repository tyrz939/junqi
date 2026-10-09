//! Decks (MAP.md §2.5, §6.1, ART.md §3.1): the looks a span's deck is drawn from, a cell at a
//! time, as sprites the presenter lays over everything on the ground under it (`Depth::Deck`):
//! the deck's surface, its parapets, the piers of a viaduct standing down beside it, and under a
//! deck that runs east-west its face, piers and arches. Five kinds: a stone viaduct (the
//! Lowfields' and the Waters'), a brick one (the Works'), a timber footbridge, an iron walkway
//! (the Works'), and a railway's embankment over an underpass arch.
//!
//! [`layout`] says which piece goes where for a span; [`render`] draws each piece once. Every piece
//! is a pure function of its kind and name, so a console bakes them and never runs this.

use alloc::vec::Vec;

use crate::canvas::{Canvas, FLAT, Normal, normal};
use crate::palette::{Ix, Ramp, Tone};

/// Canvas px a cell.
const C: i32 = crate::canvas::CELL_PX;

/// What a deck is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A viaduct of dressed stone: setts on its deck, an ashlar parapet.
    Stone,
    /// A viaduct of sooted brick: cinders on its deck, a brick parapet under stone coping.
    Brick,
    /// A footbridge of planks on beams, a post-and-rail on each side.
    Timber,
    /// A walkway of chequer plate on girders, railings.
    Iron,
    /// A railway's embankment: ballast, sleepers and rails over an underpass arch.
    Rail,
}

impl Kind {
    /// Every kind, in [`slot`] order.
    pub const ALL: [Kind; 5] = [Kind::Stone, Kind::Brick, Kind::Timber, Kind::Iron, Kind::Rail];

    /// Its name in a sheet.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Stone => "stone",
            Kind::Brick => "brick",
            Kind::Timber => "timber",
            Kind::Iron => "iron",
            Kind::Rail => "rail",
        }
    }

    /// A span's kind by the region it is in (0 the Lowfields, 1 the Waters, 2 the Works), how
    /// wide it is and which way it runs, until spans carry their own (MAP.md R4): two cells wide
    /// is a footbridge (timber, iron in the Works); wider along y a viaduct (stone, brick in the
    /// Works); wider along x a railway's embankment over its arch (brick in the Works).
    pub const fn of(region: u8, width: i32, along_x: bool) -> Kind {
        match (region, width <= 2, along_x) {
            (2, true, _) => Kind::Iron,
            (_, true, _) => Kind::Timber,
            (2, false, _) => Kind::Brick,
            (_, false, true) => Kind::Rail,
            _ => Kind::Stone,
        }
    }
}

/// A piece of a deck.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Piece {
    /// A deck running north-south: its surface (two variants), its west and east edges with
    /// their parapets, an edge with a pier standing down beside it (`PierW`, `PierE`: wider than
    /// a cell, out over the ground beside), and a broken deck's ends (`StubN`, the north stub's
    /// ragged south end; `StubS`).
    DeckY(u8),
    EdgeW,
    EdgeE,
    PierW,
    PierE,
    StubN,
    StubS,
    /// A deck running east-west: its surface, its north and south edges.
    DeckX(u8),
    EdgeN,
    EdgeS,
    /// Under an east-west deck's south edge, two cells of its face a level: a pier's, an arch's
    /// springing on its west and on its east, the arch's crown, and an arch a cell wide.
    FacePier,
    ArchW,
    ArchMid,
    ArchE,
    ArchOne,
}

impl Piece {
    /// Every piece, in [`slot`] order.
    pub const ALL: [Piece; 17] = [
        Piece::DeckY(0),
        Piece::DeckY(1),
        Piece::EdgeW,
        Piece::EdgeE,
        Piece::PierW,
        Piece::PierE,
        Piece::StubN,
        Piece::StubS,
        Piece::DeckX(0),
        Piece::DeckX(1),
        Piece::EdgeN,
        Piece::EdgeS,
        Piece::FacePier,
        Piece::ArchW,
        Piece::ArchMid,
        Piece::ArchE,
        Piece::ArchOne,
    ];

    /// Its place in [`Piece::ALL`].
    pub fn index(self) -> usize {
        Piece::ALL.iter().position(|&p| p == self).unwrap_or(0)
    }

    /// Its size in px, and where its top-left stands from its cell's top-left.
    pub const fn frame(self) -> (i32, i32, i32, i32) {
        match self {
            Piece::PierW => (C + PIER_OUT, C + 2 * C, -PIER_OUT, 0),
            Piece::PierE => (C + PIER_OUT, C + 2 * C, 0, 0),
            Piece::FacePier | Piece::ArchW | Piece::ArchMid | Piece::ArchE | Piece::ArchOne => (C, 2 * C, 0, 0),
            _ => (C, C, 0, 0),
        }
    }
}

/// How far a viaduct's pier stands out past its parapet, px.
const PIER_OUT: i32 = 6;

/// The index of a kind's piece in a flat table of every kind's pieces ([`Kind::ALL`] by
/// [`Piece::ALL`]).
pub fn slot(kind: Kind, piece: Piece) -> usize {
    kind as usize * Piece::ALL.len() + piece.index()
}

/// Every kind's every piece, in [`slot`] order: what a presenter packs.
pub fn all() -> Vec<(Kind, Piece)> {
    Kind::ALL.iter().flat_map(|&k| Piece::ALL.iter().map(move |&p| (k, p))).collect()
}

/// A piece laid: the cell its frame is placed from (`Piece::frame`), and the piece.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Laid {
    pub x: i32,
    pub y: i32,
    pub piece: Piece,
}

/// The pieces of a span over `rect` (cells), along x or y, whole or broken, its deck `levels`
/// over the ground under it; appended to `out`, north to south, so they overlap as they should.
pub fn layout(rect: (i32, i32, i32, i32), along_x: bool, whole: bool, levels: i32, out: &mut Vec<Laid>) {
    let (x0, y0, w, h) = rect;
    let v = |x: i32, y: i32| (crate::hash::h32(x as u32, y as u32, 0xdec4) & 1) as u8;
    if !along_x {
        for y in y0..y0 + h {
            let r = y - y0;
            // A broken deck keeps a stub at each end: two rows, its ragged end inward.
            if !whole && r >= 2 && r < h - 2 {
                continue;
            }
            for x in x0..x0 + w {
                let c = x - x0;
                let piece = if !whole && (r == 1 || r == h - 2) && c > 0 && c < w - 1 {
                    if r == 1 { Piece::StubN } else { Piece::StubS }
                } else if c == 0 {
                    if r % 4 == 2 && r > 0 && r < h - 1 { Piece::PierW } else { Piece::EdgeW }
                } else if c == w - 1 {
                    if r % 4 == 2 && r > 0 && r < h - 1 { Piece::PierE } else { Piece::EdgeE }
                } else {
                    Piece::DeckY(v(x, y))
                };
                out.push(Laid { x, y, piece });
            }
        }
        return;
    }
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let piece = if y == y0 {
                Piece::EdgeN
            } else if y == y0 + h - 1 {
                Piece::EdgeS
            } else {
                Piece::DeckX(v(x, y))
            };
            if whole || x == x0 || x == x0 + w - 1 {
                out.push(Laid { x, y, piece });
            }
        }
    }
    // Under its south edge, its face: a pier at each end, the arches between, every level.
    let n = w - 2;
    for lv in 0..levels.max(1) {
        let fy = y0 + h + 2 * lv;
        for x in x0..x0 + w {
            let c = x - x0;
            let piece = if c == 0 || c == w - 1 || lv > 0 {
                Piece::FacePier
            } else if n == 1 {
                Piece::ArchOne
            } else if c == 1 {
                Piece::ArchW
            } else if c == w - 2 {
                Piece::ArchE
            } else {
                Piece::ArchMid
            };
            if whole || c == 0 || c == w - 1 {
                out.push(Laid { x, y: fy, piece });
            }
        }
    }
}

/// The ramps of a kind: its deck, its walling and its trim.
struct Mats {
    deck: Ramp,
    wall: Ramp,
    trim: Ramp,
}

const fn mats(kind: Kind) -> Mats {
    match kind {
        Kind::Stone => Mats { deck: Ramp::Setts, wall: Ramp::Stone, trim: Ramp::Stone },
        Kind::Timber => Mats { deck: Ramp::WoodOak, wall: Ramp::WoodDark, trim: Ramp::WoodOak },
        Kind::Iron => Mats { deck: Ramp::Iron, wall: Ramp::Iron, trim: Ramp::Iron },
        Kind::Brick | Kind::Rail => Mats { deck: Ramp::Ballast, wall: Ramp::Brick, trim: Ramp::Stone },
    }
}

/// A cheap hash of a px.
fn hx(x: i32, y: i32, s: u32) -> u32 {
    crate::terrain::fast(x as u32, y as u32, s)
}

/// The deck's own surface at px `(x, y)` of a cell (`along_x`: planks and rails turned), the
/// variant `v` shifting its pattern.
fn surface(kind: Kind, x: i32, y: i32, along_x: bool, v: u8) -> (Ix, Normal) {
    surface_of(kind, x, y, along_x, v, true)
}

/// [`surface`], with a railway's track on it or not (its edges carry ballast alone).
fn surface_of(kind: Kind, x: i32, y: i32, along_x: bool, v: u8, track: bool) -> (Ix, Normal) {
    let m = mats(kind);
    // Across the deck and along it.
    let (a, l) = if along_x { (y, x) } else { (x, y) };
    let l = l + i32::from(v) * 5;
    match kind {
        Kind::Stone => {
            // Setts in courses across the way, four px square and rounded (their corners in the
            // joint), each lit along its top; worn paler in drifts along the way.
            let row = l.div_euclid(4);
            let xs = a + (row & 1) * 2;
            let (sx, sy) = (xs.rem_euclid(4), l.rem_euclid(4));
            let s = hx(xs.div_euclid(4), row, 0x5e7);
            let body = match s % 9 {
                0 | 1 => Tone::Shade,
                2 => Tone::Base,
                _ => Tone::Mid,
            };
            let worn = hx(x >> 3, y >> 3, 0x5e8) % 3 == 0 && s % 2 == 0;
            let body = if worn { body.step(1) } else { body };
            let corner = (sx == 0 || sx == 3) && (sy == 0 || sy == 3);
            let t = if sx == 0 || sy == 3 || corner {
                Tone::Deep
            } else if sy == 0 {
                body.step(1)
            } else if sx == 3 {
                body.step(-1)
            } else {
                body
            };
            (m.deck.at(t), normal(if sx == 1 { -20 } else { 0 }, if sy == 0 { -35 } else { 0 }))
        }
        Kind::Brick | Kind::Rail => {
            // Ballast: cinders and stones in clusters; on a railway, sleepers and two rails.
            if kind == Kind::Rail && track {
                let sl = l.rem_euclid(5);
                let rail = a == 4 || a == 11;
                if rail {
                    return (Ramp::Iron.at(Tone::High), normal(0, -40));
                }
                if a == 5 || a == 12 {
                    return (Ramp::Iron.at(Tone::Shade), FLAT);
                }
                if (sl == 0 || sl == 1) && (2..14).contains(&a) {
                    return (Ramp::WoodDark.at(if sl == 0 { Tone::Base } else { Tone::Shade }), FLAT);
                }
            }
            let s = hx(x >> 1, y >> 1, 0xba1);
            let t = match s % 9 {
                0 | 1 => Tone::Mid,
                2 => Tone::Light,
                3 => Tone::Shade,
                _ => Tone::Base,
            };
            (m.deck.at(t), FLAT)
        }
        Kind::Timber => {
            // Planks across the way, three px and a gap, a nail at each beam, worn pale in the middle.
            let p = l.rem_euclid(4);
            let plank = l.div_euclid(4);
            let s = hx(plank, 7, 0x71b);
            if p == 3 {
                return (m.wall.at(Tone::Deep), FLAT);
            }
            let body = match s % 5 {
                0 => Tone::Mid,
                1 => Tone::Lift,
                _ => Tone::Base,
            };
            let body = if (5..11).contains(&a) { body.step(1) } else { body };
            let nail = (a == 2 || a == 13) && p == 1;
            let t = if nail {
                Tone::Deep
            } else if p == 0 {
                body.step(1)
            } else {
                body
            };
            (m.deck.at(t), normal(0, if p == 0 { -30 } else { 0 }))
        }
        Kind::Iron => {
            // Chequer plate: a raised diamond every four px, lit on its upper left; rivets at
            // the plate's joints.
            let (u, w) = ((x + y).rem_euclid(4), (x - y).rem_euclid(4));
            let joint = l.rem_euclid(16) == 0;
            let t = if joint {
                Tone::Shade
            } else if u == 0 && w == 0 {
                Tone::High
            } else if u == 1 && w == 1 {
                Tone::Mid
            } else {
                Tone::Base
            };
            (m.deck.at(t), normal(0, if u == 0 { -40 } else { 0 }))
        }
    }
}

/// A parapet's coping and walling at `d` px in from its outer edge (0 the outside), `long` px
/// along it, for a parapet `deep` px across; `None` past it.
fn parapet(kind: Kind, d: i32, long: i32, deep: i32) -> Option<(Ix, Normal, u8)> {
    if d >= deep {
        return None;
    }
    let m = mats(kind);
    Some(match kind {
        Kind::Timber => {
            // A post every six px, a rail between them.
            let post = long.rem_euclid(6) < 2;
            match (d, post) {
                (0, _) => (m.wall.at(Tone::Deep), FLAT, 6),
                (1 | 2, true) => (m.wall.at(if d == 1 { Tone::Lift } else { Tone::Base }), normal(-30, -30), 9),
                (1, false) => (m.trim.at(Tone::Light), normal(0, -50), 8),
                _ => (m.wall.at(Tone::Shade), FLAT, 4),
            }
        }
        Kind::Iron => {
            let post = long.rem_euclid(8) == 0;
            match (d, post) {
                (0, _) => (m.wall.at(Tone::Deep), FLAT, 7),
                (1, _) => (m.trim.at(Tone::High), normal(0, -50), 9),
                (_, true) => (m.wall.at(Tone::Lift), FLAT, 8),
                _ => (m.wall.at(Tone::Shade), FLAT, 3),
            }
        }
        _ => {
            // Coping stones a few px long, a joint between each: a dark arris outside, the
            // coping's top lit, its inner arris catching the light, and its inner face in shade.
            let joint = long.rem_euclid(9) == 0;
            let t = match d {
                0 => Tone::Deep,
                1 => Tone::Lift,
                2 => Tone::High,
                3 => Tone::Light,
                _ => Tone::Shade,
            };
            let t = if joint && (1..4).contains(&d) { Tone::Mid } else { t };
            let z = if d == 4 { 6 } else { 11 };
            (m.trim.at(t), normal(if d == 3 { -40 } else { 0 }, if d == 4 { 80 } else { -50 }), z)
        }
    })
}

/// How deep a kind's parapet is across, px.
const fn parapet_deep(kind: Kind) -> i32 {
    match kind {
        Kind::Timber | Kind::Iron => 3,
        _ => 5,
    }
}

/// Px of a deck's face (its walling seen from the south): `fx` along it, `fy` down it.
fn walling(kind: Kind, fx: i32, fy: i32) -> (Ix, Normal) {
    let m = mats(kind);
    match kind {
        Kind::Stone => {
            let course = fy.div_euclid(5);
            let xs = fx + (course & 1) * 5;
            let (lx, ly) = (xs.rem_euclid(10), fy.rem_euclid(5));
            let s = hx(xs.div_euclid(10), course, 0x5a5);
            let body = match s % 6 {
                0 => Tone::Mid,
                1 => Tone::Lift,
                _ => Tone::Base,
            };
            let t = if lx == 0 || ly == 4 {
                Tone::Shade
            } else if ly == 0 {
                body.step(1)
            } else {
                body
            };
            (m.wall.at(t.step(-1)), normal(0, 100))
        }
        Kind::Brick | Kind::Rail => {
            let course = fy.div_euclid(4);
            let xs = fx + (course & 1) * 4;
            let (lx, ly) = (xs.rem_euclid(8), fy.rem_euclid(4));
            let s = hx(xs.div_euclid(8), course, 0xb1c);
            let body = match s % 7 {
                0 | 1 => Tone::Mid,
                2 => Tone::Lift,
                _ => Tone::Base,
            };
            let t = if lx == 7 || ly == 3 { Tone::Deep } else { body.step(-1) };
            (m.wall.at(t), normal(0, 100))
        }
        Kind::Timber => {
            let t = if fx.rem_euclid(5) == 0 { Tone::Deep } else { Tone::Shade };
            (m.wall.at(t), normal(0, 100))
        }
        Kind::Iron => {
            let rivet = fx.rem_euclid(5) == 2 && fy.rem_euclid(4) == 1;
            (m.wall.at(if rivet { Tone::Light } else { Tone::Mid }), normal(0, 100))
        }
    }
}

/// Draws piece `piece` of a `kind` deck.
pub fn render(kind: Kind, piece: Piece) -> Canvas {
    let (w, h, ox, _) = piece.frame();
    let mut c = Canvas::new(w, h);
    let deep = parapet_deep(kind);
    match piece {
        Piece::DeckY(v) | Piece::DeckX(v) => {
            let along_x = matches!(piece, Piece::DeckX(_));
            for y in 0..C {
                for x in 0..C {
                    let (ix, n) = surface(kind, x, y, along_x, v);
                    c.put(x, y, ix, n, 2);
                }
            }
        }
        Piece::EdgeW | Piece::EdgeE | Piece::PierW | Piece::PierE => {
            let west = matches!(piece, Piece::EdgeW | Piece::PierW);
            let pier = matches!(piece, Piece::PierW | Piece::PierE);
            // The deck's own cell, where it stands in the frame.
            let cx = if west { -ox } else { 0 };
            for y in 0..C {
                for x in 0..C {
                    let d = if west { x } else { C - 1 - x };
                    let (ix, n, z) = match parapet(kind, d, y, deep) {
                        Some(p) => p,
                        None => {
                            let (ix, n) = surface_of(kind, x, y, false, 0, false);
                            (ix, n, 2)
                        }
                    };
                    c.put(cx + x, y, ix, n, z);
                }
                // The parapet's shadow on the deck: west's falls east of it (the light is from the
                // top-left, so a px), east's two px of contact under its inner face.
                let (sx, steps) = if west { (cx + deep, 1) } else { (cx + C - 1 - deep, 1) };
                shade(&mut c, sx, y, steps);
            }
            if pier {
                // The pier stands down from the deck to the ground under it, out past the parapet:
                // its cap level with the coping, its face dropping two cells, lit west, dark east.
                let (px0, px1) = if west { (0, PIER_OUT + 1) } else { (C - 1, C + PIER_OUT) };
                let m = mats(kind);
                for y in 2..h {
                    for x in px0..px1 {
                        let edge = x == px0 || x == px1 - 1;
                        let cap = y < 6;
                        let (ix, n) = if cap {
                            let t = if y == 2 {
                                Tone::Deep
                            } else if y == 3 {
                                Tone::Light
                            } else {
                                Tone::Base
                            };
                            (m.trim.at(if edge { Tone::Deep } else { t }), normal(0, -40))
                        } else {
                            let (ix, n) = walling(kind, x + if west { 3 } else { 0 }, y);
                            let t = Ramp::of(ix).map_or(Tone::Base, |r| r.1);
                            let t = match (west, x == px0 + 1, x == px1 - 2) {
                                (true, true, _) => t.step(1),
                                (false, _, true) => t.step(-1),
                                _ => t,
                            };
                            let r = Ramp::of(ix).map_or(m.wall, |r| r.0);
                            (if edge { r.at(Tone::Deep) } else { r.at(t) }, n)
                        };
                        // Its heights: the cap at the deck, the face falling to the ground under.
                        let z = if cap { 10 } else { (2 * C + 6 - y).max(1) * 40 / (2 * C) };
                        c.put(x, y, ix, n, z.clamp(1, 255) as u8);
                    }
                }
            }
        }
        Piece::StubN | Piece::StubS => {
            // The deck runs out ragged: each plank or course its own length, splinters, the
            // ground's dark beyond left clear.
            let north = piece == Piece::StubN;
            for x in 0..C {
                let reach = 4 + (hx(x >> 1, 3, 0x57b) % 7) as i32;
                for d in 0..reach {
                    let y = if north { d } else { C - 1 - d };
                    let (ix, n) = surface(kind, x, y, false, 0);
                    let ix = if d == reach - 1 { Ramp::of(ix).map_or(ix, |(r, _)| r.at(Tone::Deep)) } else { ix };
                    c.put(x, y, ix, n, 2);
                }
            }
        }
        Piece::EdgeN | Piece::EdgeS => {
            let north = piece == Piece::EdgeN;
            let solid = !matches!(kind, Kind::Timber | Kind::Iron);
            // The south parapet's coping rows, over its outer face's two.
            let cope0 = C - 2 - deep;
            for y in 0..C {
                for x in 0..C {
                    // The north parapet: its coping at the top, its inner face under it, seen;
                    // the south: its coping over the deck's south edge, its outer face below.
                    let (ix, n, z) = if north {
                        match parapet(kind, y, x, deep) {
                            Some(p) => p,
                            None if y < deep + 3 && solid => {
                                let (ix, n) = walling(kind, x, y - deep);
                                (ix, n, (10 - 2 * (y - deep)).max(3) as u8)
                            }
                            None => {
                                let (ix, n) = surface_of(kind, x, y, true, 0, false);
                                (ix, n, 2)
                            }
                        }
                    } else if y < cope0 {
                        let (ix, n) = surface_of(kind, x, y, true, 0, false);
                        (ix, n, 2)
                    } else if y < C - 2 {
                        parapet(kind, C - 3 - y, x, deep).unwrap_or((mats(kind).trim.at(Tone::Base), FLAT, 8))
                    } else {
                        let (ix, n) = walling(kind, x, y);
                        (ix, n, 8)
                    };
                    c.put(x, y, ix, n, z);
                }
            }
            if north {
                // The inner face's shadow on the deck under it.
                let y = if solid { deep + 3 } else { deep };
                for x in 0..C {
                    shade(&mut c, x, y, 2);
                    shade(&mut c, x, y + 1, 1);
                }
            }
        }
        Piece::FacePier | Piece::ArchW | Piece::ArchMid | Piece::ArchE | Piece::ArchOne => {
            arch(&mut c, kind, piece);
        }
    }
    c
}

/// One step darker along its own ramp at `(x, y)`, `steps` times.
fn shade(c: &mut Canvas, x: i32, y: i32, steps: i32) {
    if let Some((r, t)) = Ramp::of(c.get(x, y)) {
        c.recolour(x, y, r.at(t.step(-steps)));
    }
}

/// A piece of an east-west deck's face: walling with a string course under the parapet, and an
/// arch's ring of voussoirs over the dark of the way under it.
fn arch(c: &mut Canvas, kind: Kind, piece: Piece) {
    let m = mats(kind);
    let h = 2 * C;
    // The arch's curve over this piece: its crown `rise` px under the top, its springing at the
    // piece's edges; the void under it. A half ellipse over the arch's whole span, read per column.
    let (span_x0, span) = match piece {
        Piece::ArchW => (0, 3 * C),
        Piece::ArchMid => (-C, 3 * C),
        Piece::ArchE => (-2 * C, 3 * C),
        Piece::ArchOne => (0, C),
        _ => (0, 0),
    };
    let crown = 7;
    let foot = h;
    for x in 0..C {
        // The arch's intrados at this column: y where the void begins.
        let into = if span > 0 {
            // A half ellipse: y = crown + (foot - crown) (1 - sqrt(1 - (d / span)^2)), `d` the
            // column's distance from the span's middle in half px.
            let d = ((x - span_x0) * 2 + 1 - span).abs();
            let q = 1024 - (d * d * 1024) / (span * span).max(1);
            let s = jane_core::num::isqrt(q.max(0) as u64) as i32;
            crown + (foot - crown) * (32 - s) / 32
        } else {
            h + 1
        };
        for y in 0..h {
            let (ix, n, z) = if y < 2 {
                // The string course under the parapet: lit along its top.
                let t = if y == 0 { Tone::Light } else { Tone::Mid };
                (m.trim.at(t), normal(0, -30), 40)
            } else if y >= into + 2 {
                // The way under the arch: dark, the far mouth's light low down.
                let t = if y > h - 4 { Tone::Shade } else { Tone::Deep };
                (Ramp::WallDark.at(t), FLAT, 1)
            } else if y >= into - 1 {
                // The ring: voussoirs a few px wide, their joints radiating.
                let joint = (x + y).rem_euclid(4) == 0;
                let t = if y > into {
                    Tone::Deep
                } else if joint {
                    Tone::Shade
                } else {
                    Tone::Light
                };
                (m.trim.at(t), normal(0, 60), (40 - y).max(2) as u8)
            } else {
                let (ix, n) = walling(kind, x, y);
                (ix, n, (40 - y).max(2) as u8)
            };
            c.put(x, y, ix, n, z);
        }
    }
    if piece == Piece::FacePier {
        // A pier's pilaster: lit west, dark east.
        for y in 2..h {
            for (x, s) in [(1, 1), (2, 1), (C - 3, -1), (C - 2, -1)] {
                if let Some((r, t)) = Ramp::of(c.get(x, y)) {
                    c.recolour(x, y, r.at(t.step(s)));
                }
            }
        }
    }
}
