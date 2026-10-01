//! The cast pipeline where only a `Ctx` can reach: the queue before the flush, casts by an AI,
//! verbs run as lists, a spell row made for the test. Carries `jane/test/sim.test.ts` "cast
//! pipeline", `coop.test.ts` "healing a friend" and the verb parts of `verbs2.test.ts` E7 on a
//! field of grass standing in for every zone. The rest of combat is proven through commands in
//! `tests/combat.rs`.

use std::sync::Arc;

use jane_core::action::{Facing, Heal, School, Stat};
use jane_core::blueprint::Mark;
use jane_core::{Action, Angle, Blueprint, Cell, Fx, Key, Milli, Permille, Rect, SpellId, Tick, Tile, Vec2, ZoneId};
use jane_data::{CastAnim, SpellDef, SpellKind, SpellPower};

use crate::actions::{Subject, run_action};
use crate::blueprints::Blueprints;
use crate::combat::{Hit, try_cast, try_cast_with};
use crate::ctx::{Ctx, PartySnap};
use crate::event::{EventKind, SpellError};
use crate::ids::{ClientToken, Seat, UnitId};
use crate::sim::Sim;
use crate::state::{StatusInst, Unit};
use crate::status::{resist_factor, speed_factor};
use crate::tuning::PLAYER_GCD;
use crate::units::{max_hp, new_unit};

const Z: ZoneId = ZoneId::County;

/// Every zone a 128 x 64 field of grass; the county's `start` at (10, 10) facing east and a
/// rect `trap` over (20..24, 8..12).
fn field() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        if z == Z {
            let k = bp.local("trap");
            bp.rects.insert(k, Rect::new(20, 8, 4, 4));
        }
        Arc::new(bp)
    });
    Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane")
}

fn me(s: &Sim) -> UnitId {
    s.state.players[0].unit
}

fn unit(s: &Sim, id: UnitId) -> &Unit {
    s.state.zone(Z).unwrap().unit(id).unwrap()
}

fn unit_mut(s: &mut Sim, id: UnitId) -> &mut Unit {
    s.state.zone_mut(Z).unwrap().unit_mut(id).unwrap()
}

fn spawn(s: &mut Sim, def: &str, x: i32, y: i32) -> UnitId {
    let d = jane_data::catalog().combat.unit_id(def).unwrap();
    let id = s.state.next.unit();
    let mut u = new_unit(id, None, d, Vec2::centre(x, y), Facing::West, s.state.tick);
    u.awake = true;
    s.state.zone_mut(Z).unwrap().insert_unit(u);
    s.rebuild_runtimes();
    id
}

fn in_ctx<R>(s: &mut Sim, seat: Option<Seat>, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
    let snap = PartySnap::of(&s.state);
    s.with_ctx(Z, seat, &snap, false, f)
}

fn spell(id: &str) -> SpellId {
    jane_data::catalog().combat.spell_id(id).unwrap()
}

fn step(s: &mut Sim, n: u32) {
    for _ in 0..n {
        s.step(&crate::input::StepInput::IDLE);
    }
}

fn queued(s: &Sim) -> usize {
    s.scratch.hits[Z.index()].len()
}

/// sim.test.ts "queues damage and only changes hp at the flush".
#[test]
fn a_blow_waits_in_the_queue_and_only_the_flush_changes_hp() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 11, 10);
    let before = unit(&s, foe).hp;
    let r = in_ctx(&mut s, Some(Seat(0)), |cx| {
        let body = cx.actor_unit().unwrap();
        try_cast(cx, body, spell("melee_player"), Some(Angle::EAST), None)
    });
    assert_eq!(r, Ok(()));
    assert_eq!(unit(&s, foe).hp, before);
    assert_eq!(queued(&s), 1);
    step(&mut s, 1);
    assert!(unit(&s, foe).hp < before);
    assert_eq!(queued(&s), 0);
}

