//! Combat through commands (PORT.md §7 P4; ARCHITECTURE.md §4.2 steps 5 and 8 to 12, §4.3).
//! Carries the combat parts of `jane/test/sim.test.ts` ("cast pipeline", "determinism" with
//! casts on the tape), `verbs2.test.ts` (E7 Explosion and Spark, E10 phases, `dev kill`),
//! `coop.test.ts` (the co-op penalty, one fire, growth is the world's) and `budget.test.ts`
//! (sleepers stand up on schedule), on the field of `tests/field` and on the real seed. The
//! pipeline's inside (the queue before the flush, casts by an AI, a made-up heal row) is
//! `src/combat_tests.rs`.

mod common;
mod field;

use field::*;
use jane_core::action::School;
use jane_core::{Angle, Milli, Sfc32, Tick, Vec2, ZoneId};
use jane_sim::event::{EventKind, SpellError, ToastKind};
use jane_sim::state::{FlagKey, RestPoint, StatusInst};
use jane_sim::units::{max_hp, max_mp};
use jane_sim::{
    AssistProfile, ClientToken, Command, DevOp, Hit, InputFrame, Seat, Sim, StampedCommand, StepInput, UnitId,
};

fn hit(to: UnitId, points: i32, school: School, from: Option<UnitId>) -> Hit {
    Hit { to, amount: Milli::from_points(points), school, from, crit: false, status: None }
}

fn effect(id: &str) -> jane_core::EffectId {
    jane_data::catalog().combat.effect_id(id).unwrap()
}

/// A status on a unit from now for `ticks`, pulsing from its row's first pulse.
fn put_status(s: &mut Sim, id: UnitId, e: &str, ticks: u32, from: Option<UnitId>) {
    let now = s.state().tick;
    let def = jane_data::catalog().combat.effect(effect(e));
    let next_pulse = def.pulse.map_or(Tick::ZERO, |p| now.after(p.every));
    edit(s, id, |u| {
        u.statuses.push(StatusInst {
            effect: effect(e),
            until: now.after(Tick(ticks)),
            next_pulse,
            from,
            pool: Milli::ZERO,
        });
    });
}

fn strong(s: &mut Sim, id: UnitId, strength: u16) {
    edit(s, id, |u| {
        u.strength = strength;
        u.hp = max_hp(u);
    });
}

// --- the cast pipeline ------------------------------------------------------------------------

/// sim.test.ts "cast pipeline": a bolt from the bar's spell flies, lands, leaves its status,
/// and the first blow pulls the skeleton's aggro from where it stood.
#[test]
fn a_bolt_flies_lands_chills_and_pulls_aggro() {
    let mut s = field();
    learn(&mut s, "icebolt");
    let foe = spawn(&mut s, "skeleton", 16, 10);
    let mp = unit(&s, me(&s)).mp;
    s.drain_events();
    cast(&mut s, 0, "icebolt", aim(Angle::EAST), None);
    assert_eq!(s.state().zone(Z).unwrap().projectiles.len(), 1);
    assert_eq!(unit(&s, me(&s)).mp.0, mp.0 - 14_000 + i32::from(unit(&s, me(&s)).spirit), "paid, then a tick of regen");
    steps(&mut s, 12);
    let ev = events(&mut s);
    let dealt = damage_to(&ev, foe);
    assert_eq!(dealt.len(), 1);
    let f = unit(&s, foe);
    assert_eq!(f.hp.0, max_hp(f).0 - dealt[0].0);
    assert!(dealt[0].0 % 1000 == 0 && dealt[0].0 > 0, "a spell's blow is whole points");
    assert!(f.statuses.iter().any(|st| st.effect == effect("chilled")));
    assert_eq!((f.target, f.combat), (Some(me(&s)), jane_sim::state::CombatState::Combat));
    assert!(s.state().zone(Z).unwrap().projectiles.is_empty());
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::Cast { unit, .. } if unit == me(&s))));
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::Impact { school: School::Frost, .. })));
}

/// sim.test.ts "melee is forgiving": the bar's first slot, aimed east, finds the skeleton west.
#[test]
fn the_bar_swings_and_melee_finds_what_is_behind_her() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 9, 10);
    s.drain_events();
    cmd_with(&mut s, Some(0), Command::Bar { slot: 0, on: None }, aim(Angle::EAST));
    let ev = events(&mut s);
    assert_eq!(damage_to(&ev, foe).len(), 1, "the blow lands in the step that dealt it");
    assert_eq!(unit(&s, me(&s)).facing, jane_core::action::Facing::West);
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::Swing { .. })));
    // The GCD holds the next swing; a failure says why only when it is worth saying.
    cmd_with(&mut s, Some(0), Command::Bar { slot: 0, on: None }, aim(Angle::EAST));
    let ev = events(&mut s);
    assert!(
        ev.iter()
            .any(|e| matches!(e.kind, EventKind::CastFailed { why: SpellError::OnCooldown | SpellError::OnGcd, .. }))
    );
    assert!(!ev.iter().any(|e| matches!(e.kind, EventKind::Toast(ToastKind::SpellError(_)))));
    // A spell she does not know is not cast at all.
    cast(&mut s, 0, "fireball", aim(Angle::EAST), None);
    assert!(events(&mut s).iter().all(|e| !matches!(e.kind, EventKind::CastFailed { .. } | EventKind::Cast { .. })));
}

/// verbs2.test.ts E7 "Explosion: everything near the burst takes all of it".
#[test]
fn explosion_splashes_the_whole_blow() {
    let mut s = field();
    learn(&mut s, "explosion");
    let a = spawn(&mut s, "skeleton", 16, 10);
    let b = spawn(&mut s, "skeleton", 17, 11);
    strong(&mut s, a, 400);
    strong(&mut s, b, 400);
    rooted(&mut s, a);
    rooted(&mut s, b);
    cast(&mut s, 0, "explosion", aim(Angle::EAST), None);
    steps(&mut s, 60);
    let (ua, ub) = (unit(&s, a), unit(&s, b));
    let lost = |u: &jane_sim::Unit| max_hp(u).0 - u.hp.0;
    assert!(lost(ua) > 0);
    assert_eq!(lost(ub), lost(ua), "the splash is the whole blow, not a fifth of it");
}

