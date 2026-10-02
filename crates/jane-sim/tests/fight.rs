//! The player's side of a fight (PLAY-PLAN §2.1): hard targets carried in the frame and validated
//! by the sim, bolts that curve onto their target, every Word cast at its prop, auto-attack,
//! casts with weight (completion, interrupts, nothing spent, the queue, half-speed walking), click
//! to move (the fog rule, the no-push rule, WASD and a hit cancel it), and the hash with all of it.

mod common;
mod field;

use field::*;
use jane_core::action::School;
use jane_core::angle::bearing;
use jane_core::{Angle, Milli, Tick, Vec2};
use jane_sim::event::{EventKind, SpellError};
use jane_sim::state::WalkThen;
use jane_sim::{Command, DevOp, Goto, Hit, InputFrame, PropId, Seat, Sim, StampedCommand, StepInput, TargetRef, UnitId};

/// A prop of `def` on a cell of the county, made at runtime (as `tests/assist.rs` makes them).
fn put_prop(s: &mut Sim, def: &str, x: u16, y: u16) -> PropId {
    let cat = jane_data::catalog();
    let d = cat.story.prop_id(def).unwrap();
    let st = s.state_mut();
    let id = st.next.prop();
    let key = st.syms.intern(&format!("test_{def}_{}", id.get()));
    st.zone_mut(Z).unwrap().props.push(jane_sim::Prop {
        id,
        key,
        def: d,
        spawn: None,
        cell: jane_core::Cell::new(x, y),
        solid: cat.story.prop(d).solid,
        hidden: false,
        locked: false,
        used: false,
        on: false,
        loot: jane_sim::state::LootState::AsSpawned,
        under_done: false,
        regrow: None,
        night: jane_sim::state::NightState::AsSpawned,
    });
    s.rebuild_runtimes();
    id
}

fn prop(s: &Sim, id: PropId) -> &jane_sim::Prop {
    s.state().zone(Z).unwrap().props.iter().find(|p| p.id == id).unwrap()
}

fn at_unit(u: UnitId) -> InputFrame {
    InputFrame { target: Some(TargetRef::Unit(u)), ..InputFrame::IDLE }
}

fn at_prop(p: PropId) -> InputFrame {
    InputFrame { target: Some(TargetRef::Prop(p)), aim: Some(Angle::WEST), ..InputFrame::IDLE }
}

/// Step `n` frames holding `frame`, gathering the events.
fn hold(s: &mut Sim, n: u32, frame: InputFrame) -> Vec<jane_sim::Event> {
    let mut ev = Vec::new();
    for _ in 0..n {
        s.step(&StepInput::solo(frame));
        ev.extend_from_slice(s.drain_events());
    }
    ev
}

fn press(s: &mut Sim, spell_id: &str, frame: InputFrame) -> Vec<jane_sim::Event> {
    press_only(s, 0, spell_id, frame, None);
    s.drain_events().to_vec()
}

fn fight(s: &Sim) -> &jane_sim::state::Fight {
    &s.state().players[0].fight
}

fn count(ev: &[jane_sim::Event], f: impl Fn(&EventKind) -> bool) -> usize {
    ev.iter().filter(|e| f(&e.kind)).count()
}

fn stunned() -> jane_core::EffectId {
    jane_data::catalog().combat.effect_id("stunned").unwrap()
}

/// Ready, with every Word, and nobody's blows land on her.
fn witch() -> Sim {
    let mut s = field();
    for w in ["icebolt", "fireball", "explosion", "spark", "grow", "repair"] {
        learn(&mut s, w);
    }
    s.drain_events();
    s
}

// --- targets -------------------------------------------------------------------------------------

