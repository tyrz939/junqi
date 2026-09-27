//! Missions and templates into the model: ids resolved through `cx`, names interned, lists
//! pooled. The checks are `check.rs`'s; what fails here is a reference to another table.

use jane_core::action::Stack;
use jane_core::blueprint::TriggerMode;
use jane_core::grid::{Cell, Rect};
use jane_core::ids::{NameId, PoolId, PropDefId, TemplateId};
use jane_core::num::{Permille, Tick};
use jane_core::tile::Tile;

use crate::compile::ctx::{Ctx, RowIds, leak, leak_str};
use crate::compile::lists;
use crate::model::{
    self, MissionBind, MissionBudget, MissionContract, MissionDef, MissionDoorTo, MissionDress, MissionEdge,
    MissionEdgeKind, MissionGate, MissionGrant, MissionHolding, MissionLights, MissionLockinSpawn, MissionName,
    MissionNode, MissionNodeKind, MissionNodeNames, MissionPatrol, MissionPlacement, MissionProp, MissionSetPart,
    MissionSetPiece, MissionSetRoom, MissionState, MissionTrigger, RoomBlock, RoomDoor, RoomMark, RoomPool, RoomRect,
    RoomShape, RoomSocket, RoomTemplate, RoomTileChar, RoomTurn, SetAgainst,
};

use super::check::{node_lists, walk_conds, walk_names};
use super::facts::Facts;
use super::names::{self, local_name};
use super::pools::{Library, at_line};
use super::raw::{RawEdge, RawEdgeKind, RawMission, RawMode, RawNode, RawNodeKind, RawStack};
use super::room::{self, Door, Room, Shape};

pub fn tile_named(name: &str) -> Option<Tile> {
    Tile::ALL.iter().copied().find(|t| t.name() == name)
}

fn tile(cx: &mut Ctx, at: &str, name: &str) -> Tile {
    tile_named(name).unwrap_or_else(|| {
        cx.diag.error(at, format!("no tile called \"{name}\""));
        Tile::Void
    })
}

fn u16_of(v: i32) -> u16 {
    u16::try_from(v).unwrap_or(0)
}

fn u8_of(v: i32) -> u8 {
    u8::try_from(v).unwrap_or(0)
}

fn door(d: &Door) -> RoomDoor {
    RoomDoor {
        id: leak_str(&d.id),
        side: d.side,
        bay: u8_of(d.bay),
        required: d.required,
        x: u16_of(d.cx),
        y: u16_of(d.cy),
    }
}

fn rect4(r: (i32, i32, i32, i32)) -> Rect {
    Rect::new(r.0, r.1, r.2, r.3)
}

fn shape(s: &Shape) -> RoomShape {
    let cells: Vec<u8> = s.cells.iter().flatten().copied().collect();
    RoomShape {
        turn: RoomTurn::from_degrees(s.turn as u16).unwrap_or(RoomTurn::R0),
        mirror: s.mirror,
        w: u16_of(s.w),
        h: u16_of(s.h),
        bays: (u8_of(s.bays.0), u8_of(s.bays.1)),
        ox: s.ox,
        oy: s.oy,
        cells: leak_str(&String::from_utf8_lossy(&cells)),
        doors: leak(s.doors.iter().map(door).collect()),
        sockets: leak(s.sockets.iter().copied().map(rect4).collect()),
        marks: leak(s.marks.iter().map(|&(x, y)| Cell::new(u16_of(x), u16_of(y))).collect()),
        rects: leak(s.rects.iter().copied().map(rect4).collect()),
    }
}

fn stacks(cx: &mut Ctx, at: &str, raw: Option<&[RawStack]>) -> &'static [Stack] {
    let mut out = Vec::new();
    for s in raw.into_iter().flatten() {
        cx.diag.need(s.qty >= 1, at, "qty < 1");
        if let Some(item) = cx.item(at, &s.item) {
            out.push(Stack { item, qty: s.qty });
        }
    }
    leak(out)
}

