//! Transports (ARCHITECTURE.md §7, decided 2026-09-27: LAN only for now, behind a trait so
//! another transport can be added without touching the lockstep logic). A [`Link`] carries
//! whole frames (one encoded message each) both ways and never blocks; a [`Listener`] hands the
//! host new links. The lockstep logic in [`crate::host`] and [`crate::guest`] sees only these.
//!
//! | Transport | |
//! | --- | --- |
//! | [`TcpLink`], [`TcpListen`] | the LAN: `TCP_NODELAY`, a `u32` little-endian length before each frame |
//! | [`MemLink`], [`mem_listener`] | in one process: tests and a session that talks to itself |
//! | [`Lossy`] | any link, with frames dropped, doubled and held back by a seeded roll (tests) |
//!
//! Internet play would be one more `Link` (UDP, or a relay both sides dial out to); the protocol
//! already tolerates loss and reordering (`wire.rs`).

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use jane_core::Sfc32;

/// A frame larger than this is refused (a snapshot is kilobytes; a busy county ~150 KB).
pub const MAX_FRAME: usize = 64 << 20;

#[derive(Debug)]
pub enum LinkError {
    /// The other end went away.
    Closed,
    Io(io::Error),
    /// A frame over [`MAX_FRAME`]: not a Jane peer, or a broken one.
    TooBig(usize),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::Closed => write!(f, "the connection closed"),
            LinkError::Io(e) => write!(f, "{e}"),
            LinkError::TooBig(n) => write!(f, "a frame of {n} bytes"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<io::Error> for LinkError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::NotConnected => LinkError::Closed,
            _ => LinkError::Io(e),
        }
    }
}

/// Frames both ways, never blocking.
pub trait Link: std::fmt::Debug + Send {
    /// Queue a frame. What cannot go now goes on the next [`flush`](Link::flush).
    fn send(&mut self, frame: Vec<u8>) -> Result<(), LinkError>;
    /// The next whole frame, if one has come.
    fn recv(&mut self) -> Result<Option<Vec<u8>>, LinkError>;
    /// Push what is queued as far as the transport takes it now.
    fn flush(&mut self) -> Result<(), LinkError> {
        Ok(())
    }
    /// Who is at the other end, for a log line.
    fn peer(&self) -> String;
    /// Hang up. Later calls may fail with [`LinkError::Closed`].
    fn close(&mut self);
}

/// New links, never blocking.
pub trait Listener: std::fmt::Debug + Send {
    fn accept(&mut self) -> Result<Option<Box<dyn Link>>, LinkError>;
}

// --- TCP -------------------------------------------------------------------------------------

/// A TCP stream carrying length-prefixed frames.
#[derive(Debug)]
pub struct TcpLink {
    s: TcpStream,
    peer: String,
    rbuf: Vec<u8>,
    /// Bytes of `rbuf` already taken as frames.
    rpos: usize,
    wbuf: VecDeque<u8>,
    closed: bool,
}

impl TcpLink {
    /// Dial `addr` (`host:port`, or a host alone for [`DEFAULT_PORT`](crate::wire::DEFAULT_PORT)),
    /// giving up after `timeout`.
    pub fn connect(addr: &str, timeout: Duration) -> Result<TcpLink, LinkError> {
        let with_port = if addr.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) && !addr.ends_with(']')
        {
            addr.to_owned()
        } else {
            format!("{addr}:{}", crate::wire::DEFAULT_PORT)
        };
        let mut last = None;
        for a in with_port.to_socket_addrs()? {
            match TcpStream::connect_timeout(&a, timeout) {
                Ok(s) => return TcpLink::new(s),
                Err(e) => last = Some(e),
            }
        }
        Err(last.map_or_else(|| LinkError::Io(io::Error::other(format!("no address for {addr}"))), LinkError::Io))
    }

    /// A connected stream, made non-blocking with Nagle off.
    pub fn new(s: TcpStream) -> Result<TcpLink, LinkError> {
        s.set_nodelay(true)?;
        s.set_nonblocking(true)?;
        let peer = s.peer_addr().map_or_else(|_| "?".to_owned(), |a| a.to_string());
        Ok(TcpLink { s, peer, rbuf: Vec::with_capacity(4096), rpos: 0, wbuf: VecDeque::new(), closed: false })
    }

    fn take_frame(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        let avail = &self.rbuf[self.rpos..];
        if avail.len() < 4 {
            return Ok(None);
        }
        let n = u32::from_le_bytes([avail[0], avail[1], avail[2], avail[3]]) as usize;
        if n > MAX_FRAME {
            return Err(LinkError::TooBig(n));
        }
        if avail.len() < 4 + n {
            return Ok(None);
        }
        let frame = avail[4..4 + n].to_vec();
        self.rpos += 4 + n;
        if self.rpos == self.rbuf.len() {
            self.rbuf.clear();
            self.rpos = 0;
        } else if self.rpos > 1 << 16 {
            self.rbuf.drain(..self.rpos);
            self.rpos = 0;
        }
        Ok(Some(frame))
    }
}

