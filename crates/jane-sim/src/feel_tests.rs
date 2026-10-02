//! The fight's feel, the foes' side (`feel.rs`; PLAY-PLAN.md §2.1): wind-ups she can step out of,
//! the table's delay on them, interrupts each way, her hop and its i-frames, hitlag's bounds,
//! knockback, and the shorter boss fights.

use std::sync::Arc;

use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Angle, Blueprint, Cell, Key, Milli, SpellId, Tick, Tile, Vec2, ZoneId};

use crate::blueprints::Blueprints;
use crate::combat::{Hit, try_cast};
use crate::ctx::{Ctx, PartySnap};
use crate::feel::{cast_or_windup, lagged};
use crate::ids::{Seat, UnitId};
use crate::input::{Command, InputFrame, StampedCommand, StepInput};
use crate::sim::Sim;
use crate::state::{CombatState, Unit};
use crate::status::apply_effect;
use crate::tuning::{HOP_ENERGY, HOP_EVERY, HOP_FX, HOP_IFRAMES};
use crate::units::{max_hp, new_unit};

const Z: ZoneId = ZoneId::County;

/// Every zone a 128 x 64 field of grass; the county's `start` at (10, 10) facing east.
fn field() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let mut bp = Blueprint::new(ZoneId::ALL[i], 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
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

/// A foe of row `def` on cell (x, y), awake and in a fight with her.
fn foe(s: &mut Sim, def: &str, x: i32, y: i32) -> UnitId {
    let d = jane_data::catalog().combat.unit_id(def).unwrap();
    let id = s.state.next.unit();
    let mut u = new_unit(id, None, d, Vec2::centre(x, y), Facing::West, s.state.tick);
    u.awake = true;
    u.target = Some(me(s));
    u.combat = CombatState::Combat;
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
        s.step(&StepInput::IDLE);
    }
}

fn cmd(s: &mut Sim, seat: Option<u8>, c: Command, frame: InputFrame) {
    let cmds = [StampedCommand { seat: seat.map(Seat), seq: 0, cmd: c }];
    let mut frames = StepInput::IDLE.frames;
    frames[0] = frame;
    s.step(&StepInput { frames, commands: &cmds });
}

/// Step until `id` is winding up (at most `n` steps); its wind-up's length in ticks.
fn until_windup(s: &mut Sim, id: UnitId, n: u32) -> u32 {
    for _ in 0..n {
        if let Some(w) = unit(s, id).feel.windup {
            return w.lands.0 - w.began.0;
        }
        step(s, 1);
    }
    panic!("no wind-up in {n} steps");
}

fn place(s: &mut Sim, id: UnitId, x: i32, y: i32) {
    unit_mut(s, id).pos = Vec2::centre(x, y);
    s.rebuild_runtimes();
}

fn status(id: &str) -> jane_core::EffectId {
    jane_data::catalog().combat.effect_id(id).unwrap()
}

// --- wind-ups ---------------------------------------------------------------------------------------

#[test]
fn a_skeletons_blow_winds_up_half_a_second_and_misses_her_if_she_steps_out() {
    for stays in [true, false] {
        let mut s = field();
        let her = me(&s);
        let sk = foe(&mut s, "skeleton", 11, 10);
        assert_eq!(until_windup(&mut s, sk, 10), 30, "a skeleton's blow is 0.5 s");
        let hp = unit(&s, her).hp;
        if !stays {
            place(&mut s, her, 14, 10);
        }
        step(&mut s, 32);
        let w = unit(&s, sk);
        assert!(w.feel.windup.is_none(), "it landed");
        assert!(w.cooldowns.iter().any(|&(id, _)| id == spell("melee")), "landed or whiffed, the blow is spent");
        if stays {
            assert!(unit(&s, her).hp < hp, "stood in its reach, she is hit");
        } else {
            assert_eq!(unit(&s, her).hp, hp, "stepped out, it swings at the air");
        }
    }
}

