//! Names a mission puts in a blueprint, as strings (generate.ts, checks.ts `contractOf`).
//! Everything here depends only on the mission and the pools, never on a seed, which is why
//! the build can intern it and the P3 generator never has to make these names itself.

use crate::model::{MissionNameWhat, RoomSocketKind};

use super::pools::Library;
use super::raw::{RawEdge, RawEdgeKind, RawMission, RawNode, RawWhat};

/// The name the generator gives a socket, mark or rect of a node: its bind if it has one,
/// else `<zone>_<node>_<local>` with `:` as `_` (generate.ts `localName`).
pub fn local_name(zone: &str, node: &RawNode, local: &str) -> String {
    match node.binds.iter().find(|b| b.from == local) {
        Some(b) => b.name.clone(),
        None => format!("{zone}_{}_{}", node.id, local.replace(':', "_")),
    }
}

/// `@x` resolved in a node, as generate.ts `resolve` does; `None` for a name that is not a placeholder.
pub fn resolve(zone: &str, node: &RawNode, self_name: &str, v: &str) -> Option<String> {
    if v == "@self" {
        return Some(self_name.to_owned());
    }
    v.strip_prefix('@').map(|local| local_name(zone, node, local))
}

pub fn what_of(w: RawWhat) -> MissionNameWhat {
    match w {
        RawWhat::Prop => MissionNameWhat::Prop,
        RawWhat::Unit => MissionNameWhat::Unit,
        RawWhat::Mark => MissionNameWhat::Mark,
        RawWhat::Rect => MissionNameWhat::Rect,
    }
}

/// The gate an edge puts in its corridor, if any of its kinds does: the first `gateAs` among
/// them, else `<zone>_gate_<from>_<to>`.
pub fn gate_name(zone: &str, e: &RawEdge) -> Option<String> {
    if !e.kinds().any(RawEdgeKind::gates) {
        return None;
    }
    Some(
        e.kinds()
            .find_map(RawEdgeKind::gate_as)
            .map_or_else(|| format!("{zone}_gate_{}_{}", e.from, e.to), str::to_owned),
    )
}

/// The prop a verb kind stands in the corridor: `propAs`, else `<zone>_<prop>_<from>_<to>`.
pub fn verb_prop_name(zone: &str, e: &RawEdge, kind: &RawEdgeKind) -> Option<String> {
    let RawEdgeKind::Verb { prop, prop_as, .. } = kind else { return None };
    Some(prop_as.clone().unwrap_or_else(|| format!("{zone}_{prop}_{}_{}", e.from, e.to)))
}

/// Names a lock-in writes for the room it seals (`writeLockin`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockinNames {
    pub rect: String,
    pub mark: String,
    pub way_in: String,
    pub lock: String,
    pub clear: String,
    /// The units a `spawn` stands up.
    pub spawned: Vec<String>,
}

