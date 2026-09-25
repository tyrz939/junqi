//! The county's authored places against the real catalog (PORT.md §6.f, §9.2). They were code in
//! `jane/src/world/chunks.ts` (`CHUNKS`); now they are `data/chunks/*.chunk`, and these rules are what
//! the county builder leans on: every place the TypeScript drew is here at its size, the names the
//! county's contract needs come from them, every gate can be walked in at, and every slot a row or a
//! door asks for is offered.

use jane_core::Tile;
use jane_core::tile::F_SOLID;
use jane_data::{Catalog, ChunkDef, ChunkSide, DoorAt, NameKind, PlaceAt, ProvidedBy};

fn cat() -> &'static Catalog {
    jane_data::catalog()
}

fn name(c: &Catalog, n: jane_core::ids::NameId) -> &'static str {
    c.name(n)
}

/// `CHUNKS` in chunks.ts, with each box's size.
const TS_CHUNKS: [(&str, u16, u16); 16] = [
    ("station", 36, 34),
    ("julie_house", 42, 28),
    ("town", 100, 70),
    ("farm", 60, 44),
    ("car_wood", 15, 11),
    ("gold_mine", 30, 24),
    ("graveyard", 44, 32),
    ("burial", 19, 16),
    ("reed_camp", 34, 26),
    ("canteen", 34, 26),
    ("museum", 40, 30),
    ("library", 32, 26),
    ("butterfly_forest", 30, 24),
    ("lake_statue", 20, 18),
    ("factory", 56, 40),
    ("school", 60, 44),
];

#[test]
fn every_place_the_typescript_drew_is_data_at_its_size() {
    let c = cat();
    for (id, w, h) in TS_CHUNKS {
        let ch = c.chunks.get(id).unwrap_or_else(|| panic!("no chunk {id}"));
        assert_eq!((ch.w, ch.h), (w, h), "{id}");
        assert_eq!(c.county.site(ch.site).id, id, "{id} stands at its own site");
        assert_eq!(ch.cells.len(), usize::from(w) * usize::from(h), "{id}");
    }
    assert_eq!(c.chunks.defs.len(), TS_CHUNKS.len(), "a chunk the TypeScript never drew");
}

/// Is a box cell open to feet: ground that does not block, and no solid prop over it?
fn walkable(c: &Catalog, ch: &ChunkDef, x: u16, y: u16) -> bool {
    if ch.tile(x, y).flags() & F_SOLID != 0 {
        return false;
    }
    !ch.props.iter().any(|p| {
        let d = c.story.prop(p.prop.def);
        d.solid && x >= p.at.x && y >= p.at.y && x < p.at.x + u16::from(d.w) && y < p.at.y + u16::from(d.h)
    })
}

#[test]
fn every_gate_is_on_the_ring_and_walked_in_at() {
    let c = cat();
    for ch in c.chunks.defs {
        assert!(!ch.gates.is_empty(), "{}: no gate", ch.id);
        assert!(ch.gates.iter().any(|g| g.side == ch.face), "{}: no gate on its face", ch.id);
        for &g in ch.gates {
            let (x, y) = ch.gate_cell(g);
            let (w, h) = (i32::from(ch.w), i32::from(ch.h));
            let on_ring = match g.side {
                ChunkSide::N => y == -1 && (0..w).contains(&x),
                ChunkSide::S => y == h && (0..w).contains(&x),
                ChunkSide::W => x == -1 && (0..h).contains(&y),
                ChunkSide::E => x == w && (0..h).contains(&y),
            };
            assert!(on_ring, "{}: gate {g:?} is not one step outside the box", ch.id);
            let inner = ch.gate_inner(g);
            assert!(walkable(c, ch, inner.x, inner.y), "{}: gate {g:?} leads into a wall", ch.id);
        }
    }
}

