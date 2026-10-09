//! Height in the sim (MAP.md §2, §3; R1): the rules a zone with levels adds, each asked only
//! where [`ZoneGrid::has_levels`] says there are levels, so a flat zone (every zone built so far)
//! steps, saves and hashes exactly as it did.
//!
//! - **A unit's level** is the level of the ground under its feet ([`level_of`]): a function of
//!   its position, read where it is needed and never stored, so it can never disagree with a
//!   rebuild (MAP R2 keeps it in the unit when the save's version moves).
//! - **The ledge's hop** (MAP.md §2.4): a body walking (or pushed) into a ledge's face in the
//!   ledge's direction is carried over the face to the first walkable cell beyond it, in
//!   [`LEDGE_HOP_TICKS`], with no control, no swing and no cast. It rides the push's own state
//!   (`feel::Knock` with [`LEDGE_HOP_BIT`] in its `left`), so the save's form is unchanged and
//!   a save mid-hop loads mid-hop. Into a ledge from any other side, it is a wall.
//! - **Ladders** halve the pace, and nothing is swung or cast on one.
//! - **Blows across height** (MAP.md §3.3): melee reaches only its own level, or one level over a
//!   join (a stair's or a ramp's step, no face between); a pool and a splash touch only bodies at
//!   the level of the ground they land on.
//! - **Who hops** (MAP.md §3.4): she does; an AI does inside its leash unless it is a boss, big
//!   (`feel::Build::Heavy`, or not pushed at all) or a perch row (`holds: level`); nothing else.

use jane_core::tile::{BLOCK_MOVE, F_SOLID};
use jane_core::{Fx, Tile, Vec2};
use jane_data::{Controller, UnitDef};

use crate::feel::{Build, Knock};
use crate::grid::ZoneGrid;
use crate::runtime::ZoneRuntime;
use crate::state::Unit;
use crate::tuning::{BODY_HALF_FX, LEDGE_FACE_MAX, LEDGE_HOP_BIT, LEDGE_HOP_TICKS};

/// The level of the ground under `p`.
#[inline]
pub fn level_of(g: &ZoneGrid, p: Vec2) -> u8 {
    let (x, y) = p.cell();
    g.level_at(x, y)
}

/// Do `a` and `b` stand on ground of one level? Always, in a zone without levels.
#[inline]
pub fn same_level(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    !g.has_levels() || level_of(g, a) == level_of(g, b)
}

/// Can a blow struck at `a` reach a body at `b` (MAP.md §3.3)? On one level, as ever; one level
/// apart only across a join, where nothing solid lies between them (a face, a ledge, a rim is
/// solid; a stair's or a ramp's step is not). Never further apart.
pub fn melee_reaches(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    if !g.has_levels() {
        return true;
    }
    let (la, lb) = (level_of(g, a), level_of(g, b));
    la == lb || la.abs_diff(lb) == 1 && crate::los::walk_cells(a, b, |x, y| g.flags_at(x, y) & F_SOLID != 0).is_none()
}

/// Is this body being carried over a ledge?
#[inline]
pub fn hopping(u: &Unit) -> bool {
    u.feel.knock.is_some_and(|k| k.left & LEDGE_HOP_BIT != 0)
}

/// Is this body's feet on a ladder?
#[inline]
pub fn on_ladder(g: &ZoneGrid, p: Vec2) -> bool {
    let (x, y) = p.cell();
    g.has_levels() && g.tile_at(x, y) == Tile::Ladder
}

/// Neither swing nor cast: mid-hop, or on a ladder.
pub fn cannot_strike(g: &ZoneGrid, u: &Unit) -> bool {
    g.has_levels() && (hopping(u) || on_ladder(g, u.pos))
}

/// Where a hop over the ledge cell `(x, y)` in direction `dir` comes down: past every face cell
/// of that ledge (at most [`LEDGE_FACE_MAX`]), the first cell feet may stand on; with how many
/// face cells it crossed. `None` if `(x, y)` is no ledge hopped that way, or nothing walkable
/// lies beyond.
pub fn ledge_landing(g: &ZoneGrid, (x, y): (i32, i32), dir: (i32, i32)) -> Option<((i32, i32), i32)> {
    let t = g.tile_at(x, y);
    if !g.has_levels() || t.ledge_dir() != Some(dir) {
        return None;
    }
    let (mut cx, mut cy, mut faces) = (x, y, 0);
    while g.inside(cx, cy) && g.tile_at(cx, cy) == t && faces < LEDGE_FACE_MAX {
        cx += dir.0;
        cy += dir.1;
        faces += 1;
    }
    (g.inside(cx, cy) && g.flags_at(cx, cy) & BLOCK_MOVE == 0).then_some(((cx, cy), faces))
}

