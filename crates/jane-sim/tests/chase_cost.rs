//! What a chase costs, on ground with levels and on the same ground flat (MAP.md R2 follow-up,
//! "chasers": the search count was the gap). Six strong chasers hold her for 6 000 ticks while she
//! walks a loop; each ground is run twice, with its levels and spans and without them (the cliffs
//! and ledges a gap of road, so she walks the same loop), beside a plain field, and a small loop
//! round a pillar where they keep on her. Printed for the report (`cargo test -p jane-sim --release
//! --test chase_cost -- --nocapture`): the tick's best of three runs in ns (a machine's timing is
//! not a test), the searches, nodes and failures, which are exact, and how hard they press her
//! (the mean distance of a chaser from her, the share of chaser-ticks within three cells, the blows
//! begun), which is the chase's feel. What is held, because no machine moves it: the searches in a run,
//! the most nodes a tick opens, and round the pillar the blows begun and the distance kept.

// The clock here times the steps from outside them, for the report; it never reaches the sim.
#![allow(clippy::disallowed_types)]

mod field;

use std::sync::Arc;
use std::time::Instant;

use field::{Z, body_of, cmd, edit, spawn};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::num::CELL_FX;
use jane_core::{Angle, Blueprint, Cell, Key, Rect, Tile, Vec2, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::height::terraces as T;
use jane_sim::path::{PATH_BUDGET, PATH_TICK_BUDGET, PathStats};
use jane_sim::span::viaduct as V;
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, InputFrame, Sim, StepInput};

/// A ground: its blueprint of the county zone, her cell, the chasers' cells and her walk.
struct Ground {
    name: &'static str,
    blueprint: fn(bool) -> Blueprint,
    start: (i32, i32),
    chasers: &'static [(i32, i32)],
    walk: fn(&Sim, u32) -> Angle,
    /// The stick's push, 0 to 127: she walks at her god's double pace at 127, and a chaser (1.2 px a
    /// tick against her 2) overtakes her below about 76.
    pace: u8,
    /// The most searches a run may make, either way: well under what the chase made before it planned
    /// only on a real change (4,046 on the terraces, 535 flat, 3,473 in the field, 1,823 round the
    /// pillar), with room for a change of tuning.
    searches: u64,
}

fn terraces_blueprint(levels: bool) -> Blueprint {
    let mut bp = T::blueprint(Z);
    if !levels {
        bp.level = None;
        bp.tiles.fill_rect(T::LEDGE, Tile::Road);
    }
    bp
}

fn viaduct_blueprint(levels: bool) -> Blueprint {
    let mut bp = V::blueprint(Z);
    if !levels {
        bp.level = None;
        bp.spans.clear();
        bp.tiles.fill_rect(V::FACE, Tile::Road);
        bp.tiles.fill_rect(V::RIM, Tile::Road);
        bp.tiles.fill_rect(V::STAIR, Tile::Road);
        bp.tiles.fill_rect(V::RIM_STAIR, Tile::Road);
    }
    bp
}

/// A field of grass, walled round, with a wall across its middle for them to go round.
fn field_blueprint(_: bool) -> Blueprint {
    let mut bp = Blueprint::new(Z, 128, 64, Tile::Grass);
    bp.tiles.fill_rect(Rect::new(0, 0, 128, 1), Tile::Wall);
    bp.tiles.fill_rect(Rect::new(0, 63, 128, 1), Tile::Wall);
    bp.tiles.fill_rect(Rect::new(0, 0, 1, 64), Tile::Wall);
    bp.tiles.fill_rect(Rect::new(127, 0, 1, 64), Tile::Wall);
    bp.tiles.fill_rect(Rect::new(64, 20, 1, 30), Tile::Wall);
    bp
}

/// The same field with a pillar of three cells a side where the wall was.
fn pillar_blueprint(_: bool) -> Blueprint {
    let mut bp = field_blueprint(false);
    bp.tiles.fill_rect(Rect::new(64, 20, 1, 30), Tile::Grass);
    bp.tiles.fill_rect(Rect::new(59, 29, 3, 3), Tile::Wall);
    bp
}

