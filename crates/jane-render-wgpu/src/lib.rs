//! T2 backend (PRESENTATION.md §1.2, §1.3, §1.7): Vulkan, DX12, Metal or GLES 3 through `wgpu`,
//! WGSL shaders.
//!
//! A frame is five steps on the GPU:
//!
//! 1. **G-buffer**: terrain chunks, then each sprite pass (its contact shadows first, then its
//!    opaque and ghost runs in draw order) into albedo, normal and height, and emissive, over the
//!    canvas and a guard band round it. Sprite albedo and emissive are 16-bit master-palette
//!    indices looked up in a 1024-entry CLUT (§1.4).
//! 2. **Height field**: a compute pass stands every lifted pixel on its ground point (a pixel
//!    `h` up at `(x, y)` stands at `(x, y + h)`), the tallest winning, so the field seen from
//!    above holds every roof, wall, post and person where it stands.
//! 3. **Light**: per canvas pixel the sky's fill, the sun or moon, and the point lights of its
//!    32 x 32 tile, each by N dot L with the light's height as z and a soft shadow traced through
//!    the height field (`shaders/light.wgsl`), emissive added unlit.
//! 4. **Bloom** on what glows, a chain of halvings and a tent back up.
//! 5. **Grade** per region and hour into the canvas, which `read_back` reads and `present`
//!    upscales to the window by sharp bilinear.

// Canvas sizes, px counts and timestamp deltas become f32 and f64 for the GPU: all far below
// the 2^23 a float holds exactly.
#![allow(clippy::cast_precision_loss)]

mod gpu;
pub mod prep;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use jane_present::frame::{CHUNK_PX, ChunkLayers};
use jane_present::{AO_TINT, AtlasPages, Backend, CLUT_LEN, Caps, Frame, FrameStats, FrameTimes, StatPass, Tier};

use crate::gpu::{B, Gpu, array_view, group, layout, texture, write_layer};
use crate::prep::{GUARD, Kind, MAX_LIGHTS, Prep, TILE, TILE_CAP};

pub use crate::gpu::block_on;

const COMMON: &str = include_str!("shaders/common.wgsl");
const GBUFFER: &str = include_str!("shaders/gbuffer.wgsl");
const SCATTER: &str = include_str!("shaders/scatter.wgsl");
const LIGHT: &str = include_str!("shaders/light.wgsl");
const POST: &str = include_str!("shaders/post.wgsl");

/// Chunk slots the GPU holds: the presenter's LRU (PRESENTATION.md §1.6).
const CHUNK_SLOTS: u32 = jane_present::chunks::LRU as u32;
/// Halvings in the bloom chain.
const BLOOM_LEVELS: usize = 5;

const ALBEDO: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const NH: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The pipelines and their layouts.
#[derive(Debug)]
struct Pipes {
    gbuf_layout: wgpu::BindGroupLayout,
    scatter_layout: wgpu::BindGroupLayout,
    light_layout: wgpu::BindGroupLayout,
    post_layout: wgpu::BindGroupLayout,
    step_layout: wgpu::BindGroupLayout,
    chunk: wgpu::RenderPipeline,
    sprite: wgpu::RenderPipeline,
    contact: wgpu::RenderPipeline,
    ghost: wgpu::RenderPipeline,
    scatter: wgpu::ComputePipeline,
    light: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    grade: wgpu::RenderPipeline,
    upscale: Option<wgpu::RenderPipeline>,
    sampler: wgpu::Sampler,
}

fn module(device: &wgpu::Device, label: &str, src: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{COMMON}\n{}\n{src}", ao_tint()).into()),
    })
}

/// The contact shadow's tint as a WGSL constant, from the one the palette holds.
fn ao_tint() -> String {
    let [r, g, b] = AO_TINT.map(|c| f32::from(c) / 256.0);
    format!("const AO_TINT = vec3<f32>({r:.6}, {g:.6}, {b:.6});")
}

/// A colour target as the pipeline's list holds it (`Some`: an attachment written to).
#[allow(clippy::unnecessary_wraps)]
fn target(format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })
}

/// Straight alpha over what is there.
const OVER: wgpu::BlendState = wgpu::BlendState::ALPHA_BLENDING;
/// What is there, multiplied.
const MULTIPLY: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::Src,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::REPLACE,
};
/// Added to what is there.
const ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::REPLACE,
};

#[allow(clippy::too_many_arguments)]
fn render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layouts: &[&wgpu::BindGroupLayout],
    module: &wgpu::ShaderModule,
    vs: &str,
    fs: &str,
    buffers: &[wgpu::VertexBufferLayout<'_>],
    strip: bool,
    targets: &[Option<wgpu::ColorTargetState>],
) -> wgpu::RenderPipeline {
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &layouts,
        immediate_size: 0,
    });
    let buffers: Vec<Option<wgpu::VertexBufferLayout<'_>>> = buffers.iter().cloned().map(Some).collect();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vs),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: if strip {
                wgpu::PrimitiveTopology::TriangleStrip
            } else {
                wgpu::PrimitiveTopology::TriangleList
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

const CHUNK_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Sint32x4];
const SPRITE_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Uint32x4, 1 => Sint32x4, 2 => Uint32x4];

