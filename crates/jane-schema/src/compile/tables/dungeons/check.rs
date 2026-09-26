//! Mistakes in a mission, before any layout (checks.ts `lintDef` and PORT.md §5.3): grants
//! nothing gives, a sight line with no corridor, states, the entrance, C3 (no order of spending
//! plain keys strands her), node order, pools that have every socket, bind and placeholder the
//! mission asks of them, edges and their props, and the fallback stamp.

use std::collections::{BTreeMap, BTreeSet};

use super::facts::{Facts, Gain, action_gains};
use super::fallback;
use super::pools::{Library, has_local};
use super::raw::{RawEdgeKind, RawGrant, RawMission, RawNode, RawNodeKind, RawWhat};
use crate::compile::diag::Diagnostics;
use crate::compile::lists::{RawAction, RawCond};

/// What a node gives, read off its holdings (`gainsOf`).
#[derive(Clone, Debug, Default)]
pub struct Gains {
    pub keys: BTreeMap<String, i64>,
    pub items: BTreeMap<String, i64>,
    pub verbs: Vec<String>,
    pub flags: Vec<String>,
    pub states: Vec<String>,
}

impl Gains {
    fn take(&mut self, g: &Gain, fx: &Facts) {
        match g {
            Gain::Item(id, qty) => match fx.item_opens.get(id) {
                Some(tag) => *self.keys.entry(tag.clone()).or_default() += qty,
                None => *self.items.entry(id.clone()).or_default() += qty,
            },
            Gain::Learn(s) => self.verbs.push(s.clone()),
            Gain::Flag(f) => self.flags.push(f.clone()),
        }
    }
}

pub fn gains_of(node: &RawNode, fx: &Facts) -> Gains {
    let mut g = Gains::default();
    for h in &node.holds {
        let mut list = Vec::new();
        if let Some(u) = &h.unit {
            list.extend(fx.unit_gains.get(u).cloned().unwrap_or_default());
            for x in &list {
                g.take(x, fx);
            }
            continue;
        }
        for s in h.loot.iter().flatten() {
            list.push(Gain::Item(s.item.clone(), i64::from(s.qty)));
        }
        action_gains(h.use_list.as_deref(), &mut list);
        if let Some(c) = &h.controls {
            g.states.push(c.clone());
            for l in h.becomes.iter().flat_map(|b| b.values()) {
                action_gains(Some(l), &mut list);
            }
        }
        if let Some(t) = &h.talk {
            list.extend(fx.dialogue_gains.get(t).cloned().unwrap_or_default());
        }
        for x in &list {
            g.take(x, fx);
        }
    }
    g
}

fn grant_json(g: &RawGrant) -> String {
    let mut parts = Vec::new();
    for (k, v) in [("key", &g.key), ("verb", &g.verb), ("item", &g.item)] {
        if let Some(v) = v {
            parts.push(format!("\"{k}\":\"{v}\""));
        }
    }
    if let Some(q) = g.qty {
        parts.push(format!("\"qty\":{q}"));
    }
    for (k, v) in [("flag", &g.flag), ("state", &g.state)] {
        if let Some(v) = v {
            parts.push(format!("\"{k}\":\"{v}\""));
        }
    }
    format!("{{{}}}", parts.join(","))
}

/// Every `@x` in a list in a position generate.ts `resolve` rewrites (`prop`, `unit`, `at`,
/// `rect`, `id`, `to`, `rects`, a `dead` unit in an `if`), and every one it would leave as
/// written. Calls `f(name, bound)`.
pub fn walk_names(list: &[RawAction], f: &mut dyn FnMut(&str, bool)) {
    for a in list {
        match a {
            RawAction::Grow { id, .. } => f(id, true),
            RawAction::Lock { prop }
            | RawAction::Unlock { prop }
            | RawAction::Show { prop }
            | RawAction::Hide { prop }
            | RawAction::Switch { prop, .. } => f(prop, true),
            RawAction::Spawn { unit, def, at } => {
                f(unit, true);
                f(def, false);
                f(at, true);
            }
            RawAction::Despawn { unit } | RawAction::Aggro { unit } => f(unit, true),
            RawAction::Location { name } => f(name, false),
            RawAction::Fill { rect, .. } | RawAction::Strike { rect, .. } => f(rect, true),
            RawAction::Camera { rect: Some(r), .. } => f(r, true),
            RawAction::Flag { flag, .. } => f(flag, false),
            RawAction::Travel { mark, .. } => f(mark, false),
            RawAction::If { when, then, els } => {
                walk_conds(when, f);
                walk_names(then, f);
                if let Some(e) = els {
                    walk_names(e, f);
                }
            }
            RawAction::Send { unit, to, then } => {
                f(unit, true);
                f(to, true);
                if let Some(t) = then {
                    walk_names(t, f);
                }
            }
            RawAction::Reveal { rects } => {
                for r in rects {
                    f(r, true);
                }
            }
            _ => {}
        }
    }
}

