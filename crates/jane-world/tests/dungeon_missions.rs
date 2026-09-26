//! PORT.md §6.m stages 14 and 15, per dungeon: what each mission's own TypeScript file proved
//! with the solver and the first completion, without the sim. Carries, from
//! `jane/test/{dungeon-gen,burial,factory,forest,library,museum,pipes,school}.test.ts`: "the first
//! completion is the walk that was designed" (the designed order, inside its band), "the solver
//! rejects the layout without" a building's control, its spell or its key, the forest's "no
//! self-lockout", and the burial's stair and `everywhere`. The bots carry in P4.

use jane_core::action::{Action, FlagKey};
use jane_core::blueprint::Door;
use jane_core::{Blueprint, Key, Rect, Tile, ZoneId};
use jane_data::{MissionDef, catalog};
use jane_world::dungeon::checks::{check, first_completion, rules_of};
use jane_world::dungeon::{Built, build};
use jane_world::solve::ablate::Grant;
use jane_world::solve::model::{KeyTag, Options};
use jane_world::solve::report::Report;
use jane_world::solve::run::solve;

/// The seeds of the TypeScript suite, the first `n` of them.
fn seeds(n: u32) -> impl Iterator<Item = u32> {
    (0..n).map(|i| 31 + i * 7919)
}

fn mission(zone: ZoneId) -> &'static MissionDef {
    catalog().dungeons.mission_of(zone).expect("a generated dungeon")
}

fn key(bp: &Blueprint, name: &str) -> Key {
    if let Some(i) = bp.local_names.iter().position(|n| n == name) {
        return Key::Local(i as u32);
    }
    Key::Name(catalog().name_id(name).unwrap_or_else(|| panic!("no name \"{name}\"")))
}

fn walk_is(zone: ZoneId, n: u32, want: &[&str], check_order: impl Fn(&[&str]) -> bool) {
    let m = mission(zone);
    for seed in seeds(n) {
        let b = build(zone, seed);
        let walk = first_completion(&b.blueprint, &b.info);
        assert_eq!(walk.errors, Vec::<String>::new(), "{} seed {seed}", m.id);
        let order: Vec<&str> = walk.order.iter().map(|&i| m.nodes[i].id).collect();
        assert!(check_order(&order), "{} seed {seed}: walked {order:?}, designed {want:?}", m.id);
        let (lo, hi) = m.budget.crit_path_cells;
        assert!(
            (i32::from(lo)..=i32::from(hi)).contains(&walk.cells),
            "{} seed {seed}: {} cells, band {lo} to {hi}",
            m.id,
            walk.cells
        );
    }
}

fn walk_exactly(zone: ZoneId, n: u32, want: &[&str]) {
    walk_is(zone, n, want, |order| order == want);
}

#[test]
fn the_mines_first_completion_is_the_walk_that_was_designed() {
    // The rest room before the Headmaster, the boss last but one.
    walk_exactly(
        ZoneId::Mine,
        8,
        &["entry", "plate", "store", "guard", "core", "firstaid", "office", "gallery", "vault", "arena", "nook"],
    );
}

#[test]
fn the_burials_first_completion_is_the_walk_that_was_designed() {
    // The fire before the corners, Goldskin last but one.
    walk_exactly(
        ZoneId::Burial,
        6,
        &[
            "entry",
            "hall",
            "alcove",
            "east",
            "rat",
            "garden",
            "orchard",
            "vigil",
            "lockin",
            "glasshouse",
            "snake",
            "web",
            "nursery",
            "dark",
            "parade",
            "vault",
            "stair",
        ],
    );
}

#[test]
fn the_works_first_completion_is_the_mission_in_order() {
    walk_exactly(
        ZoneId::Factory,
        8,
        &[
            "yard",
            "loading",
            "lockers",
            "line_a",
            "time_office",
            "vent",
            "generator",
            "gen_shutter",
            "press_hall",
            "office",
            "assembly",
            "roller_door",
        ],
    );
}

#[test]
fn the_forests_first_completion_is_the_walk_that_was_designed() {
    // The hearth off the ring, the stone last but one.
    let head = ["gate", "first_glade", "hut", "ring", "hearth"];
    let tail = ["stone", "reward"];
    walk_is(ZoneId::Forest, 8, &head, |order| order.starts_with(&head) && order.ends_with(&tail));
}

#[test]
fn the_librarys_first_completion_is_the_walk_that_was_designed() {
    // The fire before the last book, and the way back is short.
    walk_exactly(ZoneId::Library, 8, &["entry", "stacks", "nook", "shelf"]);
}

