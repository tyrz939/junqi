//! Step 3, presence (ARCHITECTURE.md §4.6.a; `sim.ts stepDayOnly`). Every [`PRESENCE_EVERY`]
//! ticks, and on arrival in a zone, each unit with somewhere to be by the hour is put there.
//!
//! **The slot.** A row's `schedule` names a [`ScheduleSlot`] per span of hours. A row may carry
//! a `when` (while a quest is in the log, or once it is handed in): those are looked at first,
//! and the plain rows cover the rest of the day. A person's hours move a few minutes from day to
//! day ([`own_hour`], the row's `vary`), never across a bell. `day_only` and
//! `night_only` are two-slot schedules, not special cases: `day_only` is `Patrol` from 06:00 to
//! 21:00 and `Absent` the rest (only once its `day_only_after` quest is done, if it names one:
//! the dog keeps Julie's hours only after the key is given), `night_only` the other way round.
//! A row with neither is always present and presence never touches it.
//!
//! | Slot | |
//! | --- | --- |
//! | `Absent`, `Inside(prop)` | hidden (out of occupancy and the unit blocks, skipped by every controller) |
//! | `Patrol` | shown where it is; its controller does the rest |
//! | `Mark(name)` | shown, and at the mark |
//!
//! **Rule: nothing teleports inside the watcher box.** Nothing vanishes, appears or jumps while
//! a seat in the zone stands within 120 x 80 px of it (of where it is, and for a jump, of where
//! it lands). Nothing stands up in view either: every respawn asks [`stands_up_unseen`] first. A unit going to its mark while watched is given an order and walks there; one
//! nobody sees is moved and re-stamped in place. Something hidden or shown while watched simply
//! waits for the next look. A unit shown where something solid now stands comes back on the
//! nearest free cell.
//!
//! Seats' bodies are never scheduled, and a snake (a mover of its own) is shown and hidden but
//! never sent.

use alloc::boxed::Box;

use jane_core::{Vec2, ZoneId};
use jane_data::{Controller, ScheduleRow, ScheduleSlot, ScheduleWhen, UnitDef};
use jane_world::steps::Step;

use crate::ctx::Ctx;
use crate::ids::UnitId;
use crate::state::{GameState, Unit};
use crate::tuning::{
    NIGHT_END_HOUR, NIGHT_START_HOUR, ORDER_ARRIVED_FX, PRESENCE_EVERY, PRESENCE_NUDGE_RADIUS, TICKS_PER_DAY,
    TICKS_PER_HOUR, WATCH_X_FX, WATCH_Y_FX,
};
use crate::units::def_of;

/// The slot a row puts a unit in now, at the hours as written, or `None` when it has no schedule
/// (always present) or its schedule does not say (it stays as it is). A person's own hours for
/// the day are [`slot_of_unit`]'s.
pub fn slot_of(def: &UnitDef, world: &GameState) -> Option<ScheduleSlot> {
    slot_at(def, world, world.hour() as u8)
}

/// The slot this unit is in now: its row's, at its own hour of the day ([`own_hour`]).
pub fn slot_of_unit(u: &Unit, world: &GameState) -> Option<ScheduleSlot> {
    let def = def_of(u);
    slot_at(def, world, own_hour(def, world))
}

/// The schedule row that holds at `hour`: the first override (`while`, `after`) whose hours and
/// whose quest hold, else the plain row for the hour (the compiler puts the overrides first).
pub fn row_at<'a>(def: &'a UnitDef, world: &GameState, hour: u8) -> Option<&'a ScheduleRow> {
    def.schedule.iter().find(|r| in_hours(hour, r.hour_from, r.hour_to) && when_holds(r.when, world))
}

/// Does a row's `when` hold in this world?
pub fn when_holds(when: Option<ScheduleWhen>, world: &GameState) -> bool {
    match when {
        None => true,
        Some(ScheduleWhen::While(q)) => world.quests.active.iter().any(|p| p.quest == q),
        Some(ScheduleWhen::After(q)) => world.quests.done.contains(&q),
        Some(ScheduleWhen::Flag(n)) => {
            world.flags.get(&crate::state::FlagKey::Named(crate::sym::of_name(n))).is_some_and(|&v| v != 0)
        }
    }
}

/// Clock ticks in a minute.
const TICKS_PER_MINUTE: u32 = TICKS_PER_HOUR / 60;

/// How many minutes a person's hours are moved today, `-vary..=vary` in fives: one draw from the
/// person's own dice for the day (`Step::SimHours`, keyed by the row's `vary_key`: its id, or the
/// id of the one it keeps hours with), so every seat and every load of the same save agrees, and
/// nothing is kept for it.
pub fn hours_offset(seed: u32, vary_key: u32, day: u32, vary: u8) -> i32 {
    let fives = u32::from(vary / 5);
    let mut dice = jane_world::steps::dice(seed, ZoneId::County, Step::SimHours, 0, vary_key as i32, day as i32);
    dice.below(fives * 2 + 1) as i32 * 5 - i32::from(vary)
}

