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

/// The atlas pages built at boot from `jane-art` (PRESENTATION.md §1.4), handed to a backend once.
#[derive(Debug, Default)]
pub struct AtlasPages {}

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