#[test]
fn the_museums_first_completion_is_the_mission_in_order() {
    walk_exactly(
        ZoneId::Museum,
        8,
        &[
            "entry",
            "atrium",
            "history",
            "cloakroom",
            "toilets",
            "maintenance",
            "arts",
            "science",
            "stores",
            "magic",
            "magic_case",
        ],
    );
}

#[test]
fn the_culverts_first_completion_is_the_mission_in_order() {
    walk_exactly(
        ZoneId::Pipes,
        8,
        &["sump", "north_run", "junction", "chamber", "valve_house", "east_run", "west_run", "outfall"],
    );
}

#[test]
fn the_schools_first_completion_is_the_mission_in_order() {
    walk_exactly(
        ZoneId::School,
        8,
        &[
            "boiler",
            "hall",
            "sick_bay",
            "corridor",
            "top_corridor",
            "woodwork",
            "chemistry",
            "botany",
            "physics",
            "domestic",
            "ice_house",
            "tower",
            "top_room",
        ],
    );
}

// --- the solver without a building's control, spell or key -------------------------------

/// The whole solve and a `without` solve of a seed, traced, with the mission's rules.
fn ablated(b: &Built, grant: Grant) -> (Report, Report) {
    let rules = rules_of(b.info.mission);
    let opts = Options { trace: true, ..Options::default() };
    (solve(&b.blueprint, &rules, &opts), solve(&b.blueprint, &rules, &opts.without(grant)))
}

fn spell(id: &str) -> jane_core::SpellId {
    let i = catalog().combat.spells.iter().position(|s| s.id == id).unwrap_or_else(|| panic!("no spell {id}"));
    jane_core::SpellId(i as u16)
}

fn lines(b: &Built, r: &Report) -> String {
    r.lines(&b.blueprint).join("\n")
}

#[test]
fn the_works_are_refused_without_the_board_without_spark_and_without_the_call_box() {
    for seed in seeds(4) {
        let b = build(ZoneId::Factory, seed);
        let bp = &b.blueprint;
        let (whole, no_board) = ablated(&b, Grant::Prop(key(bp, "factory_board")));
        assert!(whole.ok(), "seed {seed}: {}", lines(&b, &whole));
        assert_eq!(whole.info.layers, vec![0, 1], "seed {seed}");
        // The stores are behind a shutter that only a started generator lifts.
        assert_eq!(no_board.info.layers, vec![0], "seed {seed}");
        // Spark is found inside, and everything after it needs it.
        let (_, no_spark) = ablated(&b, Grant::Verb(spell("spark")));
        assert!(!no_spark.ok());
        let t = lines(&b, &no_spark);
        assert!(t.contains("factory_reward_chest") || t.contains("factory_big_jar"), "seed {seed}: {t}");
        // And the office is behind a wall a hauler has to be called through.
        let (_, no_wall) = ablated(&b, Grant::Flag(FlagKey::Named(key(bp, "factory_wall_down"))));
        assert!(!no_wall.ok(), "seed {seed}");
    }
}

#[test]
fn the_museum_is_refused_without_the_breaker_and_without_explosion() {
    for seed in seeds(4) {
        let b = build(ZoneId::Museum, seed);
        let (whole, without) = ablated(&b, Grant::Prop(key(&b.blueprint, "museum_breaker")));
        assert!(whole.ok(), "seed {seed}: {}", lines(&b, &whole));
        assert_eq!(whole.info.layers, vec![0, 1]);
        // The dark wing really is the breaker's.
        assert!(!without.ok(), "seed {seed}");
        assert_eq!(without.info.layers, vec![0]);
        // And without Explosion the stores open in neither state, which is the whole point of them.
        let (_, no_blast) = ablated(&b, Grant::Verb(spell("explosion")));
        assert!(!no_blast.ok());
        assert!(lines(&b, &no_blast).contains("museum_attendant_key_chest"), "seed {seed}: {}", lines(&b, &no_blast));
    }
}

#[test]
fn the_culvert_is_refused_without_the_valve_and_no_manhole_opens_without_repair() {
    for seed in seeds(4) {
        let b = build(ZoneId::Pipes, seed);
        let (whole, without) = ablated(&b, Grant::Prop(key(&b.blueprint, "pipes_valve")));
        assert!(whole.ok(), "seed {seed}: {}", lines(&b, &whole));
        assert_eq!(whole.info.layers, vec![0, 1]);
        // The west run really is the valve's.
        assert!(!without.ok(), "seed {seed}");
        assert_eq!(without.info.layers, vec![0]);
        let (_, no_repair) = ablated(&b, Grant::Verb(spell("repair")));
        assert_eq!(no_repair.info.fired(key(&b.blueprint, "pipes_north_ladder")), None, "seed {seed}");
    }
}

