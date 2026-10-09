//! The fight's feel, the foes' side of it (PLAY-PLAN.md §2.1; `progress/audit/research-combat.md`
//! §4.5 to 4.8). Everything here is integer, in ticks, inside the sim: saved, hashed and replayed
//! like any other state, so a LAN table never sees two fights.
//!
//! - **Wind-ups.** A foe's blow (an AI's melee, bolt or pool) waits before it lands: its spell
//!   row's `windup`, else its unit row's, plus the table's input delay D at a LAN table
//!   (`GameState::table_delay`), so a guest reads it in a solo player's window. The wind-up is a
//!   [`Windup`] on the unit (the foe's `PendingCast`): the spell, the tick it lands, and the aim
//!   and point locked when it began. A melee lands only on its victim still in reach and inside
//!   a 90° arc of the line it began on; a bolt flies at the point it was aimed at; a pool is laid
//!   at once on its point (it is the tell) and bites when the wind-up ends. While it winds up the
//!   foe stands, faces its point and does nothing else (it is committed): stunning it cancels the
//!   blow, and a pool not yet bitten goes with it. After it lands the foe keeps its row's `stop`,
//!   her window to strike back.
//! - **Interrupts on foes.** A stun breaks an interruptible wind-up; a jolt (Spark, which takes
//!   only on a machine) breaks any; a slow (a chill) lengthens one by half, once; her swing's
//!   knockback breaks a small foe's. A broken blow costs the foe as if it had swung: its cooldown
//!   and its recovery. Which boss tells are interruptible is data (`interruptible: false`).
//! - **The hop** (`Command::Hop`): 30 energy, 1.5 m over ten ticks along the stick (else her
//!   facing), blows pass through her on ticks 1 to 7, 18 ticks to the next. It ends her recovery
//!   and her cast ([`interrupt_her_cast`] is the player side's seam).
//! - **Hitlag**, per unit, freezing only the attacker and the victim: her swing 4 ticks, a crit
//!   or a kill 6 (never past a tenth of her swing's cycle), a boss changing phase or falling 10
//!   (the boss alone). Never mid-hop, never stacked: the longest wins. A frozen foe's wind-up
//!   waits with it.
//! - **Knockback**, spread over four ticks with the walk's own collision, stepped at the flush:
//!   her swing pushes a small or middling foe 6 px, an Explosion pushes 10 px out from its burst,
//!   a heavy foe pushes her 4 px. Bosses, plated and rooted things are not pushed, and a push
//!   never carries a body onto a plate (plates read only who walks onto them), nor moves a prop.

use jane_core::angle::{along, bearing};
use jane_core::{Angle, Fx, SpellId, Tick, Vec2};
use jane_data::{Controller, SpellDef, SpellKind};
use serde::{Deserialize, Serialize};

use crate::combat::{is_enemy, metres_between, try_cast, try_cast_with};
use crate::ctx::Ctx;
use crate::event::{EventKind, SpellError};
use crate::ids::{GroundId, Seat, UnitId};
use crate::input::InputFrame;
use crate::state::Unit;
use crate::tuning::{
    GCD, HEAVY_BOUNDS_FX, HOP_EASE, HOP_ENERGY, HOP_EVERY, HOP_FX, HOP_IFRAMES, HOP_TICKS, KNOCK_BURST_PX,
    KNOCK_HEAVY_PX, KNOCK_SWING_PX, KNOCK_TICKS, LAG_BOSS, LAG_SWING, LAG_SWING_BIG, MOVE_DEADZONE, PLATED_RESIST,
    SMALL_BOUNDS_FX, WINDUP_HALF_ARC,
};
use crate::units::{def_of, face_vector, facing_angle, move_unit, spend_energy};

/// A unit's part of the fight's feel. All zero on a unit that is doing none of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feel {
    /// A foe's blow on its way.
    pub windup: Option<Windup>,
    /// Frozen by a blow until this tick (hitlag).
    pub lag_until: Tick,
    /// Being pushed.
    pub knock: Option<Knock>,
    /// Her hop, while it lasts.
    pub hop: Option<Hop>,
    /// She may hop again from this tick.
    pub hop_ready: Tick,
}

