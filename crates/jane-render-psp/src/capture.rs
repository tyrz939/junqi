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
    /// Sub-passes (PLAN-PSP-SPEED.md Q0): a breakdown of the pass named before the dot, whose own
    /// time still holds them all (`Lister::sub_marks`; a profile does not add them again).
    pub const SUN_CLEAR: u8 = 17;
    pub const SUN_RAISED: u8 = 18;
    pub const SUN_POLYS: u8 = 19;
    pub const SUN_BANDS: u8 = 20;
    pub const WF_CLEAR: u8 = 21;
    pub const WF_WET: u8 = 22;
    pub const WF_MARKS: u8 = 23;
    pub const WF_SKY: u8 = 24;
    pub const WF_CRESTS: u8 = 25;
    pub const WF_MIRROR: u8 = 26;
    pub const WF_STREAKS: u8 = 27;
    /// How many there are.
    pub const N: usize = 28;
    /// The pass a sub-pass is part of (`None` for a pass of its own).
    pub const fn parent(id: u8) -> Option<u8> {
        match id {
            SUN_CLEAR..=SUN_BANDS => Some(SUN_SHADOWS),
            WF_CLEAR..=WF_STREAKS => Some(WATER_FX),
            _ => None,
        }
    }
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
        "sun_shadows.clear",
        "sun_shadows.raised",
        "sun_shadows.polys",
        "sun_shadows.bands",
        "water_fx.clear",
        "water_fx.wet",
        "water_fx.marks",
        "water_fx.sky",
        "water_fx.crests",
        "water_fx.mirror",
        "water_fx.streaks",
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
    /// 1 when the frame's flip took more vblanks than the pacing's target (a vblank missed): 3 or
    /// more at a locked 30, 2 or more at 60. Before v0.4.1 it was the loop period over budget.
    pub const LATE: usize = 38;
    /// The capture's own cost this frame (its bookkeeping).
    pub const OVERHEAD: usize = 39;
    // Suspect events (`ev_`, and the counts beside them): what a glitch is matched against
    // (`jane psp-profile` marks a frame with any `ev_` field set).
    /// Chunks on screen still the stand-in's swatches.
    pub const EV_PLACEHOLDERS: usize = 40;
    /// Swatches painted into slots this frame (a chunk new to the view).
    pub const EV_ROUGH: usize = 41;
    /// Chunks the painter's thread landed this frame.
    pub const EV_LANDED: usize = 42;
    /// A view of swatches painted at once (a leap).
    pub const EV_VIEW_JUMP: usize = 43;
    /// A chunk's paint waited for the GE (the fence: a slot the last list read).
    pub const EV_FENCE: usize = 44;
    /// Pages the RAM cache let go, and VRAM slots refilled.
    pub const EV_EVICT: usize = 45;
    pub const EV_SLOT_UPLOAD: usize = 46;
    /// Lamp pools built, and uploaded to VRAM.
    pub const EV_LAMP_BUILD: usize = 47;
    pub const EV_LAMP_UPLOAD: usize = 48;
    /// Render-target switches (the lightmap's), stencil-mode changes.
    pub const EV_RT: usize = 49;
    pub const EV_STENCIL: usize = 50;
    /// The GE's sync said something other than done (its value, as `u32`).
    pub const EV_GE_ERROR: usize = 51;
    /// Data-cache write-backs before and in the list: calls and kilobytes.
    pub const EV_WB: usize = 52;
    pub const EV_WB_KB: usize = 53;
    /// A resume from sleep, an audio underrun.
    pub const EV_RESUME: usize = 54;
    pub const EV_UNDERRUN: usize = 55;
    /// Chunks converted or rebound with a new generation this frame.
    pub const EV_CHUNK_NEW: usize = 56;
    /// Vblanks the frame's flip took since the flip before (the display's count: 2 on a steady
    /// 30, 1 at 60), the display list's size in KB, and times the list guard cut the world's
    /// batches short (`ge::DrawStats::list_full`). Added in v0.4.1: an older capture has 57
    /// fields and none of these (the format is self-describing, `VERSION` stays 1).
    pub const VBLANKS: usize = 57;
    pub const LIST_KB: usize = 58;
    pub const EV_LIST_FULL: usize = 59;
    /// The audio thread's slowest block this frame (us), and the blocks it mixed (Q0/Q7: one slow
    /// block against catch-up after a stall). Added after v0.5: older captures lack them.
    pub const AUDIO_WORST: usize = 60;
    pub const AUDIO_BLOCKS: usize = 61;
    pub const N: usize = 62;
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
    "ev_placeholders",
    "ev_rough",
    "ev_landed",
    "ev_view_jump",
    "ev_fence",
    "ev_evict",
    "ev_slot_upload",
    "ev_lamp_build",
    "ev_lamp_upload",
    "n_rt",
    "n_stencil",
    "ev_ge_error",
    "n_wb",
    "wb_kb",
    "ev_resume",
    "ev_underrun",
    "ev_chunk_new",
    "vblanks",
    "list_kb",
    "ev_list_full",
    "audio_worst",
    "n_audio_blocks",
];

