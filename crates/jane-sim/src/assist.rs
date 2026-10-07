//! Aim assist (ARCHITECTURE.md §5.4). It lives in the sim so the pad and the mouse play the same
//! game and a replay lands the same bolts: the frame carries the raw aim and the profile, and
//! the cast resolves them here, inside the commands step.
//!
//! Candidates are hostile units (to the caster's side; never a friend, a prop or a corpse, and
//! never someone the party does not fight), alive, present, within the spell's reach and inside
//! the cone about the raw aim; the sticky unit (`PlayerState.assist`) stays a candidate for
//! `slack` more while it lasts. The best is the smallest turn from the raw aim, less a bonus for
//! the caster's own target and a smaller one for the sticky unit, ties to the lower id. Within
//! `snap` the cast takes its bearing; otherwise the raw aim moves `magnet` permille of the way
//! toward it (floored). With none, the raw aim. The chosen unit becomes the sticky one.
//!
//! Only spells that hurt what they are aimed at are assisted: bolts and melee. Heals, self,
//! ally, world and ground spells cast along the raw aim (a heal keeps the cursor rule).
//!
//! **A bolt aimed at a prop stays on it.** Icebolt, Explosion, Spark and Fireball also switch the
//! props that answer their school where the bolt ends (`flight.rs`, `school_touch`). When the raw
//! aim passes within the spell's `touch` of such a prop (shown, not yet on) nearer the caster
//! than the best candidate, the bolt flies raw: a torch beside a lurker is lit, not missed.

use alloc::vec::Vec;

use jane_core::angle::{cos_q15, iatan2, sin_q15};
use jane_core::num::mul_div_floor;
use jane_core::{Angle, Fx, SpellId, Tick, Vec2};
use jane_data::{Answers, Controller, SpellDef, SpellKind};

use crate::combat::{bounds, distance, is_enemy, max_bounds, query_near};
use crate::ctx::Ctx;
use crate::ids::{PropIx, Seat, UnitId};
use crate::input::AssistProfile;
use crate::light::prop_centre;
use crate::runtime::ZoneRuntime;
use crate::state::{Assisted, Unit, ZoneState};
use crate::tuning::{ASSIST_MOUSE, ASSIST_PAD, ASSIST_STICKY_BONUS, ASSIST_TARGET_BONUS, Assist, SCHOOL_TOUCH_FX};

/// A profile's numbers; `Off` has none.
pub const fn profile(p: AssistProfile) -> Option<Assist> {
    match p {
        AssistProfile::Off => None,
        AssistProfile::Pad => Some(ASSIST_PAD),
        AssistProfile::Mouse => Some(ASSIST_MOUSE),
    }
}

/// Is this spell aimed at someone it hurts?
pub fn assists(spell: &SpellDef) -> bool {
    matches!(spell.kind, SpellKind::Bolt | SpellKind::Melee) && spell.school != jane_core::action::School::Heal
}

/// How far from the caster (between bodies) a candidate may stand: a melee's reach, a bolt's
/// flight (it starts at the caster and flies `range` plus two of her bodies).
fn reach(spell: &SpellDef, caster: &Unit) -> i64 {
    i64::from(spell.range.0) + if spell.kind == SpellKind::Bolt { bounds(caster) } else { 0 }
}

/// How far along `raw` from the caster the nearest prop lies that a bolt of `spell` flown raw
/// would switch: shown, not on, answering the spell's school, its centre within the spell's
/// `touch` of the line and no further along it than the bolt flies plus `touch`. `Fx` along the
/// line; `None` when there is none, or the spell is not a bolt of a school a prop answers.
fn prop_on_line(
    zone: &ZoneState,
    rt: &ZoneRuntime,
    caster: &Unit,
    spell: &SpellDef,
    raw: Angle,
    reach: i64,
    props: &mut Vec<PropIx>,
) -> Option<i64> {
    if spell.kind != SpellKind::Bolt {
        return None;
    }
    let cat = jane_data::catalog();
    let touch = i64::from(spell.touch.unwrap_or(SCHOOL_TOUCH_FX).0);
    let (c, s) = (i64::from(cos_q15(raw).0), i64::from(sin_q15(raw).0));
    let far = reach + touch;
    let end = caster.pos + Vec2::new(Fx(((far * c) >> 15) as i32), Fx(((far * s) >> 15) as i32));
    let pad = Fx(touch as i32).cell() + 1;
    let (x0, x1) = (caster.pos.x.cell().min(end.x.cell()) - pad, caster.pos.x.cell().max(end.x.cell()) + pad);
    let (y0, y1) = (caster.pos.y.cell().min(end.y.cell()) - pad, caster.pos.y.cell().max(end.y.cell()) + pad);
    rt.props.query(x0, y0, x1, y1, props);
    let mut best: Option<i64> = None;
    for &ix in props.iter() {
        let Some(p) = zone.props.get(ix as usize) else { continue };
        let def = cat.story.prop(p.def);
        if p.hidden || p.on || def.answers.and_then(Answers::school) != Some(spell.school) {
            continue;
        }
        let at = prop_centre(def, p);
        let (vx, vy) = (i64::from(at.x.0 - caster.pos.x.0), i64::from(at.y.0 - caster.pos.y.0));
        let along = (vx * c + vy * s) >> 15;
        let off = ((vx * s - vy * c) >> 15).abs();
        if along < 0 || along > far || off > touch {
            continue;
        }
        best = Some(best.map_or(along, |b| b.min(along)));
    }
    best
}