/// Conditions: only a `dead` unit is resolved.
pub fn walk_conds(when: &[RawCond], f: &mut dyn FnMut(&str, bool)) {
    for c in when {
        match c {
            RawCond::Dead { unit, .. } => f(unit, true),
            RawCond::Flag { flag, .. } => f(flag, false),
            _ => {}
        }
    }
}

/// Every list of a node, with what `@self` means in it (`None`: nothing; a trigger's).
pub fn node_lists(n: &RawNode) -> Vec<(String, &[RawAction], bool)> {
    let mut out: Vec<(String, &[RawAction], bool)> = Vec::new();
    for (i, h) in n.holds.iter().enumerate() {
        if let Some(l) = &h.use_list {
            out.push((format!("holds[{i}].use"), l, true));
        }
        if let Some(l) = &h.release {
            out.push((format!("holds[{i}].release"), l, true));
        }
        for (v, l) in h.becomes.iter().flatten() {
            out.push((format!("holds[{i}].becomes.{v}"), l, true));
        }
    }
    for (i, t) in n.triggers.iter().enumerate() {
        out.push((format!("triggers[{i}].actions"), &t.actions, false));
        if let Some(l) = &t.reset {
            out.push((format!("triggers[{i}].reset"), l, false));
        }
    }
    out
}

