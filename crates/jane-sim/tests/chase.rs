//! How a chaser thinks (MAP.md §3.5, the lean chase; `ai::chase`): it plans on a real change and
//! on its own tick, keeps and lengthens a path she has not left, steps straight at her close in,
//! shares the zone's budget of search nodes with the rest and goes first the tick after it was
//! refused, and none of it is lost to a save.

mod field;

use std::sync::Arc;

use field::{Z, body_of, cmd, edit, spawn, steps};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Blueprint, Cell, CellIx, Key, Rect, Tile, Vec2, ZoneId};
use jane_sim::ai::live_path;
use jane_sim::height::terraces as T;
use jane_sim::path::{PATH_BUDGET, PATH_TICK_BUDGET};
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, Sim, UnitId};

/// A field of grass, 128 x 64, walled round, with whatever `shape` puts in it; she is a god and
/// stands at `her`.
fn ground(her: (i32, i32), shape: impl Fn(&mut Blueprint)) -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        let cell = Cell::new(her.0 as u16, her.1 as u16);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell, facing: Some(Facing::East) });
        if z == Z {
            for r in
                [Rect::new(0, 0, 128, 1), Rect::new(0, 63, 128, 1), Rect::new(0, 0, 1, 64), Rect::new(127, 0, 1, 64)]
            {
                bp.tiles.fill_rect(r, Tile::Wall);
            }
            shape(&mut bp);
        }
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let me = body_of(&s, 0);
    edit(&mut s, me, |u| u.pos = Vec2::centre(her.0, her.1));
    s
}

fn open() -> Sim {
    ground((10, 10), |_| {})
}

/// A quarryman at `(x, y)` set on her.
fn chaser(s: &mut Sim, x: i32, y: i32) -> UnitId {
    let her = body_of(s, 0);
    let id = spawn(s, "quarryman", x, y);
    edit(s, id, |u| {
        // Strong enough to outlive a long run; set on her at once.
        u.strength = 2000;
        u.hp = jane_sim::units::max_hp(u);
        u.target = Some(her);
        u.combat = CombatState::Combat;
    });
    id
}

fn searches(s: &Sim) -> u64 {
    s.path_stats().searches
}

/// The tick the next step runs at.
fn next_tick(s: &Sim) -> u32 {
    s.state().tick.0 + 1
}

/// Does `id` plan on this tick of the stagger?
fn on_slot(tick: u32, id: UnitId) -> bool {
    (tick + id.get()) % 4 == 0
}

/// The cell she stands on.
fn her_cell(s: &Sim) -> (i32, i32) {
    let me = body_of(s, 0);
    s.state().zone(Z).and_then(|z| z.unit(me)).expect("her").pos.cell()
}

fn cells_of(s: &Sim, id: UnitId) -> Vec<(i32, i32)> {
    let w = s.runtime(Z).expect("the zone").grid.w();
    let u = s.state().zone(Z).and_then(|z| z.unit(id)).expect("it");
    live_path(u).map(|p| p.cells.iter().map(|c| ((c.0 % w) as i32, (c.0 / w) as i32)).collect()).unwrap_or_default()
}

fn at_of(s: &Sim, id: UnitId) -> usize {
    let u = s.state().zone(Z).and_then(|z| z.unit(id)).expect("it");
    live_path(u).map_or(0, |p| usize::from(p.at))
}

