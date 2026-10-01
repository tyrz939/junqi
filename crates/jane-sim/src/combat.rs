//! The cast pipeline (`sim/combat.ts`, SYSTEMS.md §3). Player and AI call the same
//! [`try_cast`]:
//!
//! 1. **validate**: dead, stunned, target, enemy, sight, range, *then* cooldown, GCD, mp, energy
//!    (2020's order: range before cooldown, so an AI that hears `TooFar` keeps walking and one
//!    that hears `OnCooldown` stands still);
//! 2. **spawn a kind** (melee, bolt, self, ally, world, ground), never a class;
//! 3. **pay** only if the kind says the cast was valid;
//! 4. **queue**: a blow is a [`Hit`] in the zone's queue, never straight to hp;
//! 5. **flush** (`flush.rs`, step 10) is the only place hp changes.
//!
//! Failure is a [`SpellError`], never a silent no-op. The target is resolved to a live unit (or
//! nothing) before anything is read from it: 2020's `CastSpell` read `current_target.faction`
//! before `instance_exists(current_target)`.
//!
//! **Power rolls** are whole points, as the TS's were (`makeHit` rounded): the fixed part
//! `stat / div` is floored to a milli-point (`div` is in thousandths, so it is `stat * 10^6 /
//! div` milli), the random part is `irandom(floor(stat / var_div))` whole points (GameMaker's
//! `irandom` floors its bound), `flat` is added, a crit doubles the sum, and the sum is rounded
//! half up to a whole point (ARCHITECTURE.md §2). Draw order per blow: the power roll, then
//! the crit roll, from `ZoneState.rng` only (§4.4).

use jane_core::action::{School, Stat};
use jane_core::angle::{along, bearing, cos_q15, sin_q15};
use jane_core::num::{CELL_FX, dist_sq, div_round, isqrt};
use jane_core::tile::BLOCK_SHOT;
use jane_core::{Angle, EffectId, Fx, Key, Milli, Sfc32, SpellId, UnitDefId, Vec2};
use jane_data::{Controller, Faction, SpellDef, SpellKind, SpellPower, WorldSpell};

use crate::actions::Subject;
use crate::ctx::Ctx;
use crate::event::{Event, EventKind, SfxKind, SpellError, ToastKind};
use crate::ids::{Seat, UnitId};
use crate::input::InputFrame;
use crate::los::{first_blocked_cell, line_of_sight};
use crate::runtime::ZoneRuntime;
use crate::state::{GameState, Ground, Projectile, Unit};
use crate::status::{apply_effect, is_stunned, offence};
use crate::tuning::{
    ALLY_AIM_SLACK_FX, BOLT_START_FX, CRIT_ONE_IN, DEV_KILL_HIT, DEV_KILL_REACH_FX, DEV_SPAWN_OFFSET, DEV_SPAWN_RADIUS,
    GCD, MELEE_BEHIND_FX, SNAKE_HITBOX_EVERY, px,
};
use crate::units::{def_of, face_angle, face_vector, facing_angle, new_unit, restore_energy};

/// A blow waiting for the flush. Hits live in `Scratch.hits`, one queue per zone: a step lands
/// everything it deals, so the queue is empty between steps and is neither saved nor hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub to: UnitId,
    /// Whole points for a spell's blow; any milli for a status pulse or a verb.
    pub amount: Milli,
    /// `Heal` heals; anything else hurts.
    pub school: School,
    pub from: Option<UnitId>,
    pub crit: bool,
    /// Applied to the victim if it lives through the blow.
    pub status: Option<EffectId>,
}

/// Queue a blow in the ctx's zone. It lands at this step's flush (step 10).
pub fn queue_hit(cx: &mut Ctx<'_>, hit: Hit) {
    cx.scratch.hits[cx.zone.id.index()].push(hit);
}

/// Enemies: different sides, one of them the party's (`units.ts isEnemy`). Undead and beasts do
/// not fight each other.
pub fn is_enemy(a: Faction, b: Faction) -> bool {
    a != b && (a == Faction::Friendly || b == Faction::Friendly)
}