#[test]
fn the_school_is_refused_without_the_bell_rope_and_again_without_the_caretakers_key() {
    for seed in seeds(4) {
        let b = build(ZoneId::School, seed);
        let bp = &b.blueprint;
        let (whole, no_rope) = ablated(&b, Grant::Prop(key(bp, "school_bell_rope")));
        assert!(whole.ok(), "seed {seed}: {}", lines(&b, &whole));
        assert_eq!(whole.info.layers, vec![0, 1]);
        // No rope, no break: three lessons, the tower and the ending are on the other side of it.
        assert!(!no_rope.ok());
        assert_eq!(no_rope.info.layers, vec![0]);
        let t = lines(&b, &no_rope);
        assert!(t.contains("school_clock_physics") || t.contains("school_physics_fuse"), "seed {seed}: {t}");
        // The six clocks are not enough on their own: the rope will not ring nine without his key.
        let (_, no_key) = ablated(&b, Grant::Key(KeyTag::Tag(key(bp, "school_tower"))));
        assert!(!no_key.ok());
        let t = lines(&b, &no_key);
        assert!(
            t.contains("school_tower_door") || t.contains("ringer") || t.contains("school_register_book"),
            "seed {seed}: {t}"
        );
    }
}

// --- the forest and the burial ------------------------------------------------------------

#[test]
fn no_self_lockout_grow_every_hedge_seed_shut_and_the_forest_can_still_be_finished() {
    // The generator cannot express this, so it is written against the built blueprint: take
    // every `fill` a hedge seed would do, lay it into the tiles, and solve the zone again. A
    // closed gap must only ever take a way round away, never the only way.
    let cat = catalog();
    let hedge_seed = cat.story.prop_id("hedge_seed").expect("the hedge seed row");
    let mut closed = 0;
    for seed in seeds(24) {
        let b = build(ZoneId::Forest, seed);
        let mut bp = b.blueprint.clone();
        let mut fills: Vec<Rect> = Vec::new();
        for p in bp.props.iter().filter(|p| p.def == hedge_seed) {
            for a in p.use_list.and_then(|l| bp.list(l)).unwrap_or(&[]) {
                if let Action::Fill { rect, tile } = *a {
                    assert_eq!(tile, Tile::Hedge, "a hedge seed grows a hedge");
                    fills.push(*bp.rects.get(&rect).expect("the rect it fills"));
                }
            }
        }
        assert_eq!(fills.len(), 5, "seed {seed}: the zone should hold five closable gaps");
        closed += fills.len();
        for r in fills {
            bp.tiles.fill_rect(r, Tile::Hedge);
        }
        let v = solve(&bp, &rules_of(b.info.mission), &Options::default());
        assert!(v.ok(), "seed {seed}: with every gap grown shut: {:?}", v.lines(&bp));
        let faults: Vec<String> = check(&bp, &b.info).iter().map(ToString::to_string).collect();
        assert_eq!(faults, Vec::<String>::new(), "seed {seed}: with every gap grown shut");
    }
    assert_eq!(closed, 24 * 5);
}

#[test]
fn the_burials_stair_up_is_the_way_into_the_school_behind_goldskin_and_everywhere_is_the_whole_zone() {
    let b = build(ZoneId::Burial, 31);
    let bp = &b.blueprint;
    // All thirteen zones exist statically (PORT.md §6.l), so the stair always leads into the
    // School's boiler room (DUNGEONS.md §3.6: the School's entrance). The room it stands in opens
    // when Goldskin is down, and not before; the stair itself is locked to Julie's Other Key, which
    // the dog gives with the School (the mission's `givenKeys`), so she goes up when she is sent.
    let stair = bp.props.iter().find(|p| p.key == key(bp, "stair_up")).expect("the stair up");
    assert!(stair.locked);
    let tag = jane_data::catalog().name_id("school_stair").expect("the tag");
    assert_eq!(stair.key_tag, Some(jane_core::Key::Name(tag)));
    assert_eq!(stair.to, Some(Door { zone: ZoneId::School, mark: key(bp, "boiler") }));
    let at = Rect::new(i32::from(stair.cell.x), i32::from(stair.cell.y), 1, 1);
    let (whole, alive) = ablated(&b, Grant::Flag(FlagKey::Named(key(bp, "burial_cleared"))));
    assert!(whole.ok() && whole.info.reached_rect(at.grow(1)), "{}", lines(&b, &whole));
    assert!(!alive.info.reached_rect(at.grow(1)), "the stair while he stands");
    // The story's own trigger rows say `everywhere`; the generator calls the whole map `<zone>_all`.
    let whole = Rect::new(0, 0, bp.w() as i32, bp.h() as i32);
    assert_eq!(bp.rects.get(&key(bp, "everywhere")), Some(&whole));
    assert_eq!(bp.rects.get(&key(bp, "burial_all")), Some(&whole));
}
