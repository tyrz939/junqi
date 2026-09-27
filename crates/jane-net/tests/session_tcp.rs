//! The session the app plays through, over real TCP on this machine: a player's game opened to
//! the LAN, a second game joining it by address, both stepping one world; and pause, which
//! holds the world only alone.

mod common;

use common::bps;
use jane_net::{GuestConfig, HostConfig, Session};
use jane_sim::input::{Command, InputFrame};
use jane_sim::{ClientToken, Seat, Sim};

#[test]
fn alone_a_pause_holds_the_world_and_hosting_it_does_not() {
    let mut s = Session::local(Sim::new_game_with(bps(), "Jane"));
    let mut presses = vec![];
    assert!(s.pauses());
    assert!(s.try_step(0, InputFrame::IDLE, &mut presses, true).is_none());
    assert_eq!(s.sim().unwrap().state().frame, 0);
    s.try_step(16, InputFrame::IDLE, &mut presses, false).unwrap();
    assert_eq!(s.sim().unwrap().state().frame, 1);

    let mut s = match s.open_to_lan(HostConfig::default(), 0) {
        Ok(s) => s,
        Err((_, e)) => panic!("{e}"),
    };
    assert!(!s.pauses());
    assert!(s.port().is_some_and(|p| p != 0));
    for i in 0..10 {
        s.poll(32 + i * 16);
        s.try_step(32 + i * 16, InputFrame::IDLE, &mut presses, true).expect("a host steps alone, paused or not");
    }
    assert_eq!(s.sim().unwrap().state().frame, 11);
    assert!(s.sim().unwrap().state().open, "the host opened the world");
}

#[test]
fn a_game_opened_to_the_lan_is_joined_over_tcp_and_both_step_one_world() {
    let mut host = Session::host(Sim::new_game_with(bps(), "Jane"), HostConfig::default(), 0).unwrap();
    let port = host.port().unwrap();
    let mut guest =
        Session::join(&format!("127.0.0.1:{port}"), GuestConfig::new(ClientToken(9)), Some(bps()), 0).unwrap();
    let (mut hp, mut gp): (Vec<Command>, Vec<Command>) = (vec![], vec![]);
    let walk = InputFrame::walk(jane_core::Angle(16384));
    let mut now = 0;
    let mut seated_at = None;
    for tick in 0..6000u32 {
        now = u64::from(tick) * 1000 / 60;
        host.poll(now);
        guest.poll(now);
        host.try_step(now, InputFrame::walk(jane_core::Angle(0)), &mut hp, false);
        host.poll(now);
        guest.poll(now);
        while guest.backlog() > 0 {
            if guest.try_step(now, walk, &mut gp, false).is_none() {
                break;
            }
        }
        if seated_at.is_none() && guest.seat().is_some() {
            seated_at = Some(tick);
        }
        if seated_at.is_some_and(|s| tick > s + 600) {
            break;
        }
        // Real sockets: give the bytes a moment.
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    assert_eq!(guest.seat(), Some(Seat(1)), "{:?}", guest.status());
    assert_eq!(host.sim().unwrap().state().party_size(), 2);
    // Catch the guest up to the host's frame, then one world.
    for _ in 0..200 {
        if guest.sim().unwrap().state().frame == host.sim().unwrap().state().frame {
            break;
        }
        guest.poll(now);
        guest.try_step(now, walk, &mut gp, false);
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    let (h, g) = (host.sim().unwrap(), guest.sim().unwrap());
    assert_eq!(h.state().frame, g.state().frame);
    assert_eq!(h.hash(), g.hash());
    if let Session::Host(hh) = &host {
        assert!(hh.checks().ok >= 5, "{:?}", hh.checks());
        assert_eq!(hh.checks().bad, 0);
    }
    // Both bodies moved: the host's east, the guest's south, each by her own seat's input.
    let body = |sim: &Sim, s: Seat| {
        let p = sim.state().player(s).unwrap();
        sim.state().zone(p.zone).unwrap().unit(p.unit).unwrap().pos
    };
    assert_ne!(body(h, Seat(0)), body(g, Seat(1)));
    guest.close();
    for i in 0..200 {
        host.poll(now + i * 16);
        host.try_step(now + i * 16, InputFrame::IDLE, &mut hp, false);
        std::thread::sleep(std::time::Duration::from_micros(200));
        if host.sim().unwrap().state().party_size() == 1 {
            break;
        }
    }
    assert_eq!(host.sim().unwrap().state().party_size(), 1, "she got up when she left");
}