impl Feel {
    /// None of it (the `Default`).
    pub const NONE: Feel = Feel { windup: None, lag_until: Tick::ZERO, knock: None, hop: None, hop_ready: Tick::ZERO };
}

/// A foe's blow winding up: its `PendingCast`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Windup {
    pub spell: SpellId,
    pub began: Tick,
    /// The tick it lands on (later if a chill or a hitlag held it).
    pub lands: Tick,
    /// Whom it was for (a melee lands only on her).
    pub target: Option<UnitId>,
    /// Where it was aimed when it began: a bolt flies at it, a pool lies on it.
    pub point: Vec2,
    /// The line from the foe to `point` when it began; a melee's arc is about it.
    pub aim: Angle,
    pub interruptible: bool,
    /// A chill has lengthened it already.
    pub chilled: bool,
    /// A ground spell's pool, laid when it began; it goes if the blow is broken before it bites.
    pub pool: Option<GroundId>,
}

/// A push under way: `Fx` a tick, for `left` more ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knock {
    pub dx: i32,
    pub dy: i32,
    pub left: u8,
}

/// Her hop: when it was pressed (its first tick is the next) and which way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hop {
    pub began: Tick,
    pub dir: Angle,
}

/// How a body takes a push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    /// Under a metre across: a rat, a bat, a crow. Her swing breaks its wind-up.
    Small,
    Medium,
    /// A hauler, anything a metre and a quarter across: not pushed; it pushes her.
    Heavy,
    /// Bosses, plated and rooted things: not pushed at all.
    Unmoved,
}

pub fn build(u: &Unit) -> Build {
    let d = def_of(u);
    let rooted = d.walk.0 <= 0 && d.run.0 <= 0;
    let plated = d.resist_of(jane_core::action::School::Physical).0 >= PLATED_RESIST.0;
    if d.boss || rooted || plated {
        Build::Unmoved
    } else if d.bounds.0 >= HEAVY_BOUNDS_FX {
        Build::Heavy
    } else if d.bounds.0 < SMALL_BOUNDS_FX {
        Build::Small
    } else {
        Build::Medium
    }
}

/// Does a heavy body's blow push her? Haulers, and the big bosses.
fn pushes_her(u: &Unit) -> bool {
    def_of(u).bounds.0 >= HEAVY_BOUNDS_FX
}

// --- wind-ups -------------------------------------------------------------------------------------

/// How long a unit's blow with `spell` winds up before the table's delay: the row's `windup`, else
/// the unit's; a pool at least its own `delay`. 0 for anything but an AI's melee, bolt or pool.
pub fn windup_of(u: &Unit, spell: &SpellDef) -> Tick {
    if u.controller != Controller::Ai || !matches!(spell.kind, SpellKind::Melee | SpellKind::Bolt | SpellKind::Ground) {
        return Tick::ZERO;
    }
    let w = spell.windup.unwrap_or(def_of(u).windup);
    match spell.ground {
        Some(p) if w.0 > 0 || p.delay.0 > 1 => Tick(w.0.max(p.delay.0)),
        _ => w,
    }
}

/// The AI's cast (`ai::fight`): a blow with a wind-up begins it, anything else casts at once
/// through [`try_cast`]. The checks are the cast's own (range, sight, cooldown, cost), so an AI
/// that hears `TooFar` walks in as before.
pub fn cast_or_windup(cx: &mut Ctx<'_>, id: UnitId, spell: SpellId) -> Result<(), SpellError> {
    let def = cx.cat.combat.spell(spell);
    let Some(u) = cx.zone.unit(id) else { return Err(SpellError::CastUnsuccessful) };
    let ticks = windup_of(u, def);
    if ticks.0 == 0 {
        return try_cast(cx, id, spell, None, None);
    }
    check(cx, id, spell, def)?;
    begin(cx, id, spell, def, ticks)
}