pub fn check_mission(file: &str, m: &RawMission, lib: &Library, fx: &Facts, diag: &mut Diagnostics) {
    let at = |p: &str| format!("{file}: {p}");
    let mut err = |p: &str, msg: String| diag.error(at(p), msg);

    // --- lintDef -------------------------------------------------------------------------
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for n in &m.nodes {
        let p = format!("nodes.{}", n.id);
        if !ids.insert(&n.id) {
            err(&p, format!("node \"{}\" is defined twice", n.id));
        }
        // A holding locked to a key tag is locked to a key from outside: the mission gives it.
        for h in &n.holds {
            if let Some(t) = h.key_tag.as_ref().filter(|t| !m.given_keys.contains(t)) {
                err(&p, format!("node \"{}\": {} is locked to \"{t}\", which givenKeys does not name", n.id, h.socket));
            }
        }
        // A thing that answers a verb and holds loot must be locked with no key.
        for h in &n.holds {
            let (Some(_), Some(prop)) = (&h.loot, &h.prop) else { continue };
            if let Some(answers) = fx.prop(prop).and_then(|f| f.answers.as_ref()) {
                if h.locked != Some(true) {
                    err(
                        &p,
                        format!(
                            "node \"{}\": {} holds \"{prop}\", which answers {answers}, and loot she could reach without it. Lock it with no key, or hide a chest behind it",
                            n.id, h.socket
                        ),
                    );
                }
            }
        }
        let g = gains_of(n, fx);
        for want in &n.grants {
            let shapes =
                [&want.key, &want.verb, &want.item, &want.flag, &want.state].iter().filter(|x| x.is_some()).count();
            if shapes != 1 || want.qty.is_some() != want.item.is_some() {
                err(&p, format!("grant {} is none of key, verb, item and qty, flag, state", grant_json(want)));
                continue;
            }
            let ok = if let Some(k) = &want.key {
                g.keys.get(k).copied().unwrap_or(0) > 0
            } else if let Some(v) = &want.verb {
                g.verbs.contains(v)
            } else if let Some(f) = &want.flag {
                g.flags.contains(f)
            } else if let Some(s) = &want.state {
                g.states.contains(s)
            } else {
                let item = want.item.as_deref().unwrap_or_default();
                g.items.get(item).copied().unwrap_or(0) >= i64::from(want.qty.unwrap_or(0))
            };
            if !ok {
                err(
                    &p,
                    format!("node \"{}\" says it grants {} and nothing it holds gives that", n.id, grant_json(want)),
                );
            }
        }
    }
    for (i, e) in m.edges.iter().enumerate() {
        let p = format!("edges[{i}]");
        if !ids.contains(e.from.as_str()) || !ids.contains(e.to.as_str()) {
            err(&p, format!("edge {} -> {} names a node that does not exist", e.from, e.to));
        }
        let rides = m.edges.iter().enumerate().any(|(j, f)| {
            j != i
                && !matches!(f.kind, RawEdgeKind::Sight)
                && ((f.from == e.from && f.to == e.to) || (f.from == e.to && f.to == e.from))
        });
        if matches!(e.kind, RawEdgeKind::Sight) && !rides {
            err(&p, format!("edge {} -> {} is a sight line with no corridor to look along", e.from, e.to));
        }
    }
    if m.states.len() > 3 {
        err("states", "a dungeon may have three states at most".into());
    }
    for (i, s) in m.states.iter().enumerate() {
        let p = format!("states[{i}]");
        if !s.values.contains(&s.initial) {
            err(&p, format!("state \"{}\" starts as \"{}\", which is not one of its values", s.id, s.initial));
        }
        if !m.nodes.iter().any(|n| n.holds.iter().any(|h| h.controls.as_deref() == Some(s.id.as_str()))) {
            err(&p, format!("state \"{}\" has no control: nothing in the mission can change it", s.id));
        }
    }
    for (i, e) in m.edges.iter().enumerate() {
        for kind in e.kinds() {
            let RawEdgeKind::State { var, is, .. } = kind else { continue };
            match m.states.iter().find(|s| s.id == *var) {
                None => err(
                    &format!("edges[{i}]"),
                    format!("edge {} -> {} waits on a state called \"{var}\", and the mission has none", e.from, e.to),
                ),
                Some(s) if !s.values.contains(is) => err(
                    &format!("edges[{i}]"),
                    format!("edge {} -> {}: state \"{var}\" has no value \"{is}\"", e.from, e.to),
                ),
                Some(_) => {}
            }
        }
    }
    if m.nodes.iter().filter(|n| n.kind == RawNodeKind::Entrance).count() != 1 {
        err("nodes", "a dungeon has exactly one entrance".into());
    }
    let mut by_order: Vec<&RawNode> = m.nodes.iter().collect();
    by_order.sort_by_key(|n| n.order);
    if by_order.first().is_some_and(|n| n.kind != RawNodeKind::Entrance) {
        err("nodes", "the entrance must come first in the order".into());
    }
    for e in spending_orders(m, fx) {
        err("edges", e);
    }

    // --- PORT.md §5.3 ---------------------------------------------------------------------
    let stem = file.trim_start_matches("dungeons/").trim_end_matches(".json");
    if m.id != stem {
        err("id", format!("id \"{}\" is not the file's name \"{stem}\"", m.id));
    }
    let node = |id: &str| m.nodes.iter().find(|n| n.id == id);
    // Node order is monotone: no way forward between critical nodes runs back in the order.
    for (i, e) in m.edges.iter().enumerate() {
        let (Some(a), Some(b)) = (node(&e.from), node(&e.to)) else { continue };
        if a.critical && b.critical && !e.shortcut && b.order < a.order {
            err(
                &format!("edges[{i}]"),
                format!(
                    "edge {} -> {} runs back in the order ({} -> {}) and is not a shortcut",
                    e.from, e.to, a.order, b.order
                ),
            );
        }
    }
    for n in &m.nodes {
        check_node(m, n, lib, fx, &mut err);
    }
    check_edges(m, lib, fx, &mut err);
    check_rest(m, fx, &mut err);
    for (i, msg) in fallback::check(m, lib) {
        err(&i, msg);
    }
}

