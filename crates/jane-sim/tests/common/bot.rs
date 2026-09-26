//! A headless player for the rule tests (`jane/test/bot.ts`). It only does what a person can:
//! hold a direction, hold USE, press USE, choose a line. It finds its way with the sim's own
//! path finder over the live grid, so "the bot got there" also proves the map is walkable.

use jane_core::angle::iatan2;
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Angle, Fx, Sym, Vec2, ZoneId};
use jane_sim::ids::PropId;
use jane_sim::path::{PathAsk, PathScratch, cost_of_cells};
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput, UnitId};

pub fn sym(sim: &Sim, name: &str) -> Sym {
    sim.state().syms.find(name).unwrap_or_else(|| panic!("no name {name:?}"))
}

pub fn zone_of(sim: &Sim) -> ZoneId {
    sim.state().players[0].zone
}

pub fn me(sim: &Sim) -> &jane_sim::Unit {
    sim.view(Seat(0)).expect("seat 0").body()
}

pub fn step(sim: &mut Sim, frame: InputFrame) {
    sim.step(&StepInput::solo(frame));
}

pub fn idle(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&StepInput::IDLE);
    }
}

/// One command from seat `seat`, in a step of its own.
pub fn cmd_as(sim: &mut Sim, seat: u8, c: Command) {
    let cmds = [StampedCommand { seat: Some(Seat(seat)), seq: 0, cmd: c }];
    sim.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
}

pub fn cmd(sim: &mut Sim, c: Command) {
    cmd_as(sim, 0, c);
}

/// A prop of her zone by key, as it stands now.
pub fn prop(sim: &Sim, key: &str) -> jane_sim::Prop {
    let s = sym(sim, key);
    let z = sim.state().zone(zone_of(sim)).expect("her zone");
    z.props.iter().find(|p| p.key == s).unwrap_or_else(|| panic!("no prop {key}")).clone()
}

pub fn prop_id(sim: &Sim, key: &str) -> PropId {
    prop(sim, key).id
}

/// A unit of her zone by key.
pub fn unit(sim: &Sim, key: &str) -> jane_sim::Unit {
    let s = sym(sim, key);
    let z = sim.state().zone(zone_of(sim)).expect("her zone");
    z.units.iter().find(|u| u.key == Some(s)).unwrap_or_else(|| panic!("no unit {key}")).clone()
}

pub fn unit_by_id(sim: &Sim, id: UnitId) -> jane_sim::Unit {
    sim.state().zone(zone_of(sim)).and_then(|z| z.unit(id)).expect("the unit").clone()
}

/// Stand seat `seat` on the middle of a cell, facing `f` (tests only: the runtimes are rebuilt).
pub fn place_seat(sim: &mut Sim, seat: u8, x: i32, y: i32, f: jane_core::action::Facing) {
    let (z, id) = {
        let p = &sim.state().players[usize::from(seat)];
        (p.zone, p.unit)
    };
    let u = sim.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)).expect("her body");
    u.pos = Vec2::centre(x, y);
    u.facing = f;
    sim.rebuild_runtimes();
}

pub fn place(sim: &mut Sim, x: i32, y: i32, f: jane_core::action::Facing) {
    place_seat(sim, 0, x, y, f);
}

/// Hold USE (and the stick along `dir`) for `n` steps.
pub fn hold(sim: &mut Sim, dir: Angle, n: u32) {
    for _ in 0..n {
        step(sim, InputFrame { use_held: true, ..InputFrame::walk(dir) });
    }
}

/// Every event since the last drain, kinds only.
pub fn events(sim: &mut Sim) -> Vec<jane_sim::Event> {
    sim.drain_events().to_vec()
}

fn d(a: Vec2, b: Vec2) -> i64 {
    i64::from(jane_core::num::isqrt(dist_sq(a, b) as u64))
}

/// Walk (sprinting when far) to within `near` of a point. False on a timeout, a dead end or a
/// conversation.
pub fn walk_to(sim: &mut Sim, to: Vec2, near: Fx) -> bool {
    let mut path = PathScratch::new();
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let mut at = 0;
    let mut replan = 0;
    for _ in 0..60 * 120 {
        let u = me(sim).clone();
        if !u.alive || sim.state().players[0].dialogue.is_some() {
            return false;
        }
        if d(u.pos, to) <= i64::from(near.0) {
            return true;
        }
        if at >= cells.len() || replan == 0 {
            let rt = sim.runtime(zone_of(sim)).expect("runtime");
            let (tx, ty) = to.cell();
            let own = u.pos.cell();
            let Some(goal) = rt.grid.nearest_free(tx, ty, 6, Some(own)) else { return false };
            let mut ask = PathAsk::new(own, goal, cost_of_cells(4000));
            ask.budget = 400_000;
            if path.find(&rt.grid, ask).is_none() {
                return false;
            }
            cells.clear();
            cells.extend_from_slice(&path.out);
            at = 0;
            replan = 90;
            if cells.is_empty() {
                return true;
            }
        }
        replan -= 1;
        let (cx, cy) = cells[at];
        let c = Vec2::centre(cx, cy);
        let dd = d(u.pos, c);
        if dd < 384 {
            at += 1;
            continue;
        }
        let dir = iatan2(c.y.0 - u.pos.y.0, c.x.0 - u.pos.x.0);
        step(sim, InputFrame { sprint: dd > i64::from(2 * CELL_FX), ..InputFrame::walk(dir) });
    }
    false
}