#[test]
fn a_rat_winds_up_a_third_of_a_second_and_the_cactus_bristles_a_whole_one() {
    let mut s = field();
    let rat = foe(&mut s, "rat", 11, 10);
    assert_eq!(until_windup(&mut s, rat, 10), 21);
    let mut s = field();
    let c = foe(&mut s, "cactus", 18, 10);
    assert_eq!(until_windup(&mut s, c, 10), 60);
}

#[test]
fn a_melee_wind_up_lands_only_inside_its_arc() {
    let mut s = field();
    let her = me(&s);
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    let hp = unit(&s, her).hp;
    // Round behind it, still touching: out of the arc it began on.
    place(&mut s, her, 12, 10);
    step(&mut s, 32);
    assert_eq!(unit(&s, her).hp, hp, "behind it, the blow goes where she was");
}

#[test]
fn a_bolt_flies_at_the_point_locked_when_it_began() {
    let mut s = field();
    let her = me(&s);
    let c = foe(&mut s, "cactus", 18, 10);
    until_windup(&mut s, c, 10);
    let w = unit(&s, c).feel.windup.unwrap();
    assert_eq!(w.point, unit(&s, her).pos);
    // She walks off the line; the needles go where she stood.
    let hp = unit(&s, her).hp;
    place(&mut s, her, 10, 16);
    step(&mut s, 120);
    assert_eq!(unit(&s, her).hp, hp, "off the line it was aimed on, nothing finds her");
}

#[test]
fn at_a_lan_table_every_wind_up_is_d_ticks_longer() {
    let mut s = field();
    cmd(&mut s, None, Command::Table { delay: 3 }, InputFrame::IDLE);
    assert_eq!(s.state.table_delay, 3);
    let sk = foe(&mut s, "skeleton", 11, 10);
    assert_eq!(until_windup(&mut s, sk, 10), 33, "0.5 s and the table's three frames");
    // A save loads alone: no delay.
    let bytes = s.save();
    let back = Sim::from_save_with(&bytes, s.blueprints().clone()).unwrap();
    assert_eq!(back.state.table_delay, 0);
}

#[test]
fn the_hand_bells_pool_is_committed_and_a_stun_takes_it_up() {
    let mut s = field();
    let hm = foe(&mut s, "headmaster", 11, 10);
    // Its book's first spell is the bell; cast it as the AI would.
    let r = in_ctx(&mut s, None, |cx| cast_or_windup(cx, hm, spell("hand_bell")));
    assert!(r.is_ok(), "{r:?}");
    let w = unit(&s, hm).feel.windup.expect("winding up");
    assert!(w.pool.is_some() && s.state.zone(Z).unwrap().grounds.len() == 1, "the pool is laid at once: the tell");
    in_ctx(&mut s, None, |cx| apply_effect(cx, hm, status("stunned"), None));
    assert!(unit(&s, hm).feel.windup.is_none(), "stunned, the bell is not rung");
    assert!(s.state.zone(Z).unwrap().grounds.is_empty(), "and its pool goes with it");
}

// --- interrupts -----------------------------------------------------------------------------------

#[test]
fn a_stun_breaks_an_interruptible_wind_up_and_not_a_boss_tell_drawn_steady() {
    let mut s = field();
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    in_ctx(&mut s, None, |cx| apply_effect(cx, sk, status("stunned"), None));
    let u = unit(&s, sk);
    assert!(u.feel.windup.is_none(), "broken");
    assert!(u.cooldowns.iter().any(|&(id, _)| id == spell("melee")), "a broken blow costs its cooldown");
    // Goldskin's ring is drawn uninterruptible.
    let mut s = field();
    let g = foe(&mut s, "goldskin", 20, 10);
    let r = in_ctx(&mut s, None, |cx| cast_or_windup(cx, g, spell("gold_ring")));
    assert!(r.is_ok(), "{r:?}");
    assert!(!unit(&s, g).feel.windup.unwrap().interruptible);
    in_ctx(&mut s, None, |cx| apply_effect(cx, g, status("stunned"), None));
    assert!(unit(&s, g).feel.windup.is_some(), "a steady tell is not broken by a stun");
}

