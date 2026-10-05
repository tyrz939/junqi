//! A dungeon read as rooms (ART-PLAN M5, B2), for the generic dressing kit (ART.md §2.6.1):
//! found once when the zone is entered, from its tiles, its theme and its room graph (each
//! room's rect and what it is: boss, hub, rest), so the painter can frame what it paints.
//!
//! A room is the open floor in its rect; with no graph, it is floor wide enough to hold a 5 x 5
//! square (a corridor is three cells wide and never does), its boss's room the one its boss
//! stands in and the largest of the rest its set room. A room's doors are where it opens onto
//! other floor; its worn lanes run between its doors; and a theme that heaps bones keeps a few
//! places in each room for them.
//!
//! All of it is drawing: nothing here moves a tile, a prop or a hash of the world. Pure integer
//! functions of the tiles, the props' footprints, the graph and the seed, so a zone is read the
//! same on every machine. Nothing here knows a dungeon by name: the theme is a data row.

use jane_core::Tile;
use jane_core::grid::Rect;
use jane_core::ids::ZoneId;
use jane_core::tile::F_BLOCK_LOS;
use jane_data::{DungeonTheme, FloorMotif, FloorMotifKind};

use super::CELL;
use crate::hash::{below, h32};
use crate::palette::Ramp;

/// A dungeon theme resolved for painting: the data row, its ramps found in the palette. A ramp
/// the row leaves blank (`""`) falls back to what the painter would use without it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The row as the data has it: its trims, motifs and kinds.
    pub row: DungeonTheme,
    /// The head trim's ramp, if not the face's own.
    pub trim_ramp: Option<Ramp>,
    /// The door frames', posts' and lintels' ramp.
    pub frame: Ramp,
    /// The border's two ramps (none: the floor's own).
    pub border_ramps: [Option<Ramp>; 2],
    /// The payoff floors' inlay.
    pub inlay: Ramp,
    /// The emblem's two ramps.
    pub emblem_ramps: [Ramp; 2],
}

fn ramp(name: &str) -> Option<Ramp> {
    if name.is_empty() { None } else { Ramp::by_name(name) }
}

impl Theme {
    /// A data row's theme.
    pub fn of(row: DungeonTheme) -> Theme {
        Theme {
            row,
            trim_ramp: ramp(row.trim_ramp),
            frame: ramp(row.frame).unwrap_or(Ramp::Stone),
            border_ramps: [ramp(row.border_ramps[0]), ramp(row.border_ramps[1])],
            inlay: ramp(row.inlay).unwrap_or(Ramp::Brass),
            emblem_ramps: [
                ramp(row.emblem_ramps[0]).unwrap_or(Ramp::Iron),
                ramp(row.emblem_ramps[1]).unwrap_or(Ramp::Brass),
            ],
        }
    }

    /// The theme zone `zone` wears in this build's data, if any.
    pub fn of_zone(zone: ZoneId) -> Option<Theme> {
        jane_data::dungeon_themes().of(zone).map(|t| Theme::of(*t))
    }

    /// Its floor motif of kind `k`, if it lays one.
    pub fn floor(&self, k: FloorMotifKind) -> Option<&'static FloorMotif> {
        let f: &'static [FloorMotif] = self.row.floor;
        f.iter().find(|m| m.kind == k)
    }
}

/// What a room is in the dungeon's graph: the kinds the dressing keys off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// An ordinary room.
    Plain,
    /// Where the party rests.
    Rest,
    /// The boss's room: its own floor, braziers, a dais, the motif writ large.
    Boss,
    /// The hub: the dungeon's one set room, symmetric and lit as a stage.
    Set,
}

/// Which wall a door is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The north wall.
    N,
    /// The south wall.
    S,
    /// The west wall.
    W,
    /// The east wall.
    E,
}

