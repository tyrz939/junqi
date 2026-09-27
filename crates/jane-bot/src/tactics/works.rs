//! The pipes and the Factory (DUNGEONS.md §3.4): see `mod.rs`.
//!
//! *Light is how the machines see you.* A sentry, a hauler or the Foreman (`sight: "lit"` in its
//! row) notices only what stands in a prop's light, and keeps only what stays in it; out of it,
//! it stands down. So:
//!
//! - a sentry or a hauler is never hunted, nor fought back: a sentry is walked past in the dark
//!   and across its light at a run, a hauler is weather (DUNGEONS.md: "cannot be fought, only
//!   avoided"); an idle one mends faster than she can hurt it;
//! - her feet pay a toll for lit floor while one of them stands in the zone ([`observe`],
//!   `Nav::toll`): the long way round in the dark is the short way;
//! - a sentry that guards a locked thing (the generator hall's, over the orb) is put down from a
//!   lit cell in its sight ([`guards`], [`snipe`]): from the dark it never fights;
//! - the Charge Hand is kept at bolt range ([`kite`]): his live hand takes most of her in one
//!   grip, so he is never let near, frost slowing him while she backs off across open floor;
//! - the Foreman is fought from the dark ([`from_dark`]) with the verb he is weak to: there he
//!   cannot see her ("he stops"), and a floor grid under him is sparked instead of him;
//! - a floor grid is not sprung on nothing, nor a fuse by the Foreman blown ([`hold_fire`]).

use jane_core::action::{Action, School};
use jane_core::angle::iatan2;
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Fx, SpellId, Vec2, ZoneId};
use jane_data::{Answers, UnitSight};
use jane_sim::ids::UnitId;
use jane_sim::{Command, InputFrame, Unit, View};

use crate::nav::{Go, dist, stick, walkable};
use crate::sense::{self, holds, knows};
use crate::task::Ctx;
use crate::{Act, fight};

/// What a step onto lit floor costs over the step, in tenths, while anything that sees by light
/// stands in the zone (a straight step is 10).
pub const LIT_TOLL: u8 = 40;

/// How far out, in cells, she looks for floor to back off across.
const STEER_R: i32 = 4;

/// Frames between re-reckonings of the lit floor (lamps go on and off with the fuses).
const TOLL_EVERY: u32 = 20;

/// Does this thing see only by light (the Factory's sentries, haulers and Foreman)?
pub fn sees_by_light(u: &Unit) -> bool {
    jane_data::catalog().combat.unit(u.def).sight == UnitSight::Lit
}

/// Never fought unless sent after: it sees only by light and is no boss (a sentry, a hauler).
/// She gets on with what she was doing, out of its light.
pub fn ignore(u: &Unit) -> bool {
    sees_by_light(u) && !jane_data::catalog().combat.unit(u.def).boss
}

/// Leave it be: [`ignore`]d and not on her (the crawl does not hunt it, nor walk back to where
/// it stood).
pub fn leave_be(v: &View<'_>, u: &Unit) -> bool {
    ignore(u) && !fight::on_me(v, u)
}

/// Where a boss stands in the crawl's order (its class: 9, last, as a rule). The Foreman is
/// taken as soon as he can be reached and she has the verb he is weak to (4, with the reading
/// and the using): he is fought from the dark, which costs nothing to wait for, and the halls
/// the crawl would walk first are the Factory's twenty minutes.
pub fn boss_class(v: &View<'_>, def: jane_core::UnitDefId) -> u8 {
    let row = jane_data::catalog().combat.unit(def);
    if row.sight == UnitSight::Lit && knows(v, sense::spell("spark")) { 4 } else { 9 }
}

/// Stands still, sees only by light, no boss: a sentry.
fn sentry(u: &Unit) -> bool {
    ignore(u) && jane_data::catalog().combat.unit(u.def).walk.0 == 0
}

/// Is the point in a warm prop light, by the sim's rule (two thirds of the radius)?
pub fn lit(v: &View<'_>, at: Vec2) -> bool {
    v.props().any(|p| {
        v.light_showing(p).is_some_and(|l| !l.cold && dist_sq(sense::prop_centre(p), at) <= reach_sq(l.radius))
    })
}