/// A chaser's ordinary plan comes on its own tick of four, staggered by the id, however often her
/// cell changes: she jumps 20 cells every tick, out of any path's keeping, and the searches of two
/// chasers fall on the ticks `(tick + id) % 4 == 0` of their ids and no others.
#[test]
fn a_chaser_plans_only_on_its_own_tick() {
    let mut s = open();
    let her = body_of(&s, 0);
    let a = chaser(&mut s, 30, 20);
    let b = chaser(&mut s, 30, 21);
    assert_ne!(a.get() % 4, b.get() % 4, "neighbours stagger apart");
    // Five places, none within two cells of another, in an order that does not fall in step with
    // the four of the stagger: on each of a chaser's ticks she has moved on.
    let places = [(14, 12), (14, 28), (22, 36), (12, 20), (24, 8)];
    let (mut ticks, mut every) = (Vec::new(), 0);
    for t in 0..120 {
        let (x, y) = places[t % places.len()];
        edit(&mut s, her, |u| u.pos = Vec2::centre(x, y));
        let before = searches(&s);
        steps(&mut s, 1);
        let n = searches(&s) - before;
        every += 1;
        if n > 0 {
            ticks.push((s.state().tick.0, n));
        }
    }
    assert!(ticks.len() >= 20, "they planned all the while: {ticks:?}");
    for &(tick, n) in &ticks {
        assert!(on_slot(tick, a) || on_slot(tick, b), "tick {tick} is neither's: {ticks:?}");
        assert!(n <= 2, "at most one plan each on a tick: {ticks:?}");
        if n == 2 {
            assert!(on_slot(tick, a) && on_slot(tick, b));
        }
    }
    let (sa, sb) = (ticks.iter().any(|&(t, _)| on_slot(t, a)), ticks.iter().any(|&(t, _)| on_slot(t, b)));
    assert!(sa && sb, "both planned: {ticks:?}");
    assert!(ticks.len() <= every / 4 * 2 + 2, "no more than a plan in four ticks each: {} in {every}", ticks.len());
}

/// A path she has not left is lengthened to her and not planned again: walking away from it along
/// a line, with a chaser behind, costs the plan it began with and the refresh of a trail, not a
/// search each cell she steps.
#[test]
fn a_chaser_keeps_its_path_while_she_walks_away() {
    let mut s = open();
    let a = chaser(&mut s, 30, 10);
    // It plans (on its own tick) before she moves; she then walks away from it, slower than it runs.
    steps(&mut s, 8);
    let planned = searches(&s);
    assert!(planned >= 1);
    let from = her_cell(&s);
    let frame = jane_sim::InputFrame { mv_mag: 40, ..jane_sim::InputFrame::walk(jane_core::Angle::WEST) };
    for _ in 0..100 {
        s.step(&jane_sim::StepInput::solo(frame));
    }
    let to = her_cell(&s);
    let stepped = (from.0 - to.0).abs();
    assert!(stepped >= 6, "she changed cell {stepped} times");
    let kept = s.path_stats().kept;
    assert!(kept >= 5, "its path was lengthened to her cell by cell: {kept}");
    // A search a cell would be six or more; it plans afresh now and then, as a trail is not the
    // shortest way, and that is all.
    let more = searches(&s) - planned;
    assert!(more <= 3, "{more} searches in a hundred ticks");
    // And its path ends where she stands.
    let end = *cells_of(&s, a).last().expect("a path");
    assert!((end.0 - to.0).abs().max((end.1 - to.1).abs()) <= 2, "ends at {end:?}, she is at {to:?}");
}

/// A path as short as a path can be is not planned again for its timer: walking a straight way to her
/// longer than the net for a stuck walker (two seconds), a chaser searches once.
#[test]
fn a_straight_path_is_not_looked_at_again_when_its_timer_is_up() {
    let mut s = ground((10, 20), |_| {});
    let a = chaser(&mut s, 34, 20);
    steps(&mut s, 130);
    // It is still on its way: the net is 120 ticks, and the way to her is 22 cells of 0.15 a tick.
    let u = s.state().zone(Z).unwrap().unit(a).unwrap();
    assert!(u.pos.cell().0 > 14, "still walking: {:?}", u.pos.cell());
    assert_eq!(searches(&s), 1, "the one plan it began with, and no look at its straight way: {:?}", s.path_stats());
}

