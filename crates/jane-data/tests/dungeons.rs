//! Rule tests for the compiled dungeons group (PORT.md §9.2): the room templates and the eight
//! missions as the build left them. Carries the data-only parts of `jane/test/templates.test.ts`
//! ("every .room file parses and is clean in every transform it claims", "turning and mirroring
//! keep every socket on the floor and every door on the rim", "variants of a pool differ in
//! their grid and never in their contract; every pool a mission names exists") and of
//! `jane/test/dungeon-gen.test.ts` ("the contract derived from the mission's binds contains
//! every name the story already leans on", "generated names come from node and socket", the
//! fallback's "every critical node"). The seeded parts (layout, solver, C1-C12, the harness)
//! are P3's.

use jane_core::ids::{NameId, ZoneId};
use jane_data::{
    Catalog, MissionDef, MissionEdgeKind, MissionNameWhat, MissionProp, RoomCell, RoomSide, RoomSocketKind,
    RoomTemplate, catalog,
};

fn c() -> &'static Catalog {
    catalog()
}

fn name(n: NameId) -> &'static str {
    c().name(n)
}

fn missions() -> &'static [MissionDef] {
    c().dungeons.missions
}

#[test]
fn every_room_file_is_a_template_and_every_mission_a_dungeon_zone() {
    let d = &c().dungeons;
    assert_eq!(d.templates.len(), 109);
    assert_eq!(missions().len(), 8);
    let ids: Vec<&str> = missions().iter().map(|m| m.id).collect();
    assert_eq!(ids, ["burial", "factory", "forest", "library", "mine", "museum", "pipes", "school"]);
    for m in missions() {
        assert_eq!(Some(m.zone), ZoneId::from_name(m.id));
    }
    for (i, t) in d.templates.iter().enumerate() {
        assert!(i == 0 || d.templates[i - 1].id < t.id, "templates in id order");
        assert_eq!(d.pool(t.pool).id, t.id.rsplit_once('.').unwrap().0, "{}: pool is its id less the variant", t.id);
        assert!(d.pool(t.pool).templates.iter().any(|x| d.template(*x).id == t.id));
    }
}

#[test]
fn every_template_fits_in_every_transform_it_claims() {
    for t in c().dungeons.templates {
        let want = t.turns.len() * if t.mirror { 2 } else { 1 };
        assert_eq!(t.shapes.len(), want, "{}: every transform it claims fits", t.id);
        assert!(t.shape(jane_data::RoomTurn::R0, false).is_some(), "{} has its unturned shape", t.id);
        assert_eq!(t.rects[0].id, "room");
    }
}

fn check_shape(t: &RoomTemplate) {
    for s in t.shapes {
        assert_eq!(s.cells.len(), usize::from(s.w) * usize::from(s.h), "{}", t.id);
        for d in s.doors {
            let on_rim = match d.side {
                RoomSide::N => d.y == 0,
                RoomSide::S => d.y + 1 == s.h,
                RoomSide::W => d.x == 0,
                RoomSide::E => d.x + 1 == s.w,
            };
            assert!(on_rim, "{} turn {:?} door {}", t.id, s.turn, d.id);
            assert_eq!(s.cell(t.tiles, d.x, d.y), RoomCell::Door, "{} door {} is a door cell", t.id, d.id);
            assert_eq!(d.id, format!("{}{}", d.side.letter(), d.bay + 1));
        }
        for (k, b) in t.sockets.iter().zip(s.sockets) {
            assert_eq!((b.w * b.h) as u16, k.w * k.h, "{} {}", t.id, k.id);
            for (x, y) in b.cells() {
                assert_eq!(s.cell(t.tiles, x as u16, y as u16), RoomCell::Floor, "{} turn {:?} {}", t.id, s.turn, k.id);
            }
        }
        for m in s.marks {
            assert_eq!(s.cell(t.tiles, m.x, m.y), RoomCell::Floor, "{} mark", t.id);
        }
    }
    // The unturned shape is the grid as drawn.
    let s0 = t.shape(jane_data::RoomTurn::R0, false).unwrap();
    assert_eq!((s0.w, s0.h), (t.w, t.h));
    assert_eq!(
        s0.doors.iter().map(|d| (d.id, d.x, d.y)).collect::<Vec<_>>(),
        t.doors.iter().map(|d| (d.id, d.x, d.y)).collect::<Vec<_>>()
    );
}