/// Pools that have every socket, bind and placeholder the node asks of them; holdings that fit.
fn check_node(m: &RawMission, n: &RawNode, lib: &Library, fx: &Facts, err: &mut impl FnMut(&str, String)) {
    let p = format!("nodes.{}", n.id);
    let templates: Vec<_> = lib.pool(&n.pool).collect();
    if templates.is_empty() {
        err(&p, format!("pool \"{}\" has no templates", n.pool));
        return;
    }
    let heat = n.heat.permille().map(|h| h.0);
    if !heat.is_ok_and(|h| (0..=1300).contains(&h)) {
        err(&p, "heat is a share of the base heat, 0 to 1.3".into());
    }
    let edges =
        m.edges.iter().filter(|e| !matches!(e.kind, RawEdgeKind::Sight) && (e.from == n.id || e.to == n.id)).count();
    for t in &templates {
        // Enough doors for every edge the node has, whatever the layout asks of it.
        if t.doors.len() < edges {
            err(&p, format!("{}: {edges} edges and only {} doors", t.id, t.doors.len()));
        }
        for h in &n.holds {
            let Some(s) = t.sockets.iter().find(|s| s.id == h.socket) else {
                err(&p, format!("{} has no socket {} for node {}", t.id, h.socket, n.id));
                continue;
            };
            if h.unit.is_some() {
                continue;
            }
            let prop = h.prop.as_deref().unwrap_or("chest");
            let (w, hh) = (i64::from(s.w as u16), i64::from(s.h as u16));
            if let Some(f) = fx.prop(prop) {
                if f.w != w || f.h != hh {
                    err(
                        &p,
                        format!("\"{prop}\" ({}x{}) does not fit socket {} of {} ({w}x{hh})", f.w, f.h, s.id, t.id),
                    );
                }
            } else if let Some(rows) = fx.family(prop) {
                for r in rows {
                    let f = fx.prop(&r).cloned().unwrap_or_default();
                    if f.w != w || f.h != hh {
                        err(
                            &p,
                            format!("\"{r}\" ({}x{}) does not fit socket {} of {} ({w}x{hh})", f.w, f.h, s.id, t.id),
                        );
                    }
                }
            }
        }
        for b in &n.binds {
            let ok = match b.what {
                RawWhat::Mark => t.marks.iter().any(|x| x.id == b.from),
                RawWhat::Rect => t.rects.iter().any(|x| x.id == b.from),
                RawWhat::Prop | RawWhat::Unit => has_local(t, &b.from),
            };
            if !ok {
                err(&p, format!("{} has nothing called {} for node {} (a {:?})", t.id, b.from, n.id, b.what));
            }
        }
        for h in &n.holds {
            for pt in h.patrol.iter().flatten() {
                if !t.marks.iter().any(|x| x.id == pt.mark) {
                    err(&p, format!("{} has no mark {} for {}'s patrol", t.id, pt.mark, h.socket));
                }
            }
        }
        for tr in &n.triggers {
            let r = tr.rect.as_deref().unwrap_or("room");
            if !t.rects.iter().any(|x| x.id == r) {
                err(&p, format!("{} has no rect {r} for trigger {}", t.id, tr.id));
            }
        }
    }
    // Binds: a unit from a socket a unit holds, a prop from one a prop holds.
    for b in &n.binds {
        let held = n.holds.iter().find(|h| h.socket == b.from);
        let bad = match b.what {
            RawWhat::Unit => held.is_none_or(|h| h.unit.is_none()),
            RawWhat::Prop => held.is_some_and(|h| h.unit.is_some()),
            _ => false,
        };
        if bad {
            err(
                &p,
                format!(
                    "bind {} -> {} says it is a {:?}, and the node holds something else there",
                    b.from, b.name, b.what
                ),
            );
        }
    }
    let mut seen = BTreeSet::new();
    for h in &n.holds {
        if !seen.insert(h.socket.as_str()) {
            err(&p, format!("socket {} is held twice", h.socket));
        }
        if h.unit.is_some() {
            let extra = h.loot.is_some()
                || h.prop.is_some()
                || h.talk.is_some()
                || h.needs.is_some()
                || h.use_list.is_some()
                || h.release.is_some()
                || h.locked.is_some()
                || h.key_tag.is_some()
                || h.hidden.is_some()
                || h.on.is_some()
                || h.label.is_some()
                || h.to.is_some()
                || h.guarded_by.is_some()
                || h.controls.is_some()
                || h.becomes.is_some();
            if extra {
                err(&p, format!("{} holds a unit, which takes a patrol and nothing else", h.socket));
            }
        } else if h.patrol.is_some() {
            err(&p, format!("{}: only a unit patrols", h.socket));
        }
        if let Some(prop) = &h.prop {
            if fx.prop(prop).is_none() && fx.family(prop).is_none() {
                let hint = if fx.prop(&format!("{prop}_n")).is_some() {
                    " (a lamp family needs _n, _e, _s and _w)"
                } else {
                    ""
                };
                err(&p, format!("{}: \"{prop}\" is not a prop row{hint}", h.socket));
            }
        }
        if h.becomes.is_some() && h.controls.is_none() {
            err(&p, format!("{}: becomes without controls", h.socket));
        }
        if let (Some(c), Some(b)) = (&h.controls, &h.becomes) {
            if let Some(s) = m.states.iter().find(|s| s.id == *c) {
                for v in b.keys() {
                    if !s.values.contains(v) {
                        err(&p, format!("{}: becomes \"{v}\", which is not a value of state \"{c}\"", h.socket));
                    }
                }
            }
        }
        if let Some(c) = &h.controls {
            if !m.states.iter().any(|s| s.id == *c) {
                err(&p, format!("{} controls a state called \"{c}\", and the mission has none", h.socket));
            }
        }
        for g in h.guarded_by.iter().flatten() {
            if let Some(local) = g.strip_prefix('@') {
                let held = n.holds.iter().find(|x| x.socket == local);
                if held.is_none_or(|x| x.unit.is_none()) {
                    err(&p, format!("{} is guarded by {g}, which is no unit of this node", h.socket));
                }
            }
        }
    }
    // Placeholders: every `@x` must name a socket, mark or rect of every template, in a
    // position the generator rewrites.
    for (where_, list, self_ok) in node_lists(n) {
        check_placeholders(n, lib, list, self_ok, &format!("{p}.{where_}"), err);
    }
    for (i, t) in n.triggers.iter().enumerate() {
        if let Some(w) = &t.when {
            let mut f = |name: &str, bound: bool| {
                placeholder(n, lib, name, bound, false, &format!("{p}.triggers[{i}].when"), err);
            };
            walk_conds(w, &mut f);
        }
    }
}

