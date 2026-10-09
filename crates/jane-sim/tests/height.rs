//! MAP.md R1, height in the sim, on the hand-built `terraces` (`jane_sim::height::terraces`: a
//! plateau at level 2 over the north, its face at rows 14 and 15 with a ledge, a stair, a ladder
//! and a ramp in it; a hedge and a rise in the field at level 1). Walking stops at faces, joins
//! change level, a ledge is hopped one way; paths take a ledge down and never up; sight follows
//! the eye-to-eye rule, and on flat ground is the flags test exactly; shots fall off cliffs and
//! never climb them; melee never crosses a face; a chaser hops after her, a boss does not, and
//! what cannot reach her holds at the foot and evades home whole; co-op stays in lockstep through
//! hops, and a save mid-hop loads mid-hop.

mod field;

use std::sync::Arc;

use field::{Z, body_of, cmd, edit, events, learn, rested, spawn, spell, steps, unit, walk};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::grid::Grid;
use jane_core::num::{CELL_FX, dist_sq, isqrt};
use jane_core::search::PathEnd;
use jane_core::tile::{BLOCK_SHOT, BLOCK_SIGHT, F_BLOCK_LOS};
use jane_core::{Angle, Blueprint, Cell, Fx, Key, Milli, Plane, Rect, Sfc32, Tile, Vec2, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::grid::ZoneGrid;
use jane_sim::height::{self, terraces as T};
use jane_sim::los::{
    first_blocked_cell, first_blocked_cell_levels, first_blocked_free_shot, line_of_sight, walk_cells,
};
use jane_sim::path::{PathAsk, PathScratch, cost_of_cells};
use jane_sim::state::CombatState;
use jane_sim::units::max_hp;
use jane_sim::{Blueprints, ClientToken, Command, DevOp, InputFrame, Sim, StepInput, UnitId};

/// Every zone a field of grass but the county, which is the terraces.
fn blueprints() -> Blueprints {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            return Arc::new(T::blueprint(z));
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

fn put(s: &mut Sim, id: UnitId, x: i32, y: i32) {
    edit(s, id, |u| u.pos = Vec2::centre(x, y));
}

fn grid(s: &Sim) -> &ZoneGrid {
    &s.runtime(Z).expect("the terraces").grid
}

fn level(s: &Sim, id: UnitId) -> u8 {
    height::level_of(grid(s), unit(s, id).pos)
}

fn cell_of(s: &Sim, id: UnitId) -> (i32, i32) {
    unit(s, id).pos.cell()
}

fn cells(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64)) / i64::from(CELL_FX)
}

/// On her, its post at (hx, hy), wounded to half.
fn on_her(s: &mut Sim, id: UnitId, hx: i32, hy: i32) {
    let target = her(s);
    edit(s, id, |u| {
        u.home = Vec2::centre(hx, hy);
        u.hp = Milli(u.hp.0 / 2);
        u.target = Some(target);
        u.combat = CombatState::Combat;
    });
}

// --- walking ---------------------------------------------------------------------------------------

#[test]
fn walking_stops_at_a_face_and_at_a_ledge_from_below() {
    let mut s = world();
    let me = her(&s);
    for x in [30, 12] {
        put(&mut s, me, x, 20);
        walk(&mut s, 300, InputFrame::walk(Angle::NORTH));
        assert!(cell_of(&s, me).1 >= 16, "she stands below the face at x {x}: {:?}", cell_of(&s, me));
        assert_eq!(level(&s, me), 1);
        assert!(!height::hopping(unit(&s, me)));
    }
}

