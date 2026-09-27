//! PORT.md §6.m stage 14: the solver. Carries the solver parts of `jane/test/world.test.ts` ("the
//! solver really rejects a sealed gate"), `dungeon-verbs.test.ts` ("the validator reads the
//! blueprint's rows too", "what the solver learned": verbs, when, hops), `verbs2.test.ts` ("the
//! solver reads both branches", "the stateful flood: a building with a breaker") and the solver
//! half of `dungeon-gen.test.ts`, on hand-built blueprints: the zone generators are ported
//! beside this, and "every zone, every seed" lands with them.

use jane_core::action::{Action, Cond, Condition, FlagOp, FlagTest, Stack};
use jane_core::blueprint::{Door, Trigger, TriggerMode};
use jane_core::{Key, ZoneId};
use jane_data::NameKind;
use jane_world::solve::{Grant, KeyTag, Options, Owner, RowFault, Sketch, SolveError, ZoneRules, lock_holds, solve};

/// Three rooms in a row: A (x 1..=3), the hub (x 5..=7), B (x 9..=11), rows 1..=3; a gate may
/// stand in each doorway column (x 4, x 8). She comes in at the hub.
fn three_rooms() -> Sketch {
    let mut k = Sketch::new(
        ZoneId::Arms,
        &["#############", "#...........#", "#...........#", "#...........#", "#############"],
    );
    k.mark("start", 6, 2);
    k
}

fn gate(k: &mut Sketch, name: &str, x: u16, locked: bool, tag: Option<&str>) -> Key {
    let g = k.prop(name, "gate_v", x, 1);
    k.bp.props[g].locked = locked;
    k.bp.props[g].key_tag = tag.map(|t| k.name(t));
    k.bp.props[g].key
}

fn chest(k: &mut Sketch, name: &str, x: u16, y: u16, item: &str) -> Key {
    let c = k.prop(name, "chest", x, y);
    k.bp.props[c].loot = vec![Stack { item: k.item(item), qty: 1 }];
    k.bp.props[c].key
}

fn flag_is(k: &mut Sketch, name: &str) -> Cond {
    Cond { not: false, c: Condition::Flag { key: k.flag(name), test: FlagTest::NonZero } }
}

#[test]
fn a_room_behind_a_locked_door_with_its_key_in_a_chest_is_solved() {
    let mut k = three_rooms();
    let far = k.mark("far", 2, 2);
    let door = gate(&mut k, "door", 4, true, Some("generic"));
    let ch = chest(&mut k, "chest", 10, 2, "key_generic");
    k.rules.contract.props = vec![door, ch];
    k.rules.contract.marks = vec![far];
    let r = solve(&k.bp, &k.rules, &Options { trace: true, ..Options::default() });
    assert!(r.ok(), "{:?}", r.lines(&k.bp));
    // The chest is looted on the first flood and the door opened on it; room A is reached on the next.
    assert_eq!((r.info.fired(ch), r.info.fired(door)), (Some(0), Some(0)));
    assert_eq!(r.info.passes, 2);
    assert_eq!((r.info.first_seen_at(9, 1), r.info.first_seen_at(2, 2)), (Some(0), Some(1)));
    assert_eq!(r.info.layers, vec![0]);
    // A chest stands on its front row; its back row is floor she walks behind it (`base`).
    assert_eq!(r.info.reached_cells, 11 * 3 - 2, "every floor cell but the chest's front row");
}

