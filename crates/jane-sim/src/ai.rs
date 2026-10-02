//! Step 7, the controllers (`sim/ai.ts`; ARCHITECTURE.md §4.2 step 7). AI is an MMO loop, not a
//! chase script (2020's `scr_ai_logic`):
//!
//! | State | What it does |
//! | --- | --- |
//! | idle | look about every [`AGGRO_PERIOD`] ticks, staggered by the id, for someone in reach and in sight; else the ecology rows (`hunts`, `flees`, §4.6.c); else its bait; else its patrol |
//! | combat | drop a target that is gone or no enemy; leash when too far from home, when the target leaves the light a `sight: lit` row needs, or when it stands in the warm light a `shuns_light` row cannot; else the first spell of its book it can afford, walking in on `TooFar` or `NotInLos`, standing and facing the fight on anything else |
//! | leash | let go, run home (regen is step 5's), then idle; a home it cannot reach becomes wherever it stands |
//!
//! The book's order is the priority, and the AI casts through the player's own
//! [`try_cast`](crate::combat::try_cast). Three rows change the loop and none is a class: an
//! **order** (`Send`: it walks there and minds nothing else, [`crate::npc`]), **`sight: lit`**
//! (it only notices, and only keeps, a target that stands in a prop's light) and
//! **`shuns_light`** (it will not step into warm light: it walks to the edge and waits). And one
//! thing that is not a row at all: the night ([`night_reach`]), which lengthens its reach. How
//! far it notices her is [`aggro_reach`]: its row's aggro, shorter the more she has grown past
//! its phase and a little longer while she is behind it, a quarter longer in the dark, never
//! off the screen (WoW's rule scaled to the view; PLAN.md §2.6 *Aggro*). By day
//! nothing is wary: a hostile comes for her on sight within its aggro, day or night, county or
//! dungeon (the owner, 2026-09-29; PLAN.md §2.6 *Day*). What leaves her be is a row with no
//! aggro (the county's rabbits, sheep, hens and its people), not an hour.
//!
//! **Walking** is [`follow_to`]: windowed A* ([`crate::path`]) along a cached cell path, re-planned
//! when it is used up, when the goal moved, or every `REPATH_TICKS`; at most four searches a
//! tick in a zone, the rest wait a tick. A cell someone stepped into since is waited at.
//!
//! **Target choice** asks the party snapshot (`PartySnap`, §4.2) who the party is, and reads
//! their bodies as they stand. Something that bites has only the party's side for enemies, and
//! of that side only the ones it fights (never an npc), so it looks at the party's bodies rather
//! than every unit of a county of several thousand.
//!
//! **A path that is let go** keeps its box: [`clear_path`] empties it and marks it goal-less, so
//! a chase that stops and starts allocates nothing once warm. [`live_path`] is the path as the
//! TS saw it (`null` for both `None` and a cleared one).

use jane_core::action::School;
use jane_core::angle::{along, bearing};
use jane_core::num::{CELL_FX, dist_sq, isqrt};
use jane_core::{CellIx, Fx, SpellId, Tick, Vec2};
use jane_data::{Controller, Faction, UnitDef, UnitSight};

use crate::combat::{Hit, distance, is_enemy, max_bounds, metres_between, query_near, queue_hit};
use crate::ctx::Ctx;
use crate::event::SpellError;
use crate::ids::UnitId;
use crate::light::{lit_at, max_light_radius, prop_centre, reach_sq};
use crate::los::line_of_sight;
use crate::path::{PATH_WINDOW, PathAsk, REPATH_BACKOFF_MAX, REPATH_TICKS, cost_of_cells};
use crate::runtime::ZoneRuntime;
use crate::state::{CombatState, PathCache, Unit, ZoneState};
use crate::status::{is_stunned, speed_factor};
use crate::tuning::{
    AGGRO_FLOOR_FX, AGGRO_MAX_FX, AGGRO_PAR, AGGRO_PERIOD, BAIT_EAT_FX, BAIT_HIT, CHASE_PATH_TIMES, LEASH_PATH_TIMES,
    LEASH_SNAP_FX, NIGHT_AGGRO, NIGHT_LEASH, PATROL_PATH_CELLS, PATROL_REACHED_FX, REPATH_SOON, WORKS_SCALE,
};
use crate::units::{def_of, face_vector, move_unit, think_offset};

/// A path's goal that no cell has: the path was let go.
pub const NO_GOAL: CellIx = CellIx(u32::MAX);

