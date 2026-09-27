//! The bar (`bot.ts fight`, SYSTEMS.md): walk up to it and swing, or throw a bolt from range
//! when she has one and the mana; eat an apple when low; back off when there is nothing left to
//! eat. Only what a person presses: a held stick with an aim, and `Cast`/`Item` commands.
//!
//! A bot fights what is fighting her (an enemy in combat with her body as its target) and what
//! its plan sends it after (`hunt`); it walks past the rest, as a person keeping to the road
//! does. Her health does not come back on its own (only a fire, a bed, food or a potion mends
//! her), so a fight is paid for: below [`FLEE_BELOW`] with nothing to eat she backs off until
//! nothing is on her, and the plan takes her to a fire. A pool laid under her that has not
//! bitten yet (the Headmaster's hand-bell, a lobbed charge) is a tell, and she steps out of it
//! before anything else.

use jane_core::num::CELL_FX;
use jane_core::{Fx, ItemId, SpellId, Tick};
use jane_data::SpellKind;
use jane_sim::ids::UnitId;
use jane_sim::state::CombatState;
use jane_sim::{Command, InputFrame, Unit, View};

use crate::Act;
use crate::nav::{Go, dist, stick};
use crate::sense::{self, enemies, holds, hp_permille, knows};
use crate::task::Ctx;

/// Eat below this (permille of the most hp).
pub const EAT_BELOW: i32 = 450;
/// Back off below this with nothing to eat.
pub const FLEE_BELOW: i32 = 330;

#[derive(Debug, Default)]
pub struct Fight {
    /// What the plan sent her after.
    pub hunt: Option<UnitId>,
    /// What she is hitting now.
    pub target: Option<UnitId>,
    /// Frames spent on the target.
    pub t: u32,
    /// Frames left backing off.
    pub fleeing: u32,
    /// Fights begun, and fights she backed out of.
    pub fights: u32,
    pub fled: u32,
    /// Where she is backing off to.
    pub retreat: Option<jane_core::Vec2>,
    /// Units her feet could find no way to, and the frame to try again.
    pub unreachable: std::collections::BTreeMap<UnitId, u32>,
}

pub fn ready(u: &Unit, s: SpellId, now: Tick) -> bool {
    let def = jane_data::catalog().combat.spell(s);
    !u.cooldowns.iter().any(|&(c, until)| c == s && until > now)
        && (def.gcd_immune || u.gcd_until <= now)
        && u.mp >= def.mp
}

/// An item she may use now: off its cooldown, and off the global one (the sim refuses it on
/// the GCD, and a press refused every frame is a frame she stood still for: she froze under the
/// Foreman pressing stone skin after each bolt).
pub fn item_ready(u: &Unit, i: ItemId, now: Tick) -> bool {
    !u.item_cooldowns.iter().any(|&(c, until)| c == i && until > now) && u.gcd_until <= now
}

/// Body gap between two units (centre distance less both bodies), `Fx`.
pub fn gap(a: &Unit, b: &Unit) -> i64 {
    let cat = jane_data::catalog();
    (dist(a.pos, b.pos) - i64::from(cat.combat.unit(a.def).bounds.0) - i64::from(cat.combat.unit(b.def).bounds.0))
        .max(0)
}

/// The cell within a short walk that is farthest from `from` (a corridor's far end, not the
/// wall at her back): a flood over the ground about her.
///
/// `tether` keeps her inside a circle (its home and leash, shrunk): kited past its leash a thing
/// walks home and mends, and the fight starts over.
pub fn retreat_point(
    v: &View<'_>,
    me: jane_core::Vec2,
    from: jane_core::Vec2,
    tether: Option<(jane_core::Vec2, i64)>,
) -> Option<jane_core::Vec2> {
    use std::collections::{BTreeSet, VecDeque};
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    let mut best: Option<(i64, (i32, i32))> = None;
    while let Some((c, steps)) = q.pop_front() {
        let at = jane_core::Vec2::centre(c.0, c.1);
        let score = dist(at, from) - i64::from(steps) * i64::from(CELL_FX) / 3;
        let inside = tether.is_none_or(|(home, r)| dist(at, home) <= r);
        if inside && best.is_none_or(|b| (score, c) > b) {
            best = Some((score, c));
        }
        if steps >= 18 || seen.len() > 1500 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back((n, steps + 1));
            }
        }
    }
    best.map(|(_, c)| jane_core::Vec2::centre(c.0, c.1))
}