/// How far a light counts as lit, squared (the sim's `light::reach_sq`).
fn reach_sq(radius: Fx) -> i64 {
    let r = i64::from(radius.0);
    r * r * 4 / 9
}

/// Anything standing here that sees by light and is no boss?
fn watched(v: &View<'_>) -> bool {
    sense::enemies(v).iter().any(|u| ignore(u))
}

/// What the story wants from the Factory is in hand (quest Not Relieved): the Foreman is down
/// and the key from his locker (the Chairman's, `opens: burial_gate`, the only key to the
/// Burial's stair) is in her bag, and a door out to the county is in reach (shut in behind the
/// assembly, the roller door's socket is sparked first). The crawl then walks out, by the
/// roller door or the way she came, rather than walking every hall of the building again.
pub fn done(v: &View<'_>, reach: &crate::crawl::Reach, zone: ZoneId, boss_down: bool) -> bool {
    let cat = jane_data::catalog();
    let stair = cat.name_id("burial_gate");
    zone == ZoneId::Factory
        && boss_down
        && stair.is_some()
        && v.me().bag.iter().flatten().any(|s| cat.combat.item(s.item).opens == stair)
        && v.props().any(|p| {
            !p.locked && !p.hidden && sense::door_of(v, p).is_some_and(|d| d.zone == ZoneId::County) && reach.beside(p)
        })
}

/// Each frame of a crawl: the toll on lit floor, kept up to date while anything that sees by
/// light stands here, and taken off when nothing does.
pub fn observe(v: &View<'_>, cx: &mut Ctx) {
    let here = cx.nav.toll.as_ref().is_some_and(|t| t.0 == v.zone());
    if !watched(v) {
        if cx.nav.toll.is_some() {
            cx.nav.toll = None;
        }
        return;
    }
    if here && v.frame() % TOLL_EVERY != 0 {
        return;
    }
    let (w, h) = v.size();
    let mut costs = vec![0u8; (w * h) as usize];
    for p in v.props() {
        let Some(l) = v.light_showing(p) else { continue };
        if l.cold {
            continue;
        }
        let c = sense::prop_centre(p);
        let r2 = reach_sq(l.radius);
        let rc = l.radius.0 / CELL_FX + 1;
        let (px, py) = c.cell();
        for y in (py - rc).max(0)..=(py + rc).min(h as i32 - 1) {
            for x in (px - rc).max(0)..=(px + rc).min(w as i32 - 1) {
                if dist_sq(Vec2::centre(x, y), c) <= r2 {
                    costs[(y as u32 * w + x as u32) as usize] = LIT_TOLL;
                }
            }
        }
    }
    cx.nav.toll = Some((v.zone(), w, costs));
}

