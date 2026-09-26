//! The county group's rules against the real catalog (PORT.md §9.2: content_* rule tests). Each test
//! names the TypeScript it carries: `jane/src/world/index.ts` (CONTRACTS, ZONE_IDS),
//! `jane/test/county.test.ts`, `jane/test/stories.test.ts`, `jane/test/tales.test.ts`. The seeded
//! halves of those files are the world crate's (P2).

use jane_core::ids::{NameId, ZoneId};
use jane_data::{Catalog, NameKind, PlaceAt, PlaceKind, ProvidedBy, Region, ZoneKind};

fn cat() -> &'static Catalog {
    jane_data::catalog()
}

fn n(s: &str) -> NameId {
    cat().name_id(s).unwrap_or_else(|| panic!("no name \"{s}\" in the catalog"))
}

/// world/index.ts ZONE_IDS: the scheduler's order, the five original zones then the rest by file name.
#[test]
fn zones_are_the_thirteen_in_tick_order() {
    let c = &cat().county;
    assert_eq!(c.zones.iter().map(|z| z.id).collect::<Vec<_>>(), ZoneId::ALL);
    for z in c.zones {
        let want = match z.id {
            ZoneId::County => ZoneKind::County,
            ZoneId::House | ZoneId::Cellar | ZoneId::Arms | ZoneId::Church => ZoneKind::Interior,
            _ => ZoneKind::Dungeon,
        };
        assert_eq!(z.kind, want, "{}", z.id.name());
        assert_eq!(z.mission.is_some(), want == ZoneKind::Dungeon, "{}", z.id.name());
        // Every hand-built zone gates nothing on a spell (GIVEN_VERBS leaves them out) and has no states.
        if want != ZoneKind::Dungeon {
            assert_eq!(z.given_verbs, None, "{}", z.id.name());
            assert!(z.states.is_empty(), "{}", z.id.name());
        }
    }
}

/// world/index.ts CONTRACTS and GIVEN_KEYS, and the placement promises the county contract takes on.
#[test]
fn the_county_contract_is_the_story_s_names_and_what_placements_promise() {
    let c = &cat().county;
    let has = |kind, s| c.contract_has(ZoneId::County, kind, n(s));
    for m in ["start", "house_front", "mine_mouth", "mine_adit", "burial_mouth"] {
        assert!(has(NameKind::Mark, m), "mark {m}");
    }
    for p in ["house_door", "mine_door", "adit_door", "burial_door"] {
        assert!(has(NameKind::Prop, p), "prop {p}");
    }
    assert!(has(NameKind::Rect, "stoop"));
    assert!(has(NameKind::Unit, "yard_skeleton"));
    assert_eq!(c.zone(ZoneId::County).given_keys, [n("auntie_house")]);
    // A placed row is promised; a story's row and an edit are not (placements.ts placementContract).
    assert!(has(NameKind::Prop, "lost_property_book"));
    assert!(has(NameKind::Prop, "washing_church"));
    assert!(!has(NameKind::Prop, "car_wreck"), "an edit: the chunk promised it");
    for p in c.placements.iter().filter(|p| p.at_place()) {
        for k in p.keys {
            assert!(!c.promised.names().any(|(_, x)| x == *k), "{} is a story's row", cat().name(*k));
        }
    }
    // The burial's triggers name `everywhere`, which its generator calls `<zone>_all` (burial.ts).
    assert!(c.contract_has(ZoneId::Burial, NameKind::Rect, n("everywhere")));
    assert!(c.contract_has(ZoneId::House, NameKind::Prop, n("julies_note")));
    assert!(c.contract_has(ZoneId::Arms, NameKind::Mark, n("entry")));
}

