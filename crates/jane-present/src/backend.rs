//! The backend contract (PRESENTATION.md §1.2): a backend is a function of the `Frame` and its
//! own caches. It reads no view, no event and no tuning row.

use crate::frame::{Frame, Tier};

/// What a backend can do, reported once at boot.
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    pub tier: Tier,
    pub max_lights: u16,
    pub has_readback: bool,
}

/// Entries in the CLUT: the master palette's ceiling (ART.md §2.7).
pub const CLUT_LEN: usize = 1024;

/// One albedo page: `w * h` master-palette indices, row-major (PRESENTATION.md §1.4). Index 0
/// is clear, 1 the contact shadow, every other index opaque.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub w: u16,
    pub h: u16,
    pub albedo: Vec<u16>,
}

/// The atlas pages built at boot from `jane-art` (PRESENTATION.md §1.4), handed to a backend
/// once, again on a CLUT change. Albedo only so far; the normal, emissive and height pages land
/// with T1.
#[derive(Clone, Debug, Default)]
pub struct AtlasPages {
    /// `0xAARRGGBB` per master-palette index, `CLUT_LEN` long; index 0 and 1 are never read.
    pub clut: Vec<u32>,
    pub pages: Vec<Page>,
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
}
