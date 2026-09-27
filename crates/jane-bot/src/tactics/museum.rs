//! The Museum (DUNGEONS.md §3.2): see `mod.rs`. Three hooks in the crawl: [`first`] (before its
//! general order), [`idle`] (when that order has nothing), [`fight`] (before the general fight),
//! and [`done`] (out, with what the story needs).
//!
//! **One breaker, two buildings.** The crawl throws the breaker in Maintenance once as it throws
//! any lever (the lights go out, the arts door opens, and the dark wing gives up the floor key
//! that lets her at the Shot-Firer). What it cannot do on its own is throw it back: a lever used
//! is used. The dark was for the verb; once she has Explosion she wants the building lit, where
//! the exhibits stand still to be blown away and the power shutters are up (the stores). So,
//! dark and knowing Explosion: back to the breaker before anything else; lit, not knowing it yet,
//! with nothing else to do: to the breaker. Dark or lit is what she sees: the atrium's gallery
//! lamps are out or on.
//!
//! **The verb first.** The Shot-Firer is hunted once she can walk to him, not left for last as a
//! boss, and his page is read as soon as she has seen him go down. Then, lit, the way in as the
//! plan gives it: the stores' armour, the Attendant's locker behind it, the painted-over arch.
//!
//! **The stove.** A fire costs nothing but the walk: she rests there the first time she passes
//! (it is where she wakes), when she is down to half, before the rotunda, and whenever the small
//! fry have worn her down, so that the apples she was given all go into the rotunda, which has
//! no fire.
//!
//! **The rotunda** is [`fight`]'s: see there.

use crate::crawl::{Reach, Try};
use crate::sense;
use crate::task::{Task, UseProp};
use jane_core::num::CELL_FX;
use jane_core::{Fx, Vec2, ZoneId};
use jane_sim::View;

/// The breaker in Maintenance, when she can walk up to it.
fn breaker<'a>(v: &View<'a>, reach: &Reach) -> Option<&'a jane_sim::Prop> {
    sense::prop_named(v, "museum_breaker").filter(|p| reach.beside(p))
}

/// The building's lights are out: the atrium's gallery lamps are dark (a hidden prop is not in
/// the view at all).
pub fn dark(v: &View<'_>) -> bool {
    v.zone() == ZoneId::Museum && sense::prop_named(v, "museum_atrium_lamp_a").is_none()
}

/// She has the verb the dark was for.
fn has_explosion(v: &View<'_>) -> bool {
    jane_data::catalog().combat.spell_id("explosion").is_some_and(|s| sense::knows(v, s))
}

fn throw(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let b = breaker(v, reach)?;
    Some((Task::Use(UseProp::new(b.id)), Try::Prop(b.id)))
}

/// Before the crawl's general order (see the module doc).
pub fn first(v: &View<'_>, cx: &crate::task::Ctx, reach: &Reach) -> Option<(Task, Try)> {
    if v.zone() != ZoneId::Museum {
        return None;
    }
    let verb = has_explosion(v);
    // The verb before the stove: the Shot-Firer's page, once she has seen him go down (he guards
    // it; it lets go when she stands in his room with him dead, so she walks up to it locked or
    // not). A man out of her sight is not a man down.
    let firer_down = jane_data::catalog().combat.unit_id("shot_firer").is_some_and(|d| {
        sense::units_of(v, d).is_empty() && cx.seen_foes.get(&d).is_some_and(std::collections::BTreeMap::is_empty)
    });
    if !verb && firer_down {
        if let Some(p) = sense::prop_named(v, "museum_page").filter(|p| !p.used && reach.beside(p)) {
            return Some((Task::Use(UseProp::new(p.id)), Try::Prop(p.id)));
        }
    }
    if let Some(r) = mend_first(v, reach) {
        return Some(r);
    }
    // The stove once, the first time she passes near it: it is where she will wake (the county's
    // door is shut after four, and a woman who wakes at the Halt is not coming back in today).
    let passing = |s: &&jane_sim::Prop| crate::nav::dist(v.body().pos, sense::prop_centre(s)) < i64::from(40 * CELL_FX);
    if let Some(s) = stove(v, reach).filter(|s| !cx.used.contains_key(&(v.zone(), s.id))).filter(passing) {
        return Some((Task::Use(UseProp::new(s.id)), Try::Rest(s.id)));
    }
    if !verb {
        return firer(v, reach);
    }
    if dark(v) {
        return throw(v, reach);
    }
    if let Some(r) = clear_plinths(v, reach) {
        return Some(r);
    }
    the_way_in(v, reach)
}