#[test]
fn turning_and_mirroring_keep_every_socket_on_the_floor_and_every_door_on_the_rim() {
    for t in c().dungeons.templates {
        check_shape(t);
        for s in t.shapes {
            // A half turn keeps the size, a quarter turn swaps it.
            let quarter = matches!(s.turn, jane_data::RoomTurn::R90 | jane_data::RoomTurn::R270);
            assert_eq!((s.w, s.h), if quarter { (t.h, t.w) } else { (t.w, t.h) });
            // Doors lie on their bay's mouth line.
            for d in s.doors {
                match d.side {
                    RoomSide::N | RoomSide::S => {
                        assert_eq!(s.ox + i32::from(d.x), i32::from(d.bay) * jane_data::BAY_W + jane_data::MOUTH_X);
                    }
                    RoomSide::E | RoomSide::W => {
                        assert_eq!(s.oy + i32::from(d.y), i32::from(d.bay) * jane_data::BAY_H + jane_data::MOUTH_Y);
                    }
                }
            }
        }
    }
}

#[test]
fn variants_of_a_pool_differ_in_their_grid_and_never_in_their_contract() {
    let d = &c().dungeons;
    for p in d.pools {
        let face = |t: &RoomTemplate| {
            let req: Vec<&str> = t.doors.iter().filter(|x| x.required).map(|x| x.id).collect();
            let mut grants: Vec<&str> = t.grants.iter().map(|&g| t.sockets[usize::from(g)].id).collect();
            grants.sort();
            let blocks: Vec<(&str, &str)> = t
                .blocks
                .iter()
                .map(|b| (t.sockets[usize::from(b.what)].id, t.sockets[usize::from(b.until)].id))
                .collect();
            format!("{:?} {req:?} {:?} {:?} {grants:?} {blocks:?}", t.bays, t.needs_verbs, t.needs_items)
        };
        let first = d.template(p.templates[0]);
        for &id in &p.templates[1..] {
            assert_eq!(face(d.template(id)), face(first), "{} against {}", d.template(id).id, first.id);
        }
    }
}

#[test]
fn every_pool_a_mission_names_has_templates_with_its_sockets_binds_and_doors() {
    let d = &c().dungeons;
    for m in missions() {
        for n in m.nodes {
            let pool: Vec<&RoomTemplate> = d.templates_of(n.pool).collect();
            assert!(!pool.is_empty(), "{}: pool of {} is empty", m.id, n.id);
            let edges = m
                .edges
                .iter()
                .filter(|e| {
                    e.kind != MissionEdgeKind::Sight
                        && (m.nodes[usize::from(e.from)].id == n.id || m.nodes[usize::from(e.to)].id == n.id)
                })
                .count();
            for t in &pool {
                for h in n.holds {
                    assert!(
                        t.socket_index(h.socket).is_some(),
                        "{} has no socket {} for node {}",
                        t.id,
                        h.socket,
                        n.id
                    );
                }
                for b in n.binds {
                    let found = t.socket_index(b.from).is_some()
                        || t.mark_index(b.from).is_some()
                        || t.rect_index(b.from).is_some();
                    assert!(found, "{} has nothing called {} for node {}", t.id, b.from, n.id);
                }
                assert!(t.doors.len() >= edges, "{}: {edges} edges", t.id);
            }
        }
    }
}

#[test]
fn node_order_is_monotone_and_the_entrance_comes_first() {
    for m in missions() {
        for e in m.edges {
            let (a, b) = (&m.nodes[usize::from(e.from)], &m.nodes[usize::from(e.to)]);
            if a.critical && b.critical && !e.shortcut {
                assert!(a.order <= b.order, "{}: {} -> {} runs back", m.id, a.id, b.id);
            }
        }
        let entrances: Vec<_> = m.nodes.iter().filter(|n| n.kind == jane_data::MissionNodeKind::Entrance).collect();
        assert_eq!(entrances.len(), 1, "{}", m.id);
        assert_eq!(m.nodes.iter().map(|n| n.order).min(), Some(entrances[0].order), "{}", m.id);
        assert!(m.states.len() <= 3);
    }
}

