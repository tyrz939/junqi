//! The map as a memory, from the real sim (EXPERIENCE.md §5.2; the world audit's recommendations
//! 3 and 4): a sign read is on the chart only after a rest, with the fire she rested at, and
//! nothing inked lies under the fog; a named place crossed into puts its name up once a day.

use jane_core::action::{Action, TextRef};
use jane_core::{Blueprint, ZoneId};
use jane_present::memory::Note;
use jane_present::view::ViewBuffers;
use jane_sim::event::{Event, EventKind};
use jane_sim::state::{Dialogue, Speaker};
use jane_sim::{Seat, Sim};

/// Stand her on cell `(x, y)` of the county.
fn stand(sim: &mut Sim, x: i32, y: i32) {
    let (z, id) = (sim.state().players[0].zone, sim.state().players[0].unit);
    sim.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)).expect("her body").pos = jane_core::Vec2::centre(x, y);
    sim.rebuild_runtimes();
    // Steps enough for the fog to see where she stands (it is stamped every `FOG_EVERY`).
    for _ in 0..jane_sim::tuning::FOG_EVERY {
        sim.step(&jane_sim::StepInput { frames: [jane_sim::InputFrame::IDLE; 4], commands: &[] });
        sim.drain_events();
    }
}

fn tick(sim: &Sim, b: &mut ViewBuffers, events: &[Event]) {
    let v = sim.view(Seat(0)).expect("seated");
    b.tick(&v, events);
}

fn county(sim: &Sim) -> std::sync::Arc<Blueprint> {
    sim.view(Seat(0)).unwrap().blueprints().get(ZoneId::County).clone()
}

#[test]
fn a_sign_read_is_inked_at_the_next_rest_with_the_fire_and_never_under_the_fog() {
    let mut sim = Sim::new_game(7, "Tess");
    let bp = county(&sim);
    let cat = jane_data::catalog();
    let her = sim.view(Seat(0)).unwrap().body().pos.cell();
    // The nearest thing to read to where she steps off the train, and the Halt's fire.
    let near = |c: jane_core::Cell| (i32::from(c.x) - her.0).pow(2) + (i32::from(c.y) - her.1).pow(2);
    let (sign, words) = bp
        .props
        .iter()
        .filter_map(|p| {
            let l = p.use_list?;
            let t = bp.list(l).unwrap_or(&[]).iter().find_map(|a| match a {
                Action::Read(t) => Some(*t),
                _ => None,
            })?;
            Some((p.cell, t))
        })
        .min_by_key(|(c, _)| near(*c))
        .expect("something to read in the county");
    let fire = bp
        .props
        .iter()
        .filter(|p| {
            let d = cat.story.prop(p.def);
            d.rest && d.light.is_some()
        })
        .min_by_key(|p| near(p.cell))
        .expect("a fire in the county")
        .cell;
    let mut b = ViewBuffers::new();
    // She reads it, standing by it.
    stand(&mut sim, i32::from(sign.x), i32::from(sign.y) + 2);
    tick(&sim, &mut b, &[]);
    sim.state_mut().players[0].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::None, read: Some(words) });
    tick(&sim, &mut b, &[]);
    sim.state_mut().players[0].dialogue = None;
    tick(&sim, &mut b, &[]);
    assert!(b.memory.inked.is_empty(), "nothing on the chart before a rest");
    assert!(
        b.memory.pending.iter().any(|m| matches!(&m.note, Note::Sign(w) if !w.is_empty())),
        "the sign waits for a rest: {:?}",
        b.memory.pending
    );
    if let TextRef::Text(_) = words {
        assert!(!sim.view(Seat(0)).unwrap().text(words).is_empty());
    }
    // She rests at the fire: the quill.
    stand(&mut sim, i32::from(fire.x), i32::from(fire.y) + 2);
    tick(&sim, &mut b, &[Event { to: None, in_zone: None, kind: EventKind::Rest }]);
    assert!(b.memory.pending.is_empty(), "a rest inks what waited");
    assert!(b.memory.inked.iter().any(|m| matches!(m.note, Note::Sign(_))), "the sign is on the chart");
    assert!(b.memory.inked.iter().any(|m| matches!(m.note, Note::Fire(_))), "and the fire she rested at");
    // Nothing inked is under the fog: a sign and a fire are where she stood; a name is a place she
    // entered (seen) or one a sign she read names.
    let v = sim.view(Seat(0)).unwrap();
    for m in &b.memory.inked {
        match &m.note {
            Note::Name(n) => {
                let read = b
                    .memory
                    .inked
                    .iter()
                    .any(|s| matches!(&s.note, Note::Sign(w) if jane_present::memory::names(w, n)));
                assert!(read || v.seen(m.at.0, m.at.1), "{n} is neither seen nor read");
            }
            _ => assert!(v.seen(m.at.0, m.at.1), "{m:?} is under the fog"),
        }
    }
}

#[test]
fn a_named_place_puts_its_name_up_once_a_day() {
    let mut sim = Sim::new_game(7, "Tess");
    let mut b = ViewBuffers::new();
    tick(&sim, &mut b, &[]);
    let place = b
        .crossings
        .places
        .iter()
        .find(|p| p.banner && matches!(p.shape, jane_present::memory::Shape::Ring { .. }))
        .expect("the county has its patches")
        .clone();
    let jane_present::memory::Shape::Ring { c, r } = place.shape else { unreachable!() };
    let (out, inside) = ((c.0 + r + 30, c.1), (c.0 + r / 2, c.1));
    let cross = |sim: &mut Sim, b: &mut ViewBuffers| {
        stand(sim, out.0, out.1);
        tick(sim, b, &[]);
        b.hud.banner = None;
        stand(sim, inside.0, inside.1);
        tick(sim, b, &[]);
        b.hud.banner.clone()
    };
    let first = cross(&mut sim, &mut b).expect("the first crossing today puts the name up");
    assert!(first.place && first.text == place.name, "{first:?} for {}", place.name);
    assert!(cross(&mut sim, &mut b).is_none(), "once a day");
    sim.state_mut().day += 1;
    assert!(cross(&mut sim, &mut b).is_some_and(|bn| bn.text == place.name), "and again the next day");
    assert!(
        b.memory.pending.iter().any(|m| matches!(&m.note, Note::Name(n) if *n == place.name)),
        "its name waits for a rest to be inked"
    );
}
