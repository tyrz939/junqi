//! The library and Butterfly Forest (DUNGEONS.md §3.3): see `mod.rs`.
//!
//! The library asks nothing the crawl does not already do. The forest asks more of it, and the
//! hooks here are each called from one line in the crawl or the fight:
//!
//! - **The Collector** ([`offers`], [`engage`]): a boss that is not the dungeon's, so fought as
//!   anything else in the way and not left for last (the net is his), and stood up to with Stone
//!   Skin, bolts and her hands: he throws his net from twelve metres, and backing off from him
//!   is only being netted further off.
//! - **The hearth** ([`offers`]): rested at the first time she can reach it, so a death wakes
//!   her in the forest and not at the Halt, and again, whole, before the summoning glade.
//! - **Butterflies** ([`offers`], [`look`]): with the net, the cases are read again (the key in
//!   the one marked NIGHT WATCHMAN), every butterfly she can reach is talked to ("Take it"), and
//!   each glade's bud whose butterfly she has not got is walked to and grown when the sky's light
//!   is on it (the butterfly comes down to it): the bud's own list says which butterfly it calls.
//!   A butterfly not there with her standing by its bud is in her bag.
//! - **What is not to be spent** ([`skip`]): the hedge seeds (they close a gap and open nothing),
//!   every bud (the glades' are the butterflies', the summoning glade's are the Emperor's), and
//!   the bare bank and the stone while she holds fewer butterflies than the stone asks for. Four
//!   apples are kept for the Emperor ([`keep_food`]) unless she is near the end.
//! - **The Emperor** ([`engage`], [`look`]), played the way the glade is built to be played: the
//!   stone opened with the five, then a bud in the sky's light grown whenever Grow comes round.
//!   The bud's list sends the Emperor down to it to feed, stunned for six seconds and taking
//!   every blow twice over: she stands off the flower while it comes down (on it, she keeps it
//!   from landing), then puts everything into it (the Explosion, the Icebolt, her hands), and
//!   sets off for the next lit bud in time to be beside it as Grow comes round, frosting it and
//!   running while it follows. Out of its dust; Stone Skin first; the Explosion kept for the
//!   windows. With no lit bud left it is shot from range, as it is between windows.
//! - **Done** ([`done`]): with the Emperor down, the Night Watchman's Key in her bag and the
//!   growth by the stone taken, she goes out by the gate.

use std::collections::BTreeMap;

use jane_core::action::Action;
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Fx, Rect, SpellId, ZoneId};
use jane_sim::ids::PropId;
use jane_sim::{Command, InputFrame, Prop, Unit, View};

use crate::Act;
use crate::crawl::{Reach, Try};
use crate::fight;
use crate::nav::{Go, dist, stick};
use crate::sense::{self, holds, knows, prop_centre};
use crate::task::{Ctx, Status, Task, UseProp};

/// Her walk, roughly, in `Fx` a tick (eight cells a second): how early she sets off for a bud.
const WALK_FX_PER_TICK: i32 = 8 * CELL_FX / 60;

/// What the forest's tactic carries between frames (in [`Ctx`]).
#[derive(Debug, Default)]
pub struct Forest {
    /// A bud being grown or the stone being opened for the Emperor, with the prop.
    job: Option<(Task, PropId)>,
    /// Props that would not take (a bud in shade, the stone short), with the frame to try again.
    refused: BTreeMap<PropId, u32>,
    /// Butterflies (their keys) known to be gone from their glades: in her bag.
    gone: std::collections::BTreeSet<jane_core::Sym>,
}

fn here(v: &View<'_>) -> bool {
    v.zone() == ZoneId::Forest
}

/// The summoning glade (the boss room's `inner` rect, bound as `forest_arena`).
fn arena(v: &View<'_>) -> Option<Rect> {
    v.sym("forest_arena").and_then(|s| v.rect(s))
}

fn def_id(p: &Prop) -> &'static str {
    jane_data::catalog().story.prop(p.def).id
}

