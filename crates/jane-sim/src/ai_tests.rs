//! The controllers where only a `Ctx` reaches: orders (`Send`'s work, `npc::send`), the ecology
//! rows on a row made for the test (`hunts`, `flees`: no content has them yet), a schedule's
//! `Mark` slot, and the light a path search gathers. Carries `jane/test/verbs2.test.ts` E9
//! ("send": walked past someone it would attack, then its list runs with it as the subject; a
//! walk it cannot finish is given up; a save mid-walk loads into the same walk and the list
//! runs for whoever sent it) and ARCHITECTURE.md §4.6.a and §4.6.c. The rest is `tests/ai.rs`.

use std::sync::Arc;

use jane_core::action::{Facing, FlagKey as ContentFlag, FlagOp};
use jane_core::blueprint::Mark;
use jane_core::num::{dist_sq, isqrt};
use jane_core::{Action, Blueprint, Cell, Key, ListRef, Rect, Tick, Tile, UnitDefId, Vec2, ZoneId};
use jane_data::{ScheduleSlot, UnitDef};

use crate::ai::{LitField, flee, tick_ai_with};
use crate::blueprints::Blueprints;
use crate::ctx::{Ctx, PartySnap};
use crate::ids::{ClientToken, PropId, Seat, UnitId};
use crate::input::{Command, StampedCommand, StepInput};
use crate::light::lit_at;
use crate::npc::send;
use crate::sim::Sim;
use crate::state::{CombatState, FlagKey, LootState, Prop, Unit};
use crate::units::{new_unit, think_offset};

const Z: ZoneId = ZoneId::County;

/// The field: every zone 128 x 64 of grass, `start` at (10, 10) facing east; in the county a
/// walled box at (100..109, 30..39) with nothing inside, a mark named `yard_skeleton` at
/// (60, 10), and three lists: `arrived` (a flag
/// and a stun on the subject), `flag` (the flag alone), `home` (travel to the house's start).
struct Field {
    s: Sim,
    arrived: ListRef,
    flag_only: ListRef,
    to_house: ListRef,
    flag: Key,
}

fn field() -> Field {
    let cat = jane_data::catalog();
    let start = Key::Name(cat.story.start.mark);
    let stunned = cat.combat.effect_id("stunned").unwrap();
    let mut lists = (ListRef::Blueprint(0), ListRef::Blueprint(0), ListRef::Blueprint(0), Key::Local(0));
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(start, Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        if z == Z {
            for r in
                [Rect::new(100, 30, 9, 1), Rect::new(100, 38, 9, 1), Rect::new(100, 30, 1, 9), Rect::new(108, 30, 1, 9)]
            {
                bp.tiles.fill_rect(r, Tile::Wall);
            }
            let there = Key::Name(cat.name_id("yard_skeleton").unwrap());
            bp.marks.insert(there, Mark { cell: Cell::new(60, 10), facing: None });
            let flag = bp.local("test_arrived");
            let set = Action::Flag { key: ContentFlag::Named(flag), op: FlagOp::Set(1) };
            let a = bp.push_list(vec![set, Action::Status(stunned)]);
            let b = bp.push_list(vec![set]);
            let c = bp.push_list(vec![Action::Travel { zone: ZoneId::House, mark: start }]);
            lists = (a, b, c, flag);
        }
        Arc::new(bp)
    });
    let s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    Field { s, arrived: lists.0, flag_only: lists.1, to_house: lists.2, flag: lists.3 }
}

fn me(s: &Sim) -> UnitId {
    s.state.players[0].unit
}

fn unit(s: &Sim, id: UnitId) -> &Unit {
    s.state.zone(Z).unwrap().unit(id).unwrap()
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

fn edit(s: &mut Sim, id: UnitId, f: impl FnOnce(&mut Unit)) {
    f(s.state.zone_mut(Z).unwrap().unit_mut(id).unwrap());
    s.rebuild_runtimes();
}

fn in_ctx<R>(s: &mut Sim, seat: Option<Seat>, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
    let snap = PartySnap::of(&s.state);
    s.with_ctx(Z, seat, &snap, false, f)
}

fn step(s: &mut Sim, n: u32) {
    for _ in 0..n {
        s.step(&StepInput::IDLE);
    }
}

fn cmd(s: &mut Sim, seat: Option<u8>, c: Command) {
    let cmds = [StampedCommand { seat: seat.map(Seat), seq: 0, cmd: c }];
    s.step(&StepInput { frames: StepInput::IDLE.frames, commands: &cmds });
}

fn dist_px(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64)) / 256
}

