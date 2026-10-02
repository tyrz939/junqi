//! Click to move (PLAY-PLAN §2.1), beside WASD, never instead of it.
//!
//! A right-click sends one [`Goto`]; every peer plans the same path with the sim's one A*
//! (`path.rs`), capped at [`WALK_PATH_CELLS`] and re-planned at its end or toward a goal that
//! moves. Four rules:
//!
//! - **It never cheats the fog.** It routes only through cells whose fog block she has seen; into
//!   unseen ground it walks to the nearest seen cell, then straight, and stops at the first
//!   thing in the way ([`WALK_STUCK`] ticks without moving).
//! - **It never pushes a puzzle prop.** It routes round everything solid (a pushable is solid),
//!   and walking never pushes: only a held USE does (`interact::hold_use`).
//! - **It prefers made ways and light**: a cell off a road, cobbles, a grown path or a floor
//!   costs [`WALK_OFF_ROAD`] tenths more, a dark one [`WALK_DARK`] more.
//! - **Anything else she does ends it**: a held move, a hit (her health lower than when it last
//!   looked), Esc, a door. At its end she does what the click asked: swing, use, cast.

use jane_core::num::{dist_sq, isqrt};
use jane_core::search::PathEnd;
use jane_core::tile::{F_INDOOR, Tile};
use jane_core::{CellIx, Fx, SpellId, Tick, Vec2};

use crate::combat::{book_of, distance, is_enemy, metres_between};
use crate::ctx::Ctx;
use crate::fog::fog_seen;
use crate::ids::Seat;
use crate::input::{Goto, InputFrame, TargetRef};
use crate::interact::{FocusRef, prop_distance_sq};
use crate::light::prop_centre;
use crate::los::line_of_sight;
use crate::path::{PATH_WINDOW, PathAsk, REPATH_TICKS, cost_of_cells};
use crate::state::{ClickWalk, WalkThen};
use crate::tuning::{
    TALK_REACH_FX, USE_REACH_FX, WALK_ARRIVED_FX, WALK_DARK, WALK_GIVE_UP, WALK_OFF_ROAD, WALK_PATH_CELLS,
    WALK_REACH_SLACK_FX, WALK_STUCK,
};
use crate::units::{def_of, face_vector, move_unit};

/// A made way: what a click-walk prefers.
fn made_way(t: Tile) -> bool {
    matches!(t, Tile::Road | Tile::Cobble | Tile::GrownPath) || t.flags() & F_INDOOR != 0
}

/// Start (or restart) a walk for the seat toward `to`, then `then`.
pub fn start(cx: &mut Ctx<'_>, seat: Seat, to: Vec2, then: WalkThen) {
    let now = cx.world.tick;
    let Some(hp) = cx.world.player(seat).and_then(|p| cx.zone.unit(p.unit)).map(|u| u.hp) else { return };
    cx.world.players[seat.index()].fight.walk = Some(Box::new(ClickWalk {
        to,
        then,
        cells: Vec::new(),
        at: 0,
        repath_at: now,
        until: now.after(WALK_GIVE_UP),
        health: hp,
        stuck: 0,
    }));
}

/// `Command::Goto`: a right-click on ground, a unit or a prop.
pub fn goto(cx: &mut Ctx<'_>, seat: Seat, g: Goto) {
    let Some(p) = cx.world.player(seat) else { return };
    if p.dialogue.is_some() {
        return;
    }
    let body = p.unit;
    let Some(u) = cx.zone.unit(body).filter(|u| u.alive) else { return };
    match g {
        Goto::Ground(at) => start(cx, seat, at, WalkThen::Stop),
        Goto::Unit(id) => {
            let Some(o) = cx.zone.unit(id).filter(|o| o.alive && !o.hidden && o.id != body) else { return };
            let opos = o.pos;
            if crate::target::hostile(cx.zone, u, TargetRef::Unit(id)) {
                // A caster already in range of it only targets it (the client did).
                if caster_in_range(cx, body, id) {
                    cx.world.players[seat.index()].fight.walk = None;
                    return;
                }
                if crate::cast::swing_reach(u).is_some_and(|r| metres_between(u, o) <= r) {
                    let f = &mut cx.world.players[seat.index()].fight;
                    f.walk = None;
                    f.auto = Some(id);
                    return;
                }
                start(cx, seat, opos, WalkThen::Attack(id));
            } else if is_enemy(u.faction, o.faction) {
                // Someone she could fight but does not (a person the party does not fight).
                start(cx, seat, opos, WalkThen::Stop);
            } else {
                start(cx, seat, opos, WalkThen::Use(TargetRef::Unit(id)));
            }
        }
        Goto::Prop(id) => {
            let Some(at) = crate::target::pos_of(cx.zone, TargetRef::Prop(id)) else { return };
            start(cx, seat, at, WalkThen::Use(TargetRef::Prop(id)));
        }
    }
}

