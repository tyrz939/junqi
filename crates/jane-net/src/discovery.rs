//! LAN discovery (ARCHITECTURE.md §7): a UDP broadcast of `JANE?` on the discovery port; every
//! host there answers `JANE!` and an [`Offer`] (its name, the TCP port to dial, seats, frame,
//! content hash, protocol and build), so the Join screen can list what is on the network and
//! grey out what this build cannot join. Joining by address needs none of it.
//!
//! The discovery port is [`DISCOVERY_PORT`] whatever port a host plays on, so a host on 7800 is
//! found by a joiner who knows nothing of 7800: the offer says where to dial. Only one process on
//! a machine can listen there; a second host on the same machine answers on its own game port,
//! which a finder asks as well when told it ([`Finder::also`]).

use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use serde::{Deserialize, Serialize};

const ASK: &[u8] = b"JANE?";
/// Where every host listens for askers (UDP), whatever port it plays on.
pub const DISCOVERY_PORT: u16 = crate::wire::DEFAULT_PORT;
const ANSWER: &[u8] = b"JANE!";

/// What a host says of itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub name: String,
    /// The TCP port to dial.
    pub port: u16,
    pub seats_used: u8,
    pub seats: u8,
    pub frame: u32,
    pub content_hash: u64,
    pub proto: u16,
    pub build: String,
}

impl Offer {
    /// Whether this build can join it.
    pub fn joinable(&self) -> bool {
        self.proto == crate::wire::PROTO
            && self.content_hash == crate::wire::content_hash()
            && self.build == crate::wire::BUILD
            && self.seats_used < self.seats
    }
}

/// A host found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// Where to dial: the answering address with the offer's port.
    pub addr: SocketAddr,
    pub offer: Offer,
}

/// The host's side: answers askers. Polled by the host; never blocks.
#[derive(Debug)]
pub struct Beacon {
    sock: UdpSocket,
}

impl Beacon {
    /// Listen for askers on `port` (UDP) on every interface.
    pub fn bind(port: u16) -> io::Result<Beacon> {
        Self::bind_any(&[port])
    }

    /// Listen on the first of `ports` that is free (the discovery port, else the game's own).
    pub fn bind_any(ports: &[u16]) -> io::Result<Beacon> {
        Self::bind_any_on(Ipv4Addr::UNSPECIFIED, ports)
    }

    /// [`bind_any`](Self::bind_any) on `ip` only (`Ipv4Addr::LOCALHOST` in tests: no firewall
    /// prompt, and only this machine's finders hear it).
    pub fn bind_any_on(ip: Ipv4Addr, ports: &[u16]) -> io::Result<Beacon> {
        let mut last = io::Error::other("no port to listen on");
        for &port in ports {
            match UdpSocket::bind(SocketAddrV4::new(ip, port)) {
                Ok(sock) => {
                    sock.set_nonblocking(true)?;
                    return Ok(Beacon { sock });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn port(&self) -> u16 {
        self.sock.local_addr().map_or(0, |a| a.port())
    }

    /// Answer every asker waiting.
    pub fn answer(&mut self, offer: &Offer) {
        let mut buf = [0u8; 64];
        while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
            if &buf[..n] != ASK {
                continue;
            }
            let mut out = ANSWER.to_vec();
            if let Ok(body) = postcard::to_allocvec(offer) {
                out.extend_from_slice(&body);
                let _ = self.sock.send_to(&out, from);
            }
        }
    }
}

/// The joiner's side: asks, then collects answers as they come.
#[derive(Debug)]
pub struct Finder {
    sock: UdpSocket,
    ports: Vec<u16>,
    found: Vec<Found>,
}

impl Finder {
    /// Ask on discovery port `port` (the hosts' UDP port).
    pub fn new(port: u16) -> io::Result<Finder> {
        Self::new_on(Ipv4Addr::UNSPECIFIED, port)
    }

    /// [`new`](Self::new) from `ip` only (`Ipv4Addr::LOCALHOST` in tests: it finds hosts on
    /// this machine, and its broadcast goes nowhere).
    pub fn new_on(ip: Ipv4Addr, port: u16) -> io::Result<Finder> {
        let sock = UdpSocket::bind(SocketAddrV4::new(ip, 0))?;
        sock.set_broadcast(true)?;
        sock.set_nonblocking(true)?;
        Ok(Finder { sock, ports: vec![port], found: Vec::new() })
    }

    /// Ask on this port too (a second host on one machine answers on its game port).
    pub fn also(mut self, port: u16) -> Finder {
        if !self.ports.contains(&port) {
            self.ports.push(port);
        }
        self
    }

    /// Ask the whole LAN, and this machine (a host on the same machine as the joiner).
    pub fn ask(&self) {
        for &port in &self.ports {
            let _ = self.sock.send_to(ASK, SocketAddrV4::new(Ipv4Addr::BROADCAST, port));
            let _ = self.sock.send_to(ASK, SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        }
    }

    /// Every host that has answered so far, one row per address, newest answer kept.
    pub fn poll(&mut self) -> &[Found] {
        let mut buf = [0u8; 1024];
        while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
            let Some(body) = buf[..n].strip_prefix(ANSWER) else { continue };
            let Ok(offer) = postcard::from_bytes::<Offer>(body) else { continue };
            let addr = SocketAddr::new(from.ip(), offer.port);
            match self.found.iter_mut().find(|f| f.addr == addr) {
                Some(f) => f.offer = offer,
                None => self.found.push(Found { addr, offer }),
            }
        }
        &self.found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_on_this_machine_is_found() {
        let mut beacon = Beacon::bind_any_on(Ipv4Addr::LOCALHOST, &[0]).unwrap();
        assert!(beacon.sock.local_addr().unwrap().ip().is_loopback(), "a test listens on loopback only");
        let mut finder = Finder::new_on(Ipv4Addr::LOCALHOST, beacon.port()).unwrap();
        let offer = Offer {
            name: "Tess's".into(),
            port: 7777,
            seats_used: 1,
            seats: 4,
            frame: 99,
            content_hash: crate::wire::content_hash(),
            proto: crate::wire::PROTO,
            build: crate::wire::BUILD.into(),
        };
        let mut found = Vec::new();
        for _ in 0..2000 {
            finder.ask();
            std::thread::sleep(std::time::Duration::from_millis(1));
            beacon.answer(&offer);
            std::thread::sleep(std::time::Duration::from_millis(1));
            found = finder.poll().to_vec();
            if !found.is_empty() {
                break;
            }
        }
        assert!(!found.is_empty());
        for f in &found {
            assert_eq!(f.offer, offer);
            assert_eq!(f.addr.port(), 7777);
            assert!(f.offer.joinable());
        }
    }
}
