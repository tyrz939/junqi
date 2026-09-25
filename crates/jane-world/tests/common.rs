//! Shared test helpers.

/// Seeds a sweep runs: `SEEDS` (PORT.md §9.1: 64 by default, more in CI and the soak).
#[allow(clippy::disallowed_methods, dead_code)]
pub fn seeds() -> u32 {
    std::env::var("SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(64)
}