fn placeholder(
    n: &RawNode,
    lib: &Library,
    name: &str,
    bound: bool,
    self_ok: bool,
    p: &str,
    err: &mut impl FnMut(&str, String),
) {
    let Some(local) = name.strip_prefix('@') else { return };
    if !bound {
        err(p, format!("\"{name}\" stands where the generator does not bind names; it would stay as written"));
        return;
    }
    if local == "self" {
        if !self_ok {
            err(p, "@self means nothing in a trigger's list".into());
        }
        return;
    }
    for t in lib.pool(&n.pool) {
        if !has_local(t, local) {
            err(p, format!("{name}: {} has no socket, mark or rect called {local}", t.id));
        }
    }
}

fn check_placeholders(
    n: &RawNode,
    lib: &Library,
    list: &[RawAction],
    self_ok: bool,
    p: &str,
    err: &mut impl FnMut(&str, String),
) {
    let mut f = |name: &str, bound: bool| placeholder(n, lib, name, bound, self_ok, p, err);
    walk_names(list, &mut f);
}

/// Edges: their props fit a corridor, their gates are gates, lock-ins find what they name.
fn check_edges(m: &RawMission, lib: &Library, fx: &Facts, err: &mut impl FnMut(&str, String)) {
    let node = |id: &str| m.nodes.iter().find(|n| n.id == id);
    let gate_rows = |rows: [&str; 2], p: &str, err: &mut dyn FnMut(&str, String)| {
        for (r, (w, h)) in rows.iter().zip([(3, 1), (1, 3)]) {
            match fx.prop(r) {
                None => err(p, format!("gate row \"{r}\" is not a prop row")),
                Some(f) if (f.w, f.h) != (w, h) => err(p, format!("gate row \"{r}\" is {}x{}, not {w}x{h}", f.w, f.h)),
                Some(_) => {}
            }
        }
    };
    for (i, e) in m.edges.iter().enumerate() {
        let p = format!("edges[{i}]");
        if e.from == e.to {
            err(&p, format!("edge {} -> {} joins a node to itself", e.from, e.to));
        }
        if e.also.iter().any(|k| matches!(k, RawEdgeKind::Sight | RawEdgeKind::Open)) {
            err(&p, "also takes locks, not open or sight".into());
        }
        if e.kinds().filter(|k| matches!(k, RawEdgeKind::Lockin { .. })).count() > 1 {
            err(&p, "one lock-in to an edge".into());
        }
        let mut gated = false;
        for kind in e.kinds() {
            match kind {
                RawEdgeKind::Key { .. } | RawEdgeKind::Oneway { .. } | RawEdgeKind::Lockin { .. } if !gated => {
                    gate_rows(["gate_h", "gate_v"], &p, &mut *err);
                    gated = true;
                }
                RawEdgeKind::State { gate, .. } => {
                    let rows = gate.as_ref().map_or(["gate_h", "gate_v"], |[a, b]| [a.as_str(), b.as_str()]);
                    gate_rows(rows, &p, &mut *err);
                }
                RawEdgeKind::Verb { prop, use_list, .. } => {
                    // Sealing a corridor of 3 either way: 3 wide, at most 3 tall.
                    match fx.prop(prop) {
                        Some(f) if f.w != 3 || f.h > 3 => err(
                            &p,
                            format!(
                                "\"{prop}\" is {}x{} and cannot seal a corridor of 3 both ways (3 wide, at most 3 tall)",
                                f.w, f.h
                            ),
                        ),
                        _ => {}
                    }
                    if let (Some(l), Some(from)) = (use_list, node(&e.from)) {
                        check_placeholders(from, lib, l, true, &format!("{p}.use"), err);
                    }
                }
                _ => {}
            }
            if let RawEdgeKind::Lockin { spawn, reward, .. } = kind {
                let Some(room) = node(&e.to) else { continue };
                for t in lib.pool(&room.pool) {
                    if let Some(r) = reward {
                        if !t.sockets.iter().any(|s| s.id == *r) {
                            err(&p, format!("lock-in reward {r}: {} has no such socket", t.id));
                        }
                    }
                    for a in spawn.iter().flat_map(|s| s.at.iter()) {
                        if !t.marks.iter().any(|x| x.id == *a) {
                            err(&p, format!("lock-in spawn at {a}: {} has no such mark", t.id));
                        }
                    }
                }
                if let Some(s) = spawn {
                    if s.names.as_ref().is_some_and(|a| a.len() != s.at.len()) {
                        err(&p, "lock-in spawn: one name per mark".into());
                    }
                }
                if spawn.is_none()
                    && !room
                        .holds
                        .iter()
                        .any(|h| h.unit.is_some() && (h.socket.starts_with("boss") || room.kind == RawNodeKind::Boss))
                {
                    err(&p, format!("lock-in into {} has neither a spawn nor a boss to fight", room.id));
                }
            }
        }
    }
}