fn template(cx: &mut Ctx, fx: &Facts, t: &Room, shapes: &[Shape], pool: PoolId) -> RoomTemplate {
    let at = at_line(&t.file, 0);
    let mut sockets = Vec::new();
    for s in &t.sockets {
        let mut options = Vec::new();
        for o in &s.options {
            let at = at_line(&t.file, t.row_line(s.cy));
            let Some(id) = cx.prop(&at, o) else { continue };
            if let Some(f) = fx.prop(o) {
                if (f.w, f.h) != (i64::from(s.w), i64::from(s.h)) {
                    cx.diag.error(
                        &at,
                        format!("{} offers \"{o}\" ({}x{}), which does not fit it ({}x{})", s.id, f.w, f.h, s.w, s.h),
                    );
                }
            }
            options.push(id);
        }
        sockets.push(RoomSocket {
            id: leak_str(&s.id),
            kind: s.kind,
            x: u16_of(s.cx),
            y: u16_of(s.cy),
            w: u16_of(s.w),
            h: u16_of(s.h),
            solid: room::socket_is_solid(s),
            options: leak(options),
        });
    }
    let socket_ix = |id: &str| t.sockets.iter().position(|s| s.id == id).map_or(0, |i| i as u16);
    let tiles: Vec<RoomTileChar> =
        t.tiles.iter().map(|(ch, name)| RoomTileChar { ch: *ch, tile: tile(cx, &at, name) }).collect();
    let needs_verbs =
        t.needs_verbs.iter().filter_map(|v| cx.spell(&at_line(&t.file, t.head_line("needs")), v)).collect();
    let needs_items = t
        .needs_items
        .iter()
        .filter_map(|(i, q)| cx.item(&at_line(&t.file, t.head_line("needs")), i).map(|item| Stack { item, qty: *q }))
        .collect();
    RoomTemplate {
        id: leak_str(&t.id),
        pool,
        bays: (u8_of(t.bays.0), u8_of(t.bays.1)),
        turns: leak(t.turns.iter().filter_map(|&d| RoomTurn::from_degrees(d as u16)).collect()),
        mirror: t.mirror,
        w: u16_of(t.gw()),
        h: u16_of(t.gh()),
        doors: leak(t.doors.iter().map(door).collect()),
        sockets: leak(sockets),
        marks: leak(
            t.marks.iter().map(|m| RoomMark { id: leak_str(&m.id), x: u16_of(m.cx), y: u16_of(m.cy) }).collect(),
        ),
        rects: leak(
            t.rects.iter().map(|r| RoomRect { id: leak_str(&r.id), rect: Rect::new(r.cx, r.cy, r.w, r.h) }).collect(),
        ),
        needs_verbs: leak(needs_verbs),
        needs_items: leak(needs_items),
        grants: leak(t.grants.iter().map(|g| socket_ix(g)).collect()),
        blocks: leak(t.blocks.iter().map(|(w, u)| RoomBlock { what: socket_ix(w), until: socket_ix(u) }).collect()),
        coop: leak(t.coop.clone()),
        heat_max: t.heat_max,
        tiles: leak(tiles),
        shapes: leak(shapes.iter().filter(|s| s.fits).map(shape).collect()),
    }
}

/// Templates in id order, and pools in id order.
pub fn rooms(
    cx: &mut Ctx,
    fx: &Facts,
    lib: &Library,
) -> (RowIds, &'static [RoomTemplate], RowIds, &'static [RoomPool]) {
    let template_ids = RowIds::from_keys(lib.rooms.iter().map(|r| r.id.as_str()));
    let pool_ids = RowIds::from_keys(lib.pools.keys().map(String::as_str));
    let mut by_id: Vec<usize> = (0..lib.rooms.len()).collect();
    by_id.sort_by(|&a, &b| lib.rooms[a].id.cmp(&lib.rooms[b].id));
    let mut templates = Vec::with_capacity(by_id.len());
    for i in by_id {
        let r = &lib.rooms[i];
        let pool = PoolId(pool_ids.get(&r.pool).unwrap_or(0));
        templates.push(template(cx, fx, r, &lib.shapes[i], pool));
    }
    let pools = lib
        .pools
        .iter()
        .map(|(id, members)| RoomPool {
            id: leak_str(id),
            templates: leak(
                members.iter().map(|&i| TemplateId(template_ids.get(&lib.rooms[i].id).unwrap_or(0))).collect(),
            ),
        })
        .collect();
    (template_ids, leak(templates), pool_ids, leak(pools))
}