/// verbs2.test.ts E7 "Spark jolts what is weak to shock and nothing else".
#[test]
fn spark_jolts_what_is_weak_to_shock_and_nothing_else() {
    let now_jolted = |s: &Sim, id| {
        let u = unit(s, id);
        jane_sim::status::is_stunned(u, s.state().tick) && u.statuses.iter().any(|st| st.effect == effect("jolted"))
    };
    let mut s = field();
    learn(&mut s, "spark");
    let boss = spawn(&mut s, "foreman", 15, 10);
    cast(&mut s, 0, "spark", aim(Angle::EAST), None);
    steps(&mut s, 8);
    assert!(unit(&s, boss).hp < max_hp(unit(&s, boss)));
    assert!(now_jolted(&s, boss));
    steps(&mut s, 40);
    assert!(!now_jolted(&s, boss), "half a second");
    assert!(unit(&s, boss).statuses.is_empty());

    let mut s = field();
    learn(&mut s, "spark");
    let plain = spawn(&mut s, "skeleton", 15, 10);
    cast(&mut s, 0, "spark", aim(Angle::EAST), None);
    steps(&mut s, 8);
    assert!(unit(&s, plain).hp < max_hp(unit(&s, plain)));
    assert!(unit(&s, plain).statuses.is_empty());
}

// --- the flush ------------------------------------------------------------------------------

/// coop.test.ts "the co-op penalty weakens everyone by head count, server wide, and lifts when
/// they leave".
#[test]
fn the_party_penalty_is_by_head_count_wherever_they_stand() {
    for n in 1..=4u8 {
        let mut s = party(u64::from(n));
        for seat in 1..n {
            cmd(&mut s, Some(seat), Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: start_sym() }));
        }
        assert_eq!(s.state().party_size(), n);
        let foe = spawn(&mut s, "iron_knuckles", 15, 10);
        let (foe_hp, my_hp) = (unit(&s, foe).hp.0, unit(&s, me(&s)).hp.0);
        s.queue_hit(Z, hit(foe, 100, School::Frost, Some(me(&s))));
        s.queue_hit(Z, hit(me(&s), 40, School::Physical, None));
        steps(&mut s, 1);
        let i = usize::from(n - 1);
        let dealt = jane_sim::tuning::PARTY_DEALT[i].0;
        let taken = jane_sim::tuning::PARTY_TAKEN[i].0;
        assert_eq!(foe_hp - unit(&s, foe).hp.0, (100 * i32::from(dealt) + 500) / 1000 * 1000, "{n}: dealt");
        assert_eq!(my_hp - unit(&s, me(&s)).hp.0, (40 * i32::from(taken) + 500) / 1000 * 1000, "{n}: taken");
    }
    // Together is a little better than alone; alone in a party is much worse.
    let (d, t) = (jane_sim::tuning::PARTY_DEALT, jane_sim::tuning::PARTY_TAKEN);
    for n in 2..=4 {
        assert!(i32::from(d[n - 1].0) * n as i32 > 1000 && d[n - 1] < d[n - 2] && t[n - 1] > t[n - 2]);
    }
    // Lifts when they leave. God takes nothing.
    let mut s = party(2);
    cmd(&mut s, Some(1), Command::Leave);
    let foe = spawn(&mut s, "iron_knuckles", 15, 10);
    let hp = unit(&s, foe).hp.0;
    s.queue_hit(Z, hit(foe, 100, School::Frost, Some(me(&s))));
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let mine = unit(&s, me(&s)).hp;
    s.queue_hit(Z, hit(me(&s), 40, School::Physical, None));
    steps(&mut s, 1);
    assert_eq!(hp - unit(&s, foe).hp.0, 100_000);
    assert_eq!(unit(&s, me(&s)).hp, mine);
}

/// The defensive and offensive statuses (`status.ts offenceMods, defenceMods`): a mana shield
/// pays from mp, mana on hit, lifesteal lands the same tick (the flush's second pass), and a
/// weapon status adds its blow and its effect to every swing.
#[test]
fn shields_steals_and_weapon_statuses() {
    let mut s = field();
    let body = me(&s);
    put_status(&mut s, body, "manashield", 1200, None);
    let (hp, mp) = (unit(&s, body).hp, unit(&s, body).mp);
    s.drain_events();
    s.queue_hit(Z, hit(body, 20, School::Physical, None));
    steps(&mut s, 1);
    assert_eq!(unit(&s, body).hp, hp);
    assert_eq!(unit(&s, body).mp.0, mp.0 - 20_000);
    assert!(
        events(&mut s)
            .iter()
            .any(|e| matches!(e.kind, EventKind::Damage { amount: Milli(0), absorbed: Milli(20_000), .. }))
    );

    let mut s = field();
    let body = me(&s);
    put_status(&mut s, body, "sparktongue", 1200, None);
    edit(&mut s, body, |u| u.mp = Milli(1000));
    s.queue_hit(Z, hit(body, 5, School::Physical, None));
    steps(&mut s, 1);
    assert_eq!(unit(&s, body).mp.0, 1000 + i32::from(unit(&s, body).spirit) + 6000, "a tick of regen, then the hit");

    let mut s = field();
    let body = me(&s);
    let foe = spawn(&mut s, "iron_knuckles", 15, 10);
    put_status(&mut s, body, "lifesteal", 1200, None);
    edit(&mut s, body, |u| u.hp = Milli(50_000));
    s.queue_hit(Z, hit(foe, 100, School::Frost, Some(body)));
    steps(&mut s, 1);
    assert_eq!(unit(&s, body).hp, Milli(80_000), "30 % of 100, the same tick");

    let mut s = field();
    let body = me(&s);
    let foe = spawn(&mut s, "skeleton", 11, 10);
    strong(&mut s, foe, 400);
    put_status(&mut s, body, "firelash", 1200, None);
    s.drain_events();
    cmd_with(&mut s, Some(0), Command::Bar { slot: 0, on: None }, aim(Angle::EAST));
    let ev = events(&mut s);
    let schools: Vec<School> = ev
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Damage { unit, school, .. } if unit == foe => Some(school),
            _ => None,
        })
        .collect();
    assert_eq!(schools, [School::Physical, School::Fire]);
    assert!(unit(&s, foe).statuses.iter().any(|st| st.effect == effect("burning")));
}

