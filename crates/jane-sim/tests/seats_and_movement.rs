//! Seats, movement, the ring, travel, the clock and freezing, on an open field standing in for
//! the county (so distances are exact) and on the real seed. Carries the movement-only parts of
//! `jane/test/sim.test.ts` and `coop.test.ts` (join, leave, rejoin by token, the story items
//! handed on, the world held still only for one) and ENGINE.md §5's rules.

mod common;

use std::sync::Arc;

use jane_core::blueprint::{Mark, UnitSpawn};
use jane_core::{Angle, Blueprint, Cell, Fx, Key, Rect, Tile, Vec2, ZoneId};
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::state::{Dialogue, Speaker};
use jane_sim::tuning::{ENERGY_MAX, ENERGY_SPRINT};
use jane_sim::{Blueprints, ClientToken, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput, UnitId};

fn start_sym() -> jane_core::Sym {
    jane_sim::sym::of_name(jane_data::catalog().name_id("start").unwrap())
}

/// The seed's blueprints with the county swapped for a 128 x 64 field of grass: a wall down
/// x = 40 from y = 20, a skeleton near the start and one far off, `start` at (10, 10).
fn field() -> Sim {
    let cat = jane_data::catalog();
    let real = common::bps();
    let mut bp = Blueprint::new(ZoneId::County, 128, 64, Tile::Grass);
    bp.tiles.fill_rect(Rect::new(40, 20, 1, 44), Tile::Wall);
    bp.marks.insert(
        Key::Name(cat.story.start.mark),
        Mark { cell: Cell::new(10, 10), facing: Some(jane_core::action::Facing::East) },
    );
    let skeleton = cat.combat.unit_id("skeleton").unwrap();
    for (name, x) in [("near", 20u16), ("far", 100)] {
        let key = bp.local(name);
        bp.units.push(UnitSpawn {
            key,
            def: skeleton,
            cell: Cell::new(x, 10),
            facing: None,
            patrol: Vec::new(),
            phase: 0,
        });
    }
    let zones =
        std::array::from_fn(|i| if i == 0 { Arc::new(bp.clone()) } else { Arc::clone(real.get(ZoneId::ALL[i])) });
    Sim::new_game_with(Blueprints::from_parts(common::SEED, zones), "Jane")
}

fn body(s: &Sim, seat: u8) -> &jane_sim::Unit {
    s.view(Seat(seat)).expect("seat connected").body()
}

fn steps(s: &mut Sim, n: u32, frame: InputFrame) {
    for _ in 0..n {
        s.step(&StepInput::solo(frame));
    }
}

fn cmd(s: &mut Sim, seat: Option<u8>, c: Command) {
    let cmds = [StampedCommand { seat: seat.map(Seat), seq: 0, cmd: c }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
}

fn by_key(s: &Sim, name: &str) -> UnitId {
    let sym = s.state().syms.find(name).unwrap();
    s.state().zone(ZoneId::County).unwrap().units.iter().find(|u| u.key == Some(sym)).unwrap().id
}

#[test]
fn new_game_stands_her_at_the_start_mark() {
    let mut s = common::new_game();
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.zone(), ZoneId::County);
    assert_eq!(v.size(), (2000, 2000));
    assert!(!v.indoor());
    let start = s.blueprint(ZoneId::County).marks[&Key::Name(jane_data::catalog().story.start.mark)];
    let (cx, cy) = v.body().pos.cell();
    assert!((cx - i32::from(start.cell.x)).abs() <= 8 && (cy - i32::from(start.cell.y)).abs() <= 8);
    assert!(v.body().awake && v.body().alive);
    assert_eq!(v.units_in(Rect::new(cx, cy, 1, 1)).count(), 1);
    assert!(s.runtime(ZoneId::House).is_none(), "only live zones have runtimes");
    let ev = s.drain_events().to_vec();
    assert!(
        ev.iter().any(|e| e.kind == EventKind::Zone { zone: ZoneId::County, first: true } && e.to == Some(Seat(0)))
    );
    assert!(s.drain_events().is_empty());
    // The start kit.
    let me = s.state().players[0].clone();
    assert!(me.bag.iter().flatten().count() >= 1);
    assert!(me.bar.iter().flatten().count() >= 1);
    assert_eq!(s.state().quests.active.len(), jane_data::catalog().story.start.quests.len());
}

