//! A dungeon's sanctuary (PLAN.md §2.6 *Leash*, the owner, 2026-10-07): nothing follows her into
//! its rest room or out over the threshold of a way out, so the blueprint names both, for every
//! dungeon on every seed tried. Sanctuary is no wall: it only turns what chases her, so it can
//! never seal a room or a way out (the solver and C1 to C13 judge the same tiles and props as
//! before). What stands guard in it would never fight there at all, so nothing hostile is put
//! down inside it.

use jane_data::{MissionNodeKind, catalog};
use jane_world::dungeon::build;

#[test]
fn every_rest_room_and_way_out_is_sanctuary() {
    let cat = catalog();
    for m in cat.dungeons.missions {
        for seed in 1..=3 {
            let b = build(m.zone, seed);
            assert!(b.info.errors.is_empty(), "{} seed {seed}: {:?}", m.id, b.info.errors);
            let bp = &b.blueprint;
            let inside = |x: i32, y: i32| bp.sanctuary.iter().any(|r| r.contains(x, y));
            let rests: Vec<_> =
                b.info.rooms.iter().filter(|r| b.info.mission.nodes[r.node].kind == MissionNodeKind::Rest).collect();
            assert!(!rests.is_empty(), "{} has a rest room", m.id);
            for r in &rests {
                assert!(bp.sanctuary.contains(&r.rect), "{} seed {seed}: its rest room", m.id);
            }
            let outs: Vec<_> = bp.props.iter().filter(|p| p.to.is_some_and(|d| d.zone != m.zone)).collect();
            assert!(!outs.is_empty(), "{} has a way out", m.id);
            for p in outs {
                assert!(inside(i32::from(p.cell.x), i32::from(p.cell.y)), "{} seed {seed}: a way out", m.id);
            }
            for u in &bp.units {
                let row = cat.combat.unit(u.def);
                assert!(
                    row.aggro.0 == 0 || !inside(i32::from(u.cell.x), i32::from(u.cell.y)),
                    "{} seed {seed}: {} stands guard in sanctuary at {:?}",
                    m.id,
                    row.id,
                    u.cell
                );
            }
        }
    }
}
