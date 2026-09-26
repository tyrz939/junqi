//! Pass 4: the hard tiles, each by its own painter per cell. Walls and cliffs show a face where
//! their south is open and a top otherwise; out of doors a wide block of wall is a building seen
//! from above, its roof. Roofs lay courses under a ridge and over an eave; house walls take an
//! eave's shadow, a plinth and a window in the middle course every few cells, lit at night.
//! Floors, rails, boards, sills and glass lay their own courses. Then the soft, cool contact
//! shade at the foot of every raised thing and in the inside corners where two walls meet.
//!
//! Edges are drawn in the material's own darkest tone, never near-black (ART.md §3). Heights: a
//! top stands at the style's `rise`; a face climbs from 1 at its foot to the top over every row
//! of the face; a roof falls from the ridge to the eave.

use jane_core::Tile;
use jane_core::angle::iatan2;
use jane_data::{TileGroup, TilePattern as P};

use super::{CELL, CHUNK_CELLS, NONE, Painter, Style, TileSource, fast, salt};
use crate::canvas::{FLAT, Normal, UNIT, normal};
use crate::hash::{below, h32};
use crate::palette::{Ix, Ramp, Tone};

/// A cell being painted.
#[derive(Clone, Copy)]
struct Cell {
    /// Chunk-local cell.
    cx: i32,
    cy: i32,
    /// Chunk-local px of its top-left.
    px: i32,
    py: i32,
    /// World cell.
    wx: i32,
    wy: i32,
    /// Its hash.
    h: u32,
    st: Style,
}

/// A face's normal: south, up a little.
const FACE: i32 = UNIT * 8 / 10;

impl Cell {
    /// World px of local px `x, y`.
    fn w(&self, x: i32, y: i32) -> (i32, i32) {
        (self.wx * CELL + x, self.wy * CELL + y)
    }
}

/// The tile `(dx, dy)` cells from the cell.
fn nb(p: &Painter, c: &Cell, dx: i32, dy: i32) -> Tile {
    p.s.raw[Painter::at(c.cx + dx, c.cy + dy)]
}

/// The style `(dx, dy)` cells from the cell, its paint applied.
fn nst(p: &Painter, c: &Cell, dx: i32, dy: i32) -> Style {
    *p.style_k(Painter::at(c.cx + dx, c.cy + dy))
}

fn put(p: &mut Painter, c: &Cell, x: i32, y: i32, ix: Ix, n: Normal, z: i32) {
    p.s.ly.put(c.px + x, c.py + y, ix, n, z);
}

fn step(p: &mut Painter, c: &Cell, x: i32, y: i32, s: i32) {
    p.s.ly.step(c.px + x, c.py + y, s);
}

/// A face's height at row `y` of a face `rows` cells tall whose top is `top` px, the cell being
/// `k` rows from the face's foot (0 is the bottom cell).
fn face_z(top: i32, y: i32, k: i32, rows: i32) -> i32 {
    let from_foot = k * CELL + (CELL - y);
    (from_foot * top / (rows * CELL)).max(1)
}

/// How many cells up (`dy = -1`) or down (`dy = 1`) the run of `same` goes from the cell, three
/// at most (the scratch reaches four beyond the chunk, so every chunk counts the same).
fn run(p: &Painter, c: &Cell, dy: i32, same: impl Fn(&Style) -> bool) -> i32 {
    let mut n = 0;
    while n < 3 && same(&nst(p, c, 0, dy * (n + 1))) {
        n += 1;
    }
    n
}

/// Whether a style stands and casts: a raised wall or roof that is not the void.
fn raised(s: &Style) -> bool {
    s.raised() && matches!(s.row.group, TileGroup::Wall | TileGroup::Roof) && s.tile != Tile::Void
}

pub(super) fn paint(p: &mut Painter, src: &impl TileSource, x0: i32, y0: i32, seed: u32) {
    let outdoor = src.outdoor();
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let k = Painter::at(cx, cy);
            if p.s.surf[k] != NONE {
                continue;
            }
            let t = p.s.paint[k];
            let st = if t == p.s.raw[k] { *p.style_k(k) } else { *p.styles.tile(t) };
            let (wx, wy) = (x0 + cx, y0 + cy);
            let c = Cell {
                cx,
                cy,
                px: cx * CELL,
                py: cy * CELL,
                wx,
                wy,
                h: h32(wx as u32, wy as u32, seed ^ salt::CELL),
                st,
            };
            match st.row.pattern {
                P::Void => fill(p, &c, st.ramp.at(Tone::Deep), 1),
                P::Block | P::Rock => wall(p, &c, outdoor),
                P::Cliff => cliff(p, &c),
                P::RoofTile | P::Slate | P::Thatch => {
                    roof(p, &c, st.ramp, st.row.pattern, |s| s.row.group == TileGroup::Roof, 0);
                }
                P::Plaster | P::Brick => house_wall(p, &c, seed),
                P::Hedge => hedge(p, &c),
                P::Slabs => slabs(p, &c),
                P::Boards => boards(p, &c),
                P::RockFloor => rock_floor(p, &c),
                P::Sill => sill(p, &c),
                P::Glass => glass(p, &c),
                P::Rail => rail(p, &c),
                P::Boardwalk => boardwalk(p, &c),
                _ => fill(p, &c, st.ramp.at(Tone::Base), i32::from(st.row.rise)),
            }
        }
    }
    contact(p, x0, y0, seed);
}

