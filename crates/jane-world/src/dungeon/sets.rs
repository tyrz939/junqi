//! Set pieces (DUNGEONS.md §2.10): a few authored arrangements of props per room, stood whole
//! after everything the mission, the templates and the lamps have put down, and before the
//! scatter. A rug under a table and its chairs, a heap against a wall, a workbench under its
//! rack of tools: a room takes up to `most` of the pieces its node lists (`sets.rooms`), the
//! first tried first and the rest in a seeded order.
//!
//! A piece is dressing and never a lock. Its parts are inert (the build refuses a part that
//! pushes, carries, lights, blocks sight or is used), and where it may stand is decided here,
//! cell by cell, so that it can never stand in a check's way:
//!
//! - every part lies on the dungeon's own floor tile, on a cell nothing has claimed and no
//!   `fill` will ever change, and outside the threshold of every door it uses (three cells
//!   each way of the doorway);
//! - a solid part keeps a cell from anything a player uses (two from a thing a spell wakes), a
//!   cell from a unit, a mark and the room's own middle cell (the one the checks walk between),
//!   out of the five-cell lane from each door in to the middle of the room, and clear of the
//!   ground between a plate and what is pushed or carried onto it;
//! - the worn paths stay clear: the shortest walk from each door to every other door and every
//!   goal (`SetRoom::goals`) has no solid part on it or beside it (`desire_lines`);
//! - where anything is fought (`SetRoom::calm` false) a piece with anything solid in it keeps to
//!   the walls; the generator furnishes no arena at all, and no room with a thing to push that
//!   no plate waits for;
//! - the room stays whole: every cell she could reach from a door before, she still can, and
//!   every cell outside the piece's own box that a body two cells wide could reach, one still
//!   can, so a piece never leaves a gap of one cell (a thing the solver's flood would walk and
//!   a person could not). That is also what keeps a piece two cells off anything used on the
//!   sides it can be reached from.
//!
//! Where: `n` stands its back row flush to a north wall, `s` its front row to a south wall,
//! `side` its side to the west wall (laid out mirrored against the east), `corner` into a north
//! corner, `wall` any of those four, `free` anywhere with a clear cell all round it.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::{PropDefId, Rect, Sfc32};
use jane_data::{MissionSetPiece, RoomSide, SetAgainst, catalog};

/// The zone as the set pieces see it.
#[derive(Clone, Debug)]
pub struct Floor {
    pub w: i32,
    pub h: i32,
    /// A part may lie here: the dungeon's own floor, unclaimed, never filled.
    pub open: Vec<bool>,
    /// Feet cross it: terrain that is not solid, and no solid prop that stays.
    pub walk: Vec<bool>,
    /// A solid part may not stand here.
    pub keep: Vec<bool>,
    /// The dungeon's wall tile: what a piece stands flush against.
    pub wall: Vec<bool>,
}

impl Floor {
    pub fn new(w: i32, h: i32) -> Self {
        let n = (w * h) as usize;
        Self { w, h, open: vec![false; n], walk: vec![false; n], keep: vec![false; n], wall: vec![false; n] }
    }

