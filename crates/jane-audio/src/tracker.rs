//! The consoles' music and sound (PORT.md §13.4): the PC's synth rendered once, at build time,
//! into a **module** (`jane-psp.jau`): the songs as patterns (the same steps, chords, forms and
//! seeded choices `data/audio/songs` holds) over one shared bank of short samples (each
//! instrument's notes a few semitones apart, every sound effect, a loop of each bed), and the
//! integer player and mixer that play it. MOD-style by decision (2026-10-08): never streamed
//! audio, so the file is kilobytes of patterns and the bank, and the console runs no synth.
//!
//! `no_std` plus `alloc` and integer only. The mixer never allocates once made (it runs on the
//! console's audio thread, which must not touch the game's heap), and its sequencer draws the
//! same notes from the same seed as the PC's (`seq.rs`; `tests` hold the two equal note for note).
//!
//! Fixed point: gains and pans Q12 (4096 is 1), envelopes and fades Q16, sample positions Q16,
//! samples and buses in 16-bit units (32767 is full scale).

#![deny(clippy::float_arithmetic)]

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// The module file's first bytes.
pub const MAGIC: [u8; 4] = *b"JAU1";
/// One in Q12.
pub const Q12: i32 = 4096;
const ONE16: u32 = 1 << 16;
/// The velocity every instrument's samples are rendered at (Q12, 0.7): the PC's notes sit near
/// it, so their brightness (FM index, filter) is the rendered one.
pub const REF_VEL: u32 = 2867;
/// Samples a block (the sequencer steps at block starts, notes land on their sample).
pub const BLOCK: usize = 64;
/// Gains and envelopes move every this many samples.
const CHUNK: usize = 32;
/// Voices at most: the music's, the effects' and the beds' together. Fixed, so the audio thread
/// never allocates.
pub const MAX_VOICES: usize = 80;
/// Music voices sounding at most: past it the quietest fades in 8 ms (the PC lets 48 sound and
/// steals the oldest; on the PSP each voice is a share of the CPU, and past two dozen the
/// quietest are a plucked string's or a pad's long tail, under the rest by 30 dB and more).
pub const MAX_MUSIC: usize = 24;
/// Effects at once at most (the PC's 32).
const MAX_SFX: usize = 16;
/// Songs playing at once (the current and those fading out).
const PLAYERS: usize = 3;
/// A song's tracks and sections at most (the bake refuses more).
pub const MAX_TRACKS: usize = 12;
pub const MAX_SECTIONS: usize = 8;
/// The beds (`Bed::ALL`'s ten).
pub const BEDS: usize = 10;
/// The loudest the mix may be: the PC's 0.93 of full scale.
const CEILING: i32 = 30_473;

// ---------------------------------------------------------------- the module

/// How a sample's frames are stored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    /// Little-endian 16-bit.
    #[default]
    Pcm16,
    /// The PlayStation's ADPCM (`vag_decode`): 28 frames in 15 bytes.
    Adpcm,
    /// Eight bits a frame, companded (`mu8`): for what ADPCM roughens (bright, fast voices).
    Mu8,
}

/// One sound in the bank: mono, at its own rate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// Bytes from the start of the bank's data.
    pub at: u32,
    /// Frames.
    pub len: u32,
    pub rate: u32,
    pub codec: Codec,
    /// Stored near full scale: the frames times `level / 65536` are the sound as rendered.
    pub level: u32,
    /// The loop (`loop_len` 0: none, it plays once).
    pub loop_start: u32,
    pub loop_len: u32,
    /// The decoder's state before `loop_start` (ADPCM): the two frames before it, packed.
    pub loop_state: i32,
    /// Past `loop_start` it falls 60 dB over this many ms: a struck or plucked note's tail, looped
    /// (0: held while the note is).
    pub tail_ms: u32,
    /// Kept on the Memory Stick, not in RAM: `at` is from the start of the module's far part, and
    /// the game loads it into the mixer's one far slot when it is wanted (a lesson's cue).
    pub far: bool,
}

/// An instrument's notes: each zone's sample serves the notes up to `top`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    pub top: u8,
    pub root: u8,
    pub sample: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inst {
    pub name: String,
    /// By `top`, rising.
    pub zones: Vec<Zone>,
    /// The PC envelope's release (60 dB), ms.
    pub release_ms: u32,
    /// Reverb send, Q12.
    pub send: u16,
    /// A drum's own pitch (the MIDI note nearest its `hz`): it is tuned to the song's tonic.
    pub own: i8,
    /// A plucked string: a chord on it is strummed.
    pub pluck: bool,
    /// A held voice's envelope, which the player draws (its samples are rendered at full
    /// sustain): the PC's attack and decay, ms, and its sustain (Q12). A struck voice's is in its
    /// samples (`attack_ms` 0).
    pub attack_ms: u32,
    pub decay_ms: u32,
    pub sustain: u16,
    /// A note's loudness against one at [`REF_VEL`] (Q12), at velocity `k / 8` (k = 0 to 12),
    /// measured on the PC's own voice at the bake: louder notes there are brighter, so the curve
    /// is the instrument's, not one law for all.
    pub vel: Vec<u16>,
    /// Each note's own level against its zone's root (Q12), from MIDI note `lo` up, measured: a
    /// filter or a formant that stays put on the PC moves with a pitched sample.
    pub lo: u8,
    pub gains: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Melody,
    Chord,
    Bass,
    Arp,
    Drum,
    Drift,
}

pub const REST: u8 = 0;
pub const HOLD: u8 = 1;
pub const NOTE: u8 = 2;
pub const HIT: u8 = 3;

/// One step of a pattern (`pattern::Step`). Stored as one varint (a rest or a hold one byte).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u32", into = "u32")]
pub struct Step {
    pub tag: u8,
    pub maybe: bool,
    pub deg: i8,
    pub acc: i8,
    pub oct: i8,
    /// Q12.
    pub vel: u16,
}

impl From<Step> for u32 {
    fn from(s: Step) -> u32 {
        u32::from(s.tag & 3)
            | u32::from(s.maybe) << 2
            | (s.deg as u32 & 31) << 3
            | ((i32::from(s.acc) + 4) as u32 & 7) << 8
            | ((i32::from(s.oct) + 4) as u32 & 7) << 11
            | (u32::from(s.vel) & 8191) << 14
    }
}

impl From<u32> for Step {
    fn from(x: u32) -> Step {
        Step {
            tag: (x & 3) as u8,
            maybe: x >> 2 & 1 == 1,
            deg: (x >> 3 & 31) as i8,
            acc: (x >> 8 & 7) as i8 - 4,
            oct: (x >> 11 & 7) as i8 - 4,
            vel: (x >> 14 & 8191) as u16,
        }
    }
}

/// Half a bar's chord, resolved against the song's mode.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    /// Semitones above the tonic of chord degree 0 to 6 (`Chord::tone`).
    pub tone: [i8; 7],
    /// `Chord::tones`.
    pub tones: Vec<i8>,
    /// Its root altered (`b7`).
    pub acc: bool,
    /// Altered or its third forced: a note on it counts as chromatic.
    pub borrowed: bool,
}

impl Chord {
    fn tone(&self, k: i32) -> i32 {
        i32::from(self.tone[k.rem_euclid(7) as usize]) + 12 * k.div_euclid(7)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drift {
    pub pool: Vec<i8>,
    pub gap: [u32; 2],
    pub len: [u32; 2],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub kind: Kind,
    /// The instrument and its alternatives, by index into the module's.
    pub insts: Vec<u8>,
    pub octave: i8,
    /// Q12.
    pub vel: u16,
    /// Q12, -4096 left to 4096 right.
    pub pan: i16,
    pub voices: u8,
    /// Q12 of a step.
    pub gate: u16,
    pub drift: Option<Drift>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub bars: u32,
    /// Two a bar.
    pub chords: Vec<Chord>,
    /// Per track: its patterns (empty: it rests here).
    pub pats: Vec<Vec<Vec<Step>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    /// `patch::hash` of the name: the song's own draws.
    pub hash: u32,
    pub tonic: i8,
    /// The mode's semitones a degree.
    pub steps: [i8; 7],
    pub step_ticks: u32,
    pub bar: u32,
    /// Q12.
    pub gain: u16,
    pub looped: bool,
    /// The humanising, microseconds.
    pub hum_us: u32,
    pub tracks: Vec<Track>,
    pub sections: Vec<Section>,
    pub forms: Vec<Vec<u8>>,
}

/// A sound effect: its sample, its send (Q12) and its variants' tunings (Q16 rates; the PC
/// renders each variant a few cents apart).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sfx {
    pub name: String,
    pub sample: u16,
    pub send: u16,
    pub tunes: Vec<u32>,
}

/// A bed: a seamless loop at its full level. A wide one plays twice, half a loop apart, one copy
/// to each side (noise beds decorrelate so); the other plays in the middle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BedLoop {
    pub name: String,
    pub sample: u16,
    pub wide: bool,
    pub send: u16,
    /// Where the second copy starts, and the decoder's state there.
    pub half: u32,
    pub half_state: i32,
}

/// Everything in the module but the sample frames.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub samples: Vec<Sample>,
    pub insts: Vec<Inst>,
    pub songs: Vec<Song>,
    pub sfx: Vec<Sfx>,
    /// In `Bed::ALL`'s order.
    pub beds: Vec<BedLoop>,
}

/// A module, read: its header and the bytes its samples are in.
///
/// The file: `MAGIC`, the header's length and the resident frames' length (`u32` LE each), the
/// header (postcard), the resident frames, then the far frames (kept on the Memory Stick).
#[derive(Debug)]
pub struct Bank {
    pub head: Header,
    bytes: Vec<u8>,
    data: usize,
    /// Where the far frames start in `bytes`, when the bytes hold them (a host's read).
    far: Option<usize>,
    /// The resident part's end: where the far frames start in the file.
    ram: usize,
}