fn node_kind(k: RawNodeKind) -> MissionNodeKind {
    match k {
        RawNodeKind::Entrance => MissionNodeKind::Entrance,
        RawNodeKind::Teach => MissionNodeKind::Teach,
        RawNodeKind::Fight => MissionNodeKind::Fight,
        RawNodeKind::Puzzle => MissionNodeKind::Puzzle,
        RawNodeKind::Key => MissionNodeKind::Key,
        RawNodeKind::Hub => MissionNodeKind::Hub,
        RawNodeKind::Rest => MissionNodeKind::Rest,
        RawNodeKind::Miniboss => MissionNodeKind::Miniboss,
        RawNodeKind::Verb => MissionNodeKind::Verb,
        RawNodeKind::Bosskey => MissionNodeKind::Bosskey,
        RawNodeKind::Boss => MissionNodeKind::Boss,
        RawNodeKind::Reward => MissionNodeKind::Reward,
        RawNodeKind::Exit => MissionNodeKind::Exit,
        RawNodeKind::Side => MissionNodeKind::Side,
    }
}

fn permille(cx: &mut Ctx, at: &str, n: crate::compile::fraction::Num) -> Permille {
    n.permille().map_err(|e| cx.diag.error(at, e)).unwrap_or(Permille::ZERO)
}

struct Mx<'a> {
    file: &'a str,
    m: &'a RawMission,
    zone: &'a str,
}

impl Mx<'_> {
    fn at(&self, p: &str) -> String {
        format!("{}: {p}", self.file)
    }

    fn state_ix(&self, id: &str) -> Option<u8> {
        self.m.states.iter().position(|s| s.id == id).map(|i| i as u8)
    }

    fn value_ix(&self, state: &str, v: &str) -> Option<u8> {
        self.m.states.iter().find(|s| s.id == state)?.values.iter().position(|x| x == v).map(|i| i as u8)
    }
}

fn holding(cx: &mut Ctx, fx: &Facts, x: &Mx<'_>, n: &RawNode, i: usize) -> MissionHolding {
    let h = &n.holds[i];
    let at = x.at(&format!("nodes.{}.holds[{i}]", n.id));
    let key_s = local_name(x.zone, n, &h.socket);
    let key = cx.name(&key_s);
    let unit = h.unit.as_deref().and_then(|u| cx.unit(&at, u));
    let patrol = h
        .patrol
        .iter()
        .flatten()
        .map(|p| MissionPatrol { mark: leak_str(&p.mark), dwell: Tick(p.dwell.unwrap_or(0)) })
        .collect();
    let prop = if h.unit.is_some() {
        None
    } else {
        let name = h.prop.as_deref().unwrap_or("chest");
        match fx.family(name) {
            Some(rows) => {
                let ids: Vec<PropDefId> = rows.iter().filter_map(|r| cx.prop(&at, r)).collect();
                <[PropDefId; 4]>::try_from(ids).ok().map(MissionProp::Family)
            }
            None => cx.prop(&at, name).map(MissionProp::Row),
        }
    };
    let loot = h.loot.as_deref().map(|l| stacks(cx, &at, Some(l)));
    let talk = h.talk.as_deref().and_then(|t| cx.dialogue(&at, t));
    let needs = stacks(cx, &at, h.needs.as_deref());
    let use_list = lists::list(cx, &format!("{at}.use"), h.use_list.as_deref());
    let release = lists::list(cx, &format!("{at}.release"), h.release.as_deref());
    let label = h.label.as_deref().map(|l| cx.text(l));
    let to = h.to.as_ref().and_then(|t| {
        let zone = cx.zone(&at, &t.zone)?;
        Some(MissionDoorTo { zone, mark: cx.name(&t.mark) })
    });
    let guarded: Vec<NameId> = h
        .guarded_by
        .iter()
        .flatten()
        .map(|g| {
            let s = names::resolve(x.zone, n, &key_s, g).unwrap_or_else(|| g.clone());
            cx.name(&s)
        })
        .collect();
    let free_trigger = (!guarded.is_empty()).then(|| cx.name(&format!("{key_s}_free")));
    let controls = h.controls.as_deref().and_then(|c| x.state_ix(c));
    let mut becomes = [None, None];
    for (v, l) in h.becomes.iter().flatten() {
        let ix = h.controls.as_deref().and_then(|c| x.value_ix(c, v));
        let r = lists::list(cx, &format!("{at}.becomes.{v}"), Some(l));
        if let Some(ix) = ix {
            becomes[usize::from(ix)] = r;
        }
    }
    MissionHolding {
        socket: leak_str(&h.socket),
        key,
        unit,
        patrol: leak(patrol),
        prop,
        loot,
        talk,
        needs,
        use_list,
        release,
        locked: h.locked.unwrap_or(false) || h.key_tag.is_some(),
        key_tag: h.key_tag.as_deref().map(|t| cx.name(t)),
        hidden: h.hidden.unwrap_or(false),
        on: h.on,
        label,
        to,
        guarded_by: leak(guarded),
        free_trigger,
        controls,
        becomes,
    }
}