/// Running from something at `from`: to the retreat point, by the path there.
pub fn away_from(
    v: &View<'_>,
    cx: &mut Ctx,
    from: jane_core::Vec2,
    tether: Option<(jane_core::Vec2, i64)>,
) -> InputFrame {
    let me = v.body().pos;
    let far = cx.fight.retreat.filter(|r| {
        dist(*r, from) > dist(me, from) + i64::from(CELL_FX)
            && dist(*r, me) > i64::from(CELL_FX)
            && tether.is_none_or(|(home, radius)| dist(*r, home) <= radius)
    });
    let to = match far {
        Some(r) => r,
        None => match retreat_point(v, me, from, tether) {
            Some(r) => {
                cx.fight.retreat = Some(r);
                r
            }
            None => return InputFrame::IDLE,
        },
    };
    match cx.nav.go(v, to, Fx::from_px(3), true) {
        Go::Walk(f) => InputFrame { sprint: true, ..f },
        _ => {
            cx.fight.retreat = None;
            stick(me, to, true)
        }
    }
}

/// A hostile pool she should be out of (one that has not bitten yet; one that has, while her
/// feet are her own) with her in it (her body's edge inside its radius, with a cell to spare):
/// where it lies.
pub fn tell_under(v: &View<'_>) -> Option<jane_core::Vec2> {
    let me = v.body();
    let body = i64::from(jane_data::catalog().combat.unit(me.def).bounds.0);
    let now = v.tick();
    let cat = jane_data::catalog();
    // Not bitten yet: its first pulse, a `delay` after the cast, is still to come. One that has
    // bitten is walked out of only by feet that can: slowed in a web, stepping out of it for
    // ever, she never ate or struck back while the spider webbed her again (the pipes).
    let unbitten = |g: &jane_sim::state::Ground| {
        cat.combat.spell(g.spell).ground.is_some_and(|p| {
            p.delay.0 > 1 && g.until.0.saturating_sub(g.next_pulse.0) + 1 >= p.duration.0.saturating_sub(p.delay.0)
        })
    };
    let free = jane_sim::status::speed_factor(me, now) >= 1000;
    v.grounds()
        .iter()
        .filter(|g| g.faction != me.faction && g.next_pulse > now && g.until > now && (free || unbitten(g)))
        .find(|g| dist(g.pos, me.pos) <= i64::from(g.radius.0) + body + i64::from(CELL_FX))
        .map(|g| g.pos)
}

/// Out of a pool before it lands: straight away from its middle, at a run, unless a wall is that
/// way (a pool laid on her in a doorway), then the nearest way round that is open ground.
fn step_out(v: &View<'_>, cx: &mut Ctx, from: jane_core::Vec2) -> InputFrame {
    let me = v.body().pos;
    let pool = v.grounds().iter().find(|g| g.pos == from).map_or(2 * CELL_FX, |g| g.radius.0);
    // Standing on its middle: any way will do; the way she faces is as good as any.
    let from = if from == me {
        let (dx, dy) = v.body().facing.delta();
        jane_core::Vec2 { x: Fx(me.x.0 - dx * CELL_FX), y: Fx(me.y.0 - dy * CELL_FX) }
    } else {
        from
    };
    cx.fight.retreat = None;
    // The eight ways out, the most nearly straight away first; the first that lands on open
    // ground she can see clear to (a diagonal is 181/256 of a straight step).
    let body = jane_data::catalog().combat.unit(v.body().def).bounds.0;
    let out = i64::from(pool + body + CELL_FX);
    let (ax, ay) = (i64::from(me.x.0 - from.x.0), i64::from(me.y.0 - from.y.0));
    let mut ways: Vec<(i64, i64, i64)> = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]
        .into_iter()
        .map(|(dx, dy): (i64, i64)| (-(dx * ax + dy * ay) * if dx != 0 && dy != 0 { 181 } else { 256 }, dx, dy))
        .collect();
    ways.sort();
    for (_, dx, dy) in ways {
        let step = if dx != 0 && dy != 0 { out * 181 / 256 } else { out };
        let to = jane_core::Vec2 {
            x: Fx((i64::from(me.x.0) + dx * step) as i32),
            y: Fx((i64::from(me.y.0) + dy * step) as i32),
        };
        let (cx_, cy_) = to.cell();
        if crate::nav::walkable(v, cx_, cy_) && v.sight(me, to) {
            return stick(me, to, true);
        }
    }
    // No way out in sight: the way round to the open ground farthest from its middle.
    let (ax, ay) = (me.x.0 + (me.x.0 - from.x.0).signum() * CELL_FX, me.y.0 + (me.y.0 - from.y.0).signum() * CELL_FX);
    let ahead = jane_core::Vec2 { x: Fx(ax), y: Fx(ay) }.cell();
    if !crate::nav::walkable(v, ahead.0, ahead.1) {
        if let Some(to) = retreat_point(v, me, from, None) {
            if let Go::Walk(f) = cx.nav.go(v, to, Fx::from_px(3), true) {
                return InputFrame { sprint: true, ..f };
            }
        }
    }
    stick(from, me, true)
}