/// What stands guard over a locked thing with no key to it, in reach (the generator hall's
/// orb, locked while the Charge Hand and the hall's sentry stand), once no boss is left about
/// it: each sentry within twenty cells of it. The crawl hunts these; [`engage`] fights one
/// from its light.
pub fn guards(v: &View<'_>, reach: &crate::crawl::Reach) -> Vec<(UnitId, Vec2)> {
    let cat = jane_data::catalog();
    let r = i64::from(20 * CELL_FX);
    let enemies = sense::enemies(v);
    let mut out = Vec::new();
    for p in v.props() {
        let def = cat.story.prop(p.def);
        let tagless = v.prop_spawn(p).is_some_and(|s| s.key_tag.is_none());
        if !p.locked || p.hidden || def.gate || !tagless || sense::door_of(v, p).is_some() || !reach.beside(p) {
            continue;
        }
        let at = sense::prop_centre(p);
        if enemies.iter().any(|u| cat.combat.unit(u.def).boss && dist(u.pos, at) <= r) {
            continue;
        }
        for u in &enemies {
            if sentry(u) && dist(u.pos, at) <= r {
                out.push((u.id, u.pos));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The Factory's bosses, fought as their rooms ask, and a guard she was sent after, fought
/// from its light: `Some(act)` when this tactic has the fight (the inner `None`: it is over for
/// now), `None` when it is not one of them.
pub fn engage(v: &View<'_>, cx: &mut Ctx, id: UnitId) -> Option<Option<Act>> {
    let t = v.unit(id).filter(|u| u.alive)?;
    let cat = jane_data::catalog();
    if ignore(t) {
        return Some(snipe(v, cx, t));
    }
    if sees_by_light(t) {
        return Some(from_dark(v, cx, t));
    }
    if cat.combat.unit(t.def).id != "charge_hand" {
        return None;
    }
    // Frost first: it slows him, and he shrugs off shock.
    Some(Some(kite(v, cx, t, &["icebolt", "spark"].map(sense::spell))))
}

/// What she is on, and for how long, for a fight this tactic has.
fn begin(cx: &mut Ctx, t: &Unit) {
    if cx.fight.target != Some(t.id) {
        cx.fight.target = Some(t.id);
        cx.fight.t = 0;
        cx.fight.fights += 1;
        cx.fight.retreat = None;
    }
    cx.fight.t += 1;
    cx.foes.insert(t.id);
}

/// A cell she can stand in, lit or dark as `want_lit` says, `near` to `far` from `at` and in its
/// sight, with half a cell to spare every way (on the edge of a light, or of the range, she
/// steps in and out of it where she stands): the nearest to her. `None`: there is none.
fn spot(v: &View<'_>, at: Vec2, near: i64, far: i64, want_lit: bool) -> Option<Vec2> {
    let me = v.body().pos;
    let (tx, ty) = at.cell();
    let rc = (far / i64::from(CELL_FX)) as i32 + 1;
    let half = CELL_FX / 2;
    let mut best: Option<(i64, (i32, i32))> = None;
    for y in ty - rc..=ty + rc {
        for x in tx - rc..=tx + rc {
            if !walkable(v, x, y) {
                continue;
            }
            let c = Vec2::centre(x, y);
            let d = dist(c, at);
            if d < near + i64::from(half) || d > far - i64::from(half) || !v.sight(c, at) {
                continue;
            }
            let firm = [(0, 0), (half, 0), (-half, 0), (0, half), (0, -half)]
                .into_iter()
                .all(|(dx, dy)| lit(v, Vec2 { x: Fx(c.x.0 + dx), y: Fx(c.y.0 + dy) }) == want_lit);
            if !firm {
                continue;
            }
            let k = dist(me, c);
            if best.is_none_or(|b| (k, (x, y)) < b) {
                best = Some((k, (x, y)));
            }
        }
    }
    best.map(|(_, (x, y))| Vec2::centre(x, y))
}

/// Give up on a fight for a minute (no light to fight it from, no way there).
fn let_be(v: &View<'_>, cx: &mut Ctx, t: &Unit) -> Option<Act> {
    cx.fight.unreachable.insert(t.id, v.frame() + 60 * 60);
    cx.fight.hunt = None;
    cx.fight.target = None;
    None
}

/// A bolt at `t` with the first of `spells` she knows and has ready, in range and in sight.
fn bolt(v: &View<'_>, t: &Unit, spells: &[SpellId]) -> Option<Act> {
    let cat = jane_data::catalog();
    let me = v.body();
    if !v.sight(me.pos, t.pos) {
        return None;
    }
    let gap = fight::gap(me, t);
    let dir = iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    spells
        .iter()
        .find(|&&s| {
            knows(v, s) && fight::ready(me, s, v.tick()) && gap <= i64::from(cat.combat.spell(s).range.0) * 9 / 10
        })
        .map(|&s| Act {
            frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
            cmds: vec![Command::Cast { spell: s, on: Some(t.id) }],
        })
}

/// Walk to `to` for a fight; `None` when there is no way.
fn walk_to(v: &View<'_>, cx: &mut Ctx, to: Vec2) -> Option<InputFrame> {
    match cx.nav.go(v, to, Fx::from_px(3), false) {
        Go::Walk(f) => Some(f),
        Go::Arrived => Some(stick(v.body().pos, to, false)),
        Go::NoWay => None,
    }
}

/// A thing that sees only by light and stands still, put down from a lit cell in its sight
/// (from the dark it never fights, and mends faster than she can hurt it): to the nearest such
/// cell three to nine out from it, then bolts, holding the light between them.
fn snipe(v: &View<'_>, cx: &mut Ctx, t: &Unit) -> Option<Act> {
    begin(cx, t);
    let me = v.body();
    let dir = iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    let (near, far) = (i64::from(3 * CELL_FX), i64::from(9 * CELL_FX));
    let good = |at: Vec2| {
        let d = dist(at, t.pos);
        d >= near && d <= far && v.sight(at, t.pos) && lit(v, at)
    };
    if good(me.pos) {
        cx.fight.retreat = None;
        let icy = ["icebolt", "spark"].map(sense::spell);
        return Some(bolt(v, t, &icy).unwrap_or(Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE })));
    }
    let to = match cx.fight.retreat.filter(|&r| good(r)) {
        Some(r) => r,
        None => match spot(v, t.pos, near, far, true) {
            Some(r) => {
                cx.fight.retreat = Some(r);
                r
            }
            None => return let_be(v, cx, t),
        },
    };
    match walk_to(v, cx, to) {
        Some(f) => Some(Act::hold(f)),
        None => let_be(v, cx, t),
    }
}

/// A boss that sees only by light (the Foreman), fought from the dark with the verb he is weak
/// to (DUNGEONS.md §3.4: out of the light "he cannot see her ... so he stops"): from a dark cell
/// five to ten out from him and in his sight, a spark whenever one is ready, the floor grid he
/// stands on sparked instead when there is one; when he has her and comes on, away across dark
/// floor.
fn from_dark(v: &View<'_>, cx: &mut Ctx, t: &Unit) -> Option<Act> {
    begin(cx, t);
    let me = v.body();
    let d = dist(me.pos, t.pos);
    let dir = iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    let (near, far) = (i64::from(5 * CELL_FX), i64::from(10 * CELL_FX));
    if fight::on_me(v, t) && d < near {
        let f = steer(v, cx, t.pos, None, Some(false), true);
        return Some(Act::hold(InputFrame { aim: Some(dir), ..f }));
    }
    let good = |at: Vec2| {
        let k = dist(at, t.pos);
        k >= near && k <= far && v.sight(at, t.pos) && !lit(v, at)
    };
    if good(me.pos) {
        cx.fight.retreat = None;
        if let Some(a) = grid(v, t) {
            return Some(a);
        }
        let shock = ["spark", "icebolt"].map(sense::spell);
        return Some(bolt(v, t, &shock).unwrap_or(Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE })));
    }
    let to = match cx.fight.retreat.filter(|&r| good(r)) {
        Some(r) => r,
        None => match spot(v, t.pos, near, far, false) {
            Some(r) => {
                cx.fight.retreat = Some(r);
                r
            }
            None => return let_be(v, cx, t),
        },
    };
    match walk_to(v, cx, to) {
        Some(f) => Some(Act::hold(InputFrame { aim: Some(dir), ..f })),
        None => let_be(v, cx, t),
    }
}