fn node(
    cx: &mut Ctx,
    fx: &Facts,
    x: &Mx<'_>,
    lib: &Library,
    ids: (&RowIds, &RowIds),
    ni: usize,
    base_heat: u16,
) -> MissionNode {
    let (template_ids, pool_ids) = ids;
    let n = &x.m.nodes[ni];
    let at = x.at(&format!("nodes.{}", n.id));
    // Names first, so a node's names are interned together.
    let mut names_rows = Vec::new();
    for t in lib.pool(&n.pool) {
        let mut row = |ids: Vec<&str>| -> &'static [NameId] {
            leak(ids.into_iter().map(|l| cx.name(&local_name(x.zone, n, l))).collect())
        };
        let sockets = row(t.sockets.iter().map(|s| s.id.as_str()).collect());
        let marks = row(t.marks.iter().map(|m| m.id.as_str()).collect());
        let rects = row(t.rects.iter().map(|r| r.id.as_str()).collect());
        names_rows.push(MissionNodeNames {
            template: TemplateId(template_ids.get(&t.id).unwrap_or(0)),
            sockets,
            marks,
            rects,
        });
    }
    let holds: Vec<MissionHolding> = (0..n.holds.len()).map(|i| holding(cx, fx, x, n, i)).collect();
    let binds = n
        .binds
        .iter()
        .map(|b| MissionBind { from: leak_str(&b.from), name: cx.name(&b.name), what: names::what_of(b.what) })
        .collect();
    let demands = n.demands.iter().filter_map(|d| cx.spell(&at, d)).collect();
    let mut grants = Vec::new();
    for g in &n.grants {
        let got = if let Some(k) = &g.key {
            Some(MissionGrant::Key(cx.name(k)))
        } else if let Some(v) = &g.verb {
            cx.spell(&at, v).map(MissionGrant::Verb)
        } else if let Some(i) = &g.item {
            cx.item(&at, i).map(|item| MissionGrant::Item(Stack { item, qty: g.qty.unwrap_or(1) }))
        } else if let Some(f) = &g.flag {
            Some(MissionGrant::Flag(cx.name(f)))
        } else {
            g.state.as_deref().and_then(|s| x.state_ix(s)).map(MissionGrant::State)
        };
        grants.extend(got);
    }
    let mut triggers = Vec::new();
    for (i, t) in n.triggers.iter().enumerate() {
        let tat = format!("{at}.triggers[{i}]");
        triggers.push(MissionTrigger {
            id: leak_str(&t.id),
            name: cx.name(&names::trigger_name(x.zone, n, &t.id)),
            rect: cx.name(&local_name(x.zone, n, t.rect.as_deref().unwrap_or("room"))),
            mode: match t.mode {
                Some(RawMode::While) => TriggerMode::While,
                Some(RawMode::Enter) | None => TriggerMode::Enter,
            },
            once: t.once.unwrap_or(true),
            when: lists::conds(cx, &tat, t.when.as_deref()),
            actions: lists::list(cx, &format!("{tat}.actions"), Some(&t.actions))
                .unwrap_or(jane_core::ListRef::Catalog(0)),
            reset: lists::list(cx, &format!("{tat}.reset"), t.reset.as_deref()),
        });
    }
    // Placeholders: every `@x` its lists hold, and the `use` of verb edges out of it.
    let mut placeholders: Vec<String> = Vec::new();
    {
        let mut f = |name: &str, bound: bool| {
            if bound && name.starts_with('@') && name != "@self" && !placeholders.iter().any(|p| p == name) {
                placeholders.push(name.to_owned());
            }
        };
        for (_, list, _) in node_lists(n) {
            walk_names(list, &mut f);
        }
        for t in &n.triggers {
            walk_conds(t.when.as_deref().unwrap_or_default(), &mut f);
        }
        for e in x.m.edges.iter().filter(|e| e.from == n.id) {
            for k in e.kinds() {
                if let RawEdgeKind::Verb { use_list: Some(l), .. } = k {
                    walk_names(l, &mut f);
                }
            }
        }
    }
    let at_names = placeholders
        .iter()
        .map(|p| {
            let bound = names::resolve(x.zone, n, "", p).unwrap_or_default();
            (cx.name(p), cx.name(&bound))
        })
        .collect();
    let heat = permille(cx, &at, n.heat);
    MissionNode {
        id: leak_str(&n.id),
        kind: node_kind(n.kind),
        order: n.order,
        critical: n.critical,
        pool: PoolId(pool_ids.get(&n.pool).unwrap_or(0)),
        holds: leak(holds),
        binds: leak(binds),
        demands: leak(demands),
        grants: leak(grants),
        heat,
        heat_cost: u16::try_from(names::heat_cost(n, i64::from(base_heat))).unwrap_or(u16::MAX),
        triggers: leak(triggers),
        dark: n.dark,
        names: leak(names_rows),
        at_names: leak(at_names),
    }
}

