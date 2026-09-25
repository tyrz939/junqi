//! C9: a loop, and a shortcut that comes out near the rest room. The room graph this seed placed
//! has a cycle (corridors - rooms + 1 >= 1); with every lock open the boss is within
//! `budget.rest_to_boss_cells` of the rest room; and a shortcut was placed, one end of which is
//! as near the rest room as that.

use std::collections::BTreeSet;

use jane_data::MissionNodeKind;

use super::{Check, Ctx, Fault, distances};

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let m = c.m;
    let mut out = Vec::new();
    let Some(layout) = c.info.layout.as_ref() else { return out };
    // corridors - rooms + 1 independent cycles.
    if layout.corridors.len() < c.info.rooms.len() {
        out.push(Fault::new(Check::C9, "the room graph has no loop"));
    }
    let find = |kind| c.info.rooms.iter().find(|r| m.nodes[r.node].kind == kind);
    let (Some(rest), Some(boss)) = (find(MissionNodeKind::Rest), find(MissionNodeKind::Boss)) else { return out };
    let everything: BTreeSet<_> = c.info.locks.iter().map(|l| l.prop).collect();
    let dist = distances(c.bp, rest.centre, &everything);
    let near = i32::from(m.budget.rest_to_boss_cells);
    let to_boss = dist.read(boss.centre.0, boss.centre.1, -1);
    if to_boss < 0 || to_boss > near {
        out.push(Fault::new(
            Check::C9,
            format!("the boss is {to_boss} cells from the rest room with everything open, over {near}"),
        ));
    }
    let ends: Vec<_> = layout
        .corridors
        .iter()
        .filter(|cor| m.edges[cor.edge].shortcut)
        .flat_map(|cor| [c.room_of(cor.a.node), c.room_of(cor.b.node)])
        .collect();
    if ends.is_empty() {
        out.push(Fault::new(Check::C9, "no shortcut was placed"));
    } else if !ends.iter().flatten().any(|r| {
        let d = dist.read(r.centre.0, r.centre.1, -1);
        d >= 0 && d <= near
    }) {
        out.push(Fault::new(Check::C9, "no shortcut comes out near the rest room"));
    }
    out
}
