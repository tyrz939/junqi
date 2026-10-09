//! Which lights cast (PRESENTATION.md §1.7, decided 2026-09-27): one rule in the presenter, the
//! nearest `Features::shadows` of those that may cast, so a tier that casts from fewer casts from
//! the nearest of the same lights a tier of more casts from. The town at 22:00 at every tier.

use jane_present::{Features, Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

/// A light by its ground point and height.
type Seen = ((i32, i32), u8);

/// The casting lights of the town at 22:00 drawn at `tier`: ground point and height, nearest the
/// middle first; and how many lights the frame draws.
fn casting(tier: Tier) -> (Vec<Seen>, usize) {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(tier);
    let cmd = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour: 22 }) }];
    for k in 0..90 {
        let cmds: &[StampedCommand] = if k == 0 { &cmd } else { &[] };
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
    }
    let f = p.draw(0, (768, 432));
    let d2 = |p: (i32, i32)| {
        let (dx, dy) = (i64::from(p.0 - 384), i64::from(p.1 - 216));
        dx * dx + dy * dy
    };
    let mut c: Vec<_> = f.lights.iter().filter(|l| l.casts).map(|l| (l.pos, l.height)).collect();
    c.sort_by_key(|l| d2(l.0));
    (c, f.lights.len())
}

#[test]
fn every_tier_casts_from_the_nearest_of_the_same_lights() {
    let tiers = [Tier::T0, Tier::T1, Tier::T2].map(|t| (t, casting(t)));
    let (t2, _) = &tiers[2].1;
    assert!(t2.len() > Features::of(Tier::T0).shadows as usize, "too few lamps in view to tell: {}", t2.len());
    for (t, (c, _)) in &tiers {
        let n = usize::from(Features::of(*t).shadows).min(t2.len());
        // As many as the tier's row says, and they are the first of T2's: the nearest.
        assert_eq!(c.len(), n, "{t:?} casts from {} lights, its row says {n}", c.len());
        assert_eq!(c[..], t2[..n], "{t:?} casts from other lights than the nearest of T2's");
    }
}

#[test]
fn a_frame_casts_from_no_more_lights_than_its_row() {
    use jane_present::present::pick_lights;
    use jane_present::{Light, LightKind};
    let light = |x: i32, casts: bool| Light {
        pos: (x, 0),
        height: 30,
        colour: [255; 3],
        radius: 100,
        size: 4,
        casts,
        kind: LightKind::Point,
        holder: None,
        base: 0,
    };
    // Eight lamps and eight glows, the glows nearer: four cast, the nearest lamps, and when the
    // frame keeps six, the four casting ones are among them.
    let mut v: Vec<Light> = (0..8).map(|k| light(100 + k * 10, true)).chain((0..8).map(|k| light(k, false))).collect();
    pick_lights(&mut v, (0, 0), 6, 4);
    assert_eq!(v.len(), 6);
    let casting: Vec<i32> = v.iter().filter(|l| l.casts).map(|l| l.pos.0).collect();
    assert_eq!(casting, [100, 110, 120, 130]);
    assert_eq!(v.iter().filter(|l| !l.casts).map(|l| l.pos.0).collect::<Vec<_>>(), [0, 1]);
}
