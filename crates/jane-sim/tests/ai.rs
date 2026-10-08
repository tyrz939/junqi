//! The controllers through the step (ARCHITECTURE.md §4.2 steps 3 and 7, §4.6.a, §8, §9).
//! Carries the AI parts of `jane/test/sim.test.ts` ("enemies chase a player who is standing
//! still", "checks range before cooldown, so a cooling-down AI keeps walking", "the dog keeps
//! Julie's hours"), `verbs2.test.ts` (E8 `sight: lit` and `shunsLight`, E11 a patrol point with
//! a dwell), `burial.test.ts` ("a shade will not step into warm light"), `factory.test.ts` ("a
//! sentry only notices what stands in a lamp's light"), `school.test.ts` (the Caretaker is
//! night only), `budget.test.ts` (3 000 sleepers and 8 000 props: the tick costs what is awake)
//! and the §8 gates with the AI on the tape (`same_tape_same_hash`, `save_load_continue`,
//! `runtime_rebuild_is_invisible` mid-chase, `awake_only_equals_everyone`). What only a `Ctx`
//! reaches (orders, the ecology rows on a made row, a schedule's mark) is `src/ai_tests.rs`.

mod common;
mod field;

use field::*;
use jane_core::num::{dist_sq, isqrt};
use jane_core::{Angle, Cell, Fx, Milli, Sfc32, SpellId, Tick, Vec2, ZoneId};
use jane_sim::ai::live_path;
use jane_sim::light::{lit_at, prop_centre};
use jane_sim::state::{CombatState, Patrol, Prop};
use jane_sim::units::max_hp;
use jane_sim::{ClientToken, Command, DevOp, InputFrame, PropId, Seat, Sim, StampedCommand, StepInput, Unit, UnitId};

// --- helpers ---------------------------------------------------------------------------------

/// A prop of `def` on a cell of the county, `on` or off.
fn put_prop(s: &mut Sim, def: &str, x: u16, y: u16, on: bool) -> PropId {
    let cat = jane_data::catalog();
    let d = cat.story.prop_id(def).unwrap();
    let st = s.state_mut();
    let id = st.next.prop();
    let key = st.syms.intern(&format!("test_{def}_{}", id.get()));
    st.zone_mut(Z).unwrap().props.push(Prop {
        id,
        key,
        def: d,
        spawn: None,
        cell: Cell::new(x, y),
        solid: cat.story.prop(d).solid,
        hidden: false,
        locked: false,
        used: false,
        on,
        under_done: false,
        more: jane_core::Rare::empty(),
    });
    s.rebuild_runtimes();
    id
}

fn prop(s: &Sim, id: PropId) -> &Prop {
    let zs = s.state().zone(Z).unwrap();
    &zs.props[zs.prop_ix(id).unwrap() as usize]
}

fn switch(s: &mut Sim, id: PropId, on: bool) {
    let zs = s.state_mut().zone_mut(Z).unwrap();
    let ix = zs.prop_ix(id).unwrap() as usize;
    zs.props[ix].on = on;
}

/// Is the point lit, as the sim sees it (`warm`: warm light only)?
fn lit(s: &Sim, at: Vec2, warm: bool) -> bool {
    lit_at(s.state().zone(Z).unwrap(), s.runtime(Z).unwrap(), s.state().clock, at, warm)
}

fn px(d: i64) -> i64 {
    d / 256
}

fn dist(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64))
}

fn place(s: &mut Sim, id: UnitId, x: i32, y: i32) {
    edit(s, id, |u| {
        u.pos = Vec2::centre(x, y);
        u.home = u.pos;
    });
}

fn remove(s: &mut Sim, id: UnitId) {
    s.state_mut().zone_mut(Z).unwrap().remove_unit(id);
    s.rebuild_runtimes();
}

fn god(s: &mut Sim, on: bool) {
    cmd(s, Some(0), Command::Dev(DevOp::God(on)));
}

fn hour(s: &mut Sim, h: u8) {
    cmd(s, Some(0), Command::Dev(DevOp::Time { hour: h }));
}

fn target(s: &Sim, id: UnitId) -> (CombatState, Option<UnitId>) {
    let u = unit(s, id);
    (u.combat, u.target)
}

// --- chasing ---------------------------------------------------------------------------------

/// sim.test.ts "enemies chase a player who is standing still, and hit them", after the bell.
#[test]
fn enemies_chase_a_player_standing_still_and_hit_her() {
    let mut s = field();
    hour(&mut s, 22);
    let foe = spawn(&mut s, "skeleton", 19, 10);
    let hp = unit(&s, me(&s)).hp;
    steps(&mut s, 60 * 12);
    assert_eq!(target(&s, foe), (CombatState::Combat, Some(me(&s))));
    assert!(unit(&s, me(&s)).hp < hp);
    assert!(px(dist(unit(&s, foe).pos, unit(&s, me(&s)).pos)) <= 16, "it came to her");
}