/// `world.test.ts` "the solver really rejects a sealed gate": the key behind the door it opens.
#[test]
fn a_key_behind_the_door_it_opens_is_a_sealed_gate_and_is_rejected() {
    let mut k = three_rooms();
    let far = k.mark("far", 2, 2);
    let door = gate(&mut k, "boss_door", 4, true, Some("generic"));
    chest(&mut k, "chest", 1, 1, "key_generic");
    k.rules.contract.props = vec![door];
    let r = k.validate();
    assert!(!r.ok());
    assert!(r.errors.contains(&SolveError::MarkUnreachable(far)), "{:?}", r.lines(&k.bp));
    assert!(r.errors.contains(&SolveError::GateNeverOpens(door)));
    assert!(r.lines(&k.bp).join(" ").contains("gate \"boss_door\" can never be opened"));
    // And a key tag nothing in the zone carries is as sealed.
    let mut k = three_rooms();
    k.mark("far", 2, 2);
    let door = gate(&mut k, "boss_door", 4, true, Some("no_such_key"));
    chest(&mut k, "chest", 9, 1, "key_generic");
    k.rules.contract.props = vec![door];
    assert!(k.validate().errors.contains(&SolveError::GateNeverOpens(door)));
}

#[test]
fn a_key_the_story_hands_over_opens_the_door_and_withheld_it_does_not() {
    let mut k = three_rooms();
    let far = k.mark("far", 2, 2);
    gate(&mut k, "door", 4, true, Some("generic"));
    let generic = k.name("generic");
    assert!(k.validate().errors.contains(&SolveError::MarkUnreachable(far)));
    k.rules.given_keys = vec![generic];
    assert!(k.validate().ok());
    let r = k.bp.marks[&far].cell;
    let behind = jane_core::Rect::new(i32::from(r.x), i32::from(r.y), 1, 1);
    assert!(lock_holds(&k.bp, &k.rules, &Options::default(), Grant::Key(KeyTag::Tag(generic)), behind));
}

#[test]
fn a_plate_under_a_barrel_opens_its_gate() {
    let mut k = three_rooms();
    k.mark("far", 2, 2);
    let door = gate(&mut k, "door", 4, true, None);
    let l = k.list(vec![Action::Unlock(door)]);
    let plate = k.prop("plate", "plate", 9, 1);
    k.bp.props[plate].use_list = Some(l);
    k.prop("barrel", "barrel", 7, 1);
    let r = k.validate();
    assert!(r.ok(), "{:?}", r.lines(&k.bp));
    let plate_key = k.bp.props[plate].key;
    let without = solve(&k.bp, &k.rules, &Options::default().without(Grant::Prop(plate_key)));
    assert!(!without.ok());
}

/// `dungeon-verbs.test.ts` "verbs: Repair is learned from the drawer's page, and nothing that
/// answers Repair fires without it".
#[test]
fn a_repair_waits_for_the_spell_and_ungated_it_mends_itself() {
    let mut k = Sketch::new(ZoneId::Arms, &["##########", "#....#####", "#........#", "#........#", "##########"]);
    k.mark("start", 1, 3);
    let far = k.mark("far", 8, 2);
    let st = k.prop("steps", "broken_steps", 5, 2);
    let steps = k.bp.props[st].key;
    let mend = k.list(vec![Action::Hide(steps)]);
    k.bp.props[st].use_list = Some(mend);
    let wood = k.item("wood");
    k.bp.props[st].needs = vec![Stack { item: wood, qty: 2 }];
    let d = k.prop("drawer", "page_repair", 1, 1);
    let drawer = k.bp.props[d].key;
    let page = k.list(vec![Action::Learn(k.spell("repair"))]);
    k.bp.props[d].use_list = Some(page);
    let c = k.prop("planks", "chest", 3, 1);
    k.bp.props[c].loot = vec![Stack { item: wood, qty: 2 }];
    k.rules.contract.props = vec![steps];

    // Ungated (no verbs), a broken thing mends itself, which is how the older zones are judged.
    assert!(k.validate().ok(), "{:?}", k.validate().lines(&k.bp));
    k.rules.given_verbs = Some(vec![k.spell("icebolt")]);
    let whole = k.validate();
    assert!(whole.ok(), "{:?}", whole.lines(&k.bp));
    assert!(whole.info.knows(k.spell("repair")));
    assert!(whole.info.fired(steps) >= whole.info.fired(drawer));
    let without = solve(&k.bp, &k.rules, &Options::default().without(Grant::Verb(k.spell("repair"))));
    assert_eq!(without.errors, vec![SolveError::MarkUnreachable(far)]);
    assert_eq!(without.info.fired(steps), None);
    // Without the planks it cannot be mended either.
    let planks = k.bp.props[c].key;
    assert!(!solve(&k.bp, &k.rules, &Options::default().without(Grant::Prop(planks))).ok());
}

