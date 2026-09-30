//! Feet meet where a prop's look meets the ground (`PropDef::feet`, `PropDef::solid_parts`), not
//! its whole cells: the owner's playtest, "approaching from above she still can't get close to a
//! crate, nor to the Lost Property table". The cells stay solid for paths and the solver; the
//! notch behind a prop is entered from the north only. And a rock she lifts is put down clear of
//! her own feet, from whichever side she came.

mod common;

use jane_core::action::Facing;
use jane_core::num::CELL_FX;
use jane_core::{Angle, Fx, Rect, Tile, Vec2, ZoneId};
use jane_sim::interact::Verb;
use jane_sim::{Command, InputFrame, Seat, Sim};

use common::bot::*;
use common::room::Room;

/// A canvas px, a sixteenth of a cell.
const SUB: i32 = CELL_FX / 16;
const HALF: i32 = 3 * 256;

/// Stand her feet at `(x, y)` in `Fx` (tests only: the runtimes are rebuilt).
fn stand(s: &mut Sim, x: i32, y: i32, f: Facing) {
    let (z, id) = (s.state().players[0].zone, s.state().players[0].unit);
    let u = s.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)).expect("her body");
    u.pos = Vec2::new(Fx(x), Fx(y));
    u.facing = f;
    s.rebuild_runtimes();
}

fn walk(s: &mut Sim, dir: Angle, n: u32) {
    (0..n).for_each(|_| step(s, InputFrame::walk(dir)));
}

/// Every kind the owner named, and the like: from the north, down the middle of it, her body box
/// stops within 2 px of the ground its look stands on, never in it; the cells still block paths.
#[test]
fn from_the_north_she_walks_up_to_where_each_prop_meets_the_ground() {
    let cat = jane_data::catalog();
    let kinds = [
        "crate",
        "barrel",
        "chest",
        "boss_chest",
        "luggage_trunk",
        "table",
        "lost_property_desk",
        "study_desk",
        "school_desk",
        "bench",
        "town_stall",
        "rock",
    ];
    let mut far = Vec::new();
    for id in kinds {
        let def = cat.story.prop(cat.story.prop_id(id).unwrap_or_else(|| panic!("a {id} row")));
        let feet = def.feet.unwrap_or_else(|| panic!("{id} has feet"));
        let (px, py) = (10, 20);
        let mut r = Room::new(false);
        r.prop("it", id, px, py, |_| {});
        let mut s = r.build();
        let x = i32::from(px) * CELL_FX + i32::from(def.w) * CELL_FX / 2;
        stand(&mut s, x, (i32::from(py) - 4) * CELL_FX, Facing::South);
        walk(&mut s, Angle::SOUTH, 90);
        let ground = i32::from(py) * CELL_FX + i32::from(feet[1]) * SUB;
        let gap = ground - (me(&s).pos.y.0 + HALF);
        if !(0..=2 * 256).contains(&gap) {
            far.push(format!("{id}: {} px short of its ground", f64::from(gap) / 256.0));
        }
        let rt = s.runtime(ZoneId::County).unwrap();
        let cells = def.solid_rect(i32::from(px), i32::from(py));
        assert!(cells.cells().all(|(x, y)| rt.grid.solid(x, y)), "{id}: paths still see its cells");
    }
    assert!(far.is_empty(), "stopped short of or inside the ground: {far:#?}");
}

/// The table at Lost Property, where the owner found the gap: its row, from its placement.
#[test]
fn the_lost_property_desk_has_feet_well_inside_its_back_row() {
    let cat = jane_data::catalog();
    let def = cat.story.prop(cat.story.prop_id("lost_property_desk").unwrap());
    let [_, y, _, h] = def.feet.expect("feet");
    assert_eq!(i32::from(y) + i32::from(h), i32::from(def.h) * 16);
    assert!(y >= 8, "she comes at least half a cell into its back row ({y} sixteenths)");
}

