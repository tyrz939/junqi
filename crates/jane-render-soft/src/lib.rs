//! T0 backend: the CPU rasteriser (PRESENTATION.md §1.2, §1.4). Integer pixel path, so its
//! frames are byte-identical across targets.

use jane_present::{AtlasPages, Backend, Caps, Frame, Tier};

/// The `soft` backend: one `u32` framebuffer, `0xAARRGGBB`.
#[derive(Debug, Default)]
pub struct Soft {
    fb: Vec<u32>,
    w: u16,
    h: u16,
}

impl Soft {
    pub fn new() -> Soft {
        Soft::default()
    }

    /// The last frame drawn: pixels, width, height.
    pub fn pixels(&self) -> (&[u32], u16, u16) {
        (&self.fb, self.w, self.h)
    }
}

impl Backend for Soft {
    fn caps(&self) -> Caps {
        Caps { tier: Tier::T0, max_lights: 16, has_readback: true }
    }

    fn upload_atlas(&mut self, pages: &AtlasPages) {
        let _ = pages;
    }

    fn draw(&mut self, frame: &Frame) {
        (self.w, self.h) = frame.canvas;
        let n = usize::from(self.w) * usize::from(self.h);
        self.fb.clear();
        self.fb.resize(n, frame.clear);
    }

    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        out.clear();
        out.extend_from_slice(&self.fb);
        (self.w, self.h)
    }
}