/// The contact shade of raised things on what lies at their foot: four rows under a wall, a roof
/// or a hedge to the north, two columns beside one to the west or east, and a deeper wedge in an
/// inside corner. Tone steps down the material's own ramp, which runs cool (ART.md §3.1); the
/// last row fades in 2 px clusters, never a checker.
fn contact(p: &mut Painter, x0: i32, y0: i32, seed: u32) {
    for cy in 0..CHUNK_CELLS {
        for cx in 0..CHUNK_CELLS {
            let k = Painter::at(cx, cy);
            let own = *p.style_k(k);
            if (own.raised() && own.row.group != TileGroup::Flora) || own.row.wall_like {
                continue;
            }
            let at = |dx: i32, dy: i32| *p.style_k(Painter::at(cx + dx, cy + dy));
            let (n, w, e) = (raised(&at(0, -1)), raised(&at(-1, 0)), raised(&at(1, 0)));
            let (px, py) = (cx * CELL, cy * CELL);
            let fade = |x: i32, y: i32| {
                fast(((x0 * CELL + px + x) >> 1) as u32, ((y0 * CELL + py + y) >> 1) as u32, seed) & 1 == 0
            };
            for y in 0..CELL {
                for x in 0..CELL {
                    let mut s = 0;
                    if n {
                        s = match y {
                            0 | 1 => -2,
                            2 | 3 => -1,
                            4 if fade(x, y) => -1,
                            _ => 0,
                        };
                    }
                    if w && x < 3 {
                        s = s.min(if x == 0 { -2 } else { -1 });
                    }
                    if e && x > CELL - 3 {
                        s = s.min(-1);
                    }
                    if n && w && x + y < 7 {
                        s -= 1;
                    }
                    if s != 0 {
                        p.s.ly.step(px + x, py + y, s);
                    }
                }
            }
        }
    }
}

fn fill(p: &mut Painter, c: &Cell, ix: Ix, z: i32) {
    for y in 0..CELL {
        for x in 0..CELL {
            put(p, c, x, y, ix, FLAT, z);
        }
    }
}

/// A tone for a stone of a course: its own hash picks one of three.
fn stone_tone(h: u32) -> Tone {
    match h % 7 {
        0 | 1 => Tone::Mid,
        2..=5 => Tone::Base,
        _ => Tone::Lift,
    }
}

/// Walls: a face where the south is open, a top otherwise, a roof for a wide block out of doors.
fn wall(p: &mut Painter, c: &Cell, outdoor: bool) {
    let wl = |p: &Painter, dx: i32, dy: i32| nst(p, c, dx, dy).row.wall_like;
    let south_open = !wl(p, 0, 1);
    let thin = (!wl(p, -1, 0) && !wl(p, 1, 0)) || (!wl(p, 0, -1) && !wl(p, 0, 1));
    let top = i32::from(c.st.row.rise);
    let r = c.st.ramp;
    if outdoor && !south_open && !thin && c.st.row.pattern == P::Block {
        // The roof of a works, a library, a school: slate gone dark with soot.
        roof(p, c, Ramp::Slate, P::Slate, |s| s.row.wall_like, -1);
        return;
    }
    if south_open {
        for y in 0..CELL {
            for x in 0..CELL {
                let (wx, wy) = c.w(x, y);
                let z = face_z(top, y, 0, 1);
                let (ix, n) = if c.st.row.pattern == P::Rock {
                    rock_face(r, wx, wy)
                } else {
                    // Courses of dressed stone, `detail` px tall with a px of mortar; each stone lit
                    // along its top, shaded along its foot.
                    let pitch = i32::from(c.st.row.detail).clamp(3, 8) + 1;
                    let course = wy.div_euclid(pitch);
                    let yy = wy.rem_euclid(pitch);
                    let xs = wx + (course & 1) * 6;
                    let block = xs.div_euclid(12);
                    let lx = xs.rem_euclid(12);
                    if yy == pitch - 1 || lx == 11 {
                        (r.at(Tone::Deep), normal(0, FACE + 10))
                    } else {
                        let t = stone_tone(fast(block as u32, course as u32, c.h));
                        let t = if yy == 0 {
                            t.step(1)
                        } else if yy == pitch - 2 {
                            t.step(-1)
                        } else {
                            t
                        };
                        (r.at(t), normal(if lx == 0 { -30 } else { 0 }, if yy == 0 { FACE - 40 } else { FACE }))
                    }
                };
                put(p, c, x, y, ix, n, z);
            }
        }
        // The face darkens toward its foot; its top edge catches the light.
        for x in 0..CELL {
            put(p, c, x, 0, if wl(p, 0, -1) { r.at(Tone::Light) } else { r.at(Tone::Deep) }, FLAT, top);
            put(p, c, x, 1, r.at(Tone::Lift), normal(0, -40), top);
            for y in 11..CELL - 2 {
                step(p, c, x, y, -1);
            }
            step(p, c, x, CELL - 2, -2);
            step(p, c, x, CELL - 1, -3);
        }
        if !wl(p, -1, 0) {
            for y in 0..CELL {
                put(p, c, 0, y, r.at(Tone::Deep), FLAT, face_z(top, y, 0, 1));
            }
        }
        if !wl(p, 1, 0) {
            for y in 0..CELL {
                put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, face_z(top, y, 0, 1));
            }
        }
        return;
    }
    // The top: dark and calm, a crack now and then, lit where it meets open floor so a block
    // reads as one.
    for y in 0..CELL {
        for x in 0..CELL {
            put(p, c, x, y, r.at(Tone::Shade), FLAT, top);
        }
    }
    if c.h % 5 == 0 {
        let (mut x, mut y) = (2 + (c.h >> 4) as i32 % 10, 3 + (c.h >> 8) as i32 % 8);
        for i in 0..5 {
            put(p, c, x, y, r.at(Tone::Deep), FLAT, top);
            put(p, c, x, y - 1, r.at(Tone::Mid), FLAT, top);
            x += 1;
            y += i32::from((c.h >> (12 + i)) & 1 == 1);
        }
    }
    if !wl(p, 0, -1) {
        for x in 0..CELL {
            put(p, c, x, 0, r.at(Tone::Light), normal(0, -60), top);
            put(p, c, x, 1, r.at(Tone::Base), FLAT, top);
        }
    }
    if !wl(p, -1, 0) {
        for y in 0..CELL {
            put(p, c, 0, y, r.at(Tone::Lift), normal(-60, 0), top);
        }
    }
    if !wl(p, 1, 0) {
        for y in 0..CELL {
            put(p, c, CELL - 1, y, r.at(Tone::Deep), normal(60, 0), top);
        }
    }
}

