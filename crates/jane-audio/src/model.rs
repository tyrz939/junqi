//! The shapes of `data/audio` (PRESENTATION.md §5): sound-effect patches, instruments and songs.
//! Shared with `build.rs`, which reads every file through these types and refuses the build on a
//! file that does not fit, so a typo in a pattern is a build error, not a wrong note at night.
//!
//! Every list here is short (a patch's layers, a bell's partials, a pattern's notes): the sound is
//! made by code from a few numbers, never read from a table of samples.

use std::collections::BTreeMap;

use serde::Deserialize;

/// What a layer or an instrument's oscillator is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Wave {
    #[default]
    Sine,
    /// Band-limited triangle (soft, hollow).
    Tri,
    /// Band-limited saw (PolyBLEP), always under a filter.
    Saw,
    /// Band-limited pulse; `duty` is its width.
    Square,
    /// White noise.
    Noise,
    /// Pink noise (-3 dB an octave): wind, surf, breath.
    Pink,
    /// Two-operator FM; `ratio` the modulator's, `duty` its index.
    Fm,
    /// Karplus-Strong: a plucked string; `duty` its brightness.
    Pluck,
    /// A struck bell: inharmonic partials, each with its own decay.
    Bell,
}

/// A state-variable filter's response.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterKind {
    #[default]
    Lp,
    Bp,
    Hp,
}

/// A filter over a sound-effect layer: its cutoff sweeps from the first to the second value (Hz)
/// over the layer's life.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SfxFilter {
    pub kind: FilterKind,
    pub cutoff: [f32; 2],
    #[serde(default = "q_default")]
    pub q: f32,
}

fn q_default() -> f32 {
    0.707
}

fn one() -> f32 {
    1.0
}

fn attack_default() -> f32 {
    2.0
}

/// One voice of a sound effect (PRESENTATION.md §5's `SfxPatch { wave, pitch, decay_ms, duty,
/// vibrato }`, grown a filter, a hold, a delay and a little arpeggio).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub wave: Wave,
    /// Hz at the start and at the end (for noise under a filter, what the filter follows when
    /// the filter gives no cutoff of its own).
    pub pitch: [f32; 2],
    /// How long the pitch takes to get from the first to the second value; the layer's whole
    /// life when not given.
    #[serde(default)]
    pub pitch_ms: Option<f32>,
    #[serde(default = "attack_default")]
    pub attack_ms: f32,
    #[serde(default)]
    pub hold_ms: f32,
    /// Time to fall 60 dB after the hold.
    pub decay_ms: f32,
    /// Pulse width for `square`, the index for `fm`, the brightness for `pluck` (0 to 1).
    #[serde(default)]
    pub duty: f32,
    /// The modulator's ratio for `fm`.
    #[serde(default = "one")]
    pub ratio: f32,
    /// Rate in Hz, depth in semitones.
    #[serde(default)]
    pub vibrato: [f32; 2],
    /// Rate in Hz, depth 0 to 1.
    #[serde(default)]
    pub tremolo: [f32; 2],
    #[serde(default)]
    pub filter: Option<SfxFilter>,
    /// When the layer starts, after the patch does.
    #[serde(default)]
    pub delay_ms: f32,
    #[serde(default = "one")]
    pub gain: f32,
    /// Semitone steps played one after another, `every_ms` apart (a little figure: a quest
    /// given, a spell learned). Empty: one note.
    #[serde(default)]
    pub notes: Vec<f32>,
    #[serde(default)]
    pub every_ms: f32,
}

/// A sound effect: layers mixed and rendered into a buffer at boot.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SfxPatch {
    pub name: String,
    pub layers: Vec<Layer>,
    /// The peak the rendered buffer is scaled to (0 to 1).
    #[serde(default = "peak_default")]
    pub peak: f32,
    /// Renders of the patch, each with its own noise and a pitch a few cents off: a footstep is
    /// never the same step twice running.
    #[serde(default = "variants_default")]
    pub variants: u8,
    /// How much of it goes to the shared reverb (0 to 1).
    #[serde(default = "send_default")]
    pub send: f32,
}

fn peak_default() -> f32 {
    0.5
}

fn variants_default() -> u8 {
    1
}

fn send_default() -> f32 {
    0.15
}

/// Which synthesis an instrument uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceKind {
    /// Two modulators into one carrier, each index with its own decay.
    #[default]
    Fm,
    /// Additive: harmonics summed into band-limited tables at boot, played with unison.
    Table,
    /// Karplus-Strong.
    Pluck,
    /// Inharmonic modes: a bell, a bar, a glass.
    Bell,
    /// A pitched membrane: a sine that falls, with a noise click.
    Drum,
    /// Filtered noise.
    Noise,
}

