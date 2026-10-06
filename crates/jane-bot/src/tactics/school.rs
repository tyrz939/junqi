//! The School (DUNGEONS.md §3.6): the timetable, the lessons, the Timekeeper.
//!
//! The crawl does the lessons by itself (a Repair, a Grow, bolts at what answers them, the clock
//! each one shows). What it cannot do alone is the building's one idea, the timetable, and the
//! sick bay that makes the rest survivable. Read the way a player reads the hall:
//!
//! - **The sick bay fire** is where she wakes if it goes badly, and it mends her without turning
//!   the clock: she sits by it the first time she can get to it, goes back to it between fights
//!   when she is hurt or out of mana, breaks off a walk for it when badly hurt, and backs off to
//!   it from a fight she is losing against things slower than she is. The apples are kept for the
//!   Timekeeper, since nothing mends her in the belfry.
//! - **The bell rope** turns the period (the doors that were shut open, and the others shut).
//!   She rings for break first (the staff walk the corridors in lesson time), and after that when
//!   there is nothing left to do in this period, once for each state of her progress (clocks
//!   stopped, the tower key, the period) and each day: pulled again with nothing changed, it
//!   would only turn the building round and back; a night slept on it is a change (a break
//!   lesson that did not take is gone back to the next morning, 2026-10-06).
//! - **The beds.** The near one sleeps to six: a lesson left after the sun has gone (the
//!   glasshouse wants daylight) is left till morning. The far one sleeps to the bell at nine:
//!   with every clock stopped and no tower key, she lies down on it, and at night she walks the
//!   corridor for the Caretaker, who carries the key.
//! - **The way out** comes before anything left lying about: the rope for the tower once the key
//!   is hers, and the Timekeeper, kited round the belfry with the frost (he walks slower than she
//!   does), with the stone skin and the life steal she brought.
//! - **The fights.** Everything in the building is as strong as she is. Only the Caretaker and the
//!   Timekeeper are hunted; the rest is fought where it wakes, every bolt thrown as it comes ready,
//!   what is at her elbow first and then what throws things, out of a web as soon as she is in one.

use std::collections::BTreeSet;

use jane_core::Vec2;
use jane_core::action::Action;
use jane_sim::{Prop, View};

use crate::crawl::{Reach, Try};
use crate::sense::{self, holds, prop_does};
use crate::task::{Ctx, Task, UseProp};

/// Mend at the fire below this (permille of her health, or half of it of her mana), when it is
/// within [`NEAR_FIRE`] cells; from further off, below [`MEND_FAR_BELOW`].
const MEND_BELOW: i32 = 900;
const MEND_FAR_BELOW: i32 = 850;
const NEAR_FIRE: i32 = 40;

/// What the School tactic remembers.
#[derive(Debug, Default)]
pub struct School {
    /// Progress states the rope was pulled from: (clocks stopped, tower key held, at break, day).
    pulled: BTreeSet<(u32, bool, bool, u32)>,
    /// Places walked to looking for the Caretaker, by the day of the night.
    searched: BTreeSet<(u32, i32, i32)>,
    /// Nights slept through to six for a lesson left over: (day, clocks stopped).
    slept: BTreeSet<(u32, u32)>,
    /// She has lain down for the morning: what would not take in the dark is worth another go.
    woke: bool,
    /// Progress states (clocks stopped, at break) she has given the lessons one more round from.
    retried: BTreeSet<(u32, bool)>,
}

impl School {
    /// Once after a night slept through for a lesson: the crawl forgets what failed in the dark.
    pub fn take_woke(&mut self) -> bool {
        std::mem::take(&mut self.woke)
    }
}

/// Does using this prop rest her without turning the clock (a fire, a stove)?
fn fire(v: &View<'_>, p: &Prop) -> bool {
    jane_data::catalog().story.prop(p.def).rest
        && prop_does(v, p, &|a| matches!(a, Action::Rest { until: None }))
        && !prop_does(v, p, &|a| matches!(a, Action::Rest { until: Some(_) }))
}