/// Rough rock, ridged top to bottom: the column decides the ridge, so stacked cells line up; a
/// ridge is lit on its left edge and shaded on its right, in runs of 2 or 3 px.
fn rock_face(r: Ramp, wx: i32, wy: i32) -> (Ix, Normal) {
    // Ridges of 3 to 6 px: a ridge's index and px within it, by blocks of 16 px split by hash.
    let block = wx.div_euclid(16);
    let lx = wx.rem_euclid(16);
    let h = fast(block as u32, 77, 0);
    let c1 = 4 + (h % 3) as i32;
    let c2 = c1 + 4 + (h >> 4) as i32 % 3;
    let (start, end) = if lx < c1 {
        (0, c1)
    } else if lx < c2 {
        (c1, c2)
    } else {
        (c2, 16)
    };
    let ridge = fast((block * 4 + start) as u32, 78, 0);
    let body = match ridge % 3 {
        0 => Tone::Mid,
        1 => Tone::Base,
        _ => Tone::Lift,
    };
    // A ledge across the face every so often, lit on top.
    let ledge = (wy + (ridge >> 4) as i32 % 5).rem_euclid(11);
    let (t, nx) = if lx == start {
        (body.step(1), -60)
    } else if lx == end - 1 {
        (body.step(-2), 60)
    } else if lx == end - 2 {
        (body.step(-1), 30)
    } else {
        (body, 0)
    };
    let t = match ledge {
        0 => t.step(1),
        1 => t.step(-1),
        _ => t,
    };
    (r.at(t), normal(nx, FACE))
}

/// Cliffs as Link's Awakening draws them: a rocky top with a lit lip, and where the rock drops to
/// open ground a face two cells tall, ridged top to bottom, dark at its foot.
fn cliff(p: &mut Painter, c: &Cell) {
    let open = |p: &Painter, dx: i32, dy: i32| {
        let t = nb(p, c, dx, dy);
        t != Tile::Cliff && !nst(p, c, dx, dy).row.wall_like
    };
    let top = i32::from(c.st.row.rise);
    let (rtop, rface) = (c.st.ramp, c.st.accent.unwrap_or(c.st.ramp));
    let lower = open(p, 0, 1);
    let upper = !lower && nb(p, c, 0, 1) == Tile::Cliff && open(p, 0, 2);
    if lower || upper {
        let k = i32::from(upper);
        for y in 0..CELL {
            for x in 0..CELL {
                let (wx, wy) = c.w(x, y);
                let (ix, n) = rock_face(rface, wx, wy);
                put(p, c, x, y, ix, n, face_z(top, y, k, 2));
            }
        }
        // The lip where the top breaks over into the face: not between the face's two rows.
        if !(nb(p, c, 0, -1) == Tile::Cliff && is_face(p, c, 0, -1)) {
            for x in 0..CELL {
                put(p, c, x, 0, rtop.at(Tone::Deep), FLAT, top);
                put(p, c, x, 1, rtop.at(Tone::High), normal(0, -60), top);
                put(p, c, x, 2, rtop.at(Tone::Light), normal(0, -30), top);
                step(p, c, x, 3, 1);
            }
        }
        if lower {
            for x in 0..CELL {
                step(p, c, x, CELL - 4, -1);
                step(p, c, x, CELL - 3, -2);
                put(p, c, x, CELL - 2, rface.at(Tone::Deep), normal(0, FACE), 2);
                put(p, c, x, CELL - 1, rface.at(Tone::Deep), FLAT, 1);
            }
        }
        if open(p, -1, 0) {
            for y in 0..CELL {
                put(p, c, 0, y, rface.at(Tone::Deep), FLAT, face_z(top, y, k, 2));
            }
        }
        if open(p, 1, 0) {
            for y in 0..CELL {
                put(p, c, CELL - 1, y, rface.at(Tone::Deep), FLAT, face_z(top, y, k, 2));
            }
        }
        return;
    }
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            // The top is broken rock: slabs round jittered centres, each a tone of its own, lit
            // along its upper side and shaded along its lower, a crack where two slabs meet.
            let v = voronoi(wx, wy, 8, 0x0c11);
            let body = match v.id % 5 {
                0 => Tone::Mid,
                1 => Tone::Lift,
                _ => Tone::Base,
            };
            let (t, n) = if v.edge {
                (Tone::Shade, FLAT)
            } else if v.dy < -2 {
                (body.step(1), normal(0, -40))
            } else if v.dy > 2 {
                (body.step(-1), normal(0, 40))
            } else {
                (body, FLAT)
            };
            put(p, c, x, y, rtop.at(t), n, top);
        }
    }
    let h = c.h;
    if h & 3 == 0 {
        // A loose stone: lit on top, its shadow under it.
        let (x, y) = (1 + (h >> 2) as i32 % 11, 2 + (h >> 6) as i32 % 11);
        for i in 0..3 {
            put(p, c, x + i, y, rtop.at(Tone::Light), normal(0, -50), top + 1);
            put(p, c, x + i, y + 1, rtop.at(Tone::Mid), normal(0, 50), top + 1);
            step(p, c, x + i + 1, y + 2, -1);
        }
    } else if h & 31 == 10 {
        for (dx, dy, t) in
            [(0, 1, Tone::Shade), (0, 0, Tone::Light), (2, 1, Tone::Mid), (2, 0, Tone::High), (1, 1, Tone::Base)]
        {
            put(p, c, 5 + dx, 5 + dy, Ramp::Turf.at(t), FLAT, top + 2);
        }
    }
    if open(p, 0, -1) {
        for x in 0..CELL {
            put(p, c, x, 0, rtop.at(Tone::Deep), FLAT, top);
            put(p, c, x, 1, rtop.at(Tone::Light), normal(0, -70), top);
        }
    }
    if open(p, -1, 0) {
        for y in 0..CELL {
            put(p, c, 0, y, rtop.at(Tone::Deep), FLAT, top);
            put(p, c, 1, y, rtop.at(Tone::Light), normal(-70, 0), top);
        }
    }
    if open(p, 1, 0) {
        for y in 0..CELL {
            put(p, c, CELL - 1, y, rtop.at(Tone::Deep), FLAT, top);
            put(p, c, CELL - 2, y, rtop.at(Tone::Shade), normal(70, 0), top);
        }
    }
}