/// May this unit hop a ledge whose landing is at `to`? She may; an AI inside its leash unless it
/// is a boss, big or a perch row; nothing else.
pub fn may_hop(zone: jane_core::ZoneId, u: &Unit, def: &UnitDef, to: Vec2) -> bool {
    match u.controller {
        Controller::Player => true,
        Controller::Ai => hops(u, def) && crate::combat::distance(to, u.home) <= crate::ai::leash_in(zone, def),
        Controller::Npc | Controller::Snake => false,
    }
}

/// An AI row that may hop at all: not a boss, not big, not a perch.
pub fn hops(u: &Unit, def: &UnitDef) -> bool {
    !def.boss && !def.holds_level && matches!(crate::feel::build(u), Build::Small | Build::Medium)
}

/// A body moving by `(dx, dy)` into a ledge's face in the ledge's direction (along the axis it
/// mostly moves, or either on an exact diagonal): its hop begins, and true. It rises no further
/// this call; `feel::step_knocks` carries it from this tick on.
pub fn try_hop(rt: &mut ZoneRuntime, u: &mut Unit, def: &UnitDef, dx: Fx, dy: Fx) -> bool {
    if hopping(u) || !u.alive {
        return false;
    }
    let (ax, ay) = (dx.0.abs(), dy.0.abs());
    let axes = [(ay >= ax && ay > 0, (0, dy.0.signum())), (ax >= ay && ax > 0, (dx.0.signum(), 0))];
    for (moving, dir) in axes {
        if !moving {
            continue;
        }
        let reach = BODY_HALF_FX + ax.max(ay);
        let probe = Vec2 { x: Fx(u.pos.x.0 + dir.0 * reach), y: Fx(u.pos.y.0 + dir.1 * reach) };
        let Some(((lx, ly), _)) = ledge_landing(&rt.grid, probe.cell(), dir) else { continue };
        // Feet come down on the landing cell's middle line along the hop, their own line across.
        let centre = Vec2::centre(lx, ly);
        let to = if dir.0 == 0 { Vec2 { x: u.pos.x, y: centre.y } } else { Vec2 { x: centre.x, y: u.pos.y } };
        let n = i32::from(LEDGE_HOP_TICKS);
        let step = |d: i32| if d == 0 { 0 } else { d.signum() * (d.abs() + n - 1) / n };
        let (sx, sy) = (step(to.x.0 - u.pos.x.0), step(to.y.0 - u.pos.y.0));
        let end = Vec2 { x: Fx(u.pos.x.0 + sx * n), y: Fx(u.pos.y.0 + sy * n) };
        if end.cell() != (lx, ly) || crate::units::box_blocked(&rt.grid, end.x, end.y) || !may_hop(rt.zone, u, def, end)
        {
            continue;
        }
        u.feel.knock = Some(Knock { dx: sx, dy: sy, left: LEDGE_HOP_TICKS | LEDGE_HOP_BIT });
        return true;
    }
    false
}

/// One tick of a hop under way (`feel::step_knocks`): the body moves its share, over whatever is
/// under it, and the hop ends on its last tick.
pub fn step_hop(rt: &mut ZoneRuntime, u: &mut Unit, k: Knock) {
    u.pos = Vec2 { x: Fx(u.pos.x.0 + k.dx), y: Fx(u.pos.y.0 + k.dy) };
    rt.moved(u);
    let left = (k.left & !LEDGE_HOP_BIT) - 1;
    u.feel.knock = (left > 0).then_some(Knock { left: left | LEDGE_HOP_BIT, ..k });
}

/// A hop cut short (it died mid-air): the body is put down where it was going.
pub fn finish_hop(rt: &mut ZoneRuntime, u: &mut Unit) {
    while let Some(k) = u.feel.knock.filter(|k| k.left & LEDGE_HOP_BIT != 0) {
        step_hop(rt, u, k);
    }
}

/// A ladder's pace: half.
#[inline]
pub fn ladder_pace(g: &ZoneGrid, p: Vec2, dx: Fx, dy: Fx) -> (Fx, Fx) {
    if on_ladder(g, p) { (Fx(dx.0 / 2), Fx(dy.0 / 2)) } else { (dx, dy) }
}