/// verbs2.test.ts E10 "phases[n].onEnter runs once as the boss crosses into each phase, takes
/// the phase's book, and starts over when it is whole again". The Foreman's rows: 66 % and 33 %,
/// each saying something and shaking the screen.
#[test]
fn a_boss_crosses_its_phases_once_each_and_starts_over_whole() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let boss = spawn(&mut s, "foreman", 14, 10);
    let full = max_hp(unit(&s, boss)).points();
    let def = jane_data::catalog().combat.unit(unit(&s, boss).def);
    let toasts =
        |ev: &[jane_sim::Event]| ev.iter().filter(|e| matches!(e.kind, EventKind::Toast(ToastKind::Text(_)))).count();
    s.drain_events();
    // Nature: the Foreman has no resist to it, so a blow is what it says.
    let hurt = |s: &mut Sim, pct: i32, from: Option<UnitId>, who: UnitId| {
        s.queue_hit(Z, hit(who, full * pct / 100, School::Nature, from));
        steps(s, 1);
        events(s)
    };
    let from = Some(me(&s));
    let ev = hurt(&mut s, 10, from, boss);
    assert_eq!((unit(&s, boss).phase, toasts(&ev)), (0, 0));
    let ev = hurt(&mut s, 30, from, boss);
    assert_eq!((unit(&s, boss).phase, toasts(&ev)), (1, 1));
    assert_eq!(jane_sim::combat::book_of(unit(&s, boss)), def.phases[0].book);
    let ev = hurt(&mut s, 5, from, boss);
    assert_eq!((unit(&s, boss).phase, toasts(&ev)), (1, 0), "once");
    let ev = hurt(&mut s, 30, from, boss);
    assert_eq!((unit(&s, boss).phase, toasts(&ev)), (2, 1));
    assert_eq!(jane_sim::combat::book_of(unit(&s, boss)), def.phases[1].book);

    // One blow through two thresholds runs both lists, in order, from nobody.
    let other = spawn(&mut s, "foreman", 14, 14);
    let ev = hurt(&mut s, 70, None, other);
    assert_eq!((unit(&s, other).phase, toasts(&ev)), (2, 2));
    assert_eq!(unit(&s, other).combat, jane_sim::state::CombatState::Idle, "poison pulls nobody's aggro");

    // Let off, it mends, and the fight starts from the top (walking home: the Foreman does not
    // mend standing at his post, `autoRegen: false`, so the dark is a window, DUNGEONS.md §3.4).
    edit(&mut s, boss, |u| {
        u.combat = jane_sim::state::CombatState::Leash;
        u.target = None;
        u.hp = Milli(max_hp(u).0 - 1000);
    });
    steps(&mut s, 2);
    assert_eq!(unit(&s, boss).phase, 0);
    assert_eq!(jane_sim::combat::book_of(unit(&s, boss)), def.book);
}

/// verbs2.test.ts "dev kill takes what she can see within 25 m: not what is behind a wall, and
/// not what is not there".
#[test]
fn dev_kill_takes_what_she_can_see() {
    let mut s = field();
    let body = me(&s);
    edit(&mut s, body, |u| u.pos = Vec2::centre(36, 30));
    let seen = spawn(&mut s, "rat", 36, 34);
    let hidden = spawn(&mut s, "rat", 33, 30);
    edit(&mut s, hidden, |u| u.hidden = true);
    let behind = spawn(&mut s, "rat", 44, 30);
    let far = spawn(&mut s, "rat", 36, 60);
    cmd(&mut s, Some(0), Command::Dev(DevOp::Kill));
    steps(&mut s, 1);
    let alive = |id| unit(&s, id).alive;
    assert_eq!([alive(seen), alive(hidden), alive(behind), alive(far)], [false, true, true, true]);
}