/// A unit's body radius.
pub fn bounds(u: &Unit) -> i64 {
    i64::from(def_of(u).bounds.0)
}

/// The largest body radius in the catalog, for widening a search box by it.
pub fn max_bounds() -> i32 {
    static MAX: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *MAX.get_or_init(|| jane_data::catalog().combat.units.iter().map(|d| d.bounds.0).max().unwrap_or(0))
}

/// Centre distance, `Fx` (one root).
pub fn distance(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64))
}

/// `GetDistanceBetweenBounds`: centre distance less both bodies, never below 0, `Fx`. Range 0
/// therefore means touching, which is how every 2020 melee row is written.
pub fn metres_between(a: &Unit, b: &Unit) -> i64 {
    (distance(a.pos, b.pos) - bounds(a) - bounds(b)).max(0)
}

/// The squared distance from a point to a unit's body: its feet, or for a snake the nearest of
/// every [`SNAKE_HITBOX_EVERY`]-th trail point (`snake.ts bodyDistance`).
pub fn body_dist_sq(u: &Unit, at: Vec2) -> i64 {
    let mut best = dist_sq(u.pos, at);
    if let Some(s) = &u.snake {
        for p in s.trail.iter().skip(SNAKE_HITBOX_EVERY - 1).step_by(SNAKE_HITBOX_EVERY) {
            best = best.min(dist_sq(*p, at));
        }
    }
    best
}

/// Present units whose cell lies within `reach` of `at` on either axis, into `out` (a
/// superset: callers test exactly). Block then id order.
pub fn query_near(rt: &ZoneRuntime, at: Vec2, reach: i64, out: &mut Vec<UnitId>) {
    let r = reach.clamp(0, i64::from(i32::MAX / 4)) as i32;
    let c = |v: i32| v.div_euclid(CELL_FX);
    rt.unit_blocks.query(c(at.x.0 - r), c(at.y.0 - r), c(at.x.0 + r), c(at.y.0 + r), out);
}

/// Round milli-points half up to a whole point (the TS's `Math.round` on whole-point hits).
pub fn round_points(m: i64) -> Milli {
    Milli((div_round(m, 1000) * 1000) as i32)
}

/// `stat / div + irandom(stat / var_div) + flat`, in milli-points, unrounded.
pub fn roll_power(rng: &mut Sfc32, u: &Unit, p: &SpellPower) -> i64 {
    let stat = i64::from(match p.stat {
        Stat::Strength => u.strength,
        Stat::Spirit => u.spirit,
    });
    let fixed = stat * 1_000_000 / i64::from(p.div.max(1));
    let var = i64::from(rng.irandom((stat * 1000 / i64::from(p.var_div.max(1))) as i32)) * 1000;
    fixed + var + i64::from(p.flat.0)
}

/// A spell's blow from `caster` (`makeHit`): the power roll, then the crit roll (a status's
/// `crit_one_in` beats the 1 in 20), doubled on a crit and rounded to a whole point.
fn make_hit(cx: &mut Ctx<'_>, caster: UnitId, spell: &SpellDef, to: UnitId, status: Option<EffectId>) -> Hit {
    let now = cx.world.tick;
    let zone = &mut *cx.zone;
    let ix = zone.unit_ix(caster).expect("a caster in its zone");
    let u = &zone.units[ix];
    let one_in = match offence(u, now).crit_one_in {
        0 => CRIT_ONE_IN,
        n => u32::from(n),
    };
    let power = spell.power.as_ref().map_or(0, |p| roll_power(&mut zone.rng, &zone.units[ix], p));
    let crit = zone.rng.below(one_in) == 0;
    let amount = round_points(if crit { power * 2 } else { power });
    Hit { to, amount, school: spell.school, from: Some(caster), crit, status }
}

/// What a unit can cast: the def's book (or its phase's), and for a seat's body everything the
/// table has learned. An AI's phase book replaces its own.
pub fn book_of(u: &Unit) -> &'static [SpellId] {
    let def = def_of(u);
    match u.phase {
        0 => def.book,
        n => def.phases.get(usize::from(n) - 1).map_or(def.book, |p| p.book),
    }
}

