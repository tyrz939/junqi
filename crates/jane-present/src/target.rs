//! Her target, as this client holds it (PLAY-PLAN §2.1, PRESENTATION.md §4.2). The client chooses
//! (a left click, Tab, RB and LB), carries it in every `InputFrame` ([`Input::target`]) and lets
//! it go; the sim validates it each tick and the view says what it kept. Nothing here is state the
//! sim reads but the frame: a target chosen in a frame is the frame's.
//!
//! - **Left click** ([`pick_target`]): the unit under the cursor (not her), else a prop under it
//!   that answers a Word (a blue torch, a cracked wall, a socket, a bud); over open ground,
//!   nothing: the target is let go.
//! - **Tab, RB** (Shift-Tab, LB back): `View::tab_next`, facing her aim (else her facing).
//! - **Settling** ([`settle`]): a target the sim would not keep (dead, gone, too far) is let go;
//!   with none, the sim's soft target (the foe she swings at, the thing her cast is building
//!   at) is taken up, so an attack with no target chooses one.
//! - **Right click** ([`goto_at`]): `Goto::Unit` on someone, `Goto::Prop` on something she can
//!   use, `Goto::Ground` anywhere else.

use alloc::vec::Vec;
use jane_core::{Angle, Rect, Vec2};
use jane_sim::input::{Goto, TargetRef};
use jane_sim::view::View;

use crate::input::{Input, pick};

/// The cells round a prop's footprint the cursor may be in and still be over it: its look stands
/// up to this many cells above its footprint.
const PROP_TALL_CELLS: i32 = 2;

/// The props the cursor at `at` is over, nearest the cursor first: inside the footprint, or the
/// cells its look stands in above it.
fn props_under<'a>(view: &View<'a>, at: Vec2) -> impl Iterator<Item = &'a jane_sim::state::Prop> + 'a {
    let (cx, cy) = at.cell();
    let cat = jane_data::catalog();
    let mut v: Vec<_> = view
        .props_in(Rect::new(cx - 4, cy - 1, 9, 4 + PROP_TALL_CELLS))
        .filter(|p| !p.hidden)
        .filter(move |p| {
            let def = cat.story.prop(p.def);
            let r = jane_sim::interact::footprint(def, p);
            let tall = Rect::new(r.x, r.y - PROP_TALL_CELLS, r.w, r.h + PROP_TALL_CELLS);
            tall.contains(cx, cy)
        })
        .collect();
    v.sort_by_key(|p| {
        let def = cat.story.prop(p.def);
        let c = jane_sim::light::prop_centre(def, p);
        (jane_core::num::dist_sq(c, at), p.id)
    });
    v.into_iter()
}

/// What a left click at `at` (a world point) targets: the unit under the cursor (not her), else a
/// prop under it she may target; `None` over open ground.
pub fn pick_target(view: &View<'_>, at: Vec2) -> Option<TargetRef> {
    let u = pick(view, at);
    if u != view.body().id {
        let t = TargetRef::Unit(u);
        return view.target_valid(t).then_some(t);
    }
    props_under(view, at).map(|p| TargetRef::Prop(p.id)).find(|&t| view.target_valid(t))
}

/// Can she use this prop (a door, a fire, a chest, a bench, a cupboard, a sign, a thing to carry)?
fn usable(view: &View<'_>, p: &jane_sim::state::Prop) -> bool {
    let def = jane_data::catalog().story.prop(p.def);
    let s = view.prop_spawn(p);
    p.locked
        || def.rest
        || def.carry
        || def.bench
        || def.store
        || s.is_some_and(|s| s.to.is_some() || s.talk.is_some() || s.use_list.is_some())
        || (!p.used && !view.prop_loot(p).is_empty())
}

/// What a right click at `at` asks (`Command::Goto`).
pub fn goto_at(view: &View<'_>, at: Vec2) -> Goto {
    let u = pick(view, at);
    if u != view.body().id {
        return Goto::Unit(u);
    }
    if let Some(p) = props_under(view, at).find(|p| usable(view, p)) {
        return Goto::Prop(p.id);
    }
    Goto::Ground(at)
}

/// The direction Tab looks: her aim, else her facing.
pub fn tab_dir(view: &View<'_>, aim: Option<Angle>) -> Angle {
    aim.unwrap_or_else(|| jane_sim::units::facing_angle(view.body().facing))
}

/// Tab (RB; `back`: Shift-Tab, LB): the next foe in front. Nobody there keeps what she had.
pub fn tab(input: &mut Input, view: &View<'_>, aim: Option<Angle>, back: bool) {
    if let Some(u) = view.tab_next(tab_dir(view, aim), input.target, back) {
        input.target = Some(TargetRef::Unit(u));
        input.dropped = None;
    }
}

