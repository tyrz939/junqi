//! A house's front (ART-PLAN Q2): its walls in the material its look names (cream, pink,
//! limewashed or ochre plaster; Flemish-bond brick with burnt headers; knapped flint with brick
//! quoins), a beam under the eave on some, the windows in the house's own rhythm (casements,
//! sashes, bays; St Anne's lancets), shutters in the door's paint on one house in three, a window
//! box in flower on one in two, ivy, wisteria or a climbing rose on one in four, and a number or a
//! nameplate over the door. A house with no look (a sheet's, a test's) is drawn as the town's
//! houses were: cream plaster, a window every third cell.
//!
//! Everything a house's front shows is a pure function of the world px and the house (its block,
//! its door, its seed), so a front split across chunks is one front.

use jane_data::{TileGroup, TilePattern as P};

use super::super::houses::{Age, Climber, House, Kind, Look, Roof, Wall, Window};
use super::super::{CELL, Painter, Style, fast, salt};
use super::{Cell, FACE, face_z, nst, put, run, step, voronoi};
use crate::canvas::{FLAT, UNIT, normal};
use crate::hash::h32;
use crate::palette::{Ramp, Tone};

/// The ramp and pattern a look's roof is laid in.
pub(super) fn roof_of(l: &Look) -> (Ramp, P) {
    match l.roof {
        Roof::Tile if l.age == Age::New => (Ramp::RoofTileNew, P::RoofTile),
        Roof::Tile => (Ramp::RoofTile, P::RoofTile),
        Roof::Slate => (Ramp::Slate, P::Slate),
        Roof::Thatch => (Ramp::Thatch, P::Thatch),
    }
}

/// What the wall is made of.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mat {
    Plaster,
    Brick,
    Flint,
}

/// An opening in the face, cell-local px: the glass's box.
#[derive(Clone, Copy)]
struct Opening {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
}

impl Opening {
    const fn x1(self) -> i32 {
        self.x0 + self.w - 1
    }
    const fn y1(self) -> i32 {
        self.y0 + self.h - 1
    }
}

/// A ground floor window, an upper one.
const GROUND: Opening = Opening { x0: 3, y0: 3, w: 10, h: 10 };
const SASH: Opening = Opening { x0: 4, y0: 2, w: 8, h: 11 };
const UPPER: Opening = Opening { x0: 4, y0: 7, w: 8, h: 8 };