/// sim.test.ts "melee is forgiving: aimed the wrong way, it still finds the enemy in reach".
#[test]
fn melee_aimed_the_wrong_way_still_finds_the_enemy_in_reach() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 9, 10);
    let r = in_ctx(&mut s, Some(Seat(0)), |cx| {
        let body = cx.actor_unit().unwrap();
        try_cast(cx, body, spell("melee_player"), Some(Angle::EAST), None)
    });
    assert_eq!(r, Ok(()));
    assert_eq!(s.scratch.hits[Z.index()][0].to, foe);
    assert_eq!(unit(&s, me(&s)).facing, Facing::West, "she turns to face what she hit");
}

/// sim.test.ts "checks range before cooldown, so a cooling-down AI keeps walking" and
/// "survives a target that no longer exists".
#[test]
fn range_before_cooldown_and_a_target_that_is_gone() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 18, 10);
    let (body, melee) = (me(&s), spell("melee"));
    let now = s.state.tick;
    let u = unit_mut(&mut s, foe);
    u.target = Some(body);
    u.cooldowns.push((melee, now.after(Tick(100))));
    assert_eq!(in_ctx(&mut s, None, |cx| try_cast(cx, foe, melee, None, None)), Err(SpellError::TooFar));
    unit_mut(&mut s, foe).target = UnitId::new(987_654);
    assert_eq!(in_ctx(&mut s, None, |cx| try_cast(cx, foe, melee, None, None)), Err(SpellError::NoTarget));
    // In reach and cooling down: it stands still.
    let foe2 = spawn(&mut s, "skeleton", 11, 10);
    let u = unit_mut(&mut s, foe2);
    u.target = Some(body);
    u.cooldowns.push((melee, now.after(Tick(100))));
    assert_eq!(in_ctx(&mut s, None, |cx| try_cast(cx, foe2, melee, None, None)), Err(SpellError::OnCooldown));
}

/// sim.test.ts "charges nothing for a failed cast and starts the GCD on success".
#[test]
fn a_failed_cast_costs_nothing_and_a_good_one_starts_the_gcd() {
    let mut s = field();
    for id in ["icebolt", "repair"] {
        crate::combat::teach(&mut s.state, spell(id));
    }
    let body = me(&s);
    let mp = unit(&s, body).mp;
    let now = s.state.tick;
    let r = in_ctx(&mut s, Some(Seat(0)), |cx| try_cast(cx, body, spell("repair"), None, None));
    assert_eq!(r, Err(SpellError::CastUnsuccessful), "nothing here to repair");
    assert_eq!(unit(&s, body).gcd_until, Tick::ZERO);
    let bolt =
        |s: &mut Sim| in_ctx(s, Some(Seat(0)), |cx| try_cast(cx, body, spell("icebolt"), Some(Angle::EAST), None));
    assert_eq!(bolt(&mut s), Ok(()));
    assert_eq!(unit(&s, body).mp, Milli(mp.0 - 14_000));
    assert_eq!(unit(&s, body).gcd_until, now.after(PLAYER_GCD), "a seat's GCD is 1 s");
    assert_eq!(bolt(&mut s), Err(SpellError::OnCooldown));
    assert_eq!(s.state.zone(Z).unwrap().projectiles.len(), 1);
    assert_eq!(s.state.players[0].stats.casts, 1);
}