/// The file's fixed start: magic and two lengths.
pub const PREFIX: usize = 12;

impl Bank {
    pub fn write(head: &Header, resident: &[u8], far: &[u8]) -> Vec<u8> {
        let h = postcard::to_allocvec(head).unwrap_or_default();
        let mut out = Vec::with_capacity(PREFIX + h.len() + resident.len() + far.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(h.len() as u32).to_le_bytes());
        out.extend_from_slice(&(resident.len() as u32).to_le_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(resident);
        out.extend_from_slice(far);
        out
    }

    /// From the file's first [`PREFIX`] bytes: how many to read into RAM, and where the far
    /// frames start in the file.
    pub fn ram_len(prefix: &[u8]) -> Option<usize> {
        if prefix.len() < PREFIX || prefix[..4] != MAGIC {
            return None;
        }
        let u = |k: usize| u32::from_le_bytes([prefix[k], prefix[k + 1], prefix[k + 2], prefix[k + 3]]) as usize;
        PREFIX.checked_add(u(4))?.checked_add(u(8))
    }

    /// Frames' bytes of a sample.
    pub fn sample_bytes(s: &Sample) -> usize {
        match s.codec {
            Codec::Pcm16 => s.len as usize * 2,
            Codec::Adpcm => (s.len as usize).div_ceil(VAG_FRAMES) * VAG_BYTES,
            Codec::Mu8 => s.len as usize,
        }
    }

    /// Reads a module: the resident part at least (a console's read), or the whole file (a
    /// host's). Every sample must lie where it says.
    pub fn parse(bytes: Vec<u8>) -> Result<Bank, &'static str> {
        let ram = Bank::ram_len(&bytes).ok_or("not a JAU1 module")?;
        if ram > bytes.len() {
            return Err("short module");
        }
        let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let end = PREFIX + n;
        let head: Header = postcard::from_bytes(&bytes[PREFIX..end]).map_err(|_| "bad module header")?;
        let far = (bytes.len() > ram).then_some(ram);
        for s in &head.samples {
            let room = if s.far { far.map_or(usize::MAX, |f| bytes.len() - f) } else { ram - end };
            if s.at as usize + Bank::sample_bytes(s) > room || s.rate == 0 || s.loop_start + s.loop_len > s.len {
                return Err("a sample lies outside the module");
            }
        }
        let ok_sample = |i: u16| usize::from(i) < head.samples.len();
        if !head.insts.iter().all(|i| !i.zones.is_empty() && i.zones.iter().all(|z| ok_sample(z.sample)))
            || !head.sfx.iter().all(|s| ok_sample(s.sample) && !s.tunes.is_empty())
            || !head.beds.iter().all(|b| ok_sample(b.sample))
            || !head.insts.iter().all(|i| i.vel.len() >= 2)
        {
            return Err("a sample index past the bank");
        }
        for s in &head.songs {
            let tracks_ok = s.tracks.len() <= MAX_TRACKS
                && s.tracks
                    .iter()
                    .all(|t| !t.insts.is_empty() && t.insts.iter().all(|&i| usize::from(i) < head.insts.len()));
            let secs_ok = s.sections.len() <= MAX_SECTIONS
                && s.sections
                    .iter()
                    .all(|x| x.pats.len() == s.tracks.len() && x.chords.len() as u32 >= x.bars * 2 && x.bars > 0);
            let forms_ok = !s.forms.is_empty()
                && s.forms.iter().all(|f| !f.is_empty() && f.iter().all(|&x| usize::from(x) < s.sections.len()));
            if !(tracks_ok && secs_ok && forms_ok && s.bar > 0 && s.step_ticks > 0) {
                return Err("a song does not fit the player");
            }
        }
        let mut bytes = bytes;
        if far.is_none() {
            bytes.shrink_to_fit();
        }
        Ok(Bank { head, bytes, data: end, far, ram })
    }

    /// The bytes held in RAM (the far part, on a console, is not).
    pub fn bytes(&self) -> usize {
        self.far.unwrap_or(self.bytes.len())
    }

    /// Where the far frames start in the file.
    pub fn far_at(&self) -> usize {
        self.ram
    }

    fn data(&self) -> &[u8] {
        &self.bytes[self.data..]
    }

    /// A far sample's frames, when this read holds them (a host's).
    fn far_frames(&self, s: &Sample) -> Option<&[u8]> {
        let f = self.far?;
        self.bytes.get(f + s.at as usize..f + s.at as usize + Bank::sample_bytes(s))
    }

    pub fn song_index(&self, name: &str) -> Option<usize> {
        self.head.songs.iter().position(|s| s.name == name)
    }

    pub fn sfx_index(&self, name: &str) -> Option<usize> {
        self.head.sfx.iter().position(|s| s.name == name)
    }

    pub fn bed_index(&self, name: &str) -> Option<usize> {
        self.head.beds.iter().position(|s| s.name == name)
    }
}
// ---------------------------------------------------------------- ADPCM
// The PlayStation's own (its SPU's "VAG"): blocks of 28 frames, each a byte (the shift and which
// of five two-tap predictors) and 14 bytes of 4-bit residuals, the low nibble first. A predictor
// follows a tone far better than a lone step does, so bright voices keep their clarity at a
// little over four bits a frame.

/// Frames a block, and its bytes.
pub const VAG_FRAMES: usize = 28;
pub const VAG_BYTES: usize = 15;
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// One frame: the block's header byte, the frame's nibble, and the two frames before it (moved
/// on).
#[inline]
pub fn vag_decode(head: u8, nib: u8, old: &mut i32, older: &mut i32) -> i32 {
    let shift = u32::from(head & 15).min(12);
    let (f0, f1) = FILTERS[usize::from(head >> 4).min(4)];
    let s = (i32::from(nib) << 28 >> 16) >> shift;
    let y = (s + ((*old * f0 + *older * f1 + 32) >> 6)).clamp(-32_768, 32_767);
    *older = *old;
    *old = y;
    y
}

/// The two frames before frame `i`, packed as a loop's or a copy's start keeps them.
pub fn vag_pack(old: i32, older: i32) -> i32 {
    (old & 0xffff) | (older << 16)
}

pub fn vag_unpack(p: i32) -> (i32, i32) {
    (p << 16 >> 16, p >> 16)
}

/// Encodes `x`: each block the predictor and shift that land nearest. Returns the bytes and the
/// decoder's state before each frame in `marks` (packed, [`vag_pack`]).
pub fn vag_encode(x: &[i16], marks: &[usize]) -> (Vec<u8>, Vec<i32>) {
    let blocks = x.len().div_ceil(VAG_FRAMES);
    let mut out = alloc::vec![0u8; blocks * VAG_BYTES];
    let mut states = alloc::vec![0; marks.len()];
    let (mut old, mut older) = (0i32, 0i32);
    for b in 0..blocks {
        let frames = &x[b * VAG_FRAMES..((b + 1) * VAG_FRAMES).min(x.len())];
        let mut best = (u64::MAX, 0u8);
        for f in 0..5u8 {
            for shift in 0..=12u8 {
                let head = f << 4 | shift;
                let (mut o, mut oo) = (old, older);
                let mut err = 0u64;
                for &s in frames {
                    let nib = vag_nibble(head, s, o, oo);
                    let y = vag_decode(head, nib, &mut o, &mut oo);
                    let e = i64::from(y) - i64::from(s);
                    err = err.saturating_add((e * e) as u64);
                    if err >= best.0 {
                        break;
                    }
                }
                if err < best.0 {
                    best = (err, head);
                }
            }
        }
        let head = best.1;
        out[b * VAG_BYTES] = head;
        for (k, &s) in frames.iter().enumerate() {
            let i = b * VAG_FRAMES + k;
            for (m, st) in marks.iter().zip(states.iter_mut()) {
                if *m == i {
                    *st = vag_pack(old, older);
                }
            }
            let nib = vag_nibble(head, s, old, older);
            vag_decode(head, nib, &mut old, &mut older);
            out[b * VAG_BYTES + 1 + k / 2] |= if k % 2 == 0 { nib } else { nib << 4 };
        }
    }
    (out, states)
}

/// The nibble whose decoding lands nearest `s`.
fn vag_nibble(head: u8, s: i16, old: i32, older: i32) -> u8 {
    let mut best = (u32::MAX, 0u8);
    for nib in 0..16u8 {
        let (mut o, mut oo) = (old, older);
        let e = (vag_decode(head, nib, &mut o, &mut oo) - i32::from(s)).unsigned_abs();
        if e < best.0 {
            best = (e, nib);
        }
    }
    best.1
}
// ---------------------------------------------------------------- integer helpers

/// `2^(-x / 65536)` in Q16.
pub fn exp2_neg_q16(x: u32) -> u32 {
    let ip = x >> 16;
    if ip >= 31 {
        return 0;
    }
    let f = i64::from(x & 0xffff);
    // Taylor of e^(-f ln 2) to f^4 (0.13% at worst).
    let p = 65_536 - ((f * (45_426 - ((f * (15_743 - ((f * (3_638 - ((f * 630) >> 16))) >> 16))) >> 16))) >> 16);
    (p.clamp(0, 65_536) as u32) >> ip
}

/// The multiplier a stretch of `n` samples takes to fall 60 dB in `ms` at `rate` (Q16).
pub fn fall_q16(ms: u32, rate: u32, n: u32) -> u32 {
    // log2(1000) * 65536 * 1000 = 653_123_000 (to the third figure).
    let samples = u64::from(ms.max(1)) * u64::from(rate);
    let x = 653_123_000u64 * u64::from(n) / samples.max(1);
    exp2_neg_q16(x.min(u64::from(u32::MAX)) as u32)
}