/// An envelope in milliseconds; `s` is a level (0 to 1).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Env {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

impl Default for Env {
    fn default() -> Env {
        Env { a: 5.0, d: 300.0, s: 0.7, r: 300.0 }
    }
}

/// The filter an instrument's voice runs through.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct InstFilter {
    pub kind: FilterKind,
    /// Hz at middle C with no envelope.
    pub cutoff: f32,
    pub q: f32,
    /// Octaves the cutoff opens at the attack, falling back over `env_ms`.
    pub env: f32,
    pub env_ms: f32,
    /// 0: the cutoff stays put; 1: it follows the note.
    pub key: f32,
    /// Octaves a louder note opens it.
    pub vel: f32,
    /// A slow wobble: rate Hz, depth octaves.
    pub lfo: [f32; 2],
}

impl Default for InstFilter {
    fn default() -> InstFilter {
        InstFilter {
            kind: FilterKind::Lp,
            cutoff: 2000.0,
            q: 0.707,
            env: 0.0,
            env_ms: 300.0,
            key: 0.5,
            vel: 0.5,
            lfo: [0.0, 0.0],
        }
    }
}

/// An instrument the sequencer plays.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Instrument {
    pub name: String,
    pub voice: VoiceKind,
    pub env: Env,
    pub gain: f32,
    /// Reverb send, 0 to 1.
    pub send: f32,
    pub filter: Option<InstFilter>,
    /// Rate Hz, depth semitones, delay ms.
    pub vibrato: [f32; 3],
    // --- fm ---
    pub ratio: f32,
    pub index: f32,
    /// The index falls to `index * index_sustain` over this.
    pub index_ms: f32,
    pub index_sustain: f32,
    /// A second modulator (a tine, a chiff): ratio, index, decay ms.
    pub mod2: [f32; 3],
    pub feedback: f32,
    // --- table ---
    /// Harmonic amplitudes from the first (at most 32); empty: `1 / n^rolloff` for `partials`.
    pub harmonics: Vec<f32>,
    pub rolloff: f32,
    pub partials: u8,
    pub odd: bool,
    /// Voices per note, spread `detune` cents.
    pub unison: u8,
    pub detune: f32,
    /// Breath noise under the tone, 0 to 1.
    pub breath: f32,
    /// Formant bands laid over the tone: [Hz, q, gain] (a choir's "oo").
    pub formants: Vec<[f32; 3]>,
    // --- pluck ---
    pub bright: f32,
    /// Seconds to fall 60 dB at middle C.
    pub sustain_s: f32,
    /// Where the string is plucked, 0 to 0.5.
    pub pick: f32,
    // --- bell ---
    /// [ratio, amplitude, decay seconds] per mode (at most 16).
    pub modes: Vec<[f32; 3]>,
    /// Hz between the two halves of each mode: the bell's slow beating.
    pub beat: f32,
    /// The strike's noise, 0 to 1.
    pub strike: f32,
    // --- drum ---
    /// Semitones the pitch falls from, over `drop_ms`.
    pub drop: f32,
    pub drop_ms: f32,
    pub click: f32,
    /// A drum's own pitch in Hz, when the pattern names no note.
    pub hz: f32,
}

impl Default for Instrument {
    fn default() -> Instrument {
        Instrument {
            name: String::new(),
            voice: VoiceKind::Fm,
            env: Env::default(),
            gain: 1.0,
            send: 0.25,
            filter: None,
            vibrato: [0.0, 0.0, 0.0],
            ratio: 1.0,
            index: 1.0,
            index_ms: 400.0,
            index_sustain: 0.3,
            mod2: [0.0, 0.0, 0.0],
            feedback: 0.0,
            harmonics: Vec::new(),
            rolloff: 1.0,
            partials: 12,
            odd: false,
            unison: 1,
            detune: 0.0,
            breath: 0.0,
            formants: Vec::new(),
            bright: 0.5,
            sustain_s: 3.0,
            pick: 0.2,
            modes: Vec::new(),
            beat: 0.0,
            strike: 0.0,
            drop: 0.0,
            drop_ms: 60.0,
            click: 0.0,
            hz: 60.0,
        }
    }
}

/// A song's scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    /// Harmonic minor.
    Harmonic,
    Locrian,
}