/// [`try_cast_with`]'s checks for an AI casting at its target, without the cast.
fn check(cx: &Ctx<'_>, id: UnitId, spell: SpellId, def: &SpellDef) -> Result<(), SpellError> {
    let now = cx.world.tick;
    let c = cx.zone.unit(id).ok_or(SpellError::CastUnsuccessful)?;
    if !c.alive {
        return Err(SpellError::YouAreDead);
    }
    if crate::status::is_stunned(c, now) || crate::height::cannot_strike(&cx.rt.grid, c) {
        return Err(SpellError::CastUnsuccessful);
    }
    let target = c.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.alive);
    if def.needs_target {
        let Some(t) = target else { return Err(SpellError::NoTarget) };
        if def.needs_enemy != is_enemy(c.faction, t.faction) {
            return Err(SpellError::NotValidTarget);
        }
        if def.needs_los && !crate::los::sees(&cx.rt.grid, c, t) {
            return Err(SpellError::NotInLos);
        }
        if metres_between(c, t) > i64::from(def.range.0) {
            return Err(SpellError::TooFar);
        }
    }
    if c.cooldowns.iter().any(|&(s, until)| s == spell && until > now) {
        return Err(SpellError::OnCooldown);
    }
    if c.gcd_until > now && !def.gcd_immune {
        return Err(SpellError::OnGcd);
    }
    if c.mp < def.mp {
        return Err(SpellError::NotEnoughMp);
    }
    if c.energy < def.energy {
        return Err(SpellError::NotEnoughEnergy);
    }
    Ok(())
}

fn begin(cx: &mut Ctx<'_>, id: UnitId, spell: SpellId, def: &SpellDef, ticks: Tick) -> Result<(), SpellError> {
    let now = cx.world.tick;
    let lands = now.after(ticks).after(Tick(u32::from(cx.world.table_delay)));
    let u = cx.zone.unit(id).expect("checked");
    let target = u.target.filter(|&t| cx.zone.unit(t).is_some_and(|t| t.alive));
    let point = target
        .and_then(|t| cx.zone.unit(t))
        .map_or_else(|| u.pos + along(facing_angle(u.facing), Fx(def.range.0.max(jane_core::num::CELL_FX))), |t| t.pos);
    let aim = if point == u.pos { facing_angle(u.facing) } else { bearing(u.pos, point) };
    // A pool is laid now, on its point: it is the tell, seen until it bites, and the cast is paid.
    let pool = if def.kind == SpellKind::Ground {
        let before = cx.zone.grounds.len();
        try_cast(cx, id, spell, None, None)?;
        let g = cx.zone.grounds.get_mut(before).filter(|g| g.from == Some(id));
        g.map(|g| {
            hold_pool(g, lands);
            g.id
        })
    } else {
        None
    };
    let u = cx.zone.unit_mut(id).expect("checked");
    crate::ai::clear_path(u);
    face_vector(u, i64::from(point.x.0 - u.pos.x.0), i64::from(point.y.0 - u.pos.y.0));
    u.feel.windup = Some(Windup {
        spell,
        began: now,
        lands,
        target,
        point,
        aim,
        interruptible: def.interruptible,
        chilled: false,
        pool,
    });
    let interruptible = def.interruptible;
    cx.emit(EventKind::Windup { unit: id, spell, at: point, lands, interruptible });
    Ok(())
}

/// A foe winding up (`ai::fight`): it stands, faces its point, and on its tick the blow lands.
pub fn hold(cx: &mut Ctx<'_>, id: UnitId) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    let Some(w) = u.feel.windup else { return };
    if now < w.lands {
        if w.point != u.pos {
            face_vector(u, i64::from(w.point.x.0 - u.pos.x.0), i64::from(w.point.y.0 - u.pos.y.0));
        }
        return;
    }
    u.feel.windup = None;
    land(cx, id, w);
}

