//! The co-op rules (PLATFORM.md §2, §7; PLAN.md, decided 2026-09-21) where the network touches
//! them, played through lockstep peers: the party penalty follows the head count as seats come
//! and go; keys never leave with a guest; the world is plain single-player again when the
//! guests leave; a guest's rest saves the host's world; everyone arrives at the party's fire;
//! each sees only her own numbers.

mod common;

use common::*;
use jane_bot::task::{Ctx, Status, Task, UseProp};
use jane_bot::{Act, Model};
use jane_core::Milli;
use jane_core::action::School;
use jane_net::{HostConfig, Note};
use jane_sim::event::EventKind;
use jane_sim::input::{Command, DevOp};
use jane_sim::tuning::PARTY_TAKEN;
use jane_sim::{Hit, Seat};

/// The same blow on seat `s`'s body on every peer at the same frame (a test's hand, so every
/// peer's), and the damage each said it did.
fn strike(t: &mut Table, s: Seat) -> Vec<Milli> {
    t.same_hash();
    let mut sims: Vec<&mut jane_sim::Sim> = vec![t.host.sim_mut()];
    for p in &mut t.peers {
        if !p.frozen {
            if let Some(sim) = p.g.sim_mut() {
                sims.push(sim);
            }
        }
    }
    for sim in sims {
        let p = sim.state().player(s).unwrap();
        let (z, u) = (p.zone, p.unit);
        sim.queue_hit(
            z,
            Hit { to: u, amount: Milli(10_000), school: School::Physical, from: None, crit: false, status: None },
        );
    }
    let unit = t.host.sim().state().player(s).unwrap().unit;
    t.tick();
    let mut out = vec![];
    let dmg = |evs: &[jane_sim::Event]| {
        evs.iter().find_map(|e| match e.kind {
            EventKind::Damage { unit: u, amount, .. } if u == unit => Some(amount),
            _ => None,
        })
    };
    out.push(dmg(&t.host_heard).expect("the host saw the blow"));
    for p in &t.peers {
        if !p.frozen && p.g.sim().is_some() {
            out.push(dmg(&p.heard).expect("a guest saw the blow"));
        }
    }
    t.same_hash();
    out
}

fn one(v: &[Milli]) -> Milli {
    assert!(v.windows(2).all(|w| w[0] == w[1]), "every peer dealt the same: {v:?}");
    v[0]
}

#[test]
fn the_penalty_follows_the_head_count_as_seats_come_and_go() {
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    t.run(30);
    let solo = one(&strike(&mut t, Seat(0)));
    let mut seen = vec![(1, solo)];
    let mut guests = vec![];
    for n in 2..=4u8 {
        let i = t.knock(100 + u64::from(n), Policy::Idle);
        guests.push(t.seated(i));
        t.run(40);
        assert_eq!(t.host.sim().state().party_size(), n);
        for p in &t.peers {
            assert_eq!(p.g.sim().unwrap().state().party_size(), n, "every peer counts the same table");
        }
        // Hurt more for every player connected (PLATFORM.md §2: 115%, 130%, 145%).
        let d = one(&strike(&mut t, Seat(0)));
        let want = Milli(
            ((i64::from(solo.0) * i64::from(PARTY_TAKEN[usize::from(n) - 1].0) / 1000 + 500) / 1000 * 1000) as i32,
        );
        assert_eq!(d, want, "{n} at the table");
        seen.push((n, d));
    }
    println!("damage by head count: {seen:?}");
    // Guests leave one by one: the penalty eases with each.
    for n in (1..=3u8).rev() {
        let mut p = t.peers.pop().unwrap();
        p.g.leave();
        t.run(20);
        assert_eq!(t.host.sim().state().party_size(), n);
        let d = one(&strike(&mut t, Seat(0)));
        assert_eq!(d, seen[usize::from(n) - 1].1, "{n} at the table again");
    }
}

#[test]
fn a_key_never_leaves_with_a_guest() {
    let cat = jane_data::catalog();
    let (key, _) = cat
        .combat
        .items
        .iter()
        .enumerate()
        .find(|(_, d)| d.story && d.opens.is_some())
        .map(|(i, d)| (jane_core::ItemId(i as u16), d))
        .expect("a key in the content");
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    let mut given = false;
    let a = t.knock(
        201,
        Policy::Script(Box::new(move |_, _| {
            if std::mem::replace(&mut given, true) {
                Act::idle()
            } else {
                Act::press(Command::Dev(DevOp::Give { item: key, qty: 1 }))
            }
        })),
    );
    let b = t.knock(202, Policy::Idle);
    let sa = t.seated(a);
    t.seated(b);
    t.run(120);
    let holds =
        |sim: &jane_sim::Sim, s: Seat| sim.state().player(s).unwrap().bag.iter().flatten().any(|st| st.item == key);
    assert!(holds(t.host.sim(), sa), "the guest holds the key");
    assert!(!holds(t.host.sim(), Seat(0)));
    // Her machine drops off the network.
    t.peers.remove(a);
    t.run(30);
    assert!(t.notes.contains(&Note::Left { seat: sa }));
    assert!(!holds(t.host.sim(), sa), "it did not go with her");
    assert!(holds(t.host.sim(), Seat(0)), "the host, who stayed, holds it");
    assert!(holds(t.peers[0].g.sim().unwrap(), Seat(0)), "on every machine");
    t.same_hash();
}

