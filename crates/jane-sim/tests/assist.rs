//! Aim assist (ARCHITECTURE.md §5.4, §12 defaults): the frame carries the raw aim and the
//! profile; the sim resolves them at cast time, so the pad and the mouse play the same game and
//! a replay of the recorded frames lands the same bolts. New with the port (the TS had no
//! assist; PORT.md §7 P4 "assist replays exactly").

mod common;
mod field;

use field::*;
use jane_core::angle::iatan2;
use jane_core::{Angle, Milli, Sfc32, Tick, Vec2};
use jane_sim::state::Assisted;
use jane_sim::tuning::{ASSIST_MOUSE, ASSIST_PAD};
use jane_sim::{AssistProfile, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput, UnitId};

fn frame(raw: Angle, assist: AssistProfile) -> InputFrame {
    InputFrame { aim: Some(raw), assist, ..InputFrame::IDLE }
}

fn bearing_to(s: &Sim, from: UnitId, to: UnitId) -> Angle {
    let (a, b) = (unit(s, from).pos, unit(s, to).pos);
    iatan2(b.y.0 - a.y.0, b.x.0 - a.x.0)
}

/// The heading of the bolt a cast of icebolt along `raw` under `p` flies, from a fresh ready
/// caster.
fn bolt_heading(s: &mut Sim, raw: Angle, p: AssistProfile) -> Angle {
    rested(s, me(s));
    let before = s.state().zone(Z).unwrap().projectiles.len();
    cast(s, 0, "icebolt", frame(raw, p), None);
    let zs = s.state().zone(Z).unwrap();
    assert_eq!(zs.projectiles.len(), before + 1, "the cast went off");
    zs.projectiles.last().unwrap().heading
}

fn toward(raw: Angle, d: i32, magnet: i32) -> Angle {
    raw.wrapping_add((d * magnet).div_euclid(1000))
}

fn fresh() -> Sim {
    let mut s = field();
    learn(&mut s, "icebolt");
    s
}

/// §5.4: inside the cone and outside the snap, the pad pulls the aim `magnet` of the way; inside
/// the snap it takes the bearing. The mouse's cone is narrower; `Off` is raw.
#[test]
fn the_pad_pulls_the_mouse_barely_and_off_not_at_all() {
    let mut s = fresh();
    let foe = spawn(&mut s, "skeleton", 20, 12);
    let bearing = bearing_to(&s, me(&s), foe);
    let d = Angle::EAST.diff(bearing);
    assert!(d > i32::from(ASSIST_MOUSE.cone.0) && d < i32::from(ASSIST_PAD.cone.0) && d > i32::from(ASSIST_PAD.snap.0));
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), toward(Angle::EAST, d, 350));
    let now = s.state().tick;
    assert_eq!(
        s.state().players[0].assist,
        Some(Assisted { unit: foe, until: Tick(now.0 - 1).after(ASSIST_PAD.sticky_ticks) })
    );
    let mut s = fresh();
    let foe = spawn(&mut s, "skeleton", 20, 12);
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Mouse), Angle::EAST, "outside the mouse's cone");
    assert_eq!(s.state().players[0].assist, None);
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Off), Angle::EAST);

    // Nearly on it: the pad snaps, the mouse (a finer snap) pulls a fifth of the way.
    let raw = bearing.wrapping_add(-500);
    assert_eq!(bolt_heading(&mut s, raw, AssistProfile::Pad), bearing_to(&s, me(&s), foe));
    assert_eq!(bolt_heading(&mut s, raw, AssistProfile::Mouse), toward(raw, 500, 200));
    assert_eq!(bolt_heading(&mut s, raw, AssistProfile::Off), raw);
    // Out of the spell's reach, nothing is a candidate.
    let mut s = fresh();
    let far = spawn(&mut s, "skeleton", 40, 12);
    let raw = bearing_to(&s, me(&s), far).wrapping_add(-900);
    assert_eq!(bolt_heading(&mut s, raw, AssistProfile::Pad), raw);
}

/// §5.4 rule: assist never targets a friend, a prop or a corpse.
#[test]
fn never_a_friend_a_passer_by_or_a_corpse() {
    let mut s = party(2);
    learn(&mut s, "icebolt");
    let guest = body_of(&s, 1);
    edit(&mut s, guest, |u| u.pos = Vec2::centre(20, 10));
    let hen = spawn(&mut s, "hen", 16, 11);
    let corpse = spawn(&mut s, "skeleton", 18, 9);
    edit(&mut s, corpse, |u| {
        u.alive = false;
        u.hp = Milli::ZERO;
    });
    // A creature of the county's that is friendly to her is no target either.
    let dog = spawn(&mut s, "dog", 14, 10);
    for p in [AssistProfile::Pad, AssistProfile::Mouse] {
        for raw in
            [Angle::EAST, bearing_to(&s, me(&s), hen), bearing_to(&s, me(&s), corpse), bearing_to(&s, me(&s), dog)]
        {
            let raw = raw.wrapping_add(200);
            assert_eq!(bolt_heading(&mut s, raw, p), raw, "{p:?}");
        }
    }
    assert_eq!(s.state().players[0].assist, None);
    // With an enemy in the cone too, it is the enemy the aim is pulled to, never her.
    let foe = spawn(&mut s, "rat", 20, 13);
    let d = Angle::EAST.diff(bearing_to(&s, me(&s), foe));
    assert!(d > 0 && d < i32::from(ASSIST_PAD.cone.0));
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), toward(Angle::EAST, d, 350));
    assert_eq!(s.state().players[0].assist.map(|a| a.unit), Some(foe));
}