fn land(cx: &mut Ctx<'_>, id: UnitId, w: Windup) {
    let now = cx.world.tick;
    let def = cx.cat.combat.spell(w.spell);
    match def.kind {
        SpellKind::Melee => {
            let u = cx.zone.unit(id).expect("unit");
            let range = i64::from(def.range.0);
            let caught = w
                .target
                .and_then(|t| cx.zone.unit(t))
                .is_some_and(|t| t.alive && !t.hidden && metres_between(u, t) <= range && in_arc(u.pos, t.pos, w.aim));
            if caught {
                cx.zone.unit_mut(id).expect("unit").target = w.target;
                if try_cast(cx, id, w.spell, None, None).is_ok() {
                    return;
                }
            }
            // A swing at the air where she stood: its recovery all the same.
            let u = cx.zone.unit_mut(id).expect("unit");
            spend(u, w.spell, def, now);
            let (at, facing) = (u.pos, u.facing);
            cx.emit(EventKind::Swing { unit: id, at, facing });
        }
        SpellKind::Bolt => {
            // At the point it was aimed at, wherever she is now.
            let row = SpellDef { needs_target: false, ..*def };
            if try_cast_with(cx, id, w.spell, &row, Some(w.aim), None).is_err() {
                let u = cx.zone.unit_mut(id).expect("unit");
                spend(u, w.spell, def, now);
            }
        }
        _ => {
            // The pool was laid and paid for when it began: it bites now, and the foe recovers.
            let u = cx.zone.unit_mut(id).expect("unit");
            u.stop_until = u.stop_until.max(now.after(def.stop));
        }
    }
}

/// A pool laid by a wind-up bites when the wind-up lands: its first pulse, and its end with it,
/// move later to `lands` (a chill, a hitlag, the table's delay).
fn hold_pool(g: &mut crate::state::Ground, lands: Tick) {
    if g.next_pulse < lands {
        let by = lands.0 - g.next_pulse.0;
        g.next_pulse = lands;
        g.until = g.until.after(Tick(by));
    }
}

/// Is `at` inside the 90° arc about `aim` from `from`?
fn in_arc(from: Vec2, at: Vec2, aim: Angle) -> bool {
    if from == at {
        return true;
    }
    let d = i32::from(bearing(from, at).0.wrapping_sub(aim.0) as i16);
    d.abs() <= WINDUP_HALF_ARC
}

/// What a blow costs the foe whether it lands or not: the cooldown, the GCD, the recovery.
fn spend(u: &mut Unit, spell: SpellId, def: &SpellDef, now: Tick) {
    if def.cooldown.0 > 0 {
        u.cooldowns.retain(|&(s, until)| s != spell && until > now);
        u.cooldowns.push((spell, now.after(def.cooldown)));
    }
    if !def.gcd_immune {
        u.gcd_until = now.after(GCD);
    }
    u.stop_until = u.stop_until.max(now.after(def.stop));
}

/// Break a unit's wind-up: the blow is lost but costs as if swung, and a pool it laid that has
/// not bitten yet is taken up.
pub fn interrupt(cx: &mut Ctx<'_>, id: UnitId) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    let Some(w) = u.feel.windup.take() else { return };
    let def = jane_data::catalog().combat.spell(w.spell);
    spend(u, w.spell, def, now);
    let at = u.pos;
    if let Some(g) = w.pool {
        cx.zone.grounds.retain(|p| p.id != g || p.next_pulse <= now);
    }
    cx.emit(EventKind::Interrupted { unit: id, spell: w.spell, at });
}

/// Let go of a wind-up without a word (it leashed, it died): a pool not yet bitten goes too.
pub fn drop_windup(cx: &mut Ctx<'_>, id: UnitId) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    if let Some(g) = u.feel.windup.take().and_then(|w| w.pool) {
        cx.zone.grounds.retain(|p| p.id != g || p.next_pulse <= now);
    }
}