/// A house wall cell: its material, its beam, what climbs it, its eave's shadow, its windows,
/// the plate over its door, its plinth and its corners.
pub(super) fn wall(p: &mut Painter, c: &Cell, seed: u32, house: Option<House>) {
    if let Some(h) = house.filter(|h| h.ruin) {
        ruin(p, c, &h);
        return;
    }
    let is_wall = |s: &Style| matches!(s.row.pattern, P::Plaster | P::Brick);
    let look = house.map(|h| h.look());
    let timber = c.st.accent.unwrap_or(Ramp::WoodDark);
    let (r, mat) = match look.map(|l| l.wall) {
        Some(Wall::Plaster(r)) => (r, Mat::Plaster),
        Some(Wall::Brick) => (Ramp::Brick, Mat::Brick),
        Some(Wall::Flint) => (Ramp::Flint, Mat::Flint),
        None if c.st.row.pattern == P::Brick => (c.st.ramp, Mat::Brick),
        None => (c.st.ramp, Mat::Plaster),
    };
    let (up, down) = (run(p, c, -1, is_wall), run(p, c, 1, is_wall));
    let rows = up + down + 1;
    let z_at = |y: i32| face_z(y, down);
    let (n, s, w, e) = (nst(p, c, 0, -1), nst(p, c, 0, 1), nst(p, c, -1, 0), nst(p, c, 1, 0));
    let (west_end, east_end) = (!is_wall(&w), !is_wall(&e));
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (ramp, t) = match mat {
                Mat::Plaster => {
                    // Plaster weathered in broad soft patches.
                    let v = p.s.fine.at(wx, wy) + (p.s.wob_x.at(wx, wy) - 128) / 3;
                    (r, if v > 214 { Tone::Light } else { Tone::Lift })
                }
                Mat::Brick => (r, flemish(wx, wy)),
                Mat::Flint => {
                    let quoin = |west: bool| {
                        let q = wy.div_euclid(5);
                        let qw = if q & 1 == 0 { 9 } else { 5 };
                        if west { x < qw } else { x >= CELL - qw }
                    };
                    if (west_end && quoin(true)) || (east_end && quoin(false)) {
                        let t = if wy.rem_euclid(5) == 4 {
                            Tone::Shade
                        } else if wy.rem_euclid(5) == 0 {
                            Tone::Lift
                        } else {
                            Tone::Base
                        };
                        (Ramp::Brick, t)
                    } else {
                        flint(wx, wy)
                    }
                }
            };
            // The face darkens a little toward its foot.
            let t = if down == 0 && y > 8 { t.step(-1) } else { t };
            put(p, c, x, y, ramp.at(t), normal(0, FACE), z_at(y));
        }
    }
    // A plaster wall's timber: a beam under the eave.
    let beam = look.map_or(mat == Mat::Plaster, |l| l.beam);
    if beam && n.row.group == TileGroup::Roof {
        for x in 0..CELL {
            put(p, c, x, 3, timber.at(Tone::Lift), normal(0, FACE - 30), z_at(3));
            put(p, c, x, 4, timber.at(Tone::Base), normal(0, FACE), z_at(4));
            put(p, c, x, 5, timber.at(Tone::Shade), normal(0, FACE), z_at(5));
            step(p, c, x, 6, -1);
        }
    }
    if let (Some(l), Some(h)) = (look, house) {
        match l.climber {
            Some(Climber::Ivy) => ivy(p, c, &h, &z_at),
            Some(Climber::Wisteria) => wisteria(p, c, &h, &z_at),
            Some(Climber::Rose) => rose(p, c, &h, &z_at),
            None => {}
        }
    }
    if n.row.group == TileGroup::Roof {
        for x in 0..CELL {
            step(p, c, x, 0, -3);
            step(p, c, x, 1, -2);
            step(p, c, x, 2, -1);
            if fast(x as u32 >> 1, c.wx as u32, c.h) & 1 == 0 {
                step(p, c, x, 3, -1);
            }
        }
    }
    // Windows: the ground floor's in the face's middle course; a wall three courses or more tall
    // has an upper floor, its smaller windows under the eave. A house with a look sets them in
    // its own rhythm and leaves its door's cells clear.
    let inner = !west_end && !east_end;
    let (ground_course, upper_course) = (up == (rows - 1) / 2 && !(rows >= 3 && up == 0), rows >= 3 && up == 0);
    let (ground, upper) = match (look, house) {
        (Some(l), Some(h)) => {
            let rel = c.wx - h.rect.x;
            let stagger = i32::from(l.window == Window::Casement);
            let (g, u) = (windows(&h, &l, 0), windows(&h, &l, stagger));
            let on = |m: u64| (0..64).contains(&rel) && m >> rel & 1 == 1;
            (ground_course && on(g), upper_course && on(u))
        }
        _ => (inner && ground_course && c.wx.rem_euclid(3) == 1, inner && upper_course && c.wx.rem_euclid(3) == 2),
    };
    let lit = h32(c.wx as u32 / 3, c.wy as u32, seed ^ salt::WINDOW) % 3 != 0;
    let frame = match (look.map(|l| l.window), mat) {
        (Some(Window::Sash | Window::Bay), _) | (_, Mat::Brick) => Ramp::Limewash,
        _ => timber,
    };
    let kind = house.map_or(Kind::Home, |h| h.kind);
    if ground {
        match (kind, look.map(|l| l.window)) {
            (Kind::Church, _) => lancet(p, c, lit, &z_at),
            (_, Some(Window::Bay)) => bay(p, c, lit, &z_at),
            (_, Some(Window::Sash)) => sash(p, c, SASH, lit, frame, &z_at),
            _ => casement(p, c, GROUND, lit, frame, &z_at),
        }
        if let Some(l) = look.filter(|l| l.window != Window::Bay && kind != Kind::Church) {
            let o = if l.window == Window::Sash { SASH } else { GROUND };
            if l.shutters {
                shutters(p, c, o, l.door_ramp(), &z_at);
            }
            if l.boxes {
                window_box(p, c, o, h32(c.wx as u32, c.wy as u32, 0xb0c5), &z_at);
            }
        }
    } else if upper {
        let lit = lit && h32(c.wx as u32, 1, seed ^ salt::WINDOW) % 2 == 0;
        match look.map(|l| l.window) {
            Some(Window::Sash) => sash(p, c, UPPER, lit, frame, &z_at),
            _ => casement(p, c, UPPER, lit, frame, &z_at),
        }
        if let Some(l) = look.filter(|l| l.shutters && kind != Kind::Church) {
            shutters(p, c, UPPER, l.door_ramp(), &z_at);
        }
    }
    if let (Some(l), Some(h)) = (look, house) {
        plate(p, c, &h, &l, rows, &z_at);
    }
    if !is_wall(&s) {
        // The plinth: stone under plaster (tarred under limewash), brick under brick and flint.
        let foot = match (mat, r) {
            (Mat::Plaster, Ramp::Limewash) => Ramp::ClothBlack,
            (Mat::Plaster, _) => Ramp::Stone,
            (Mat::Brick, _) => r,
            (Mat::Flint, _) => Ramp::Brick,
        };
        for x in 0..CELL {
            put(p, c, x, CELL - 3, foot.at(Tone::Base), normal(0, FACE), 3);
            put(p, c, x, CELL - 2, foot.at(Tone::Mid), normal(0, FACE), 2);
            put(p, c, x, CELL - 1, foot.at(Tone::Deep), FLAT, 1);
        }
    }
    let corner = match mat {
        Mat::Plaster => timber,
        Mat::Brick => r,
        Mat::Flint => Ramp::Brick,
    };
    if west_end {
        for y in 0..CELL {
            put(p, c, 0, y, r.at(Tone::Deep), FLAT, z_at(y));
            let t = if mat == Mat::Plaster { Tone::Base } else { Tone::Light };
            put(p, c, 1, y, corner.at(t), normal(-50, FACE), z_at(y));
        }
    }
    if east_end {
        for y in 0..CELL {
            put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, z_at(y));
            put(p, c, CELL - 2, y, corner.at(Tone::Shade), normal(50, FACE), z_at(y));
        }
    }
}

/// A ruin's masonry at world px `(wx, wy)` in its look's material: brick and flint as built; a
/// plaster wall's render come away in broad patches, the brick under it showing, more of it the
/// higher up the wall (`up`, 0 to 255).
fn ruin_stone(p: &Painter, mat: Mat, r: Ramp, wx: i32, wy: i32, up: i32) -> (Ramp, Tone) {
    match mat {
        Mat::Brick => (r, flemish(wx, wy)),
        Mat::Flint => flint(wx, wy),
        Mat::Plaster => {
            let v = p.s.fine.at(wx * 3, wy * 3) + (p.s.wob_x.at(wx, wy) - 128) / 2;
            if v + up / 3 > 190 {
                (Ramp::Brick, flemish(wx, wy))
            } else if v + up / 3 > 180 {
                // The render's broken edge, a px proud of the brick and lit.
                (r, Tone::High)
            } else {
                (r, if v > 150 { Tone::Light } else { Tone::Lift })
            }
        }
    }
}