/// `2^(semis / 12)` (Q16) for `semis` in Q16: a note's step against its sample's root.
pub fn pitch_q16(semis_q16: i32) -> u32 {
    // In octaves, Q16.
    let oct = i64::from(semis_q16) * 65_536 / (12 * 65_536);
    let oct = oct as i32;
    if oct >= 0 {
        let whole = (oct >> 16) as u32 + 1;
        let rest = (whole << 16) - oct as u32;
        (u64::from(exp2_neg_q16(rest)) << whole).min(u64::from(u32::MAX)) as u32
    } else {
        exp2_neg_q16(oct.unsigned_abs())
    }
}

/// `sin(pi/2 * t / 4096)` in Q12 for `t` in 0 to 4096.
fn quarter_sin(t: i32) -> i32 {
    let t = i64::from(t.clamp(0, 4096));
    let t2 = (t * t) >> 12;
    let t3 = (t2 * t) >> 12;
    let t5 = (t3 * t2) >> 12;
    (((6_434 * t - 2_646 * t3 + 326 * t5) >> 12) as i32).clamp(0, 4096)
}

/// Equal-power pan (Q12 gains) of `pan` (Q12, -4096 left to 4096 right): the PC's `pan_gains`.
pub fn pan_gains(pan: i32) -> (i32, i32) {
    let t = (pan.clamp(-Q12, Q12) + Q12) / 2;
    (quarter_sin(Q12 - t), quarter_sin(t))
}

/// The PC sequencer's noise (`dsp::Rng`, xorshift32), bit for bit: the same seed draws the same
/// notes on every target.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        let mut s = seed.wrapping_mul(0x9e37_79b9) ^ 0x6a09_e667;
        if s == 0 {
            s = 0x1234_5678;
        }
        let mut r = Rng(s);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// `f()`'s draw in Q24 (`f() == u24 / 2^24`).
    pub fn u24(&mut self) -> u32 {
        self.next_u32() >> 8
    }

    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo { lo } else { lo + self.next_u32() % (hi - lo + 1) }
    }
}

// ---------------------------------------------------------------- the sequencer

/// A note the sequencer played (the tests compare it with the PC's `NoteOn`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteOn {
    pub track: u16,
    pub midi: i32,
    pub chromatic: bool,
    /// Frames from the song's start.
    pub at: u64,
}

/// A note to start.
#[derive(Clone, Copy, Debug, Default)]
struct Start {
    inst: u8,
    midi: i32,
    /// Q12.
    vel: u32,
    pan: i32,
    delay: u32,
    gate: u32,
}

/// One song playing: `seq::Player` in integers.
#[derive(Clone, Debug)]
pub struct Player {
    pub song: usize,
    seed: u32,
    pub loop_n: u32,
    form: usize,
    form_pos: usize,
    step_in_sec: u32,
    choice: [[u8; MAX_TRACKS]; MAX_SECTIONS],
    inst: [u8; MAX_TRACKS],
    drift_wait: [u32; MAX_TRACKS],
    steps: u64,
    started: u64,
    next_at: u64,
    /// Q24.
    gain: u32,
    target: u32,
    fade_step: u32,
    pub ended: bool,
    rng: Rng,
    /// Filled when asked (tests): every note played.
    pub log: Option<Vec<NoteOn>>,
}

impl Player {
    pub fn new(song: usize, data: &Song, seed: u32, now: u64) -> Player {
        let mut county = Rng::new(seed ^ data.hash);
        let mut inst = [0u8; MAX_TRACKS];
        for (slot, t) in inst.iter_mut().zip(&data.tracks) {
            *slot = t.insts[county.range(0, t.insts.len() as u32 - 1) as usize];
        }
        let mut p = Player {
            song,
            seed,
            loop_n: 0,
            form: 0,
            form_pos: 0,
            step_in_sec: 0,
            choice: [[0; MAX_TRACKS]; MAX_SECTIONS],
            inst,
            drift_wait: [0; MAX_TRACKS],
            steps: 0,
            started: now,
            next_at: now,
            gain: 1 << 24,
            target: 1 << 24,
            fade_step: 0,
            ended: false,
            rng: county,
            log: None,
        };
        p.pick(data);
        p
    }

    fn pick(&mut self, data: &Song) {
        let mut r = Rng::new(self.seed ^ data.hash ^ self.loop_n.wrapping_mul(0x2545_f491));
        self.form = r.range(0, data.forms.len() as u32 - 1) as usize;
        for (row, s) in self.choice.iter_mut().zip(&data.sections) {
            for (c, alts) in row.iter_mut().zip(&s.pats) {
                *c = if alts.len() > 1 { r.range(0, alts.len() as u32 - 1) as u8 } else { 0 };
            }
        }
        self.rng = r;
    }

    /// Fades toward `to` (Q24) over `ms` at `rate`.
    fn fade(&mut self, to: u32, ms: u32, rate: u32) {
        self.target = to;
        if ms == 0 {
            self.gain = to;
            self.fade_step = 0;
            return;
        }
        let diff = u64::from(to.abs_diff(self.gain));
        self.fade_step = (diff * 1000 / (u64::from(ms) * u64::from(rate))).max(1) as u32;
    }

    /// Moves the fade on `n` frames.
    fn ramp(&mut self, n: u32) {
        let d = self.fade_step.saturating_mul(n);
        if self.gain < self.target {
            self.gain = self.gain.saturating_add(d).min(self.target);
        } else {
            self.gain = self.gain.saturating_sub(d).max(self.target);
        }
    }

    /// The song's level now, Q16: the fade squared, times the song's gain (as the PC).
    fn level(&self, data: &Song) -> u32 {
        let g = u64::from(self.gain >> 8);
        ((((g * g) >> 16) * u64::from(data.gain)) >> 12) as u32
    }

    fn step_frames(data: &Song, steps: u64, rate: u32) -> u64 {
        steps * u64::from(data.step_ticks) * u64::from(rate) / 60
    }