/// Does she know a bolt that would reach this foe from where she stands, in sight?
fn caster_in_range(cx: &Ctx<'_>, body: crate::ids::UnitId, foe: crate::ids::UnitId) -> bool {
    let (Some(u), Some(o)) = (cx.zone.unit(body), cx.zone.unit(foe)) else { return false };
    let reach = metres_between(u, o);
    let bolt = |s: SpellId| {
        let d = cx.cat.combat.spell(s);
        crate::assist::assists(d) && d.kind == jane_data::SpellKind::Bolt && reach <= i64::from(d.range.0)
    };
    (cx.world.growth.spells.iter().copied().any(bolt) || book_of(u).iter().copied().any(bolt))
        && line_of_sight(&cx.rt.grid, u.pos, o.pos)
}

/// Is the walk done: is she where the click asked?
fn arrived(cx: &Ctx<'_>, seat: Seat, w: &ClickWalk) -> bool {
    let Some(u) = cx.world.player(seat).and_then(|p| cx.zone.unit(p.unit)) else { return true };
    match w.then {
        WalkThen::Stop => dist_sq(u.pos, w.to) <= WALK_ARRIVED_FX * WALK_ARRIVED_FX,
        WalkThen::Attack(id) => cx.zone.unit(id).is_some_and(|o| {
            crate::cast::swing_reach(u).is_some_and(|r| metres_between(u, o) <= (r - WALK_REACH_SLACK_FX).max(0))
        }),
        WalkThen::Use(TargetRef::Unit(id)) => {
            cx.zone.unit(id).is_some_and(|o| distance(u.pos, o.pos) <= i64::from(TALK_REACH_FX) - WALK_REACH_SLACK_FX)
        }
        WalkThen::Use(TargetRef::Prop(id)) => cx.zone.prop_ix(id).is_some_and(|ix| {
            let p = &cx.zone.props[ix as usize];
            let r = i64::from(USE_REACH_FX) - WALK_REACH_SLACK_FX;
            prop_distance_sq(cx.cat.story.prop(p.def), p, u.pos) <= r * r
        }),
        WalkThen::Cast { spell, .. } => {
            let t = cx.world.players[seat.index()].fight.target;
            t.is_some_and(|t| crate::cast::reach_check(cx, u, cx.cat.combat.spell(spell), t).is_ok())
        }
    }
}

/// Where the walk's goal is now (a unit moves); `None`: it is gone, and so is the walk.
fn goal_now(cx: &Ctx<'_>, seat: Seat, w: &ClickWalk) -> Option<Vec2> {
    match w.then {
        WalkThen::Stop => Some(w.to),
        WalkThen::Attack(id) | WalkThen::Use(TargetRef::Unit(id)) => {
            cx.zone.unit(id).filter(|o| o.alive && !o.hidden).map(|o| o.pos)
        }
        WalkThen::Use(TargetRef::Prop(id)) => cx
            .zone
            .prop_ix(id)
            .map(|ix| prop_centre(cx.cat.story.prop(cx.zone.props[ix as usize].def), &cx.zone.props[ix as usize])),
        WalkThen::Cast { .. } => {
            cx.world.players[seat.index()].fight.target.and_then(|t| crate::target::pos_of(cx.zone, t))
        }
    }
}

/// Step 6, for a seat on a click-walk with no move held: one tick of walking at `speed`, or its
/// end. The caller has checked that the walk is there.
pub fn step(cx: &mut Ctx<'_>, seat: Seat, frame: InputFrame, speed: Fx) {
    let now = cx.world.tick;
    let Some(mut w) = cx.world.players[seat.index()].fight.walk.take() else { return };
    let body = cx.world.players[seat.index()].unit;
    let Some(ix) = cx.zone.unit_ix(body) else { return };
    let hp = cx.zone.units[ix].hp;
    if now > w.until || hp < w.health {
        return;
    }
    w.health = hp;
    let Some(goal) = goal_now(cx, seat, &w) else { return };
    let moved_goal = goal.cell() != w.to.cell();
    w.to = goal;
    if arrived(cx, seat, &w) {
        finish(cx, seat, w.then, frame);
        return;
    }
    if (now >= w.repath_at || (moved_goal && usize::from(w.at) >= w.cells.len())) && cx.rt.take_path_search() {
        plan(cx, ix, &mut w);
    }
    let from = cx.zone.units[ix].pos;
    follow(cx, ix, &mut w, speed);
    if cx.zone.units[ix].pos == from {
        w.stuck = w.stuck.saturating_add(1);
        if w.stuck >= WALK_STUCK {
            return;
        }
    } else {
        w.stuck = 0;
    }
    cx.world.players[seat.index()].fight.walk = Some(w);
}