/// county.test.ts "doors into the dungeons", PORT.md §6.l: every row applies, into one of the thirteen.
#[test]
fn every_door_leads_into_a_real_zone() {
    let c = &cat().county;
    assert!(c.doors.len() >= 10);
    assert!(cat().name_id("icehouse_door").is_none(), "the dead icehouse row is gone");
    for d in c.doors {
        let key = cat().name(d.key);
        assert_ne!(d.zone, ZoneId::County, "{key}");
        assert_ne!(c.zone(d.zone).kind, ZoneKind::County, "{key}");
        match d.to {
            // A way that opens from below is a mark and a cover, not a door down.
            None => assert!(d.mark.is_some(), "{key}: from below, and no mark to arrive at"),
            Some(m) => assert_eq!(cat().name(m), "entry", "{key}"),
        }
    }
    let manholes: Vec<_> =
        c.doors.iter().filter(|d| d.to.is_none()).filter_map(|d| d.mark).map(|m| cat().name(m)).collect();
    assert_eq!(manholes, ["manhole_1", "manhole_2", "manhole_3", "manhole_4"]);
    for z in [ZoneId::Museum, ZoneId::Library, ZoneId::Forest, ZoneId::Factory, ZoneId::School, ZoneId::Pipes] {
        assert!(c.doors.iter().any(|d| d.zone == z && d.to.is_some()), "a way into {}", z.name());
    }
}

/// county.test.ts "every anchor, dressed area, footpath end and chunk slot exists on every seed, by
/// name": the data half. Every anchor gives a mark and a rect of its name; every footpath its end marks.
#[test]
fn the_places_the_story_needs_are_provided_by_name() {
    let c = &cat().county;
    let provided = c.provides();
    let county = |kind, name| provided.iter().any(|p| p.zone == ZoneId::County && p.kind == kind && p.name == name);
    for a in c.anchors {
        assert!(county(NameKind::Mark, a.id) && county(NameKind::Rect, a.id), "anchor {}", cat().name(a.id));
    }
    for m in ["hedge_stile_town", "hedge_stile_farm", "top_track_farm", "quarry_track_mine", "allotment_path_town"] {
        assert!(county(NameKind::Mark, n(m)), "path end {m}");
    }
    for s in c.stories {
        assert!(county(NameKind::Rect, s.place_name) && county(NameKind::Mark, s.place_name), "story_{}", s.key);
    }
    // Nothing is provided twice in one zone (the compile refuses it; this is the proof it ran).
    for (i, p) in provided.iter().enumerate() {
        let dup = provided[..i].iter().any(|q| q.zone == p.zone && q.kind == p.kind && q.name == p.name);
        let rename =
            p.by != ProvidedBy::Zones && c.placements.iter().any(|r| r.edit.and_then(|e| e.key) == Some(p.name));
        assert!(!dup || rename, "{:?} {} twice", p.kind, cat().name(p.name));
    }
}

/// The skeleton's rows: the first walk is station, Julie's house, town, in that order (place.ts,
/// FIRST_WALK), each site leans only on earlier ones, and every patch a story names is required.
#[test]
fn the_skeleton_rows_hold_together() {
    let c = &cat().county;
    let ids: Vec<&str> = c.sites.iter().map(|s| s.id).collect();
    assert_eq!(ids[..3], ["station", "julie_house", "town"]);
    for (i, s) in c.sites.iter().enumerate() {
        assert!(s.dist.iter().all(|d| usize::from(d.to) < i), "{}", s.id);
        assert!(s.road_from.is_none_or(|r| usize::from(r) < i), "{}", s.id);
    }
    // Three regions, and a rest in each (skeleton/index.ts "every region has somewhere to rest").
    for r in [Region::Lowfields, Region::Waters, Region::Works] {
        assert!(c.sites.iter().any(|s| s.region == r && s.rest), "{r:?}");
    }
    for a in c.areas {
        assert!(a.threat <= 6 && a.radius > 0, "{}", cat().name(a.id));
    }
    for p in c.placements {
        if let PlaceAt::Area(i) = p.at {
            assert!(c.area(i).required, "{} is in a patch that may be skipped", cat().name(p.key));
        }
    }
    assert_eq!(c.anchor(0).id, n("halt_well"));
}

