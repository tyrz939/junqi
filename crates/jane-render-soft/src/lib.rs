//! T0 backend: the CPU rasteriser (PRESENTATION.md §1.2, §1.4). Integer pixel path, so its
//! frames are byte-identical across targets.

pub mod atmos;
pub mod blit;
pub mod grade;
pub mod lightmap;
pub mod silhouette;
pub mod ui;

use std::time::Instant;

use jane_present::frame::CHUNK_PX;
use jane_present::frame::SkyLook;
use jane_present::{AtlasPages, Backend, Caps, Depth, Frame, FrameStats, FrameTimes, Page, Pass, StatPass, Tier};

use crate::blit::Target;
use crate::lightmap::LightMap;
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
    /// The terrain's height under each canvas px, for the silhouettes to climb (only filled in a
    /// frame that has them).
    heights: Vec<u8>,
    /// The light buffer at a quarter of the canvas.
    lights: LightMap,
    /// Pixels written by the last frame (the bench's proxy, §1.12).
    pub pixels_written: u64,
    /// The CPU's time per pass (§1.12); never read by the pixel path.
    times: FrameTimes,
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
        Caps { tier: Tier::T0, max_lights: 16, has_readback: true, max_texture: u32::MAX, name: "soft" }
    }

    /// Keeps the CLUT and the albedo; the normal, emissive and height pages are never read here.
    fn upload_atlas(&mut self, pages: &AtlasPages) {
        self.atlas.clut.clone_from(&pages.clut);
        self.atlas.mist.clone_from(&pages.mist);
        self.atlas.pages.clear();
        self.atlas.pages.extend(pages.pages.iter().map(|p| Page {
            w: p.w,
            h: p.h,
            albedo: p.albedo.clone(),
            ..Page::default()
        }));
    }

    fn draw(&mut self, frame: &Frame) {
        let start = Instant::now();
        let mut pass_us = [0u32; StatPass::COUNT];
        let mut calls = 0u32;
        (self.w, self.h) = frame.canvas;
        let n = usize::from(self.w) * usize::from(self.h);
        if self.fb.len() == n {
            self.fb.fill(frame.clear);
        } else {
            self.fb.clear();
            self.fb.resize(n, frame.clear);
        }
        let mut written = n as u64;
        let climb = frame.passes.iter().any(|p| matches!(p, Pass::Silhouettes { .. }));
        if climb {
            self.heights.clear();
            self.heights.resize(n, 0);
        }
        let t = &mut Target { px: &mut self.fb, w: i32::from(self.w), h: i32::from(self.h) };
        pass_us[StatPass::Sky as usize] = start.elapsed().as_micros() as u32;
        // The sky, once drawn, keeps the terrain inside the zone (its chunks paint the frame's
        // clear beyond the edge, where the sky is).
        let mut sky: Option<SkyLook> = None;
        for pass in &frame.passes {
            let at = Instant::now();
            calls += 1;
            let stat = match *pass {
                Pass::Terrain { .. } => StatPass::Chunks,
                Pass::Sprites { .. } => StatPass::List,
                Pass::Silhouettes { .. } => StatPass::Shadows,
                Pass::Lights { .. } => StatPass::Light,
                Pass::Post(_) | Pass::Rays { .. } => StatPass::Grade,
                Pass::Sky(_) => StatPass::Sky,
                Pass::Parallax { .. } => StatPass::Parallax,
                Pass::Water { .. } => StatPass::Water,
                Pass::Weather(_) | Pass::Particles { layer: Depth::Weather, .. } => StatPass::Weather,
                Pass::Fog { .. } => StatPass::Fog,
                Pass::Particles { .. } => StatPass::Fx,
            };
            match *pass {
                Pass::Terrain { chunks } => {
                    let top = sky.map_or(0, |s| s.zone.1.clamp(0, t.h));
                    // Below the sky: the rows the zone covers.
                    let n = (top * t.w) as usize;
                    let mut ground = Target { px: &mut t.px[n..], w: t.w, h: t.h - top };
                    for c in frame.chunks_in(chunks) {
                        calls += 1;
                        let l = frame.layers_of(c);
                        blit::chunk(&mut ground, &l.albedo, CHUNK_PX, c.x, c.y - top);
                        if climb && l.has_height() {
                            blit::heights(&mut self.heights, (t.w, t.h), &l.height, CHUNK_PX, c.x, c.y);
                        }
                        written += (CHUNK_PX * CHUNK_PX) as u64;
                    }
                }
                Pass::Sky(s) => {
                    sky = Some(s);
                    written += atmos::sky(t, &s, &frame.stars[s.star_list.range()]);
                }
                Pass::Parallax { sprites, .. } => {
                    if let Some(s) = &sky {
                        for sp in frame.sprites_in(sprites) {
                            if let Some(page) = self.atlas.pages.get(usize::from(sp.page)) {
                                written += atmos::parallax(t, s, page, &self.atlas.clut, sp);
                            }
                        }
                    }
                }
                Pass::Water { cells } => written += atmos::shimmer(t, frame.water_in(cells), frame.tick),
                Pass::Fog { volumes, drift } => {
                    written += atmos::fog(t, frame.fog_in(volumes), &self.atlas.mist, frame.camera, drift);
                }
                Pass::Particles { parts, .. } => written += atmos::particles(t, frame.parts_in(parts)),
                // What the sky is doing reached T0 through the ambient already, and the rain is
                // particles. Light shafts are T2's, which a T0 frame never holds (§1.3).
                Pass::Weather(_) | Pass::Rays { .. } => {}
                // The grade, as T2 draws it (§1.9): the tiers are one look.
                Pass::Post(p) => written += grade::Grade::new(&p).apply(t),
                Pass::Sprites { cmds, .. } => {
                    for s in frame.sprites_in(cmds) {
                        if let Some(page) = self.atlas.pages.get(usize::from(s.page)) {
                            calls += 1;
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
                    written += silhouette::apply(
                        t,
                        &mut self.mask,
                        shade,
                        &self.heights,
                        jane_present::shadow::feather(sun.spread),
                    );
                }
                // T0 lights by the lightmap (§1.7): the ambient, the sun's share already in it,
                // and every point light's pool; by the ambient alone when no light shows.
                Pass::Lights { ambient, points, .. } => {
                    let points = frame.lights_in(points);
                    if !points.is_empty() {
                        self.lights.build((t.w, t.h), ambient, points);
                        written += self.lights.apply(t);
                    } else if ambient.iter().any(|&c| c < 254) {
                        blit::multiply(t, ambient);
                        written += n as u64;
                    }
                }
            }
            pass_us[stat as usize] += at.elapsed().as_micros() as u32;
        }
        // The `Ui` pass: after everything, unlit (PRESENTATION.md §3.1).
        if !frame.ui.is_empty() {
            let at = Instant::now();
            calls += frame.ui.len() as u32;
            written += ui::draw(t, frame, &self.atlas);
            pass_us[StatPass::Ui as usize] += at.elapsed().as_micros() as u32;
        }
        self.pixels_written = written;
        self.times.push_passes(start.elapsed().as_micros() as u32, pass_us);
        self.times.set_counts(calls, frame.lights.len() as u32, frame.casters.len() as u32, written);
    }

    fn stats(&self) -> Option<FrameStats> {
        Some(self.times.stats())
    }

    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        out.clear();
        out.extend_from_slice(&self.fb);
        (self.w, self.h)
    }
}
