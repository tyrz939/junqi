//! A seat's side of a fight (PLAY-PLAN §2.1): casts with weight, the queue, auto-attack.
//!
//! **A press** ([`press`]) resolves what it is cast at ([`chosen`]): her hard target if the spell
//! fits it (a foe for a bolt that hurts, a prop that answers the bolt's school or the verb, a
//! friend for a friendly spell), else the foe under the cursor, else (no aim at all, a pad at
//! rest) the nearest foe in front; with the free-aim key held, along the aim. A target needs
//! sight to start and range at start and at release; out of range, a hard target walks her to
//! the edge of range first (`walk.rs`, League's rule).
//!
//! **A cast with a cast time** checks everything when it begins, starts the 1 s GCD, and lands
//! [`crate::state::PendingCast::done`] later through the same pipeline (`combat::land_checked`),
//! paying then. She walks at half speed meanwhile, never rooted. A stun, a heavy blow
//! ([`interrupt_unit`], the knockback's to call), Esc or its target gone stops it: nothing is
//! spent and the GCD is handed back. Plain damage does not.
//!
//! **The queue**: a press that comes while she is building a cast or on the GCD, within
//! [`QUEUE_TICKS`] of her being free, waits for that tick.
//!
//! **Auto-attack**: a swing pressed at a foe in reach (her target, the one under the cursor, or
//! the nearest in front), or a right-click on one, engages it. She faces it and swings on the
//! swing's own timer while she walks; every swing that lands hands back energy (the row's
//! `restoreEnergy`: the generator). It pauses while she casts and stops when the foe dies or
//! leaves her reach.

use jane_core::num::dist_sq;
use jane_core::tile::BLOCK_SHOT;
use jane_core::{Angle, SpellId, Tick};
use jane_data::{Answers, SpellDef, SpellKind};

use crate::combat::{Checked, Landing, book_of, bounds, check_cast, land_checked, metres_between, say_failed};
use crate::ctx::Ctx;
use crate::event::{EventKind, SpellError};
use crate::ids::{Seat, UnitId};
use crate::input::{InputFrame, TargetRef};
use crate::interact::{footprint, in_verb_reach};
use crate::los::{first_blocked_cell, line_of_sight};
use crate::state::{PendingCast, QueuedCast, Seek, Unit, WalkThen};
use crate::target::{hostile, pos_of, soft_target, valid};
use crate::tuning::{AUTO_SLACK_FX, MOVE_DEADZONE, PLAYER_GCD, QUEUE_TICKS, SCHOOL_TOUCH_FX};
use crate::units::{face_angle, face_vector, facing_angle};

/// Held still: stunned, or rooted to nothing by a status (the bell's "stunned", a web). She
/// cannot begin a cast held still, and being held stops the one building.
pub fn held_still(u: &Unit, now: Tick) -> bool {
    crate::status::is_stunned(u, now) || crate::status::speed_factor(u, now) == 0
}

/// Her swing: the first melee spell in her book.
pub fn swing_of(u: &Unit) -> Option<SpellId> {
    let cat = jane_data::catalog();
    book_of(u).iter().copied().find(|&s| cat.combat.spell(s).kind == SpellKind::Melee)
}

/// Her swing's reach, between bodies.
pub fn swing_reach(u: &Unit) -> Option<i64> {
    swing_of(u).map(|s| i64::from(jane_data::catalog().combat.spell(s).range.0))
}

/// Does this spell hurt what it is cast at?
fn hurts(def: &SpellDef) -> bool {
    crate::assist::assists(def) && def.needs_enemy
}

/// Does the prop answer this spell: its school for a bolt, its verb for a world spell?
fn prop_answers(cx: &Ctx<'_>, def: &SpellDef, id: crate::ids::PropId) -> bool {
    let Some(p) = cx.zone.prop_ix(id).and_then(|ix| cx.zone.props.get(ix as usize)) else { return false };
    let a = cx.cat.story.prop(p.def).answers;
    match def.kind {
        SpellKind::Bolt => a.and_then(Answers::school) == Some(def.school),
        SpellKind::World => matches!(
            (a, def.world),
            (Some(Answers::Repair), Some(jane_data::WorldSpell::Repair))
                | (Some(Answers::Grow), Some(jane_data::WorldSpell::Grow))
        ),
        _ => false,
    }
}

