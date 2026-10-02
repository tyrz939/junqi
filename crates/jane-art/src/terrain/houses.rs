//! Houses with owners (ART-PLAN Q2, M7, M4): every tile-built house in a zone (a block of
//! `HouseWall`, `HouseRoof` and `Eaves`) found once when the zone is entered, given a seed, and
//! from the seed a [`Look`]: its roof, its walls, its door, its windows' rhythm, what grows up
//! it, its chimney's pots, and the garden in front of it with its boundary. A room seen from
//! inside (Julie's house, the Arms, St Anne's) is a [`Room`], whose seed papers its walls.
//!
//! All of it is drawing: a house's look moves no tile, no collision and no hash of the world. The
//! seeds are picked in reading order so that no two houses a frame can hold at once share three
//! or more of roof, wall, door, window rhythm, boundary and hero detail (ART-PLAN §7 rule 1).

use jane_core::Tile;
use jane_core::grid::Rect;

use crate::hash::{below, h32};
use crate::palette::Ramp;

/// The salt a house's seed is picked under.
const SALT: u32 = 0x484f_5553;
/// The salt a look reads its seed under.
const LOOK: u32 = 0x4c4f_4f4b;
/// A frame's reach in cells (the scene's 768 x 432 at 16 px): two houses nearer than this each
/// way can be seen at once.
pub const VIEW: (i32, i32) = (48, 27);
/// The most rows of garden in front of a house.
const PLOT_ROWS: i32 = 3;
/// Bucket side, cells, of the lookup.
const BUCKET: i32 = 16;

/// A roof's material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Roof {
    /// Clay tiles.
    Tile,
    /// Welsh slate.
    Slate,
    /// Long straw.
    Thatch,
}

/// How a roof has aged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Age {
    /// Darkened, a tile here and there gone pale.
    Weathered,
    /// Newly laid: bright and even.
    New,
    /// Moss in its courses.
    Mossy,
}

/// A house's walls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wall {
    /// Plaster in this ramp: cream, Suffolk pink, limewash or ochre.
    Plaster(Ramp),
    /// Red brick in Flemish bond.
    Brick,
    /// Knapped flint with brick quoins at the corners and round the openings.
    Flint,
}

/// How its door is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DoorStyle {
    /// Four panels and a fanlight over it.
    Panelled,
    /// A cottage's ledged boards.
    Ledged,
    /// A stable door: two leaves, the top one apart from the bottom.
    Stable,
}

/// Its windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Window {
    /// Side-hung, four panes.
    Casement,
    /// Taller, six panes over a meeting rail.
    Sash,
    /// Ground floor bays under a lead roof; casements above.
    Bay,
}

/// What climbs its face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Climber {
    /// Ivy up a corner, to the eave.
    Ivy,
    /// Wisteria under the eave, its racemes hanging.
    Wisteria,
    /// A climbing rose round the door.
    Rose,
}

/// Its front garden (ART-PLAN M7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Garden {
    /// Flowers in clusters, a rose arch over the gate, a crazy-paving path.
    Cottage,
    /// Cabbages in rows, bean canes, a water butt.
    Veg,
    /// A lawn, a bird bath, a sundial.
    Tidy,
    /// Long grass and a rusted bike.
    Neglect,
}

/// What bounds the garden, and the gate in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Boundary {
    /// White pickets, a gate of them.
    Picket,
    /// A low wall of brick or stone with a coping, a timber gate.
    LowWall,
    /// A clipped privet hedge, a gap with a gate.
    Privet,
    /// Iron railings on a plinth, an iron gate.
    Railings,
}

/// What a building is, as the door in it says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    /// A home, a shop: anyone's.
    #[default]
    Home,
    /// The Castle Arms.
    Inn,
    /// St Anne's.
    Church,
}

/// What a building is by where its door opens: into the Arms an inn, into St Anne's a church.
pub fn door_kind(to: Option<jane_core::ids::ZoneId>) -> Kind {
    match to {
        Some(jane_core::ids::ZoneId::Arms) => Kind::Inn,
        Some(jane_core::ids::ZoneId::Church) => Kind::Church,
        _ => Kind::Home,
    }
}

