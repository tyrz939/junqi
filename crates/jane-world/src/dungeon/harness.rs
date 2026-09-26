//! The template harness (DUNGEONS.md §2.4, `harness.ts`, PORT.md §6.m stage 16). A template is
//! an interface, and this proves it keeps its word: stamp the room ALONE, holding what its
//! mission node holds, with a dead-end stub on every door ([`build_room_alone`]); then run the
//! lock-and-key solver from each door in turn, as a fragment of its zone.
//!
//! - **grants**: every socket it names that the node fills is reached, and every other door can
//!   be walked out of.
//! - **blocks**: with the `until` socket's lists withheld, the `what` socket stays shut.
//!
//! A variant that cannot prove itself cannot ship. The generator never calls this; the tests do.

use jane_core::action::FlagKey;
use jane_core::blueprint::Blueprint;
use jane_core::ids::{Key, TemplateId};
use jane_data::{MissionDef, catalog};

use super::generate::{build_room_alone, door_mark};
use crate::solve::ablate::Grant;
use crate::solve::model::{Options, ZoneRules};
use crate::solve::report::BuildInfo as Trace;
use crate::solve::run::solve;

/// The rules a room alone is solved under: no contract, the mission's keys at the door, and its
/// states (a room of a building with a breaker is solved as that building is, in every state its
/// own controls can make). Nothing is gated on a spell: the room is judged on its own sockets.
pub fn rules_alone(m: &MissionDef) -> ZoneRules {
    let mut r = ZoneRules::open(m.zone);
    r.given_keys = m.given_keys.iter().map(|&n| Key::Name(n)).collect();
    r.states = m.states.iter().map(|s| FlagKey::Named(Key::Name(s.flag))).collect();
    r
}

/// A name the generator made, if this blueprint has it.
fn local(bp: &Blueprint, name: &str) -> Option<Key> {
    bp.local_names.iter().position(|n| n == name).map(|i| Key::Local(i as u32))
}

/// Is the thing called `key` reached? A locked prop when it has been opened; any other prop when
/// she can stand beside it; a unit when she can stand on its cell.
fn reached(bp: &Blueprint, trace: &Trace, key: Key) -> bool {
    if let Some(p) = bp.props.iter().find(|p| p.key == key) {
        if p.locked {
            return trace.fired(key).is_some();
        }
        let d = catalog().story.prop(p.def);
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        return (y - 1..=y + i32::from(d.h))
            .any(|cy| (x - 1..=x + i32::from(d.w)).any(|cx| trace.first_seen_at(cx, cy).is_some()));
    }
    bp.units
        .iter()
        .find(|u| u.key == key)
        .is_some_and(|u| trace.first_seen_at(i32::from(u.cell.x), i32::from(u.cell.y)).is_some())
}

/// Every promise the template makes as node `node` of `m`, tried in every transform it allows,
/// from every door. Empty means it holds.
pub fn prove_template(m: &'static MissionDef, node: usize, tid: TemplateId) -> Vec<String> {
    let cat = catalog();
    let template = cat.dungeons.template(tid);
    let n = &m.nodes[node];
    let mut errors = Vec::new();
    let Some(names) = n.names.iter().find(|r| r.template == tid) else {
        return vec![format!("{}: node \"{}\" has no names for it", template.id, n.id)];
    };
    let rules = rules_alone(m);
    let socket_name = |i: u16| Key::Name(names.sockets[usize::from(i)]);
    for shape in template.shapes {
        let at =
            format!("{} turn {}{}", template.id, shape.turn.degrees(), if shape.mirror { " mirrored" } else { "" });
        let built = build_room_alone(m, node, tid, shape);
        if !built.info.errors.is_empty() {
            errors.push(format!("{at}: {}", built.info.errors.join("; ")));
            continue;
        }
        let bp = &built.blueprint;
        let marks: Vec<Option<Key>> = shape.doors.iter().map(|d| local(bp, &door_mark(m, node, d))).collect();
        for (di, door) in shape.doors.iter().enumerate() {
            let Some(entry) = marks[di] else {
                errors.push(format!("{at}: door {} has no stub", door.id));
                continue;
            };
            let opts = Options { entry: Some(entry), fragment: true, trace: true, ..Options::default() };
            let report = solve(bp, &rules, &opts);
            if report.info.first_seen.is_none() {
                errors.push(format!("{at}: {}", report.lines(bp).join("; ")));
                break;
            }
            let trace = &report.info;
            for (oi, other) in shape.doors.iter().enumerate() {
                let out = marks[oi].and_then(|k| bp.marks.get(&k));
                if out.is_none_or(|mk| trace.first_seen_at(i32::from(mk.cell.x), i32::from(mk.cell.y)).is_none()) {
                    errors.push(format!("{at}: in by {}, she cannot leave by {}", door.id, other.id));
                }
            }
            for &g in template.grants {
                let id = template.sockets[usize::from(g)].id;
                if !n.holds.iter().any(|h| h.socket == id) {
                    continue;
                }
                if !reached(bp, trace, socket_name(g)) {
                    errors.push(format!("{at}: in by {}, {id} is never reached", door.id));
                }
            }
            for b in template.blocks {
                let without = solve(bp, &rules, &opts.without(Grant::Prop(socket_name(b.until))));
                if without.info.first_seen.is_some() && reached(bp, &without.info, socket_name(b.what)) {
                    errors.push(format!(
                        "{at}: {} opens without {}",
                        template.sockets[usize::from(b.what)].id,
                        template.sockets[usize::from(b.until)].id
                    ));
                }
            }
        }
    }
    errors
}

/// Every template of every mission, as every node whose pool holds it: `(mission, node,
/// template)`, in mission, node and pool order.
pub fn every_template() -> Vec<(&'static MissionDef, usize, TemplateId)> {
    let d = &catalog().dungeons;
    let mut out = Vec::new();
    for m in d.missions {
        for (i, n) in m.nodes.iter().enumerate() {
            for &t in d.pool(n.pool).templates {
                out.push((m, i, t));
            }
        }
    }
    out
}