/// Planning is at once, with no wait for its tick, when the next step goes solid under it.
#[test]
fn a_chaser_plans_at_once_when_its_next_step_goes_solid() {
    let build = || {
        let mut s = open();
        let a = chaser(&mut s, 34, 10);
        // Let it plan and walk a while.
        steps(&mut s, 12);
        // Then an off tick: the next step is not its own.
        while on_slot(next_tick(&s), a) {
            steps(&mut s, 1);
        }
        assert!(cells_of(&s, a).len() - at_of(&s, a) >= 4, "it has some way to go");
        (s, a)
    };
    let (mut blocked, a) = build();
    let (mut control, _) = build();
    // The cell it is making for goes solid (a door shut, a crate pushed in).
    let w = blocked.runtime(Z).unwrap().grid.w();
    let next = cells_of(&blocked, a)[at_of(&blocked, a)];
    blocked.state_mut().zone_mut(Z).unwrap().tile_deltas.insert(CellIx(next.1 as u32 * w + next.0 as u32), Tile::Wall);
    blocked.rebuild_runtimes();
    let (b0, c0) = (searches(&blocked), searches(&control));
    steps(&mut blocked, 1);
    steps(&mut control, 1);
    assert_eq!(searches(&control) - c0, 0, "off its tick it plans nothing");
    assert_eq!(searches(&blocked) - b0, 1, "with its next step solid it plans at once");
    let new_next = cells_of(&blocked, a);
    assert!(!new_next.contains(&next), "and goes round: {new_next:?}");
}

/// Planning is at once when she changes level: she drops off the plateau and it plans that tick,
/// where an unmoved target costs it nothing.
#[test]
fn a_chaser_plans_at_once_when_she_changes_level() {
    let build = || {
        let cat = jane_data::catalog();
        let zones = std::array::from_fn(|i| {
            let z = ZoneId::ALL[i];
            if z == Z {
                return Arc::new(T::blueprint(z));
            }
            let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
            bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: None });
            Arc::new(bp)
        });
        let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
        cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
        let me = body_of(&s, 0);
        edit(&mut s, me, |u| u.pos = Vec2::centre(30, 10));
        let a = chaser(&mut s, 20, 8);
        steps(&mut s, 12);
        while on_slot(next_tick(&s), a) {
            steps(&mut s, 1);
        }
        (s, a)
    };
    let (mut dropped, a) = build();
    let (mut control, _) = build();
    let her = body_of(&dropped, 0);
    // The plateau is level 2, the field below it level 1.
    {
        let g = &dropped.runtime(Z).unwrap().grid;
        assert_eq!((g.level_at(30, 10), g.level_at(30, 20)), (2, 1));
    }
    edit(&mut dropped, her, |u| u.pos = Vec2::centre(30, 20));
    let (d0, c0) = (searches(&dropped), searches(&control));
    steps(&mut dropped, 1);
    steps(&mut control, 1);
    assert_eq!(searches(&control) - c0, 0, "an unmoved target costs it nothing");
    assert!(searches(&dropped) - d0 >= 1, "she is on other ground: it plans that tick, off its own");
    let _ = a;
}

/// Close in, on her ground and in sight, a chaser steps straight at her and searches nothing; with
/// a wall between it does not.
#[test]
fn close_in_a_chaser_steps_straight_at_her() {
    let mut s = ground((10, 20), |bp| bp.tiles.fill_rect(Rect::new(40, 10, 1, 20), Tile::Wall));
    let her = body_of(&s, 0);
    let a = chaser(&mut s, 10, 20);
    // A little under three cells from her: out of its reach, within its sight.
    edit(&mut s, a, |u| u.pos = Vec2::centre(10, 20) + Vec2::new(jane_core::Fx(2 * 2048 + 1500), jane_core::Fx(0)));
    let before = searches(&s);
    let start = s.state().zone(Z).unwrap().unit(a).unwrap().pos;
    steps(&mut s, 3);
    let u = s.state().zone(Z).unwrap().unit(a).unwrap();
    let hers = s.state().zone(Z).unwrap().unit(her).unwrap().pos;
    assert_eq!(searches(&s), before, "no search");
    assert!(s.path_stats().straight >= 1, "a straight step");
    assert!(jane_sim::combat::distance(u.pos, hers) < jane_sim::combat::distance(start, hers), "and nearer");
    assert!(live_path(u).is_none_or(|p| p.cells.is_empty()), "with no path");

    // A wall between them, within three cells: she is in no sight, and it plans.
    let mut s = ground((38, 20), |bp| bp.tiles.fill_rect(Rect::new(40, 10, 1, 20), Tile::Wall));
    let a = chaser(&mut s, 41, 20);
    steps(&mut s, 8);
    assert_eq!(s.path_stats().straight, 0, "not through a wall");
    assert!(searches(&s) >= 1);
    let _ = a;
}