// --- step 7 --------------------------------------------------------------------------------

/// Step 7: every awake unit's controller, over the awake list as it stood when the step began
/// (a wake or a spawn lands at step 13). The dead, the hidden and the stunned do nothing. With
/// `everyone` (§8 `awake_only_equals_everyone`) the pass is the TS's: every unit of the zone,
/// asked whether it is awake.
pub fn step_controllers(cx: &mut Ctx<'_>, everyone: bool) {
    let now = cx.world.tick;
    let mut ids = std::mem::take(&mut cx.scratch.units);
    ids.clear();
    if everyone {
        ids.extend(cx.zone.units.iter().filter(|u| u.awake).map(|u| u.id));
    } else {
        ids.extend_from_slice(&cx.rt.awake_units);
    }
    for &id in &ids {
        let Some(u) = cx.zone.unit(id) else { continue };
        // Frozen by a blow's hitlag, it waits with the stunned.
        if !u.alive || u.hidden || is_stunned(u, now) || crate::feel::lagged(u, now) {
            continue;
        }
        match u.controller {
            Controller::Ai => tick_ai(cx, id),
            Controller::Snake => crate::snake::tick_snake(cx, id),
            // Nobody fights these, but they may be sent somewhere, a butterfly keeps its round of
            // flowers, and a hen runs from the fox.
            Controller::Npc => {
                if u.order.is_some() || u.patrol.is_some() || !def_of(u).flees.is_empty() {
                    crate::npc::tick_npc(cx, id);
                }
            }
            Controller::Player => {}
        }
    }
    cx.scratch.units = ids;
}

/// One tick of an AI unit (`ai.ts tickAi`).
pub fn tick_ai(cx: &mut Ctx<'_>, id: UnitId) {
    let Some(u) = cx.zone.unit(id) else { return };
    tick_ai_with(cx, id, def_of(u));
}

/// [`tick_ai`] over a row that need not be the unit's own (tests prove the ecology rows on a
/// row made for them, as `combat_tests` does for spells).
pub fn tick_ai_with(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef) {
    let Some(u) = cx.zone.unit(id) else { return };
    if u.order.is_some() {
        crate::npc::follow_order_with(cx, id, def);
        return;
    }
    let shy = def.shuns_light;
    // A boss in a later phase may have another speed.
    let run = match u.phase {
        0 => def.run,
        n => def.phases.get(usize::from(n) - 1).and_then(|p| p.run).unwrap_or(def.run),
    };
    // Out of the fight (it lost her, it leashed), a blow it was winding up is let go.
    let combat = u.combat;
    if combat != CombatState::Combat && u.feel.windup.is_some() {
        crate::feel::drop_windup(cx, id);
    }
    match combat {
        CombatState::Idle => idle(cx, id, def, shy),
        CombatState::Leash => leash(cx, id, def, run, shy),
        CombatState::Combat => fight(cx, id, def, run, shy),
    }
}

// --- the three states ------------------------------------------------------------------------

fn idle(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, shy: bool) {
    let now = cx.world.tick;
    if (now.0 + think_offset(id)) % AGGRO_PERIOD == 0 {
        // The only place a sleeping county asks about the dark, and it asks once every ten ticks.
        let dark = if def.aggro.0 > 0 { night_reach(cx, id, def) } else { 0 };
        let party = nearest_enemy(cx, id, def, dark, def.sight == UnitSight::Lit);
        let found = party.or_else(|| {
            let prey = cx.zone.unit(id).map_or(0, |u| aggro_reach(def, u.strength, None, dark));
            hunted(cx, id, def, prey)
        });
        if let Some(t) = found {
            let u = cx.zone.unit_mut(id).expect("unit");
            u.target = Some(t);
            u.combat = CombatState::Combat;
            clear_path(u);
            return;
        }
    }
    if flee(cx, id, def, shy) {
        return;
    }
    if let Some(item) = def.bait {
        if seek_bait(cx, id, def, item) {
            return;
        }
    }
    patrol(cx, id, def.walk, shy);
}