/// `combat.ts killUnit`, `loot.ts`, `sim.ts respawn`: a kill counts for her, marks the name
/// dead, drops the loot fanned out, and the corpse stands up at home when `sleeping_due` says.
#[test]
fn a_kill_drops_loot_and_the_corpse_stands_up_on_schedule() {
    let mut s = field();
    let rat = spawn(&mut s, "rat", 15, 10);
    let key = s.state_mut().syms.intern("test_rat");
    edit(&mut s, rat, |u| {
        u.key = Some(key);
        u.home = Vec2::centre(20, 20);
    });
    s.drain_events();
    s.queue_hit(Z, hit(rat, 1000, School::Physical, Some(me(&s))));
    steps(&mut s, 1);
    let now = s.state().tick;
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::Death { unit, .. } if unit == rat)));
    let r = unit(&s, rat);
    assert!(!r.alive && r.hp.0 == 0 && r.died_at == Some(now));
    assert_eq!(s.state().players[0].stats.kills, 1);
    assert_eq!(s.state().flags.get(&FlagKey::Dead(key)), Some(&1));
    let zs = s.state().zone(Z).unwrap();
    assert_eq!(zs.sleeping_due, [(now.after(jane_data::catalog().combat.unit(r.def).respawn), rat)]);
    assert_eq!(zs.drops.len(), 1, "a rat always leaves its meat");
    assert!(!s.runtime(Z).unwrap().is_in(rat), "a corpse stands nowhere");
    // The drop ages out unless the story needs it.
    let story = jane_data::catalog().combat.item(zs.drops[0].item).story;
    s.state_mut().zone_mut(Z).unwrap().drops[0].born = Tick(now.0.saturating_sub(18_000 - 1));
    steps(&mut s, 1);
    assert_eq!(s.state().zone(Z).unwrap().drops.len(), usize::from(story));

    // Ten minutes, fast-forwarded: not while she stands over it (nothing stands up in view)...
    s.state_mut().zone_mut(Z).unwrap().sleeping_due[0].0 = s.state().tick.after(Tick(3));
    steps(&mut s, 3);
    assert!(!unit(&s, rat).alive, "she is watching where it lies");
    let due = s.state().zone(Z).unwrap().sleeping_due[0];
    assert_eq!(due, (s.state().tick.after(Tick(jane_sim::tuning::PRESENCE_EVERY)), rat), "a beat later");
    // ...and nor when she stands over its home instead.
    let body = me(&s);
    edit(&mut s, body, |u| u.pos = Vec2::centre(22, 21));
    steps(&mut s, jane_sim::tuning::PRESENCE_EVERY);
    assert!(!unit(&s, rat).alive, "she is watching its home");
    // Out of sight of both, it stands up at home, whole.
    edit(&mut s, body, |u| u.pos = Vec2::centre(15, 32));
    s.drain_events();
    steps(&mut s, jane_sim::tuning::PRESENCE_EVERY);
    let r = unit(&s, rat);
    assert!(r.alive && r.hp == max_hp(r) && r.pos == Vec2::centre(20, 20) && r.died_at.is_none());
    assert!(!s.state().flags.contains_key(&FlagKey::Dead(key)));
    assert!(s.state().zone(Z).unwrap().sleeping_due.is_empty());
    assert!(events(&mut s).iter().any(|e| e.kind == EventKind::Respawn { unit: rat }));
    assert!(s.runtime(Z).unwrap().is_in(rat));
}

/// coop.test.ts "one fire": whoever dies wakes at the party's last fire, else at the door she
/// came in by; the body lies for 240 ticks first.
#[test]
fn she_falls_lies_and_wakes_at_the_fire_or_the_door() {
    let mut s = field();
    let body = me(&s);
    s.drain_events();
    s.queue_hit(Z, hit(body, 10_000, School::Physical, None));
    steps(&mut s, 1);
    let fell = s.state().tick;
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::PlayerDied && e.to == Some(Seat(0))));
    assert_eq!(s.state().players[0].respawn_at, Some(fell.after(Tick(240))));
    assert_eq!(s.state().players[0].stats.deaths, 1);
    assert!(!unit(&s, body).alive);
    walk(&mut s, 239, InputFrame::walk(Angle::EAST));
    assert!(!unit(&s, body).alive, "no walking while dead");
    steps(&mut s, 1);
    let u = unit(&s, body);
    assert!(u.alive && u.hp == max_hp(u) && u.mp == max_mp(u));
    assert_eq!(u.pos, Vec2::centre(10, 10), "the door she came in by");
    assert!(events(&mut s).iter().any(|e| e.kind == EventKind::Toast(ToastKind::WokeAtDoor)));

    // The party's fire, here, then in another zone.
    let fire = Vec2::centre(30, 5);
    s.state_mut().rest = Some(RestPoint { zone: Z, pos: fire });
    s.queue_hit(Z, hit(body, 10_000, School::Physical, None));
    steps(&mut s, 241);
    assert_eq!(unit(&s, body).pos, fire);
    assert!(events(&mut s).iter().any(|e| e.kind == EventKind::Toast(ToastKind::WokeAtRest)));
    s.state_mut().rest = Some(RestPoint { zone: ZoneId::House, pos: Vec2::centre(5, 5) });
    s.queue_hit(Z, hit(body, 10_000, School::Physical, None));
    steps(&mut s, 241);
    assert_eq!(s.state().players[0].zone, ZoneId::House);
    assert_eq!(unit(&s, body).pos, Vec2::centre(5, 5));
}

/// EXPERIENCE.md §2 "The yard skeleton": before anyone has rested, a death wakes her at the Halt
/// fire, the party's from New Game, never at a door she has walked through since (so never
/// resting is never the better plan).
#[test]
fn before_the_first_rest_she_wakes_at_the_halt_fire() {
    let cat = jane_data::catalog();
    let mut s = common::new_game();
    let rest = s.state().rest.expect("a rest point from New Game");
    assert_eq!(rest.zone, Z);
    let fire_key = jane_core::Key::Name(cat.name_id("station_fire").unwrap());
    let fire = s.blueprint(Z).props.iter().find(|p| p.key == fire_key).expect("the Halt fire").cell;
    let (rx, ry) = rest.pos.cell();
    assert!(
        (rx - i32::from(fire.x)).abs() <= 3 && (ry - i32::from(fire.y)).abs() <= 3,
        "beside the Halt fire: {:?} vs {fire:?}",
        rest.pos.cell()
    );
    // Walked into the house (a door she came in by), she dies there: back to the fire.
    s.drain_events();
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: start_sym() }));
    assert_eq!(s.state().players[0].zone, ZoneId::House);
    let body = me(&s);
    s.queue_hit(ZoneId::House, hit(body, 10_000, School::Physical, None));
    steps(&mut s, 242);
    assert_eq!(s.state().players[0].zone, Z);
    assert_eq!(unit(&s, body).pos, rest.pos);
    assert!(events(&mut s).iter().any(|e| e.kind == EventKind::Toast(ToastKind::WokeAtRest)));
}