pub fn lockin_names(zone: &str, room: &RawNode, kind: &RawEdgeKind) -> Option<LockinNames> {
    let RawEdgeKind::Lockin { spawn, .. } = kind else { return None };
    let id = &room.id;
    let spawned = spawn
        .as_ref()
        .map(|s| {
            (0..s.at.len())
                .map(|n| {
                    s.names
                        .as_ref()
                        .and_then(|a| a.get(n))
                        .cloned()
                        .unwrap_or_else(|| format!("{zone}_{id}_lockin_{n}"))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(LockinNames {
        rect: local_name(zone, room, "inner"),
        mark: format!("{zone}_{id}_in"),
        way_in: format!("{zone}_{id}_wayin"),
        lock: format!("{zone}_{id}_lock"),
        clear: format!("{zone}_{id}_clear"),
        spawned,
    })
}

/// The trigger a `oneway` edge writes.
pub fn oneway_trigger(zone: &str, e: &RawEdge) -> String {
    format!("{zone}_{}_{}_opens", e.from, e.to)
}

/// A node trigger's name.
pub fn trigger_name(zone: &str, node: &RawNode, id: &str) -> String {
    format!("{zone}_{}_{id}", node.id)
}

/// A mission's derived zone rows, as strings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contract {
    pub units: Vec<String>,
    pub props: Vec<String>,
    pub marks: Vec<String>,
    pub rects: Vec<String>,
}

fn push_new(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_owned());
    }
}

/// Every name the mission binds for a node that is always there (checks.ts `contractOf`): the
/// story may lean on these and no others.
pub fn contract_of(m: &RawMission) -> Contract {
    let mut c = Contract::default();
    let critical = |id: &str| m.nodes.iter().find(|n| n.id == id).is_some_and(|n| n.critical);
    for n in m.nodes.iter().filter(|n| n.critical) {
        for b in &n.binds {
            let list = match b.what {
                RawWhat::Unit => &mut c.units,
                RawWhat::Prop => &mut c.props,
                RawWhat::Mark => &mut c.marks,
                RawWhat::Rect => &mut c.rects,
            };
            push_new(list, &b.name);
        }
    }
    for e in &m.edges {
        if !critical(&e.from) || !critical(&e.to) {
            continue;
        }
        for kind in e.kinds() {
            if let Some(g) = kind.gate_as() {
                push_new(&mut c.props, g);
            }
            if let RawEdgeKind::Verb { prop_as: Some(p), .. } = kind {
                push_new(&mut c.props, p);
            }
        }
    }
    c
}

/// Every name a blueprint of the mission can hold, with what it names, in a fixed order
/// (duplicates of the same name and kind dropped). Lamps are left out: how many a room gets
/// depends on the layout.
pub fn provides(m: &RawMission, lib: &Library, base_heat: i64, min_cost: i64) -> Vec<(String, MissionNameWhat)> {
    let zone = m.id.as_str();
    let mut out: Vec<(String, MissionNameWhat)> = Vec::new();
    let mut add = |s: String, w: MissionNameWhat| {
        if !out.iter().any(|(n, x)| *n == s && *x == w) {
            out.push((s, w));
        }
    };
    add(format!("{zone}_all"), MissionNameWhat::Rect);
    for n in &m.nodes {
        for b in &n.binds {
            add(b.name.clone(), what_of(b.what));
        }
        let mut spawns_max = 0i64;
        let mut cap_max = 0i64;
        for t in lib.pool(&n.pool) {
            for s in &t.sockets {
                let held = n.holds.iter().find(|h| h.socket == s.id);
                let what = match (held, s.kind) {
                    (Some(h), _) if h.unit.is_some() => Some(MissionNameWhat::Unit),
                    (Some(_), _) | (None, RoomSocketKind::Push | RoomSocketKind::Dress) => Some(MissionNameWhat::Prop),
                    _ => None,
                };
                if let Some(w) = what {
                    add(local_name(zone, n, &s.id), w);
                }
            }
            for mk in &t.marks {
                add(local_name(zone, n, &mk.id), MissionNameWhat::Mark);
            }
            for r in &t.rects {
                add(local_name(zone, n, &r.id), MissionNameWhat::Rect);
            }
            let spawns = t.sockets.iter().filter(|s| s.kind == RoomSocketKind::Spawn).count() as i64;
            spawns_max = spawns_max.max(spawns * 2);
            cap_max = cap_max.max(i64::from(t.heat_max));
        }
        // The enemy fill: at most two to a spawn socket, and no more than the heat buys.
        let cost = heat_cost(n, base_heat).min(cap_max);
        let most = if min_cost > 0 { spawns_max.min(cost / min_cost) } else { 0 };
        for k in 0..most {
            add(format!("{zone}_{}_spawn_{k}", n.id), MissionNameWhat::Unit);
        }
        for t in &n.triggers {
            add(trigger_name(zone, n, &t.id), MissionNameWhat::Trigger);
        }
        for h in &n.holds {
            if h.guarded_by.as_ref().is_some_and(|g| !g.is_empty()) {
                add(format!("{}_free", local_name(zone, n, &h.socket)), MissionNameWhat::Trigger);
            }
        }
    }
    for e in &m.edges {
        if let Some(g) = gate_name(zone, e) {
            add(g, MissionNameWhat::Prop);
        }
        for kind in e.kinds() {
            if let Some(p) = verb_prop_name(zone, e, kind) {
                add(p, MissionNameWhat::Prop);
            }
            if matches!(kind, RawEdgeKind::Oneway { .. }) {
                add(oneway_trigger(zone, e), MissionNameWhat::Trigger);
            }
            let Some(room) = m.nodes.iter().find(|n| n.id == e.to) else { continue };
            if let Some(l) = lockin_names(zone, room, kind) {
                add(l.rect, MissionNameWhat::Rect);
                add(l.mark, MissionNameWhat::Mark);
                add(l.way_in, MissionNameWhat::Prop);
                add(l.lock, MissionNameWhat::Trigger);
                add(l.clear.clone(), MissionNameWhat::Trigger);
                add(l.clear, MissionNameWhat::Flag);
                for u in l.spawned {
                    add(u, MissionNameWhat::Unit);
                }
            }
        }
    }
    out
}

/// `round(heat * base_heat)`, half up, in integers (generate.ts `fillEnemies`).
pub fn heat_cost(n: &RawNode, base_heat: i64) -> i64 {
    let p = n.heat.permille().map_or(0, |p| i64::from(p.0));
    (p * base_heat + 500).div_euclid(1000)
}