pub fn book_has(u: &Unit, learned: &[SpellId], spell: SpellId) -> bool {
    book_of(u).contains(&spell) || (u.controller == Controller::Player && learned.contains(&spell))
}

/// Cast `spell` from `caster` (a unit in the ctx's zone). `aim`: the (assisted) aim, `None` to
/// cast at the caster's target or along its facing. `on`: the unit under the cursor; `None`
/// with no cursor (a pad, or an AI). See [`try_cast_with`].
pub fn try_cast(
    cx: &mut Ctx<'_>,
    caster: UnitId,
    spell: SpellId,
    aim: Option<Angle>,
    on: Option<UnitId>,
) -> Result<(), SpellError> {
    let def = cx.cat.combat.spell(spell);
    try_cast_with(cx, caster, spell, def, aim, on)
}

/// [`try_cast`] over a row that need not be the catalog's (tests prove a verb on a row made
/// for them, as the TS did for `mend`).
pub fn try_cast_with(
    cx: &mut Ctx<'_>,
    caster: UnitId,
    id: SpellId,
    spell: &SpellDef,
    aim: Option<Angle>,
    on: Option<UnitId>,
) -> Result<(), SpellError> {
    let now = cx.world.tick;
    let Some(cix) = cx.zone.unit_ix(caster) else { return Err(SpellError::CastUnsuccessful) };
    crate::life::pay_regen(&mut cx.zone.units[cix], now);
    let c = &cx.zone.units[cix];
    if !c.alive {
        return Err(SpellError::YouAreDead);
    }
    if is_stunned(c, now) {
        return Err(SpellError::CastUnsuccessful);
    }
    let mut target = c.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.alive);
    if spell.needs_target {
        let Some(t) = target else { return Err(SpellError::NoTarget) };
        if spell.needs_enemy != is_enemy(c.faction, t.faction) {
            return Err(SpellError::NotValidTarget);
        }
        if spell.needs_los && !line_of_sight(&cx.rt.grid, c.pos, t.pos) {
            return Err(SpellError::NotInLos);
        }
        if metres_between(c, t) > i64::from(spell.range.0) {
            return Err(SpellError::TooFar);
        }
    } else if target.is_some_and(|t| spell.needs_enemy && !is_enemy(c.faction, t.faction)) {
        target = None;
    }
    let mut target = target.map(|t| t.id);
    // Friendly spells choose who before they check what it costs, like every other target check.
    let friend = if spell.kind == SpellKind::Ally { choose_friend(cx, cix, spell, aim, target, on)? } else { None };
    let c = &cx.zone.units[cix];
    if c.cooldowns.iter().any(|&(s, until)| s == id && until > now) {
        return Err(SpellError::OnCooldown);
    }
    if c.gcd_until > now && !spell.gcd_immune {
        return Err(SpellError::OnGcd);
    }
    if c.mp < spell.mp {
        return Err(SpellError::NotEnoughMp);
    }
    if c.energy < spell.energy {
        return Err(SpellError::NotEnoughEnergy);
    }
    if let Some(a) = aim {
        face_angle(&mut cx.zone.units[cix], a);
        target = None;
    }

    let valid = match spell.kind {
        SpellKind::Melee => {
            cast_melee(cx, caster, id, spell, target);
            true
        }
        SpellKind::Bolt => {
            cast_bolt(cx, caster, id, spell, target, aim);
            true
        }
        SpellKind::OnSelf => {
            if let Some(e) = spell.effect {
                apply_effect(cx, caster, e, Some(caster));
            }
            true
        }
        SpellKind::Ally => {
            let to = friend.unwrap_or(caster);
            if spell.power.is_some() {
                let hit = make_hit(cx, caster, spell, to, None);
                queue_hit(cx, hit);
            }
            if let Some(e) = spell.effect {
                apply_effect(cx, to, e, Some(caster));
            }
            true
        }
        SpellKind::Ground => {
            cast_ground(cx, caster, id, spell, target);
            true
        }
        SpellKind::World => crate::hooks::world_verb(cx, caster, spell.world.unwrap_or(WorldSpell::Repair)),
    };
    if !valid {
        return Err(SpellError::CastUnsuccessful);
    }

    // Pay.
    let tpos = target.and_then(|t| cx.zone.unit(t)).map(|t| t.pos);
    let u = &mut cx.zone.units[cix];
    u.mp -= spell.mp;
    u.energy -= spell.energy;
    if spell.cooldown.0 > 0 {
        u.cooldowns.retain(|&(s, until)| s != id && until > now);
        u.cooldowns.push((id, now.after(spell.cooldown)));
    }
    if !spell.gcd_immune {
        u.gcd_until = now.after(GCD);
    }
    u.stop_until = u.stop_until.max(now.after(spell.stop));
    if let Some(t) = tpos {
        face_vector(u, i64::from(t.x.0 - u.pos.x.0), i64::from(t.y.0 - u.pos.y.0));
    }
    let at = u.pos;
    if let Some(seat) = cx.party.seat_of(caster) {
        cx.world.players[seat.index()].stats.casts += 1;
    }
    cx.emit(EventKind::Cast { unit: caster, spell: id, at });
    Ok(())
}