/// A ruin's wall cell (the owner's playtest, 2026-10-07): where its south is open, a face of
/// masonry, cracked, a brick out here and there, an empty window frame in one run in three
/// (no glass, nothing lit), stones fallen against its foot; its broken top is drawn above it by
/// [`ruin_tops`]. Where the wall runs on south, its top: coursed stone, uneven, worn to the core
/// in places.
fn ruin(p: &mut Painter, c: &Cell, h: &House) {
    let is_wall = |s: &Style| matches!(s.row.pattern, P::Plaster | P::Brick);
    let l = h.look();
    let (r, mat) = match l.wall {
        Wall::Plaster(r) => (r, Mat::Plaster),
        Wall::Brick => (Ramp::Brick, Mat::Brick),
        Wall::Flint => (Ramp::Flint, Mat::Flint),
    };
    let cap = if mat == Mat::Brick { Ramp::Brick } else { Ramp::Stone };
    let (s, w, e) = (nst(p, c, 0, 1), nst(p, c, -1, 0), nst(p, c, 1, 0));
    let (west_end, east_end) = (!is_wall(&w), !is_wall(&e));
    if is_wall(&s) {
        // The wall's top, seen from above: its courses lit (a plaster wall's brick core), its
        // west edge lit and its east in shade, and worn down to the rubble core in one cell in
        // three.
        let worn = c.h % 3 == 0;
        for y in 0..CELL {
            for x in 0..CELL {
                let (wx, wy) = c.w(x, y);
                let (ramp, t) = match mat {
                    Mat::Flint => flint(wx, wy),
                    _ => (if mat == Mat::Brick { r } else { Ramp::Brick }, flemish(wy, wx)),
                };
                let core = worn && (3..13).contains(&x) && fast(wx as u32 / 3, wy as u32 / 3, 0x636f) % 3 != 0;
                let (ramp, t) = if core {
                    let v = voronoi(wx, wy, 3, 0x7275);
                    (
                        Ramp::Stone,
                        if v.edge {
                            Tone::Shade
                        } else if v.dy < 0 {
                            Tone::Lift
                        } else {
                            Tone::Mid
                        },
                    )
                } else {
                    (ramp, t.step(1))
                };
                let (ramp, t) = if west_end && x == 0 {
                    (ramp, Tone::Light)
                } else if east_end && x >= CELL - 2 {
                    (ramp, if x == CELL - 1 { Tone::Deep } else { Tone::Shade })
                } else {
                    (ramp, t)
                };
                put(p, c, x, y, ramp.at(t), normal(0, -60), face_z(0, 0));
            }
        }
        return;
    }
    // The face: masonry, darker toward its foot, a brick out and a crack or two.
    let crack = (c.h >> 4) as i32 % 12 + 2;
    // One frame in four, never two side by side.
    let window = !west_end && !east_end && c.wx.rem_euclid(2) == 0 && (c.h >> 8) % 2 == 0;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (ramp, t) = ruin_stone(p, mat, r, wx, wy, (CELL - y) * 16);
            let t = if y > 10 { t.step(-1) } else { t };
            let hole = fast(wx as u32 / 3, wy as u32 / 4, 0x686f) % 23 == 0;
            let t = if hole { Tone::Deep } else { t };
            let cracked = (x == crack && y < 7) || (x == crack + 1 && (6..10).contains(&y));
            let (ramp, t) = if cracked { (r, Tone::Deep) } else { (ramp, t) };
            put(p, c, x, y, ramp.at(t), normal(0, FACE), face_z(y, 0));
        }
    }
    // An empty window frame: the opening dark through to the ground behind, a stone lintel over
    // it with its end broken off, the sill gone.
    if window {
        for y in 2..10 {
            for x in 5..11 {
                let jag = (y == 9 && fast(c.wx as u32, x as u32, 0x6a61) % 2 == 0) || (x == 10 && y < 4);
                if jag {
                    continue;
                }
                let t = if y == 2 || x == 5 { Tone::Deep } else { Tone::Shade };
                put(p, c, x, y, Ramp::Earth.at(t), FLAT, 1);
            }
        }
        for x in 4..11 {
            put(p, c, x, 1, cap.at(if x == 4 { Tone::Light } else { Tone::Lift }), normal(0, -40), face_z(1, 0));
        }
    }
    // Stones fallen against its foot.
    for y in CELL - 4..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let v = voronoi(wx, wy, 3, 0x6661);
            if v.id % 3 != 0 || v.edge && y < CELL - 2 {
                continue;
            }
            let t = if v.dy < 0 {
                Tone::Light
            } else if v.edge {
                Tone::Shade
            } else {
                Tone::Base
            };
            put(p, c, x, y, cap.at(t), normal(0, -30), face_z(y, 0) + 1);
        }
    }
    // The corners: the end of a broken wall, its stones' ends lit or shaded.
    for y in 0..CELL {
        if west_end {
            put(p, c, 0, y, r.at(Tone::Deep), FLAT, face_z(y, 0));
            put(p, c, 1, y, cap.at(Tone::Light), normal(-50, FACE), face_z(y, 0));
        }
        if east_end {
            put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, face_z(y, 0));
            put(p, c, CELL - 2, y, cap.at(Tone::Shade), normal(50, FACE), face_z(y, 0));
        }
    }
}

