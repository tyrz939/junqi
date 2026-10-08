//! The detailed capture (PORT.md §13.13): L + R + START on a PSP records ten seconds of frames,
//! every part of each, and writes them to `capture-<n>.bin` on the Memory Stick for
//! `jane psp-profile` to read on a PC. Pure and integer: the format is written by the PSP glue
//! and read on the host, and a test reads back what it wrote.
//!
//! The file is self-describing, so the tool reads a capture from any build:
//!
//! ```text
//! "JCAP" u16 version, u16 0
//! u32 header bytes, then "key value\n" lines (UTF-8): build, settings, place, clocks
//! u16 frame fields, u16 GE passes, then each name as u8 length + bytes (fields, then passes)
//! u32 frames, then each frame as (fields + passes) u32 little-endian
//! ```
//!
//! A frame's GE passes are microseconds from the GE's signals ([`pass`]): the time between the
//! GE reaching one pass's first command and the next pass's.

use alloc::string::String;
use alloc::vec::Vec;

/// The format's magic and version.
pub const MAGIC: &[u8; 4] = b"JCAP";
pub const VERSION: u16 = 1;

/// The GE passes a frame is cut into ([`crate::list::Lister::marks`]): their index is the mark's
/// id, the name what the profile prints.
pub mod pass {
    pub const CLEAR: u8 = 0;
    pub const SKY: u8 = 1;
    pub const TERRAIN: u8 = 2;
    pub const SPRITES: u8 = 3;
    pub const SUN_SHADOWS: u8 = 4;
    pub const LIGHTMAP: u8 = 5;
    pub const LIGHT_MUL: u8 = 6;
    pub const GLOW: u8 = 7;
    pub const SHAFTS: u8 = 8;
    pub const WATER_FX: u8 = 9;
    pub const FOG: u8 = 10;
    pub const PARTICLES: u8 = 11;
    pub const SHIMMER: u8 = 12;
    pub const SATURATION: u8 = 13;
    pub const GRADE: u8 = 14;
    pub const UI: u8 = 15;
    pub const RELIEF: u8 = 16;
    /// How many there are.
    pub const N: usize = 17;
    /// Their names, by id.
    pub const NAMES: [&str; N] = [
        "clear",
        "sky",
        "terrain",
        "sprites",
        "sun_shadows",
        "lightmap",
        "light_mul",
        "glow",
        "shafts",
        "water_fx",
        "fog",
        "particles",
        "shimmer",
        "saturation",
        "grade",
        "ui",
        "relief",
    ];
}

/// A frame's fields, by index into [`FIELDS`].
pub mod field {
    /// The frame's span, top of one frame to the top of the next.
    pub const FRAME: usize = 0;
    pub const SIM: usize = 1;
    pub const TICK: usize = 2;
    pub const BUFS: usize = 3;
    /// The presenter's tick by part (summed over the frame's ticks): `Present::prof`'s order.
    pub const T_UNITS: usize = 4;
    pub const T_EMOTES: usize = 5;
    pub const T_PAINT: usize = 6;
    pub const T_WALLS: usize = 7;
    pub const T_SKY: usize = 8;
    pub const T_FX: usize = 9;
    pub const T_AMBIENT: usize = 10;
    pub const T_HEAD: usize = 11;
    pub const T_PROPS: usize = 12;
    pub const T_LIGHTS: usize = 13;
    pub const DRAW: usize = 14;
    pub const UI: usize = 15;
    pub const LIST: usize = 16;
    pub const GE_BUILD: usize = 17;
    pub const GE_WAIT: usize = 18;
    pub const VBLANK: usize = 19;
    pub const AUDIO: usize = 20;
    /// The GE's own time for the list, its first signal to its last.
    pub const GE_TOTAL: usize = 21;
    pub const QUADS: usize = 22;
    pub const BATCHES: usize = 23;
    pub const BINDS: usize = 24;
    pub const CLUT_LOADS: usize = 25;
    pub const MODES: usize = 26;
    pub const LIGHTS: usize = 27;
    pub const CASTERS: usize = 28;
    pub const SLABS: usize = 29;
    pub const PARTICLES: usize = 30;
    pub const PAINTED: usize = 31;
    pub const LANDED: usize = 32;
    pub const PAGE_LOADS: usize = 33;
    pub const UPLOADS: usize = 34;
    pub const FREE: usize = 35;
    pub const LARGEST: usize = 36;
    pub const TICKS: usize = 37;
    /// 1 when the frame took longer than its budget (a vblank missed).
    pub const LATE: usize = 38;
    /// The capture's own cost this frame (its bookkeeping).
    pub const OVERHEAD: usize = 39;
    pub const N: usize = 40;
}

/// The fields' names, by index (microseconds unless the name says).
pub const FIELDS: [&str; field::N] = [
    "frame",
    "sim",
    "tick",
    "bufs",
    "t_units",
    "t_emotes",
    "t_paint",
    "t_walls",
    "t_sky",
    "t_fx",
    "t_ambient",
    "t_head",
    "t_props",
    "t_lights",
    "draw",
    "ui",
    "list",
    "ge_build",
    "ge_wait",
    "vblank",
    "audio",
    "ge_total",
    "n_quads",
    "n_batches",
    "n_binds",
    "n_clut_loads",
    "n_modes",
    "n_lights",
    "n_casters",
    "n_slabs",
    "n_particles",
    "n_painted",
    "n_landed",
    "n_page_loads",
    "n_uploads",
    "free_bytes",
    "largest_bytes",
    "n_ticks",
    "late",
    "overhead",
];