/// Who a friendly spell lands on. One button, no target frame, no modifier key:
///
/// - **a cursor** (`on` is `Some`): over a friend, her; she must be in range and in sight, and if
///   she is not the cast fails and says why (the player pointed at someone and meant it). Over
///   herself, an enemy, a stranger or grass (the client names her own body for "over
///   nobody"): herself.
/// - **no cursor** (`on` is `None`, a pad): the friend nearest the aim line, or herself with the
///   stick at rest or nobody that way.
/// - **an AI**: its friendly target, else itself.
///
/// `None` is the caster. It never fails for want of a friend, so it is the same spell alone.
fn choose_friend(
    cx: &Ctx<'_>,
    cix: usize,
    spell: &SpellDef,
    aim: Option<Angle>,
    target: Option<UnitId>,
    on: Option<UnitId>,
) -> Result<Option<UnitId>, SpellError> {
    let c = &cx.zone.units[cix];
    if let Some(id) = on {
        if id == c.id {
            return Ok(None);
        }
        let Some(u) = cx.zone.unit(id) else { return Ok(None) };
        if !u.alive || u.hidden || cx.party.seat_of(id).is_none() {
            return Ok(None);
        }
        if spell.needs_los && !line_of_sight(&cx.rt.grid, c.pos, u.pos) {
            return Err(SpellError::NotInLos);
        }
        if metres_between(c, u) > i64::from(spell.range.0) {
            return Err(SpellError::TooFar);
        }
        return Ok(Some(id));
    }
    if let Some(a) = aim {
        return Ok(friend_along(cx, c, spell, a));
    }
    Ok(target.filter(|&t| cx.zone.unit(t).is_some_and(|u| !is_enemy(c.faction, u.faction))))
}

/// The living party member here closest to the aim ray, in range and (if the spell asks) in sight.
fn friend_along(cx: &Ctx<'_>, c: &Unit, spell: &SpellDef, aim: Angle) -> Option<UnitId> {
    let (cos, sin) = (i64::from(cos_q15(aim).0), i64::from(sin_q15(aim).0));
    let mut best = None;
    let mut best_off = ALLY_AIM_SLACK_FX;
    for &(zone, id, _) in cx.party.bodies.iter().flatten() {
        if zone != cx.zone.id || id == c.id {
            continue;
        }
        let Some(u) = cx.zone.unit(id) else { continue };
        if !u.alive || u.hidden {
            continue;
        }
        let dx = i64::from(u.pos.x.0 - c.pos.x.0);
        let dy = i64::from(u.pos.y.0 - c.pos.y.0);
        if dx * cos + dy * sin <= 0 || metres_between(c, u) > i64::from(spell.range.0) {
            continue;
        }
        let off = (dx * sin - dy * cos).abs() >> 15;
        if off >= best_off {
            continue;
        }
        if spell.needs_los && !line_of_sight(&cx.rt.grid, c.pos, u.pos) {
            continue;
        }
        best = Some(id);
        best_off = off;
    }
    best
}

