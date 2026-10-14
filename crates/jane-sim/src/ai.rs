//! Step 7, the controllers (`sim/ai.ts`; ARCHITECTURE.md §4.2 step 7). AI is an MMO loop, not a
//! chase script (2020's `scr_ai_logic`):
//!
//! | State | What it does |
//! | --- | --- |
//! | idle | look about every [`AGGRO_PERIOD`] ticks, staggered by the id, for someone in reach and in sight; else the ecology rows (`hunts`, `flees`, §4.6.c); else its bait; else its patrol |
//! | combat | drop a target that is gone or no enemy; leash when too far from home, when the target leaves the light a `sight: lit` row needs, or when it stands in the warm light a `shuns_light` row cannot; else the first spell of its book it can afford, walking in on `TooFar` or `NotInLos`, standing and facing the fight on anything else |
//! | leash | let go, run home (regen is step 5's), then idle; a home it cannot reach becomes wherever it stands |
//! | evade | pulled past its leash, or (in a dungeon, where nothing but a boss has a leash) her or itself at sanctuary's edge, a rest room or a way out (not a boss): as a leash, but whole again the tick it lets go, at half again its run, and nothing lands on it until it is home (WoW's evade, PLAN.md §2.6 *Leash*) |
//!
//! The book's order is the priority, and the AI casts through the player's own
//! [`try_cast`](crate::combat::try_cast). Three rows change the loop and none is a class: an
//! **order** (`Send`: it walks there and minds nothing else, [`crate::npc`]), **`sight: lit`**
//! (it only notices, and only keeps, a target that stands in a prop's light) and
//! **`shuns_light`** (it will not step into warm light: it walks to the edge and waits). And one
//! thing that is not a row at all: the night ([`night_reach`]), which lengthens its reach. How
//! far it notices her is [`aggro_reach`]: its row's aggro, shorter the more she has grown past
//! its phase and a little longer while she is behind it, half again in the county's dark, never
//! off the screen (WoW's rule scaled to the view; PLAN.md §2.6 *Aggro*). By day
//! nothing is wary: a hostile comes for her on sight within its aggro, day or night, county or
//! dungeon (the owner, 2026-09-29; PLAN.md §2.6 *Day*). What leaves her be is a row with no
//! aggro (the county's rabbits, sheep, hens and its people), not an hour.
//!
//! **Walking** is [`follow_to`]: windowed A* ([`crate::path`]) along a cached cell path, re-planned
//! when it is used up, when the goal moved, or every `REPATH_TICKS`. A cell someone stepped into
//! since is waited at. The zone's searches share a hard bound of nodes a tick
//! ([`PATH_TICK_BUDGET`](crate::path::PATH_TICK_BUDGET)): none opens more than is left of it, and a
//! walker it is spent on, or whose search it cut short, keeps its old path and is asked first the
//! next tick ([`step_controllers`]).
//!
//! **A chaser** ([`chase`], [`approach`]) plans on a real change and not on every cell she steps
//! to (MAP.md §3.5; [`LEAN_CHASE`]). It keeps its path while she is within [`CHASE_KEEP_CELLS`] of
//! its end and in sight from there, and lengthens it by the step or two between ([`keep_path`]);
//! close in ([`CHASE_CLOSE_CELLS`]), on her ground and in sight, it steps straight at her with no
//! path at all ([`close_in`]). Otherwise it plans at most every [`CHASE_PLAN_EVERY`] ticks, on
//! the ticks `(tick + id) % 4 == 0`; at once, with no wait for its tick, when its next step has
//! gone solid, when she has changed level or deck, when sight to her breaks across a change of
//! level, or when it has been thrown far from its path. A timer (a trail's refresh, the net for a
//! walker that is stuck, a step a body has held a while) only looks for a way no dearer than the
//! path it walks, or round a body three times as dear and a dozen cells more, and keeps that path
//! if there is none: a search that finds nothing is not what a working path is traded for (and a
//! path no more than three cells over the straight way is not looked at: there is no shortcut to
//! find). A search that does find nothing
//! when it was wanted leaves it standing for a wait that doubles. Nothing here is stored but what a
//! path already holds (its cells, the cell it was planned toward, and when to plan again), and the
//! zone's queue of the walkers a tick's budget could not serve.
//!
//! **Target choice** asks the party snapshot (`PartySnap`, §4.2) who the party is, and reads
//! their bodies as they stand. Something that bites has only the party's side for enemies, and
//! of that side only the ones it fights (never an npc), so it looks at the party's bodies rather
//! than every unit of a county of several thousand.
//!
//! **A path that is let go** keeps its box: [`clear_path`] empties it and marks it goal-less, so
//! a chase that stops and starts allocates nothing once warm. [`live_path`] is the path as the
//! TS saw it (`null` for both `None` and a cleared one).

use alloc::boxed::Box;
use alloc::vec::Vec;

use jane_core::action::School;
use jane_core::angle::{along, bearing};
use jane_core::grid::DIRS8;
use jane_core::num::{CELL_FX, dist_sq, isqrt, octile10};
use jane_core::tile::{BLOCK_MOVE, F_OCC};
use jane_core::{CellIx, Fx, SpellId, Tick, Vec2};
use jane_data::{Controller, Faction, UnitDef, UnitSight};