/// A prop of `def` on a cell of the county, `on` or off (made at runtime, as the tests of the AI
/// make them).
fn put_prop(s: &mut Sim, def: &str, x: u16, y: u16, on: bool) -> jane_sim::PropId {
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
        on,
        loot: jane_sim::state::LootState::AsSpawned,
        under_done: false,
    });
    s.rebuild_runtimes();
    id
}

fn prop_on(s: &Sim, id: jane_sim::PropId) -> bool {
    let zs = s.state().zone(Z).unwrap();
    zs.props[zs.prop_ix(id).unwrap() as usize].on
}

/// §5.4: a bolt aimed at a prop that answers its school flies raw when the prop is nearer than
/// the best candidate: a brazier beside a lurker is lit, not missed. Lit already, or behind the
/// lurker, or of another school, the aim is pulled as ever; the view shows the same.
#[test]
fn a_bolt_aimed_at_a_prop_it_would_light_is_not_pulled_off_it() {
    let mut s = fresh();
    learn(&mut s, "fireball");
    let lurker = spawn(&mut s, "lurker", 20, 12);
    rooted(&mut s, lurker);
    let d = Angle::EAST.diff(bearing_to(&s, me(&s), lurker));
    assert!(d > i32::from(ASSIST_PAD.snap.0) && d < i32::from(ASSIST_PAD.cone.0));
    let fireball = |s: &mut Sim, p: AssistProfile| {
        rested(s, me(s));
        let before = s.state().zone(Z).unwrap().projectiles.len();
        cast(s, 0, "fireball", frame(Angle::EAST, p), None);
        let zs = s.state().zone(Z).unwrap();
        assert_eq!(zs.projectiles.len(), before + 1, "the cast went off");
        zs.projectiles.last().unwrap().heading
    };
    // A brazier on the line, nearer than the lurker: the reticle and the bolt stay on it.
    let brazier = put_prop(&mut s, "brazier", 15, 10, false);
    let shown = s.view(Seat(0)).unwrap().assisted_aim(&frame(Angle::EAST, AssistProfile::Pad), spell("fireball"));
    assert_eq!(shown, Some(Angle::EAST));
    assert_eq!(fireball(&mut s, AssistProfile::Pad), Angle::EAST);
    assert_eq!(s.state().players[0].assist, None, "nothing made sticky");
    steps(&mut s, 30);
    assert!(prop_on(&s, brazier), "lit");
    // Lit, it no longer answers: the aim is pulled to the lurker again.
    assert_eq!(fireball(&mut s, AssistProfile::Pad), toward(Angle::EAST, d, 350));
    // Behind the lurker, it does not hold the aim; nor does an icebolt, which it does not answer.
    let mut s = fresh();
    learn(&mut s, "fireball");
    let lurker = spawn(&mut s, "lurker", 20, 12);
    rooted(&mut s, lurker);
    put_prop(&mut s, "brazier", 30, 10, false);
    assert_eq!(fireball(&mut s, AssistProfile::Pad), toward(Angle::EAST, d, 350));
    put_prop(&mut s, "brazier", 15, 10, false);
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), toward(Angle::EAST, d, 350));
}

/// The sticky unit stays a candidate `slack` outside the cone for `sticky_ticks`.
#[test]
fn the_sticky_unit_holds_a_little_outside_the_cone_for_a_while() {
    let mut s = fresh();
    let foe = spawn(&mut s, "skeleton", 20, 14);
    rooted(&mut s, foe);
    let bearing = bearing_to(&s, me(&s), foe);
    let d = Angle::EAST.diff(bearing);
    assert!(d > i32::from(ASSIST_PAD.cone.0) && d <= i32::from(ASSIST_PAD.cone.0) + i32::from(ASSIST_PAD.slack.0));
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), Angle::EAST, "outside the cone");
    assert_eq!(bolt_heading(&mut s, bearing, AssistProfile::Pad), bearing);
    assert_eq!(s.state().players[0].assist.map(|a| a.unit), Some(foe));
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), toward(Angle::EAST, d, 350), "sticky");
    steps(&mut s, 40);
    assert_eq!(bolt_heading(&mut s, Angle::EAST, AssistProfile::Pad), Angle::EAST, "let go");
}