#[test]
fn a_jolt_breaks_a_machines_wind_up_and_takes_nothing_else() {
    let mut s = field();
    // The Foreman sees only by lamplight: his blow is cast as his fight would cast it.
    let f = foe(&mut s, "foreman", 11, 10);
    let r = in_ctx(&mut s, None, |cx| cast_or_windup(cx, f, spell("melee_stun")));
    assert!(r.is_ok(), "{r:?}");
    unit_mut(&mut s, f).feel.windup.as_mut().unwrap().interruptible = false;
    in_ctx(&mut s, None, |cx| apply_effect(cx, f, status("jolted"), None));
    assert!(unit(&s, f).feel.windup.is_none(), "Spark stops a machine, steady tell or not");
    let mut s = field();
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    in_ctx(&mut s, None, |cx| apply_effect(cx, sk, status("jolted"), None));
    assert!(unit(&s, sk).feel.windup.is_some(), "it does not stop anything else");
}

#[test]
fn a_chill_draws_a_wind_up_out_by_half_once() {
    let mut s = field();
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    for _ in 0..2 {
        in_ctx(&mut s, None, |cx| apply_effect(cx, sk, status("chilled"), None));
    }
    let w = unit(&s, sk).feel.windup.unwrap();
    assert_eq!(w.lands.0 - w.began.0, 45, "0.5 s, and half again");
}

#[test]
fn her_swing_knocks_a_small_foe_out_of_its_wind_up_and_a_middling_one_back() {
    let mut s = field();
    let her = me(&s);
    let rat = foe(&mut s, "rat", 11, 10);
    until_windup(&mut s, rat, 10);
    let before = unit(&s, rat).pos;
    in_ctx(&mut s, Some(Seat(0)), |cx| try_cast(cx, her, spell("melee_player"), None, None)).unwrap();
    assert!(unit(&s, rat).feel.windup.is_none(), "a small foe's wind-up breaks");
    step(&mut s, 12);
    let moved = unit(&s, rat).pos.x.0 - before.x.0;
    assert_eq!(moved, 6 * 256, "pushed 6 px, along the blow");
    // A skeleton is pushed but keeps its blow coming.
    let mut s = field();
    let her = me(&s);
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    in_ctx(&mut s, Some(Seat(0)), |cx| try_cast(cx, her, spell("melee_player"), None, None)).unwrap();
    assert!(unit(&s, sk).feel.windup.is_some());
    assert!(unit(&s, sk).feel.knock.is_some());
}

#[test]
fn bosses_plated_and_rooted_things_are_not_pushed_and_heavy_ones_push_her() {
    let mut s = field();
    let her = me(&s);
    for def in ["goldskin", "armour", "cactus"] {
        let id = foe(&mut s, def, 11, 10);
        unit_mut(&mut s, id).feel.windup = None;
        in_ctx(&mut s, Some(Seat(0)), |cx| {
            crate::feel::melee_landed(cx, her, id, jane_data::catalog().combat.spell(spell("melee_player")), false);
        });
        assert!(unit(&s, id).feel.knock.is_none(), "{def} is not pushed");
        unit_mut(&mut s, id).alive = false;
        s.rebuild_runtimes();
    }
    let h = foe(&mut s, "hauler", 11, 12);
    in_ctx(&mut s, None, |cx| {
        crate::feel::melee_landed(cx, h, her, jane_data::catalog().combat.spell(spell("melee_stun")), false);
    });
    assert!(unit(&s, her).feel.knock.is_some(), "a hauler's blow pushes her");
}

// --- the hop --------------------------------------------------------------------------------------