#[test]
fn walking_is_one_px_a_tick_along_the_stick() {
    let mut s = field();
    let p0 = body(&s, 0).pos;
    assert_eq!(p0, Vec2::centre(10, 10));
    steps(&mut s, 60, InputFrame::walk(Angle::EAST));
    assert_eq!(body(&s, 0).pos, Vec2::new(Fx(p0.x.0 + 60 * 256), p0.y));
    assert_eq!(body(&s, 0).facing, jane_core::action::Facing::East);
    assert_eq!(body(&s, 0).energy, ENERGY_MAX);
    // Half a stick is half a pace; under the dead zone is standing still.
    let p1 = body(&s, 0).pos;
    steps(&mut s, 10, InputFrame { mv_mag: 64, ..InputFrame::walk(Angle::SOUTH) });
    assert_eq!(body(&s, 0).pos, Vec2::new(p1.x, Fx(p1.y.0 + 10 * (256 * 64 / 127))));
    assert_eq!(body(&s, 0).facing, jane_core::action::Facing::South);
    let p2 = body(&s, 0).pos;
    steps(&mut s, 10, InputFrame { mv_mag: 6, ..InputFrame::walk(Angle::WEST) });
    assert_eq!(body(&s, 0).pos, p2);
    // A diagonal moves both ways at once, and the view says it moved.
    steps(&mut s, 1, InputFrame::walk(Angle::from_degrees(45)));
    let b = body(&s, 0);
    assert!(b.pos.x.0 > p2.x.0 && b.pos.y.0 > p2.y.0);
    let (cx, cy) = b.pos.cell();
    let uv = s.view(Seat(0)).unwrap().units_in(Rect::new(cx, cy, 1, 1)).next().unwrap();
    assert!(uv.moved && uv.prev_pos == p2);
}

#[test]
fn sprint_spends_energy_until_it_locks_and_it_unlocks_only_when_full() {
    let mut s = field();
    let run = InputFrame { sprint: true, ..InputFrame::walk(Angle::SOUTH) };
    let y0 = body(&s, 0).pos.y.0;
    steps(&mut s, 10, run);
    assert_eq!(body(&s, 0).pos.y.0, y0 + 10 * 512, "run is 2 px a tick");
    assert_eq!(body(&s, 0).energy.0, ENERGY_MAX.0 - 10 * ENERGY_SPRINT.0);
    // Round the field so she never meets a wall: 200 ticks empty it.
    for i in 0..190 {
        let dir = Angle::from_degrees((i * 7) % 360);
        steps(&mut s, 1, InputFrame { sprint: true, ..InputFrame::walk(dir) });
    }
    assert_eq!(body(&s, 0).energy.0, 0);
    assert!(body(&s, 0).energy_locked);
    // Locked: a sprint is a walk, and walking restores.
    let y = body(&s, 0).pos.y.0;
    steps(&mut s, 10, InputFrame { sprint: true, ..InputFrame::walk(Angle::NORTH) });
    assert_eq!(body(&s, 0).pos.y.0, y - 10 * 256);
    assert!(body(&s, 0).energy_locked);
    steps(&mut s, 190, InputFrame::IDLE);
    assert_eq!(body(&s, 0).energy, ENERGY_MAX);
    assert!(!body(&s, 0).energy_locked);
}

#[test]
fn a_wall_stops_her_flush_and_she_slides_along_it() {
    let mut s = field();
    // Down to y = 30 (in the wall's rows), then east into x = 40.
    steps(&mut s, 160, InputFrame::walk(Angle::SOUTH));
    steps(&mut s, 400, InputFrame::walk(Angle::EAST));
    let b = body(&s, 0).pos;
    assert_eq!(b.x.0, 40 * 2048 - 768, "flush: the wall's edge less half a body");
    // Pushing into it at an angle slides her north along it.
    steps(&mut s, 10, InputFrame::walk(Angle::from_degrees(-30)));
    let c = body(&s, 0).pos;
    assert_eq!(c.x, b.x);
    assert!(c.y.0 < b.y.0);
}