/// No hour is wary (the owner, 2026-09-29; PLAN.md §2.6 "Day"): at ten in the morning on the
/// county's gentlest ground (threat 1, as the phase table left its strength) a skeleton that sees
/// her within its aggro comes for her, and so does the same row at threat 2. A rabbit's row has
/// no aggro and leaves her be.
#[test]
fn by_day_the_gentlest_ground_comes_for_her_on_sight() {
    let mut s = field();
    hour(&mut s, 10);
    let her = me(&s);
    let foe = spawn(&mut s, "skeleton", 13, 10);
    let hp = unit(&s, her).hp;
    steps(&mut s, 20);
    assert_eq!(target(&s, foe), (CombatState::Combat, Some(her)), "it saw her by day and came");
    steps(&mut s, 60 * 4);
    assert!(unit(&s, her).hp < hp, "and it hit her");
    remove(&mut s, foe);

    let hard = spawn(&mut s, "skeleton", 13, 14);
    edit(&mut s, hard, |u| u.strength *= 2);
    steps(&mut s, 20);
    assert_eq!(target(&s, hard).1, Some(her));
    remove(&mut s, hard);

    let rabbit = spawn(&mut s, "rabbit", 12, 11);
    steps(&mut s, 60 * 2);
    assert_ne!(target(&s, rabbit).1, Some(her), "a rabbit is no hostile");
}

/// In a dungeon, the Gold Mine included, a skeleton comes for her on sight at noon (the owner's
/// playtests: "enemies don't aggro unless hit first"; WORLD.md §*hostiles come on sight*).
#[test]
fn a_skeleton_in_the_mine_comes_on_sight_by_day() {
    let mut s = field();
    hour(&mut s, 12);
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: ZoneId::Mine, mark: start_sym() }));
    steps(&mut s, 2);
    let her = me(&s);
    assert_eq!(zone_of(&s, her), ZoneId::Mine);
    let at = unit(&s, her).pos;
    let d = jane_data::catalog().combat.unit_id("skeleton").unwrap();
    let st = s.state_mut();
    let id = st.next.unit();
    let mut u = jane_sim::units::new_unit(
        id,
        None,
        d,
        Vec2::new(at.x + Fx::from_px(40), at.y),
        jane_core::action::Facing::West,
        st.tick,
    );
    u.awake = true;
    st.zone_mut(ZoneId::Mine).unwrap().insert_unit(u);
    s.rebuild_runtimes();
    steps(&mut s, 20);
    assert_eq!(target(&s, id), (CombatState::Combat, Some(her)), "it saw her and came");
}

/// sim.test.ts "checks range before cooldown, so a cooling-down AI keeps walking": `TooFar`
/// comes before `OnCooldown`, so a creature with its only blow cooling walks in anyway.
#[test]
fn a_cooling_down_creature_keeps_walking() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 18, 10);
    let her = me(&s);
    let melee = spell("melee");
    let now = s.state().tick;
    edit(&mut s, foe, |u| {
        u.target = Some(her);
        u.combat = CombatState::Combat;
        u.cooldowns.push((melee, now.after(Tick(400))));
    });
    let d0 = dist(unit(&s, foe).pos, unit(&s, her).pos);
    s.drain_events();
    steps(&mut s, 30);
    assert!(dist(unit(&s, foe).pos, unit(&s, her).pos) < d0 - Fx::from_px(10).0 as i64);
    assert!(!events(&mut s).iter().any(|e| matches!(e.kind, jane_sim::EventKind::Cast { unit, .. } if unit == foe)));
}

/// PLAN.md §2.6 *Leash* (2026-10-06, WoW's evade): a creature taken too far from its post lets
/// go, sheds what is on it, is whole again at once and runs home, and nothing she throws at it
/// lands until it is there; a home it cannot reach becomes where it stands.
#[test]
fn a_leash_takes_it_home_whole() {
    let mut s = field();
    let foe = spawn(&mut s, "skeleton", 14, 10);
    let her = me(&s);
    learn(&mut s, "spark");
    // Its post is far behind it: 87 m.
    edit(&mut s, foe, |u| {
        u.home = Vec2::centre(101, 10);
        u.hp = Milli(u.hp.0 / 2);
        u.target = Some(her);
        u.combat = CombatState::Combat;
    });
    steps(&mut s, 1);
    let f = unit(&s, foe);
    assert_eq!((f.combat, f.target), (CombatState::Evade, None));
    assert_eq!(f.hp, max_hp(f), "whole the tick it lets go");
    // Nothing lands on an evade: her blows, and the pull they would be.
    s.drain_events();
    for _ in 0..3 {
        cmd(&mut s, Some(0), Command::Dev(DevOp::Mp(100)));
        cmd(&mut s, Some(0), Command::Cast { spell: spell("spark"), on: Some(foe) });
        steps(&mut s, 30);
    }
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| matches!(e.kind, jane_sim::EventKind::Cast { unit, .. } if unit == her)), "she shot");
    assert!(!ev.iter().any(|e| matches!(e.kind, jane_sim::EventKind::Damage { unit, .. } if unit == foe)));
    let f = unit(&s, foe);
    assert_eq!(f.hp, max_hp(f));
    assert_ne!(f.combat, CombatState::Combat);
    steps(&mut s, 900);
    let f = unit(&s, foe);
    assert_eq!((f.combat, f.target), (CombatState::Idle, None));
    assert!(px(dist(f.pos, Vec2::centre(101, 10))) <= 2, "home: {:?}", f.pos);
    assert_eq!(f.hp, max_hp(f));
}