/// One frame's words: its fields, then its passes.
pub const WORDS: usize = field::N + pass::N;

/// The dashcam (PORT.md §13.13): the last `most` frames always kept in a ring reserved once (no
/// allocation after), the oldest overwritten; [`Recorder::encode`] writes them oldest first.
#[derive(Debug, Default)]
pub struct Recorder {
    words: Vec<u32>,
    most: usize,
    /// The next frame's place in the ring, and frames held.
    head: usize,
    len: usize,
    /// Frames pushed since it began.
    pub pushed: u32,
}

impl Recorder {
    /// Room for `frames` frames (`WORDS * 4` bytes each); `None` if the RAM is not there.
    pub fn new(frames: usize) -> Option<Recorder> {
        let mut words = Vec::new();
        words.try_reserve_exact(frames * WORDS).ok()?;
        words.resize(frames * WORDS, 0);
        Some(Recorder { words, most: frames, head: 0, len: 0, pushed: 0 })
    }

    /// Frames held (at most the ring's).
    pub fn frames(&self) -> usize {
        self.len
    }

    /// The ring's bytes.
    pub fn bytes(&self) -> usize {
        self.words.len() * 4
    }

    /// One frame's fields and passes, over the oldest when full.
    pub fn push(&mut self, fields: &[u32; field::N], passes: &[u32; pass::N]) {
        if self.most == 0 {
            return;
        }
        let at = self.head * WORDS;
        self.words[at..at + field::N].copy_from_slice(fields);
        self.words[at + field::N..at + WORDS].copy_from_slice(passes);
        self.head = (self.head + 1) % self.most;
        self.len = (self.len + 1).min(self.most);
        self.pushed = self.pushed.wrapping_add(1);
    }