/// Whether the cliff cell `(dx, dy)` away shows a face.
fn is_face(p: &Painter, c: &Cell, dx: i32, dy: i32) -> bool {
    let open = |t: Tile, s: &Style| t != Tile::Cliff && !s.row.wall_like;
    let (t1, s1) = (nb(p, c, dx, dy + 1), nst(p, c, dx, dy + 1));
    if t1 != Tile::Cliff {
        return open(t1, &s1);
    }
    let (t2, s2) = (nb(p, c, dx, dy + 2), nst(p, c, dx, dy + 2));
    open(t2, &s2)
}

/// Px from a roof's north edge to its ridge: above it the north slope, lit and foreshortened;
/// below it the south slope down to the eave.
const RIDGE: i32 = 10;

/// Roofs: courses of slate, tile or thatch, a north slope over a ridge cap over the south slope,
/// an eave along the bottom, a verge down each open side. `same` says what continues the roof;
/// `shift` darkens it. The ridge's place is counted from the roof's north edge, a few cells at
/// most, so every chunk agrees where it is.
fn roof(p: &mut Painter, c: &Cell, r: Ramp, pat: P, same: impl Fn(&Style) -> bool + Copy, shift: i32) {
    let top = i32::from(c.st.row.rise.max(28));
    let up = run(p, c, -1, same);
    let eave = !same(&nst(p, c, 0, 1));
    let west = !same(&nst(p, c, -1, 0));
    let east = !same(&nst(p, c, 1, 0));
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            // Px below the ridge (negative: on the north slope).
            let d = up * CELL + y - RIDGE;
            let z = if d < 0 { top + d / 2 } else { top - d.min(40) * 16 / 40 };
            let body = match pat {
                P::Thatch => {
                    // Bundles 7 px deep whose lower edge waves by the handful, straw running down
                    // each in strands a px wide, lit at the lip, a shadow under each bundle.
                    let wave = (fast(wx.div_euclid(5) as u32, 3, c.h) % 3) as i32;
                    let course = (wy + wave).div_euclid(7);
                    let yy = (wy + wave).rem_euclid(7);
                    let strand = fast(wx as u32, course as u32, c.h) % 6;
                    match (yy, strand) {
                        (6, _) => Tone::Shade,
                        (0, _) => Tone::Light,
                        (5, _) | (_, 0) => Tone::Mid,
                        (_, 1 | 2) => Tone::Lift,
                        _ => Tone::Base,
                    }
                }
                P::Slate => {
                    let course = wy.div_euclid(6);
                    let xs = wx + (course & 1) * 5;
                    let yy = wy.rem_euclid(6);
                    let slate = fast(xs.div_euclid(10) as u32, course as u32, c.h);
                    if yy == 5 {
                        Tone::Deep
                    } else if xs.rem_euclid(10) == 0 {
                        Tone::Shade
                    } else if yy == 4 {
                        Tone::Lift
                    } else {
                        match slate % 7 {
                            0 => Tone::Mid,
                            1 => Tone::Lift,
                            _ => Tone::Base,
                        }
                    }
                }
                _ => {
                    // Clay tiles: rounded, 5 to 8 px wide by course, each lit on its left curve,
                    // a shadow under each course, a tile here and there weathered lighter, darker
                    // or green with moss.
                    let course = wy.div_euclid(6);
                    let xs = wx + (fast(course as u32, 0, c.h ^ 0x7e) % 13) as i32;
                    let block = xs.div_euclid(20);
                    let lx = xs.rem_euclid(20);
                    let hb = fast(block as u32, course as u32, 0x7e7e);
                    let c1 = 5 + (hb % 3) as i32;
                    let c2 = c1 + 5 + (hb >> 4) as i32 % 4;
                    let (start, end, k) = if lx < c1 {
                        (0, c1, 0)
                    } else if lx < c2 {
                        (c1, c2, 1)
                    } else {
                        (c2, 20, 2)
                    };
                    let own = match (hb >> (8 + k * 4)) % 11 {
                        0 | 1 => Tone::Mid,
                        2 => Tone::Lift,
                        _ => Tone::Base,
                    };
                    let (yy, tx) = (wy.rem_euclid(6), lx - start);
                    if (hb >> (20 + k)) % 61 == 0 && yy < 4 && tx > 0 {
                        // Moss on an old tile.
                        let moss = Ramp::Marsh;
                        put(p, c, x, y, moss.at(if yy == 0 { Tone::Lift } else { Tone::Base }), normal(0, 56), z);
                        continue;
                    }
                    match (yy, tx, end - 1 - start - tx) {
                        (5, _, _) => Tone::Deep,
                        (4, _, _) => own.step(-1),
                        (_, 0, _) => Tone::Shade,
                        (_, 1, _) => own.step(1),
                        (_, 2, _) => own,
                        (_, _, 0) => own.step(-1),
                        _ => own,
                    }
                }
            };
            let (body, n) = if d < -2 { (body.step(1), normal(0, -50)) } else { (body, normal(0, 56)) };
            put(p, c, x, y, r.at(body.step(shift)), n, z);
        }
    }
    if up == 0 {
        for x in 0..CELL {
            put(p, c, x, 0, r.at(Tone::Deep), FLAT, top - 5);
        }
    }
    // The ridge cap, where it falls in this cell.
    for (dy, t) in [(-2, Tone::Light), (-1, Tone::High), (0, Tone::Light), (1, Tone::Mid), (2, Tone::Deep)] {
        let y = RIDGE - up * CELL + dy;
        if (0..CELL).contains(&y) {
            for x in 0..CELL {
                put(p, c, x, y, r.at(t.step(shift)), if dy < 0 { normal(0, -30) } else { normal(0, 30) }, top);
            }
        }
    }
    if eave {
        for x in 0..CELL {
            step(p, c, x, CELL - 4, -1);
            put(p, c, x, CELL - 3, r.at(Tone::Shade.step(shift)), normal(0, 90), top - 16);
            put(p, c, x, CELL - 2, r.at(Tone::Deep), normal(0, 90), top - 16);
            put(p, c, x, CELL - 1, r.at(Tone::Deep), FLAT, top - 16);
        }
    }
    if west {
        for y in 0..CELL {
            let z = p.s.ly.z(c.px + 2, c.py + y);
            put(p, c, 0, y, r.at(Tone::Deep), FLAT, z);
            put(p, c, 1, y, r.at(Tone::Light.step(shift)), normal(-60, 40), z);
        }
    }
    if east {
        for y in 0..CELL {
            let z = p.s.ly.z(c.px + CELL - 3, c.py + y);
            put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, z);
            put(p, c, CELL - 2, y, r.at(Tone::Shade.step(shift)), normal(60, 40), z);
        }
    }
}