/// A bud in the summoning glade: the Emperor's, not a butterfly's.
fn boss_bud(v: &View<'_>, p: &Prop) -> bool {
    def_id(p) == "bud"
        && arena(v).is_some_and(|r| {
            let (x, y) = prop_centre(p).cell();
            r.contains(x, y)
        })
}

/// Butterflies in her bag.
fn caught(v: &View<'_>) -> u32 {
    holds(v, sense::item("butterfly"))
}

/// The summoning stone.
fn stone<'a>(v: &View<'a>) -> Option<&'a Prop> {
    v.props().find(|p| def_id(p) == "summon_stone")
}

/// How many butterflies the stone asks for, as it says it when she is short ("Five, the stone
/// says, and it says it in trades").
const ASKS: u32 = 5;

/// Is the point in the sky's light (a showing sunbeam or moonbeam, or a bank in the sun)? What
/// she sees: the beam is on the grass or it is not.
fn sky_lit(v: &View<'_>, p: &Prop) -> bool {
    let at = prop_centre(p);
    v.props().any(|l| {
        v.light_showing(l).is_some_and(|light| {
            let r = i64::from(light.radius.0);
            light.sky && dist_sq(prop_centre(l), at) <= r * r * 4 / 9
        })
    })
}

/// Should the crawl leave this prop alone? (See the module's doc.)
pub fn skip(v: &View<'_>, p: &Prop) -> bool {
    if !here(v) {
        return false;
    }
    // The glade is not opened so late that the sunbeams go out halfway through the fight.
    let short = caught(v) < ASKS || v.hour() >= 18 || v.lamps_lit();
    match def_id(p) {
        // Every bud is the forest's own business: a glade's (`offers`, when she has the net to
        // keep what comes to it) and the Emperor's (`engage`).
        "hedge_seed" | "bud" => true,
        "summon_stone" | "forest_vine_bank" => short,
        _ => false,
    }
}

/// The butterfly a bud calls (its `use` list sends it to the flower), as the sym of its key.
fn called_by(v: &View<'_>, bud: &Prop) -> Option<jane_core::Sym> {
    let l = v.prop_spawn(bud)?.use_list?;
    v.list(l).iter().find_map(|a| match *a {
        Action::Send { unit, .. } => Some(v.key_sym(unit)),
        _ => None,
    })
}

/// The glades' own buds (not the Emperor's), each with the butterfly it calls, that butterfly
/// not yet known to be gone.
fn glade_buds<'a>(v: &View<'a>, f: &Forest) -> Vec<(&'a Prop, jane_core::Sym)> {
    v.props()
        .filter(|p| def_id(p) == "bud" && !boss_bud(v, p))
        .filter_map(|p| called_by(v, p).map(|k| (p, k)))
        .filter(|(_, k)| !f.gone.contains(k))
        .collect()
}