impl Pipes {
    fn new(device: &wgpu::Device, surface: Option<wgpu::TextureFormat>) -> Pipes {
        let gbuf_layout = layout(
            device,
            "gbuffer",
            &[
                B::Uniform,
                B::Tex,
                B::UintArray,
                B::TexArray,
                B::UintArray,
                B::TexArray,
                B::TexArray,
                B::TexArray,
                B::TexArray,
            ],
        );
        let scatter_layout = layout(device, "scatter", &[B::Uniform, B::Tex, B::ReadWrite]);
        let light_layout =
            layout(device, "light", &[B::Uniform, B::Tex, B::Tex, B::Tex, B::Read, B::Read, B::Read, B::Read]);
        let post_layout = layout(device, "post", &[B::Uniform, B::Tex, B::Sampler, B::Tex]);
        let step_layout = layout(device, "step", &[B::Uniform]);

        let gm = module(device, "gbuffer", GBUFFER);
        let chunk_buf = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &CHUNK_ATTRS,
        };
        let sprite_buf = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &SPRITE_ATTRS,
        };
        let three = [target(ALBEDO, None), target(NH, None), target(ALBEDO, None)];
        // The albedo alone: the other two attachments are there and left as they are.
        let masked =
            |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::empty() });
        let over = [target(ALBEDO, Some(OVER)), masked(NH), masked(ALBEDO)];
        let multiply = [target(ALBEDO, Some(MULTIPLY)), masked(NH), masked(ALBEDO)];
        let chunk =
            render_pipeline(device, "chunks", &[&gbuf_layout], &gm, "vs_chunk", "fs_chunk", &[chunk_buf], true, &three);
        let sprite = render_pipeline(
            device,
            "sprites",
            &[&gbuf_layout],
            &gm,
            "vs_sprite",
            "fs_sprite",
            std::slice::from_ref(&sprite_buf),
            true,
            &three,
        );
        let contact = render_pipeline(
            device,
            "contact",
            &[&gbuf_layout],
            &gm,
            "vs_sprite",
            "fs_contact",
            std::slice::from_ref(&sprite_buf),
            true,
            &multiply,
        );
        let ghost = render_pipeline(
            device,
            "ghosts",
            &[&gbuf_layout],
            &gm,
            "vs_sprite",
            "fs_ghost",
            &[sprite_buf],
            true,
            &over,
        );

        let sm = module(device, "scatter", SCATTER);
        let spl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter"),
            bind_group_layouts: &[Some(&scatter_layout)],
            immediate_size: 0,
        });
        let scatter = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("scatter"),
            layout: Some(&spl),
            module: &sm,
            entry_point: Some("scatter"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let lm = module(device, "light", LIGHT);
        let light = render_pipeline(
            device,
            "light",
            &[&light_layout],
            &lm,
            "vs_full",
            "fs_light",
            &[],
            false,
            &[target(HDR, None), target(HDR, None)],
        );
        let pm = module(device, "post", POST);
        let pl = [&post_layout, &step_layout];
        let down =
            render_pipeline(device, "bloom down", &pl, &pm, "vs_full", "fs_down", &[], false, &[target(HDR, None)]);
        let up =
            render_pipeline(device, "bloom up", &pl, &pm, "vs_full", "fs_up", &[], false, &[target(HDR, Some(ADD))]);
        let grade =
            render_pipeline(device, "grade", &pl, &pm, "vs_full", "fs_grade", &[], false, &[target(ALBEDO, None)]);
        let upscale = surface.map(|f| {
            render_pipeline(device, "upscale", &pl, &pm, "vs_full", "fs_upscale", &[], false, &[target(f, None)])
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Pipes {
            gbuf_layout,
            scatter_layout,
            light_layout,
            post_layout,
            step_layout,
            chunk,
            sprite,
            contact,
            ghost,
            scatter,
            light,
            down,
            up,
            grade,
            upscale,
            sampler,
        }
    }
}

/// The atlas on the GPU: the CLUT and the four layers of every page as arrays.
#[derive(Debug)]
struct AtlasTex {
    clut: wgpu::TextureView,
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    emissive: wgpu::TextureView,
    height: wgpu::TextureView,
}

/// The chunk slots on the GPU, one array layer a slot, and the generation each holds.
#[derive(Debug)]
struct ChunkTex {
    albedo: wgpu::Texture,
    nh: wgpu::Texture,
    emissive: wgpu::Texture,
    held: Vec<Option<u32>>,
    scratch: Vec<u8>,
}

/// Everything sized by the canvas.
#[derive(Debug)]
struct Targets {
    canvas: (u32, u32),
    full: (u32, u32),
    galb: wgpu::TextureView,
    gnh: wgpu::TextureView,
    gem: wgpu::TextureView,
    hmap: wgpu::Buffer,
    hdr: wgpu::TextureView,
    bsrc: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    canvas_tex: wgpu::Texture,
    canvas_view: wgpu::TextureView,
    lights: wgpu::Buffer,
    tiles: wgpu::Buffer,
    tile_lights: wgpu::Buffer,
    scatter_bg: wgpu::BindGroup,
    light_bg: wgpu::BindGroup,
    /// Per bloom step: its post group and its step group, down then up.
    down: Vec<(wgpu::BindGroup, wgpu::BindGroup)>,
    up: Vec<(wgpu::BindGroup, wgpu::BindGroup)>,
    grade: (wgpu::BindGroup, wgpu::BindGroup),
    /// The window's upscale: the canvas and the scale.
    upscale: (wgpu::BindGroup, wgpu::BindGroup),
    upscale_step: wgpu::Buffer,
}

/// Timestamp queries at pass boundaries, read a frame or two late so a frame never waits for
/// them.
#[derive(Debug)]
struct Stamps {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    /// 0 idle, 1 mapping, 2 mapped.
    state: Arc<AtomicU8>,
    period_ns: f32,
    count: u32,
}

/// The frame's timestamps: the chunks, the list, the height field, the light, bloom to grade,
/// each a begin and an end.
const FRAME_STAMPS: u32 = 10;

impl Stamps {
    fn new(gpu: &Gpu, label: &str, count: u32) -> Stamps {
        let d = &gpu.device;
        let size = u64::from(count) * 8;
        Stamps {
            set: d.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some(label),
                ty: wgpu::QueryType::Timestamp,
                count,
            }),
            resolve: d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(0)),
            period_ns: gpu.queue.get_timestamp_period(),
            count,
        }
    }

    /// Free to be written this frame (the last reading has been taken).
    fn idle(&self) -> bool {
        self.state.load(Ordering::Acquire) == 0
    }

    /// The pass's timestamp writes: `begin` and `end` query indices.
    fn writes(&self, begin: Option<u32>, end: Option<u32>) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        }
    }

    /// Resolves the queries into the read buffer (before the submit).
    fn resolve(&self, enc: &mut wgpu::CommandEncoder) {
        enc.resolve_query_set(&self.set, 0..self.count, &self.resolve, 0);
        enc.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, u64::from(self.count) * 8);
    }

    /// Maps the read buffer (after the submit).
    fn map(&self) {
        self.state.store(1, Ordering::Release);
        let state = self.state.clone();
        self.read.map_async(wgpu::MapMode::Read, .., move |r| {
            state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release);
        });
    }

    /// The readings in microseconds from the first, if they are in: `out[k]` for query `k`.
    fn take(&self, out: &mut [u32]) -> bool {
        if self.state.load(Ordering::Acquire) != 2 {
            return false;
        }
        if let Ok(m) = self.read.slice(..).get_mapped_range() {
            let tick = |k: usize| u64::from_le_bytes(m[k * 8..k * 8 + 8].try_into().unwrap_or([0; 8]));
            let t0 = tick(0);
            for (k, o) in out.iter_mut().enumerate().take(self.count as usize) {
                *o = (tick(k).saturating_sub(t0) as f64 * f64::from(self.period_ns) / 1000.0) as u32;
            }
        }
        self.read.unmap();
        self.state.store(0, Ordering::Release);
        true
    }
}