/// Melee: the locked target if it is in reach; else the nearest enemy in reach, preferring the
/// facing half-plane. Forgiving on purpose: a parked cursor or a centred stick should not make
/// thirty swings whiff at a skeleton chewing on her back, so anything behind still counts, it
/// just loses ties to what she faces. A swing that finds nobody is still a swing. Ties go to
/// the lower id.
fn cast_melee(cx: &mut Ctx<'_>, caster: UnitId, id: SpellId, spell: &SpellDef, locked: Option<UnitId>) {
    let c = Caster::of(cx.zone.unit(caster).expect("caster"));
    let range = i64::from(spell.range.0);
    let mut victim =
        locked.filter(|&t| cx.zone.unit(t).is_some_and(|u| is_enemy(c.faction, u.faction) && c.reach_to(u) <= range));
    if victim.is_none() {
        let (fx, fy) = c.facing.delta();
        let mut best: Option<(i64, UnitId)> = None;
        let mut near = std::mem::take(&mut cx.scratch.near);
        query_near(cx.rt, c.pos, range + c.bounds + i64::from(max_bounds()) + i64::from(CELL_FX), &mut near);
        for &uid in &near {
            let Some(u) = cx.zone.unit(uid) else { continue };
            if uid == caster
                || !u.alive
                || u.hidden
                || u.controller == Controller::Npc
                || !is_enemy(c.faction, u.faction)
            {
                continue;
            }
            let d = c.reach_to(u);
            if d > range {
                continue;
            }
            let dot =
                i64::from(u.pos.x.0 - c.pos.x.0) * i64::from(fx) + i64::from(u.pos.y.0 - c.pos.y.0) * i64::from(fy);
            let score = d + if dot < -i64::from(CELL_FX / 2) { MELEE_BEHIND_FX } else { 0 };
            if best.is_some_and(|b| (score, uid) >= b) {
                continue;
            }
            if !line_of_sight(&cx.rt.grid, c.pos, u.pos) {
                continue;
            }
            best = Some((score, uid));
        }
        cx.scratch.near = near;
        victim = best.map(|b| b.1);
    }
    cx.emit(EventKind::Swing { unit: caster, at: c.pos, facing: c.facing });
    let Some(v) = victim else { return };
    let vpos = cx.zone.unit(v).expect("victim").pos;
    if let Some(u) = cx.zone.unit_mut(caster) {
        face_vector(u, i64::from(vpos.x.0 - u.pos.x.0), i64::from(vpos.y.0 - u.pos.y.0));
    }
    let hit = make_hit(cx, caster, spell, v, spell.effect);
    queue_hit(cx, hit);
    // What a status adds to every melee blow (Firelash, Winterbite).
    let now = cx.world.tick;
    let cat = cx.cat;
    let n = cx.zone.unit(caster).map_or(0, |u| u.statuses.len());
    for i in 0..n {
        let Some(s) = cx.zone.unit(caster).and_then(|u| u.statuses.get(i)).copied() else { break };
        if s.until <= now {
            continue;
        }
        if let Some(m) = cat.combat.effect(s.effect).on_melee {
            queue_hit(
                cx,
                Hit { to: v, amount: m.amount, school: m.school, from: Some(caster), crit: false, status: m.effect },
            );
        }
    }
    if spell.restore_energy.0 > 0 {
        if let Some(u) = cx.zone.unit_mut(caster) {
            restore_energy(u, Milli(spell.restore_energy.0 * if hit.crit { 3 } else { 1 }));
        }
    }
    cx.emit(EventKind::Impact { spell: id, school: spell.school, at: vpos });
}

/// What a cast reads of its caster while the zone is written.
#[derive(Clone, Copy)]
struct Caster {
    pos: Vec2,
    faction: Faction,
    facing: jane_core::action::Facing,
    bounds: i64,
}