/// A status took on `id` (`status::apply_effect`): a stun breaks an interruptible wind-up, a jolt
/// (only a machine takes one) breaks any, a slow (a chill) lengthens it by half, once. On her, a
/// stun breaks her cast.
pub fn on_status(cx: &mut Ctx<'_>, id: UnitId, e: &jane_data::EffectDef) {
    let Some(u) = cx.zone.unit_mut(id) else { return };
    if u.controller == Controller::Player {
        if e.stun || e.speed.0 == 0 {
            interrupt_her_cast(cx, id);
        }
        return;
    }
    let Some(w) = u.feel.windup else { return };
    if e.stun || e.speed.0 == 0 {
        if w.interruptible || e.only_if_weak.is_some() {
            interrupt(cx, id);
        }
    } else if e.harmful && e.speed.0 < 1000 && !w.chilled {
        let w = u.feel.windup.as_mut().expect("winding up");
        w.lands = w.lands.after(Tick(w.lands.0.saturating_sub(w.began.0) / 2));
        w.chilled = true;
        let (pool, lands) = (w.pool, w.lands);
        if let Some(g) = pool.and_then(|g| cx.zone.grounds.iter_mut().find(|p| p.id == g)) {
            hold_pool(g, lands);
        }
    }
}

// --- her side: the hop, and the seam for her cast ---------------------------------------------------

/// Her own cast (the player side's `PendingCast`, `cast.rs`): a hop, a stun or a heavy foe's
/// push breaks it; the mana is kept and the GCD and cooldown are handed back.
pub fn interrupt_her_cast(cx: &mut Ctx<'_>, body: UnitId) {
    crate::cast::interrupt_unit(cx, body);
}

/// `Command::Hop`: along the stick, else her facing. Not while reading, stunned, carrying, short
/// of energy, or too soon after the last.
pub fn hop(cx: &mut Ctx<'_>, seat: Seat, frame: InputFrame) {
    let now = cx.world.tick;
    let Some(p) = cx.world.player(seat) else { return };
    if p.dialogue.is_some() {
        return;
    }
    let body = p.unit;
    let Some(u) = cx.zone.unit_mut(body) else { return };
    if !u.alive
        || u.carrying.is_some()
        || u.feel.hop.is_some()
        || crate::height::hopping(u)
        || now < u.feel.hop_ready
        || u.energy < HOP_ENERGY
        || crate::status::is_stunned(u, now)
    {
        return;
    }
    let dir = if frame.mv_mag > MOVE_DEADZONE { frame.mv_dir } else { facing_angle(u.facing) };
    spend_energy(u, HOP_ENERGY);
    u.feel.hop = Some(Hop { began: now, dir });
    u.feel.hop_ready = now.after(HOP_EVERY);
    // It ends her recovery and any hitlag on her: a hop is never frozen.
    u.stop_until = u.stop_until.min(now);
    u.feel.lag_until = u.feel.lag_until.min(now);
    u.feel.knock = None;
    let at = u.pos;
    interrupt_her_cast(cx, body);
    cx.emit(EventKind::Hop { unit: body, at, dir });
}

/// Step 6, before her stick: a hop under way carries her (its tick's share of its length, with the
/// walk's collision) and her stick waits. Whether it did.
pub fn hop_step(cx: &mut Ctx<'_>, ix: usize, now: Tick) -> bool {
    let u = &mut cx.zone.units[ix];
    let Some(h) = u.feel.hop else { return false };
    let k = now.0.saturating_sub(h.began.0);
    if k == 0 || k > HOP_TICKS {
        if k > HOP_TICKS {
            u.feel.hop = None;
        }
        return k == 0;
    }
    let d = along(h.dir, Fx(HOP_FX * HOP_EASE[(k - 1) as usize] / 100));
    move_unit(cx.rt, u, d.x, d.y);
    if k == HOP_TICKS {
        u.feel.hop = None;
    }
    true
}

/// Blows pass through her on her hop's ticks 1 to 7.
pub fn evading(u: &Unit, now: Tick) -> bool {
    u.feel.hop.is_some_and(|h| now.0 > h.began.0 && now.0 <= h.began.0 + HOP_IFRAMES)
}

/// Frozen by hitlag this tick.
pub fn lagged(u: &Unit, now: Tick) -> bool {
    u.feel.lag_until > now
}

// --- hitlag ---------------------------------------------------------------------------------------

