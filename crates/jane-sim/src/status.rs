//! Statuses (`sim/status.ts`): effect rows on a unit. 2020 had one `speed_multiplier` and an
//! alarm that only the player's movement read, so an AI could never be slowed or stunned; here
//! every unit reads the same modifiers, because every unit is the same `Unit`.
//!
//! A status is **active** while `until > now`. Step 9 pulses the awake living units' statuses
//! and drops what ran out; a sleeper's statuses wait, and on waking the pulses it missed land as
//! one blow (ARCHITECTURE.md §4.3). Timed parts refresh rather than stack (a second Winterbite
//! potion resets the clock, it does not double it).

use jane_core::action::School;
use jane_core::{EffectId, Milli, Tick};

use crate::combat::{Hit, queue_hit};
use crate::ctx::Ctx;
use crate::event::EventKind;
use crate::ids::UnitId;
use crate::state::{StatusInst, Unit};
use crate::units::{def_of, max_mp};

fn effect(s: &StatusInst) -> &'static jane_data::EffectDef {
    jane_data::catalog().combat.effect(s.effect)
}

fn active(u: &Unit, now: Tick) -> impl Iterator<Item = (&StatusInst, &'static jane_data::EffectDef)> {
    u.statuses.iter().filter(move |s| s.until > now).map(|s| (s, effect(s)))
}

pub fn has_status(u: &Unit, e: EffectId, now: Tick) -> bool {
    u.statuses.iter().any(|s| s.effect == e && s.until > now)
}

pub fn is_stunned(u: &Unit, now: Tick) -> bool {
    active(u, now).any(|(_, d)| d.stun)
}

/// The product of every speed modifier, permille; 0 is rooted (a stun roots).
pub fn speed_factor(u: &Unit, now: Tick) -> i32 {
    let mut f = 1000;
    for (_, d) in active(u, now) {
        if d.stun {
            return 0;
        }
        f = f * i32::from(d.speed.0) / 1000;
    }
    f
}

/// The incoming damage multiplier for a school, permille, never below 0: the unit row's resist,
/// then each status's. Softened (`no_resist`): what it was proof against, it is not; what it
/// was weak to, it still is.
pub fn resist_factor(u: &Unit, school: School, now: Tick) -> i32 {
    let mut own = i32::from(def_of(u).resist_of(school).0);
    let mut f: i64 = 1000;
    for (_, d) in active(u, now) {
        let r = i64::from(d.resist_of(school).0);
        if r != 1000 {
            f = f * r / 1000;
        }
        if d.no_resist && own > 0 {
            own = 0;
        }
    }
    (f * i64::from(1000 - own) / 1000).max(0) as i32
}

/// What a unit's statuses add to the blows it deals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offence {
    /// Permille of damage dealt returned as health (summed).
    pub lifesteal: i32,
    /// The best 1-in-N crit roll any status grants; 0 for none.
    pub crit_one_in: u16,
}

pub fn offence(u: &Unit, now: Tick) -> Offence {
    let mut o = Offence::default();
    for (_, d) in active(u, now) {
        o.lifesteal += i32::from(d.lifesteal.0);
        if d.crit_one_in > 0 && (o.crit_one_in == 0 || d.crit_one_in < o.crit_one_in) {
            o.crit_one_in = d.crit_one_in;
        }
    }
    o
}

/// What a unit's statuses do to the blows it takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Defence {
    /// Mp paid per point of damage absorbed, permille (the cheapest shield wins); 0 for none.
    pub mana_shield: i32,
    pub mana_on_hit: Milli,
}

pub fn defence(u: &Unit, now: Tick) -> Defence {
    let mut d = Defence::default();
    for (_, e) in active(u, now) {
        let ms = i32::from(e.mana_shield.0);
        if ms > 0 && (d.mana_shield == 0 || ms < d.mana_shield) {
            d.mana_shield = ms;
        }
        d.mana_on_hit += e.mana_on_hit;
    }
    d
}

/// Apply an effect row to a unit here (`status.ts applyEffect`). Instant parts land now (a heal
/// is a blow in the queue; mana is paid at once); timed parts refresh rather than stack. An
/// `only_if_weak` effect takes only on what is weak to its school: a spark jolts a machine, not a
/// rat.
pub fn apply_effect(cx: &mut Ctx<'_>, id: UnitId, e: EffectId, from: Option<UnitId>) {
    let now = cx.world.tick;
    let def = cx.cat.combat.effect(e);
    let Some(ix) = cx.zone.unit_ix(id) else { return };
    let u = &mut cx.zone.units[ix];
    if !u.alive {
        return;
    }
    if let Some(school) = def.only_if_weak {
        if def_of(u).resist_of(school).0 >= 0 {
            return;
        }
    }
    if def.mana.0 > 0 {
        crate::life::pay_regen(u, now);
        u.mp = Milli((u.mp.0 + def.mana.0).min(max_mp(u).0));
    }
    if def.duration.0 > 0 {
        if let Some(s) = u.statuses.iter_mut().find(|s| s.effect == e) {
            s.until = now.after(def.duration);
            s.from = from;
        } else {
            let next_pulse = def.pulse.map_or(Tick::ZERO, |p| now.after(p.every));
            u.statuses.push(StatusInst {
                effect: e,
                until: now.after(def.duration),
                next_pulse,
                from,
                pool: Milli::ZERO,
            });
            cx.emit(EventKind::Status { unit: id, effect: e, on: true });
        }
        // A stun breaks a wind-up, a chill draws one out (`feel.rs`).
        crate::feel::on_status(cx, id, def);
    }
    if def.heal.0 > 0 {
        queue_hit(cx, Hit { to: id, amount: def.heal, school: School::Heal, from, crit: false, status: None });
    }
}

/// Every status off (death), each said.
pub fn clear_statuses(cx: &mut Ctx<'_>, id: UnitId) {
    let Some(ix) = cx.zone.unit_ix(id) else { return };
    while let Some(s) = cx.zone.units[ix].statuses.pop() {
        cx.emit(EventKind::Status { unit: id, effect: s.effect, on: false });
    }
}

/// Step 9: the awake living units' statuses pulse into the queue (the heal school heals) and
/// what ran out comes off. Pulses land at `next_pulse` up to and including `until`; a unit that
/// slept through some takes them now, summed into one blow.
pub fn step_statuses(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    let zi = cx.zone.id.index();
    for i in 0..cx.rt.awake_units.len() {
        let id = cx.rt.awake_units[i];
        let Some(ix) = cx.zone.unit_ix(id) else { continue };
        if !cx.zone.units[ix].alive {
            continue;
        }
        let mut k = 0;
        while k < cx.zone.units[ix].statuses.len() {
            let s = &mut cx.zone.units[ix].statuses[k];
            let def = cx.cat.combat.effect(s.effect);
            if let Some(p) = def.pulse {
                let last = s.until.0.min(now.0);
                let every = p.every.0.max(1);
                if s.next_pulse.0 <= last {
                    let n = (last - s.next_pulse.0) / every + 1;
                    s.next_pulse = Tick(s.next_pulse.0 + n * every);
                    let amount = Milli(p.amount.0.saturating_mul(n as i32));
                    let hit = Hit { to: id, amount, school: p.school, from: s.from, crit: false, status: None };
                    cx.scratch.hits[zi].push(hit);
                }
            }
            let s = cx.zone.units[ix].statuses[k];
            if s.until <= now {
                cx.zone.units[ix].statuses.remove(k);
                cx.emit(EventKind::Status { unit: id, effect: s.effect, on: false });
            } else {
                k += 1;
            }
        }
    }
}