/// A door of a room: the run of its edge cells that open onto other floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    /// Which wall it is in.
    pub side: Side,
    /// The first and last cell of the opening along its wall (x for N and S, y for W and E).
    pub a: i32,
    /// See `a`.
    pub b: i32,
    /// The row (N, S) or column (W, E) of the wall the opening is cut through.
    pub at: i32,
    /// Into the boss's room: wider, framed and carved.
    pub boss: bool,
}

impl Door {
    /// The middle of the opening, world px, on the room's side of the wall.
    pub fn mid(&self) -> (i32, i32) {
        let m = (self.a + self.b + 1) * CELL / 2;
        match self.side {
            Side::N => (m, (self.at + 1) * CELL),
            Side::S => (m, self.at * CELL),
            Side::W => ((self.at + 1) * CELL, m),
            Side::E => (self.at * CELL, m),
        }
    }
}

/// A room of the dungeon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DRoom {
    /// Its cells' bounding box.
    pub rect: Rect,
    /// How many cells it has.
    pub cells: u32,
    /// What it is in the graph.
    pub role: Role,
    /// Its openings.
    pub doors: Vec<Door>,
    /// Its worn lanes, world px segments `(ax, ay, bx, by)`, every one square to the walls.
    pub lanes: Vec<(i32, i32, i32, i32)>,
    /// Where its bones are heaped or its toadstools ring (a theme with `heaps` or `toadstools`), world cells.
    pub spots: Vec<(i32, i32)>,
    /// Its seed.
    pub seed: u32,
}

impl DRoom {
    /// Its middle, world px.
    pub fn centre(&self) -> (i32, i32) {
        ((2 * self.rect.x + self.rect.w) * CELL / 2, (2 * self.rect.y + self.rect.h) * CELL / 2)
    }
}

/// What a chunk drew of a room's framing (`Painter::framed`): a face two cells tall over it.
pub const FRAMED_TALL: u8 = 1;
/// Its floor's inset border.
pub const FRAMED_BORDER: u8 = 2;
/// Its theme's motif.
pub const FRAMED_MOTIF: u8 = 4;

/// No room: a corridor, a wall, outside.
pub const NO_ROOM: u8 = 255;

/// The rooms of a dungeon zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dungeon {
    /// How it is dressed.
    pub theme: Theme,
    w: i32,
    h: i32,
    /// Per cell, the room it is in, or [`NO_ROOM`].
    room: Vec<u8>,
    /// Per cell, whether a prop stands on it (no motif is laid there).
    busy: Vec<bool>,
    /// Its rooms, in graph (or reading) order; a room too small is kept with no cells.
    pub rooms: Vec<DRoom>,
}

/// The side of the square a room must hold with no graph, cells.
const SQUARE: i32 = 5;
/// Fewer cells than this is no room.
const LEAST: u32 = 20;
/// How far a lane's wear reaches each side of its line, px.
pub const LANE_HALF: i32 = 11;

/// What stops a room: a wall, the void, a tree (what blocks sight).
fn closed(t: Tile) -> bool {
    t == Tile::Void || t.flags() & F_BLOCK_LOS != 0
}

