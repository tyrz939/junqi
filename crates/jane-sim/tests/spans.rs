//! MAP.md R2, spans, on the hand-built `viaduct` (`jane_sim::span::viaduct`: a valley at level 0
//! between two terraces at level 1, a viaduct 4 wide over it along y at x 30 to 33, a broken
//! footbridge at x 46 and 47, a stair down the north face and one up the south rim). She walks
//! over the deck and under it; nothing climbs onto it from beneath; its sides are parapets; the
//! deck and the ground under it are held apart; sight and shots keep to their layer and the deck
//! is a slab between; blows never cross; paths take the deck along its axis only; a chaser comes
//! along the deck to her; a save on the deck loads on it; two seats over and under stay in
//! lockstep; a broken span mended carries feet and saves so.

mod field;

use std::sync::Arc;

use field::{Z, body_of, cmd, edit, events, learn, rested, spawn, steps, unit, walk};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::search::PathEnd;
use jane_core::{Angle, Blueprint, Cell, Key, Milli, Tile, Vec2, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::grid::ZoneGrid;
use jane_sim::los::line_of_sight_on;
use jane_sim::path::{PathAsk, PathScratch, cost_of_cells};
use jane_sim::span::{self, viaduct as V};
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, ClientToken, Command, DevOp, InputFrame, Sim, StepInput, UnitId};

fn blueprints() -> Blueprints {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            return Arc::new(V::blueprint(z));
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        Arc::new(bp)
    });
    Blueprints::from_parts(7, zones)
}

fn world() -> Sim {
    let mut s = Sim::new_game_with(blueprints(), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    s
}

fn her(s: &Sim) -> UnitId {
    body_of(s, 0)
}

/// Stand `id` on cell (x, y): on the viaduct's deck if `deck`, else on the ground.
fn put(s: &mut Sim, id: UnitId, x: i32, y: i32, deck: bool) {
    edit(s, id, |u| {
        u.pos = Vec2::centre(x, y);
        u.on_span = deck.then_some(0);
    });
}

fn grid(s: &Sim) -> &ZoneGrid {
    &s.runtime(Z).expect("the viaduct").grid
}

fn cell_of(s: &Sim, id: UnitId) -> (i32, i32) {
    unit(s, id).pos.cell()
}

fn foe_at(s: &mut Sim, x: i32, y: i32, deck: bool) -> UnitId {
    let id = spawn(s, "quarryman", x, y);
    field::rooted(s, id);
    put(s, id, x, y, deck);
    id
}

fn hurt(ev: &[jane_sim::Event], who: UnitId) -> bool {
    ev.iter().any(|e| matches!(e.kind, EventKind::Damage { unit, .. } if unit == who))
}

#[test]
fn the_viaduct_is_well_formed_and_the_footbridge_broken() {
    let s = world();
    let g = grid(&s);
    assert!(g.spans().iter().all(|sp| span::well_formed(g, sp)));
    assert!(g.span_whole(0) && !g.span_whole(1));
    assert_eq!(g.deck_at(31, 24), Some(0));
    assert_eq!(g.deck_at(46, 24), None, "a broken span has no deck");
}

// --- walking ---------------------------------------------------------------------------------------

#[test]
fn she_walks_over_the_deck_end_to_end() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 31, 14, false);
    let mut on = 0;
    for _ in 0..600 {
        walk(&mut s, 1, InputFrame::walk(Angle::SOUTH));
        let u = unit(&s, me);
        let (_, y) = u.pos.cell();
        if (18..=28).contains(&y) {
            assert_eq!(u.on_span, Some(0), "on the deck over row {y}");
            assert_eq!(span::unit_level(grid(&s), u), 1);
            on += 1;
        }
        if y >= 32 {
            break;
        }
    }
    assert!(on > 0, "she crossed on the deck");
    let u = unit(&s, me);
    assert!(u.pos.cell().1 >= 32 && u.on_span.is_none(), "off it at the far end: {:?}", u.pos.cell());
}