    pub fn ix(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    fn get(v: &[bool], f: &Floor, x: i32, y: i32) -> bool {
        f.ix(x, y).is_some_and(|i| v[i])
    }

    /// Mark `r` grown by `by` cells all round as a place no solid part stands.
    pub fn keep_off(&mut self, r: Rect, by: i32) {
        for y in r.y - by..r.bottom() + by {
            for x in r.x - by..r.right() + by {
                if let Some(i) = self.ix(x, y) {
                    self.keep[i] = true;
                }
            }
        }
    }
}

/// A room to furnish: its floor and the doors it uses (the door's middle rim cell and its side).
#[derive(Clone, Debug)]
pub struct SetRoom {
    pub rect: Rect,
    pub doors: Vec<(i32, i32, RoomSide)>,
    /// What she walks to in the room: the things she uses, the units, the marks, its middle.
    pub goals: Vec<Rect>,
    /// Nothing is fought here (no heat): a piece may stand out in the floor. Where there is a
    /// fight, every piece keeps to the walls and the floor is left to it.
    pub calm: bool,
}

/// The ways she walks through the room: from each door, the shortest walk (on the room as it
/// is before any piece) to every other door and to every goal. No solid part stands on one of
/// them or beside it, so a piece never makes a walk longer, and the room reads with its paths
/// worn clear between its furniture.
fn desire_lines(f: &mut Floor, room: &SetRoom) {
    let r = room.rect;
    let at = |x: i32, y: i32| -> Option<usize> {
        (x >= r.x && y >= r.y && x < r.right() && y < r.bottom()).then(|| ((y - r.y) * r.w + (x - r.x)) as usize)
    };
    let mids: Vec<(i32, i32)> = room
        .doors
        .iter()
        .map(|&(dx, dy, side)| {
            let (ix, iy) = inward(side);
            (dx + ix, dy + iy)
        })
        .collect();
    let mut targets: Vec<Rect> = mids.iter().map(|&(x, y)| Rect::new(x, y, 1, 1)).collect();
    targets.extend(room.goals.iter().copied());
    let mut worn: Vec<(i32, i32)> = Vec::new();
    for &(sx, sy) in &mids {
        let Some(s) = at(sx, sy) else { continue };
        let mut from = vec![usize::MAX; (r.w * r.h) as usize];
        from[s] = s;
        let mut queue = alloc::collections::VecDeque::from([(sx, sy)]);
        while let Some((x, y)) = queue.pop_front() {
            let i = at(x, y).expect("queued inside");
            for (nx, ny) in [(x, y - 1), (x + 1, y), (x, y + 1), (x - 1, y)] {
                if let Some(n) = at(nx, ny) {
                    if from[n] == usize::MAX && Floor::get(&f.walk, f, nx, ny) {
                        from[n] = i;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        for t in &targets {
            // The nearest reached cell in or beside the goal, then back along the walk.
            let near = (t.y - 1..=t.bottom())
                .flat_map(|y| (t.x - 1..=t.right()).map(move |x| (x, y)))
                .filter_map(|(x, y)| at(x, y).filter(|&i| from[i] != usize::MAX).map(|i| (i, x, y)))
                .min_by_key(|&(_, x, y)| ((x - sx).abs() + (y - sy).abs(), y, x));
            let Some((mut i, _, _)) = near else { continue };
            while i != s {
                worn.push((r.x + (i as i32 % r.w), r.y + (i as i32 / r.w)));
                i = from[i];
            }
        }
    }
    for (x, y) in worn {
        f.keep_off(Rect::new(x, y, 1, 1), 1);
    }
}

/// One part stood: the prop and its cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub prop: PropDefId,
    pub x: i32,
    pub y: i32,
}

/// How many placements a piece tries the floods on before it gives up in this room.
const TRIES: usize = 40;

const fn inward(side: RoomSide) -> (i32, i32) {
    match side {
        RoomSide::N => (0, 1),
        RoomSide::S => (0, -1),
        RoomSide::W => (1, 0),
        RoomSide::E => (-1, 0),
    }
}

impl SetRoom {
    /// Within three cells of a doorway's three cells: the threshold, where nothing lies.
    fn at_door(&self, x: i32, y: i32) -> bool {
        self.doors.iter().any(|&(dx, dy, _)| (x - dx).abs() <= 3 && (y - dy).abs() <= 3)
    }

    /// In the five-cell lane from a door in to the middle of the room: the way in is kept clear
    /// as far as where she can see the whole room and choose.
    fn in_lane(&self, x: i32, y: i32) -> bool {
        let r = self.rect;
        self.doors.iter().any(|&(dx, dy, side)| match side {
            RoomSide::N => (x - dx).abs() <= 2 && y < r.y + r.h / 2 + 1,
            RoomSide::S => (x - dx).abs() <= 2 && y >= r.y + r.h / 2 - 1,
            RoomSide::W => (y - dy).abs() <= 2 && x < r.x + r.w / 2 + 1,
            RoomSide::E => (y - dy).abs() <= 2 && x >= r.x + r.w / 2 - 1,
        })
    }

    /// The floor cells just inside each door: where a flood of the room starts.
    fn seeds(&self) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for &(dx, dy, side) in &self.doors {
            let (ix, iy) = inward(side);
            let along = if matches!(side, RoomSide::N | RoomSide::S) { (1, 0) } else { (0, 1) };
            for s in -1..=1 {
                v.push((dx + along.0 * s + ix, dy + along.1 * s + iy));
            }
        }
        v
    }
}

/// What she can reach in the room, one cell wide and two cells wide.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Reach {
    one: Vec<bool>,
    two: Vec<bool>,
}

fn reach(f: &Floor, room: &SetRoom, extra: &[(i32, i32)]) -> Reach {
    let r = room.rect;
    let (rw, rh) = (r.w, r.h);
    let n = (rw * rh) as usize;
    let at = |x: i32, y: i32| -> Option<usize> {
        (x >= r.x && y >= r.y && x < r.right() && y < r.bottom()).then(|| ((y - r.y) * rw + (x - r.x)) as usize)
    };
    let walk = |x: i32, y: i32| Floor::get(&f.walk, f, x, y) && !extra.contains(&(x, y));
    let mut one = vec![false; n];
    let mut stack: Vec<(i32, i32)> = Vec::new();
    for (x, y) in room.seeds() {
        if let Some(i) = at(x, y).filter(|_| walk(x, y)) {
            if !one[i] {
                one[i] = true;
                stack.push((x, y));
            }
        }
    }
    while let Some((x, y)) = stack.pop() {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if let Some(i) = at(nx, ny).filter(|_| walk(nx, ny)) {
                if !one[i] {
                    one[i] = true;
                    stack.push((nx, ny));
                }
            }
        }
    }
    // Two wide: the 2x2 boxes (by their top-left cell) that stand on walkable cells, flooded from
    // any box over a seed; a cell is covered if a reached box covers it.
    let fits2 = |x: i32, y: i32| walk(x, y) && walk(x + 1, y) && walk(x, y + 1) && walk(x + 1, y + 1);
    let inside2 = |x: i32, y: i32| x >= r.x && y >= r.y && x + 1 < r.right() && y + 1 < r.bottom();
    let mut boxes = vec![false; n];
    let seeds = room.seeds();
    for &(sx, sy) in &seeds {
        for (bx, by) in [(sx, sy), (sx - 1, sy), (sx, sy - 1), (sx - 1, sy - 1)] {
            if inside2(bx, by) && fits2(bx, by) {
                let i = at(bx, by).expect("inside");
                if !boxes[i] {
                    boxes[i] = true;
                    stack.push((bx, by));
                }
            }
        }
    }
    while let Some((x, y)) = stack.pop() {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if inside2(nx, ny) && fits2(nx, ny) {
                let i = at(nx, ny).expect("inside");
                if !boxes[i] {
                    boxes[i] = true;
                    stack.push((nx, ny));
                }
            }
        }
    }
    let mut two = vec![false; n];
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if boxes[at(x, y).expect("inside")] {
                for (cx, cy) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
                    two[at(cx, cy).expect("a box lies inside")] = true;
                }
            }
        }
    }
    Reach { one, two }
}