    /// One step's notes into `out` (as `seq::Player::step`, draw for draw).
    #[allow(clippy::too_many_lines)]
    fn step(&mut self, data: &Song, insts: &[Inst], off: u32, rate: u32, out: &mut Vec<Start>) {
        let sec_ix = usize::from(data.forms[self.form][self.form_pos]);
        let sec = &data.sections[sec_ix];
        let s = self.step_in_sec;
        let bar = data.bar;
        let chord = &sec.chords[((s / bar) * 2 + (s % bar) * 2 / bar) as usize];
        let hum = (u64::from(data.hum_us) * u64::from(rate) / 1_000_000) as u32;
        let step_len = |q12: u64| (q12 * u64::from(data.step_ticks) * u64::from(rate) / (4096 * 60)) as u32;
        for (ti, track) in data.tracks.iter().enumerate() {
            self.drift_wait[ti] = self.drift_wait[ti].saturating_sub(1);
            let alts = &sec.pats[ti];
            if alts.is_empty() {
                continue;
            }
            let pat = &alts[usize::from(self.choice[sec_ix][ti]).min(alts.len() - 1)];
            let Some(&tok) = pat.get(s as usize) else { continue };
            let holds = pat[s as usize + 1..].iter().take_while(|t| t.tag == HOLD).count() as u64;
            let base = 12 * (i32::from(track.octave) + 1) + i32::from(data.tonic);
            if tok.tag == REST || tok.tag == HOLD {
                continue;
            }
            if tok.maybe && self.rng.next_u32() < 1 << 31 {
                continue;
            }
            let inst = usize::from(self.inst[ti]);
            let mut notes = [0i32; 8];
            let (n, chromatic) = match (tok.tag, track.kind) {
                (NOTE, Kind::Melody | Kind::Drum) => {
                    notes[0] =
                        base + degree(data.steps, i32::from(tok.deg)) + i32::from(tok.acc) + 12 * i32::from(tok.oct);
                    (1, tok.acc != 0)
                }
                (NOTE, _) => {
                    notes[0] = base + chord.tone(i32::from(tok.deg)) + i32::from(tok.acc) + 12 * i32::from(tok.oct);
                    (1, tok.acc != 0 || chord.borrowed)
                }
                (_, Kind::Chord) => (voice_chord(&chord.tones, base, track.voices, &mut notes), chord.borrowed),
                (_, Kind::Drift) => {
                    let Some(d) = &track.drift else { continue };
                    if self.drift_wait[ti] > 0 || d.pool.is_empty() {
                        continue;
                    }
                    let k = d.pool[self.rng.range(0, d.pool.len() as u32 - 1) as usize];
                    let l = self.rng.range(d.len[0], d.len[1]);
                    self.drift_wait[ti] = self.rng.range(d.gap[0], d.gap[1]);
                    let midi = base + chord.tone(i32::from(k));
                    let gate = step_len((u64::from(l) * 4096 + u64::from(track.gate)).saturating_sub(4096));
                    // 0.75 + 0.25 f, Q12.
                    let jit = 3072 + (self.rng.u24() >> 14);
                    let v = (u32::from(track.vel) * u32::from(tok.vel)) >> 12;
                    let h = self.rng.range(0, hum * 3);
                    self.play(ti, inst, midi, (v * jit) >> 12, track.pan, off + h, gate, chord.acc, out);
                    continue;
                }
                _ => {
                    let own = i32::from(insts[inst].own);
                    let up = (i32::from(data.tonic) - own).rem_euclid(12);
                    notes[0] = if up <= 6 { own + up } else { own + up - 12 };
                    (1, false)
                }
            };
            let gate = step_len(holds * 4096 + u64::from(track.gate));
            let strum = if insts[inst].pluck && n > 1 { rate * 16 / 1000 } else { 0 };
            for (j, &midi) in notes[..n].iter().enumerate() {
                let h = if hum > 0 { self.rng.range(0, hum) } else { 0 };
                // 1 + 0.12 (f - 0.5), Q12.
                let jit = (4096 + (((i64::from(self.rng.u24()) - (1 << 23)) * 492) >> 24)) as u32;
                let v = (u32::from(track.vel) * u32::from(tok.vel)) >> 12;
                self.play(ti, inst, midi, (v * jit) >> 12, track.pan, off + h + strum * j as u32, gate, chromatic, out);
            }
        }
        self.step_in_sec += 1;
        if self.step_in_sec == sec.bars * bar {
            self.step_in_sec = 0;
            self.form_pos += 1;
            if self.form_pos == data.forms[self.form].len() {
                self.form_pos = 0;
                if data.looped {
                    self.loop_n += 1;
                    self.pick(data);
                } else {
                    self.ended = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn play(
        &mut self,
        track: usize,
        inst: usize,
        midi: i32,
        vel: u32,
        pan: i16,
        delay: u32,
        gate: u32,
        chromatic: bool,
        out: &mut Vec<Start>,
    ) {
        if let Some(log) = &mut self.log {
            log.push(NoteOn {
                track: track as u16,
                midi,
                chromatic,
                at: self.next_at - self.started + u64::from(delay),
            });
        }
        // The PC draws a voice's own seed here; the draw is kept so the next notes stay the PC's.
        self.rng.next_u32();
        if out.len() < out.capacity() {
            out.push(Start { inst: inst as u8, midi, vel, pan: i32::from(pan), delay, gate });
        }
    }
}

fn degree(steps: [i8; 7], deg: i32) -> i32 {
    i32::from(steps[deg.rem_euclid(7) as usize]) + 12 * deg.div_euclid(7)
}

/// `seq::voice_chord`, into `out`; returns how many.
fn voice_chord(tones: &[i8], base: i32, voices: u8, out: &mut [i32; 8]) -> usize {
    let centre = base + 4;
    let mut n = 0;
    for &t in tones.iter().take(7) {
        let mut p = base + i32::from(t);
        while p < centre - 6 {
            p += 12;
        }
        while p >= centre + 6 {
            p -= 12;
        }
        out[n] = p;
        n += 1;
    }
    out[..n].sort_unstable();
    // Dedup.
    let mut m = 0;
    for i in 0..n {
        if m == 0 || out[i] != out[m - 1] {
            out[m] = out[i];
            m += 1;
        }
    }
    if usize::from(voices) > m && !tones.is_empty() && m < 8 {
        let lo = out[0];
        let mut root = base + i32::from(tones[0]);
        while root >= lo {
            root -= 12;
        }
        while root + 12 < lo {
            root += 12;
        }
        out.copy_within(0..m, 1);
        out[0] = root;
        m += 1;
    }
    m.min(usize::from(voices.max(1)))
}

// ---------------------------------------------------------------- the mixer

/// What the game asks of the mixer (`engine::Cmd` in integers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// Change the music (`None`: fade to silence); the same song again is nothing.
    Music {
        song: Option<u16>,
        fade_out_ms: u16,
        fade_in_ms: u16,
    },
    /// Effect `id` at `gain` (Q12), `pan` (Q12), `send` (Q12) and `rate` (Q16, 65536 as rendered).
    Sfx {
        id: u16,
        gain: u16,
        pan: i16,
        send: u16,
        rate: u32,
    },
    /// A bed to a level (0 to 255); it fades there over about two seconds.
    Bed {
        bed: u8,
        level: u8,
    },
    /// Master, music and effects, Q12.
    Volume {
        master: u16,
        music: u16,
        sfx: u16,
    },
    Seed(u32),
    /// The music and the beds to this share of their level (of 255).
    Duck(u8),
}

/// A command as two words: a tag in the low byte, its fields above.
pub fn pack(c: Cmd) -> (u64, u64) {
    let w = |x: u16| u64::from(x);
    match c {
        Cmd::Music { song, fade_out_ms, fade_in_ms } => (
            1 | u64::from(song.is_some()) << 8
                | w(song.unwrap_or(0)) << 16
                | w(fade_out_ms) << 32
                | w(fade_in_ms) << 48,
            0,
        ),
        Cmd::Sfx { id, gain, pan, send, rate } => {
            (2 | w(id) << 16 | w(gain) << 32 | w(pan as u16) << 48, w(send) | u64::from(rate) << 16)
        }
        Cmd::Bed { bed, level } => (3 | u64::from(bed) << 16 | u64::from(level) << 32, 0),
        Cmd::Volume { master, music, sfx } => (4 | w(master) << 16 | w(music) << 32 | w(sfx) << 48, 0),
        Cmd::Seed(s) => (5, u64::from(s)),
        Cmd::Duck(d) => (6 | u64::from(d) << 16, 0),
    }
}

pub fn unpack(a: u64, b: u64) -> Option<Cmd> {
    let f = |k: u32| (a >> k) as u16;
    Some(match a & 0xff {
        1 => Cmd::Music { song: (a >> 8 & 1 == 1).then_some(f(16)), fade_out_ms: f(32), fade_in_ms: f(48) },
        2 => Cmd::Sfx { id: f(16), gain: f(32), pan: f(48) as i16, send: b as u16, rate: (b >> 16) as u32 },
        3 => Cmd::Bed { bed: (a >> 16) as u8, level: (a >> 32) as u8 },
        4 => Cmd::Volume { master: f(16), music: f(32), sfx: f(48) },
        5 => Cmd::Seed(b as u32),
        6 => Cmd::Duck((a >> 16) as u8),
        _ => return None,
    })
}

/// The groups voices are mixed in: nine places across and eight sends (0 to 7 sevenths).
const PANS: usize = 9;
const SENDS: usize = 8;
const GROUPS: usize = PANS * SENDS;

/// A voice's group: its place to the nearest eighth of the way across, its send to the nearest
/// seventh.
fn group_for(pan: i32, send: i32) -> u8 {
    let p = ((pan.clamp(-Q12, Q12) + Q12) * 4 + Q12 / 2) / Q12;
    let s = (send.clamp(0, Q12) * 7 + Q12 / 2) / Q12;
    (p * SENDS as i32 + s) as u8
}

/// A group's place and send (Q12).
fn group_of(g: u8) -> (i32, i32) {
    let (p, s) = (i32::from(g) / SENDS as i32, i32::from(g) % SENDS as i32);
    (p * Q12 / 4 - Q12, s * Q12 / 7)
}

/// Who a voice belongs to: a song playing, the effects or a bed.
const OWN_SFX: u8 = PLAYERS as u8;
const OWN_BED: u8 = OWN_SFX + 1;
const OWNERS: usize = PLAYERS + 1 + BEDS;

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    sample: u16,
    owner: u8,
    /// The next frame to decode; a run of frames decoded ahead (`dec`, the next one's place in it
    /// and how many it holds), so the mixing loop reads a frame with one compare; the ADPCM
    /// decoder's last two frames.
    i: u32,
    dec: [i16; VAG_FRAMES],
    di: u8,
    dn: u8,
    old: i32,
    older: i32,
    frac: u32,
    step: u32,
    s0: i32,
    s1: i32,
    ended: bool,
    delay: u32,
    /// Frames until let go (`u32::MAX`: held).
    gate: u32,
    /// Q12, with the sample's level and the note's velocity in it.
    amp: i32,
    /// Where it sits (Q12) and how much it sends to the room (Q12), and the group of voices that
    /// sit and send alike (`group`): a voice is mixed mono into its group, and each group panned
    /// and sent once.
    pan: i16,
    send: i16,
    group: u8,
    /// Q16.
    env: u32,
    /// A held voice's attack and decay (`Inst::attack_ms`): how far through the attack (Q16) and
    /// its step a chunk, the decay's level (Q16), its multiplier a chunk and the sustain.
    ramp: u32,
    ramp_step: u32,
    shape: u32,
    decay: u32,
    sustain: u32,
    /// The envelope's multiplier a chunk (Q16): held, the tail's, the release's.
    coef: u32,
    release: u32,
    tail: u32,
    tailing: bool,
    music: bool,
    /// Its sample holds nothing over a quarter of the mixer's rate: it is mixed at half the rate
    /// (half the work) and the half-rate bus is brought up to the full one.
    half: bool,
}

/// The integer reverb: `dsp::Reverb`'s eight lines, pre-delay and diffusers, run at half the
/// mixer's rate (the room is dark, its damping and tone under 6.5 kHz on the PC: the half rate's
/// 5.5 kHz band loses little of it and halves its cost). Q12 coefficients, 32-bit arithmetic.
#[derive(Debug)]
struct Room {
    buf: Vec<i32>,
    /// Per line: its start in `buf`, its length, where it is.
    lines: [(usize, usize, usize); 8],
    gains: [i32; 8],
    damp_a: i32,
    damp: [i32; 8],
    pre: (usize, usize, usize),
    diff: [((usize, usize, usize), i32); 2],
    /// The DC each side carries (Q6), and the last output pair (to interpolate the half rate).
    hp: [i32; 2],
    last: (i32, i32),
}

/// `a * b` for a Q12 `b`, rounded.
#[inline]
fn m12(a: i32, b: i32) -> i32 {
    (a * b + 2048) >> 12
}

impl Room {
    fn new(rate: u32) -> Room {
        // `dsp::Reverb::new(sr, 2.4, 1.25)`; lengths in tenths of a millisecond.
        const TENTHS: [u32; 8] = [311, 373, 419, 473, 531, 599, 673, 731];
        let rate = rate / 2;
        let mut buf_len = 0usize;
        let mut take = |n: usize| {
            let at = buf_len;
            buf_len += n.max(1);
            (at, n.max(1), 0usize)
        };
        let mut lines = [(0, 0, 0); 8];
        let mut gains = [0; 8];
        for (k, t) in TENTHS.iter().enumerate() {
            let n = (u64::from(*t) * 125 * u64::from(rate) / 1_000_000) as usize;
            lines[k] = take(n);
            // 0.001^(n / (2.4 rate)).
            gains[k] = (exp2_neg_q16((653_123u64 * n as u64 * 1000 / (2400 * u64::from(rate))) as u32) >> 4) as i32;
        }
        let pre = take((u64::from(rate) * 18 / 1000) as usize);
        let d0 = take((u64::from(rate) * 71 / 10_000) as usize);
        let d1 = take((u64::from(rate) * 113 / 10_000) as usize);
        // A one-pole's `1 - e^(-2 pi f / sr)`: 2 pi log2(e) * 65536 = 594_089.
        let one_pole =
            |hz: u32| (65_536 - exp2_neg_q16((594_089u64 * u64::from(hz) / u64::from(rate)) as u32)) as i32 >> 4;
        Room {
            buf: alloc::vec![0; buf_len],
            lines,
            gains,
            damp_a: one_pole(6500),
            damp: [0; 8],
            pre,
            diff: [(d0, 2_540), (d1, 2_376)],
            hp: [0; 2],
            last: (0, 0),
        }
    }

    #[inline]
    fn line(buf: &mut [i32], l: &mut (usize, usize, usize), x: i32) -> i32 {
        let at = l.0 + l.2;
        let y = buf[at];
        buf[at] = x;
        l.2 += 1;
        if l.2 == l.1 {
            l.2 = 0;
        }
        y
    }

    #[inline]
    fn peek(buf: &[i32], l: &(usize, usize, usize)) -> i32 {
        buf[l.0 + l.2]
    }

    /// Two frames of the send (mono, 16-bit units) in, two wet pairs out.
    fn run2(&mut self, x0: i32, x1: i32) -> [(i32, i32); 2] {
        let x = Room::line(&mut self.buf, &mut self.pre, (x0 + x1) >> 1);
        let mut x = x;
        for (d, g) in &mut self.diff {
            let z = Room::peek(&self.buf, d);
            let v = x + m12(z, *g);
            Room::line(&mut self.buf, d, v);
            x = z - m12(v, *g);
        }
        let mut outs = [0i32; 8];
        let mut sum = 0;
        for (k, o) in outs.iter_mut().enumerate() {
            let r = Room::peek(&self.buf, &self.lines[k]);
            self.damp[k] += m12(r - self.damp[k], self.damp_a);
            *o = m12(self.damp[k], self.gains[k]);
            sum += *o;
        }
        let k = sum >> 2;
        // 0.35 of the input, signs alternating.
        let xin = m12(x, 1_434);
        for (i, o) in outs.iter().enumerate() {
            let v = if i % 2 == 0 { o - k + xin } else { o - k - xin };
            Room::line(&mut self.buf, &mut self.lines[i], v);
        }
        let wl = outs[0] - outs[2] + outs[4] - outs[6] + ((outs[1] - outs[5]) >> 1);
        let wr = outs[1] - outs[3] + outs[5] - outs[7] + ((outs[2] - outs[6]) >> 1);
        let mut w = [wl, wr];
        for (x, hp) in w.iter_mut().zip(self.hp.iter_mut()) {
            // The DC out (a mean followed at about 3 Hz), then the PC's 0.6, and a tenth more for
            // what the half rate leaves out above 5.5 kHz.
            *hp += ((*x << 6) - *hp) >> 9;
            *x = m12(*x - (*hp >> 6), 2_703);
        }
        let (l0, r0) = self.last;
        self.last = (w[0], w[1]);
        [((l0 + w[0]) >> 1, (r0 + w[1]) >> 1), (w[0], w[1])]
    }
}
/// The mixer: songs, effects and beds into one stereo stream through the room, the volumes, a DC
/// blocker and a look-ahead limiter (`engine::Engine` in integers).
#[derive(Debug)]
pub struct Mixer {
    bank: Bank,
    pub rate: u32,
    seed: u32,
    players: [Option<Player>; PLAYERS],
    current: Option<usize>,
    voices: Vec<Voice>,
    starts: Vec<Start>,
    round: Vec<u32>,
    born: u32,
    /// Per owner, the level a chunk (Q16): songs' fades, effects, beds.
    own: [u32; OWNERS],
    bed_level: [u32; BEDS],
    bed_target: [u32; BEDS],
    /// Master, music, effects: Q16 now and wanted.
    vol: [u32; 3],
    vol_want: [u32; 3],
    duck: u32,
    duck_want: u32,
    room: Room,
    dc: [i32; 2],
    /// The limiter: a block held back, its gain (Q12) and the release a block (Q12).
    held: [i32; 2 * BLOCK],
    held_n: usize,
    lim: u32,
    lim_release: u32,
    /// Coefficients a chunk: the 8 ms steal, each instrument's release, each sample's tail.
    steal: u32,
    inst_release: Vec<u32>,
    sample_tail: Vec<u32>,
    pub now: u64,
    /// The busiest the voices got (`jane audio`'s report, the PSP's log).
    pub peak_voices: usize,
    /// Voice-frames mixed and frames fetched since made (the cost's measure).
    pub mixed: u64,
    /// Every note the next song plays (tests), and the last song's when it played through.
    pub log_notes: bool,
    done_log: Option<Vec<NoteOn>>,
    /// Each instrument's attack step and decay multiplier a chunk (Q16).
    inst_env: Vec<(u32, u32)>,
    bus: [[i32; BLOCK]; 3],
    /// Each group's mono bus, its half-rate bus (`Voice::half`), the last half-rate frame each
    /// held, its gains (left, right, send; Q12), and which groups sounded this block.
    group: Vec<[i32; BLOCK]>,
    hgroup: Vec<[i32; BLOCK / 2]>,
    hlast: Vec<i32>,
    ggain: Vec<(i32, i32, i32)>,
    used: u128,
    /// The far slot: one far sample's frames (a lesson's cue), loaded when wanted.
    slot: Vec<u8>,
    slot_holds: Option<u16>,
}

impl Mixer {
    /// A mixer at `rate` over `bank`, the county's `seed` varying the music. Everything it will
    /// hold is made here: it never allocates again.
    pub fn new(bank: Bank, rate: u32, seed: u32) -> Mixer {
        let rate = rate.max(8000);
        let chunk = CHUNK as u32;
        let inst_release = bank.head.insts.iter().map(|i| fall_q16(i.release_ms.max(6), rate, chunk)).collect();
        let sample_tail = bank
            .head
            .samples
            .iter()
            .map(|s| if s.tail_ms == 0 { ONE16 } else { fall_q16(s.tail_ms, rate, chunk) })
            .collect();
        let round = alloc::vec![0; bank.head.sfx.len()];
        let inst_env = bank
            .head
            .insts
            .iter()
            .map(|i| {
                let frames = (u64::from(i.attack_ms) * u64::from(rate) / 1000).max(1);
                let step = if i.attack_ms == 0 {
                    ONE16
                } else {
                    (u64::from(ONE16) * u64::from(chunk) / frames).clamp(1, u64::from(ONE16)) as u32
                };
                (step, if i.decay_ms == 0 { 0 } else { fall_q16(i.decay_ms, rate, chunk) })
            })
            .collect();
        let slot_len = bank.head.samples.iter().filter(|s| s.far).map(Bank::sample_bytes).max().unwrap_or(0);
        let mut m = Mixer {
            slot: alloc::vec![0; slot_len],
            slot_holds: None,
            rate,
            seed,
            players: [None, None, None],
            current: None,
            voices: Vec::with_capacity(MAX_VOICES),
            starts: Vec::with_capacity(48),
            round,
            born: 0,
            own: [0; OWNERS],
            bed_level: [0; BEDS],
            bed_target: [0; BEDS],
            vol: [ONE16; 3],
            vol_want: [ONE16; 3],
            duck: ONE16,
            duck_want: ONE16,
            room: Room::new(rate),
            dc: [0; 2],
            held: [0; 2 * BLOCK],
            held_n: 0,
            lim: 4096,
            lim_release: (65_536 - fall_q16(1_000, rate, BLOCK as u32).min(65_535)) >> 4,
            steal: fall_q16(8, rate, chunk),
            inst_release,
            sample_tail,
            now: 0,
            peak_voices: 0,
            mixed: 0,
            log_notes: false,
            done_log: None,
            inst_env,
            bus: [[0; BLOCK]; 3],
            group: alloc::vec![[0; BLOCK]; GROUPS],
            hgroup: alloc::vec![[0; BLOCK / 2]; GROUPS],
            hlast: alloc::vec![0; GROUPS],
            ggain: (0..GROUPS)
                .map(|g| {
                    let (p, s) = group_of(g as u8);
                    let (l, r) = pan_gains(p);
                    (l, r, ((l + r) * s) >> 13)
                })
                .collect(),
            used: 0,
            bank,
        };
        // The beds' voices, sleeping until wanted.
        for b in 0..m.bank.head.beds.len().min(BEDS) {
            let bed = m.bank.head.beds[b].clone();
            let owner = OWN_BED + b as u8;
            if bed.wide {
                m.start(bed.sample, owner, 0, ONE16, Q12, -Q12, i32::from(bed.send), u32::MAX, ONE16, 0, None);
                let half = Some((bed.half, bed.half_state));
                m.start(bed.sample, owner, 0, ONE16, Q12, Q12, i32::from(bed.send), u32::MAX, ONE16, 0, half);
            } else {
                m.start(bed.sample, owner, 0, ONE16, Q12, 0, i32::from(bed.send), u32::MAX, ONE16, 0, None);
            }
        }
        m
    }

    pub fn bank(&self) -> &Bank {
        &self.bank
    }

    /// The far sample effect `id` needs loaded first, if it is far and not in the slot: the
    /// sample, and its frames' place and length in the module's file.
    pub fn far_wanted(&self, id: u16) -> Option<(u16, usize, usize)> {
        let sample = self.bank.head.sfx.get(usize::from(id))?.sample;
        let s = &self.bank.head.samples[usize::from(sample)];
        (s.far && self.slot_holds != Some(sample))
            .then(|| (sample, self.bank.far_at() + s.at as usize, Bank::sample_bytes(s)))
    }

    /// Puts a far sample's frames in the slot (what it held stops).
    pub fn load_far(&mut self, sample: u16, frames: &[u8]) -> bool {
        let Some(s) = self.bank.head.samples.get(usize::from(sample)) else { return false };
        if !s.far || frames.len() != Bank::sample_bytes(s) || frames.len() > self.slot.len() {
            return false;
        }
        if let Some(old) = self.slot_holds {
            self.voices.retain(|v| v.sample != old);
        }
        self.slot[..frames.len()].copy_from_slice(frames);
        self.slot_holds = Some(sample);
        true
    }

    /// The bytes the mixer holds: the resident module, the far slot, the room's lines.
    pub fn ram(&self) -> usize {
        self.bank.bytes() + self.slot.len() + self.room.buf.len() * 4
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The notes the current song has played, when `log_notes` was on as it started.
    pub fn note_log(&self) -> Option<&[NoteOn]> {
        let p = self.current.and_then(|c| self.players.iter().flatten().find(|p| p.song == c && p.target > 0));
        p.and_then(|p| p.log.as_deref()).or(self.done_log.as_deref())
    }

    /// Voices sounding now.
    pub fn voices(&self) -> usize {
        self.voices.iter().filter(|v| v.owner < OWN_BED || self.own[usize::from(v.owner)] > 0).count()
    }

    pub fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Music { song, fade_out_ms, fade_in_ms } => {
                let song = song.map(usize::from).filter(|&s| s < self.bank.head.songs.len());
                if song.is_some() && song == self.current {
                    return;
                }
                let rate = self.rate;
                for p in self.players.iter_mut().flatten() {
                    if p.target > 0 {
                        p.fade(0, u32::from(fade_out_ms), rate);
                    }
                }
                self.current = song;
                let Some(s) = song else { return };
                let slot = match self.players.iter().position(Option::is_none) {
                    Some(i) => i,
                    None => {
                        // The quietest gives way at once.
                        let i =
                            (0..PLAYERS).min_by_key(|&i| self.players[i].as_ref().map_or(0, |p| p.gain)).unwrap_or(0);
                        self.drop_player(i);
                        i
                    }
                };
                let mut p = Player::new(s, &self.bank.head.songs[s], self.seed, self.now);
                if self.log_notes {
                    p.log = Some(Vec::new());
                }
                if fade_in_ms > 0 {
                    p.gain = 0;
                    p.fade(1 << 24, u32::from(fade_in_ms), rate);
                }
                self.players[slot] = Some(p);
            }
            Cmd::Sfx { id, gain, pan, send, rate } => {
                if gain == 0 {
                    return;
                }
                let Some(sfx) = self.bank.head.sfx.get(usize::from(id)) else { return };
                let (sample, sfx_send) = (sfx.sample, i32::from(sfx.send));
                let k = self.round[usize::from(id)] as usize % sfx.tunes.len();
                let tune = sfx.tunes[k];
                self.round[usize::from(id)] += 1;
                if self.bank.head.samples[usize::from(sample)].far && self.slot_holds != Some(sample) {
                    // A host holds the far frames; a console's game loads them first.
                    let s = self.bank.head.samples[usize::from(sample)].clone();
                    let Some(frames) = self.bank.far_frames(&s).map(<[u8]>::to_vec) else { return };
                    self.load_far(sample, &frames);
                }
                let count = self.voices.iter().filter(|v| v.owner == OWN_SFX).count();
                if count >= MAX_SFX {
                    // The one furthest through gives way.
                    if let Some(i) = (0..self.voices.len())
                        .filter(|&i| self.voices[i].owner == OWN_SFX)
                        .max_by_key(|&i| self.voices[i].i)
                    {
                        self.voices.swap_remove(i);
                    }
                }
                let rate_q16 = ((u64::from(rate.clamp(16_384, 262_144)) * u64::from(tune)) >> 16) as u32;
                let send = i32::from(send).max(sfx_send);
                self.start(
                    sample,
                    OWN_SFX,
                    0,
                    ONE16,
                    i32::from(gain),
                    i32::from(pan),
                    send,
                    u32::MAX,
                    rate_q16,
                    0,
                    None,
                );
            }
            Cmd::Bed { bed, level } => {
                if let Some(t) = self.bed_target.get_mut(usize::from(bed)) {
                    *t = u32::from(level) * ONE16 / 255;
                }
            }
            Cmd::Volume { master, music, sfx } => {
                let q = |v: u16| u32::from(v).min(4096) << 4;
                self.vol_want = [q(master), q(music), q(sfx)];
            }
            Cmd::Seed(s) => self.seed = s,
            Cmd::Duck(d) => self.duck_want = u32::from(d) * ONE16 / 255,
        }
    }

    fn drop_player(&mut self, i: usize) {
        if let Some(p) = self.players[i].take()
            && p.log.is_some()
            && p.target > 0
        {
            self.done_log = p.log;
        }
        self.voices.retain(|v| usize::from(v.owner) != i);
    }

    /// Starts a voice; with no room the quietest goes.
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        sample: u16,
        owner: u8,
        delay: u32,
        env: u32,
        amp: i32,
        pan: i32,
        send: i32,
        gate: u32,
        step_q16: u32,
        release: u32,
        from: Option<(u32, i32)>,
    ) {
        let Some(s) = self.bank.head.samples.get(usize::from(sample)) else { return };
        if self.voices.len() >= MAX_VOICES {
            let Some(i) = (0..self.voices.len())
                .filter(|&i| self.voices[i].owner < OWN_BED)
                .min_by_key(|&i| (self.voices[i].env >> 8) * self.voices[i].amp.unsigned_abs())
            else {
                return;
            };
            self.voices.swap_remove(i);
        }
        // The sample's own level and rate come in here.
        let lv = |g: i32| ((i64::from(g) * i64::from(s.level)) >> 16) as i32;
        let half = u64::from(s.rate) * 2 <= u64::from(self.rate);
        let at = if half { self.rate / 2 } else { self.rate };
        let step = (u64::from(step_q16) * u64::from(s.rate) / u64::from(at)).clamp(1, 8 << 16) as u32;
        let (i, state) = from.unwrap_or((0, 0));
        self.born = self.born.wrapping_add(1);
        let mut v = Voice {
            sample,
            owner,
            i,
            dec: [0; VAG_FRAMES],
            di: 0,
            dn: 0,
            old: 0,
            older: 0,
            frac: 0,
            step,
            s0: 0,
            s1: 0,
            ended: false,
            delay,
            gate,
            amp: lv(amp),
            pan: pan.clamp(-Q12, Q12) as i16,
            send: send.clamp(0, Q12) as i16,
            group: group_for(pan, send),
            env,
            ramp: ONE16,
            ramp_step: 0,
            shape: ONE16,
            decay: 0,
            sustain: ONE16,
            coef: ONE16,
            release,
            tail: self.sample_tail[usize::from(sample)],
            tailing: false,
            music: owner < OWN_SFX,
            half,
        };
        let frames = if s.far { &self.slot[..] } else { &self.bank.data()[s.at as usize..] };
        seek(&mut v, i, state);
        v.s0 = fetch(frames, s, &mut v);
        v.s1 = fetch(frames, s, &mut v);
        self.voices.push(v);
        self.peak_voices = self.peak_voices.max(self.voices.len());
    }