/// House walls: plaster or brick in one face from the eave to the plinth, the eave's shadow
/// under the roof, a plinth at the foot, dark corners, and a window in the face's middle course
/// every fourth cell of a long wall, lit at night more often than not.
fn house_wall(p: &mut Painter, c: &Cell, seed: u32) {
    let is_wall = |s: &Style| matches!(s.row.pattern, P::Plaster | P::Brick);
    let top = i32::from(c.st.row.rise);
    let r = c.st.ramp;
    let timber = c.st.accent.unwrap_or(Ramp::WoodDark);
    let brick = c.st.row.pattern == P::Brick;
    let (up, down) = (run(p, c, -1, is_wall), run(p, c, 1, is_wall));
    let rows = up + down + 1;
    let z_at = |y: i32| face_z(top, y, down, rows);
    let (n, s, w, e) = (nst(p, c, 0, -1), nst(p, c, 0, 1), nst(p, c, -1, 0), nst(p, c, 1, 0));
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let t = if brick {
                let course = wy.div_euclid(4);
                let xs = wx + (course & 1) * 4;
                if wy.rem_euclid(4) == 3 || xs.rem_euclid(8) == 0 {
                    Tone::Shade
                } else {
                    let b = fast(xs.div_euclid(8) as u32, course as u32, c.h);
                    let t = stone_tone(b);
                    if wy.rem_euclid(4) == 0 { t.step(1) } else { t }
                }
            } else {
                // Plaster weathered in broad soft patches.
                let v = p.s.fine.at(wx, wy) + (p.s.wob_x.at(wx, wy) - 128) / 3;
                if v < 52 {
                    Tone::Lift
                } else if v > 214 {
                    Tone::Light
                } else {
                    Tone::Lift
                }
            };
            // The face darkens a little toward its foot.
            let t = if down == 0 && y > 8 { t.step(-1) } else { t };
            put(p, c, x, y, r.at(t), normal(0, FACE), z_at(y));
        }
    }
    // Windows: the ground floor's in the face's middle course; a wall three courses or more tall
    // has an upper floor, its smaller windows under the eave between the ones below.
    let inner = is_wall(&w) && is_wall(&e);
    let ground = inner && c.wx.rem_euclid(3) == 1 && up == (rows - 1) / 2 && !(rows >= 3 && up == 0);
    let upper = inner && rows >= 3 && up == 0 && c.wx.rem_euclid(3) == 2;
    let lit = h32(c.wx as u32 / 3, c.wy as u32, seed ^ salt::WINDOW) % 3 != 0;
    if ground {
        window(p, c, (3, 3, 10, 10), lit, timber, &z_at);
    } else if upper {
        window(p, c, (4, 7, 8, 8), lit && h32(c.wx as u32, 1, seed ^ salt::WINDOW) % 2 == 0, timber, &z_at);
    }
    // A plaster wall's timber: a beam under the eave.
    if !brick && n.row.group == TileGroup::Roof {
        for x in 0..CELL {
            put(p, c, x, 3, timber.at(Tone::Lift), normal(0, FACE - 30), z_at(3));
            put(p, c, x, 4, timber.at(Tone::Base), normal(0, FACE), z_at(4));
            put(p, c, x, 5, timber.at(Tone::Shade), normal(0, FACE), z_at(5));
            step(p, c, x, 6, -1);
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
    if !is_wall(&s) {
        for x in 0..CELL {
            let stone = if brick { r } else { Ramp::Stone };
            put(p, c, x, CELL - 3, stone.at(Tone::Base), normal(0, FACE), 3);
            put(p, c, x, CELL - 2, stone.at(Tone::Mid), normal(0, FACE), 2);
            put(p, c, x, CELL - 1, stone.at(Tone::Deep), FLAT, 1);
        }
    }
    if !is_wall(&w) {
        for y in 0..CELL {
            put(p, c, 0, y, r.at(Tone::Deep), FLAT, z_at(y));
            put(p, c, 1, y, if brick { r.at(Tone::Light) } else { timber.at(Tone::Base) }, normal(-50, FACE), z_at(y));
        }
    }
    if !is_wall(&e) {
        for y in 0..CELL {
            put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, z_at(y));
            put(
                p,
                c,
                CELL - 2,
                y,
                if brick { r.at(Tone::Shade) } else { timber.at(Tone::Shade) },
                normal(50, FACE),
                z_at(y),
            );
        }
    }
}