#[test]
fn stairs_ramps_and_ladders_change_level_and_a_ladder_is_slow_and_quiet() {
    let mut s = world();
    let me = her(&s);
    for (x, what) in [(81, "stair"), (90, "ramp"), (60, "ladder")] {
        put(&mut s, me, x, 20);
        walk(&mut s, 400, InputFrame::walk(Angle::NORTH));
        assert_eq!(level(&s, me), 2, "up the {what}: {:?}", cell_of(&s, me));
        assert!(cell_of(&s, me).1 <= 13);
        walk(&mut s, 400, InputFrame::walk(Angle::SOUTH));
        assert_eq!(level(&s, me), 1, "down the {what}: {:?}", cell_of(&s, me));
        assert!(cell_of(&s, me).1 >= 16);
    }
    // On the ladder her pace is half her walk's.
    put(&mut s, me, 60, 18);
    let mut paces = Vec::new();
    for _ in 0..200 {
        let y0 = unit(&s, me).pos.y.0;
        let on = grid(&s).tile_at(60, unit(&s, me).pos.cell().1);
        walk(&mut s, 1, InputFrame::walk(Angle::NORTH));
        paces.push((on, y0 - unit(&s, me).pos.y.0));
    }
    let grass = paces.iter().find(|(t, d)| *t == Tile::Grass && *d > 0).expect("walked on grass").1;
    let ladder = paces.iter().find(|(t, d)| *t == Tile::Ladder && *d > 0).expect("climbed the ladder").1;
    assert_eq!(ladder, grass / 2, "half pace on the ladder");
    // Nothing is cast on a ladder.
    learn(&mut s, "spark");
    put(&mut s, me, 60, 15);
    rested(&mut s, me);
    s.drain_events();
    cmd(&mut s, Some(0), Command::Cast { spell: spell("spark"), on: None });
    assert!(s.state().zone(Z).unwrap().projectiles.is_empty(), "no bolt from the ladder");
}

#[test]
fn a_ledge_is_hopped_one_way_with_no_control_mid_air() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 12, 10);
    let mut hop_ticks = 0;
    for t in 0..120 {
        // West once it has begun: the hop goes on south regardless.
        let frame = if hop_ticks > 0 { InputFrame::walk(Angle::WEST) } else { InputFrame::walk(Angle::SOUTH) };
        let x0 = unit(&s, me).pos.x;
        walk(&mut s, 1, frame);
        let u = unit(&s, me);
        if height::hopping(u) {
            hop_ticks += 1;
            assert_eq!(u.pos.x, x0, "no control mid-hop (tick {t})");
        } else {
            assert!(!(14..=15).contains(&u.pos.cell().1), "never stood in the face: {:?}", u.pos.cell());
        }
        if hop_ticks > 0 && !height::hopping(u) {
            break;
        }
    }
    assert!((20..=26).contains(&hop_ticks), "a hop of about 0.4 s ({hop_ticks} ticks)");
    assert_eq!(level(&s, me), 1, "down: {:?}", cell_of(&s, me));
    assert_eq!(cell_of(&s, me).1, 16, "on the landing");
    // And back up it is a wall.
    walk(&mut s, 200, InputFrame::walk(Angle::NORTH));
    assert_eq!(level(&s, me), 1);
    assert!(cell_of(&s, me).1 >= 16);
}

// --- paths -----------------------------------------------------------------------------------------

/// Two cells of a path that are not side by side: a hop.
fn hops(start: (i32, i32), path: &[(i32, i32)]) -> usize {
    let mut prev = start;
    let mut n = 0;
    for &c in path {
        n += usize::from((c.0 - prev.0).abs() > 1 || (c.1 - prev.1).abs() > 1);
        prev = c;
    }
    n
}

#[test]
fn paths_take_a_ledge_down_and_never_up() {
    let s = world();
    let g = grid(&s);
    let mut p = PathScratch::new();
    let down = |ledges| PathAsk { ledges, ..PathAsk::new((12, 8), (12, 20), cost_of_cells(400)) };
    assert_eq!(p.find(g, down(Some(((12, 8), 30)))), Some(PathEnd::Found));
    assert_eq!(hops((12, 8), &p.out), 1, "down by the ledge: {:?}", p.out);
    assert!(p.out.len() < 12, "the short way: {}", p.out.len());
    // A walker that may not hop goes round by the ladder.
    assert_eq!(p.find(g, down(None)), Some(PathEnd::Found));
    assert_eq!(hops((12, 8), &p.out), 0);
    assert!(p.out.iter().any(|c| c.0 >= 59), "round by the ladder");
    // Nor one whose leash ends short of the landing.
    assert_eq!(p.find(g, down(Some(((12, 8), 3)))), Some(PathEnd::Found));
    assert_eq!(hops((12, 8), &p.out), 0);
    // Up, the ledge is never taken.
    let up = PathAsk { ledges: Some(((12, 20), 99)), ..PathAsk::new((12, 20), (12, 8), cost_of_cells(400)) };
    assert_eq!(p.find(g, up), Some(PathEnd::Found));
    assert_eq!(hops((12, 20), &p.out), 0);
    assert!(p.out.iter().any(|c| c.0 >= 59), "up by the ladder");
    // A perch row's search never leaves its level.
    let perch = PathAsk { level: Some(2), ..down(Some(((12, 8), 30))) };
    assert_eq!(p.find(g, perch), None);
    let along = PathAsk { level: Some(2), ..PathAsk::new((12, 8), (70, 6), cost_of_cells(400)) };
    assert_eq!(p.find(g, along), Some(PathEnd::Found));
    assert!(p.out.iter().all(|&(x, y)| g.level_at(x, y) == 2));
}