fn leash(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, run: Fx, shy: bool) {
    // A thing that never runs (the Burial's small snakes: they spit, they do not chase) walks
    // home. At a run of nothing it stood leashing where the fight left it for good, and a thing
    // leashing takes no bait.
    let run = if run.0 > 0 { run } else { def.walk };
    let clock = cx.world.clock;
    let u = cx.zone.unit_mut(id).expect("unit");
    u.target = None;
    let (pos, home) = (u.pos, u.home);
    // Home, or in home's own cell: a path from a cell to itself has no steps, so a walker a few
    // pixels off its post inside that cell would lead on for ever, mending and deaf to her (the
    // Foreman, back from a chase, stood two and a half pixels from his mark in Leash all day).
    if distance(pos, home) <= i64::from(run.0.max(LEASH_SNAP_FX)) || pos.cell() == home.cell() {
        move_unit(cx.rt, u, home.x - pos.x, home.y - pos.y);
        u.combat = CombatState::Idle;
        clear_path(u);
        return;
    }
    // Caught in the light it goes home by the straight way; otherwise it keeps to the dark.
    let round = shy && !lit_at(cx.zone, cx.rt, clock, pos, true);
    let cells = cells_of(i64::from(def.leash.0) * i64::from(LEASH_PATH_TIMES));
    let found = follow_to(cx, id, home, run, cells, round);
    let u = cx.zone.unit_mut(id).expect("unit");
    let waiting = round && live_path(u).is_some_and(|p| usize::from(p.at) >= p.cells.len());
    if !found || waiting {
        // It cannot get home (a door shut behind it, or its post is lit now): it stands guard here.
        u.home = u.pos;
        u.combat = CombatState::Idle;
    }
}

fn fight(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, run: Fx, shy: bool) {
    let now = cx.world.tick;
    let clock = cx.world.clock;
    let u = cx.zone.unit(id).expect("unit");
    let target = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.alive && is_enemy(u.faction, t.faction));
    let Some(tpos) = target.map(|t| t.pos) else {
        let u = cx.zone.unit_mut(id).expect("unit");
        u.target = None;
        u.combat = CombatState::Leash;
        return;
    };
    // How far it follows her from its post. In the dark it is much further, and it shortens
    // again the moment the chase reaches a lamp: a thing runs you to the light and turns back.
    let dark = night_reach(cx, id, def);
    let leash = i64::from(def.leash.0) * i64::from(10 + NIGHT_LEASH * dark) / 10;
    let u = cx.zone.unit(id).expect("unit");
    let (pos, home) = (u.pos, u.home);
    // A rooted thing (a flower, a cactus: no feet) never leaves its post, so no chase takes it
    // past its leash: it lets go once she is half again the screen's notice from it (or its own
    // aggro, if longer), or it would hold her in its fight from across the zone for good. Not its
    // own shorter aggro (2026-09-30): she shoots it from bolt range, on the screen, and a thing
    // that let go there stood idle and mended whole between her bolts.
    let rooted_off = run.0 <= 0 && distance(tpos, home) > i64::from(def.aggro.0.max(AGGRO_MAX_FX)) * 3 / 2;
    let too_far = distance(pos, home) > leash || rooted_off;
    // Light is how a sentry sees: a target that steps into the dark is a target it no longer has.
    let unseen = !too_far && def.sight == UnitSight::Lit && !lit_at(cx.zone, cx.rt, clock, tpos, false);
    // Warm light keeps a shade off: standing in it, it does nothing but leave.
    let scorched = !too_far && !unseen && shy && lit_at(cx.zone, cx.rt, clock, pos, true);
    if too_far || unseen || scorched {
        let u = cx.zone.unit_mut(id).expect("unit");
        if !too_far {
            u.target = None;
        }
        u.combat = CombatState::Leash;
        clear_path(u);
        return;
    }
    // Winding up a blow: committed to it until it lands (`feel.rs`).
    if u.feel.windup.is_some() {
        crate::feel::hold(cx, id);
        return;
    }
    if now < u.stop_until {
        return;
    }
    // A thing that cannot resist its bait cannot resist it with her in its sight either ("A
    // burial snake that smells it will come, and will not leave"): the fight waits on the meat.
    if let Some(item) = def.bait {
        if seek_bait(cx, id, def, item) {
            return;
        }
    }
    let u = cx.zone.unit(id).expect("unit");
    let Some(spell) = pick_spell(u, now) else {
        approach(cx, id, tpos, run, leash, shy);
        return;
    };
    match crate::feel::cast_or_windup(cx, id, spell) {
        Ok(()) => {
            let u = cx.zone.unit_mut(id).expect("unit");
            clear_path(u);
            face_point(u, tpos);
        }
        Err(SpellError::TooFar | SpellError::NotInLos) => approach(cx, id, tpos, run, leash, shy),
        // On cooldown, on the GCD, short of mana: hold position and keep facing the fight.
        Err(_) => {
            if let Some(u) = cx.zone.unit_mut(id) {
                face_point(u, tpos);
            }
        }
    }
}