/// A left click at `at`: target what is under it; over open ground, let go.
pub fn click(input: &mut Input, view: &View<'_>, at: Vec2) {
    match pick_target(view, at) {
        Some(t) => {
            input.target = Some(t);
            input.dropped = None;
        }
        None => clear(input),
    }
}

/// Let the target go (Esc, a click on open ground).
pub fn clear(input: &mut Input) {
    if let Some(t) = input.target.take() {
        input.dropped = Some(t);
    }
}

/// Does the sim say she is doing something Esc would stop (a cast building, swings, a walk)?
pub fn busy(view: &View<'_>) -> bool {
    let f = view.fight();
    f.cast.is_some() || f.auto.is_some() || f.walk.is_some()
}

/// Each frame, before the frame goes out: let go of a target the sim would not keep, and with
/// none, take up the sim's soft target (unless it is the one just let go and the sim has not yet
/// heard).
pub fn settle(input: &mut Input, view: &View<'_>) {
    if let Some(t) = input.target
        && !view.target_valid(t)
    {
        input.target = None;
    }
    let f = view.fight();
    let soft = f.auto.map(TargetRef::Unit).or_else(|| f.cast.and_then(|c| c.at));
    if input.dropped.is_some() && soft != input.dropped {
        input.dropped = None;
    }
    if input.target.is_none()
        && let Some(s) = soft
        && Some(s) != input.dropped
        && view.target_valid(s)
    {
        input.target = Some(s);
    }
}

#[cfg(test)]
mod tests {
    use jane_core::Fx;

    use super::*;

    #[test]
    fn open_ground_targets_nothing_and_a_right_click_there_walks() {
        let sim = jane_sim::Sim::new_game(1, "Jane");
        let v = sim.view(jane_sim::Seat(0)).expect("seat 0");
        let me = v.body();
        let far = Vec2 { x: Fx(me.pos.x.0 + 256 * 40), y: me.pos.y };
        assert_eq!(pick_target(&v, far), None);
        assert_eq!(goto_at(&v, far), Goto::Ground(far));
        let mut input = Input::new();
        input.target = Some(TargetRef::Unit(me.id));
        click(&mut input, &v, far);
        assert_eq!(input.target, None, "a click on open ground lets the target go");
        assert_eq!(input.dropped, Some(TargetRef::Unit(me.id)));
        // Her own body is no target: settling drops it.
        input.target = Some(TargetRef::Unit(me.id));
        settle(&mut input, &v);
        assert_eq!(input.target, None);
        assert!(!busy(&v));
    }

    #[test]
    fn tab_finds_the_foe_in_front_and_a_click_on_it_targets_it() {
        use jane_sim::input::{Command, DevOp, InputFrame, StampedCommand, StepInput};
        let mut sim = jane_sim::Sim::new_game(1, "Jane");
        let skel = jane_data::catalog().combat.unit_id("skeleton").expect("a skeleton row");
        let cmds = [StampedCommand { seat: Some(jane_sim::Seat(0)), seq: 0, cmd: Command::Dev(DevOp::Spawn(skel)) }];
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        let v = sim.view(jane_sim::Seat(0)).expect("seat 0");
        let foe = v
            .units_in(Rect::new(v.body().pos.cell().0, v.body().pos.cell().1 - 3, 8, 7))
            .map(|u| u.unit)
            .find(|u| u.id != v.body().id && v.target_hostile(TargetRef::Unit(u.id)))
            .expect("the skeleton three cells east");
        let mut input = Input::new();
        tab(&mut input, &v, Some(Angle::EAST), false);
        assert_eq!(input.target, Some(TargetRef::Unit(foe.id)), "east, in front");
        // Again: the only foe, round to itself.
        tab(&mut input, &v, Some(Angle::EAST), true);
        assert_eq!(input.target, Some(TargetRef::Unit(foe.id)));
        // Facing away there is nobody: what she had is kept.
        let mut other = Input::new();
        tab(&mut other, &v, Some(Angle::WEST), false);
        assert_eq!(other.target, None);
        // A click on its chest targets it; settling keeps it.
        let chest = Vec2 { x: foe.pos.x, y: Fx(foe.pos.y.0 - 256 * 6) };
        let mut clicked = Input::new();
        click(&mut clicked, &v, chest);
        assert_eq!(clicked.target, Some(TargetRef::Unit(foe.id)));
        settle(&mut clicked, &v);
        assert_eq!(clicked.target, Some(TargetRef::Unit(foe.id)));
        assert_eq!(goto_at(&v, chest), Goto::Unit(foe.id), "a right click there walks to it and swings");
    }
}
