//! One unit's making and moving (`sim/units.ts`).
//!
//! **Movement** is a 6 x 6 px box on the feet ([`BODY_HALF_FX`]), axis-separated: x first, then
//! y, each sliding flush against what it meets: terrain by the cell, a solid prop by where its
//! look meets the ground (`PropDef::solid_parts`, in sixteenths of a cell), so from the north she
//! walks up to a crate's back though paths keep its cells. Units never block each other's feet;
//! they shape paths through occupancy instead, so nothing wedges in a corridor.

use jane_core::action::Facing;
use jane_core::angle::{cos_q15, sin_q15};
use jane_core::num::CELL_FX;
use jane_core::{Angle, Fx, Milli, Tick, UnitDefId, Vec2};
use jane_data::UnitDef;

use crate::grid::{Meets, ZoneGrid};
use crate::ids::UnitId;
use crate::runtime::ZoneRuntime;
use crate::state::{CombatState, Patrol, SnakeBody, Unit};
use crate::sym::of_name;
use crate::tuning::{BODY_HALF_FX, ENERGY_MAX, HP_PER_STRENGTH, MP_PER_SPIRIT};

pub fn def_of(u: &Unit) -> &'static UnitDef {
    jane_data::catalog().combat.unit(u.def)
}

pub fn max_hp(u: &Unit) -> Milli {
    Milli::from_points(i32::from(u.strength) * HP_PER_STRENGTH)
}

pub fn max_mp(u: &Unit) -> Milli {
    Milli::from_points(i32::from(u.spirit) * MP_PER_SPIRIT)
}

/// The aggro scan's stagger, from the id: no draw at spawn (ARCHITECTURE.md §3.2).
pub const fn think_offset(id: UnitId) -> u32 {
    (id.get().wrapping_mul(0x9E37)) % 10
}

/// The facing's angle.
pub const fn facing_angle(f: Facing) -> Angle {
    match f {
        Facing::East => Angle::EAST,
        Facing::South => Angle::SOUTH,
        Facing::West => Angle::WEST,
        Facing::North => Angle::NORTH,
    }
}

/// A unit of `def` at `pos`, alive, asleep (the ring wakes it), with its row's stats.
pub fn new_unit(
    id: UnitId,
    key: Option<jane_core::Sym>,
    def_id: UnitDefId,
    pos: Vec2,
    facing: Facing,
    now: Tick,
) -> Unit {
    let def = jane_data::catalog().combat.unit(def_id);
    let snake = def.body.map(|b| {
        Box::new(SnakeBody {
            heading: facing_angle(facing),
            phase_tick: now,
            step: 0,
            trail: vec![pos; usize::from(b.segments)],
        })
    });
    let mut u = Unit {
        id,
        key,
        def: def_id,
        controller: def.controller,
        faction: def.faction,
        pos,
        facing,
        strength: def.strength,
        spirit: def.spirit,
        hp: Milli::ZERO,
        mp: Milli::ZERO,
        energy: ENERGY_MAX,
        energy_locked: false,
        alive: true,
        gcd_until: Tick::ZERO,
        stop_until: Tick::ZERO,
        cooldowns: Vec::new(),
        item_cooldowns: Vec::new(),
        synced: now,
        target: None,
        combat: CombatState::Idle,
        home: pos,
        patrol: None,
        patrol_at: 0,
        dwell_until: Tick::ZERO,
        order: None,
        path: None,
        statuses: Vec::new(),
        died_at: None,
        awake: false,
        hidden: false,
        carrying: None,
        hold: 0,
        phase: 0,
        snake,
    };
    u.hp = max_hp(&u);
    u.mp = max_mp(&u);
    u
}

/// Growth by finding lands on a body: the stat rises, and the health or mana it brings is hers
/// at once if she is alive (finding a jar at 10 hp should feel like finding a jar).
pub fn grow_body(u: &mut Unit, stat: jane_core::action::Stat, amount: i16) {
    use jane_core::action::Stat;
    let gain = Milli::from_points(i32::from(amount) * HP_PER_STRENGTH);
    match stat {
        Stat::Strength => {
            u.strength = u.strength.saturating_add_signed(amount);
            if u.alive {
                u.hp = Milli((u.hp.0 + gain.0).min(max_hp(u).0));
            }
        }
        Stat::Spirit => {
            u.spirit = u.spirit.saturating_add_signed(amount);
            if u.alive {
                u.mp = Milli((u.mp.0 + gain.0).min(max_mp(u).0));
            }
        }
    }
}