/// A bed that sleeps to `hour`.
fn bed_to(v: &View<'_>, p: &Prop, hour: u8) -> bool {
    jane_data::catalog().story.prop(p.def).rest
        && prop_does(v, p, &|a| matches!(a, Action::Rest { until: Some(h) } if *h == hour))
}

fn def_named(p: &Prop, name: &str) -> bool {
    jane_data::catalog().story.prop_id(name) == Some(p.def)
}

/// Periods on the timetable in the hall: Woodwork, Chemistry, Botany, Physics, Domestic Science
/// and the snowflake. Each ends at a classroom clock.
const LESSONS: u32 = 6;

/// The classroom clocks she has stopped.
fn stopped(v: &View<'_>) -> u32 {
    v.props().filter(|p| def_named(p, "clock_stopped") && p.used).count() as u32
}

/// Break: the lesson-time doors are shut (the woodwork door, by its name on the door).
fn at_break(v: &View<'_>) -> bool {
    sense::prop_named(v, "school_door_woodwork").is_some_and(|p| p.locked)
}

fn has_tower_key(v: &View<'_>) -> bool {
    jane_data::catalog().combat.item_id("key_tower").is_some_and(|k| holds(v, k) > 0)
}

/// First, before the crawl's own order: the fire, when she has not sat by it yet or is hurt; the
/// rope, while staff walk the corridors. `fresh` is the crawl's rule for trying a thing again.
pub fn first(
    s: &mut School,
    v: &View<'_>,
    cx: &Ctx,
    reach: &Reach,
    fresh: &dyn Fn(Try) -> bool,
    boss_down: bool,
) -> Option<(Task, Try)> {
    mend(v, cx, reach, fresh).or_else(|| endgame(s, v, cx, reach, boss_down)).or_else(|| ring_for_break(s, v, reach))
}

/// "The corridors during lessons are why that is a bad idea and why break is the time to move"
/// (DUNGEONS.md §3.6): with staff walking the corridors in lesson time, she rings for break
/// before she goes anywhere, unless she has rung from here already.
fn ring_for_break(s: &mut School, v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    // Only the first time: once she has rung the period back to lessons, it was on purpose.
    if at_break(v) || !s.pulled.is_empty() {
        return None;
    }
    pull(s, v, reach)
}

/// The rope, if she has not pulled it from this state of her progress before.
fn pull(s: &mut School, v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let rope = v.props().find(|p| !p.hidden && def_named(p, "bell_rope") && reach.beside(p))?;
    if !s.pulled.insert((stopped(v), has_tower_key(v), at_break(v), v.clock().1)) {
        return None;
    }
    Some((Task::Use(UseProp::new(rope.id)), Try::Prop(rope.id)))
}

/// The sick bay fire: the first time she can get to it, and again when she is hurt or low.
fn mend(v: &View<'_>, cx: &Ctx, reach: &Reach, fresh: &dyn Fn(Try) -> bool) -> Option<(Task, Try)> {
    let me = v.body();
    let grate = v.props().filter(|p| !p.hidden && fire(v, p) && reach.beside(p)).min_by_key(|p| p.id)?;
    let sat = cx.used.contains_key(&(v.zone(), grate.id));
    // Near the fire, a little hurt is worth mending; across the building, only a lot (the walk
    // back is a minute, and the crawl's own rule takes her there when she has nothing to eat).
    let near = sense::to_prop(grate, me.pos) < i64::from(NEAR_FIRE * jane_core::num::CELL_FX);
    // The tower stair is open and the door shuts behind her at the top: she goes up whole.
    let tower = sense::prop_named(v, "school_tower_door").is_some_and(|p| !p.locked);
    let below = if tower {
        950
    } else if near {
        MEND_BELOW
    } else {
        MEND_FAR_BELOW
    };
    let mp_low = i64::from(me.mp.0) * 1000 < i64::from(jane_sim::units::max_mp(me).0) * i64::from(below / 2);
    let hurt = sense::hp_permille(me) < below || mp_low;
    let fighting = sense::enemies(v).iter().any(|u| pressing(v, u));
    if (sat && !hurt) || fighting {
        return None;
    }
    let what = Try::Rest(grate.id);
    // A rest is always worth another go once she is hurt again; only a failed one waits.
    if !fresh(what) && cx.used.get(&(v.zone(), grate.id)).is_none_or(|&f| f + 60 > v.frame()) {
        return None;
    }
    Some((Task::Use(UseProp::new(grate.id)), what))
}