/// The paints a front door takes, in [`Look::door`] order: racing green, oxblood, navy, black.
pub const DOORS: [Ramp; 4] = [Ramp::RacingGreen, Ramp::Oxblood, Ramp::ClothNavy, Ramp::ClothBlack];
/// The plasters, in pick order.
const PLASTERS: [Ramp; 4] = [Ramp::Plaster, Ramp::PlasterPink, Ramp::Limewash, Ramp::PlasterOchre];

/// A house's look, all from its seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Look {
    /// The roof's material.
    pub roof: Roof,
    /// How the roof has aged.
    pub age: Age,
    /// The walls.
    pub wall: Wall,
    /// A timber beam under the eave (plaster only).
    pub beam: bool,
    /// The door's paint, an index into [`DOORS`].
    pub door: u8,
    /// How the door is made.
    pub door_style: DoorStyle,
    /// The windows.
    pub window: Window,
    /// A window every `period` cells, `phase` cells from the west end.
    pub period: u8,
    /// See `period`.
    pub phase: u8,
    /// Shutters in the door's colour (one house in three).
    pub shutters: bool,
    /// A window box under each ground floor window (one in two).
    pub boxes: bool,
    /// What climbs the face (one in four).
    pub climber: Option<Climber>,
    /// A brass nameplate by the door, else a painted number.
    pub nameplate: bool,
    /// Chimney pots, 1 to 3, on a stack 0 to 2 steps tall.
    pub pots: u8,
    /// See `pots`.
    pub stack: u8,
    /// The front garden.
    pub garden: Garden,
    /// Its boundary and gate.
    pub boundary: Boundary,
}

impl Look {
    /// The look seed `seed` gives a building of kind `kind`. The church is flint under slate
    /// with no garden of its own (its yard is the graves); the rest are what the seed says.
    pub fn of(seed: u32, kind: Kind) -> Look {
        let d = |i: u32, n: u32| below(h32(seed, i, LOOK), n);
        let roof = match d(0, 10) {
            0..=4 => Roof::Tile,
            5..=7 => Roof::Slate,
            _ => Roof::Thatch,
        };
        let age = [Age::Weathered, Age::New, Age::Mossy][d(1, 3) as usize];
        let wall = match d(2, 12) {
            0..=6 => Wall::Plaster(PLASTERS[(d(3, 7) * 4 / 7) as usize]),
            7..=9 => Wall::Brick,
            _ => Wall::Flint,
        };
        let window = match d(5, 7) {
            0..=2 => Window::Casement,
            3..=5 => Window::Sash,
            _ => Window::Bay,
        };
        let period = 3 + d(6, 2) as u8;
        let garden = match d(12, 8) {
            0..=2 => Garden::Cottage,
            3..=4 => Garden::Veg,
            5..=6 => Garden::Tidy,
            _ => Garden::Neglect,
        };
        let b = d(13, 10);
        // Each recipe its usual boundary, and another now and then.
        let (usual, other, odds) = match garden {
            Garden::Cottage => (Boundary::Picket, Boundary::Privet, 7),
            Garden::Veg => (Boundary::LowWall, Boundary::Picket, 6),
            Garden::Tidy => (Boundary::Railings, Boundary::Privet, 5),
            Garden::Neglect => (Boundary::Picket, Boundary::LowWall, 6),
        };
        let boundary = if b < odds { usual } else { other };
        let look = Look {
            roof,
            age,
            wall,
            beam: matches!(wall, Wall::Plaster(_)) && d(4, 3) == 0,
            door: d(7, 4) as u8,
            door_style: [DoorStyle::Panelled, DoorStyle::Ledged, DoorStyle::Stable][d(8, 3) as usize],
            window,
            period,
            phase: d(9, u32::from(period)) as u8,
            shutters: d(10, 3) == 0,
            boxes: d(11, 2) == 0,
            climber: (d(14, 4) == 0).then(|| [Climber::Ivy, Climber::Wisteria, Climber::Rose][d(15, 3) as usize]),
            nameplate: d(16, 2) == 0,
            pots: 1 + d(17, 3) as u8,
            stack: d(18, 3) as u8,
            garden,
            boundary,
        };
        match kind {
            Kind::Home => look,
            Kind::Inn => Look { roof: Roof::Tile, climber: None, ..look },
            Kind::Church => Look {
                roof: Roof::Slate,
                age: Age::Weathered,
                wall: Wall::Flint,
                beam: false,
                window: Window::Sash,
                period: 4,
                shutters: false,
                boxes: false,
                climber: (d(14, 2) == 0).then_some(Climber::Ivy),
                ..look
            },
        }
    }

