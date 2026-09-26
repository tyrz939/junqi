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

use jane_core::angle::iatan2;
use jane_core::num::mul_div_floor;
use jane_core::{Angle, SpellId, Tick};
use jane_data::{Controller, SpellDef, SpellKind};

use crate::combat::{bounds, distance, is_enemy, max_bounds, query_near};
use crate::ctx::Ctx;
use crate::ids::{Seat, UnitId};
use crate::input::AssistProfile;
use crate::runtime::ZoneRuntime;
use crate::state::{Assisted, Unit, ZoneState};
use crate::tuning::{ASSIST_MOUSE, ASSIST_PAD, ASSIST_STICKY_BONUS, ASSIST_TARGET_BONUS, Assist};

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

/// The assisted angle for a cast from `caster` along `raw`, and the unit it chose. Pure: reads
/// the zone, the runtime and the sticky unit; `near` is a reused buffer.
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
    let mut near = std::mem::take(&mut cx.scratch.near);
    let (angle, chose) = pick(cx.zone, cx.rt, now, caster, sticky, def, raw, p, &mut near);
    cx.scratch.near = near;
    if let (Some(unit), Some(a)) = (chose, profile(p)) {
        cx.world.players[seat.index()].assist = Some(Assisted { unit, until: now.after(a.sticky_ticks) });
    }
    angle
}