/// A strong thing kept at bolt range: a bolt whenever one is ready and it is far enough off
/// that the cast's stop does not let it reach her; otherwise away across open floor, never
/// out past its leash (it would go home and mend).
fn kite(v: &View<'_>, cx: &mut Ctx, t: &Unit, bolts: &[SpellId]) -> Act {
    let cat = jane_data::catalog();
    begin(cx, t);
    let me = v.body();
    let now = v.tick();
    let d = dist(me.pos, t.pos);
    let dir = iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    // What she brought for it: stone skin (it hits with its fists too).
    if d < i64::from(14 * CELL_FX) && me.statuses.is_empty() {
        let p = sense::item("potion_stoneskin");
        if holds(v, p) > 0 && fight::item_ready(me, p, now) {
            return Act::press(Command::Item(p));
        }
    }
    // Two of its blows from dead, with food in the bag: she eats between bolts (at better than
    // half health, eleven apples uneaten, two blows of the Charge Hand's killed her).
    if me.hp.points() <= 2 * fight::max_hit(t) {
        if let Some(f) = fight::food(v) {
            return Act::press(Command::Item(f));
        }
    }
    // Far enough that the cast's stop does not let it reach her: its reach (both bodies and a
    // melee's), and what it closes at its pace (slowed or not) while she stands, and a cell.
    let row = cat.combat.unit(t.def);
    let reach = i64::from(row.bounds.0) + i64::from(cat.combat.unit(me.def).bounds.0) + i64::from(CELL_FX) / 2;
    let pace = i64::from(row.run.0) * i64::from(jane_sim::status::speed_factor(t, now)) / 1000;
    let stop = bolts.iter().map(|&s| i64::from(cat.combat.spell(s).stop.0)).max().unwrap_or(30);
    if d >= reach + pace * stop + i64::from(CELL_FX) {
        if let Some(a) = bolt(v, t, bolts) {
            return a;
        }
    }
    let leash = i64::from(cat.combat.unit(t.def).leash.0);
    let tether = (leash > 0).then_some((t.home, leash * 2 / 3));
    if d < i64::from(8 * CELL_FX) {
        let f = steer(v, cx, t.pos, tether, None, d < i64::from(6 * CELL_FX));
        return Act::hold(InputFrame { aim: Some(dir), ..f });
    }
    cx.fight.retreat = None;
    if d > i64::from(11 * CELL_FX) || !v.sight(me.pos, t.pos) {
        let f = match cx.nav.go(v, t.pos, Fx(8 * CELL_FX), false) {
            Go::Walk(f) => f,
            _ => stick(me.pos, t.pos, false),
        };
        return Act::hold(InputFrame { aim: Some(dir), ..f });
    }
    Act::hold(InputFrame { aim: Some(dir), ..InputFrame::IDLE })
}

