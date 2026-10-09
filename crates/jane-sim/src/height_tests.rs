//! Height where only a `Ctx` reaches (MAP.md §3.4): a perch row (`holds: level`) on a row made
//! for the test, as no content has one yet. The rest of R1's tests are `tests/height.rs`.

use std::sync::Arc;

use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Blueprint, Cell, Key, Tick, Tile, Vec2, ZoneId};

use crate::ai::tick_ai_with;
use crate::blueprints::Blueprints;
use crate::ctx::PartySnap;
use crate::height::{level_of, terraces};
use crate::sim::Sim;
use crate::state::CombatState;
use crate::units::new_unit;

const Z: ZoneId = ZoneId::County;

fn world() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            return Arc::new(terraces::blueprint(z));
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        Arc::new(bp)
    });
    Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane")
}

/// A perch row on the plateau with her below its ledge: it never hops, never takes the ladder or
/// the stair round, holds at the top and evades; the same row without `holds` hops after her.
#[test]
fn a_perch_row_never_leaves_its_plateau() {
    let cat = jane_data::catalog();
    let def = cat.combat.unit_id("quarryman").unwrap();
    let base = *cat.combat.unit(def);
    for perch in [true, false] {
        let row: &'static jane_data::UnitDef = Box::leak(Box::new(jane_data::UnitDef { holds_level: perch, ..base }));
        let mut s = world();
        s.state.players[0].god = true;
        let her = s.state.players[0].unit;
        s.state.zone_mut(Z).unwrap().unit_mut(her).unwrap().pos = Vec2::centre(12, 19);
        let id = s.state.next.unit();
        let mut u = new_unit(id, None, def, Vec2::centre(12, 8), Facing::South, s.state.tick);
        u.awake = true;
        u.target = Some(her);
        u.combat = CombatState::Combat;
        s.state.zone_mut(Z).unwrap().insert_unit(u);
        s.rebuild_runtimes();
        let (mut left, mut evaded) = (false, false);
        for _ in 0..60 * 8 {
            let snap = PartySnap::of(&s.state);
            s.with_ctx(Z, None, &snap, false, |cx| {
                cx.rt.paths_this_tick = 0;
                tick_ai_with(cx, id, row);
                crate::feel::step_knocks(cx);
            });
            s.state.tick = s.state.tick.after(Tick(1));
            let u = s.state.zone(Z).unwrap().unit(id).unwrap();
            left |= level_of(&s.rts[Z.index()].as_ref().unwrap().grid, u.pos) != 2;
            evaded |= u.combat == CombatState::Evade;
        }
        assert_eq!(left, !perch, "perch {perch}: left its plateau {left}");
        assert_eq!(evaded, perch, "perch {perch}: a perch holds and lets go; a chaser came down");
    }
}