/// Food she holds and may eat now (what a dungeon's tactic keeps back for its boss only when
/// she is near the end).
pub fn food(v: &View<'_>) -> Option<ItemId> {
    let me = v.body();
    let keep = if hp_permille(me) < 250 { 0 } else { crate::tactics::forest::keep_food(v) };
    ["apple", "grape"].into_iter().map(sense::item).find(|&i| holds(v, i) > keep && item_ready(me, i, v.tick()))
}

/// Does she hold anything to eat at all (ready or not), past what is kept back for a boss?
pub fn has_food(v: &View<'_>) -> bool {
    let keep = crate::tactics::forest::keep_food(v);
    ["apple", "grape"].into_iter().any(|n| holds(v, sense::item(n)) > keep)
}

/// The most one blow of `u`'s can take off her, in points: its row's book (and its phases'),
/// each spell's power at its top roll, no crit. What its row says a player could read off it.
pub fn max_hit(u: &Unit) -> i32 {
    let cat = jane_data::catalog();
    let def = cat.combat.unit(u.def);
    let books = std::iter::once(def.book).chain(def.phases.iter().map(|p| p.book));
    books
        .flat_map(|b| b.iter())
        .filter_map(|&s| cat.combat.spell(s).power)
        .map(|p| {
            let stat = i64::from(match p.stat {
                jane_core::action::Stat::Strength => u.strength,
                jane_core::action::Stat::Spirit => u.spirit,
            });
            let milli = stat * 1_000_000 / i64::from(p.div.max(1))
                + stat * 1_000_000 / i64::from(p.var_div.max(1))
                + i64::from(p.flat.0);
            (milli / 1000) as i32
        })
        .max()
        .unwrap_or(0)
}

/// Something to eat, if she is low and can.
/// Under fire from something with no feet that she is not fighting (a sentry by the fire she
/// woke at, a cactus by the path): the nearest ground out of its reach or its sight, and the stick
/// toward it. Stood still in its reach (waiting out the night, nothing left to choose), it shot
/// her dead again and again where she woke.
pub fn out_of_fire(v: &View<'_>, cx: &mut Ctx) -> Option<InputFrame> {
    use std::collections::{BTreeSet, VecDeque};
    let me = v.body();
    let cat = jane_data::catalog();
    let shooters: Vec<(jane_core::Vec2, i64)> = enemies(v)
        .into_iter()
        .filter(|u| on_me(v, u) && rooted(u) && reaches_her(v, u))
        .map(|u| {
            let far = jane_sim::combat::book_of(u).iter().map(|&s| i64::from(cat.combat.spell(s).range.0)).max().unwrap_or(0);
            (u.pos, far + i64::from(2 * CELL_FX))
        })
        .collect();
    if shooters.is_empty() {
        return None;
    }
    let safe = |c: (i32, i32)| {
        let at = jane_core::Vec2::centre(c.0, c.1);
        shooters.iter().all(|&(p, r)| dist(at, p) > r || !v.sight(p, at))
    };
    let start = me.pos.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    while let Some((c, steps)) = q.pop_front() {
        if c != start && safe(c) {
            let to = jane_core::Vec2::centre(c.0, c.1);
            return Some(match cx.nav.go(v, to, Fx::from_px(4), true) {
                Go::Walk(f) => f,
                _ => stick(me.pos, to, true),
            });
        }
        if steps >= 30 || seen.len() > 3000 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back((n, steps + 1));
            }
        }
    }
    None
}