/// Whole cells between two points, the longer axis (what a way's length is weighed against).
pub fn cells_apart(a: Vec2, b: Vec2) -> i64 {
    let (ax, ay) = a.cell();
    let (bx, by) = b.cell();
    i64::from((ax - bx).abs().max((ay - by).abs()))
}

/// `terraces`: a hand-built ground with every case of R1's height (MAP.md §9, dev only, never a
/// zone of the game), 96 x 48 cells, for the tests and the tick bench:
///
/// - a plateau at level 2 over the whole north (rows 1 to 15), its south face rows 14 and 15
///   ([`Tile::Cliff`], level 2); level 1 below;
/// - in that face a ledge hopped south ([`LEDGE`], `LedgeS`), a stair ([`STAIR`]: row 14 at
///   level 2, row 15 at 1), a ladder ([`LADDER`], one wide) and a ramp of road ([`RAMP`], the
///   same step);
/// - a hedge at row 17 ([`HEDGE`]) in the field below;
/// - a rise ([`RISE`], level 2) in the field, rimmed with cliff, its south face two rows;
/// - walls round the edge, `start` at (50, 24) facing north.
pub mod terraces {
    use alloc::vec;

    use jane_core::blueprint::Mark;
    use jane_core::{Blueprint, Cell, Key, Plane, Rect, Tile, ZoneId};

    pub const W: u32 = 96;
    pub const H: u32 = 48;
    /// The plateau's face: rows 14 and 15.
    pub const FACE: Rect = Rect { x: 1, y: 14, w: 94, h: 2 };
    pub const LEDGE: Rect = Rect { x: 8, y: 14, w: 8, h: 2 };
    pub const STAIR: Rect = Rect { x: 80, y: 14, w: 4, h: 2 };
    pub const LADDER: Rect = Rect { x: 60, y: 14, w: 1, h: 2 };
    pub const RAMP: Rect = Rect { x: 88, y: 12, w: 6, h: 6 };
    pub const HEDGE: Rect = Rect { x: 22, y: 17, w: 8, h: 1 };
    pub const RISE: Rect = Rect { x: 30, y: 28, w: 10, h: 8 };

    /// The terraces in `zone`'s blueprint.
    pub fn blueprint(zone: ZoneId) -> Blueprint {
        let cat = jane_data::catalog();
        let mut bp = Blueprint::new(zone, W, H, Tile::Grass);
        let (w, h) = (W as i32, H as i32);
        for r in [Rect::new(0, 0, w, 1), Rect::new(0, h - 1, w, 1), Rect::new(0, 0, 1, h), Rect::new(w - 1, 0, 1, h)] {
            bp.tiles.fill_rect(r, Tile::Wall);
        }
        let mut lv = vec![1u8; (W * H) as usize];
        let mut level = |r: Rect, l: u8| {
            for (x, y) in r.cells() {
                lv[(y * w + x) as usize] = l;
            }
        };
        level(Rect::new(0, 0, w, FACE.bottom()), 2);
        bp.tiles.fill_rect(FACE, Tile::Cliff);
        bp.tiles.fill_rect(LEDGE, Tile::LedgeS);
        bp.tiles.fill_rect(STAIR, Tile::Stair);
        bp.tiles.fill_rect(LADDER, Tile::Ladder);
        bp.tiles.fill_rect(RAMP, Tile::Road);
        // A join's cell is at the level it leads to from its half of the run.
        level(Rect::new(STAIR.x, STAIR.y + 1, STAIR.w, 1), 1);
        level(Rect::new(LADDER.x, LADDER.y + 1, 1, 1), 1);
        level(Rect::new(RAMP.x, FACE.y + 1, RAMP.w, 1), 1);
        bp.tiles.fill_rect(HEDGE, Tile::Hedge);
        level(RISE, 2);
        bp.tiles.fill_rect(Rect::new(RISE.x, RISE.y, RISE.w, 1), Tile::Cliff);
        bp.tiles.fill_rect(Rect::new(RISE.x, RISE.y, 1, RISE.h), Tile::Cliff);
        bp.tiles.fill_rect(Rect::new(RISE.right() - 1, RISE.y, 1, RISE.h), Tile::Cliff);
        bp.tiles.fill_rect(Rect::new(RISE.x, RISE.bottom() - 2, RISE.w, 2), Tile::Cliff);
        bp.level = Some(Plane::pack(W, H, &lv));
        bp.marks.insert(
            Key::Name(cat.story.start.mark),
            Mark { cell: Cell::new(50, 24), facing: Some(jane_core::action::Facing::North) },
        );
        bp
    }
}