    /// Fills `out`, interleaved stereo 16-bit.
    pub fn render(&mut self, out: &mut [i16]) {
        for chunk in out.chunks_mut(2 * BLOCK) {
            self.block(chunk);
        }
    }

    fn sequence(&mut self, n: usize) {
        let rate = self.rate;
        for slot in 0..PLAYERS {
            let Some(mut p) = self.players[slot].take() else { continue };
            let song = &self.bank.head.songs[p.song];
            self.starts.clear();
            while !p.ended && p.next_at < self.now + n as u64 {
                let off = p.next_at.saturating_sub(self.now) as u32;
                p.step(song, &self.bank.head.insts, off, rate, &mut self.starts);
                p.steps += 1;
                p.next_at = p.started + Player::step_frames(song, p.steps, rate);
            }
            self.players[slot] = Some(p);
            for k in 0..self.starts.len() {
                let st = self.starts[k];
                self.note(slot, st);
            }
        }
    }

    /// One note of instrument `inst` alone, dry, at `vel` (Q12), let go after `gate` frames, as
    /// an effect (`jane audio render inst:` and the tests).
    pub fn audition(&mut self, inst: usize, midi: i32, vel: u32, gate: u32) {
        self.note(usize::from(OWN_SFX), Start { inst: inst as u8, midi, vel, pan: 0, delay: 0, gate });
        if let Some(v) = self.voices.last_mut() {
            v.send = 0;
            v.group = group_for(i32::from(v.pan), 0);
        }
    }

