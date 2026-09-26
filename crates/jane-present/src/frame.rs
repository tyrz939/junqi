//! The `Frame` (PRESENTATION.md §1.1): what one frame draws, in canvas px, naming no texture,
//! shader or pixel format. Filled into reused buffers; no allocation after the second frame.

/// The canvas height in px, always (PRESENTATION.md, the canvas): 27 cells of 16.
pub const CANVAS_H: u16 = 432;
/// The canvas width at 16:9; a wider window widens the canvas at the same height.
pub const CANVAS_W: u16 = 768;

/// The render tier a backend draws at (PRESENTATION.md §1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// `soft`: the CPU framebuffer.
    T0,
    /// `gl2`: OpenGL 2.1 / GLES 2.
    T1,
    /// `wgpu`: Vulkan 1.1 class.
    T2,
}

/// One frame's draw, in pass order.
#[derive(Debug)]
pub struct Frame {
    /// The tier this frame was built for; no pass above it is present.
    pub tier: Tier,
    /// Canvas size in px: `CANVAS_H` tall, `CANVAS_W` or wider.
    pub canvas: (u16, u16),
    /// Top-left of the view in canvas px, interpolated.
    pub camera: (i32, i32),
    /// Filled before anything else, as `0xAARRGGBB`.
    pub clear: u32,
}

impl Frame {
    /// An empty frame at `tier`, 16:9.
    pub fn new(tier: Tier) -> Frame {
        Frame { tier, canvas: (CANVAS_W, CANVAS_H), camera: (0, 0), clear: 0xff10_1014 }
    }
}