/// `status.ts tickStatuses`: a burn pulses once a second for its four seconds; a sleeper takes
/// the pulses it slept through as one blow when it wakes (ARCHITECTURE.md §4.3).
#[test]
fn statuses_pulse_and_a_sleeper_takes_what_it_missed_at_once() {
    let mut s = field();
    let awake = spawn(&mut s, "skeleton", 15, 10);
    let asleep = spawn(&mut s, "skeleton", 15, 14);
    let body = me(&s);
    put_status(&mut s, awake, "burning", 240, Some(body));
    put_status(&mut s, asleep, "burning", 240, Some(body));
    edit(&mut s, asleep, |u| u.awake = false);
    s.drain_events();
    steps(&mut s, 250);
    let ev = events(&mut s);
    assert_eq!(damage_to(&ev, awake), [Milli(4000); 4]);
    assert!(unit(&s, awake).statuses.is_empty());
    assert!(ev.iter().any(|e| e.kind == EventKind::Status { unit: awake, effect: effect("burning"), on: false }));
    assert!(damage_to(&ev, asleep).is_empty());
    edit(&mut s, asleep, |u| u.awake = true);
    steps(&mut s, 1);
    let ev = events(&mut s);
    assert_eq!(damage_to(&ev, asleep), [Milli(16_000)]);
    assert!(unit(&s, asleep).statuses.is_empty());
}

/// `status.ts speedFactor, isStunned`: slows multiply her pace, a stun holds her and her casts.
#[test]
fn slows_and_stuns_hold_her() {
    let mut s = field();
    let body = me(&s);
    put_status(&mut s, body, "chilled", 180, None);
    let x0 = unit(&s, body).pos.x.0;
    walk(&mut s, 60, InputFrame::walk(Angle::EAST));
    assert_eq!(unit(&s, body).pos.x.0, x0 + 60 * (256 * 600 / 1000));
    put_status(&mut s, body, "jolted", 30, None);
    let p = unit(&s, body).pos;
    walk(&mut s, 10, InputFrame::walk(Angle::EAST));
    assert_eq!(unit(&s, body).pos, p);
    s.drain_events();
    cmd_with(&mut s, Some(0), Command::Bar { slot: 0, on: None }, aim(Angle::EAST));
    assert!(
        events(&mut s)
            .iter()
            .any(|e| matches!(e.kind, EventKind::CastFailed { why: SpellError::CastUnsuccessful, .. }))
    );
    walk(&mut s, 30, InputFrame::walk(Angle::EAST));
    assert!(unit(&s, body).pos.x.0 > p.x.0, "it wears off");
}

/// A ground spell's `delay` is its tell (the Headmaster's hand-bell, DUNGEONS.md §3.1): the pool
/// lies under her, seen, and bites only when the delay is up. Stood still in it she is stunned
/// and struck; walked out of it in time, it rings on nobody. He stands with it raised meanwhile.
#[test]
fn a_ground_with_a_delay_is_seen_before_it_bites() {
    let cat = jane_data::catalog();
    let bell = spell("hand_bell");
    let delay = cat.combat.spell(bell).ground.unwrap().delay;
    assert!(delay.0 >= 30, "long enough to see: {delay:?}");
    for stand in [true, false] {
        let mut s = field();
        let her = me(&s);
        let hm = spawn(&mut s, "headmaster", 12, 10);
        edit(&mut s, hm, |u| {
            u.target = Some(her);
            u.combat = jane_sim::state::CombatState::Combat;
        });
        let hp = unit(&s, her).hp;
        let mut rung = None;
        for _ in 0..30 {
            steps(&mut s, 1);
            if let Some(g) = s.state().zone(Z).unwrap().grounds.iter().find(|g| g.spell == bell) {
                rung = Some((g.next_pulse, g.pos));
                break;
            }
        }
        let (at, pos) = rung.expect("he lifts the bell");
        assert_eq!(unit(&s, her).hp, hp, "nothing yet");
        assert_eq!(at.0 - s.state().tick.0, delay.0, "it rings a delay after it is lifted");
        let hm_at = unit(&s, hm).pos;
        if stand {
            steps(&mut s, delay.0 + 2);
            let u = unit(&s, her);
            assert!(u.hp < hp, "struck");
            assert!(u.statuses.iter().any(|st| st.effect == effect("stunned")), "and stood still");
        } else {
            walk(&mut s, delay.0 + 2, InputFrame { sprint: true, ..InputFrame::walk(Angle::WEST) });
            let u = unit(&s, her);
            assert!(u.pos.x.0 < pos.x.0 - 3 * 8 * 256, "out of it");
            assert_eq!(u.hp, hp, "it rang on nobody");
            assert!(!u.statuses.iter().any(|st| st.effect == effect("stunned")));
            assert_eq!(unit(&s, hm).pos, hm_at, "he stood with it raised");
        }
    }
}

/// coop.test.ts "what one learns they all know, including whoever is away and whoever comes
/// later": growth is the world's; the bar takes it where there is room.
#[test]
fn what_one_learns_they_all_know() {
    let mut s = party(2);
    cmd(&mut s, Some(1), Command::Leave);
    s.drain_events();
    learn(&mut s, "icebolt");
    let ice = spell("icebolt");
    assert!(s.state().growth.spells.contains(&ice));
    for p in &s.state().players {
        assert!(p.bar.contains(&Some(jane_data::BarSlot::Spell(ice))), "seat {:?}", p.seat);
    }
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::Learn(ice) && e.to.is_none() && e.in_zone.is_none()));
    learn(&mut s, "icebolt");
    assert!(events(&mut s).iter().all(|e| e.kind != EventKind::Learn(ice)), "once");
    cmd(&mut s, None, Command::Join { who: ClientToken(1) });
    let foe = spawn(&mut s, "skeleton", 16, 10);
    let guest = body_of(&s, 1);
    edit(&mut s, guest, |u| u.pos = Vec2::centre(10, 10));
    let host = me(&s);
    edit(&mut s, host, |u| u.pos = Vec2::centre(10, 30));
    cast(&mut s, 1, "icebolt", aim(Angle::EAST), None);
    steps(&mut s, 12);
    assert!(unit(&s, foe).hp < max_hp(unit(&s, foe)), "the guest casts what the host learned");
}