#[test]
fn a_target_is_what_the_frame_says_if_the_sim_agrees() {
    let mut s = witch();
    let foe = spawn(&mut s, "skeleton", 20, 10);
    rooted(&mut s, foe);
    let torch = put_prop(&mut s, "torch_blue", 14, 14);
    let crate_ = put_prop(&mut s, "crate", 14, 4);
    hold(&mut s, 1, at_unit(foe));
    assert_eq!(fight(&s).target, Some(TargetRef::Unit(foe)));
    hold(&mut s, 1, at_prop(torch));
    assert_eq!(fight(&s).target, Some(TargetRef::Prop(torch)), "a prop that answers a Word");
    hold(&mut s, 1, at_prop(crate_));
    assert_eq!(fight(&s).target, None, "a crate answers nothing: no target");
    hold(&mut s, 1, at_unit(UnitId::new(99_999).unwrap()));
    assert_eq!(fight(&s).target, None, "nobody by that id here");
    let m = me(&s);
    hold(&mut s, 1, at_unit(m));
    assert_eq!(fight(&s).target, None, "not herself");
    // Too far, then dead.
    edit(&mut s, foe, |u| u.pos = Vec2::centre(100, 10));
    hold(&mut s, 1, at_unit(foe));
    assert_eq!(fight(&s).target, None, "two screens off");
    edit(&mut s, foe, |u| u.pos = Vec2::centre(20, 10));
    hold(&mut s, 1, at_unit(foe));
    assert_eq!(fight(&s).target, Some(TargetRef::Unit(foe)));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Kill));
    hold(&mut s, 1, at_unit(foe));
    assert_eq!(fight(&s).target, None, "a corpse is no target");
}

#[test]
fn tab_walks_the_foes_in_front_nearest_first() {
    let mut s = witch();
    let near = spawn(&mut s, "rat", 16, 10);
    let far = spawn(&mut s, "rat", 24, 11);
    let behind = spawn(&mut s, "rat", 4, 10);
    for u in [near, far, behind] {
        rooted(&mut s, u);
    }
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.tab_order(Angle::EAST), vec![near, far], "behind her is not in front");
    assert_eq!(v.tab_next(Angle::EAST, None, false), Some(near));
    assert_eq!(v.tab_next(Angle::EAST, Some(TargetRef::Unit(near)), false), Some(far));
    assert_eq!(v.tab_next(Angle::EAST, Some(TargetRef::Unit(far)), false), Some(near), "round again");
    assert_eq!(v.tab_next(Angle::EAST, Some(TargetRef::Unit(near)), true), Some(far), "back");
    assert_eq!(v.tab_order(Angle::WEST), vec![behind]);
}

// --- bolts at a target ---------------------------------------------------------------------------

/// Where a bolt at a rat walking south across her line lands: targeted, it curves on; aimed
/// along where the rat stood, it misses.
fn shot_at_a_walker(free: bool) -> bool {
    let mut s = witch();
    let rat = spawn(&mut s, "rat", 24, 10);
    rooted(&mut s, rat);
    let hp = unit(&s, rat).hp;
    let line = bearing(unit(&s, me(&s)).pos, unit(&s, rat).pos);
    let frame = InputFrame { target: Some(TargetRef::Unit(rat)), aim: Some(line), free, ..InputFrame::IDLE };
    press(&mut s, "icebolt", frame);
    for _ in 0..140 {
        s.step(&StepInput::solo(frame));
        if !s.state().zone(Z).unwrap().projectiles.is_empty() {
            edit(&mut s, rat, |u| u.pos.y.0 += 200);
        }
    }
    unit(&s, rat).hp < hp
}

#[test]
fn a_targeted_bolt_curves_onto_a_moving_foe_and_a_free_one_flies_straight() {
    assert!(shot_at_a_walker(false), "the bolt cast at it follows it");
    assert!(!shot_at_a_walker(true), "aimed freely where it stood, it misses");
}

#[test]
fn a_bolt_at_a_target_still_strikes_a_wall_and_needs_sight_to_start() {
    let mut s = witch();
    // The field's wall runs down x = 40 from y = 20; stand west of it and target a rat east.
    let m = me(&s);
    edit(&mut s, m, |u| u.pos = Vec2::centre(36, 30));
    let rat = spawn(&mut s, "rat", 44, 30);
    rooted(&mut s, rat);
    let ev = press(&mut s, "icebolt", at_unit(rat));
    assert!(
        ev.iter().any(|e| matches!(e.kind, EventKind::CastFailed { why: SpellError::NotInLos, .. })),
        "no sight, no cast"
    );
    assert!(fight(&s).cast.is_none());
}

/// Every Word reaches its prop from her target alone, the aim pointing the other way.
#[test]
fn every_word_cast_at_its_prop_lands_there() {
    for (word, def, x, y) in [
        ("icebolt", "torch_blue", 20, 14),
        ("spark", "socket_dead", 20, 14),
        ("explosion", "cracked_wall_v", 20, 13),
        ("fireball", "cold_hearth", 20, 14),
        ("grow", "bud", 11, 11),
        ("repair", "broken_steps", 11, 11),
    ] {
        let mut s = witch();
        let p = put_prop(&mut s, def, x, y);
        let m = me(&s);
        rested(&mut s, m);
        press(&mut s, word, at_prop(p));
        hold(&mut s, 150, at_prop(p));
        assert!(prop(&s, p).on, "{word} at a {def}");
    }
}

