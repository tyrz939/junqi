//! Where the lamps go (`lights.ts`). A lamp hangs on a wall, in the wall cell itself, so it
//! never stands in anyone's way and never claims a floor cell a crate might be pushed through.
//! Nothing here is rolled: the same rooms and corridors are lit the same way on every seed.
//!
//! - rooms: a pair flanking every door the room uses (a doorway, or a door prop standing against
//!   the wall: a way out, a stair), one clear cell from the opening, never over it; then along
//!   each wall at the dungeon's rhythm, spaced evenly between those pairs and the corners. North
//!   and south walls at `every`, east and west at half the count.
//! - corridors: one every `corridor` cells, on the wall a lamp would be seen on (the north face
//!   of a corridor running east-west, alternate sides of one running north-south).
//! - dark: a node marked `dark`, and every corridor that leads to a boss or a mini-boss. The walk
//!   to him is the one walk nobody lit.

use jane_core::num::div_round;
use jane_core::tile::F_SOLID;
use jane_core::{Grid, Rect, Tile};
use jane_data::{MissionLights, RoomSide};

/// A room to light: its node and its shape's box in zone cells.
#[derive(Clone, Copy, Debug)]
pub struct LitRoom {
    pub node: usize,
    pub dark: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// What one corridor carved, and whether it leads to a boss.
#[derive(Clone, Debug, Default)]
pub struct LitCorridor {
    pub rects: Vec<Rect>,
    pub dark: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lamp {
    /// `<zone>_<node>_lamp_<n>` for a room's, `<zone>_lamp_<n>` for a corridor's.
    pub name: String,
    /// The wall it hangs on, as seen from the floor it lights.
    pub side: RoomSide,
    pub x: i32,
    pub y: i32,
}

/// Into the room from a wall on that side.
pub const fn into(side: RoomSide) -> (i32, i32) {
    match side {
        RoomSide::N => (0, 1),
        RoomSide::S => (0, -1),
        RoomSide::W => (1, 0),
        RoomSide::E => (-1, 0),
    }
}

/// Along that wall.
const fn along(side: RoomSide) -> (i32, i32) {
    match side {
        RoomSide::N | RoomSide::S => (1, 0),
        RoomSide::W | RoomSide::E => (0, 1),
    }
}

/// The index of a side's row in a lamp family (`_n`, `_e`, `_s`, `_w`).
pub const fn family_index(side: RoomSide) -> usize {
    match side {
        RoomSide::N => 0,
        RoomSide::E => 1,
        RoomSide::S => 2,
        RoomSide::W => 3,
    }
}

/// Indices along a run of wall of length `len` where lamps hang. `door_at_0` / `door_at_end`: the
/// run stops at a doorway there (else at a corner). A doorway gets a lamp one cell clear of it;
/// the rest are spread evenly, and a corner counts as half a gap, so lamps sit symmetrically.
pub fn lamps_along(len: i32, every: i32, door_at_0: bool, door_at_end: bool) -> Vec<i32> {
    if len <= 0 {
        return Vec::new();
    }
    let half = every / 2;
    let mut out = Vec::new();
    let a = if door_at_0 { 1.min(len - 1) } else { -half };
    let b = if door_at_end { 0.max(len - 2) } else { len - 1 + half };
    if door_at_0 {
        out.push(a);
    }
    if door_at_end {
        out.push(b);
    }
    let gap = b - a;
    let m = div_round(i64::from(gap), i64::from(every)).max(1);
    for i in 1..m {
        let p = a + div_round(i * i64::from(gap), m) as i32;
        if (0..len).contains(&p) {
            out.push(p);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn open(tiles: &Grid<Tile>, x: i32, y: i32) -> bool {
    tiles.read(x, y, Tile::Void).flags() & F_SOLID == 0
}

/// The wall a lamp standing at (x, y) should hang on instead: north first, the face she sees,
/// then west, east, south. The lamp goes in that wall cell. `None` if no wall touches the cell.
pub fn mount_on(tiles: &Grid<Tile>, wall: Tile, x: i32, y: i32) -> Option<(RoomSide, i32, i32)> {
    [RoomSide::N, RoomSide::W, RoomSide::E, RoomSide::S].into_iter().find_map(|side| {
        let (ix, iy) = into(side);
        (tiles.read(x - ix, y - iy, Tile::Void) == wall).then_some((side, x - ix, y - iy))
    })
}

/// Every lamp of the dungeon, rooms first (in room order), then the corridors (in reading order).
/// `reserved` marks wall cells a lamp of the mission's own holds; `doors` are door props
/// standing against a wall, which lamps flank like doorways.
#[allow(clippy::too_many_arguments)]
pub fn place_lamps(
    zone: &str,
    node_ids: &[&str],
    plan: &MissionLights,
    tiles: &Grid<Tile>,
    wall: Tile,
    rooms: &[LitRoom],
    corridors: &[LitCorridor],
    reserved: &[bool],
    doors: &[Rect],
) -> Vec<Lamp> {
    let (w, h) = (tiles.w() as i32, tiles.h() as i32);
    let at = |x: i32, y: i32| (y * w + x) as usize;
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    let mut lamps = Vec::new();
    let mut taken = reserved.to_vec();
    let is_wall = |x: i32, y: i32| tiles.read(x, y, Tile::Void) == wall;
    // A door prop (a way out, a stair) stands against a wall: its footprint, and the stretch of
    // wall it stands against, is an opening like any doorway, so the lamps flank it one cell clear.
    let mut door = vec![false; (w * h) as usize];
    for d in doors {
        for (x, y) in d.cells() {
            if inside(x, y) {
                door[at(x, y)] = true;
            }
        }
    }
    let is_door = |x: i32, y: i32| inside(x, y) && door[at(x, y)];
    let by_door = |x: i32, y: i32| {
        is_door(x, y) || is_door(x + 1, y) || is_door(x - 1, y) || is_door(x, y + 1) || is_door(x, y - 1)
    };
    let near_door = |x: i32, y: i32| (-1..=1).any(|oy| (-1..=1).any(|ox| is_door(x + ox, y + oy)));
    let mut add = |lamps: &mut Vec<Lamp>, name: String, side: RoomSide, x: i32, y: i32| {
        // Never over a door or in the cell beside it, whatever put it there.
        if taken[at(x, y)] || near_door(x, y) {
            return;
        }
        taken[at(x, y)] = true;
        lamps.push(Lamp { name, side, x, y });
    };

    // --- rooms -------------------------------------------------------------------------------
    for r in rooms {
        if r.dark {
            continue;
        }
        let in_box = |x: i32, y: i32| x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h;
        let inner = |x: i32, y: i32| x > r.x && y > r.y && x < r.x + r.w - 1 && y < r.y + r.h - 1;
        let mut n = 0;
        for side in [RoomSide::N, RoomSide::S, RoomSide::W, RoomSide::E] {
            let (ix, iy) = into(side);
            let (ax, ay) = along(side);
            let every = i32::from(if matches!(side, RoomSide::N | RoomSide::S) { plan.every } else { plan.every * 2 });
            // A cell of this room's wall that shows its face to the room's floor on this side. The
            // floor it faces is inside the rim: the cells of a doorway are not a room to light.
            let face = |x: i32, y: i32| {
                in_box(x, y) && is_wall(x, y) && inner(x + ix, y + iy) && open(tiles, x + ix, y + iy) && !by_door(x, y)
            };
            let opening = |x: i32, y: i32| in_box(x, y) && (open(tiles, x, y) || by_door(x, y));
            let mut seen = vec![false; (r.w * r.h) as usize];
            let local = |x: i32, y: i32| ((y - r.y) * r.w + (x - r.x)) as usize;
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    if seen[local(x, y)] || !face(x, y) || face(x - ax, y - ay) {
                        continue;
                    }
                    // A run of face from here, along the wall.
                    let mut len = 0;
                    while face(x + ax * len, y + ay * len) {
                        seen[local(x + ax * len, y + ay * len)] = true;
                        len += 1;
                    }
                    // The run ends in an opening (a doorway, or the room going on round a pillar) or a corner.
                    let door_at_0 = opening(x - ax, y - ay);
                    let door_at_end = opening(x + ax * len, y + ay * len);
                    // Only the room's own rim flanks doors; a wall standing inside the room is just wall.
                    let rim = x - r.x == 0 || y - r.y == 0 || x - r.x == r.w - 1 || y - r.y == r.h - 1;
                    for i in lamps_along(len, every, door_at_0 && rim, door_at_end && rim) {
                        let name = format!("{zone}_{}_lamp_{n}", node_ids[r.node]);
                        n += 1;
                        add(&mut lamps, name, side, x + ax * i, y + ay * i);
                    }
                }
            }
        }
    }

    // --- corridors ---------------------------------------------------------------------------
    if plan.corridor > 0 {
        // 1 lit corridor, -1 dark corridor.
        let mut mask = vec![0i8; (w * h) as usize];
        for c in corridors {
            for q in &c.rects {
                for (x, y) in q.cells() {
                    if !inside(x, y) {
                        continue;
                    }
                    let i = at(x, y);
                    // Where a lit corridor shares the lane, the lane is lit: only the last stretch is his.
                    if !c.dark {
                        mask[i] = 1;
                    } else if mask[i] == 0 {
                        mask[i] = -1;
                    }
                }
            }
        }
        let mut boxes = vec![false; (w * h) as usize];
        for r in rooms {
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    if inside(x, y) {
                        boxes[at(x, y)] = true;
                    }
                }
            }
        }
        let in_room = |x: i32, y: i32| boxes[at(x, y)];
        let lit = |x: i32, y: i32| inside(x, y) && mask[at(x, y)] == 1 && open(tiles, x, y) && !in_room(x, y);
        let every = i32::from(plan.corridor);
        let phase = |v: i32, shift: i32| (v + shift) % every == 0;
        let mut n = 0;
        for y in 0..h {
            for x in 0..w {
                if !is_wall(x, y) || in_room(x, y) {
                    continue;
                }
                // East-west: the face above the corridor, the one a lamp is seen on.
                let side = if lit(x, y + 1) && lit(x, y + 2) && lit(x, y + 3) && phase(x, 0) {
                    RoomSide::N
                // North-south: the two walls in turn, so the light steps down the passage.
                } else if lit(x + 1, y) && lit(x + 2, y) && lit(x + 3, y) && phase(y, 0) {
                    RoomSide::W
                } else if lit(x - 1, y) && lit(x - 2, y) && lit(x - 3, y) && phase(y, every >> 1) {
                    RoomSide::E
                } else {
                    continue;
                };
                let name = format!("{zone}_lamp_{n}");
                n += 1;
                add(&mut lamps, name, side, x, y);
            }
        }
    }
    lamps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lamps_spread_evenly_half_a_gap_from_a_corner_and_a_cell_clear_of_a_doorway() {
        // Half a gap (4) from each corner, a gap (8) apart.
        assert_eq!(lamps_along(25, 9, false, false), vec![4, 4 + 8, 4 + 2 * 8]);
        let flanked = lamps_along(20, 9, true, true);
        assert_eq!(flanked.first(), Some(&1));
        assert_eq!(flanked.last(), Some(&18));
        assert_eq!(lamps_along(1, 9, false, false), Vec::<i32>::new());
        assert_eq!(lamps_along(2, 9, true, false), vec![1]);
    }

    #[test]
    fn a_lamp_mounts_north_first() {
        let mut g = Grid::new(5, 5, Tile::CaveFloor);
        g.set(2, 1, Tile::CaveWall);
        g.set(1, 2, Tile::CaveWall);
        assert_eq!(mount_on(&g, Tile::CaveWall, 2, 2), Some((RoomSide::N, 2, 1)));
        assert_eq!(mount_on(&g, Tile::CaveWall, 1, 3), Some((RoomSide::N, 1, 2)));
        assert_eq!(mount_on(&g, Tile::CaveWall, 3, 3), None);
    }
}