/// The assisted angle for a cast from `caster` along `raw`, and the unit it chose. Pure: reads
/// the zone, the runtime and the sticky unit; `near` and `props` are reused buffers.
#[allow(clippy::too_many_arguments)]
pub fn pick(
    zone: &ZoneState,
    rt: &ZoneRuntime,
    now: Tick,
    caster: &Unit,
    sticky: Option<Assisted>,
    spell: &SpellDef,
    raw: Angle,
    p: AssistProfile,
    near: &mut Vec<UnitId>,
    props: &mut Vec<PropIx>,
) -> (Angle, Option<UnitId>) {
    let Some(a) = profile(p) else { return (raw, None) };
    if !assists(spell) {
        return (raw, None);
    }
    let reach = reach(spell, caster);
    let sticky = sticky.filter(|s| s.until >= now).map(|s| s.unit);
    query_near(rt, caster.pos, reach + bounds(caster) + i64::from(max_bounds()), near);
    let mut best: Option<(i32, UnitId, Angle)> = None;
    for &id in near.iter() {
        let Some(u) = zone.unit(id) else { continue };
        if id == caster.id
            || !u.alive
            || !u.awake
            || u.hidden
            || u.controller == Controller::Npc
            || !is_enemy(caster.faction, u.faction)
        {
            continue;
        }
        if (distance(caster.pos, u.pos) - bounds(caster) - bounds(u)).max(0) > reach {
            continue;
        }
        let bearing = iatan2(u.pos.y.0 - caster.pos.y.0, u.pos.x.0 - caster.pos.x.0);
        let turn = raw.diff(bearing).abs();
        let is_sticky = sticky == Some(id);
        let limit = i32::from(a.cone.0) + if is_sticky { i32::from(a.slack.0) } else { 0 };
        if turn > limit {
            continue;
        }
        let score = turn
            - if caster.target == Some(id) { ASSIST_TARGET_BONUS } else { 0 }
            - if is_sticky { ASSIST_STICKY_BONUS } else { 0 };
        if best.is_some_and(|(s, b, _)| (s, b) <= (score, id)) {
            continue;
        }
        best = Some((score, id, bearing));
    }
    let Some((_, id, bearing)) = best else { return (raw, None) };
    // A prop the raw bolt would switch, nearer than the unit: the player aimed at the prop.
    if let Some(along) = prop_on_line(zone, rt, caster, spell, raw, reach, props) {
        let to = zone.unit(id).map_or(i64::MAX, |u| distance(caster.pos, u.pos));
        if along < to {
            return (raw, None);
        }
    }
    let d = raw.diff(bearing);
    if d.abs() <= i32::from(a.snap.0) {
        return (bearing, Some(id));
    }
    (raw.wrapping_add(mul_div_floor(d, i32::from(a.magnet.0), 1000)), Some(id))
}

/// Resolve a seat's raw aim for a cast of `spell` (§5.4): the assisted angle, and the chosen
/// unit made sticky for the profile's `sticky_ticks`.
pub fn assisted_aim(cx: &mut Ctx<'_>, seat: Seat, spell: SpellId, raw: Angle, p: AssistProfile) -> Angle {
    let now = cx.world.tick;
    let Some(player) = cx.world.player(seat) else { return raw };
    let (body, sticky) = (player.unit, player.assist);
    let Some(caster) = cx.zone.unit(body) else { return raw };
    let def = cx.cat.combat.spell(spell);
    let mut near = core::mem::take(&mut cx.scratch.near);
    let mut props = core::mem::take(&mut cx.scratch.props_b);
    let (angle, chose) = pick(cx.zone, cx.rt, now, caster, sticky, def, raw, p, &mut near, &mut props);
    cx.scratch.near = near;
    cx.scratch.props_b = props;
    if let (Some(unit), Some(a)) = (chose, profile(p)) {
        cx.world.players[seat.index()].assist = Some(Assisted { unit, until: now.after(a.sticky_ticks) });
    }
    angle
}