/// The budget, the dress rows, the lights, the tiles.
fn check_rest(m: &RawMission, fx: &Facts, err: &mut impl FnMut(&str, String)) {
    let b = &m.budget;
    if b.side_rooms[0] > b.side_rooms[1] {
        err("budget.sideRooms", "least is more than most".into());
    }
    let sides = m.nodes.iter().filter(|n| !n.critical).count();
    if usize::from(b.side_rooms[0]) > sides {
        err("budget.sideRooms", format!("asks for {} side rooms and the mission has {sides}", b.side_rooms[0]));
    }
    if b.crit_path_cells[0] > b.crit_path_cells[1] {
        err("budget.critPathCells", "least is more than most".into());
    }
    if let Some([lo, hi]) = b.rest_at {
        match (lo.permille(), hi.permille()) {
            (Ok(l), Ok(h)) if 0 <= l.0 && l.0 <= h.0 && h.0 <= 1000 => {}
            _ => err("budget.restAt", "two fractions of the walk, 0 <= least <= most <= 1".into()),
        }
    }
    for (u, cost) in &b.enemies {
        if *cost == 0 {
            err("budget.enemies", format!("\"{u}\" costs nothing"));
        }
    }
    if b.base_heat == 0 && m.nodes.iter().any(|n| n.heat.is_positive()) {
        err("budget.baseHeat", "nodes have heat and the base heat is 0".into());
    }
    for (name, d) in &m.dress {
        if !d.chance.permille().is_ok_and(|p| (0..=1000).contains(&p.0)) {
            err(&format!("dress.{name}"), "chance is a fraction, 0 to 1".into());
        }
        for pr in &d.props {
            if fx.prop(pr).is_none() {
                err(&format!("dress.{name}"), format!("\"{pr}\" is not a prop row"));
            }
        }
    }
    if let Some(l) = &m.lights {
        if fx.family(&l.prop).is_none() {
            err("lights.prop", format!("\"{}\" is not a lamp family (rows _n, _e, _s and _w)", l.prop));
        }
        if let Some(s) = &l.state {
            match m.states.iter().find(|x| x.id == s.var) {
                None => {
                    err("lights.state", format!("lamps follow a state called \"{}\", and the mission has none", s.var));
                }
                Some(x) if !x.values.contains(&s.is) => {
                    err("lights.state", format!("lamps follow \"{}\" being \"{}\", which it never is", s.var, s.is));
                }
                Some(_) => {}
            }
        }
        if l.every == 0 {
            err("lights.every", "lamps every 0 cells".into());
        }
    }
    if !m.ambient.permille().is_ok_and(|p| (0..=1000).contains(&p.0)) {
        err("ambient", "ambient light is a fraction, 0 to 1".into());
    }
    if m.lattice.cols == 0 || m.lattice.rows == 0 {
        err("lattice", "an empty lattice".into());
    }
    if !(1..=6).contains(&m.phase) {
        err("phase", "phase is 1 to 6".into());
    }
}