use crate::combat::{Hit, distance, is_enemy, max_bounds, metres_between, query_near, queue_hit};
use crate::ctx::{Ctx, unit_mut_or_skip, unit_or_skip};
use crate::event::SpellError;
use crate::grid::ZoneGrid;
use crate::ids::UnitId;
use crate::light::{lit_at, max_light_radius, prop_centre, reach_sq};
use crate::los::line_of_sight;
use crate::path::{PATH_TICK_BUDGET, PATH_WINDOW, PathAsk, REPATH_BACKOFF_MAX, REPATH_TICKS, cost_of_cells};
use crate::runtime::ZoneRuntime;
use crate::state::{CombatState, PathCache, Unit, ZoneState};
use crate::status::{is_stunned, speed_factor};
use crate::tuning::{
    AGGRO_FLOOR_FX, AGGRO_MAX_FX, AGGRO_PAR, AGGRO_PERIOD, BAIT_EAT_FX, BAIT_HIT, CHASE_CLOSE_CELLS, CHASE_KEEP_CELLS,
    CHASE_PATH_TIMES, CHASE_PLAN_EVERY, CHASE_TRIM_CELLS, EVADE_RUN, HOLD_FOOT, HOLD_PATH_PLUS, HOLD_PATH_TIMES,
    INDOOR_LEASH, LEAN_CHASE, LEASH_PATH_TIMES, LEASH_SNAP_FX, LEDGE_FACE_MAX, LEDGE_PATH_EXTRA, NIGHT_AGGRO,
    NIGHT_LEASH, PATH_REACH_FX, PATROL_PATH_CELLS, PATROL_REACHED_FX, REPATH_SOON, ROOTED_REACH_FX, WORKS_SCALE,
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
    let mut ids = core::mem::take(&mut cx.scratch.units);
    ids.clear();
    if everyone {
        ids.extend(cx.zone.units.iter().filter(|u| u.awake).map(|u| u.id));
    } else {
        ids.extend_from_slice(&cx.rt.awake_units);
    }
    if LEAN_CHASE {
        lead_with_refused(cx, &mut ids);
        cx.scratch.sight.arm();
    }
    for &id in &ids {
        let Some(u) = cx.zone.unit(id) else { continue };
        // Frozen by a blow's hitlag, it waits with the stunned; hopping a ledge, it is in the air.
        if !u.alive || u.hidden || is_stunned(u, now) || crate::feel::lagged(u, now) || crate::height::hopping(u) {
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
    cx.scratch.sight.disarm();
    cx.scratch.units = ids;
}

/// The walkers a path search was refused at the last tick lead this one's pass, in the order they
/// were refused: the budget is theirs first, and none waits behind the ones that came first by id
/// for ever (a round robin by id, with the refused as its turn). The zone's queue is state
/// (`ZoneState::path_queue`, saved) and so identical on a loaded game; it is emptied here and
/// refilled by this pass's refusals ([`refuse`]). `ids` is ascending and stays so but for the
/// refused at its front. Their own list lasts the pass in `scratch.retry`, which a walker with no
/// path to wait on reads to know it is one of them ([`chase_wants`]).
fn lead_with_refused(cx: &mut Ctx<'_>, ids: &mut [UnitId]) {
    cx.scratch.retry.clear();
    if cx.zone.path_queue.is_empty() {
        return;
    }
    cx.scratch.retry.append(&mut cx.zone.path_queue);
    let mut led = 0;
    for &id in &cx.scratch.retry {
        if let Ok(at) = ids[led..].binary_search(&id) {
            ids[led..=led + at].rotate_right(1);
            led += 1;
        }
    }
}

/// A search was asked for and the tick's nodes are spent: the walker keeps its path and is asked
/// first at the next tick's pass.
fn refuse(cx: &mut Ctx<'_>, id: UnitId) {
    cx.scratch.path.stats.refused += 1;
    if !cx.zone.path_queue.contains(&id) {
        cx.zone.path_queue.push(id);
    }
}

/// May this walker search now? If not, it is queued ([`refuse`]).
fn may_search(cx: &mut Ctx<'_>, id: UnitId) -> bool {
    if cx.rt.take_path_search() {
        return true;
    }
    if LEAN_CHASE {
        refuse(cx, id);
    }
    false
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
        // An evade runs home at half again its run (PLAN.md §2.6 *Leash*).
        CombatState::Evade => leash(cx, id, def, Fx(run.0 * EVADE_RUN / 100), shy),
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
            let u = unit_mut_or_skip!(cx, id, "ai::idle");
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

pub(crate) fn leash(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, run: Fx, shy: bool) {
    // A thing that never runs (the Burial's small snakes: they spit, they do not chase) walks
    // home. At a run of nothing it stood leashing where the fight left it for good, and a thing
    // leashing takes no bait.
    let run = if run.0 > 0 { run } else { def.walk };
    let clock = cx.world.clock;
    let u = unit_mut_or_skip!(cx, id, "ai::leash");
    u.target = None;
    end_hold(u);
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
    let round = shy && !lit_at(cx.zone, cx.rt, clock, pos, Some(cx.world.night.stage));
    let cells = cells_of(path_reach(def, i64::from(def.leash.0)) * i64::from(LEASH_PATH_TIMES));
    let found = follow(cx, id, def, home, run, cells, round);
    let u = unit_mut_or_skip!(cx, id, "ai::leash");
    let waiting = round && live_path(u).is_some_and(|p| usize::from(p.at) >= p.cells.len());
    if !found || waiting {
        // It cannot get home (a door shut behind it, or its post is lit now): it stands guard here.
        u.home = u.pos;
        u.combat = CombatState::Idle;
    }
}

pub(crate) fn fight(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, run: Fx, shy: bool) {
    let now = cx.world.tick;
    let clock = cx.world.clock;
    let u = unit_or_skip!(cx, id, "ai::fight");
    let target = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.alive && is_enemy(u.faction, t.faction));
    let Some(tpos) = target.map(|t| t.pos) else {
        let u = unit_mut_or_skip!(cx, id, "ai::fight");
        u.target = None;
        u.combat = CombatState::Leash;
        return;
    };
    // How far it follows her from its post. In the dark it is much further, and it shortens
    // again the moment the chase reaches a lamp: a thing runs you to the light and turns back.
    let dark = night_reach(cx, id, def);
    let leash = leash_in(cx.zone.id, def) * i64::from(10 + NIGHT_LEASH * dark) / 10;
    let u = unit_or_skip!(cx, id, "ai::fight");
    let (pos, home) = (u.pos, u.home);
    // A rooted thing (a flower, a cactus: no feet) never leaves its post, so no chase takes it
    // past its leash: it lets go once she is half again [`ROOTED_REACH_FX`] from it (or its own
    // aggro, if longer), or it would hold her in its fight from across the zone for good. Not its
    // own shorter aggro (2026-09-30): she shoots it from bolt range, on the screen, and a thing
    // that let go there stood idle and mended whole between her bolts.
    let rooted_off = run.0 <= 0 && distance(tpos, home) > i64::from(def.aggro.0.max(ROOTED_REACH_FX)) * 3 / 2;
    // Shut in with her by a lock-in, it fights it out: the gate is the leash (PLAN.md §2.6).
    let shut = crate::triggers::shut_in(cx, home.cell());
    // In a dungeon nothing but a boss has a leash: it follows her until it or she is dead. What
    // it will not do is follow her into sanctuary (a rest room, the threshold of a way out), or
    // step into one itself: there it evades home, as at a WoW instance's edge (PLAN.md §2.6).
    let endless = chases_to_the_end(cx.zone.id, def);
    let barred = !def.boss
        && !shut
        && (cx.rt.in_sanctuary(tpos.cell()) || cx.rt.in_sanctuary(pos.cell()) && !cx.rt.in_sanctuary(home.cell()));
    let too_far = ((!endless && distance(pos, home) > leash) || rooted_off || barred) && !shut;
    // Light is how a sentry sees: a target that steps into the dark is a target it no longer has.
    let unseen = !too_far && def.sight == UnitSight::Lit && !lit_at(cx.zone, cx.rt, clock, tpos, None);
    // Warm light keeps a shade off: standing in it, it does nothing but leave.
    let scorched = !too_far && !unseen && shy && lit_at(cx.zone, cx.rt, clock, pos, Some(cx.world.night.stage));
    if too_far && !def.boss {
        evade(cx, id, def);
        return;
    }
    if too_far || unseen || scorched {
        let u = unit_mut_or_skip!(cx, id, "ai::fight");
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
    let u = unit_or_skip!(cx, id, "ai::fight");
    // On a ladder nothing is swung or cast (MAP.md §2.3): it climbs on toward her.
    if crate::height::cannot_strike(&cx.rt.grid, u) {
        approach(cx, id, def, tpos, run, path_reach(def, leash), shy);
        return;
    }
    let Some(spell) = pick_spell(u, now) else {
        approach(cx, id, def, tpos, run, path_reach(def, leash), shy);
        return;
    };
    match crate::feel::cast_or_windup(cx, id, spell) {
        Ok(()) => {
            let u = unit_mut_or_skip!(cx, id, "ai::fight");
            clear_path(u);
            face_point(u, tpos);
        }
        Err(SpellError::TooFar | SpellError::NotInLos) => {
            approach(cx, id, def, tpos, run, path_reach(def, leash), shy);
        }
        // On cooldown, on the GCD, short of mana: hold position and keep facing the fight.
        Err(_) => {
            if let Some(u) = cx.zone.unit_mut(id) {
                face_point(u, tpos);
            }
        }
    }
}

/// Pulled past its leash: WoW's evade (PLAN.md §2.6 *Leash*). It lets her go, sheds what is on
/// it, mends whole (a row that keeps its wounds keeps them) and runs home, and while it runs
/// nothing lands on it (`flush`). A boss never evades: its arena is its leash, and a lock-in
/// has a gate for one.
fn evade(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef) {
    crate::status::clear_statuses(cx, id);
    let u = unit_mut_or_skip!(cx, id, "ai::evade");
    u.target = None;
    u.combat = CombatState::Evade;
    end_hold(u);
    if !def.keeps_wounds {
        crate::life::mend_whole(u);
    }
    clear_path(u);
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
fn approach(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, tpos: Vec2, speed: Fx, leash: i64, shy: bool) {
    if speed.0 <= 0 {
        if let Some(u) = cx.zone.unit_mut(id) {
            face_point(u, tpos);
        }
        return;
    }
    // Close in, on her ground and in sight, it needs no path.
    if LEAN_CHASE && !shy && close_in(cx, id, tpos, speed) {
        return;
    }
    let cells = cells_of(leash * i64::from(CHASE_PATH_TIMES));
    if cx.rt.grid.has_levels() && hold_below(cx, id, def, tpos, speed, cells, shy) {
        return;
    }
    chase(cx, id, def, tpos, speed, cells, shy);
    // At the edge of her light with nowhere nearer to stand: it waits, and it watches her.
    let u = unit_mut_or_skip!(cx, id, "ai::approach");
    if shy && live_path(u).is_some_and(|p| usize::from(p.at) >= p.cells.len()) {
        face_point(u, tpos);
    }
}

/// She is on other ground (MAP.md §3.4): above it on a plateau whose stair is round the back,
/// or below a ledge it may not hop. If the way to her is one worth taking (no more than
/// [`HOLD_PATH_TIMES`] the straight distance plus [`HOLD_PATH_PLUS`] cells, a ledge's hop
/// counted short) it takes it; if there is none, or only the long way round, it does not
/// wall-hug: it holds at the foot below her (straight at her, flush against what is between),
/// watching her, for [`HOLD_FOOT`], and then evades home whole as a leash does, so shooting it
/// from a cliff it cannot climb earns nothing. Until when it holds is its `foot_until`.
/// Returns whether it held (or began to), when [`approach`] does nothing more.
fn hold_below(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, tpos: Vec2, speed: Fx, cells: u32, shy: bool) -> bool {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit(id) else { return true };
    let g = &cx.rt.grid;
    let theirs = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.pos == tpos).and_then(|t| t.on_span);
    if crate::span::unit_level(g, u) == crate::span::level_on(g, tpos, theirs) {
        if u.foot_until != Tick::ZERO {
            end_hold(cx.zone.unit_mut(id).expect("unit"));
        }
        return false;
    }
    if u.foot_until == Tick::ZERO {
        let found = chase(cx, id, def, tpos, speed, cells, shy);
        let (planned, cell) = goal_of(cx, id, tpos, LEAN_CHASE && !shy);
        let u = cx.zone.unit_mut(id).expect("unit");
        // Judged only on a search toward her (the zone's searches may all be spent this tick).
        let Some(p) = live_path(u).filter(|p| p.goal == planned) else { return true };
        let left = p.cells.len().saturating_sub(usize::from(p.at)) as i64;
        let worth = found
            && p.cells.last() == Some(&cell)
            && left <= HOLD_PATH_TIMES * crate::height::cells_apart(u.pos, tpos) + HOLD_PATH_PLUS;
        if !worth {
            u.foot_until = now.after(HOLD_FOOT);
            clear_path(u);
        }
        return true;
    }
    let u = cx.zone.unit_mut(id).expect("unit");
    if now >= u.foot_until {
        evade(cx, id, def);
        return true;
    }
    if distance(u.pos, tpos) > i64::from(CELL_FX) {
        let d = along(bearing(u.pos, tpos), speed);
        crate::units::press_unit(cx.rt, u, d.x, d.y);
    }
    face_point(u, tpos);
    true
}

/// Out of a hold at the foot (`hold_below`).
fn end_hold(u: &mut Unit) {
    u.foot_until = Tick::ZERO;
}

// --- looking about ----------------------------------------------------------------------------

/// How much further this thing notices, and follows, because it is dark where it stands: 0 by
/// day, in warm light, or anywhere but the county (a dungeon's dark is its own, and tuned so), 2
/// for a creature of the Works (four times its row's strength or more: the phase table put it
/// down deep), else 1. By day nothing here asks about the light at all.
pub fn night_reach(cx: &Ctx<'_>, id: UnitId, def: &UnitDef) -> i32 {
    if !cx.world.is_night() || cx.zone.id != jane_core::ZoneId::County {
        return 0;
    }
    let Some(u) = cx.zone.unit(id) else { return 0 };
    if lit_at(cx.zone, cx.rt, cx.world.clock, u.pos, Some(cx.world.night.stage)) {
        return 0;
    }
    if u32::from(u.strength) >= u32::from(def.strength) * u32::from(WORKS_SCALE) { 2 } else { 1 }
}

/// Is a body at `at` out in the county's night, out of any warm light? What a night blow asks
/// ([`NIGHT_HIT`](crate::tuning::NIGHT_HIT)): a lamp or a fire is as safe as the day.
pub fn out_in_the_night(cx: &Ctx<'_>, at: jane_core::Vec2) -> bool {
    cx.world.is_night()
        && cx.zone.id == jane_core::ZoneId::County
        && !lit_at(cx.zone, cx.rt, cx.world.clock, at, Some(cx.world.night.stage))
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
/// - in the dark half again ([`NIGHT_AGGRO`]), and never past [`AGGRO_MAX_FX`];
/// - a boss is its arena's: its row, half again in the dark; a row with no aggro is 0.
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
        let mut near = core::mem::take(&mut cx.scratch.near);
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
        // Nothing but a boss notices her in sanctuary (a dungeon's rest room, its way out).
        if !def.boss && cx.rt.in_sanctuary(o.pos.cell()) {
            continue;
        }
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
    if lit_only && !lit_at(cx.zone, cx.rt, clock, o.pos, None) {
        return false;
    }
    if !crate::los::sees(&cx.rt.grid, u, o) {
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
    let mut near = core::mem::take(&mut cx.scratch.near);
    query_near(cx.rt, u.pos, reach + crate::combat::bounds(u) + i64::from(max_bounds()), &mut near);
    let mut best: Option<(i64, UnitId)> = None;
    for &oid in &near {
        let Some(o) = cx.zone.unit(oid) else { continue };
        if oid == id || !o.alive || o.hidden || !def.hunts.contains(&o.def) {
            continue;
        }
        let d = metres_between(u, o);
        if d > reach || best.is_some_and(|b| (d, oid) >= b) || !crate::los::sees(&cx.rt.grid, u, o) {
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
    let reach = leash_in(cx.zone.id, def);
    let mut near = core::mem::take(&mut cx.scratch.near);
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
    let to = from + along(away, Fx(reach as i32));
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
    let u = unit_mut_or_skip!(cx, id, "ai::patrol");
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

/// Is `zone` a generated dungeon (a mission's), not the county or a building?
pub fn is_dungeon(zone: jane_core::ZoneId) -> bool {
    jane_data::catalog().dungeons.mission_of(zone).is_some()
}

/// Does this row, in `zone`, chase her until it or she is dead? Everything but a boss in a
/// dungeon (the owner, 2026-10-07: WoW's instances have no leash). It still never follows her
/// into sanctuary (`ZoneRuntime::in_sanctuary`), and a boss keeps its arena's row.
pub fn chases_to_the_end(zone: jane_core::ZoneId, def: &UnitDef) -> bool {
    !def.boss && is_dungeon(zone)
}

/// How far a row follows her from home, `Fx`, in `zone`: its row's leash in the open county
/// (three and three quarter times its aggro, the owner's playtest of 2026-10-07), and two thirds
/// of it (two and a half times) in a building, whose rooms are small. A boss keeps its arena's
/// row anywhere. In a dungeon nothing else has a leash ([`chases_to_the_end`]): there this is
/// only the room a fight is kept in, what its searches are budgeted from, how far a fleeing
/// thing runs, and the tether a bot kites inside so as not to drag it across the dungeon.
pub fn leash_in(zone: jane_core::ZoneId, def: &UnitDef) -> i64 {
    let row = i64::from(def.leash.0);
    if def.boss || zone == jane_core::ZoneId::County {
        row
    } else {
        row * i64::from(INDOOR_LEASH.0) / i64::from(INDOOR_LEASH.1)
    }
}

/// Whole cells in a length (a path's reach), never negative.
/// What a chase's and a way home's search is budgeted from: the leash, and for anything but a
/// boss never under [`PATH_REACH_FX`]. The leash says how far from home it follows her, not how
/// winding the way back is (2026-10-06: at 17.5 m a dungeon's way home outran its search).
fn path_reach(def: &UnitDef, leash: i64) -> i64 {
    if def.boss { leash } else { leash.max(i64::from(PATH_REACH_FX)) }
}

fn cells_of(fx: i64) -> u32 {
    (fx.max(0) / i64::from(CELL_FX)).min(i64::from(u32::MAX / 16)) as u32
}

/// Walk unit `id` toward `goal` at `speed` (slowed by its statuses) along a cached cell path,
/// planning at most `max_cells` of path (`ai.ts followTo`). Re-plans when the path is used up,
/// when the goal wandered, or when its `repath_at` comes; the zone's searches share a budget of
/// nodes a tick, and a walker refused keeps its path and is asked first next tick. Returns false
/// only when a search ran and found nothing; that is remembered as an empty path toward the goal,
/// and the walker stands until its re-plan comes or the goal moves (the TS searched again every
/// tick it was let, which with sixty things round her spent the zone's searches on the ones that
/// could not get through).
///
/// `shy`: it shuns warm light. Its searches go round lit cells ([`LitField`]), and one that
/// cannot reach the goal ends at the nearest dark cell, where it stands until the next re-plan
/// (a path that is used up is not stale for it, or it would search every tick it waited).
pub fn follow_to(cx: &mut Ctx<'_>, id: UnitId, goal: Vec2, speed: Fx, max_cells: u32, shy: bool) -> bool {
    let Some(u) = cx.zone.unit(id) else { return false };
    follow(cx, id, def_of(u), goal, speed, max_cells, shy)
}

/// [`follow_to`] for a unit of row `def` (a test's row may not be its own). Across height
/// (MAP.md §3.5) an AI's searches hop ledges whose landing lies inside its leash, unless it is a
/// boss, big or a perch row; a perch row's never leave its home's level.
pub(crate) fn follow(
    cx: &mut Ctx<'_>,
    id: UnitId,
    def: &UnitDef,
    goal: Vec2,
    speed: Fx,
    max_cells: u32,
    shy: bool,
) -> bool {
    walk_to(cx, id, def, goal, Gait { speed, max_cells, shy }, Walk::Plain)
}

/// [`follow`] for a chaser after the body at `goal`: it plans on a real change, not on every cell
/// she steps to ([`chase_wants`]).
pub(crate) fn chase(
    cx: &mut Ctx<'_>,
    id: UnitId,
    def: &UnitDef,
    goal: Vec2,
    speed: Fx,
    max_cells: u32,
    shy: bool,
) -> bool {
    walk_to(cx, id, def, goal, Gait { speed, max_cells, shy }, Walk::Chase)
}

/// When a walker plans ([`walk_to`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Walk {
    /// When the path is used up, when the goal's cell moved or every `REPATH_TICKS`: a patrol, a
    /// way home, a flight, a bait, an order.
    Plain,
    /// A chaser's way ([`chase_wants`]); the same as `Plain` unless the chase is lean and the
    /// walker does not shun light.
    Chase,
}

/// How a walker goes.
#[derive(Clone, Copy, Debug)]
struct Gait {
    speed: Fx,
    max_cells: u32,
    shy: bool,
}

/// How long a fresh chase path is walked before it is planned again if she does not move from the
/// cell it was planned toward: two seconds, a safety net for a walker that is stuck or thrown off
/// its path and not a refresh (the path is the shortest there was).
const CHASE_WALK_TICKS: u32 = 6 * REPATH_TICKS;
/// How long a chase path lengthened to her cell by cell (a trail, no longer the shortest way) is
/// kept before it is planned afresh, so a chaser cuts a corner she turned, and does not follow her
/// round a pillar when the other way is shorter: two thirds of a second.
const CHASE_TRAIL_TICKS: u32 = 2 * REPATH_TICKS;
/// Doublings of `REPATH_TICKS` a chaser waits, at most, after searches in a row that found nothing.
const CHASE_BACKOFF_MAX: u16 = 3;
/// How long a chaser waits for a step a body stands in before it looks for a way round.
const CHASE_BLOCKED_TICKS: Tick = Tick(12);
/// A path is as short as it can be if it is no more than this many cells over the straight way to her.
const CHASE_STRAIGHT_SLACK: i64 = 3;
/// What a way round a step held by a body may cost over three times the path it walks, tenths of a
/// cell: twelve cells.
const CHASE_DETOUR: u32 = 120;

fn walk_to(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef, goal: Vec2, gait: Gait, how: Walk) -> bool {
    let Gait { speed, max_cells, shy } = gait;
    let now = cx.world.tick;
    let clock = cx.world.clock;
    let Some(ix) = cx.zone.unit_ix(id) else { return false };
    let w = cx.rt.grid.w();
    let (gx, gy) = goal.cell();
    let lean = LEAN_CHASE && how == Walk::Chase && !shy;
    let (goal_cell, _) = goal_of(cx, id, goal, lean);
    let want = if lean {
        chase_wants(cx, ix, id, goal, goal_cell, max_cells)
    } else {
        let u = &cx.zone.units[ix];
        let (used, moved, due) = match live_path(u) {
            None => (true, true, true),
            // An empty path toward a real goal is a search that found nothing (or a walker already
            // there): it waits for its re-plan or a new goal, rather than searching every tick.
            Some(p) => {
                (usize::from(p.at) >= p.cells.len() && !p.cells.is_empty(), p.goal != goal_cell, now >= p.repath_at)
            }
        };
        let stale = if shy { live_path(u).is_none() || moved } else { used || moved };
        if stale || due { Want::Full } else { Want::No }
    };
    if want != Want::No && may_search(cx, id) {
        let u = &cx.zone.units[ix];
        let (sx, sy) = u.pos.cell();
        let mut ask = PathAsk::new((sx, sy), (gx, gy), cost_of_cells(max_cells));
        if cx.rt.grid.has_spans() {
            // On a deck, or after something on one (MAP.md §2.5): the search knows the layers.
            ask.start_deck = u.on_span;
            ask.goal_deck = deck_goal(cx, id, goal);
        }
        if cx.rt.grid.has_levels() && u.controller == jane_data::Controller::Ai {
            let home = u.home.cell();
            if def.holds_level {
                ask.level = Some(cx.rt.grid.level_at(home.0, home.1));
            } else if crate::height::hops(u, def) {
                let reach = i64::from(cells_of(leash_in(cx.zone.id, def)));
                ask.ledges = Some((home, reach.min(i64::from(i32::MAX)) as i32));
            }
        }
        // A look for a way no dearer than the one it walks, or round a body (three times as dear and
        // a dozen cells more): a short search that finds what is worth finding, or nothing.
        let within = match want {
            Want::Refresh => remaining_cost(cx, ix),
            Want::Round => remaining_cost(cx, ix).map(|c| c.saturating_mul(3).saturating_add(CHASE_DETOUR)),
            _ => None,
        };
        if let Some(cost) = within {
            ask.max_cost = ask.max_cost.min(cost);
        }
        // The tick's nodes are a hard bound: a search opens no more than are left of them.
        let left = cx.rt.path_nodes_left;
        if LEAN_CHASE {
            ask.budget = ask.budget.min(left);
        }
        let opened = cx.scratch.path.stats.expanded;
        let s = &mut *cx.scratch;
        if shy {
            let half = (PATH_WINDOW >> 1) as i32;
            s.lights.gather(
                cx.zone,
                cx.rt,
                clock,
                (sx - half, sy - half),
                (sx + half, sy + half),
                Some(cx.world.night.stage),
            );
        }
        let lights = &s.lights;
        let found =
            s.path.find_shunning(&cx.rt.grid, ask, shy && !lights.is_empty(), |x, y| lights.cell_lit(x, y)).is_some();
        let spent = s.path.stats.expanded - opened;
        cx.rt.spend_path_nodes(spent);
        // Cut short by what was left of the tick's nodes, not by all a search may open: it was
        // refused, not answered. It keeps its path and goes first next tick, with them all.
        let cut = LEAN_CHASE && left < PATH_TICK_BUDGET && spent > u64::from(ask.budget);
        let u = &mut cx.zone.units[ix];
        if cut {
            if !found {
                s.path.stats.failed -= 1;
            }
            s.path.stats.refused += 1;
            if !cx.zone.path_queue.contains(&id) {
                cx.zone.path_queue.push(id);
            }
        } else if !found && within.is_some() {
            // A look for a shorter way, or a way round a body, that found none: it keeps the path it
            // has (a search that finds nothing is not what a working path is replaced by), and looks
            // again when its timer comes.
            s.path.stats.failed -= 1;
            s.path.stats.held += 1;
            if let Some(p) = u.path.as_deref_mut() {
                p.repath_at =
                    now.after(if want == Want::Refresh { Tick(CHASE_WALK_TICKS) } else { CHASE_BLOCKED_TICKS });
            }
        } else if !found {
            // Remembered as an empty path toward this goal, so the walker asks again when its
            // re-plan comes or the goal moves, not every tick (the TS asked every tick it could).
            // Asked again toward the same goal and found nothing again, it waits twice as long
            // each time (an empty path's `at` counts the failures), up to `REPATH_BACKOFF_MAX`
            // doublings: the dog's order to a mark it cannot reach searched its whole budget
            // every third of a second.
            //
            // A chaser's goal is a moving one, so what it counts is the searches in a row that found
            // nothing, toward whatever cell she was on, and it waits at most [`CHASE_BACKOFF_MAX`]
            // doublings (two and two thirds of a second).
            let again = live_path(u).is_some_and(|p| p.cells.is_empty() && (lean || p.goal == goal_cell));
            let fails = if again { live_path(u).map_or(0, |p| p.at).saturating_add(1) } else { 1 };
            clear_path(u);
            let most = if lean { CHASE_BACKOFF_MAX } else { REPATH_BACKOFF_MAX };
            let wait = Tick(REPATH_TICKS << u32::from(fails.min(most)));
            let repath_at = now.after(wait);
            if let Some(p) = u.path.as_deref_mut() {
                p.goal = goal_cell;
                p.at = fails;
                p.repath_at = repath_at;
            } else {
                u.path = Some(Box::new(PathCache { cells: Vec::new(), at: fails, goal: goal_cell, repath_at }));
            }
            return false;
        } else {
            let cells = s.path.out.iter().map(|&(x, y)| CellIx(y as u32 * w + x as u32));
            // A path that stops short of a fixed goal (partial: it was out of the search's reach)
            // is walked to its end before it is planned again; `due` would re-run the same whole
            // search every third of a second for the same answer.
            let partial = s.path.out.last().is_none_or(|&(x, y)| (x, y) != (gx, gy));
            let wait = if partial {
                REPATH_TICKS << REPATH_BACKOFF_MAX
            } else if lean {
                CHASE_WALK_TICKS
            } else {
                REPATH_TICKS
            };
            let repath_at = now.after(Tick(wait));
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
        // Otherwise someone stepped in since planning: wait, and plan again soon. On a deck, or
        // stepping onto one, it is the deck's holder that counts (MAP.md §2.5).
        let deck = crate::span::steps_on_deck(&cx.rt.grid, u.on_span, u.pos.cell(), (x, y));
        let taken = !cx.rt.grid.free_on(x, y, deck, Some(u.pos.cell()));
        // A lamp came on across its way since it planned: stop short, and think again soon.
        let lit = !taken && shy && lit_at(cx.zone, cx.rt, clock, Vec2::centre(x, y), Some(cx.world.night.stage));
        if taken || lit {
            if lit || !last {
                // A step held by a body: the one in it may well move on; a chaser waits longer to see.
                let soon = now.after(if lean { CHASE_BLOCKED_TICKS } else { REPATH_SOON });
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

// --- the chase's planning (MAP.md §3.5) ---------------------------------------------------------

/// The cell a walk toward `goal` is planned toward, with the layer when `lean`: standing on a span's
/// deck (the unit's target at the goal's cell is on one) the index carries [`CellIx::DECK_BIT`], so a
/// path knows the layer it was planned for. And the plain cell, `NO_GOAL` off the grid, for both.
fn goal_of(cx: &Ctx<'_>, id: UnitId, goal: Vec2, lean: bool) -> (CellIx, CellIx) {
    let g = &cx.rt.grid;
    let (gx, gy) = goal.cell();
    if !g.inside(gx, gy) {
        return (NO_GOAL, NO_GOAL);
    }
    let cell = g.ix(gx, gy);
    if lean && g.has_spans() && deck_goal(cx, id, goal) {
        (CellIx(cell.0 | CellIx::DECK_BIT), cell)
    } else {
        (cell, cell)
    }
}

/// Is `goal` the cell of the body this unit is after, and is that body on a span's deck?
fn deck_goal(cx: &Ctx<'_>, id: UnitId, goal: Vec2) -> bool {
    cx.zone
        .unit(id)
        .and_then(|u| u.target)
        .and_then(|t| cx.zone.unit(t))
        .is_some_and(|t| t.on_span.is_some() && t.pos.cell() == goal.cell())
}

/// The layer a planned goal stood on: its level, and whether on a deck.
fn goal_layer(g: &ZoneGrid, goal: CellIx) -> (u8, bool) {
    let deck = goal.0 & CellIx::DECK_BIT != 0;
    let c = goal.0 & !CellIx::DECK_BIT;
    let (x, y) = ((c % g.w()) as i32, (c / g.w()) as i32);
    match g.deck_at(x, y).filter(|_| deck) {
        Some(i) => (g.spans()[usize::from(i)].deck_level, true),
        None => (g.level_at(x, y), false),
    }
}

/// The layer the body this unit is after stands on at `goal` now.
fn layer_now(cx: &Ctx<'_>, u: &Unit, goal: Vec2) -> (u8, bool) {
    let g = &cx.rt.grid;
    let span = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.pos.cell() == goal.cell()).and_then(|t| t.on_span);
    (crate::span::level_on(g, goal, span), span.is_some())
}

/// What a chaser makes of its path this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Judge {
    /// Nothing to plan: walk on.
    Hold,
    /// Plan, on its tick.
    Slot,
    /// Plan now, with no wait for its tick.
    Now,
    /// A search that found nothing is remembered, and she has not left its neighbourhood: note
    /// where she is, plan nothing.
    Slide,
    /// She has moved, or the path is used up: keep it if it reaches near enough to lengthen.
    Keep,
    /// Its timer is up, its way is clear, and its path is as short as a path can be: the timer is
    /// set again, and nothing is asked of a search.
    Idle,
    /// Its timer is up and its way is clear: look for a path no dearer than the one it walks.
    Refresh,
    /// Its timer is up and its next step is held by a body: look for a way round.
    Blocked,
}

/// What a chaser asks of a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Want {
    /// None: it walks on.
    No,
    /// A plan, as far as a search goes: it has no path that serves, or something real has changed.
    Full,
    /// A timer is up: a look for a path no dearer than the one it walks. Finding none, it keeps it.
    Refresh,
    /// A step held by a body has been waited at: a look for a way round it, dearer than the path it
    /// walks by no more than twice as much again and [`CHASE_DETOUR`]. Finding none, it keeps the
    /// path and waits some more.
    Round,
}

/// Does this chaser search this tick? (MAP.md §3.5). It plans on a real change and not on each cell
/// she steps to, and at most every [`CHASE_PLAN_EVERY`] ticks, on the ticks `(tick + id) % 4 == 0`
/// (a walker refused last tick has its turn now). At once, with no wait for its tick: when its
/// next step has gone solid, when it has been thrown far from its path, when she has changed
/// level or deck since it was planned, and when sight to her breaks across a change of level. A
/// path she has not left (within [`CHASE_KEEP_CELLS`] of its end, in sight from there) is lengthened
/// to her ([`keep_path`]) and not planned again, but for its refresh ([`CHASE_TRAIL_TICKS`] after it
/// was first lengthened, [`CHASE_WALK_TICKS`] after it was planned if it never was), which is a
/// [`Want::Refresh`]: a look for a way no dearer than the path it walks.
fn chase_wants(cx: &mut Ctx<'_>, ix: usize, id: UnitId, goal: Vec2, goal_cell: CellIx, max_cells: u32) -> Want {
    let now = cx.world.tick;
    let on_slot = now.0.wrapping_add(id.get()) % CHASE_PLAN_EVERY == 0 || cx.scratch.retry.contains(&id);
    let on = |want: Want| if on_slot { want } else { Want::No };
    match judge(cx, ix, goal, goal_cell) {
        Judge::Hold => Want::No,
        Judge::Slot => on(Want::Full),
        Judge::Now => Want::Full,
        Judge::Slide => {
            if let Some(p) = cx.zone.units[ix].path.as_deref_mut() {
                p.goal = goal_cell;
            }
            Want::No
        }
        Judge::Idle => {
            postpone(cx, ix);
            Want::No
        }
        Judge::Refresh => on(Want::Refresh),
        Judge::Blocked => on(Want::Round),
        Judge::Keep => match keep_path(cx, ix, goal, goal_cell, max_cells) {
            Kept::Urgent => Want::Full,
            Kept::No => on(Want::Full),
            // Lengthened to her: its timer may be up all the same.
            Kept::Yes => match live_path(&cx.zone.units[ix]) {
                Some(p) if now >= p.repath_at => {
                    if next_step_held(cx, ix) {
                        on(Want::Round)
                    } else if as_short_as_can_be(cx, ix, goal) {
                        postpone(cx, ix);
                        Want::No
                    } else {
                        on(Want::Refresh)
                    }
                }
                _ => Want::No,
            },
        },
    }
}

/// What a chaser makes of the path it has and where she is (see [`chase_wants`]).
fn judge(cx: &Ctx<'_>, ix: usize, goal: Vec2, goal_cell: CellIx) -> Judge {
    let now = cx.world.tick;
    let g = &cx.rt.grid;
    let w = g.w();
    let u = &cx.zone.units[ix];
    let Some(p) = live_path(u) else { return Judge::Slot };
    let (ux, uy) = u.pos.cell();
    if let Some(&c) = p.cells.get(usize::from(p.at)) {
        let (x, y) = ((c.0 % w) as i32, (c.0 / w) as i32);
        // Thrown far from its path (a ledge's hop is at most a face's cells and one), or its next
        // step gone solid (a door shut, a crate pushed in; the last cell is her own, which may
        // hold a prop's notch): at once.
        if (x - ux).abs().max((y - uy).abs()) > LEDGE_FACE_MAX + 2 {
            return Judge::Now;
        }
        let ahead = usize::from(p.at) + 1 < p.cells.len();
        if ahead && g.solid(x, y) && !crate::span::steps_on_deck(g, u.on_span, (ux, uy), (x, y)) {
            return Judge::Now;
        }
    }
    // The layer it was planned for against the one she stands on now (MAP.md §3.4).
    if (g.has_levels() || g.has_spans()) && goal_layer(g, p.goal) != layer_now(cx, u, goal) {
        return Judge::Now;
    }
    if p.cells.is_empty() {
        // A search that found nothing, remembered: it stands till its wait is over, wherever she
        // goes (what stopped it, a ring of bodies about her or a fence, is rarely where she is,
        // and the search is the dearest thing a chaser does).
        if now >= p.repath_at {
            return Judge::Slot;
        }
        return if p.goal == goal_cell { Judge::Hold } else { Judge::Slide };
    }
    if p.goal != goal_cell || usize::from(p.at) >= p.cells.len() {
        return Judge::Keep;
    }
    if now < p.repath_at {
        Judge::Hold
    } else if next_step_held(cx, ix) {
        Judge::Blocked
    } else if as_short_as_can_be(cx, ix, goal) {
        Judge::Idle
    } else {
        Judge::Refresh
    }
}

/// Has this chaser a path of no more than [`CHASE_STRAIGHT_SLACK`] cells over the straight way to
/// her? Then no search could find it a shortcut worth the search (the ways of a flat field are the
/// straight way and little else); a path winding round something, as the trail of one she led round
/// a pillar is, is not, and its timer is a look for a shorter.
fn as_short_as_can_be(cx: &Ctx<'_>, ix: usize, goal: Vec2) -> bool {
    let u = &cx.zone.units[ix];
    let Some(p) = live_path(u) else { return false };
    let left = p.cells.len().saturating_sub(usize::from(p.at)) as i64;
    left <= crate::height::cells_apart(u.pos, goal) + CHASE_STRAIGHT_SLACK
}

/// Put the timer of a path off: a safety net again.
fn postpone(cx: &mut Ctx<'_>, ix: usize) {
    let now = cx.world.tick;
    if let Some(p) = cx.zone.units[ix].path.as_deref_mut() {
        p.repath_at = now.after(Tick(CHASE_WALK_TICKS));
    }
}

/// Is the step this chaser is making for held by a body? (The last cell is her own, which she holds.)
fn next_step_held(cx: &Ctx<'_>, ix: usize) -> bool {
    let g = &cx.rt.grid;
    let u = &cx.zone.units[ix];
    let Some(p) = live_path(u) else { return false };
    let at = usize::from(p.at);
    let Some(&c) = p.cells.get(at).filter(|_| at + 1 < p.cells.len()) else { return false };
    let w = g.w();
    let (x, y) = ((c.0 % w) as i32, (c.0 / w) as i32);
    let deck = crate::span::steps_on_deck(g, u.on_span, u.pos.cell(), (x, y));
    !g.free_on(x, y, deck, Some(u.pos.cell()))
}

/// What the path a chaser walks would cost it from where it stands, tenths of a cell, as the search
/// reckons a step: None if it has none.
fn remaining_cost(cx: &Ctx<'_>, ix: usize) -> Option<u32> {
    let w = cx.rt.grid.w();
    let u = &cx.zone.units[ix];
    let p = live_path(u)?;
    let mut from = u.pos.cell();
    let mut cost = 0u32;
    for &c in p.cells.get(usize::from(p.at)..)? {
        let to = ((c.0 % w) as i32, (c.0 / w) as i32);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let apart = dx.abs().max(dy.abs()) as u32;
        // A step, or a ledge's hop to its landing (path.rs charges a hop its length and LEDGE_PATH_EXTRA).
        cost += if apart <= 1 { octile10(dx, dy) } else { apart * 10 + LEDGE_PATH_EXTRA };
        from = to;
    }
    Some(cost)
}
/// What came of keeping a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kept {
    /// Lengthened to her (or already ending on her cell): no search.
    Yes,
    /// Too far, out of sight on her own ground, in the way: plan, on its tick.
    No,
    /// Sight to her broke across a change of level: plan now.
    Urgent,
}

/// The path ends within [`CHASE_KEEP_CELLS`] of her, at her level and not near a span, with the
/// step or two to her open to feet ([`steps_between`]) and her in sight from the path's end: it
/// is lengthened by them and its goal is her cell. (A path longer than a search plans stays what
/// it is: it is planned again.)
fn keep_path(cx: &mut Ctx<'_>, ix: usize, goal: Vec2, goal_cell: CellIx, max_cells: u32) -> Kept {
    let now = cx.world.tick;
    let g = &cx.rt.grid;
    let w = g.w();
    let u = &cx.zone.units[ix];
    let Some(p) = live_path(u) else { return Kept::No };
    let Some(&last) = p.cells.last() else { return Kept::No };
    let end = ((last.0 % w) as i32, (last.0 / w) as i32);
    let to = goal.cell();
    let at = usize::from(p.at);
    if (end.0 - to.0).abs().max((end.1 - to.1).abs()) > CHASE_KEEP_CELLS || (end == to && at >= p.cells.len()) {
        return Kept::No;
    }
    // Decks are the search's: a walker on one, a goal on one, a span at either end or between.
    if g.has_spans()
        && (u.on_span.is_some()
            || deck_goal(cx, u.id, goal)
            || g.deck_at(end.0, end.1).is_some()
            || g.deck_at(to.0, to.1).is_some()
            || g.span_near(end, to))
    {
        return Kept::No;
    }
    if g.level_at(end.0, end.1) != g.level_at(to.0, to.1) || p.cells.len() - at >= max_cells as usize {
        return Kept::No;
    }
    let steps = if end == to { Some(([to; 2], 0)) } else { steps_between(g, end, to) };
    let Some((steps, n)) = steps else { return Kept::No };
    if !cx.scratch.sight.line(g, Vec2::centre(end.0, end.1), None, goal, None) {
        // Lost her across a change of level: at once. Behind a wall on her own ground: on its tick.
        return if g.flat_between(end, to) { Kept::No } else { Kept::Urgent };
    }
    cx.scratch.path.stats.kept += 1;
    let Some(p) = cx.zone.units[ix].path.as_deref_mut() else { return Kept::No };
    // The cells already walked are dropped once there are enough of them.
    if usize::from(p.at) >= CHASE_TRIM_CELLS {
        p.cells.drain(..usize::from(p.at));
        p.at = 0;
    }
    p.cells.extend(steps[..n].iter().map(|&(x, y)| CellIx(y as u32 * w + x as u32)));
    p.goal = goal_cell;
    // A path lengthened this way is no longer the shortest: it is planned again soon.
    p.repath_at = p.repath_at.min(now.after(Tick(CHASE_TRAIL_TICKS)));
    Kept::Yes
}

/// The one or two cells a walker steps from `from` to `to`, at most two cells apart (`steps` and how
/// many): each a step the search would take: open to feet and held by no one (but `to`, where she
/// stands), at the level of `from`, a diagonal with both its sides open. With two, the middle
/// cell is the cheapest of those beside both ends, the searches' neighbour order breaking ties.
pub(crate) fn steps_between(g: &ZoneGrid, from: (i32, i32), to: (i32, i32)) -> Option<([(i32, i32); 2], usize)> {
    let level = g.level_at(from.0, from.1);
    let open = |x: i32, y: i32, goal: bool| {
        let f = g.flags_at(x, y);
        g.inside(x, y) && f & BLOCK_MOVE == 0 && (goal || f & F_OCC == 0) && g.level_at(x, y) == level
    };
    let step = |a: (i32, i32), b: (i32, i32), goal: bool| {
        open(b.0, b.1, goal) && (a.0 == b.0 || a.1 == b.1 || (!g.solid(a.0, b.1) && !g.solid(b.0, a.1)))
    };
    let apart = (to.0 - from.0).abs().max((to.1 - from.1).abs());
    match apart {
        1 => step(from, to, true).then_some(([to, to], 1)),
        2 => {
            let mut best: Option<(u32, (i32, i32))> = None;
            for (dx, dy) in DIRS8 {
                let m = (from.0 + dx, from.1 + dy);
                if m == to
                    || (m.0 - to.0).abs().max((m.1 - to.1).abs()) > 1
                    || !step(from, m, false)
                    || !step(m, to, true)
                {
                    continue;
                }
                let cost = octile10(m.0 - from.0, m.1 - from.1) + octile10(to.0 - m.0, to.1 - m.1);
                if best.is_none_or(|(c, _)| cost < c) {
                    best = Some((cost, m));
                }
            }
            best.map(|(_, m)| ([m, to], 2))
        }
        _ => None,
    }
}

/// Close in (MAP.md §3.5): within [`CHASE_CLOSE_CELLS`] of her on her own ground, with nothing
/// between but open ground at one level that no one stands in, and in sight, a chaser steps
/// straight at her and plans nothing; the last cells of a chase are the commonest and the least
/// worth a search. It stops half a cell from her (a body's reach is more than that, and only a
/// thing with no reach would stand there). Whether it spent its tick so: not when any of that
/// fails, nor when the step went nowhere, and then its path walks it. A step leaves its old path
/// behind it.
fn close_in(cx: &mut Ctx<'_>, id: UnitId, tpos: Vec2, speed: Fx) -> bool {
    let now = cx.world.tick;
    let Some(ix) = cx.zone.unit_ix(id) else { return false };
    let u = &cx.zone.units[ix];
    let Some(t) = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.pos == tpos) else { return false };
    let g = &cx.rt.grid;
    let (from, to) = (u.pos.cell(), tpos.cell());
    if u.on_span.is_some()
        || t.on_span.is_some()
        || crate::height::cells_apart(u.pos, tpos) > i64::from(CHASE_CLOSE_CELLS)
        || (g.has_spans() && g.span_near(from, to))
    {
        return false;
    }
    let levels = g.has_levels();
    let level = g.level_at(from.0, from.1);
    if levels && g.level_at(to.0, to.1) != level {
        return false;
    }
    // Every cell the line touches: open to feet, at this level, and held by no one but her.
    let closed = crate::los::walk_cells(u.pos, tpos, |x, y| {
        let f = g.flags_at(x, y);
        f & BLOCK_MOVE != 0 || (levels && g.level_at(x, y) != level) || ((x, y) != to && f & F_OCC != 0)
    });
    if closed.is_some() || !cx.scratch.sight.sees(g, u, t) {
        return false;
    }
    let d = distance(u.pos, tpos);
    let budget = i64::from(speed.0) * i64::from(speed_factor(u, now)) / 1000;
    let step = budget.min(d - i64::from(CELL_FX / 2));
    if step <= 0 {
        // On her already: it stands and watches.
        if let Some(u) = cx.zone.unit_mut(id) {
            face_point(u, tpos);
        }
        return d <= i64::from(CELL_FX / 2);
    }
    let (dx, dy) = (i64::from(tpos.x.0 - u.pos.x.0) * step / d, i64::from(tpos.y.0 - u.pos.y.0) * step / d);
    let u = &mut cx.zone.units[ix];
    face_vector(u, dx, dy);
    if !crate::units::press_unit(cx.rt, u, Fx(dx as i32), Fx(dy as i32)) {
        return false;
    }
    clear_path(u);
    // On her ground there is no foot to hold at (`hold_below` would have ended it).
    end_hold(u);
    cx.scratch.path.stats.straight += 1;
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
        warm: Option<u8>,
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
                if warm.is_none_or(|stage| crate::light::warm(l, p, rt, stage)) {
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