/// The first spell of the book that is off cooldown and affordable: the order is the priority.
/// Everything cooling down, it reports the first, so range and sight still drive its feet.
pub fn pick_spell(u: &Unit, now: Tick) -> Option<SpellId> {
    let cat = jane_data::catalog();
    let book = crate::combat::book_of(u);
    for &id in book {
        let s = cat.combat.spell(id);
        if u.cooldowns.iter().any(|&(c, until)| c == id && until > now) {
            continue;
        }
        if u.mp < s.mp || u.energy < s.energy {
            continue;
        }
        if u.gcd_until > now && !s.gcd_immune {
            continue;
        }
        return Some(id);
    }
    book.first().copied()
}

/// In after a target it cannot reach from here. The path may be twice the leash; a search that
/// finds nothing leaves it standing, and it asks again when it may (the TS's `Fail -> leash`
/// never fired: its failed search had just reset the clock it read).
fn approach(cx: &mut Ctx<'_>, id: UnitId, tpos: Vec2, speed: Fx, leash: i64, shy: bool) {
    if speed.0 <= 0 {
        if let Some(u) = cx.zone.unit_mut(id) {
            face_point(u, tpos);
        }
        return;
    }
    follow_to(cx, id, tpos, speed, cells_of(leash * i64::from(CHASE_PATH_TIMES)), shy);
    // At the edge of her light with nowhere nearer to stand: it waits, and it watches her.
    let u = cx.zone.unit_mut(id).expect("unit");
    if shy && live_path(u).is_some_and(|p| usize::from(p.at) >= p.cells.len()) {
        face_point(u, tpos);
    }
}

// --- looking about ----------------------------------------------------------------------------

/// How much further this thing notices, and follows, because it is dark where it stands: 0 by
/// day or in warm light, 2 for a creature of the Works (four times its row's strength or more:
/// the phase table put it down deep), else 1. By day nothing here asks about the light at all.
pub fn night_reach(cx: &Ctx<'_>, id: UnitId, def: &UnitDef) -> i32 {
    if !cx.world.is_night() {
        return 0;
    }
    let Some(u) = cx.zone.unit(id) else { return 0 };
    if lit_at(cx.zone, cx.rt, cx.world.clock, u.pos, true) {
        return 0;
    }
    if u32::from(u.strength) >= u32::from(def.strength) * u32::from(WORKS_SCALE) { 2 } else { 1 }
}

/// How far (between bodies, `Fx`) a unit of row `def` standing at `strength` notices a body whose
/// strength and spirit come to `her` (`None`: not her, the prey of its `hunts`), with `dark` from
/// [`night_reach`]. WoW's rule (20 yards at her level, a yard less for each level she is over it,
/// never under 5) scaled to a view 27 cells high (PLAN.md §2.6 *Aggro*, the owner, 2026-09-30):
///
/// - its match is [`AGGRO_PAR`] times its phase's scale (its strength over its row's: the
///   phase table's multiple, as spawned). At her match it is the row's aggro;
/// - past her match it shortens by a quarter for each match more (twice it: three quarters;
///   five times: nothing), never under [`AGGRO_FLOOR_FX`] (a row shorter than that keeps its own);
/// - short of it, it lengthens by half of what she lacks (half her match: a quarter longer);
/// - in the dark a quarter longer again ([`NIGHT_AGGRO`]), and never past [`AGGRO_MAX_FX`];
/// - a boss is its arena's: its row, a quarter longer in the dark; a row with no aggro is 0.
///
/// Integer throughout, and nothing drawn: every machine at the table gets the same reach.
pub fn aggro_reach(def: &UnitDef, strength: u16, her: Option<u32>, dark: i32) -> i64 {
    let base = i64::from(def.aggro.0);
    if base <= 0 {
        return 0;
    }
    let night = |r: i64| if dark > 0 { r * NIGHT_AGGRO / 100 } else { r };
    if def.boss {
        return night(base);
    }
    let max = i64::from(AGGRO_MAX_FX).max(base);
    let r = match her {
        None => base,
        Some(her) => {
            let scale = (i64::from(strength) / i64::from(def.strength.max(1))).max(1);
            let par = AGGRO_PAR * scale;
            let her = i64::from(her);
            let r = if her >= par { base * (5 * par - her) / (4 * par) } else { base * (3 * par - her) / (2 * par) };
            r.clamp(base.min(i64::from(AGGRO_FLOOR_FX)), max)
        }
    };
    night(r).min(max)
}