/// Where `present` upscales the canvas to.
#[derive(Debug)]
enum Window {
    /// The window's surface.
    Surface { surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration },
    /// A texture of the window's size and no window: the bench's 4K output.
    Offscreen { view: wgpu::TextureView, size: (u32, u32) },
}

impl Window {
    fn size(&self) -> (u32, u32) {
        match self {
            Window::Surface { config, .. } => (config.width, config.height),
            Window::Offscreen { size, .. } => *size,
        }
    }
}

/// The `wgpu` backend.
#[derive(Debug)]
pub struct Wgpu {
    gpu: Gpu,
    pipes: Pipes,
    globals: wgpu::Buffer,
    dummy: wgpu::TextureView,
    atlas: Option<AtlasTex>,
    gbuf_bg: Option<wgpu::BindGroup>,
    chunks: ChunkTex,
    targets: Option<Targets>,
    prep: Prep,
    sprite_buf: wgpu::Buffer,
    chunk_buf: wgpu::Buffer,
    window: Option<Window>,
    stamps: Option<Stamps>,
    /// The upscale's own timestamps, in `present`.
    present_stamps: Option<Stamps>,
    /// The last upscale's time, microseconds.
    upscale_us: u32,
    times: FrameTimes,
    frames: u32,
    describe: String,
}

impl Wgpu {
    /// A backend with no window: frames are drawn into the canvas and read back (sheets, bench).
    pub fn headless() -> Result<Wgpu, String> {
        let gpu = Gpu::new(&gpu::instance(), None)?;
        Ok(Wgpu::build(gpu, None, ALBEDO))
    }

