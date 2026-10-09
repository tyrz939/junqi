//! Height and decks in the frame (MAP.md §6.2), on the dev grounds (`jane_sim::dev_ground`): on a
//! plateau she stands on its height (her caster's base, nothing of the plateau hiding her); on a
//! deck she is drawn over its pieces; under it the deck is drawn over her and she is seen through
//! it; a light's flat pool lies where its ground is drawn.

use jane_core::Angle;
use jane_present::{Depth, Frame, Pass, Present, SpriteCmd, Tier, Tint};
use jane_sim::dev_ground::{self, Ground};
use jane_sim::input::DevOp;
use jane_sim::{Blueprints, Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (640, 360);

/// New Game on dev ground `g`, she put at `(x, y)` and walked `legs`; the presenter beside.
fn stand(g: Ground, (x, y): (i32, i32), legs: &[(Angle, u32)]) -> (Sim, Present) {
    let bps = dev_ground::stand_in(Blueprints::build(1).expect("seed 1 builds"), g);
    let mut sim = Sim::new_game_with(bps, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let county = jane_core::ids::ZoneId::County;
    let cat = jane_data::catalog();
    let mark = sim.state().syms.find(cat.name(cat.story.start.mark)).expect("a start mark");
    sim.state_mut().players[0].travel =
        Some(jane_sim::state::TravelRequest { zone: county, mark, at: Some(jane_core::num::Vec2::centre(x, y)) });
    let god = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::God(true)) }];
    let tick = |sim: &mut Sim, p: &mut Present, frame: InputFrame, cmds: &[StampedCommand]| {
        sim.step(&StepInput { frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
    };
    tick(&mut sim, &mut p, InputFrame::IDLE, &god);
    for _ in 0..30 {
        tick(&mut sim, &mut p, InputFrame::IDLE, &[]);
    }
    for &(dir, n) in legs {
        for _ in 0..n {
            tick(&mut sim, &mut p, InputFrame::walk(dir), &[]);
        }
    }
    for _ in 0..4 {
        tick(&mut sim, &mut p, InputFrame::IDLE, &[]);
    }
    (sim, p)
}

/// The standing pass's sprites, and the index among them of hers: the one whose caster's foot is
/// the canvas's middle (the camera follows her).
fn standing(f: &Frame) -> (&[SpriteCmd], Option<usize>) {
    let cmds = f
        .passes
        .iter()
        .find_map(|q| match *q {
            Pass::Sprites { layer: Depth::Standing, cmds }
                if f.sprites_in(cmds).iter().all(|s| s.flags.tint != Tint::Seen) =>
            {
                Some(cmds)
            }
            _ => None,
        })
        .expect("a standing pass");
    // Hers is the standing sprite her seen copy (`Tint::Seen`, drawn after) stands on.
    let seen = f.sprites.iter().find(|s| s.flags.tint == Tint::Seen).expect("her seen copy");
    let her = f
        .sprites_in(cmds)
        .iter()
        .position(|s| s.flags.tint != Tint::Seen && s.x == seen.x && s.y == seen.y && s.src == seen.src);
    (f.sprites_in(cmds), her)
}

fn overlaps(a: &SpriteCmd, b: &SpriteCmd) -> bool {
    let (ax, ay, aw, ah) = (i32::from(a.x), i32::from(a.y), i32::from(a.src.w), i32::from(a.src.h));
    let (bx, by, bw, bh) = (i32::from(b.x), i32::from(b.y), i32::from(b.src.w), i32::from(b.src.h));
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

#[test]
fn on_the_plateau_she_stands_on_its_height_and_it_hides_nothing_of_her() {
    let (sim, mut p) = stand(Ground::Terraces, (14, 10), &[]);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    assert_eq!(v.unit_level(v.body()), 2, "she is on the plateau");
    let f = p.draw(255, CANVAS);
    let (sprites, i) = standing(f);
    let me = &sprites[i.expect("her sprite")];
    let her = f
        .casters
        .iter()
        .find(|c| f.sprites.get(c.sprite as usize).is_some_and(|s| s.x == me.x && s.y == me.y && s.src == me.src))
        .expect("she casts");
    assert_eq!(i32::from(her.base), 2 * jane_art::terrain::levels::LEVEL_PX, "her shadow stands on the plateau");
    assert!(me.foot.is_none(), "the plateau she stands on is not in front of her: {:?}", me.foot);
}

#[test]
fn on_a_deck_she_is_drawn_over_it_and_under_it_the_deck_is_drawn_over_her() {
    // On the viaduct: from its north end, south along it.
    let (sim, mut p) = stand(Ground::Viaduct, (31, 16), &[(Angle::SOUTH, 28)]);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    assert!(v.on_deck(v.body()).is_some(), "she is on the deck");
    let f = p.draw(255, CANVAS);
    let (sprites, i) = standing(f);
    let i = i.expect("her sprite");
    let me = sprites[i];
    assert_eq!(me.foot.map(|f| f.base), Some(jane_art::terrain::levels::LEVEL_PX as u8), "she stands at the deck's");
    assert!(
        !sprites[i + 1..].iter().any(|s| overlaps(s, &me) && s.foot.is_some_and(|f| f.y == i16::MAX)),
        "no deck over her"
    );
    assert!(
        sprites[..i].iter().any(|s| overlaps(s, &me) && s.foot.is_some_and(|f| f.y == i16::MAX)),
        "the deck under her"
    );

    // On the towpath under it.
    let (sim, mut p) = stand(Ground::Viaduct, (31, 22), &[]);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    assert!(v.on_deck(v.body()).is_none() && v.unit_level(v.body()) == 0, "she is under the deck");
    let f = p.draw(255, CANVAS);
    let (sprites, i) = standing(f);
    let i = i.expect("her sprite");
    let me = sprites[i];
    assert!(
        sprites[i + 1..].iter().any(|s| overlaps(s, &me) && s.foot.is_some_and(|f| f.y == i16::MAX)),
        "the deck over her"
    );
    let seen = f.passes.iter().any(|q| match *q {
        Pass::Sprites { cmds, .. } => {
            f.sprites_in(cmds).iter().any(|s| s.flags.tint == Tint::Seen && s.x == me.x && s.y == me.y)
        }
        _ => false,
    });
    assert!(seen, "she is seen through the deck");
}

#[test]
fn a_lights_flat_pool_lies_where_its_ground_is_drawn() {
    let l = jane_present::Light {
        pos: (100, 132),
        height: 60,
        colour: [255; 3],
        radius: 40,
        size: 3,
        casts: false,
        kind: jane_present::LightKind::Point,
        holder: None,
        base: 40,
    };
    assert_eq!(l.drawn_ground(), (100, 100));
    assert_eq!(jane_present::Light { base: 0, ..l }.drawn_ground(), (100, 132));
    let foot = jane_present::Foot { y: 200, see: false, base: 40 };
    assert!(!foot.hides_over(44, 190), "a plateau's tuft under her hides nothing");
    assert!(foot.hides_over(90, 190), "a house on the plateau, in front of her, does");
}