/// The party body nearest `id` (between bodies) within its [`aggro_reach`] of that body, alive,
/// awake, unhidden, not a god, an enemy, in sight (and, `lit_only`, standing in a prop's light).
/// A tie goes to the later seat, as the TS's scan did. A friendly AI (no row has one) looks at
/// every present unit, at its row's reach.
pub fn nearest_enemy(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, dark: i32, lit_only: bool) -> Option<UnitId> {
    let u = cx.zone.unit(id)?;
    let (pos, faction) = (u.pos, u.faction);
    let mut best: Option<UnitId> = None;
    let reach = aggro_reach(def, u.strength, None, dark);
    let mut best_d = reach;
    let here = cx.zone.id;
    let clock = cx.world.clock;
    if faction == Faction::Friendly {
        let mut near = std::mem::take(&mut cx.scratch.near);
        query_near(cx.rt, pos, reach + crate::combat::bounds(u) + i64::from(max_bounds()), &mut near);
        for &oid in &near {
            if seen(cx, u, oid, &mut best_d, lit_only, clock) && best.is_none_or(|b| best_d > 0 || oid < b) {
                best = Some(oid);
            }
        }
        cx.scratch.near = near;
        return best;
    }
    for (seat, body) in cx.party.bodies.iter().enumerate() {
        let Some((z, oid, _)) = *body else { continue };
        if z != here || cx.world.players.get(seat).is_some_and(|p| p.god) {
            continue;
        }
        let Some(o) = cx.zone.unit(oid) else { continue };
        let mine = aggro_reach(def, u.strength, Some(u32::from(o.strength) + u32::from(o.spirit)), dark);
        let mut within = if best.is_some() { mine.min(best_d) } else { mine };
        if seen(cx, u, oid, &mut within, lit_only, clock) {
            best_d = within;
            best = Some(oid);
        }
    }
    best
}

/// Would `u` notice `oid` within `*best_d`? If so, `*best_d` becomes its distance.
fn seen(cx: &Ctx<'_>, u: &Unit, oid: UnitId, best_d: &mut i64, lit_only: bool, clock: u32) -> bool {
    if oid == u.id {
        return false;
    }
    let Some(o) = cx.zone.unit(oid) else { return false };
    if !o.alive || !o.awake || o.hidden || o.controller == Controller::Npc || !is_enemy(u.faction, o.faction) {
        return false;
    }
    let d = metres_between(u, o);
    if d > *best_d {
        return false;
    }
    if lit_only && !lit_at(cx.zone, cx.rt, clock, o.pos, false) {
        return false;
    }
    if !line_of_sight(&cx.rt.grid, u.pos, o.pos) {
        return false;
    }
    *best_d = d;
    true
}

/// The ecology's `hunts` (§4.6.c): the nearest present unit of a hunted row within `reach`
/// (between bodies) and in sight; ties to the lower id.
fn hunted(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, reach: i64) -> Option<UnitId> {
    if def.hunts.is_empty() {
        return None;
    }
    let u = cx.zone.unit(id)?;
    let mut near = std::mem::take(&mut cx.scratch.near);
    query_near(cx.rt, u.pos, reach + crate::combat::bounds(u) + i64::from(max_bounds()), &mut near);
    let mut best: Option<(i64, UnitId)> = None;
    for &oid in &near {
        let Some(o) = cx.zone.unit(oid) else { continue };
        if oid == id || !o.alive || o.hidden || !def.hunts.contains(&o.def) {
            continue;
        }
        let d = metres_between(u, o);
        if d > reach || best.is_some_and(|b| (d, oid) >= b) || !line_of_sight(&cx.rt.grid, u.pos, o.pos) {
            continue;
        }
        best = Some((d, oid));
    }
    cx.scratch.near = near;
    best.map(|b| b.1)
}

