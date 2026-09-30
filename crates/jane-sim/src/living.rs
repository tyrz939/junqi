//! The living world (ARCHITECTURE.md §4.6; WORLD.md): the sky, the rain ramp, the ecology of the
//! county's patches, consequences and rumours, and a bed's night. All of it is clock-driven and
//! deterministic, and none of it is a per-tick pass over sleepers.
//!
//! **Where it runs.** Step 2, the clock, after the clock moves:
//!
//! 1. *Every ten game minutes* ([`ECOLOGY_EVERY`]), the world stream draws in a fixed order
//!    (§4.4): when the mark is also the hour, for each region in region order a kind roll and a
//!    length roll ([`Sim::world_rolls`]); then, at every mark, one roll for every area of every
//!    blueprint, zones in `ZoneId` order and areas in the blueprint's order, whether or not the
//!    zone is live or even visited. A roll a region or an area has no use for is drawn and thrown
//!    away, so the order the party goes about the county can never move a roll.
//! 2. The clock rows of the hour (`sim.rs`).
//! 3. Every tick: the rain ramp of every zone ever visited ([`WetnessTuning`] `every` ticks), the
//!    consequences whose bit is clear and whose trigger holds, the rumours whose story has just
//!    been done, and the edits owed to a zone that is live now.
//!
//! **The sky.** Each region keeps a [`WeatherState`]. When the hour turns and the region's `until`
//! has come, the kind roll picks from the region's band for the hour (weights out of 1000) and the
//! length roll picks a whole number of hours from the kind's band; before `until` both are drawn
//! and discarded. New Game is clear for `clear_hours` (the first walk). **The sky follows the
//! region under her feet:** a zone under one sky (`data/weather.json`'s zone lists) is all that
//! region's; the county is three, by the skeleton's region of each macro cell
//! (`Blueprint.regions`, [`region_at`]). The sky over a seat is her cell's region's, and
//! `EventKind::Weather` tells her when it changes, the sky's own turn or her walking into another
//! region ([`Sim::say_skies`], at the end of the step).
//!
//! **The rain ramp.** A zone keeps a ramp per region (`ZoneState.wetness`, region order): the
//! county one for each of its three, any other zone only its own region's (the other two stay 0).
//! A ramp under a sky that wets (rain, storm), in a zone not indoors, climbs by `rise` every
//! `every` ticks, else falls by `fall`, 0..=255. It steps for every zone whose state exists, live
//! or not, so a zone left in the rain dries while nobody is there. The sim reads it for one rule:
//! a prop whose def has a `douse` shows no light while the ramp of the region the prop stands in
//! is at or over it (`light::light_showing`, [`wetness_at`]). It is still a bed or a fire to rest
//! at. The ramp is a closed form over any span in which the skies hold ([`Sim::ramp`]): a
//! saturating step of one sign, taken n times, is one step of n.
//!
//! **Ecology.** A kill of a unit whose home is in an area raises the area's pressure by its
//! population row's `weight` ([`on_kill`], from `hooks::unit_died`); every ten game minutes the
//! ecology roll lowers every area's pressure by its `recover`, give or take half, floor 0. A
//! patch's creature is due to stand up at the next ten-minute mark after it dies, not after its
//! row's `respawn` ([`patch_mark`], from `hooks::respawn_at`); anything else keeps its row's. A
//! corpse due to stand up ([`may_stand`], from `hooks::respawn_allowed`) stands only while fewer
//! than `cap` of its row stand in the area and the pressure is under `hold`; otherwise it is put
//! back on `sleeping_due` at the next ten-minute mark ([`next_mark`]), where step 2 has just
//! lowered the pressure. Hunting thins a patch; time refills it, and a held patch is looked at
//! again every ten game minutes.
//!
//! **Consequences.** A row fires once per save, ever: its bit in `consequences_done` is the
//! proof. It writes the journal's `Consequence` fact (and `Confirmed` or `Contradicted` for the
//! claim it names), says `EventKind::Consequence` to everyone, and runs its edits (world verbs,
//! `Lock`, `Unlock` and a door's hours among them) in its zone with no actor, now if the zone is live, else as
//! soon as it is (`consequences_owed`, landed in step 2 and on arrival in step 14), so a zone
//! nobody was in when it fired has it when she comes.
//!
//! **Rumours.** When one of a story's quests is first handed in, each person its `spreads` names
//! is written into `rumours` at that tick plus `after`; `Condition::SpeakerKnows` asks it of the
//! person she is talking to ([`speaker_knows`]). She learns the rumour when a line that `tells` it
//! plays (`dialogue.rs`).
//!
//! **The town's news.** A consequence row may spread too: when it fires, each group of its
//! `spreads` hears at that tick plus the group's `after`, the first to hear first, so what she did
//! reaches the Arms before the milk round. It is kept in the same `rumours` map, under the key
//! [`news_key`] (the top of the story id range, counting down, where no story is), and
//! `Condition::SpeakerHeard` asks it ([`speaker_heard`]). A save's layout does not change.
//!
//! **A bed's night** ([`Sim::sleep_to`]). `Rest { until }` with the whole party resting runs the
//! clock *and the tick* on to the hour, and the world lives through the time skipped as though it
//! had passed a tick at a time: every ten-minute mark and hour crossed draws its rolls and runs
//! its clock rows, the ramp runs its closed form between them, consequences and owed edits land
//! at the tick they would have, and in every live zone the corpses due in the gap stand up (or
//! are held) at their tick, under the ecology. Only the ticks where something happens are worked;
//! the rest are counted. What does not run is the zones' own steps: nobody moves, hunts, talks
//! or is shown or hidden by the hour while she sleeps, and a timer (`*_until`, a status, a drop)
//! simply finds its tick has come. `verbs::rest` asks for it (`WorldOps.sleep`); the step runs it
//! after the commands, and at the end of the step for a rest a later step ran.
//!
//! **Schedules** are `presence.rs`'s; [`schedule_state`] says where a scheduled unit is for the
//! View (a door can say who is behind it).

