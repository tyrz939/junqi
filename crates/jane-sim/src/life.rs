//! Regen, standing up again and waking (ARCHITECTURE.md §4.3; `sim.ts tickTimers, respawn,
//! revivePlayer`, `ai.ts regen`). Besides the flush, the only writers of hp: regen, a creature
//! standing up, a seat waking, and the console.
//!
//! **Regen is paid, not ticked.** `Unit.synced` is the tick regen is paid up to; [`pay_regen`]
//! pays everything since in one step. Step 5 pays every awake unit every tick; a sleeper is paid
//! when it wakes, and anything that reads or writes mp or hp pays first. Per tick a living unit
//! gains `spirit` milli-mp (the TS's `spirit / 1000` a step, exact), and an idle AI (its row's
//! `auto_regen` while idle, any AI while leashing, never under orders) gains `max / 300` hp and
//! mp, floored; the sums are the per-tick sums because every rate is fixed between writes and
//! clamping a sum of gains equals clamping each. Sleepers are idle by definition. The AI
//! controller does not regen: step 5 does it for everyone.

use jane_core::{Milli, Tick, Vec2};
use jane_data::Controller;

use crate::ctx::Ctx;
use crate::event::{EventKind, ToastKind};
use crate::flush::reset_phases;
use crate::ids::{Seat, UnitId};
use crate::state::{CombatState, FlagKey, TravelRequest, Unit};
use crate::tuning::{ENERGY_MAX, REGEN_DIVISOR, RESPAWN_RADIUS, REVIVE_RADIUS};
use crate::units::{def_of, max_hp, max_mp};

/// Idle regen applies (`ai.ts regen`, called while idle with the row's flag and while leashing
/// always).
fn idle_regen(u: &Unit) -> bool {
    u.controller == Controller::Ai
        && u.order.is_none()
        && match u.combat {
            CombatState::Leash => true,
            CombatState::Idle => def_of(u).auto_regen,
            CombatState::Combat => false,
        }
}

/// Pay a unit's regen up to `now` (see the module doc). A whole AI starts its fight from the top.
pub fn pay_regen(u: &mut Unit, now: Tick) {
    if now <= u.synced {
        return;
    }
    let n = i64::from(now.0 - u.synced.0);
    u.synced = now;
    if !u.alive {
        return;
    }
    let (hp_max, mp_max) = (i64::from(max_hp(u).0), i64::from(max_mp(u).0));
    let mut mp = i64::from(u.mp.0) + n * i64::from(u.spirit);
    if idle_regen(u) {
        let hp = i64::from(u.hp.0) + n * (hp_max / i64::from(REGEN_DIVISOR));
        u.hp = Milli(hp.min(hp_max).max(i64::from(u.hp.0)) as i32);
        mp += n * (mp_max / i64::from(REGEN_DIVISOR));
        if u.hp.0 >= max_hp(u).0 {
            reset_phases(u);
        }
    }
    if u.mp.0 < mp_max as i32 {
        u.mp = Milli(mp.min(mp_max) as i32);
    }
}

/// Step 5: every awake unit's regen, paid to now (a unit that woke this tick catches up here).
pub fn pay_awake(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    for i in 0..cx.rt.awake_units.len() {
        let id = cx.rt.awake_units[i];
        if let Some(u) = cx.zone.unit_mut(id) {
            pay_regen(u, now);
        }
    }
}

/// Step 12: corpses whose time has come stand up at home (`sleeping_due`, sorted `(tick, id)`;
/// a pop, not a pass over every corpse).
pub fn respawn_due(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    let n = cx.zone.sleeping_due.partition_point(|&(t, _)| t <= now);
    if n == 0 {
        return;
    }
    let mut due = std::mem::take(&mut cx.scratch.due);
    due.clear();
    due.extend(cx.zone.sleeping_due.drain(..n).map(|(_, id)| id));
    for &id in &due {
        if !crate::hooks::respawn_allowed(cx, id) {
            continue;
        }
        respawn(cx, id);
    }
    cx.scratch.due = due;
}

/// A creature stands up again at home, whole, with nothing on its mind.
fn respawn(cx: &mut Ctx<'_>, id: UnitId) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit(id) else { return };
    if u.alive {
        return;
    }
    let (hx, hy) = u.home.cell();
    let pos = cx.rt.grid.nearest_free(hx, hy, RESPAWN_RADIUS, None).map_or(u.home, |(x, y)| Vec2::centre(x, y));
    let u = cx.zone.unit_mut(id).expect("unit");
    u.pos = pos;
    u.path = None;
    u.alive = true;
    u.hp = max_hp(u);
    u.mp = max_mp(u);
    // The TS left a respawned creature with the empty energy it died with.
    u.energy = ENERGY_MAX;
    u.energy_locked = false;
    u.died_at = None;
    u.combat = CombatState::Idle;
    u.target = None;
    u.synced = now;
    reset_phases(u);
    let key = u.key;
    cx.rt.enter(cx.zone.unit(id).expect("unit"));
    if let Some(k) = key {
        cx.world.flags.remove(&FlagKey::Dead(k));
    }
    cx.emit(EventKind::Respawn { unit: id });
}

