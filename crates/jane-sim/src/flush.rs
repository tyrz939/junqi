//! Step 10, the flush (`combat.ts flushIncoming`): **the only place a blow changes hp.** It runs
//! after every command, controller, bolt, pool and status has had its turn this tick, so
//! same-tick ordering can never decide who dies.
//!
//! A pass takes the zone's whole queue and lands it unit by unit in ascending id, each unit's
//! blows in the order they were dealt. What a pass raises (lifesteal, an effect's instant heal,
//! a list run by a death or a phase) waits for the next pass, and passes run until the queue is
//! empty: two in practice (lifesteal lands in the second, as in the TS's double flush), a third
//! only when a list run in the second strikes again. It ends: a blow that raises a damaging blow
//! needs a death or a phase entry, and each happens once. The TS let a blow raised on a later
//! unit land in the same pass; here nothing a pass raises lands before the pass is done.
//!
//! Per blow: heal (only what is missing), or damage = blow x resist x the party table (taken
//! by a seat's body, dealt by one to anything else; by head count, not by who stands where) x
//! god, less a mana shield, rounded to a whole point. Then mana on hit, the shake (hers alone),
//! lifesteal to the dealer, first-hit aggro (from outside the aggro radius too), the blow's
//! status, boss phases, and death.

use jane_core::action::School;
use jane_core::{Milli, Tick};
use jane_data::Controller;

use crate::actions::{Subject, run_actions};
use crate::combat::{Hit, emit_to, is_enemy, queue_hit, round_points};
use crate::ctx::{Ctx, forget_unit};
use crate::event::EventKind;
use crate::ids::UnitId;
use crate::state::{CombatState, FlagKey};
use crate::status::{apply_effect, clear_statuses, defence, offence, resist_factor};
use crate::tuning::{MAX_PLAYERS, PARTY_DEALT, PARTY_TAKEN, PLAYER_RESPAWN};
use crate::units::{def_of, max_hp, max_mp};

/// A pass cap no content reaches (see the module doc); what is left after it is dropped.
const MAX_PASSES: u32 = 16;

/// Step 10: land the zone's queue.
pub fn flush(cx: &mut Ctx<'_>) {
    let zi = cx.zone.id.index();
    for _ in 0..MAX_PASSES {
        if cx.scratch.hits[zi].is_empty() {
            return;
        }
        std::mem::swap(&mut cx.scratch.hits[zi], &mut cx.scratch.flushing);
        let flushing = std::mem::take(&mut cx.scratch.flushing);
        let mut ids = std::mem::take(&mut cx.scratch.hit_ids);
        ids.clear();
        for h in &flushing {
            if let Err(at) = ids.binary_search(&h.to) {
                ids.insert(at, h.to);
            }
        }
        for &id in &ids {
            flush_unit(cx, id, &flushing);
        }
        let mut flushing = flushing;
        flushing.clear();
        cx.scratch.flushing = flushing;
        cx.scratch.hit_ids = ids;
    }
    debug_assert!(cx.scratch.hits[zi].is_empty(), "the flush did not settle in {MAX_PASSES} passes");
    cx.scratch.hits[zi].clear();
}

fn flush_unit(cx: &mut Ctx<'_>, id: UnitId, hits: &[Hit]) {
    let now = cx.world.tick;
    let owner = cx.party.seat_of(id);
    // The co-op penalty: by how many are connected, not by who is standing here.
    let seats = usize::from(cx.party.size).clamp(1, MAX_PLAYERS) - 1;
    for h in hits.iter().filter(|h| h.to == id) {
        let Some(ix) = cx.zone.unit_ix(id) else { return };
        let u = &mut cx.zone.units[ix];
        if !u.alive {
            return;
        }
        crate::life::pay_regen(u, now);
        if h.school == School::Heal {
            let before = u.hp;
            u.hp = Milli((u.hp.0.saturating_add(h.amount.0)).min(max_hp(u).0));
            let gained = u.hp - before;
            if gained.0 > 0 {
                let at = u.pos;
                cx.emit(EventKind::Heal { unit: id, from: h.from, at, amount: gained });
            }
            continue;
        }
        let mut dmg = i64::from(h.amount.0) * i64::from(resist_factor(u, h.school, now));
        let party = if owner.is_some() {
            PARTY_TAKEN[seats].0
        } else if h.from.is_some_and(|f| cx.party.seat_of(f).is_some()) {
            PARTY_DEALT[seats].0
        } else {
            1000
        };
        dmg = dmg * i64::from(party) / 1_000_000;
        if owner.is_some_and(|s| cx.world.players[s.index()].god) {
            dmg = 0;
        }
        let def = defence(u, now);
        let mut absorbed = 0i64;
        if def.mana_shield > 0 && dmg > 0 && u.mp.0 > 0 {
            let ms = i64::from(def.mana_shield);
            absorbed = dmg.min(i64::from(u.mp.0) * 1000 / ms);
            u.mp.0 -= (absorbed * ms / 1000) as i32;
            dmg -= absorbed;
        }
        let amount = round_points(dmg);
        u.hp = Milli((u.hp.0 - amount.0).max(0));
        if def.mana_on_hit.0 > 0 {
            u.mp = Milli((u.mp.0 + def.mana_on_hit.0).min(max_mp(u).0));
        }
        let (at, hp, controller, combat, faction) = (u.pos, u.hp, u.controller, u.combat, u.faction);
        cx.emit(EventKind::Damage {
            unit: id,
            from: h.from,
            at,
            amount,
            school: h.school,
            crit: h.crit,
            absorbed: round_points(absorbed),
        });
        // Only the one who was hit feels it.
        if let Some(seat) = owner.filter(|_| amount.0 > 0) {
            emit_to(cx, seat, EventKind::Shake((1 + amount.points() / 40).min(4) as u8));
        }
        let source = h.from.and_then(|f| cx.zone.unit(f)).filter(|s| s.alive).map(|s| (s.id, s.faction));
        if let Some((sid, _)) = source.filter(|_| amount.0 > 0) {
            let steal = offence(cx.zone.unit(sid).expect("source"), now).lifesteal;
            if steal > 0 {
                let heal = round_points(i64::from(amount.0) * i64::from(steal) / 1000);
                queue_hit(
                    cx,
                    Hit { to: sid, amount: heal, school: School::Heal, from: Some(sid), crit: false, status: None },
                );
            }
        }
        // The first blow pulls aggro, even from outside the aggro radius.
        if let Some((sid, sf)) = source {
            let fights = !matches!(controller, Controller::Player | Controller::Npc);
            if fights && combat != CombatState::Combat && is_enemy(faction, sf) {
                let u = cx.zone.unit_mut(id).expect("victim");
                u.target = Some(sid);
                u.combat = CombatState::Combat;
                if !u.awake && !cx.ops.wake.contains(&id) {
                    cx.ops.wake.push(id);
                }
            }
        }
        if let Some(st) = h.status.filter(|_| hp.0 > 0) {
            apply_effect(cx, id, st, h.from);
        }
        if hp.0 > 0 && amount.0 > 0 && controller == Controller::Ai {
            enter_phases(cx, id, source.map(|s| s.0));
        }
        if hp.0 <= 0 {
            kill_unit(cx, id, source.map(|s| s.0));
            return;
        }
    }
}