// --- light -----------------------------------------------------------------------------------

/// verbs2.test.ts E8 and factory.test.ts "a sentry only notices what stands in a lamp's light":
/// `sight: lit` (the Works' hauler). It does not see her in the dark, sees her in the light,
/// loses her when the lamp goes out and does not find her again in the dark; of two people it
/// takes the lit one, though the other is nearer.
#[test]
fn a_lit_sighted_creature_sees_only_what_stands_in_the_light() {
    let mut s = field();
    hour(&mut s, 22);
    let her = me(&s);
    let sentry = spawn(&mut s, "hauler", 17, 10);
    let plain = spawn(&mut s, "skeleton", 17, 13);
    steps(&mut s, 40);
    assert_eq!(target(&s, plain).1, Some(her), "an ordinary skeleton at the same distance has her at once");
    assert_eq!(target(&s, sentry), (CombatState::Idle, None));
    remove(&mut s, plain);
    let lamp = put_prop(&mut s, "brazier", 10, 9, true);
    assert!(lit(&s, unit(&s, her).pos, false));
    steps(&mut s, 12);
    assert_eq!(target(&s, sentry), (CombatState::Combat, Some(her)));
    switch(&mut s, lamp, false);
    steps(&mut s, 2);
    assert_eq!(target(&s, sentry).1, None);
    steps(&mut s, 40);
    assert_eq!(target(&s, sentry).1, None, "and it does not find her again in the dark");

    // Two of them: the nearer one in the dark, the further one under the lamp.
    cmd(&mut s, Some(0), Command::Open(true));
    cmd(&mut s, None, Command::Join { who: ClientToken(9) });
    let friend = body_of(&s, 1);
    place(&mut s, sentry, 17, 10);
    edit(&mut s, sentry, |u| {
        u.combat = CombatState::Idle;
        u.target = None;
    });
    place(&mut s, friend, 15, 14);
    place(&mut s, her, 10, 10);
    switch(&mut s, lamp, true);
    assert!(lit(&s, unit(&s, her).pos, false) && !lit(&s, unit(&s, friend).pos, false));
    let (df, dh) = (dist(unit(&s, sentry).pos, unit(&s, friend).pos), dist(unit(&s, sentry).pos, unit(&s, her).pos));
    assert!(df < dh);
    steps(&mut s, 12);
    assert_eq!(target(&s, sentry).1, Some(her));
}

/// verbs2.test.ts E8 "shunsLight" and burial.test.ts "a shade will not step into warm light":
/// it comes for her, stops at the edge of the warm light and waits there, re-planning every
/// 20 ticks and not every tick; a cold light does not hold it; caught in the light when a lamp
/// comes on, it does nothing but leave.
#[test]
fn a_shade_waits_at_the_edge_of_warm_light() {
    let mut s = field();
    hour(&mut s, 22);
    let her = me(&s);
    let lamp = put_prop(&mut s, "brazier", 10, 9, true);
    let shade = spawn(&mut s, "shade", 20, 10);
    let hp = unit(&s, her).hp;
    let centre = prop_centre(jane_data::catalog().story.prop(prop(&s, lamp).def), prop(&s, lamp));
    let mut searches = 0;
    for t in 0..600 {
        steps(&mut s, 1);
        assert!(!lit(&s, unit(&s, shade).pos, true), "tick {t}: it never stands in the light");
        if t == 300 {
            searches = s.path_stats().searches;
        }
    }
    assert_eq!(target(&s, shade).1, Some(her));
    assert_eq!(unit(&s, her).hp, hp);
    // As near as the dark lets it: just outside the 53 px an 80 px brazier lights.
    let from_lamp = px(dist(unit(&s, shade).pos, centre));
    assert!(from_lamp > 53 && from_lamp < 69, "{from_lamp} px");
    let waited = s.path_stats().searches - searches;
    assert!(waited < 25, "waiting costs a re-plan every 20 ticks, not a search a tick: {waited}");

    // A cold light does not hold it; nor does no light.
    switch(&mut s, lamp, false);
    put_prop(&mut s, "torch_blue", 11, 9, true);
    steps(&mut s, 300);
    assert!(unit(&s, her).hp < hp);

    // Caught in the light when a lamp comes on, it does nothing but leave.
    god(&mut s, true);
    switch(&mut s, lamp, true);
    let before = unit(&s, her).hp;
    steps(&mut s, 240);
    assert!(!lit(&s, unit(&s, shade).pos, true));
    assert_eq!(unit(&s, her).hp, before);
    assert_eq!(unit(&s, shade).combat, CombatState::Idle);
}