impl Caster {
    fn of(u: &Unit) -> Caster {
        Caster { pos: u.pos, faction: u.faction, facing: u.facing, bounds: bounds(u) }
    }

    /// [`metres_between`] from the caster.
    fn reach_to(&self, u: &Unit) -> i64 {
        (distance(self.pos, u.pos) - self.bounds - bounds(u)).max(0)
    }
}

/// Bolts fly straight from the caster: along the aim, else at the target, else along her facing.
/// A fan is random inside its arc (2020's cactus); a ring (`fan` 360) is evenly spaced with the
/// first bolt on the line. Each bolt rolls its own blow now.
fn cast_bolt(
    cx: &mut Ctx<'_>,
    caster: UnitId,
    id: SpellId,
    spell: &SpellDef,
    target: Option<UnitId>,
    aim: Option<Angle>,
) {
    let c = Caster::of(cx.zone.unit(caster).expect("caster"));
    let dir = match (aim, target.and_then(|t| cx.zone.unit(t))) {
        (Some(a), _) => a,
        (None, Some(t)) if t.pos != c.pos => bearing(c.pos, t.pos),
        _ => facing_angle(c.facing),
    };
    let speed = spell.speed.unwrap_or(px(2));
    let count = u32::from(spell.count.max(1));
    let fan = spell.fan.0;
    let left = Fx(spell.range.0 + 2 * c.bounds as i32);
    let now = cx.world.tick;
    for i in 0..count {
        let heading = if count > 1 || fan > 0 {
            let offset = if fan == u16::MAX {
                (i * 65_536 / count) as i32
            } else {
                cx.zone.rng.irandom(i32::from(fan)) - i32::from(fan) / 2
            };
            dir.wrapping_add(offset)
        } else {
            dir
        };
        let hit = make_hit(cx, caster, spell, caster, spell.effect);
        // Born past her chest, unless that is through something that stops a shot: hard against a
        // shut gate the start would sit inside its cells, and a flight never tests the cell it
        // starts in. Then it is born at her centre and the gate stops it on its first moves.
        let born = c.pos + along(heading, BOLT_START_FX);
        let pos = if first_blocked_cell(&cx.rt.grid, c.pos, born, BLOCK_SHOT).is_some() { c.pos } else { born };
        let pid = cx.world.next.proj();
        cx.zone.projectiles.push(Projectile {
            id: pid,
            spell: id,
            from: Some(caster),
            faction: c.faction,
            pos,
            vel: along(heading, speed),
            heading,
            left,
            born: now,
            hit: hit.amount,
            crit: hit.crit,
        });
    }
}

/// A pool on the ground under the target (or the caster, cast with an aim or at nobody). Its
/// first pulse comes the row's `delay` after the cast: until then it is only seen.
fn cast_ground(cx: &mut Ctx<'_>, caster: UnitId, id: SpellId, spell: &SpellDef, target: Option<UnitId>) {
    let Some(pool) = spell.ground else { return };
    let c = cx.zone.unit(caster).expect("caster");
    let (faction, cpos) = (c.faction, c.pos);
    let pos = target.and_then(|t| cx.zone.unit(t)).map_or(cpos, |t| t.pos);
    let now = cx.world.tick;
    let gid = cx.world.next.ground();
    cx.zone.grounds.push(Ground {
        id: gid,
        spell: id,
        from: Some(caster),
        faction,
        pos,
        radius: pool.radius,
        until: now.after(pool.duration),
        next_pulse: now.after(pool.delay),
    });
}

// --- the seat's side: commands, verbs, the console -------------------------------------------