impl Mode {
    /// Semitones above the root of each degree.
    pub const fn steps(self) -> [i32; 7] {
        match self {
            Mode::Major => [0, 2, 4, 5, 7, 9, 11],
            Mode::Minor => [0, 2, 3, 5, 7, 8, 10],
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian => [0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian => [0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => [0, 2, 4, 5, 7, 9, 10],
            Mode::Harmonic => [0, 2, 3, 5, 7, 8, 11],
            Mode::Locrian => [0, 1, 3, 5, 6, 8, 10],
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Mode::Major => "major",
            Mode::Minor => "minor",
            Mode::Dorian => "dorian",
            Mode::Phrygian => "phrygian",
            Mode::Lydian => "lydian",
            Mode::Mixolydian => "mixolydian",
            Mode::Harmonic => "harmonic minor",
            Mode::Locrian => "locrian",
        }
    }
}

/// How a track reads its pattern.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    /// Digits are degrees of the key.
    #[default]
    Melody,
    /// `x` sounds the bar's chord, voiced near the track's octave.
    Chord,
    /// Digits are degrees above the chord's root (1 root, 3 third, 5 fifth, 8 octave).
    Bass,
    /// As `bass`, higher: an arpeggio over the chord.
    Arp,
    /// `x` a hit, `X` an accent, `o` a ghost; digits pitch it by degrees of the key.
    Drum,
    /// Generative: notes from `pool` (chord degrees) at seeded intervals; the pattern only says
    /// where it may play (`x`) and where it may not (`.`).
    Drift,
}

/// A drift track's rules.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drift {
    /// Chord degrees it picks from ("1 3 5 8 9").
    pub pool: String,
    /// Steps between notes, least and most.
    pub gap: [u32; 2],
    /// Steps a note lasts, least and most.
    pub len: [u32; 2],
}

/// One part of a song.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub name: String,
    pub inst: String,
    /// Instruments the county may play the part on instead (one is picked per seed).
    #[serde(default)]
    pub alt: Vec<String>,
    pub kind: TrackKind,
    /// The octave degree 1 sits in (4 is middle C's).
    #[serde(default = "octave_default")]
    pub octave: i32,
    #[serde(default = "vel_default")]
    pub vel: f32,
    #[serde(default)]
    pub pan: f32,
    /// A chord track's notes (3 or 4).
    #[serde(default = "voices_default")]
    pub voices: u8,
    /// How much of each step a note sounds before its release when the next token is not `-`
    /// (1: legato).
    #[serde(default = "gate_default")]
    pub gate: f32,
    #[serde(default)]
    pub drift: Option<Drift>,
}

fn octave_default() -> i32 {
    4
}

fn vel_default() -> f32 {
    0.8
}

fn voices_default() -> u8 {
    4
}

fn gate_default() -> f32 {
    0.9
}

/// A pattern, or several the county picks between.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    One(String),
    Alts(Vec<String>),
}