/// A verb at a prop beyond its reach walks her there first, then lands.
#[test]
fn a_verb_at_a_far_prop_walks_her_to_it() {
    let mut s = witch();
    let bud = put_prop(&mut s, "bud", 22, 12);
    press(&mut s, "grow", at_prop(bud));
    assert!(matches!(fight(&s).walk.as_ref().map(|w| w.then), Some(WalkThen::Cast { .. })));
    hold(&mut s, 400, at_prop(bud));
    assert!(prop(&s, bud).on, "she walked to it and it grew");
}

// --- auto-attack ---------------------------------------------------------------------------------

#[test]
fn a_swing_at_a_foe_in_reach_keeps_swinging_until_it_dies_or_she_leaves() {
    let mut s = witch();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let foe = spawn(&mut s, "foreman", 11, 10);
    rooted(&mut s, foe);
    let ev = press(&mut s, "melee_player", at_unit(foe));
    assert_eq!(fight(&s).auto, Some(foe));
    let mut swings = count(&ev, |k| matches!(k, EventKind::Swing { .. }));
    // Walking (back and forth beside it) does not stop it.
    for i in 0..16 {
        let dir = if i % 2 == 0 { Angle::NORTH } else { Angle::SOUTH };
        let ev = hold(&mut s, 10, InputFrame { mv_dir: dir, mv_mag: 30, ..at_unit(foe) });
        swings += count(&ev, |k| matches!(k, EventKind::Swing { unit, .. } if *unit == me(&s)));
    }
    assert!(swings >= 3, "{swings} swings in 2.7 s");
    assert_eq!(fight(&s).auto, Some(foe));
    // A cast pauses it.
    let rat = spawn(&mut s, "rat", 30, 10);
    rooted(&mut s, rat);
    let m = me(&s);
    rested(&mut s, m);
    press(&mut s, "icebolt", InputFrame { aim: Some(Angle::EAST), free: true, ..at_unit(foe) });
    assert!(fight(&s).cast.is_some());
    let ev = hold(&mut s, 50, at_unit(foe));
    assert_eq!(count(&ev, |k| matches!(k, EventKind::Swing { unit, .. } if *unit == me(&s))), 0, "no swing mid-cast");
    // Out of reach: it stops.
    edit(&mut s, foe, |u| u.pos = Vec2::centre(20, 10));
    hold(&mut s, 20, at_unit(foe));
    assert_eq!(fight(&s).auto, None, "out of reach");
}

#[test]
fn a_swing_with_no_target_takes_the_nearest_foe_in_front() {
    let mut s = witch();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let foe = spawn(&mut s, "skeleton", 11, 10);
    rooted(&mut s, foe);
    press(&mut s, "melee_player", InputFrame { aim: Some(Angle::EAST), ..InputFrame::IDLE });
    assert_eq!(fight(&s).auto, Some(foe), "the soft target");
    cmd(&mut s, Some(0), Command::Dev(DevOp::Kill));
    hold(&mut s, 2, InputFrame::IDLE);
    assert_eq!(fight(&s).auto, None, "it died");
}

#[test]
fn a_right_click_on_a_foe_walks_into_reach_and_swings() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let foe = spawn(&mut s, "skeleton", 20, 10);
    rooted(&mut s, foe);
    cmd(&mut s, Some(0), Command::Goto(Goto::Unit(foe)));
    assert!(matches!(fight(&s).walk.as_ref().map(|w| w.then), Some(WalkThen::Attack(_))));
    let ev = hold(&mut s, 300, at_unit(foe));
    assert!(count(&ev, |k| matches!(k, EventKind::Swing { unit, .. } if *unit == me(&s))) >= 2);
    assert!(fight(&s).walk.is_none());
    // With a bolt that reaches, a right-click only targets: no walk.
    let mut s = witch();
    let foe = spawn(&mut s, "skeleton", 20, 10);
    rooted(&mut s, foe);
    cmd(&mut s, Some(0), Command::Goto(Goto::Unit(foe)));
    assert!(fight(&s).walk.is_none() && fight(&s).auto.is_none(), "a caster in range only targets");
}

// --- casts ---------------------------------------------------------------------------------------