/// world/index.ts CONTRACTS: the names the county promises on every seed that its set places draw.
#[test]
fn the_county_contract_s_set_places_are_drawn_by_chunks() {
    let c = cat();
    let drawn = |kind: NameKind, s: &str| c.chunks.exports().any(|(k, n)| k == kind && name(c, n) == s);
    for (kind, s) in [
        (NameKind::Unit, "dog"),
        (NameKind::Unit, "yard_skeleton"),
        (NameKind::Prop, "house_door"),
        (NameKind::Prop, "mine_door"),
        (NameKind::Prop, "adit_door"),
        (NameKind::Prop, "burial_door"),
        (NameKind::Mark, "start"),
        (NameKind::Mark, "house_front"),
        (NameKind::Mark, "mine_mouth"),
        (NameKind::Mark, "mine_adit"),
        (NameKind::Mark, "burial_mouth"),
        (NameKind::Rect, "stoop"),
        (NameKind::Rect, "mine_yard"),
    ] {
        assert!(drawn(kind, s), "no chunk draws {kind:?} {s}");
    }
    // And every name the county's zones.json contract holds is drawn by a chunk or provided by
    // another county table: nothing in it is a promise only.
    let others: Vec<_> = c.county.provides().into_iter().filter(|p| p.by != ProvidedBy::Zones).collect();
    for (kind, n) in c.county.zone(jane_core::ids::ZoneId::County).contract.names() {
        let by_chunk = c.chunks.exports().any(|e| e == (kind, n));
        let by_row = others.iter().any(|p| p.kind == kind && p.name == n);
        assert!(by_chunk || by_row, "the county promises {kind:?} {} and nothing draws it", name(c, n));
    }
}

#[test]
fn the_names_quests_used_to_find_in_code_are_drawn() {
    let c = cat();
    let drawn = |kind: NameKind, s: &str| c.chunks.exports().any(|(k, n)| k == kind && name(c, n) == s);
    for (kind, s) in [
        (NameKind::Prop, "station_lamp"),
        (NameKind::Unit, "sixpence"),
        (NameKind::Mark, "fountain"),
        (NameKind::Rect, "street_west"),
        (NameKind::Rect, "street_east"),
        (NameKind::Rect, "halt_approach"),
        (NameKind::Rect, "platform"),
    ] {
        assert!(drawn(kind, s), "no chunk draws {kind:?} {s}");
    }
}

/// placements.ts `at.slot` and county.ts's landmark doors: every slot asked for is offered.
#[test]
fn every_slot_a_row_or_a_door_asks_for_is_offered() {
    let c = cat();
    let offered = |s: &str| c.chunks.defs.iter().any(|ch| ch.slots.iter().any(|x| name(c, x.name) == s));
    for p in c.county.placements {
        if let PlaceAt::Slot(n) = p.at {
            // `quarry_adit` is a dressed area's (areas.ts), which is still code.
            let s = name(c, n);
            assert!(offered(s) || s == "quarry_adit", "placement {}: no chunk offers slot {s}", name(c, p.key));
        }
    }
    for d in c.county.doors {
        if let DoorAt::Chunk(site) = d.at {
            let id = c.county.site(site).id;
            let ch = c.chunks.at_site(site).unwrap_or_else(|| panic!("door {}: no chunk at {id}", name(c, d.key)));
            let want = format!("{id}_door");
            assert!(ch.slots.iter().any(|s| name(c, s.name) == want), "{id} offers no {want} slot");
        }
    }
}

