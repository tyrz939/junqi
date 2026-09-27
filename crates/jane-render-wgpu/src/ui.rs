//! The `Ui` pass on T2 (PRESENTATION.md §3.1; `jane_present::ui::cmd`): after the grade, into
//! the canvas, unlit. One instanced quad per command, straight alpha; runs of fills and sprites
//! share a draw, an image draws with its own texture, and a clip is a scissor. The frame's UI
//! images are kept on the GPU by slot and uploaded again only when their generation moves.
//!
//! It draws into the canvas through a plain (not sRGB) view, so blending is in the stored bytes
//! as `soft`'s is and a translucent panel reads the same on both backends.

use jane_present::Frame;
use jane_present::ui::{Rect, UiCmd};

use crate::gpu::{B, group, layout, texture};

const UI_WGSL: &str = include_str!("shaders/ui.wgsl");
/// The canvas's plain view: the Ui pass writes sRGB-encoded bytes itself.
pub const RAW: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Bytes a quad: dst (4 x i32), src (4 x u32), info (4 x u32), argb (u32), padding to 16.
const INST: u64 = 64;
const ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Sint32x4, 1 => Uint32x4, 2 => Uint32x4, 3 => Uint32];

/// One draw: a run of quads, the scissor they share and the image they read (`None`: fills and
/// sprites).
#[derive(Clone, Copy, Debug)]
struct Draw {
    first: u32,
    count: u32,
    clip: Rect,
    image: Option<u16>,
}

/// The pass's pipeline, its bindings and its images.
#[derive(Debug)]
pub struct UiPass {
    pipeline: wgpu::RenderPipeline,
    layout0: wgpu::BindGroupLayout,
    layout1: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    group0: Option<wgpu::BindGroup>,
    /// A 1 x 1 image for the draws that read none.
    blank: wgpu::BindGroup,
    /// Per slot: the generation held and its group.
    images: Vec<Option<(u32, wgpu::BindGroup)>>,
    quads: wgpu::Buffer,
    bytes: Vec<u8>,
    draws: Vec<Draw>,
}

impl UiPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> UiPass {
        let module = super::module(device, "ui", UI_WGSL);
        let layout0 = layout(device, "ui", &[B::Uniform, B::UintArray, B::Tex]);
        let layout1 = layout(device, "ui image", &[B::Tex]);
        let buffer = wgpu::VertexBufferLayout {
            array_stride: INST,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRS,
        };
        let pipeline = super::render_pipeline(
            device,
            "ui",
            &[&layout0, &layout1],
            &module,
            "vs_ui",
            "fs_ui",
            &[buffer],
            true,
            &[super::target(RAW, Some(super::OVER))],
        );
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let px = texture(device, "ui blank", (1, 1, 1), RAW, usage);
        super::write_layer(queue, &px, 0, (1, 1), 4, &[0, 0, 0, 0]);
        let view = px.create_view(&wgpu::TextureViewDescriptor::default());
        let blank = group(device, "ui blank", &layout1, &[super::r(&view)]);
        UiPass {
            pipeline,
            layout0,
            layout1,
            globals,
            group0: None,
            blank,
            images: Vec::new(),
            quads: super::instance_buffer(device, "ui quads", 1024 * INST),
            bytes: Vec::with_capacity(4096 * INST as usize),
            draws: Vec::with_capacity(64),
        }
    }

    /// The atlas changed: bind its albedo pages and CLUT.
    pub fn atlas(&mut self, device: &wgpu::Device, albedo: &wgpu::TextureView, clut: &wgpu::TextureView) {
        self.group0 = Some(group(
            device,
            "ui",
            &self.layout0,
            &[self.globals.as_entire_binding(), super::r(albedo), super::r(clut)],
        ));
    }

    /// Uploads what changed of the frame's images.
    fn images(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) {
        if self.images.len() < frame.ui_images.len() {
            self.images.resize_with(frame.ui_images.len(), || None);
        }
        for (i, img) in frame.ui_images.iter().enumerate() {
            if img.w == 0 || img.h == 0 || self.images[i].as_ref().is_some_and(|h| h.0 == img.generation) {
                continue;
            }
            let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
            let t = texture(device, "ui image", (u32::from(img.w), u32::from(img.h), 1), RAW, usage);
            let mut bytes = Vec::with_capacity(img.argb.len() * 4);
            for &p in &img.argb {
                let [a, r, g, b] = p.to_be_bytes();
                bytes.extend_from_slice(&[r, g, b, a]);
            }
            super::write_layer(queue, &t, 0, (u32::from(img.w), u32::from(img.h)), 4, &bytes);
            let view = t.create_view(&wgpu::TextureViewDescriptor::default());
            self.images[i] = Some((img.generation, group(device, "ui image", &self.layout1, &[super::r(&view)])));
        }
    }

    /// Encodes the frame's `Ui` pass into `view` (the canvas, `canvas` px). Returns draw calls.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        frame: &Frame,
        canvas: (u32, u32),
    ) -> u32 {
        if frame.ui.is_empty() {
            return 0;
        }
        if self.group0.is_none() {
            return 0;
        }
        self.images(device, queue, frame);
        // The quads and the runs they draw in.
        self.bytes.clear();
        self.draws.clear();
        let mut clip = Rect::CANVAS;
        let mut n = 0u32;
        let mut push = |bytes: &mut Vec<u8>, draws: &mut Vec<Draw>, clip: Rect, image: Option<u16>, q: [u32; 13]| {
            for v in q {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            bytes.extend_from_slice(&[0; 12]);
            match draws.last_mut() {
                Some(d) if d.clip == clip && d.image == image => d.count += 1,
                _ => draws.push(Draw { first: n, count: 1, clip, image }),
            }
            n += 1;
        };
        let dst4 =
            |r: Rect| [i32::from(r.x) as u32, i32::from(r.y) as u32, i32::from(r.w) as u32, i32::from(r.h) as u32];
        for c in &frame.ui {
            match *c {
                UiCmd::Clip(r) => clip = r,
                UiCmd::Fill { dst, argb } => {
                    let d = dst4(dst);
                    push(
                        &mut self.bytes,
                        &mut self.draws,
                        clip,
                        None,
                        [d[0], d[1], d[2], d[3], 0, 0, 1, 1, 0, 0, 0, 255, argb],
                    );
                }
                UiCmd::Sprite { page, src, dst, ink, alpha, mirror } => {
                    let d = dst4(dst);
                    let info = u32::from(alpha) | u32::from(mirror) << 8;
                    let s = [u32::from(src.x), u32::from(src.y), u32::from(src.w), u32::from(src.h)];
                    push(
                        &mut self.bytes,
                        &mut self.draws,
                        clip,
                        None,
                        [d[0], d[1], d[2], d[3], s[0], s[1], s[2], s[3], 1, u32::from(page), u32::from(ink), info, 0],
                    );
                }
                UiCmd::Image { slot, src, dst, alpha } => {
                    if !self.images.get(usize::from(slot)).is_some_and(Option::is_some) {
                        continue;
                    }
                    let d = dst4(dst);
                    let s = [u32::from(src.x), u32::from(src.y), u32::from(src.w), u32::from(src.h)];
                    push(
                        &mut self.bytes,
                        &mut self.draws,
                        clip,
                        Some(slot),
                        [d[0], d[1], d[2], d[3], s[0], s[1], s[2], s[3], 2, 0, 0, u32::from(alpha), 0],
                    );
                }
            }
        }
        if self.draws.is_empty() {
            return 0;
        }
        if self.bytes.len() as u64 > self.quads.size() {
            self.quads = super::instance_buffer(device, "ui quads", (self.bytes.len() as u64).next_power_of_two());
        }
        queue.write_buffer(&self.quads, 0, &self.bytes);
        let mut g = Vec::with_capacity(16);
        for x in [canvas.0 as f32, canvas.1 as f32, 0.0, 0.0] {
            g.extend_from_slice(&x.to_le_bytes());
        }
        queue.write_buffer(&self.globals, 0, &g);
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, self.group0.as_ref(), &[]);
        pass.set_vertex_buffer(0, self.quads.slice(..));
        let mut calls = 0;
        for d in &self.draws {
            let (x0, y0) =
                (i32::from(d.clip.x).clamp(0, canvas.0 as i32), i32::from(d.clip.y).clamp(0, canvas.1 as i32));
            let x1 = d.clip.right().clamp(0, canvas.0 as i32);
            let y1 = d.clip.bottom().clamp(0, canvas.1 as i32);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            pass.set_scissor_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
            let img = d.image.and_then(|s| self.images.get(usize::from(s))).and_then(Option::as_ref);
            pass.set_bind_group(1, img.map_or(&self.blank, |h| &h.1), &[]);
            pass.draw(0..4, d.first..d.first + d.count);
            calls += 1;
        }
        calls
    }
}