// --- C3: every order of spending plain keys ------------------------------------------------

struct GraphLock<'a> {
    edge: usize,
    a: &'a str,
    b: &'a str,
    kinds: Vec<&'a RawEdgeKind>,
}

/// A plain key fits more than one lock, so she chooses where it goes. Search every order of
/// opening the plain locks she can reach; whenever she runs out of choices, every critical
/// node must have been reached. Everything that is not a choice (a named key, a verb, a flag)
/// is opened the moment it can be (checks.ts `spendingOrders`).
pub fn spending_orders(m: &RawMission, fx: &Facts) -> Vec<String> {
    let nodes: Vec<&RawNode> = m.nodes.iter().collect();
    let mut locks: Vec<GraphLock<'_>> = Vec::new();
    for (i, e) in m.edges.iter().enumerate() {
        if matches!(e.kind, RawEdgeKind::Sight) {
            continue;
        }
        locks.push(GraphLock { edge: i, a: &e.from, b: &e.to, kinds: e.kinds().collect() });
    }
    let mut tag_uses: BTreeMap<&str, usize> = BTreeMap::new();
    for l in &locks {
        for k in &l.kinds {
            if let RawEdgeKind::Key { tag, .. } = k {
                *tag_uses.entry(tag.as_str()).or_default() += 1;
            }
        }
    }
    let plain = |l: &GraphLock<'_>| -> Option<String> {
        l.kinds.iter().find_map(|k| match k {
            RawEdgeKind::Key { tag, .. } if tag_uses.get(tag.as_str()).copied().unwrap_or(0) > 1 => Some(tag.clone()),
            _ => None,
        })
    };
    let gains: Vec<Gains> = nodes.iter().map(|n| gains_of(n, fx)).collect();
    let Some(entrance) = nodes.iter().find(|n| n.kind == RawNodeKind::Entrance) else { return Vec::new() };
    let mut s = Search {
        m,
        nodes: &nodes,
        locks: &locks,
        gains: &gains,
        entrance: &entrance.id,
        plain: &plain,
        errors: Vec::new(),
        seen: BTreeSet::new(),
    };
    s.explore(Vec::new(), &BTreeMap::new(), &[]);
    s.errors
}

struct Search<'a, 'b> {
    m: &'a RawMission,
    nodes: &'b [&'a RawNode],
    locks: &'b [GraphLock<'a>],
    gains: &'b [Gains],
    entrance: &'a str,
    plain: &'b dyn Fn(&GraphLock<'_>) -> Option<String>,
    errors: Vec<String>,
    seen: BTreeSet<Vec<usize>>,
}

