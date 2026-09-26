//! The living world (ARCHITECTURE.md §4.6; WORLD.md): the sky, the rain ramp, the ecology of the
//! county's patches, consequences and rumours. All of it is clock-driven and deterministic, and
//! none of it is a per-tick pass over sleepers.
//!
//! **Where it runs.** Step 2, the clock, after the clock moves:
//!
//! 1. *On the hour*, the world stream draws in a fixed order (§4.4): for each region in region
//!    order a kind roll and a length roll ([`Sim::world_rolls`]); then one roll for every area of
//!    every blueprint, zones in `ZoneId` order and areas in the blueprint's order, whether or not
//!    the zone is live or even visited. A roll a region or an area has no use for is drawn and
//!    thrown away, so the order the party goes about the county can never move a roll.
//! 2. The clock rows of the hour (`sim.rs`).
//! 3. Every tick: the rain ramp of every zone ever visited ([`WetnessTuning`] `every` ticks), the
//!    consequences whose bit is clear and whose trigger holds, the rumours whose story has just
//!    been done, and the edits owed to a zone that is live now.
//!
//! **The sky.** Each region keeps a [`WeatherState`]. When the hour turns and the region's `until`
//! has come, the kind roll picks from the region's band for the hour (weights out of 1000) and the
//! length roll picks a whole number of hours from the kind's band; before `until` both are drawn
//! and discarded. New Game is clear for `clear_hours` (the first walk).
//!
//! **The rain ramp.** A zone under a sky that wets (rain, storm) and not indoors climbs by `rise`
//! every `every` ticks, else falls by `fall`, 0..=255. It steps for every zone whose state exists,
//! live or not, so a zone left in the rain dries while nobody is there. The sim reads it for one
//! rule: a prop whose def has a `douse` shows no light while the wetness is at or over it
//! (`light::light_showing`). It is still a bed or a fire to rest at.
//!
//! **Ecology.** A kill of a unit whose home is in an area raises the area's pressure by its
//! population row's `weight` ([`on_kill`], from `hooks::unit_died`); the hourly roll lowers every
//! area's pressure by its `recover`, give or take half, floor 0. A corpse due to stand up
//! ([`may_stand`], from `hooks::respawn_allowed`) stands only while fewer than `cap` of its row
//! stand in the area and the pressure is under `hold`; otherwise it is put back on
//! `sleeping_due` at the next hour. Hunting thins a patch; time refills it.
//!
//! **Consequences.** A row fires once per save, ever: its bit in `consequences_done` is the
//! proof. It writes the journal's `Consequence` fact (and `Confirmed` or `Contradicted` for the
//! claim it names), says `EventKind::Consequence` to everyone, and runs its edits in its zone with
//! no actor, now if the zone is live, else as soon as it is (`consequences_owed`, landed in step 2
//! and on arrival in step 14), so a zone nobody was in when it fired has it when she comes.
//!
//! **Rumours.** When one of a story's quests is first handed in, each person its `spreads` names
//! is written into `rumours` at that tick plus `after`; `Condition::SpeakerKnows` asks it of the
//! person she is talking to ([`speaker_knows`]). She learns the rumour when a line that `tells` it
//! plays (`dialogue.rs`).
//!
//! **Schedules** are `presence.rs`'s; [`schedule_state`] says where a scheduled unit is for the
//! View (a door can say who is behind it).

use jane_core::action::{Condition, FlagTest};
use jane_core::num::dist_sq;
use jane_core::{Cell, ConsequenceId, Key, NameId, StoryId, Tick, Vec2, ZoneId};
use jane_data::{Region, ScheduleSlot};

use crate::actions::{Subject, run_actions};
use crate::ctx::{Ctx, PartySnap};
use crate::event::{Event, EventKind};
use crate::ids::{PropId, UnitId};
use crate::runtime::ZoneRuntime;
use crate::sim::Sim;
use crate::state::{FactKey, FlagKey, GameState, Source, Speaker, WeatherKind, WeatherState, ZoneState};
use crate::sym::of_name;
use crate::tuning::{ORDER_ARRIVED_FX, TICKS_PER_HOUR};
use crate::units::def_of;

/// The regions, in region order: the order of `GameState.weather` and of the hourly rolls.
pub const REGION_ORDER: [Region; 3] = [Region::Lowfields, Region::Waters, Region::Works];

/// A region's index into `GameState.weather`.
pub const fn region_ix(r: Region) -> usize {
    match r {
        Region::Lowfields => 0,
        Region::Waters => 1,
        Region::Works => 2,
    }
}

/// The skies of New Game: clear, and held clear for the first walk.
pub fn first_skies() -> [WeatherState; 3] {
    let hours = u32::from(jane_data::catalog().living.tuning.clear_hours);
    [WeatherState { kind: WeatherKind::Clear, since: Tick::ZERO, until: Tick(hours * TICKS_PER_HOUR) }; 3]
}