    /// The door's paint.
    pub fn door_ramp(&self) -> Ramp {
        DOORS[usize::from(self.door) % DOORS.len()]
    }

    /// The door sprite's variant (`kit`'s house doors): paint by style.
    pub fn door_variant(&self) -> u32 {
        u32::from(self.door) * 3 + self.door_style as u32
    }

    /// The chimney sprite's variant: pots by stack.
    pub fn chimney_variant(&self) -> u32 {
        u32::from(self.pots - 1) * 3 + u32::from(self.stack)
    }

    /// The one detail a passer-by would name the house by: what climbs it, its shutters and its
    /// window boxes, as one value.
    pub fn hero(&self) -> u8 {
        self.climber.map_or(0, |c| c as u8 + 1) * 4 + u8::from(self.shutters) * 2 + u8::from(self.boxes)
    }

    /// How many of the six things ART-PLAN §7 rule 1 names it shares with `o`: roof (and its
    /// age), wall, door (paint and make), window rhythm, boundary and hero detail.
    pub fn shared(&self, o: &Look) -> u32 {
        u32::from(self.roof == o.roof && self.age == o.age)
            + u32::from(self.wall == o.wall)
            + u32::from(self.door == o.door && self.door_style == o.door_style)
            + u32::from(self.window == o.window && self.period == o.period && self.phase == o.phase)
            + u32::from(self.boundary == o.boundary)
            + u32::from(self.hero() == o.hero())
    }
}

/// A house as found: its block, the row its walls start on, its front garden and its door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct House {
    /// The block of roof and wall, cells.
    pub rect: Rect,
    /// The first row of wall (the roof is the rows above it).
    pub eave: i32,
    /// The garden in front of it (empty: none), cells.
    pub plot: Rect,
    /// The west cell of its door, if a door is set in its front.
    pub door: Option<i32>,
    /// Its seed, picked under rule 1.
    pub seed: u32,
    /// What it is, by its door.
    pub kind: Kind,
}

impl House {
    /// Its look.
    pub fn look(&self) -> Look {
        Look::of(self.seed, self.kind)
    }

    /// Whether cell `(x, y)` is its roof or wall.
    pub fn holds(&self, x: i32, y: i32) -> bool {
        self.rect.contains(x, y)
    }
}

/// Whether `t` is a house's roof or wall.
fn built(t: Tile) -> bool {
    matches!(t, Tile::HouseWall | Tile::HouseRoof | Tile::Eaves)
}

/// Whether `t` is ground a front garden is laid on.
fn plot_ground(t: Tile) -> bool {
    matches!(t, Tile::Grass | Tile::FlowerBed | Tile::Garden | Tile::GrassTall)
}

/// Every house of a zone, with a lookup by cell. Filled once a zone; nothing is allocated per
/// chunk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Houses {
    list: Vec<House>,
    /// Bucket `(x / BUCKET, y / BUCKET)` to the houses whose block or garden touch it.
    buckets: std::collections::BTreeMap<(i32, i32), Vec<u16>>,
}