fn edge_kind(cx: &mut Ctx, x: &Mx<'_>, at: &str, e: &RawEdge, kind: &RawEdgeKind) -> MissionEdgeKind {
    let text = |cx: &mut Ctx, s: &Option<String>| s.as_deref().map(|s| cx.text(s));
    match kind {
        RawEdgeKind::Open => MissionEdgeKind::Open,
        RawEdgeKind::Sight => MissionEdgeKind::Sight,
        RawEdgeKind::Key { tag, label, .. } => MissionEdgeKind::Key { tag: cx.name(tag), label: text(cx, label) },
        RawEdgeKind::Verb { verb, prop, needs, label, toast, use_list, .. } => {
            let verb = cx.spell(at, verb).unwrap_or_default();
            let p = cx.prop(at, prop).unwrap_or_default();
            let name = names::verb_prop_name(x.zone, e, kind).unwrap_or_default();
            MissionEdgeKind::Verb {
                verb,
                prop: p,
                needs: stacks(cx, at, needs.as_deref()),
                name: cx.name(&name),
                label: text(cx, label),
                toast: text(cx, toast),
                use_list: lists::list(cx, &format!("{at}.use"), use_list.as_deref()),
            }
        }
        RawEdgeKind::Lockin { spawn, toast, reward, .. } => {
            let room = x.m.nodes.iter().find(|n| n.id == e.to);
            let names = room.and_then(|r| names::lockin_names(x.zone, r, kind));
            let n = |cx: &mut Ctx, s: Option<&String>| cx.name(s.map_or("", String::as_str));
            let spawn = spawn.as_ref().and_then(|s| {
                let def = cx.unit(at, &s.def)?;
                let room = room?;
                let marks = s.at.iter().map(|a| cx.name(&local_name(x.zone, room, a))).collect();
                let units = names.iter().flat_map(|l| l.spawned.iter()).map(|u| cx.name(u)).collect();
                Some(MissionLockinSpawn { def, at: leak(marks), names: leak(units) })
            });
            let reward = match (reward, room) {
                (Some(r), Some(room)) => Some(cx.name(&local_name(x.zone, room, r))),
                _ => None,
            };
            MissionEdgeKind::Lockin {
                spawn,
                toast: text(cx, toast),
                reward,
                rect: n(cx, names.as_ref().map(|l| &l.rect)),
                mark: n(cx, names.as_ref().map(|l| &l.mark)),
                way_in: n(cx, names.as_ref().map(|l| &l.way_in)),
                lock: n(cx, names.as_ref().map(|l| &l.lock)),
                clear: n(cx, names.as_ref().map(|l| &l.clear)),
            }
        }
        RawEdgeKind::Oneway { flag, label, .. } => MissionEdgeKind::Oneway {
            flag: cx.name(flag),
            trigger: cx.name(&names::oneway_trigger(x.zone, e)),
            label: text(cx, label),
        },
        RawEdgeKind::State { var, is, label, .. } => MissionEdgeKind::State {
            var: x.state_ix(var).unwrap_or(0),
            is: x.value_ix(var, is).unwrap_or(0),
            label: text(cx, label),
        },
    }
}