/// Is `u` at her: in a fight with her, and not a shade standing off at the edge of the light
/// (it will not come in, and she does not wait on it).
fn pressing(v: &View<'_>, u: &jane_sim::Unit) -> bool {
    crate::fight::on_me(v, u)
        && !(jane_data::catalog().combat.unit(u.def).shuns_light
            && crate::fight::gap(v.body(), u) > i64::from(jane_core::num::CELL_FX))
}

/// Hurt this much, she breaks off whatever she is walking to for the fire (permille).
const BREAK_OFF_BELOW: i32 = 600;

/// Should she drop what she is doing for the sick bay fire? She is hurt, nothing is on her, and
/// she can walk to it (she keeps her apples for the Timekeeper, so the crawl's own rule, which
/// waits for her to have nothing to eat, never fires here).
pub fn breaks_off(v: &View<'_>, reach: &Reach) -> bool {
    sense::hp_permille(v.body()) < BREAK_OFF_BELOW
        && !sense::enemies(v).iter().any(|u| pressing(v, u))
        && v.props().any(|p| !p.hidden && fire(v, p) && reach.beside(p))
}

/// Last, when the crawl has nothing left: the rope, the far bed, the Caretaker.
/// `boss_down`: the Timekeeper has been seen to fall.
pub fn last(s: &mut School, v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    // Woken at six for a lesson left over: the sun is not in the glasshouse till half past.
    if stopped(v) < LESSONS && v.hour() == 6 {
        s.woke = true;
        return Some((Task::Wait(60 * 20), Try::Explore(-1, i32::from(v.hour()))));
    }
    // A lesson left after the sun has gone (the glasshouse wants daylight, half past six to half
    // past six): the near bed, to six.
    let (clock, _) = v.clock();
    let hour = jane_sim::tuning::TICKS_PER_HOUR;
    let sun = (13 * hour / 2..37 * hour / 2).contains(&clock);
    if stopped(v) < LESSONS && !sun {
        if let Some(bed) = v.props().find(|p| !p.hidden && bed_to(v, p, 6) && reach.beside(p)) {
            if s.slept.insert((v.clock().1, stopped(v))) {
                s.woke = true;
                return Some((Task::Use(UseProp::new(bed.id)), Try::Rest(bed.id)));
            }
        }
    }
    pull(s, v, reach).or_else(|| {
        // Nothing left and a lesson still open: what was given up on (a verb that met a fight on
        // the way, a bolt with no mana behind it) is worth one more round, once per state of
        // her progress.
        (stopped(v) < LESSONS && s.retried.insert((stopped(v), at_break(v)))).then(|| {
            s.woke = true;
            (Task::Wait(30), Try::Explore(-2, stopped(v) as i32))
        })
    })
}