/// PLAN.md 2.6, `ai::aggro_reach`: after dark a creature out of the lamplight notices from
/// further off (aggro x 1.5, the owner 2026-10-06; it was 1.25); in warm light it is its daytime self.
#[test]
fn the_night_lengthens_its_reach_outside_the_light() {
    // A skeleton notices her at New Game at 7 m between bodies: 9 m centre to centre (10 m
    // between bodies at night, the cap: 12 m centre to centre). At 10 m, only at night.
    let at = |night: bool, lamp: bool| {
        let mut s = field();
        if night {
            hour(&mut s, 22);
        }
        let foe = spawn(&mut s, "skeleton", 20, 10);
        if lamp {
            put_prop(&mut s, "brazier", 20, 9, true);
        }
        steps(&mut s, 20);
        unit(&s, foe).combat
    };
    assert_eq!(at(false, false), CombatState::Idle);
    assert_eq!(at(true, false), CombatState::Combat);
    assert_eq!(at(true, true), CombatState::Idle);
}

// --- patrols, bait ----------------------------------------------------------------------------

/// verbs2.test.ts E11: it stands for its dwell at the point that has one and walks straight on
/// from the point that has none; a butterfly (an npc) does it too.
#[test]
fn a_patrol_stands_its_dwell_and_walks_on() {
    let mut s = field();
    god(&mut s, true); // nothing to chase: this is about the round
    for (def, row) in [("rat", 0), ("dog", 3)] {
        let id = spawn(&mut s, def, 22, 14 + row);
        let (a, b) = (Vec2::centre(22, 14 + row), Vec2::centre(28, 14 + row));
        edit(&mut s, id, |u| u.patrol = Some(Box::new(Patrol { points: vec![(a, Tick(0)), (b, Tick(120))] })));
        let (mut stood, mut longest, mut reached) = (0, 0, false);
        for _ in 0..1200 {
            let x = unit(&s, id).pos.x;
            steps(&mut s, 1);
            let u = unit(&s, id);
            reached |= dist(u.pos, b) <= Fx::from_px(8).0 as i64;
            stood = if u.pos.x == x && reached { stood + 1 } else { 0 };
            longest = longest.max(stood);
        }
        assert!(reached, "{def}");
        assert!(longest >= 120, "{def} waits at the far point: {longest}");
        assert!(longest < 200, "{def} and then goes: {longest}");
    }
}

/// `ai.ts seekBait` (2020's burial snakes and poisoned meat): the lurker walks to a drop of its
/// bait in sight, eats it, and dies of it.
#[test]
fn bait_is_walked_to_and_eaten() {
    let mut s = field();
    god(&mut s, true);
    let cat = jane_data::catalog();
    let lurker = spawn(&mut s, "lurker", 30, 30);
    let bait = cat.combat.unit(unit(&s, lurker).def).bait.unwrap();
    {
        let st = s.state_mut();
        let id = st.next.drop();
        let born = st.tick;
        st.zone_mut(Z).unwrap().drops.push(jane_sim::state::Drop {
            id,
            item: bait,
            qty: 1,
            pos: Vec2::centre(36, 30),
            born,
        });
    }
    let mut died = false;
    for _ in 0..600 {
        steps(&mut s, 1);
        if !unit(&s, lurker).alive {
            died = true;
            break;
        }
    }
    assert!(died, "it ate the bait");
    assert!(s.state().zone(Z).unwrap().drops.iter().all(|d| d.item != bait));
    assert!(px(dist(unit(&s, lurker).pos, Vec2::centre(36, 30))) <= 16);
}

// --- presence --------------------------------------------------------------------------------