/// The sky over a zone.
pub fn weather_in(state: &GameState, z: ZoneId) -> &WeatherState {
    &state.weather[region_ix(jane_data::catalog().living.region_of(z))]
}

/// `below(n)` of a draw already taken: `n` values, uniform, from one `u32`.
const fn scale(roll: u32, n: u32) -> u32 {
    ((roll as u64 * n as u64) >> 32) as u32
}

/// The kind a roll of `0..1000` picks from a band's weights.
fn pick(weights: [u16; 4], roll: u32) -> usize {
    let mut acc = 0u32;
    for (i, w) in weights.iter().enumerate() {
        acc += u32::from(*w);
        if roll < acc {
            return i;
        }
    }
    0
}

/// The tick the next hour turns at (a bed moves the clock, not the tick: this counts from now).
pub fn next_hour(state: &GameState) -> Tick {
    state.tick.after(Tick(TICKS_PER_HOUR - state.clock % TICKS_PER_HOUR))
}

impl Sim {
    /// Step 2, on the hour: the world stream's draws, in their fixed order (see the module doc).
    pub(crate) fn world_rolls(&mut self) {
        let cat = jane_data::catalog();
        let now = self.state.tick;
        let hour = (self.state.clock / TICKS_PER_HOUR) as u8;
        for (r, &region) in REGION_ORDER.iter().enumerate() {
            let kind_roll = self.state.rng.below(1000);
            let len_roll = self.state.rng.next_u32();
            let w = self.state.weather[r];
            if now < w.until {
                continue;
            }
            let Some(row) = cat.living.sky(region) else { continue };
            let Some(band) = row.band_at(hour) else { continue };
            let i = pick(band.weights, kind_roll);
            let (lo, hi) = row.last[i];
            let hours = u32::from(lo) + scale(len_roll, u32::from(hi.saturating_sub(lo)) + 1);
            let kind = WeatherKind::of(jane_data::Sky::ALL[i]);
            let since = if kind == w.kind { w.since } else { now };
            self.state.weather[r] = WeatherState { kind, since, until: now.after(Tick(hours * TICKS_PER_HOUR)) };
            if kind != w.kind {
                self.events.push(Event { to: None, in_zone: None, kind: EventKind::Weather { region, kind } });
            }
        }
        for z in ZoneId::ALL {
            let bp = self.bps.get(z);
            for (a, area) in bp.areas.iter().enumerate() {
                let roll = self.state.rng.next_u32();
                let Some(zs) = self.state.zones[z.index()].as_deref_mut() else { continue };
                let Key::Name(n) = area.name else { continue };
                let Some(eco) = cat.living.ecology_of(n) else { continue };
                let Some(p) = zs.pressure.get_mut(a) else { continue };
                let r = u32::from(eco.recover);
                let off = r / 2 + scale(roll, r + 1);
                *p = p.saturating_sub(off.min(u32::from(u16::MAX)) as u16);
            }
        }
    }

    /// Step 2, every tick after the clock rows: the ramp, consequences, rumours, owed edits.
    pub(crate) fn step_living(&mut self) {
        self.step_wetness();
        self.fire_consequences();
        spread_rumours(&mut self.state);
        self.land_owed_live();
    }

    fn step_wetness(&mut self) {
        let cat = jane_data::catalog();
        let t = cat.living.tuning.wetness;
        if t.every.0 == 0 || self.state.tick.0 % t.every.0 != 0 {
            return;
        }
        for z in ZoneId::ALL {
            let wets = !self.bps.get(z).indoor && weather_in(&self.state, z).kind.wets();
            let Some(zs) = self.state.zones[z.index()].as_deref_mut() else { continue };
            zs.wetness = if wets { zs.wetness.saturating_add(t.rise) } else { zs.wetness.saturating_sub(t.fall) };
        }
    }

    fn fire_consequences(&mut self) {
        let rows = jane_data::catalog().living.consequences;
        for (i, row) in rows.iter().enumerate() {
            let id = ConsequenceId(i as u16);
            if self.state.consequences_done.get(i as u32) || !triggered(&self.state, row.on) {
                continue;
            }
            self.state.consequences_done.grow_to(rows.len() as u32);
            self.state.consequences_done.set(i as u32, true);
            let at = Cell::new(0, 0);
            let (s, ev) = (&mut self.state, &mut self.events);
            crate::journal::record(s, ev, FactKey::Consequence(id), Source::Seen, row.zone, at);
            if let Some(t) = row.confirms {
                crate::journal::record(s, ev, FactKey::Claim(t), Source::Confirmed, row.zone, at);
            }
            if let Some(t) = row.contradicts {
                crate::journal::record(s, ev, FactKey::Claim(t), Source::Contradicted, row.zone, at);
            }
            self.events.push(Event { to: None, in_zone: None, kind: EventKind::Consequence(id) });
            if self.state.is_live(row.zone) {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(row.zone, None, &snap, false, |cx| run_actions(cx, row.edits, Subject::None));
            } else {
                self.state.consequences_owed.push((row.zone, id));
            }
        }
    }

