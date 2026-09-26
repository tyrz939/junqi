//! T0 backend: the CPU rasteriser (PRESENTATION.md §1.2, §1.4). Integer pixel path, so its
//! frames are byte-identical across targets.

pub mod blit;

use jane_present::frame::CHUNK_PX;
use jane_present::{AtlasPages, Backend, Caps, Frame, Pass, Tier};

use crate::blit::Target;

/// The `soft` backend: one `u32` framebuffer, `0xAARRGGBB`.
#[derive(Debug, Default)]
pub struct Soft {
    fb: Vec<u32>,
    w: u16,
    h: u16,
    atlas: AtlasPages,
    /// Pixels written by the last frame (the bench's proxy, §1.12).
    pub pixels_written: u64,
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
        self.atlas.clone_from(pages);
    }

    fn draw(&mut self, frame: &Frame) {
        (self.w, self.h) = frame.canvas;
        let n = usize::from(self.w) * usize::from(self.h);
        if self.fb.len() == n {
            self.fb.fill(frame.clear);
        } else {
            self.fb.clear();
            self.fb.resize(n, frame.clear);
        }
        let mut written = n as u64;
        let t = &mut Target { px: &mut self.fb, w: i32::from(self.w), h: i32::from(self.h) };
        for pass in &frame.passes {
            match *pass {
                Pass::Terrain { chunks } => {
                    for c in frame.chunks_in(chunks) {
                        blit::chunk(t, &frame.layers_of(c).albedo, CHUNK_PX, c.x, c.y);
                        written += (CHUNK_PX * CHUNK_PX) as u64;
                    }
                }
                Pass::Sprites { cmds, .. } => {
                    for s in frame.sprites_in(cmds) {
                        if let Some(page) = self.atlas.pages.get(usize::from(s.page)) {
                            blit::sprite(t, page, &self.atlas.clut, s.src, i32::from(s.x), i32::from(s.y), s.flags);
                            written += u64::from(s.src.w) * u64::from(s.src.h);
                        }
                    }
                }
                Pass::Lights { ambient } => {
                    blit::multiply(t, ambient);
                    written += n as u64;
                }
            }
        }
        self.pixels_written = written;
    }

    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        out.clear();
        out.extend_from_slice(&self.fb);
        (self.w, self.h)
    }
}
