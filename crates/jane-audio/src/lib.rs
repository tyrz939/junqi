//! Jane's sound, all of it made by code (PRESENTATION.md §5; DESIGN-2020.md §6: no audio file
//! ships). Soft FM, additive tables, filtered noise and plucked strings under gentle envelopes,
//! in one small shared room, through a limiter.
//!
//! - [`patch`]: the sound effects of `data/audio/sfx.json`, rendered into buffers at boot;
//! - [`bed`]: the ambient loops, made live (rain, wind, birds, crickets, the lake, the hum);
//! - [`seq`]: the music, a sequencer on the game's tick over `data/audio/songs`;
//! - [`engine`]: the mixer the app's audio callback runs;
//! - [`analysis`] and [`wav`]: how it is checked and heard without ears;
//! - [`bake`] and [`tracker`]: the consoles' form (PORT.md §13.4): the synth rendered once into a
//!   module of patterns and a shared sample bank, and the integer player and mixer that play it.
//!
//! Floats throughout the PC's path. Nothing here is read by the sim, and nothing here reads it:
//! the cue table that turns the game into sound is `jane-present::audio`. Without `std` (the
//! consoles) only [`tracker`] builds: `no_std` plus `alloc`, integers only.

#![cfg_attr(not(feature = "std"), no_std)]
// Sample counts and indices become time and phase on every line of a synth; an f32 holds every
// count this crate makes (at most minutes of samples) closely enough for sound.
#![allow(clippy::cast_precision_loss)]

extern crate alloc;

#[cfg(feature = "std")]
pub mod analysis;
#[cfg(feature = "std")]
pub mod bake;
#[cfg(feature = "std")]
pub mod bed;
#[cfg(feature = "std")]
pub mod dsp;
#[cfg(feature = "std")]
pub mod engine;
#[cfg(feature = "std")]
pub mod model;
#[cfg(feature = "std")]
pub mod patch;
#[cfg(feature = "std")]
pub mod pattern;
#[cfg(feature = "std")]
pub mod seq;
pub mod tracker;
#[cfg(feature = "std")]
pub mod voice;
#[cfg(feature = "std")]
pub mod wav;

#[cfg(feature = "std")]
pub use bed::Bed;
#[cfg(feature = "std")]
pub use engine::{CEILING, Cmd, Engine};
#[cfg(feature = "std")]
pub use model::Library;

#[cfg(feature = "std")]
mod data {
    include!(concat!(env!("OUT_DIR"), "/audio_data.rs"));
}

/// The sample rate the game asks the device for.
pub const RATE: u32 = 48_000;

/// The gated loudness every cue is matched to, dBFS (`analysis::loudness`): a player never
/// reaches for the volume when the music changes. The songs' `gain`s are set to it, and
/// `tests/music.rs` holds them there.
#[cfg(feature = "std")]
pub const LOUDNESS: f32 = -21.0;

/// Everything under `data/audio`, parsed once (build.rs has already checked it).
#[cfg(feature = "std")]
pub fn library() -> &'static Library {
    static LIB: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
    LIB.get_or_init(|| Library {
        sfx: serde_json::from_str(data::SFX_JSON).expect("sfx.json was checked at build"),
        instruments: serde_json::from_str(data::INSTRUMENTS_JSON).expect("instruments.json was checked at build"),
        songs: data::SONGS_JSON.iter().map(|s| serde_json::from_str(s).expect("songs were checked at build")).collect(),
    })
}