/// The terraces loop (`height_alloc`): west along the plateau, south off the ledge, east below
/// the face, north up the ladder.
fn terraces_walk(s: &Sim, _: u32) -> Angle {
    let me = body_of(s, 0);
    let u = s.state().zone(Z).and_then(|z| z.unit(me)).expect("her");
    let (x, y) = u.pos.cell();
    if y < T::FACE.y {
        if x > 12 { if y > 10 { Angle::NORTH } else { Angle::WEST } } else { Angle::SOUTH }
    } else if y < 20 && x < 58 {
        Angle::SOUTH
    } else {
        let mid = Vec2::centre(60, 0).x.0;
        if u.pos.x.0 < mid - 128 {
            Angle::EAST
        } else if u.pos.x.0 > mid + 128 {
            Angle::WEST
        } else {
            Angle::NORTH
        }
    }
}

/// The viaduct: over the deck and back, then along the towpath and the terraces' edges.
fn viaduct_walk(_: &Sim, f: u32) -> Angle {
    [Angle::SOUTH, Angle::EAST, Angle::NORTH, Angle::WEST][(f / 240 % 4) as usize]
}

/// A square in the field, a few cells a side.
fn field_walk(_: &Sim, f: u32) -> Angle {
    [Angle::EAST, Angle::SOUTH, Angle::WEST, Angle::NORTH][(f / 150 % 4) as usize]
}

/// Round the pillar at a distance of four cells.
fn pillar_walk(s: &Sim, _: u32) -> Angle {
    let me = body_of(s, 0);
    let (x, y) = s.state().zone(Z).and_then(|z| z.unit(me)).expect("her").pos.cell();
    if y <= 25 && x < 65 {
        Angle::EAST
    } else if x >= 65 && y < 35 {
        Angle::SOUTH
    } else if y >= 35 && x > 55 {
        Angle::WEST
    } else {
        Angle::NORTH
    }
}

const GROUNDS: [Ground; 4] = [
    Ground {
        name: "terraces",
        blueprint: terraces_blueprint,
        start: (30, 10),
        chasers: &[(20, 8), (40, 6), (24, 22), (44, 24), (70, 20), (66, 9)],
        walk: terraces_walk,
        pace: 127,
        searches: 600,
    },
    Ground {
        name: "viaduct",
        blueprint: viaduct_blueprint,
        start: (31, 14),
        chasers: &[(36, 10), (20, 24), (44, 24), (26, 6), (50, 34), (12, 36)],
        walk: viaduct_walk,
        pace: 127,
        searches: 300,
    },
    Ground {
        name: "field",
        blueprint: field_blueprint,
        start: (30, 30),
        chasers: &[(20, 28), (40, 24), (24, 38), (44, 36), (36, 18), (34, 44)],
        walk: field_walk,
        pace: 127,
        searches: 800,
    },
    Ground {
        name: "pillar",
        blueprint: pillar_blueprint,
        start: (55, 25),
        chasers: &[(50, 22), (66, 24), (68, 36), (52, 38)],
        walk: pillar_walk,
        pace: 40,
        searches: 900,
    },
];

fn world(g: &Ground, levels: bool) -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            let mut bp = (g.blueprint)(levels);
            let cell = Cell::new(g.start.0 as u16, g.start.1 as u16);
            bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell, facing: Some(Facing::East) });
            return Arc::new(bp);
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let me = body_of(&s, 0);
    edit(&mut s, me, |u| u.pos = Vec2::centre(g.start.0, g.start.1));
    for &(x, y) in g.chasers {
        let f = spawn(&mut s, "quarryman", x, y);
        // Strong enough to outlive a long run; they are set on her at once.
        edit(&mut s, f, |u| {
            u.strength = 2000;
            u.hp = jane_sim::units::max_hp(u);
            u.target = Some(me);
            u.combat = CombatState::Combat;
        });
    }
    s
}

/// One tick of the loop; how many blows the foes began in it.
fn tick(g: &Ground, s: &mut Sim, f: u32) -> u64 {
    let dir = (g.walk)(s, f);
    let frame = InputFrame { aim: Some(dir), mv_mag: g.pace, ..InputFrame::walk(dir) };
    s.step(&StepInput { frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: &[] });
    s.drain_events().iter().filter(|e| matches!(e.kind, EventKind::Windup { .. })).count() as u64
}

const WARM: u32 = 600;
const TICKS: u32 = 6000;