/// A wall across the field with the gaps at the cells `gaps` of its column, she on the far side of it
/// and a chaser coming through the gap at y = 20 on the near: a body stands in the gap.
fn gap_with_a_body(gaps: &[i32]) -> (Sim, UnitId, UnitId) {
    let gaps = gaps.to_vec();
    let mut s = ground((10, 20), move |bp| {
        for y in 1..63 {
            if !gaps.contains(&y) {
                bp.tiles.fill_rect(Rect::new(20, y, 1, 1), Tile::Wall);
            }
        }
    });
    let a = chaser(&mut s, 28, 20);
    // It plans and sets off for the gap; then a body, standing where it is going, stays there.
    steps(&mut s, 8);
    assert!(cells_of(&s, a).len() > 3, "it has a path through the gap");
    let blocker = spawn(&mut s, "quarryman", 20, 20);
    field::rooted(&mut s, blocker);
    (s, a, blocker)
}

/// A step held by a body for some time sends a chaser looking for a way round it, and a look that
/// finds none leaves it its path: no failure is made of it and it is not led away from the gap. When
/// the body goes it walks on through.
#[test]
fn a_look_for_a_way_round_a_body_that_finds_none_leaves_the_chaser_its_path() {
    let (mut s, a, blocker) = gap_with_a_body(&[20]);
    steps(&mut s, 110);
    let ps = s.path_stats();
    assert!(ps.held >= 1, "it looked for a way round, in the time and found none: {ps:?}");
    assert_eq!(ps.failed, 0, "and it was not made a failure of: {ps:?}");
    let u = s.state().zone(Z).unwrap().unit(a).unwrap();
    assert!(live_path(u).is_some_and(|p| !p.cells.is_empty()), "its path is its own still");
    assert!(u.pos.cell().0 >= 21, "and it waits at the gap: {:?}", u.pos.cell());
    // The body goes: it is through the gap within a few seconds.
    s.state_mut().zone_mut(Z).unwrap().remove_unit(blocker);
    s.rebuild_runtimes();
    steps(&mut s, 60);
    assert!(s.state().zone(Z).unwrap().unit(a).unwrap().pos.cell().0 < 20, "it went through");
}

/// A body that stays does not stop it for ever: with another gap near enough (the way round costs
/// no more than three times the path it walked and a dozen cells) it goes by that.
#[test]
fn a_chaser_goes_round_a_body_that_stays_by_another_gap() {
    let (mut s, a, _) = gap_with_a_body(&[20, 26]);
    steps(&mut s, 220);
    let u = s.state().zone(Z).unwrap().unit(a).unwrap();
    assert!(u.pos.cell().0 < 20, "it went through the other gap: {:?}", u.pos.cell());
}

/// A chaser that searched and found nothing stands till its wait is out, wherever she goes: she
/// moves to another sealed box, twenty cells off, and it does not search again for forty ticks,
/// then it does.
#[test]
fn a_chaser_that_found_no_way_waits_wherever_she_goes() {
    let boxed = |bp: &mut Blueprint, (cx, cy): (i32, i32)| {
        for r in [
            Rect::new(cx - 2, cy - 2, 5, 1),
            Rect::new(cx - 2, cy + 2, 5, 1),
            Rect::new(cx - 2, cy - 2, 1, 5),
            Rect::new(cx + 2, cy - 2, 1, 5),
        ] {
            bp.tiles.fill_rect(r, Tile::Wall);
        }
    };
    let mut s = ground((100, 30), |bp| {
        boxed(bp, (100, 30));
        boxed(bp, (100, 50));
    });
    let her = body_of(&s, 0);
    let a = chaser(&mut s, 60, 40);
    let mut found_nothing = false;
    for _ in 0..40 {
        steps(&mut s, 1);
        if s.path_stats().failed > 0 {
            found_nothing = true;
            break;
        }
    }
    assert!(found_nothing, "its first search found nothing");
    edit(&mut s, her, |u| u.pos = Vec2::centre(100, 50));
    let before = searches(&s);
    steps(&mut s, 30);
    assert_eq!(searches(&s), before, "she is twenty cells off and it waits all the same");
    steps(&mut s, 40);
    assert!(searches(&s) > before, "and when its wait is out it asks again");
    let _ = a;
}