#[test]
fn the_ring_wakes_what_is_near_and_sleeps_what_is_far() {
    let mut s = field();
    let (near, far) = (by_key(&s, "near"), by_key(&s, "far"));
    let z = || ZoneId::County;
    let awake = |s: &Sim, id| s.state().zone(z()).unwrap().unit(id).unwrap().awake;
    assert!(awake(&s, near) && !awake(&s, far));
    let rt = s.runtime(z()).unwrap();
    assert!(rt.awake_units.contains(&near) && !rt.awake_units.contains(&far));
    assert_eq!(rt.grid.occupants(20, 10), 1, "a woken unit stands on its cell");
    assert_eq!(rt.grid.occupants(100, 10), 0, "a sleeper does not");
    // East until the far one (804 px) is inside the ring (384 px from her block's middle) and
    // the near one (164 px) is not: 480 px puts her block's middle at 568.
    steps(&mut s, 480, InputFrame::walk(Angle::EAST));
    assert!(awake(&s, far));
    assert!(!awake(&s, near), "left behind, it sleeps");
    let rt = s.runtime(z()).unwrap();
    assert_eq!(rt.grid.occupants(100, 10), 1);
    assert_eq!(rt.grid.occupants(20, 10), 0);
}

#[test]
fn travel_takes_her_through_and_drops_the_empty_zone() {
    let mut s = field();
    s.drain_events();
    let start = start_sym();
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: start }));
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.zone(), ZoneId::House);
    assert!(v.indoor());
    assert!(s.runtime(ZoneId::County).is_none(), "nobody is in the county");
    assert!(s.state().zone(ZoneId::County).unwrap().unit(s.state().players[0].unit).is_none());
    let ev = s.drain_events().to_vec();
    assert!(ev.iter().any(|e| e.kind == EventKind::Zone { zone: ZoneId::House, first: true }));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    assert_eq!(s.view(Seat(0)).unwrap().zone(), ZoneId::County);
    assert!(s.drain_events().iter().any(|e| e.kind == EventKind::Zone { zone: ZoneId::County, first: false }));
    // Arrived at the mark: the field's start.
    assert_eq!(body(&s, 0).pos, Vec2::centre(10, 10));
    assert_eq!(s.state().players[0].last_mark, start);
}