/// One frame's words: its fields, then its passes.
pub const WORDS: usize = field::N + pass::N;

/// A capture being recorded: the frames in a buffer reserved at its start (no allocation while
/// it runs), at most `most`.
#[derive(Debug, Default)]
pub struct Recorder {
    pub header: String,
    words: Vec<u32>,
    most: usize,
}

impl Recorder {
    /// Room for `frames` frames (`WORDS * 4` bytes each); `None` if the RAM is not there.
    pub fn new(header: String, frames: usize) -> Option<Recorder> {
        let mut words = Vec::new();
        words.try_reserve_exact(frames * WORDS).ok()?;
        Some(Recorder { header, words, most: frames })
    }

    pub fn frames(&self) -> usize {
        self.words.len() / WORDS
    }

    pub fn full(&self) -> bool {
        self.frames() >= self.most
    }

    /// One frame's fields and passes; false (nothing kept) when full.
    pub fn push(&mut self, fields: &[u32; field::N], passes: &[u32; pass::N]) -> bool {
        if self.full() {
            return false;
        }
        self.words.extend_from_slice(fields);
        self.words.extend_from_slice(passes);
        true
    }

    /// The file's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.header.len() + self.words.len() * 4 + 1024);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.header.len() as u32).to_le_bytes());
        out.extend_from_slice(self.header.as_bytes());
        out.extend_from_slice(&(field::N as u16).to_le_bytes());
        out.extend_from_slice(&(pass::N as u16).to_le_bytes());
        for n in FIELDS.iter().chain(pass::NAMES.iter()) {
            out.push(n.len() as u8);
            out.extend_from_slice(n.as_bytes());
        }
        out.extend_from_slice(&(self.frames() as u32).to_le_bytes());
        for w in &self.words[..self.frames() * WORDS] {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out
    }
}

/// A capture read back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capture {
    pub version: u16,
    /// The header's `key value` lines, in order.
    pub header: Vec<(String, String)>,
    pub fields: Vec<String>,
    pub passes: Vec<String>,
    /// Each frame's fields, then its passes.
    pub frames: Vec<Vec<u32>>,
}

/// Why a capture could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureError(pub &'static str);

impl Capture {
    pub fn decode(b: &[u8]) -> Result<Capture, CaptureError> {
        let mut at = 0usize;
        let mut take = |n: usize| -> Result<&[u8], CaptureError> {
            let s = b.get(at..at + n).ok_or(CaptureError("short"))?;
            at += n;
            Ok(s)
        };
        if take(4)? != MAGIC {
            return Err(CaptureError("not a capture"));
        }
        let u16_ = |s: &[u8]| u16::from_le_bytes([s[0], s[1]]);
        let u32_ = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        let version = u16_(take(2)?);
        if version == 0 || version > VERSION {
            return Err(CaptureError("unknown version"));
        }
        take(2)?;
        let hn = u32_(take(4)?) as usize;
        let text = core::str::from_utf8(take(hn)?).map_err(|_| CaptureError("header not text"))?;
        let header = text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let (k, v) = l.split_once(' ').unwrap_or((l, ""));
                (String::from(k), String::from(v))
            })
            .collect();
        let nf = usize::from(u16_(take(2)?));
        let np = usize::from(u16_(take(2)?));
        let mut names = Vec::with_capacity(nf + np);
        for _ in 0..nf + np {
            let n = usize::from(take(1)?[0]);
            names.push(String::from(core::str::from_utf8(take(n)?).map_err(|_| CaptureError("name not text"))?));
        }
        let passes = names.split_off(nf);
        let frames_n = u32_(take(4)?) as usize;
        let words = nf + np;
        let body = take(frames_n * words * 4)?;
        let frames = body.chunks_exact(words * 4).map(|f| f.chunks_exact(4).map(u32_).collect()).collect();
        Ok(Capture { version, header, fields: names, passes, frames })
    }

    /// A field's index by name.
    pub fn field(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f == name)
    }

    /// A header value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.header.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_reads_back_as_written() {
        let mut r = Recorder::new(String::from("build abc123\nseed 1\nzone county\n"), 3).unwrap();
        let mut f = [0u32; field::N];
        let mut p = [0u32; pass::N];
        for k in 0..4u32 {
            f[field::FRAME] = 16_000 + k;
            f[field::FREE] = 3_000_000;
            p[usize::from(pass::GRADE)] = 900 + k;
            let kept = r.push(&f, &p);
            assert_eq!(kept, k < 3, "a full capture keeps nothing more");
        }
        let c = Capture::decode(&r.encode()).unwrap();
        assert_eq!(c.version, VERSION);
        assert_eq!(c.get("build"), Some("abc123"));
        assert_eq!(c.get("zone"), Some("county"));
        assert_eq!(c.fields.len(), field::N);
        assert_eq!(c.passes[usize::from(pass::GRADE)], "grade");
        assert_eq!(c.frames.len(), 3);
        assert_eq!(c.frames[2][c.field("frame").unwrap()], 16_002);
        assert_eq!(c.frames[1][field::N + usize::from(pass::GRADE)], 901);
        assert!(Capture::decode(&r.encode()[..20]).is_err());
        assert!(Capture::decode(b"nope").is_err());
    }
}