#[test]
fn generated_names_come_from_node_and_socket() {
    let d = &c().dungeons;
    for m in missions() {
        for n in m.nodes {
            let pool: Vec<&RoomTemplate> = d.templates_of(n.pool).collect();
            assert_eq!(n.names.len(), pool.len());
            for (row, t) in n.names.iter().zip(&pool) {
                assert_eq!(d.template(row.template).id, t.id);
                assert_eq!(
                    (row.sockets.len(), row.marks.len(), row.rects.len()),
                    (t.sockets.len(), t.marks.len(), t.rects.len())
                );
                for (s, &nm) in t.sockets.iter().zip(row.sockets) {
                    let bound = n.binds.iter().find(|b| b.from == s.id);
                    let want = bound.map_or_else(
                        || format!("{}_{}_{}", m.id, n.id, s.id.replace(':', "_")),
                        |b| name(b.name).to_owned(),
                    );
                    assert_eq!(name(nm), want);
                }
                // A holding's key is its socket's name in every template of the pool.
                for h in n.holds {
                    let i = t.socket_index(h.socket).unwrap();
                    assert_eq!(row.sockets[i], h.key, "{} {}", n.id, h.socket);
                }
            }
            // Every placeholder binds to a name the mission provides.
            for &(at, bound) in n.at_names {
                assert!(name(at).starts_with('@'));
                assert!(
                    m.provides.iter().any(|p| p.name == bound),
                    "{}: {} -> {} is provided",
                    m.id,
                    name(at),
                    name(bound)
                );
            }
        }
    }
    // The mine's jars keep their names whatever the seed: a jar's id is its key.
    let mine = c().dungeons.mission_of(ZoneId::Mine).unwrap();
    let keys: Vec<&str> = mine.nodes.iter().flat_map(|n| n.holds).map(|h| name(h.key)).collect();
    for k in ["cabinet_jar", "mine_big_jar", "mine_vault_page_main", "plate_chest"] {
        assert!(keys.contains(&k), "{k}");
    }
    let office = mine.nodes.iter().find(|n| n.id == "office").unwrap();
    let cabinet = office.holds.iter().find(|h| h.socket == "verbprop:cabinet").unwrap();
    assert!(office.at_names.iter().any(|&(a, b)| name(a) == "@jar:cabinet" && name(b) == "cabinet_jar"));
    assert!(cabinet.use_list.is_some());
    let drawer = office.holds.iter().find(|h| h.socket == "prop:drawer").unwrap();
    assert_eq!(drawer.guarded_by.iter().map(|&g| name(g)).collect::<Vec<_>>(), ["headmaster"]);
    assert_eq!(drawer.free_trigger.map(name), Some("hm_drawer_free"));
}

#[test]
fn the_contract_derived_from_the_missions_binds_holds_every_name_the_story_leans_on() {
    // jane/src/world/index.ts CONTRACTS.mine, as the story wrote it before the mine was generated.
    let mine = c().dungeons.mission_of(ZoneId::Mine).unwrap();
    let has = |list: &[NameId], s: &str| list.iter().any(|&n| name(n) == s);
    for u in ["clerk", "headmaster", "iron_knuckles"] {
        assert!(has(mine.contract.units, u), "{u}");
    }
    for p in [
        "exit_door",
        "plate_a",
        "plate_chest",
        "store_chest",
        "gate_generic_a",
        "gate_generic_b",
        "gate_hm",
        "broken_steps",
        "boss_key_chest",
        "adit_door",
        "gate_vault",
        "vault_chest",
        "gate_boss",
    ] {
        assert!(has(mine.contract.props, p), "{p}");
    }
    for mk in ["entry", "adit"] {
        assert!(has(mine.contract.marks, mk), "{mk}");
    }
    for r in ["mine_entry", "boss_arena"] {
        assert!(has(mine.contract.rects, r), "{r}");
    }
    // Everything a contract names is something a blueprint of the mission can hold, as that kind.
    for m in missions() {
        for (list, what) in [
            (m.contract.units, MissionNameWhat::Unit),
            (m.contract.props, MissionNameWhat::Prop),
            (m.contract.marks, MissionNameWhat::Mark),
            (m.contract.rects, MissionNameWhat::Rect),
        ] {
            for &n in list {
                assert!(m.provides.iter().any(|p| p.name == n && p.what == what), "{}: {} as {what:?}", m.id, name(n));
            }
        }
        assert!(
            m.provides.windows(2).all(|w| (w[0].name, w[0].what) < (w[1].name, w[1].what)),
            "provides sorted, no repeats"
        );
        assert!(m.provides.iter().any(|p| p.name == m.all_rect && p.what == MissionNameWhat::Rect));
    }
}