#[test]
fn seats_join_leave_and_come_back_to_their_own_body() {
    let cat = jane_data::catalog();
    let mut s = field();
    // Closed: nobody else sits down.
    cmd(&mut s, None, Command::Join { who: ClientToken(5) });
    assert_eq!(s.state().players.len(), 1);
    cmd(&mut s, Some(0), Command::Open(true));
    assert!(s.state().open);
    s.drain_events();
    cmd(&mut s, None, Command::Join { who: ClientToken(5) });
    assert_eq!(s.state().party_size(), 2);
    let guest = s.state().players[1].clone();
    assert_eq!(guest.seat, Seat(1));
    assert!(s.drain_events().iter().any(|e| e.kind == EventKind::Party { connected: 2 }));
    // Beside the host on the nearest free cell: the ring's first corner.
    assert_eq!(body(&s, 1).pos.cell(), (9, 9));
    // Only the host opens and closes the world.
    cmd(&mut s, Some(1), Command::Open(false));
    assert!(s.state().open);
    // With company nothing pauses.
    s.state_mut().players[0].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::None, read: None });
    assert!(!s.frozen());
    s.state_mut().players[0].dialogue = None;

    let story_qty = |bag: &[Option<jane_core::Stack>]| -> u32 {
        bag.iter().flatten().filter(|st| cat.combat.item(st.item).story).map(|st| u32::from(st.qty)).sum()
    };
    let story = story_qty(&guest.bag[..]);
    let host_before = story_qty(&s.state().players[0].bag[..]);
    cmd(&mut s, Some(1), Command::Leave);
    assert_eq!(s.state().party_size(), 1);
    let parked = s.state().players[1].parked.as_ref().expect("her body waits").id;
    assert_eq!(parked, guest.unit);
    assert!(s.state().zone(ZoneId::County).unwrap().unit(guest.unit).is_none());
    // What the story needs went to the host (her bag has room).
    assert_eq!(story_qty(&s.state().players[0].bag[..]), host_before + story);
    assert_eq!(story_qty(&s.state().players[1].bag[..]), 0);
    let ev = s.drain_events().to_vec();
    assert!(ev.iter().any(|e| e.kind == EventKind::Party { connected: 1 }));
    if story > 0 {
        assert!(ev.iter().any(|e| e.kind == EventKind::Toast(ToastKind::LeftWhatMattered) && e.to == Some(Seat(0))));
    }
    // Back by her token: the same seat, the same body.
    cmd(&mut s, None, Command::Join { who: ClientToken(5) });
    assert_eq!(s.state().players.len(), 2);
    assert_eq!(s.state().players[1].unit, guest.unit);
    assert!(s.state().players[1].parked.is_none());
    // Four seats, and a fifth is refused.
    for t in 6..9 {
        cmd(&mut s, None, Command::Join { who: ClientToken(t) });
    }
    assert_eq!(s.state().players.len(), 4);
    assert_eq!(s.state().party_size(), 4);
    let ids: Vec<_> = s.state().zone(ZoneId::County).unwrap().units.iter().map(|u| u.id).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "units stay in id order");
}

#[test]
fn alone_the_world_holds_still_while_she_talks() {
    let mut s = field();
    let t = s.state().tick;
    s.state_mut().players[0].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::None, read: None });
    assert!(s.frozen());
    let r = s.step(&StepInput::solo(InputFrame::walk(Angle::EAST)));
    assert!(!r.ran);
    assert_eq!(s.state().tick, t);
    assert_eq!(body(&s, 0).pos, Vec2::centre(10, 10));
    // Commands still land while frozen: closing the box lets time go on.
    cmd(&mut s, Some(0), Command::CloseDialogue);
    assert!(!s.frozen());
    assert_eq!(s.state().tick.0, t.0 + 1);
    assert_eq!(s.state().frame, 2);
}

#[test]
fn a_clock_row_fires_once_however_many_are_at_the_table() {
    let count = |guests: u64| {
        let mut s = field();
        cmd(&mut s, Some(0), Command::Open(true));
        for g in 0..guests {
            cmd(&mut s, None, Command::Join { who: ClientToken(10 + g) });
        }
        cmd(&mut s, Some(0), Command::Dev(DevOp::Time { hour: 20 }));
        s.drain_events();
        steps(&mut s, 7200, InputFrame::IDLE);
        assert_eq!(s.state().hour(), 21);
        s.drain_events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Toast(ToastKind::Text(_))) && e.to.is_none() && e.in_zone.is_none())
            .count()
    };
    let alone = count(0);
    assert!(alone >= 1, "the bell at nine");
    assert_eq!(count(2), alone);
}

#[test]
fn the_bar_binds_unbinds_and_swaps() {
    let mut s = field();
    let bar0 = s.state().players[0].bar;
    cmd(&mut s, Some(0), Command::BarSwap { a: 0, b: 5 });
    assert_eq!(s.state().players[0].bar[5], bar0[0]);
    assert_eq!(s.state().players[0].bar[0], bar0[5]);
    cmd(&mut s, Some(0), Command::Unbind { slot: 5 });
    assert_eq!(s.state().players[0].bar[5], None);
    let spell = jane_data::catalog().combat.spell_id("melee_player").unwrap();
    cmd(&mut s, Some(0), Command::Bind { slot: 2, to: jane_sim::input::BarSlotWire::Spell(spell) });
    assert_eq!(s.state().players[0].bar[2], Some(jane_data::BarSlot::Spell(spell)));
    // A slot past the bar is ignored.
    cmd(&mut s, Some(0), Command::Unbind { slot: 200 });
}