/// What a press is cast at, and whether that is her hard target (only a hard target walks her
/// to range). `on` may be changed: a friendly spell lands on her friendly target.
fn chosen(
    cx: &Ctx<'_>,
    u: &Unit,
    def: &SpellDef,
    on: &mut Option<UnitId>,
    frame: &InputFrame,
) -> (Option<TargetRef>, bool) {
    if frame.free || matches!(def.kind, SpellKind::Ground | SpellKind::OnSelf) {
        return (None, false);
    }
    let hard = frame.target.filter(|&t| valid(cx.zone, u, t));
    match (def.kind, hard) {
        (SpellKind::Bolt | SpellKind::Melee, Some(t @ TargetRef::Unit(_))) if hurts(def) && hostile(cx.zone, u, t) => {
            return (Some(t), true);
        }
        (SpellKind::Bolt | SpellKind::World, Some(t @ TargetRef::Prop(p))) if prop_answers(cx, def, p) => {
            return (Some(t), true);
        }
        (SpellKind::Ally, Some(TargetRef::Unit(f))) if cx.party.seat_of(f).is_some() => {
            *on = Some(f);
            return (None, false);
        }
        _ => {}
    }
    if !hurts(def) || def.kind != SpellKind::Bolt {
        return (None, false);
    }
    // The foe under the cursor (a mouseover cast), else with no aim at all the nearest in front.
    if let Some(o) = on.filter(|&o| o != u.id && valid(cx.zone, u, TargetRef::Unit(o))) {
        if hostile(cx.zone, u, TargetRef::Unit(o)) {
            return (Some(TargetRef::Unit(o)), false);
        }
    }
    if frame.aim.is_none() {
        return (soft_target(cx.zone, cx.rt, u, facing_angle(u.facing)).map(TargetRef::Unit), false);
    }
    (None, false)
}

/// Sight and range to a target for `def` from `u`: a unit between bodies (a bolt flies its range
/// plus her body), a prop's middle (a bolt's range plus its touch; a verb's reach). A prop's own
/// cells do not block the sight to it.
pub fn reach_check(cx: &Ctx<'_>, u: &Unit, def: &SpellDef, t: TargetRef) -> Result<(), SpellError> {
    match t {
        TargetRef::Unit(id) => {
            let Some(o) = cx.zone.unit(id) else { return Err(SpellError::NoTarget) };
            if def.needs_los && !line_of_sight(&cx.rt.grid, u.pos, o.pos) {
                return Err(SpellError::NotInLos);
            }
            let slack = if def.kind == SpellKind::Bolt { bounds(u) } else { 0 };
            if metres_between(u, o) > i64::from(def.range.0) + slack {
                return Err(SpellError::TooFar);
            }
            Ok(())
        }
        TargetRef::Prop(id) => {
            let Some(ix) = cx.zone.prop_ix(id) else { return Err(SpellError::NoTarget) };
            if def.kind == SpellKind::World {
                return if in_verb_reach(cx.zone, ix, u.pos) { Ok(()) } else { Err(SpellError::TooFar) };
            }
            let p = &cx.zone.props[ix as usize];
            let at = pos_of(cx.zone, t).ok_or(SpellError::NoTarget)?;
            // A shot stopped short still touches what is within its touch of where it stopped
            // (`flight.rs`): sight to a prop is sight to a cell of it, or near enough.
            let touch = i64::from(def.touch.unwrap_or(SCHOOL_TOUCH_FX).0) + i64::from(jane_core::num::CELL_FX);
            if let Some((x, y)) = first_blocked_cell(&cx.rt.grid, u.pos, at, BLOCK_SHOT) {
                let near = dist_sq(jane_core::Vec2::centre(x, y), at) <= touch * touch;
                if !near && !footprint(cx.cat.story.prop(p.def), p).contains(x, y) {
                    return Err(SpellError::NotInLos);
                }
            }
            // As far as a bolt goes: born past her chest, flying its range and two of her bodies
            // (and the last step it takes past that), and touching what is within its touch of
            // where it ends.
            let reach = i64::from(crate::tuning::BOLT_START_FX.0)
                + i64::from(def.range.0)
                + 2 * bounds(u)
                + i64::from(def.speed.map_or(0, |s| s.0))
                + i64::from(def.touch.unwrap_or(SCHOOL_TOUCH_FX).0);
            if dist_sq(u.pos, at) > reach * reach {
                return Err(SpellError::TooFar);
            }
            Ok(())
        }
    }
}