/// school.test.ts: the Caretaker is night only. It is not there by day and is there by night;
/// nothing appears or vanishes while she stands within 120 x 80 px of it.
#[test]
fn the_caretaker_keeps_the_night_and_nobody_sees_it_come_or_go() {
    let mut s = field();
    god(&mut s, true);
    let c = spawn(&mut s, "caretaker", 60, 10);
    steps(&mut s, 30);
    assert!(unit(&s, c).hidden, "by day: not there");
    let rt = s.runtime(Z).unwrap();
    assert!(!rt.is_in(c) && rt.grid.occupants(60, 10) == 0, "and not standing anywhere");
    hour(&mut s, 22);
    steps(&mut s, 30);
    assert!(!unit(&s, c).hidden, "22:00: there");
    assert!(s.runtime(Z).unwrap().is_in(c));
    // She stands beside it at dawn: it stays until she has walked off.
    let her = me(&s);
    place(&mut s, her, 60, 14);
    hour(&mut s, 7);
    steps(&mut s, 60);
    assert!(!unit(&s, c).hidden, "watched: it does not vanish");
    place(&mut s, her, 10, 10);
    steps(&mut s, 30);
    assert!(unit(&s, c).hidden);
}

/// sim.test.ts "the dog keeps Julie's hours, but not on the first night": after nine it is on
/// the step until the key is given (`day_only_after`), and gone after nine once it has been.
#[test]
fn the_dog_keeps_julies_hours_only_after_the_key() {
    let mut s = field();
    let dog = spawn(&mut s, "dog", 60, 10);
    hour(&mut s, 22);
    steps(&mut s, 30);
    assert!(!unit(&s, dog).hidden);
    let q = jane_data::catalog().story.quest_id("defeat_skeleton").unwrap();
    s.state_mut().quests.done.push(q);
    steps(&mut s, 30);
    assert!(unit(&s, dog).hidden);
    hour(&mut s, 8);
    steps(&mut s, 30);
    assert!(!unit(&s, dog).hidden);
}

/// §4.6.a on arrival: a zone entered at night already has its night-only folk out and its
/// day-only folk away, watcher box or no (the TS's `stepDayOnly(to, true)`).
#[test]
fn arriving_at_night_it_was_already_gone() {
    let mut s = field();
    let dog = spawn(&mut s, "sheep", 11, 10);
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: start_sym() }));
    hour(&mut s, 23);
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: Z, mark: start_sym() }));
    assert!(unit(&s, dog).hidden, "right beside the door, and gone");
}

// --- the tape --------------------------------------------------------------------------------

/// The chase: six creatures round her on the field (and a snake past the wall), and a seeded
/// tape of her walking about and casting along random aims.
fn chase() -> Sim {
    let mut s = field();
    // After the bell, when the county's own ground comes for her too.
    hour(&mut s, 22);
    for id in ["icebolt", "fireball"] {
        learn(&mut s, id);
    }
    for (def, x, y) in [
        ("skeleton", 22, 10),
        ("rat", 16, 18),
        ("bandit", 30, 6),
        ("shade", 24, 24),
        ("spider", 5, 30),
        ("bat", 34, 12),
    ] {
        spawn(&mut s, def, x, y);
    }
    spawn(&mut s, "burial_snake", 60, 30);
    s
}

const SPELLS: [&str; 3] = ["icebolt", "fireball", "melee_player"];

struct ChaseTape {
    rng: Sfc32,
    held: InputFrame,
    cmds: Vec<StampedCommand>,
}

impl ChaseTape {
    fn new(seed: u32) -> Self {
        Self { rng: Sfc32::seeded(seed, 3), held: InputFrame::IDLE, cmds: Vec::new() }
    }

    fn step(&mut self, s: &mut Sim) {
        if self.rng.below(30) == 0 {
            let mag = if self.rng.below(3) == 0 { 0 } else { 60 + self.rng.below(60) as u8 };
            self.held = InputFrame {
                mv_dir: Angle(self.rng.next_u32() as u16),
                mv_mag: mag,
                sprint: self.rng.below(4) == 0,
                ..InputFrame::IDLE
            };
        }
        self.cmds.clear();
        let mut frame = self.held;
        if self.rng.below(9) == 0 {
            frame.aim = Some(Angle(self.rng.next_u32() as u16));
            let spell: SpellId = spell(SPELLS[self.rng.below(3) as usize]);
            self.cmds.push(StampedCommand { seat: Some(Seat(0)), seq: 0, cmd: Command::Cast { spell, on: None } });
        }
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = frame;
        s.step(&StepInput { frames, commands: &self.cmds });
    }
}

fn run_chase(s: &mut Sim, t: &mut ChaseTape, n: u32) {
    for _ in 0..n {
        t.step(s);
    }
}

/// Something fought: the tape is a chase, not a walk.
fn fought(s: &Sim) -> usize {
    s.state().zone(Z).unwrap().units.iter().filter(|u| u.combat != CombatState::Idle || !u.alive).count()
}