/// Lit, with the verb: the way to the Attendant as the plan and the building give it, before
/// the exhibits and the side cases (they keep): the armour in the stores' doorway, the locker
/// with his key behind it, the painted-over arch. The rotunda's gate the crawl unlocks itself.
fn the_way_in(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let cat = jane_data::catalog();
    let spell = cat.combat.spell_id("explosion")?;
    let blast = |name: &str| -> Option<(Task, Try)> {
        let p = sense::prop_named(v, name)?;
        if v.body().mp < cat.combat.spell(spell).mp {
            return None;
        }
        let (from, at) = blast_spot(v, reach, p, None)?;
        Some((Task::Aim { spell, from, at, t: 0 }, Try::Cast(p.id)))
    };
    if let Some(r) = blast("museum_stores_armour") {
        return Some(r);
    }
    if let Some(p) = sense::prop_named(v, "museum_attendant_key_chest").filter(|p| !p.used && reach.beside(p)) {
        return Some((Task::Use(UseProp::new(p.id)), Try::Prop(p.id)));
    }
    blast("museum_arch")
}

/// The Shot-Firer, once she can walk to him: the verb is in his hand, and every door after it
/// wants the verb, so he is not left for last as a boss is (DUNGEONS.md §3.2: "He is fought in
/// the dark by necessity, which is the lesson of the wing").
fn firer(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let def = jane_data::catalog().combat.unit_id("shot_firer")?;
    let u = sense::units_of(v, def).into_iter().find(|u| reach.point(u.pos))?;
    Some((Task::Hunt(u.id), Try::Fight(u.id)))
}

/// She has what the story needs from the Museum (the verb, the council key from the case that is
/// not on the plan, the big jar beside it): the rest can wait for another day.
pub fn done(v: &View<'_>) -> bool {
    let cat = jane_data::catalog();
    v.zone() == ZoneId::Museum
        && has_explosion(v)
        && cat.combat.item_id("key_forest").is_some_and(|k| sense::holds(v, k) > 0)
        && sense::prop_named(v, "museum_big_jar").is_none_or(|p| p.used)
}

/// The cloakroom stove, when she can walk up to it (a fire: it mends her, and no time passes).
fn stove<'a>(v: &View<'a>, reach: &Reach) -> Option<&'a jane_sim::Prop> {
    sense::prop_named(v, "museum_stove").filter(|p| reach.beside(p))
}

/// The stove in reach and she is down to half, or holds the Attendant's key and is not whole:
/// mend there. A fire costs nothing but the walk, and the apples are wanted in the rotunda, where
/// the door shuts behind her and there is no fire (the stove is where she will wake).
fn mend_first(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let key = jane_data::catalog().combat.item_id("key_attendant")?;
    let me = v.body();
    let whole = me.hp >= jane_sim::units::max_hp(me) && me.mp >= jane_sim::units::max_mp(me);
    let low = sense::hp_permille(me) < 550;
    if whole || !low && sense::holds(v, key) == 0 {
        return None;
    }
    let s = stove(v, reach)?;
    Some((Task::Use(UseProp::new(s.id)), Try::Rest(s.id)))
}

