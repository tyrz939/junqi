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

/// Frame times, measured by the backend that drew them (§1.12). Microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Frames measured.
    pub frames: u32,
    /// The last frame's time on the device (the GPU's own clock where it has one, else the
    /// CPU's time in `draw`).
    pub last_us: u32,
    /// Median and 99th percentile over the last 120 frames.
    pub p50_us: u32,
    pub p99_us: u32,
    /// Whether the times are the GPU's own (timestamp queries) rather than the CPU's.
    pub gpu_clock: bool,
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

/// A rolling window of frame times, for a backend's [`FrameStats`].
#[derive(Clone, Debug)]
pub struct FrameTimes {
    ring: [u32; 120],
    n: u32,
    gpu_clock: bool,
}

impl FrameTimes {
    pub fn new(gpu_clock: bool) -> FrameTimes {
        FrameTimes { ring: [0; 120], n: 0, gpu_clock }
    }

    pub fn push(&mut self, us: u32) {
        self.ring[(self.n % 120) as usize] = us;
        self.n += 1;
    }

    pub fn stats(&self) -> FrameStats {
        let k = self.n.min(120) as usize;
        let mut v = [0u32; 120];
        v[..k].copy_from_slice(&self.ring[..k]);
        let v = &mut v[..k];
        v.sort_unstable();
        let at = |q: usize| if k == 0 { 0 } else { v[((k - 1) * q / 100).min(k - 1)] };
        FrameStats {
            frames: self.n,
            last_us: if self.n == 0 { 0 } else { self.ring[((self.n - 1) % 120) as usize] },
            p50_us: at(50),
            p99_us: at(99),
            gpu_clock: self.gpu_clock,
        }
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
}
