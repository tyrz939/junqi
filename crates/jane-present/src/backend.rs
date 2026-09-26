//! The backend contract (PRESENTATION.md §1.2): a backend is a function of the `Frame` and its
//! own caches. It reads no view, no event and no tuning row.

use crate::frame::{Frame, Tier};

/// What a backend can do, reported once at boot.
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    pub tier: Tier,
    pub max_lights: u16,
    pub has_readback: bool,
    /// `soft`, `gl2` or `wgpu`, and for a GPU the API under it: the title bar, F2, `jane bench`.
    pub name: &'static str,
}

/// Entries in the CLUT: the master palette's ceiling (ART.md §2.7).
pub const CLUT_LEN: usize = 1024;

/// The contact shadow's multiply per channel, 1/256ths, at full cover (`jane_art::palette`):
/// every backend darkens under index 1 by it, scaled by how much of the texel's 3 x 3 is index 1.
pub const AO_TINT: [u16; 3] = jane_art::palette::AO_TINT;

/// One atlas page, four layers of one layout (PRESENTATION.md §1.4), each `w * h` row-major.
/// Albedo is master-palette indices: 0 clear, 1 the contact shadow, every other index opaque.
/// The other three are empty on a page built for `soft` alone and never read by it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub w: u16,
    pub h: u16,
    pub albedo: Vec<u16>,
    /// Tangent-space `[nx, ny]`, 128 is 0; `+x` east, `+y` south (`jane_art::canvas::Normal`).
    pub normal: Vec<[u8; 2]>,
    /// Master-palette indices that glow unlit; 0 is dark.
    pub emissive: Vec<u16>,
    /// Px above the ground: a person's head is 40.
    pub height: Vec<u8>,
}

impl Page {
    /// Whether the page carries the normal, emissive and height layers.
    pub fn lit(&self) -> bool {
        self.height.len() == self.albedo.len() && !self.albedo.is_empty()
    }
}

/// The atlas pages built at boot from `jane-art` (PRESENTATION.md §1.4), handed to a backend
/// once, again on a CLUT change.
#[derive(Clone, Debug, Default)]
pub struct AtlasPages {
    /// `0xAARRGGBB` per master-palette index, `CLUT_LEN` long; index 0 and 1 are never read.
    pub clut: Vec<u32>,
    pub pages: Vec<Page>,
}

/// The passes a frame's time is split into (§1.12's measurement, in its order). A backend
/// reports what it can split: a pass it does not draw, or draws inside another, reads 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatPass {
    Sky,
    Parallax,
    Chunks,
    Water,
    /// The sprite draw list, ground and standing.
    List,
    Fx,
    /// Shadows: the height field on T2, the silhouettes on T0.
    Shadows,
    /// Lighting: the light pass on T2, the lightmap or the multiply on T0.
    Light,
    Fog,
    Weather,
    /// Grade and bloom.
    Grade,
    Ui,
    /// The canvas to the window.
    Upscale,
}

impl StatPass {
    /// How many there are: the length of every per-pass array in [`FrameStats`].
    pub const COUNT: usize = 13;
    /// All of them, in order: `ALL[p as usize] == p`.
    pub const ALL: [StatPass; StatPass::COUNT] = [
        StatPass::Sky,
        StatPass::Parallax,
        StatPass::Chunks,
        StatPass::Water,
        StatPass::List,
        StatPass::Fx,
        StatPass::Shadows,
        StatPass::Light,
        StatPass::Fog,
        StatPass::Weather,
        StatPass::Grade,
        StatPass::Ui,
        StatPass::Upscale,
    ];

    /// Its name as F2 and `jane bench` print it.
    pub const fn name(self) -> &'static str {
        match self {
            StatPass::Sky => "sky",
            StatPass::Parallax => "parallax",
            StatPass::Chunks => "chunks",
            StatPass::Water => "water",
            StatPass::List => "list",
            StatPass::Fx => "fx",
            StatPass::Shadows => "shadows",
            StatPass::Light => "light",
            StatPass::Fog => "fog",
            StatPass::Weather => "weather",
            StatPass::Grade => "grade",
            StatPass::Ui => "ui",
            StatPass::Upscale => "upscale",
        }
    }
}

/// What a backend measured of its frames (§1.12), a plain value the F2 overlay and `jane bench`
/// read through [`Backend::stats`]. Times are microseconds; per-pass arrays are indexed by
/// `StatPass as usize`. On a GPU with timestamp queries the times are the GPU's own clock, read
/// a frame or two late; otherwise they are the CPU's time in `draw` (and, for the upscale, in
/// `present`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Frames measured so far; it counts up by one each time new times arrive.
    pub frames: u32,
    /// The last measured frame, whole.
    pub last_us: u32,
    /// Median and 99th percentile of the whole frame over the last 120 measured.
    pub p50_us: u32,
    pub p99_us: u32,
    /// Whether the times are the GPU's own (timestamp queries) rather than the CPU's.
    pub gpu_clock: bool,
    /// The last measured frame, per pass.
    pub pass_us: [u32; StatPass::COUNT],
    /// Median and 99th percentile per pass over the last 120 measured.
    pub pass_p50_us: [u32; StatPass::COUNT],
    pub pass_p99_us: [u32; StatPass::COUNT],
    /// The last frame drawn: draw calls issued (on `soft`, blits and passes), lights and
    /// casters in it, and px written (`soft` only; 0 on a GPU).
    pub draw_calls: u32,
    pub lights: u32,
    pub casters: u32,
    pub pixels_written: u64,
}

