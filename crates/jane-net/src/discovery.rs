//! LAN discovery (ARCHITECTURE.md §7): a UDP broadcast of `JANE?` on the discovery port; every
//! host there answers `JANE!` and an [`Offer`] (its name, the TCP port to dial, seats, frame,
//! content hash, protocol and build), so the Join screen can list what is on the network and
//! grey out what this build cannot join. Joining by address needs none of it.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use serde::{Deserialize, Serialize};

const ASK: &[u8] = b"JANE?";
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
        let sock = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))?;
        sock.set_nonblocking(true)?;
        Ok(Beacon { sock })
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
    port: u16,
    found: Vec<Found>,
}

impl Finder {
    /// Ask on discovery port `port` (the hosts' UDP port).
    pub fn new(port: u16) -> io::Result<Finder> {
        let sock = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
        sock.set_broadcast(true)?;
        sock.set_nonblocking(true)?;
        Ok(Finder { sock, port, found: Vec::new() })
    }

    /// Ask the whole LAN, and this machine (a host on the same machine as the joiner).
    pub fn ask(&self) {
        let _ = self.sock.send_to(ASK, SocketAddrV4::new(Ipv4Addr::BROADCAST, self.port));
        let _ = self.sock.send_to(ASK, SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port));
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
        let mut beacon = Beacon::bind(0).unwrap();
        let mut finder = Finder::new(beacon.port()).unwrap();
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
        // Once by the broadcast and once by loopback, maybe, from two of this machine's addresses.
        assert!(!found.is_empty());
        for f in &found {
            assert_eq!(f.offer, offer);
            assert_eq!(f.addr.port(), 7777);
            assert!(f.offer.joinable());
        }
    }
}