/// What a ruin shows past its wall cells, drawn by every chunk it falls in from the scratch (which
/// reaches past the chunk, so a ruin split across two chunks is one ruin): the broken tops of its
/// faces, standing up to eight px over them, uneven in runs of a few px, the masonry showing in
/// them and their tops lit; stones fallen inside it and round its feet; and a charred beam or two
/// on its floor.
pub(super) fn ruin_tops(p: &mut Painter, x0: i32, y0: i32) {
    use super::super::{CHUNK_CELLS, M};
    use jane_core::Tile;
    let m = M - 1;
    for cy in -m..CHUNK_CELLS + m {
        for cx in -m..CHUNK_CELLS + m {
            let k = Painter::at(cx, cy);
            let Some(h) = p.s.house[k].filter(|h| h.ruin) else { continue };
            let raw = |p: &Painter, dx: i32, dy: i32| p.s.raw[Painter::at(cx + dx, cy + dy)];
            let (wx0, wy0) = (x0 + cx, y0 + cy);
            let (bx, by) = (cx * CELL, cy * CELL);
            let l = h.look();
            let (r, mat) = match l.wall {
                Wall::Plaster(r) => (r, Mat::Plaster),
                Wall::Brick => (Ramp::Brick, Mat::Brick),
                Wall::Flint => (Ramp::Flint, Mat::Flint),
            };
            let cap = if mat == Mat::Brick { Ramp::Brick } else { Ramp::Stone };
            if raw(p, 0, 0) == Tile::HouseWall {
                // A face's broken top: only where the wall faces south and stands free above.
                if raw(p, 0, 1) == Tile::HouseWall || raw(p, 0, -1) == Tile::HouseWall {
                    continue;
                }
                let (west_end, east_end) = (raw(p, -1, 0) != Tile::HouseWall, raw(p, 1, 0) != Tile::HouseWall);
                let height = |x: i32| -> i32 {
                    let wx = wx0 * CELL + x;
                    let seg = wx.div_euclid(3);
                    let hh = [0, 1, 2, 3, 5, 6, 7, 8][(fast(seg as u32, wy0 as u32, 0x746f) % 8) as usize];
                    // A free end crumbles lower.
                    let to_end = match (west_end, east_end) {
                        (true, true) => x.min(CELL - 1 - x),
                        (true, false) => x,
                        (false, true) => CELL - 1 - x,
                        _ => CELL,
                    };
                    hh.min(to_end / 2 + 1)
                };
                for x in 0..CELL {
                    let hx = height(x);
                    let (hl, hr) =
                        (if x > 0 { height(x - 1) } else { hx }, if x < CELL - 1 { height(x + 1) } else { hx });
                    let wx = wx0 * CELL + x;
                    for k in 0..=hx {
                        let y = by - k;
                        let wy = wy0 * CELL - k;
                        let (ramp, t) = if k == hx {
                            (cap, Tone::Light)
                        } else if k + 1 == hx {
                            (cap, Tone::Lift)
                        } else {
                            ruin_stone(p, mat, r, wx, wy, 255)
                        };
                        // A step down beside it: its west end lit, its east in shade.
                        let t = if k > hl && k < hx {
                            Tone::High.min(t.step(1))
                        } else if k > hr && k < hx {
                            t.step(-1)
                        } else {
                            t
                        };
                        let n = if k >= hx.saturating_sub(1) { normal(0, -60) } else { normal(0, FACE) };
                        p.s.ly.put(bx + x, y, ramp.at(t), n, super::face_z(0, 0) + k);
                    }
                }
                continue;
            }
            // Inside the walls, and in front of them: fallen stones, thicker by the walls.
            let inside = h.rect.contains(wx0, wy0);
            let by_wall =
                raw(p, 0, -1) == Tile::HouseWall || raw(p, -1, 0) == Tile::HouseWall || raw(p, 1, 0) == Tile::HouseWall;
            if !(inside || by_wall) || raw(p, 0, 0).flags() & jane_core::tile::F_SOLID != 0 {
                continue;
            }
            let every = if by_wall { 3 } else { 6 };
            for y in 0..CELL {
                for x in 0..CELL {
                    let (wx, wy) = (wx0 * CELL + x, wy0 * CELL + y);
                    let v = voronoi(wx, wy, 3, 0x6661);
                    if v.id % every != 0 || v.edge {
                        continue;
                    }
                    let t = match v.dy.cmp(&0) {
                        core::cmp::Ordering::Less => Tone::Light,
                        core::cmp::Ordering::Greater => Tone::Shade,
                        core::cmp::Ordering::Equal => Tone::Base,
                    };
                    let ramp = if mat == Mat::Brick || v.id % 5 == 0 { Ramp::Brick } else { cap };
                    p.s.ly.put(bx + x, by + y, ramp.at(t), normal(0, -40), 2);
                }
            }
            // A charred beam fallen across the floor, in a cell or two of each ruin.
            if inside && fast(wx0 as u32, wy0 as u32, h.seed ^ 0x6265) % 11 == 0 {
                let rise = if fast(wx0 as u32, wy0 as u32, 0x7373) & 1 == 0 { 1 } else { -1 };
                for i in 0..13 {
                    let (x, y) = (bx + 1 + i, by + 7 + rise * (i / 4));
                    let char_ = fast(i as u32, wx0 as u32, 0x6368) % 3 == 0;
                    p.s.ly.put(x, y, Ramp::WoodDark.at(if char_ { Tone::Mid } else { Tone::Shade }), normal(0, -40), 3);
                    p.s.ly.put(x, y + 1, Ramp::WoodDark.at(Tone::Deep), normal(0, FACE), 2);
                    p.s.ly.put(x, y + 2, Ramp::Earth.at(Tone::Deep), FLAT, 1);
                }
            }
        }
    }
}

