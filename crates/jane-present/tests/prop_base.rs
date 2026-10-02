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
//! until a verb, a blow or a hand clears it (whatever `answers` one, and a row marked `way`): the generator sets it across a passage
//! either way round, and the solver proves the way shut (C1) by its whole footprint; on its front
//! row alone, one set across a west or east passage two rows deep let her round it behind. And
//! what she pushes or carries, and a gate (the schema refuses them a `base`): to push a barrel
//! south she leans on it from the row above, where on its front row alone she would walk into it
//! (the bots that push crates onto the factory's plates were killed in the corner it left them).

use std::fmt::Write as _;

use jane_art::canvas::rows_up;
use jane_art::looks::{self, Family, Rendered};
use jane_art::palette::Ix;
use jane_data::PropDef;

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
        let whole = (*building && d.w >= 4) || d.h > d.w || d.keeps_width();
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

// --- feet ------------------------------------------------------------------------------------

/// Where `data/prop_feet.json` lives.
const FEET: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/prop_feet.json");

/// Sixteenths of a cell feet may leave open at either side and hold their footprint's width.
const SIDE_SLACK: i32 = jane_data::FEET_SIDE_SLACK;

/// A px this high or lower touches the ground: a crate's foot, a table's legs, a trunk's base, a
/// lamp post's foot (canvas px of true height).
const CONTACT: i32 = 2;

/// What stands on that contact up to this high is the thing's own body over the ground (a
/// crate's lid, a table's top, a counter's): her body meets it. Higher is over her feet (a
/// tree's crown, a lamp's head, a sign's board, a stall's awning, a cupboard's top).
const BODY: i32 = 20;

/// The least depth feet take, when its contact is at least this wide (sixteenths: 2 px).
const MIN_DEEP: i32 = 4;

/// Whether a prop keeps its whole width to feet: what shuts a way or is part of a puzzle the
/// solver proves by its cells (a pushed or carried thing, a gate, whatever answers a verb or a
/// blow, a row marked `way`: `PropDef::keeps_width`). Its feet leave at most [`SIDE_SLACK`] open at either side, so none of them
/// side by side lets her slip between, and the notch behind it is a dead end
/// (`PropDef::solid_parts`).
fn keeps_width(d: &PropDef) -> bool {
    d.keeps_width()
}

/// Where a look meets the ground, in sixteenths of a cell (canvas px) from its footprint's
/// top-left, over every frame (the AO shadow is not the thing): across, the columns of its px
/// that touch the ground ([`CONTACT`]); deep, the ground under its px in those columns up to
/// [`BODY`] high, run down to its footprint's front edge, within its `base` rows. A thing that
/// [`keeps_width`] spans all but [`SIDE_SLACK`] of its footprint across. `None` when that is its
/// `base` rows whole (nothing to gain), or it keeps them whole: a house (its walls stand on every
/// side of its footprint, the back one under its roof) and a hanging (the wall's).
fn feet_of(d: &PropDef, r: &Rendered, building: bool) -> Option<[u8; 4]> {
    let hung = matches!(looks::find(r.name), Some((_, jane_data::Look::Prop(p))) if jane_art::kit::hung(p));
    if (building && d.w >= 4) || hung {
        return None;
    }
    let (fw, fh) = (i32::from(d.w) * CELL, i32::from(d.h) * CELL);
    let top = (i32::from(d.h) - i32::from(d.base)) * CELL;
    let px = |f: &dyn Fn(i32, i32, i32, i32)| {
        for (_, c) in &r.set.frames {
            let back = c.h() - fh;
            for y in 0..c.h() {
                for x in 0..c.w() {
                    if c.get(x, y).is_opaque() {
                        let h = i32::from(c.height_at(x, y));
                        f(x, y + rows_up(h) - back, h, y);
                    }
                }
            }
        }
    };
    let (x0, x1) = (std::cell::Cell::new(fw), std::cell::Cell::new(0));
    px(&|x, _, h, _| {
        if h <= CONTACT {
            x0.set(x0.get().min(x));
            x1.set(x1.get().max(x + 1));
        }
    });
    let (mut x0, mut x1) = (x0.get(), x1.get());
    if x1 <= x0 {
        return None;
    }
    let y0 = std::cell::Cell::new(fh - 1);
    px(&|x, gy, h, _| {
        if h <= BODY && (x0..x1).contains(&x) {
            y0.set(y0.get().min(gy.clamp(top, fh - 1)));
        }
    });
    // Drawn standing up with no top (a trunk, a post, a rock), it is as deep as its contact is
    // wide, up to [`MIN_DEEP`]: never a line a sixteenth deep.
    let y0 = y0.get().min(fh - (x1 - x0).min(MIN_DEEP)).max(top);
    if keeps_width(d) {
        (x0, x1) = (x0.clamp(0, SIDE_SLACK), x1.clamp(fw - SIDE_SLACK, fw));
    }
    if x0 == 0 && x1 == fw && y0 == top {
        return None;
    }
    Some([x0 as u8, y0 as u8, (x1 - x0) as u8, (fh - y0) as u8])
}

/// The owner's playtest: "approaching from above, she still can't get close to a crate, nor to
/// the Lost Property table". Feet collide with where each look meets the ground (`PropDef::feet`,
/// `jane_sim::units`), and that is written from the art: this checks `data/prop_feet.json`
/// against it. Re-bless after an intended change to a look or a footprint with
/// `JANE_BLESS=1 cargo test -p jane-present --test prop_base`.
#[test]
fn a_prop_s_feet_are_where_its_look_meets_the_ground() {
    let all = solid_looks();
    let want: Vec<(&str, [u8; 4])> =
        all.iter().filter_map(|(d, r, building)| feet_of(d, r, *building).map(|f| (d.id, f))).collect();
    if std::env::var_os("JANE_BLESS").is_some() {
        let mut s = String::from("{\n");
        for (i, (id, [x, y, w, h])) in want.iter().enumerate() {
            let comma = if i + 1 < want.len() { "," } else { "" };
            writeln!(s, "  \"{id}\": [{x}, {y}, {w}, {h}]{comma}").expect("a String");
        }
        s.push_str("}\n");
        std::fs::write(FEET, s).expect("data/prop_feet.json");
        return;
    }
    let mut wrong = Vec::new();
    for (d, r, building) in &all {
        let f = feet_of(d, r, *building);
        if d.feet != f {
            wrong.push(format!("{}: data {:?}, drawn {:?}", d.id, d.feet, f));
        }
    }
    assert!(want.len() > 100, "most solid props stand on less than their cells ({})", want.len());
    assert!(wrong.is_empty(), "prop_feet.json is not what the art draws (bless it: see above): {wrong:#?}");
}
