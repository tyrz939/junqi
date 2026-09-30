//! Behind the terrain (PRESENTATION.md §1.6): a house built of tiles is painted into the ground
//! layers, and what stands behind it is drawn behind it. She walks round Julie's house from its
//! front and in under its eaves from the back (`Tile::Eaves`, walkable), where the frame marks her
//! sprite as standing behind the terrain, seen through it; standing in front of the house, it
//! does not.

use jane_core::{Angle, Tile};
use jane_present::{Depth, Frame, Pass, Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

/// She stands at Julie's front door, then walks each leg; the presenter ticks beside.
fn walk(legs: &[(Angle, u32)]) -> (Sim, Present) {
    walk_at(11, legs)
}

/// As [`walk`], at `hour`.
fn walk_at(hour: u8, legs: &[(Angle, u32)]) -> (Sim, Present) {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let mark = sim.state().syms.find("house_front").expect("Julie's house has a front");
    let cmds = [
        StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::God(true)) },
        StampedCommand { seat: Some(Seat(0)), seq: 2, cmd: Command::Dev(DevOp::Time { hour }) },
        StampedCommand {
            seat: Some(Seat(0)),
            seq: 3,
            cmd: Command::Dev(DevOp::Tp { zone: jane_core::ids::ZoneId::County, mark }),
        },
    ];
    let tick = |sim: &mut Sim, p: &mut Present, frame: InputFrame, cmds: &[StampedCommand]| {
        sim.step(&StepInput { frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
    };
    tick(&mut sim, &mut p, InputFrame::IDLE, &cmds);
    for _ in 0..60 {
        tick(&mut sim, &mut p, InputFrame::IDLE, &[]);
    }
    for &(dir, n) in legs {
        for _ in 0..n {
            tick(&mut sim, &mut p, InputFrame::walk(dir), &[]);
        }
    }
    (sim, p)
}

/// Her sprite's `Foot`, if the frame marks one: hers is the standing sprite nearest the middle.
fn her_foot(f: &Frame) -> Option<jane_present::Foot> {
    let cmds = f.passes.iter().find_map(|q| match *q {
        Pass::Sprites { layer: Depth::Standing, cmds } => Some(cmds),
        _ => None,
    })?;
    let (cx, cy) = (i32::from(CANVAS.0) / 2, i32::from(CANVAS.1) / 2);
    f.sprites_in(cmds)
        .iter()
        .filter(|s| {
            let (x, y) = (i32::from(s.x), i32::from(s.y));
            (x..x + i32::from(s.src.w)).contains(&cx) && (y..y + i32::from(s.src.h)).contains(&(cy - 8))
        })
        .find_map(|s| s.foot)
}

#[test]
fn behind_julies_house_she_stands_under_its_eaves_and_is_seen_through_its_roof() {
    // Round the west end of the house, north past it, east along its back, then south under it.
    let (sim, mut p) = walk(&[(Angle::WEST, 90), (Angle::NORTH, 140), (Angle::EAST, 75), (Angle::SOUTH, 60)]);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    let (x, y) = v.body().pos.cell();
    assert_eq!(v.tile(x, y), Tile::Eaves, "she walked in under the eaves, at ({x}, {y})");
    // South of her, the roof over the house blocks: she is as near its true back as a row allows.
    assert_eq!(v.tile(x, y + 1), Tile::HouseRoof);
    let f = p.draw(255, CANVAS);
    let foot = her_foot(f).expect("behind the roof, her sprite stands behind the terrain");
    assert!(foot.see, "a player is seen through what hides her");
}

#[test]
fn in_front_of_julies_house_nothing_stands_before_her() {
    let (_, mut p) = walk(&[(Angle::SOUTH, 10)]);
    let f = p.draw(255, CANVAS);
    assert_eq!(her_foot(f), None);
}

/// Round the west end of Julie's house, north past it, east along its back, then south under
/// its eaves.
const BEHIND: [(Angle, u32); 4] = [(Angle::WEST, 90), (Angle::NORTH, 140), (Angle::EAST, 75), (Angle::SOUTH, 60)];

#[test]
fn behind_julies_house_at_night_her_lantern_falls_on_the_ground_not_the_roof() {
    let (sim, mut p) = walk_at(22, &BEHIND);
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    let (x, y) = v.body().pos.cell();
    assert_eq!(v.tile(x, y), Tile::Eaves, "she walked in under the eaves, at ({x}, {y})");
    let f = p.draw(255, CANVAS);
    let feet = i32::from(her_foot(f).expect("behind the roof").y);
    let lantern = f.lights.iter().find(|l| l.radius == 136 && l.size == 3).expect("her lantern is lit at 22:00");
    // Its ground point is by her feet (two rows at most either way as she faces), not the
    // roof's ground a lift's rows south; and it shines from her hand, not the roof's height.
    assert!((lantern.pos.1 - feet).abs() <= 2, "its pool lies on her row {feet}, not at {}", lantern.pos.1);
    assert!(lantern.height < 40, "it shines from her hand, not over the roof: {} px", lantern.height);
}