fn flagged(f: &Field) -> bool {
    let k = crate::sym::of_key(f.flag, &f.s.rts[Z.index()].as_ref().unwrap().locals);
    f.s.state.flags.get(&FlagKey::Named(k)).copied().unwrap_or(0) == 1
}

/// A row like `base` with its ecology rows swapped in; leaked, as a static row is.
fn made_row(base: &str, hunts: &[UnitDefId], flees: &[UnitDefId], leash_m: i32) -> &'static UnitDef {
    let cat = jane_data::catalog();
    let mut d = *cat.combat.unit(cat.combat.unit_id(base).unwrap());
    d.hunts = Box::leak(hunts.to_vec().into_boxed_slice());
    d.flees = Box::leak(flees.to_vec().into_boxed_slice());
    if leash_m > 0 {
        d.leash = jane_core::Fx::from_metres(leash_m);
    }
    Box::leak(Box::new(d))
}

fn def_id(id: &str) -> UnitDefId {
    jane_data::catalog().combat.unit_id(id).unwrap()
}

// --- aggro -----------------------------------------------------------------------------------

/// PLAN.md §2.6 *Aggro* (the owner, 2026-09-30; shortened 2026-10-06 with the 40 x 22.5 view):
/// WoW's rule scaled to the view. A skeleton (7 m) notices her at 7 m at her match (New Game
/// against phase 1), shorter as she grows past it down to 3 m, longer while she is behind its
/// phase but never past 10 m; the dark adds a half again, however deep, and the cap holds; a boss
/// keeps its arena's row; a rabbit notices nobody.
#[test]
fn aggro_shrinks_as_she_outgrows_it_and_never_leaves_the_screen() {
    use crate::ai::aggro_reach;
    use jane_core::num::METRE_FX;
    let cat = jane_data::catalog();
    let row = |id: &str| cat.combat.unit(def_id(id));
    let m = |metres_x100: i64| metres_x100 * i64::from(METRE_FX) / 100;
    let bones = row("skeleton");
    let base = i64::from(bones.aggro.0);
    assert_eq!(base, m(700));
    let at = |her: u32, mult: u16, dark: i32| aggro_reach(bones, bones.strength * mult, Some(her), dark);
    // Her match: New Game (strength 30, spirit 30) against phase 1.
    assert_eq!(at(60, 1, 0), m(700));
    // Twice her match: three quarters; five times and past: the floor.
    assert_eq!(at(120, 1, 0), m(525));
    assert_eq!(at(300, 1, 0), m(300));
    assert_eq!(at(1_000, 1, 0), m(300));
    // Behind it: phase 3 (a match of 180) against New Game notices from a third further...
    assert_eq!(at(60, 3, 0), base * 4 / 3);
    // ...and nothing is ever past the cap, by day or by night (phase 8's match is 480: 10.3 m).
    assert_eq!(at(30, 8, 0), m(1000));
    assert_eq!(at(30, 8, 2), m(1000));
    // The dark: half again, the same however deep, and never past the cap (7 m x 1.5 is 10.5).
    assert_eq!(at(60, 1, 1), m(1000));
    assert_eq!(at(60, 1, 2), m(1000));
    assert_eq!(at(300, 1, 1), m(450));
    assert_eq!(at(120, 1, 1), m(525) * 3 / 2);
    assert_eq!(at(60, 3, 1), m(1000));
    // Never longer as she grows, at any phase, day or night, and always on the screen.
    for mult in [1, 2, 3, 4, 6, 8] {
        for dark in 0..=2 {
            let mut last = i64::MAX;
            for her in (30..=1_200).step_by(10) {
                let r = at(her, mult, dark);
                assert!(r <= last && (m(300)..=m(1000)).contains(&r), "{mult} {dark} {her}: {r}");
                last = r;
            }
        }
    }
    // Not her (the prey of a `hunts` row): the row's own, half again in the dark.
    assert_eq!(aggro_reach(bones, bones.strength * 6, None, 0), base);
    assert_eq!(aggro_reach(bones, bones.strength, None, 1), m(1000));
    // A boss keeps its arena's reach, grown or not; a passive row notices nobody.
    let boss = row("headmaster");
    assert!(boss.boss);
    assert_eq!(aggro_reach(boss, boss.strength, Some(2_000), 0), i64::from(boss.aggro.0));
    assert_eq!(aggro_reach(boss, boss.strength, Some(2_000), 1), i64::from(boss.aggro.0) * 3 / 2);
    let rabbit = row("rabbit");
    assert_eq!(aggro_reach(rabbit, rabbit.strength, Some(60), 2), 0);
    // The yard's bones (4 m) are her match at New Game and come down to the floor as she grows.
    let yard = row("yard_bones");
    assert_eq!(aggro_reach(yard, yard.strength, Some(60), 0), m(400));
    assert_eq!(aggro_reach(yard, yard.strength, Some(300), 0), m(300));
}