impl Link for TcpLink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), LinkError> {
        if self.closed {
            return Err(LinkError::Closed);
        }
        if frame.len() > MAX_FRAME {
            return Err(LinkError::TooBig(frame.len()));
        }
        self.wbuf.extend((frame.len() as u32).to_le_bytes());
        self.wbuf.extend(frame);
        self.flush()
    }

    fn recv(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        if let Some(f) = self.take_frame()? {
            return Ok(Some(f));
        }
        if self.closed {
            return Err(LinkError::Closed);
        }
        let mut tmp = [0u8; 16 * 1024];
        loop {
            match self.s.read(&mut tmp) {
                Ok(0) => {
                    self.closed = true;
                    // Frames already whole are still delivered.
                    return match self.take_frame()? {
                        Some(f) => Ok(Some(f)),
                        None => Err(LinkError::Closed),
                    };
                }
                Ok(n) => {
                    self.rbuf.extend_from_slice(&tmp[..n]);
                    if n < tmp.len() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.closed = true;
                    return Err(e.into());
                }
            }
        }
        self.take_frame()
    }

    fn flush(&mut self) -> Result<(), LinkError> {
        while !self.wbuf.is_empty() {
            let (a, _) = self.wbuf.as_slices();
            match self.s.write(a) {
                Ok(0) => {
                    self.closed = true;
                    return Err(LinkError::Closed);
                }
                Ok(n) => {
                    self.wbuf.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.closed = true;
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    fn peer(&self) -> String {
        self.peer.clone()
    }

    fn close(&mut self) {
        // Say what was queued (a Bye) before hanging up, as far as the socket takes it now.
        let _ = self.flush();
        let _ = self.s.shutdown(Shutdown::Both);
        self.closed = true;
    }
}

/// A TCP listener handing out [`TcpLink`]s.
#[derive(Debug)]
pub struct TcpListen {
    l: TcpListener,
}

impl TcpListen {
    /// Listen on every interface at `port` (0: any free port; see [`port`](Self::port)).
    pub fn bind(port: u16) -> Result<TcpListen, LinkError> {
        let l = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
        l.set_nonblocking(true)?;
        Ok(TcpListen { l })
    }

    pub fn port(&self) -> u16 {
        self.l.local_addr().map_or(0, |a| a.port())
    }
}

impl Listener for TcpListen {
    fn accept(&mut self) -> Result<Option<Box<dyn Link>>, LinkError> {
        match self.l.accept() {
            Ok((s, _)) => Ok(Some(Box::new(TcpLink::new(s)?))),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

// --- in memory -------------------------------------------------------------------------------

/// One end of an in-process pair. Dropping or closing it closes the pair.
#[derive(Debug)]
pub struct MemLink {
    tx: Option<Sender<Vec<u8>>>,
    rx: Receiver<Vec<u8>>,
    name: String,
}

/// Two joined ends.
pub fn mem_pair(a: &str, b: &str) -> (MemLink, MemLink) {
    let (ta, rb) = channel();
    let (tb, ra) = channel();
    (MemLink { tx: Some(ta), rx: ra, name: b.to_owned() }, MemLink { tx: Some(tb), rx: rb, name: a.to_owned() })
}

impl Link for MemLink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), LinkError> {
        self.tx.as_ref().ok_or(LinkError::Closed)?.send(frame).map_err(|_| LinkError::Closed)
    }

    fn recv(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        match self.rx.try_recv() {
            Ok(f) => Ok(Some(f)),
            Err(TryRecvError::Empty) if self.tx.is_some() => Ok(None),
            Err(_) => Err(LinkError::Closed),
        }
    }

    fn peer(&self) -> String {
        self.name.clone()
    }

    fn close(&mut self) {
        self.tx = None;
    }
}

/// The host's side of an in-process network.
#[derive(Debug)]
pub struct MemListener {
    rx: Receiver<MemLink>,
}

/// Dials a [`MemListener`].
#[derive(Clone, Debug)]
pub struct MemDialer {
    tx: Sender<MemLink>,
    n: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl MemDialer {
    /// A new link to the listener (the host sees it on its next accept).
    pub fn dial(&self) -> MemLink {
        let n = self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (host_end, guest_end) = mem_pair("host", &format!("mem-{n}"));
        let _ = self.tx.send(host_end);
        guest_end
    }
}

pub fn mem_listener() -> (MemListener, MemDialer) {
    let (tx, rx) = channel();
    (MemListener { rx }, MemDialer { tx, n: std::sync::Arc::default() })
}

impl Listener for MemListener {
    fn accept(&mut self) -> Result<Option<Box<dyn Link>>, LinkError> {
        match self.rx.try_recv() {
            Ok(l) => Ok(Some(Box::new(l))),
            Err(_) => Ok(None),
        }
    }
}

// --- a bad network ---------------------------------------------------------------------------

/// How bad: chances per mille per frame sent.
#[derive(Clone, Copy, Debug)]
pub struct Loss {
    pub drop: u32,
    pub dup: u32,
    /// Held back and sent after one to four later frames (overtaken).
    pub hold: u32,
    pub seed: u32,
}

/// A link that loses, doubles and reorders what it sends (tests: the protocol must not care).
#[derive(Debug)]
pub struct Lossy<L: Link> {
    inner: L,
    loss: Loss,
    rng: Sfc32,
    held: Vec<(u32, Vec<u8>)>,
    pub dropped: u32,
}

impl<L: Link> Lossy<L> {
    pub fn new(inner: L, loss: Loss) -> Lossy<L> {
        Lossy { inner, loss, rng: Sfc32::seeded(loss.seed, 9), held: Vec::new(), dropped: 0 }
    }

    fn roll(&mut self, per_mille: u32) -> bool {
        per_mille > 0 && self.rng.below(1000) < per_mille
    }
}

impl<L: Link> Link for Lossy<L> {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), LinkError> {
        // What was held back goes once its count runs out.
        for h in &mut self.held {
            h.0 = h.0.saturating_sub(1);
        }
        while let Some(i) = self.held.iter().position(|h| h.0 == 0) {
            let (_, f) = self.held.remove(i);
            self.inner.send(f)?;
        }
        if self.roll(self.loss.drop) {
            self.dropped += 1;
            return Ok(());
        }
        if self.roll(self.loss.hold) {
            let n = 1 + self.rng.below(4);
            self.held.push((n, frame));
            return Ok(());
        }
        if self.roll(self.loss.dup) {
            self.inner.send(frame.clone())?;
        }
        self.inner.send(frame)
    }

    fn recv(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        self.inner.recv()
    }

    fn flush(&mut self) -> Result<(), LinkError> {
        self.inner.flush()
    }

    fn peer(&self) -> String {
        self.inner.peer()
    }

    fn close(&mut self) {
        self.inner.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_frames_arrive_whole_and_in_order_and_a_close_is_seen() {
        let mut l = TcpListen::bind(0).unwrap();
        let port = l.port();
        let mut c = TcpLink::connect(&format!("127.0.0.1:{port}"), Duration::from_secs(2)).unwrap();
        let mut s = loop {
            if let Some(s) = l.accept().unwrap() {
                break s;
            }
            std::thread::yield_now();
        };
        let big = vec![7u8; 300_000];
        c.send(b"one".to_vec()).unwrap();
        c.send(big.clone()).unwrap();
        c.send(Vec::new()).unwrap();
        let mut got = Vec::new();
        for _ in 0..200_000 {
            c.flush().unwrap();
            if let Some(f) = s.recv().unwrap() {
                got.push(f);
                if got.len() == 3 {
                    break;
                }
            }
        }
        assert_eq!(got, vec![b"one".to_vec(), big, Vec::new()]);
        c.close();
        let mut closed = false;
        for _ in 0..200_000 {
            match s.recv() {
                Err(LinkError::Closed) => {
                    closed = true;
                    break;
                }
                Ok(None) => std::thread::yield_now(),
                other => panic!("{other:?}"),
            }
        }
        assert!(closed);
    }

    #[test]
    fn a_lossy_link_loses_some_and_delivers_the_rest() {
        let (a, mut b) = mem_pair("a", "b");
        let mut a = Lossy::new(a, Loss { drop: 100, dup: 50, hold: 100, seed: 3 });
        for i in 0..1000u32 {
            a.send(i.to_le_bytes().to_vec()).unwrap();
        }
        let mut got = Vec::new();
        while let Ok(Some(f)) = b.recv() {
            got.push(u32::from_le_bytes([f[0], f[1], f[2], f[3]]));
        }
        assert!(a.dropped > 50, "{}", a.dropped);
        assert!(got.len() > 800 && got.len() < 1000, "{}", got.len());
        assert!(got.windows(2).any(|w| w[1] < w[0]), "something was overtaken");
        drop(a);
        assert!(matches!(b.recv(), Err(LinkError::Closed)));
    }
}
