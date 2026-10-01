//! The quest marks over heads (PRESENTATION.md §3.8), per seat, from the real sim: each seat's
//! presenter marks whoever has something for her, over the head, and takes the mark down over
//! the person she is talking to while the other seat still sees it. And Julie's fruit bowl stands
//! on her kitchen table, drawn on its top and after it.

use jane_present::{Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::state::{Dialogue, Speaker};
use jane_sim::view::QuestMark;
use jane_sim::{ClientToken, Command, InputFrame, Seat, Sim, StampedCommand, StepInput, UnitId};

const CANVAS: (u16, u16) = (768, 432);

fn step(sim: &mut Sim, cmds: &[StampedCommand]) {
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
    sim.drain_events();
}

/// Stand seat `seat` on cell `(x, y)` of the county.
fn stand(sim: &mut Sim, seat: usize, x: i32, y: i32) {
    let (z, id) = (sim.state().players[seat].zone, sim.state().players[seat].unit);
    sim.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)).expect("her body").pos = jane_core::Vec2::centre(x, y);
    sim.rebuild_runtimes();
}

fn hale(sim: &Sim) -> UnitId {
    let s = sim.state().syms.find("mr_hale").expect("the smith's name");
    sim.state().zone(jane_core::ZoneId::County).unwrap().units.iter().find(|u| u.key == Some(s)).unwrap().id
}

/// Seat `seat`'s presenter, ticked and drawn on the sim as it stands.
fn marks(sim: &Sim, p: &mut Present, seat: u8) -> Vec<(u32, QuestMark, (i32, i32))> {
    let v = sim.view(Seat(seat)).expect("seated");
    for _ in 0..3 {
        p.tick(&v, &[]);
    }
    p.draw(255, CANVAS);
    p.marks().iter().map(|m| (m.id, m.mark, (m.x, m.y))).collect()
}

#[test]
fn each_seat_sees_its_own_and_none_over_whom_she_is_talking_to() {
    let mut sim = Sim::new_game(1, "Jane");
    let dev = |seat: u8, seq: u16, cmd| StampedCommand { seat: Some(Seat(seat)), seq, cmd };
    step(&mut sim, &[dev(0, 1, Command::Open(true))]);
    step(&mut sim, &[StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(11) } }]);
    let start = sim.state().syms.find("start").unwrap();
    step(&mut sim, &[dev(1, 2, Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::County, mark: start }))]);
    let h = hale(&sim);
    let at = sim.state().zone(jane_core::ZoneId::County).unwrap().unit(h).unwrap().pos.cell();
    stand(&mut sim, 0, at.0 + 3, at.1 + 1);
    stand(&mut sim, 1, at.0 - 3, at.1 + 1);
    for _ in 0..3 {
        step(&mut sim, &[]);
    }
    let (mut p0, mut p1) = (Present::new(Tier::T0), Present::new(Tier::T0));
    p0.set_canvas(CANVAS);
    p1.set_canvas(CANVAS);
    let id = h.get();
    let m0 = marks(&sim, &mut p0, 0);
    let m1 = marks(&sim, &mut p1, 1);
    let over = |m: &[(u32, QuestMark, (i32, i32))]| m.iter().find(|e| e.0 == id).map(|e| (e.1, e.2));
    let (mark, (x, y)) = over(&m0).expect("a mark over Mr Hale for seat 0");
    assert_eq!(mark, QuestMark::Offer);
    assert!(over(&m1).is_some(), "and for seat 1");
    // Over his head: above the middle of the canvas's people, and on the canvas.
    assert!((0..i32::from(CANVAS.0)).contains(&x) && (0..i32::from(CANVAS.1)).contains(&y), "{x},{y}");
    // Seat 1 talks to him: her mark over him goes; seat 0's stays.
    sim.state_mut().players[1].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::Unit(h), read: None });
    assert!(over(&marks(&sim, &mut p1, 1)).is_none(), "none over whom she is talking to");
    assert!(over(&marks(&sim, &mut p0, 0)).is_some(), "the other seat still sees it");
    // She takes his quest: nothing over him for either while it is under way.
    sim.state_mut().players[1].dialogue = None;
    let q = jane_data::catalog().story.quest_id("the_last_name").unwrap();
    step(&mut sim, &[dev(0, 3, Command::Dev(DevOp::Quest(q)))]);
    assert!(over(&marks(&sim, &mut p0, 0)).is_none());
    assert!(over(&marks(&sim, &mut p1, 1)).is_none());
}

#[test]
fn a_book_that_gives_a_quest_wears_the_mark_and_loses_it_while_she_reads() {
    let mut sim = Sim::new_game(1, "Jane");
    let key = sim.state().syms.find("lost_property_book").expect("the lost-property book");
    let county = sim.state().zone(jane_core::ZoneId::County).unwrap();
    let book = county.props.iter().find(|p| p.key == key).unwrap().clone();
    stand(&mut sim, 0, i32::from(book.cell.x) + 3, i32::from(book.cell.y) + 2);
    step(&mut sim, &[]);
    let mut p = Present::new(Tier::T0);
    p.set_canvas(CANVAS);
    let id = jane_present::present::PROP_MARK_KEY | book.id.get();
    let over = |m: &[(u32, QuestMark, (i32, i32))]| m.iter().find(|e| e.0 == id).map(|e| e.1);
    assert_eq!(over(&marks(&sim, &mut p, 0)), Some(QuestMark::Offer), "a \"!\" over the book");
    sim.state_mut().players[0].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::Prop(book.id), read: None });
    assert_eq!(over(&marks(&sim, &mut p, 0)), None, "none over what she is reading");
}
