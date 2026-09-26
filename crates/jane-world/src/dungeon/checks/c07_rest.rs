//! C7: a rest room off the hub, with no enemies, at the right depth: no heat, no spawn socket,
//! somewhere to rest, an edge to the hub, and first reached between `budget.rest_at` (40 % to
//! 60 % unless the mission says) of the way along the first completion (C8's walk).

use jane_core::num::Permille;
use jane_data::{MissionNodeKind, RoomSocketKind};

use super::{Check, Ctx, Fault};

/// Where the rest room falls on the walk when the mission does not say.
const REST_AT: (Permille, Permille) = (Permille(400), Permille(600));

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (m, bp, cat) = (c.m, c.bp, c.cat);
    let mut out = Vec::new();
    let Some(rest) = c.info.rooms.iter().find(|r| m.nodes[r.node].kind == MissionNodeKind::Rest) else {
        return vec![Fault::new(Check::C7, "no rest room")];
    };
    let node = &m.nodes[rest.node];
    let template = cat.dungeons.template(rest.template);
    if node.heat.0 != 0 {
        out.push(Fault::new(Check::C7, "the rest room has heat"));
    }
    if template.sockets.iter().any(|s| s.kind == RoomSocketKind::Spawn) {
        out.push(Fault::new(Check::C7, format!("{} is a rest room with spawn sockets", template.id)));
    }
    let rests = bp
        .props
        .iter()
        .any(|p| rest.rect.contains(i32::from(p.cell.x), i32::from(p.cell.y)) && cat.story.prop(p.def).rest);
    if !rests {
        out.push(Fault::new(Check::C7, "the rest room has nowhere to rest"));
    }
    let hub = |n: u8| c.room_of(usize::from(n)).is_some_and(|r| m.nodes[r.node].kind == MissionNodeKind::Hub);
    let off_hub = m
        .edges
        .iter()
        .any(|e| (usize::from(e.from) == rest.node && hub(e.to)) || (usize::from(e.to) == rest.node && hub(e.from)));
    if !off_hub {
        out.push(Fault::new(Check::C7, "the rest room is not off the hub"));
    }
    let w = &c.walk;
    let (from, to) = m.budget.rest_at.unwrap_or(REST_AT);
    if w.errors.is_empty() && w.cells > 0 {
        let at = w.reached_at.get(&rest.node).copied();
        // at / cells against a permille, in integers: at * 1000 against permille * cells.
        let outside = at.is_none_or(|at| {
            i64::from(at) * 1000 < i64::from(from.0) * i64::from(w.cells)
                || i64::from(at) * 1000 > i64::from(to.0) * i64::from(w.cells)
        });
        if outside {
            let when = match at {
                None => "never".to_owned(),
                Some(at) => format!("{}%", jane_core::num::div_round(100 * i64::from(at), i64::from(w.cells))),
            };
            out.push(Fault::new(
                Check::C7,
                format!(
                    "the rest room is first reached {when} of the way through, outside {}% to {}%",
                    from.0 / 10,
                    to.0 / 10
                ),
            ));
        }
    }
    out
}