/// A dead seat's `respawn_at` has come (step 6). Death is harsh on purpose: she wakes at the
/// party's last bed or fire, however far that is (another zone: travel at step 14, like every
/// other zone change); before anyone has rested, at the door she came in by.
pub fn revive_player(cx: &mut Ctx<'_>, seat: Seat) {
    let now = cx.world.tick;
    let p = &mut cx.world.players[seat.index()];
    p.respawn_at = None;
    let (body, last_mark) = (p.unit, p.last_mark);
    // Whatever she was carrying stays where she fell (the interact unit's).
    crate::hooks::put_down_dead(cx, seat, body);
    let Some(u) = cx.zone.unit_mut(body) else { return };
    u.alive = true;
    u.hp = max_hp(u);
    u.mp = max_mp(u);
    u.energy = ENERGY_MAX;
    u.energy_locked = false;
    u.died_at = None;
    u.synced = now;
    u.carrying = None;
    let dead_at = u.pos;
    // Lock-ins undo themselves and re-arm, so a death never leaves a gate shut in her face (the
    // triggers unit's).
    crate::hooks::reset_lock_ins(cx, seat, body);
    let rest = cx.world.rest;
    let here = cx.zone.id;
    let pos = match rest {
        Some(r) if r.zone != here => {
            cx.world.players[seat.index()].travel =
                Some(TravelRequest { zone: r.zone, mark: last_mark, at: Some(r.pos) });
            dead_at
        }
        Some(r) => r.pos,
        None => {
            let mark = cx.rt.mark(last_mark).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y)));
            let (mx, my) = mark.unwrap_or(dead_at.cell());
            cx.rt
                .grid
                .nearest_free(mx, my, REVIVE_RADIUS, None)
                .map_or(Vec2::centre(mx, my), |(x, y)| Vec2::centre(x, y))
        }
    };
    let u = cx.zone.unit_mut(body).expect("her body");
    u.pos = pos;
    u.path = None;
    cx.rt.enter(cx.zone.unit(body).expect("her body"));
    cx.emit(EventKind::Respawn { unit: body });
    cx.emit(EventKind::Toast(if rest.is_some() { ToastKind::WokeAtRest } else { ToastKind::WokeAtDoor }));
}

/// `Dev(Hp)`: whole points, at least 1, at most full.
pub fn dev_hp(u: &mut Unit, points: i32, now: Tick) {
    pay_regen(u, now);
    u.hp = Milli::from_points(points.clamp(1, max_hp(u).points()));
}

/// `Dev(Mp)`: whole points, 0 to full.
pub fn dev_mp(u: &mut Unit, points: i32, now: Tick) {
    pay_regen(u, now);
    u.mp = Milli::from_points(points.clamp(0, max_mp(u).points()));
}

#[cfg(test)]
mod tests {
    use jane_core::action::Facing;

    use super::*;
    use crate::ids::UnitId;
    use crate::units::new_unit;

    fn unit(def: &str) -> Unit {
        let d = jane_data::catalog().combat.unit_id(def).unwrap();
        new_unit(UnitId::new(1).unwrap(), None, d, Vec2::ZERO, Facing::South, Tick::ZERO)
    }

    /// §4.3: a sleeper paid on waking has exactly what a unit paid every tick has.
    #[test]
    fn paid_at_once_equals_paid_every_tick() {
        for (def, combat) in
            [("skeleton", CombatState::Leash), ("bandit", CombatState::Idle), ("jane", CombatState::Idle)]
        {
            let mut a = unit(def);
            a.hp = Milli(1_234);
            a.mp = Milli(17);
            a.combat = combat;
            a.phase = 1;
            let mut b = a.clone();
            for t in 1..=700 {
                pay_regen(&mut a, Tick(t));
            }
            pay_regen(&mut b, Tick(700));
            assert_eq!((a.hp, a.mp, a.phase), (b.hp, b.mp, b.phase), "{def}");
            assert_eq!(b.synced, Tick(700));
        }
        // A leashing creature is whole again in a little over 300 ticks (the rate is floored to
        // the milli-point); she only gets mana.
        let mut s = unit("skeleton");
        s.hp = Milli(1);
        s.combat = CombatState::Leash;
        pay_regen(&mut s, Tick(300));
        assert!(s.hp < max_hp(&s));
        pay_regen(&mut s, Tick(301));
        assert_eq!(s.hp, max_hp(&s));
        let mut j = unit("jane");
        j.hp = Milli(1);
        j.mp = Milli::ZERO;
        pay_regen(&mut j, Tick(10));
        assert_eq!((j.hp, j.mp), (Milli(1), Milli(10 * i32::from(j.spirit))));
        // In the fight nothing comes back but mana; the dead gain nothing at all.
        let mut f = unit("skeleton");
        f.hp = Milli(1);
        f.combat = CombatState::Combat;
        pay_regen(&mut f, Tick(100));
        assert_eq!(f.hp, Milli(1));
        f.alive = false;
        f.mp = Milli::ZERO;
        pay_regen(&mut f, Tick(200));
        assert_eq!(f.mp, Milli::ZERO);
    }
}
