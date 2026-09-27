//! The wire (ARCHITECTURE.md §7): every message is one postcard-encoded [`Msg`] in one frame of
//! the [`Link`](crate::link::Link) it travels on.
//!
//! **Versioning.** [`PROTO`] is bumped by any change to a message. [`Msg::Hello`] and
//! [`Msg::Refuse`] are variants 0 and 1 and their layouts never change, so a peer of any version
//! can say who it is and be told why not; everything after them may change with `PROTO`.
//!
//! **Loss.** The lockstep stream needs no reliability under it: a guest's [`Msg::Inputs`] carries
//! every input the host has not yet bundled, and the host resends the bundles a guest has not
//! acknowledged, so a message lost, doubled or overtaken costs a resend and nothing else. Over
//! TCP nothing is lost; the rule is what lets another transport (UDP, a relay) carry the same
//! protocol later.

use jane_sim::input::{Command, InputFrame, StampedCommand};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{ClientToken, Seat};
use serde::{Deserialize, Serialize};

/// The protocol. Bumped by any change to a message after [`Msg::Refuse`].
pub const PROTO: u16 = 1;
/// This build: the package version and the commit (`build.rs`). Peers of different builds are
/// refused, as peers with different content are: lockstep needs the same code and the same data.
pub const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("JANE_BUILD_ID"));
/// The TCP port a host listens on and the UDP port it answers discovery on.
pub const DEFAULT_PORT: u16 = 7777;
/// Frames of input delay (§7: 50 ms, below perception under a 90-tick GCD); a host may choose
/// 2..=6.
pub const DEFAULT_DELAY: u8 = 3;
pub const MIN_DELAY: u8 = 2;
pub const MAX_DELAY: u8 = 6;
/// Every peer hashes its state after the step that brings the frame to a multiple of this.
pub const HASH_EVERY: u32 = 60;

/// This build's content: the compiled data's hash.
pub fn content_hash() -> u64 {
    jane_data::catalog().content_hash
}

/// Who knocks. Variant 0; the layout is frozen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub proto: u16,
    pub content_hash: u64,
    pub build: String,
    /// The client's token (kept by the client, never shown): a returning guest gets her own body
    /// and bags back by it.
    pub token: u64,
}

impl Hello {
    /// This build's hello for `token`.
    pub fn ours(token: ClientToken) -> Hello {
        Hello { proto: PROTO, content_hash: content_hash(), build: BUILD.to_owned(), token: token.0 }
    }
}

/// Why a host said no.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Why {
    /// Another protocol.
    Proto,
    /// Other content: the two content hashes differ.
    Content,
    /// Another build of the game.
    Build,
    /// Every seat the host offers is taken.
    Full,
    /// The host has closed the world to newcomers.
    Closed,
}

/// A refusal, with the host's side of what was compared (a refused join shows both hashes, §7).
/// Variant 1; the layout is frozen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub why: Why,
    pub proto: u16,
    pub content_hash: u64,
    pub build: String,
}

impl Refusal {
    /// The refusal as a joiner reads it, with both sides of what was compared: the host's, and
    /// what `ours` said at the door.
    pub fn explain(&self, ours: &Hello) -> String {
        match self.why {
            Why::Proto => format!("the host speaks protocol {}, this build {}", self.proto, ours.proto),
            Why::Content => format!(
                "the host's content is {:016x}, this build's is {:016x}: both must run the same data",
                self.content_hash, ours.content_hash
            ),
            Why::Build => {
                format!("the host is build {}, this is build {}: both must run the same build", self.build, ours.build)
            }
            Why::Full => "every seat at the host's table is taken".to_owned(),
            Why::Closed => "the host's world is closed to newcomers".to_owned(),
        }
    }
}

impl std::fmt::Display for Refusal {
    /// Against this build's own hello.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.explain(&Hello::ours(ClientToken(0))))
    }
}

/// One seat's input for one frame: the held frame and what was pressed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub frame: InputFrame,
    pub cmds: Vec<Command>,
}

/// One frame for the whole table, as every peer steps it: a replay frame (§7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    pub frame: u32,
    pub frames: [InputFrame; MAX_PLAYERS],
    /// Sorted `(seat, seq)`: the host's sequencing.
    pub cmds: Vec<StampedCommand>,
}

/// Where two peers parted (§7 `Desync`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// The hash point that differed (the frame count after the step).
    pub frame: u32,
    pub seat: Seat,
    pub host: u64,
    pub guest: u64,
    /// The first step whose result differed, found by both re-simulating from the hash point
    /// before from their own saves; `None` when neither re-simulation differs (the state was
    /// changed outside a step, or the guest had no save from before).
    pub first_step: Option<u32>,
    /// Which parts of the state differ at `frame` (`jane_sim::replay::diff_states`).
    pub parts: Vec<String>,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "desync at frame {}: seat {} hashed {:016x}, the host {:016x}",
            self.frame, self.seat.0, self.guest, self.host
        )?;
        match self.first_step {
            Some(s) => write!(f, "; the first step to differ was frame {s}")?,
            None => write!(f, "; no step differs when re-simulated")?,
        }
        if !self.parts.is_empty() {
            write!(f, "; differs in: {}", self.parts.join(", "))?;
        }
        Ok(())
    }
}