/// The rect a prop's use list strikes (a floor grid's), if it strikes one.
fn strike_rect(v: &View<'_>, p: &jane_sim::Prop) -> Option<jane_core::Rect> {
    let list = v.prop_spawn(p)?.use_list?;
    v.list(list).iter().find_map(|a| match *a {
        Action::Strike { rect, .. } => v.rect(v.key_sym(rect)),
        _ => None,
    })
}

/// The face of a prop toward `from`: the first cell out of its footprint on the way to her
/// (what a bolt at a prop that blocks sight stops on, and touches it from).
fn face(p: &jane_sim::Prop, from: Vec2) -> Vec2 {
    let r = sense::prop_rect(p);
    let c = sense::prop_centre(p);
    let (sx, sy) = ((from.x.0 - c.x.0).signum(), (from.y.0 - c.y.0).signum());
    let (mut x, mut y) = c.cell();
    while r.contains(x, y) {
        x += sx;
        y += sy;
    }
    Vec2::centre(x, y)
}

/// `t` stands on a floor grid whose socket she can spark from here: the spark, at the socket.
fn grid(v: &View<'_>, t: &Unit) -> Option<Act> {
    let cat = jane_data::catalog();
    let spark = cat.combat.spell_id("spark")?;
    let me = v.body();
    if !knows(v, spark) || !fight::ready(me, spark, v.tick()) {
        return None;
    }
    let range = i64::from(cat.combat.spell(spark).range.0) * 9 / 10;
    let (tx, ty) = t.pos.cell();
    v.props()
        .filter(|p| {
            !p.hidden && !p.on && cat.story.prop(p.def).answers.and_then(Answers::school) == Some(School::Shock)
        })
        .filter(|p| strike_rect(v, p).is_some_and(|r| r.contains(tx, ty)))
        .map(|p| (p, face(p, me.pos)))
        .find(|&(_, f)| dist(me.pos, f) <= range && v.sight(me.pos, f))
        .map(|(p, _)| {
            let c = sense::prop_centre(p);
            let dir = iatan2(c.y.0 - me.pos.y.0, c.x.0 - me.pos.x.0);
            Act {
                frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
                cmds: vec![Command::Cast { spell: spark, on: None }],
            }
        })
}