/// A patrol from blueprint waypoints (cell centres); `None` for fewer than two.
pub fn patrol_of(points: &[jane_core::blueprint::Waypoint]) -> Option<Box<Patrol>> {
    (points.len() > 1).then(|| {
        Box::new(Patrol {
            points: points.iter().map(|w| (Vec2::centre(i32::from(w.cell.x), i32::from(w.cell.y)), w.dwell)).collect(),
        })
    })
}

/// A content name's key.
pub fn key_of_name(n: jane_core::NameId) -> Option<jane_core::Sym> {
    Some(of_name(n))
}

/// Dominant axis wins; a tie keeps the current axis so diagonals do not flicker
/// (`units.ts faceVector`).
pub fn face_vector(u: &mut Unit, dx: i64, dy: i64) {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax == 0 && ay == 0 {
        return;
    }
    u.facing = if ax > ay || (ax == ay && matches!(u.facing, Facing::East | Facing::West)) {
        if dx > 0 { Facing::East } else { Facing::West }
    } else if dy > 0 {
        Facing::South
    } else {
        Facing::North
    };
}

/// Face along an angle, by the same rule: an exact 45 degrees keeps the current axis.
pub fn face_angle(u: &mut Unit, a: Angle) {
    face_vector(u, i64::from(cos_q15(a).0), i64::from(sin_q15(a).0));
}

/// A sixteenth of a cell (a canvas px), the grain of a prop's feet.
const SUB_FX: i32 = CELL_FX / 16;

/// An axis-aligned box in `Fx`, half-open: `[x0, x1) x [y0, y1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