/// What the forest puts up for the crawl to do, before its own order where it matters:
///
/// - the Collector, a boss that is not the dungeon's, as any other thing in the way (class 7,
///   not the last boss's 9): the net is his;
/// - with the net: the cases again for what is in them now (class 1); every butterfly she can
///   reach (class 1), and each glade's bud whose butterfly she has not got, grown when it is in
///   the light (the butterfly comes to it) or walked to when not (class 1): a glade is where a
///   butterfly is, and the bud says which one;
/// - the hearth, the first time she can reach it (class 1), and with the five in hand and hurt,
///   a bed or a fire first (class 0): the glade has neither.
pub fn offers(v: &View<'_>, cx: &Ctx, reach: &Reach) -> Vec<(u8, i64, Try, Task)> {
    let mut out = Vec::new();
    if !here(v) {
        return out;
    }
    let at = v.body().pos;
    let cat = jane_data::catalog();
    for u in sense::enemies(v) {
        if cat.combat.unit(u.def).boss && crate::crawl::boss_of(ZoneId::Forest) != Some(u.def) && reach.point(u.pos) {
            out.push((7, dist(at, u.pos), Try::Fight(u.id), Task::Hunt(u.id)));
        }
    }
    let rest = v
        .props()
        .filter(|p| cat.story.prop(p.def).rest && reach.beside(p) && v.prop_spawn(p).is_some_and(|s| s.talk.is_some()))
        .min_by_key(|p| (sense::to_prop(p, at), p.id));
    if let Some(r) = rest {
        // The hearth the first time she can reach it (a death wakes her where she last slept,
        // and the county's road back to the gate is long); whole again before the glade.
        let first = !cx.used.contains_key(&(v.zone(), r.id));
        if first || caught(v) >= ASKS && sense::hp_permille(v.body()) < 900 {
            out.push((u8::from(first), sense::to_prop(r, at), Try::Rest(r.id), Task::Use(UseProp::new(r.id))));
        }
    }
    if holds(v, sense::item("net")) == 0 {
        return out;
    }
    if holds(v, sense::item("key_works")) == 0 {
        if let Some(c) = v.props().find(|p| def_id(p) == "forest_cases" && reach.beside(p)) {
            out.push((1, sense::to_prop(c, at), Try::Prop(c.id), Task::Use(UseProp::new(c.id))));
        }
    }
    if caught(v) >= ASKS {
        return out;
    }
    for u in sense::talkers(v) {
        let Some(tree) = cat.combat.unit(u.def).talk else { continue };
        if !sense::tree_has(v, tree, &|a| matches!(a, Action::Give(_))) || !reach.near(u.pos, 2) {
            continue;
        }
        // Far off, the walk first: a butterfly across the forest is asleep by the time she is
        // halfway (out of the ground about her that lives), and a talk to no one fails.
        let d = dist(at, u.pos);
        if d > i64::from(12 * CELL_FX) {
            let (x, y) = u.pos.cell();
            out.push((1, d, Try::Explore(x, y), Task::Walk { to: u.pos, near: Fx(6 * CELL_FX) }));
        } else {
            out.push((1, d, Try::Talk(u.id), Task::talk(u.id)));
        }
    }
    let grow = sense::spell("grow");
    for (b, fly) in glade_buds(v, &cx.forest) {
        let seen = sense::unit_by_key(v, fly).is_some_and(|u| dist(at, u.pos) <= i64::from(12 * CELL_FX));
        if seen || !reach.beside(b) {
            continue;
        }
        let d = sense::to_prop(b, at);
        // Far off, the walk first (a cast gives up after a minute and a half, a long way round
        // and a fight or two included); beside it, Grow when it is in the light.
        let far = d > i64::from(12 * CELL_FX);
        if far {
            let (x, y) = prop_centre(b).cell();
            out.push((1, d, Try::Explore(x, y), Task::Walk { to: prop_centre(b), near: Fx(6 * CELL_FX) }));
        } else if !b.used && !b.on && knows(v, grow) && sky_lit(v, b) {
            out.push((1, d, Try::Cast(b.id), Task::cast(grow, b.id)));
        } else {
            out.push((1, d, Try::Prop(b.id), Task::Walk { to: prop_centre(b), near: Fx(2 * CELL_FX) }));
        }
    }
    out
}

/// Every frame in the forest: a glade's butterfly not there with her standing by its bud is
/// gone (in her bag); and once the stone is open the Emperor is the only thing to do (fed and
/// idle at a bud, or across the glade, it is still hunted: a rest or a jar can wait, and it
/// mends if she leaves).
pub fn look(v: &View<'_>, cx: &mut Ctx) {
    if !here(v) {
        return;
    }
    let me = v.body().pos;
    let gone: Vec<jane_core::Sym> = glade_buds(v, &cx.forest)
        .into_iter()
        .filter(|(b, k)| sense::to_prop(b, me) < i64::from(4 * CELL_FX) && sense::unit_by_key(v, *k).is_none())
        .map(|(_, k)| k)
        .collect();
    cx.forest.gone.extend(gone);
    if cx.fight.hunt.is_some() || !v.props().any(|p| boss_bud(v, p)) {
        return;
    }
    let boss = crate::crawl::boss_of(ZoneId::Forest);
    if let Some(e) = sense::enemies(v).into_iter().find(|u| Some(u.def) == boss) {
        cx.fight.hunt = Some(e.id);
    }
}