/// Hold fire at a prop that answers her bolt (the crawl asks before it sparks one): a trap
/// (a floor grid that strikes a rect) is left to the fight, which springs it on the boss, and
/// a fuse that puts out lamps is left alone while a boss who sees by light stands within thirty
/// cells of it (the assembly's: lit or dark is the fight's to have, not the crawl's).
pub fn hold_fire(v: &View<'_>, p: &jane_sim::Prop) -> bool {
    let cat = jane_data::catalog();
    // The Factory's rule only: the Burial's braziers strike too, and are the crawl's to light.
    if v.zone() != ZoneId::Factory {
        return false;
    }
    let Some(list) = v.prop_spawn(p).and_then(|s| s.use_list) else { return false };
    let enemies = sense::enemies(v);
    // A trap is the fight's to spring, on the boss standing in it ([`grid`]), not the crawl's.
    if strike_rect(v, p).is_some() {
        return true;
    }
    let darkens = v.list(list).iter().any(|a| match *a {
        Action::Switch { prop, on: Some(false) } => {
            sense::prop_by_key(v, v.key_sym(prop)).is_some_and(|l| cat.story.prop(l.def).light.is_some())
        }
        _ => false,
    });
    let at = sense::prop_centre(p);
    darkens
        && enemies
            .iter()
            .any(|u| cat.combat.unit(u.def).boss && sees_by_light(u) && dist(u.pos, at) <= i64::from(30 * CELL_FX))
}

/// Away from `from` across open floor: the best of the cells four out from her that a straight
/// line reaches, for its distance from the threat, the room about it (a corner is where she is
/// caught), inside the tether, and lit or dark as `want` says (`None`: out of the light while
/// a sentry stands here). The choice is kept until she is there or it is no longer away.
fn steer(
    v: &View<'_>,
    cx: &mut Ctx,
    from: Vec2,
    tether: Option<(Vec2, i64)>,
    want: Option<bool>,
    run: bool,
) -> InputFrame {
    let me = v.body().pos;
    let (mx, my) = me.cell();
    let keep = cx.fight.retreat.filter(|&r| {
        let (rx, ry) = r.cell();
        dist(r, me) > i64::from(CELL_FX) && dist(r, from) > dist(me, from) && walkable(v, rx, ry)
    });
    let to = match keep {
        Some(r) => r,
        None => {
            let (want, cost) = match want {
                Some(w) => (Some(w), i64::from(10 * CELL_FX)),
                None => (watched(v).then_some(false), i64::from(6 * CELL_FX)),
            };
            let mut best: Option<(i64, (i32, i32))> = None;
            for k in 0..8 * STEER_R {
                let (dx, dy) = ring(k, STEER_R);
                let (x, y) = (mx + dx, my + dy);
                if !line_clear(v, (mx, my), (x, y)) {
                    continue;
                }
                let c = Vec2::centre(x, y);
                if tether.is_some_and(|(home, r)| dist(c, home) > r) {
                    continue;
                }
                let mut score = dist(c, from) + openness(v, x, y) * i64::from(CELL_FX) / 4;
                if want.is_some_and(|w| lit(v, c) != w) {
                    score -= cost;
                }
                if best.is_none_or(|b| (score, (x, y)) > b) {
                    best = Some((score, (x, y)));
                }
            }
            match best {
                Some((_, (x, y))) => {
                    let r = Vec2::centre(x, y);
                    cx.fight.retreat = Some(r);
                    r
                }
                // Nowhere four out: straight away.
                None => Vec2 { x: Fx(2 * me.x.0 - from.x.0), y: Fx(2 * me.y.0 - from.y.0) },
            }
        }
    };
    stick(me, to, run)
}

/// The `k`th cell of the square ring of radius `r` about the origin, going round.
fn ring(k: i32, r: i32) -> (i32, i32) {
    let side = 2 * r;
    match k / side {
        0 => (-r + k % side, -r),
        1 => (r, -r + k % side),
        2 => (r - k % side, r),
        _ => (-r, r - k % side),
    }
}

/// A straight walk from `a` to `b` crosses only walkable cells (sampled every half cell).
fn line_clear(v: &View<'_>, a: (i32, i32), b: (i32, i32)) -> bool {
    let n = 2 * (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
    (1..=n).all(|i| {
        let x = a.0 * 2 * n + (b.0 - a.0) * 2 * i + n;
        let y = a.1 * 2 * n + (b.1 - a.1) * 2 * i + n;
        walkable(v, x.div_euclid(2 * n), y.div_euclid(2 * n))
    })
}

/// Walkable cells in the 5 x 5 about a cell (a corner has few).
fn openness(v: &View<'_>, x: i32, y: i32) -> i64 {
    let mut n = 0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            if walkable(v, x + dx, y + dy) {
                n += 1;
            }
        }
    }
    n
}