use jane_core::action::{Condition, FlagTest};
use jane_core::num::dist_sq;
use jane_core::{Blueprint, Cell, ConsequenceId, Key, NameId, RegionMap, StoryId, Tick, Vec2, ZoneId};
use jane_data::{Region, ScheduleSlot};

use crate::actions::{Subject, run_actions};
use crate::blueprints::Blueprints;
use crate::ctx::{Ctx, PartySnap};
use crate::event::{Event, EventKind};
use crate::ids::{PropId, Seat, UnitId};
use crate::runtime::ZoneRuntime;
use crate::sim::Sim;
use crate::state::{FactKey, FlagKey, GameState, REGIONS, Source, Speaker, WeatherKind, WeatherState, ZoneState};
use crate::sym::of_name;
use crate::tuning::{ECOLOGY_EVERY, ORDER_ARRIVED_FX, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::units::def_of;

/// The regions, in region order: the order of `GameState.weather`, of a zone's ramps, of the
/// bytes of a `RegionMap`, and of the skies' rolls.
pub const REGION_ORDER: [Region; 3] = [Region::Lowfields, Region::Waters, Region::Works];

/// A region's index into `GameState.weather` and `ZoneState.wetness`.
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

/// The region a cell of zone `zone` lies in, by the zone's region map: the county's is the
/// skeleton's; a zone with none is all the region `data/weather.json` puts it under.
pub fn region_in(map: &RegionMap, zone: ZoneId, x: i32, y: i32) -> Region {
    match map.region_at(x, y) {
        Some(b) if usize::from(b) < REGION_ORDER.len() => REGION_ORDER[usize::from(b)],
        _ => jane_data::catalog().living.region_of(zone),
    }
}

/// The region under cell `(x, y)` of a blueprint's zone (see [`region_in`]).
pub fn region_at(bp: &Blueprint, x: i32, y: i32) -> Region {
    region_in(&bp.regions, bp.zone, x, y)
}

/// The sky over cell `(x, y)` of a blueprint's zone.
pub fn weather_at<'a>(state: &'a GameState, bp: &Blueprint, x: i32, y: i32) -> &'a WeatherState {
    &state.weather[region_ix(region_at(bp, x, y))]
}