/// Freeze `id` for `ticks` from now, unless she is mid-hop; the longest freeze wins (none
/// stacks). A wind-up under way waits as long.
pub fn lag(cx: &mut Ctx<'_>, id: UnitId, ticks: u32) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    if u.feel.hop.is_some() || ticks == 0 {
        return;
    }
    let until = now.after(Tick(ticks));
    let from = u.feel.lag_until.max(now);
    if until <= from {
        return;
    }
    let added = until.0 - from.0;
    u.feel.lag_until = until;
    if let Some(w) = u.feel.windup.as_mut() {
        w.lands = w.lands.after(Tick(added));
        let (pool, lands) = (w.pool, w.lands);
        if let Some(g) = pool.and_then(|g| cx.zone.grounds.iter_mut().find(|p| p.id == g)) {
            hold_pool(g, lands);
        }
    }
}

/// Her lag for a swing: never past a tenth of the swing's cycle (its cooldown).
fn swing_lag(spell: &SpellDef, big: bool) -> u32 {
    let lag = if big { LAG_SWING_BIG } else { LAG_SWING };
    lag.min((spell.cooldown.0 / 10).max(1))
}

/// A melee blow was dealt (`combat::cast_melee`), `crit` known, the kill not yet: her swing
/// freezes her and its victim and pushes it (a small foe's wind-up breaks); a heavy foe's blow
/// pushes her and breaks her cast.
pub fn melee_landed(cx: &mut Ctx<'_>, caster: UnitId, victim: UnitId, spell: &SpellDef, crit: bool) {
    let (Some(c), Some(v)) = (cx.zone.unit(caster), cx.zone.unit(victim)) else { return };
    let dir = if c.pos == v.pos { facing_angle(c.facing) } else { bearing(c.pos, v.pos) };
    if c.controller == Controller::Player {
        let lag_ticks = swing_lag(spell, crit);
        let victim_build = build(v);
        lag(cx, caster, lag_ticks);
        lag(cx, victim, lag_ticks);
        // Her crit shakes her screen, and only hers.
        if let Some(seat) = cx.party.seat_of(caster).filter(|_| crit) {
            crate::combat::emit_to(cx, seat, EventKind::Shake(1));
        }
        if matches!(victim_build, Build::Small | Build::Medium) {
            knock(cx, victim, dir, KNOCK_SWING_PX);
        }
        if victim_build == Build::Small
            && cx.zone.unit(victim).and_then(|v| v.feel.windup).is_some_and(|w| w.interruptible)
        {
            interrupt(cx, victim);
        }
    } else if c.controller == Controller::Ai && v.controller == Controller::Player && pushes_her(c) {
        knock(cx, victim, dir, KNOCK_HEAVY_PX);
        interrupt_her_cast(cx, victim);
    }
}

/// A unit fell to `killer` (`flush::kill_unit`): her killing swing freezes her the longer lag;
/// a boss's fall freezes it. What it was winding up goes with it.
pub fn on_death(cx: &mut Ctx<'_>, id: UnitId, killer: Option<UnitId>) {
    let now = cx.world.tick;
    drop_windup(cx, id);
    if let Some(u) = cx.zone.unit_mut(id) {
        // Felled mid-hop over a ledge, it comes down where it was going (MAP.md §2.4).
        crate::height::finish_hop(cx.rt, u);
        u.feel.knock = None;
        u.feel.hop = None;
        u.feel.lag_until = u.feel.lag_until.min(now);
        if def_of(u).boss {
            u.feel.lag_until = now.after(Tick(LAG_BOSS));
        }
    }
    // Her swing that landed this tick (her lag began now) and killed: the longer lag.
    let Some(k) = killer.filter(|&k| cx.party.seat_of(k).is_some()) else { return };
    let swinging = cx.zone.unit(k).is_some_and(|u| u.feel.lag_until > now && u.feel.lag_until.0 - now.0 <= LAG_SWING);
    if swinging {
        let melee = jane_data::catalog().combat.spell_id("melee_player");
        let ticks = melee.map_or(LAG_SWING_BIG, |m| swing_lag(jane_data::catalog().combat.spell(m), true));
        lag(cx, k, ticks);
        // Her felling blow shakes her screen, and only hers.
        if let Some(seat) = cx.party.seat_of(k) {
            crate::combat::emit_to(cx, seat, EventKind::Shake(2));
        }
    }
}