/// The cells of house `h`'s front (bits from its west end) that take a window: a cell in every
/// `period` from `phase` (and `off` more), never an end cell nor the door's; if that leaves the
/// front fewer than two, the phase that gives it the most.
fn windows(h: &House, l: &Look, off: i32) -> u64 {
    let w = h.rect.w.min(64);
    let mask = |phase: i32| {
        let mut m = 0u64;
        for rel in 1..w - 1 {
            let x = h.rect.x + rel;
            let door = h.door.is_some_and(|d| x == d || x == d + 1);
            if !door && (rel - phase - off).rem_euclid(i32::from(l.period)) == 0 {
                m |= 1 << rel;
            }
        }
        m
    };
    let own = mask(i32::from(l.phase));
    if own.count_ones() >= 2 {
        return own;
    }
    (0..i32::from(l.period)).map(mask).max_by_key(|m| m.count_ones()).unwrap_or(own)
}

/// Flemish bond at world px `(wx, wy)`: courses 4 px, a stretcher (7 px) and a header (3 px) in
/// turn along each, the headers burnt darker, the bed joint shadowed under each course and the
/// perpends a half-step; a brick here and there a tone of its own.
fn flemish(wx: i32, wy: i32) -> Tone {
    let course = wy.div_euclid(4);
    let yy = wy.rem_euclid(4);
    let xs = wx + (course & 1) * 6;
    let u = xs.rem_euclid(12);
    if yy == 3 {
        return Tone::Shade;
    }
    if u == 0 || u == 8 {
        return Tone::Mid;
    }
    let header = u > 8;
    let b = fast((xs.div_euclid(12) * 2 + i32::from(header)) as u32, course as u32, 0xb41c);
    let body = if header {
        if b % 4 == 0 { Tone::Base } else { Tone::Mid }
    } else {
        match b % 11 {
            0 => Tone::Mid,
            1 => Tone::Lift,
            _ => Tone::Base,
        }
    };
    if yy == 0 { body.step(1) } else { body }
}

/// Knapped flint at world px `(wx, wy)`: nodules round the jittered centres of a 4 px lattice,
/// blue-grey to near black, lit on their upper sides, a glint on one in three; lime mortar
/// between them.
fn flint(wx: i32, wy: i32) -> (Ramp, Tone) {
    let v = voronoi(wx, wy, 5, 0xf117);
    if v.edge {
        return (Ramp::Stone, Tone::Mid);
    }
    let body = match v.id % 7 {
        0 => Tone::Mid,
        1 => Tone::Lift,
        _ => Tone::Base,
    };
    let t = if v.dx == -1 && v.dy == -1 && v.id % 4 == 0 {
        Tone::Light
    } else if v.dy < -1 {
        body.step(1)
    } else if v.dy > 1 {
        body.step(-1)
    } else {
        body
    };
    (Ramp::Flint, t)
}

/// A casement window in `o`: panes in a frame under a stone lintel, on a lit sill with its
/// shadow under it; lit from within when `lit` (the glass in the emissive layer too), else a
/// pane catching the sky.
fn casement(p: &mut Painter, c: &Cell, o: Opening, lit: bool, frame: Ramp, z_at: &dyn Fn(i32) -> i32) {
    lintel(p, c, o, z_at);
    let (mx, my) = (o.x0 + o.w / 2, o.y0 + o.h / 2);
    for y in o.y0..=o.y1() {
        for x in o.x0..=o.x1() {
            let bar = x == mx || (o.w >= 10 && x == mx - 1);
            let edge = y == o.y0 || y == o.y1() || x == o.x0 || x == o.x1() || bar || y == my;
            if edge {
                let t = if x == o.x0 || y == o.y0 || y == my { Tone::Lift } else { Tone::Shade };
                put(p, c, x, y, frame.at(t), normal(0, FACE), z_at(y));
            } else {
                pane(p, c, x, y, o, lit, y < my, z_at);
            }
        }
    }
    sill(p, c, o, z_at);
}

/// A sash window in `o`: two sashes, each two panes wide and two high, over a meeting rail;
/// painted frames.
fn sash(p: &mut Painter, c: &Cell, o: Opening, lit: bool, frame: Ramp, z_at: &dyn Fn(i32) -> i32) {
    lintel(p, c, o, z_at);
    let mx = o.x0 + o.w / 2;
    let my = o.y0 + o.h / 2;
    let (q1, q3) = (o.y0 + (my - o.y0) / 2, my + (o.y1() - my) / 2 + 1);
    for y in o.y0..=o.y1() {
        for x in o.x0..=o.x1() {
            let edge = y == o.y0 || y == o.y1() || x == o.x0 || x == o.x1();
            let rail = y == my;
            let bar = x == mx || y == q1 || y == q3;
            if edge || rail {
                let t = if x == o.x0 || y == o.y0 || rail { Tone::Light } else { Tone::Mid };
                put(p, c, x, y, frame.at(t), normal(0, FACE), z_at(y));
            } else if bar {
                put(p, c, x, y, frame.at(Tone::Lift), normal(0, FACE), z_at(y));
            } else {
                pane(p, c, x, y, o, lit, y < my, z_at);
            }
        }
    }
    sill(p, c, o, z_at);
}

/// One px of glass: warm and in the emissive layer when lit, else the sky in it, its top panes
/// paler, a glint at a pane's top-left.
#[allow(clippy::too_many_arguments)]
fn pane(p: &mut Painter, c: &Cell, x: i32, y: i32, o: Opening, lit: bool, high: bool, z_at: &dyn Fn(i32) -> i32) {
    let (glass, t) = if lit {
        (Ramp::GlassLit, if high { Tone::Light } else { Tone::Base })
    } else if y == o.y0 + 1 && (x == o.x0 + 1 || x == o.x0 + o.w / 2 + 1) {
        (Ramp::Glass, Tone::High)
    } else {
        (Ramp::Glass, if high { Tone::Base } else { Tone::Shade })
    };
    put(p, c, x, y, glass.at(t), normal(0, UNIT * 9 / 10), z_at(y));
    if lit {
        p.s.ly.glow(c.px + x, c.py + y, glass.at(t));
    }
}