/// Why a peer went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByeWhy {
    /// She got up.
    Left,
    /// Her input did not come for the stall limit (§12: 10 s) and the host dropped her seat.
    Dropped,
    /// The host is closing the session.
    HostClosed,
    /// A newer connection with the same token took her seat.
    Replaced,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Msg {
    /// Guest → host. Frozen layout (variant 0).
    Hello(Hello),
    /// Host → guest. Frozen layout (variant 1).
    Refuse(Refusal),
    /// Host → guest: build this seed's county, then say [`Msg::Ready`].
    Accept { seed: u32, name: String, delay: u8 },
    /// Guest → host: the county is built; send the world. Repeated until a welcome comes.
    Ready,
    /// Host → guest: the world at a step boundary. `epoch` counts this connection's welcomes; a
    /// guest takes one newer than what it has (a resync is a welcome without a join). Inputs
    /// are wanted from `need_from` on.
    Welcome { epoch: u32, seat: Seat, frame: u32, delay: u8, need_from: u32, snapshot: Vec<u8> },
    /// Guest → host: every input the host has not bundled yet, from frame `first`, and the next
    /// bundle this guest lacks (`ack`).
    Inputs { epoch: u32, ack: u32, first: u32, items: Vec<Item> },
    /// Host → guests: a frame to step.
    Bundle(Bundle),
    /// Guest → host: the state hash after the step that brought the frame to `frame`.
    Hash { frame: u32, hash: u64 },
    /// Host → guests: the table waits for these seats (a bit each) at `frame`; `seats == 0` says
    /// the wait is over. `wait` is the host's toggle: nobody is dropped.
    Stall { frame: u32, seats: u8, waited_ms: u32, wait: bool },
    /// Host → guest: your hash at `frame` was not mine; send what you had.
    DesyncFound { frame: u32 },
    /// Guest → host: the save at `frame`, and the hash after every step since the hash point
    /// before it, re-simulated from its own save (empty when it had none).
    DesyncDump { frame: u32, trail_from: u32, trail: Vec<u64>, save: Vec<u8> },
    /// Host → guests: where it parted.
    DesyncReport(Report),
    /// Either way: going.
    Bye(ByeWhy),
    /// Host → guest while nothing else is said (a guest counts the host gone after 10 s of
    /// silence).
    Beat,
}

#[derive(Debug)]
pub enum WireError {
    Decode(postcard::Error),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::Decode(e) => write!(f, "a message that does not decode: {e}"),
        }
    }
}

impl std::error::Error for WireError {}

pub fn encode(m: &Msg) -> Vec<u8> {
    postcard::to_allocvec(m).expect("a message encodes")
}

pub fn decode(b: &[u8]) -> Result<Msg, WireError> {
    postcard::from_bytes(b).map_err(WireError::Decode)
}

#[cfg(test)]
mod tests {
    use jane_core::Angle;

    use super::*;

    #[test]
    fn messages_round_trip_and_a_bundle_is_small() {
        let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
        frames[1] = InputFrame::walk(Angle(9000));
        let b = Bundle {
            frame: 1234,
            frames,
            cmds: vec![StampedCommand { seat: Some(Seat(1)), seq: 0, cmd: Command::Use }],
        };
        for m in [
            Msg::Hello(Hello::ours(ClientToken(77))),
            Msg::Refuse(Refusal { why: Why::Content, proto: PROTO, content_hash: 5, build: BUILD.into() }),
            Msg::Bundle(b.clone()),
            Msg::Inputs {
                epoch: 1,
                ack: 3,
                first: 4,
                items: vec![Item::default(), Item { frame: frames[1], cmds: vec![Command::Use] }],
            },
            Msg::Stall { frame: 9, seats: 0b100, waited_ms: 600, wait: false },
            Msg::Bye(ByeWhy::Dropped),
        ] {
            assert_eq!(decode(&encode(&m)).unwrap(), m);
        }
        let n = encode(&Msg::Bundle(b)).len();
        assert!(n < 48, "a bundle of four seats and a press is {n} bytes");
    }

    #[test]
    fn hello_and_refuse_are_variants_zero_and_one() {
        assert_eq!(encode(&Msg::Hello(Hello::ours(ClientToken(1))))[0], 0);
        assert_eq!(
            encode(&Msg::Refuse(Refusal { why: Why::Full, proto: 1, content_hash: 0, build: String::new() }))[0],
            1
        );
    }
}