impl Pattern {
    pub fn alts(&self) -> Vec<&str> {
        match self {
            Pattern::One(s) => vec![s.as_str()],
            Pattern::Alts(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// A run of bars with its chords and what each track plays over them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub bars: u32,
    /// One token a bar, or `a/b` for two chords in a bar: a degree of the key (`b7`, `#4`),
    /// then `m` or `M` to force the third, `7` for the seventh, `s2` or `s4` for a suspension.
    pub chords: String,
    pub play: BTreeMap<String, Pattern>,
}

/// A piece of music: a cue's.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Song {
    pub name: String,
    /// What it is for, in a line (the docs and `jane audio list` read it).
    pub mood: String,
    /// The tonic's name: C, C#, Db, ... B.
    pub key: String,
    pub mode: Mode,
    /// Ticks (1/60 s) a step lasts: the sequencer runs on the game's clock.
    pub step_ticks: u32,
    /// Steps in a bar.
    pub bar: u32,
    #[serde(default = "one")]
    pub gain: f32,
    /// Loops until the cue changes; otherwise plays its form once and rings out.
    #[serde(default = "looped_default")]
    pub looped: bool,
    #[serde(default)]
    pub humanize_ms: f32,
    pub tracks: Vec<Track>,
    pub sections: BTreeMap<String, Section>,
    /// Orders of sections ("A A B A"); the county picks one, and each loop may pick again.
    pub forms: Vec<String>,
}

fn looped_default() -> bool {
    true
}

/// Everything under `data/audio`.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub sfx: Vec<SfxPatch>,
    pub instruments: Vec<Instrument>,
    pub songs: Vec<Song>,
}

/// A tonic's pitch class, 0 for C.
pub fn pitch_class(name: &str) -> Option<i32> {
    let mut ch = name.chars();
    let base: i32 = match ch.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let acc = match ch.as_str() {
        "" => 0,
        "#" => 1,
        "b" => -1,
        _ => return None,
    };
    Some((base + acc).rem_euclid(12))
}

/// Everything wrong with a library: names that clash or are missing, patterns that do not fit
/// their sections, numbers out of range. Empty when it is sound.
pub fn check(lib: &Library) -> Vec<String> {
    let mut errs = Vec::new();
    let mut names: Vec<&str> = Vec::new();
    for p in &lib.sfx {
        if names.contains(&p.name.as_str()) {
            errs.push(format!("sfx {}: named twice", p.name));
        }
        names.push(&p.name);
        if p.layers.is_empty() {
            errs.push(format!("sfx {}: no layers", p.name));
        }
        if !(0.0..=1.0).contains(&p.peak) {
            errs.push(format!("sfx {}: peak {} is not 0 to 1", p.name, p.peak));
        }
        for (i, l) in p.layers.iter().enumerate() {
            if l.notes.len() > 32 {
                errs.push(format!("sfx {} layer {i}: more than 32 notes", p.name));
            }
            if l.decay_ms <= 0.0 || l.attack_ms < 0.0 {
                errs.push(format!("sfx {} layer {i}: decay_ms must be over 0", p.name));
            }
            if l.pitch[0] <= 0.0 || l.pitch[1] <= 0.0 {
                errs.push(format!("sfx {} layer {i}: pitch must be over 0 Hz", p.name));
            }
        }
    }
    let mut inst_names: Vec<&str> = Vec::new();
    for i in &lib.instruments {
        if inst_names.contains(&i.name.as_str()) {
            errs.push(format!("instrument {}: named twice", i.name));
        }
        inst_names.push(&i.name);
        if i.harmonics.len() > 32 || i.modes.len() > 16 || i.formants.len() > 4 {
            errs.push(format!("instrument {}: too many harmonics, modes or formants", i.name));
        }
        if i.voice == VoiceKind::Bell && i.modes.is_empty() {
            errs.push(format!("instrument {}: a bell needs modes", i.name));
        }
        if i.env.a < 0.0 || i.env.r < 0.0 || !(0.0..=1.0).contains(&i.env.s) {
            errs.push(format!("instrument {}: envelope out of range", i.name));
        }
    }
    let mut song_names: Vec<&str> = Vec::new();
    for s in &lib.songs {
        let who = format!("song {}", s.name);
        if song_names.contains(&s.name.as_str()) {
            errs.push(format!("{who}: named twice"));
        }
        song_names.push(&s.name);
        if pitch_class(&s.key).is_none() {
            errs.push(format!("{who}: key {} is not a note", s.key));
        }
        if s.step_ticks == 0 || s.bar == 0 {
            errs.push(format!("{who}: step_ticks and bar must be over 0"));
        }
        if s.forms.is_empty() {
            errs.push(format!("{who}: no forms"));
        }
        for t in &s.tracks {
            for n in std::iter::once(&t.inst).chain(&t.alt) {
                if !inst_names.contains(&n.as_str()) {
                    errs.push(format!("{who} track {}: no instrument {n}", t.name));
                }
            }
            if t.kind == TrackKind::Drift && t.drift.is_none() {
                errs.push(format!("{who} track {}: a drift track needs its drift", t.name));
            }
            if let Some(d) = &t.drift {
                if d.gap[0] == 0 || d.gap[0] > d.gap[1] || d.len[0] == 0 || d.len[0] > d.len[1] {
                    errs.push(format!("{who} track {}: drift gap and len are [least, most], over 0", t.name));
                }
                if let Err(e) = crate::pattern::degrees(&d.pool) {
                    errs.push(format!("{who} track {}: drift pool: {e}", t.name));
                }
            }
        }
        for form in &s.forms {
            for sec in form.split_whitespace() {
                if !s.sections.contains_key(sec) {
                    errs.push(format!("{who}: form {form:?} names no section {sec}"));
                }
            }
        }
        for (name, sec) in &s.sections {
            let steps = sec.bars * s.bar;
            match crate::pattern::chords(&sec.chords, s.mode) {
                Ok(c) if c.len() as u32 != sec.bars * 2 => {
                    errs.push(format!("{who} section {name}: {} bars but chords for {}", sec.bars, c.len() / 2));
                }
                Ok(_) => {}
                Err(e) => errs.push(format!("{who} section {name} chords: {e}")),
            }
            for (track, pat) in &sec.play {
                let Some(t) = s.tracks.iter().find(|t| &t.name == track) else {
                    errs.push(format!("{who} section {name}: no track {track}"));
                    continue;
                };
                for (k, alt) in pat.alts().into_iter().enumerate() {
                    match crate::pattern::parse(alt, t.kind) {
                        Ok(v) if v.len() as u32 != steps => errs.push(format!(
                            "{who} section {name} {track} ({k}): {} steps, the section has {steps}",
                            v.len()
                        )),
                        Ok(_) => {}
                        Err(e) => errs.push(format!("{who} section {name} {track} ({k}): {e}")),
                    }
                }
            }
        }
    }
    errs
}