/// The console's combat rows: spawn beside her, set hp and mp.
#[test]
fn the_console_spawns_and_sets() {
    let mut s = field();
    let n = s.state().zone(Z).unwrap().units.len();
    cmd(&mut s, Some(0), Command::Dev(DevOp::Spawn(jane_data::catalog().combat.unit_id("rat").unwrap())));
    let zs = s.state().zone(Z).unwrap();
    assert_eq!(zs.units.len(), n + 1);
    assert_eq!(zs.units.last().unwrap().pos, Vec2::centre(13, 10));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Hp(5)));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Mp(0)));
    let u = unit(&s, me(&s));
    assert_eq!(u.hp, Milli(5000));
    assert_eq!(u.mp.0, i32::from(u.spirit), "a tick of mana since");
    cmd(&mut s, Some(0), Command::Dev(DevOp::Hp(-3)));
    assert_eq!(unit(&s, me(&s)).hp, Milli(1000), "the console never kills");
}

/// ARCHITECTURE.md §4.2 step 10: the flush is the only place a blow changes hp. Every write to
/// `hp` in the sim is in the flush, in `life.rs` (regen, standing up, waking, the console), or
/// where a unit is made or sits down whole.
#[test]
fn the_flush_is_the_only_hp_writer() {
    let allowed = ["flush.rs", "life.rs", "units.rs", "zone.rs", "seats.rs", "combat_tests.rs"];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut writers = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for (i, line) in text.lines().enumerate() {
            let l = line.trim_start();
            if l.starts_with("//") {
                continue;
            }
            let writes = ["hp =", "hp +=", "hp -=", "hp.0 =", "hp.0 +=", "hp.0 -="]
                .iter()
                .any(|w| l.contains(&format!(".{w}")) && !l.contains(&format!(".{w}=")));
            if writes {
                writers.push((name.clone(), i + 1));
            }
        }
    }
    let stray: Vec<_> = writers.iter().filter(|(f, _)| !allowed.contains(&f.as_str())).collect();
    assert!(stray.is_empty(), "hp written outside the flush and life: {stray:?}");
    assert!(writers.iter().any(|(f, _)| f == "flush.rs"));
    // The flush lowers hp; nothing else in the tick does.
    let lowers: Vec<_> = writers
        .iter()
        .filter(|(f, line)| {
            let text = std::fs::read_to_string(dir.join(f)).unwrap();
            let l = text.lines().nth(line - 1).unwrap();
            l.contains("hp.0 -") || l.contains("hp -=") || l.contains("hp.0 - ")
        })
        .collect();
    assert!(lowers.iter().all(|(f, _)| f == "flush.rs"), "{lowers:?}");
}

// --- determinism with the fight on the tape ------------------------------------------------------

/// A seeded tape with the fight on it: the host learns her spells, the console stands creatures
/// by her and now and then clears them, and she casts whatever she knows with a random aim and
/// assist profile, sometimes by the bar, while walking about. Guests come and go when asked.
struct FightTape {
    rng: Sfc32,
    held: [InputFrame; 4],
    cmds: Vec<StampedCommand>,
    seq: u16,
    guests: bool,
}

const TAPE_SPELLS: [&str; 5] = ["icebolt", "fireball", "explosion", "spark", "melee_player"];
const TAPE_UNITS: [&str; 5] = ["skeleton", "rat", "bandit", "spider", "foreman"];

impl FightTape {
    fn new(seed: u32, guests: bool) -> Self {
        Self { rng: Sfc32::seeded(seed, 5), held: [InputFrame::IDLE; 4], cmds: Vec::new(), seq: 0, guests }
    }

    fn cmd(&mut self, seat: Option<u8>, cmd: Command) {
        self.seq = self.seq.wrapping_add(1);
        self.cmds.push(StampedCommand { seat: seat.map(Seat), seq: self.seq, cmd });
    }

    fn frame(&mut self, f: u32) -> StepInput<'_> {
        self.cmds.clear();
        for s in 0..4 {
            if self.rng.below(30) == 0 {
                let mag = if self.rng.below(2) == 0 { 0 } else { 60 + self.rng.below(68) as u8 };
                let assist = match self.rng.below(3) {
                    0 => AssistProfile::Off,
                    1 => AssistProfile::Pad,
                    _ => AssistProfile::Mouse,
                };
                // Mostly east-ish, where the console stands what it spawns; sometimes anywhere,
                // sometimes no aim at all.
                let aim = match self.rng.below(5) {
                    0 => None,
                    1 => Some(Angle(self.rng.next_u32() as u16)),
                    _ => Some(Angle::EAST.wrapping_add(self.rng.range(-4000, 4000))),
                };
                self.held[s] = InputFrame {
                    mv_dir: Angle(self.rng.next_u32() as u16),
                    mv_mag: mag,
                    aim,
                    sprint: self.rng.below(4) == 0,
                    use_held: false,
                    assist,
                };
            }
        }
        if f < TAPE_SPELLS.len() as u32 {
            let id = spell(TAPE_SPELLS[f as usize]);
            self.cmd(Some(0), Command::Dev(DevOp::Learn(id)));
        }
        if f == 6 && self.guests {
            self.cmd(Some(0), Command::Open(true));
        }
        if self.guests && self.rng.below(200) == 0 {
            let who = ClientToken(1 + u64::from(self.rng.below(3)));
            self.cmd(None, Command::Join { who });
        }
        if self.guests && self.rng.below(500) == 0 {
            let seat = 1 + self.rng.below(3) as u8;
            self.cmd(Some(seat), Command::Leave);
        }
        if self.rng.below(25) == 0 {
            let def = jane_data::catalog().combat.unit_id(TAPE_UNITS[self.rng.below(5) as usize]).unwrap();
            let seat = if self.guests { self.rng.below(2) as u8 } else { 0 };
            self.cmd(Some(seat), Command::Dev(DevOp::Spawn(def)));
        }
        if self.rng.below(700) == 0 {
            self.cmd(Some(0), Command::Dev(DevOp::Kill));
        }
        for seat in 0..4u8 {
            if self.rng.below(4) == 0 {
                let c = if self.rng.below(4) == 0 {
                    Command::Bar { slot: self.rng.below(3) as u8, on: None }
                } else {
                    Command::Cast { spell: spell(TAPE_SPELLS[self.rng.below(5) as usize]), on: None }
                };
                self.cmd(Some(seat), c);
            }
        }
        if self.rng.below(400) == 0 {
            let hp = 1 + self.rng.below(150) as i32;
            self.cmd(Some(0), Command::Dev(DevOp::Hp(hp)));
        }
        // A full well now and then, so the bolts keep coming.
        if self.rng.below(40) == 0 {
            let seat = self.rng.below(4) as u8;
            self.cmd(Some(seat), Command::Dev(DevOp::Mp(150)));
        }
        self.cmds.sort_by_key(|c| (c.seat, c.seq));
        StepInput { frames: self.held, commands: &self.cmds }
    }
}