/// A boss crossed into its next phase (`flush::enter_phases`): it alone freezes.
pub fn phase_changed(cx: &mut Ctx<'_>, id: UnitId) {
    lag(cx, id, LAG_BOSS);
}

// --- knockback ------------------------------------------------------------------------------------

/// Push `id` `px` along `dir`, over [`KNOCK_TICKS`] (it does not stack: a new push replaces one
/// under way). Who may be pushed is the caller's rule ([`build`]).
pub fn knock(cx: &mut Ctx<'_>, id: UnitId, dir: Angle, px: i32) {
    let Some(u) = cx.zone.unit_mut(id) else { return };
    // A body hopping a ledge is in the air: no push takes it (MAP.md §2.4).
    if !u.alive || u.feel.hop.is_some() || crate::height::hopping(u) {
        return;
    }
    let total = along(dir, Fx::from_px(px));
    let n = i32::from(KNOCK_TICKS);
    u.feel.knock = Some(Knock { dx: total.x.0 / n, dy: total.y.0 / n, left: KNOCK_TICKS });
}

/// A bolt burst (`flight::step_projectiles`): an Explosion (a blast) of hers pushes everyone of the
/// other side it caught out from where it burst.
pub fn burst(cx: &mut Ctx<'_>, spell: &SpellDef, at: Vec2, from: Option<UnitId>, caught: &[UnitId]) {
    if spell.school != jane_core::action::School::Blast || from.is_none_or(|f| cx.party.seat_of(f).is_none()) {
        return;
    }
    for &id in caught {
        let Some(u) = cx.zone.unit(id) else { continue };
        if !matches!(build(u), Build::Small | Build::Medium) {
            continue;
        }
        let dir = if u.pos == at { facing_angle(u.facing) } else { bearing(at, u.pos) };
        knock(cx, id, dir, KNOCK_BURST_PX);
    }
}

/// Step 10, after the blows: every push under way moves its body a tick's share (a frozen body
/// waits), by the walk's collision, and never onto a plate it was not already on.
pub fn step_knocks(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    for i in 0..cx.rt.awake_units.len() {
        let id = cx.rt.awake_units[i];
        let Some(ix) = cx.zone.unit_ix(id) else { continue };
        let u = &cx.zone.units[ix];
        let Some(k) = u.feel.knock else { continue };
        // A ledge's hop (MAP.md §2.4): carried over the face whatever is under it, never frozen.
        if k.left & crate::tuning::LEDGE_HOP_BIT != 0 {
            crate::height::step_hop(cx.rt, &mut cx.zone.units[ix], k);
            continue;
        }
        if !u.alive {
            cx.zone.units[ix].feel.knock = None;
            continue;
        }
        if lagged(u, now) {
            continue;
        }
        let to = Vec2 { x: Fx(u.pos.x.0 + k.dx), y: Fx(u.pos.y.0 + k.dy) };
        let onto_plate = on_plate(cx, to) && !on_plate(cx, u.pos);
        let u = &mut cx.zone.units[ix];
        if onto_plate {
            u.feel.knock = None;
            continue;
        }
        move_unit(cx.rt, u, Fx(k.dx), Fx(k.dy));
        // Pushed off a ledge the way it is hopped, it falls over it: the hop is its push now.
        if !crate::height::hopping(u) {
            u.feel.knock = (k.left > 1).then_some(Knock { left: k.left - 1, ..k });
        }
    }
}

/// Does `at` lie on any plate's footprint here?
fn on_plate(cx: &Ctx<'_>, at: Vec2) -> bool {
    let (x, y) = at.cell();
    let cat = cx.cat;
    cx.rt.plates.iter().any(|&ix| {
        let p = &cx.zone.props[ix as usize];
        crate::interact::footprint(cat.story.prop(p.def), p).contains(x, y)
    })
}