    /// A backend presenting to a window `size` px, the window handed over as a surface target
    /// (the SDL2 window through raw-window-handle, in `jane-app`).
    pub fn for_window(target: wgpu::SurfaceTarget<'static>, size: (u32, u32), vsync: bool) -> Result<Wgpu, String> {
        let instance = gpu::instance();
        let surface = instance.create_surface(target).map_err(|e| format!("no surface: {e}"))?;
        let gpu = Gpu::new(&instance, Some(&surface))?;
        let mut config = surface
            .get_default_config(&gpu.adapter, size.0.max(1), size.1.max(1))
            .ok_or("the adapter cannot present to this window")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some(&f) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = if vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        config.desired_maximum_frame_latency = 2;
        surface.configure(&gpu.device, &config);
        let format = config.format;
        Ok(Wgpu::build(gpu, Some(Window::Surface { surface, config }), format))
    }

    /// A backend with no window that still upscales every frame to `size` px (an offscreen
    /// texture), so a bench measures the whole frame at 4K output.
    pub fn headless_output(size: (u32, u32)) -> Result<Wgpu, String> {
        let gpu = Gpu::new(&gpu::instance(), None)?;
        let tex = texture(&gpu.device, "output", (size.0, size.1, 1), ALBEDO, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Wgpu::build(gpu, Some(Window::Offscreen { view, size }), ALBEDO))
    }

    fn build(gpu: Gpu, window: Option<Window>, format: wgpu::TextureFormat) -> Wgpu {
        let device = &gpu.device;
        let pipes = Pipes::new(device, window.as_ref().map(|_| format));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy = texture(device, "dummy", (1, 1, 1), HDR, wgpu::TextureUsages::TEXTURE_BINDING)
            .create_view(&wgpu::TextureViewDescriptor::default());
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let side = CHUNK_PX as u32;
        let chunks = ChunkTex {
            albedo: texture(device, "chunk albedo", (side, side, CHUNK_SLOTS), ALBEDO, usage),
            nh: texture(device, "chunk normal height", (side, side, CHUNK_SLOTS), NH, usage),
            emissive: texture(device, "chunk emissive", (side, side, CHUNK_SLOTS), ALBEDO, usage),
            held: vec![None; CHUNK_SLOTS as usize],
            scratch: Vec::new(),
        };
        let stamps = gpu.timestamps.then(|| Stamps::new(&gpu, "frame stamps", FRAME_STAMPS));
        let present_stamps = gpu.timestamps.then(|| Stamps::new(&gpu, "upscale stamps", 2));
        let sprite_buf = instance_buffer(device, "sprites", 4096 * 48);
        let chunk_buf = instance_buffer(device, "chunks", 128 * 16);
        let describe = format!("wgpu, {}", gpu.describe());
        let times = FrameTimes::new(stamps.is_some());
        Wgpu {
            gpu,
            pipes,
            globals,
            dummy,
            atlas: None,
            gbuf_bg: None,
            chunks,
            targets: None,
            prep: Prep::default(),
            sprite_buf,
            chunk_buf,
            window,
            stamps,
            present_stamps,
            upscale_us: 0,
            times,
            frames: 0,
            describe,
        }
    }

    /// `wgpu, Vulkan, <adapter>`.
    pub fn describe(&self) -> &str {
        &self.describe
    }

    /// The window was resized to `size` px.
    pub fn resize(&mut self, size: (u32, u32)) {
        if let Some(Window::Surface { surface, config }) = &mut self.window {
            config.width = size.0.max(1);
            config.height = size.1.max(1);
            surface.configure(&self.gpu.device, config);
        }
    }

    /// Shows the last canvas drawn in the window: scaled so its height fills the window's, by
    /// sharp bilinear (nearest where the scale is whole).
    pub fn present(&mut self) -> Result<(), String> {
        let t0 = Instant::now();
        if let Some(s) = &self.present_stamps {
            let mut us = [0u32; 2];
            if s.take(&mut us) {
                self.upscale_us = us[1];
            }
        }
        let (Some(win), Some(t), Some(pipe)) = (&mut self.window, &self.targets, &self.pipes.upscale) else {
            return Ok(());
        };
        let stamps = self.present_stamps.as_ref().filter(|s| s.idle());
        let size = win.size();
        let frame = match win {
            Window::Surface { surface, config } => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => Some(f),
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    surface.configure(&self.gpu.device, config);
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Validation => return Err("the surface texture failed validation".into()),
            },
            Window::Offscreen { .. } => None,
        };
        let owned = frame.as_ref().map(|f| f.texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let view = match (&owned, &*win) {
            (Some(v), _) => v,
            (None, Window::Offscreen { view, .. }) => view,
            (None, Window::Surface { .. }) => return Ok(()),
        };
        let scale = size.1 as f32 / t.canvas.1 as f32;
        let mut step = Vec::with_capacity(16);
        for v in [0.0f32, 0.0, scale, 0.0] {
            step.extend_from_slice(&v.to_le_bytes());
        }
        self.gpu.queue.write_buffer(&t.upscale_step, 0, &step);
        let mut enc =
            self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("present") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: stamps.map(|s| s.writes(Some(0), Some(1))),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &t.upscale.0, &[]);
            pass.set_bind_group(1, &t.upscale.1, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(s) = stamps {
            s.resolve(&mut enc);
        }
        self.gpu.queue.submit([enc.finish()]);
        if let Some(s) = stamps {
            s.map();
        } else if self.present_stamps.is_none() {
            // No GPU clock: the CPU's encode and submit.
            self.upscale_us = t0.elapsed().as_micros() as u32;
        }
        if let Some(f) = frame {
            self.gpu.queue.present(f);
        }
        Ok(())
    }