pub fn eat(v: &View<'_>) -> Option<Command> {
    if hp_permille(v.body()) >= EAT_BELOW {
        return None;
    }
    // The School: the apples are kept for the Timekeeper (`tactics::school::may_eat`).
    if v.zone() == jane_core::ZoneId::School && !crate::tactics::school::may_eat(v) {
        return None;
    }
    food(v).map(Command::Item)
}

/// Milli-points a second a unit's best melee does, as its rows say (the fixed part and half the
/// random).
fn melee_rate(u: &Unit) -> i64 {
    let cat = jane_data::catalog();
    cat.combat
        .unit(u.def)
        .book
        .iter()
        .map(|&s| cat.combat.spell(s))
        .filter(|s| s.kind == SpellKind::Melee)
        .filter_map(|s| {
            let p = s.power.as_ref()?;
            let stat = i64::from(match p.stat {
                jane_core::action::Stat::Strength => u.strength,
                jane_core::action::Stat::Spirit => u.spirit,
            });
            let hit = stat * 1_000_000 / i64::from(p.div.max(1))
                + stat * 1000 / i64::from(p.var_div.max(1)) * 1000 / 2
                + i64::from(p.flat.0);
            Some(hit * 60 / i64::from(s.cooldown.0.max(1)))
        })
        .max()
        .unwrap_or(0)
}

/// Would she put down everything on her before it put her down, blow for blow as the rows say
/// (with a third to spare)?
pub fn would_win(v: &View<'_>) -> bool {
    let me = v.body();
    let on: Vec<&Unit> = enemies(v).into_iter().filter(|u| on_me(v, u)).collect();
    let (hp, rate) = on.iter().fold((0i64, 0i64), |(h, r), u| (h + i64::from(u.hp.0), r + melee_rate(u)));
    let mine = melee_rate(me);
    mine > 0 && hp * rate * 3 < i64::from(me.hp.0) * mine * 2
}

/// Out of doors, on her way, is `id` better run from than fought? Not what she was sent after;
/// only when she would lose the trade of blows with everything on her ([`would_win`]); and
/// only while she can get away: it is slower than her walk, or she has the legs for a sprint.
pub fn outrun(v: &View<'_>, cx: &Ctx, id: UnitId) -> bool {
    if v.zone() != jane_core::ZoneId::County || cx.fight.hunt == Some(id) || would_win(v) {
        return false;
    }
    let Some(t) = v.unit(id) else { return false };
    // A thing with no feet is not outrun: it shoots, and its webs hold her in its reach.
    if rooted(t) {
        return false;
    }
    let me = v.body();
    let cat = jane_data::catalog();
    let theirs = cat.combat.unit(t.def).run.0.max(cat.combat.unit(t.def).walk.0);
    let mine = cat.combat.unit(me.def).walk.0;
    theirs < mine || me.energy.0 >= jane_sim::tuning::ENERGY_MAX.0 / 5
}

/// A unit a bot fights or feeds, never both: a row with a bait is fed.
pub fn fightable(u: &Unit) -> bool {
    jane_data::catalog().combat.unit(u.def).bait.is_none()
}

/// Is `u` in a fight with her?
pub fn on_me(v: &View<'_>, u: &Unit) -> bool {
    u.alive && u.combat == CombatState::Combat && u.target == Some(v.body().id)
}

/// Can it touch her from where it is? Something rooted to the spot (a statue, a cactus) that
/// is still after her from beyond its longest reach is not a fight: running from it or back to
/// it for ever is how a crawl stalls. What only bites reaches no further than a step past its
/// bite (the garden's seedlings); what throws, a few cells past its throw.
pub fn can_reach(v: &View<'_>, u: &Unit) -> bool {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(u.def);
    if d.walk.0 > 0 || d.run.0 > 0 {
        return true;
    }
    let range = d.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
    let bites = d.book.iter().all(|&s| cat.combat.spell(s).kind == SpellKind::Melee);
    let margin = if bites { CELL_FX } else { 4 * CELL_FX };
    gap(v.body(), u) <= i64::from(range) + i64::from(margin)
}