    fn land_owed_live(&mut self) {
        if self.state.consequences_owed.is_empty() {
            return;
        }
        for z in ZoneId::ALL {
            if self.state.is_live(z) && self.state.consequences_owed.iter().any(|&(oz, _)| oz == z) {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(z, None, &snap, false, land_owed);
            }
        }
    }
}

/// Does a consequence's trigger hold? Flags are the world's; a named death is its `dead:` flag.
fn triggered(state: &GameState, on: Condition) -> bool {
    let name = |k: Key| match k {
        Key::Name(n) => Some(of_name(n)),
        Key::Local(_) => None,
    };
    match on {
        Condition::Flag { key, test } => {
            let k = match key {
                jane_core::FlagKey::Named(k) => name(k).map(FlagKey::Named),
                jane_core::FlagKey::Been(k) => name(k).map(FlagKey::Been),
                jane_core::FlagKey::Dead(k) => name(k).map(FlagKey::Dead),
            };
            let v = k.and_then(|k| state.flags.get(&k).copied()).unwrap_or(0);
            match test {
                FlagTest::Eq(n) => v == n,
                FlagTest::Min(n) => v >= n,
                FlagTest::NonZero => v != 0,
            }
        }
        Condition::QuestDone(q) => crate::quests::done(state, q),
        Condition::Dead(k) => name(k).is_some_and(|s| state.flags.get(&FlagKey::Dead(s)).is_some_and(|&v| v != 0)),
        _ => false,
    }
}

/// Run the edits owed to the ctx's zone, in the order their consequences fired.
pub fn land_owed(cx: &mut Ctx<'_>) {
    let z = cx.zone.id;
    let mut i = 0;
    while i < cx.world.consequences_owed.len() {
        let (oz, id) = cx.world.consequences_owed[i];
        if oz != z {
            i += 1;
            continue;
        }
        cx.world.consequences_owed.remove(i);
        let row = &cx.cat.living.consequences[id.index()];
        run_actions(cx, row.edits, Subject::None);
    }
}

/// A story whose quest has just been handed in for the first time is heard of: each person it
/// spreads to, from now plus `after`.
fn spread_rumours(state: &mut GameState) {
    let stories = jane_data::catalog().county.stories;
    for s in stories {
        let Some(sp) = s.spreads else { continue };
        let Some(&first) = sp.to.first() else { continue };
        if state.rumours.contains_key(&(first, s.id)) || !s.quests.iter().any(|&q| crate::quests::done(state, q)) {
            continue;
        }
        let at = state.tick.after(sp.after);
        for &n in sp.to {
            state.rumours.entry((n, s.id)).or_insert(at);
        }
    }
}

/// `Condition::SpeakerKnows`: whoever she is talking to (a person, or a door that speaks for
/// whoever is behind it) has heard of the story by now. False outside a conversation.
pub fn speaker_knows(cx: &Ctx<'_>, story: StoryId) -> bool {
    let Some(d) = cx.actor.and_then(|s| cx.world.player(s)).and_then(|p| p.dialogue) else { return false };
    let key = match d.speaker {
        Speaker::Unit(id) => cx.zone.unit(id).and_then(|u| u.key),
        Speaker::Prop(id) => cx.zone.prop_ix(id).map(|ix| cx.zone.props[ix as usize].key),
        Speaker::None => None,
    };
    let Some(n) = key.filter(|k| (k.0 as usize) < cx.cat.names.len()).map(|k| NameId(k.0 as u16)) else {
        return false;
    };
    cx.world.rumours.get(&(n, story)).is_some_and(|&t| t <= cx.world.tick)
}

/// The area a unit's home lies in, and its population row there, if the area has an ecology.
fn population_of(cx: &Ctx<'_>, unit: UnitId) -> Option<(usize, &'static jane_data::Population)> {
    let u = cx.zone.unit(unit)?;
    let (hx, hy) = u.home.cell();
    let a = cx.bp.area_at(hx, hy)?;
    let Key::Name(n) = cx.bp.areas[a].name else { return None };
    let pop = cx.cat.living.ecology_of(n)?.population(u.def)?;
    Some((a, pop))
}

/// A creature died: its area's pressure rises by its row's weight.
pub fn on_kill(cx: &mut Ctx<'_>, unit: UnitId) {
    let Some((a, pop)) = population_of(cx, unit) else { return };
    if cx.zone.pressure.len() < cx.bp.areas.len() {
        cx.zone.pressure.resize(cx.bp.areas.len(), 0);
    }
    cx.zone.pressure[a] = cx.zone.pressure[a].saturating_add(pop.weight);
}

