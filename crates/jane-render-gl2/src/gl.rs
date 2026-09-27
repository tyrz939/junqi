//! The one module with unsafe code in the workspace's GL path (PORT.md §3.4, §4: `jane-render-gl2`
//! is `deny`, not `forbid`, for this file alone).
//!
//! Every `glow` call is an `unsafe fn`: GL is a C API, and `glow` cannot know that a context is
//! current on the calling thread or that a pointer handed to the driver is as long as the call
//! says. This module makes both true once, here, and lends the rest of the crate a safe [`Gl`]:
//!
//! - A [`Gl`] is made only by [`Gl::load`], from the function pointers SDL gives for the context
//!   the caller has just made current on this thread (`sdl.rs`), and it is neither `Send` nor
//!   `Sync`, so it is never called from a thread the context is not current on.
//! - Every call that hands the driver memory takes a slice and checks its length against the size
//!   the call names (texture uploads, read-back, buffer data); vertex attributes point into the
//!   bound buffer by offset, never into client memory, and [`Gl::draw_quads`] checks the quads it
//!   draws against the vertices and indices uploaded.
//! - Names (textures, framebuffers, programs) are only ever ones this context made. A wrong enum
//!   or a stale name is a GL error, not undefined behaviour.

#![allow(unsafe_code)]

use std::fmt::Write as _;
use std::marker::PhantomData;

use glow::HasContext;

pub use glow::{NativeBuffer as Buffer, NativeFramebuffer as Fbo, NativeProgram as Program, NativeQuery as Query};
pub use glow::{NativeTexture as Texture, NativeUniformLocation as Uniform};

/// `GL_RGBA16F`'s cousins are not used; these are the formats T1 stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Four bytes a texel.
    Rgba8,
    /// Two bytes a texel: a 16-bit index as its low and high byte (`LUMINANCE_ALPHA`).
    La8,
}

impl Format {
    fn bytes(self) -> usize {
        match self {
            Format::Rgba8 => 4,
            Format::La8 => 2,
        }
    }

    fn gl(self) -> u32 {
        match self {
            Format::Rgba8 => glow::RGBA,
            Format::La8 => glow::LUMINANCE_ALPHA,
        }
    }
}

/// How a draw combines with what is in the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// Written as is.
    Off,
    /// Straight alpha over what is there.
    Over,
    /// What is there times the colour.
    Multiply,
    /// Added to what is there.
    Add,
    /// The larger of the two, per channel (GL 1.4, `EXT_blend_minmax` on GLES 2).
    Max,
}

/// What the context is and can do.
#[derive(Clone, Debug, Default)]
pub struct Info {
    /// A GLES context (GLSL ES 1.00) rather than desktop GL (GLSL 1.20).
    pub es: bool,
    pub version: (u32, u32),
    pub renderer: String,
    pub version_string: String,
    pub max_texture: i32,
    pub texture_units: i32,
    /// `GL_MAX` blending is there.
    pub minmax: bool,
    /// `GL_TIME_ELAPSED` queries (`EXT_timer_query` or `ARB_timer_query`).
    pub timer: bool,
    /// Framebuffer objects are there (core in GLES 2; `ARB_` or `EXT_framebuffer_object` on GL 2.1).
    pub fbo: bool,
}

/// The GL context, safe to call on the thread it is current on.
#[derive(Debug)]
#[allow(clippy::struct_field_names)]
pub struct Gl {
    gl: glow::Context,
    pub info: Info,
    /// Vertices and indices in the buffers bound for drawing, for [`Gl::draw_quads`]'s check.
    verts: usize,
    quads: usize,
    /// The vertex bytes on their way to the driver.
    bytes: Vec<u8>,
    /// Not `Send`, not `Sync`: a context is current on one thread.
    _thread: PhantomData<*const ()>,
}

const TIME_ELAPSED: u32 = 0x88BF;
/// The texture unit uploads and copies bind on, so they never disturb what a draw has bound
/// (units 0 to 5).
const SCRATCH_UNIT: u32 = 7;