/// A casement window in the box `(x0, y0, w, h)` of the cell: panes in a timber frame under a
/// stone lintel, on a lit sill with its shadow under it; lit from within when `lit` (the glass in
/// the emissive layer too), else a pane catching the sky.
fn window(
    p: &mut Painter,
    c: &Cell,
    (x0, y0, w, h): (i32, i32, i32, i32),
    lit: bool,
    timber: Ramp,
    z_at: &dyn Fn(i32) -> i32,
) {
    let stone = Ramp::Stone;
    let (x1, y1) = (x0 + w - 1, y0 + h - 1);
    let (mx, my) = (x0 + w / 2, y0 + h / 2);
    for x in x0 - 1..=x1 + 1 {
        put(
            p,
            c,
            x,
            y0 - 2,
            stone.at(if x == x0 - 1 { Tone::Light } else { Tone::Lift }),
            normal(0, FACE - 40),
            z_at(y0 - 2),
        );
        put(p, c, x, y0 - 1, stone.at(Tone::Mid), normal(0, FACE), z_at(y0 - 1));
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let bar = x == mx || (w >= 10 && x == mx - 1);
            let frame = y == y0 || y == y1 || x == x0 || x == x1 || bar || y == my;
            if frame {
                let t = if x == x0 || y == y0 || y == my { Tone::Lift } else { Tone::Shade };
                put(p, c, x, y, timber.at(t), normal(0, FACE), z_at(y));
            } else {
                let (glass, t) = if lit {
                    (Ramp::GlassLit, if y < my { Tone::Light } else { Tone::Base })
                } else if y == y0 + 1 && (x == x0 + 1 || x == mx + 1) {
                    (Ramp::Glass, Tone::High)
                } else {
                    (Ramp::Glass, if y < my { Tone::Base } else { Tone::Shade })
                };
                put(p, c, x, y, glass.at(t), normal(0, UNIT * 9 / 10), z_at(y));
                if lit {
                    p.s.ly.glow(c.px + x, c.py + y, glass.at(t));
                }
            }
        }
    }
    for x in x0 - 1..=x1 + 1 {
        put(
            p,
            c,
            x,
            y1 + 1,
            stone.at(if x < x1 + 1 { Tone::High } else { Tone::Base }),
            normal(0, 20),
            z_at(y1 + 1) + 1,
        );
        step(p, c, x, y1 + 2, -2);
        step(p, c, x + 1, y1 + 3, -1);
    }
}

/// A px's place among the jittered centres of a `size` px lattice: its offset from the nearest
/// centre, that centre's hash, and whether it lies on the seam to the next nearest.
struct Cellular {
    dx: i32,
    dy: i32,
    id: u32,
    edge: bool,
}

fn voronoi(wx: i32, wy: i32, size: i32, salt: u32) -> Cellular {
    let (gx, gy) = (wx.div_euclid(size), wy.div_euclid(size));
    let (mut best, mut second) = ((i32::MAX, 0, 0, 0u32), i32::MAX);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let hc = fast((gx + ox) as u32, (gy + oy) as u32, salt);
            let (cx, cy) =
                ((gx + ox) * size + (hc % size as u32) as i32, (gy + oy) * size + ((hc >> 8) % size as u32) as i32);
            let d = (wx - cx) * (wx - cx) + (wy - cy) * (wy - cy);
            if d < best.0 {
                second = best.0;
                best = (d, wx - cx, wy - cy, hc);
            } else if d < second {
                second = d;
            }
        }
    }
    // On the seam: the two nearest centres are within a px of each other's distance.
    let edge = isqrt_i(second) - isqrt_i(best.0) < 1;
    Cellular { dx: best.1, dy: best.2, id: best.3, edge }
}

fn isqrt_i(v: i32) -> i32 {
    jane_core::num::isqrt(v.max(0) as u64) as i32
}

/// A clipped hedge: leaf in clumps round jittered centres, each lit on its upper left, lit along
/// the hedge's top, a shaded face where it drops south.
fn hedge(p: &mut Painter, c: &Cell) {
    let same = |p: &Painter, dx: i32, dy: i32| nb(p, c, dx, dy) == Tile::Hedge;
    let top = i32::from(c.st.row.rise);
    let r = c.st.ramp;
    let south = !same(p, 0, 1);
    let face_from = if south { CELL - 6 } else { CELL };
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            // Leaf clumps round the jittered centres of a 6 px lattice, each a small dome lit
            // from the top-left, a crevice of shade where two clumps meet.
            let v = voronoi(wx, wy, 6, 0x4e);
            let (dx, dy) = (v.dx, v.dy);
            let s = -(dx * 2 + dy * 3) - (dx * dx + dy * dy) / 2;
            let t = if v.edge {
                Tone::Mid
            } else if s > 5 {
                Tone::Light
            } else if s > 0 {
                Tone::Lift
            } else if s > -8 {
                Tone::Base
            } else {
                Tone::Mid
            };
            if y >= face_from {
                let t2 = if y - face_from > 3 { Tone::Deep } else { t.step(-2) };
                put(p, c, x, y, r.at(t2), normal(0, FACE), (top * (CELL - y) / 6).max(1));
            } else {
                put(p, c, x, y, r.at(t), normal((dx * 20).clamp(-60, 60), (dy * 20).clamp(-60, 60)), top);
            }
        }
    }
    if !same(p, 0, -1) {
        for x in 0..CELL {
            put(p, c, x, 0, r.at(Tone::Deep), FLAT, top);
            put(p, c, x, 1, r.at(Tone::High), normal(0, -60), top);
        }
    }
    if south {
        for x in 0..CELL {
            put(p, c, x, CELL - 1, r.at(Tone::Deep), FLAT, 1);
        }
    }
    if !same(p, -1, 0) {
        for y in 0..CELL {
            let z = p.s.ly.z(c.px + 1, c.py + y);
            put(p, c, 0, y, r.at(Tone::Deep), FLAT, z);
        }
    }
    if !same(p, 1, 0) {
        for y in 0..CELL {
            let z = p.s.ly.z(c.px + CELL - 2, c.py + y);
            put(p, c, CELL - 1, y, r.at(Tone::Deep), FLAT, z);
        }
    }
}