    fn gbuf_group(&mut self) {
        let Some(a) = &self.atlas else { return };
        let (ca, cn, ce) =
            (array_view(&self.chunks.albedo), array_view(&self.chunks.nh), array_view(&self.chunks.emissive));
        self.gbuf_bg = Some(group(
            &self.gpu.device,
            "gbuffer",
            &self.pipes.gbuf_layout,
            &[
                self.globals.as_entire_binding(),
                r(&a.clut),
                r(&a.albedo),
                r(&a.normal),
                r(&a.emissive),
                r(&a.height),
                r(&ca),
                r(&cn),
                r(&ce),
            ],
        ));
    }

    /// Makes the targets for a `w x h` canvas, if they are not that size already.
    fn fit(&mut self, (w, h): (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.canvas == (w, h)) {
            return;
        }
        let d = &self.gpu.device;
        let p = &self.pipes;
        let full = (w + 2 * GUARD, h + 2 * GUARD);
        let rt = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let galb = view(texture(d, "g albedo", (full.0, full.1, 1), ALBEDO, rt));
        let gnh = view(texture(d, "g normal height", (full.0, full.1, 1), NH, rt));
        let gem = view(texture(d, "g emissive", (full.0, full.1, 1), ALBEDO, rt));
        let hmap = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("height field"),
            size: u64::from(full.0) * u64::from(full.1) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hdr = view(texture(d, "hdr", (w, h, 1), HDR, rt));
        let bsrc = view(texture(d, "bloom source", (w, h, 1), HDR, rt));
        let sizes: Vec<(u32, u32)> = (1..=BLOOM_LEVELS).map(|k| ((w >> k).max(1), (h >> k).max(1))).collect();
        let levels: Vec<wgpu::TextureView> =
            sizes.iter().map(|&(lw, lh)| view(texture(d, "bloom", (lw, lh, 1), HDR, rt))).collect();
        let canvas_tex = texture(d, "canvas", (w, h, 1), ALBEDO, rt | wgpu::TextureUsages::COPY_SRC);
        let canvas_view = canvas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let tiles_n = u64::from(w.div_ceil(TILE) * h.div_ceil(TILE));
        let storage = |label: &str, size: u64| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let lights = storage("lights", MAX_LIGHTS as u64 * 48);
        let tiles = storage("tiles", tiles_n * 8);
        let tile_lights = storage("tile lights", tiles_n * TILE_CAP as u64 * 4);
        let g = self.globals.as_entire_binding();
        let scatter_bg = group(d, "scatter", &p.scatter_layout, &[g.clone(), r(&gnh), hmap.as_entire_binding()]);
        let light_bg = group(
            d,
            "light",
            &p.light_layout,
            &[
                g.clone(),
                r(&galb),
                r(&gnh),
                r(&gem),
                hmap.as_entire_binding(),
                lights.as_entire_binding(),
                tiles.as_entire_binding(),
                tile_lights.as_entire_binding(),
            ],
        );
        let smp = wgpu::BindingResource::Sampler(&p.sampler);
        let q = &self.gpu.queue;
        let step = |label: &str, texel: (u32, u32), scale: f32| {
            let b = d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut bytes = Vec::with_capacity(16);
            for x in [1.0 / texel.0 as f32, 1.0 / texel.1 as f32, scale, 0.0] {
                bytes.extend_from_slice(&x.to_le_bytes());
            }
            q.write_buffer(&b, 0, &bytes);
            b
        };
        let post = |label: &str, src: &wgpu::TextureView, bloom: &wgpu::TextureView| {
            group(d, label, &p.post_layout, &[g.clone(), r(src), smp.clone(), r(bloom)])
        };
        let stepg = |b: &wgpu::Buffer| group(d, "step", &p.step_layout, &[b.as_entire_binding()]);
        let mut down = Vec::new();
        for k in 0..BLOOM_LEVELS {
            let (src, size) = if k == 0 { (&bsrc, (w, h)) } else { (&levels[k - 1], sizes[k - 1]) };
            down.push((post("bloom down", src, &self.dummy), stepg(&step("down", size, 1.0))));
        }
        let mut up = Vec::new();
        for k in (1..BLOOM_LEVELS).rev() {
            up.push((post("bloom up", &levels[k], &self.dummy), stepg(&step("up", sizes[k], 1.0))));
        }
        let grade = (post("grade", &hdr, &levels[0]), stepg(&step("grade", (w, h), 1.0)));
        let upscale_step = step("upscale", (w, h), 1.0);
        let upscale = (post("upscale", &canvas_view, &self.dummy), stepg(&upscale_step));
        self.targets = Some(Targets {
            canvas: (w, h),
            full,
            galb,
            gnh,
            gem,
            hmap,
            hdr,
            bsrc,
            levels,
            canvas_tex,
            canvas_view,
            lights,
            tiles,
            tile_lights,
            scatter_bg,
            light_bg,
            down,
            up,
            grade,
            upscale,
            upscale_step,
        });
    }