/// The aim a cast flies along: at its target, else the frame's raw aim through assist.
fn aim_for(
    cx: &mut Ctx<'_>,
    seat: Seat,
    u_pos: jane_core::Vec2,
    spell: SpellId,
    at: Option<TargetRef>,
    raw: Option<Angle>,
    assist: crate::input::AssistProfile,
) -> Option<Angle> {
    match at.and_then(|t| pos_of(cx.zone, t)) {
        Some(p) if p != u_pos => Some(jane_core::angle::bearing(u_pos, p)),
        Some(_) => raw,
        None => raw.map(|r| crate::assist::assisted_aim(cx, seat, spell, r, assist)),
    }
}

/// How a cast at `at` lands.
fn landing(cx: &Ctx<'_>, def: &SpellDef, at: Option<TargetRef>, started: bool) -> Landing {
    let mut l = Landing { started, ..Landing::default() };
    match (def.kind, at) {
        (SpellKind::Bolt, Some(TargetRef::Unit(u))) => l.seek = Seek::Unit(u),
        (SpellKind::Bolt, Some(t @ TargetRef::Prop(_))) => l.seek = pos_of(cx.zone, t).map_or(Seek::None, Seek::Point),
        (SpellKind::World, Some(TargetRef::Prop(p))) => l.prop = cx.zone.prop_ix(p),
        _ => {}
    }
    l
}

/// When she is free for `def`: her cast landed and, unless it is off the GCD, her GCD over.
fn free_at(cx: &Ctx<'_>, seat: Seat, u: &Unit, def: &SpellDef) -> Tick {
    let f = &cx.world.players[seat.index()].fight;
    let mut t = f.cast.map_or(Tick(0), |c| c.done);
    if !def.gcd_immune {
        t = t.max(u.gcd_until);
    }
    t
}

/// A bar press or `Command::Cast` for a spell she knows (`combat::player_cast` checked).
pub fn press(cx: &mut Ctx<'_>, seat: Seat, spell: SpellId, on: Option<UnitId>, frame: InputFrame) {
    let now = cx.world.tick;
    let def = cx.cat.combat.spell(spell);
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body).filter(|u| u.alive) else { return };
    if def.kind == SpellKind::Melee {
        attack(cx, seat, spell, on, frame);
        return;
    }
    let free = free_at(cx, seat, u, def);
    if free > now {
        if free.0 - now.0 <= QUEUE_TICKS {
            cx.world.players[seat.index()].fight.queued =
                Some(QueuedCast { spell, on, until: free.after(Tick(QUEUE_TICKS)) });
        } else {
            say_failed(cx, body, spell, SpellError::OnGcd);
        }
        return;
    }
    begin(cx, seat, spell, on, frame, true);
}