#[test]
fn nothing_a_chunk_draws_leaves_its_box() {
    let c = cat();
    for ch in c.chunks.defs {
        let inside = |x: u16, y: u16, w: u16, h: u16| x + w <= ch.w && y + h <= ch.h;
        for p in ch.props {
            let d = c.story.prop(p.prop.def);
            assert!(inside(p.at.x, p.at.y, u16::from(d.w), u16::from(d.h)), "{}: prop {:?}", ch.id, p.key);
        }
        for u in ch.units {
            assert!(inside(u.at.x, u.at.y, 1, 1), "{}: unit", ch.id);
            assert!(walkable(c, ch, u.at.x, u.at.y), "{}: a unit stands in a wall", ch.id);
            for wp in u.route {
                assert!(inside(wp.at.x, wp.at.y, 1, 1), "{}: waypoint", ch.id);
            }
        }
        for m in ch.marks {
            assert!(inside(m.at.x, m.at.y, 1, 1), "{}: mark {}", ch.id, name(c, m.name));
        }
        for s in ch.slots {
            assert!(inside(s.at.x, s.at.y, 1, 1), "{}: slot {}", ch.id, name(c, s.name));
        }
        let rects = ch.rects.iter().map(|r| r.rect).chain(ch.claims.iter().copied());
        for r in rects.chain(ch.fills.iter().flat_map(|f| f.cells.iter().copied())) {
            assert!(r.x >= 0 && r.y >= 0 && r.w >= 1 && r.h >= 1, "{}: {r:?}", ch.id);
            assert!(r.x + r.w <= i32::from(ch.w) && r.y + r.h <= i32::from(ch.h), "{}: {r:?}", ch.id);
        }
        assert!(ch.anchor.x < ch.w && ch.anchor.y < ch.h, "{}: anchor", ch.id);
    }
}

/// county.ts `boxAt` and the station's `cx: 4`: every box is centred on its site, kept 8 cells in,
/// but the halt's, which stands on the line.
#[test]
fn boxes_stand_centred_on_their_sites_but_the_halt_on_the_line() {
    let c = cat();
    assert_eq!(c.chunks.tuning.box_margin, 8);
    for ch in c.chunks.defs {
        assert_eq!((ch.anchor.x, ch.anchor.y), (ch.w / 2, ch.h / 2), "{}", ch.id);
        let pin = ch.pin.map(|p| (p.side, p.at));
        assert_eq!(pin, if ch.id == "station" { Some((ChunkSide::W, 4)) } else { None }, "{}", ch.id);
    }
    let st = c.chunks.get("station").expect("station");
    assert_eq!(st.origin(300, 300, 2000, 2000, 8), (4, 283));
    let town = c.chunks.get("town").expect("town");
    assert_eq!(town.origin(300, 300, 2000, 2000, 8), (250, 265));
    assert_eq!(town.origin(3, 1999, 2000, 2000, 8), (8, 2000 - 70 - 8));
    // The halt's platform runs along the line: its box's west edge is rail, top to bottom.
    assert!((0..st.h).all(|y| st.tile(0, y) == Tile::Rail));
    let halt = st.around.iter().find(|a| name(c, a.name) == "halt_approach").expect("halt_approach");
    assert_eq!((halt.n, halt.e, halt.s, halt.w), (30, 30, 30, 4));
}

/// The graveyard's coffins were thrown by `k.spot` in chunks.ts: now a fill, kept off the path
/// between the gates the way the TypeScript's `|cy + 1 - my| > 4` kept them.
#[test]
fn the_graveyard_s_coffins_are_a_fill_off_the_path() {
    let c = cat();
    let g = c.chunks.get("graveyard").expect("graveyard");
    assert_eq!(g.fills.len(), 1);
    let f = c.chunks.fill(&g.fills[0]);
    assert_eq!((f.id, c.story.prop(f.def).id), ("graveyard_coffins", "coffin"));
    let my = i32::from(g.h) / 2;
    for r in g.fills[0].cells {
        assert!(r.y < my - 3 || r.y > my + 3, "a coffin could lie across the path at row {}", r.y);
    }
    // Every other chunk is a pure grid.
    assert!(c.chunks.defs.iter().filter(|ch| ch.id != "graveyard").all(|ch| ch.fills.is_empty()));
}

#[test]
fn julie_s_door_is_locked_to_her_key_and_opens_on_the_house() {
    let c = cat();
    let j = c.chunks.get("julie_house").expect("julie_house");
    let door = j.props.iter().find(|p| p.key.is_some_and(|k| name(c, k) == "house_door")).expect("house_door");
    assert!(door.prop.locked);
    assert_eq!(door.prop.key_tag.map(|t| name(c, t)), Some("auntie_house"));
    let to = door.to.expect("a way in");
    assert_eq!((to.zone, name(c, to.mark)), (jane_core::ids::ZoneId::House, "front"));
}