/// Nothing else to do here: the other state may open what this one shuts.
pub fn idle(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    if v.zone() != ZoneId::Museum || dark(v) || has_explosion(v) {
        return None;
    }
    throw(v, reach)
}

/// The rotunda's four armours still on their plinths (lit: props that answer Explosion).
fn armours<'a>(v: &View<'a>) -> impl Iterator<Item = &'a jane_sim::Prop> + 'a {
    let v = *v;
    ["museum_rotunda_armour_a", "museum_rotunda_armour_b", "museum_rotunda_armour_c", "museum_rotunda_armour_d"]
        .into_iter()
        .filter_map(move |n| sense::prop_named(&v, n))
}

/// The Attendant, standing.
fn attendant<'a>(v: &View<'a>) -> Option<&'a jane_sim::Unit> {
    let def = jane_data::catalog().combat.unit_id("attendant")?;
    sense::units_of(v, def).into_iter().next()
}

/// Where to throw Explosion at `p` from: a cell she can walk to, two to seven cells off, that sees
/// the prop's near face, and none nearer `avoid` than its radius; the nearest to her. Returns the
/// cell's middle and the point to aim at.
fn blast_spot(v: &View<'_>, reach: &Reach, p: &jane_sim::Prop, avoid: Option<(Vec2, i64)>) -> Option<(Vec2, Vec2)> {
    use crate::nav::dist;
    let c = sense::prop_centre(p);
    let r = sense::prop_rect(p);
    let (px, py) = c.cell();
    let me = v.body().pos;
    let mut best: Option<(i64, Vec2)> = None;
    for dy in -7..=7 {
        for dx in -7..=7 {
            let (x, y) = (px + dx, py + dy);
            let d2 = dx * dx + dy * dy;
            if !(4..=49).contains(&d2) || !reach.get(x, y) {
                continue;
            }
            let at = Vec2::centre(x, y);
            if avoid.is_some_and(|(o, rad)| dist(at, o) < rad) {
                continue;
            }
            let clamp = |q: i32, lo: i32, hi: i32| q.clamp(lo * CELL_FX - 1, hi * CELL_FX);
            let face = Vec2::new(Fx(clamp(at.x.0, r.x, r.right())), Fx(clamp(at.y.0, r.y, r.bottom())));
            if !v.sight(at, face) {
                continue;
            }
            let d = dist(me, at);
            if best.is_none_or(|b| (d, at.x.0, at.y.0) < (b.0, b.1.x.0, b.1.y.0)) {
                best = Some((d, at));
            }
        }
    }
    best.map(|(_, at)| (at, c))
}

/// Before he has noticed her: an armour blown off its plinth from out of his reach (he stands in
/// the middle of the room, the plinths round him), so it never steps down at all.
fn clear_plinths(v: &View<'_>, reach: &Reach) -> Option<(Task, Try)> {
    let cat = jane_data::catalog();
    let boss = attendant(v)?;
    if crate::fight::on_me(v, boss) {
        return None;
    }
    let spell = cat.combat.spell_id("explosion").filter(|&s| sense::knows(v, s))?;
    if v.body().mp < cat.combat.spell(spell).mp {
        return None;
    }
    let aggro = i64::from(cat.combat.unit(boss.def).aggro.0) + i64::from(3 * CELL_FX);
    let me = v.body().pos;
    armours(v)
        .filter_map(|p| blast_spot(v, reach, p, Some((boss.pos, aggro))).map(|(from, at)| (p.id, from, at)))
        .min_by_key(|&(id, from, _)| (crate::nav::dist(me, from), id))
        .map(|(id, from, at)| (Task::Aim { spell, from, at, t: 0 }, Try::Cast(id)))
}