/// §8 `same_tape_same_hash` with the AI on the tape: two sims agree every 120 frames.
#[test]
fn same_tape_same_hash_with_the_chase_on_it() {
    let (mut a, mut b) = (chase(), chase());
    let (mut ta, mut tb) = (ChaseTape::new(5), ChaseTape::new(5));
    let mut most = 0;
    for f in 0..25 {
        run_chase(&mut a, &mut ta, 120);
        run_chase(&mut b, &mut tb, 120);
        assert_eq!(a.hash(), b.hash(), "frame {}", (f + 1) * 120);
        most = most.max(fought(&a));
    }
    assert!(most >= 3, "the creatures came for her: {most}");
    assert_eq!(a.state(), b.state());
}

/// §8 `save_load_continue` mid-chase: saved at frames 131 and 257 of 900 (paths, orders, the
/// snake's clock and trail in the save), loaded and continued, it is the unbroken game.
#[test]
fn save_load_continue_mid_chase() {
    let mut straight = chase();
    let mut t = ChaseTape::new(8);
    run_chase(&mut straight, &mut t, 900);
    for at in [131, 257] {
        let mut first = chase();
        let mut t = ChaseTape::new(8);
        run_chase(&mut first, &mut t, at);
        assert!(fought(&first) >= 1, "saved mid-chase ({at})");
        let bytes = first.save();
        let mut resumed = Sim::from_save_with(&bytes, first.blueprints().clone()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash());
        run_chase(&mut resumed, &mut t, 900 - at);
        assert_eq!(resumed.hash(), straight.hash(), "saved at {at}");
        assert_eq!(resumed.state(), straight.state());
    }
}

/// §8 `runtime_rebuild_is_invisible` mid-chase: every runtime thrown away and rebuilt every few
/// frames while things chase her; the hash stream does not move.
#[test]
fn runtime_rebuild_is_invisible_mid_chase() {
    let (mut a, mut b) = (chase(), chase());
    let (mut ta, mut tb) = (ChaseTape::new(13), ChaseTape::new(13));
    let mut chasing = 0;
    for f in 0..1500 {
        run_chase(&mut a, &mut ta, 1);
        if f % 37 == 11 {
            b.rebuild_runtimes();
        }
        run_chase(&mut b, &mut tb, 1);
        if f % 60 == 0 {
            assert_eq!(a.hash(), b.hash(), "frame {f}");
        }
        chasing += usize::from(a.state().zone(Z).unwrap().units.iter().any(|u| live_path(u).is_some()));
    }
    assert!(chasing > 100, "things were walking paths when it rebuilt: {chasing}");
    assert_eq!(a.state(), b.state());
}

/// §8 `awake_only_equals_everyone`: ticking every unit the old way (regen paid to every unit
/// every tick; the controllers' pass over every unit, asking whether it is awake) equals
/// ticking only the awake, on the chase and on the real seed's tape of joins, leaves and zone
/// changes. The old way keeps a sleeper's `synced` current, so both are settled (every unit
/// paid to now) before they are compared; a third run that is never settled until the end
/// proves settling changes nothing.
#[test]
fn awake_only_equals_everyone() {
    fn gate<T>(make: fn() -> (Sim, T), run: fn(&mut Sim, &mut T, u32, u32), what: &str) {
        let (mut a, mut ta) = make();
        let (mut b, mut tb) = make();
        let (mut c, mut tc) = make();
        b.tick_everyone(true);
        for chunk in 0..20 {
            let (from, to) = (chunk * 120, (chunk + 1) * 120);
            run(&mut a, &mut ta, from, to);
            run(&mut b, &mut tb, from, to);
            run(&mut c, &mut tc, from, to);
            a.settle();
            b.settle();
            assert_eq!(a.hash(), b.hash(), "{what}, frame {to}");
        }
        c.settle();
        assert_eq!(a.state(), b.state());
        assert_eq!(c.state(), a.state(), "{what}: settling is invisible");
    }
    gate(|| (chase(), ChaseTape::new(21)), |s, t, from, to| run_chase(s, t, to - from), "the chase");
    gate(|| (common::new_game(), common::Tape::new(17)), common::run, "the seed's tape");
}

// --- the budget ------------------------------------------------------------------------------

