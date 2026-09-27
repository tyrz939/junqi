//! Jane's sound, all of it made by code (PRESENTATION.md §5; DESIGN-2020.md §6: no audio file
//! ships). Soft FM, additive tables, filtered noise and plucked strings under gentle envelopes,
//! in one small shared room, through a limiter.
//!
//! - [`patch`]: the sound effects of `data/audio/sfx.json`, rendered into buffers at boot;
//! - [`bed`]: the ambient loops, made live (rain, wind, birds, crickets, the lake, the hum);
//! - [`seq`]: the music, a sequencer on the game's tick over `data/audio/songs`;
//! - [`engine`]: the mixer the app's audio callback runs;
//! - [`analysis`] and [`wav`]: how it is checked and heard without ears.
//!
//! Floats throughout. Nothing here is read by the sim, and nothing here reads it: the cue table
//! that turns the game into sound is `jane-present::audio`.

// Sample counts and indices become time and phase on every line of a synth; an f32 holds every
// count this crate makes (at most minutes of samples) closely enough for sound.
#![allow(clippy::cast_precision_loss)]

pub mod analysis;
pub mod bed;
pub mod dsp;
pub mod engine;
pub mod model;
pub mod patch;
pub mod pattern;
pub mod seq;
pub mod voice;
pub mod wav;

pub use bed::Bed;
pub use engine::{CEILING, Cmd, Engine};
pub use model::Library;

mod data {
    include!(concat!(env!("OUT_DIR"), "/audio_data.rs"));
}

/// The sample rate the game asks the device for.
pub const RATE: u32 = 48_000;

/// Everything under `data/audio`, parsed once (build.rs has already checked it).
pub fn library() -> &'static Library {
    static LIB: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
    LIB.get_or_init(|| Library {
        sfx: serde_json::from_str(data::SFX_JSON).expect("sfx.json was checked at build"),
        instruments: serde_json::from_str(data::INSTRUMENTS_JSON).expect("instruments.json was checked at build"),
        songs: data::SONGS_JSON.iter().map(|s| serde_json::from_str(s).expect("songs were checked at build")).collect(),
    })
}