/// The way out, once the lessons are done, before anything left lying about: the far bed to
/// the bell, the Caretaker and his key, the rope that opens the tower, and the Timekeeper.
/// `boss_down`: the Timekeeper has been seen to fall.
fn endgame(s: &mut School, v: &View<'_>, cx: &Ctx, reach: &Reach, boss_down: bool) -> Option<(Task, Try)> {
    if boss_down {
        return None;
    }
    // The Timekeeper himself, however often he has walked her off: he is the way out.
    if let Some(u) =
        crate::crawl::boss_of(v.zone()).and_then(|b| sense::units_of(v, b).into_iter().find(|u| reach.point(u.pos)))
    {
        return Some((Task::Hunt(u.id), Try::Fight(u.id)));
    }
    // The tower is open and the bell is up there: up the stair to it (what rings it sleeps until
    // someone comes up).
    if let Some(bell) = v.props().find(|p| def_named(p, "school_bell") && reach.beside(p)) {
        let c = sense::prop_centre(bell);
        if crate::nav::dist(v.body().pos, c) > i64::from(4 * jane_core::num::CELL_FX) {
            let (x, y) = c.cell();
            return Some((Task::Walk { to: c, near: jane_core::Fx::from_px(24) }, Try::Explore(x, y)));
        }
    }
    if stopped(v) < LESSONS {
        return None;
    }
    // Every clock stopped and the key in her hand: the rope, for the tower.
    if has_tower_key(v) {
        return pull(s, v, reach);
    }
    // No key: by day, the far bed to the bell at nine; at night, the Caretaker.
    if !v.is_night() {
        let bed = v.props().find(|p| !p.hidden && bed_to(v, p, 21) && reach.beside(p))?;
        return Some((Task::Use(UseProp::new(bed.id)), Try::Rest(bed.id)));
    }
    caretaker(s, v, cx, reach)
}

/// The bolts she throws in a fight, in the order she reaches for them: the fireball (it keeps
/// burning), the icebolt (it slows), the spark (cheap and quick).
const BOLTS: [&str; 3] = ["fireball", "icebolt", "spark"];

/// Back to the fire below this (permille of her health) with two or more on her, or alone.
const RETREAT_CROWD: i32 = 550;
const RETREAT_ALONE: i32 = 350;

/// How close (cells, between bodies) she lets a slow boss come before she backs off.
const KITE_CELLS: i32 = 5;

/// Hunting something this long that will not come to her (a shade at the edge of the light)
/// is given up for a while.
const SHY_FRAMES: u32 = 60 * 12;