/// The ecology's `flees` (§4.6.c): with a unit of a fled row inside its leash (centre to
/// centre), it walks its leash away from the nearest one, at a run. Returns whether it fled.
/// Its home is unchanged, so a leash takes it back; nothing is remembered of the fright.
pub fn flee(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, shy: bool) -> bool {
    if def.flees.is_empty() || def.leash.0 <= 0 {
        return false;
    }
    let Some(u) = cx.zone.unit(id) else { return false };
    let (pos, facing) = (u.pos, u.facing);
    let reach = i64::from(def.leash.0);
    let mut near = std::mem::take(&mut cx.scratch.near);
    query_near(cx.rt, pos, reach, &mut near);
    let mut threat: Option<(i64, UnitId, Vec2)> = None;
    for &oid in &near {
        let Some(o) = cx.zone.unit(oid) else { continue };
        if oid == id || !o.alive || o.hidden || !def.flees.contains(&o.def) {
            continue;
        }
        let d = dist_sq(pos, o.pos);
        if d > reach * reach || threat.is_some_and(|t| (d, oid) >= (t.0, t.1)) {
            continue;
        }
        threat = Some((d, oid, o.pos));
    }
    cx.scratch.near = near;
    let Some((_, _, from)) = threat else { return false };
    let away = if from == pos { crate::units::facing_angle(facing) } else { bearing(from, pos) };
    let to = from + along(away, def.leash);
    let (w, h) = (cx.rt.grid.w() as i32, cx.rt.grid.h() as i32);
    let (gx, gy) = (to.x.cell().clamp(0, w - 1), to.y.cell().clamp(0, h - 1));
    let Some((fx, fy)) = cx.rt.grid.nearest_free(gx, gy, 4, None) else { return true };
    let speed = if def.run.0 > 0 { def.run } else { def.walk };
    follow_to(cx, id, Vec2::centre(fx, fy), speed, cells_of(reach * 2), shy);
    true
}

/// A row that cannot resist its bait (2020's burial snakes and poisoned rat meat): the first drop
/// of it within its nose (twice its aggro, `BAIT_NOSE_TIMES`) and in sight, walked to at a walk
/// (never under half a pixel a tick), even with a fight on (DUNGEONS.md §3.5), and
/// eaten within 16 px, which kills it at this step's flush. Returns whether it went for one.
fn seek_bait(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, item: jane_core::ItemId) -> bool {
    let Some(u) = cx.zone.unit(id) else { return false };
    let pos = u.pos;
    let reach = i64::from(def.aggro.0) * crate::tuning::BAIT_NOSE_TIMES;
    for i in 0..cx.zone.drops.len() {
        let d = cx.zone.drops[i];
        if d.item != item {
            continue;
        }
        let dd = dist_sq(pos, d.pos);
        if dd > reach * reach || !line_of_sight(&cx.rt.grid, pos, d.pos) {
            continue;
        }
        if dd <= i64::from(BAIT_EAT_FX) * i64::from(BAIT_EAT_FX) {
            cx.zone.drops.remove(i);
            queue_hit(
                cx,
                Hit { to: id, amount: BAIT_HIT, school: School::Nature, from: None, crit: false, status: None },
            );
            return true;
        }
        let speed = Fx(def.walk.0.max(Fx::from_px(1).0 / 2));
        follow_to(cx, id, d.pos, speed, cells_of(reach * 2), false);
        return true;
    }
    false
}

/// Walk the patrol at `speed`, standing at each point for its dwell (a butterfly on its
/// flower, the Caretaker at a door). Home follows the patrol, so a leash returns it to the route.
pub fn patrol(cx: &mut Ctx<'_>, id: UnitId, speed: Fx, shy: bool) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    let Some(p) = u.patrol.as_deref() else { return };
    if p.points.len() < 2 || speed.0 <= 0 || now <= u.dwell_until {
        return;
    }
    let n = p.points.len();
    let at = usize::from(u.patrol_at) % n;
    let (to, dwell) = p.points[at];
    if dist_sq(u.pos, to) <= i64::from(PATROL_REACHED_FX) * i64::from(PATROL_REACHED_FX) {
        u.dwell_until = now.after(dwell);
        u.patrol_at = ((at + 1) % n) as u16;
        clear_path(u);
        return;
    }
    follow_to(cx, id, to, speed, PATROL_PATH_CELLS, shy);
    let u = cx.zone.unit_mut(id).expect("unit");
    u.home = u.pos;
}

// --- walking ----------------------------------------------------------------------------------

/// The path as the TS saw it: `None` for no box and for a box that was let go.
pub fn live_path(u: &Unit) -> Option<&PathCache> {
    u.path.as_deref().filter(|p| p.goal != NO_GOAL)
}

/// Let go of the path, keeping its box (and the box's buffer) for the next one.
pub fn clear_path(u: &mut Unit) {
    if let Some(p) = u.path.as_deref_mut() {
        p.cells.clear();
        p.at = 0;
        p.goal = NO_GOAL;
        p.repath_at = Tick::ZERO;
    }
}