/// The rain ramp over a cell: that of the region it lies in.
pub fn wetness_at(zone: &ZoneState, rt: &ZoneRuntime, x: i32, y: i32) -> u8 {
    zone.wetness[region_ix(rt.region_at(x, y))]
}

/// Which of a zone's ramps step: the county's three, any other zone's own region's alone.
fn ramps_of(bp: &Blueprint) -> [bool; REGIONS] {
    if !bp.regions.is_empty() {
        return [true; REGIONS];
    }
    let own = region_ix(jane_data::catalog().living.region_of(bp.zone));
    std::array::from_fn(|r| r == own)
}

/// The region and the sky over a connected seat, where her body stands.
pub fn sky_over(state: &GameState, bps: &Blueprints, seat: usize) -> Option<(Region, WeatherKind)> {
    let p = state.players.get(seat).filter(|p| p.connected)?;
    let u = state.zone(p.zone)?.unit(p.unit)?;
    let (x, y) = u.pos.cell();
    let r = region_at(bps.get(p.zone), x, y);
    Some((r, state.weather[region_ix(r)].kind))
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

/// The tick of the next ecology step, ten game minutes apart on the clock (counted from the
/// clock, so a clock set by hand still finds its mark).
pub fn next_mark(state: &GameState) -> Tick {
    state.tick.after(Tick(ECOLOGY_EVERY - state.clock % ECOLOGY_EVERY))
}

impl Sim {
    /// Step 2, on a ten-minute mark: the world stream's draws, in their fixed order (see the
    /// module doc); the skies' two rolls a region only when the mark is the hour.
    pub(crate) fn world_rolls(&mut self, hour_turned: bool) {
        let cat = jane_data::catalog();
        let now = self.state.tick;
        let hour = (self.state.clock / TICKS_PER_HOUR) as u8;
        if hour_turned {
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
    /// Whether a consequence fired or owed edits landed (a flag a later row reads may have moved).
    pub(crate) fn step_living(&mut self) -> bool {
        let now = self.state.tick;
        self.ramp(Tick(now.0.saturating_sub(1)), now);
        let fired = self.fire_consequences();
        spread_rumours(&mut self.state);
        fired | self.land_owed_live()
    }

    /// The rain ramp over the ticks `from` (exclusive) to `to` (inclusive): each ramp of every
    /// zone state steps once for every multiple of `every` in the span, up while its region's
    /// sky wets and the zone is outdoors, else down. The skies must hold for the whole span
    /// (they turn only on the hour, before the ramp's step of that tick), so the sum is exact.
    pub(crate) fn ramp(&mut self, from: Tick, to: Tick) {
        let t = jane_data::catalog().living.tuning.wetness;
        if t.every.0 == 0 || to <= from {
            return;
        }
        let n = to.0 / t.every.0 - from.0 / t.every.0;
        if n == 0 {
            return;
        }
        let skies = self.state.weather;
        for z in ZoneId::ALL {
            let bp = self.bps.get(z);
            let Some(zs) = self.state.zones[z.index()].as_deref_mut() else { continue };
            for (r, on) in ramps_of(bp).into_iter().enumerate() {
                if !on {
                    continue;
                }
                let w = u32::from(zs.wetness[r]);
                zs.wetness[r] = if !bp.indoor && skies[r].kind.wets() {
                    (w + n.saturating_mul(u32::from(t.rise))).min(255) as u8
                } else {
                    w.saturating_sub(n.saturating_mul(u32::from(t.fall))) as u8
                };
            }
        }
    }

    fn fire_consequences(&mut self) -> bool {
        let rows = jane_data::catalog().living.consequences;
        let mut fired = false;
        for (i, row) in rows.iter().enumerate() {
            let id = ConsequenceId(i as u16);
            if self.state.consequences_done.get(i as u32) || !triggered(&self.state, row.on) {
                continue;
            }
            fired = true;
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
            for sp in row.spreads {
                let at = self.state.tick.after(sp.after);
                for &n in sp.to {
                    self.state.rumours.entry((n, news_key(id))).or_insert(at);
                }
            }
            if self.state.is_live(row.zone) {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(row.zone, None, &snap, false, |cx| run_actions(cx, row.edits, Subject::None));
            } else {
                self.state.consequences_owed.push((row.zone, id));
            }
        }
        fired
    }

    fn land_owed_live(&mut self) -> bool {
        if self.state.consequences_owed.is_empty() {
            return false;
        }
        let mut landed = false;
        for z in ZoneId::ALL {
            if self.state.is_live(z) && self.state.consequences_owed.iter().any(|&(oz, _)| oz == z) {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(z, None, &snap, false, land_owed);
                landed = true;
            }
        }
        landed
    }

    /// A bed's night: the clock runs on to `hour` (the next time it reads that hour, a whole day
    /// if it reads it now) and the tick with it, and the world lives through the skipped time as
    /// it would have tick by tick (see the module doc). Called by the step, never mid-zone.
    ///
    /// Worked ticks: the first of the gap (anything made true since the last clock step fires
    /// there, as it would have), every ten-minute mark, every tick a corpse is due in a live zone,
    /// and the tick after one where a consequence fired or owed edits landed (a flag its edits
    /// set may trigger a row checked before it). Between them only the ramp moves, in closed form.
    pub(crate) fn sleep_to(&mut self, hour: u8) {
        let target = u32::from(hour % 24) * TICKS_PER_HOUR;
        let clock = self.state.clock;
        let gap = if clock < target { target - clock } else { TICKS_PER_DAY - clock + target };
        let end = self.state.tick.0.saturating_add(gap);
        let mut next_too = true;
        while self.state.tick.0 < end {
            let now = self.state.tick.0;
            let mark = now + (ECOLOGY_EVERY - self.state.clock % ECOLOGY_EVERY);
            let mut at = if next_too { now + 1 } else { mark.min(end) };
            if let Some(due) = self.first_due_live() {
                at = at.min(due.0.max(now + 1));
            }
            // The quiet ticks before `at`: counted, the ramp run over them.
            let quiet = at - 1 - now;
            if quiet > 0 {
                self.ramp(Tick(now), Tick(at - 1));
                let s = &mut self.state;
                s.tick = Tick(at - 1);
                let c = s.clock + quiet;
                s.day += c / TICKS_PER_DAY;
                s.clock = c % TICKS_PER_DAY;
            }
            // Tick `at`, as step 2 works it, then step 12's corpses in every live zone.
            next_too = self.step_clock();
            self.respawns_live();
        }
    }

    /// The earliest corpse due in a live zone.
    fn first_due_live(&self) -> Option<Tick> {
        ZoneId::ALL
            .iter()
            .filter(|&&z| self.state.is_live(z))
            .filter_map(|&z| self.state.zone(z).and_then(|zs| zs.sleeping_due.first()).map(|e| e.0))
            .min()
    }

    /// Step 12's corpses due, in every live zone whose first is due now.
    fn respawns_live(&mut self) {
        let now = self.state.tick;
        for z in ZoneId::ALL {
            let due = self.state.zone(z).and_then(|zs| zs.sleeping_due.first()).is_some_and(|e| e.0 <= now);
            if due && self.state.is_live(z) {
                let snap = PartySnap::of(&self.state);
                self.with_ctx(z, None, &snap, false, crate::life::respawn_due);
            }
        }
    }

    /// Tell each seat the sky over her when it has changed since the last step: its region's
    /// turn, or her walking under another region's (a seat that has just sat down, or a sim just
    /// loaded, is told nothing: presentation reads `View::weather`).
    pub(crate) fn say_skies(&mut self) {
        for seat in 0..self.skies.len() {
            let now = sky_over(&self.state, &self.bps, seat);
            if let (Some(was), Some((region, kind))) = (self.skies[seat], now) {
                if was != (region, kind) {
                    let to = Some(Seat(seat as u8));
                    self.events.push(Event { to, in_zone: None, kind: EventKind::Weather { region, kind } });
                }
            }
            self.skies[seat] = now;
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

/// Where the town's news of a consequence is kept in `rumours`: the story ids counted down from
/// the top, which no story reaches (the catalog's pools are checked well under it).
pub const fn news_key(c: ConsequenceId) -> StoryId {
    StoryId(u16::MAX - c.0)
}

/// `Condition::SpeakerHeard`: whoever she is talking to has heard of what the county did (a
/// consequence's `spreads`) by now. False outside a conversation.
pub fn speaker_heard(cx: &crate::actions::Ask<'_>, c: ConsequenceId) -> bool {
    speaker_knows(cx, news_key(c))
}

/// `Condition::SpeakerKnows`: whoever she is talking to (a person, or a door that speaks for
/// whoever is behind it) has heard of the story by now. False outside a conversation.
pub fn speaker_knows(cx: &crate::actions::Ask<'_>, story: StoryId) -> bool {
    let key = match cx.speaker {
        Speaker::Unit(id) => cx.zone.unit(id).and_then(|u| u.key),
        Speaker::Prop(id) => cx.zone.prop_ix(id).map(|ix| cx.zone.props[ix as usize].key),
        Speaker::None => None,
    };
    let Some(n) = key.filter(|k| (k.0 as usize) < cx.cat.names.len()).map(|k| NameId(k.0 as u16)) else {
        return false;
    };
    cx.world.rumours.get(&(n, story)).is_some_and(|&t| t <= cx.world.tick)
}

/// `Condition::SpeakerLit`: the prop she is talking to shows its light, by the one rule the
/// view draws it by (`light::light_showing`, with the rain where it stands): a doused fire's
/// words never say it burns. False outside a conversation, or talking to a unit.
pub fn speaker_lit(cx: &Ctx<'_>) -> bool {
    let Some(d) = cx.actor.and_then(|s| cx.world.player(s)).and_then(|p| p.dialogue) else { return false };
    let Speaker::Prop(id) = d.speaker else { return false };
    let Some(p) = cx.zone.prop_ix(id).map(|ix| &cx.zone.props[ix as usize]) else { return false };
    let wet = crate::light::prop_wetness(cx.zone, cx.rt, p);
    crate::light::light_showing(cx.cat.story.prop(p.def), p, cx.world.clock, wet).is_some()
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

/// The next ten-minute mark, if this unit is one of a patch's populations (its home in an
/// area whose ecology row names its def): when a corpse of it is due (`hooks::respawn_at`), and
/// when one held back is looked at again, by the ecology or by the watcher box.
pub fn patch_mark(cx: &Ctx<'_>, unit: UnitId) -> Option<Tick> {
    population_of(cx, unit).map(|_| next_mark(cx.world))
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
/// pressure is at `hold`: then it is put back on `sleeping_due` at the next ten-minute mark.
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
    let at = next_mark(cx.world);
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
    let slot = crate::presence::slot_of_unit(u, state)?;
    let (x, y) = u.pos.cell();
    let here = ScheduleWhere::Walking(Cell::new(x.max(0) as u16, y.max(0) as u16));
    let inside = |n: NameId| {
        rt.names.get(&of_name(n)).map_or(ScheduleWhere::Away, |&ix| ScheduleWhere::Inside(zone.props[ix as usize].id))
    };
    let at = match slot {
        // Hidden, whatever the hour says now: still wherever the last hiding slot put it (a
        // person waits behind her door until nobody is watching the step).
        _ if u.hidden => {
            hidden_in(def_of(u), state, crate::presence::own_hour(def_of(u), state)).map_or(ScheduleWhere::Away, inside)
        }
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

/// The prop the latest hiding slot of a unit's hours (at or before `hour`, the rows that hold
/// now) puts it behind: `Inside(prop)` names it, `Absent` (and the `dayOnly` shorthand) none.
fn hidden_in(def: &jane_data::UnitDef, state: &GameState, hour: u8) -> Option<NameId> {
    for back in 0..24u8 {
        let h = (hour + 24 - back) % 24;
        let Some(r) = crate::presence::row_at(def, state, h) else { continue };
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