/// One frame of a School fight against `id`, after the general fight has eaten, stepped out of
/// a tell and decided not to run: everything in the building is as strong as she is, so she
/// throws every bolt she has as each comes ready and swings between them (a swing costs no
/// global cooldown), and stands her ground rather than backing into the next room's patrol.
/// `None`: the general fight's; `Some(None)`: the fight is over for now.
pub fn strike(v: &View<'_>, cx: &mut Ctx, id: jane_sim::ids::UnitId) -> Option<Option<crate::Act>> {
    use crate::Act;
    use crate::fight::{gap, ready};
    use crate::nav::{Go, dist, stick};
    use jane_core::num::CELL_FX;
    use jane_sim::{Command, InputFrame};
    let cat = jane_data::catalog();
    let me = v.body();
    let t = v.unit(id).filter(|u| u.alive)?;
    // A shade waits at the edge of the light and will not come in: she has other things to do.
    let far = gap(me, t) > i64::from(CELL_FX);
    let shy = t.combat != jane_sim::state::CombatState::Combat;
    if cat.combat.unit(t.def).shuns_light && (far || shy) && cx.fight.hunt != Some(id) {
        return Some(None);
    }
    if cx.fight.t > SHY_FRAMES && !crate::fight::on_me(v, t) && !cat.combat.unit(t.def).boss {
        cx.fight.unreachable.insert(id, v.frame() + 60 * 60);
        cx.fight.hunt = None;
        cx.fight.target = None;
        return Some(None);
    }
    let now = v.tick();
    let d = dist(me.pos, t.pos);
    let g = gap(me, t);
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    // Who is on her. A caster (a bolt in its book) hits hardest and from anywhere: her bolts go
    // at the weakest of those she can see first.
    let on: Vec<&jane_sim::Unit> = sense::enemies(v).into_iter().filter(|u| pressing(v, u)).collect();
    let caster = |u: &jane_sim::Unit| {
        cat.combat
            .unit(u.def)
            .book
            .iter()
            .any(|&s| matches!(cat.combat.spell(s).kind, jane_data::SpellKind::Bolt | jane_data::SpellKind::Ground))
    };
    // (Within an icebolt's reach: one further off waits for the one at hand.)
    let bolt_reach = i64::from(cat.combat.spell(sense::spell("icebolt")).range.0) * 9 / 10;
    // Something at her elbow is hit first, bolts and all (a bolt thrown past it at a caster
    // behind ends in it anyway).
    let close = gap(me, t) <= i64::from(2 * CELL_FX);
    let mark = on
        .iter()
        .copied()
        .filter(|u| !close && caster(u) && v.sight(me.pos, u.pos) && gap(me, u) <= bolt_reach)
        .min_by_key(|u| (u.hp, u.id))
        .unwrap_or(t);

    // Losing: back to the sick bay fire while she can still walk there (the general fight runs
    // home from here on, `run_home`), and the fight goes on beside it. Not from things that do
    // not walk (a web spinner on the wall): backing off through their webs is how she dies.
    // Nor from what she cannot walk away from (a caster shoots her in the back; a guard or a
    // bat runs faster than she walks): that fight is finished where it stands.
    let walks = |u: &&&jane_sim::Unit| cat.combat.unit(u.def).run.0 > 0;
    let chasers = on.iter().filter(walks).count();
    let hp = sense::hp_permille(me);
    let away = on.iter().all(|u| !outpaces(v, u));
    // (A web spinner on the wall counts in the crowd: it cannot follow, but it shoots while she
    // trades blows with what walks.)
    let losing = away && chasers >= 1 && (hp < RETREAT_CROWD && on.len() >= 2 || hp < RETREAT_ALONE);
    let timekeeper = crate::crawl::boss_of(v.zone()) == Some(t.def);
    if losing && cx.fight.fleeing == 0 && !stands(v, t) {
        if let Some(f) = run_home(v, cx) {
            cx.fight.fleeing = 180;
            cx.fight.fled += 1;
            cx.fight.hunt = None;
            return Some(Some(Act::hold(f)));
        }
    }
    // What she brought, kept for the Timekeeper: the stone skin against his blows, the life steal.
    let boss = cat.combat.unit(t.def).boss;
    // A potion waits for the global cooldown like a spell: she holds her bolts for it.
    let mut drinking = false;
    if timekeeper && d < i64::from(12 * CELL_FX) {
        let has = |e: &str| me.statuses.iter().any(|s| cat.combat.effect(s.effect).id == e);
        // Not the mana shield: it pays for his blows out of the mana her bolts are made of, and
        // a caster with no mana does not bring him down.
        for (name, effect) in [("potion_stoneskin", "stoneskin"), ("potion_lifesteal", "lifesteal")] {
            let p = sense::item(name);
            if !has(effect) && holds(v, p) > 0 && !me.item_cooldowns.iter().any(|&(c, until)| c == p && until > now) {
                if me.gcd_until <= now {
                    return Some(Some(Act::press(Command::Item(p))));
                }
                drinking = true;
                break;
            }
        }
    }
    let melee = sense::spell("melee_player");
    let reach = i64::from(cat.combat.spell(melee).range.0);
    let mut cmds = Vec::new();
    let mut aim = dir;
    if !drinking && v.sight(me.pos, mark.pos) {
        let mg = gap(me, mark);
        let max = i64::from(jane_sim::units::max_mp(me).0);
        // The dear one only while the mana is more than half there: the lamps want some. The
        // blast when two or more stand where it lands.
        let flush = i64::from(me.mp.0) * 2 > max;
        let bunched = sense::enemies(v).iter().filter(|u| dist(u.pos, mark.pos) < i64::from(3 * CELL_FX)).count() >= 2;
        let blast = (flush && bunched).then_some("explosion");
        if let Some(s) = blast
            .into_iter()
            .chain(BOLTS.iter().skip(usize::from(!flush)).copied())
            .filter_map(|n| cat.combat.spell_id(n))
            .find(|&s| {
                let def = cat.combat.spell(s);
                sense::knows(v, s) && ready(me, s, now) && mg <= i64::from(def.range.0) * 9 / 10
            })
        {
            aim = jane_core::angle::iatan2(mark.pos.y.0 - me.pos.y.0, mark.pos.x.0 - me.pos.x.0);
            cmds.push(Command::Cast { spell: s, on: Some(mark.id) });
        }
    }
    // Something with feet at her and a web spinner on the wall in reach of her: she walks the one
    // with feet back out of the spinner's reach (it follows, the spinner cannot) and fights it
    // there, rather than trading blows in the doorway under the webs.
    let spinner_on_her = on.iter().any(|u| {
        let def = cat.combat.unit(u.def);
        let far = def.book.iter().map(|&s| i64::from(cat.combat.spell(s).range.0)).max().unwrap_or(0);
        def.run.0 == 0 && far > i64::from(2 * CELL_FX) && gap(me, u) <= far + i64::from(CELL_FX)
    });
    if !boss && chasers >= 1 && spinner_on_her {
        if let Some(f) = run_home(v, cx) {
            return Some(Some(Act { frame: InputFrame { aim: Some(aim), ..f }, cmds }));
        }
    }
    // A boss that walks slower than she does (the Timekeeper, until the last of him) is kept at
    // bolt range: she backs off round the room while he closes and throws as she goes, and
    // stands to throw once he is far enough off.
    let def = cat.combat.unit(t.def);
    let his = match t.phase {
        0 => def.run,
        n => def.phases.get(usize::from(n) - 1).and_then(|p| p.run).unwrap_or(def.run),
    };
    let ice = sense::spell("icebolt");
    // Chilled, even the last of him (as quick as she walks) falls behind.
    let chilled = t.statuses.iter().any(|s| cat.combat.effect(s.effect).id == "chilled");
    let slower = his.0 < cat.combat.unit(me.def).walk.0;
    if boss && sense::knows(v, ice) {
        // The frost first whenever he is not slowed by it: chilled, he is a long way behind her.
        let frost = !chilled && !drinking && ready(me, ice, now) && v.sight(me.pos, t.pos);
        if frost {
            cmds = vec![Command::Cast { spell: ice, on: Some(id) }];
            aim = dir;
        }
        // (Only while he comes at her: one standing off or walking home is gone up to.)
        let coming = t.combat == jane_sim::state::CombatState::Combat;
        if g < i64::from(KITE_CELLS * CELL_FX) && (slower || chilled) && coming {
            // A throw roots her for a moment: backing off, only the frost is worth it.
            if !frost {
                cmds.clear();
            }
            // Inside half his leash from where he stands guard: kited past it he walks home
            // and mends, and the fight starts over.
            let tether =
                (def.leash.0 > 0).then_some((t.home, jane_sim::ai::leash_in(jane_core::ZoneId::School, def) / 2));
            if let Some(to) = kite_point(v, me.pos, t.pos, tether) {
                if let Go::Walk(f) = cx.nav.go(v, to, jane_core::Fx::from_px(3), false) {
                    return Some(Some(Act { frame: InputFrame { aim: Some(aim), ..f }, cmds }));
                }
            }
        } else if (slower || chilled)
            && v.sight(me.pos, t.pos)
            && (!cmds.is_empty() || g < i64::from((KITE_CELLS + 4) * CELL_FX))
        {
            return Some(Some(Act { frame: InputFrame { aim: Some(aim), ..InputFrame::IDLE }, cmds }));
        }
    }
    // Standing in a web (or any pool of theirs that bites every second): out of it, throwing as
    // she goes. Nine of its ten bites are the ones she walks out of.
    let body = i64::from(cat.combat.unit(me.def).bounds.0);
    if let Some(pool) = v
        .grounds()
        .iter()
        .filter(|p| p.faction != me.faction && p.until > now)
        .find(|p| dist(p.pos, me.pos) <= i64::from(p.radius.0) + body)
    {
        let away = if pool.pos == me.pos { t.pos } else { pool.pos };
        let f = stick(away, me.pos, false);
        let f = if away == t.pos { stick(me.pos, away, false) } else { f };
        return Some(Some(Act { frame: InputFrame { aim: Some(aim), ..f }, cmds }));
    }
    if g <= reach {
        // A swing (it wants her facing it) only when no bolt is thrown elsewhere this frame.
        if ready(me, melee, now) && (cmds.is_empty() || mark.id == id) {
            cmds.push(Command::Cast { spell: melee, on: Some(id) });
        }
        return Some(Some(Act {
            frame: InputFrame { aim: Some(aim), mv_dir: dir, mv_mag: 10, ..InputFrame::IDLE },
            cmds,
        }));
    }
    let frame = match cx.nav.go(v, t.pos, jane_core::Fx(reach as i32 + 6 * 256), false) {
        Go::Walk(f) if d > i64::from(4 * CELL_FX) => f,
        Go::NoWay => return None,
        _ => stick(me.pos, t.pos, false),
    };
    Some(Some(Act { frame: InputFrame { aim: Some(aim), ..frame }, cmds }))
}