// --- E9: send --------------------------------------------------------------------------------

/// verbs2.test.ts E9 "it walks to the mark past someone it would otherwise attack, then its
/// list runs with it as the subject".
#[test]
fn a_sent_unit_walks_past_her_and_its_list_runs_on_it() {
    let mut f = field();
    let walker = spawn(&mut f.s, "skeleton", 2, 10);
    let to = Vec2::centre(20, 10);
    let then = f.arrived;
    in_ctx(&mut f.s, Some(Seat(0)), |cx| send(cx, walker, to, Some(then)));
    assert!(unit(&f.s, walker).order.is_some());
    let hp = unit(&f.s, me(&f.s)).hp;
    let mut ticks = 0;
    while unit(&f.s, walker).order.is_some() && ticks < 2000 {
        step(&mut f.s, 1);
        ticks += 1;
    }
    assert!(flagged(&f), "its list ran");
    let w = unit(&f.s, walker);
    assert!(dist_px(w.pos, to) <= 12);
    let stunned = jane_data::catalog().combat.effect_id("stunned").unwrap();
    assert!(w.statuses.iter().any(|s| s.effect == stunned), "`then` lands on the one that was sent");
    assert!(unit(&f.s, me(&f.s)).statuses.is_empty());
    assert_eq!(unit(&f.s, me(&f.s)).hp, hp, "it minded nothing on the way");
    assert_eq!(w.home, w.pos, "and it stays where it stops");
}

/// verbs2.test.ts E9 "a walk it cannot finish is given up after its budget, and then nothing
/// happens"; and nobody of the party, and no snake, is ever sent.
#[test]
fn a_walk_it_cannot_finish_is_given_up() {
    let mut f = field();
    f.s.state.players[0].god = true;
    let walker = spawn(&mut f.s, "skeleton", 90, 34);
    let then = f.flag_only;
    in_ctx(&mut f.s, Some(Seat(0)), |cx| send(cx, walker, Vec2::centre(104, 34), Some(then)));
    let budget = unit(&f.s, walker).order.as_ref().unwrap().until.0 - f.s.state.tick.0;
    assert!(budget > 600);
    for _ in 0..=budget {
        if unit(&f.s, walker).order.is_none() {
            break;
        }
        step(&mut f.s, 1);
    }
    assert!(unit(&f.s, walker).order.is_none());
    assert!(!flagged(&f));
    let (her, snake) = (me(&f.s), spawn(&mut f.s, "burial_snake", 60, 40));
    in_ctx(&mut f.s, Some(Seat(0)), |cx| {
        send(cx, her, Vec2::centre(20, 20), None);
        send(cx, snake, Vec2::centre(20, 20), None);
    });
    assert!(unit(&f.s, her).order.is_none() && unit(&f.s, snake).order.is_none());
}