    /// A note of a song: its zone's sample, pitched, at its velocity, panned.
    fn note(&mut self, slot: usize, st: Start) {
        let Some(inst) = self.bank.head.insts.get(usize::from(st.inst)) else { return };
        let z =
            inst.zones.iter().find(|z| i32::from(z.top) >= st.midi).or(inst.zones.last()).copied().unwrap_or_default();
        let curve = &inst.vel;
        // The velocity curve, between its points (velocity Q12; a point each 1/8).
        let at = (st.vel.min(4096 * (curve.len() as u32 - 1) / 8 - 1) * 8) as usize;
        let (k, f) = ((at >> 12).min(curve.len() - 2), (at & 4095) as u32);
        let amp = (u32::from(curve[k]) * (4096 - f) + u32::from(curve[k + 1]) * f) >> 12;
        let own = usize::try_from(st.midi - i32::from(inst.lo))
            .ok()
            .and_then(|i| inst.gains.get(i))
            .map_or(4096, |&g| u32::from(g));
        let amp = (amp * own) >> 12;
        let send = i32::from(inst.send);
        let release = self.inst_release[usize::from(st.inst)];
        let step = pitch_q16((st.midi - i32::from(z.root)) << 16);
        // Past the music's voices the oldest of them fades in 8 ms.
        let sounding = self.voices.iter().filter(|v| v.music && v.coef != self.steal).count();
        if sounding >= MAX_MUSIC {
            let steal = self.steal;
            let loud = |v: &Voice| ((u64::from(v.env) * u64::from(v.shape)) >> 16) * u64::from(v.amp.unsigned_abs());
            if let Some(v) =
                self.voices.iter_mut().filter(|v| v.music && v.coef != steal && v.delay == 0).min_by_key(|v| loud(v))
            {
                v.coef = steal;
                v.gate = u32::MAX;
            }
        }
        let sustain = u32::from(inst.sustain) << 4;
        let (ramp_step, decay) = self.inst_env[usize::from(st.inst)];
        let held = inst.attack_ms > 0 || inst.decay_ms > 0;
        self.start(z.sample, slot as u8, st.delay, ONE16, amp as i32, st.pan, send, st.gate, step, release, None);
        if held && let Some(v) = self.voices.last_mut() {
            v.ramp = 0;
            v.ramp_step = ramp_step;
            v.shape = 0;
            v.decay = decay;
            v.sustain = sustain;
        }
    }