/// Where to back off to from something at `from`: the far end of a short walk, in open floor
/// (a corner is where a slow thing catches up), inside the tether: a flood over the ground
/// about her, as `fight::retreat_point`, with the open cells about each counted in.
fn kite_point(v: &View<'_>, me: Vec2, from: Vec2, tether: Option<(Vec2, i64)>) -> Option<Vec2> {
    use crate::nav::{dist, walkable};
    use jane_core::num::CELL_FX;
    use std::collections::VecDeque;
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    let open = |(x, y): (i32, i32)| {
        let mut n = 0i64;
        for dy in -2..=2 {
            for dx in -2..=2 {
                n += i64::from(walkable(v, x + dx, y + dy));
            }
        }
        n
    };
    let mut best: Option<(i64, (i32, i32))> = None;
    while let Some((c, steps)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        let score = dist(at, from) + open(c) * i64::from(CELL_FX) / 4 - i64::from(steps) * i64::from(CELL_FX) / 3;
        if tether.is_none_or(|(home, r)| dist(at, home) <= r) && best.is_none_or(|b| (score, c) > b) {
            best = Some((score, c));
        }
        if steps >= 14 || seen.len() > 1200 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back((n, steps + 1));
            }
        }
    }
    best.map(|(_, c)| Vec2::centre(c.0, c.1))
}