/// Face a point by the dominant axis (`units.ts facePoint`).
pub fn face_point(u: &mut Unit, at: Vec2) {
    face_vector(u, i64::from(at.x.0 - u.pos.x.0), i64::from(at.y.0 - u.pos.y.0));
}

/// Whole cells in a length (a path's reach), never negative.
fn cells_of(fx: i64) -> u32 {
    (fx.max(0) / i64::from(CELL_FX)).min(i64::from(u32::MAX / 16)) as u32
}

/// Walk unit `id` toward `goal` at `speed` (slowed by its statuses) along a cached cell path,
/// planning at most `max_cells` of path (`ai.ts followTo`). Re-plans when the path is used up,
/// when the goal wandered, or when its `repath_at` comes; at most `PATHS_PER_TICK` searches run
/// in a zone per tick and the rest wait a tick. Returns false only when a search ran and found
/// nothing; that is remembered as an empty path toward the goal, and the walker stands until
/// its re-plan comes or the goal moves (the TS searched again every tick it was let, which with
/// sixty things round her spent the zone's searches on the ones that could not get through).
///
/// `shy`: it shuns warm light. Its searches go round lit cells ([`LitField`]), and one that
/// cannot reach the goal ends at the nearest dark cell, where it stands until the next re-plan
/// (a path that is used up is not stale for it, or it would search every tick it waited).
pub fn follow_to(cx: &mut Ctx<'_>, id: UnitId, goal: Vec2, speed: Fx, max_cells: u32, shy: bool) -> bool {
    let now = cx.world.tick;
    let clock = cx.world.clock;
    let Some(ix) = cx.zone.unit_ix(id) else { return false };
    let w = cx.rt.grid.w();
    let (gx, gy) = goal.cell();
    let goal_cell = if cx.rt.grid.inside(gx, gy) { cx.rt.grid.ix(gx, gy) } else { NO_GOAL };
    let u = &cx.zone.units[ix];
    let (used, moved, due) = match live_path(u) {
        None => (true, true, true),
        // An empty path toward a real goal is a search that found nothing (or a walker already
        // there): it waits for its re-plan or a new goal, rather than searching every tick.
        Some(p) => (usize::from(p.at) >= p.cells.len() && !p.cells.is_empty(), p.goal != goal_cell, now >= p.repath_at),
    };
    let stale = if shy { live_path(u).is_none() || moved } else { used || moved };
    if (stale || due) && cx.rt.take_path_search() {
        let (sx, sy) = u.pos.cell();
        let ask = PathAsk::new((sx, sy), (gx, gy), cost_of_cells(max_cells));
        let s = &mut *cx.scratch;
        if shy {
            let half = (PATH_WINDOW >> 1) as i32;
            s.lights.gather(cx.zone, cx.rt, clock, (sx - half, sy - half), (sx + half, sy + half), true);
        }
        let lights = &s.lights;
        let found =
            s.path.find_shunning(&cx.rt.grid, ask, shy && !lights.is_empty(), |x, y| lights.cell_lit(x, y)).is_some();
        let u = &mut cx.zone.units[ix];
        if !found {
            // Remembered as an empty path toward this goal, so the walker asks again when its
            // re-plan comes or the goal moves, not every tick (the TS asked every tick it could).
            // Asked again toward the same goal and found nothing again, it waits twice as long
            // each time (an empty path's `at` counts the failures), up to `REPATH_BACKOFF_MAX`
            // doublings: the dog's order to a mark it cannot reach searched its whole budget
            // every third of a second.
            let again = live_path(u).is_some_and(|p| p.cells.is_empty() && p.goal == goal_cell);
            let fails = if again { live_path(u).map_or(0, |p| p.at).saturating_add(1) } else { 1 };
            clear_path(u);
            let wait = Tick(REPATH_TICKS << u32::from(fails.min(REPATH_BACKOFF_MAX)));
            let repath_at = now.after(wait);
            if let Some(p) = u.path.as_deref_mut() {
                p.goal = goal_cell;
                p.at = fails;
                p.repath_at = repath_at;
            } else {
                u.path = Some(Box::new(PathCache { cells: Vec::new(), at: fails, goal: goal_cell, repath_at }));
            }
            return false;
        }
        let cells = s.path.out.iter().map(|&(x, y)| CellIx(y as u32 * w + x as u32));
        // A path that stops short of a fixed goal (partial: it was out of the search's reach)
        // is walked to its end before it is planned again; `due` would re-run the same whole
        // search every third of a second for the same answer.
        let partial = s.path.out.last().is_none_or(|&(x, y)| (x, y) != (gx, gy));
        let repath_at = now.after(Tick(if partial { REPATH_TICKS << REPATH_BACKOFF_MAX } else { REPATH_TICKS }));
        match u.path.as_deref_mut() {
            Some(p) => {
                p.cells.clear();
                p.cells.extend(cells);
                p.at = 0;
                p.goal = goal_cell;
                p.repath_at = repath_at;
            }
            None => {
                u.path = Some(Box::new(PathCache { cells: cells.collect(), at: 0, goal: goal_cell, repath_at }));
            }
        }
    }
    let u = &cx.zone.units[ix];
    if live_path(u).is_none_or(|p| usize::from(p.at) >= p.cells.len()) {
        return true;
    }
    let mut budget = i64::from(speed.0) * i64::from(speed_factor(u, now)) / 1000;
    while budget > 0 {
        let u = &cx.zone.units[ix];
        let Some(p) = live_path(u) else { break };
        let Some(&c) = p.cells.get(usize::from(p.at)) else { break };
        let last = usize::from(p.at) + 1 >= p.cells.len();
        let (x, y) = ((c.0 % w) as i32, (c.0 / w) as i32);
        // Held by someone else. If it is the goal, that is the target's own feet: stop beside it.
        // Otherwise someone stepped in since planning: wait, and plan again soon.
        let taken = !cx.rt.grid.free(x, y, Some(u.pos.cell()));
        // A lamp came on across its way since it planned: stop short, and think again soon.
        let lit = !taken && shy && lit_at(cx.zone, cx.rt, clock, Vec2::centre(x, y), true);
        if taken || lit {
            if lit || !last {
                let soon = now.after(REPATH_SOON);
                if let Some(p) = cx.zone.units[ix].path.as_deref_mut() {
                    p.repath_at = p.repath_at.min(soon);
                }
            }
            break;
        }
        let to = Vec2::centre(x, y);
        let u = &mut cx.zone.units[ix];
        let d = i64::from(isqrt(dist_sq(u.pos, to) as u64));
        if d <= budget {
            let (dx, dy) = (to.x - u.pos.x, to.y - u.pos.y);
            move_unit(cx.rt, u, dx, dy);
            budget -= d;
            if let Some(p) = u.path.as_deref_mut() {
                p.at += 1;
            }
        } else {
            let dx = i64::from(to.x.0 - u.pos.x.0) * budget / d;
            let dy = i64::from(to.y.0 - u.pos.y.0) * budget / d;
            face_vector(u, dx, dy);
            move_unit(cx.rt, u, Fx(dx as i32), Fx(dy as i32));
            budget = 0;
        }
    }
    true
}