    /// Owners' levels for the chunk ahead (Q16), the fades moved on `n` frames.
    fn levels(&mut self, n: u32) {
        let [_, vmu, vs] = self.vol;
        let d = self.duck;
        let music = ((u64::from(vmu) * u64::from(d)) >> 16) as u32;
        let beds = ((u64::from(vs) * u64::from(d)) >> 16) as u32;
        for slot in 0..PLAYERS {
            self.own[slot] = match &mut self.players[slot] {
                Some(p) => {
                    p.ramp(n);
                    let song = &self.bank.head.songs[p.song];
                    ((u64::from(p.level(song)) * u64::from(music)) >> 16) as u32
                }
                None => 0,
            };
        }
        self.own[usize::from(OWN_SFX)] = vs;
        // About two seconds from silence to full.
        let ramp = (u64::from(ONE16) * u64::from(n) / (2 * u64::from(self.rate))).max(1) as u32;
        for b in 0..BEDS {
            let (l, t) = (self.bed_level[b], self.bed_target[b]);
            self.bed_level[b] = if l < t { (l + ramp).min(t) } else { l.saturating_sub(ramp).max(t) };
            self.own[usize::from(OWN_BED) + b] = ((u64::from(self.bed_level[b]) * u64::from(beds)) >> 16) as u32;
        }
    }

    fn block(&mut self, out: &mut [i16]) {
        let n = out.len() / 2;
        self.sequence(n);
        for b in &mut self.bus {
            b[..n].fill(0);
        }
        let mut u = self.used;
        while u != 0 {
            let g = u.trailing_zeros() as usize;
            u &= u - 1;
            self.group[g].fill(0);
            self.hgroup[g].fill(0);
        }
        self.used = 0;
        // Volumes and the duck move over a block, as the PC's.
        for k in 0..3 {
            let (v, w) = (self.vol[k] as i64, self.vol_want[k] as i64);
            self.vol[k] = (v + (w - v) / 20) as u32;
        }
        let (d, dw) = (i64::from(self.duck), i64::from(self.duck_want));
        let quarter = i64::from(self.rate) / 4;
        self.duck = (d + (dw - d) * (n as i64).min(quarter) / quarter.max(1)) as u32;
        let mut k0 = 0;
        while k0 < n {
            let k1 = (k0 + CHUNK).min(n);
            self.levels((k1 - k0) as u32);
            self.chunk(k0, k1);
            k0 = k1;
        }
        // Songs played through or faded out, and their voices.
        for slot in 0..PLAYERS {
            let done = self.players[slot].as_ref().is_some_and(|p| {
                (p.gain == 0 && p.target == 0) || (p.ended && !self.voices.iter().any(|v| usize::from(v.owner) == slot))
            });
            if done {
                self.drop_player(slot);
            }
        }
        // Each group that sounded: its half-rate bus up to the full rate (each odd frame halfway
        // between its neighbours, half a frame late, which nothing hears), then panned and sent.
        let mut u = self.used;
        let [bl, br, bs] = &mut self.bus;
        while u != 0 {
            let g = u.trailing_zeros() as usize;
            u &= u - 1;
            let (h, last) = (&self.hgroup[g], &mut self.hlast[g]);
            let m = &mut self.group[g];
            for (k, x) in m[..n].iter_mut().enumerate() {
                let j = k / 2;
                let prev = if j == 0 { *last } else { h[j - 1] };
                *x += if k % 2 == 0 { (prev + h[j]) >> 1 } else { h[j] };
            }
            *last = h[(n / 2).max(1) - 1];
            let (gl, gr, gs) = self.ggain[g];
            for (((x, l), r), s) in m[..n].iter().zip(bl.iter_mut()).zip(br.iter_mut()).zip(bs.iter_mut()) {
                *l += (x * gl) >> 12;
                *r += (x * gr) >> 12;
                *s += (x * gs) >> 12;
            }
        }
        self.mix_out(out, n);
        self.now += n as u64;
    }

    fn chunk(&mut self, k0: usize, k1: usize) {
        let data = &self.bank.bytes[self.bank.data..];
        let slot = &self.slot[..];
        let samples = &self.bank.head.samples;
        let (groups, hgroups) = (&mut self.group, &mut self.hgroup);
        let used = &mut self.used;
        let mixed = &mut self.mixed;
        let own = &self.own;
        self.voices.retain_mut(|v| {
            let lvl = own[usize::from(v.owner)];
            if v.owner >= OWN_BED && lvl == 0 {
                // A sleeping bed keeps its place.
                return true;
            }
            let mut k = k0;
            if v.delay > 0 {
                let d = (v.delay as usize).min(k1 - k0);
                v.delay -= d as u32;
                k += d;
                if k >= k1 {
                    return true;
                }
            }
            let s = &samples[usize::from(v.sample)];
            let frames = if s.far { slot } else { &data[s.at as usize..] };
            if v.gate != u32::MAX {
                let n = (k1 - k) as u32;
                if v.gate <= n {
                    v.gate = u32::MAX;
                    v.coef = v.coef.min(v.release);
                } else {
                    v.gate -= n;
                }
            }
            if !v.tailing && s.loop_len > 0 && v.i > s.loop_start && v.tail < ONE16 {
                v.tailing = true;
                v.coef = v.coef.min(v.tail);
            }
            if v.gate == u32::MAX && v.coef < ONE16 {
                // Let go: the attack and decay stop where they are (the release falls from there).
            } else if v.ramp < ONE16 {
                // The attack: a raised curve over a straight ramp, as the PC's.
                v.ramp = (v.ramp + v.ramp_step).min(ONE16);
                let x = u64::from(v.ramp);
                v.shape = ((((x * x) >> 16) * (3 * u64::from(ONE16) - 2 * x)) >> 16) as u32;
            } else if v.shape != v.sustain && v.decay > 0 {
                let (s, l) = (i64::from(v.sustain), i64::from(v.shape));
                v.shape = (s + (((l - s) * i64::from(v.decay)) >> 16)) as u32;
            }
            let e = (u64::from(v.env) * u64::from(v.shape)) >> 16;
            let m = ((e * u64::from(lvl)) >> 20) as i32;
            let g = (v.amp * m) >> 12;
            if g == 0 && v.coef < ONE16 {
                // Fallen under the last bit: it is over.
                return false;
            }
            *used |= 1 << v.group;
            let (mut s0, mut s1, mut frac) = (v.s0, v.s1, v.frac);
            let step = v.step;
            let out = if v.half {
                &mut hgroups[usize::from(v.group)][k.div_ceil(2)..k1 / 2]
            } else {
                &mut groups[usize::from(v.group)][k..k1]
            };
            *mixed += out.len() as u64;
            for o in out {
                let x = s0 + (((s1 - s0) * (frac >> 2) as i32) >> 14);
                *o += (x * g) >> 12;
                frac += step;
                while frac >= ONE16 {
                    frac -= ONE16;
                    s0 = s1;
                    s1 = fetch(frames, s, v);
                }
            }
            v.s0 = s0;
            v.s1 = s1;
            v.frac = frac;
            // At least a step a chunk, so a slow fall reaches the floor.
            let fallen = ((u64::from(v.env) * u64::from(v.coef)) >> 16) as u32;
            v.env = if v.coef < ONE16 { fallen.min(v.env.saturating_sub(1)) } else { fallen };
            !(v.env < 16 || (v.ended && s0 == 0 && s1 == 0))
        });
    }