/// `Command::Cast` and a bar slot's spell (`sim.ts playerCast`): not while reading, only a
/// spell she knows, along the frame's aim resolved by assist (§5.4). A failure says why to her.
pub fn player_cast(cx: &mut Ctx<'_>, seat: Seat, spell: SpellId, on: Option<UnitId>, frame: InputFrame) {
    let Some(p) = cx.world.player(seat) else { return };
    if p.dialogue.is_some() {
        return;
    }
    let body = p.unit;
    let Some(u) = cx.zone.unit(body) else { return };
    if !book_has(u, &cx.world.growth.spells, spell) {
        return;
    }
    let aim = frame.aim.map(|raw| crate::assist::assisted_aim(cx, seat, spell, raw, frame.assist));
    if let Err(why) = try_cast(cx, body, spell, aim, on) {
        cx.emit(EventKind::CastFailed { unit: body, spell, why });
        if why.says() {
            cx.emit(EventKind::Toast(ToastKind::SpellError(why)));
        }
    }
}

/// `Command::Bar`: the slot's spell is cast; its item is used (the inventory unit's).
pub fn bar_command(cx: &mut Ctx<'_>, seat: Seat, slot: u8, on: Option<UnitId>, frame: InputFrame) {
    let Some(s) = cx.world.player(seat).and_then(|p| p.bar.get(usize::from(slot)).copied().flatten()) else { return };
    match s {
        jane_data::BarSlot::Spell(spell) => player_cast(cx, seat, spell, on, frame),
        jane_data::BarSlot::Item(item) => crate::hooks::use_item(cx, seat, item),
    }
}

/// Learn a spell for the table (`actions.ts teach`): growth is the world's, so every body knows
/// it at once, the away and the late included (a seat's book is derived), and it goes on every
/// seat's bar where there is room. Returns whether it was new; the caller says so.
pub fn teach(world: &mut GameState, spell: SpellId) -> bool {
    if world.growth.spells.contains(&spell) || spell.index() >= jane_data::catalog().combat.spells.len() {
        return false;
    }
    world.growth.spells.push(spell);
    for p in &mut world.players {
        bind_learned(&mut p.bar, spell);
    }
    true
}

/// A learned spell goes in the first free slot of a bar that does not already hold it.
fn bind_learned(bar: &mut [Option<jane_data::BarSlot>], spell: SpellId) {
    if bar.contains(&Some(jane_data::BarSlot::Spell(spell))) {
        return;
    }
    if let Some(free) = bar.iter_mut().find(|s| s.is_none()) {
        *free = Some(jane_data::BarSlot::Spell(spell));
    }
}

/// The events a lesson makes, heard by the whole party.
pub fn learned_events(spell: SpellId) -> [EventKind; 2] {
    [EventKind::Learn(spell), EventKind::Toast(ToastKind::Learned(spell))]
}

/// `Action::Learn`: only with someone to learn it.
pub fn learn_verb(cx: &mut Ctx<'_>, spell: SpellId) {
    if cx.actor.is_none() || !teach(cx.world, spell) {
        return;
    }
    for k in learned_events(spell) {
        cx.emit_all(k);
    }
}

/// The unit a verb lands on: the subject if it is a unit here, else the actor's body.
fn verb_target(cx: &Ctx<'_>, subject: Subject) -> Option<UnitId> {
    match subject {
        Subject::Unit(id) if cx.zone.unit(id).is_some() => Some(id),
        _ => cx.actor_unit(),
    }
}

/// `Action::Strike`: every living, present-or-sleeping unit standing in the rect is hit once,
/// the party's own only if the row says so. Whoever pulled the lever struck the blow (the kill is
/// hers, and so is the party's penalty); a trap nobody set strikes as the world, from nobody.
pub fn strike_verb(
    cx: &mut Ctx<'_>,
    rect: Key,
    amount: Milli,
    school: School,
    effect: Option<EffectId>,
    hits_friends: bool,
    subject: Subject,
) {
    let s = cx.sym(rect);
    let Some(r) = cx.rt.rects.get(&s).copied() else {
        cx.emit(EventKind::Missing(s));
        return;
    };
    let from = match subject {
        Subject::Unit(id) => cx.zone.unit(id).filter(|u| u.alive).map(|u| u.id),
        _ => None,
    };
    let zi = cx.zone.id.index();
    for u in &cx.zone.units {
        if !u.alive || u.hidden || (u.faction == Faction::Friendly && !hits_friends) {
            continue;
        }
        let (x, y) = u.pos.cell();
        if !r.contains(x, y) {
            continue;
        }
        cx.scratch.hits[zi].push(Hit { to: u.id, amount, school, from, crit: false, status: effect });
    }
    let at = Vec2::centre(r.x + r.w / 2, r.y + r.h / 2);
    cx.emit(EventKind::Sfx { kind: SfxKind::Strike, at });
    cx.emit(EventKind::Shake(3));
}