/// A boss crosses into its next phase when its health falls to that row's `hp_below` of full
/// (a row at 1000 is entered by the first blow that hurts): it takes that phase's book (and
/// speed), and the phase's `on_enter` runs once with the boss as the subject, on behalf of
/// whoever landed the blow (nobody, if poison did). One blow through two thresholds runs both,
/// in order. Health thresholds only, and only for the ordinary AI: a snake's phases are its own
/// clock.
fn enter_phases(cx: &mut Ctx<'_>, id: UnitId, source: Option<UnitId>) {
    loop {
        let Some(u) = cx.zone.unit_mut(id) else { return };
        let phases = def_of(u).phases;
        let Some(row) = phases.get(usize::from(u.phase)) else { return };
        if i64::from(u.hp.0) * 1000 > i64::from(max_hp(u).0) * i64::from(row.hp_below.0) {
            return;
        }
        u.phase += 1;
        if let Some(list) = row.on_enter {
            let prev = cx.actor;
            cx.actor = source.and_then(|s| cx.party.seat_of(s));
            run_actions(cx, list, Subject::Unit(id));
            cx.actor = prev;
        }
    }
}

/// All its health back (it leashed and mended, or it stood up again): the fight starts from the
/// top, and the phase lists will run again when it is brought down again.
pub fn reset_phases(u: &mut crate::state::Unit) {
    if u.phase != 0 && u.controller == Controller::Ai && !def_of(u).phases.is_empty() {
        u.phase = 0;
    }
}

/// A unit's hp reached 0 (`combat.ts killUnit`). Everyone lets go of it; a seat's body waits for
/// its `respawn_at`; anything else rolls its loot, marks its name dead, counts for the party's
/// quests if one of them landed the blow, joins `sleeping_due` if its row stands up again, and
/// runs its `on_death` list on behalf of the slayer.
pub fn kill_unit(cx: &mut Ctx<'_>, id: UnitId, killer: Option<UnitId>) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    u.alive = false;
    u.order = None;
    u.dwell_until = Tick::ZERO;
    u.hp = Milli::ZERO;
    u.mp = Milli::ZERO;
    u.energy = Milli::ZERO;
    u.target = None;
    u.path = None;
    u.combat = CombatState::Idle;
    u.died_at = Some(now);
    u.synced = now;
    let (pos, def_id, key) = (u.pos, u.def, u.key);
    let def = cx.cat.combat.unit(def_id);
    clear_statuses(cx, id);
    forget_unit(cx.zone, cx.rt, cx.party, id);
    cx.emit(EventKind::Death { unit: id, def: def_id, at: pos });
    if let Some(seat) = cx.party.seat_of(id) {
        let p = &mut cx.world.players[seat.index()];
        p.stats.deaths += 1;
        p.dialogue = None;
        p.respawn_at = Some(now.after(PLAYER_RESPAWN));
        emit_to(cx, seat, EventKind::PlayerDied);
        return;
    }
    // A kill by anyone in the party counts for the party's quests.
    let slayer = killer.and_then(|k| cx.party.seat_of(k));
    if let Some(seat) = slayer {
        cx.world.players[seat.index()].stats.kills += 1;
        crate::hooks::quest_kill(cx, seat, def_id);
    }
    if let Some(k) = key {
        cx.world.flags.insert(FlagKey::Dead(k), 1);
    }
    crate::loot::roll_loot(cx, def_id, pos);
    if def.respawn.0 > 0 {
        let due = (now.after(def.respawn), id);
        let at = cx.zone.sleeping_due.partition_point(|&e| e < due);
        cx.zone.sleeping_due.insert(at, due);
    }
    crate::hooks::unit_died(cx, id, slayer);
    if let Some(list) = def.on_death {
        let prev = cx.actor;
        cx.actor = slayer;
        run_actions(cx, list, Subject::Unit(id));
        cx.actor = prev;
    }
}