/// The notch behind a crate is a dead end: along its back she cannot walk through it, from the
/// side she meets its whole footprint, and so a crate across a two-row passage still shuts it.
#[test]
fn the_notch_behind_a_crate_joins_nothing() {
    let mut r = Room::new(false);
    // A passage two rows high (rows 19 and 20) with the crate filling it.
    r.bp.tiles.fill_rect(Rect::new(1, 18, 40, 1), Tile::Wall);
    r.bp.tiles.fill_rect(Rect::new(1, 21, 40, 1), Tile::Wall);
    r.prop("crate", "crate", 20, 19, |_| {});
    let mut s = r.build();
    for y in [19 * CELL_FX + HALF, 19 * CELL_FX + 4 * 256, 20 * CELL_FX + 4 * 256] {
        stand(&mut s, 17 * CELL_FX, y, Facing::East);
        walk(&mut s, Angle::EAST, 120);
        assert!(me(&s).pos.x.0 + HALF <= 20 * CELL_FX, "stopped at its west edge (row at {y})");
    }
}

/// Pushing still works from every side she can touch, the notch behind it included.
#[test]
fn a_crate_is_pushed_from_the_notch_behind_it_and_from_the_side() {
    let mut r = Room::new(false);
    r.prop("crate", "crate", 20, 16, |_| {});
    let mut s = r.build();
    stand(&mut s, 21 * CELL_FX, 13 * CELL_FX, Facing::South);
    walk(&mut s, Angle::SOUTH, 60);
    assert!(me(&s).pos.y.0 > 16 * CELL_FX, "in the notch behind it");
    hold(&mut s, Angle::SOUTH, 30);
    assert_eq!(prop(&s, "crate").cell.y, 17, "pushed south from behind");
    stand(&mut s, 18 * CELL_FX, 18 * CELL_FX + CELL_FX / 2, Facing::East);
    walk(&mut s, Angle::EAST, 30);
    hold(&mut s, Angle::EAST, 30);
    assert_eq!(prop(&s, "crate").cell.x, 21, "pushed east from the side");
}

/// The owner: "picking up rocks seems bugged". From each side she walks up to a rock, is offered
/// it, lifts it, puts it down ahead of her, walks off, and can lift it again.
#[test]
fn a_rock_is_lifted_from_every_side_and_put_down_clear_of_her() {
    let sides = [
        (Angle::SOUTH, Facing::South, (20, 12)),
        (Angle::NORTH, Facing::North, (20, 26)),
        (Angle::EAST, Facing::East, (14, 20)),
        (Angle::WEST, Facing::West, (26, 20)),
    ];
    let mut caught = Vec::new();
    for (dir, facing, (sx, sy)) in sides {
        // Put down after every few px of walking on, so her feet stand everywhere in a cell.
        for on in 0..12 {
            let mut r = Room::new(false);
            r.prop("stone", "rock", 20, 20, |_| {});
            let mut s = r.build();
            place(&mut s, sx, sy, facing);
            walk(&mut s, dir, 90);
            let f = s.view(Seat(0)).unwrap().focus().map(|f| f.verb);
            assert_eq!(f, Some(Verb::PickUp), "{facing:?}: offered the rock");
            cmd(&mut s, Command::Use);
            let stone = prop_id(&s, "stone");
            assert_eq!(me(&s).carrying, Some(stone), "{facing:?}: lifted");
            walk(&mut s, dir, 8 + on);
            cmd(&mut s, Command::Use);
            assert!(me(&s).carrying.is_none(), "{facing:?}: put down");
            assert!(prop(&s, "stone").solid, "{facing:?}: solid again");
            // She is not caught in it: she steps back, and either way across.
            for turn in [32768, 16384, -16384] {
                let at = me(&s).pos;
                walk(&mut s, dir.wrapping_add(turn), 4);
                if me(&s).pos == at {
                    caught.push(format!("{facing:?}, {on} steps on: cannot walk off at {turn}"));
                }
            }
            walk(&mut s, dir, 30);
            let f = s.view(Seat(0)).unwrap().focus().map(|f| f.verb);
            assert_eq!(f, Some(Verb::PickUp), "{facing:?}: offered it again");
            cmd(&mut s, Command::Use);
            assert_eq!(me(&s).carrying, Some(stone), "{facing:?}: lifted again");
        }
    }
    assert!(caught.is_empty(), "caught in the rock she put down: {caught:#?}");
}