/// The hour of the day by this person's clock today: the world's, moved by [`hours_offset`] when
/// the row varies. Never across a bell: from nine to six everyone keeps the world's hour, and a
/// moved hour that would fall in the night is the day's last or first.
pub fn own_hour(def: &UnitDef, world: &GameState) -> u8 {
    let hour = world.hour() as u8;
    if def.vary == 0 || world.is_night() {
        return hour;
    }
    let off = hours_offset(world.seed, def.vary_key, world.day, def.vary);
    let back = off.unsigned_abs() * TICKS_PER_MINUTE;
    let clock = if off >= 0 {
        (world.clock + TICKS_PER_DAY - back) % TICKS_PER_DAY
    } else {
        (world.clock + back) % TICKS_PER_DAY
    };
    ((clock / TICKS_PER_HOUR) as u8).clamp(NIGHT_END_HOUR as u8, NIGHT_START_HOUR as u8 - 1)
}

/// The slot at `hour`: the schedule's, else the `day_only` / `night_only` shorthand (by the
/// world's own night, which no person's hours move).
fn slot_at(def: &UnitDef, world: &GameState, hour: u8) -> Option<ScheduleSlot> {
    if !def.schedule.is_empty() {
        return row_at(def, world, hour).map(|r| r.slot);
    }
    let night = world.is_night();
    if def.day_only {
        let keeps_hours = def.day_only_after.is_none_or(|q| world.quests.done.contains(&q));
        return Some(if night && keeps_hours { ScheduleSlot::Absent } else { ScheduleSlot::Patrol });
    }
    if def.night_only {
        return Some(if night { ScheduleSlot::Patrol } else { ScheduleSlot::Absent });
    }
    None
}

/// `from` up to `to`, wrapping midnight; `from == to` is the whole day.
pub const fn in_hours(hour: u8, from: u8, to: u8) -> bool {
    if from < to {
        hour >= from && hour < to
    } else if from > to {
        hour >= from || hour < to
    } else {
        true
    }
}

/// Step 3 on its beat.
pub fn step_presence(cx: &mut Ctx<'_>) {
    if cx.world.tick.0 % PRESENCE_EVERY == 0 {
        presence(cx, false);
    }
}

/// Put every scheduled unit of the zone where its slot says. `arriving`: a seat has just come
/// in (travel, step 14): nobody was watching, so whatever changed while the zone was empty has
/// already happened.
pub fn presence(cx: &mut Ctx<'_>, arriving: bool) {
    for i in 0..cx.zone.units.len() {
        let u = &cx.zone.units[i];
        let Some(slot) = slot_of_unit(u, cx.world) else { continue };
        if u.controller == Controller::Player || cx.party.seat_of(u.id).is_some() {
            continue;
        }
        put(cx, i, slot, arriving);
    }
}

/// Put the zone's `i`th unit where `slot` says, minding the watcher box (see the module doc).
pub fn put(cx: &mut Ctx<'_>, i: usize, slot: ScheduleSlot, arriving: bool) {
    let u = &cx.zone.units[i];
    let (pos, hidden) = (u.pos, u.hidden);
    let seen = |cx: &Ctx<'_>, at: Vec2| !arriving && watched(cx, at);
    match slot {
        ScheduleSlot::Absent | ScheduleSlot::Inside(_) => {
            if hidden {
                return;
            }
            if !seen(cx, pos) {
                go_in(cx, i);
                return;
            }
            // After the bell nobody is out (WORLD.md §2.2), watched or not: a person she is
            // looking at walks to her own door (one with none, to where she lives) and goes in
            // there in sight, which is going in, not vanishing. By day the step waits for her
            // to look away, as before.
            if !cx.world.is_night() {
                return;
            }
            let home = cx.zone.units[i].home;
            let to = match slot {
                ScheduleSlot::Inside(n) => door_step(cx, n).unwrap_or(home),
                _ => home,
            };
            if jane_core::num::dist_sq(pos, to) <= i64::from(DOOR_REACHED_FX).pow(2) {
                go_in(cx, i);
            } else if cx.zone.units[i].order.is_none() {
                walk_to(cx, i, to);
            }
        }
        ScheduleSlot::Patrol => {
            if hidden && !seen(cx, pos) {
                show(cx, i, pos);
            }
        }
        ScheduleSlot::Mark(n) => {
            // A mark this zone does not have (a row played on a test field): shown where it is.
            let Some(m) = cx.rt.mark(crate::sym::of_name(n)) else {
                if hidden && !seen(cx, pos) {
                    show(cx, i, pos);
                }
                return;
            };
            let to = Vec2::centre(i32::from(m.cell.x), i32::from(m.cell.y));
            let there = jane_core::num::dist_sq(pos, to) <= i64::from(ORDER_ARRIVED_FX).pow(2);
            let (seen_here, seen_there) = (seen(cx, pos), seen(cx, to));
            if hidden {
                if !seen_here && !seen_there {
                    show(cx, i, to);
                }
            } else if !there && cx.zone.units[i].order.is_none() {
                if seen_here || seen_there {
                    walk_to(cx, i, to);
                } else {
                    jump(cx, i, to);
                }
            }
        }
    }
}