// --- sight -----------------------------------------------------------------------------------------

/// MAP.md §3.2: from `a` cells below the face's foot she sees a body `b` cells back from its top
/// exactly when `b < a`, both ways.
#[test]
fn from_below_she_sees_up_to_the_edge() {
    let s = world();
    let g = grid(&s);
    for a in 1..=8 {
        for b in 1..=8 {
            let (low, high) = (Vec2::centre(50, 15 + a), Vec2::centre(50, 14 - b));
            assert_eq!(line_of_sight(g, low, high), b < a, "a {a}, b {b}");
            assert_eq!(line_of_sight(g, high, low), b < a, "a {a}, b {b}, from above");
        }
    }
}

#[test]
fn high_ground_sees_over_a_hedge_and_a_rise_blocks_the_ground_below() {
    let s = world();
    let g = grid(&s);
    let far = Vec2::centre(25, 40);
    assert!(line_of_sight(g, Vec2::centre(25, 13), far), "from the lip, over the hedge");
    assert!(!line_of_sight(g, Vec2::centre(25, 16), far), "from beside it, the hedge is a hedge");
    assert!(!line_of_sight(g, Vec2::centre(28, 31), Vec2::centre(42, 31)), "the rise between");
    assert!(line_of_sight(g, Vec2::centre(28, 38), Vec2::centre(42, 38)), "south of the rise");
}

/// The flat equivalence (MAP.md §9.1): on ground of one level the eye-to-eye rule is the flags
/// test, cell for cell, on random grids of every tile but the cliff (whose height is its level)
/// with random props, for every mask the sim asks.
#[test]
fn on_flat_ground_the_new_sight_is_the_old() {
    let tiles: Vec<Tile> = Tile::ALL.iter().copied().filter(|&t| t != Tile::Cliff).collect();
    let mut rng = Sfc32::seeded(0x5ee, 1);
    let mut n = 0;
    for round in 0..20 {
        let (w, h) = (40 + rng.below(40), 30 + rng.below(30));
        let cells: Vec<Tile> = (0..w * h)
            .map(|_| if rng.below(3) == 0 { tiles[rng.below(tiles.len() as u32) as usize] } else { Tile::Grass })
            .collect();
        let lv = (round % 4) as u8;
        let mut flat = ZoneGrid::new(Grid::from_vec(w, h, cells.clone()));
        let mut high = ZoneGrid::with_levels(Grid::from_vec(w, h, cells), Plane::pack_by(w, h, |_| lv));
        for _ in 0..12 {
            let r =
                Rect::new(rng.below(w) as i32, rng.below(h) as i32, 1 + rng.below(3) as i32, 1 + rng.below(3) as i32);
            let los = rng.below(2) == 0;
            flat.stamp_prop(r, los);
            high.stamp_prop(r, los);
        }
        for _ in 0..5_000 {
            let mut p =
                || Vec2::new(Fx(rng.below(w * CELL_FX as u32) as i32), Fx(rng.below(h * CELL_FX as u32) as i32));
            let (a, b) = (p(), p());
            for mask in [BLOCK_SIGHT, BLOCK_SHOT, F_BLOCK_LOS] {
                let old = first_blocked_cell(&flat, a, b, mask);
                assert_eq!(first_blocked_cell_levels(&high, a, b, mask), old, "{a:?} -> {b:?}, mask {mask}");
                assert_eq!(first_blocked_free_shot(&high, a, b, mask), old);
                assert_eq!(first_blocked_cell(&high, a, b, mask), old, "the fast path");
                n += 1;
            }
        }
    }
    assert_eq!(n, 300_000);
}