#[test]
fn a_cast_builds_then_lands_and_pays_only_then() {
    let mut s = witch();
    let rat = spawn(&mut s, "rat", 24, 10);
    rooted(&mut s, rat);
    let mp = unit(&s, me(&s)).mp;
    let t0 = s.state().tick;
    let ev = press(&mut s, "icebolt", at_unit(rat));
    let pc = fight(&s).cast.expect("building");
    assert_eq!(pc.done, t0.after(Tick(60)), "Icebolt casts in 1.0 s");
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::CastBegin { .. })));
    assert_eq!(unit(&s, me(&s)).mp, mp, "nothing spent yet");
    assert_eq!(unit(&s, me(&s)).gcd_until, t0.after(Tick(60)), "the 1 s GCD starts with it");
    assert!(s.state().zone(Z).unwrap().projectiles.is_empty());
    let mut ev = hold(&mut s, 58, at_unit(rat));
    let before = unit(&s, me(&s)).mp;
    assert!(fight(&s).cast.is_some());
    ev.extend(hold(&mut s, 2, at_unit(rat)));
    assert!(fight(&s).cast.is_none());
    assert_eq!(count(&ev, |k| matches!(k, EventKind::Cast { .. })), 1);
    let spent = before.0 - unit(&s, me(&s)).mp.0;
    assert!((13_900..=14_000).contains(&spent), "spent at the end: {spent}");
    let _ = mp;
    assert_eq!(unit(&s, me(&s)).stop_until, Tick::ZERO, "never rooted after");
}

#[test]
fn a_stun_stops_a_cast_and_nothing_is_spent_but_plain_damage_does_not() {
    let mut s = witch();
    let rat = spawn(&mut s, "rat", 24, 10);
    rooted(&mut s, rat);
    let mp = unit(&s, me(&s)).mp;
    press(&mut s, "fireball", at_unit(rat));
    let m = me(&s);
    let blow = |status| Hit {
        to: m,
        amount: Milli(3000),
        school: School::Physical,
        from: None,
        crit: false,
        status,
    };
    let b = blow(None);
    s.queue_hit(Z, b);
    hold(&mut s, 2, at_unit(rat));
    assert!(fight(&s).cast.is_some(), "a plain blow does not stop it");
    let b = blow(Some(stunned()));
    s.queue_hit(Z, b);
    let ev = hold(&mut s, 2, at_unit(rat));
    assert!(fight(&s).cast.is_none(), "a stun does");
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::CastStopped { .. })));
    assert_eq!(unit(&s, me(&s)).mp, mp, "an interrupted cast costs nothing");
    assert!(unit(&s, me(&s)).gcd_until <= s.state().tick, "and hands back the GCD");
    hold(&mut s, 120, at_unit(rat));
    assert!(s.state().zone(Z).unwrap().projectiles.is_empty(), "it never landed");
}

#[test]
fn esc_stops_everything_and_out_of_range_at_release_lands_nothing() {
    let mut s = witch();
    let rat = spawn(&mut s, "rat", 24, 10);
    rooted(&mut s, rat);
    let mp = unit(&s, me(&s)).mp;
    press(&mut s, "icebolt", at_unit(rat));
    cmd(&mut s, Some(0), Command::Halt);
    assert!(fight(&s).cast.is_none());
    let m = me(&s);
    rested(&mut s, m);
    press(&mut s, "icebolt", at_unit(rat));
    edit(&mut s, rat, |u| u.pos = Vec2::centre(36, 10));
    let ev = hold(&mut s, 70, at_unit(rat));
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::CastFailed { why: SpellError::TooFar, .. })));
    assert_eq!(unit(&s, me(&s)).mp, mp, "range is checked at release, and costs nothing");
}

#[test]
fn a_press_in_the_last_200_ms_waits_for_her() {
    let mut s = witch();
    let rat = spawn(&mut s, "rat", 24, 10);
    rooted(&mut s, rat);
    press(&mut s, "icebolt", at_unit(rat));
    hold(&mut s, 20, at_unit(rat));
    let ev = press(&mut s, "spark", at_unit(rat));
    assert!(fight(&s).queued.is_none(), "too early to queue");
    assert!(ev.iter().any(|e| matches!(e.kind, EventKind::CastFailed { why: SpellError::OnGcd, .. })));
    hold(&mut s, 29, at_unit(rat));
    press(&mut s, "spark", at_unit(rat));
    assert!(fight(&s).queued.is_some(), "inside the window it waits");
    let ev = hold(&mut s, 12, at_unit(rat));
    let casts: Vec<_> = ev
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Cast { spell: sp, .. } => Some(sp),
            _ => None,
        })
        .collect();
    assert_eq!(casts, vec![spell("icebolt"), spell("spark")], "the bolt lands and the spark goes at once");
}