impl Houses {
    /// Find the houses of a zone of `(w, h)` cells whose tile at `(x, y)` is `tile(x, y)`, with
    /// its doors at `doors` (the west cell of each, the cell it stands on, and what it opens
    /// into), and pick their seeds under world seed `seed`.
    pub fn fill(&mut self, (w, h): (i32, i32), tile: impl Fn(i32, i32) -> Tile, doors: &[(i32, i32, Kind)], seed: u32) {
        self.list.clear();
        self.buckets.clear();
        let mut seen = vec![0u64; ((w.max(0) * h.max(0)) as usize).div_ceil(64)];
        let mark = |seen: &mut Vec<u64>, x: i32, y: i32| {
            let k = (y * w + x) as usize;
            let was = seen[k / 64] >> (k % 64) & 1 == 1;
            seen[k / 64] |= 1 << (k % 64);
            was
        };
        let mut stack = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if !built(tile(x, y)) || mark(&mut seen, x, y) {
                    continue;
                }
                // The block, 4-connected.
                let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
                let mut eave = i32::MAX;
                stack.push((x, y));
                while let Some((cx, cy)) = stack.pop() {
                    (x0, y0, x1, y1) = (x0.min(cx), y0.min(cy), x1.max(cx), y1.max(cy));
                    if tile(cx, cy) == Tile::HouseWall {
                        eave = eave.min(cy);
                    }
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, ny) = (cx + dx, cy + dy);
                        if nx >= 0 && ny >= 0 && nx < w && ny < h && built(tile(nx, ny)) && !mark(&mut seen, nx, ny) {
                            stack.push((nx, ny));
                        }
                    }
                }
                if eave == i32::MAX {
                    continue;
                }
                let rect = Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
                // The garden: the rows in front of it that are mostly grass, three at most.
                let mut rows = 0;
                while rows < PLOT_ROWS {
                    let gy = rect.bottom() + rows;
                    let green = (rect.x..rect.right()).filter(|&gx| plot_ground(tile(gx, gy))).count() as i32;
                    if gy >= h || green * 3 < rect.w * 2 {
                        break;
                    }
                    rows += 1;
                }
                let plot = Rect::new(rect.x, rect.bottom(), rect.w, rows);
                let door = doors.iter().find(|d| d.1 >= eave && rect.contains(d.0, d.1));
                self.list.push(House {
                    rect,
                    eave,
                    plot,
                    door: door.map(|d| d.0),
                    seed: 0,
                    kind: door.map_or(Kind::Home, |d| d.2),
                });
            }
        }
        // Seeds in reading order, each the first of its tries that shares fewer than three of
        // rule 1's six with every house picked before it that a frame can hold beside it.
        for i in 0..self.list.len() {
            let me = self.list[i];
            let mut best = (u32::MAX, 0);
            for k in 0..64u32 {
                let s = h32(me.rect.x as u32, me.rect.y as u32, seed ^ SALT ^ k.wrapping_mul(0x9e37_79b9));
                let look = Look::of(s, me.kind);
                let worst = self.list[..i]
                    .iter()
                    .filter(|o| in_view(o.rect, me.rect))
                    .map(|o| look.shared(&o.look()))
                    .max()
                    .unwrap_or(0);
                if worst < best.0 {
                    best = (worst, s);
                }
                if worst < 3 {
                    break;
                }
            }
            self.list[i].seed = best.1;
        }
        for (i, house) in self.list.iter().enumerate() {
            let r = Rect::new(house.rect.x, house.rect.y, house.rect.w, house.rect.h + house.plot.h);
            for by in r.y.div_euclid(BUCKET)..=(r.bottom() - 1).div_euclid(BUCKET) {
                for bx in r.x.div_euclid(BUCKET)..=(r.right() - 1).div_euclid(BUCKET) {
                    self.buckets.entry((bx, by)).or_default().push(i as u16);
                }
            }
        }
    }

    /// The house whose block or garden holds cell `(x, y)`.
    pub fn at(&self, x: i32, y: i32) -> Option<&House> {
        let b = self.buckets.get(&(x.div_euclid(BUCKET), y.div_euclid(BUCKET)))?;
        b.iter().map(|&i| &self.list[usize::from(i)]).find(|h| h.rect.contains(x, y) || h.plot.contains(x, y))
    }

    /// Every house, in reading order.
    pub fn list(&self) -> &[House] {
        &self.list
    }
}