/// Can a rooted thing reach her from where it stands: the Burial's rule ([`can_reach`], a
/// shooter four cells past its reach), else the forest's ([`reaches_her`], one cell).
fn reaches_her_here(v: &View<'_>, u: &Unit) -> bool {
    if v.zone() == jane_core::ZoneId::Burial { can_reach(v, u) } else { reaches_her(v, u) }
}

/// Has she lately found no way to it?
pub fn reachable(cx: &Ctx, id: UnitId, frame: u32) -> bool {
    cx.fight.unreachable.get(&id).is_none_or(|&until| until <= frame)
}

/// Has it no feet (a cactus, a flower, a statue)?
pub fn rooted(u: &Unit) -> bool {
    let d = jane_data::catalog().combat.unit(u.def);
    d.run.0 <= 0 && d.walk.0 <= 0
}

/// Can it hurt her from where it stands? A rooted thing only as far as its longest spell
/// reaches; anything with feet, always.
pub fn reaches_her(v: &View<'_>, u: &Unit) -> bool {
    let cat = jane_data::catalog();
    if !rooted(u) {
        return true;
    }
    let far = jane_sim::combat::book_of(u).iter().map(|&s| i64::from(cat.combat.spell(s).range.0)).max().unwrap_or(0);
    gap(v.body(), u) <= far + i64::from(CELL_FX)
}

/// The enemy to deal with now: the nearest one fighting her (and able to reach her), else the
/// one hunted. Low with nothing to eat, a rooted thing is left where it stands (it cannot
/// follow her to the fire; backing off from it only walks her back and forth in its reach).
pub fn threat(v: &View<'_>, cx: &mut Ctx) -> Option<UnitId> {
    // What sees only by light and is no boss is not fought unless hunted (tactics::works); a
    // thing with no feet is not fought from where it cannot reach, nor when she is low.
    let ignore = crate::tactics::works::ignore;
    let low = hp_permille(v.body()) < FLEE_BELOW && food(v).is_none();
    let on: Vec<&Unit> = enemies(v)
        .into_iter()
        .filter(|u| on_me(v, u) && fightable(u) && !ignore(u) && reaches_her_here(v, u) && !(low && rooted(u)))
        .collect();
    if let Some(&nearest) = on.first() {
        // With a crowd on her, one at a time: the one she is hitting while it is still at her,
        // else the weakest within reach of her (a blow spread over four kills none of them).
        let me = v.body();
        let reach = i64::from(2 * CELL_FX);
        let held = on.iter().find(|u| Some(u.id) == cx.fight.target && gap(me, u) <= reach);
        let weakest = on.iter().filter(|u| gap(me, u) <= reach).min_by_key(|u| (u.hp, u.id));
        return Some(held.or(weakest).unwrap_or(&nearest).id);
    }
    let now = v.frame();
    match cx.fight.hunt.and_then(|t| v.unit(t)).filter(|u| u.alive && reachable(cx, u.id, now)) {
        Some(u) => Some(u.id),
        None => {
            cx.fight.hunt = None;
            None
        }
    }
}