/// A lock-in (`dungeon-verbs.test.ts` "when"): walking into the arena stands the guard up and
/// shuts the way in; the way on opens once everything it stood up is dead. The solver skips the
/// lock, kills the guard on its mark, and lets the `while` row fire.
#[test]
fn a_lock_in_opens_once_what_it_stood_up_is_dead() {
    let build = |cleared: bool| {
        let mut k = three_rooms();
        let far = k.mark("far", 10, 2);
        let way_in = gate(&mut k, "way_in", 4, false, None);
        let way_on = gate(&mut k, "way_on", 8, true, None);
        let arena = k.rect("arena", 5, 1, 3, 3);
        let at = k.mark("lockin_a", 7, 3);
        let guard = k.name("lockin_guard");
        let def = k.cat.combat.unit_id("skeleton").unwrap();
        let seal = k.list(vec![Action::Lock(way_in), Action::Spawn { key: guard, def, at }]);
        let open = k.list(vec![Action::Unlock(way_on), Action::Unlock(way_in)]);
        let when = if cleared {
            k.conds(vec![Cond { not: false, c: Condition::Dead(guard) }])
        } else {
            let c = flag_is(&mut k, "test_never_set");
            k.conds(vec![c])
        };
        let (a, b) = (k.name("seal"), k.name("cleared"));
        k.bp.triggers.insert(
            a,
            Trigger { rect: arena, mode: TriggerMode::Enter, once: true, when: None, actions: seal, reset: None },
        );
        k.bp.triggers.insert(
            b,
            Trigger { rect: arena, mode: TriggerMode::While, once: true, when: Some(when), actions: open, reset: None },
        );
        (k, far, guard)
    };
    let (k, _, guard) = build(true);
    let r = k.validate();
    assert!(r.ok(), "{:?}", r.lines(&k.bp));
    assert!(r.info.dead.contains(&guard));
    let (k, far, _) = build(false);
    assert_eq!(k.validate().errors, vec![SolveError::MarkUnreachable(far)]);
}

/// `verbs2.test.ts` "the solver reads both branches: a `then` waits for its flag, and a lever
/// that works once does not get a second pull".
#[test]
fn the_solver_reads_both_branches() {
    let run = |switch_def: &str, power: bool, withhold_power: bool, backwards: bool| {
        let mut k = three_rooms();
        let far = k.mark("far", 10, 2);
        let g = gate(&mut k, "g", 8, true, None);
        let cond = flag_is(&mut k, if backwards { "never_set" } else { "test_power" });
        let when = k.conds(vec![cond]);
        let nothing = k.list(vec![Action::Toast(jane_core::TextRef::Local(0))]);
        let opens = k.list(vec![Action::Unlock(g)]);
        let l = if backwards {
            k.list(vec![Action::If { when, then: nothing, els: Some(opens) }])
        } else {
            k.list(vec![Action::If { when, then: opens, els: Some(nothing) }])
        };
        let sw = k.prop("sw", switch_def, 5, 1);
        k.bp.props[sw].use_list = Some(l);
        if power {
            let set = k.flag("test_power");
            let pl = k.list(vec![Action::Flag { key: set, op: FlagOp::Add(1) }]);
            let p = k.prop("power", "lever", 7, 3);
            k.bp.props[p].use_list = Some(pl);
            if withhold_power {
                let key = k.bp.props[p].key;
                k.opts.withhold.props.push(key);
            }
        }
        let errors = k.validate().errors;
        (errors, far)
    };
    // The switch is reached before the power is on. It can be pulled again, so its `then` is kept.
    assert_eq!(run("lever", true, false, false).0, vec![]);
    let (e, far) = run("lever", true, true, false);
    assert_eq!(e, vec![SolveError::MarkUnreachable(far)]);
    // `else` alone opens nothing.
    let (e, far) = run("lever", false, false, false);
    assert_eq!(e, vec![SolveError::MarkUnreachable(far)]);
    // A lever that works once was spent on the `else`: no second pull.
    let (e, far) = run("hoist_lever", true, false, false);
    assert_eq!(e, vec![SolveError::MarkUnreachable(far)]);
    // `else` is a branch too: what it unlocks is unlocked.
    assert_eq!(run("lever", false, false, true).0, vec![]);
}