/// Begin a cast now: at what [`chosen`] says, its sight and range, every check; then build it
/// (a cast time) or land it (an instant). Out of range of a hard target, with `may_walk`, she
/// walks to range and it begins there.
pub fn begin(cx: &mut Ctx<'_>, seat: Seat, spell: SpellId, mut on: Option<UnitId>, frame: InputFrame, may_walk: bool) {
    let now = cx.world.tick;
    let def = cx.cat.combat.spell(spell);
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body).filter(|u| u.alive) else { return };
    if crate::status::is_stunned(u, now) {
        say_failed(cx, body, spell, SpellError::CastUnsuccessful);
        return;
    }
    let (at, hard) = chosen(cx, u, def, &mut on, &frame);
    if let Some(t) = at {
        match reach_check(cx, u, def, t) {
            Ok(()) => {}
            Err(SpellError::TooFar) if hard && may_walk => {
                if let Some(to) = pos_of(cx.zone, t) {
                    crate::walk::start(cx, seat, to, WalkThen::Cast { spell, on });
                }
                return;
            }
            Err(why) => {
                say_failed(cx, body, spell, why);
                return;
            }
        }
    }
    let upos = u.pos;
    let aim = aim_for(cx, seat, upos, spell, at, frame.aim, frame.assist);
    let checked = match check_cast(cx, body, spell, def, aim, on) {
        Ok(c) => c,
        Err(why) => {
            say_failed(cx, body, spell, why);
            return;
        }
    };
    if def.cast.0 == 0 {
        let l = landing(cx, def, at, false);
        if let Err(why) = land_checked(cx, body, spell, def, aim, checked, l) {
            say_failed(cx, body, spell, why);
        }
        return;
    }
    let ix = cx.zone.unit_ix(body).expect("her body");
    let u = &mut cx.zone.units[ix];
    if !def.gcd_immune {
        u.gcd_until = now.after(PLAYER_GCD);
    }
    // Its own cooldown runs from when it begins, so a cast time is not paid twice (Icebolt's
    // 1 s cast inside its 2 s cooldown, not before it).
    if def.cooldown.0 > 0 {
        u.cooldowns.retain(|&(s, until)| s != spell && until > now);
        u.cooldowns.push((spell, now.after(def.cooldown)));
    }
    if let Some(a) = aim {
        face_angle(u, a);
    }
    let done = now.after(def.cast);
    // The unit assist settled on stays sticky through the cast, so the aim read at release
    // bends as the reticle showed while it built.
    if let Some(a) = cx.world.players[seat.index()].assist.as_mut() {
        if a.until >= now {
            a.until = a.until.max(done);
        }
    }
    let f = &mut cx.world.players[seat.index()].fight;
    f.queued = None;
    f.cast = Some(PendingCast {
        spell,
        on: checked.friend.or(on),
        at,
        aim: frame.aim,
        assist: frame.assist,
        started: now,
        done,
    });
    cx.emit(EventKind::CastBegin { unit: body, spell, done });
}

/// A cast built to its end lands: its target still there and in range, its aim at release (the
/// frame's, else the one it began with), its costs checked again, then the pipeline.
fn release(cx: &mut Ctx<'_>, seat: Seat, frame: InputFrame) {
    let Some(pc) = cx.world.players[seat.index()].fight.cast.take() else { return };
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body).filter(|u| u.alive) else { return };
    let def = cx.cat.combat.spell(pc.spell);
    if let Some(t) = pc.at {
        let why = if valid(cx.zone, u, t) {
            match reach_check(cx, u, def, t) {
                Err(SpellError::TooFar) => Some(SpellError::TooFar),
                _ => None,
            }
        } else {
            Some(SpellError::NoTarget)
        };
        if let Some(why) = why {
            stopped(cx, seat, body, pc.spell);
            say_failed(cx, body, pc.spell, why);
            return;
        }
    }
    let upos = u.pos;
    // The frame's aim with the frame's assist, else the aim and assist it began with.
    let (raw, assist) = match frame.aim {
        Some(a) => (Some(a), frame.assist),
        None => (pc.aim, pc.assist),
    };
    let aim = aim_for(cx, seat, upos, pc.spell, pc.at, raw, assist);
    // The GCD and the cooldown it started are its own: the checks at release do not wait on
    // them. The cooldown is put back if it lands (and stays handed back if it does not).
    let mut row = *def;
    row.gcd_immune = true;
    let ix = cx.zone.unit_ix(body).expect("her body");
    let own = cx.zone.units[ix].cooldowns.iter().position(|&(s, _)| s == pc.spell);
    let own = own.map(|i| cx.zone.units[ix].cooldowns.remove(i));
    let checked: Checked = match check_cast(cx, body, pc.spell, &row, aim, pc.on) {
        Ok(c) => c,
        Err(why) => {
            stopped(cx, seat, body, pc.spell);
            say_failed(cx, body, pc.spell, why);
            return;
        }
    };
    let l = landing(cx, def, pc.at, true);
    match land_checked(cx, body, pc.spell, def, aim, checked, l) {
        Ok(()) => {
            if let (Some(c), Some(u)) = (own, cx.zone.unit_mut(body)) {
                u.cooldowns.push(c);
            }
        }
        Err(why) => say_failed(cx, body, pc.spell, why),
    }
}