/// budget.test.ts's county: 3 000 units (one in seven a corpse on a 300-tick clock) and 8 000
/// props across the far side of it, nothing within 1 200 px of her.
fn busy_county() -> (Sim, Vec<UnitId>, Vec<PropId>) {
    const KEEP_CLEAR: i32 = 1200 * 256;
    let mut s = common::new_game();
    let cat = jane_data::catalog();
    let defs: Vec<_> =
        ["skeleton", "rat", "bat", "bandit", "spider"].iter().map(|d| cat.combat.unit_id(d).unwrap()).collect();
    let props: Vec<_> = ["rock", "crate", "torch", "lamp_post", "herb", "apple_tree", "car_wreck", "sign"]
        .iter()
        .map(|d| cat.story.prop_id(d).unwrap_or_else(|| panic!("{d}")))
        .collect();
    let home = unit(&s, me(&s)).pos;
    let (w, h) = {
        let rt = s.runtime(ZoneId::County).unwrap();
        (rt.grid.w() as i32, rt.grid.h() as i32)
    };
    let (mut units, mut extra) = (Vec::new(), Vec::new());
    {
        let st = s.state_mut();
        let key = st.syms.intern("budget_extra");
        let mut i = 0;
        for cy in (2..h - 4).step_by(3) {
            for cx in (2..w - 6).step_by(3) {
                let at = Vec2::centre(cx, cy);
                if (at.x.0 - home.x.0).abs() < KEEP_CLEAR && (at.y.0 - home.y.0).abs() < KEEP_CLEAR {
                    continue;
                }
                i += 1;
                if i % 11 < 3 {
                    if units.len() >= 3000 {
                        continue;
                    }
                    let id = st.next.unit();
                    let def = defs[units.len() % defs.len()];
                    let mut u = jane_sim::units::new_unit(id, None, def, at, jane_core::action::Facing::South, st.tick);
                    let zs = st.zone_mut(ZoneId::County).unwrap();
                    if units.len() % 7 == 0 {
                        u.alive = false;
                        u.hp = Milli::ZERO;
                        let due = (Tick(300), id);
                        let at = zs.sleeping_due.partition_point(|&e| e < due);
                        zs.sleeping_due.insert(at, due);
                    }
                    zs.insert_unit(u);
                    units.push(id);
                } else if extra.len() < 8000 {
                    let id = st.next.prop();
                    let def = props[extra.len() % props.len()];
                    st.zone_mut(ZoneId::County).unwrap().props.push(Prop {
                        id,
                        key,
                        def,
                        spawn: None,
                        cell: Cell::new(cx as u16, cy as u16),
                        solid: cat.story.prop(def).solid,
                        hidden: false,
                        locked: false,
                        used: false,
                        on: false,
                        under_done: false,
                        more: jane_core::Rare::empty(),
                    });
                    extra.push(id);
                }
            }
        }
    }
    s.rebuild_runtimes();
    assert_eq!((units.len(), extra.len()), (3000, 8000));
    (s, units, extra)
}

/// Walk a square, sprinting half the time: she crosses ring blocks all the way round.
fn stroll(t: u32) -> InputFrame {
    let dir = [Angle::EAST, Angle::SOUTH, Angle::WEST, Angle::NORTH][(t / 100 % 4) as usize];
    InputFrame { sprint: t % 200 < 100, ..InputFrame::walk(dir) }
}

/// budget.test.ts "3000 units and 8000 props asleep across the county": she strolls; the far
/// side sleeps, nothing of it stands anywhere, every corpse on the short clock is up, a sleeping
/// car wreck is still a wall, and at home the yard's skeleton still takes exception to her.
#[test]
fn the_tick_costs_what_is_awake() {
    let (mut s, units, props) = busy_county();
    let x0 = unit(&s, me(&s)).pos;
    let mut far = 0;
    for t in 0..600 {
        s.step(&StepInput::solo(stroll(t)));
        let p = unit(&s, me(&s)).pos;
        far = far.max((p.x.0 - x0.x.0).abs() + (p.y.0 - x0.y.0).abs());
    }
    assert!(far > 64 * 256);
    let zs = s.state().zone(ZoneId::County).unwrap();
    let rt = s.runtime(ZoneId::County).unwrap();
    assert!(units.iter().all(|&id| !zs.unit(id).unwrap().awake && !rt.is_in(id)));
    assert!(units.iter().step_by(7).all(|&id| zs.unit(id).unwrap().alive), "every corpse on a 300-tick clock is up");
    assert!(props.iter().all(|&id| !rt.awake_props[zs.prop_ix(id).unwrap() as usize]));
    let wreck = props
        .iter()
        .map(|&id| &zs.props[zs.prop_ix(id).unwrap() as usize])
        .find(|p| jane_data::catalog().story.prop(p.def).id == "car_wreck");
    let wreck = wreck.unwrap();
    assert!(rt.grid.solid(i32::from(wreck.cell.x) + 4, i32::from(wreck.cell.y) + 2), "a sleeping wreck is a wall");

    // And home is as it was.
    let key = jane_sim::sym::of_name(jane_data::catalog().name_id("yard_skeleton").unwrap());
    let skel = *s.runtime(ZoneId::County).unwrap().unit_names.get(&key).unwrap();
    // The seed puts it far from the start (and the lattice all round it): she is stood a few
    // cells off after the bell (by day it walks its fence and minds it), the ring wakes it, and
    // it has her.
    hour(&mut s, 22);
    let (x, y) = unit(&s, skel).pos.cell();
    let (fx, fy) = s.runtime(ZoneId::County).unwrap().grid.nearest_free(x - 6, y, 6, None).unwrap();
    let her = me(&s);
    edit(&mut s, her, |u| u.pos = Vec2::centre(fx, fy));
    steps(&mut s, 30);
    assert_eq!(target(&s, skel), (CombatState::Combat, Some(me(&s))));
}