impl Search<'_, '_> {
    fn explore(&mut self, opened: Vec<usize>, spent: &BTreeMap<String, i64>, order: &[String]) {
        let mut id = opened.clone();
        id.sort();
        if !self.errors.is_empty() || !self.seen.insert(id) {
            return;
        }
        // Settle: reach, collect, open what is not a choice, until nothing moves.
        let mut open = opened;
        for _guard in 0..64 {
            let mut reached: Vec<&str> = vec![self.entrance];
            let mut q = 0;
            while q < reached.len() {
                for l in self.locks {
                    let passable = open.contains(&l.edge)
                        || !l.kinds.iter().any(|k| !matches!(k, RawEdgeKind::Open | RawEdgeKind::Lockin { .. }));
                    if !passable {
                        continue;
                    }
                    let next = if l.a == reached[q] {
                        Some(l.b)
                    } else if l.b == reached[q] {
                        Some(l.a)
                    } else {
                        None
                    };
                    if let Some(nx) = next {
                        if !reached.contains(&nx) {
                            reached.push(nx);
                        }
                    }
                }
                q += 1;
            }
            let mut verbs: Vec<&str> = self.m.given_verbs.iter().map(String::as_str).collect();
            let mut flags: Vec<&str> = Vec::new();
            let mut controlled: Vec<&str> = Vec::new();
            let mut keys: BTreeMap<&str, i64> = BTreeMap::new();
            for t in &self.m.given_keys {
                keys.insert(t, 99);
            }
            for (i, n) in self.nodes.iter().enumerate() {
                if !reached.contains(&n.id.as_str()) {
                    continue;
                }
                let g = &self.gains[i];
                verbs.extend(g.verbs.iter().map(String::as_str));
                flags.extend(g.flags.iter().map(String::as_str));
                controlled.extend(g.states.iter().map(String::as_str));
                for (tag, n) in &g.keys {
                    *keys.entry(tag).or_default() += n;
                }
            }
            let state_open = |id: &str, is: &str| {
                self.m.states.iter().any(|s| s.id == id && s.initial == is) || controlled.contains(&id)
            };
            let mut moved = false;
            for l in self.locks {
                if open.contains(&l.edge)
                    || (self.plain)(l).is_some()
                    || (!reached.contains(&l.a) && !reached.contains(&l.b))
                {
                    continue;
                }
                let can = l.kinds.iter().all(|k| match k {
                    RawEdgeKind::Key { tag, .. } => keys.get(tag.as_str()).copied().unwrap_or(0) > 0,
                    RawEdgeKind::Verb { verb, .. } => verbs.contains(&verb.as_str()),
                    RawEdgeKind::Oneway { flag, .. } => flags.contains(&flag.as_str()),
                    RawEdgeKind::State { var, is, .. } => state_open(var, is),
                    _ => true,
                });
                if can {
                    open.push(l.edge);
                    moved = true;
                }
            }
            if moved {
                continue;
            }
            // Her choices now: each plain lock in reach that a key in hand fits.
            let choices: Vec<(usize, String, String)> = self
                .locks
                .iter()
                .filter_map(|l| {
                    let tag = (self.plain)(l)?;
                    let fits = !open.contains(&l.edge)
                        && (reached.contains(&l.a) || reached.contains(&l.b))
                        && keys.get(tag.as_str()).copied().unwrap_or(0) - spent.get(&tag).copied().unwrap_or(0) > 0;
                    fits.then(|| (l.edge, tag, format!("{}-{}", l.a, l.b)))
                })
                .collect();
            if choices.is_empty() {
                let lost: Vec<&str> = self
                    .nodes
                    .iter()
                    .filter(|n| n.critical && !reached.contains(&n.id.as_str()))
                    .map(|n| n.id.as_str())
                    .collect();
                if !lost.is_empty() {
                    self.errors.push(format!(
                        "C3: opening plain locks in the order [{}] strands her short of {}",
                        order.join(", "),
                        lost.join(", ")
                    ));
                }
                return;
            }
            for (edge, tag, name) in choices {
                let mut o = open.clone();
                o.push(edge);
                let mut sp = spent.clone();
                *sp.entry(tag).or_default() += 1;
                let mut ord = order.to_vec();
                ord.push(name);
                self.explore(o, &sp, &ord);
            }
            return;
        }
    }
}