#[test]
fn when_the_guests_leave_it_is_single_player_again() {
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    t.run(30);
    let solo = one(&strike(&mut t, Seat(0)));
    let a = t.knock(301, Policy::bot(Model::Reader));
    let b = t.knock(302, Policy::bot(Model::Rusher));
    t.seated(a);
    t.seated(b);
    t.run(300);
    assert_eq!(t.host.sim().state().party_size(), 3);
    for mut p in t.peers.drain(..) {
        p.g.leave();
    }
    t.run(30);
    assert_eq!(t.host.sim().state().party_size(), 1);
    assert!(t.notes.iter().filter(|n| matches!(n, Note::Left { .. })).count() == 2);
    assert_eq!(one(&strike(&mut t, Seat(0))), solo, "no penalty left behind");
    // The host steps on alone, needing nobody's input.
    let frame = t.host.sim().state().frame;
    t.run(10);
    assert_eq!(t.host.sim().state().frame, frame + 10, "the table steps alone");
}

/// Walk to the nearest bed or fire and rest there, answering what it says.
fn rester() -> Policy {
    let mut cx = Ctx::new(Model::Reader);
    let mut task: Option<Task> = None;
    Policy::Script(Box::new(move |v, events| {
        cx.observe(v, events);
        if let Some(a) = jane_bot::talk::answer(v, &mut cx) {
            return a;
        }
        let cat = jane_data::catalog();
        if task.is_none() {
            let at = v.body().pos;
            let centre = |p: &jane_sim::Prop| jane_core::Vec2::centre(i32::from(p.cell.x), i32::from(p.cell.y));
            let Some(fire) = v
                .props()
                .filter(|p| cat.story.prop(p.def).rest)
                .min_by_key(|p| (jane_core::num::dist_sq(at, centre(p)), p.id))
            else {
                return Act::idle();
            };
            task = Some(Task::Use(UseProp::new(fire.id)));
        }
        match task.as_mut().map(|t| t.tick(v, &mut cx)) {
            Some(Status::Act(a)) => a,
            _ => {
                task = None;
                Act::idle()
            }
        }
    }))
}

#[test]
fn a_guests_rest_saves_the_hosts_world_and_the_next_arrives_at_her_fire() {
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    t.run(30);
    let first_rest = t.host.sim().state().rest.expect("the party's first fire");
    assert!(!t.host.take_rested());
    let a = t.knock(401, rester());
    let sa = t.seated(a);
    t.until(60 * 90, "she rests", |t| {
        t.host.events().iter().any(|e| e.kind == EventKind::Rest) || t.host.sim().state().rest != Some(first_rest)
    });
    t.run(5);
    assert!(t.host.take_rested(), "the host is asked to save for a guest's rest");
    // What the app and `jane serve` do then: the host writes its world.
    let dir = std::env::temp_dir().join(format!("jane-net-rest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slot.jsave");
    std::fs::write(&path, t.host.sim().save()).unwrap();
    let loaded = jane_sim::Sim::from_save(&std::fs::read(&path).unwrap()).expect("the save loads");
    let rest = loaded.state().rest.unwrap();
    assert_eq!(rest, t.host.sim().state().rest.unwrap(), "her fire is the world's");
    // A save is the host's world: she waits, parked, with her bags.
    assert!(!loaded.state().player(sa).unwrap().connected);
    assert!(loaded.state().player(sa).unwrap().parked.is_some());
    let _ = std::fs::remove_dir_all(&dir);
    // Whoever sits down next arrives at the party's last fire.
    let b = t.knock(402, Policy::Idle);
    let sb = t.seated(b);
    t.run(2);
    let p = t.host.sim().state().player(sb).unwrap();
    let body = t.host.sim().state().zone(p.zone).unwrap().unit(p.unit).unwrap();
    assert_eq!(p.zone, rest.zone);
    let (dx, dy) = ((body.pos.x.0 - rest.pos.x.0).abs(), (body.pos.y.0 - rest.pos.y.0).abs());
    assert!(dx <= 4 * 2048 && dy <= 4 * 2048, "arrived by the fire: {dx}, {dy}");
    t.same_hash();
}

#[test]
fn each_sees_only_her_own_numbers() {
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    let a = t.knock(501, Policy::Idle);
    let sa = t.seated(a);
    t.run(20);
    strike(&mut t, sa);
    let dmg = t.peers[a].heard.iter().find(|e| matches!(e.kind, EventKind::Damage { .. })).unwrap().kind;
    let hers = t.peers[a].g.sim().unwrap().view(sa).unwrap();
    assert!(hers.is_my_number(&dmg), "she took it: hers to see");
    let host = t.host.sim().view(Seat(0)).unwrap();
    assert!(!host.is_my_number(&dmg), "a friend's blow is sparks, not a number");
    // And the coat is the seat's on every machine.
    let body = t.host.sim().state().player(sa).unwrap().unit;
    assert_eq!(host.seat_of(body), Some(sa));
    assert_eq!(hers.seat_of(host.me().unit), Some(Seat(0)));
}