/// A stone lintel over an opening.
fn lintel(p: &mut Painter, c: &Cell, o: Opening, z_at: &dyn Fn(i32) -> i32) {
    let stone = Ramp::Stone;
    for x in o.x0 - 1..=o.x1() + 1 {
        let t = if x == o.x0 - 1 { Tone::Light } else { Tone::Lift };
        if o.y0 >= 2 {
            put(p, c, x, o.y0 - 2, stone.at(t), normal(0, FACE - 40), z_at(o.y0 - 2));
        }
        put(p, c, x, o.y0 - 1, stone.at(Tone::Mid), normal(0, FACE), z_at(o.y0 - 1));
    }
}

/// A lit stone sill under an opening, its shadow under it.
fn sill(p: &mut Painter, c: &Cell, o: Opening, z_at: &dyn Fn(i32) -> i32) {
    let stone = Ramp::Stone;
    for x in o.x0 - 1..=o.x1() + 1 {
        let t = if x < o.x1() + 1 { Tone::High } else { Tone::Base };
        put(p, c, x, o.y1() + 1, stone.at(t), normal(0, 20), z_at(o.y1() + 1));
        step(p, c, x, o.y1() + 2, -2);
        step(p, c, x + 1, o.y1() + 3, -1);
    }
}

/// A bay on the ground floor: three lights, the side ones canted away and darker, under a lead
/// roof, on a panelled stone base; it stands a px or two proud of the face.
fn bay(p: &mut Painter, c: &Cell, lit: bool, z_at: &dyn Fn(i32) -> i32) {
    let (x0, x1, y0, y1) = (1, 14, 3, 12);
    let frame = Ramp::Limewash;
    // The lead roof: a sloped cap over it, lit along its ridge.
    for (y, a, b, t) in [(0, 3, 12, Tone::Light), (1, 2, 13, Tone::Base), (2, 1, 14, Tone::Shade)] {
        for x in a..=b {
            put(p, c, x, y, Ramp::Iron.at(t), normal(0, -40 + y * 30), z_at(y));
        }
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let side = x <= 3 || x >= 12;
            let mullion = x == x0 || x == x1 || x == 4 || x == 11 || x == 7 || x == 8 || y == y0 || y == y1;
            let transom = y == y0 + 3;
            let z = z_at(y);
            if mullion || transom {
                let t = if x == x0 || y == y0 || transom { Tone::Light } else { Tone::Mid };
                put(p, c, x, y, frame.at(t), normal(if side { if x < 8 { -50 } else { 50 } } else { 0 }, FACE), z);
            } else {
                let (glass, t) = if lit {
                    (Ramp::GlassLit, if side { Tone::Base } else { Tone::Light })
                } else if side {
                    (Ramp::Glass, Tone::Shade)
                } else if y == y0 + 1 {
                    (Ramp::Glass, Tone::High)
                } else {
                    (Ramp::Glass, if y < y0 + 4 { Tone::Lift } else { Tone::Base })
                };
                put(p, c, x, y, glass.at(t), normal(0, UNIT * 9 / 10), z);
                if lit {
                    p.s.ly.glow(c.px + x, c.py + y, glass.at(t));
                }
            }
        }
    }
    // The base, a panel of stone, its shadow on the ground under it.
    for x in x0..=x1 {
        put(p, c, x, y1 + 1, Ramp::Stone.at(Tone::High), normal(0, 20), z_at(y1 + 1));
        put(p, c, x, y1 + 2, Ramp::Stone.at(if x == x0 { Tone::Lift } else { Tone::Base }), normal(0, FACE), 3);
    }
}

/// A lancet: a tall window with a pointed head, leaded in diamonds, in a stone surround.
fn lancet(p: &mut Painter, c: &Cell, lit: bool, z_at: &dyn Fn(i32) -> i32) {
    let (x0, x1, y0, y1) = (5, 10, 1, 13);
    for y in y0 - 1..=y1 + 1 {
        for x in x0 - 1..=x1 + 1 {
            // The pointed head: the opening narrows over its top three rows.
            let inset = (y0 + 2 - y).max(0);
            if x < x0 - 1 + inset || x > x1 + 1 - inset {
                continue;
            }
            let rim = y == y0 - 1 || y == y1 + 1 || x == x0 - 1 + inset || x == x1 + 1 - inset;
            if rim {
                let t = if x <= x0 || y == y0 - 1 { Tone::Light } else { Tone::Mid };
                put(p, c, x, y, Ramp::Stone.at(t), normal(0, FACE - 20), z_at(y));
                continue;
            }
            let lead = (x + y).rem_euclid(3) == 0 || (x - y).rem_euclid(3) == 0;
            let (glass, t) = match (lit, lead) {
                (_, true) => (Ramp::Iron, Tone::Shade),
                (true, false) => (Ramp::GlassLit, if y < 6 { Tone::Light } else { Tone::Base }),
                (false, false) => (Ramp::Glass, if y < 5 { Tone::Lift } else { Tone::Shade }),
            };
            put(p, c, x, y, glass.at(t), normal(0, UNIT * 9 / 10), z_at(y));
            if lit && !lead {
                p.s.ly.glow(c.px + x, c.py + y, glass.at(t));
            }
        }
    }
}