/// The rotunda (DUNGEONS.md §3.2, the Attendant): he fights in the light, and at three quarters,
/// a half and a quarter he throws the breaker and four armours step down in the dark. The room is
/// built to be played one way: throw the rotunda's own breaker back (the armours freeze on their
/// plinths and he stands dazzled), and in the light spend Explosion on a frozen armour, so it
/// never steps down again; and while armours stand, do not be the one who pushes him past the
/// next quarter. He out-hits her several times over (toe to toe she is dead in seconds), so she
/// never stands and trades: a bolt when it is ready, and between bolts she keeps away, inside his
/// leash (a man let go home mends, and the fight starts over). Only a man on his last legs is
/// finished by hand. This owns the whole fight while he is on her.
///
/// The Shot-Firer is fought the same way without the room's parts: he walks slower than she does
/// and lobs charges, so she keeps her distance, steps out of what lands under her, and bolts him
/// when the bolt is ready.
pub fn fight(v: &View<'_>, cx: &mut crate::task::Ctx, reach: &Reach) -> Option<crate::Act> {
    use crate::nav::{Go, dist};
    use jane_sim::{Command, InputFrame};
    if v.zone() != ZoneId::Museum {
        return None;
    }
    let cat = jane_data::catalog();
    let me = v.body();
    let rotunda = v.sym("museum_rotunda").and_then(|s| v.rect(s))?;
    let (x, y) = me.pos.cell();
    // Worn down by the building's small fry with the stove anywhere she can walk to: to the stove,
    // not into the apples. Every apple is wanted in the rotunda, which has no fire (a run that
    // ate its apples on the armours lost the Attendant with him nearly down); the small fry
    // follow her to the stove, and it mends her all the same.
    let boss_on = sense::enemies(v).into_iter().any(|u| crate::fight::on_me(v, u) && cat.combat.unit(u.def).boss);
    if !boss_on && sense::hp_permille(me) < crate::fight::EAT_BELOW && crate::fight::has_food(v) {
        if let Some(a) = stove(v, reach).and_then(|s| use_now(v, cx, s)) {
            return Some(a);
        }
    }
    // The Attendant anywhere near (a kite that strays from the rotunda must not become a flight:
    // a man let go home mends); the Shot-Firer only where she can walk to him (he stands in
    // combat with her from the far side of a locked door).
    let near = |b: &jane_sim::Unit, r: i32| dist(me.pos, b.pos) <= i64::from(r * CELL_FX);
    let (boss, is_attendant) = match attendant(v)
        .filter(|b| crate::fight::on_me(v, b) && (rotunda.contains(x, y) || near(b, 40)))
    {
        Some(b) => (b, true),
        None => {
            let def = cat.combat.unit_id("shot_firer")?;
            let u = sense::units_of(v, def).into_iter().find(|u| crate::fight::on_me(v, u) && reach.point(u.pos))?;
            (u, false)
        }
    };
    // A pool about to bite: the general fight steps out of it. Food sooner than the general
    // fight eats: one of his flurries with a stun in it takes half of her, and what she carried
    // into this room was carried for this.
    if crate::fight::tell_under(v).is_some() {
        return None;
    }
    let is_boss = cat.combat.unit(boss.def).boss;
    let eat_below = if is_attendant { 650 } else { crate::fight::EAT_BELOW };
    if sense::hp_permille(me) < eat_below {
        if let Some(i) = crate::fight::food(v) {
            return Some(crate::Act::press(Command::Item(i)));
        }
    }
    let now = v.tick();
    let d = dist(me.pos, boss.pos);
    let aim = jane_core::angle::iatan2(boss.pos.y.0 - me.pos.y.0, boss.pos.x.0 - me.pos.x.0);
    // What she brought for something strong: stone skin when he is close and she is hurt, life
    // steal at the start. Not the mana shield: it pays his blows from the mana her bolts need.
    let close_hurt = d < i64::from(5 * CELL_FX) && sense::hp_permille(me) < 750;
    if is_boss && me.statuses.is_empty() && (close_hurt || d < i64::from(12 * CELL_FX)) {
        let names: &[&str] =
            if close_hurt || !is_attendant { &["potion_stoneskin", "potion_lifesteal"] } else { &["potion_lifesteal"] };
        for &name in names {
            let p = sense::item(name);
            let ready = !me.item_cooldowns.iter().any(|&(c, until)| c == p && until > now);
            if sense::holds(v, p) > 0 && ready {
                return Some(crate::Act::press(Command::Item(p)));
            }
        }
    }
    // Dark: the breaker first.
    if is_attendant && sense::prop_named(v, "museum_rotunda_lamp_a").is_none() {
        if let Some(a) = sense::prop_named(v, "museum_rotunda_breaker").and_then(|b| use_now(v, cx, b)) {
            return Some(a);
        }
    }
    let dazzled = cat.combat.effect_id("dazzled").is_some_and(|e| boss.statuses.iter().any(|s| s.effect == e));
    let ice = cat.combat.spell_id("icebolt").filter(|&s| sense::knows(v, s));
    let bolt_ready = ice.filter(|&s| crate::fight::ready(me, s, now));
    let in_sight = v.sight(me.pos, boss.pos);
    // On his last legs: finish him, by bolt or by hand.
    let last_legs = boss.hp.0 < 60_000;
    if last_legs {
        if let Some(s) = bolt_ready.filter(|_| in_sight) {
            return Some(crate::Act {
                frame: InputFrame { aim: Some(aim), ..InputFrame::IDLE },
                cmds: vec![Command::Cast { spell: s, on: Some(boss.id) }],
            });
        }
    }
    // Lit, armours standing: Explosion at one while it is ready and he is not on top of her.
    let close = d < i64::from(4 * CELL_FX);
    if let Some(blast) =
        cat.combat.spell_id("explosion").filter(|&s| sense::knows(v, s) && crate::fight::ready(me, s, now))
    {
        if is_attendant && (!close || dazzled) {
            let avoid = (!dazzled).then_some((boss.pos, i64::from(4 * CELL_FX)));
            let spot = armours(v)
                .filter_map(|p| blast_spot(v, reach, p, avoid))
                .min_by_key(|&(from, _)| (dist(me.pos, from), from.x.0, from.y.0));
            if let Some((from, at)) = spot {
                if dist(me.pos, from) > i64::from(Fx::from_px(3).0) {
                    if let Go::Walk(f) = cx.nav.go(v, from, Fx::from_px(2), true) {
                        return Some(crate::Act::hold(f));
                    }
                } else {
                    let dir = jane_core::angle::iatan2(at.y.0 - me.pos.y.0, at.x.0 - me.pos.x.0);
                    return Some(crate::Act {
                        frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
                        cmds: vec![Command::Cast { spell: blast, on: None }],
                    });
                }
            }
        }
    }
    // A bolt, unless armours stand and he is near the next quarter (let the armours go first).
    let def = cat.combat.unit(boss.def);
    let hp = sense::hp_permille(boss);
    let next = def.phases.iter().map(|p| i32::from(p.hp_below.0)).filter(|&t| t < hp).max().unwrap_or(0);
    let hold = is_attendant && armours(v).next().is_some() && hp - next <= 80 && !dazzled;
    let range = ice.map_or(0, |s| i64::from(cat.combat.spell(s).range.0) * 9 / 10);
    if let Some(s) = bolt_ready.filter(|_| !hold && in_sight && d > i64::from(3 * CELL_FX) && d <= range) {
        return Some(crate::Act {
            frame: InputFrame { aim: Some(aim), ..InputFrame::IDLE },
            cmds: vec![Command::Cast { spell: s, on: Some(boss.id) }],
        });
    }
    // Between bolts: out of bolt range or sight, come back in (only as far as a bolt's reach:
    // walking at him while he runs at her halves the gap twice as fast); far enough off, stand
    // and let the mana come; nearer, keep away, inside his leash.
    if d > range || !in_sight {
        return match cx.nav.go(v, boss.pos, Fx(range as i32 * 3 / 4), false) {
            Go::Walk(f) => Some(crate::Act::hold(InputFrame { aim: Some(aim), ..f })),
            _ => Some(crate::Act::hold(crate::nav::stick(me.pos, boss.pos, false))),
        };
    }
    if d > i64::from(8 * CELL_FX) && !hold {
        return Some(crate::Act::hold(InputFrame { aim: Some(aim), ..InputFrame::IDLE }));
    }
    let from = boss.pos;
    let leash = i64::from(def.leash.0);
    let tether = (leash > 0).then_some((boss.home, leash * 2 / 3));
    let to = kite_point(v, me.pos, from, tether)?;
    // Sprint only when he is close: a run spent is a run she has not got when he is.
    let run = d < i64::from(5 * CELL_FX);
    Some(crate::Act::hold(InputFrame { sprint: run, aim: Some(aim), ..crate::nav::stick(me.pos, to, run) }))
}