/// Floor slabs `detail` px square, a joint round each, each slab a tone of its own, lit along
/// its inner top and left and shaded along its bottom and right; a crack across one now and then.
fn slabs(p: &mut Painter, c: &Cell) {
    let size = i32::from(c.st.row.detail).clamp(4, 16);
    let z = i32::from(c.st.row.rise).max(1);
    let r = c.st.ramp;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            // Slabs laid in a running bond, so the joints never rule a grid; most slabs one tone,
            // the odd one lighter or darker; worn in broad patches over many slabs.
            let row = wy.div_euclid(size);
            let xs = wx + (row & 1) * size / 2;
            let (lx, ly) = (xs.rem_euclid(size), wy.rem_euclid(size));
            let slab = fast(xs.div_euclid(size) as u32, row as u32, 0x51ab);
            let body = match slab % 9 {
                0 => Tone::Mid,
                1 => Tone::Lift,
                _ => Tone::Base,
            };
            let wear = p.s.fine.at(wx, wy) + (p.s.wob_x.at(wx, wy) - 128) / 4;
            let body = if wear < 70 {
                body.step(-1)
            } else if wear > 196 {
                body.step(1)
            } else {
                body
            };
            let t = if lx == 0 || ly == 0 {
                Tone::Shade
            } else if lx == 1 || ly == 1 {
                body.step(1)
            } else if lx == size - 1 || ly == size - 1 {
                body.step(-1)
            } else {
                body
            };
            put(p, c, x, y, r.at(t), FLAT, z);
        }
    }
    if c.h % 7 == 0 {
        let (mut x, mut y) = (3 + (c.h >> 3) as i32 % 6, 3 + (c.h >> 6) as i32 % 6);
        for i in 0..6 {
            step(p, c, x, y, -2);
            x += 1;
            y += i32::from((c.h >> (9 + i)) & 1 == 1);
        }
    }
}

/// Floorboards `detail` px wide along the room, butt joints staggered by board, each board a tone
/// of its own with a grain run or two along it.
fn boards(p: &mut Painter, c: &Cell) {
    let wide = i32::from(c.st.row.detail).clamp(3, 8);
    let z = i32::from(c.st.row.rise).max(1);
    let r = c.st.ramp;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let board = wy.div_euclid(wide);
            let off = (fast(board as u32, 0, 0xb0a2) % 40) as i32;
            let seg = (wx + off).div_euclid(40);
            let along = (wx + off).rem_euclid(40);
            let bt = fast(seg as u32, board as u32, 0xb0a2);
            let row = wy.rem_euclid(wide);
            // A grain run: one row of the board, 5 to 12 px long, somewhere along it.
            let (gs, gl, gr) =
                ((bt >> 4) as i32 % 28, 5 + (bt >> 9) as i32 % 8, 1 + (bt >> 13) as i32 % (wide - 2).max(1));
            let grain = row == gr && along >= gs && along < gs + gl;
            let t = if row == wide - 1 || along == 0 {
                Tone::Shade
            } else if row == 0 {
                Tone::Lift
            } else if grain {
                Tone::Mid
            } else {
                match bt % 3 {
                    0 => Tone::Mid,
                    1 => Tone::Base,
                    _ => Tone::Lift,
                }
            };
            let t = if grain && bt % 3 == 0 { Tone::Shade } else { t };
            put(p, c, x, y, r.at(t), FLAT, z);
        }
    }
}

/// A cave's rough floor: broad patches, a stone or two lit on top.
fn rock_floor(p: &mut Painter, c: &Cell) {
    let z = i32::from(c.st.row.rise).max(1);
    let r = c.st.ramp;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let v = (p.s.patch.at(wx, wy) * 2 + p.s.fine.at(wx, wy)) / 3 + (p.s.wob_x.at(wx, wy) - 128) / 4;
            let t = if v < 100 {
                Tone::Mid
            } else if v > 168 {
                Tone::Lift
            } else {
                Tone::Base
            };
            put(p, c, x, y, r.at(t), FLAT, z);
        }
    }
    for s in 0..(c.h & 3) as i32 {
        let hs = h32(c.h, s as u32, 3);
        let (x, y) = (2 + below(hs, 11) as i32, 2 + below(hs.rotate_right(8), 11) as i32);
        put(p, c, x, y, r.at(Tone::Light), normal(-50, -50), z + 2);
        put(p, c, x + 1, y, r.at(Tone::Lift), normal(40, -40), z + 2);
        put(p, c, x, y + 1, r.at(Tone::Mid), normal(-40, 40), z + 1);
        put(p, c, x + 1, y + 1, r.at(Tone::Shade), normal(50, 50), z + 1);
        step(p, c, x + 1, y + 2, -1);
        step(p, c, x + 2, y + 2, -1);
    }
}

/// A threshold stone, grooved across the way through.
fn sill(p: &mut Painter, c: &Cell) {
    let across = nb(p, c, -1, 0) == Tile::Sill || nb(p, c, 1, 0) == Tile::Sill;
    let z = i32::from(c.st.row.rise).max(1);
    let r = c.st.ramp;
    for y in 0..CELL {
        for x in 0..CELL {
            let b = if across { y } else { x };
            let t = match b {
                0 | 1 => Tone::Light,
                2 | 13 => Tone::Shade,
                14 | 15 => Tone::Mid,
                _ => Tone::Base,
            };
            put(p, c, x, y, r.at(t), FLAT, z);
        }
    }
}

/// A glass case: a timber frame, the pane pale with a glint across it.
fn glass(p: &mut Painter, c: &Cell) {
    let z = i32::from(c.st.row.rise).max(1);
    let (r, frame) = (c.st.ramp, c.st.accent.unwrap_or(Ramp::WoodDark));
    for y in 0..CELL {
        for x in 0..CELL {
            let edge = x == 0 || y == 0 || x == CELL - 1 || y == CELL - 1;
            let ix = if edge {
                frame.at(if x == 0 || y == 0 { Tone::Lift } else { Tone::Shade })
            } else if (x + y == 9 || x + y == 10 || x + y == 17) && x > 1 && y > 1 {
                r.at(Tone::Glint)
            } else {
                r.at(if y > 11 { Tone::Base } else { Tone::Lift })
            };
            put(p, c, x, y, ix, FLAT, z);
        }
    }
}