/// One frame of fighting `id`; `None` when it is over (it is down, or gone, or she has backed
/// off and nothing is on her).
pub fn engage(v: &View<'_>, cx: &mut Ctx, id: UnitId) -> Option<Act> {
    let me = v.body();
    if !me.alive {
        return Some(Act::idle());
    }
    let Some(t) = v.unit(id).filter(|u| u.alive) else {
        cx.fight.target = None;
        cx.fight.t = 0;
        return None;
    };
    // Let be a while ago, and not after her: still let be (a hunt taken up again at once started
    // the three minutes over, for ever, at something out of sight she could not get to).
    if !reachable(cx, id, v.frame()) && !on_me(v, t) {
        return None;
    }
    if cx.fight.target != Some(id) {
        cx.fight.target = Some(id);
        cx.fight.t = 0;
        cx.fight.fights += 1;
    }
    cx.fight.t += 1;
    cx.foes.insert(id);
    if cx.fight.t > 60 * 180 {
        // Three minutes on one thing: let it be, for five.
        cx.fight.target = None;
        cx.fight.hunt = None;
        cx.fight.unreachable.insert(id, v.frame() + 60 * 300);
        return None;
    }
    let now = v.tick();
    if let Some(at) = tell_under(v) {
        return Some(Act::hold(step_out(v, cx, at)));
    }
    if let Some(c) = eat(v) {
        return Some(Act::press(c));
    }
    // One blow of its could put her down: eat now, above the usual line (the Charge Hand's live
    // hand takes over half of her at once).
    if me.hp.points() <= max_hit(t) && me.hp < jane_sim::units::max_hp(me) {
        if let Some(f) = food(v) {
            return Some(Act::press(Command::Item(f)));
        }
    }
    // The Factory's bosses are fought as their rooms ask (tactics::works).
    if let Some(a) = crate::tactics::works::engage(v, cx, id) {
        return a;
    }
    // A boss fought the way its room is built to be fought (tactics/*.rs).
    if let Some(a) = crate::tactics::forest::engage(v, cx, t) {
        return Some(a);
    }
    let cat = jane_data::catalog();
    let d = dist(me.pos, t.pos);
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    // Low with nothing to eat: back off (it may leash), and let the plan find a fire. Not from
    // what she would put down first, trading blows as the rows say (a skeleton between her and
    // the fire is walked through, not fled from for ever), nor from what could not put her down
    // in three blows (a rat between her and the fire followed her back and forth all day).
    // Mended since (an apple, a fire): the flight is over.
    if hp_permille(me) >= 2 * FLEE_BELOW {
        cx.fight.fleeing = 0;
    }
    // The School: what she cannot walk away from is fought out (`tactics::school::stands`).
    let kited = v.zone() == jane_core::ZoneId::School && crate::tactics::school::stands(v, t);
    let dangerous = i64::from(max_hit(t)) * 3 >= i64::from(me.hp.points());
    if hp_permille(me) < FLEE_BELOW && cx.fight.fleeing == 0 && !has_food(v) && !would_win(v) && !kited && dangerous {
        cx.fight.fleeing = 180;
        cx.fight.fled += 1;
        cx.fight.hunt = None;
    }
    if cx.fight.fleeing > 0 {
        cx.fight.fleeing -= 1;
        if !on_me(v, t) || !reaches_her(v, t) {
            cx.fight.fleeing = 0;
            cx.fight.target = None;
            return None;
        }
        let from = t.pos;
        // The School: back to the sick bay fire, and sit down by it (`tactics::school`).
        if v.zone() == jane_core::ZoneId::School {
            if let Some(c) = crate::tactics::school::at_home(v) {
                return Some(Act::press(c));
            }
            if let Some(f) = crate::tactics::school::run_home(v, cx) {
                return Some(Act::hold(f));
            }
        }
        let frame = away_from(v, cx, from, None);
        // Backing off with the mana for it: a bolt over her shoulder when one is ready (a thing
        // that keeps after her is put down on the way, not led round the dungeon).
        let ice = sense::spell("icebolt");
        let reach = i64::from(cat.combat.spell(ice).range.0) * 9 / 10;
        if knows(v, ice) && ready(me, ice, now) && gap(me, t) <= reach && v.sight(me.pos, t.pos) {
            return Some(Act {
                frame: InputFrame { aim: Some(dir), ..frame },
                cmds: vec![Command::Cast { spell: ice, on: Some(id) }],
            });
        }
        return Some(Act::hold(frame));
    }
    // The School: every bolt as it comes ready, and her ground held (`tactics::school`).
    if v.zone() == jane_core::ZoneId::School {
        if let Some(a) = crate::tactics::school::strike(v, cx, id) {
            return a;
        }
    }
    let melee = sense::spell("melee_player");
    let range = i64::from(cat.combat.spell(melee).range.0);
    let g = gap(me, t);
    // A bolt from range.
    let ice = sense::spell("icebolt");
    let def = cat.combat.spell(ice);
    // A rooted thing (no feet: a cactus, a flower) is shot from where she stands while she has
    // a bolt and the mana, never walked up to: up close all of a cactus's fan lands. Too near
    // one that shoots, she steps back out; out of range or sight, she walks in until it is in
    // both (a flower with only its teeth is walked up to and struck if it cannot be seen).
    if rooted(t) && knows(v, ice) && me.mp >= def.mp {
        let seen = v.sight(me.pos, t.pos);
        let shoots = jane_sim::combat::book_of(t).iter().any(|&s| cat.combat.spell(s).range.0 > 2 * CELL_FX);
        if seen && shoots && d <= i64::from(4 * CELL_FX) {
            return Some(Act::hold(InputFrame { aim: Some(dir), ..away_from(v, cx, t.pos, None) }));
        }
        if seen && d > i64::from(3 * CELL_FX) && g <= i64::from(def.range.0) * 9 / 10 {
            let cmds = if ready(me, ice, now) { vec![Command::Cast { spell: ice, on: Some(id) }] } else { Vec::new() };
            return Some(Act { frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE }, cmds });
        }
    }
    // (Up close too: a bolt point-blank is still twice her swing, and a swing does not wait on it.
    // Stood toe to toe with three hundred mana unspent is how the School's guards killed her.)
    if def.kind == SpellKind::Bolt
        && knows(v, ice)
        && ready(me, ice, now)
        && g <= i64::from(def.range.0) * 9 / 10
        && v.sight(me.pos, t.pos)
    {
        return Some(Act {
            frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
            cmds: vec![Command::Cast { spell: ice, on: Some(id) }],
        });
    }
    // Something that out-hits her (a boss, or more health than she has) is kept at bolt range
    // while the mana lasts: she backs off between casts (it is slower than she is; the frost
    // slows it more), as a player with a bolt does.
    let strong = cat.combat.unit(t.def).boss || t.hp > me.hp;
    // A crowd on her she would not put down standing (the Gold Mine's hub pulls three skeletons,
    // the clerk and a rat at once): stone skin, which halves their blows, while it is not on her.
    let skin = sense::item("potion_stoneskin");
    let crowd = enemies(v).iter().filter(|u| on_me(v, u)).count() >= 2;
    if crowd && holds(v, skin) > 0 && item_ready(me, skin, now) && !would_win(v) {
        let effect = jane_data::catalog().combat.effect_id("stoneskin");
        if !me.statuses.iter().any(|s| Some(s.effect) == effect) {
            return Some(Act::press(Command::Item(skin)));
        }
    }
    // Against a boss, what she brought for it: stone skin, then the mana shield (a tough thing on
    // the way is not what they were brought for).
    if strong && cat.combat.unit(t.def).boss && d < i64::from(12 * CELL_FX) {
        for name in ["potion_stoneskin", "potion_lifesteal", "potion_manashield"] {
            let p = sense::item(name);
            if holds(v, p) > 0 && item_ready(me, p, now) && me.statuses.is_empty() {
                return Some(Act::press(Command::Item(p)));
            }
        }
    }
    if strong && knows(v, ice) && me.mp >= def.mp && d < i64::from(6 * CELL_FX) {
        // Inside two thirds of its leash from home, so it keeps coming.
        let leash = i64::from(cat.combat.unit(t.def).leash.0);
        let tether = (leash > 0).then_some((t.home, leash * 2 / 3));
        return Some(Act::hold(InputFrame { aim: Some(dir), ..away_from(v, cx, t.pos, tether) }));
    }
    if g <= range {
        let cmds = if ready(me, melee, now) { vec![Command::Cast { spell: melee, on: Some(id) }] } else { Vec::new() };
        // Face it without walking into it.
        return Some(Act { frame: InputFrame { aim: Some(dir), mv_dir: dir, mv_mag: 10, ..InputFrame::IDLE }, cmds });
    }
    let frame = match cx.nav.go(v, t.pos, Fx(range as i32 + 6 * 256), false) {
        Go::Walk(f) if d > i64::from(4 * CELL_FX) => f,
        Go::NoWay if !on_me(v, t) => {
            // Behind a gate, over water: not today.
            cx.fight.unreachable.insert(id, v.frame() + 60 * 20);
            cx.fight.hunt = None;
            cx.fight.target = None;
            return None;
        }
        _ => stick(me.pos, t.pos, false),
    };
    Some(Act::hold(InputFrame { aim: Some(dir), ..frame }))
}