#[cfg(test)]
mod tests {
    use jane_present::backend::Page;
    use jane_present::frame::Src;
    use jane_present::{AtlasPages, Backend, Frame, Tier};

    use super::*;

    /// A frame with nothing of the world and three UI commands, read back.
    #[test]
    fn the_ui_pass_draws_what_soft_draws() {
        let Ok(mut w) = crate::Wgpu::headless() else {
            eprintln!("no adapter: skipped");
            return;
        };
        let mut clut = vec![0xff00_0000; jane_present::CLUT_LEN];
        clut[5] = 0xffff_0000;
        let pages = AtlasPages { clut, pages: vec![Page { w: 2, h: 1, albedo: vec![5, 0], ..Page::default() }], ..AtlasPages::default() };
        w.upload_atlas(&pages);
        let mut f = Frame::new(Tier::T2);
        f.canvas = (8, 4);
        f.ui.push(UiCmd::Fill { dst: Rect::new(0, 0, 8, 4), argb: 0xff10_2030 });
        f.ui.push(UiCmd::Fill { dst: Rect::new(0, 1, 8, 1), argb: 0x8000_0000 });
        let src = Src { x: 0, y: 0, w: 2, h: 1 };
        f.ui.push(UiCmd::Sprite { page: 0, src, dst: Rect::new(0, 3, 4, 1), ink: 0, alpha: 255, mirror: false });
        w.draw(&f);
        let mut px = Vec::new();
        let (cw, _) = w.read_back(&mut px);
        let at = |x: usize, y: usize| px[y * usize::from(cw) + x] & 0x00ff_ffff;
        assert_eq!(at(3, 0), 0x10_2030, "an opaque fill is its colour");
        // Half black over (16, 32, 48): half of each channel, in the stored bytes, as soft.
        let half = at(3, 1);
        for (s, want) in [(16, 8u32), (8, 16), (0, 24)] {
            let got = (half >> s) & 0xff;
            assert!(got.abs_diff(want) <= 1, "channel {s}: {got} not {want}");
        }
        assert_eq!(at(0, 3), 0xff_0000, "the sprite's texel, stretched two wide");
        assert_eq!(at(1, 3), 0xff_0000);
        assert_eq!(at(2, 3), 0x10_2030, "index 0 is clear");
    }
}