    /// Uploads the chunk slots the frame draws whose generation the GPU does not hold.
    fn upload_chunks(&mut self, frame: &Frame) {
        let side = CHUNK_PX as u32;
        for &(slot, generation) in &self.prep.chunk_slots {
            let s = usize::from(slot);
            if self.chunks.held.get(s).copied().flatten() == Some(generation) || s >= self.chunks.held.len() {
                continue;
            }
            let Some(l) = frame.layers.get(s) else { continue };
            let q = &self.gpu.queue;
            let buf = &mut self.chunks.scratch;
            rgba(buf, &l.albedo);
            write_layer(q, &self.chunks.albedo, u32::from(slot), (side, side), 4, buf);
            nh(buf, l);
            write_layer(q, &self.chunks.nh, u32::from(slot), (side, side), 4, buf);
            if l.lit() {
                rgba(buf, &l.emissive);
            } else {
                buf.clear();
                buf.resize(l.albedo.len() * 4, 0);
            }
            write_layer(q, &self.chunks.emissive, u32::from(slot), (side, side), 4, buf);
            self.chunks.held[s] = Some(generation);
        }
    }

    /// Reads a past frame's timestamps if they are in, per pass (§1.12).
    fn collect_stamps(&mut self) {
        let Some(s) = &self.stamps else { return };
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
        let mut at = [0u32; FRAME_STAMPS as usize];
        if !s.take(&mut at) {
            return;
        }
        let span = |a: usize, b: usize| at[b].saturating_sub(at[a]);
        let mut pass = [0u32; StatPass::COUNT];
        pass[StatPass::Chunks as usize] = span(0, 1);
        pass[StatPass::List as usize] = span(2, 3);
        pass[StatPass::Shadows as usize] = span(4, 5);
        pass[StatPass::Light as usize] = span(6, 7);
        pass[StatPass::Grade as usize] = span(8, 9);
        pass[StatPass::Upscale as usize] = self.upscale_us;
        self.times.push_passes(at[9] + self.upscale_us, pass);
    }
}

/// A texture view as a binding.
fn r(v: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::TextureView(v)
}

fn instance_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// `0xAARRGGBB` as RGBA bytes.
fn rgba(out: &mut Vec<u8>, px: &[u32]) {
    out.clear();
    out.reserve(px.len() * 4);
    for &p in px {
        let [a, r, g, b] = p.to_be_bytes();
        out.extend_from_slice(&[r, g, b, a]);
    }
}

/// A chunk's normal and height as `(nx, ny, height, depth)`; a T0 chunk is flat.
fn nh(out: &mut Vec<u8>, l: &ChunkLayers) {
    out.clear();
    if l.lit() {
        for (n, &h) in l.normal.iter().zip(&l.height) {
            out.extend_from_slice(&[n[0], n[1], h, 2]);
        }
    } else {
        for _ in &l.albedo {
            out.extend_from_slice(&[128, 128, 0, 0]);
        }
    }
}

impl Backend for Wgpu {
    fn caps(&self) -> Caps {
        Caps { tier: Tier::T2, max_lights: MAX_LIGHTS as u16, has_readback: true, name: "wgpu" }
    }