#[test]
fn the_towpath_runs_under_it_and_nothing_climbs_on_from_beneath() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 20, 24, false);
    walk(&mut s, 400, InputFrame::walk(Angle::EAST));
    let u = unit(&s, me);
    assert!(u.pos.cell().0 >= 40, "under the viaduct along the towpath: {:?}", u.pos.cell());
    assert!(u.on_span.is_none() && span::unit_level(grid(&s), u) == 0);
    // Under it, north and south: the face and the rim, never the deck.
    for (dir, x) in [(Angle::NORTH, 31), (Angle::SOUTH, 32)] {
        put(&mut s, me, x, 24, false);
        walk(&mut s, 300, InputFrame::walk(dir));
        let u = unit(&s, me);
        assert!((20..=27).contains(&u.pos.cell().1) && u.on_span.is_none(), "{:?}", u.pos.cell());
    }
}

#[test]
fn its_sides_are_parapets() {
    let mut s = world();
    let me = her(&s);
    for (dir, x) in [(Angle::EAST, 31), (Angle::WEST, 32)] {
        put(&mut s, me, x, 24, true);
        walk(&mut s, 200, InputFrame::walk(dir));
        let u = unit(&s, me);
        assert!((30..=33).contains(&u.pos.cell().0) && u.on_span == Some(0), "kept on the deck: {:?}", u.pos.cell());
    }
}

#[test]
fn the_deck_and_the_ground_under_it_are_held_apart() {
    let mut s = world();
    let a = foe_at(&mut s, 31, 24, true);
    let b = foe_at(&mut s, 31, 24, false);
    let g = grid(&s);
    assert_eq!((g.occupants(31, 24), g.deck_occupants(31, 24)), (1, 1));
    assert!(!g.free_on(31, 24, true, None) && !g.free_on(31, 24, false, None));
    assert!(g.free_on(31, 23, true, None), "the next cell of the deck is free");
    let _ = (a, b);
}

// --- sight, shots and blows ---------------------------------------------------------------------------

#[test]
fn the_deck_is_a_slab_between_its_layers() {
    let s = world();
    let g = grid(&s);
    let (c, d) = (Vec2::centre, Some(0u8));
    let see = |a: Vec2, da, b: Vec2, db| line_of_sight_on(g, a, da, b, db) && line_of_sight_on(g, b, db, a, da);
    assert!(!see(c(31, 24), d, c(31, 22), None), "nobody sees through the deck");
    assert!(see(c(25, 24), None, c(40, 24), None), "under it, along the towpath");
    assert!(see(c(31, 24), d, c(40, 24), None), "from the deck over the parapet and down");
    assert!(see(c(31, 22), d, c(31, 10), None), "along the deck to the terrace");
    assert!(!see(c(31, 24), None, c(31, 10), None), "from under it, not up to the terrace");
}

#[test]
fn a_free_bolt_keeps_to_its_layer() {
    for (deck, x0) in [(true, 31), (false, 32)] {
        let mut s = world();
        let me = her(&s);
        learn(&mut s, "spark");
        put(&mut s, me, x0, 26, deck);
        // On the other layer first along its way, then on its own.
        let other = foe_at(&mut s, x0, 24, !deck);
        let same = foe_at(&mut s, x0, 21, deck);
        rested(&mut s, me);
        s.drain_events();
        field::cast(&mut s, 0, "spark", field::aim(Angle::NORTH), None);
        steps(&mut s, 60);
        let ev = events(&mut s);
        assert!(!hurt(&ev, other), "deck {deck}: it flew past the other layer");
        assert!(hurt(&ev, same), "deck {deck}: and hit its own");
    }
}

#[test]
fn blows_never_cross_a_layer() {
    let mut s = world();
    let a = foe_at(&mut s, 31, 24, true);
    let b = foe_at(&mut s, 31, 24, false);
    let c = foe_at(&mut s, 32, 24, true);
    let g = grid(&s);
    let (ua, ub, uc) = (unit(&s, a), unit(&s, b), unit(&s, c));
    assert!(!span::blow_reaches(g, ua.pos, ua.on_span, ub), "deck to towpath");
    assert!(!span::blow_reaches(g, ub.pos, ub.on_span, ua), "towpath to deck");
    assert!(span::blow_reaches(g, ua.pos, ua.on_span, uc), "along the deck");
}