impl Gl {
    /// The context SDL has just made current on this thread, through its entry points. A name
    /// missing under the core spelling is looked up again with the `EXT`, `ARB` and `OES`
    /// suffixes, so a 2006 driver that has framebuffer objects only as `EXT_framebuffer_object`
    /// still gets them.
    pub(crate) fn load(video: &sdl2::VideoSubsystem) -> Gl {
        let lookup = |name: &str| -> *const std::os::raw::c_void {
            let p = video.gl_get_proc_address(name);
            if !p.is_null() {
                return p.cast();
            }
            for suffix in ["EXT", "ARB", "OES"] {
                let p = video.gl_get_proc_address(&format!("{name}{suffix}"));
                if !p.is_null() {
                    return p.cast();
                }
            }
            std::ptr::null()
        };
        // SAFETY: the caller (sdl.rs) made this context current on this thread just now, and SDL's
        // lookup returns that context's entry points (or null, which glow treats as missing).
        // glow reads GL_VERSION through them and panics, not UB, if no context is current.
        let gl = unsafe { glow::Context::from_loader_function(lookup) };
        let v = gl.version();
        let es = v.is_embedded;
        let ext = gl.supported_extensions();
        let has = |e: &str| ext.contains(e);
        // SAFETY: plain state queries on the current context.
        let (renderer, version_string, max_texture, texture_units) = unsafe {
            (
                gl.get_parameter_string(glow::RENDERER),
                gl.get_parameter_string(glow::VERSION),
                gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE),
                gl.get_parameter_i32(glow::MAX_TEXTURE_IMAGE_UNITS),
            )
        };
        let desktop3 = !es && v.major >= 3;
        let info = Info {
            es,
            version: (v.major, v.minor),
            renderer,
            version_string,
            max_texture,
            texture_units,
            minmax: !es || has("GL_EXT_blend_minmax") || v.major >= 3,
            timer: !es
                && (has("GL_EXT_timer_query")
                    || has("GL_ARB_timer_query")
                    || v.major >= 4
                    || (v.major == 3 && v.minor >= 3)),
            fbo: es || desktop3 || has("GL_ARB_framebuffer_object") || has("GL_EXT_framebuffer_object"),
        };
        // SAFETY: state settings on the current context: rows of any width are tightly packed.
        unsafe {
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::DITHER);
        }
        Gl { gl, info, verts: 0, quads: 0, bytes: Vec::new(), _thread: PhantomData }
    }

    /// A `w x h` texture of `format`, nearest or linear, clamped at its edges, contents undefined.
    pub fn texture(&self, w: u32, h: u32, format: Format, linear: bool) -> Result<Texture, String> {
        let f = format.gl();
        let filter = if linear { glow::LINEAR } else { glow::NEAREST } as i32;
        // SAFETY: a new name, bound and given storage with no data (`None`: no pointer is read).
        unsafe {
            let t = self.gl.create_texture()?;
            self.gl.active_texture(glow::TEXTURE0 + SCRATCH_UNIT);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                f as i32,
                w as i32,
                h as i32,
                0,
                f,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            Ok(t)
        }
    }

    /// Makes `t` repeat past its edges (the mist tile, which drifts). Its sides must be powers of
    /// two on GLES 2.
    pub fn repeat(&self, t: Texture) {
        // SAFETY: state settings on a texture this context made, bound on the scratch unit.
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + SCRATCH_UNIT);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
        }
    }

    /// Writes `data` (`w x h` texels of `format`, rows tight) into `t` at `(x, y)`.
    #[allow(clippy::too_many_arguments)]
    pub fn upload(&self, t: Texture, x: u32, y: u32, w: u32, h: u32, format: Format, data: &[u8]) {
        let n = w as usize * h as usize * format.bytes();
        assert!(data.len() >= n, "an upload of {w} x {h} needs {n} bytes, not {}", data.len());
        if w == 0 || h == 0 {
            return;
        }
        let f = format.gl();
        // SAFETY: `data` holds at least the `w * h` texels GL reads (unpack alignment is 1).
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + SCRATCH_UNIT);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                x as i32,
                y as i32,
                w as i32,
                h as i32,
                f,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&data[..n])),
            );
        }
    }

    pub fn delete_texture(&self, t: Texture) {
        // SAFETY: a name this context made.
        unsafe { self.gl.delete_texture(t) }
    }

    /// A framebuffer drawing into `t`; `Err` when the driver cannot render into it.
    pub fn framebuffer(&self, t: Texture) -> Result<Fbo, String> {
        // SAFETY: a new name bound, `t` attached, the status read, the default bound again.
        unsafe {
            let f = self.gl.create_framebuffer()?;
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(f));
            self.gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(t), 0);
            let status = self.gl.check_framebuffer_status(glow::FRAMEBUFFER);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            if status == glow::FRAMEBUFFER_COMPLETE {
                Ok(f)
            } else {
                self.gl.delete_framebuffer(f);
                Err(format!("framebuffer incomplete: 0x{status:x}"))
            }
        }
    }

    pub fn delete_framebuffer(&self, f: Fbo) {
        // SAFETY: a name this context made.
        unsafe { self.gl.delete_framebuffer(f) }
    }

    /// Draws into `f` (`None`: the window), `w x h` px, with no scissor.
    pub fn target(&self, f: Option<Fbo>, w: u32, h: u32) {
        // SAFETY: state settings on the current context.
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, f);
            self.gl.viewport(0, 0, w as i32, h as i32);
            self.gl.disable(glow::SCISSOR_TEST);
        }
    }

    /// Cuts drawing to `[x, x + w) x [y, y + h)` in the target's px, row 0 first (`None`: no cut).
    pub fn scissor(&self, r: Option<(i32, i32, i32, i32)>) {
        // SAFETY: state settings on the current context.
        unsafe {
            match r {
                Some((x, y, w, h)) => {
                    self.gl.enable(glow::SCISSOR_TEST);
                    self.gl.scissor(x, y, w.max(0), h.max(0));
                }
                None => self.gl.disable(glow::SCISSOR_TEST),
            }
        }
    }

    /// Fills the target (inside the scissor) with one colour.
    pub fn clear(&self, c: [f32; 4]) {
        // SAFETY: state settings and a clear on the current context.
        unsafe {
            self.gl.clear_color(c[0], c[1], c[2], c[3]);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    pub fn blend(&self, b: Blend) {
        // SAFETY: state settings on the current context.
        unsafe {
            if b == Blend::Off {
                self.gl.disable(glow::BLEND);
                return;
            }
            self.gl.enable(glow::BLEND);
            let (eq, s, d) = match b {
                Blend::Over => (glow::FUNC_ADD, glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
                Blend::Multiply => (glow::FUNC_ADD, glow::DST_COLOR, glow::ZERO),
                Blend::Add => (glow::FUNC_ADD, glow::ONE, glow::ONE),
                Blend::Max => (glow::MAX, glow::ONE, glow::ONE),
                Blend::Off => unreachable!(),
            };
            self.gl.blend_equation(eq);
            self.gl.blend_func(s, d);
        }
    }

    /// Binds `t` to texture unit `unit`.
    pub fn bind(&self, unit: u32, t: Texture) {
        // SAFETY: state settings on the current context; `unit` is below the 8 T1 asks for.
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + unit);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
            self.gl.active_texture(glow::TEXTURE0);
        }
    }

    /// Copies `[x, x + w) x [y, y + h)` of the bound framebuffer into `t` at the same place.
    pub fn copy_to(&self, t: Texture, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        // SAFETY: a copy between GL objects; no client memory.
        unsafe {
            self.gl.active_texture(glow::TEXTURE0 + SCRATCH_UNIT);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
            self.gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, x, y, x, y, w, h);
        }
    }

    /// A program from GLSL sources, attribute `k` of `attribs` at location `k`.
    pub fn program(&self, vs: &str, fs: &str, attribs: &[&str]) -> Result<Program, String> {
        // SAFETY: new names, sources handed as Rust strings (glow passes their lengths), statuses
        // and logs read back; failed shaders are deleted.
        unsafe {
            let p = self.gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
                let s = self.gl.create_shader(kind)?;
                self.gl.shader_source(s, src);
                self.gl.compile_shader(s);
                if !self.gl.get_shader_compile_status(s) {
                    let log = self.gl.get_shader_info_log(s);
                    self.gl.delete_shader(s);
                    let what = if kind == glow::VERTEX_SHADER { "vertex" } else { "fragment" };
                    let mut numbered = String::new();
                    for (i, l) in src.lines().enumerate() {
                        let _ = writeln!(numbered, "{:4} {l}", i + 1);
                    }
                    return Err(format!("{what} shader: {log}\n{numbered}"));
                }
                self.gl.attach_shader(p, s);
                shaders.push(s);
            }
            for (k, a) in attribs.iter().enumerate() {
                self.gl.bind_attrib_location(p, k as u32, a);
            }
            self.gl.link_program(p);
            for s in shaders {
                self.gl.detach_shader(p, s);
                self.gl.delete_shader(s);
            }
            if !self.gl.get_program_link_status(p) {
                let log = self.gl.get_program_info_log(p);
                self.gl.delete_program(p);
                return Err(format!("link: {log}"));
            }
            Ok(p)
        }
    }

    pub fn use_program(&self, p: Program) {
        // SAFETY: a program this context linked.
        unsafe { self.gl.use_program(Some(p)) }
    }

    pub fn uniform(&self, p: Program, name: &str) -> Option<Uniform> {
        // SAFETY: a query on a program this context linked.
        unsafe { self.gl.get_uniform_location(p, name) }
    }

    /// Sets a uniform of the program in use: an int (a sampler's unit) or 1 to 4 floats.
    pub fn set_i(&self, u: Option<&Uniform>, v: i32) {
        // SAFETY: a uniform of the program in use; a missing one (`None`) is ignored by GL.
        unsafe { self.gl.uniform_1_i32(u, v) }
    }

    pub fn set_f(&self, u: Option<&Uniform>, v: &[f32]) {
        // SAFETY: as above; the arity matches the slice.
        unsafe {
            match *v {
                [a] => self.gl.uniform_1_f32(u, a),
                [a, b] => self.gl.uniform_2_f32(u, a, b),
                [a, b, c] => self.gl.uniform_3_f32(u, a, b, c),
                [a, b, c, d] => self.gl.uniform_4_f32(u, a, b, c, d),
                _ => {}
            }
        }
    }

    /// Sets a `vec4` array uniform of the program in use from `v`, four floats an element.
    pub fn set_f4s(&self, u: Option<&Uniform>, v: &[f32]) {
        if v.len() < 4 {
            return;
        }
        // SAFETY: as above; glow hands GL the slice's length over four, whole elements only.
        unsafe { self.gl.uniform_4_f32_slice(u, &v[..v.len() / 4 * 4]) }
    }

    pub fn buffer(&self) -> Result<Buffer, String> {
        // SAFETY: a new name.
        unsafe { self.gl.create_buffer() }
    }

    /// Fills vertex buffer `b` with `data` (whole vertices of `sizes.iter().sum()` floats) and
    /// points the attributes at it ([`Gl::point`]).
    pub fn vertices(&mut self, b: Buffer, data: &[f32], sizes: &[i32]) {
        let per: i32 = sizes.iter().sum();
        assert!(per > 0 && data.len() % per as usize == 0, "vertices of {per} floats");
        self.bytes.clear();
        for f in data {
            self.bytes.extend_from_slice(&f.to_ne_bytes());
        }
        // SAFETY: the buffer's data is copied from a live slice of its own length.
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(b));
            self.gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &self.bytes, glow::STREAM_DRAW);
        }
        self.point(b, sizes, data.len() / per as usize);
    }

    /// Points attribute `k` at `sizes[k]` floats of each of the `verts` vertices in buffer `b`
    /// (filled before by [`Gl::vertices`] with the same sizes), in order.
    pub fn point(&mut self, b: Buffer, sizes: &[i32], verts: usize) {
        let per: i32 = sizes.iter().sum();
        // SAFETY: attributes point into the bound buffer by offset, inside the stride.
        unsafe {
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(b));
            let stride = per * 4;
            let mut off = 0;
            for k in 0..8u32 {
                if let Some(&n) = sizes.get(k as usize) {
                    self.gl.enable_vertex_attrib_array(k);
                    self.gl.vertex_attrib_pointer_f32(k, n, glow::FLOAT, false, stride, off);
                    off += n * 4;
                } else {
                    self.gl.disable_vertex_attrib_array(k);
                }
            }
        }
        self.verts = verts;
    }

    /// Which channels drawing writes.
    pub fn color_mask(&self, m: [bool; 4]) {
        // SAFETY: state settings on the current context.
        unsafe { self.gl.color_mask(m[0], m[1], m[2], m[3]) }
    }

    /// Fills index buffer `b` with two triangles per quad for `quads` quads (4 vertices each).
    pub fn quad_indices(&mut self, b: Buffer, quads: usize) {
        let quads = quads.min(16383);
        let mut ix: Vec<u16> = Vec::with_capacity(quads * 6);
        for q in 0..quads as u16 {
            let v = q * 4;
            ix.extend_from_slice(&[v, v + 1, v + 2, v + 2, v + 1, v + 3]);
        }
        let bytes: Vec<u8> = ix.iter().flat_map(|i| i.to_ne_bytes()).collect();
        // SAFETY: the data is copied from a live slice of its own length.
        unsafe {
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(b));
            self.gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
        }
        self.quads = quads;
    }

    /// Draws quads `first..first + n` of the vertices last given (corners in the order top-left,
    /// top-right, bottom-left, bottom-right).
    pub fn draw_quads(&self, first: usize, n: usize) {
        if n == 0 {
            return;
        }
        assert!(first + n <= self.quads && (first + n) * 4 <= self.verts, "quads past the buffers");
        // SAFETY: the indices read are inside the index buffer and name vertices inside the vertex
        // buffer (checked above).
        unsafe {
            self.gl.draw_elements(glow::TRIANGLES, (n * 6) as i32, glow::UNSIGNED_SHORT, (first * 12) as i32);
        }
    }

    /// Reads `[x, x + w) x [y, y + h)` of the bound framebuffer as RGBA bytes, row 0 first.
    pub fn read_rgba(&self, x: i32, y: i32, w: i32, h: i32, out: &mut [u8]) {
        let n = (w.max(0) * h.max(0) * 4) as usize;
        assert!(out.len() >= n, "a read of {w} x {h} needs {n} bytes");
        // SAFETY: `out` holds the `w * h * 4` bytes GL writes (pack alignment is 1).
        unsafe {
            self.gl.read_pixels(
                x,
                y,
                w,
                h,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut out[..n])),
            );
        }
    }

    /// Waits for everything issued so far.
    pub fn finish(&self) {
        // SAFETY: a sync on the current context.
        unsafe { self.gl.finish() }
    }

    pub fn query(&self) -> Option<Query> {
        // SAFETY: a new name.
        unsafe { self.gl.create_query().ok() }
    }

    /// Starts timing into `q` (one at a time).
    pub fn time_begin(&self, q: Query) {
        // SAFETY: a query this context made; `info.timer` said `TIME_ELAPSED` is there.
        unsafe { self.gl.begin_query(TIME_ELAPSED, q) }
    }

    pub fn time_end(&self) {
        // SAFETY: ends the one query begun.
        unsafe { self.gl.end_query(TIME_ELAPSED) }
    }

    /// The query's nanoseconds, if the GPU has them yet.
    pub fn time_read(&self, q: Query) -> Option<u32> {
        // SAFETY: reads of a query this context made, after it ended.
        unsafe {
            (self.gl.get_query_parameter_u32(q, glow::QUERY_RESULT_AVAILABLE) != 0)
                .then(|| self.gl.get_query_parameter_u32(q, glow::QUERY_RESULT))
        }
    }
}