/// The piece's parts at `(x, y)`, laid out mirrored if `flip`: each part's prop, its cell, its
/// footprint and whether it is solid.
fn layout(piece: &MissionSetPiece, x: i32, y: i32, flip: bool) -> Vec<(PropDefId, Rect, bool)> {
    let c = catalog();
    piece
        .parts
        .iter()
        .map(|p| {
            let d = c.story.prop(p.prop);
            let (pw, ph) = (i32::from(d.w), i32::from(d.h));
            let px = if flip { x + i32::from(piece.w) - (i32::from(p.x) + pw) } else { x + i32::from(p.x) };
            (p.prop, Rect::new(px, y + i32::from(p.y), pw, ph), d.solid)
        })
        .collect()
}

/// Every place the piece might stand in the room, as `(x, y, flip)`.
fn places(f: &Floor, room: &SetRoom, piece: &MissionSetPiece, rng: &mut Sfc32) -> Vec<(i32, i32, bool)> {
    let r = room.rect;
    let (w, h) = (i32::from(piece.w), i32::from(piece.h));
    // A piece nothing stands up in (toadstools, a runner) is walked over, fight or no fight.
    let walk_over = piece.parts.iter().all(|p| !catalog().story.prop(p.prop).solid);
    let wall = |x: i32, y: i32| Floor::get(&f.wall, f, x, y);
    let row_wall = |x: i32, y: i32| (x..x + w).all(|i| wall(i, y));
    let col_wall = |x: i32, y: i32| (y..y + h).all(|j| wall(x, j));
    let free_ring = |x: i32, y: i32| {
        (x - 1..=x + w).all(|i| {
            (y - 1..=y + h).all(|j| {
                (i >= x && i < x + w && j >= y && j < y + h)
                    || (Floor::get(&f.walk, f, i, j) && !wall(i, j) && r.contains(i, j))
            })
        })
    };
    let mut v = Vec::new();
    for y in r.y..=r.bottom() - h {
        for x in r.x..=r.right() - w {
            let (n, s, west, east) = (row_wall(x, y - 1), row_wall(x, y + h), col_wall(x - 1, y), col_wall(x + w, y));
            let flip_free = piece.mirror && rng.below(2) == 1;
            // Against the east wall (or into a north-east corner) the parts are laid out mirrored.
            let place = match piece.against {
                SetAgainst::N => n.then_some(flip_free),
                SetAgainst::S => s.then_some(flip_free),
                SetAgainst::Wall if n || s => Some(flip_free),
                SetAgainst::Side | SetAgainst::Wall => (west || east).then_some(!west),
                SetAgainst::Corner => (n && (west || east)).then_some(!west),
                SetAgainst::Free => ((room.calm || walk_over) && free_ring(x, y)).then_some(flip_free),
            };
            if let Some(flip) = place {
                v.push((x, y, flip));
            }
        }
    }
    rng.shuffle(&mut v);
    v
}