/// The same over every zone of a seed as built, which have no levels: given one level
/// everywhere, sight is unchanged on 100 000 segments (those that cross a cliff aside, the one
/// tile whose height is its level).
#[test]
fn over_every_zone_the_new_sight_is_the_old() {
    let bps = Blueprints::build(7).expect("seed 7 builds");
    let mut rng = Sfc32::seeded(0x10f, 2);
    let (mut n, mut cliffs) = (0u32, 0u32);
    for z in ZoneId::ALL {
        let bp = bps.get(z);
        assert!(bp.level.is_none(), "{z:?} has no levels yet");
        let flat = ZoneGrid::over(bp, []);
        let mut one = (**bp).clone();
        one.level = Some(Plane::pack_by(bp.w(), bp.h(), |_| 1));
        let high = ZoneGrid::over(&Arc::new(one), []);
        assert!(!flat.has_levels() && high.has_levels());
        let (w, h) = (bp.w(), bp.h());
        for _ in 0..100_000 / ZoneId::ALL.len() as u32 {
            let (ax, ay) = (rng.below(w) as i32, rng.below(h) as i32);
            let reach = 48;
            let bx = (ax + rng.below(2 * reach) as i32 - reach as i32).clamp(0, w as i32 - 1);
            let by = (ay + rng.below(2 * reach) as i32 - reach as i32).clamp(0, h as i32 - 1);
            let (a, b) = (Vec2::centre(ax, ay), Vec2::centre(bx, by));
            if walk_cells(a, b, |x, y| flat.tile_at(x, y) == Tile::Cliff).is_some() {
                cliffs += 1;
                continue;
            }
            for mask in [BLOCK_SIGHT, BLOCK_SHOT] {
                assert_eq!(
                    first_blocked_cell_levels(&high, a, b, mask),
                    first_blocked_cell(&flat, a, b, mask),
                    "{z:?}"
                );
            }
            n += 1;
        }
    }
    assert!(n > 80_000, "{n} segments ({cliffs} across a cliff)");
}

// --- shots and blows --------------------------------------------------------------------------------

/// A foe rooted on cell (x, y), awake, whole.
fn foe_at(s: &mut Sim, x: i32, y: i32) -> UnitId {
    let id = spawn(s, "quarryman", x, y);
    field::rooted(s, id);
    id
}

fn hurt(ev: &[jane_sim::Event], who: UnitId) -> bool {
    ev.iter().any(|e| matches!(e.kind, EventKind::Damage { unit, .. } if unit == who))
}

#[test]
fn a_free_bolt_falls_off_a_cliff_and_stops_at_a_face() {
    let mut s = world();
    let me = her(&s);
    learn(&mut s, "spark");
    // Off the plateau's lip, south: over the face and down onto the field, where it hits.
    put(&mut s, me, 50, 11);
    let below = foe_at(&mut s, 50, 21);
    rested(&mut s, me);
    s.drain_events();
    field::cast(&mut s, 0, "spark", field::aim(Angle::SOUTH), None);
    steps(&mut s, 60);
    assert!(hurt(&events(&mut s), below), "it fell off the cliff onto the field");
    // From the field north into the face: it stops there, and nothing on top is touched.
    let mut s = world();
    learn(&mut s, "spark");
    put(&mut s, me, 50, 22);
    let above = foe_at(&mut s, 50, 12);
    rested(&mut s, me);
    s.drain_events();
    field::cast(&mut s, 0, "spark", field::aim(Angle::NORTH), None);
    steps(&mut s, 60);
    let ev = events(&mut s);
    assert!(!hurt(&ev, above), "a free bolt never climbs");
    let at = ev.iter().find_map(|e| match e.kind {
        EventKind::Impact { at, .. } => Some(at.cell()),
        _ => None,
    });
    assert!(at.is_some_and(|(_, y)| (15..=16).contains(&y)), "stopped at the face: {at:?}");
}

