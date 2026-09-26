//! Lockstep over loopback (PORT.md P8 gate, in one process): two to four peers played by bot
//! models hold one hash; a desync is found at its hash point and named; a stall is shown and a
//! silent seat dropped after 10 s (simulated); a guest who drops comes back to her own seat and
//! catches up; other content is refused showing both hashes; a lossy network changes nothing.

mod common;

use common::*;
use jane_bot::Model;
use jane_net::link::Loss;
use jane_net::wire::{HASH_EVERY, Why};
use jane_net::{GuestConfig, HostConfig, Note, Phase};
use jane_sim::{ClientToken, Seat};

#[test]
fn four_seats_hold_one_hash_for_thousands_of_ticks() {
    let mut t = Table::new(HostConfig::default(), Policy::bot(Model::Rusher));
    let a = t.knock(11, Policy::bot(Model::Reader));
    t.run(200);
    let b = t.knock(12, Policy::bot(Model::Rusher));
    t.run(700);
    let c = t.knock(13, Policy::bot(Model::Reader));
    let seats = [t.seated(a), t.seated(b), t.seated(c)];
    assert_eq!(seats, [Seat(1), Seat(2), Seat(3)]);
    // Five minutes of four at the table.
    t.run(5 * 60 * 60);
    assert_eq!(t.host.sim().state().party_size(), 4);
    let checks = t.host.checks();
    let frames = t.host.sim().state().frame;
    println!("{frames} frames, {} checks ok, {} bad", checks.ok, checks.bad);
    assert_eq!(checks.bad, 0, "{:?}", t.desyncs());
    assert!(checks.ok >= u64::from(3 * 5 * 60 * 60 / HASH_EVERY), "{checks:?}");
    t.same_hash();
    // Nobody stalled on a perfect network.
    assert!(!t.notes.iter().any(|n| matches!(n, Note::Dropped { .. })));
}

#[test]
fn a_desync_is_found_at_its_hash_point_and_named() {
    let mut t = Table::new(HostConfig::default(), Policy::bot(Model::Rusher));
    let a = t.knock(21, Policy::bot(Model::Reader));
    let seat = t.seated(a);
    t.run(200);
    // The guest's world is changed outside any step: her hp, on her machine only.
    t.until(400, "a frame just past a hash point", |t| t.peers[a].g.sim().unwrap().state().frame % HASH_EVERY == 7);
    let at = t.peers[a].g.sim().unwrap().state().frame;
    {
        let sim = t.peers[a].g.sim_mut().unwrap();
        let p = sim.state().player(seat).unwrap();
        let (z, u) = (p.zone, p.unit);
        sim.state_mut().zone_mut(z).unwrap().unit_mut(u).unwrap().hp.0 -= 1000;
    }
    t.until(300, "the desync is reported", |t| !t.desyncs().is_empty());
    let r = t.desyncs()[0].clone();
    println!("{r}");
    assert_eq!(r.frame, at - 7 + HASH_EVERY, "found at the first hash point after the change");
    assert_eq!(r.seat, seat);
    assert_ne!(r.host, r.guest);
    // Re-simulated from their saves both sides agree: nothing a step did.
    assert_eq!(r.first_step, None);
    assert!(r.parts.iter().any(|p| p.contains("county.units")), "{:?}", r.parts);
    // The guest is put back on the host's timeline, and holds it.
    assert!(t.notes.contains(&Note::Resynced { seat }));
    let bad = t.host.checks().bad;
    t.run(600);
    assert_eq!(t.host.checks().bad, bad, "no desync after the resync");
    t.same_hash();
}

#[test]
fn a_step_that_differs_is_named_by_its_frame() {
    let mut t = Table::new(HostConfig::default(), Policy::bot(Model::Rusher));
    let a = t.knock(22, Policy::Idle);
    let seat = t.seated(a);
    t.run(300);
    // A broken build: the guest steps one frame's bundle with the host's stick turned.
    let f = t.peers[a].g.sim().unwrap().state().frame + 20;
    t.peers[a].g.corrupt_at = Some(f);
    t.until(400, "the desync is reported", |t| !t.desyncs().is_empty());
    let r = t.desyncs()[0].clone();
    println!("{r}");
    assert_eq!(r.seat, seat);
    // Stepping frame f leaves the frame count at f + 1; the next hash point at or after it.
    assert_eq!(r.frame, (f + 1).div_ceil(HASH_EVERY) * HASH_EVERY);
    assert_eq!(r.first_step, Some(f), "the first step to differ is the one that was broken");
    assert!(r.parts.iter().any(|p| p.contains("county")), "{:?}", r.parts);
}

#[test]
fn a_stalled_seat_is_shown_then_dropped_after_ten_seconds() {
    let mut t = Table::new(HostConfig::default(), Policy::bot(Model::Rusher));
    let a = t.knock(31, Policy::bot(Model::Reader));
    let b = t.knock(32, Policy::Idle);
    t.seated(a);
    let sb = t.seated(b);
    t.run(300);
    t.same_hash();
    // Seat b's machine hangs.
    t.peers[b].frozen = true;
    let hung = t.host.sim().state().frame;
    t.run(60);
    let stall = t.host.stall().expect("the table waits");
    assert_eq!(stall.seats, 1 << sb.0);
    assert!(stall.waited_ms >= 500);
    assert!(t.host.sim().state().frame <= hung + 4, "nobody runs ahead of a stalled seat");
    // The other guest is told who is awaited.
    assert_eq!(t.peers[a].g.stall().map(|s| s.seats), Some(1 << sb.0));
    t.run(9 * 60);
    assert!(!t.notes.contains(&Note::Dropped { seat: sb }), "not before 10 s");
    t.run(90);
    assert!(t.notes.contains(&Note::Dropped { seat: sb }));
    assert_eq!(t.host.sim().state().party_size(), 2, "she got up");
    t.run(300);
    assert!(t.host.sim().state().frame > hung + 200, "the table goes on");
    assert!(t.peers[a].g.stall().is_none());
    assert_eq!(t.host.checks().bad, 0);
    t.peers.remove(b);
    t.same_hash();
}