/// Whether two blocks can stand in one frame: nearer than [`VIEW`] each way.
pub fn in_view(a: Rect, b: Rect) -> bool {
    let gap = |a0: i32, a1: i32, b0: i32, b1: i32| (b0 - a1).max(a0 - b1).max(0);
    gap(a.x, a.right(), b.x, b.right()) < VIEW.0 && gap(a.y, a.bottom(), b.y, b.bottom()) < VIEW.1
}

/// A room seen from inside (ART-PLAN M4): whose it is, its seed, its back wall and what hangs
/// on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// Whose it is.
    pub kind: RoomKind,
    /// Its seed: the paper on its walls, what hangs on them.
    pub seed: u32,
    /// The columns of the back wall something tall stands against (a dresser, a shelf), a bit a
    /// cell: no window or picture is hung behind them.
    pub busy: u128,
    /// Per column, the row of floor under the back wall's face, plus one (0: no back wall): the
    /// face drawn two cells tall over it.
    pub face: [u8; ROOM_COLS],
    /// Per column, what hangs on the back wall there ([`Decor`] as a byte).
    pub decor: [u8; ROOM_COLS],
}

/// The widest room, cells.
pub const ROOM_COLS: usize = 128;

impl Default for Room {
    fn default() -> Room {
        Room { kind: RoomKind::Julie, seed: 0, busy: 0, face: [0; ROOM_COLS], decor: [0; ROOM_COLS] }
    }
}

/// What hangs on a room's back wall (ART-PLAN M4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Decor {
    /// Nothing.
    None = 0,
    /// A sash window: the sky by day, laying light on the floor; dark at night.
    Window,
    /// A landscape in a gilt frame.
    Picture,
    /// A wall clock (Julie's has stopped).
    Clock,
    /// A calendar on a nail.
    Calendar,
    /// A mirror.
    Mirror,
    /// A rack of plates.
    PlateRack,
    /// The Arms' dartboard and its chalk board.
    Dartboard,
    /// The inn sign's twin.
    Sign,
    /// St Anne's hymn board.
    HymnBoard,
}

impl Decor {
    /// The decor a byte is.
    pub fn of(b: u8) -> Decor {
        [
            Decor::None,
            Decor::Window,
            Decor::Picture,
            Decor::Clock,
            Decor::Calendar,
            Decor::Mirror,
            Decor::PlateRack,
            Decor::Dartboard,
            Decor::Sign,
            Decor::HymnBoard,
        ]
        .get(usize::from(b))
        .copied()
        .unwrap_or(Decor::None)
    }
}

/// Whose room it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RoomKind {
    /// Julie's house: her clock stopped.
    #[default]
    Julie,
    /// The Castle Arms: a dartboard and the sign's twin.
    Inn,
    /// St Anne's: the hymn board.
    Church,
}

impl Room {
    /// Whether column `x` of the back wall is free to hang a thing on.
    pub fn free(&self, x: i32) -> bool {
        !(0..128).contains(&x) || self.busy >> x & 1 == 0
    }

    /// The row of floor under the back wall's face in column `x`, if the back wall runs there.
    pub fn face_foot(&self, x: i32) -> Option<i32> {
        let f = *self.face.get(usize::try_from(x).ok()?)?;
        (f > 0).then(|| i32::from(f) - 1)
    }

    /// What hangs on the back wall in column `x`.
    pub fn decor_at(&self, x: i32) -> Decor {
        usize::try_from(x).ok().and_then(|i| self.decor.get(i)).map_or(Decor::None, |&b| Decor::of(b))
    }