/// What the click asked, now that she is there.
fn finish(cx: &mut Ctx<'_>, seat: Seat, then: WalkThen, frame: InputFrame) {
    let body = cx.world.players[seat.index()].unit;
    match then {
        WalkThen::Stop => {}
        WalkThen::Attack(id) => cx.world.players[seat.index()].fight.auto = Some(id),
        WalkThen::Use(t) => {
            let before = cx.actor.replace(seat);
            let r = match t {
                TargetRef::Unit(u) => FocusRef::Unit(u),
                TargetRef::Prop(p) => FocusRef::Prop(p),
            };
            crate::interact::use_ref(cx, seat, body, r);
            cx.actor = before;
        }
        WalkThen::Cast { spell, on } => crate::cast::begin(cx, seat, spell, on, frame, false),
    }
}

/// Plan the walk from her cell toward its goal: seen cells only, made ways and light preferred,
/// capped; a goal in something solid is walked to the nearest free cell beside it.
fn plan(cx: &mut Ctx<'_>, ix: usize, w: &mut ClickWalk) {
    let now = cx.world.tick;
    let clock = cx.world.clock;
    let grid = &cx.rt.grid;
    let start = cx.zone.units[ix].pos.cell();
    let (mut gx, mut gy) = w.to.cell();
    gx = gx.clamp(0, grid.w() as i32 - 1);
    gy = gy.clamp(0, grid.h() as i32 - 1);
    if grid.solid(gx, gy) {
        if let Some(c) = grid.nearest_free(gx, gy, 3, Some(start)) {
            (gx, gy) = c;
        }
    }
    let half = (PATH_WINDOW >> 1) as i32;
    let s = &mut *cx.scratch;
    s.lights.gather(cx.zone, cx.rt, clock, (start.0 - half, start.1 - half), (start.0 + half, start.1 + half), false);
    let (fog, geom) = (&cx.zone.fog, cx.rt.fog);
    let lights = &s.lights;
    let ask = PathAsk::new(start, (gx, gy), cost_of_cells(WALK_PATH_CELLS));
    let found = s.path.find_weighted(grid, ask, true, |x, y| {
        if (x, y) != start && !fog_seen(fog, geom, x / geom.cells as i32, y / geom.cells as i32) {
            return None;
        }
        let road = if made_way(grid.tile_at(x, y)) { 0 } else { WALK_OFF_ROAD };
        let dark = if lights.is_empty() || lights.cell_lit(x, y) { 0 } else { WALK_DARK };
        Some(road + dark)
    });
    let width = grid.w();
    w.cells.clear();
    if matches!(found, Some(PathEnd::Found | PathEnd::Partial)) {
        w.cells.extend(s.path.out.iter().map(|&(x, y)| CellIx(y as u32 * width + x as u32)));
    }
    w.at = 0;
    w.repath_at = now.after(Tick(REPATH_TICKS));
}

/// Walk `speed` along the planned cells; past their end (unseen ground, or the goal's own cell),
/// straight at the goal.
fn follow(cx: &mut Ctx<'_>, ix: usize, w: &mut ClickWalk, speed: Fx) {
    let width = cx.rt.grid.w();
    let mut budget = i64::from(speed.0);
    while budget > 0 {
        let to = match w.cells.get(usize::from(w.at)) {
            Some(&c) => Vec2::centre((c.0 % width) as i32, (c.0 / width) as i32),
            None => w.to,
        };
        let u = &mut cx.zone.units[ix];
        let d = i64::from(isqrt(dist_sq(u.pos, to) as u64));
        if d == 0 {
            if usize::from(w.at) < w.cells.len() {
                w.at += 1;
                continue;
            }
            return;
        }
        let step = d.min(budget);
        let dx = i64::from(to.x.0 - u.pos.x.0) * step / d;
        let dy = i64::from(to.y.0 - u.pos.y.0) * step / d;
        face_vector(u, dx, dy);
        let before = u.pos;
        move_unit(cx.rt, u, Fx(dx as i32), Fx(dy as i32));
        budget -= step;
        if step == d && usize::from(w.at) < w.cells.len() {
            w.at += 1;
        }
        if cx.zone.units[ix].pos == before {
            return;
        }
    }
}

/// A seat's walking speed this tick: her walk (or run, sprinting), her statuses, halved while a
/// cast builds.
pub fn speed_of(u: &crate::state::Unit, now: Tick, sprinting: bool, casting: bool, god: bool) -> Fx {
    let def = def_of(u);
    let mut speed = if sprinting { def.run.0 } else { def.walk.0 };
    speed = speed * crate::status::speed_factor(u, now) / 1000;
    if casting {
        speed /= 2;
    }
    if god {
        speed *= 2;
    }
    Fx(speed)
}