/// A cast that will not land: the GCD and the cooldown it started are handed back (it was off
/// cooldown when it began, so its spell's entry is this cast's) and it is said.
fn stopped(cx: &mut Ctx<'_>, seat: Seat, body: UnitId, spell: SpellId) {
    let now = cx.world.tick;
    if let Some(u) = cx.zone.unit_mut(body) {
        u.gcd_until = u.gcd_until.min(now);
        u.cooldowns.retain(|&(s, _)| s != spell);
    }
    cx.world.players[seat.index()].fight.queued = None;
    cx.emit(EventKind::CastStopped { unit: body, spell });
}

/// Stop the seat's building cast, if any: nothing spent.
pub fn interrupt(cx: &mut Ctx<'_>, seat: Seat) {
    if let Some(pc) = cx.world.players[seat.index()].fight.cast.take() {
        let body = cx.world.players[seat.index()].unit;
        stopped(cx, seat, body, pc.spell);
    }
}

/// A heavy blow or a stun landed on `unit`: if it is a seat's body building a cast, the cast
/// stops (PLAY-PLAN §2.1). For the knockback and the hop to call.
pub fn interrupt_unit(cx: &mut Ctx<'_>, unit: UnitId) {
    if let Some(seat) = cx.party.seat_of(unit) {
        interrupt(cx, seat);
    }
}

/// `Command::Halt` (Esc): stop casting, swinging and walking.
pub fn halt(cx: &mut Ctx<'_>, seat: Seat) {
    interrupt(cx, seat);
    let f = &mut cx.world.players[seat.index()].fight;
    f.auto = None;
    f.walk = None;
    f.queued = None;
}

/// A swing pressed: at her hostile target, the foe under the cursor, or the nearest in front. In
/// reach it engages auto-attack (and swings now if the timer allows); a hard target out of
/// reach walks her into it; with nobody, a swing at the air as before.
fn attack(cx: &mut Ctx<'_>, seat: Seat, spell: SpellId, on: Option<UnitId>, frame: InputFrame) {
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body) else { return };
    let hard = frame.target.filter(|&t| valid(cx.zone, u, t) && hostile(cx.zone, u, t));
    let under = on.map(TargetRef::Unit).filter(|&t| valid(cx.zone, u, t) && hostile(cx.zone, u, t));
    let facing = frame.aim.unwrap_or_else(|| facing_angle(u.facing));
    let t = match hard.or(under) {
        Some(TargetRef::Unit(t)) => Some(t),
        _ => soft_target(cx.zone, cx.rt, u, facing),
    };
    let reach = i64::from(cx.cat.combat.spell(spell).range.0);
    if let Some(t) = t {
        let o = cx.zone.unit(t).expect("a valid target");
        if metres_between(u, o) <= reach {
            cx.world.players[seat.index()].fight.auto = Some(t);
            swing_at(cx, seat, t);
            return;
        }
        if hard.is_some() {
            let to = o.pos;
            crate::walk::start(cx, seat, to, WalkThen::Attack(t));
            return;
        }
    }
    let aim = frame.aim.map(|raw| crate::assist::assisted_aim(cx, seat, spell, raw, frame.assist));
    if let Err(why) = crate::combat::try_cast(cx, body, spell, aim, on) {
        say_failed(cx, body, spell, why);
    }
}

/// One swing at `t` if the swing's timer allows and no cast is building; quiet otherwise.
fn swing_at(cx: &mut Ctx<'_>, seat: Seat, t: UnitId) {
    let now = cx.world.tick;
    if cx.world.players[seat.index()].fight.cast.is_some() {
        return;
    }
    let body = cx.world.players[seat.index()].unit;
    let Some(ix) = cx.zone.unit_ix(body) else { return };
    let Some(swing) = swing_of(&cx.zone.units[ix]) else { return };
    if cx.zone.units[ix].cooldowns.iter().any(|&(s, until)| s == swing && until > now) {
        return;
    }
    let tpos = cx.zone.unit(t).map(|o| o.pos);
    let u = &mut cx.zone.units[ix];
    u.target = Some(t);
    if let Some(p) = tpos {
        face_vector(u, i64::from(p.x.0 - u.pos.x.0), i64::from(p.y.0 - u.pos.y.0));
    }
    let _ = crate::combat::try_cast(cx, body, swing, None, None);
}