/// A renderer of `Frame`s.
pub trait Backend {
    fn caps(&self) -> Caps;
    /// Once at boot, again on a CLUT change.
    fn upload_atlas(&mut self, pages: &AtlasPages);
    /// Draws `frame` in pass order.
    fn draw(&mut self, frame: &Frame);
    /// The canvas as `0xAARRGGBB` rows into `out`; returns its size.
    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16);
    /// Frame times, if the backend measures them.
    fn stats(&self) -> Option<FrameStats> {
        None
    }
}

/// Frames a [`FrameTimes`] window holds.
const WINDOW: usize = 120;

/// A rolling window of frame times, whole and per pass, for a backend's [`FrameStats`]. Fixed
/// arrays: pushing never allocates.
#[derive(Clone, Debug)]
pub struct FrameTimes {
    ring: [u32; WINDOW],
    passes: [[u32; WINDOW]; StatPass::COUNT],
    n: u32,
    gpu_clock: bool,
    counts: (u32, u32, u32, u64),
}

impl Default for FrameTimes {
    fn default() -> Self {
        FrameTimes::new(false)
    }
}

/// The median and 99th percentile of the first `k` of `ring`.
fn p50_p99(ring: &[u32; WINDOW], k: usize) -> (u32, u32) {
    if k == 0 {
        return (0, 0);
    }
    let mut v = [0u32; WINDOW];
    v[..k].copy_from_slice(&ring[..k]);
    let v = &mut v[..k];
    v.sort_unstable();
    (v[(k - 1) / 2], v[(k - 1) * 99 / 100])
}

impl FrameTimes {
    pub fn new(gpu_clock: bool) -> FrameTimes {
        FrameTimes { ring: [0; WINDOW], passes: [[0; WINDOW]; StatPass::COUNT], n: 0, gpu_clock, counts: (0, 0, 0, 0) }
    }

    /// A frame whose passes were not split: its whole time alone.
    pub fn push(&mut self, us: u32) {
        self.push_passes(us, [0; StatPass::COUNT]);
    }

    /// A frame, whole and per pass.
    pub fn push_passes(&mut self, us: u32, pass_us: [u32; StatPass::COUNT]) {
        let i = (self.n as usize) % WINDOW;
        self.ring[i] = us;
        for (p, v) in self.passes.iter_mut().zip(pass_us) {
            p[i] = v;
        }
        self.n += 1;
    }

    /// The last frame's counts: draw calls, lights, casters, px written.
    pub fn set_counts(&mut self, draw_calls: u32, lights: u32, casters: u32, pixels_written: u64) {
        self.counts = (draw_calls, lights, casters, pixels_written);
    }

    pub fn stats(&self) -> FrameStats {
        let k = (self.n as usize).min(WINDOW);
        let last = if self.n == 0 { None } else { Some((self.n as usize - 1) % WINDOW) };
        let (p50_us, p99_us) = p50_p99(&self.ring, k);
        let mut s = FrameStats {
            frames: self.n,
            last_us: last.map_or(0, |i| self.ring[i]),
            p50_us,
            p99_us,
            gpu_clock: self.gpu_clock,
            draw_calls: self.counts.0,
            lights: self.counts.1,
            casters: self.counts.2,
            pixels_written: self.counts.3,
            ..FrameStats::default()
        };
        for (p, ring) in self.passes.iter().enumerate() {
            s.pass_us[p] = last.map_or(0, |i| ring[i]);
            (s.pass_p50_us[p], s.pass_p99_us[p]) = p50_p99(ring, k);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_over_the_window() {
        let mut t = FrameTimes::new(false);
        assert_eq!(t.stats().p99_us, 0);
        for i in 1..=200 {
            t.push(i);
        }
        let s = t.stats();
        // The last 120 are 81..=200.
        assert_eq!((s.frames, s.last_us), (200, 200));
        assert_eq!(s.p50_us, 81 + 59);
        assert_eq!(s.p99_us, 81 + 117);
    }

    #[test]
    fn passes_are_kept_by_name_and_in_order() {
        for (i, p) in StatPass::ALL.iter().enumerate() {
            assert_eq!(*p as usize, i, "{}", p.name());
        }
        let mut t = FrameTimes::new(true);
        let mut passes = [0; StatPass::COUNT];
        passes[StatPass::Shadows as usize] = 300;
        passes[StatPass::Light as usize] = 700;
        t.push_passes(1000, passes);
        t.set_counts(12, 5, 30, 0);
        let s = t.stats();
        assert_eq!(s.pass_us[StatPass::Light as usize], 700);
        assert_eq!(s.pass_p99_us[StatPass::Shadows as usize], 300);
        assert_eq!((s.draw_calls, s.lights, s.casters, s.gpu_clock), (12, 5, 30, true));
    }
}