/// A breaker: lights off (the start), gate A shut and gate B open; lights on, the other way.
fn breaker_building(breaker_at: (u16, u16)) -> (Sketch, Key, Key) {
    let mut k = three_rooms();
    let a = k.mark("room_a", 2, 2);
    k.mark("room_b", 10, 2);
    let gate_a = gate(&mut k, "gate_a", 4, true, None);
    let gate_b = gate(&mut k, "gate_b", 8, false, None);
    let lit = k.flag("test_lit");
    let is_lit = flag_is(&mut k, "test_lit");
    let when = k.conds(vec![is_lit]);
    let off = k.list(vec![Action::Flag { key: lit, op: FlagOp::Set(0) }, Action::Lock(gate_a), Action::Unlock(gate_b)]);
    let on = k.list(vec![Action::Flag { key: lit, op: FlagOp::Set(1) }, Action::Unlock(gate_a), Action::Lock(gate_b)]);
    let l = k.list(vec![Action::If { when, then: off, els: Some(on) }]);
    let p = k.prop("breaker", "lever", breaker_at.0, breaker_at.1);
    k.bp.props[p].use_list = Some(l);
    k.rules.states = vec![lit];
    let breaker = k.bp.props[p].key;
    (k, a, breaker)
}

/// `verbs2.test.ts` "solver: accepts it, reaches both states, and rejects the same layout with
/// the control withheld".
#[test]
fn a_breaker_flipping_two_gates_is_solved_in_both_states() {
    let (k, a, breaker) = breaker_building((6, 1));
    let opts = Options { trace: true, ..Options::default() };
    let whole = solve(&k.bp, &k.rules, &opts);
    assert!(whole.ok(), "{:?}", whole.lines(&k.bp));
    assert_eq!(whole.info.layers, vec![0, 1]);
    let without = solve(&k.bp, &k.rules, &opts.without(Grant::Prop(breaker)));
    assert_eq!(without.errors, vec![SolveError::MarkUnreachable(a)]);
    assert_eq!(without.info.layers, vec![0]);
    // A state gate is a lock like any other: withhold the control, and room A is never entered.
    assert!(lock_holds(&k.bp, &k.rules, &opts, Grant::Prop(breaker), jane_core::Rect::new(1, 1, 3, 3)));
}

/// `verbs2.test.ts` "solver: you are where you stood when you pulled it. A breaker behind a
/// dark-only door leaves her shut in with the lights on".
#[test]
fn you_are_where_you_stood_when_you_pulled_it() {
    let (k, a, _) = breaker_building((10, 1));
    let r = solve(&k.bp, &k.rules, &Options { trace: true, ..Options::default() });
    // Both states are reached, and every cell of room A is open in one of them: a flood that
    // started each state from the front door would call this finished.
    assert_eq!(r.info.layers, vec![0, 1]);
    assert_eq!(r.errors, vec![SolveError::MarkUnreachable(a)]);
    assert_eq!(r.info.first_seen_at(2, 2), None);
}

#[test]
fn more_than_three_states_is_refused() {
    let mut k = three_rooms();
    let states = ["s1", "s2", "s3", "s4"].iter().map(|n| k.flag(n)).collect();
    k.rules.states = states;
    assert_eq!(k.validate().errors, vec![SolveError::TooManyStates(4)]);
}