impl Dungeon {
    /// Reads a zone (`w` x `h` cells) as rooms: `tile` its tiles, `busy` the cells props stand
    /// on, `graph` each room's rect and role (empty: found from the tiles), `boss` where its boss
    /// stands (for a zone with no graph), `seed` the world's.
    pub fn find(
        theme: Theme,
        (w, h): (i32, i32),
        tile: impl Fn(i32, i32) -> Tile,
        busy: impl Iterator<Item = (i32, i32)>,
        graph: &[(Rect, Role)],
        boss: Option<(i32, i32)>,
        seed: u32,
    ) -> Dungeon {
        let n = (w.max(0) * h.max(0)) as usize;
        let mut room = vec![NO_ROOM; n];
        let mut rooms: Vec<DRoom> = Vec::new();
        let new_room = |members: &[(i32, i32)], role: Role| {
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for &(mx, my) in members {
                (x0, y0, x1, y1) = (x0.min(mx), y0.min(my), x1.max(mx), y1.max(my));
            }
            DRoom {
                rect: Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1),
                cells: members.len() as u32,
                role,
                doors: Vec::new(),
                lanes: Vec::new(),
                spots: Vec::new(),
                seed: h32(seed, ((x0 as u32) << 16) | (y0 as u32 & 0xffff), 0x4452_4f4f),
            }
        };
        if graph.is_empty() {
            let open = squares(w, h, &tile);
            flood(w, h, &open, &mut room, &mut rooms, |m| new_room(m, Role::Plain));
        } else {
            for &(r, role) in graph {
                if rooms.len() >= usize::from(NO_ROOM) {
                    break;
                }
                let id = rooms.len() as u8;
                let mut members = Vec::new();
                for y in r.y.max(0)..r.bottom().min(h) {
                    for x in r.x.max(0)..r.right().min(w) {
                        let k = (y * w + x) as usize;
                        if room[k] == NO_ROOM && !closed(tile(x, y)) {
                            room[k] = id;
                            members.push((x, y));
                        }
                    }
                }
                if members.is_empty() {
                    rooms.push(DRoom { cells: 0, ..new_room(&[(r.x, r.y)], role) });
                } else {
                    rooms.push(new_room(&members, role));
                }
            }
        }
        // Too small to be a room: a wide bend of corridor.
        for r in &mut room {
            if *r != NO_ROOM && rooms[usize::from(*r)].cells < LEAST {
                *r = NO_ROOM;
            }
        }
        let mut busy_v = vec![false; n];
        for (x, y) in busy {
            if x >= 0 && y >= 0 && x < w && y < h {
                busy_v[(y * w + x) as usize] = true;
            }
        }
        let mut d = Dungeon { theme, w, h, room, busy: busy_v, rooms };
        d.doors(&tile);
        if graph.is_empty() {
            // No graph: the boss's room is where it stands, the largest of the rest the hub.
            if let Some((bx, by)) = boss {
                if let Some(id) = d.room_id(bx, by).or_else(|| d.nearest(bx, by)) {
                    d.rooms[usize::from(id)].role = Role::Boss;
                }
            }
            let set = d
                .real()
                .filter(|(_, r)| r.role == Role::Plain)
                .max_by_key(|(i, r)| (r.cells, std::cmp::Reverse(*i)))
                .map(|(i, _)| i);
            if let Some(i) = set {
                d.rooms[i].role = Role::Set;
            }
        }
        let spots = theme
            .floor(FloorMotifKind::Heaps)
            .or_else(|| theme.floor(FloorMotifKind::Toadstools))
            .map(|f| f.count.max(1));
        for i in 0..d.rooms.len() {
            if d.rooms[i].cells < LEAST {
                continue;
            }
            let boss = d.rooms[i].role == Role::Boss;
            for door in &mut d.rooms[i].doors {
                door.boss = boss;
            }
            d.lanes(i);
            if let Some(most) = spots {
                d.spots(i, usize::from(most));
            }
        }
        d
    }

    /// The room cell `(x, y)` is in.
    pub fn room_id(&self, x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        Some(self.room[(y * self.w + x) as usize]).filter(|&r| r != NO_ROOM)
    }

    /// The room cell `(x, y)` is in.
    pub fn room_at(&self, x: i32, y: i32) -> Option<&DRoom> {
        self.room_id(x, y).map(|i| &self.rooms[usize::from(i)])
    }

    /// Whether a prop stands on cell `(x, y)`.
    pub fn busy(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.busy[(y * self.w + x) as usize]
    }

    /// The real rooms (big enough), with their index.
    pub fn real(&self) -> impl Iterator<Item = (usize, &DRoom)> {
        self.rooms.iter().enumerate().filter(|(_, r)| r.cells >= LEAST)
    }

    fn nearest(&self, x: i32, y: i32) -> Option<u8> {
        self.real()
            .min_by_key(|(_, r)| {
                let (cx, cy) = (r.rect.x + r.rect.w / 2, r.rect.y + r.rect.h / 2);
                (cx - x).abs() + (cy - y).abs()
            })
            .map(|(i, _)| i as u8)
    }

    /// Each room's doors: runs of its edge cells whose neighbour across a side is floor of no room.
    fn doors(&mut self, tile: &impl Fn(i32, i32) -> Tile) {
        let mut found: Vec<(u8, Side, i32, i32)> = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                let Some(id) = self.room_id(x, y) else { continue };
                for (side, dx, dy) in [(Side::N, 0, -1), (Side::S, 0, 1), (Side::W, -1, 0), (Side::E, 1, 0)] {
                    let (nx, ny) = (x + dx, y + dy);
                    let inside = nx >= 0 && ny >= 0 && nx < self.w && ny < self.h;
                    if inside && self.room_id(nx, ny).is_none() && !closed(tile(nx, ny)) {
                        found.push((id, side, x, y));
                    }
                }
            }
        }
        let key = |&(id, side, x, y): &(u8, Side, i32, i32)| match side {
            Side::N | Side::S => (id, side as u8, y, x),
            Side::W | Side::E => (id, side as u8, x, y),
        };
        found.sort_by_key(key);
        let mut i = 0;
        while i < found.len() {
            let (id, side, at, a) = key(&found[i]);
            let mut b = a;
            let mut j = i + 1;
            while j < found.len() {
                let (id2, side2, at2, a2) = key(&found[j]);
                if id2 != id || side2 != side || at2 != at || a2 != b + 1 {
                    break;
                }
                b = a2;
                j += 1;
            }
            let side = found[i].1;
            // The wall the opening is cut through is the row (column) outside the room.
            let wall = match side {
                Side::N | Side::W => at - 1,
                Side::S | Side::E => at + 1,
            };
            self.rooms[usize::from(id)].doors.push(Door { side, a, b, at: wall, boss: false });
            i = j;
        }
    }

    /// A room's worn lanes: from each door straight in two cells, then on to the next door's (or
    /// to the middle when it has one door), turning once.
    fn lanes(&mut self, i: usize) {
        let r = &self.rooms[i];
        let (cx, cy) = r.centre();
        let mut lanes = Vec::new();
        let ends: Vec<(i32, i32)> = r
            .doors
            .iter()
            .map(|d| {
                let (x, y) = d.mid();
                let e = match d.side {
                    Side::N => (x, y + 2 * CELL),
                    Side::S => (x, y - 2 * CELL),
                    Side::W => (x + 2 * CELL, y),
                    Side::E => (x - 2 * CELL, y),
                };
                lanes.push((x, y, e.0, e.1));
                e
            })
            .collect();
        let wide = r.rect.w >= r.rect.h;
        let mut join = |a: (i32, i32), b: (i32, i32)| {
            // Along the room's longer side first.
            let corner = if wide { (b.0, a.1) } else { (a.0, b.1) };
            lanes.push((a.0, a.1, corner.0, corner.1));
            lanes.push((corner.0, corner.1, b.0, b.1));
        };
        match ends.len() {
            0 => {}
            1 => join(ends[0], (cx, cy)),
            _ => {
                for k in 1..ends.len() {
                    join(ends[k - 1], ends[k]);
                }
            }
        }
        self.rooms[i].lanes = lanes;
    }

    /// Up to `most` places in a room its bones lie heaped (two at least where there is room):
    /// clear of its lanes and its props, a cell or more in from its walls.
    fn spots(&mut self, i: usize, most: usize) {
        let r = self.rooms[i].clone();
        let want = (2 + below(r.seed, most.max(2) as u32 - 1) as usize).min(most.max(1));
        let mut got: Vec<(i32, i32)> = Vec::new();
        for k in 0..96u32 {
            if got.len() >= want {
                break;
            }
            let h = h32(r.seed, k, 0x4845_4150);
            let x = r.rect.x + 1 + below(h, (r.rect.w - 2).max(1) as u32) as i32;
            let y = r.rect.y + 1 + below(h.rotate_right(11), (r.rect.h - 2).max(1) as u32) as i32;
            let inside = (-1..=1).all(|dy| (-1..=1).all(|dx| self.room_id(x + dx, y + dy) == Some(i as u8)));
            let (px, py) = (x * CELL + CELL / 2, y * CELL + CELL / 2);
            let clear = self.lane_dist(i, px, py) > 2 * CELL;
            let apart = got.iter().all(|&(gx, gy)| (gx - x).abs() + (gy - y).abs() >= 4);
            if inside && clear && apart && !self.busy(x, y) {
                got.push((x, y));
            }
        }
        self.rooms[i].spots = got;
    }

    /// Px from world px `(x, y)` to the nearest lane of room `i`.
    pub fn lane_dist(&self, i: usize, x: i32, y: i32) -> i32 {
        self.rooms[i].lanes.iter().map(|&(ax, ay, bx, by)| seg_dist(x, y, ax, ay, bx, by)).min().unwrap_or(i32::MAX)
    }

    /// The door whose frame stands at wall cell `(x, y)`, if any: `(room, door, along)` where
    /// `along` is the cell's offset from the opening (negative before its first cell, positive
    /// past its last), within `reach` cells of it (a cell more for a boss's door).
    pub fn jamb(&self, x: i32, y: i32, reach: i32) -> Option<(usize, Door, i32)> {
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
            let Some(id) = self.room_id(x + dx, y + dy) else { continue };
            let r = &self.rooms[usize::from(id)];
            for d in &r.doors {
                let (at, along) = match d.side {
                    Side::N | Side::S => (y, x),
                    Side::W | Side::E => (x, y),
                };
                if at != d.at {
                    continue;
                }
                let reach = if d.boss { reach + 1 } else { reach };
                if along < d.a && along >= d.a - reach {
                    return Some((usize::from(id), *d, along - d.a));
                }
                if along > d.b && along <= d.b + reach {
                    return Some((usize::from(id), *d, along - d.b));
                }
            }
        }
        None
    }
}