/// May she eat now? The sick bay fire mends her for nothing between fights, and nothing mends
/// her in the belfry once the stair door has shut: the apples are kept for the Timekeeper,
/// unless she is nearly done for.
pub fn may_eat(v: &View<'_>) -> bool {
    let boss = crate::crawl::boss_of(v.zone());
    sense::hp_permille(v.body()) < 200
        || sense::enemies(v).iter().any(|u| {
            crate::fight::on_me(v, u) && (Some(u.def) == boss || outpaces(v, u))
                || Some(u.def) == boss && u.combat == jane_sim::state::CombatState::Combat
        })
}

/// Does she stand and fight `t` out, low as she is? The Timekeeper (the stair door is shut
/// behind her; he is kited, not run from), and anything she cannot walk away from.
pub fn stands(v: &View<'_>, t: &jane_sim::Unit) -> bool {
    let boss = crate::crawl::boss_of(v.zone());
    // (In the belfry, with him on her, there is nowhere to run to: the shades he brings too.)
    boss == Some(t.def)
        || outpaces(v, t)
        || sense::enemies(v).iter().any(|u| Some(u.def) == boss && crate::fight::on_me(v, u))
}

/// Can `u` keep up with her walking away: it runs as fast as she walks, or it throws bolts?
fn outpaces(v: &View<'_>, u: &jane_sim::Unit) -> bool {
    let cat = jane_data::catalog();
    let def = cat.combat.unit(u.def);
    // Something with no feet (a web spinner on the wall) does not keep up: its reach is left.
    if def.run.0 == 0 {
        return false;
    }
    def.run.0 >= cat.combat.unit(v.body().def).walk.0
        || def
            .book
            .iter()
            .any(|&s| matches!(cat.combat.spell(s).kind, jane_data::SpellKind::Bolt | jane_data::SpellKind::Ground))
}

