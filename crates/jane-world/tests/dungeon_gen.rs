//! PORT.md §6.m stages 12 to 15: the dungeon generator's layout, locks, lamps, fill and names,
//! held to all eight missions over `SEEDS` seeds, every one of them proven by the solver and
//! checks C1 to C13 inside the attempt budget, the fallback included. Carries the structural parts of
//! `jane/test/{dungeon-gen,dungeons,dungeon-dressing}.test.ts` and of the per-dungeon files: every
//! seed embeds without the fallback, corridors never cross or break into a room, rooms sit in
//! their bays, the contract's names are all there, names do not move with the attempt, the lamps
//! follow their rules, heat stays within its cap, and the same seed is the same dungeon.
//! The checks' own rejections are `dungeon_checks.rs`; the walks and the per-dungeon proofs are
//! `dungeon_missions.rs`; what needs the sim (the bots) carries in P4.

mod common;

use std::fmt::Write as _;
use std::sync::OnceLock;

use jane_core::action::{Action, Condition, FlagKey, FlagOp};
use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Key, ListRef, Rect, Tile, ZoneId};
use jane_data::{
    BAY_H, BAY_W, BORDER, MissionDef, MissionEdgeKind, MissionNodeKind, MissionProp, RoomSide, RoomSocketKind, catalog,
};
use jane_world::dungeon::checks::check_dungeon;
use jane_world::dungeon::layout::Lanes;
use jane_world::dungeon::lights::into;
use jane_world::dungeon::{Built, build, build_candidate, build_dungeon, build_with};

fn missions() -> &'static [MissionDef] {
    catalog().dungeons.missions
}

/// The seeds of the sweep, spread as the TypeScript suite spread them.
fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| 31 + i * 7919).collect()
}