#[test]
fn the_fallback_places_every_critical_node_from_its_own_pool() {
    let d = &c().dungeons;
    for m in missions() {
        for (i, n) in m.nodes.iter().enumerate() {
            let row = m.fallback.iter().find(|f| usize::from(f.node) == i);
            if n.critical {
                assert!(row.is_some(), "{}: the fallback places {}", m.id, n.id);
            }
            if let Some(f) = row {
                let t = d.template(f.template);
                assert_eq!(t.pool, n.pool, "{}: {}", m.id, t.id);
                let s =
                    t.shape(f.turn, f.mirror).unwrap_or_else(|| panic!("{}: {} in a transform it allows", m.id, t.id));
                assert!(
                    f.bay.0 + s.bays.0 <= m.cols && f.bay.1 + s.bays.1 <= m.rows,
                    "{}: {} inside the lattice",
                    m.id,
                    n.id
                );
            }
        }
    }
}

#[test]
fn edges_and_holdings_are_typed_as_the_generator_wants_them() {
    let mine = c().dungeons.mission_of(ZoneId::Mine).unwrap();
    let edge = |a: &str, b: &str| {
        mine.edges
            .iter()
            .find(|e| mine.nodes[usize::from(e.from)].id == a && mine.nodes[usize::from(e.to)].id == b)
            .unwrap()
    };
    // A key edge with its gateAs; the boss door carries its lock-in in `also`.
    let boss = mine.edges.iter().find(|e| e.gate.is_some_and(|g| name(g.name) == "gate_boss")).unwrap();
    assert!(matches!(boss.kind, MissionEdgeKind::Key { .. }));
    let MissionEdgeKind::Lockin { rect, mark, way_in, lock, clear, .. } = boss.also[0] else { panic!() };
    assert_eq!(
        [rect, mark, way_in, lock, clear].map(name),
        ["boss_arena", "mine_arena_in", "mine_arena_wayin", "mine_arena_lock", "mine_arena_clear"]
    );
    // A verb edge names its prop by propAs.
    let MissionEdgeKind::Verb { name: steps, .. } = edge("core", "gallery").kind else { panic!() };
    assert_eq!(name(steps), "broken_steps");
    // A oneway edge with no gateAs gets the generator's gate name.
    let oneway = edge("arena", "nook");
    assert_eq!(oneway.gate.map(|g| name(g.name)), Some("mine_gate_arena_nook"));
    let MissionEdgeKind::Oneway { trigger, .. } = oneway.kind else { panic!() };
    assert_eq!(name(trigger), "mine_arena_nook_opens");
    // A lamp family in a holding resolves to its four wall rows; a loot holding is a chest.
    for m in missions() {
        for h in m.nodes.iter().flat_map(|n| n.holds) {
            match (h.unit, h.prop) {
                (Some(_), None) | (None, Some(MissionProp::Row(_) | MissionProp::Family(_))) => {}
                other => panic!("{}: {} is {other:?}", m.id, h.socket),
            }
        }
        assert!(m.budget.enemies.windows(2).all(|w| w[0].0 < w[1].0));
    }
    // Heat: round(heat * base heat).
    let guard = mine.nodes.iter().find(|n| n.id == "guard").unwrap();
    assert_eq!((guard.heat.0, guard.heat_cost), (800, 5));
    // Rest rooms have no heat to spend.
    for m in missions() {
        for n in m.nodes.iter().filter(|n| n.kind == jane_data::MissionNodeKind::Rest) {
            assert_eq!(n.heat_cost, 0, "{}: {}", m.id, n.id);
        }
    }
}

#[test]
fn spawn_and_boss_sockets_stand_clear_of_every_door() {
    // Rule 3, read back from the compiled templates.
    for t in c().dungeons.templates {
        for s in t
            .sockets
            .iter()
            .filter(|s| matches!(s.kind, RoomSocketKind::Spawn | RoomSocketKind::Boss | RoomSocketKind::Unit))
        {
            for d in t.doors {
                let dist = (i32::from(s.x) - i32::from(d.x)).abs().max((i32::from(s.y) - i32::from(d.y)).abs());
                assert!(dist >= 4, "{} {} is {dist} from {}", t.id, s.id, d.id);
            }
        }
    }
}