/// Each cell some 5 x 5 square of open cells covers.
fn squares(w: i32, h: i32, tile: &impl Fn(i32, i32) -> Tile) -> Vec<bool> {
    let (wu, hu) = (w.max(0) as usize, h.max(0) as usize);
    let at = |v: &Vec<i32>, x: i32, y: i32| v[y as usize * (wu + 1) + x as usize];
    let mut sum = vec![0i32; (wu + 1) * (hu + 1)];
    for y in 0..h {
        for x in 0..w {
            let k = (y as usize + 1) * (wu + 1) + x as usize + 1;
            sum[k] = i32::from(closed(tile(x, y))) + sum[k - 1] + sum[k - wu - 1] - sum[k - wu - 2];
        }
    }
    let rect_sum =
        |x0: i32, y0: i32, x1: i32, y1: i32| at(&sum, x1, y1) - at(&sum, x0, y1) - at(&sum, x1, y0) + at(&sum, x0, y0);
    let mut fits = vec![0i32; (wu + 1) * (hu + 1)];
    for y in 0..h {
        for x in 0..w {
            let ok = x + SQUARE <= w && y + SQUARE <= h && rect_sum(x, y, x + SQUARE, y + SQUARE) == 0;
            let k = (y as usize + 1) * (wu + 1) + x as usize + 1;
            fits[k] = i32::from(ok) + fits[k - 1] + fits[k - wu - 1] - fits[k - wu - 2];
        }
    }
    let mut open = vec![false; wu * hu];
    for y in 0..h {
        for x in 0..w {
            let (x0, y0) = ((x - SQUARE + 1).max(0), (y - SQUARE + 1).max(0));
            let n = at(&fits, x + 1, y + 1) - at(&fits, x0, y + 1) - at(&fits, x + 1, y0) + at(&fits, x0, y0);
            open[(y * w + x) as usize] = n > 0;
        }
    }
    open
}