/// §11 `View::assisted_aim`: the reticle's angle is the one the cast flies along; spells that do
/// not hurt what they are aimed at (a world spell, a heal) are never assisted.
#[test]
fn the_view_shows_where_the_bolt_will_go() {
    let mut s = fresh();
    learn(&mut s, "repair");
    let foe = spawn(&mut s, "skeleton", 20, 12);
    let raw = Angle::EAST.wrapping_add(300);
    for p in [AssistProfile::Off, AssistProfile::Pad, AssistProfile::Mouse] {
        let body = me(&s);
        rested(&mut s, body);
        let v = s.view(Seat(0)).unwrap();
        let shown = v.assisted_aim(&frame(raw, p), spell("icebolt")).unwrap();
        assert_eq!(v.assisted_aim(&frame(raw, p), spell("repair")), Some(raw));
        assert_eq!(v.assisted_aim(&InputFrame { aim: None, ..frame(raw, p) }, spell("icebolt")), None);
        assert_eq!(bolt_heading(&mut s, raw, p), shown, "{p:?}");
    }
    let _ = foe;
}

/// PORT.md §7 P4 "assist replays exactly": a tape of casts under each profile, recorded as the
/// seven-byte frames the wire carries, re-simulated from the bytes, lands every bolt the same
/// way (the same hash every 60 frames). The assisted casts did bend under `Pad` and `Mouse`
/// and never under `Off`.
#[test]
fn assist_replays_exactly() {
    let setup = || {
        let mut s = fresh();
        learn(&mut s, "spark");
        for (i, (x, y)) in [(20, 12), (22, 7), (18, 16), (24, 11), (19, 4), (26, 14)].into_iter().enumerate() {
            let def = ["skeleton", "rat", "bandit"][i % 3];
            let id = spawn(&mut s, def, x, y);
            edit(&mut s, id, |u| {
                u.strength = 4000;
                u.hp = jane_sim::units::max_hp(u);
            });
            rooted(&mut s, id);
        }
        s
    };
    let mut rng = Sfc32::seeded(77, 1);
    let mut a = setup();
    let mut tape: Vec<([u8; 7], Option<Command>)> = Vec::new();
    let mut bent = [0u32; 3];
    let mut casts = [0u32; 3];
    for f in 0..3000u32 {
        let p = [AssistProfile::Off, AssistProfile::Pad, AssistProfile::Mouse][(f / 200 % 3) as usize];
        let raw = Angle::EAST.wrapping_add(rng.range(-5000, 5000));
        let fr =
            InputFrame { mv_dir: Angle::SOUTH, mv_mag: 0, aim: Some(raw), sprint: false, use_held: false, assist: p };
        let c = match rng.below(12) {
            0 => Some(Command::Cast { spell: spell("icebolt"), on: None }),
            1 => Some(Command::Cast { spell: spell("spark"), on: None }),
            2 => Some(Command::Dev(DevOp::Mp(150))),
            _ => None,
        };
        tape.push((fr.to_bytes(), c));
        let before = a.state().next.proj;
        step_with(&mut a, fr, c);
        let zs = a.state().zone(Z).unwrap();
        if let Some(b) = zs.projectiles.iter().find(|b| b.id.get() > before) {
            casts[p as usize] += 1;
            bent[p as usize] += u32::from(b.heading != raw);
        }
    }
    assert!(casts.iter().all(|&n| n >= 5), "{casts:?}");
    assert_eq!(bent[AssistProfile::Off as usize], 0);
    assert!(bent[AssistProfile::Pad as usize] > 0 && bent[AssistProfile::Mouse as usize] > 0, "{bent:?}");

    let mut b = setup();
    let mut hashes = Vec::new();
    let mut a2 = setup();
    for (f, (bytes, c)) in tape.iter().enumerate() {
        step_with(&mut b, InputFrame::from_bytes(*bytes), *c);
        step_with(&mut a2, InputFrame::from_bytes(*bytes), *c);
        if f % 60 == 59 {
            hashes.push(b.hash());
            assert_eq!(a2.hash(), b.hash(), "frame {f}");
        }
    }
    assert_eq!(b.hash(), a.hash());
    assert_eq!(b.state(), a.state());
    assert_eq!(hashes.len(), 50);
}

fn step_with(s: &mut Sim, fr: InputFrame, c: Option<Command>) {
    let cmds: Vec<StampedCommand> =
        c.into_iter().map(|cmd| StampedCommand { seat: Some(Seat(0)), seq: 0, cmd }).collect();
    let mut frames = [InputFrame::IDLE; 4];
    frames[0] = fr;
    s.step(&StepInput { frames, commands: &cmds });
}