/// A chaser under her on the towpath, rooted, never lands a blow on her on the deck above it.
#[test]
fn a_foe_under_the_deck_never_reaches_her_on_it() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 31, 24, true);
    let foe = foe_at(&mut s, 31, 25, false);
    edit(&mut s, foe, |u| {
        u.target = Some(me);
        u.combat = CombatState::Combat;
    });
    s.drain_events();
    steps(&mut s, 300);
    let ev = events(&mut s);
    assert!(
        !ev.iter().any(|e| matches!(e.kind, EventKind::Damage { unit, from: Some(f), .. } if unit == me && f == foe))
    );
}

// --- paths and the AI --------------------------------------------------------------------------------

#[test]
fn paths_take_the_deck_along_its_axis_and_the_towpath_under_it() {
    let s = world();
    let g = grid(&s);
    let mut p = PathScratch::new();
    // Over: from the north terrace to the south, on the deck in one lane.
    assert_eq!(p.find(g, PathAsk::new((31, 12), (31, 36), cost_of_cells(200))), Some(PathEnd::Found));
    let decked: Vec<_> = p.out.iter().zip(p.out_on_deck()).filter(|(_, d)| **d).map(|(c, _)| *c).collect();
    assert!(decked.len() >= 11, "over the deck: {:?}", p.out);
    assert!(decked.iter().all(|&(x, y)| V::VIADUCT.contains(x, y)));
    assert!(decked.windows(2).all(|w| w[0].0 == w[1].0), "along its axis only: {decked:?}");
    // Under: the towpath, no deck.
    assert_eq!(p.find(g, PathAsk::new((20, 24), (44, 24), cost_of_cells(200))), Some(PathEnd::Found));
    assert!(p.out_on_deck().iter().all(|d| !d));
    assert!(p.out.iter().any(|&(x, _)| x == 31), "straight under it");
    // From the deck, and up onto it from the towpath (the long way, by the stair).
    let off = PathAsk { start_deck: Some(0), ..PathAsk::new((31, 24), (31, 10), cost_of_cells(200)) };
    assert_eq!(p.find(g, off), Some(PathEnd::Found));
    assert!(p.out_on_deck()[0], "it starts along the deck");
    let up = PathAsk { goal_deck: true, ..PathAsk::new((25, 24), (31, 24), cost_of_cells(400)) };
    assert_eq!(p.find(g, up), Some(PathEnd::Found));
    assert!(p.out.iter().any(|&c| V::STAIR.contains(c.0, c.1)), "by the stair: {:?}", p.out);
    assert_eq!(p.out_on_deck().last(), Some(&true), "and ends on the deck");
    // The broken footbridge carries nothing: over the valley at x 46 is the long way, by the viaduct.
    assert_eq!(p.find(g, PathAsk::new((46, 12), (46, 36), cost_of_cells(400))), Some(PathEnd::Found));
    assert!(p.out.iter().zip(p.out_on_deck()).all(|(&(x, _), &d)| !d || V::VIADUCT.contains(x, 24)));
}

#[test]
fn a_chaser_comes_along_the_deck_to_her() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 32, 25, true);
    let foe = spawn(&mut s, "quarryman", 31, 12);
    edit(&mut s, foe, |u| {
        u.target = Some(me);
        u.combat = CombatState::Combat;
        u.hp = Milli(u.hp.0 / 2);
    });
    let mut decked = false;
    for _ in 0..600 {
        steps(&mut s, 1);
        decked |= unit(&s, foe).on_span == Some(0);
    }
    let (f, h) = (unit(&s, foe).pos.cell(), cell_of(&s, me));
    assert!(decked, "it stepped onto the deck");
    assert!((f.0 - h.0).abs() <= 2 && (f.1 - h.1).abs() <= 2, "and came to her: {f:?} {h:?}");
    assert_eq!(unit(&s, foe).on_span, Some(0));
}

// --- lockstep and the save ------------------------------------------------------------------------------