/// Shutters either side of an opening, in the door's paint: louvred, lit on the left, a dark
/// line where each meets the wall.
fn shutters(p: &mut Painter, c: &Cell, o: Opening, paint: Ramp, z_at: &dyn Fn(i32) -> i32) {
    for (xa, xb) in [(o.x0 - 4, o.x0 - 2), (o.x1() + 2, o.x1() + 4)] {
        for y in o.y0 - 1..=o.y1() {
            for x in xa.max(0)..=xb.min(CELL - 1) {
                let t = if x == xa || y == o.y0 - 1 {
                    Tone::Light
                } else if x == xb || y == o.y1() {
                    Tone::Shade
                } else if (y - o.y0).rem_euclid(2) == 1 {
                    Tone::Mid
                } else {
                    Tone::Base
                };
                put(p, c, x, y, paint.at(t), normal(if x == xa { -40 } else { 0 }, FACE), z_at(y));
            }
        }
    }
}

/// A window box under an opening's sill: a painted trough, and in it a low mat of leaf with
/// blooms of one colour in clusters, a trail of leaf over its front.
fn window_box(p: &mut Painter, c: &Cell, o: Opening, h: u32, z_at: &dyn Fn(i32) -> i32) {
    let bloom = [Ramp::ClothRed, Ramp::Bloom, Ramp::ClothMustard, Ramp::ClothLinen][(h % 4) as usize];
    let y = o.y1() + 2;
    for x in o.x0 - 1..=o.x1() + 1 {
        let t = if x == o.x0 - 1 { Tone::Lift } else { Tone::Base };
        put(p, c, x, y, Ramp::WoodDark.at(t), normal(0, FACE), z_at(y));
        if y + 1 < CELL {
            put(p, c, x, y + 1, Ramp::WoodDark.at(Tone::Shade), normal(0, FACE), z_at(y + 1));
        }
    }
    for x in o.x0 - 1..=o.x1() + 1 {
        let k = fast(x as u32, 0, h);
        // Leaf on the sill and over the box's lip.
        let leaf = if k & 1 == 0 { Tone::Base } else { Tone::Mid };
        put(p, c, x, o.y1() + 1, Ramp::Shrub.at(leaf), normal(0, -20), z_at(o.y1()));
        if k % 5 == 0 && y + 1 < CELL {
            put(p, c, x, y + 1, Ramp::Shrub.at(Tone::Shade), normal(0, FACE), z_at(y + 1));
        }
    }
    // Blooms in clusters of two or three px, the top one lit.
    let mut x = o.x0 + (h >> 4) as i32 % 2;
    while x <= o.x1() {
        put(p, c, x, o.y1(), bloom.at(Tone::Light), normal(-20, -30), z_at(o.y1()));
        put(p, c, x + 1, o.y1() + 1, bloom.at(Tone::Base), normal(20, 0), z_at(o.y1()));
        if fast(x as u32, 1, h) & 1 == 0 {
            put(p, c, x, o.y1() + 1, bloom.at(Tone::Mid), normal(0, 20), z_at(o.y1()));
        }
        x += 3 + (fast(x as u32, 2, h) % 2) as i32;
    }
}

/// The house's front in world px: west, east, the eave's row, the foot.
fn front(h: &House) -> (i32, i32, i32, i32) {
    (h.rect.x * CELL, h.rect.right() * CELL, h.eave * CELL, h.rect.bottom() * CELL)
}

/// Ivy up one corner of the front from its foot to the eave, spreading under it: leaf in clumps
/// round a 3 px lattice, each lit on its upper left, ragged at its edge. On one ivied house in
/// two it is Virginia creeper, gone crimson in October.
fn ivy(p: &mut Painter, c: &Cell, h: &House, z_at: &dyn Fn(i32) -> i32) {
    let (fx0, fx1, fy0, fy1) = front(h);
    let hs = h32(h.seed, 0, 0x1717);
    let west = hs & 1 == 0;
    let creeper = hs & 2 == 0;
    let tall = fy1 - fy0;
    let w0 = 14 + (hs >> 4) as i32 % 14;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let hx = if west { wx - fx0 } else { fx1 - 1 - wx };
            let hy = fy1 - 1 - wy;
            if hx < 0 || hy < 3 || hy > tall - 3 {
                continue;
            }
            let wobble = (fast((hy / 3) as u32, 0, hs) % 7) as i32 - 3;
            let spread = if hy > tall - 14 { (hy - (tall - 14)) * 3 } else { 0 };
            let reach = w0 - hy / 4 + wobble + spread;
            if hx >= reach {
                continue;
            }
            let v = voronoi(wx, wy, 3, hs);
            if hx > reach - 4 && v.id % 3 == 0 {
                continue;
            }
            let ramp = match (creeper, v.id % 7) {
                (true, 0 | 1) => Ramp::LeafOak,
                (true, 2) => Ramp::LeafBeech,
                (true, _) => Ramp::ClothRed,
                (false, 0) => Ramp::LeafOlive,
                (false, _) => Ramp::LeafDeep,
            };
            let s = -(v.dx * 2 + v.dy * 3) - (v.dx * v.dx + v.dy * v.dy);
            let t = if v.edge {
                Tone::Shade
            } else if s > 3 {
                Tone::Light
            } else if s > -1 {
                Tone::Base
            } else if s > -6 {
                Tone::Mid
            } else {
                Tone::Shade
            };
            put(p, c, x, y, ramp.at(t), normal((v.dx * 25).clamp(-60, 60), FACE - 20), z_at(y));
        }
    }
}

