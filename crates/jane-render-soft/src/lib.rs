//! T0 backend: the CPU rasteriser (PRESENTATION.md §1.2, §1.4). Integer pixel path, so its
//! frames are byte-identical across targets.

pub mod blit;
pub mod silhouette;

use jane_present::frame::CHUNK_PX;
use jane_present::{AtlasPages, Backend, Caps, Frame, Page, Pass, Tier};

use crate::blit::Target;
use crate::silhouette::Mask;

/// The `soft` backend: one `u32` framebuffer, `0xAARRGGBB`.
#[derive(Debug, Default)]
pub struct Soft {
    fb: Vec<u32>,
    w: u16,
    h: u16,
    atlas: AtlasPages,
    /// The silhouette shadows' coverage, the canvas's size.
    mask: Mask,
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
        Caps { tier: Tier::T0, max_lights: 16, has_readback: true, name: "soft" }
    }

    /// Keeps the CLUT and the albedo; the normal, emissive and height pages are never read here.
    fn upload_atlas(&mut self, pages: &AtlasPages) {
        self.atlas.clut.clone_from(&pages.clut);
        self.atlas.pages.clear();
        self.atlas.pages.extend(pages.pages.iter().map(|p| Page {
            w: p.w,
            h: p.h,
            albedo: p.albedo.clone(),
            ..Page::default()
        }));
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
                Pass::Silhouettes { sun, shade, casters } => {
                    let Some(k) = silhouette::shear(&sun) else { continue };
                    self.mask.fit(t.w, t.h);
                    for c in frame.casters_in(casters) {
                        let Some(s) = frame.sprites.get(c.sprite as usize) else { continue };
                        if let Some(page) = self.atlas.pages.get(usize::from(s.page)) {
                            silhouette::cast(&mut self.mask, page, s, c, k);
                        }
                    }
                    written += silhouette::apply(t, &mut self.mask, shade);
                }
                // T0 lights by the ambient alone: the lightmap of §1.7 lands with PORT.md §7.1
                // step 5, and the sun's share is already in the ambient.
                Pass::Lights { ambient, .. } => {
                    if ambient.iter().any(|&c| c < 254) {
                        blit::multiply(t, ambient);
                        written += n as u64;
                    }
                }
                // A T2 pass: a T0 frame never holds one (§1.3), and soft draws nothing of its own.
                Pass::Post(_) => {}
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
