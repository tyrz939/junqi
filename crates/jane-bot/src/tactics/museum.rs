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

/// The arts wing has given up its key: the floor key in the bag, or the science door it opens
/// already open.
fn arts_done(v: &View<'_>) -> bool {
    jane_data::catalog().combat.item_id("key_floor").is_some_and(|k| sense::holds(v, k) > 0)
        || sense::prop_named(v, "museum_gate_science").is_none_or(|p| !p.locked)
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
    // The dark was for the arts wing's key: that in hand, the lights back on at once, and every
    // exhibit that walked is a thing on a plinth again (the Shot-Firer's room is dark of itself).
    if !verb && dark(v) && arts_done(v) {
        if let Some(r) = throw(v, reach) {
            return Some(r);
        }
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
    if v.zone() != ZoneId::Museum || dark(v) || has_explosion(v) || arts_done(v) {
        return None;
    }
    throw(v, reach)
}

/// Not hunted in the Museum: what is slower than half her walk and no boss (the armours). Nothing
/// the story needs is behind one, and one takes a minute and a stove to put down; it is walked
/// past ([`walk_past`]).
pub fn leave_be(v: &View<'_>, def: jane_core::UnitDefId) -> bool {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(def);
    v.zone() == ZoneId::Museum && !d.boss && i64::from(d.run.0) * 2 < i64::from(cat.combat.unit(v.body().def).walk.0)
}

/// Everything on her is slower than half her walk (the armours: "Armours walk slowly and hit
/// hard", DUNGEONS.md §3.2), and she was not sent after it: she walks on about her business and
/// lets them follow. Stood toe to toe, an armour stuns her and takes a minute to put down, and
/// the dark wing has two of them and they come back.
pub fn walk_past(v: &View<'_>, id: jane_sim::ids::UnitId, doing: Option<Try>) -> bool {
    if v.zone() != ZoneId::Museum || doing == Some(Try::Fight(id)) || doing.is_none() {
        return false;
    }
    let cat = jane_data::catalog();
    let walk = i64::from(cat.combat.unit(v.body().def).walk.0);
    let on: Vec<&jane_sim::Unit> =
        sense::enemies(v).into_iter().filter(|u| crate::fight::on_me(v, u) && crate::fight::fightable(u)).collect();
    !on.is_empty()
        && on.iter().all(|u| {
            let d = cat.combat.unit(u.def);
            !d.boss && i64::from(d.run.0) * 2 < walk
        })
}

/// A cell she can walk to, four to nine cells off `him`, that sees him: the nearest to her.
fn sight_spot(v: &View<'_>, reach: &Reach, him: Vec2) -> Option<Vec2> {
    use crate::nav::dist;
    let (hx, hy) = him.cell();
    let me = v.body().pos;
    let mut best: Option<(i64, i32, i32)> = None;
    for dy in -9..=9 {
        for dx in -9..=9 {
            let d2 = dx * dx + dy * dy;
            let (x, y) = (hx + dx, hy + dy);
            if !(16..=81).contains(&d2) || !reach.get(x, y) {
                continue;
            }
            let at = Vec2::centre(x, y);
            if !v.sight(at, him) {
                continue;
            }
            let d = dist(me, at);
            if best.is_none_or(|b| (d, x, y) < b) {
                best = Some((d, x, y));
            }
        }
    }
    best.map(|(_, x, y)| Vec2::centre(x, y))
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
    let apples = sense::holds(v, sense::item("apple"));
    if !boss_on && sense::hp_permille(me) < crate::fight::EAT_BELOW && apples > 0 && apples <= 11 {
        if let Some(a) = stove(v, reach).and_then(|s| use_now(v, cx, s)) {
            return Some(a);
        }
    }
    // The Attendant anywhere near (a kite that strays from the rotunda must not become a flight:
    // a man let go home mends); the Shot-Firer only where she can walk to him (he stands in
    // combat with her from the far side of a locked door).
    let near = |b: &jane_sim::Unit, r: i32| dist(me.pos, b.pos) <= i64::from(r * CELL_FX);
    let (boss, is_attendant) =
        match attendant(v).filter(|b| crate::fight::on_me(v, b) && (rotunda.contains(x, y) || near(b, 40))) {
            Some(b) => (b, true),
            None => {
                let def = cat.combat.unit_id("shot_firer")?;
                // (Or the one she was sent after: he keeps to the dark of his room, and waits there.)
                let sent = cx.fight.hunt;
                let u = sense::units_of(v, def)
                    .into_iter()
                    .find(|u| (crate::fight::on_me(v, u) || sent == Some(u.id)) && reach.point(u.pos))?;
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
    // Dark: the rotunda's breaker thrown back freezes whatever armours walk and leaves him
    // dazzled where he stands for five seconds (DUNGEONS.md §3.2: "Throw it back"). With armours
    // walking, it comes first; with none, it is thrown when it is hers to reach: at her elbow as
    // the loop takes her past it, or nearer her than him with him well off.
    let rotunda_dark = sense::prop_named(v, "museum_rotunda_lamp_a").is_none();
    if is_attendant && rotunda_dark {
        if let Some(b) = sense::prop_named(v, "museum_rotunda_breaker") {
            let walking = cat.combat.unit_id("armour").is_some_and(|a| {
                sense::units_of(v, a).iter().any(|u| rotunda.contains(u.pos.cell().0, u.pos.cell().1))
            });
            let at = sense::prop_centre(b);
            let mine = dist(me.pos, at);
            let focused = v.focus().is_some_and(|f| f.target == jane_sim::interact::FocusRef::Prop(b.id));
            if focused {
                return Some(crate::Act::press(Command::Use));
            }
            let clear = d > i64::from(6 * CELL_FX) && mine * 3 < dist(boss.pos, at) * 2;
            if walking || mine < i64::from(3 * CELL_FX) || clear {
                if let Some(a) = use_now(v, cx, b) {
                    return Some(a);
                }
            }
        }
    }
    let dazzled = cat.combat.effect_id("dazzled").is_some_and(|e| boss.statuses.iter().any(|s| s.effect == e));
    let ice = cat.combat.spell_id("icebolt").filter(|&s| sense::knows(v, s));
    let bolt_ready = ice.filter(|&s| crate::fight::ready(me, s, now));
    let in_sight = v.sight(me.pos, boss.pos);
    // Something small in at her heels (a fox, a bat come in by a side door) with him well off:
    // put it down first; kited round the room it bites all the way.
    if d > i64::from(6 * CELL_FX) {
        let add = sense::enemies(v).into_iter().find(|u| {
            u.id != boss.id
                && crate::fight::on_me(v, u)
                && crate::fight::fightable(u)
                && !cat.combat.unit(u.def).boss
                && crate::fight::gap(me, u) < i64::from(2 * CELL_FX)
        });
        if let Some(a) = add.and_then(|u| crate::fight::engage(v, cx, u.id)) {
            return Some(a);
        }
    }
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
    // (Only while Explosion is known and some armour can be blown from ground she can reach:
    // asked last, and only with a bolt in hand, since each armour's spot is a sight scan.)
    let hold = is_attendant
        && hp - next <= 80
        && !dazzled
        && bolt_ready.is_some()
        && has_explosion(v)
        && armours(v).any(|p| blast_spot(v, reach, p, None).is_some());
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
    // Out of sight and not coming (a case between them, the light he will not cross): to a place
    // four to nine cells off him that sees him, the nearest.
    // (The Attendant does not wait: out of sight and near, he is coming round to her.)
    if !in_sight && d <= range && (!is_attendant || d > i64::from(6 * CELL_FX)) {
        if let Some(to) = sight_spot(v, reach, boss.pos) {
            if let Go::Walk(f) = cx.nav.go(v, to, Fx::from_px(3), false) {
                return Some(crate::Act::hold(InputFrame { aim: Some(aim), ..f }));
            }
        }
        // There, and the line still not clear from where her feet are: a step in toward him.
        if !is_attendant {
            return Some(crate::Act::hold(InputFrame { aim: Some(aim), ..crate::nav::stick(me.pos, boss.pos, false) }));
        }
    }
    if d > range || !in_sight && d > i64::from(10 * CELL_FX) {
        return match cx.nav.go(v, boss.pos, Fx(range as i32 * 3 / 4), false) {
            Go::Walk(f) => Some(crate::Act::hold(InputFrame { aim: Some(aim), ..f })),
            _ => Some(crate::Act::hold(crate::nav::stick(me.pos, boss.pos, false))),
        };
    }
    if d > i64::from(8 * CELL_FX) && !hold && in_sight {
        return Some(crate::Act::hold(InputFrame { aim: Some(aim), ..InputFrame::IDLE }));
    }
    let from = boss.pos;
    let leash = i64::from(def.leash.0);
    // His leash is his distance from home, and he cuts corners behind her: she keeps well inside it.
    let tether = (leash > 0).then_some((boss.home, leash * 3 / 4));
    // His pace against hers, tenths: chilled he is slower (the bolt's other half).
    let chilled = cat.combat.effect_id("chilled").is_some_and(|e| boss.statuses.iter().any(|s| s.effect == e));
    let pace = if chilled { 7 } else { 10 };
    // Sprint only when he is close: a run spent is a run she has not got when he is.
    let run = d < i64::from(5 * CELL_FX);
    // In the rotunda, in it: a side door is a corridor, and a corridor is a dead end.
    let room = is_attendant.then_some(rotunda);
    // The loop: the rotunda less three cells about, and inside the tether; for another, a ring
    // of eleven cells about his post.
    let cell = i64::from(CELL_FX);
    let reach_out = tether.map_or(16 * cell, |(_, r)| (r - 2 * cell).max(7 * cell));
    let (centre, axes) = match room {
        Some(r) => (
            sense::rect_centre(r),
            (
                (i64::from(r.w) * cell / 2 - 5 * cell).min(reach_out).min(16 * cell),
                i64::from(r.h) * cell / 2 - 3 * cell,
            ),
        ),
        None => (boss.home, (11 * cell, 11 * cell)),
    };
    let ring = orbit(me.pos, from, centre, axes);
    let to = kite_point(v, me.pos, from, pace, tether, Some(ring), room)
        .unwrap_or(Vec2::new(Fx(me.pos.x.0 * 2 - from.x.0), Fx(me.pos.y.0 * 2 - from.y.0)));
    Some(crate::Act::hold(InputFrame { sprint: run, aim: Some(aim), ..crate::nav::stick(me.pos, to, run) }))
}

/// The point a little way on round a loop, going round away from him: a kite in a room goes
/// round it, never into its end. The loop is an ellipse about `centre` with half-axes `axes`
/// (the room, less a margin, and inside the tether), `Fx`.
fn orbit(me: Vec2, him: Vec2, centre: Vec2, axes: (i64, i64)) -> Vec2 {
    let (ax, ay) = (axes.0.max(i64::from(CELL_FX)), axes.1.max(i64::from(CELL_FX)));
    // Her place and his in the ellipse's own round frame (the loop a circle of 256).
    let (x, y) = (i64::from(me.x.0 - centre.x.0) * 256 / ax, i64::from(me.y.0 - centre.y.0) * 256 / ay);
    let (hx, hy) = (i64::from(him.x.0 - centre.x.0) * 256 / ax, i64::from(him.y.0 - centre.y.0) * 256 / ay);
    let len = i64::from(jane_core::num::isqrt((x * x + y * y) as u64));
    // At the middle: away from him.
    let (ux, uy) = if len < 32 { (x - hx, y - hy) } else { (x, y) };
    let ulen = i64::from(jane_core::num::isqrt((ux * ux + uy * uy) as u64)).max(1);
    // Round away from him: the side of her he is on, turned the other way (35 degrees on).
    let side = if x * hy - y * hx > 0 { -1 } else { 1 };
    let (c, s) = (210, 147 * side);
    let (rx, ry) = ((ux * c - uy * s) / 256, (ux * s + uy * c) / 256);
    Vec2::new(Fx((i64::from(centre.x.0) + rx * ax / ulen) as i32), Fx((i64::from(centre.y.0) + ry * ay / ulen) as i32))
}

/// Cells from `c` to the nearest cell that is not open ground, counted in square rings, up to
/// five: the middle of a room is where a kite goes on, a corner is where it ends.
fn clearance(v: &View<'_>, c: (i32, i32)) -> i64 {
    for k in 1..=5 {
        for d in -k..=k {
            for (x, y) in [(c.0 + d, c.1 - k), (c.0 + d, c.1 + k), (c.0 - k, c.1 + d), (c.0 + k, c.1 + d)] {
                if !crate::nav::walkable(v, x, y) {
                    return i64::from(k - 1);
                }
            }
        }
    }
    5
}

/// Path lengths over the ground in a box about a point, tenths of a cell (a diagonal step is 14,
/// and only where both its sides are open), by a Dijkstra from `from`. Cells outside the box or
/// not reached are `i64::MAX`.
struct Ground {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    cost: Vec<i64>,
    prev: Vec<u32>,
}

impl Ground {
    fn ix(&self, (x, y): (i32, i32)) -> Option<usize> {
        let (dx, dy) = (x - self.x0, y - self.y0);
        (dx >= 0 && dy >= 0 && dx < self.w && dy < self.h).then(|| (dy * self.w + dx) as usize)
    }

    fn at(&self, c: (i32, i32)) -> i64 {
        self.ix(c).map_or(i64::MAX, |i| self.cost[i])
    }

    fn cell(&self, i: usize) -> (i32, i32) {
        (self.x0 + i as i32 % self.w, self.y0 + i as i32 / self.w)
    }

    /// From `from`, in a box of `r` cells about `centre`; a cell is entered only when `keep`
    /// allows it (at its cost).
    fn flood(
        v: &View<'_>,
        centre: (i32, i32),
        r: i32,
        from: (i32, i32),
        keep: &dyn Fn((i32, i32), i64) -> bool,
    ) -> Ground {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let (w, h) = (2 * r + 1, 2 * r + 1);
        let mut g = Ground {
            x0: centre.0 - r,
            y0: centre.1 - r,
            w,
            h,
            cost: vec![i64::MAX; (w * h) as usize],
            prev: vec![u32::MAX; (w * h) as usize],
        };
        let Some(s) = g.ix(from) else { return g };
        g.cost[s] = 0;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0i64, s)));
        let open = |x: i32, y: i32| crate::nav::walkable(v, x, y);
        while let Some(Reverse((c, i))) = heap.pop() {
            if c > g.cost[i] {
                continue;
            }
            let (x, y) = g.cell(i);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let n = (x + dx, y + dy);
                let diag = dx != 0 && dy != 0;
                if !open(n.0, n.1) || diag && !(open(x + dx, y) && open(x, y + dy)) {
                    continue;
                }
                let Some(j) = g.ix(n) else { continue };
                let nc = c + if diag { 14 } else { 10 };
                if nc < g.cost[j] && keep(n, nc) {
                    g.cost[j] = nc;
                    g.prev[j] = i as u32;
                    heap.push(Reverse((nc, j)));
                }
            }
        }
        g
    }
}

