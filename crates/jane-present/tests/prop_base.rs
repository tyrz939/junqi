//! A solid prop blocks the ground its look stands on, no less and not a row more (`PropDef::base`,
//! the owner's playtests: "can't get close to things from above them", and after the first fix
//! "still, walking up to lots of things from above doesn't let you get close"). Every px of a look
//! stands on the ground `rows_up(height)` rows below it (ART.md §1.1); the rows of ground those
//! reach, counted from the footprint's front edge, are what it is drawn standing on. From above
//! she walks in until her feet meet its `base` rows, so those rows are that ground rounded up to
//! whole cells: never less (she would stand in it), and never a row more (she would stop short).
//!
//! Two kinds keep their whole footprint. A house-sized building (four cells wide or more): nearer,
//! its roof would hide her whole (a cottage's stands 29 px over its footprint's back row, the
//! farmhouse's 57), and what stops her at the back of it is where only her head shows over the
//! ridge. A steeple is a spire she is hidden behind for a step, as a tree's crown hides her. And a thing taller than wide
//! (a gate or a wall across a west or east way, drawn edge on; a tomb lying north to south): it
//! runs or lies along the rows it covers, so all of them are its ground. And what shuts a way
//! until a verb, a blow or a hand clears it (whatever `answers` one, and [`WAYS`]): the generator sets it across a passage
//! either way round, and the solver proves the way shut (C1) by its whole footprint; on its front
//! row alone, one set across a west or east passage two rows deep let her round it behind. And
//! what she pushes or carries, and a gate (the schema refuses them a `base`): to push a barrel
//! south she leans on it from the row above, where on its front row alone she would walk into it
//! (the bots that push crates onto the factory's plates were killed in the corner it left them).

use jane_art::canvas::rows_up;
use jane_art::looks::{self, Family, Rendered};
use jane_art::palette::Ix;
use jane_data::PropDef;

/// What shuts a way and answers no verb: a root wall, a patched wall, a den's mouth. (Whatever
/// answers a verb or a blow is a lock the dungeons may set across a way: it keeps its footprint.)
const WAYS: [&str; 3] = ["den", "root_wall", "weak_wall"];

/// Canvas px a sim cell is drawn at.
const CELL: i32 = 16;

/// How deep a look stands on the ground, px from its footprint's front edge (the canvas's last
/// row, or a hanging's anchor row: it stands on its footprint's back edge, `kit::hung`), over
/// every frame.
fn ground(r: &Rendered) -> i32 {
    let hung = matches!(looks::find(r.name), Some((_, jane_data::Look::Prop(p))) if jane_art::kit::hung(p));
    let mut deepest = 0;
    for (_, c) in &r.set.frames {
        let front = if hung { r.set.ay } else { c.h() };
        for y in 0..c.h() {
            for x in 0..c.w() {
                if c.get(x, y) != Ix::CLEAR {
                    deepest = deepest.max(front - (y + rows_up(i32::from(c.height_at(x, y)))));
                }
            }
        }
    }
    deepest
}

/// Every solid prop row with its look, and whether its look is a building's.
fn solid_looks() -> Vec<(&'static PropDef, Rendered, bool)> {
    let cat = jane_data::catalog();
    let props = looks::family(Family::Prop).expect("the props render");
    let buildings = looks::family(Family::Building).expect("the buildings render");
    let mut out = Vec::new();
    for d in cat.story.props.iter().filter(|d| d.solid && !d.flat) {
        if let Some(r) = props.iter().find(|r| r.sprite == d.sprite) {
            out.push((d, r.clone(), false));
        } else if let Some(r) = buildings.iter().find(|r| r.sprite == d.sprite) {
            out.push((d, r.clone(), true));
        }
    }
    out
}

#[test]
fn a_prop_blocks_at_least_the_ground_it_is_drawn_on() {
    let mut checked = 0;
    let mut short = Vec::new();
    for (d, r, _) in solid_looks().iter().filter(|(d, _, _)| d.base < d.h) {
        let deepest = ground(r);
        if deepest > i32::from(d.base) * CELL {
            short.push(format!("{}: drawn on {deepest} px of ground, blocks {}", d.id, i32::from(d.base) * CELL));
        }
        checked += 1;
    }
    assert!(checked > 80, "the upright props were looked at ({checked})");
    assert!(short.is_empty(), "blocks less than it stands on: {short:#?}");
}

/// The audit the owner asked for: every solid prop's blocking rect's north edge against the
/// ground its look stands on. One more row than that ground is one row she is kept out of from
/// above for nothing.
#[test]
fn a_prop_blocks_no_row_north_of_the_ground_it_is_drawn_on() {
    let all = solid_looks();
    assert!(all.len() > 200, "the solid props were looked at ({})", all.len());
    let mut over = Vec::new();
    for (d, r, building) in &all {
        let need = ((ground(r) + CELL - 1) / CELL).clamp(1, i32::from(d.h));
        let whole = (*building && d.w >= 4)
            || d.h > d.w
            || d.answers.is_some()
            || WAYS.contains(&d.id)
            || d.push
            || d.carry
            || d.gate;
        let want = if whole { i32::from(d.h) } else { need };
        if i32::from(d.base) != want {
            over.push(format!(
                "{} ({}x{}): drawn on {} px of ground, blocks {} rows; wants {want}",
                d.id,
                d.w,
                d.h,
                ground(r),
                d.base
            ));
        }
    }
    assert!(over.is_empty(), "blocks other than the rows it stands on: {over:#?}");
}