    fn mix_out(&mut self, out: &mut [i16], n: usize) {
        let [bl, br, bs] = &self.bus;
        let master = (self.vol[0] >> 4) as i32;
        let mut fresh = [0i32; 2 * BLOCK];
        let mut peak = 0i32;
        let mut k = 0;
        while k < n {
            let x1 = if k + 1 < n { bs[k + 1] } else { bs[k] };
            let wet = self.room.run2(bs[k], x1);
            for (j, (wl, wr)) in wet.into_iter().enumerate() {
                let kk = k + j;
                if kk >= n {
                    break;
                }
                let l = m12(bl[kk] + wl, master);
                let r = m12(br[kk] + wr, master);
                // The DC out: a mean followed at about 3 Hz (Q6, so it falls to zero).
                self.dc[0] += ((l << 6) - self.dc[0]) >> 10;
                self.dc[1] += ((r << 6) - self.dc[1]) >> 10;
                let (l, r) = (l - (self.dc[0] >> 6), r - (self.dc[1] >> 6));
                fresh[2 * kk] = l;
                fresh[2 * kk + 1] = r;
                peak = peak.max(l.abs()).max(r.abs());
            }
            k += 2;
        }
        // The look-ahead limiter: the block before goes out under a gain (Q12) that reaches, by
        // its end, what this block's peak needs.
        let want = if peak > CEILING { (CEILING * 4096 / peak) as u32 } else { 4096 };
        let from = self.lim;
        let to = if want < from {
            want
        } else {
            from + (((4096 - from.min(4096)) * self.lim_release) >> 12).max(1).min(4096 - from.min(4096))
        };
        let held_n = self.held_n.max(1) as i32;
        let (f, d) = (from as i32, to as i32 - from as i32);
        for (k, pair) in out.chunks_exact_mut(2).enumerate() {
            let g = f + d * (k as i32 + 1) / held_n;
            for (c, o) in pair.iter_mut().enumerate() {
                let x = if k < self.held_n { self.held[2 * k + c] } else { 0 };
                *o = ((x * g) >> 12).clamp(-CEILING, CEILING) as i16;
            }
        }
        self.lim = to;
        self.held[..2 * n].copy_from_slice(&fresh[..2 * n]);
        self.held_n = n;
    }
}
/// A companded byte back to a 16-bit frame (G.711's mu-law, in integers).
#[inline]
pub fn mu8_decode(u: u8) -> i32 {
    let u = !u;
    let exp = u32::from(u >> 4 & 7);
    let mag = (((i32::from(u & 15) << 3) + 0x84) << exp) - 0x84;
    if u & 0x80 != 0 { -mag } else { mag }
}

/// The byte whose decoding lands nearest `x`.
pub fn mu8_encode(x: i16) -> u8 {
    (0..=255u8).min_by_key(|&u| (mu8_decode(u) - i32::from(x)).unsigned_abs()).unwrap_or(0xff)
}

/// The voice's next frame (0 past the end of a sample that does not loop); `frames` start at
/// the sample's first.
#[inline]
fn fetch(frames: &[u8], s: &Sample, v: &mut Voice) -> i32 {
    if v.di < v.dn {
        let y = v.dec[usize::from(v.di)];
        v.di += 1;
        return i32::from(y);
    }
    refill(frames, s, v)
}

/// Decodes the voice's next run (to its block's end, its loop's end or the sample's) and
/// returns its first frame.
#[inline(never)]
fn refill(frames: &[u8], s: &Sample, v: &mut Voice) -> i32 {
    if s.loop_len > 0 && v.i >= s.loop_start + s.loop_len {
        seek(v, s.loop_start, s.loop_state);
    }
    if v.i >= s.len {
        v.ended = true;
        v.dn = 0;
        return 0;
    }
    let end = if s.loop_len > 0 { s.loop_start + s.loop_len } else { s.len };
    let k = (v.i % VAG_FRAMES as u32) as usize;
    let count = (VAG_FRAMES - k).min((end - v.i) as usize);
    let at = v.i as usize;
    match s.codec {
        Codec::Adpcm => vag_run(frames, at / VAG_FRAMES * VAG_BYTES, k, count, &mut v.old, &mut v.older, &mut v.dec),
        Codec::Pcm16 => {
            for (j, d) in v.dec[..count].iter_mut().enumerate() {
                let p = 2 * (at + j);
                *d = frames.get(p..p + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
            }
        }
        Codec::Mu8 => {
            for (j, d) in v.dec[..count].iter_mut().enumerate() {
                *d = frames.get(at + j).map_or(0, |&u| mu8_decode(u) as i16);
            }
        }
    }
    v.i += count as u32;
    v.di = 1;
    v.dn = count as u8;
    i32::from(v.dec[0])
}

/// Puts a voice at frame `i` of its sample, the decoder holding `state` (packed) before it.
fn seek(v: &mut Voice, i: u32, state: i32) {
    v.i = i;
    (v.old, v.older) = vag_unpack(state);
    v.di = 0;
    v.dn = 0;
}

/// Decodes `count` frames of the block at byte `at`, from its frame `k0`, into `out`.
#[inline]
fn vag_run(
    frames: &[u8],
    at: usize,
    k0: usize,
    count: usize,
    old: &mut i32,
    older: &mut i32,
    out: &mut [i16; VAG_FRAMES],
) {
    let Some(b) = frames.get(at..at + VAG_BYTES) else {
        out.fill(0);
        return;
    };
    let head = b[0];
    let shift = u32::from(head & 15).min(12);
    let (f0, f1) = FILTERS[usize::from(head >> 4).min(4)];
    let (mut o, mut oo) = (*old, *older);
    let mut one = |nib: u8| {
        let s = (i32::from(nib) << 28 >> 16) >> shift;
        let y = (s + ((o * f0 + oo * f1 + 32) >> 6)).clamp(-32_768, 32_767);
        oo = o;
        o = y;
        y as i16
    };
    // A byte holds two frames: the run's odd first frame alone, then pairs, then an odd last.
    let mut j = 0;
    let mut k = k0;
    if k % 2 == 1 && j < count {
        out[j] = one(b[1 + k / 2] >> 4);
        j += 1;
        k += 1;
    }
    while j + 1 < count {
        let byte = b[1 + k / 2];
        out[j] = one(byte & 15);
        out[j + 1] = one(byte >> 4);
        j += 2;
        k += 2;
    }
    if j < count {
        out[j] = one(b[1 + k / 2] & 15);
    }
    (*old, *older) = (o, oo);
}
/// A WAV header for `frames` of 16-bit `channels` at `rate` (the PSP's capture writes it).
pub fn wav_header(frames: u32, channels: u16, rate: u32) -> [u8; 44] {
    let data_len = frames * u32::from(channels) * 2;
    let mut h = [0u8; 44];
    let mut put = |at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
    put(0, b"RIFF");
    put(4, &(36 + data_len).to_le_bytes());
    put(8, b"WAVEfmt ");
    put(16, &16u32.to_le_bytes());
    put(20, &1u16.to_le_bytes());
    put(22, &channels.to_le_bytes());
    put(24, &rate.to_le_bytes());
    put(28, &(rate * u32::from(channels) * 2).to_le_bytes());
    put(32, &(channels * 2).to_le_bytes());
    put(34, &16u16.to_le_bytes());
    put(36, b"data");
    put(40, &data_len.to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vag_follows_a_tone_closely_and_marks_its_state() {
        let x: Vec<i16> = (0..4000).map(|i| (((i * 37 % 200) - 100) * 150) as i16).collect();
        let (bytes, st) = vag_encode(&x, &[1000]);
        assert_eq!(bytes.len(), 4000usize.div_ceil(VAG_FRAMES) * VAG_BYTES);
        let (mut o, mut oo) = (0, 0);
        let (mut sig, mut err) = (0f64, 0f64);
        for (i, &s) in x.iter().enumerate() {
            if i == 1000 {
                assert_eq!(st[0], vag_pack(o, oo));
            }
            let (b, k) = (i / VAG_FRAMES * VAG_BYTES, i % VAG_FRAMES);
            let n = bytes[b + 1 + k / 2];
            let y = vag_decode(bytes[b], if k % 2 == 0 { n & 15 } else { n >> 4 }, &mut o, &mut oo);
            sig += f64::from(s) * f64::from(s);
            err += f64::from(y - i32::from(s)).powi(2);
        }
        assert!(sig / err > 100.0, "snr {}", 10.0 * (sig / err).log10());
    }

    #[test]
    fn mu8_round_trips_within_its_step() {
        for x in [-32_000i16, -1000, -3, 0, 5, 700, 31_000] {
            let y = mu8_decode(mu8_encode(x));
            assert!((y - i32::from(x)).abs() <= i32::from(x).abs() / 16 + 8, "{x} -> {y}");
        }
    }
    #[test]
    fn the_integer_curves_hold() {
        assert_eq!(exp2_neg_q16(0), 65_536);
        assert!((i64::from(exp2_neg_q16(65_536)) - 32_768).abs() < 60);
        assert!((i64::from(pitch_q16(12 << 16)) - 131_072).abs() < 200);
        assert!((i64::from(pitch_q16(-12 << 16)) - 32_768).abs() < 60);
        assert!((i64::from(pitch_q16(7 << 16)) - 98_193).abs() < 150, "{}", pitch_q16(7 << 16));
        let (l, r) = pan_gains(0);
        assert!((l - 2896).abs() < 8 && (r - 2896).abs() < 8);
        assert_eq!(pan_gains(-Q12), (4096, 0));
        // 1 s at 1000 ms falls 60 dB.
        let c = fall_q16(1000, 1000, 1000);
        assert!(c > 60 && c < 72, "{c}");
    }
}

#[cfg(all(test, feature = "std"))]
mod room_tests {
    #[test]
    fn the_room_answers_as_loud_as_the_pcs() {
        let rate = 22_050u32;
        let mut pc = crate::dsp::Reverb::new(rate as f32, 2.4, 1.25);
        let mut room = super::Room::new(rate);
        let mut rng = crate::dsp::Rng::new(3);
        let (mut a, mut b) = (0f64, 0f64);
        let mut held = 0;
        for k in 0..(rate * 4) {
            let x = if k < rate { rng.bi() * 0.25 } else { 0.0 };
            let (l, r) = pc.run(x, x);
            a += f64::from(l * l + r * r);
            let xi = (x * 32_767.0) as i32;
            if k % 2 == 0 {
                held = xi;
                continue;
            }
            for (l, r) in room.run2(held, xi) {
                b += (f64::from(l) / 32_767.0).powi(2) + (f64::from(r) / 32_767.0).powi(2);
            }
        }
        let db = 10.0 * (b / a).log10();
        assert!(db.abs() < 1.0, "the room is {db:+.2} dB against the PC's");
    }
}