pub fn mission(
    cx: &mut Ctx,
    fx: &Facts,
    lib: &Library,
    ids: (&RowIds, &RowIds),
    file: &str,
    m: &RawMission,
) -> MissionDef {
    let x = Mx { file, m, zone: &m.id };
    let at = |p: &str| x.at(p);
    let zone = cx.zone(file, &m.id).unwrap_or(jane_core::ZoneId::County);
    let name = cx.text(&m.name);
    let floor = tile(cx, &at("tiles.floor"), &m.tiles.floor);
    let wall = tile(cx, &at("tiles.wall"), &m.tiles.wall);
    let alt: Vec<Tile> = m.tiles.alt.iter().map(|t| tile(cx, &at("tiles.alt"), t)).collect();
    let ambient = permille(cx, &at("ambient"), m.ambient);
    let given_verbs = m.given_verbs.iter().filter_map(|v| cx.spell(&at("givenVerbs"), v)).collect();
    let given_keys = m.given_keys.iter().map(|k| cx.name(k)).collect();
    let states = m
        .states
        .iter()
        .map(|s| MissionState {
            id: leak_str(&s.id),
            values: [leak_str(&s.values[0]), leak_str(&s.values[1])],
            initial: s.values.iter().position(|v| *v == s.initial).unwrap_or(0) as u8,
            flag: cx.name(&s.flag),
        })
        .collect();
    let all_rect = cx.name(&format!("{}_all", m.id));
    let nodes: Vec<MissionNode> =
        (0..m.nodes.len()).map(|i| node(cx, fx, &x, lib, ids, i, m.budget.base_heat)).collect();
    let node_ix = |id: &str| m.nodes.iter().position(|n| n.id == id).map_or(0, |i| i as u8);
    let mut edges = Vec::new();
    for (i, e) in m.edges.iter().enumerate() {
        let eat = at(&format!("edges[{i}]"));
        let kind = edge_kind(cx, &x, &eat, e, &e.kind);
        let also: Vec<MissionEdgeKind> = e.also.iter().map(|k| edge_kind(cx, &x, &eat, e, k)).collect();
        let gate = names::gate_name(&m.id, e).map(|g| {
            let rows = e
                .kinds()
                .find_map(|k| match k {
                    RawEdgeKind::State { gate: Some([a, b]), .. } => Some([a.clone(), b.clone()]),
                    _ => None,
                })
                .unwrap_or_else(|| ["gate_h".to_owned(), "gate_v".to_owned()]);
            let rows = [cx.prop(&eat, &rows[0]).unwrap_or_default(), cx.prop(&eat, &rows[1]).unwrap_or_default()];
            MissionGate { name: cx.name(&g), rows }
        });
        edges.push(MissionEdge {
            from: node_ix(&e.from),
            to: node_ix(&e.to),
            kind,
            also: leak(also),
            shortcut: e.shortcut,
            gate,
        });
    }
    let b = &m.budget;
    let mut enemies: Vec<(jane_core::UnitDefId, u16)> =
        b.enemies.iter().filter_map(|(u, c)| cx.unit(&at("budget.enemies"), u).map(|id| (id, *c))).collect();
    enemies.sort();
    let rest_at =
        b.rest_at.map(|[lo, hi]| (permille(cx, &at("budget.restAt"), lo), permille(cx, &at("budget.restAt"), hi)));
    let budget = MissionBudget {
        side_rooms: (b.side_rooms[0], b.side_rooms[1]),
        crit_path_cells: (b.crit_path_cells[0], b.crit_path_cells[1]),
        rest_at,
        rest_to_boss_cells: b.rest_to_boss_cells,
        base_heat: b.base_heat,
        enemies: leak(enemies),
    };
    let dress = m
        .dress
        .iter()
        .map(|(name, d)| MissionDress {
            name: leak_str(name),
            props: leak(d.props.iter().filter_map(|p| cx.prop(&at(&format!("dress.{name}")), p)).collect()),
            chance: permille(cx, &at(&format!("dress.{name}")), d.chance),
        })
        .collect();
    let set_names: Vec<&String> = m.sets.pieces.keys().collect();
    let sets: Vec<MissionSetPiece> = m
        .sets
        .pieces
        .iter()
        .map(|(name, p)| {
            let at = at(&format!("sets.pieces.{name}"));
            let parts: Vec<MissionSetPart> = p
                .parts
                .iter()
                .filter_map(|(pr, x, y)| cx.prop(&at, pr).map(|prop| MissionSetPart { prop, x: *x, y: *y }))
                .collect();
            let (mut w, mut h) = (0i64, 0i64);
            for (pr, x, y) in &p.parts {
                let f = fx.prop(pr).cloned().unwrap_or_default();
                w = w.max(i64::from(*x) + f.w);
                h = h.max(i64::from(*y) + f.h);
            }
            let against = match p.against.as_str() {
                "n" => SetAgainst::N,
                "s" => SetAgainst::S,
                "side" => SetAgainst::Side,
                "corner" => SetAgainst::Corner,
                "wall" => SetAgainst::Wall,
                _ => SetAgainst::Free,
            };
            MissionSetPiece {
                name: leak_str(name),
                against,
                mirror: p.mirror,
                w: u8::try_from(w).unwrap_or(u8::MAX),
                h: u8::try_from(h).unwrap_or(u8::MAX),
                parts: leak(parts),
            }
        })
        .collect();
    let set_rooms: Vec<MissionSetRoom> = m
        .sets
        .rooms
        .iter()
        .filter_map(|(node, r)| {
            Some(MissionSetRoom {
                node: u8::try_from(m.nodes.iter().position(|n| &n.id == node)?).ok()?,
                most: r.most,
                take: leak(
                    r.take
                        .iter()
                        .filter_map(|t| set_names.iter().position(|n| *n == t).and_then(|i| u8::try_from(i).ok()))
                        .collect(),
                ),
            })
        })
        .collect();
    let lights = m.lights.as_ref().map(|l| {
        let rows = fx.family(&l.prop).unwrap_or_else(|| ["n", "e", "s", "w"].map(|s| format!("{}_{s}", l.prop)));
        let ids: Vec<PropDefId> = rows.iter().map(|r| cx.prop(&at("lights.prop"), r).unwrap_or_default()).collect();
        MissionLights {
            family: <[PropDefId; 4]>::try_from(ids).unwrap_or_default(),
            every: l.every,
            corridor: l.corridor,
            state: l.state.as_ref().and_then(|s| Some((x.state_ix(&s.var)?, x.value_ix(&s.var, &s.is)?))),
        }
    });
    let fallback = m
        .fallback
        .iter()
        .map(|f| MissionPlacement {
            node: node_ix(&f.node),
            template: TemplateId(ids.0.get(&f.template).unwrap_or(0)),
            bay: (f.bay[0], f.bay[1]),
            turn: RoomTurn::from_degrees(f.turn).unwrap_or_else(|| {
                cx.diag.error(at("fallback"), format!("turn {} is not 0, 90, 180 or 270", f.turn));
                RoomTurn::R0
            }),
            mirror: f.mirror,
        })
        .collect();
    for f in &m.fallback {
        if ids.0.get(&f.template).is_none() {
            cx.diag.error(at("fallback"), format!("no room template \"{}\"", f.template));
        }
    }
    let c = names::contract_of(m);
    let mut intern = |v: &[String]| -> &'static [NameId] { leak(v.iter().map(|s| cx.name(s)).collect()) };
    let contract = MissionContract {
        units: intern(&c.units),
        props: intern(&c.props),
        marks: intern(&c.marks),
        rects: intern(&c.rects),
    };
    let min_cost = b.enemies.values().copied().min().map_or(0, i64::from);
    let mut provides: Vec<MissionName> = names::provides(m, lib, i64::from(b.base_heat), min_cost)
        .into_iter()
        .map(|(s, what)| MissionName { name: cx.name(&s), what })
        .collect();
    provides.sort_by_key(|p| (p.name, p.what));
    for n in &m.nodes {
        if !lib.pools.contains_key(&n.pool) {
            cx.diag.error(at(&format!("nodes.{}", n.id)), format!("no pool \"{}\"", n.pool));
        }
    }
    MissionDef {
        id: leak_str(&m.id),
        zone,
        name,
        phase: m.phase,
        floor,
        wall,
        alt: leak(alt),
        indoor: m.indoor,
        ambient,
        cols: m.lattice.cols,
        rows: m.lattice.rows,
        given_verbs: leak(given_verbs),
        given_keys: leak(given_keys),
        states: leak(states),
        nodes: leak(nodes),
        edges: leak(edges),
        budget,
        dress: leak(dress),
        sets: leak(sets),
        set_rooms: leak(set_rooms),
        lights,
        fallback: leak(fallback),
        all_rect,
        contract,
        provides: leak(provides),
    }
}

/// An empty `Dungeons` for a compile that had nothing to read.
pub fn empty() -> model::Dungeons {
    model::Dungeons { missions: &[], templates: &[], pools: &[] }
}