/// A field with a crowd in it after one who is walled into a box: every search is for something it
/// cannot reach, and opens all the budget a search has, so the tick's nodes run out.
fn crowd() -> (Sim, Vec<UnitId>) {
    let mut s = ground((100, 30), |bp| {
        for r in [Rect::new(98, 28, 5, 1), Rect::new(98, 32, 5, 1), Rect::new(98, 28, 1, 5), Rect::new(102, 28, 1, 5)] {
            bp.tiles.fill_rect(r, Tile::Wall);
        }
    });
    let mut ids = Vec::new();
    'rows: for x in [46, 49, 52, 55, 58] {
        for y in [14, 17, 20, 23, 26, 29, 32, 35] {
            if ids.len() == 30 {
                break 'rows;
            }
            ids.push(chaser(&mut s, x, y));
        }
    }
    (s, ids)
}

/// The tick's nodes are shared: no tick opens more than the budget and one search more; the ones
/// refused keep their place in a queue the next tick answers first, in order; and none is left
/// without a plan for ever.
#[test]
fn the_ticks_nodes_are_shared_and_the_refused_go_first() {
    let (mut s, ids) = crowd();
    let mut refused_ticks = 0;
    let mut first_in_line: Option<UnitId> = None;
    for t in 0..80 {
        steps(&mut s, 1);
        let m = s.metrics();
        assert!(
            m.path_expanded <= PATH_TICK_BUDGET + PATH_BUDGET,
            "tick {t} opened {} nodes, the budget is {PATH_TICK_BUDGET} and a search's {PATH_BUDGET}",
            m.path_expanded
        );
        // The one first in line last tick has a plan now.
        if let Some(id) = first_in_line.take() {
            let u = s.state().zone(Z).unwrap().unit(id).unwrap();
            assert!(live_path(u).is_some(), "tick {t}: {id:?} was first in line and is served first");
        }
        let queue = s.state().zone(Z).unwrap().path_queue.clone();
        if !queue.is_empty() {
            refused_ticks += 1;
            first_in_line = Some(queue[0]);
        }
    }
    assert!(refused_ticks >= 2, "the crowd strained the budget on {refused_ticks} ticks");
    assert!(s.path_stats().refused > 0);
    println!(
        "{} searches, {} refused on {refused_ticks} ticks, {:?}",
        searches(&s),
        s.path_stats().refused,
        s.path_stats()
    );
    let zone = s.state().zone(Z).unwrap();
    for id in &ids {
        assert!(live_path(zone.unit(*id).unwrap()).is_some(), "{id:?} was never served");
    }
}

/// A save in the middle of it (with walkers in the queue) is the game unbroken: the queue is
/// saved, and the loaded game steps to the same hash tick after tick.
#[test]
fn a_save_with_walkers_in_the_queue_is_the_game_unbroken() {
    let (mut s, _) = crowd();
    let mut queued = false;
    for _ in 0..80 {
        steps(&mut s, 1);
        if !s.state().zone(Z).unwrap().path_queue.is_empty() {
            queued = true;
            break;
        }
    }
    assert!(queued, "the crowd queued some");
    let queue = s.state().zone(Z).unwrap().path_queue.clone();
    let bytes = s.save();
    let mut loaded = Sim::from_save_with(&bytes, s.blueprints().clone()).expect("it loads");
    assert_eq!(loaded.state().zone(Z).unwrap().path_queue, queue, "the queue is saved");
    assert_eq!(loaded.hash(), s.hash());
    for t in 0..120 {
        steps(&mut s, 1);
        steps(&mut loaded, 1);
        assert_eq!(loaded.hash(), s.hash(), "tick {t} after the save");
    }
    assert_eq!(loaded.state(), s.state());
}