/// Does the School's crawl go looking for one of these? Only what the story needs down: the
/// Caretaker (he carries the tower key) and the Timekeeper. Everything else in the building is as
/// strong as she is and is left asleep; what wakes and comes at her is fought where it stands.
pub fn hunts(def: jane_core::UnitDefId) -> bool {
    jane_data::catalog().combat.unit(def).boss
}

/// Backing off in the School: to the sick bay fire, and sit down by it (it mends her whole, and
/// it is where she wakes if she does not get there). `None` when there is no fire to run to.
pub fn run_home(v: &View<'_>, cx: &mut Ctx) -> Option<jane_sim::InputFrame> {
    use crate::nav::Go;
    let grate = v.props().filter(|p| fire(v, p)).min_by_key(|p| p.id)?;
    let side = crate::task::sides(v, grate, v.body().pos).first().copied()?;
    match cx.nav.go(v, side.0, jane_core::Fx::from_px(3), false) {
        Go::Walk(f) => Some(f),
        Go::Arrived => Some(crate::task::nudge(side.1)),
        Go::NoWay => None,
    }
}

/// At the fire and facing it, backing off: sit down.
pub fn at_home(v: &View<'_>) -> Option<jane_sim::Command> {
    use jane_sim::interact::FocusRef;
    let f = v.focus()?;
    let FocusRef::Prop(id) = f.target else { return None };
    v.prop(id).filter(|p| fire(v, p)).map(|_| jane_sim::Command::Use)
}

/// The Caretaker, where she can see him or last saw him (he is heard before he is seen).
/// Else along the corridor he walks at night: its middle and its ends, each once a night.
fn caretaker(s: &mut School, v: &View<'_>, cx: &Ctx, reach: &Reach) -> Option<(Task, Try)> {
    use jane_core::num::CELL_FX;
    let def = jane_data::catalog().combat.unit_id("caretaker")?;
    if let Some(u) = sense::units_of(v, def).into_iter().find(|u| reach.point(u.pos)) {
        return Some((Task::Hunt(u.id), Try::Fight(u.id)));
    }
    let night = v.clock().1;
    let me = v.body().pos;
    let mut go = |p: Vec2, what: Try| {
        let (x, y) = p.cell();
        (reach.near(p, 2) && s.searched.insert((night, x, y)))
            .then_some((Task::Walk { to: p, near: jane_core::Fx(3 * CELL_FX) }, what))
    };
    if let Some((&id, &(_, pos))) =
        cx.seen_foes.get(&def).and_then(|m| m.iter().find(|(_, (z, pos))| *z == v.zone() && reach.point(*pos)))
    {
        if crate::nav::dist(me, pos) > i64::from(4 * CELL_FX) {
            if let Some(t) = go(pos, Try::Fight(id)) {
                return Some(t);
            }
        }
    }
    let r = v.rect(v.sym("school_corridor")?)?;
    let c = sense::rect_centre(r);
    let ends = if r.w >= r.h {
        [Vec2::centre(r.x + 2, c.cell().1), Vec2::centre(r.right() - 3, c.cell().1)]
    } else {
        [Vec2::centre(c.cell().0, r.y + 2), Vec2::centre(c.cell().0, r.bottom() - 3)]
    };
    for p in [c, ends[0], ends[1]] {
        let (x, y) = p.cell();
        if let Some(t) = go(p, Try::Explore(x, y)) {
            return Some(t);
        }
    }
    None
}