/// One run: its tick in ns and its path counters (warm-up included, as `height_alloc` counts).
fn run(g: &Ground, levels: bool) -> (u64, PathStats) {
    let mut s = world(g, levels);
    for f in 0..WARM {
        tick(g, &mut s, f);
    }
    let t = Instant::now();
    for f in WARM..WARM + TICKS {
        tick(g, &mut s, f);
    }
    (t.elapsed().as_nanos() as u64 / u64::from(TICKS), s.path_stats())
}

fn clock() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// The controllers' share of a run, in ns a tick (a separate run: the wall clock costs).
fn controllers(g: &Ground, levels: bool) -> u64 {
    let mut s = world(g, levels);
    s.set_wall_clock(Some(clock));
    for f in 0..WARM {
        tick(g, &mut s, f);
    }
    let mut ctl = 0u64;
    for f in WARM..WARM + TICKS {
        tick(g, &mut s, f);
        ctl += u64::from(s.metrics().phase_ns[jane_sim::metrics::Phase::Controllers.index()]);
    }
    ctl / u64::from(TICKS)
}

/// How hard they press her: the mean distance of a chaser from her (hundredths of a cell), the
/// per cent of chaser-ticks within three cells of her, and the blows begun.
fn pressure(g: &Ground, levels: bool) -> (u64, u64, u64, u32) {
    let mut s = world(g, levels);
    for f in 0..WARM {
        tick(g, &mut s, f);
    }
    let me = body_of(&s, 0);
    let (mut sum, mut near, mut n, mut blows, mut peak) = (0u64, 0u64, 0u64, 0u64, 0u32);
    for f in WARM..WARM + TICKS {
        blows += tick(g, &mut s, f);
        peak = peak.max(s.metrics().path_expanded);
        let z = s.state().zone(Z).expect("zone");
        let her = z.unit(me).expect("her").pos;
        for u in z.units.iter().filter(|u| u.id != me && u.controller == jane_data::Controller::Ai) {
            let d = jane_sim::combat::distance(u.pos, her) * 100 / i64::from(CELL_FX);
            sum += d as u64;
            near += u64::from(d <= 300);
            n += 1;
        }
    }
    (sum / n.max(1), near * 100 / n.max(1), blows, peak)
}

#[test]
fn what_a_chase_costs_on_levels_and_flat() {
    for g in &GROUNDS {
        let feel = g.name == "pillar";
        let mut rows = Vec::new();
        for levels in [false, true] {
            let mut best = u64::MAX;
            let mut stats = PathStats::default();
            for _ in 0..3 {
                let (ns, ps) = run(g, levels);
                best = best.min(ns);
                stats = ps;
            }
            rows.push((levels, best, stats, controllers(g, levels), pressure(g, levels)));
        }
        let flat = rows[0].1.max(1);
        for (levels, ns, ps, ctl, (dist, near, blows, peak)) in &rows {
            println!(
                "{:9} {:6}: {ns:5} ns a tick ({}.{:02}x flat, {ctl} in the controllers); {} searches, {} failed, {} steered, {} nodes; \
                 {} kept, {} straight, {} refused, {} held; mean distance {}.{:02} cells, {near}% within 3, {blows} blows; at most {peak} nodes a tick",
                g.name,
                if *levels { "levels" } else { "flat" },
                ns * 100 / flat / 100,
                ns * 100 / flat % 100,
                ps.searches,
                ps.failed,
                ps.steered,
                ps.expanded,
                ps.kept,
                ps.straight,
                ps.refused,
                ps.held,
                dist / 100,
                dist % 100,
            );
            // What is held is the lean chase's (with the old rule on, this prints what it was).
            if jane_sim::tuning::LEAN_CHASE {
                assert!(
                    ps.searches <= g.searches,
                    "{}: {} searches in a run, no more than {} planned",
                    g.name,
                    ps.searches,
                    g.searches
                );
                assert!(*peak <= PATH_TICK_BUDGET + PATH_BUDGET, "{}: a tick opened {peak} nodes", g.name);
                if feel {
                    // The chase's feel is what it was: before, 156 blows begun and 3.22 cells mean.
                    assert!(*blows >= 140, "round the pillar they began {blows} blows, not near 156");
                    assert!(*dist <= 380, "and kept {dist}/100 cells from her, not near 322");
                }
            }
        }
    }
}