/// The open cells' four-connected runs, in reading order, each a room.
fn flood(
    w: i32,
    h: i32,
    open: &[bool],
    room: &mut [u8],
    rooms: &mut Vec<DRoom>,
    make: impl Fn(&[(i32, i32)]) -> DRoom,
) {
    let ix = |x: i32, y: i32| (y * w + x) as usize;
    let mut stack = Vec::new();
    let mut members = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !open[ix(x, y)] || room[ix(x, y)] != NO_ROOM || rooms.len() >= usize::from(NO_ROOM) {
                continue;
            }
            let id = rooms.len() as u8;
            members.clear();
            stack.push((x, y));
            room[ix(x, y)] = id;
            while let Some((cx, cy)) = stack.pop() {
                members.push((cx, cy));
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && open[ix(nx, ny)] && room[ix(nx, ny)] == NO_ROOM {
                        room[ix(nx, ny)] = id;
                        stack.push((nx, ny));
                    }
                }
            }
            rooms.push(make(&members));
        }
    }
}

/// Px from `(x, y)` to the segment `(ax, ay)`–`(bx, by)`.
pub fn seg_dist(x: i32, y: i32, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    let (dx, dy) = (i64::from(bx - ax), i64::from(by - ay));
    let len2 = dx * dx + dy * dy;
    let (px, py) = (i64::from(x - ax), i64::from(y - ay));
    let t = if len2 == 0 { 0 } else { ((px * dx + py * dy) * 1024 / len2).clamp(0, 1024) };
    let (qx, qy) = (i64::from(ax) + dx * t / 1024, i64::from(ay) + dy * t / 1024);
    let (ex, ey) = (i64::from(x) - qx, i64::from(y) - qy);
    jane_core::num::isqrt((ex * ex + ey * ey) as u64) as i32
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use jane_data::{
        FloorMotif, ThemeBorder, ThemeCarving, ThemeEmblem, ThemeTrim, ThemeUpper, WallMotif, WallMotifKind,
    };

    /// A theme no data row has: a plain trim, a painted border, a sign on the walls and heaps
    /// on the floors. Every painter must take it as it takes the data's.
    pub(crate) fn test_theme() -> Theme {
        static WALL: [WallMotif; 1] = [WallMotif { kind: WallMotifKind::Sign, every: 4, at: 20 }];
        static FLOOR: [FloorMotif; 1] =
            [FloorMotif { kind: FloorMotifKind::Heaps, every: 0, count: 3, ramp: "", gathers: &[] }];
        Theme::of(DungeonTheme {
            id: "test",
            zones: &[],
            trim: ThemeTrim::Plain,
            trim_ramp: "",
            upper: ThemeUpper::Carry,
            frame: "stone",
            carving: ThemeCarving::Fluted,
            border: ThemeBorder::Line,
            border_ramps: ["cloth_cream", ""],
            lane_wear: 1,
            wall: &WALL,
            floor: &FLOOR,
            inlay: "brass",
            boss_spokes: 6,
            set_spokes: 4,
            braziers: 4,
            emblem: ThemeEmblem::Wheel,
            emblem_ramps: ["iron", "brass"],
            tint: 0xffffff,
            lift: 0,
            saturation: 128,
            fight: 0xffffff,
        })
    }

    /// Two rooms joined by a corridor three wide, walled.
    pub(crate) fn plan() -> (i32, i32, Vec<Tile>) {
        let (w, h) = (40, 18);
        let mut t = vec![Tile::Void; (w * h) as usize];
        let mut fill = |x0: i32, y0: i32, x1: i32, y1: i32, tile: Tile| {
            for y in y0..y1 {
                for x in x0..x1 {
                    t[(y * w + x) as usize] = tile;
                }
            }
        };
        fill(1, 1, 15, 14, Tile::TempleWall);
        fill(25, 2, 39, 17, Tile::TempleWall);
        fill(2, 3, 14, 13, Tile::TempleFloor);
        fill(14, 6, 26, 9, Tile::TempleFloor);
        fill(26, 4, 38, 16, Tile::TempleFloor);
        (w, h, t)
    }

    #[test]
    fn rooms_and_their_doors_are_found_and_the_corridor_is_none() {
        let (w, h, t) = plan();
        let tile = |x: i32, y: i32| t[(y * w + x) as usize];
        let d = Dungeon::find(test_theme(), (w, h), tile, std::iter::empty(), &[], Some((30, 8)), 7);
        assert_eq!(d.real().count(), 2, "{:?}", d.rooms);
        assert_eq!(d.room_id(20, 7), None, "the corridor is no room");
        let (a, b) = (d.room_at(5, 5).unwrap(), d.room_at(30, 8).unwrap());
        assert_eq!(a.doors.len(), 1);
        assert_eq!((a.doors[0].side, a.doors[0].a, a.doors[0].b), (Side::E, 6, 8));
        assert_eq!(b.role, Role::Boss);
        assert!(b.doors[0].boss);
        assert_eq!(a.role, Role::Set);
        assert!(!a.lanes.is_empty());
        assert!((2..=3).contains(&a.spots.len()), "{:?}", a.spots);
        // The same rooms from a graph: its rects and roles.
        let g = [(Rect::new(2, 3, 12, 10), Role::Rest), (Rect::new(26, 4, 12, 12), Role::Boss)];
        let d = Dungeon::find(test_theme(), (w, h), tile, std::iter::empty(), &g, None, 7);
        assert_eq!(d.room_at(5, 5).map(|r| r.role), Some(Role::Rest));
        assert_eq!(d.room_at(30, 8).map(|r| r.role), Some(Role::Boss));
        assert_eq!(d.room_id(20, 7), None);
    }

    /// A theme no data row has dresses a synthetic dungeon as the data's dress Jane's: every room
    /// gets its two-cell face, its border and its motif on screen.
    #[test]
    fn a_new_theme_row_frames_a_synthetic_dungeon() {
        use crate::terrain::{CHUNK_CELLS, Chunk, Painter, TileMap, paint_chunk};
        let (w, h, t) = plan();
        let mut map = TileMap::new(jane_core::grid::Grid::from_vec(w as u32, h as u32, t.clone()), false);
        let tile = |x: i32, y: i32| t[(y * w + x) as usize];
        map.dungeon = Some(Dungeon::find(test_theme(), (w, h), tile, std::iter::empty(), &[], Some((30, 8)), 7));
        let (mut p, mut c) = (Painter::new(), Chunk::new());
        let mut got = vec![0u8; 2];
        for cy in 0..(h + CHUNK_CELLS - 1) / CHUNK_CELLS {
            for cx in 0..(w + CHUNK_CELLS - 1) / CHUNK_CELLS {
                paint_chunk(&mut p, &map, 7, cx, cy, &mut c);
                for (g, f) in got.iter_mut().zip(p.framed()) {
                    *g |= f;
                }
            }
        }
        assert_eq!(got, [FRAMED_TALL | FRAMED_BORDER | FRAMED_MOTIF; 2]);
    }

    /// Every ramp a theme row names is a palette ramp (the schema cannot see the palette).
    #[test]
    fn every_theme_ramp_is_a_palette_ramp() {
        let themes = jane_data::dungeon_themes();
        assert!(!themes.themes.is_empty());
        for t in themes.themes {
            let floors = t.floor.iter().map(|f| f.ramp);
            let names =
                [t.trim_ramp, t.frame, t.inlay].into_iter().chain(t.border_ramps).chain(t.emblem_ramps).chain(floors);
            for n in names.filter(|n| !n.is_empty()) {
                assert!(Ramp::by_name(n).is_some(), "theme {}: no ramp is called \"{n}\"", t.id);
            }
        }
    }

    #[test]
    fn a_segment_measures_its_distance() {
        assert_eq!(seg_dist(5, 3, 0, 0, 10, 0), 3);
        assert_eq!(seg_dist(-4, 3, 0, 0, 10, 0), 5);
    }
}