/// Whether the parts may stand where `layout` put them: the cell rules, then the room still
/// whole with them in it.
fn stands(f: &Floor, room: &SetRoom, parts: &[(PropDefId, Rect, bool)], before: &Reach) -> Option<Reach> {
    let mut solid: Vec<(i32, i32)> = Vec::new();
    let mut bx = Rect::new(i32::MAX, i32::MAX, 0, 0);
    let (mut x1, mut y1) = (i32::MIN, i32::MIN);
    for &(_, p, is_solid) in parts {
        for y in p.y..p.bottom() {
            for x in p.x..p.right() {
                if !room.rect.contains(x, y) || !Floor::get(&f.open, f, x, y) || room.at_door(x, y) {
                    return None;
                }
                if is_solid {
                    if Floor::get(&f.keep, f, x, y) || room.in_lane(x, y) {
                        return None;
                    }
                    solid.push((x, y));
                }
                bx.x = bx.x.min(x);
                bx.y = bx.y.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    bx.w = x1 - bx.x;
    bx.h = y1 - bx.y;
    let after = reach(f, room, &solid);
    let r = room.rect;
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let i = ((y - r.y) * r.w + (x - r.x)) as usize;
            let lost_one = before.one[i] && !after.one[i] && !solid.contains(&(x, y));
            let lost_two = before.two[i] && !after.two[i] && !bx.contains(x, y);
            if lost_one || lost_two {
                return None;
            }
        }
    }
    Some(after)
}

/// Furnish one room: up to `most` of `pieces`, in the order given (the caller seeds it). Each
/// stood piece claims its cells in `f` (a solid part is no longer walkable, no part's cell is
/// open again), so the next piece, and the next room, see it.
pub fn furnish(
    f: &mut Floor,
    room: &SetRoom,
    pieces: &[&MissionSetPiece],
    most: usize,
    rng: &mut Sfc32,
) -> Vec<Placed> {
    let mut out = Vec::new();
    desire_lines(f, room);
    let mut now = reach(f, room, &[]);
    let mut stood = 0;
    for piece in pieces {
        if stood >= most {
            break;
        }
        let mut tried = 0;
        for (x, y, flip) in places(f, room, piece, rng) {
            let parts = layout(piece, x, y, flip);
            // The cheap rules first: only a place that passes them is flooded.
            let cheap = parts.iter().all(|&(_, p, is_solid)| {
                (p.y..p.bottom()).all(|j| {
                    (p.x..p.right()).all(|i| {
                        room.rect.contains(i, j)
                            && Floor::get(&f.open, f, i, j)
                            && !room.at_door(i, j)
                            && (!is_solid || (!Floor::get(&f.keep, f, i, j) && !room.in_lane(i, j)))
                    })
                })
            });
            if !cheap {
                continue;
            }
            tried += 1;
            if let Some(after) = stands(f, room, &parts, &now) {
                for &(prop, p, is_solid) in &parts {
                    for j in p.y..p.bottom() {
                        for i in p.x..p.right() {
                            let ix = f.ix(i, j).expect("inside the room");
                            f.open[ix] = false;
                            if is_solid {
                                f.walk[ix] = false;
                            }
                        }
                    }
                    out.push(Placed { prop, x: p.x, y: p.y });
                }
                now = after;
                stood += 1;
                break;
            }
            if tried >= TRIES {
                break;
            }
        }
    }
    out
}
