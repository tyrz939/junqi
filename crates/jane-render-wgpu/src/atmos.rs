//! The atmosphere on T2 (PRESENTATION.md §1.8, §1.9, §2): the sky backdrop and the far things on
//! it, the water, the fog and light shafts, the particles. Pipelines and their per-frame buffers
//! here; the textures they read and write are the frame's targets (`lib.rs`), and the passes run
//! in `Wgpu::draw` in this order:
//!
//! 1. **Sky**: the backdrop into a texture the canvas's width and 256 rows (row `y` is `y` px
//!    over the horizon): the gradient, the stars added, the moon, the School and the treeline.
//! 2. (G-buffer, height field, light: `lib.rs`; the light pass also writes where the sun reaches.)
//! 3. **Water**: the lit frame through, with every water px reflecting, into a second HDR target.
//! 4. **Particles on the ground**: splashes, ripples, marks, over it.
//! 5. **Fog** and light shafts, back into the first.
//! 6. **Particles in the air**, then **the rain**, over it.

use crate::gpu::{B, layout};
use crate::{ADD, HDR, OVER, module, render_pipeline, target};

const SKY: &str = include_str!("shaders/sky.wgsl");
const WATER: &str = include_str!("shaders/water.wgsl");
const FOG: &str = include_str!("shaders/fog.wgsl");
const PARTICLES: &str = include_str!("shaders/particles.wgsl");

/// Rows of the sky backdrop.
pub const SKY_ROWS: u32 = 256;

const STAR_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x4];
const PART_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
const SPRITE_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Uint32x4, 1 => Sint32x4, 2 => Uint32x4];

/// The atmosphere's pipelines, layouts and instance buffers.
#[derive(Debug)]
pub struct AtmosPipes {
    pub sky_layout: wgpu::BindGroupLayout,
    pub water_layout: wgpu::BindGroupLayout,
    pub fog_layout: wgpu::BindGroupLayout,
    pub part_layout: wgpu::BindGroupLayout,
    pub sky: wgpu::RenderPipeline,
    pub star: wgpu::RenderPipeline,
    pub far: wgpu::RenderPipeline,
    pub water: wgpu::RenderPipeline,
    pub fog: wgpu::RenderPipeline,
    pub part: wgpu::RenderPipeline,
    /// Repeats: the mist tile drifts past its edges.
    pub repeat: wgpu::Sampler,
    pub stars: wgpu::Buffer,
    pub far_sprites: wgpu::Buffer,
    pub parts: wgpu::Buffer,
    pub volumes: wgpu::Buffer,
}

fn instances(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl AtmosPipes {
    pub fn new(device: &wgpu::Device) -> AtmosPipes {
        let sky_layout = layout(device, "sky", &[B::Uniform, B::Tex, B::UintArray, B::UintArray]);
        let water_layout = layout(device, "water", &[B::Uniform, B::Tex, B::Tex, B::Tex, B::Tex]);
        let fog_layout = layout(
            device,
            "fog",
            &[B::Uniform, B::Tex, B::Tex, B::Tex, B::Sampler, B::Read, B::Read, B::Read, B::Read, B::Tex],
        );
        let part_layout = layout(device, "particles", &[B::Uniform, B::Read, B::Read, B::Read]);

        let sm = module(device, "sky", SKY);
        let sky =
            render_pipeline(device, "sky", &[&sky_layout], &sm, "vs_full", "fs_sky", &[], false, &[target(HDR, None)]);
        let star_buf = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &STAR_ATTRS,
        };
        let star =
            render_pipeline(device, "stars", &[&sky_layout], &sm, "vs_star", "fs_star", &[star_buf], true, &[target(
                HDR,
                Some(ADD),
            )]);
        let sprite_buf = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &SPRITE_ATTRS,
        };
        let far =
            render_pipeline(device, "far", &[&sky_layout], &sm, "vs_far", "fs_far", &[sprite_buf], true, &[target(
                HDR, None,
            )]);
        let wm = module(device, "water", WATER);
        let water = render_pipeline(device, "water", &[&water_layout], &wm, "vs_full", "fs_water", &[], false, &[
            target(HDR, None),
        ]);
        let fm = module(device, "fog", FOG);
        let fog =
            render_pipeline(device, "fog", &[&fog_layout], &fm, "vs_full", "fs_fog", &[], false, &[target(HDR, None)]);
        let pm = module(device, "particles", PARTICLES);
        let part_buf = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &PART_ATTRS,
        };
        let part = render_pipeline(device, "particles", &[&part_layout], &pm, "vs_part", "fs_part", &[part_buf], true, &[
            target(HDR, Some(OVER)),
            target(HDR, Some(ADD)),
        ]);
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let fog_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fog volumes"),
            size: (crate::MAX_FOG * 48) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        AtmosPipes {
            sky_layout,
            water_layout,
            fog_layout,
            part_layout,
            sky,
            star,
            far,
            water,
            fog,
            part,
            repeat,
            stars: instances(device, "stars", 128 * 16),
            far_sprites: instances(device, "far sprites", 64 * 48),
            parts: instances(device, "particles", 4096 * 48),
            volumes: fog_buf,
        }
    }

    /// Writes this frame's instances, growing a buffer that is too small.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, prep: &crate::prep::Prep) {
        let fit = |buf: &mut wgpu::Buffer, label: &str, bytes: &[u8]| {
            if bytes.len() as u64 > buf.size() {
                *buf = instances(device, label, (bytes.len() as u64).next_power_of_two());
            }
            if !bytes.is_empty() {
                queue.write_buffer(buf, 0, bytes);
            }
        };
        fit(&mut self.stars, "stars", &prep.stars);
        fit(&mut self.far_sprites, "far sprites", &prep.sky_sprites);
        fit(&mut self.parts, "particles", &prep.parts);
        queue.write_buffer(&self.volumes, 0, &prep.fog[..prep.fog.len().min(self.volumes.size() as usize)]);
    }
}
