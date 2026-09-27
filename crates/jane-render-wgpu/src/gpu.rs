//! The device: an adapter that can draw T2 (PRESENTATION.md §1.3: Vulkan, DX12, Metal or
//! GLES 3 with the limits T2 needs), its device and queue, and the small helpers the pipelines
//! are built with. No async runtime: wgpu's futures resolve on a parked thread.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};

/// Runs a future to completion on this thread (wgpu's native futures are ready at once or
/// after a poll).
pub fn block_on<F: Future>(f: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut f = pin!(f);
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// The adapter, device and queue.
#[derive(Debug)]
pub struct Gpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Timestamp queries are there: frame times from the GPU's own clock.
    pub timestamps: bool,
}

/// The backends T2 may run on.
pub fn backends() -> wgpu::Backends {
    wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::METAL | wgpu::Backends::GL
}

/// A fresh instance over the T2 backends.
pub fn instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends();
    wgpu::Instance::new(desc)
}

impl Gpu {
    /// An adapter able to draw T2 (compatible with `surface` when there is one), and its device.
    pub fn new(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<Gpu, String> {
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no T2 adapter: {e}"))?;
        let down = adapter.get_downlevel_capabilities();
        if !down.is_webgpu_compliant() {
            return Err(format!(
                "{} ({:?}) is below T2: it lacks {:?}",
                adapter.get_info().name,
                adapter.get_info().backend,
                wgpu::DownlevelFlags::compliant() - down.flags
            ));
        }
        let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamps { wgpu::Features::TIMESTAMP_QUERY } else { wgpu::Features::empty() };
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("jane t2"),
            required_features,
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| format!("no T2 device: {e}"))?;
        Ok(Gpu { adapter, device, queue, timestamps })
    }

    /// `wgpu, Vulkan` and the adapter's name: the title bar, F2, `jane bench`.
    pub fn describe(&self) -> String {
        let i = self.adapter.get_info();
        format!("{:?}, {}", i.backend, i.name)
    }
}

/// A binding in a layout built by [`layout`].
#[derive(Clone, Copy, Debug)]
pub enum B {
    Uniform,
    /// Storage, read only.
    Read,
    /// Storage, read and write.
    ReadWrite,
    /// A float texture, filterable.
    Tex,
    /// A float texture array.
    TexArray,
    /// A `u32` texture array.
    UintArray,
    Sampler,
}

/// A bind group layout of `entries` in binding order from 0, visible to every stage that can
/// see it.
pub fn layout(device: &wgpu::Device, label: &str, entries: &[B]) -> wgpu::BindGroupLayout {
    let entries: Vec<wgpu::BindGroupLayoutEntry> = entries
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let (visibility, ty) = match b {
                B::Uniform => (
                    wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                B::Read | B::ReadWrite => (
                    if matches!(b, B::Read) {
                        wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE
                    } else {
                        wgpu::ShaderStages::COMPUTE
                    },
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: matches!(b, B::Read) },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                B::Tex | B::TexArray | B::UintArray => (
                    wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    wgpu::BindingType::Texture {
                        sample_type: if matches!(b, B::UintArray) {
                            wgpu::TextureSampleType::Uint
                        } else {
                            wgpu::TextureSampleType::Float { filterable: true }
                        },
                        view_dimension: if matches!(b, B::Tex) {
                            wgpu::TextureViewDimension::D2
                        } else {
                            wgpu::TextureViewDimension::D2Array
                        },
                        multisampled: false,
                    },
                ),
                B::Sampler => {
                    (wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering))
                }
            };
            wgpu::BindGroupLayoutEntry { binding: i as u32, visibility, ty, count: None }
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
}

/// A bind group of `resources` in binding order from 0.
pub fn group(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    resources: &[wgpu::BindingResource<'_>],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry<'_>> = resources
        .iter()
        .enumerate()
        .map(|(i, r)| wgpu::BindGroupEntry { binding: i as u32, resource: r.clone() })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout, entries: &entries })
}

/// A 2D texture of `format`, `layers` deep.
pub fn texture(
    device: &wgpu::Device,
    label: &str,
    (w, h, layers): (u32, u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: layers.max(1) },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// A view of every layer of `t` as an array.
pub fn array_view(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// Writes `bytes` (rows of `w` texels of `bpp` bytes) into layer `layer` of `t`.
pub fn write_layer(queue: &wgpu::Queue, t: &wgpu::Texture, layer: u32, (w, h): (u32, u32), bpp: u32, bytes: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: t,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * bpp), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}