/// Every mission's dungeon for every seed, built once for the whole file: no test writes.
fn sweep() -> &'static [(&'static MissionDef, u32, Built)] {
    static SWEEP: OnceLock<Vec<(&'static MissionDef, u32, Built)>> = OnceLock::new();
    SWEEP.get_or_init(|| {
        let seeds = seed_list();
        missions().iter().flat_map(|m| seeds.iter().map(move |&s| (m, s, build(m.zone, s)))).collect()
    })
}

fn name(bp: &Blueprint, k: Key) -> String {
    match k {
        Key::Name(n) => catalog().name(n).to_owned(),
        Key::Local(i) => bp.local_names[i as usize].clone(),
    }
}

fn prop_keys(bp: &Blueprint) -> Vec<Key> {
    bp.props.iter().map(|p| p.key).collect()
}

fn list(bp: &Blueprint, r: ListRef) -> Vec<Action> {
    match r {
        ListRef::Blueprint(_) => bp.list(r).expect("a blueprint list").to_vec(),
        ListRef::Catalog(_) => catalog().list(r).to_vec(),
    }
}

fn solid(t: Tile) -> bool {
    t.flags() & F_SOLID != 0
}

#[test]
fn every_dungeon_embeds_on_every_seed_without_the_fallback() {
    let mut report = String::new();
    for m in missions() {
        let (mut worst, mut fallbacks) = (0, 0);
        for (_, seed, b) in sweep().iter().filter(|x| x.0.zone == m.zone) {
            let info = &b.info;
            assert!(info.errors.is_empty(), "{} seed {seed}: {:?}", m.id, info.errors);
            let layout = info.layout.as_ref().expect("a layout");
            fallbacks += u32::from(layout.fallback);
            worst = worst.max(b.blueprint.attempts);
            assert!(b.blueprint.attempts < ZONE_ATTEMPTS, "{} seed {seed}: {} attempts", m.id, b.blueprint.attempts);
            // No room twice, every critical node placed, side rooms inside their budget.
            let mut used: Vec<_> = layout.placements.iter().map(|p| p.template).collect();
            used.sort();
            let n = used.len();
            used.dedup();
            assert_eq!(used.len(), n, "{} seed {seed}: a template used twice", m.id);
            for (i, node) in m.nodes.iter().enumerate() {
                if node.critical {
                    assert!(info.room_of(i).is_some(), "{} seed {seed}: {} not placed", m.id, node.id);
                }
            }
            let sides = info.rooms.iter().filter(|r| !m.nodes[r.node].critical).count();
            assert!(
                (usize::from(m.budget.side_rooms.0)..=usize::from(m.budget.side_rooms.1)).contains(&sides),
                "{} seed {seed}: {sides} side rooms",
                m.id
            );
            // Every edge between two placed rooms has its corridor, and every sight edge rides on one.
            for (ei, e) in m.edges.iter().enumerate() {
                let placed = info.room_of(usize::from(e.from)).is_some() && info.room_of(usize::from(e.to)).is_some();
                let has = layout.corridors.iter().any(|c| c.edge == ei);
                if e.kind != MissionEdgeKind::Sight {
                    assert_eq!(has, placed, "{} seed {seed}: edge {ei}", m.id);
                }
            }
        }
        report.push_str(
            &[m.id, ": worst attempts ", &worst.to_string(), ", fallback ", &fallbacks.to_string(), "\n"].concat(),
        );
        assert_eq!(fallbacks, 0, "{}: fell back to the hand-placed layout", m.id);
    }
    println!("{report}");
}

#[test]
fn corridors_never_cross_or_merge_and_never_break_into_a_room() {
    for (m, seed, b) in sweep() {
        let layout = b.info.layout.as_ref().unwrap();
        let lanes = Lanes::new(i32::from(m.cols), i32::from(m.rows));
        let mut owner = vec![usize::MAX; lanes.count];
        for (ci, c) in layout.corridors.iter().enumerate() {
            for &n in &c.lane {
                let n = usize::from(n);
                assert!(owner[n] == usize::MAX, "{} seed {seed}: lane node {n} carries two corridors", m.id);
                owner[n] = ci;
            }
            // The lane is a path: each step is one link.
            for w in c.lane.windows(2) {
                let (a, b) = (lanes.cell(usize::from(w[0])), lanes.cell(usize::from(w[1])));
                assert!(a.0 == b.0 || a.1 == b.1, "{} seed {seed}: a lane jumps", m.id);
            }
        }
        for c in &b.info.corridors {
            for q in &c.rects {
                for r in &b.info.rooms {
                    assert!(
                        !q.overlaps(r.bounds()),
                        "{} seed {seed}: a corridor breaks into {}",
                        m.id,
                        m.nodes[r.node].id
                    );
                }
            }
        }
    }
}

#[test]
fn rooms_sit_in_their_bays_with_their_doors_on_the_mouths() {
    for (m, seed, b) in sweep() {
        let layout = b.info.layout.as_ref().unwrap();
        let lanes = Lanes::new(i32::from(m.cols), i32::from(m.rows));
        let mut owner = vec![false; usize::from(m.cols) * usize::from(m.rows)];
        for (p, r) in layout.placements.iter().zip(&b.info.rooms) {
            assert_eq!(usize::from(p.node), r.node);
            let (bx, by) = (i32::from(p.bay.0), i32::from(p.bay.1));
            let (bw, bh) = (i32::from(r.shape.bays.0), i32::from(r.shape.bays.1));
            for j in by..by + bh {
                for i in bx..bx + bw {
                    let o = &mut owner[(j * i32::from(m.cols) + i) as usize];
                    assert!(!*o, "{} seed {seed}: two rooms in bay ({i}, {j})", m.id);
                    *o = true;
                }
            }
            // Inside the bay group, clear of the three-cell lanes on its lines.
            let group = Rect::new(BORDER + bx * BAY_W, BORDER + by * BAY_H, bw * BAY_W, bh * BAY_H);
            let inner = Rect::new(group.x + 2, group.y + 2, group.w - 3, group.h - 3);
            let bounds = r.bounds();
            assert!(
                bounds.x >= inner.x
                    && bounds.y >= inner.y
                    && bounds.right() <= inner.right()
                    && bounds.bottom() <= inner.bottom(),
                "{} seed {seed}: {} at {bounds:?} is not inside its bays {group:?}",
                m.id,
                m.nodes[r.node].id
            );
            // Every door in use opens onto its port's mouth line.
            for &di in &r.doors {
                let d = &r.shape.doors[di];
                let (px, py) = lanes.cell(lanes.port(bx, by, r.shape.bays, d));
                let (dx, dy) = (r.x + i32::from(d.x), r.y + i32::from(d.y));
                match d.side {
                    RoomSide::N | RoomSide::S => assert_eq!(dx, px, "{} seed {seed}: door {}", m.id, d.id),
                    RoomSide::E | RoomSide::W => assert_eq!(dy, py, "{} seed {seed}: door {}", m.id, d.id),
                }
            }
        }
    }
}

#[test]
fn every_contract_name_exists_on_every_seed() {
    for (m, seed, b) in sweep() {
        let bp = &b.blueprint;
        let c = m.contract;
        for &n in c.units {
            assert!(bp.units.iter().any(|u| u.key == Key::Name(n)), "{} seed {seed}: unit {}", m.id, catalog().name(n));
        }
        for &n in c.props {
            assert!(bp.props.iter().any(|p| p.key == Key::Name(n)), "{} seed {seed}: prop {}", m.id, catalog().name(n));
        }
        for &n in c.marks {
            assert!(bp.marks.contains_key(&Key::Name(n)), "{} seed {seed}: mark {}", m.id, catalog().name(n));
        }
        for &n in c.rects {
            assert!(bp.rects.contains_key(&Key::Name(n)), "{} seed {seed}: rect {}", m.id, catalog().name(n));
        }
        assert!(bp.rects.contains_key(&Key::Name(m.all_rect)));
        assert!(bp.props.iter().any(|p| p.to.is_some()), "{} seed {seed}: no way out", m.id);
        // Every name the generator could have made from a mission's name is the interned one;
        // only its lamps, its scatter and its set pieces (anonymous dressing, nothing names them)
        // are local.
        for p in &bp.props {
            if let Key::Local(_) = p.key {
                assert!(
                    name(bp, p.key).contains("_lamp_")
                        || name(bp, p.key).contains("_scatter_")
                        || name(bp, p.key).contains("_set_"),
                    "{} seed {seed}: {} is not interned",
                    m.id,
                    name(bp, p.key)
                );
            }
        }
        for u in &bp.units {
            assert!(matches!(u.key, Key::Name(_)), "{} seed {seed}: unit {} is not interned", m.id, name(bp, u.key));
        }
    }
}

/// The names a mission always puts down: its critical nodes' holdings and the corridor props of
/// edges between critical nodes.
fn always_named(m: &MissionDef) -> Vec<Key> {
    let mut out = Vec::new();
    for n in m.nodes.iter().filter(|n| n.critical) {
        out.extend(n.holds.iter().filter(|h| h.unit.is_none()).map(|h| Key::Name(h.key)));
    }
    for e in m.edges.iter().filter(|e| m.nodes[usize::from(e.from)].critical && m.nodes[usize::from(e.to)].critical) {
        out.extend(e.gate.map(|g| Key::Name(g.name)));
        for k in std::iter::once(&e.kind).chain(e.also) {
            if let MissionEdgeKind::Verb { name, .. } = k {
                out.push(Key::Name(*name));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn names_come_from_node_and_socket_and_survive_every_reroll() {
    for m in missions() {
        let want = always_named(m);
        for bp in [
            build_dungeon(m.zone, 5, 0),
            build_dungeon(m.zone, 5, 3),
            build_dungeon(m.zone, 77777, 0),
            build_dungeon(m.zone, 5, ZONE_ATTEMPTS - 1),
        ] {
            let have = prop_keys(&bp);
            for k in &want {
                assert!(have.contains(k), "{} attempt {}: {} missing", m.id, bp.attempts - 1, name(&bp, *k));
            }
            // A jar's or a page's growth is keyed on its own name, so taking it twice does nothing.
            let mut grows = 0;
            for p in &bp.props {
                for a in p.use_list.map(|l| list(&bp, l)).unwrap_or_default() {
                    if let Action::Grow { id, .. } = a {
                        assert_eq!(id, p.key, "{}: {} grows as {}", m.id, name(&bp, p.key), name(&bp, id));
                        grows += 1;
                    }
                }
            }
            if m.zone == ZoneId::Mine {
                assert!(grows >= 3, "the mine has jars");
            }
        }
    }
}

#[test]
fn placeholders_are_bound_in_every_list() {
    let at = |k: Key| matches!(k, Key::Name(n) if catalog().name(n).starts_with('@'));
    for (m, seed, b) in sweep().iter().step_by(4) {
        let bp = &b.blueprint;
        for (i, l) in bp.lists.iter().enumerate() {
            for a in l {
                let keys: Vec<Key> = match *a {
                    Action::Lock(k)
                    | Action::Unlock(k)
                    | Action::Show(k)
                    | Action::Hide(k)
                    | Action::Despawn(k)
                    | Action::Aggro(k)
                    | Action::Location(k)
                    | Action::Switch { prop: k, .. }
                    | Action::Grow { id: k, .. }
                    | Action::Fill { rect: k, .. }
                    | Action::Strike { rect: k, .. }
                    | Action::Camera { rect: Some(k), .. }
                    | Action::Flag { key: FlagKey::Named(k) | FlagKey::Been(k) | FlagKey::Dead(k), .. } => vec![k],
                    Action::Spawn { key, at, .. } => vec![key, at],
                    Action::Send { unit, to, .. } => vec![unit, to],
                    _ => vec![],
                };
                for k in keys {
                    assert!(!at(k), "{} seed {seed}: list {i} still says {}", m.id, name(bp, k));
                }
            }
        }
        for c in bp.conds.iter().flatten() {
            if let Condition::Dead(k) = c.c {
                assert!(!at(k), "{} seed {seed}: a condition still says {}", m.id, name(bp, k));
            }
        }
        for n in bp.name_lists.iter().flatten() {
            assert!(!at(*n), "{} seed {seed}: a name list still says {}", m.id, name(bp, *n));
        }
    }
}

#[test]
fn lamps_hang_on_walls_by_rule_and_dark_rooms_have_none() {
    let c = catalog();
    for (m, seed, b) in sweep().iter().filter(|x| x.1 < 31 + 3 * 7919) {
        let Some(plan) = m.lights else { continue };
        let bp = &b.blueprint;
        let lamps: Vec<_> = bp.props.iter().filter(|p| plan.family.contains(&p.def)).collect();
        assert!(lamps.len() > b.info.rooms.len(), "{} seed {seed}: {} lamps", m.id, lamps.len());
        for p in &lamps {
            let side = [RoomSide::N, RoomSide::E, RoomSide::S, RoomSide::W]
                [plan.family.iter().position(|&f| f == p.def).unwrap()];
            let (dx, dy) = into(side);
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            let key = name(bp, p.key);
            assert_eq!(bp.tiles.read(x, y, Tile::Void), m.wall, "{} seed {seed}: {key} is not in a wall", m.id);
            assert!(!solid(bp.tiles.read(x + dx, y + dy, Tile::Void)), "{} seed {seed}: {key} faces a wall", m.id);
            assert!(!c.story.prop(p.def).solid, "{}: a lamp stands in somebody's way", m.id);
        }
        // No lamp hangs over a door or a way out, or in the wall cell beside one.
        let doors: Vec<_> = bp.props.iter().filter(|p| p.to.is_some() || c.story.prop(p.def).id == "door").collect();
        assert!(!doors.is_empty(), "{} seed {seed} has a way out", m.id);
        for d in &doors {
            let f = c.story.prop(d.def);
            let (x0, y0) = (i32::from(d.cell.x), i32::from(d.cell.y));
            let (x1, y1) = (x0 + i32::from(f.w) - 1, y0 + i32::from(f.h) - 1);
            for p in &lamps {
                let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
                let gap = (x0 - x).max(x - x1).max(0).max((y0 - y).max(y - y1).max(0));
                assert!(gap > 1, "{} seed {seed}: {} hangs over {}", m.id, name(bp, p.key), name(bp, d.key));
            }
        }
        for r in &b.info.rooms {
            let node = &m.nodes[r.node];
            let prefix = format!("{}_{}_lamp_", m.id, node.id);
            let mine = lamps
                .iter()
                .filter(|p| {
                    name(bp, p.key).strip_prefix(&prefix).is_some_and(|n| n.bytes().all(|c| c.is_ascii_digit()))
                })
                .count();
            if node.dark {
                assert_eq!(mine, 0, "{} seed {seed}: {} is dark on purpose", m.id, node.id);
            } else {
                assert!(mine >= 4, "{} seed {seed}: {} has {mine} lamps", m.id, node.id);
            }
        }
        // No standing torch is dressing: what stands on the floor was put there by the mission.
        let held: Vec<Key> = m.nodes.iter().flat_map(|n| n.holds.iter().map(|h| Key::Name(h.key))).collect();
        for p in &bp.props {
            if c.story.prop(p.def).id == "torch" {
                assert!(held.contains(&p.key), "{} seed {seed}: {} is a torch nobody asked for", m.id, name(bp, p.key));
            }
        }
        // Lamps on a state's switch are built as the state starts, and every control switches them both ways.
        if let Some((var, is)) = plan.state {
            let on = m.states[usize::from(var)].initial == is;
            for p in &lamps {
                assert_eq!(p.on, on, "{} seed {seed}: {}", m.id, name(bp, p.key));
            }
            let controls: Vec<_> = b.info.controls.iter().filter(|x| x.state == var).collect();
            assert!(!controls.is_empty(), "{}: nothing switches the lamps", m.id);
            for ctl in controls {
                let p = bp.props.iter().find(|p| p.key == ctl.prop).unwrap();
                let Some(&Action::If { then, els: Some(els), .. }) =
                    p.use_list.map(|l| list(bp, l)).unwrap_or_default().first()
                else {
                    panic!("{}: a control's list starts with its if", m.id)
                };
                let initial = m.states[usize::from(var)].initial;
                for (branch, value) in [(then, initial), (els, 1 - initial)] {
                    let actions = list(bp, branch);
                    for l in &lamps {
                        let want = Action::Switch { prop: l.key, on: Some(value == is) };
                        assert!(actions.contains(&want), "{} seed {seed}: {} is not switched", m.id, name(bp, l.key));
                    }
                }
            }
        }
    }
}

#[test]
fn heat_stays_within_its_cap_and_every_unit_is_at_the_dungeons_phase() {
    for (m, seed, b) in sweep() {
        let bp = &b.blueprint;
        let min_cost = m.budget.enemies.iter().map(|e| e.1).min().unwrap_or(0);
        for u in &bp.units {
            assert_eq!(u.phase, m.phase, "{} seed {seed}: {}", m.id, name(bp, u.key));
        }
        for r in &b.info.rooms {
            let node = &m.nodes[r.node];
            let template = catalog().dungeons.template(r.template);
            let prefix = format!("{}_{}_spawn_", m.id, node.id);
            let inside: Vec<_> = bp.units.iter().filter(|u| name(bp, u.key).starts_with(&prefix)).collect();
            let cost: u16 = inside
                .iter()
                .map(|u| m.budget.enemies.iter().find(|e| e.0 == u.def).expect("from the bestiary").1)
                .sum();
            let share = node.heat_cost.min(template.heat_max);
            assert!(cost <= share, "{} seed {seed}: {} costs {cost} of {share}", m.id, node.id);
            let spawns = template.sockets.iter().filter(|s| s.kind == RoomSocketKind::Spawn).count() as u16;
            if min_cost == 1 && spawns * 2 >= share {
                assert_eq!(cost, share, "{} seed {seed}: {} is under its share", m.id, node.id);
            }
            if node.kind == MissionNodeKind::Rest || node.heat.0 == 0 {
                assert!(inside.is_empty(), "{} seed {seed}: {} should be empty", m.id, node.id);
            }
            // At most two to a socket, and every one on open floor.
            for u in &inside {
                let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
                assert!(!solid(bp.tiles.read(x, y, Tile::Void)), "{} seed {seed}: {} in a wall", m.id, name(bp, u.key));
            }
        }
    }
}

#[test]
fn everything_stands_on_open_floor_inside_the_zone() {
    let c = catalog();
    for (m, seed, b) in sweep() {
        let bp = &b.blueprint;
        let wall_lamps: Vec<_> = m
            .lights
            .map(|l| l.family.to_vec())
            .into_iter()
            .flatten()
            .chain(
                m.nodes
                    .iter()
                    .flat_map(|n| n.holds.iter())
                    .filter_map(|h| match h.prop {
                        Some(MissionProp::Family(f)) => Some(f),
                        _ => None,
                    })
                    .flatten(),
            )
            .collect();
        for p in &bp.props {
            if wall_lamps.contains(&p.def) {
                continue;
            }
            let f = c.story.prop(p.def);
            let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(f.w), i32::from(f.h));
            assert!(r.right() <= bp.w() as i32 && r.bottom() <= bp.h() as i32);
            for (x, y) in r.cells() {
                let t = bp.tiles.read(x, y, Tile::Void);
                assert!(
                    !solid(t) || t == Tile::Glass || t == Tile::Rubble,
                    "{} seed {seed}: {} ({}) on {t:?}",
                    m.id,
                    name(bp, p.key),
                    f.id
                );
            }
        }
        for (k, mk) in &bp.marks {
            let t = bp.tiles.read(i32::from(mk.cell.x), i32::from(mk.cell.y), Tile::Void);
            assert!(!solid(t), "{} seed {seed}: mark {} on {t:?}", m.id, name(bp, *k));
        }
        for u in &bp.units {
            let t = bp.tiles.read(i32::from(u.cell.x), i32::from(u.cell.y), Tile::Void);
            assert!(!solid(t), "{} seed {seed}: unit {} on {t:?}", m.id, name(bp, u.key));
            // A patrol stays inside its room.
            if !u.patrol.is_empty() {
                let room = b
                    .info
                    .rooms
                    .iter()
                    .find(|r| r.bounds().contains(i32::from(u.cell.x), i32::from(u.cell.y)))
                    .unwrap();
                for w in &u.patrol {
                    assert!(
                        room.rect.contains(i32::from(w.cell.x), i32::from(w.cell.y)),
                        "{} seed {seed}: {} strays",
                        m.id,
                        name(bp, u.key)
                    );
                }
            }
        }
        for t in bp.triggers.values() {
            assert!(
                bp.rects.contains_key(&t.rect),
                "{} seed {seed}: a trigger over no rect ({})",
                m.id,
                name(bp, t.rect)
            );
        }
    }
}

#[test]
fn lockins_seal_behind_her_and_show_a_way_in() {
    for (m, seed, b) in sweep().iter().step_by(3) {
        let bp = &b.blueprint;
        for l in &b.info.lockins {
            let gate = bp.props.iter().find(|p| p.key == l.gate).expect("the gate");
            let way_in = bp.props.iter().find(|p| p.key == l.way_in).expect("the way in");
            assert!(way_in.hidden, "{} seed {seed}: the way in shows before the room seals", m.id);
            // In the dungeon's own stuff, never the plain grey stone row (the owner's playtest,
            // 2026-09-29: a grey block between the mine's rooms).
            let row = catalog().story.prop(way_in.def).id;
            assert_eq!(row, format!("{}_way_in", m.id), "{} seed {seed}: the way in is plain stone", m.id);
            assert_eq!(way_in.to.map(|d| (d.zone, d.mark)), Some((m.zone, l.mark)));
            let rect = bp.rects[&l.rect];
            let mark = bp.marks[&l.mark].cell;
            assert!(
                rect.contains(i32::from(mark.x), i32::from(mark.y)),
                "{} seed {seed}: she lands outside the fight",
                m.id
            );
            // The gate stands outside the rect, so it cannot drop on anyone (C12).
            assert!(
                !rect.contains(i32::from(gate.cell.x), i32::from(gate.cell.y)),
                "{} seed {seed}: gate inside",
                m.id
            );
            let lock = bp.triggers[&l.lock];
            let clear = bp.triggers[&l.clear];
            assert!(lock.reset.is_some(), "{} seed {seed}: a lock-in with no reset", m.id);
            let acts = list(bp, lock.actions);
            assert!(acts.contains(&Action::Lock(l.gate)) && acts.contains(&Action::Show(l.way_in)));
            let done = list(bp, clear.actions);
            assert!(done.contains(&Action::Unlock(l.gate)) && done.contains(&Action::Hide(l.way_in)));
            assert!(
                done.iter()
                    .any(|a| matches!(a, Action::Flag { key: FlagKey::Named(k), op: FlagOp::Set(1) } if *k == l.clear))
            );
        }
        let lockin_edges = m.edges.iter().filter(|e| {
            b.info.room_of(usize::from(e.to)).is_some()
                && b.info.room_of(usize::from(e.from)).is_some()
                && std::iter::once(&e.kind).chain(e.also).any(|k| matches!(k, MissionEdgeKind::Lockin { .. }))
        });
        assert_eq!(lockin_edges.count(), b.info.lockins.len(), "{} seed {seed}", m.id);
    }
}

#[test]
fn every_control_drives_its_gates_both_ways() {
    for (m, seed, b) in sweep().iter().step_by(4) {
        if m.states.is_empty() {
            continue;
        }
        let bp = &b.blueprint;
        for (si, sv) in m.states.iter().enumerate() {
            let controls: Vec<_> = b.info.controls.iter().filter(|c| usize::from(c.state) == si).collect();
            assert!(!controls.is_empty(), "{} seed {seed}: nothing controls {}", m.id, sv.id);
            let gates: Vec<(u8, Key)> = b
                .info
                .locks
                .iter()
                .filter_map(|l| match l.kind {
                    MissionEdgeKind::State { var, is, .. } if usize::from(var) == si => Some((is, l.prop)),
                    _ => None,
                })
                .collect();
            for &(is, g) in &gates {
                let p = bp.props.iter().find(|p| p.key == g).unwrap();
                assert_eq!(p.locked, is != sv.initial, "{} seed {seed}: {} as built", m.id, name(bp, g));
            }
            for ctl in controls {
                let p = bp.props.iter().find(|p| p.key == ctl.prop).unwrap();
                let first = p.use_list.map(|l| list(bp, l)).unwrap_or_default();
                let Some(&Action::If { then, els: Some(els), .. }) = first.first() else {
                    panic!("{} seed {seed}: {} does not start with its if", m.id, name(bp, ctl.prop))
                };
                for (branch, value, flag) in [(then, sv.initial, 0), (els, 1 - sv.initial, 1)] {
                    let acts = list(bp, branch);
                    assert_eq!(
                        acts[0],
                        Action::Flag { key: FlagKey::Named(Key::Name(sv.flag)), op: FlagOp::Set(flag) }
                    );
                    for &(is, g) in &gates {
                        let want = if is == value { Action::Unlock(g) } else { Action::Lock(g) };
                        assert!(
                            acts.contains(&want),
                            "{} seed {seed}: {} forgets {}",
                            m.id,
                            name(bp, ctl.prop),
                            name(bp, g)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn same_seed_same_dungeon_another_seed_another() {
    for m in missions() {
        let a = build_candidate(m.zone, 4242, 0);
        let b = build_candidate(m.zone, 4242, 0);
        assert_eq!(a.blueprint, b.blueprint, "{}", m.id);
        assert_eq!(a.info.layout, b.info.layout, "{}", m.id);
        let c = build_dungeon(m.zone, 4243, 0);
        assert_ne!(a.blueprint.tiles, c.tiles, "{}: another seed, the same dungeon", m.id);
        let d = build_dungeon(m.zone, 4242, 1);
        assert_ne!(a.blueprint.tiles, d.tiles, "{}: another attempt, the same dungeon", m.id);
    }
}

#[test]
fn layouts_really_differ_over_the_seeds() {
    for m in missions() {
        // The boss's room, else the last critical node's: where it stands moves with the seed.
        let node = m
            .nodes
            .iter()
            .position(|n| n.kind == MissionNodeKind::Boss)
            .unwrap_or_else(|| m.nodes.iter().rposition(|n| n.critical).unwrap());
        let mut bays: Vec<(u8, u8)> = sweep()
            .iter()
            .filter(|x| x.0.zone == m.zone)
            .take(32)
            .map(|x| {
                x.2.info.layout.as_ref().unwrap().placements.iter().find(|p| usize::from(p.node) == node).unwrap().bay
            })
            .collect();
        bays.sort();
        bays.dedup();
        assert!(bays.len() >= 3, "{}: {} stands in only {bays:?}", m.id, m.nodes[node].id);
    }
}

#[test]
fn the_fallback_is_whole_and_the_same_on_every_seed() {
    for m in missions() {
        let a = build_candidate(m.zone, 99, ZONE_ATTEMPTS - 1);
        assert!(a.info.errors.is_empty(), "{}: {:?}", m.id, a.info.errors);
        let layout = a.info.layout.as_ref().unwrap();
        assert!(layout.fallback);
        for (i, n) in m.nodes.iter().enumerate() {
            if n.critical {
                assert!(a.info.room_of(i).is_some(), "{}: the fallback leaves out {}", m.id, n.id);
            }
        }
        let b = build_candidate(m.zone, 7, ZONE_ATTEMPTS - 1);
        assert_eq!(b.info.layout, a.info.layout, "{}", m.id);
        assert_eq!(b.blueprint.tiles, a.blueprint.tiles, "{}", m.id);
    }
}

// --- stages 14 and 15 wired in: the solver and C1 to C13 judge every candidate ----------------

fn shown(faults: &[jane_world::dungeon::checks::Fault]) -> Vec<String> {
    faults.iter().map(ToString::to_string).collect()
}

/// `dungeon-gen.test.ts` (and every per-dungeon file) "64 seeds: every one is proven (solver and
/// C1 to C13) inside the attempt budget, and none needs the fallback". `build` already refused
/// every candidate the solver or a check would not pass; this proves the one it kept, again, and
/// reports how many attempts it took.
#[test]
fn every_dungeon_every_seed_is_proven_by_the_solver_and_c1_to_c12() {
    let mut report = String::new();
    for m in missions() {
        let (mut n, mut sum, mut worst, mut refused) = (0u32, 0u32, 0u8, 0usize);
        for (_, seed, b) in sweep().iter().filter(|x| x.0.zone == m.zone) {
            let faults = check_dungeon(b);
            assert!(faults.is_empty(), "{} seed {seed}: {:?}", m.id, shown(&faults));
            assert_eq!(b.info.rejected.len(), usize::from(b.blueprint.attempts) - 1, "{} seed {seed}", m.id);
            n += 1;
            sum += u32::from(b.blueprint.attempts);
            worst = worst.max(b.blueprint.attempts);
            refused += b.info.rejected.len();
        }
        // A dungeon is small and its pools are sound: a seed that needs more than a handful of
        // attempts is a pool or a mission to look at (the TypeScript held the mine to 4).
        assert!(worst <= 5, "{}: worst attempts {worst}", m.id);
        let _ = writeln!(
            report,
            "{}: {n} seeds, attempts mean {}.{:02} worst {worst}, {refused} refused",
            m.id,
            sum / n,
            sum * 100 / n % 100
        );
    }
    println!("{report}");
}

/// "The hand-placed fallback is a whole, proven dungeon: the last attempt never throws at the
/// player": it passes the solver and every check, on any seed.
#[test]
fn the_fallback_is_a_proven_dungeon() {
    for m in missions() {
        let b = build_candidate(m.zone, 99, ZONE_ATTEMPTS - 1);
        let faults = check_dungeon(&b);
        assert!(faults.is_empty(), "{}: {:?}", m.id, shown(&faults));
    }
}

/// A candidate the judge refuses is re-rolled and the refusal kept; the last attempt is judged
/// too, and kept with its reasons when even it is refused.
#[test]
fn a_refused_candidate_is_rerolled_and_the_last_is_judged_too() {
    let m = catalog().dungeons.mission_of(ZoneId::Mine).expect("the mine");
    assert_eq!(build_candidate(m.zone, 31, 0).info.errors, Vec::<String>::new(), "seed 31 embeds at once");
    let refuse_two = |b: &Built| if b.blueprint.attempts <= 2 { Err(vec!["not yet".to_owned()]) } else { Ok(()) };
    let b = build_with(m.zone, 31, &refuse_two);
    assert_eq!(b.blueprint.attempts, 3);
    assert_eq!(b.info.rejected, vec![(0, "not yet".to_owned()), (1, "not yet".to_owned())]);
    let refuse_all = |_: &Built| Err(vec!["never".to_owned()]);
    let b = build_with(m.zone, 31, &refuse_all);
    assert_eq!(b.blueprint.attempts, ZONE_ATTEMPTS);
    assert!(b.info.layout.as_ref().unwrap().fallback);
    assert_eq!(b.info.errors, vec!["never".to_owned()]);
    assert_eq!(b.info.rejected.len(), usize::from(ZONE_ATTEMPTS) - 1);
}
