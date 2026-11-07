# jane-net

Owns lockstep over `std::net` for LAN co-op: wire format, transports, host, guest, history, discovery. Float-free. No `unsafe`, no async runtime: non-blocking sockets polled from the caller's loop.

**Map:** `src/`
- `wire.rs` messages, protocol version, build and content checks; `link.rs` the `Link` trait: TCP, in-process, lossy test shim
- `host.rs` seats, pacing, bundles, stalls, hash checks, desync reports; `guest.rs` hello, welcome, step, send input
- `book.rs` the recent past each peer keeps (bundles, hashes, saves, re-simulation); `record.rs` a hosted session as a `.jrp` tape
- `discovery.rs` UDP broadcast `JANE?` / `JANE!`; `session.rs` what the app plays through: local, hosting or joined

**Invariants:**
- Wall time is passed in as milliseconds (`now`), never read here, so a test drives the clock.
- The transport is a trait: a new transport must not touch the lockstep logic.
- The `test-hooks` feature is for tests only (the crate lists itself as a dev-dependency to turn it on).

**Test it:** `cargo test --release -p jane-net` (`loopback`, `coop_rules`, `session_tcp`; helpers in `tests/common/`; uses `jane-bot` as a dev-dependency).
Dependents: `jane-app`, `jane-cli`. The headless host has a slow test: `cargo test -p jane-cli --test serve -- --ignored` (three processes over TCP, about 15 s).

**Docs:** `ARCHITECTURE.md` §7; `PORT.md` P8 and §12; `PLATFORM.md` §1 to 2.