/// Every solid piece feet meet inside `b`, to `f`: a whole cell, or one sixteenth-row of a
/// cell's props' feet from its first to its last solid sixteenth inside `b`'s columns. Stops
/// and returns true the first time `f` does.
fn pieces(g: &ZoneGrid, b: Span, mut f: impl FnMut(Span) -> bool) -> bool {
    for cy in Fx(b.y0).cell()..=Fx(b.y1 - 1).cell() {
        for cx in Fx(b.x0).cell()..=Fx(b.x1 - 1).cell() {
            let (ox, oy) = (cx * CELL_FX, cy * CELL_FX);
            match g.feet_meet(cx, cy) {
                Meets::Open => {}
                Meets::Whole => {
                    if f(Span { x0: ox, y0: oy, x1: ox + CELL_FX, y1: oy + CELL_FX }) {
                        return true;
                    }
                }
                Meets::Part(rows) => {
                    let c0 = (b.x0 - ox).max(0) / SUB_FX;
                    let c1 = (b.x1 - 1 - ox).min(CELL_FX - 1) / SUB_FX;
                    let cols = ((1u32 << (c1 + 1)) - (1u32 << c0)) as u16;
                    let r0 = (b.y0 - oy).max(0) / SUB_FX;
                    let r1 = (b.y1 - 1 - oy).min(CELL_FX - 1) / SUB_FX;
                    for r in r0..=r1 {
                        let m = rows[r as usize] & cols;
                        if m == 0 {
                            continue;
                        }
                        let (lo, hi) = (m.trailing_zeros() as i32, 16 - m.leading_zeros() as i32);
                        let y = oy + r * SUB_FX;
                        if f(Span { x0: ox + lo * SUB_FX, y0: y, x1: ox + hi * SUB_FX, y1: y + SUB_FX }) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// The body box with its feet at `(x, y)`.
fn body_at(x: i32, y: i32) -> Span {
    Span { x0: x - BODY_HALF_FX, y0: y - BODY_HALF_FX, x1: x + BODY_HALF_FX, y1: y + BODY_HALF_FX }
}

/// Is the body box at `(x, y)` in anything solid to feet: terrain, or a prop's feet?
pub fn box_blocked(g: &ZoneGrid, x: Fx, y: Fx) -> bool {
    pieces(g, body_at(x.0, y.0), |_| true)
}

/// How far the body box `b` moves by `delta` along one axis before it meets something: all of
/// it if nothing is in the way, else flush against the nearest thing ahead. What it already
/// touches is not ahead (the caller's last check keeps a body that stays inside something).
fn slide(g: &ZoneGrid, b: Span, horizontal: bool, delta: i32) -> i32 {
    let (lo, hi) = if horizontal { (b.x0, b.x1) } else { (b.y0, b.y1) };
    let swept = match (horizontal, delta > 0) {
        (true, true) => Span { x1: b.x1 + delta, ..b },
        (true, false) => Span { x0: b.x0 + delta, ..b },
        (false, true) => Span { y1: b.y1 + delta, ..b },
        (false, false) => Span { y0: b.y0 + delta, ..b },
    };
    let mut reach = delta.abs();
    pieces(g, swept, |p| {
        let (p0, p1) = if horizontal { (p.x0, p.x1) } else { (p.y0, p.y1) };
        if delta > 0 && p0 >= hi {
            reach = reach.min(p0 - hi);
        } else if delta < 0 && p1 <= lo {
            reach = reach.min(lo - p1);
        }
        false
    });
    reach * delta.signum()
}

/// Move by `(dx, dy)` with axis-separated sliding; keeps occupancy and unit blocks. Returns
/// whether it moved.
pub fn move_unit(rt: &mut ZoneRuntime, u: &mut Unit, dx: Fx, dy: Fx) -> bool {
    let from = u.pos;
    let g = &rt.grid;
    if dx.0 != 0 {
        let nx = Fx(u.pos.x.0 + dx.0);
        u.pos.x = if box_blocked(g, nx, u.pos.y) {
            Fx(u.pos.x.0 + slide(g, body_at(u.pos.x.0, u.pos.y.0), true, dx.0))
        } else {
            nx
        };
    }
    if dy.0 != 0 {
        let ny = Fx(u.pos.y.0 + dy.0);
        u.pos.y = if box_blocked(g, u.pos.x, ny) {
            Fx(u.pos.y.0 + slide(g, body_at(u.pos.x.0, u.pos.y.0), false, dy.0))
        } else {
            ny
        };
    }
    if box_blocked(g, u.pos.x, u.pos.y) {
        u.pos = from;
    }
    if u.pos == from {
        return false;
    }
    rt.moved(u);
    true
}

/// Teleport with the bookkeeping: travel, respawn, the console.
pub fn place_unit(rt: &mut ZoneRuntime, u: &mut Unit, pos: Vec2) {
    u.pos = pos;
    u.path = None;
    rt.moved(u);
}

pub fn spend_energy(u: &mut Unit, amount: Milli) {
    u.energy = Milli((u.energy.0 - amount.0).max(0));
    if u.energy.0 == 0 {
        u.energy_locked = true;
    }
}

pub fn restore_energy(u: &mut Unit, amount: Milli) {
    u.energy = Milli((u.energy.0 + amount.0).min(ENERGY_MAX.0));
    if u.energy >= ENERGY_MAX {
        u.energy_locked = false;
    }
}

#[cfg(test)]
mod tests {
    use jane_core::grid::Grid;
    use jane_core::{Rect, Tile};

    use super::*;
    use crate::state::ZoneState;

    fn runtime(tiles: Grid<Tile>) -> ZoneRuntime {
        let mut bp = jane_core::Blueprint::new(jane_core::ZoneId::House, tiles.w(), tiles.h(), Tile::Floor);
        bp.tiles = tiles;
        let zone = crate::zone::empty_zone_state(&bp, 1);
        ZoneRuntime::build(&bp, &zone, Vec::new())
    }

    fn body(rt: &mut ZoneRuntime, zone: &mut ZoneState, pos: Vec2) -> UnitId {
        let jane = jane_data::catalog().combat.unit_id("jane").unwrap();
        let mut u = new_unit(UnitId::new(1).unwrap(), None, jane, pos, Facing::South, Tick::ZERO);
        u.awake = true;
        rt.add_unit(&u);
        zone.insert_unit(u);
        UnitId::new(1).unwrap()
    }

    fn at(x: i32, y: i32) -> Vec2 {
        Vec2::new(Fx::from_px(x), Fx::from_px(y))
    }

    #[test]
    fn slides_flush_along_a_wall_and_keeps_occupancy() {
        let mut tiles = Grid::new(10, 10, Tile::Floor);
        tiles.fill_rect(Rect::new(5, 0, 1, 10), Tile::Wall);
        let mut rt = runtime(tiles);
        let mut zone =
            crate::zone::empty_zone_state(&jane_core::Blueprint::new(jane_core::ZoneId::House, 10, 10, Tile::Floor), 1);
        let id = body(&mut rt, &mut zone, at(20, 20));
        let u = zone.unit_mut(id).unwrap();
        assert_eq!(rt.grid.occupants(2, 2), 1);
        // East into the wall at x = 40 px: flush at 40 - 3.
        for _ in 0..30 {
            move_unit(&mut rt, u, Fx::from_px(1), Fx::ZERO);
        }
        assert_eq!(u.pos, at(37, 20));
        assert_eq!(rt.grid.occupants(2, 2), 0);
        assert_eq!(rt.grid.occupants(4, 2), 1);
        // Pressed into the wall, it only slides along it.
        assert!(move_unit(&mut rt, u, Fx::from_px(1), Fx::from_px(1)));
        assert_eq!(u.pos, at(37, 21));
        assert!(!move_unit(&mut rt, u, Fx::from_px(1), Fx::ZERO));
        // A step that ends past what it cannot cross stops flush against it, however far it came.
        let b = body_at(Fx::from_px(20).0, Fx::from_px(20).0);
        assert_eq!(slide(&rt.grid, b, true, Fx::from_px(30).0), Fx::from_px(17).0);
        assert_eq!(slide(&rt.grid, b, true, Fx::from_px(-30).0), Fx::from_px(-17).0, "the grid's edge");
    }

    /// The owner: "many items should just have a little bounding box on the ground, but it seems
    /// to extend to the size of the sprite". Every solid prop with feet, walked onto from the north
    /// down the middle of them: her body box stops on the ground its look stands on, never more
    /// than 2 px short of it and never in it; paths still see its cells.
    #[test]
    fn from_the_north_feet_stop_on_every_prop_s_ground() {
        let cat = jane_data::catalog();
        let (px, py) = (10, 20);
        let mut far = Vec::new();
        let mut n = 0;
        for def in cat.story.props.iter().filter(|d| d.solid) {
            let Some([fx, fy, fw, _]) = def.feet else { continue };
            let mut g = ZoneGrid::new(Grid::new(40, 40, Tile::Floor));
            let parts = def.solid_parts().map(|r| Rect::new(r.x + px * 16, r.y + py * 16, r.w, r.h));
            g.stamp_prop_parts(def.solid_rect(px, py), false, &parts);
            let x = px * CELL_FX + (i32::from(fx) * 2 + i32::from(fw)) * SUB_FX / 2;
            let b = body_at(x, 12 * CELL_FX);
            let bottom = b.y1 + slide(&g, b, false, 12 * CELL_FX);
            let gap = py * CELL_FX + i32::from(fy) * SUB_FX - bottom;
            if !(0..=2 * 256).contains(&gap) {
                far.push(format!("{}: {} px short", def.id, f64::from(gap) / 256.0));
            }
            assert!(def.solid_rect(px, py).cells().all(|(x, y)| g.solid(x, y)), "{}: cells", def.id);
            n += 1;
        }
        assert!(n > 150, "most solid props have feet ({n})");
        assert!(far.is_empty(), "stopped short of or inside the ground: {far:#?}");
    }

    #[test]
    fn a_one_cell_corridor_is_walkable() {
        let mut tiles = Grid::new(10, 3, Tile::Wall);
        tiles.fill_rect(Rect::new(0, 1, 10, 1), Tile::Floor);
        let mut rt = runtime(tiles);
        let mut zone =
            crate::zone::empty_zone_state(&jane_core::Blueprint::new(jane_core::ZoneId::House, 10, 3, Tile::Floor), 1);
        let id = body(&mut rt, &mut zone, Vec2::centre(0, 1));
        let u = zone.unit_mut(id).unwrap();
        // 100 px east from x = 4 px: stopped flush against the edge of the grid (80 - 3).
        for _ in 0..100 {
            move_unit(&mut rt, u, Fx::from_px(1), Fx::ZERO);
        }
        assert_eq!(u.pos, Vec2::new(Fx::from_px(77), Vec2::centre(0, 1).y));
        assert_eq!(rt.unit_blocks.len(), 1);
    }

    #[test]
    fn facing_by_dominant_axis_and_ties_keep_the_axis() {
        let jane = jane_data::catalog().combat.unit_id("jane").unwrap();
        let mut u = new_unit(UnitId::new(3).unwrap(), None, jane, Vec2::ZERO, Facing::East, Tick::ZERO);
        face_angle(&mut u, Angle::from_degrees(135));
        assert_eq!(u.facing, Facing::West);
        face_angle(&mut u, Angle::from_degrees(100));
        assert_eq!(u.facing, Facing::South);
        face_angle(&mut u, Angle::from_degrees(-45));
        assert_eq!(u.facing, Facing::North);
        assert_eq!(think_offset(UnitId::new(3).unwrap()), (3 * 0x9E37) % 10);
    }

    #[test]
    fn energy_locks_when_spent_and_unlocks_only_when_full() {
        let jane = jane_data::catalog().combat.unit_id("jane").unwrap();
        let mut u = new_unit(UnitId::new(1).unwrap(), None, jane, Vec2::ZERO, Facing::East, Tick::ZERO);
        spend_energy(&mut u, Milli(200_000));
        assert!(u.energy_locked && u.energy == Milli(0));
        restore_energy(&mut u, Milli(99_999));
        assert!(u.energy_locked);
        restore_energy(&mut u, Milli(5));
        assert!(!u.energy_locked && u.energy == ENERGY_MAX);
    }
}