/// `dungeon-verbs.test.ts` "a `to` into the same zone is a one-way edge".
#[test]
fn a_door_into_this_zone_is_a_one_way_edge_over_a_gate_that_never_opens() {
    let mut k = three_rooms();
    let far = k.mark("far", 10, 2);
    gate(&mut k, "shut", 8, true, None);
    let v = k.prop("vent", "lever", 7, 1);
    k.bp.props[v].to = Some(Door { zone: ZoneId::Arms, mark: far });
    let r = k.validate();
    assert!(r.ok(), "{:?}", r.lines(&k.bp));
    k.bp.props[v].hidden = true;
    assert_eq!(k.validate().errors, vec![SolveError::MarkUnreachable(far)]);
}

// --- the checks before the flood (ARCHITECTURE.md §5.3 point 2) -------------------------------

#[test]
fn a_missing_contract_name_is_refused_by_name() {
    let mut k = three_rooms();
    let (u, p, m, r) = (k.name("dog"), k.name("house_door"), k.name("house_front"), k.name("stoop"));
    k.rules.contract.units = vec![u];
    k.rules.contract.props = vec![p];
    k.rules.contract.marks = vec![m];
    k.rules.contract.rects = vec![r];
    let e = k.validate();
    assert_eq!(
        e.errors,
        vec![
            SolveError::MissingUnit(u),
            SolveError::MissingProp(p),
            SolveError::MissingMark(m),
            SolveError::MissingRect(r)
        ]
    );
    assert_eq!(e.lines(&k.bp)[1], "missing prop \"house_door\"");
}

#[test]
fn a_key_written_twice_is_refused() {
    let mut k = three_rooms();
    k.prop("twice", "chest", 1, 1);
    k.prop("twice", "chest", 9, 1);
    k.unit("rat", "rat", 2, 3);
    k.unit("rat", "rat", 3, 3);
    let twice = k.name("twice");
    let rat = k.name("rat");
    assert_eq!(k.validate().errors, vec![SolveError::DuplicateProp(twice), SolveError::DuplicateUnit(rat)]);
}

#[test]
fn nothing_stands_in_a_wall_or_outside_the_zone() {
    let mut k = three_rooms();
    let m = k.mark("in_the_wall", 0, 0);
    let u = k.unit("stuck", "rat", 4, 0);
    let p = k.prop("overhang", "chest", 12, 4);
    let unit = k.bp.units[u].key;
    let prop = k.bp.props[p].key;
    assert_eq!(
        k.validate().errors,
        vec![SolveError::MarkInWall(m), SolveError::UnitInWall(unit), SolveError::PropOutside(prop)]
    );
}

#[test]
fn no_entrance_is_an_error_not_a_guess() {
    let mut k = Sketch::new(ZoneId::Arms, &["...."]);
    k.mark("somewhere", 1, 0);
    assert_eq!(k.validate().errors, vec![SolveError::NoEntrance]);
    let somewhere = k.name("somewhere");
    k.opts.entry = Some(somewhere);
    assert!(k.validate().ok());
}