fn percentiles(mut times: Vec<u64>) -> String {
    times.sort();
    let us = |ns: u64| format!("{}.{} us", ns / 1000, ns % 1000 / 100);
    let pct = |p: usize| us(times[times.len() * p / 100]);
    format!("median {}, p90 {}, p99 {}, worst {}", pct(50), pct(90), pct(99), us(*times.last().unwrap()))
}

/// §9.4 "median tick, county, TS budget test's sleepers" (release: `cargo test --release -p
/// jane-sim --test ai -- --ignored --nocapture`).
#[test]
#[ignore = "a timing, not a rule: run in release with --ignored --nocapture"]
#[allow(clippy::disallowed_types)]
fn budget_timing() {
    let (mut s, _, _) = busy_county();
    let mut times = Vec::with_capacity(2400);
    for t in 0..2400 {
        let a = std::time::Instant::now();
        s.step(&StepInput::solo(stroll(t)));
        times.push(a.elapsed().as_nanos() as u64);
    }
    let awake = s.runtime(ZoneId::County).unwrap().awake_units.len();
    println!("budget: 3000 sleepers, 8000 props, {awake} awake: {}", percentiles(times));
}

/// §9 "60 awake AI" in the county (release, `--ignored --nocapture`): sixty skeletons set on
/// her, chasing her round a small square inside their leash; she is a god so nobody dies.
#[test]
#[ignore = "a timing, not a rule: run in release with --ignored --nocapture"]
#[allow(clippy::disallowed_types)]
fn sixty_chasers_timing() {
    let mut s = common::new_game();
    god(&mut s, true);
    let her = me(&s);
    // In the yard, the county's own creatures about (the seed's start is at its west edge).
    let key = jane_sim::sym::of_name(jane_data::catalog().name_id("yard_skeleton").unwrap());
    let yard = *s.runtime(ZoneId::County).unwrap().unit_names.get(&key).unwrap();
    let (x, y) = unit(&s, yard).pos.cell();
    let (fx, fy) = s.runtime(ZoneId::County).unwrap().grid.nearest_free(x - 6, y, 6, None).unwrap();
    edit(&mut s, her, |u| u.pos = Vec2::centre(fx, fy));
    let home = unit(&s, her).pos;
    let skel = jane_data::catalog().combat.unit_id("skeleton").unwrap();
    let mut foes = Vec::new();
    for i in 0..60 {
        let d = jane_core::angle::along(Angle((i * 65_536 / 60) as u16), Fx::from_cells(6 + i % 9));
        let (x, y) = (home + d).cell();
        let rt = s.runtime(ZoneId::County).unwrap();
        let Some((fx, fy)) = rt.grid.nearest_free(x, y, 10, None) else { continue };
        let st = s.state_mut();
        let id = st.next.unit();
        let mut u =
            jane_sim::units::new_unit(id, None, skel, Vec2::centre(fx, fy), jane_core::action::Facing::South, st.tick);
        u.awake = true;
        u.target = Some(her);
        u.combat = CombatState::Combat;
        st.zone_mut(ZoneId::County).unwrap().insert_unit(u);
        foes.push(id);
    }
    s.rebuild_runtimes();
    assert_eq!(foes.len(), 60);
    let mut times = Vec::with_capacity(4000);
    for t in 0..4000 {
        let a = std::time::Instant::now();
        // Her square is a small one, walked: the stroll's (40 cells a side at a sprint) took her
        // past every chaser's leash (2.5 x its aggro, 17.5 m, since 2026-10-06) inside a minute.
        let dir = [Angle::EAST, Angle::SOUTH, Angle::WEST, Angle::NORTH][(t / 30 % 4) as usize];
        s.step(&StepInput::solo(InputFrame::walk(dir)));
        times.push(a.elapsed().as_nanos() as u64);
    }
    let zs = s.state().zone(ZoneId::County).unwrap();
    let chasing = foes.iter().filter(|&&f| zs.unit(f).is_some_and(|u: &Unit| u.combat == CombatState::Combat)).count();
    let awake = s.runtime(ZoneId::County).unwrap().awake_units.len();
    println!("60 chasers: {chasing} still on her, {awake} awake: {}; paths {:?}", percentiles(times), s.path_stats());
    assert!(chasing >= 30);
}