/// May a corpse due stand up now, unseen? Not while a seat stands within the watcher box of
/// where it lies or of the home it stands up at: it is put back on `sleeping_due` and tries
/// again a presence beat later, or, one of a patch's, at the next ten-minute mark, when the
/// ecology looks (the rule of this module, for every respawn).
pub fn stands_up_unseen(cx: &mut Ctx<'_>, unit: UnitId) -> bool {
    let Some(u) = cx.zone.unit(unit) else { return true };
    if !watched(cx, u.pos) && !watched(cx, u.home) {
        return true;
    }
    let beat = cx.world.tick.after(jane_core::Tick(PRESENCE_EVERY));
    let at = crate::living::patch_mark(cx, unit).unwrap_or(beat);
    let ix = cx.zone.sleeping_due.partition_point(|&e| e < (at, unit));
    cx.zone.sleeping_due.insert(ix, (at, unit));
    false
}

/// Is a seat in this zone standing within the watcher box of `at`?
pub fn watched(cx: &Ctx<'_>, at: Vec2) -> bool {
    cx.party.bodies.iter().flatten().any(|&(z, b, _)| {
        z == cx.zone.id
            && cx
                .zone
                .unit(b)
                .is_some_and(|b| (b.pos.x.0 - at.x.0).abs() < WATCH_X_FX && (b.pos.y.0 - at.y.0).abs() < WATCH_Y_FX)
    })
}

/// The nearest free cell to `at` within the nudge radius, else `at`.
fn free_near(cx: &Ctx<'_>, at: Vec2) -> Vec2 {
    let (x, y) = at.cell();
    cx.rt.grid.nearest_free(x, y, PRESENCE_NUDGE_RADIUS, None).map_or(at, |(fx, fy)| Vec2::centre(fx, fy))
}

/// Shown again at `at` (its own place, or its mark), beside it if something solid stands there.
fn show(cx: &mut Ctx<'_>, i: usize, at: Vec2) {
    let (x, y) = at.cell();
    let at = if cx.rt.grid.solid(x, y) { free_near(cx, at) } else { at };
    let u = &mut cx.zone.units[i];
    if u.pos != at {
        u.pos = at;
        u.on_span = None;
        u.home = at;
        crate::ai::clear_path(u);
    }
    u.hidden = false;
    cx.rt.enter(&cx.zone.units[i]);
}

/// How near her door's step a person is when she goes in at it in sight: two cells, a little
/// past an order's arrival, so one stopped by the step's own lamp post still goes in.
const DOOR_REACHED_FX: i32 = jane_core::num::CELL_FX * 2;

/// Hidden: out of occupancy and the unit blocks, and any walk it was on forgotten.
fn go_in(cx: &mut Ctx<'_>, i: usize) {
    let u = &mut cx.zone.units[i];
    u.hidden = true;
    u.order = None;
    let id = u.id;
    cx.rt.leave(id);
}

/// The middle of the cell just below a door's footprint: where a person stands to go in at it.
fn door_step(cx: &Ctx<'_>, door: jane_core::NameId) -> Option<Vec2> {
    let &ix = cx.rt.names.get(&crate::sym::of_name(door))?;
    let p = cx.zone.props.get(ix as usize)?;
    let def = cx.cat.story.prop(p.def);
    let x = i32::from(p.cell.x) + i32::from(def.w) / 2;
    let y = i32::from(p.cell.y) + i32::from(def.h);
    Some(Vec2::centre(x, y))
}

/// Moved to its mark while nobody sees either end, and re-stamped there.
fn jump(cx: &mut Ctx<'_>, i: usize, to: Vec2) {
    let at = free_near(cx, to);
    let u = &mut cx.zone.units[i];
    u.home = at;
    crate::units::place_unit(cx.rt, u, at);
}

/// Walked to its mark by an order nobody sent, which it minds before anything else.
fn walk_to(cx: &mut Ctx<'_>, i: usize, to: Vec2) {
    let now = cx.world.tick;
    let u = &mut cx.zone.units[i];
    if u.controller == Controller::Snake {
        return;
    }
    let def = def_of(u);
    let speed = jane_core::Fx(def.run.0.max(def.walk.0));
    if speed.0 <= 0 {
        return;
    }
    let until = now.after(crate::npc::order_budget(u.pos, to, speed));
    u.order = Some(Box::new(crate::state::Order { to, until, then: None, seat: None }));
    u.target = None;
    u.combat = crate::state::CombatState::Idle;
    crate::ai::clear_path(u);
    if !u.awake && !cx.ops.wake.contains(&u.id) {
        cx.ops.wake.push(u.id);
    }
}
