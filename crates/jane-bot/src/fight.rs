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

pub fn item_ready(u: &Unit, i: ItemId, now: Tick) -> bool {
    !u.item_cooldowns.iter().any(|&(c, until)| c == i && until > now)
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
pub fn away_from(v: &View<'_>, cx: &mut Ctx, from: jane_core::Vec2, tether: Option<(jane_core::Vec2, i64)>) -> InputFrame {
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

/// A hostile pool that has not bitten yet and has her in it (her body's edge inside its
/// radius, with a cell to spare): where it lies.
pub fn tell_under(v: &View<'_>) -> Option<jane_core::Vec2> {
    let me = v.body();
    let body = i64::from(jane_data::catalog().combat.unit(me.def).bounds.0);
    let now = v.tick();
    v.grounds()
        .iter()
        .filter(|g| g.faction != me.faction && g.next_pulse > now && g.until > now)
        .find(|g| dist(g.pos, me.pos) <= i64::from(g.radius.0) + body + i64::from(CELL_FX))
        .map(|g| g.pos)
}

/// Out of a pool before it lands: straight away from its middle, at a run.
fn step_out(v: &View<'_>, cx: &mut Ctx, from: jane_core::Vec2) -> InputFrame {
    let me = v.body().pos;
    // Standing on its middle: any way will do; the way she faces is as good as any.
    let from = if from == me {
        let (dx, dy) = v.body().facing.delta();
        jane_core::Vec2 { x: Fx(me.x.0 - dx * CELL_FX), y: Fx(me.y.0 - dy * CELL_FX) }
    } else {
        from
    };
    cx.fight.retreat = None;
    stick(from, me, true)
}

/// Food she holds and may eat now.
pub fn food(v: &View<'_>) -> Option<ItemId> {
    let me = v.body();
    ["apple", "grape"].into_iter().map(sense::item).find(|&i| holds(v, i) > 0 && item_ready(me, i, v.tick()))
}

/// Does she hold anything to eat at all (ready or not)?
pub fn has_food(v: &View<'_>) -> bool {
    ["apple", "grape"].into_iter().any(|n| holds(v, sense::item(n)) > 0)
}

/// Something to eat, if she is low and can.
pub fn eat(v: &View<'_>) -> Option<Command> {
    if hp_permille(v.body()) >= EAT_BELOW {
        return None;
    }
    food(v).map(Command::Item)
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
/// it for ever is how a crawl stalls.
pub fn can_reach(v: &View<'_>, u: &Unit) -> bool {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(u.def);
    if d.walk.0 > 0 || d.run.0 > 0 {
        return true;
    }
    let range = d.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
    gap(v.body(), u) <= i64::from(range) + i64::from(4 * CELL_FX)
}

/// Has she lately found no way to it?
pub fn reachable(cx: &Ctx, id: UnitId, frame: u32) -> bool {
    cx.fight.unreachable.get(&id).is_none_or(|&until| until <= frame)
}

/// The enemy to deal with now: the nearest one fighting her, else the one hunted.
pub fn threat(v: &View<'_>, cx: &mut Ctx) -> Option<UnitId> {
    if let Some(u) = enemies(v).into_iter().find(|u| on_me(v, u) && fightable(u) && can_reach(v, u)) {
        return Some(u.id);
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
    if cx.fight.target != Some(id) {
        cx.fight.target = Some(id);
        cx.fight.t = 0;
        cx.fight.fights += 1;
    }
    cx.fight.t += 1;
    cx.foes.insert(id);
    if cx.fight.t > 60 * 180 {
        // Three minutes on one thing: let it be.
        cx.fight.target = None;
        cx.fight.hunt = None;
        return None;
    }
    let now = v.tick();
    if let Some(at) = tell_under(v) {
        return Some(Act::hold(step_out(v, cx, at)));
    }
    if let Some(c) = eat(v) {
        return Some(Act::press(c));
    }
    let cat = jane_data::catalog();
    let d = dist(me.pos, t.pos);
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    // Low with nothing to eat: back off (it may leash), and let the plan find a fire.
    if hp_permille(me) < FLEE_BELOW && cx.fight.fleeing == 0 && food(v).is_none() {
        cx.fight.fleeing = 180;
        cx.fight.fled += 1;
        cx.fight.hunt = None;
    }
    if cx.fight.fleeing > 0 {
        cx.fight.fleeing -= 1;
        if !on_me(v, t) {
            cx.fight.fleeing = 0;
            cx.fight.target = None;
            return None;
        }
        let from = t.pos;
        return Some(Act::hold(away_from(v, cx, from, None)));
    }
    let melee = sense::spell("melee_player");
    let range = i64::from(cat.combat.spell(melee).range.0);
    let g = gap(me, t);
    // A bolt from range.
    let ice = sense::spell("icebolt");
    let def = cat.combat.spell(ice);
    if def.kind == SpellKind::Bolt
        && knows(v, ice)
        && ready(me, ice, now)
        && d > i64::from(3 * CELL_FX)
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
    // Against something strong, what she brought for it: stone skin, then the mana shield.
    if strong && d < i64::from(12 * CELL_FX) {
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