/// Where to step to keep away from him at `from`: his path lengths over the ground, and hers, only
/// through cells she reaches well before he could (his pace against hers, `pace` tenths: the way
/// out of a corner is never past him); of those, the one that leaves her most ahead of him with
/// open ground about it (a corner is where a kite ends), inside the tether. The first steps of the
/// way there, as a point to steer at; `None` when every way is closed (then straight away).
fn kite_point(
    v: &View<'_>,
    me: Vec2,
    from: Vec2,
    pace: i64,
    tether: Option<(Vec2, i64)>,
    orbit: Option<Vec2>,
    room: Option<jane_core::Rect>,
) -> Option<Vec2> {
    let start = crate::nav::nearest_walkable(v, me.cell().0, me.cell().1, 1)?;
    let his = crate::nav::nearest_walkable(v, from.cell().0, from.cell().1, 2)?;
    let him = Ground::flood(v, start, 24, his, &|_, _| true);
    // Ahead of him by two cells and a half; pressed, by less; at the last, past the tether.
    [(25, tether), (10, tether), (0, tether), (0, None)]
        .into_iter()
        .find_map(|(margin, tether)| kite_on(v, start, &him, margin, pace, tether, orbit, room))
}

/// [`kite_point`] with her kept `margin` tenths of a cell ahead of him all the way.
#[allow(clippy::too_many_arguments)]
fn kite_on(
    v: &View<'_>,
    start: (i32, i32),
    him: &Ground,
    margin: i64,
    pace: i64,
    tether: Option<(Vec2, i64)>,
    orbit: Option<Vec2>,
    room: Option<jane_core::Rect>,
) -> Option<Vec2> {
    use crate::nav::dist;
    let lead = |c: (i32, i32), mine: i64| him.at(c) == i64::MAX || him.at(c) - mine * pace / 10 >= margin;
    let her = Ground::flood(v, start, 16, start, &|c, mine| mine <= 100 && lead(c, mine));
    let mut best: Option<(i64, usize)> = None;
    for (i, &mine) in her.cost.iter().enumerate() {
        if mine == i64::MAX || mine == 0 {
            continue;
        }
        let c = her.cell(i);
        let at = Vec2::centre(c.0, c.1);
        if tether.is_some_and(|(home, rad)| dist(at, home) > rad) || room.is_some_and(|r| !r.contains(c.0, c.1)) {
            continue;
        }
        let ahead = him.at(c).min(400) - mine * pace / 10;
        // Round the room rather than into its end: toward the point ahead on the loop.
        let round = orbit.map_or(0, |o| dist(at, o) * 8 / i64::from(CELL_FX));
        let score = ahead + clearance(v, c) * 30 - round;
        if best.is_none_or(|(b, bi)| (score, std::cmp::Reverse(i)) > (b, std::cmp::Reverse(bi))) {
            best = Some((score, i));
        }
    }
    let (_, mut i) = best?;
    // Back along the way to the third step from her feet.
    let mut path = vec![i];
    while her.prev[i] != u32::MAX && her.cost[her.prev[i] as usize] > 0 {
        i = her.prev[i] as usize;
        path.push(i);
    }
    let k = path.len().saturating_sub(3);
    path.get(k).map(|&i| {
        let (x, y) = her.cell(i);
        Vec2::centre(x, y)
    })
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