/// coop.test.ts "healing a friend": no heal spell is placed in the county yet, so the verb is
/// proven on a row made here.
#[test]
fn a_friendly_spell_lands_where_the_cursor_or_the_stick_says() {
    let mend = SpellDef {
        id: "mend",
        name: jane_core::TextId(0),
        description: jane_core::TextId(0),
        icon: jane_core::SpriteId(0),
        kind: SpellKind::Ally,
        school: School::Heal,
        mp: Milli::ZERO,
        energy: Milli::ZERO,
        range: Fx::from_metres(15),
        cooldown: Tick::ZERO,
        gcd_immune: true,
        needs_target: false,
        needs_enemy: false,
        needs_los: true,
        anim: CastAnim::Cast,
        power: Some(SpellPower { stat: Stat::Spirit, div: 2000, var_div: 8000, flat: Milli::ZERO }),
        speed: None,
        count: 1,
        fan: Angle(0),
        splash: None,
        effect: None,
        restore_energy: Milli::ZERO,
        ground: None,
        world: None,
        stop: Tick::ZERO,
        cast: Tick::ZERO,
        glow: None,
        touch: None,
    };
    let id = SpellId(u16::MAX);
    let party = || {
        let mut s = field();
        s.state.open = true;
        s.join(ClientToken(2)).unwrap();
        let (a, b) = (s.state.players[0].unit, s.state.players[1].unit);
        unit_mut(&mut s, a).hp = Milli(40_000);
        unit_mut(&mut s, b).hp = Milli(40_000);
        (s, a, b)
    };
    let cast = |s: &mut Sim, aim: Option<Angle>, on: Option<UnitId>| {
        let body = s.state.players[0].unit;
        let r = in_ctx(s, Some(Seat(0)), |cx| try_cast_with(cx, body, id, &mend, aim, on));
        step(s, 1);
        r
    };
    let toward = |s: &Sim, a: UnitId, b: UnitId| jane_core::angle::bearing(unit(s, a).pos, unit(s, b).pos);

    // A cursor over her: her; and the event says who cast it.
    let (mut s, a, b) = party();
    s.drain_events();
    let aim = toward(&s, a, b);
    assert_eq!(cast(&mut s, Some(aim), Some(b)), Ok(()));
    assert!(unit(&s, b).hp.0 > 40_000 && unit(&s, a).hp.0 == 40_000);
    assert!(
        s.drain_events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::Heal { unit, from, .. } if unit == b && from == Some(a)))
    );

    // Over nobody (her own body), over an enemy: herself, even with her friend dead ahead.
    let (mut s, a, b) = party();
    let foe = spawn(&mut s, "skeleton", 14, 14);
    let aim = toward(&s, a, b);
    for on in [a, foe] {
        unit_mut(&mut s, a).hp = Milli(40_000);
        assert_eq!(cast(&mut s, Some(aim), Some(on)), Ok(()));
        assert!(unit(&s, a).hp.0 > 40_000, "on {on:?}");
        assert_eq!(unit(&s, b).hp.0, 40_000);
    }

    // Pointing at a friend out of reach fails and says so; it does not fall on her instead.
    let (mut s, a, b) = party();
    unit_mut(&mut s, b).pos = Vec2::centre(100, 10);
    s.rebuild_runtimes();
    let r = cast(&mut s, None, Some(b));
    assert!(matches!(r, Err(SpellError::TooFar | SpellError::NotInLos)), "{r:?}");
    assert_eq!((unit(&s, a).hp.0, unit(&s, b).hp.0), (40_000, 40_000));

    // No cursor: the friend along the stick; the stick at rest, herself.
    let (mut s, a, b) = party();
    let aim = toward(&s, a, b);
    assert_eq!(cast(&mut s, Some(aim), None), Ok(()));
    assert!(unit(&s, b).hp.0 > 40_000 && unit(&s, a).hp.0 == 40_000);
    assert_eq!(cast(&mut s, None, None), Ok(()));
    assert!(unit(&s, a).hp.0 > 40_000);

    // A friend in another zone is not a target, whatever the client says.
    let (mut s, a, b) = party();
    s.state.players[1].travel = Some(crate::state::TravelRequest { zone: ZoneId::House, mark: s.start_sym, at: None });
    step(&mut s, 1);
    assert_eq!(cast(&mut s, None, Some(b)), Ok(()));
    assert!(unit(&s, a).hp.0 > 40_000);
    assert_eq!(s.state.zone(ZoneId::House).unwrap().unit(b).unwrap().hp.0, 40_000);
}