    fn upload_atlas(&mut self, pages: &AtlasPages) {
        let d = &self.gpu.device;
        let q = &self.gpu.queue;
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let clut = texture(d, "clut", (CLUT_LEN as u32, 1, 1), ALBEDO, usage);
        let mut bytes = Vec::with_capacity(CLUT_LEN * 4);
        let mut c = pages.clut.clone();
        c.resize(CLUT_LEN, 0xff00_0000);
        rgba(&mut bytes, &c);
        write_layer(q, &clut, 0, (CLUT_LEN as u32, 1), 4, &bytes);
        let n = pages.pages.len().max(1) as u32;
        let w = pages.pages.iter().map(|p| u32::from(p.w)).max().unwrap_or(1).max(1);
        let h = pages.pages.iter().map(|p| u32::from(p.h)).max().unwrap_or(1).max(1);
        let albedo = texture(d, "atlas albedo", (w, h, n), wgpu::TextureFormat::R16Uint, usage);
        let normal = texture(d, "atlas normal", (w, h, n), wgpu::TextureFormat::Rg8Unorm, usage);
        let emissive = texture(d, "atlas emissive", (w, h, n), wgpu::TextureFormat::R16Uint, usage);
        let height = texture(d, "atlas height", (w, h, n), wgpu::TextureFormat::R8Unorm, usage);
        for (k, p) in pages.pages.iter().enumerate() {
            let (pw, ph) = (u32::from(p.w), u32::from(p.h));
            if pw == 0 || ph == 0 {
                continue;
            }
            let k = k as u32;
            let lit = p.lit();
            bytes.clear();
            for &a in &p.albedo {
                bytes.extend_from_slice(&a.to_le_bytes());
            }
            write_layer(q, &albedo, k, (pw, ph), 2, &bytes);
            bytes.clear();
            for (i, &a) in p.albedo.iter().enumerate() {
                let n = if lit { p.normal[i] } else { [128, 128] };
                let _ = a;
                bytes.extend_from_slice(&n);
            }
            write_layer(q, &normal, k, (pw, ph), 2, &bytes);
            bytes.clear();
            for (i, _) in p.albedo.iter().enumerate() {
                let e = if lit { p.emissive[i] } else { 0 };
                bytes.extend_from_slice(&e.to_le_bytes());
            }
            write_layer(q, &emissive, k, (pw, ph), 2, &bytes);
            bytes.clear();
            for (i, &a) in p.albedo.iter().enumerate() {
                bytes.push(if lit { p.height[i] } else { u8::from(a > 1) });
            }
            write_layer(q, &height, k, (pw, ph), 1, &bytes);
        }
        self.atlas = Some(AtlasTex {
            clut: clut.create_view(&wgpu::TextureViewDescriptor::default()),
            albedo: array_view(&albedo),
            normal: array_view(&normal),
            emissive: array_view(&emissive),
            height: array_view(&height),
        });
        self.gbuf_group();
    }