#[test]
fn she_walks_at_half_speed_while_casting() {
    let walked = |casting: bool| {
        let mut s = witch();
        if casting {
            press(&mut s, "explosion", InputFrame { aim: Some(Angle::EAST), ..InputFrame::IDLE });
            assert!(fight(&s).cast.is_some());
        }
        let x0 = unit(&s, me(&s)).pos.x.0;
        hold(&mut s, 30, InputFrame::walk(Angle::SOUTH));
        let p = unit(&s, me(&s)).pos;
        (p.x.0 - x0, p.y.0)
    };
    let (_, slow) = walked(true);
    let (_, fast) = walked(false);
    let y0 = Vec2::centre(10, 10).y.0;
    let (slow, fast) = (slow - y0, fast - y0);
    assert!(slow > 0 && slow * 2 <= fast + 64 && slow * 2 >= fast - 64, "{slow} vs {fast}");
}

// --- click to move -------------------------------------------------------------------------------

#[test]
fn a_right_click_walks_her_there_round_a_wall() {
    let mut s = field();
    let m = me(&s);
    edit(&mut s, m, |u| u.pos = Vec2::centre(36, 40));
    // Everything seen, so it may plan round the wall's top.
    for w in s.state_mut().zone_mut(Z).unwrap().fog.iter_mut() {
        *w = u32::MAX;
    }
    let to = Vec2::centre(46, 40);
    cmd(&mut s, Some(0), Command::Goto(Goto::Ground(to)));
    hold(&mut s, 900, InputFrame::IDLE);
    let at = unit(&s, me(&s)).pos;
    assert!(fight(&s).walk.is_none(), "arrived");
    assert!((at.x.0 - to.x.0).abs() <= 1024 && (at.y.0 - to.y.0).abs() <= 1024, "{at:?}");
}

#[test]
fn a_click_walk_never_routes_through_the_unseen() {
    let mut s = field();
    let geom_cells = 8;
    {
        let zs = s.state_mut().zone_mut(Z).unwrap();
        for w in zs.fog.iter_mut() {
            *w = 0;
        }
    }
    // She has seen only the blocks round her, 8 cells each.
    hold(&mut s, 10, InputFrame::IDLE);
    cmd(&mut s, Some(0), Command::Goto(Goto::Ground(Vec2::centre(120, 10))));
    let w = fight(&s).walk.clone().expect("walking");
    let v = s.view(Seat(0)).unwrap();
    let width = v.size().0;
    assert!(!w.cells.is_empty());
    for c in &w.cells {
        let (x, y) = ((c.0 % width) as i32, (c.0 / width) as i32);
        assert!(v.seen(x, y), "planned through unseen ({x}, {y})");
    }
    let last = w.cells.last().unwrap();
    assert!(((last.0 % width) as i32) < 120 - geom_cells, "it did not plan to the unseen goal");
}

#[test]
fn a_click_walk_routes_round_a_crate_and_never_pushes_it() {
    let mut s = field();
    for w in s.state_mut().zone_mut(Z).unwrap().fog.iter_mut() {
        *w = u32::MAX;
    }
    let c = put_prop(&mut s, "crate", 14, 9);
    let cell = prop(&s, c).cell;
    cmd(&mut s, Some(0), Command::Goto(Goto::Ground(Vec2::centre(20, 10))));
    hold(&mut s, 600, InputFrame::IDLE);
    assert_eq!(prop(&s, c).cell, cell, "the crate never moved");
    let at = unit(&s, me(&s)).pos;
    assert!(at.x.0 > Vec2::centre(18, 10).x.0, "she got past it: {at:?}");
}

#[test]
fn a_held_move_or_a_hit_ends_a_click_walk() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Goto(Goto::Ground(Vec2::centre(30, 10))));
    hold(&mut s, 5, InputFrame::IDLE);
    assert!(fight(&s).walk.is_some());
    hold(&mut s, 1, InputFrame::walk(Angle::NORTH));
    assert!(fight(&s).walk.is_none(), "WASD takes her back");
    cmd(&mut s, Some(0), Command::Goto(Goto::Ground(Vec2::centre(30, 10))));
    hold(&mut s, 5, InputFrame::IDLE);
    let b = Hit { to: me(&s), amount: Milli(2000), school: School::Physical, from: None, crit: false, status: None };
    s.queue_hit(Z, b);
    hold(&mut s, 2, InputFrame::IDLE);
    assert!(fight(&s).walk.is_none(), "a hit stops the walk");
}