#[test]
fn a_targeted_bolt_goes_up_to_an_edge_stander_and_not_to_one_standing_back() {
    for (b, hits) in [(1, true), (4, false)] {
        let mut s = world();
        let me = her(&s);
        learn(&mut s, "spark");
        put(&mut s, me, 50, 18);
        let foe = foe_at(&mut s, 50, 14 - b);
        rested(&mut s, me);
        s.drain_events();
        field::cast(&mut s, 0, "spark", InputFrame::IDLE, Some(foe));
        steps(&mut s, 60);
        let ev = events(&mut s);
        assert_eq!(hurt(&ev, foe), hits, "a 3 below the foot, b {b} back from the top");
        if !hits {
            assert!(ev.iter().any(|e| matches!(
                e.kind,
                EventKind::Toast(jane_sim::event::ToastKind::SpellError(jane_sim::event::SpellError::NotInLos))
                    | EventKind::CastFailed { why: jane_sim::event::SpellError::NotInLos, .. }
            )));
        }
    }
}

#[test]
fn melee_never_reaches_across_a_face_but_does_across_a_join() {
    let s = world();
    let g = grid(&s);
    assert!(!height::melee_reaches(g, Vec2::centre(50, 16), Vec2::centre(50, 13)), "across the face");
    assert!(height::melee_reaches(g, Vec2::centre(81, 15), Vec2::centre(81, 14)), "a step of the stair");
    assert!(height::melee_reaches(g, Vec2::centre(90, 15), Vec2::centre(90, 14)), "a step of the ramp");
    assert!(height::melee_reaches(g, Vec2::centre(50, 20), Vec2::centre(51, 20)), "one level");
}

// --- the AI ----------------------------------------------------------------------------------------

#[test]
fn a_chaser_hops_after_her_inside_its_leash() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 12, 19);
    let foe = spawn(&mut s, "quarryman", 12, 8);
    on_her(&mut s, foe, 12, 8);
    let mut hopped = false;
    for _ in 0..600 {
        steps(&mut s, 1);
        hopped |= height::hopping(unit(&s, foe));
    }
    assert!(hopped, "it hopped the ledge");
    assert_eq!(level(&s, foe), 1);
    assert!(cells(unit(&s, foe).pos, unit(&s, me).pos) <= 2, "and came to her");
}

/// A boss never hops: she drops off its ledge and it waits at the top, then evades home whole.
#[test]
fn a_boss_never_hops_it_waits_and_evades_home_healed() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 12, 18);
    let foe = spawn(&mut s, "attendant", 12, 9);
    assert!(jane_data::catalog().combat.unit(unit(&s, foe).def).boss);
    on_her(&mut s, foe, 12, 9);
    let mut evaded = None;
    for t in 0..60 * 9 {
        steps(&mut s, 1);
        let u = unit(&s, foe);
        assert!(!height::hopping(u) && level(&s, foe) == 2, "it never leaves the plateau (tick {t})");
        if t == 60 * 3 {
            assert_eq!(u.combat, CombatState::Combat, "it holds at the edge");
            assert!(cells(u.pos, Vec2::centre(12, 13)) <= 3, "at the top of the ledge: {:?}", u.pos.cell());
        }
        if evaded.is_none() && u.combat == CombatState::Evade {
            evaded = Some(t);
            assert_eq!(u.hp, max_hp(u), "whole as it lets go");
        }
    }
    let t = evaded.expect("it evaded");
    assert!((60 * 5..=60 * 7).contains(&t), "after about six seconds ({t})");
}

/// She is on the plateau over a chaser whose way up is long: it holds at the foot under her,
/// never climbs, and after six seconds evades home whole.
#[test]
fn a_chaser_below_her_holds_at_the_foot_then_evades_whole() {
    let mut s = world();
    let me = her(&s);
    put(&mut s, me, 20, 9);
    let foe = spawn(&mut s, "quarryman", 20, 22);
    on_her(&mut s, foe, 20, 22);
    let mut evaded = None;
    for t in 0..60 * 9 {
        steps(&mut s, 1);
        let u = unit(&s, foe);
        assert_eq!(level(&s, foe), 1, "it never climbs (tick {t})");
        if t == 60 * 3 {
            assert_eq!(u.combat, CombatState::Combat);
            assert!((16..=18).contains(&u.pos.cell().1), "at the foot: {:?}", u.pos.cell());
        }
        if evaded.is_none() && u.combat == CombatState::Evade {
            evaded = Some(t);
            assert_eq!(u.hp, max_hp(u), "whole");
        }
    }
    assert!(evaded.is_some_and(|t| (60 * 5..=60 * 7).contains(&t)), "{evaded:?}");
}