#[test]
fn the_wait_toggle_waits_however_long() {
    let cfg = HostConfig { wait: true, ..HostConfig::default() };
    let mut t = Table::new(cfg, Policy::bot(Model::Rusher));
    let a = t.knock(33, Policy::Idle);
    let sa = t.seated(a);
    t.run(120);
    t.peers[a].frozen = true;
    t.run(20 * 60);
    assert!(t.host.stall().is_some_and(|s| s.wait && s.seats == 1 << sa.0));
    assert!(!t.notes.iter().any(|n| matches!(n, Note::Dropped { .. })));
    assert_eq!(t.host.sim().state().party_size(), 2);
    // She comes back; the table goes on and agrees.
    t.peers[a].frozen = false;
    t.run(300);
    assert!(t.host.stall().is_none());
    assert_eq!(t.host.checks().bad, 0);
    t.same_hash();
}

#[test]
fn a_guest_who_drops_comes_back_to_her_own_seat_and_catches_up() {
    let mut t = Table::new(HostConfig::default(), Policy::bot(Model::Rusher));
    let a = t.knock(41, Policy::bot(Model::Reader));
    let b = t.knock(42, Policy::bot(Model::Rusher));
    let sa = t.seated(a);
    t.seated(b);
    t.run(900);
    // Her machine drops off the network: the host sees the link close.
    t.peers.remove(a);
    t.run(30);
    assert!(t.notes.contains(&Note::Left { seat: sa }));
    assert!(!t.host.sim().state().player(sa).unwrap().connected);
    let bag = t.host.sim().state().player(sa).unwrap().bag.clone();
    t.run(600);
    // Back with the same token: her own seat, her own bags.
    let a = t.knock(41, Policy::bot(Model::Reader));
    assert_eq!(t.seated(a), sa);
    assert_eq!(t.host.sim().state().player(sa).unwrap().bag, bag);
    t.run(1200);
    assert_eq!(t.host.checks().bad, 0);
    assert_eq!(t.host.sim().state().party_size(), 3);
    t.same_hash();
}

#[test]
fn other_content_or_another_build_is_refused_showing_both() {
    let mut t = Table::new(HostConfig::default(), Policy::Idle);
    let mut cfg = GuestConfig::new(ClientToken(51));
    cfg.hello.content_hash ^= 0xdead;
    let a = t.knock_with(cfg.clone(), Policy::Idle);
    let mut cfg2 = GuestConfig::new(ClientToken(52));
    cfg2.hello.build = "0.0.0+elsewhere".into();
    let b = t.knock_with(cfg2, Policy::Idle);
    t.run(30);
    let Phase::Refused(r) = t.peers[a].g.phase().clone() else { panic!("{:?}", t.peers[a].g.phase()) };
    assert_eq!(r.why, Why::Content);
    assert_eq!(r.content_hash, jane_net::wire::content_hash());
    let said = t.peers[a].g.refusal().unwrap();
    assert!(said.contains(&format!("{:016x}", jane_net::wire::content_hash())), "{said}");
    assert!(said.contains(&format!("{:016x}", cfg.hello.content_hash)), "{said}");
    println!("{said}");
    let Phase::Refused(r) = t.peers[b].g.phase().clone() else { panic!("{:?}", t.peers[b].g.phase()) };
    assert_eq!(r.why, Why::Build);
    assert!(r.to_string().contains(jane_net::wire::BUILD));
    assert_eq!(t.host.sim().state().party_size(), 1, "nobody sat down");
    assert!(t.notes.iter().filter(|n| matches!(n, Note::Refused { .. })).count() == 2);
}

#[test]
fn a_full_table_refuses_the_next() {
    let cfg = HostConfig { seats: 2, ..HostConfig::default() };
    let mut t = Table::new(cfg, Policy::Idle);
    let a = t.knock(61, Policy::Idle);
    t.seated(a);
    let b = t.knock(62, Policy::Idle);
    t.run(60);
    assert!(matches!(t.peers[b].g.phase(), Phase::Refused(r) if r.why == Why::Full));
}

#[test]
fn a_lossy_network_changes_nothing() {
    let loss = Loss { drop: 50, dup: 30, hold: 60, seed: 7 };
    let mut t = Table::with_loss(HostConfig::default(), Policy::bot(Model::Rusher), Some(loss));
    let a = t.knock(71, Policy::bot(Model::Reader));
    let b = t.knock(72, Policy::bot(Model::Rusher));
    t.seated(a);
    t.seated(b);
    t.run(3000);
    let checks = t.host.checks();
    println!("lossy: {} frames, {checks:?}", t.host.sim().state().frame);
    assert_eq!(checks.bad, 0);
    assert!(checks.ok >= 40, "{checks:?}");
    assert!(t.host.sim().state().frame > 2000, "a lossy table still moves: {}", t.host.sim().state().frame);
    assert_eq!(t.host.sim().state().party_size(), 3);
    t.same_hash();
}