/// Nudge toward a point for a step so her facing turns that way (the dominant axis).
pub fn face(sim: &mut Sim, to: Vec2) {
    let u = me(sim).pos;
    let (dx, dy) = (to.x.0 - u.x.0, to.y.0 - u.y.0);
    let dir = if dx.abs() >= dy.abs() {
        if dx >= 0 { Angle::EAST } else { Angle::WEST }
    } else if dy >= 0 {
        Angle::SOUTH
    } else {
        Angle::NORTH
    };
    step(sim, InputFrame { mv_dir: dir, mv_mag: 25, ..InputFrame::IDLE });
}

fn prop_rect(p: &jane_sim::Prop) -> (i32, i32, i32, i32) {
    let def = jane_data::catalog().story.prop(p.def);
    (i32::from(p.cell.x), i32::from(p.cell.y), i32::from(def.w), i32::from(def.h))
}

pub fn prop_centre(p: &jane_sim::Prop) -> Vec2 {
    let (x, y, w, h) = prop_rect(p);
    Vec2::new(Fx(x * CELL_FX + w * CELL_FX / 2), Fx(y * CELL_FX + h * CELL_FX / 2))
}

/// Walk to a side of a prop and face it; each side in turn until one is reached.
pub fn walk_to_prop(sim: &mut Sim, key: &str) -> bool {
    let p = prop(sim, key);
    let c = prop_centre(&p);
    let (x, y, w, h) = prop_rect(&p);
    let px = |n: i32| n * 256;
    let sides = [
        Vec2::new(c.x, Fx((y + h) * CELL_FX + px(5))),
        Vec2::new(c.x, Fx(y * CELL_FX - px(5))),
        Vec2::new(Fx(x * CELL_FX - px(5)), c.y),
        Vec2::new(Fx((x + w) * CELL_FX + px(5)), c.y),
    ];
    for s in sides {
        let rt = sim.runtime(zone_of(sim)).expect("runtime");
        let (sx, sy) = s.cell();
        if rt.grid.solid(sx, sy) {
            continue;
        }
        if walk_to(sim, s, Fx::from_px(3)) {
            face(sim, c);
            return true;
        }
    }
    false
}

/// Walk up to a unit and face it.
pub fn walk_to_unit(sim: &mut Sim, key: &str) -> bool {
    let u = unit(sim, key);
    let ok = walk_to(sim, u.pos, Fx::from_px(14));
    face(sim, u.pos);
    ok
}

/// Melee a unit to death, chasing it (`bot.ts fight`): bar slot 0 is melee on a new game.
pub fn fight(sim: &mut Sim, key: &str) -> bool {
    for _ in 0..60 * 90 {
        let target = unit(sim, key);
        let u = me(sim).clone();
        if !u.alive {
            return false;
        }
        if !target.alive {
            return true;
        }
        let dist = d(u.pos, target.pos);
        let dir = iatan2(target.pos.y.0 - u.pos.y.0, target.pos.x.0 - u.pos.x.0);
        let frame = InputFrame {
            aim: Some(dir),
            mv_mag: if dist > 12 * 256 { 127 } else { 0 },
            mv_dir: dir,
            ..InputFrame::IDLE
        };
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = frame;
        let bar = [StampedCommand { seat: Some(Seat(0)), seq: 0, cmd: Command::Bar { slot: 0, on: None } }];
        let cmds: &[StampedCommand] = if dist < 18 * 256 { &bar } else { &[] };
        sim.step(&StepInput { frames, commands: cmds });
    }
    !unit(sim, key).alive
}

/// Click through a conversation, taking `choices` in order at each choice (then the first).
pub fn talk_through(sim: &mut Sim, choices: &[u8]) {
    let mut n = 0;
    for _ in 0..200 {
        let Some(d) = sim.view(Seat(0)).and_then(|v| v.dialogue()) else { return };
        if d.awaiting_choice {
            let option = choices.get(n).copied().unwrap_or(0);
            n += 1;
            cmd(sim, Command::Choose { option });
        } else {
            cmd(sim, Command::Advance);
        }
    }
    panic!("a conversation that does not end");
}

/// How many of an item seat 0 holds.
pub fn holds(sim: &Sim, item: &str) -> u32 {
    let id = jane_data::catalog().combat.item_id(item).unwrap_or_else(|| panic!("no item {item}"));
    jane_sim::bag::bag_count(&sim.state().players[0].bag[..], id)
}

pub fn quest_done(sim: &Sim, q: &str) -> bool {
    let id = jane_data::catalog().story.quest_id(q).unwrap_or_else(|| panic!("no quest {q}"));
    sim.state().quests.done.contains(&id)
}

pub fn quest_active(sim: &Sim, q: &str) -> bool {
    let id = jane_data::catalog().story.quest_id(q).unwrap_or_else(|| panic!("no quest {q}"));
    sim.state().quests.active.iter().any(|p| p.quest == id)
}
