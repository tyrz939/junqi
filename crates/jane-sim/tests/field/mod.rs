//! A field to fight on: every zone a 128 x 64 field of grass (so distances are exact and nothing
//! wanders in), the county's `start` at (10, 10) facing east, a wall down x = 40 from y = 20.
//! Units are stood on it through `state_mut` and a runtime rebuild, or by `Dev(Spawn)`.

#![allow(dead_code)]

use std::sync::Arc;

use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Angle, Blueprint, Cell, Key, Rect, SpellId, Tile, Vec2, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::units::new_unit;
use jane_sim::{
    Blueprints, ClientToken, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput, Unit, UnitId,
};

pub const Z: ZoneId = ZoneId::County;

pub fn field() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        if z == Z {
            bp.tiles.fill_rect(Rect::new(40, 20, 1, 44), Tile::Wall);
        }
        Arc::new(bp)
    });
    Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane")
}

pub fn start_sym() -> jane_core::Sym {
    jane_sim::sym::of_name(jane_data::catalog().name_id("start").unwrap())
}

pub fn spell(id: &str) -> SpellId {
    jane_data::catalog().combat.spell_id(id).unwrap()
}

pub fn body_of(s: &Sim, seat: u8) -> UnitId {
    s.state().players[usize::from(seat)].unit
}

pub fn me(s: &Sim) -> UnitId {
    body_of(s, 0)
}

pub fn zone_of(s: &Sim, id: UnitId) -> ZoneId {
    ZoneId::ALL
        .into_iter()
        .find(|&z| s.state().zone(z).is_some_and(|zs| zs.unit(id).is_some()))
        .expect("a unit somewhere")
}

pub fn unit(s: &Sim, id: UnitId) -> &Unit {
    s.state().zone(zone_of(s, id)).unwrap().unit(id).unwrap()
}

/// Change a unit outside a step, and rebuild the runtimes after (as `state_mut` asks).
pub fn edit(s: &mut Sim, id: UnitId, f: impl FnOnce(&mut Unit)) {
    let z = zone_of(s, id);
    f(s.state_mut().zone_mut(z).unwrap().unit_mut(id).unwrap());
    s.rebuild_runtimes();
}

/// Stand a unit of `def` on a cell of the county, awake.
pub fn spawn(s: &mut Sim, def: &str, x: i32, y: i32) -> UnitId {
    let d = jane_data::catalog().combat.unit_id(def).unwrap();
    let st = s.state_mut();
    let id = st.next.unit();
    let mut u = new_unit(id, None, d, Vec2::centre(x, y), Facing::West, st.tick);
    u.awake = true;
    st.zone_mut(Z).unwrap().insert_unit(u);
    s.rebuild_runtimes();
    id
}

/// Hold a creature where it stands: its stop clock never runs out, so its controller never
/// walks it (it still notices, turns and fights from where it is). For tests about something
/// other than the AI, now that the AI moves things.
pub fn rooted(s: &mut Sim, id: UnitId) {
    edit(s, id, |u| u.stop_until = jane_core::Tick(u32::MAX / 2));
}

pub fn steps(s: &mut Sim, n: u32) {
    for _ in 0..n {
        s.step(&StepInput::IDLE);
    }
}

pub fn walk(s: &mut Sim, n: u32, frame: InputFrame) {
    for _ in 0..n {
        s.step(&StepInput::solo(frame));
    }
}

/// One step with one command from `seat`, her frame `frame`.
pub fn cmd_with(s: &mut Sim, seat: Option<u8>, c: Command, frame: InputFrame) {
    let cmds = [StampedCommand { seat: seat.map(Seat), seq: 0, cmd: c }];
    let mut frames = [InputFrame::IDLE; 4];
    if let Some(i) = seat {
        frames[usize::from(i)] = frame;
    }
    s.step(&StepInput { frames, commands: &cmds });
}

pub fn cmd(s: &mut Sim, seat: Option<u8>, c: Command) {
    cmd_with(s, seat, c, InputFrame::IDLE);
}

/// Aiming along `a`, standing still.
pub fn aim(a: Angle) -> InputFrame {
    InputFrame { aim: Some(a), ..InputFrame::IDLE }
}

pub fn cast(s: &mut Sim, seat: u8, spell_id: &str, frame: InputFrame, on: Option<UnitId>) {
    cmd_with(s, Some(seat), Command::Cast { spell: spell(spell_id), on }, frame);
}

pub fn learn(s: &mut Sim, id: &str) {
    cmd(s, Some(0), Command::Dev(DevOp::Learn(spell(id))));
}

/// Ready to cast again at once: no cooldowns, no GCD, a full well.
pub fn rested(s: &mut Sim, id: UnitId) {
    edit(s, id, |u| {
        u.cooldowns.clear();
        u.gcd_until = jane_core::Tick::ZERO;
        u.mp = jane_sim::units::max_mp(u);
        u.stop_until = jane_core::Tick::ZERO;
    });
}

/// A party of `n`: the host opens, the others sit down.
pub fn party(n: u64) -> Sim {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Open(true));
    for g in 1..n {
        cmd(&mut s, None, Command::Join { who: ClientToken(g) });
    }
    s.drain_events();
    s
}

pub fn events(s: &mut Sim) -> Vec<jane_sim::Event> {
    s.drain_events().to_vec()
}

pub fn damage_to(ev: &[jane_sim::Event], who: UnitId) -> Vec<jane_core::Milli> {
    ev.iter()
        .filter_map(|e| match e.kind {
            EventKind::Damage { unit, amount, .. } if unit == who => Some(amount),
            _ => None,
        })
        .collect()
}