/// `dungeon-verbs.test.ts` "the validator reads the blueprint's rows too: a clash with the
/// catalog, a prop that is not there, a bad row, a rect that is not there, a door to nowhere".
#[test]
fn the_blueprints_own_rows_are_checked() {
    let base = || {
        let mut k = Sketch::new(ZoneId::Mine, &["######", "#....#", "#....#", "######"]);
        k.mark("entry", 1, 1);
        // The catalog's mine trigger needs its rect.
        k.rect("mine_entry", 1, 1, 2, 2);
        k
    };
    assert!(base().validate().ok(), "{:?}", base().validate().lines(&base().bp));

    let mut k = base();
    let arena = k.rect("arena", 1, 1, 1, 1);
    let none = k.list(vec![]);
    let into_mine = k.name("into_mine");
    k.bp.triggers.insert(
        into_mine,
        Trigger { rect: arena, mode: TriggerMode::Enter, once: true, when: None, actions: none, reset: None },
    );
    let r = k.validate();
    assert_eq!(r.errors, vec![SolveError::TriggerClash(into_mine)]);
    assert!(r.lines(&k.bp)[0].contains("\"into_mine\" is in the blueprint and in the catalog"));

    let mut k = base();
    let arena = k.rect("arena", 1, 1, 1, 1);
    let gone = k.name("no_such_gate");
    let bad = jane_core::SpellId(u16::MAX);
    let l = k.list(vec![Action::Lock(gone), Action::Learn(bad)]);
    let lock = k.name("arena_lock");
    k.bp.triggers.insert(
        lock,
        Trigger { rect: arena, mode: TriggerMode::Enter, once: true, when: None, actions: l, reset: None },
    );
    let r = k.validate();
    assert_eq!(
        r.errors,
        vec![
            SolveError::BadRow { at: Owner::Trigger(lock), fault: RowFault::UnknownSpell(bad) },
            SolveError::NoSuchName { at: Owner::Trigger(lock), kind: NameKind::Prop, name: gone },
        ]
    );
    assert!(r.lines(&k.bp)[1].contains("no prop \"no_such_gate\""));

    let mut k = base();
    let nowhere = k.name("nowhere");
    let l = k.list(vec![]);
    let lock = k.name("arena_lock");
    k.bp.triggers.insert(
        lock,
        Trigger { rect: nowhere, mode: TriggerMode::Enter, once: true, when: None, actions: l, reset: None },
    );
    let r = k.validate();
    assert!(r.lines(&k.bp)[0].contains("no rect \"nowhere\""), "{:?}", r.lines(&k.bp));

    let mut k = base();
    let p = k.prop("way_in", "lever", 3, 1);
    let nowhere = k.name("nowhere");
    k.bp.props[p].to = Some(Door { zone: ZoneId::Mine, mark: nowhere });
    let r = k.validate();
    assert!(r.lines(&k.bp)[0].contains("leads to mark \"nowhere\""));
}

#[test]
fn a_list_naming_a_prop_the_blueprint_lacks_is_refused_unless_it_is_a_fragment() {
    let mut k = three_rooms();
    let gone = k.name("in_another_room");
    let l = k.list(vec![Action::Unlock(gone)]);
    let p = k.prop("lever", "lever", 6, 1);
    k.bp.props[p].use_list = Some(l);
    let lever = k.bp.props[p].key;
    assert_eq!(
        k.validate().errors,
        vec![SolveError::NoSuchName { at: Owner::Prop(lever), kind: NameKind::Prop, name: gone }]
    );
    // A piece of a zone (one room, stamped alone): the name is simply absent, and the lever does nothing.
    k.opts.fragment = true;
    assert!(k.validate().ok());
}

// --- the rules content gives each zone --------------------------------------------------------

#[test]
fn zone_rules_come_from_the_catalog() {
    let cat = jane_data::catalog();
    let n = |s: &str| Key::Name(cat.name_id(s).unwrap_or_else(|| panic!("no name {s}")));
    let county = ZoneRules::for_zone(ZoneId::County);
    assert!(county.contract.units.contains(&n("dog")));
    assert!(county.contract.marks.contains(&n("start")));
    assert!(county.given_keys.contains(&n("auntie_house")));
    assert_eq!(county.given_verbs, None);
    assert_eq!(county.entrances[0], n("start"));
    assert!(county.contract.props.len() > 4, "the placement rows' promises are part of the county's contract");

    let mine = ZoneRules::for_zone(ZoneId::Mine);
    assert!(mine.contract.units.contains(&n("headmaster")));
    assert!(mine.given_verbs.is_some(), "a dungeon is judged against the spells she brings");

    let burial = ZoneRules::for_zone(ZoneId::Burial);
    assert!(burial.contract.rects.contains(&n("everywhere")), "a dungeon zone's own names join its mission's");

    let cellar = ZoneRules::for_zone(ZoneId::Cellar);
    assert!(cellar.entrances.contains(&n("stair_a")));
    for z in ZoneId::ALL {
        let r = ZoneRules::for_zone(z);
        assert!(r.states.len() <= jane_world::solve::MAX_STATES, "{z:?}");
    }
}