/// stories.test.ts "there are thirty-odd of them, most short, some chains, some that send her to a camp".
#[test]
fn thirty_odd_stories_most_short_some_chains_some_camps() {
    let c = &cat().county;
    let country: Vec<_> = c.stories.iter().filter(|s| !s.tale).collect();
    let told: Vec<_> = country.iter().filter(|s| !s.quests.is_empty()).collect();
    let quests: usize = told.iter().map(|s| s.quests.len()).sum();
    let chains: Vec<_> = told.iter().filter(|s| s.quests.len() >= 2).collect();
    let camps = told
        .iter()
        .filter(|s| country.iter().any(|h| h.near.is_some_and(|x| x.story == s.id) && h.kind == PlaceKind::Camp))
        .count();
    assert!(told.len() >= 25, "{}", told.len());
    assert!((30..=40).contains(&country.len()), "{}", country.len());
    assert!(quests >= 30, "{quests}");
    assert!((6..=8).contains(&chains.len()), "{}", chains.len());
    assert!(chains.iter().all(|s| s.quests.len() <= 3));
    assert!(camps >= 3, "{camps}");
    // Every story has rows at its place (the compile refuses one without).
    for s in c.stories {
        assert!(
            c.placements.iter().any(|p| matches!(p.at, PlaceAt::Place { story, .. } if story == s.id)),
            "{}",
            s.key
        );
        assert_eq!(c.story(s.id).key, s.key);
    }
}

/// tales.test.ts "eight to twelve of them, each two or three steps deep, every one with a fixed name of its own".
#[test]
fn eight_to_twelve_tales_with_names_of_their_own_across_the_three_regions() {
    let c = &cat().county;
    let tales: Vec<_> = c.stories.iter().filter(|s| s.tale).collect();
    assert!((8..=12).contains(&tales.len()), "{}", tales.len());
    let mut regions = Vec::new();
    for t in &tales {
        assert!(t.name.is_some(), "{}", t.key);
        assert!((1..=3).contains(&t.quests.len()), "{}", t.key);
        if !regions.contains(&t.region) {
            regions.push(t.region);
        }
    }
    regions.sort();
    assert_eq!(regions, [Region::Lowfields, Region::Waters, Region::Works]);
    // One story spreads so far (ARCHITECTURE.md §4.6.e; `tests/living.rs` has the row).
    let spreads: Vec<&str> = c.stories.iter().filter(|s| s.spreads.is_some()).map(|s| s.key).collect();
    assert_eq!(spreads, ["ames"]);
}

/// stories.test.ts "every name a seed could paint on a board is its own": every kind has a pool,
/// deeper than its stories by ten, and every board line says the name.
#[test]
fn every_kind_of_place_has_names_to_spare() {
    let c = &cat().county;
    assert_eq!(c.names.iter().map(|p| p.kind).collect::<Vec<_>>(), PlaceKind::ALL);
    for p in c.names {
        let stories = c.stories.iter().filter(|s| s.kind == p.kind && s.name.is_none()).count();
        assert!(!p.names.is_empty() && !p.boards.is_empty(), "{}", p.kind.name());
        if p.kind != PlaceKind::Inn {
            assert!(p.names.len() >= stories + 10, "{}: {} names, {stories} stories", p.kind.name(), p.names.len());
        }
        for b in p.boards {
            assert!(cat().text(*b).contains("{NAME}"), "{}: {}", p.kind.name(), cat().text(*b));
        }
    }
    assert_eq!(c.pool(PlaceKind::Inn).names.len(), 1, "every inn is the Halfway House");
    // The woodcutters' clearings are roots by ends, less the doubled (names.ts baseList).
    let woods = c.pool(PlaceKind::Woodcutter).names;
    assert!(woods.iter().any(|w| cat().text(*w) == "Hazel Copse"));
    assert!(!woods.iter().any(|w| cat().text(*w) == "Coppice Coppice Wood"));
}