/// Where to step to keep away from `from`: a flood over the ground about her that does not go
/// within three cells of him (the way out of a corner is never past him), scoring each cell by
/// its distance from him, less the steps to it, plus how open the ground about it is (a corner
/// is where a kite ends); the first steps of the way to the best, as a point to steer at.
fn kite_point(v: &View<'_>, me: Vec2, from: Vec2, tether: Option<(Vec2, i64)>) -> Option<Vec2> {
    use crate::nav::{dist, walkable};
    use std::collections::{BTreeMap, VecDeque};
    let start = me.cell();
    let near = i64::from(3 * CELL_FX);
    let mut prev: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    let mut q = VecDeque::new();
    prev.insert(start, start);
    q.push_back((start, 0i64));
    let mut best: Option<(i64, (i32, i32))> = None;
    while let Some((c, steps)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if c != start && tether.is_none_or(|(home, r)| dist(at, home) <= r) {
            let open = (-2..=2)
                .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| walkable(v, c.0 + dx, c.1 + dy))
                .count() as i64;
            let score = dist(at, from) - steps * i64::from(CELL_FX) / 3 + open * i64::from(CELL_FX) / 6;
            if best.is_none_or(|b| (score, c) > b) {
                best = Some((score, c));
            }
        }
        if steps >= 20 || prev.len() > 1600 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            // Inside three cells of him only going away from him (from under his feet, out).
            let dn = dist(Vec2::centre(n.0, n.1), from);
            if prev.contains_key(&n) || !walkable(v, n.0, n.1) || dn < near && dn <= dist(at, from) {
                continue;
            }
            prev.insert(n, c);
            q.push_back((n, steps + 1));
        }
    }
    let (_, mut c) = best?;
    // Back along the way to the third step from her feet.
    let mut path = vec![c];
    while let Some(&p) = prev.get(&c) {
        if p == start {
            break;
        }
        path.push(p);
        c = p;
    }
    let step = path.len().saturating_sub(3);
    path.get(step).map(|&(x, y)| Vec2::centre(x, y))
}

/// Walk up to a prop and press USE on it, a frame at a time and remembering nothing (a fight
/// moves too much for a task); `None` when no side of it can be walked to.
fn use_now(v: &View<'_>, cx: &mut crate::task::Ctx, p: &jane_sim::Prop) -> Option<crate::Act> {
    use crate::nav::Go;
    use jane_sim::interact::FocusRef;
    if v.focus().is_some_and(|f| f.target == FocusRef::Prop(p.id)) {
        return Some(crate::Act::press(jane_sim::Command::Use));
    }
    for (at, face) in crate::task::sides(v, p, v.body().pos) {
        match cx.nav.go(v, at, jane_core::Fx::from_px(3), true) {
            Go::Walk(f) => return Some(crate::Act::hold(f)),
            Go::Arrived => return Some(crate::Act::hold(crate::task::nudge(face))),
            Go::NoWay => cx.nav.reset(),
        }
    }
    None
}