fn run(s: &mut Sim, t: &mut FightTape, from: u32, to: u32) {
    for f in from..to {
        let input = t.frame(f);
        s.step(&input);
    }
}

/// What a run of the tape did, so a determinism test can say the fight happened.
#[derive(Default, Debug)]
struct Seen {
    damage: u32,
    deaths: u32,
    casts: u32,
    statuses: u32,
}

impl Seen {
    fn add(&mut self, ev: &[jane_sim::Event]) {
        for e in ev {
            match e.kind {
                EventKind::Damage { .. } => self.damage += 1,
                EventKind::Death { .. } => self.deaths += 1,
                EventKind::Cast { .. } => self.casts += 1,
                EventKind::Status { on: true, .. } => self.statuses += 1,
                _ => {}
            }
        }
    }
}

/// §8 `same_tape_same_hash`, with the fight on the tape: two sims agree every 120 frames.
#[test]
fn same_tape_same_hash_with_casts() {
    let mut a = common::new_game();
    let mut b = common::new_game();
    let (mut ta, mut tb) = (FightTape::new(3, true), FightTape::new(3, true));
    let mut seen = Seen::default();
    for chunk in 0..50 {
        run(&mut a, &mut ta, chunk * 120, (chunk + 1) * 120);
        run(&mut b, &mut tb, chunk * 120, (chunk + 1) * 120);
        seen.add(a.drain_events());
        b.drain_events();
        assert_eq!(a.hash(), b.hash(), "frame {}", (chunk + 1) * 120);
    }
    assert_eq!(a.state(), b.state());
    assert!(seen.casts > 100 && seen.damage > 100 && seen.deaths > 5 && seen.statuses > 10, "{seen:?}");
}

/// §8 `save_load_continue`, mid-fight: saved at frames 131 and 257 of 520, and at the first
/// frames with a bolt in the air and with a status running, loaded, continued; equal to the
/// run that never saved.
#[test]
fn save_load_continue_mid_fight() {
    let mut straight = common::new_game();
    let mut ts = FightTape::new(8, false);
    let mut seen = Seen::default();
    let (mut flying, mut burning) = (None, None);
    for f in 0..520 {
        run(&mut straight, &mut ts, f, f + 1);
        seen.add(straight.drain_events());
        let zs = straight.state().zone(ZoneId::County).unwrap();
        if flying.is_none() && f > 20 && !zs.projectiles.is_empty() {
            flying = Some(f + 1);
        }
        if burning.is_none() && zs.units.iter().any(|u| u.alive && !u.statuses.is_empty()) {
            burning = Some(f + 1);
        }
    }
    assert!(seen.damage > 0 && seen.casts > 5, "{seen:?}");
    let flying = flying.expect("a bolt in the air");
    let burning = burning.expect("a status running");
    for at in [131, 257, flying, burning] {
        let mut first = common::new_game();
        let mut t = FightTape::new(8, false);
        run(&mut first, &mut t, 0, at);
        let bytes = first.save();
        let mut resumed = Sim::from_save_with(&bytes, common::bps()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash(), "loading changes nothing (frame {at})");
        run(&mut resumed, &mut t, at, 520);
        assert_eq!(resumed.hash(), straight.hash(), "saved at {at}");
    }
}