/// verbs2.test.ts E9 "a save taken mid-walk loads into the same walk, and the list still runs
/// for whoever sent it". The list is a `Travel`, a verb only an actor can do: in the world the
/// friend stayed in, it is she who goes; in the save (the host's world alone) it is nobody.
#[test]
fn a_save_mid_walk_loads_into_the_same_walk() {
    let mut f = field();
    cmd(&mut f.s, Some(0), Command::Open(true));
    cmd(&mut f.s, None, Command::Join { who: ClientToken(5) });
    let walker = spawn(&mut f.s, "rat", 2, 12);
    f.s.state.players[0].god = true;
    f.s.state.players[1].god = true;
    let to = Vec2::centre(20, 12);
    let then = f.to_house;
    in_ctx(&mut f.s, Some(Seat(1)), |cx| send(cx, walker, to, Some(then)));
    assert_eq!(unit(&f.s, walker).order.as_ref().unwrap().seat, Some(Seat(1)));
    step(&mut f.s, 60);
    assert!(unit(&f.s, walker).order.is_some());

    let bytes = f.s.save();
    let mut a = Sim::from_save_with(&bytes, f.s.bps.clone()).unwrap();
    let mut b = Sim::from_save_with(&bytes, f.s.bps.clone()).unwrap();
    step(&mut a, 900);
    step(&mut b, 900);
    assert_eq!(a.hash(), b.hash());
    let arrived = a.state.zone(Z).unwrap().unit(walker).unwrap();
    assert!(arrived.order.is_none());
    assert!(dist_px(arrived.pos, to) <= 12);
    assert_eq!(a.state.players[0].zone, Z, "nobody sent it in the host's world: nobody goes");

    // In the world where she stayed, it is the friend who goes.
    for _ in 0..900 {
        if unit(&f.s, walker).order.is_none() {
            break;
        }
        step(&mut f.s, 1);
    }
    step(&mut f.s, 1);
    assert_eq!((f.s.state.players[0].zone, f.s.state.players[1].zone), (Z, ZoneId::House));
}

// --- §4.6.c: hunts and flees -------------------------------------------------------------------