/// `Action::Status`: on the subject, else on the actor.
pub fn status_verb(cx: &mut Ctx<'_>, effect: EffectId, subject: Subject) {
    let from = match subject {
        Subject::Unit(id) => Some(id),
        _ => None,
    };
    if let Some(u) = verb_target(cx, subject) {
        apply_effect(cx, u, effect, from);
    }
}

/// `Action::Heal`: on the subject, else on the actor; a share of full health rounds to a whole
/// point (the 2020 apple healed 25 % while its tooltip said 25).
pub fn heal_verb(cx: &mut Ctx<'_>, heal: jane_core::action::Heal, subject: Subject) {
    let Some(id) = verb_target(cx, subject) else { return };
    let Some(u) = cx.zone.unit(id) else { return };
    let amount = match heal {
        jane_core::action::Heal::Flat(m) => m,
        jane_core::action::Heal::Pct(p) => round_points(i64::from(crate::units::max_hp(u).0) * i64::from(p.0) / 1000),
    };
    queue_hit(cx, Hit { to: id, amount, school: School::Heal, from: Some(id), crit: false, status: None });
}

/// `Dev(Kill)`: everything hostile she can see within 25 m: roughly the screen, never the next
/// room through a wall, and never something that is not there.
pub fn dev_kill(cx: &mut Ctx<'_>, body: UnitId) {
    let Some(me) = cx.zone.unit(body) else { return };
    let (pos, faction) = (me.pos, me.faction);
    let mut near = std::mem::take(&mut cx.scratch.near);
    query_near(cx.rt, pos, i64::from(DEV_KILL_REACH_FX), &mut near);
    for &id in &near {
        let Some(u) = cx.zone.unit(id) else { continue };
        let close =
            (u.pos.x.0 - pos.x.0).abs() <= DEV_KILL_REACH_FX && (u.pos.y.0 - pos.y.0).abs() <= DEV_KILL_REACH_FX;
        if !close || !u.alive || !u.awake || u.hidden || u.faction == faction {
            continue;
        }
        if !line_of_sight(&cx.rt.grid, pos, u.pos) {
            continue;
        }
        cx.scratch.hits[cx.zone.id.index()].push(Hit {
            to: id,
            amount: DEV_KILL_HIT,
            school: School::Physical,
            from: Some(body),
            crit: false,
            status: None,
        });
    }
    cx.scratch.near = near;
}

/// `Dev(Spawn)`: a unit of `def` three cells east of her, on the nearest free cell. It lands at
/// step 13 of this ctx (the command's), awake.
pub fn dev_spawn(cx: &mut Ctx<'_>, body: UnitId, def: UnitDefId) {
    if def.index() >= cx.cat.combat.units.len() {
        return;
    }
    let Some(me) = cx.zone.unit(body) else { return };
    let (x, y) = me.pos.cell();
    let Some((fx, fy)) = cx.rt.grid.nearest_roomy(x + DEV_SPAWN_OFFSET, y, DEV_SPAWN_RADIUS) else { return };
    let id = cx.world.next.unit();
    let u = new_unit(id, None, def, Vec2::centre(fx, fy), jane_core::action::Facing::South, cx.world.tick);
    cx.ops.spawn.push(u);
}

/// Push an event to one seat, whatever the ctx's actor: what is hers wherever it happens (a
/// blow she took, her body falling).
pub fn emit_to(cx: &mut Ctx<'_>, seat: Seat, kind: EventKind) {
    cx.events.push(Event { to: Some(seat), in_zone: Some(cx.zone.id), kind });
}