/// Wisteria: a gnarled trunk up one end of the front, a branch along under the eave, racemes of
/// lilac hanging from it among its leaves.
fn wisteria(p: &mut Painter, c: &Cell, h: &House, z_at: &dyn Fn(i32) -> i32) {
    let (fx0, fx1, fy0, fy1) = front(h);
    let hs = h32(h.seed, 0, 0x3157);
    let trunk = if hs & 1 == 0 { fx0 + 5 } else { fx1 - 7 };
    let branch = fy0 + 7;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            if wx < fx0 + 3 || wx >= fx1 - 3 || wy < fy0 + 4 || wy >= fy1 - 3 {
                continue;
            }
            // The trunk twists as it climbs.
            let tw = ((wy / 5) & 1) * 2 - 1;
            let along = branch + (fast((wx / 7) as u32, 0, hs) % 2) as i32;
            let (ramp, t) = if wy >= branch && (wx - trunk - tw).abs() <= 1 {
                (Ramp::Bark, if wx - trunk - tw < 0 { Tone::Lift } else { Tone::Shade })
            } else if wy == along || wy == along - 1 {
                (Ramp::Bark, if wy == along - 1 { Tone::Base } else { Tone::Shade })
            } else {
                // A raceme every 5 or 6 px, hanging 4 to 8 px, tapering.
                let k = (wx - fx0).div_euclid(6);
                let hk = fast(k as u32, 1, hs);
                let rx = fx0 + k * 6 + (hk % 3) as i32;
                let len = 4 + (hk >> 4) as i32 % 5;
                let dy = wy - (along + 1);
                if (0..len).contains(&dy) && (wx - rx).abs() <= (len - dy + 1) / 3 {
                    let t = if dy == 0 || wx < rx {
                        Tone::Light
                    } else if dy > len * 2 / 3 {
                        Tone::Mid
                    } else {
                        Tone::Base
                    };
                    (Ramp::Wisteria, t)
                } else if (-3..0).contains(&dy) && fast(wx as u32 / 2, wy as u32, hs) % 3 == 0 {
                    (Ramp::LeafOlive, if dy == -3 { Tone::Light } else { Tone::Base })
                } else {
                    continue;
                }
            };
            put(p, c, x, y, ramp.at(t), normal(0, FACE - 20), z_at(y));
        }
    }
}

/// A climbing rose round the door: its stems up either side of the frame and an arch over it,
/// leaf in clumps, a bloom in one clump in five.
fn rose(p: &mut Painter, c: &Cell, h: &House, z_at: &dyn Fn(i32) -> i32) {
    let (fx0, _, _, fy1) = front(h);
    let hs = h32(h.seed, 0, 0x2053);
    let bloom = [Ramp::ClothRed, Ramp::Bloom, Ramp::ClothCream][(hs % 3) as usize];
    // Round the door, or round the west end's first window when no door is set in this front.
    let dx = h.door.map_or(fx0 + 16, |d| d * CELL);
    let top = fy1 - 37;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let rx = wx - dx;
            let stem = wy >= top && wy < fy1 - 3 && ((-3..=1).contains(&rx) || (30..=34).contains(&rx));
            let arch_y = top - 1 - (16 - (rx - 16).abs()).max(0) / 5;
            let arch = (-3..=34).contains(&rx) && (arch_y - 3..=arch_y).contains(&wy);
            if !(stem || arch) {
                continue;
            }
            let v = voronoi(wx, wy, 3, hs);
            if v.edge && v.id % 2 == 0 {
                continue;
            }
            let (ramp, t) = if v.id % 5 == 0 {
                (bloom, if v.dx < 0 && v.dy < 0 { Tone::Light } else { Tone::Base })
            } else {
                let s = -(v.dx * 2 + v.dy * 3);
                (
                    Ramp::Shrub,
                    if s > 2 {
                        Tone::Lift
                    } else if s > -3 {
                        Tone::Base
                    } else {
                        Tone::Shade
                    },
                )
            };
            put(p, c, x, y, ramp.at(t), normal(0, FACE - 20), z_at(y));
        }
    }
}

/// Over the door, a painted enamel number or a brass nameplate.
fn plate(p: &mut Painter, c: &Cell, h: &House, l: &Look, rows: i32, z_at: &dyn Fn(i32) -> i32) {
    let Some(d) = h.door else { return };
    if rows < 3 {
        return;
    }
    let (_, _, fy0, _) = front(h);
    let (w, ht) = if l.nameplate { (8, 3) } else { (5, 4) };
    let (px0, py0) = (d * CELL + CELL - w / 2, fy0 + 7);
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (u, v) = (wx - px0, wy - py0);
            if !(0..w).contains(&u) || !(0..ht).contains(&v) {
                continue;
            }
            let ix = if l.nameplate {
                // Brass, lit along its top, two lines of letters cut in it.
                match (u, v) {
                    (_, 0) => Ramp::Brass.at(Tone::Light),
                    (0 | 7, _) => Ramp::Brass.at(Tone::Mid),
                    (2..=5, 1) if u % 2 == 0 => Ramp::ClothBlack.at(Tone::Base),
                    _ => Ramp::Brass.at(Tone::Base),
                }
            } else {
                // White enamel with a navy rim and its number.
                let rim = u == 0 || u == w - 1 || v == 0 || v == ht - 1;
                let corner = (u == 0 || u == w - 1) && (v == 0 || v == ht - 1);
                if corner {
                    continue;
                }
                if rim {
                    Ramp::ClothNavy.at(Tone::Base)
                } else if u == 2 || (u == 1 && v == 1 && h.seed & 1 == 0) {
                    Ramp::ClothNavy.at(Tone::Deep)
                } else {
                    Ramp::Limewash.at(Tone::Light)
                }
            };
            put(p, c, x, y, ix, normal(0, FACE - 30), z_at(y));
        }
    }
}