/// Ballast: stones of 2 to 3 px on a jittered 3 px lattice, each lit on its top-left and shaded
/// on its bottom-right: gravel as clusters, not speckle.
fn ballast(wx: i32, wy: i32, h: u32) -> Tone {
    let (gx, gy) = (wx.div_euclid(3), wy.div_euclid(3));
    let (lx, ly) = (wx.rem_euclid(3), wy.rem_euclid(3));
    let hs = fast(gx as u32, gy as u32, h);
    let body = match hs % 5 {
        0 => Tone::Mid,
        1 | 2 => Tone::Base,
        _ => Tone::Lift,
    };
    match (lx, ly) {
        (0, 0) => body.step(1),
        (2, _) | (_, 2) => body.step(-1),
        _ => body,
    }
}

/// Rails on sleepers on ballast. Rails run the way the line runs; round a bend each rail is an arc
/// about the corner the two neighbours share, the sleepers laid across it.
fn rail(p: &mut Painter, c: &Cell) {
    let t = c.st.tile;
    let same = |p: &Painter, dx: i32, dy: i32| nb(p, c, dx, dy) == t;
    let (w, e, n, s) = (same(p, -1, 0), same(p, 1, 0), same(p, 0, -1), same(p, 0, 1));
    let z = i32::from(c.st.row.rise).max(1);
    let ballast_r = c.st.ramp;
    let iron = c.st.accent.unwrap_or(Ramp::Iron);
    let wood = Ramp::WoodDark;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            put(p, c, x, y, ballast_r.at(ballast(wx, wy, 0x0ba1)), FLAT, z);
        }
    }
    let bend = w != e && n != s;
    // `along` is the px along the line, `across` the px across it.
    let lay = |p: &mut Painter, x: i32, y: i32, along: i32, across: i32| {
        if (4..=5).contains(&across) || (10..=11).contains(&across) {
            let top = across == 4 || across == 10;
            put(
                p,
                c,
                x,
                y,
                iron.at(if top { Tone::Light } else { Tone::Shade }),
                normal(0, if top { -50 } else { 60 }),
                z + 3,
            );
        } else if along.rem_euclid(5) < 3 && (2..=13).contains(&across) {
            let edge = along.rem_euclid(5) == 0;
            put(p, c, x, y, wood.at(if edge { Tone::Lift } else { Tone::Base }), FLAT, z + 1);
        } else if across == 6 || across == 12 {
            step(p, c, x, y, -1);
        }
    };
    if bend {
        // The corner both neighbours share, in doubled px.
        let kx = if w { 0 } else { 2 * CELL };
        let ky = if n { 0 } else { 2 * CELL };
        let (hx, vy) = (if w { -1 } else { 1 }, if n { -1 } else { 1 });
        // A line laid diagonally is a staircase of bends: each turns back the way the last one
        // came. There the rails run straight across the corner, edge middle to edge middle, and
        // line up with the next cell's, where arcs would wave; the ballast off the line sinks.
        if same(p, hx, -vy) || same(p, -hx, vy) {
            for y in 0..CELL {
                for x in 0..CELL {
                    let (dx, dy) = ((2 * x + 1 - kx).abs(), (2 * y + 1 - ky).abs());
                    // Px from the corner across the line (its middle 8 px out), and along it.
                    let across = 8 + ((dx + dy) / 2 - 8) * 7 / 10;
                    let along = (dx - dy) * 7 / 20 + 8;
                    if across > 15 {
                        step(p, c, x, y, -1);
                        continue;
                    }
                    lay(p, x, y, along, across);
                }
            }
            return;
        }
        for y in 0..CELL {
            for x in 0..CELL {
                let (dx, dy) = (2 * x + 1 - kx, 2 * y + 1 - ky);
                let d = jane_core::num::isqrt((dx * dx + dy * dy) as u64) as i32 / 2;
                let a = i32::from(iatan2(dy.abs(), dx.abs()).0);
                // Px along the arc at the rails' middle radius (8 px): about 12.5 px a quarter turn.
                let along = a * 13 / 16384;
                lay(p, x, y, along, d);
            }
        }
        return;
    }
    let across_x = w || e || !(n || s);
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            if across_x {
                lay(p, x, y, wx, y);
            } else {
                lay(p, x, y, wy, x);
            }
        }
    }
}

/// A boardwalk: planks laid across the way the walk runs, a nail at each end, dark where it
/// ends over the water.
fn boardwalk(p: &mut Painter, c: &Cell) {
    let same = |p: &Painter, dx: i32, dy: i32| nb(p, c, dx, dy) == Tile::Boardwalk;
    let along_x = same(p, -1, 0) || same(p, 1, 0);
    let z = i32::from(c.st.row.rise).max(1);
    let r = c.st.ramp;
    for y in 0..CELL {
        for x in 0..CELL {
            let (wx, wy) = c.w(x, y);
            let (along, across) = if along_x { (wx, y) } else { (wy, x) };
            let plank = along.div_euclid(4);
            let hp = fast(plank as u32, 0, c.h ^ 0xb0);
            let t = if along.rem_euclid(4) == 3 {
                Tone::Deep
            } else if along.rem_euclid(4) == 0 {
                Tone::Lift
            } else if (across == 2 || across == 13) && along.rem_euclid(4) == 1 {
                Tone::Shade
            } else {
                match hp % 3 {
                    0 => Tone::Mid,
                    1 => Tone::Base,
                    _ => Tone::Lift,
                }
            };
            put(p, c, x, y, r.at(t), FLAT, z);
        }
    }
    let ends: [(bool, bool); 2] = if along_x {
        [(same(p, 0, -1), true), (same(p, 0, 1), false)]
    } else {
        [(same(p, -1, 0), true), (same(p, 1, 0), false)]
    };
    for (joined, first) in ends {
        if joined {
            continue;
        }
        for i in 0..CELL {
            let (x, y) =
                if along_x { (i, if first { 0 } else { CELL - 1 }) } else { (if first { 0 } else { CELL - 1 }, i) };
            put(p, c, x, y, r.at(Tone::Deep), FLAT, z);
        }
    }
}