/// May a corpse due stand up now? Not while its row stands at `cap` in its area or the area's
/// pressure is at `hold`: then it is put back on `sleeping_due` at the next hour.
pub fn may_stand(cx: &mut Ctx<'_>, unit: UnitId) -> bool {
    let Some((a, pop)) = population_of(cx, unit) else { return true };
    let def = cx.zone.unit(unit).map(|u| u.def);
    let pressure = cx.zone.pressure.get(a).copied().unwrap_or(0);
    let bp = cx.bp;
    let standing = cx
        .zone
        .units
        .iter()
        .filter(|o| {
            let (x, y) = o.home.cell();
            o.alive && Some(o.def) == def && bp.area_at(x, y) == Some(a)
        })
        .count();
    if pressure < pop.hold && standing < usize::from(pop.cap) {
        return true;
    }
    let at = next_hour(cx.world);
    let ix = cx.zone.sleeping_due.partition_point(|&e| e < (at, unit));
    cx.zone.sleeping_due.insert(ix, (at, unit));
    false
}

/// Where a scheduled unit is, for the View (ARCHITECTURE.md §11 `schedule_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduleState {
    /// The slot the hour gives it.
    pub slot: ScheduleSlot,
    pub at: ScheduleWhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleWhere {
    /// Standing at the slot's mark.
    Mark(NameId),
    /// Behind this prop (a door can say who is in).
    Inside(PropId),
    /// Out and about at this cell: walking to its mark, on its patrol, or not yet gone in
    /// because someone is watching.
    Walking(Cell),
    /// Not in the world.
    Away,
}

/// The slot a unit's hours give it now and where that leaves it; `None` for a unit with no
/// hours (always present) or one not here.
pub fn schedule_state(state: &GameState, zone: &ZoneState, rt: &ZoneRuntime, unit: UnitId) -> Option<ScheduleState> {
    let u = zone.unit(unit)?;
    let slot = crate::presence::slot_of(def_of(u), state)?;
    let (x, y) = u.pos.cell();
    let here = ScheduleWhere::Walking(Cell::new(x.max(0) as u16, y.max(0) as u16));
    let inside = |n: NameId| {
        rt.names.get(&of_name(n)).map_or(ScheduleWhere::Away, |&ix| ScheduleWhere::Inside(zone.props[ix as usize].id))
    };
    let at = match slot {
        // Hidden, whatever the hour says now: still wherever the last hiding slot put it (a
        // person waits behind her door until nobody is watching the step).
        _ if u.hidden => hidden_in(def_of(u), state.hour() as u8).map_or(ScheduleWhere::Away, inside),
        ScheduleSlot::Mark(n) => {
            let there = rt.mark(of_name(n)).is_some_and(|m| {
                let to = Vec2::centre(i32::from(m.cell.x), i32::from(m.cell.y));
                dist_sq(u.pos, to) <= i64::from(ORDER_ARRIVED_FX).pow(2)
            });
            if there && u.order.is_none() { ScheduleWhere::Mark(n) } else { here }
        }
        ScheduleSlot::Inside(_) | ScheduleSlot::Patrol | ScheduleSlot::Absent => here,
    };
    Some(ScheduleState { slot, at })
}

/// The prop the latest hiding slot of a unit's hours (at or before `hour`) puts it behind:
/// `Inside(prop)` names it, `Absent` (and the `dayOnly` shorthand) none.
fn hidden_in(def: &jane_data::UnitDef, hour: u8) -> Option<NameId> {
    for back in 0..24u8 {
        let h = (hour + 24 - back) % 24;
        let Some(r) = def.schedule.iter().find(|r| jane_data::in_span(h, r.hour_from, r.hour_to)) else { continue };
        match r.slot {
            ScheduleSlot::Inside(n) => return Some(n),
            ScheduleSlot::Absent => return None,
            ScheduleSlot::Mark(_) | ScheduleSlot::Patrol => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roll_picks_by_weight_and_a_band_by_scale() {
        let w = [700, 100, 200, 0];
        assert_eq!(pick(w, 0), 0);
        assert_eq!(pick(w, 699), 0);
        assert_eq!(pick(w, 700), 1);
        assert_eq!(pick(w, 799), 1);
        assert_eq!(pick(w, 800), 2);
        assert_eq!(pick(w, 999), 2);
        assert_eq!(pick([0, 0, 0, 1000], 0), 3);
        assert_eq!(scale(0, 4), 0);
        assert_eq!(scale(u32::MAX, 4), 3);
        assert_eq!(scale(u32::MAX, 1), 0);
    }
}