    fn draw(&mut self, frame: &Frame) {
        let t0 = Instant::now();
        self.collect_stamps();
        let canvas = (u32::from(frame.canvas.0).max(1), u32::from(frame.canvas.1).max(1));
        self.fit(canvas);
        self.frames = self.frames.wrapping_add(1);
        self.prep.build(frame, self.frames);
        self.upload_chunks(frame);
        let (Some(t), Some(gbuf_bg)) = (&self.targets, &self.gbuf_bg) else { return };
        let d = &self.gpu.device;
        let q = &self.gpu.queue;
        let prep = &self.prep;

        // The per-frame lists.
        let need = prep.sprites.len() as u64;
        if need > self.sprite_buf.size() {
            self.sprite_buf = instance_buffer(d, "sprites", need.next_power_of_two());
        }
        if prep.chunks.len() as u64 > self.chunk_buf.size() {
            self.chunk_buf = instance_buffer(d, "chunks", (prep.chunks.len() as u64).next_power_of_two());
        }
        if !prep.sprites.is_empty() {
            q.write_buffer(&self.sprite_buf, 0, &prep.sprites);
        }
        if !prep.chunks.is_empty() {
            q.write_buffer(&self.chunk_buf, 0, &prep.chunks);
        }
        q.write_buffer(&self.globals, 0, &prep.globals);
        q.write_buffer(&t.lights, 0, &prep.lights[..prep.lights.len().min(t.lights.size() as usize)]);
        q.write_buffer(&t.tiles, 0, &prep.tiles[..prep.tiles.len().min(t.tiles.size() as usize)]);
        q.write_buffer(
            &t.tile_lights,
            0,
            &prep.tile_lights[..prep.tile_lights.len().min(t.tile_lights.size() as usize)],
        );

        let stamps = self.stamps.as_ref().filter(|s| s.idle());
        let mut calls = 0u32;
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let [cr, cg, cb] = prep.clear;
        let attach = |view, colour: wgpu::Color| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(colour), store: wgpu::StoreOp::Store },
            })
        };
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gbuffer chunks"),
                color_attachments: &[
                    attach(&t.galb, wgpu::Color { r: cr, g: cg, b: cb, a: 1.0 }),
                    attach(&t.gnh, wgpu::Color { r: 128.0 / 255.0, g: 128.0 / 255.0, b: 0.0, a: 0.0 }),
                    attach(&t.gem, wgpu::Color::BLACK),
                ],
                depth_stencil_attachment: None,
                timestamp_writes: stamps.map(|s| s.writes(Some(0), Some(1))),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, gbuf_bg, &[]);
            if prep.n_chunks > 0 {
                pass.set_pipeline(&self.pipes.chunk);
                pass.set_vertex_buffer(0, self.chunk_buf.slice(..));
                pass.draw(0..4, 0..prep.n_chunks);
                calls += 1;
            }
        }
        {
            let keep = |view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gbuffer list"),
                color_attachments: &[keep(&t.galb), keep(&t.gnh), keep(&t.gem)],
                depth_stencil_attachment: None,
                timestamp_writes: stamps.map(|s| s.writes(Some(2), Some(3))),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, gbuf_bg, &[]);
            if prep.n_sprites > 0 {
                pass.set_vertex_buffer(0, self.sprite_buf.slice(..));
                for dr in &prep.draws {
                    pass.set_pipeline(match dr.kind {
                        Kind::Contact => &self.pipes.contact,
                        Kind::Opaque => &self.pipes.sprite,
                        Kind::Ghost => &self.pipes.ghost,
                    });
                    pass.draw(0..4, dr.range.clone());
                    calls += 1;
                }
            }
        }
        enc.clear_buffer(&t.hmap, 0, None);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("height field"),
                timestamp_writes: stamps.map(|s| wgpu::ComputePassTimestampWrites {
                    query_set: &s.set,
                    beginning_of_pass_write_index: Some(4),
                    end_of_pass_write_index: Some(5),
                }),
            });
            pass.set_pipeline(&self.pipes.scatter);
            pass.set_bind_group(0, &t.scatter_bg, &[]);
            pass.dispatch_workgroups(t.full.0.div_ceil(8), t.full.1.div_ceil(8), 1);
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("light"),
                color_attachments: &[attach(&t.hdr, wgpu::Color::BLACK), attach(&t.bsrc, wgpu::Color::BLACK)],
                depth_stencil_attachment: None,
                timestamp_writes: stamps.map(|s| s.writes(Some(6), Some(7))),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipes.light);
            pass.set_bind_group(0, &t.light_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        let fullscreen = |enc: &mut wgpu::CommandEncoder,
                          label: &str,
                          view: &wgpu::TextureView,
                          load: wgpu::LoadOp<wgpu::Color>,
                          pipe: &wgpu::RenderPipeline,
                          groups: &(wgpu::BindGroup, wgpu::BindGroup),
                          stamp: Option<wgpu::RenderPassTimestampWrites<'_>>| {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: stamp,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &groups.0, &[]);
            pass.set_bind_group(1, &groups.1, &[]);
            pass.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        for (k, g) in t.down.iter().enumerate() {
            let begin = stamps.filter(|_| k == 0).map(|s| s.writes(Some(8), None));
            fullscreen(&mut enc, "bloom down", &t.levels[k], clear, &self.pipes.down, g, begin);
        }
        for (i, g) in t.up.iter().enumerate() {
            let dst = BLOOM_LEVELS - 2 - i;
            fullscreen(&mut enc, "bloom up", &t.levels[dst], wgpu::LoadOp::Load, &self.pipes.up, g, None);
        }
        let end = stamps.map(|s| s.writes(None, Some(9)));
        fullscreen(&mut enc, "grade", &t.canvas_view, clear, &self.pipes.grade, &t.grade, end);
        // The dispatch, the light, the bloom's halvings and tents, the grade.
        calls += 1 + 1 + (2 * BLOOM_LEVELS as u32 - 1) + 1;
        if let Some(s) = stamps {
            s.resolve(&mut enc);
        }
        q.submit([enc.finish()]);
        if let Some(s) = stamps {
            s.map();
        } else if self.stamps.is_none() {
            // No GPU clock: the CPU's encode and submit, whole.
            let mut pass = [0u32; StatPass::COUNT];
            pass[StatPass::Upscale as usize] = self.upscale_us;
            self.times.push_passes(t0.elapsed().as_micros() as u32 + self.upscale_us, pass);
        }
        self.times.set_counts(calls, prep.n_lights, frame.casters.len() as u32, 0);
    }

    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        out.clear();
        let Some(t) = &self.targets else { return (0, 0) };
        let (w, h) = t.canvas;
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let d = &self.gpu.device;
        let buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = d.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("read back") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.canvas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([enc.finish()]);
        let done = Arc::new(AtomicU8::new(0));
        let flag = done.clone();
        buf.map_async(wgpu::MapMode::Read, .., move |r| flag.store(if r.is_ok() { 2 } else { 1 }, Ordering::Release));
        let _ = d.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        if done.load(Ordering::Acquire) != 2 {
            return (0, 0);
        }
        if let Ok(m) = buf.slice(..).get_mapped_range() {
            out.reserve((w * h) as usize);
            for y in 0..h as usize {
                let r = &m[y * row as usize..y * row as usize + w as usize * 4];
                out.extend(
                    r.chunks_exact(4)
                        .map(|p| 0xff00_0000 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])),
                );
            }
        }
        buf.unmap();
        (w as u16, h as u16)
    }

    fn stats(&self) -> Option<FrameStats> {
        Some(self.times.stats())
    }
}

/// Waits for every frame submitted so far to finish (the bench, to time frames end to end).
impl Wgpu {
    pub fn finish(&self) {
        let _ = self.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    }
}

/// Whether a T2 adapter is there, without making a window: `(ok, what it is or why not)`.
pub fn probe() -> Result<String, String> {
    let gpu = Gpu::new(&gpu::instance(), None)?;
    Ok(gpu.describe())
}