#[test]
fn her_hop_goes_a_metre_and_a_half_passes_blows_on_ticks_1_to_7_and_waits_18() {
    let mut s = field();
    let her = me(&s);
    let at = unit(&s, her).pos;
    let energy = unit(&s, her).energy;
    unit_mut(&mut s, her).stop_until = Tick(s.state.tick.0 + 100);
    let hp = unit(&s, her).hp;
    // A blow on each of her hop's ticks (the step the hop is pressed in is its first): the
    // first seven pass through her.
    let blow = Hit {
        to: her,
        amount: Milli(1000),
        school: jane_core::action::School::Physical,
        from: None,
        crit: false,
        status: None,
    };
    s.queue_hit(Z, blow);
    cmd(&mut s, Some(0), Command::Hop, InputFrame::IDLE);
    let u = unit(&s, her);
    assert!(u.feel.hop.is_some(), "hopping");
    assert!(u.stop_until <= s.state.tick, "a hop ends her recovery");
    assert!(u.energy.0 <= energy.0 - HOP_ENERGY.0 + 1000, "it costs 30 energy");
    let began = u.feel.hop.unwrap().began;
    let mut hits = vec![unit(&s, her).hp];
    for k in 2..=10 {
        s.queue_hit(Z, blow);
        step(&mut s, 1);
        assert_eq!(s.state.tick.0 - began.0, k);
        hits.push(unit(&s, her).hp);
    }
    let taken = hits.iter().filter(|&&h| h < hp).count();
    assert!(hits[..HOP_IFRAMES as usize].iter().all(|&h| h == hp), "i-frames on ticks 1 to 7: {hits:?}");
    assert_eq!(taken, 3, "ticks 8 to 10 are hers again: {hits:?}");
    let gone = unit(&s, her).pos.x.0 - at.x.0;
    assert!((gone - HOP_FX).abs() <= 16, "about 1.5 m along her facing: {gone}");
    // Not again until 18 ticks from its start.
    let ready = unit(&s, her).feel.hop_ready;
    assert_eq!(ready.0 - began.0, HOP_EVERY.0);
    cmd(&mut s, Some(0), Command::Hop, InputFrame::IDLE);
    assert!(unit(&s, her).feel.hop.is_none(), "too soon");
    let wait = ready.0.saturating_sub(s.state.tick.0);
    step(&mut s, wait);
    cmd(&mut s, Some(0), Command::Hop, InputFrame::walk(Angle::SOUTH));
    let h = unit(&s, her).feel.hop.expect("ready again");
    assert_eq!(h.dir, Angle::SOUTH, "along the stick");
}

#[test]
fn a_hop_steps_out_of_a_wind_up_it_could_not_walk_out_of() {
    let mut s = field();
    let her = me(&s);
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    // Rooted by her own recovery, she hops on the blow's last ticks.
    let lands = unit(&s, sk).feel.windup.unwrap().lands;
    unit_mut(&mut s, her).stop_until = Tick(lands.0 + 30);
    let wait = lands.0 - s.state.tick.0 - 3;
    step(&mut s, wait);
    let hp = unit(&s, her).hp;
    cmd(&mut s, Some(0), Command::Hop, InputFrame::walk(Angle::WEST));
    step(&mut s, 10);
    assert_eq!(unit(&s, her).hp, hp, "the blow passed through her hop");
}

// --- hitlag ---------------------------------------------------------------------------------------