/// Apples kept back for the Emperor while he is not on her: a fire mends as well as an apple
/// on the way, and there is none in his glade.
pub fn keep_food(v: &View<'_>) -> u32 {
    if !here(v) {
        return 0;
    }
    let boss = crate::crawl::boss_of(ZoneId::Forest);
    if sense::enemies(v).iter().any(|u| Some(u.def) == boss && fight::on_me(v, u)) { 0 } else { 4 }
}

/// Is the forest shut for her (the sign: "Open from sunrise to sunset")? From the hour the
/// sunbeams go out and the lamps come on nothing grows in the glades and the butterflies go to
/// roost; unless the Emperor is on her (that fight is finished where it is), the night is
/// waited out by the hearth, mended there when hurt: the frame to hold, `None` while it is open.
pub fn night(v: &View<'_>, cx: &mut Ctx, reach: &Reach) -> Option<Act> {
    if !here(v) || !v.lamps_lit() {
        return None;
    }
    let boss = crate::crawl::boss_of(ZoneId::Forest);
    if sense::enemies(v).iter().any(|u| Some(u.def) == boss && fight::on_me(v, u)) {
        return None;
    }
    let cat = jane_data::catalog();
    let at = v.body().pos;
    let Some(fire) = v
        .props()
        .filter(|p| cat.story.prop(p.def).rest && reach.beside(p) && v.prop_spawn(p).is_some_and(|s| s.talk.is_some()))
        .min_by_key(|p| (sense::to_prop(p, at), p.id))
    else {
        return Some(Act::idle());
    };
    if sense::to_prop(fire, at) <= i64::from(3 * CELL_FX) {
        return Some(Act::idle());
    }
    Some(match cx.nav.go(v, prop_centre(fire), Fx(2 * CELL_FX), false) {
        Go::Walk(f) => Act::hold(f),
        _ => Act::idle(),
    })
}

/// Is the forest not worth setting out for at `hour`? It is open from sunrise to sunset, and
/// what it asks takes most of a day: from seven till six in the evening.
pub fn shut_hour(z: ZoneId, hour: u8) -> bool {
    z == ZoneId::Forest && !(7..18).contains(&hour)
}

/// With the Emperor down, is what the story wants of the forest done? The Night Watchman's Key
/// in her bag and the growth by the stone taken (the reward glade's page and jar): she goes out
/// by the gate, and the far glades' jars are left for another visit.
pub fn done(v: &View<'_>, reach: &Reach) -> bool {
    if !here(v) || holds(v, sense::item("key_works")) == 0 {
        return false;
    }
    let Some(st) = stone(v) else { return true };
    let at = prop_centre(st);
    !v.props().any(|p| {
        !p.used
            && reach.beside(p)
            && dist(prop_centre(p), at) < i64::from(40 * CELL_FX)
            && sense::prop_does(v, p, &|a| matches!(a, Action::Grow { .. }))
    })
}

/// Is it held where it is (rooted, or stunned: `stun`), by a status?
fn held(u: &Unit, now: jane_core::Tick, stun: bool) -> bool {
    let cat = jane_data::catalog();
    u.statuses.iter().any(|s| {
        let e = cat.combat.effect(s.effect);
        s.until > now && (e.stun || !stun && e.speed.0 == 0)
    })
}

/// The Emperor's buds she may grow now: shown, not grown, in the sky's light, not refused;
/// nearest her first.
fn lit_buds<'a>(v: &View<'a>, f: &Forest) -> Vec<&'a Prop> {
    let at = v.body().pos;
    let now = v.frame();
    let mut out: Vec<&Prop> = v
        .props()
        .filter(|p| {
            !p.hidden
                && !p.used
                && !p.on
                && boss_bud(v, p)
                && sky_lit(v, p)
                && f.refused.get(&p.id).is_none_or(|&t| t <= now)
        })
        .collect();
    out.sort_by_key(|p| (sense::to_prop(p, at), p.id));
    out
}