// --- light for a search -------------------------------------------------------------------------

/// The lights near one path search, gathered once, so the search can ask about thousands of
/// cells for a short loop each (`light.ts LitField`). Scratch: reused, never state.
#[derive(Debug, Default)]
pub struct LitField {
    /// Each light's middle and how far it counts as lit, squared.
    lights: Vec<(Vec2, i64)>,
}

impl LitField {
    /// Collect every showing light that could reach into the inclusive cell rect.
    pub fn gather(
        &mut self,
        zone: &ZoneState,
        rt: &ZoneRuntime,
        clock: u32,
        (x0, y0): (i32, i32),
        (x1, y1): (i32, i32),
        warm_only: bool,
    ) {
        self.lights.clear();
        let reach = max_light_radius();
        if reach.0 <= 0 {
            return;
        }
        let cat = jane_data::catalog();
        let pad = reach.0.div_euclid(CELL_FX) + 1;
        let lights = &mut self.lights;
        rt.props.any_in(x0 - pad, y0 - pad, x1 + pad, y1 + pad, |ix| {
            let p = &zone.props[ix as usize];
            let def = cat.story.prop(p.def);
            if let Some(l) = crate::light::light_showing(def, p, clock, crate::light::prop_wetness(zone, rt, p)) {
                if !(warm_only && l.cold) {
                    lights.push((prop_centre(def, p), reach_sq(l.radius)));
                }
            }
            false
        });
    }

    pub fn is_empty(&self) -> bool {
        self.lights.is_empty()
    }

    /// Is the middle of this cell lit by anything gathered?
    pub fn cell_lit(&self, x: i32, y: i32) -> bool {
        let c = Vec2::centre(x, y);
        self.lights.iter().any(|&(at, r)| dist_sq(at, c) <= r)
    }
}