#[test]
fn hitlag_freezes_her_and_its_victim_four_ticks_never_past_a_tenth_of_her_swing_nor_mid_hop() {
    let mut s = field();
    let her = me(&s);
    let sk = foe(&mut s, "skeleton", 11, 10);
    unit_mut(&mut s, sk).combat = CombatState::Idle;
    let now = s.state.tick;
    in_ctx(&mut s, Some(Seat(0)), |cx| try_cast(cx, her, spell("melee_player"), None, None)).unwrap();
    let (a, b) = (unit(&s, her).feel.lag_until.0 - now.0, unit(&s, sk).feel.lag_until.0 - now.0);
    assert!(a == 4 || a == 6, "4, or 6 on a crit: {a}");
    assert_eq!(a, b, "both freeze alike");
    let cycle = jane_data::catalog().combat.spell(spell("melee_player")).cooldown.0;
    assert!(a * 10 <= cycle, "never past a tenth of her swing's cycle");
    assert!(lagged(unit(&s, her), now));
    // Never stacked: a second blow at once keeps the longer.
    in_ctx(&mut s, Some(Seat(0)), |cx| crate::feel::lag(cx, sk, 2));
    assert_eq!(unit(&s, sk).feel.lag_until.0 - now.0, b);
    // Mid-hop, none.
    let mut s = field();
    let her = me(&s);
    let sk = foe(&mut s, "skeleton", 11, 10);
    cmd(&mut s, Some(0), Command::Hop, InputFrame::walk(Angle::EAST));
    let now = s.state.tick;
    in_ctx(&mut s, Some(Seat(0)), |cx| crate::feel::lag(cx, her, 4));
    assert!(unit(&s, her).feel.lag_until <= now, "no hitlag mid-hop");
    let _ = sk;
}

#[test]
fn a_boss_freezes_ten_ticks_alone_when_it_changes_phase() {
    let mut s = field();
    let f = foe(&mut s, "great_flower", 20, 10);
    let full = max_hp(unit(&s, f));
    let hit = Hit {
        to: f,
        amount: Milli(full.0 * 3 / 10),
        school: jane_core::action::School::Fire,
        from: None,
        crit: false,
        status: None,
    };
    s.queue_hit(Z, hit);
    step(&mut s, 1);
    let u = unit(&s, f);
    assert_eq!(u.phase, 1);
    assert_eq!(u.feel.lag_until.0 - s.state.tick.0, 10);
    assert!(!lagged(unit(&s, me(&s)), s.state.tick), "she is not frozen by its phase");
}

// --- the boss fights --------------------------------------------------------------------------------

#[test]
fn the_long_boss_fights_are_cut_to_twenty_to_forty_of_her_blows() {
    // Her blows at mid-game (strength and spirit 80): a bolt or a swing, about 60 to 90 points
    // after resists. The fights the audit timed at 60 to 125 events are now 20 to 40.
    let cat = jane_data::catalog();
    for (def, most) in [
        ("great_flower", 1500),
        ("burial_snake", 1500),
        ("spider_queen", 1700),
        ("the_soldier", 1500),
        ("goldskin", 1400),
        ("attendant", 1500),
        ("caretaker", 1300),
        ("ringer", 1100),
    ] {
        let d = cat.combat.unit_id(def).unwrap();
        let u = new_unit(UnitId::new(1).unwrap(), None, d, Vec2::centre(1, 1), Facing::South, Tick::ZERO);
        let hp = max_hp(&u).points();
        assert!(hp <= most, "{def}: {hp} hp is a sponge again");
        assert!(cat.combat.unit(d).hp_scale.0 < 1000, "{def} is shortened");
    }
}

#[test]
fn a_wind_up_and_a_hop_save_and_load_into_the_same_world() {
    let mut s = field();
    let sk = foe(&mut s, "skeleton", 11, 10);
    until_windup(&mut s, sk, 10);
    cmd(&mut s, Some(0), Command::Hop, InputFrame::walk(Angle::WEST));
    let bytes = s.save();
    let mut back = Sim::from_snapshot_with(&bytes, s.blueprints().clone()).unwrap();
    assert_eq!(back.hash(), s.hash());
    for _ in 0..40 {
        s.step(&StepInput::IDLE);
        back.step(&StepInput::IDLE);
        assert_eq!(back.hash(), s.hash());
    }
}