fn spell_ready(v: &View<'_>, s: SpellId, reserve: i32) -> bool {
    let me = v.body();
    let def = jane_data::catalog().combat.spell(s);
    knows(v, s) && fight::ready(me, s, v.tick()) && me.mp.0 >= def.mp.0 + reserve
}

/// A bolt at it when one is ready (the Explosion first), keeping `reserve` mana back for the
/// next bud; `None` when none is.
fn bolt(v: &View<'_>, e: &Unit, reserve: i32, spells: &[&str]) -> Option<Act> {
    let cat = jane_data::catalog();
    let me = v.body();
    let dir = jane_core::angle::iatan2(e.pos.y.0 - me.pos.y.0, e.pos.x.0 - me.pos.x.0);
    if !v.sight(me.pos, e.pos) {
        return None;
    }
    let g = fight::gap(me, e);
    spells.iter().map(|n| sense::spell(n)).find_map(|s| {
        (spell_ready(v, s, reserve) && g <= i64::from(cat.combat.spell(s).range.0) * 9 / 10).then(|| Act {
            frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
            cmds: vec![Command::Cast { spell: s, on: Some(e.id) }],
        })
    })
}

/// Stood up to: a bolt when one is ready, her hands when it is in reach, walking in when not.
fn stand_up(v: &View<'_>, cx: &mut Ctx, e: &Unit, reserve: i32) -> Act {
    if let Some(a) = bolt(v, e, reserve, &["explosion", "icebolt"]) {
        return a;
    }
    let cat = jane_data::catalog();
    let me = v.body();
    let dir = jane_core::angle::iatan2(e.pos.y.0 - me.pos.y.0, e.pos.x.0 - me.pos.x.0);
    let melee = sense::spell("melee_player");
    let range = i64::from(cat.combat.spell(melee).range.0);
    if fight::gap(me, e) <= range {
        let cmds = if fight::ready(me, melee, v.tick()) {
            vec![Command::Cast { spell: melee, on: Some(e.id) }]
        } else {
            Vec::new()
        };
        return Act { frame: InputFrame { aim: Some(dir), mv_dir: dir, mv_mag: 10, ..InputFrame::IDLE }, cmds };
    }
    let frame = match cx.nav.go(v, e.pos, Fx(range as i32 + 6 * 256), false) {
        Go::Walk(f) => f,
        _ => stick(me.pos, e.pos, false),
    };
    Act::hold(InputFrame { aim: Some(dir), ..frame })
}

/// Shot from `off` cells away and no nearer: backing off when it is closer, an Icebolt whenever
/// one is ready (the frost slows it; the Explosion is kept for a window, where it lands twice).
/// She keeps inside two thirds of its leash from its home, or it goes home and mends.
fn shoot(v: &View<'_>, cx: &mut Ctx, e: &Unit, reserve: i32, off: i32) -> Act {
    let me = v.body();
    let dir = jane_core::angle::iatan2(e.pos.y.0 - me.pos.y.0, e.pos.x.0 - me.pos.x.0);
    if fight::gap(me, e) < i64::from(off * CELL_FX) {
        let leash = i64::from(jane_data::catalog().combat.unit(e.def).leash.0);
        let frame = fight::away_from(v, cx, e.pos, Some((e.home, leash * 2 / 3)));
        return match bolt(v, e, reserve, &["icebolt"]) {
            Some(a) => Act { frame: InputFrame { aim: Some(dir), ..frame }, cmds: a.cmds },
            None => Act::hold(InputFrame { aim: Some(dir), ..frame }),
        };
    }
    bolt(v, e, reserve, &["icebolt"]).unwrap_or_else(|| Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE }))
}

