//! Lockstep over std::net (ARCHITECTURE.md §7; PORT.md P8).
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4). No `unsafe`, no
//! async runtime: non-blocking `std::net` sockets polled from the caller's loop, and wall time
//! passed in as milliseconds (`now`), never read here, so a test drives the clock.
//!
//! LAN only for now, player-hosted first (decided 2026-09-27): one player's game hosts (`jane-app
//! --host`, playing seat 0) and the others join it; `jane serve` is the same host playing nobody.
//! The transport is a trait ([`link::Link`]), so internet play, a relay or NAT traversal can be
//! added later without touching the lockstep logic.
//!
//! | Module | What |
//! | --- | --- |
//! | [`wire`] | the messages, the protocol version, the build and content checks |
//! | [`link`] | transports: TCP, in-process, and a lossy shim for tests |
//! | [`host`] | seats, pacing, bundles, stalls, hash checks, desync reports, welcomes |
//! | [`guest`] | hello, build, welcome, step what the host sends, send this seat's input |
//! | [`book`] | the recent past each peer keeps: bundles, hashes, saves; re-simulation |
//! | [`discovery`] | UDP broadcast `JANE?` / `JANE!` |
//! | [`session`] | what the app plays through: local, hosting or joined |

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod book;
pub mod discovery;
pub mod guest;
pub mod host;
pub mod link;
pub mod session;
pub mod wire;

pub use guest::{Guest, GuestConfig, Phase};
pub use host::{Checks, Host, HostConfig, Note, StallView};
pub use session::{Session, Status};