#[test]
fn a_right_click_on_a_person_walks_over_and_talks() {
    let mut s = field();
    let dog = spawn(&mut s, "dog", 20, 10);
    rooted(&mut s, dog);
    cmd(&mut s, Some(0), Command::Goto(Goto::Unit(dog)));
    assert!(matches!(fight(&s).walk.as_ref().map(|w| w.then), Some(WalkThen::Use(_))));
    for _ in 0..400 {
        s.step(&StepInput::IDLE);
        if s.state().players[0].dialogue.is_some() {
            break;
        }
    }
    assert!(s.state().players[0].dialogue.is_some(), "she reached the dog and spoke");
}

// --- lockstep ------------------------------------------------------------------------------------

/// A scripted session with all of it, as frames and commands; played twice, and once more from
/// a save taken mid-cast and mid-walk, and once from the frames through their wire bytes.
fn session() -> Vec<([InputFrame; 4], Vec<StampedCommand>)> {
    let mut s = witch();
    let a = spawn(&mut s, "rat", 24, 10);
    let b = spawn(&mut s, "skeleton", 20, 16);
    let torch = put_prop(&mut s, "torch_blue", 16, 6);
    let mut tape = Vec::new();
    let mut push = |f: InputFrame, c: Option<Command>| {
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = f;
        tape.push((frames, c.map(|cmd| StampedCommand { seat: Some(Seat(0)), seq: 0, cmd }).into_iter().collect()));
    };
    let ice = Command::Cast { spell: spell("icebolt"), on: None };
    push(at_unit(a), Some(ice));
    for _ in 0..55 {
        push(InputFrame { mv_dir: Angle::SOUTH, mv_mag: 60, ..at_unit(a) }, None);
    }
    push(at_unit(a), Some(Command::Cast { spell: spell("spark"), on: None }));
    for _ in 0..30 {
        push(at_unit(a), None);
    }
    push(at_prop(torch), Some(ice));
    for _ in 0..80 {
        push(at_prop(torch), None);
    }
    push(at_unit(b), Some(Command::Cast { spell: spell("melee_player"), on: None }));
    push(at_unit(b), Some(Command::Goto(Goto::Unit(b))));
    for _ in 0..120 {
        push(at_unit(b), None);
    }
    push(InputFrame::IDLE, Some(Command::Goto(Goto::Ground(Vec2::centre(6, 20)))));
    for _ in 0..60 {
        push(InputFrame::IDLE, None);
    }
    push(InputFrame::IDLE, Some(Command::Halt));
    for _ in 0..20 {
        push(InputFrame::IDLE, None);
    }
    let _ = (a, b, torch, &mut s);
    tape
}

fn session_sim() -> Sim {
    let mut s = witch();
    spawn(&mut s, "rat", 24, 10);
    spawn(&mut s, "skeleton", 20, 16);
    put_prop(&mut s, "torch_blue", 16, 6);
    s
}

#[test]
fn the_fight_hashes_the_same_saved_or_not_through_the_wire_or_not() {
    let tape = session();
    let play = |s: &mut Sim, from: usize, to: usize, wire: bool| {
        for (frames, cmds) in &tape[from..to] {
            let frames = if wire { frames.map(|f| InputFrame::from_bytes(f.to_bytes())) } else { *frames };
            s.step(&StepInput { frames, commands: cmds });
        }
    };
    let mut a = session_sim();
    let mut b = session_sim();
    let mut hashes = Vec::new();
    for i in 0..tape.len() {
        play(&mut a, i, i + 1, false);
        play(&mut b, i, i + 1, true);
        assert_eq!(a.hash(), b.hash(), "frame {i}");
        hashes.push(a.hash());
    }
    // Saved mid-cast (frame 20) and mid-walk (frame 300), loaded, continued.
    for at in [20, 300] {
        let mut first = session_sim();
        play(&mut first, 0, at, false);
        if at == 20 {
            assert!(first.state().players[0].fight.cast.is_some(), "mid-cast");
        }
        let bytes = first.save();
        let mut resumed = Sim::from_save_with(&bytes, first.blueprints().clone()).expect("the save loads");
        assert_eq!(resumed.hash(), first.hash());
        play(&mut resumed, at, tape.len(), false);
        assert_eq!(resumed.hash(), *hashes.last().unwrap(), "saved at {at}");
    }
}