/// One frame of the job in hand (a bud grown, the stone opened); `None` when there is none or it
/// has just ended.
fn work(v: &View<'_>, cx: &mut Ctx) -> Option<Act> {
    let (mut t, p) = cx.forest.job.take()?;
    match t.tick(v, cx) {
        Status::Act(a) => {
            cx.forest.job = Some((t, p));
            Some(a)
        }
        Status::Done => None,
        Status::Failed(_) => {
            cx.forest.refused.insert(p, v.frame() + 60 * 60);
            None
        }
    }
}

/// The fight with the forest's boss, the way its glade is built to be played; `None` for any
/// other fight, or when there is nothing the glade offers (the crawl's plain fight then).
pub fn engage(v: &View<'_>, cx: &mut Ctx, e: &Unit) -> Option<Act> {
    if !here(v) {
        return None;
    }
    let cat = jane_data::catalog();
    // A boss on her comes before the bat at her elbow (a boss that has her in its eye from the
    // far side of a hedge is not on her).
    let near = |u: &Unit| dist(u.pos, v.body().pos) < i64::from(10 * CELL_FX);
    let boss_on = sense::enemies(v).into_iter().find(|u| cat.combat.unit(u.def).boss && fight::on_me(v, u) && near(u));
    let e = match boss_on {
        Some(b) if !cat.combat.unit(e.def).boss => b,
        _ => e,
    };
    if !cat.combat.unit(e.def).boss {
        return None;
    }
    let me = v.body();
    let now = v.tick();
    // Stone Skin before it is on her: his net and his hands, its wings, are all blows (not the
    // mana shield: the mana is for Grow and the bolts).
    let skin = sense::item("potion_stoneskin");
    let skinned =
        cat.combat.effect_id("stoneskin").is_some_and(|s| me.statuses.iter().any(|x| x.effect == s && x.until > now));
    if dist(me.pos, e.pos) < i64::from(12 * CELL_FX)
        && !skinned
        && holds(v, skin) > 0
        && !me.item_cooldowns.iter().any(|&(c, until)| c == skin && until > now)
    {
        return Some(Act::press(Command::Item(skin)));
    }
    if crate::crawl::boss_of(ZoneId::Forest) != Some(e.def) {
        // The Collector throws his net from twelve metres: backing off from him is only being
        // netted further off. He is stood up to, with everything she has.
        return Some(stand_up(v, cx, e, 0));
    }
    // Out of the dust: it hangs a while and bites every second she stands in it.
    let body = i64::from(cat.combat.unit(me.def).bounds.0);
    if let Some(g) = v
        .grounds()
        .iter()
        .find(|g| g.faction != me.faction && g.until > now && dist(g.pos, me.pos) < i64::from(g.radius.0) + body)
    {
        let dir = jane_core::angle::iatan2(me.pos.y.0 - g.pos.y.0, me.pos.x.0 - g.pos.x.0);
        let from = if g.pos == me.pos { e.pos } else { g.pos };
        let frame = match fight::retreat_point(v, me.pos, from, None) {
            Some(to) => match cx.nav.go(v, to, Fx::from_px(3), true) {
                Go::Walk(f) => f,
                _ => InputFrame { sprint: true, ..InputFrame::walk(dir) },
            },
            None => InputFrame { sprint: true, ..InputFrame::walk(dir) },
        };
        return Some(Act::hold(InputFrame { sprint: true, ..frame }));
    }
    let buds: Vec<&Prop> = v.props().filter(|p| boss_bud(v, p)).collect();
    // The stone first: its buds come up out of the grass (she sees none before) when it is
    // opened with the butterflies.
    if buds.is_empty() {
        let s = stone(v)?;
        if caught(v) < ASKS || cx.forest.refused.get(&s.id).is_some_and(|&t| t > v.frame()) {
            return None;
        }
        if cx.forest.job.as_ref().is_none_or(|(_, p)| *p != s.id) {
            cx.forest.job = Some((Task::Use(UseProp::new(s.id)), s.id));
        }
        return work(v, cx).or_else(|| Some(Act::idle()));
    }
    let grow = sense::spell("grow");
    let grow_mp = cat.combat.spell(grow).mp.0;
    let lit = lit_buds(v, &cx.forest);
    let reserve = if lit.is_empty() { 0 } else { grow_mp };
    // The next bud, timed: she sets off for it so as to be beside it as Grow comes round (a
    // window spent to its end and then a walk is a walk with it on her back). Once set off,
    // she keeps going.
    let left = me.cooldowns.iter().find(|&&(c, _)| c == grow).map_or(0, |&(_, until)| until.0.saturating_sub(now.0));
    let going = cx.forest.job.as_ref().is_some_and(|(_, p)| lit.iter().any(|b| b.id == *p));
    if let Some(bud) = lit.first().filter(|_| me.mp.0 >= grow_mp) {
        let walk = u32::try_from(sense::to_prop(bud, me.pos) / i64::from(WALK_FX_PER_TICK)).unwrap_or(u32::MAX);
        if going || left <= walk.saturating_add(30) {
            // Pushed off it while she waited (a blow, the dust): back beside it first, or Grow finds
            // nothing within reach.
            let off = matches!(cx.forest.job, Some((Task::WorldCast { stage: 1 | 2, .. }, p))
                if v.prop(p).is_some_and(|b| sense::to_prop(b, me.pos) > i64::from(CELL_FX * 3 / 2)));
            if !going || off {
                cx.forest.job = Some((Task::cast(grow, bud.id), bud.id));
            }
            // Beside it early, and it within reach: hands on it while Grow comes round.
            let waiting = matches!(cx.forest.job, Some((Task::WorldCast { stage: 1, .. }, _)));
            let melee = sense::spell("melee_player");
            if waiting && left > 0 && fight::gap(me, e) <= i64::from(cat.combat.spell(melee).range.0) {
                return Some(stand_up(v, cx, e, reserve));
            }
            // On the way: a bolt of frost at it now and then (it slows), and at a run.
            let walking = matches!(cx.forest.job, Some((Task::WorldCast { stage: 0, .. }, _)));
            if walking && !held(e, now, false) && fight::gap(me, e) < i64::from(6 * CELL_FX) {
                if let Some(a) = bolt(v, e, reserve, &["icebolt"]) {
                    let chilled = cat
                        .combat
                        .effect_id("chilled")
                        .is_some_and(|c| e.statuses.iter().any(|s| s.effect == c && s.until > now));
                    if !chilled {
                        return Some(a);
                    }
                }
            }
            if let Some(mut a) = work(v, cx) {
                if walking && a.frame.mv_mag > 0 {
                    a.frame.sprint = true;
                }
                return Some(a);
            }
        }
    }
    // On its way down to a bud she grew (sent: it strikes at nothing on the way): she stands
    // off the flower it is coming to (a body on the flower keeps it from landing, and then it
    // never feeds), and waits for it there.
    if let Some(o) = e.order.as_deref() {
        let dir = jane_core::angle::iatan2(e.pos.y.0 - me.pos.y.0, e.pos.x.0 - me.pos.x.0);
        if dist(me.pos, o.to) < i64::from(4 * CELL_FX) {
            let frame = fight::away_from(v, cx, o.to, None);
            return Some(Act::hold(InputFrame { aim: Some(dir), ..frame }));
        }
        return Some(Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE }));
    }
    // Feeding: it does nothing else, and everything she has goes into it. Only rooted there (a
    // status that holds it without stunning it: it still strikes): out of its reach, and every
    // bolt into it.
    if held(e, now, true) {
        return Some(stand_up(v, cx, e, reserve));
    }
    if held(e, now, false) {
        return Some(shoot(v, cx, e, reserve, 3));
    }
    // Between windows: kept at bolt range, the frost on it slowing it.
    Some(shoot(v, cx, e, reserve, 6))
}