/// A perch row (`holds: level`) never hops, and an AI that may hop does not past its leash.
#[test]
fn who_may_hop() {
    let mut s = world();
    let foe = spawn(&mut s, "quarryman", 12, 8);
    let cat = jane_data::catalog();
    let u = unit(&s, foe).clone();
    let row = *cat.combat.unit(u.def);
    let near = Vec2::centre(12, 16);
    assert!(height::may_hop(Z, &u, &row, near));
    assert!(!height::may_hop(Z, &u, &row, Vec2::centre(12 + 40, 16)), "past its leash");
    let perch = jane_data::UnitDef { holds_level: true, ..row };
    assert!(!height::may_hop(Z, &u, &perch, near));
    let boss = jane_data::UnitDef { boss: true, ..row };
    assert!(!height::may_hop(Z, &u, &boss, near));
}

// --- lockstep and the save -----------------------------------------------------------------------

/// Two seats hop the ledge and walk about while two chasers follow: two sims fed the same frames
/// hash the same every tick, and a third whose runtimes are thrown away and rebuilt every few
/// ticks (mid-hop among them) does too.
#[test]
fn co_op_stays_in_lockstep_through_hops() {
    let party = || {
        let mut s = world();
        cmd(&mut s, Some(0), Command::Open(true));
        cmd(&mut s, None, Command::Join { who: ClientToken(1) });
        let (a, b) = (body_of(&s, 0), body_of(&s, 1));
        put(&mut s, a, 10, 10);
        put(&mut s, b, 13, 10);
        for (x, y) in [(30, 8), (44, 22)] {
            let f = spawn(&mut s, "quarryman", x, y);
            on_her(&mut s, f, x, y);
        }
        s
    };
    let input =
        |f: u32| StepInput { frames: [wander(f, 0), wander(f, 1), InputFrame::IDLE, InputFrame::IDLE], commands: &[] };
    let (mut a, mut b, mut c) = (party(), party(), party());
    let mut air = 0;
    for f in 0..900 {
        a.step(&input(f));
        b.step(&input(f));
        if f % 7 == 3 {
            c.rebuild_runtimes();
        }
        c.step(&input(f));
        assert_eq!(a.hash(), b.hash(), "frame {f}");
        assert_eq!(a.hash(), c.hash(), "rebuilt, frame {f}");
        air += [0usize, 1].iter().filter(|&&i| hopping_seat(&a, i)).count();
    }
    assert!(air > 20, "they hopped ({air} seat-ticks in the air)");
    assert_eq!(a.state(), c.state());
}

/// South for a second (over the ledge), then a stick that turns every two thirds of a second.
fn wander(f: u32, seat: u32) -> InputFrame {
    if f < 60 {
        InputFrame::walk(Angle::SOUTH)
    } else {
        InputFrame::walk(Angle::from_degrees(((f / 40 + seat * 3) * 67 % 360) as i32))
    }
}

fn hopping_seat(s: &Sim, seat: usize) -> bool {
    let id = s.state().players[seat].unit;
    s.state().zone(Z).and_then(|z| z.unit(id)).is_some_and(height::hopping)
}

/// A save made mid-hop loads mid-hop and carries on to the state of the run that never saved.
#[test]
fn a_save_mid_hop_loads_mid_hop() {
    let solo = || {
        let mut s = world();
        let me = her(&s);
        put(&mut s, me, 12, 10);
        s
    };
    let mut straight = solo();
    for f in 0..400 {
        straight.step(&StepInput::solo(wander(f, 0)));
    }
    let mut first = solo();
    let mut at = None;
    for f in 0..400 {
        first.step(&StepInput::solo(wander(f, 0)));
        if hopping_seat(&first, 0) && (f % 2 == 1) {
            at = Some(f);
            break;
        }
    }
    let at = at.expect("she hopped");
    let mut resumed = Sim::from_save_with(&first.save(), blueprints()).expect("the save loads");
    assert!(hopping_seat(&resumed, 0), "loaded mid-hop");
    assert_eq!(resumed.hash(), first.hash());
    for f in at + 1..400 {
        resumed.step(&StepInput::solo(wander(f, 0)));
    }
    assert_eq!(resumed.hash(), straight.hash());
    assert_eq!(resumed.state(), straight.state());
}