fn crossing(f: u32) -> InputFrame {
    InputFrame::walk(if (f / 150) % 2 == 0 { Angle::SOUTH } else { Angle::NORTH })
}

fn under(f: u32) -> InputFrame {
    InputFrame::walk(if (f / 120) % 2 == 0 { Angle::EAST } else { Angle::WEST })
}

/// One seat over the deck and back, one under it along the towpath, a chaser after each: two sims
/// fed the same frames hash the same every tick, and a third whose runtimes are thrown away and
/// rebuilt every few ticks does too.
#[test]
fn co_op_over_and_under_stays_in_lockstep() {
    let party = || {
        let mut s = world();
        cmd(&mut s, Some(0), Command::Open(true));
        cmd(&mut s, None, Command::Join { who: ClientToken(1) });
        let (a, b) = (body_of(&s, 0), body_of(&s, 1));
        put(&mut s, a, 31, 15, false);
        put(&mut s, b, 22, 24, false);
        for (x, y, who) in [(36, 12, a), (18, 23, b)] {
            let f = spawn(&mut s, "quarryman", x, y);
            edit(&mut s, f, |u| {
                u.target = Some(who);
                u.combat = CombatState::Combat;
            });
        }
        s
    };
    let input =
        |f: u32| StepInput { frames: [crossing(f), under(f), InputFrame::IDLE, InputFrame::IDLE], commands: &[] };
    let (mut a, mut b, mut c) = (party(), party(), party());
    let mut decked = 0;
    for f in 0..900 {
        a.step(&input(f));
        b.step(&input(f));
        if f % 7 == 3 {
            c.rebuild_runtimes();
        }
        c.step(&input(f));
        assert_eq!(a.hash(), b.hash(), "frame {f}");
        assert_eq!(a.hash(), c.hash(), "rebuilt, frame {f}");
        decked += usize::from(a.state().zone(Z).is_some_and(|z| z.units.iter().any(|u| u.on_span.is_some())));
    }
    assert!(decked > 50, "on the deck a while ({decked} ticks)");
    assert_eq!(a.state(), c.state());
}

/// A save made on the deck loads on the deck and carries on to the state of the run that never
/// saved.
#[test]
fn a_save_on_the_deck_loads_on_it() {
    let solo = || {
        let mut s = world();
        let me = her(&s);
        put(&mut s, me, 31, 15, false);
        s
    };
    let mut straight = solo();
    for f in 0..400 {
        straight.step(&StepInput::solo(crossing(f)));
    }
    let mut first = solo();
    let mut at = None;
    for f in 0..400 {
        first.step(&StepInput::solo(crossing(f)));
        if unit(&first, her(&first)).on_span.is_some() && f > 40 {
            at = Some(f);
            break;
        }
    }
    let at = at.expect("she stepped onto the deck");
    let mut resumed = Sim::from_save_with(&first.save(), blueprints()).expect("the save loads");
    assert_eq!(unit(&resumed, her(&resumed)).on_span, Some(0), "loaded on the deck");
    assert_eq!(resumed.hash(), first.hash());
    for f in at + 1..400 {
        resumed.step(&StepInput::solo(crossing(f)));
    }
    assert_eq!(resumed.hash(), straight.hash());
    assert_eq!(resumed.state(), straight.state());
}

/// The footbridge mended (its bit in the zone's state): it carries her over, and a save keeps it
/// mended.
#[test]
fn a_broken_span_mended_carries_her_and_saves_so() {
    let mut s = world();
    let me = her(&s);
    s.state_mut().zone_mut(Z).unwrap().spans_changed = 1 << 1;
    s.rebuild_runtimes();
    assert!(grid(&s).span_whole(1));
    put(&mut s, me, 46, 14, false);
    walk(&mut s, 600, InputFrame::walk(Angle::SOUTH));
    assert!(cell_of(&s, me).1 >= 30, "over the mended footbridge: {:?}", cell_of(&s, me));
    let back = Sim::from_save_with(&s.save(), blueprints()).expect("the save loads");
    assert!(grid(&back).span_whole(1), "still mended");
    assert_eq!(back.hash(), s.hash());
}