/// §8 `runtime_rebuild_is_invisible`, with the fight on the tape.
#[test]
fn runtime_rebuild_is_invisible_mid_fight() {
    let mut a = common::new_game();
    let mut b = common::new_game();
    let (mut ta, mut tb) = (FightTape::new(12, true), FightTape::new(12, true));
    for f in 0..1500 {
        run(&mut a, &mut ta, f, f + 1);
        if f % 89 == 7 {
            b.rebuild_runtimes();
        }
        run(&mut b, &mut tb, f, f + 1);
        if f % 60 == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(a.state(), b.state());
}

// --- budget -------------------------------------------------------------------------------------

/// budget.test.ts "3000 units asleep across the county": they stay asleep and off the grid while
/// she walks about at home, and the one in seven that is a corpse on a 300-tick clock is up
/// again on schedule, for a pop off `sleeping_due` instead of a pass over every corpse.
#[test]
fn three_thousand_sleepers_stand_up_on_schedule() {
    const KEEP_CLEAR: i32 = 1200 * 256;
    let mut s = common::new_game();
    let cat = jane_data::catalog();
    let defs: Vec<_> =
        ["skeleton", "rat", "bat", "bandit", "spider"].iter().map(|d| cat.combat.unit_id(d).unwrap()).collect();
    let home = unit(&s, me(&s)).pos;
    let (w, h) = {
        let rt = s.runtime(ZoneId::County).unwrap();
        (rt.grid.w() as i32, rt.grid.h() as i32)
    };
    let mut extra = Vec::new();
    let mut corpses = Vec::new();
    {
        let st = s.state_mut();
        let mut i = 0;
        'fill: for cy in (2..h - 4).step_by(3) {
            for cx in (2..w - 6).step_by(3) {
                let at = Vec2::centre(cx, cy);
                if (at.x.0 - home.x.0).abs() < KEEP_CLEAR && (at.y.0 - home.y.0).abs() < KEEP_CLEAR {
                    continue;
                }
                i += 1;
                if i % 11 >= 3 {
                    continue;
                }
                if extra.len() >= 3000 {
                    break 'fill;
                }
                let id = st.next.unit();
                let mut u = jane_sim::units::new_unit(
                    id,
                    None,
                    defs[extra.len() % defs.len()],
                    at,
                    jane_core::action::Facing::South,
                    st.tick,
                );
                u.awake = false;
                let zs = st.zone_mut(ZoneId::County).unwrap();
                if extra.len() % 7 == 0 {
                    u.alive = false;
                    u.hp = Milli::ZERO;
                    zs.sleeping_due.push((Tick(300), id));
                    corpses.push(id);
                }
                zs.insert_unit(u);
                extra.push(id);
            }
        }
    }
    s.rebuild_runtimes();
    assert_eq!(extra.len(), 3000);
    assert!(corpses.len() > 400);
    let x0 = home;
    let mut far = 0;
    for t in 0..600u32 {
        let dir = [Angle::EAST, Angle::SOUTH, Angle::WEST, Angle::NORTH][(t / 100 % 4) as usize];
        s.step(&StepInput::solo(InputFrame { sprint: t % 200 < 100, ..InputFrame::walk(dir) }));
        let p = unit(&s, me(&s)).pos;
        far = far.max((p.x.0 - x0.x.0).abs() + (p.y.0 - x0.y.0).abs());
    }
    assert!(far > 64 * 256, "she walked");
    let zs = s.state().zone(ZoneId::County).unwrap();
    assert!(extra.iter().all(|&id| !zs.unit(id).unwrap().awake), "the far side of the county slept");
    assert!(corpses.iter().all(|&id| zs.unit(id).unwrap().alive), "every corpse on a 300-tick clock is up again");
    assert!(zs.sleeping_due.is_empty());
    let rt = s.runtime(ZoneId::County).unwrap();
    for &id in &extra {
        let (x, y) = zs.unit(id).unwrap().pos.cell();
        assert_eq!(rt.grid.occupants(x, y), 0, "sleepers stand nowhere");
        assert!(!rt.is_in(id));
    }
}

/// Tick timings with sixty awake units fighting her (release: `cargo test --release -p jane-sim
/// --test combat -- --ignored --nocapture`). The AI is not in yet, so the creatures' blows are
/// dealt through the queue on a seeded schedule: every one of them strikes her about every
/// 45 ticks and burns, she casts whatever is ready along a turning aim with the pad profile,
/// and nobody dies (she is god, they are strong).
#[test]
#[ignore = "a timing, not a rule: run in release with --ignored --nocapture"]
#[allow(clippy::disallowed_types)]
fn sixty_awake_units_fighting_timing() {
    let mut s = field();
    for id in ["icebolt", "fireball", "explosion", "spark"] {
        learn(&mut s, id);
    }
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let mut foes = Vec::new();
    for i in 0..60 {
        let a = Angle((i * 65_536 / 60) as u16);
        let r = 4 + (i % 5);
        let d = jane_core::angle::along(a, jane_core::Fx::from_cells(r));
        let (x, y) = (10 + d.x.0 / 2048, 10 + d.y.0 / 2048);
        let id = spawn(&mut s, "skeleton", x.clamp(1, 60), y.clamp(1, 30));
        strong(&mut s, id, 60_000);
        put_status(&mut s, id, "poisoned", 1_000_000, None);
        foes.push(id);
    }
    let mut rng = Sfc32::seeded(1, 1);
    let body = me(&s);
    let mut times = Vec::with_capacity(4000);
    let mut dealt = 0u32;
    for t in 0..4000u32 {
        for &f in &foes {
            if rng.below(45) == 0 {
                s.queue_hit(
                    Z,
                    Hit {
                        to: body,
                        amount: Milli(5000),
                        school: School::Physical,
                        from: Some(f),
                        crit: false,
                        status: None,
                    },
                );
                s.queue_hit(
                    Z,
                    Hit {
                        to: f,
                        amount: Milli(3000),
                        school: School::Fire,
                        from: Some(body),
                        crit: false,
                        status: jane_data::catalog().combat.effect_id("burning"),
                    },
                );
            }
        }
        let frame = InputFrame { aim: Some(Angle((t * 331) as u16)), assist: AssistProfile::Pad, ..InputFrame::IDLE };
        let c = [StampedCommand {
            seat: Some(Seat(0)),
            seq: 0,
            cmd: Command::Cast { spell: spell(TAPE_SPELLS[(t % 4) as usize]), on: None },
        }];
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = frame;
        let input = StepInput { frames, commands: if t % 3 == 0 { &c } else { &[] } };
        let a = std::time::Instant::now();
        s.step(&input);
        times.push(a.elapsed().as_nanos() as u64);
        dealt += s.drain_events().iter().filter(|e| matches!(e.kind, EventKind::Damage { .. })).count() as u32;
    }
    let awake = s.runtime(Z).unwrap().awake_units.len();
    times.sort();
    let us = |ns: u64| format!("{}.{} us", ns / 1000, ns % 1000 / 100);
    let pct = |p: usize| us(times[times.len() * p / 100]);
    println!(
        "60 awake units fighting: {awake} awake, {dealt} blows landed in 4000 ticks; median {}, p90 {}, p99 {}, worst {}",
        pct(50),
        pct(90),
        pct(99),
        us(*times.last().unwrap())
    );
    assert!(awake >= 61);
}