fn with_status(u: &mut Unit, effect: &str) {
    let e = jane_data::catalog().combat.effect_id(effect).unwrap();
    u.statuses.push(StatusInst {
        effect: e,
        until: Tick(10_000),
        next_pulse: Tick(10_000),
        from: None,
        pool: Milli::ZERO,
    });
}

/// verbs2.test.ts E7 "Spark jolts what is weak to shock ... softened takes resists away and
/// leaves weaknesses": the factors, on the Foreman (physical 0.7, shock -0.5).
#[test]
fn resists_and_softened_and_the_slows() {
    let mut s = field();
    let boss = spawn(&mut s, "foreman", 20, 20);
    let now = s.state.tick;
    let u = unit_mut(&mut s, boss);
    assert_eq!(resist_factor(u, School::Physical, now), 300);
    assert_eq!(resist_factor(u, School::Shock, now), 1500);
    with_status(u, "softened");
    assert_eq!(resist_factor(u, School::Physical, now), 1000);
    assert_eq!(resist_factor(u, School::Shock, now), 1500);
    with_status(u, "staggered");
    assert_eq!(resist_factor(u, School::Shock, now), 3000, "staggered doubles them too");
    let rat = spawn(&mut s, "rat", 22, 20);
    let u = unit_mut(&mut s, rat);
    with_status(u, "dusted");
    assert_eq!(speed_factor(u, now), 500);
    with_status(u, "dazzled");
    assert_eq!(speed_factor(u, now), 0);
    // Past its `until` a status counts for nothing, whether or not step 9 has taken it off yet.
    assert_eq!(speed_factor(u, Tick(10_000)), 1000);
}

/// The combat verbs as lists run them: `Strike` on a rect (from whoever pulled it, the party's
/// own spared), `Status` and `Heal` on the subject or else the actor.
#[test]
fn strike_status_and_heal_as_verbs() {
    let mut s = field();
    let inside = spawn(&mut s, "skeleton", 21, 9);
    let outside = spawn(&mut s, "skeleton", 26, 9);
    let hen = spawn(&mut s, "hen", 22, 10);
    let trap = Key::Local(0);
    let strike = Action::Strike {
        rect: trap,
        amount: Milli(5000),
        school: School::Fire,
        effect: jane_data::catalog().combat.effect_id("burning"),
        hits_friends: false,
    };
    let body = me(&s);
    in_ctx(&mut s, Some(Seat(0)), |cx| run_action(cx, &strike, Subject::Unit(body)));
    let hits: Vec<Hit> = s.scratch.hits[Z.index()].clone();
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].to, hits[0].from, hits[0].amount), (inside, Some(body), Milli(5000)));
    step(&mut s, 1);
    assert!(unit(&s, inside).hp < max_hp(unit(&s, inside)));
    assert_eq!(unit(&s, outside).hp, max_hp(unit(&s, outside)));
    assert_eq!(unit(&s, hen).hp, max_hp(unit(&s, hen)));
    assert!(!unit(&s, inside).statuses.is_empty(), "the strike's effect lands with it");

    // Status on the subject; Heal with no subject lands on the actor.
    let stoneskin = jane_data::catalog().combat.effect_id("stoneskin").unwrap();
    in_ctx(&mut s, Some(Seat(0)), |cx| run_action(cx, &Action::Status(stoneskin), Subject::Unit(outside)));
    assert!(unit(&s, outside).statuses.iter().any(|st| st.effect == stoneskin));
    unit_mut(&mut s, body).hp = Milli(10_000);
    in_ctx(&mut s, Some(Seat(0)), |cx| run_action(cx, &Action::Heal(Heal::Pct(Permille(250))), Subject::None));
    step(&mut s, 1);
    // A quarter of 150 is 37.5, rounded to 38.
    assert_eq!(unit(&s, body).hp, Milli(48_000));
}
