//! Step 3, presence (ARCHITECTURE.md §4.6.a; `sim.ts stepDayOnly`). Every [`PRESENCE_EVERY`]
//! ticks, and on arrival in a zone, each unit with somewhere to be by the hour is put there.
//!
//! **The slot.** A row's `schedule` names a [`ScheduleSlot`] per span of hours. `day_only` and
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

use jane_core::Vec2;
use jane_data::{Controller, ScheduleSlot, UnitDef};

use crate::ctx::Ctx;
use crate::ids::UnitId;
use crate::state::GameState;
use crate::tuning::{ORDER_ARRIVED_FX, PRESENCE_EVERY, PRESENCE_NUDGE_RADIUS, WATCH_X_FX, WATCH_Y_FX};
use crate::units::def_of;

/// The slot a row puts a unit in now, or `None` when it has no schedule (always present) or its
/// schedule does not say (it stays as it is).
pub fn slot_of(def: &UnitDef, world: &GameState) -> Option<ScheduleSlot> {
    let hour = world.hour() as u8;
    if !def.schedule.is_empty() {
        return def.schedule.iter().find(|r| in_hours(hour, r.hour_from, r.hour_to)).map(|r| r.slot);
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
        let Some(slot) = slot_of(def_of(u), cx.world) else { continue };
        if u.controller == Controller::Player || cx.party.seat_of(u.id).is_some() {
            continue;
        }
        put(cx, i, slot, arriving);
    }
}

/// Put the zone's `i`th unit where `slot` says, minding the watcher box (see the module doc).
pub fn put(cx: &mut Ctx<'_>, i: usize, slot: ScheduleSlot, arriving: bool) {
    let u = &cx.zone.units[i];
    let (id, pos, hidden) = (u.id, u.pos, u.hidden);
    let seen = |cx: &Ctx<'_>, at: Vec2| !arriving && watched(cx, at);
    match slot {
        ScheduleSlot::Absent | ScheduleSlot::Inside(_) => {
            if !hidden && !seen(cx, pos) {
                cx.rt.leave(id);
                cx.zone.units[i].hidden = true;
            }
        }
        ScheduleSlot::Patrol => {
            if hidden && !seen(cx, pos) {
                show(cx, i, pos);
            }
        }
        ScheduleSlot::Mark(n) => {
            let Some(m) = cx.rt.mark(crate::sym::of_name(n)) else { return };
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
/// where it lies or of the home it stands up at: it is put back on `sleeping_due` a presence beat
/// later, and tries again then (the rule of this module, for every respawn).
pub fn stands_up_unseen(cx: &mut Ctx<'_>, unit: UnitId) -> bool {
    let Some(u) = cx.zone.unit(unit) else { return true };
    if !watched(cx, u.pos) && !watched(cx, u.home) {
        return true;
    }
    let at = cx.world.tick.after(jane_core::Tick(PRESENCE_EVERY));
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
        u.home = at;
        crate::ai::clear_path(u);
    }
    u.hidden = false;
    cx.rt.enter(&cx.zone.units[i]);
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