/// Step 6, before she moves: her target validated, a stun stops her cast, a held move ends her
/// click-walk, a cast built to its end lands, and a queued press fires if she is free.
pub fn before_move(cx: &mut Ctx<'_>, seat: Seat, frame: InputFrame) {
    let now = cx.world.tick;
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body) else { return };
    let target = frame.target.filter(|&t| valid(cx.zone, u, t));
    let switch_to = match target {
        Some(t @ TargetRef::Unit(id)) if hostile(cx.zone, u, t) => Some(id),
        _ => None,
    };
    let held = held_still(u, now);
    let stunned = crate::status::is_stunned(u, now);
    let f = &mut cx.world.players[seat.index()].fight;
    f.target = target;
    // Caught as it lands (a hold that was not on her at her last step): the cast stops once.
    let caught = held && !f.held;
    f.held = held;
    // Auto-attack follows her to a new foe.
    if let (Some(a), Some(n)) = (f.auto, switch_to) {
        if a != n {
            f.auto = Some(n);
        }
    }
    if frame.mv_mag > MOVE_DEADZONE {
        f.walk = None;
    }
    if caught || stunned {
        interrupt(cx, seat);
    }
    if stunned {
        return;
    }
    if cx.world.players[seat.index()].fight.cast.is_some_and(|c| now >= c.done) {
        release(cx, seat, frame);
    }
    if let Some(q) = cx.world.players[seat.index()].fight.queued {
        if now > q.until {
            cx.world.players[seat.index()].fight.queued = None;
        } else if let Some(u) = cx.zone.unit(body) {
            let def = cx.cat.combat.spell(q.spell);
            if free_at(cx, seat, u, def) <= now {
                cx.world.players[seat.index()].fight.queued = None;
                begin(cx, seat, q.spell, q.on, frame, true);
            }
        }
    }
}

/// Step 6, after she moves: auto-attack. She faces her foe and swings when the timer allows;
/// it pauses while she casts and stops when the foe is gone or out of reach (unless a
/// click-walk is closing on it).
pub fn after_move(cx: &mut Ctx<'_>, seat: Seat, frame: InputFrame) {
    let Some(t) = cx.world.players[seat.index()].fight.auto else { return };
    let body = cx.world.players[seat.index()].unit;
    let Some(u) = cx.zone.unit(body) else { return };
    let ok = valid(cx.zone, u, TargetRef::Unit(t)) && hostile(cx.zone, u, TargetRef::Unit(t));
    let reach = swing_reach(u);
    let gap = cx.zone.unit(t).map(|o| metres_between(u, o));
    let in_reach = ok && reach.zip(gap).is_some_and(|(r, g)| g <= r);
    // Knocked back a step by her own blow it is still hers: "leaving her reach" is going
    // further than a knock and a cell (`feel.rs` pushes a swing's victim 6 px).
    let near = ok && reach.zip(gap).is_some_and(|(r, g)| g <= r + AUTO_SLACK_FX);
    let closing = cx.world.players[seat.index()].fight.walk.as_ref().is_some_and(|w| w.then == WalkThen::Attack(t));
    if !ok || (!near && !closing) {
        cx.world.players[seat.index()].fight.auto = None;
        return;
    }
    // Just out of reach, with it her target and nothing held: she steps back in (a click's
    // walk to it again), as a player holding the attack does.
    if !in_reach
        && !closing
        && frame.mv_mag <= MOVE_DEADZONE
        && frame.target == Some(TargetRef::Unit(t))
        && cx.world.players[seat.index()].fight.cast.is_none()
    {
        if let Some(to) = cx.zone.unit(t).map(|o| o.pos) {
            crate::walk::start(cx, seat, to, WalkThen::Attack(t));
        }
    }
    if in_reach {
        let tpos = cx.zone.unit(t).map(|o| o.pos);
        if let (Some(p), Some(u)) = (tpos, cx.zone.unit_mut(body)) {
            face_vector(u, i64::from(p.x.0 - u.pos.x.0), i64::from(p.y.0 - u.pos.y.0));
        }
        swing_at(cx, seat, t);
    }
}