/// Run `f` in a ctx at the next tick on which `id` looks about it (its staggered beat).
fn on_its_beat(s: &mut Sim, id: UnitId, f: impl FnOnce(&mut Ctx<'_>)) {
    while (s.state.tick.0 + think_offset(id)) % crate::tuning::AGGRO_PERIOD != 0 {
        step(s, 1);
    }
    in_ctx(s, None, f);
}

/// §4.6.c `hunts`: an idle creature takes a unit of a hunted row inside its aggro reach as its
/// target (a fox and a hen; here a skeleton and a hen), and then fights it as any target. The
/// same creature with no such row leaves the hen alone.
#[test]
fn an_idle_hunter_takes_what_it_hunts() {
    let mut f = field();
    f.s.state.players[0].god = true;
    let fox = spawn(&mut f.s, "skeleton", 40, 20);
    let hen = spawn(&mut f.s, "hen", 46, 20);
    let plain = jane_data::catalog().combat.unit(def_id("skeleton"));
    on_its_beat(&mut f.s, fox, |cx| tick_ai_with(cx, fox, plain));
    assert_eq!(unit(&f.s, fox).combat, CombatState::Idle, "no row, no hunt");
    let row = made_row("skeleton", &[def_id("hen")], &[], 0);
    on_its_beat(&mut f.s, fox, |cx| tick_ai_with(cx, fox, row));
    assert_eq!((unit(&f.s, fox).combat, unit(&f.s, fox).target), (CombatState::Combat, Some(hen)));
    let hp = unit(&f.s, hen).hp;
    step(&mut f.s, 400);
    assert!(unit(&f.s, hen).hp < hp || !unit(&f.s, hen).alive, "and it goes for it");
}

/// A rooted thing (no feet: a cactus) holds her in its fight while she is near, and lets go
/// once she is half again its aggro from it: no chase ever takes it past its leash, and it
/// would otherwise hold her from anywhere in the zone.
#[test]
fn a_rooted_thing_lets_go_of_her_far_off() {
    let mut f = field();
    f.s.state.players[0].god = true;
    let her = me(&f.s);
    edit(&mut f.s, her, |u| u.pos = Vec2::centre(10, 10));
    let cactus = spawn(&mut f.s, "cactus", 20, 10);
    edit(&mut f.s, cactus, |u| {
        u.combat = CombatState::Combat;
        u.target = Some(her);
    });
    let row = jane_data::catalog().combat.unit(def_id("cactus"));
    in_ctx(&mut f.s, None, |cx| tick_ai_with(cx, cactus, row));
    assert_eq!(unit(&f.s, cactus).combat, CombatState::Combat, "ten cells off, it keeps her");
    edit(&mut f.s, her, |u| u.pos = Vec2::centre(70, 50));
    in_ctx(&mut f.s, None, |cx| tick_ai_with(cx, cactus, row));
    assert_ne!(unit(&f.s, cactus).combat, CombatState::Combat, "across the field, it lets go");
}

/// §4.6.c `flees`: with a unit of a fled row inside its leash, it walks its leash away from it,
/// and stops once it is clear.
#[test]
fn it_runs_from_what_it_flees() {
    let mut f = field();
    f.s.state.players[0].god = true;
    let fox = spawn(&mut f.s, "skeleton", 60, 30);
    edit(&mut f.s, fox, |u| u.stop_until = Tick(u32::MAX / 2));
    let hen = spawn(&mut f.s, "hen", 63, 30);
    let row = made_row("hen", &[], &[def_id("skeleton")], 6);
    let d0 = dist_px(unit(&f.s, hen).pos, unit(&f.s, fox).pos);
    let mut fled = 0;
    for _ in 0..200 {
        fled += u32::from(in_ctx(&mut f.s, None, |cx| flee(cx, hen, row, false)));
        step(&mut f.s, 1);
    }
    let d = dist_px(unit(&f.s, hen).pos, unit(&f.s, fox).pos);
    assert!(d > d0 + 16, "{d0} -> {d}");
    assert!(d >= 44, "clear of its 6 m leash: {d}");
    assert!(fled > 0 && fled < 200, "it stopped running once clear: {fled}");
    assert!(unit(&f.s, hen).pos.x > unit(&f.s, fox).pos.x, "away from it");
}

// --- §4.6.a: a schedule's mark ---------------------------------------------------------------

/// `Mark`: nobody watching either end, it jumps there; watched, it is given an order and walks;
/// hidden, it is shown at the mark once nobody watches the mark.
#[test]
fn a_mark_slot_jumps_out_of_sight_and_walks_in_it() {
    let mut f = field();
    let cat = jane_data::catalog();
    let name = cat.name_id("yard_skeleton").unwrap();
    let mark = Vec2::centre(60, 10);
    // The slot names the mark by a content name: the field's county has one at (60, 10).
    let slot = ScheduleSlot::Mark(name);
    let far = spawn(&mut f.s, "sheep", 110, 10);
    let ix = |s: &Sim, id: UnitId| s.state.zone(Z).unwrap().unit_ix(id).unwrap();
    let i = ix(&f.s, far);
    in_ctx(&mut f.s, None, |cx| crate::presence::put(cx, i, slot, false));
    assert_eq!(unit(&f.s, far).pos, mark, "nobody saw it go or arrive");
    assert_eq!(unit(&f.s, far).home, mark);

    let near = spawn(&mut f.s, "sheep", 14, 12);
    let i = ix(&f.s, near);
    in_ctx(&mut f.s, None, |cx| crate::presence::put(cx, i, slot, false));
    assert_eq!(unit(&f.s, near).pos, Vec2::centre(14, 12), "she is beside it: it does not jump");
    assert!(unit(&f.s, near).order.is_some(), "it walks");
    let mut moved = 0;
    for _ in 0..1500 {
        let x = unit(&f.s, near).pos;
        step(&mut f.s, 1);
        moved += i32::from(unit(&f.s, near).pos != x);
        if unit(&f.s, near).order.is_none() {
            break;
        }
    }
    assert!(dist_px(unit(&f.s, near).pos, mark) <= 12 && moved > 100);

    // Hidden, with her standing at the mark: it waits; she goes, and it is there.
    edit(&mut f.s, far, |u| u.hidden = true);
    let her = me(&f.s);
    edit(&mut f.s, her, |u| u.pos = Vec2::centre(62, 12));
    let i = ix(&f.s, far);
    in_ctx(&mut f.s, None, |cx| crate::presence::put(cx, i, slot, false));
    assert!(unit(&f.s, far).hidden);
    edit(&mut f.s, her, |u| u.pos = Vec2::centre(10, 10));
    in_ctx(&mut f.s, None, |cx| crate::presence::put(cx, i, slot, false));
    assert!(!unit(&f.s, far).hidden);
    assert!(f.s.rts[Z.index()].as_ref().unwrap().is_in(far));
}

/// §4.6.a: `day_only` and `night_only` are two-slot schedules; a schedule row may wrap midnight.
#[test]
fn day_only_and_night_only_are_two_slot_schedules() {
    use crate::presence::{in_hours, slot_of};
    let f = field();
    let mut w = f.s.state.clone();
    let cat = jane_data::catalog();
    let (sheep, moth, rat) =
        (cat.combat.unit(def_id("sheep")), cat.combat.unit(def_id("forest_moth_8")), cat.combat.unit(def_id("rat")));
    for (h, day) in [(5, false), (6, true), (20, true), (21, false), (0, false)] {
        w.clock = h * crate::tuning::TICKS_PER_HOUR;
        let (up, down) = (ScheduleSlot::Patrol, ScheduleSlot::Absent);
        assert_eq!(slot_of(sheep, &w), Some(if day { up } else { down }), "{h}:00");
        assert_eq!(slot_of(moth, &w), Some(if day { down } else { up }), "{h}:00");
        assert_eq!(slot_of(rat, &w), None);
    }
    assert!(in_hours(22, 21, 6) && in_hours(3, 21, 6) && !in_hours(6, 21, 6) && !in_hours(12, 21, 6));
    assert!(in_hours(6, 6, 21) && !in_hours(21, 6, 21) && in_hours(9, 4, 4));
}

// --- light for a search ----------------------------------------------------------------------

/// The field a search gathers answers every cell as `lit_at` does at the cell's middle, warm and
/// cold, lamps and day-only light, on and off.
#[test]
fn a_gathered_field_is_the_light_rule() {
    let mut f = field();
    let cat = jane_data::catalog();
    let mut put = |def: &str, x: u16, y: u16, on: bool| {
        let d = cat.story.prop_id(def).unwrap();
        let id: PropId = f.s.state.next.prop();
        let key = f.s.state.syms.intern(&format!("t{}", id.get()));
        f.s.state.zone_mut(Z).unwrap().props.push(Prop {
            id,
            key,
            def: d,
            spawn: None,
            cell: Cell::new(x, y),
            solid: false,
            hidden: false,
            locked: false,
            used: false,
            on,
            loot: LootState::AsSpawned,
            under_done: false,
            regrow: None,
            burns_until: None,
            night: crate::state::NightState::AsSpawned,
        });
    };
    put("brazier", 20, 20, true);
    put("brazier", 30, 20, false);
    put("torch_blue", 26, 26, true);
    put("lamp_post", 34, 30, false);
    put("campfire", 16, 32, false);
    f.s.rebuild_runtimes();
    for hour in [12, 20] {
        f.s.state.clock = hour * crate::tuning::TICKS_PER_HOUR;
        let (zone, rt, clock) = (f.s.state.zone(Z).unwrap(), f.s.rts[Z.index()].as_deref().unwrap(), f.s.state.clock);
        for warm in [false, true] {
            let mut field = LitField::default();
            field.gather(zone, rt, clock, (10, 10), (45, 45), warm);
            let mut lit = 0;
            for y in 10..=45 {
                for x in 10..=45 {
                    let want = lit_at(zone, rt, clock, Vec2::centre(x, y), warm);
                    assert_eq!(field.cell_lit(x, y), want, "({x}, {y}) at {hour}:00, warm {warm}");
                    lit += u32::from(want);
                }
            }
            assert!(lit > 50, "{lit}");
        }
    }
}

/// Grok #6: a unit a step works on that is gone from its zone (a content or engine bug) is
/// skipped with a dev event, never an abort of the host and every guest with it.
#[test]
fn a_missing_unit_is_skipped_with_a_dev_event_not_a_panic() {
    use crate::event::{DevNote, EventKind};
    let mut f = field();
    let real = spawn(&mut f.s, "skeleton", 40, 20);
    let def = jane_data::catalog().combat.unit(def_id("skeleton"));
    let gone = UnitId::new(60_000).expect("an id");
    assert!(f.s.state.zone(Z).expect("the county").unit(gone).is_none());
    let notes = in_ctx(&mut f.s, None, |cx| {
        let before = cx.events.len();
        crate::ai::leash(cx, gone, def, def.run, false);
        crate::ai::fight(cx, gone, def, def.run, false);
        tick_ai_with(cx, real, def);
        cx.events[before..]
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Dev(DevNote::MissingUnit { unit, at }) => Some((unit, at)),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(notes, vec![(gone, "ai::leash"), (gone, "ai::fight")]);
}

// --- the night (the owner, 2026-10-06) -------------------------------------------------------

/// A brazier, lit, on a cell of the county.
fn brazier(s: &mut Sim, x: u16, y: u16) {
    let cat = jane_data::catalog();
    let id: PropId = s.state.next.prop();
    let key = s.state.syms.intern(&format!("t{}", id.get()));
    s.state.zone_mut(Z).unwrap().props.push(Prop {
        id,
        key,
        def: cat.story.prop_id("brazier").unwrap(),
        spawn: None,
        cell: Cell::new(x, y),
        solid: false,
        hidden: false,
        locked: false,
        used: false,
        on: true,
        loot: LootState::AsSpawned,
        under_done: false,
        regrow: None,
        burns_until: None,
        night: crate::state::NightState::AsSpawned,
    });
    s.rebuild_runtimes();
}

/// A blow from the county's own lands 30 per cent harder on her at night out of the light, and as
/// by day in a lamp's or a fire's warm light, and by day.
#[test]
fn a_night_blow_lands_harder_only_out_of_the_light() {
    use crate::combat::{Hit, queue_hit};
    use jane_core::Milli;
    use jane_core::action::School;
    let blow = |hour: u32, lamp: bool| -> i32 {
        let mut f = field();
        f.s.state.clock = hour * crate::tuning::TICKS_PER_HOUR;
        let foe = spawn(&mut f.s, "skeleton", 14, 10);
        if lamp {
            brazier(&mut f.s, 10, 8);
        }
        let her = me(&f.s);
        let before = unit(&f.s, her).hp.0;
        in_ctx(&mut f.s, None, |cx| {
            let out = crate::ai::out_in_the_night(cx, cx.zone.unit(her).unwrap().pos);
            assert_eq!(out, hour == 22 && !lamp, "{hour}:00, lamp {lamp}");
            let hit = Hit {
                to: her,
                amount: Milli(10_000),
                school: School::Physical,
                from: Some(foe),
                crit: false,
                status: None,
            };
            queue_hit(cx, hit);
            crate::flush::flush(cx);
        });
        before - unit(&f.s, her).hp.0
    };
    assert_eq!(blow(12, false), 10_000, "by day");
    assert_eq!(blow(22, false), 10_000 * crate::tuning::NIGHT_HIT as i32 / 100, "at night in the dark");
    assert_eq!(blow(22, true), 10_000, "at night by a fire");
}

/// The night's reach is the county's: out of the light a creature notices and follows further
/// (`night_reach` 1), in warm light or by day not at all.
#[test]
fn the_night_reaches_only_out_of_the_light() {
    let mut f = field();
    let foe = spawn(&mut f.s, "skeleton", 40, 20);
    let def = jane_data::catalog().combat.unit(def_id("skeleton"));
    let reach = |s: &mut Sim, hour: u32| {
        s.state.clock = hour * crate::tuning::TICKS_PER_HOUR;
        in_ctx(s, None, |cx| crate::ai::night_reach(cx, foe, def))
    };
    assert_eq!(reach(&mut f.s, 12), 0);
    assert_eq!(reach(&mut f.s, 22), 1);
    assert_eq!(reach(&mut f.s, 3), 1);
    assert_eq!(reach(&mut f.s, 6), 0);
    brazier(&mut f.s, 40, 18);
    assert_eq!(reach(&mut f.s, 22), 0, "in a fire's light");
}

/// The owner's playtest, 2026-10-07: the leash is three and three quarter times the aggro in the
/// open county, the yard's bones by Julie's gate twice that, and two and a half times in every
/// dungeon and building (small rooms; what the crawls were tuned to). A boss keeps its row.
#[test]
fn a_leash_is_longer_in_the_county_than_indoors() {
    use crate::ai::leash_in;
    use jane_core::ZoneId;
    let cat = jane_data::catalog();
    let row = |id: &str| cat.combat.unit(def_id(id));
    let bones = row("skeleton");
    let aggro = i64::from(bones.aggro.0);
    assert_eq!(leash_in(ZoneId::County, bones) * 4, aggro * 15);
    for z in [ZoneId::Mine, ZoneId::School, ZoneId::Cellar, ZoneId::House] {
        assert_eq!(leash_in(z, bones) * 2, aggro * 5, "{z:?}");
    }
    let yard = row("yard_bones");
    assert_eq!(leash_in(ZoneId::County, yard) * 4, i64::from(yard.aggro.0) * 30);
    let boss = row("headmaster");
    assert_eq!(leash_in(ZoneId::Mine, boss), i64::from(boss.leash.0));
}