    /// The file's bytes: `header`, then the frames held, oldest first.
    pub fn encode(&self, header: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + header.len() + self.len * WORDS * 4 + 1024);
        self.write_encoded(&mut out, header, self.len);
        out
    }

    /// The file's bytes for the newest `last` frames held (all of them when `last` is at least
    /// [`Self::frames`]), oldest first. Reserves the exact size and returns `None` when that
    /// allocation fails, so a bench can skip the capture and go on instead of aborting.
    pub fn encode_last(&self, header: &str, last: usize) -> Option<Vec<u8>> {
        let n = last.min(self.len);
        let mut out = Vec::new();
        out.try_reserve_exact(encoded_len(header, n)).ok()?;
        self.write_encoded(&mut out, header, n);
        Some(out)
    }

    /// `header` and the `n` newest frames, appended. `n` is at most the frames held.
    fn write_encoded(&self, out: &mut Vec<u8>, header: &str, n: usize) {
        let before = out.len();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&(field::N as u16).to_le_bytes());
        out.extend_from_slice(&(pass::N as u16).to_le_bytes());
        for name in FIELDS.iter().chain(pass::NAMES.iter()) {
            out.push(name.len() as u8);
            out.extend_from_slice(name.as_bytes());
        }
        out.extend_from_slice(&(n as u32).to_le_bytes());
        if n > 0 {
            let first = (self.head + self.most - n) % self.most;
            for k in 0..n {
                let at = (first + k) % self.most * WORDS;
                for w in &self.words[at..at + WORDS] {
                    out.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
        debug_assert_eq!(out.len() - before, encoded_len(header, n));
    }
}

/// Bytes of a capture with `n` frames and this header.
fn encoded_len(header: &str, n: usize) -> usize {
    let names = FIELDS.iter().chain(pass::NAMES.iter()).map(|name| 1 + name.len()).sum::<usize>();
    4 + 2 + 2 + 4 + header.len() + 2 + 2 + names + 4 + n * WORDS * 4
}

/// The bench's per-part summary: a fixed histogram, so a whole scene fits where the ring cannot.
pub mod summary {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// The keys, in line order. Times are µs per frame; "audio" is the audio thread's share of the frame.
    pub const KEYS: [&str; 15] = [
        "tick",
        "sim",
        "list",
        "ge_build",
        "props",
        "lights",
        "fx",
        "amb",
        "audio",
        "ge_total",
        "ge_terrain",
        "ge_sun_shadows",
        "ge_water_fx",
        "ge_glow",
        "ge_ui",
    ];
    /// The frame's CPU parts past the summary's, in µs, as the PSP game fills them: the presenter's draw,
    /// the shell's UI, the UI's list, then the world list's own timers (`Lister::prof`): the lamp
    /// lightmap (holding its casters' and blocks'), the sun shadows (holding theirs), the sprites,
    /// the water, the grouping by texture, and the rest of the list (terrain, sky, fog, particles,
    /// grade). Means per measured frame, written `<name>_ms` on the scene's line.
    pub const PARTS: [&str; 13] = [
        "draw",
        "ui",
        "ui_list",
        "l_pools",
        "l_pool_casters",
        "l_pool_blocks",
        "l_silh",
        "l_silh_casters",
        "l_silh_blocks",
        "l_sprites",
        "l_water",
        "l_order",
        "l_rest",
    ];
    /// Histogram buckets per key.
    pub const BUCKETS: usize = 1024;
    /// "audio" in [`KEYS`].
    pub const AUDIO: usize = 8;

    /// 0..=511: 100 µs wide (0-51.1 ms); 512..=1023: 1000 µs wide (51.2-562.2 ms); saturates at 1023.
    pub const fn bucket(v: u32) -> usize {
        if v < 51_200 {
            (v / 100) as usize
        } else {
            let b = 512 + (v - 51_200) / 1000;
            if b < 1023 { b as usize } else { 1023 }
        }
    }

    /// The low edge of bucket `b`, in the same units [`bucket`] took.
    pub const fn floor(b: usize) -> u32 {
        if b < 512 { (b as u32) * 100 } else { 51_200 + (b as u32 - 512) * 1000 }
    }

    /// The scene line's key names, in order: `<k>_ms`, `<k>_p95` for each k, but `audio_pct`, `audio_p95`.
    pub fn line_keys() -> Vec<String> {
        let mut v = Vec::with_capacity(KEYS.len() * 2);
        for k in KEYS {
            if k == "audio" {
                v.push(String::from("audio_pct"));
                v.push(String::from("audio_p95"));
            } else {
                v.push(format!("{k}_ms"));
                v.push(format!("{k}_p95"));
            }
        }
        v
    }

    /// Every line key as `-` (no summary was allocated).
    pub fn write_absent(out: &mut String) {
        for (k, name) in KEYS.iter().enumerate() {
            put(out, k, name, false, None);
            put(out, k, name, true, None);
        }
    }

    /// One ` <key> <value>` pair. `us` is `None` for `-`.
    pub(super) fn put(out: &mut String, k: usize, name: &str, p95: bool, us: Option<u32>) {
        use core::fmt::Write as _;
        out.push(' ');
        if k == AUDIO {
            out.push_str(if p95 { "audio_p95" } else { "audio_pct" });
        } else {
            out.push_str(name);
            out.push_str(if p95 { "_p95" } else { "_ms" });
        }
        out.push(' ');
        if let Some(us) = us {
            let _ = write!(out, "{}.{}", us / 1000, us % 1000 / 100);
        } else {
            out.push('-');
        }
    }

    /// Rank of the p95 sample: the first bucket whose running count reaches `ceil(n * 95 / 100)`.
    pub const fn p95_rank(n: u32) -> u32 {
        (n as u64 * 95).div_ceil(100) as u32
    }
}

/// One scene's means and p95s, accumulated a frame at a time. 15 × 1024 × 2 = 30,720 B.
///
/// Audio is the mix's share of the wall time since the previous frame's start (the overlay's
/// window, one frame of it), not of the `frame` field, which stops at the capture's timestamp.
/// 1000 is 1.0%, so the same one-decimal formatter prints a percent. A sample is clamped to
/// 100.0% (100_000) before it enters the histogram.
#[derive(Debug)]
pub struct Summary {
    n: u32,
    sum: [u64; summary::KEYS.len()],
    audio_us: u64,
    wall_us: u64,
    hist: Vec<u16>,
}

impl Summary {
    /// The histogram, or `None` when the RAM is not there.
    pub fn new() -> Option<Summary> {
        let mut hist = Vec::new();
        hist.try_reserve_exact(summary::KEYS.len() * summary::BUCKETS).ok()?;
        hist.resize(summary::KEYS.len() * summary::BUCKETS, 0);
        Some(Summary { n: 0, sum: [0; summary::KEYS.len()], audio_us: 0, wall_us: 0, hist })
    }

    /// Counts and sums to 0. The histogram stays allocated.
    pub fn clear(&mut self) {
        self.n = 0;
        self.sum = [0; summary::KEYS.len()];
        self.audio_us = 0;
        self.wall_us = 0;
        self.hist.fill(0);
    }

    /// Frames pushed.
    pub fn frames(&self) -> u32 {
        self.n
    }

    /// One frame. Ignored unless `fields` has at least [`field::N`] words and `passes` at least
    /// [`pass::N`], so a decoded [`Capture`] row (`&row[..field::N]`, `&row[field::N..]`) feeds it too.
    ///
    /// `wall_us` is the microseconds since the previous frame's start.
    pub fn push(&mut self, fields: &[u32], passes: &[u32], wall_us: u32) {
        if fields.len() < field::N || passes.len() < pass::N {
            return;
        }
        let pct = audio_share(u64::from(fields[field::AUDIO]), u64::from(wall_us.max(1)));
        for k in 0..summary::KEYS.len() {
            let v = if k == summary::AUDIO { pct } else { part(fields, passes, k) };
            if k != summary::AUDIO {
                self.sum[k] = self.sum[k].wrapping_add(u64::from(v));
            }
            let b = summary::bucket(v);
            let i = k * summary::BUCKETS + b;
            if self.hist[i] != u16::MAX {
                self.hist[i] = self.hist[i].saturating_add(1);
            }
        }
        self.audio_us = self.audio_us.wrapping_add(u64::from(fields[field::AUDIO]));
        self.wall_us = self.wall_us.wrapping_add(u64::from(wall_us));
        self.n = self.n.wrapping_add(1);
    }

    /// The mean, microseconds (0 when no frame has been pushed). Audio is the whole window's
    /// share, `Σ mix * 100_000 / max(Σ wall, 1)`, clamped to 100.0%.
    pub fn mean_us(&self, k: usize) -> u32 {
        if self.n == 0 || k >= summary::KEYS.len() {
            return 0;
        }
        if k == summary::AUDIO {
            return audio_share(self.audio_us, self.wall_us.max(1));
        }
        (self.sum[k] / u64::from(self.n)) as u32
    }

    /// Floor of the first bucket whose running count reaches [`summary::p95_rank`].
    pub fn p95_us(&self, k: usize) -> u32 {
        if self.n == 0 || k >= summary::KEYS.len() {
            return 0;
        }
        let need = summary::p95_rank(self.n);
        let mut run = 0u32;
        let base = k * summary::BUCKETS;
        for b in 0..summary::BUCKETS {
            run += u32::from(self.hist[base + b]);
            if run >= need {
                return summary::floor(b);
            }
        }
        summary::floor(summary::BUCKETS - 1)
    }

    /// Appends ` <key> <value>` for every line key. Values are milliseconds with one decimal
    /// (`us/1000` and `us%1000/100`); with no frames, every value is `-`.
    pub fn write(&self, out: &mut String) {
        for (k, name) in summary::KEYS.iter().enumerate() {
            let (mean, p95) = if self.n == 0 { (None, None) } else { (Some(self.mean_us(k)), Some(self.p95_us(k))) };
            summary::put(out, k, name, false, mean);
            summary::put(out, k, name, true, p95);
        }
    }
}

/// The mix's share of `wall_us`: 1000 = 1.0%, clamped to 100.0% (100_000).
fn audio_share(audio_us: u64, wall_us: u64) -> u32 {
    (audio_us.saturating_mul(100_000) / wall_us.max(1)).min(100_000) as u32
}

/// One summary key's microseconds (not audio).
fn part(fields: &[u32], passes: &[u32], k: usize) -> u32 {
    use field as F;
    use pass as P;
    match k {
        0 => fields[F::TICK].wrapping_add(fields[F::BUFS]),
        1 => fields[F::SIM],
        2 => fields[F::LIST],
        3 => fields[F::GE_BUILD],
        4 => fields[F::T_PROPS],
        5 => fields[F::T_LIGHTS],
        6 => fields[F::T_FX],
        7 => fields[F::T_AMBIENT],
        9 => fields[F::GE_TOTAL],
        10 => passes[usize::from(P::TERRAIN)],
        11 => passes[usize::from(P::SUN_SHADOWS)],
        12 => passes[usize::from(P::WATER_FX)],
        13 => passes[usize::from(P::GLOW)],
        14 => passes[usize::from(P::UI)],
        _ => 0,
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
    fn a_capture_reads_back_as_written_oldest_first() {
        let mut r = Recorder::new(3).unwrap();
        let mut f = [0u32; field::N];
        let mut p = [0u32; pass::N];
        for k in 0..5u32 {
            f[field::FRAME] = 16_000 + k;
            f[field::FREE] = 3_000_000;
            p[usize::from(pass::GRADE)] = 900 + k;
            r.push(&f, &p);
        }
        assert_eq!(r.frames(), 3, "the ring keeps the last three");
        let c = Capture::decode(&r.encode(
            "commit abc123
seed 1
zone county
",
        ))
        .unwrap();
        assert_eq!(c.version, VERSION);
        assert_eq!(c.get("commit"), Some("abc123"));
        assert_eq!(c.get("zone"), Some("county"));
        assert_eq!(c.fields.len(), field::N);
        assert_eq!(c.fields[field::EV_CHUNK_NEW], "ev_chunk_new");
        assert_eq!(c.passes[usize::from(pass::GRADE)], "grade");
        let frames: Vec<u32> = c.frames.iter().map(|f| f[c.field("frame").unwrap()]).collect();
        assert_eq!(frames, [16_002, 16_003, 16_004], "oldest first");
        assert_eq!(c.frames[1][field::N + usize::from(pass::GRADE)], 903);
        let bytes = r.encode(
            "x 1
",
        );
        assert!(Capture::decode(&bytes[..20]).is_err());
        assert!(Capture::decode(b"nope").is_err());
    }

    #[test]
    fn the_pacing_fields_round_trip_and_sit_where_their_names_say() {
        assert_eq!(FIELDS.len(), field::N);
        for (ix, name) in
            [(field::VBLANKS, "vblanks"), (field::LIST_KB, "list_kb"), (field::EV_LIST_FULL, "ev_list_full")]
        {
            assert_eq!(FIELDS[ix], name);
        }
        assert_eq!(FIELDS[field::AUDIO_WORST], "audio_worst");
        assert_eq!(FIELDS[field::AUDIO_BLOCKS], "n_audio_blocks");
        assert_eq!(field::N, 62);
        let mut r = Recorder::new(4).unwrap();
        let p = [0u32; pass::N];
        for k in 0..4u32 {
            let mut f = [0u32; field::N];
            f[field::VBLANKS] = 2 + k;
            f[field::LIST_KB] = 100 + k;
            f[field::EV_LIST_FULL] = k % 2;
            f[field::LATE] = u32::from(k > 0);
            r.push(&f, &p);
        }
        let c = Capture::decode(&r.encode("pacing Locked30 pipelined true\n")).unwrap();
        assert_eq!(c.get("pacing"), Some("Locked30 pipelined true"));
        let col = |name: &str| -> Vec<u32> {
            let w = c.field(name).unwrap();
            c.frames.iter().map(|f| f[w]).collect()
        };
        assert_eq!(col("vblanks"), [2, 3, 4, 5]);
        assert_eq!(col("list_kb"), [100, 101, 102, 103]);
        assert_eq!(col("ev_list_full"), [0, 1, 0, 1]);
        assert_eq!(col("late"), [0, 1, 1, 1]);
    }

    /// A capture as v0.4.0 wrote it: the first 57 fields, none of v0.4.1's, the same passes.
    fn v040_capture(frames: &[[u32; field::EV_CHUNK_NEW + 1]]) -> Vec<u8> {
        let nf = field::EV_CHUNK_NEW + 1;
        let header = "commit old\npacing Locked30 pipelined true\n";
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(header.len() as u32).to_le_bytes());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&(nf as u16).to_le_bytes());
        out.extend_from_slice(&(pass::N as u16).to_le_bytes());
        for n in FIELDS[..nf].iter().chain(pass::NAMES.iter()) {
            out.push(n.len() as u8);
            out.extend_from_slice(n.as_bytes());
        }
        out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
        for f in frames {
            for w in f.iter().chain([0u32; pass::N].iter()) {
                out.extend_from_slice(&w.to_le_bytes());
            }
        }
        out
    }

    #[test]
    fn an_older_capture_without_the_pacing_fields_still_decodes() {
        let mut f = [0u32; field::EV_CHUNK_NEW + 1];
        f[field::FRAME] = 33_000;
        f[field::LATE] = 1;
        let c = Capture::decode(&v040_capture(&[f, f])).unwrap();
        assert_eq!(c.fields.len(), 57);
        assert_eq!(c.passes.len(), pass::N);
        assert_eq!(c.frames.len(), 2);
        assert_eq!(c.field("vblanks"), None, "the old capture has no vblanks");
        assert_eq!(c.field("late"), Some(field::LATE));
        assert_eq!(c.frames[1][field::FRAME], 33_000);
        assert_eq!(c.get("pacing"), Some("Locked30 pipelined true"));
    }

    #[test]
    fn encode_last_keeps_the_newest_and_matches_encode() {
        let mut r = Recorder::new(5).unwrap();
        let p = [0u32; pass::N];
        for k in 0..8u32 {
            let mut f = [0u32; field::N];
            f[field::FRAME] = 16_000 + k;
            r.push(&f, &p);
        }
        let h = "bench_scene wood\n";
        let last3 = r.encode_last(h, 3).unwrap();
        let c = Capture::decode(&last3).unwrap();
        let frames: Vec<u32> = c.frames.iter().map(|f| f[field::FRAME]).collect();
        assert_eq!(frames, [16_005, 16_006, 16_007]);
        assert_eq!(r.encode(h), r.encode_last(h, 5).unwrap());
        let all = Capture::decode(&r.encode_last(h, 99).unwrap()).unwrap();
        assert_eq!(all.frames.len(), 5, "last past the ring clips to what is held");
        let none = Capture::decode(&r.encode_last(h, 0).unwrap()).unwrap();
        assert_eq!(none.frames.len(), 0);
        assert_eq!(none.get("bench_scene"), Some("wood"));
    }

    #[test]
    fn summary_buckets_and_p95() {
        assert_eq!(summary::bucket(0), 0);
        assert_eq!(summary::bucket(51_199), 511);
        assert_eq!(summary::bucket(51_200), 512);
        assert_eq!(summary::bucket(u32::MAX), 1023);
        let mut v = 0u32;
        loop {
            assert!(summary::floor(summary::bucket(v)) <= v, "{v}");
            if v > u32::MAX - 8_000 {
                break;
            }
            v += 7_777;
        }
        let sim = summary::KEYS.iter().position(|k| *k == "sim").unwrap();
        let mut fine = Summary::new().unwrap();
        let p = [0u32; pass::N];
        for k in 1..=100u32 {
            let mut f = [0u32; field::N];
            f[field::SIM] = 100 * k;
            fine.push(&f, &p, 16_000);
        }
        let mut line = String::new();
        fine.write(&mut line);
        assert!(line.contains("sim_ms 5.0"), "{line}");
        assert!(line.contains("sim_p95 9.5"), "{line}");

        let mut coarse = Summary::new().unwrap();
        for k in 1..=100u32 {
            let mut f = [0u32; field::N];
            f[field::SIM] = 1000 * k;
            coarse.push(&f, &p, 16_000);
        }
        line.clear();
        coarse.write(&mut line);
        assert!(line.contains("sim_ms 50.5"), "{line}");
        assert!(line.contains("sim_p95 94.2"), "{line}");

        // n = 20: ceil(20*95/100) = 19. SIM = 100*k lands in its own bucket, so the 19th
        // sample is 1.9 ms and the 18th (an off-by-one) would be 1.8.
        assert_eq!(summary::p95_rank(20), 19);
        let mut twenty = Summary::new().unwrap();
        for k in 1..=20u32 {
            let mut f = [0u32; field::N];
            f[field::SIM] = 100 * k;
            twenty.push(&f, &p, 16_000);
        }
        // 100 * (1..=20) sums to 21_000; the mean is 1_050 µs (1.0 ms). The p95 is the 19th.
        assert_eq!(twenty.mean_us(sim), 1_050);
        assert_eq!(twenty.p95_us(sim), 1_900);
        line.clear();
        twenty.write(&mut line);
        assert!(line.contains("sim_p95 1.9"), "{line}");

        let empty = Summary::new().unwrap();
        line.clear();
        empty.write(&mut line);
        for key in summary::line_keys() {
            assert!(line.contains(&format!("{key} -")), "{key} in {line}");
        }
        assert!(!line.contains('0'), "an empty summary writes dashes, not zeros: {line}");
    }

    #[test]
    fn summary_line_reads_back() {
        let mut s = Summary::new().unwrap();
        let p = [0u32; pass::N];
        for _ in 0..4 {
            let mut f = [0u32; field::N];
            f[field::AUDIO] = 1_666;
            // The wall is the denominator. The `frame` field is a different, shorter span.
            f[field::FRAME] = 8_000;
            s.push(&f, &p, 16_660);
        }
        let mut line = String::new();
        s.write(&mut line);
        assert!(line.contains("audio_pct 10.0"), "{line}");
        let fields = jane_present::bench::fields(&line);
        for key in summary::line_keys() {
            let hits: Vec<_> = fields.iter().filter(|p| p.0 == key).collect();
            assert_eq!(hits.len(), 1, "{key} in {line}");
            hits[0].1.parse::<f64>().unwrap_or_else(|_| panic!("{} is not a number", hits[0].1));
        }
        // A mix longer than the wall clamps to 100.0% before the histogram and in the mean.
        let mut hot = Summary::new().unwrap();
        let mut f = [0u32; field::N];
        f[field::AUDIO] = 50_000;
        hot.push(&f, &p, 10_000);
        let audio = summary::AUDIO;
        assert_eq!(hot.mean_us(audio), 100_000);
        // 100_000 sits in the 1 ms bucket whose floor is 99_200, so the line prints 99.2.
        assert_eq!(hot.p95_us(audio), 99_200);
        line.clear();
        hot.write(&mut line);
        assert!(line.contains("audio_pct 100.0"), "{line}");
        assert!(line.contains("audio_p95 99.2"), "{line}");
    }

    #[test]
    fn summary_from_a_decoded_capture_equals_the_live_one() {
        let mut rec = Recorder::new(64).unwrap();
        let mut live = Summary::new().unwrap();
        let mut walls = Vec::new();
        for k in 0..50u32 {
            let mut f = [0u32; field::N];
            let mut p = [0u32; pass::N];
            f[field::SIM] = 1_000 + k * 17;
            f[field::TICK] = 2_000 + k;
            f[field::BUFS] = 300 + k;
            f[field::LIST] = 400 + k * 3;
            f[field::GE_BUILD] = 500 + k;
            f[field::T_PROPS] = 100 + k;
            f[field::T_LIGHTS] = 80 + k;
            f[field::T_FX] = 40 + k % 7;
            f[field::T_AMBIENT] = 20 + k % 5;
            f[field::GE_TOTAL] = 8_000 + k * 13;
            f[field::AUDIO] = 1_500 + k * 11;
            f[field::FRAME] = 1;
            p[usize::from(pass::TERRAIN)] = 200 + k;
            p[usize::from(pass::SUN_SHADOWS)] = 900 + k * 2;
            p[usize::from(pass::WATER_FX)] = k % 3;
            p[usize::from(pass::GLOW)] = 30 + k;
            p[usize::from(pass::UI)] = 25 + k;
            let wall = 16_000 + k * 10;
            rec.push(&f, &p);
            live.push(&f, &p, wall);
            walls.push(wall);
        }
        let cap = Capture::decode(&rec.encode("h 1\n")).unwrap();
        let mut again = Summary::new().unwrap();
        for (row, wall) in cap.frames.iter().zip(&walls) {
            again.push(&row[..field::N], &row[field::N..], *wall);
        }
        let (mut a, mut b) = (String::new(), String::new());
        live.write(&mut a);
        again.write(&mut b);
        assert_eq!(a, b);
    }
}