    /// Find the back wall of a room `w` cells wide whose solid cells `solid` says, and hang it:
    /// down each column the first face under a wall two cells deep, in the top three rows; then
    /// along it, west to east, the room's things in turn, a cell or two apart, none behind
    /// anything tall.
    pub fn hang(&mut self, w: i32, solid: impl Fn(i32, i32) -> bool) {
        for x in 0..w.min(ROOM_COLS as i32) {
            if let Some(y) = (1..=2).find(|&y| solid(x, y) && solid(x, y - 1) && !solid(x, y + 1)) {
                self.face[x as usize] = (y + 2) as u8;
            }
        }
        let list: &[Decor] = match self.kind {
            RoomKind::Julie => &[
                Decor::PlateRack,
                Decor::Window,
                Decor::Calendar,
                Decor::Clock,
                Decor::Window,
                Decor::Picture,
                Decor::Mirror,
            ],
            RoomKind::Inn => &[
                Decor::Window,
                Decor::Dartboard,
                Decor::Picture,
                Decor::Window,
                Decor::Sign,
                Decor::Mirror,
                Decor::Window,
            ],
            RoomKind::Church => &[Decor::Window, Decor::HymnBoard, Decor::Window, Decor::Window],
        };
        let (mut i, mut last) = (0, -9);
        for x in 1..w.min(ROOM_COLS as i32) - 1 {
            let wall = |x: i32| self.face_foot(x).is_some();
            let hangs = wall(x - 1) && wall(x) && wall(x + 1) && self.free(x);
            if !hangs || x - last < 2 {
                continue;
            }
            if h32(self.seed, x as u32, 0xdec0) % 3 == 0 {
                continue;
            }
            self.decor[x as usize] = list[i % list.len()] as u8;
            (i, last) = (i + 1, x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_spread_over_every_choice() {
        let looks: Vec<Look> = (0..400).map(|s| Look::of(h32(s, 0, 1), Kind::Home)).collect();
        let n = |f: &dyn Fn(&Look) -> u32| {
            let mut v: Vec<u32> = looks.iter().map(f).collect();
            v.sort();
            v.dedup();
            v.len()
        };
        assert_eq!(n(&|l| l.roof as u32), 3);
        assert_eq!(n(&|l| l.age as u32), 3);
        assert_eq!(n(&|l| l.door_variant()), 12);
        assert_eq!(n(&|l| l.chimney_variant()), 9);
        assert_eq!(n(&|l| l.garden as u32), 4);
        assert_eq!(n(&|l| l.boundary as u32), 4);
        let walls: std::collections::BTreeSet<String> = looks.iter().map(|l| format!("{:?}", l.wall)).collect();
        assert_eq!(walls.len(), 6, "{walls:?}");
        let third = looks.iter().filter(|l| l.shutters).count();
        assert!((100..170).contains(&third), "one in three shuttered: {third}");
        let climbing = looks.iter().filter(|l| l.climber.is_some()).count();
        assert!((70..130).contains(&climbing), "one in four climbed: {climbing}");
    }

    #[test]
    fn a_row_of_houses_is_found_and_no_two_in_view_share_three() {
        // Eight houses in a row, a cell of grass between each, two rows of grass in front.
        let (w, h) = (90, 12);
        let tile = |x: i32, y: i32| {
            let inside = x % 11 != 10 && x < 88;
            match y {
                1..=5 if inside => Tile::HouseRoof,
                6..=8 if inside => Tile::HouseWall,
                _ => Tile::Grass,
            }
        };
        let mut hs = Houses::default();
        hs.fill((w, h), tile, &[(3, 7, Kind::Home), (25, 7, Kind::Inn)], 7);
        assert_eq!(hs.list().len(), 8);
        let a = hs.list()[0];
        assert_eq!((a.rect, a.eave, a.plot, a.door), (Rect::new(0, 1, 10, 8), 6, Rect::new(0, 9, 10, 3), Some(3)));
        assert_eq!(hs.list()[2].kind, Kind::Inn);
        assert_eq!(hs.at(4, 10).map(|h| h.rect.x), Some(0), "its garden");
        assert_eq!(hs.at(10, 3), None, "the gap");
        for (i, p) in hs.list().iter().enumerate() {
            for q in &hs.list()[..i] {
                assert!(p.look().shared(&q.look()) < 3, "{:?} and {:?}", p.look(), q.look());
            }
        }
        let mut again = Houses::default();
        again.fill((w, h), tile, &[(3, 7, Kind::Home), (25, 7, Kind::Inn)], 7);
        assert_eq!(again, hs, "the same zone, the same houses");
    }
}
