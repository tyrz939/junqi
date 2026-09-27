//! T1 backend (PRESENTATION.md §1.2, §1.3, §1.7): OpenGL 2.1 or OpenGL ES 2.0 through `glow`,
//! GLSL 1.20 or GLSL ES 1.00, for GPUs from about 2006 and every Raspberry Pi.
//!
//! A frame, on the GPU:
//!
//! 1. **Albedo**: the frame's passes in order into an RGBA8 target: terrain chunks, then each
//!    sprite pass by the CLUT (the 1024-entry master palette, `LUMINANCE_ALPHA` indices), the
//!    contact shadow by its 3 x 3 cover, and between the ground and the standing things the sun's
//!    silhouettes (a span mask built from the casters' rows, applied as `soft` applies it). In the
//!    exact mode this is `soft`'s frame before its light, byte for byte (`prep.rs`).
//! 2. **Normal and height**, then **emissive**: the same terrain and sprites again into two more
//!    targets (GLES 2 has no multiple render targets).
//! 3. **Shadows**: for each of the 8 nearest casting lights, every caster's rows projected from
//!    the light onto the ground, each corner carrying how high the shadow reaches there, into one
//!    channel of two small masks by `MAX` blending.
//! 4. **Light**: a light target (the canvas's size, or half each way): the sky's fill and the sun
//!    by N dot L, then one quad per point light over its disc, N dot L with the light's height as
//!    z, T0's falloff, the spot's cone and the shadow mask, added.
//! 5. **Compose**: albedo by light, emissive added unlit, the tint (T1's only post), into the
//!    canvas, which `read_back` reads and `present` upscales to the window by sharp bilinear.
//! 6. **Ui**: the frame's `UiCmd`s over the canvas, unlit (`ui.rs`).

// Canvas sizes and px become f32 for the GPU: all far below the 2^24 a float holds exactly.
#![allow(clippy::cast_precision_loss)]

mod gl;
pub mod prep;
mod sdl;
mod shaders;
pub mod ui;

use std::time::Instant;

use jane_present::frame::CHUNK_PX;
use jane_present::{
    AO_TINT, AtlasPages, Backend, CLUT_LEN, Caps, Features, Frame, FrameStats, FrameTimes, StatPass, Tier,
};

use crate::gl::{Blend, Buffer, Fbo, Format, Gl, Program, Query, Texture, Uniform};
use crate::prep::{After, PageCpu, Prep, SLOT_ROWS, SLOTS_ACROSS, Step};
use crate::shaders as sh;

pub use crate::prep::Rows;
pub use crate::sdl::{Api, attributes};

/// Chunk slots the GPU holds: the presenter's LRU (PRESENTATION.md §1.6), in an 8 x 6 atlas.
const CHUNK_SLOTS: usize = jane_present::chunks::LRU;
const _: () = assert!(CHUNK_SLOTS <= (SLOTS_ACROSS * SLOT_ROWS) as usize);
/// How bright the emissive layer shows over the lit albedo.
const EMISSIVE_GAIN: f32 = 1.0;
/// Texture units a T1 context must have (§1.3's probe).
const UNITS: i32 = 8;

/// A linked program and the uniforms it was asked for, found once.
#[derive(Debug)]
struct Prog {
    p: Program,
    names: Vec<(&'static str, Option<Uniform>)>,
}

impl Prog {
    fn new(gl: &Gl, vs: &str, fs: &str, attribs: &[&str], uniforms: &[&'static str]) -> Result<Prog, String> {
        let es = gl.info.es;
        let p = gl.program(&sh::source(es, false, vs), &sh::source(es, true, fs), attribs)?;
        let names = uniforms.iter().map(|&n| (n, gl.uniform(p, n))).collect();
        Ok(Prog { p, names })
    }

    fn u(&self, name: &str) -> Option<&Uniform> {
        self.names.iter().find(|(n, _)| *n == name).and_then(|(_, u)| u.as_ref())
    }
}

/// Every program.
#[derive(Debug)]
struct Progs {
    chunk: Prog,
    sprite: Prog,
    span: Prog,
    silhouette: Prog,
    ambient: Prog,
    point: Prog,
    compose: Prog,
    ui: Prog,
    upscale: Prog,
    sky: Prog,
    far: Prog,
    shape: Prog,
    fog: Prog,
}

/// The sky's uniforms, which the backdrop's programs and the compose share.
const SKY_UNIFORMS: [&str; 4] = ["u_zenith", "u_horizon", "u_glow", "u_glow_x"];

impl Progs {
    fn new(gl: &Gl) -> Result<Progs, String> {
        let light = |fs: &str| format!("{}{fs}", sh::LIGHT_COMMON);
        let graded = |fs: &str| format!("{}{fs}", sh::GRADE);
        let skyed = |fs: &str| format!("{}{}{fs}", sh::GRADE, sh::SKY_COMMON);
        let with_sky = |u: &[&'static str]| -> Vec<&'static str> { u.iter().chain(&SKY_UNIFORMS).copied().collect() };
        Ok(Progs {
            chunk: Prog::new(gl, sh::CHUNK_VS, sh::CHUNK_FS, &sh::CHUNK_ATTRS, &["u_canvas", "u_tex", "u_src"])?,
            sprite: Prog::new(
                gl,
                sh::SPRITE_VS,
                sh::SPRITE_FS,
                &sh::SPRITE_ATTRS,
                &["u_canvas", "u_alb", "u_pnh", "u_pem", "u_clut", "u_snap", "u_page", "u_ao", "u_mode"],
            )?,
            span: Prog::new(gl, sh::SPAN_VS, sh::SPAN_FS, &sh::SPAN_ATTRS, &["u_canvas"])?,
            silhouette: Prog::new(
                gl,
                sh::RECT_VS,
                sh::SILHOUETTE_FS,
                &sh::RECT_ATTRS,
                &["u_size", "u_mask", "u_snap", "u_height", "u_k", "u_box", "u_feather"],
            )?,
            ambient: Prog::new(
                gl,
                sh::RECT_VS,
                &light(sh::AMBIENT_FS),
                &sh::RECT_ATTRS,
                &["u_size", "u_nh", "u_canvas", "u_scale", "u_normals", "u_fill", "u_sun", "u_suncol"],
            )?,
            point: Prog::new(
                gl,
                sh::LIGHT_VS,
                &light(sh::POINT_FS),
                &sh::LIGHT_ATTRS,
                &["u_nh", "u_canvas", "u_scale", "u_normals", "u_mask_a", "u_mask_b", "u_emi", "u_wet"],
            )?,
            compose: Prog::new(
                gl,
                sh::RECT_VS,
                &skyed(sh::COMPOSE_FS),
                &sh::RECT_ATTRS,
                &[
                    "u_size",
                    "u_alb",
                    "u_light",
                    "u_emi",
                    "u_nh",
                    "u_sky",
                    "u_tint",
                    "u_lift",
                    "u_egain",
                    "u_info",
                    "u_weather",
                    "u_cam",
                    "u_fill",
                ],
            )?,
            sky: Prog::new(gl, sh::RECT_VS, &skyed(sh::SKY_FS), &sh::RECT_ATTRS, &with_sky(&["u_size"]))?,
            far: Prog::new(
                gl,
                sh::SPRITE_VS,
                &skyed(sh::FAR_FS),
                &sh::SPRITE_ATTRS,
                &with_sky(&["u_canvas", "u_alb", "u_pem", "u_clut", "u_page", "u_fill", "u_haze", "u_egain"]),
            )?,
            shape: Prog::new(
                gl,
                sh::SHAPE_VS,
                &graded(sh::SHAPE_FS),
                &sh::SHAPE_ATTRS,
                &["u_canvas", "u_light", "u_lit", "u_tint", "u_lift"],
            )?,
            fog: Prog::new(
                gl,
                sh::RECT_VS,
                &graded(sh::FOG_FS),
                &sh::RECT_ATTRS,
                &[
                    "u_size",
                    "u_nh",
                    "u_light",
                    "u_mist",
                    "u_vrect[0]",
                    "u_vcol[0]",
                    "u_vshape[0]",
                    "u_n",
                    "u_move",
                    "u_base",
                    "u_tint",
                    "u_lift",
                ],
            )?,
            ui: Prog::new(
                gl,
                sh::UI_VS,
                sh::UI_FS,
                &sh::UI_ATTRS,
                &["u_canvas", "u_alb", "u_clut", "u_img", "u_page", "u_img_size"],
            )?,
            upscale: Prog::new(
                gl,
                sh::RECT_VS,
                sh::UPSCALE_FS,
                &sh::RECT_ATTRS,
                &["u_size", "u_src", "u_src_size", "u_scale", "u_win_h", "u_sharp"],
            )?,
        })
    }
}

/// One atlas page on the GPU: its albedo and emissive indices, its normal and height, and how
/// its rows are laid in strips when it is taller than a texture may be.
#[derive(Debug)]
struct PageGl {
    alb: Texture,
    nh: Texture,
    em: Texture,
    /// The page's width, the strip height, the texture's width and height.
    shape: [f32; 4],
}

/// A texture and the framebuffer that draws into it.
#[derive(Clone, Copy, Debug)]
struct Target {
    tex: Texture,
    fbo: Fbo,
    w: u32,
    h: u32,
}

impl Target {
    fn new(gl: &Gl, w: u32, h: u32, linear: bool) -> Result<Target, String> {
        let tex = gl.texture(w, h, Format::Rgba8, linear)?;
        let fbo = gl.framebuffer(tex)?;
        Ok(Target { tex, fbo, w, h })
    }

    fn free(self, gl: &Gl) {
        gl.delete_framebuffer(self.fbo);
        gl.delete_texture(self.tex);
    }
}

/// Everything sized by the canvas.
#[derive(Debug)]
struct Targets {
    canvas: (u32, u32),
    /// Canvas px per light-target px.
    scale: u32,
    alb: Target,
    nh: Target,
    emi: Target,
    /// What the albedo was, for the texels that read what is under them.
    snap: Texture,
    /// The silhouettes' span mask, the canvas's size.
    sil: Target,
    light: Target,
    mask_a: Target,
    mask_b: Target,
    out: Target,
    /// The sky backdrop: the canvas's width, 256 rows, row `y` `y` px over the horizon.
    sky: Target,
}

impl Targets {
    fn new(gl: &Gl, (w, h): (u32, u32), scale: u32) -> Result<Targets, String> {
        let (lw, lh) = (w.div_ceil(scale), h.div_ceil(scale));
        Ok(Targets {
            canvas: (w, h),
            scale,
            alb: Target::new(gl, w, h, false)?,
            nh: Target::new(gl, w, h, false)?,
            emi: Target::new(gl, w, h, false)?,
            snap: gl.texture(w, h, Format::Rgba8, false)?,
            sil: Target::new(gl, w, h, false)?,
            light: Target::new(gl, lw, lh, true)?,
            mask_a: Target::new(gl, lw, lh, false)?,
            mask_b: Target::new(gl, lw, lh, false)?,
            out: Target::new(gl, w, h, true)?,
            sky: Target::new(gl, w, SKY_ROWS, false)?,
        })
    }

    fn free(self, gl: &Gl) {
        for t in [self.alb, self.nh, self.emi, self.sil, self.light, self.mask_a, self.mask_b, self.out, self.sky] {
            t.free(gl);
        }
        gl.delete_texture(self.snap);
    }
}

/// The GPU's own clock per section, when the context has `GL_TIME_ELAPSED` (read four frames
/// late so a frame never waits for it).
#[derive(Debug)]
struct Timer {
    /// Per frame of the ring, per section: its query, and whether it was begun this time round.
    q: Vec<[(Query, bool); SECTIONS]>,
    open: bool,
}

/// The sections a frame is timed in, and the `StatPass` each reports as.
const SECTIONS: usize = 9;
const SECTION_PASS: [StatPass; SECTIONS] = [
    StatPass::Chunks,
    StatPass::List,
    StatPass::Shadows,
    StatPass::Light,
    // The compose: the light by the albedo, the water, the sky, the wet ground and the grade.
    StatPass::Grade,
    StatPass::Ui,
    StatPass::Upscale,
    // The backdrop into its target.
    StatPass::Sky,
    // Over the canvas: the particles and the fog.
    StatPass::Fx,
];
/// The sections' indices.
const SEC_GRADE: usize = 4;
const SEC_UI: usize = 5;
const SEC_UPSCALE: usize = 6;
const SEC_SKY: usize = 7;
const SEC_AFTER: usize = 8;
/// Rows of the sky backdrop's target.
const SKY_ROWS: u32 = 256;
const RING: usize = 4;

/// The vertex buffers, one per layout, and the quad index buffer.
#[derive(Debug)]
struct Buffers {
    chunk: Buffer,
    sprite: Buffer,
    span: Buffer,
    light: Buffer,
    ui: Buffer,
    rect: Buffer,
    index: Buffer,
    /// Particles, stars and the moon.
    shape: Buffer,
    /// The far things on the backdrop.
    far: Buffer,
}

/// A UI image on the GPU: the generation it holds, its texture and its size.
type ImageGl = (u32, Texture, (u16, u16));

/// The `gl2` backend.
#[derive(Debug)]
pub struct Gl2 {
    // Dropped first: GL objects die with the context, which dies with `ctx`.
    gl: Gl,
    ctx: sdl::Context,
    rows: Rows,
    progs: Progs,
    bufs: Buffers,
    clut: Option<Texture>,
    /// The mist tile the fog drifts (§1.9), repeating.
    mist: Option<Texture>,
    pages_gl: Vec<PageGl>,
    pages: Vec<PageCpu>,
    /// The chunk atlas: albedo, normal and height, emissive; and the generation each slot holds.
    chunk_tex: [Texture; 3],
    held: Vec<Option<u32>>,
    targets: Option<Targets>,
    prep: Prep,
    ui_v: Vec<f32>,
    ui_draws: Vec<ui::UiDraw>,
    /// The frame's UI images by slot: generation, texture, size.
    images: Vec<Option<ImageGl>>,
    rect_v: Vec<f32>,
    scratch: Vec<u8>,
    times: FrameTimes,
    timer: Option<Timer>,
    frames: u64,
    /// The timer ring's slot this frame writes.
    slot: usize,
    /// CPU µs per section this frame (the fallback clock), and the upscale's from `present`.
    cpu_us: [u32; SECTIONS],
    calls: u32,
    describe: String,
    /// Where `present` upscales to when there is no window to show: an offscreen target of this
    /// size (the bench's 4K output).
    offscreen: Option<(u32, u32)>,
    offscreen_target: Option<Target>,
}

impl Gl2 {
    /// A backend with no window: frames are drawn into textures and read back (sheets, tests,
    /// the bench). A hidden SDL window holds the context.
    pub fn headless(api: Api) -> Result<Gl2, String> {
        let (ctx, gl) = sdl::Context::headless(api)?;
        Gl2::build(ctx, gl)
    }

    /// A backend with no window that still upscales every frame to `size` px (an offscreen
    /// target), so a bench measures the whole frame at its output size.
    pub fn headless_output(api: Api, size: (u32, u32)) -> Result<Gl2, String> {
        let mut g = Gl2::headless(api)?;
        g.offscreen = Some(size);
        g.offscreen_target = Some(Target::new(&g.gl, size.0.max(1), size.1.max(1), false)?);
        Ok(g)
    }

    /// A backend presenting to `window` (made with `.opengl()`, the attributes of
    /// [`attributes`] set before it was), waiting for vsync or not.
    pub fn for_window(
        video: &sdl2::VideoSubsystem,
        window: &sdl2::video::Window,
        api: Api,
        vsync: bool,
    ) -> Result<Gl2, String> {
        let (ctx, gl) = sdl::Context::for_window(video, window, api, vsync)?;
        Gl2::build(ctx, gl)
    }

    fn build(ctx: sdl::Context, mut gl: Gl) -> Result<Gl2, String> {
        let i = &gl.info;
        let what = format!("{} ({})", i.renderer, i.version_string);
        // The probe's requirements (§1.3): framebuffer objects, eight texture units, a fragment
        // shader (the programs below), textures 2048 on a side (the chunk atlas).
        if !i.fbo {
            return Err(format!("{what}: no framebuffer objects"));
        }
        if i.texture_units < UNITS {
            return Err(format!("{what}: {} texture units, T1 needs {UNITS}", i.texture_units));
        }
        if i.max_texture < 2048 {
            return Err(format!("{what}: textures up to {}, T1 needs 2048", i.max_texture));
        }
        let progs = Progs::new(&gl).map_err(|e| format!("{what}: {e}"))?;
        let bufs = Buffers {
            chunk: gl.buffer()?,
            sprite: gl.buffer()?,
            span: gl.buffer()?,
            light: gl.buffer()?,
            ui: gl.buffer()?,
            rect: gl.buffer()?,
            index: gl.buffer()?,
            shape: gl.buffer()?,
            far: gl.buffer()?,
        };
        gl.quad_indices(bufs.index, 16383);
        let side = SLOTS_ACROSS * 256;
        let rows_px = SLOT_ROWS * 256;
        let chunk_tex = [
            gl.texture(side, rows_px, Format::Rgba8, false)?,
            gl.texture(side, rows_px, Format::Rgba8, false)?,
            gl.texture(side, rows_px, Format::Rgba8, false)?,
        ];
        let timer = gl.info.timer.then(|| {
            let mut q = Vec::new();
            for _ in 0..RING {
                let mut row = Vec::new();
                for _ in 0..SECTIONS {
                    row.push((gl.query()?, false));
                }
                q.push(<[(Query, bool); SECTIONS]>::try_from(row).ok()?);
            }
            Some(Timer { q, open: false })
        });
        let timer = timer.flatten();
        let kind = if gl.info.es { "GLES" } else { "GL" };
        let describe = format!("gl2, {kind} {}.{}, {}", gl.info.version.0, gl.info.version.1, gl.info.renderer);
        // Tile GPUs flush the whole target for every copy out of it: the fast albedo there.
        let tiled = gl.info.es
            || ["VC4", "V3D", "Mali", "Adreno", "PowerVR", "Vivante"].iter().any(|t| gl.info.renderer.contains(t));
        let rows = Rows { exact: !tiled, half_light: tiled, reflect: !tiled, ..Rows::T1 };
        let times = FrameTimes::new(timer.is_some());
        Ok(Gl2 {
            gl,
            ctx,
            rows,
            progs,
            bufs,
            clut: None,
            mist: None,
            pages_gl: Vec::new(),
            pages: Vec::new(),
            chunk_tex,
            held: vec![None; CHUNK_SLOTS],
            targets: None,
            prep: Prep::default(),
            ui_v: Vec::new(),
            ui_draws: Vec::new(),
            images: Vec::new(),
            rect_v: Vec::with_capacity(8),
            scratch: Vec::new(),
            times,
            timer,
            frames: 0,
            slot: 0,
            cpu_us: [0; SECTIONS],
            calls: 0,
            describe,
            offscreen: None,
            offscreen_target: None,
        })
    }

    /// `gl2, GL 4.6, <renderer>`: the title bar, F2, `jane bench`.
    pub fn describe(&self) -> &str {
        &self.describe
    }

    /// The rows in force.
    pub fn rows(&self) -> Rows {
        self.rows
    }

    /// Sets the rows (the Controls screen, `config.json`, the bench's degrade steps).
    pub fn set_rows(&mut self, rows: Rows) {
        self.rows = rows;
    }

    /// Waits for every frame issued so far (the bench, to time frames end to end).
    pub fn finish(&self) {
        self.gl.finish();
    }

    /// The albedo target of the last frame (terrain, sprites and silhouettes, before any light),
    /// as `0xAARRGGBB` rows: the pass `soft` draws before its lightmap.
    pub fn read_albedo(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        let Some(t) = &self.targets else { return (0, 0) };
        let a = t.alb;
        self.read(a, out)
    }

    fn read(&mut self, t: Target, out: &mut Vec<u32>) -> (u16, u16) {
        let (w, h) = (t.w, t.h);
        self.gl.target(Some(t.fbo), w, h);
        self.scratch.resize((w * h * 4) as usize, 0);
        self.gl.read_rgba(0, 0, w as i32, h as i32, &mut self.scratch);
        out.clear();
        out.extend(
            self.scratch
                .chunks_exact(4)
                .map(|p| 0xff00_0000 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])),
        );
        (w as u16, h as u16)
    }

    /// Shows the last canvas drawn in the window: scaled so its height fills the window's, by
    /// sharp bilinear (nearest where the scale is whole, or where the `sharp` row is off).
    pub fn present(&mut self) -> Result<(), String> {
        let t0 = Instant::now();
        let Some(t) = &self.targets else { return Ok(()) };
        let out = t.out;
        let (dst, (ww, wh)) = match self.offscreen_target {
            Some(o) => (Some(o.fbo), (o.w, o.h)),
            None => (None, self.ctx.size()),
        };
        self.time_begin(SEC_UPSCALE);
        self.gl.target(dst, ww, wh);
        self.gl.clear([0.0, 0.0, 0.0, 1.0]);
        let scale = wh as f32 / out.h as f32;
        let whole = (scale - scale.round()).abs() < 1e-4;
        let p = &self.progs.upscale;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Off);
        self.gl.bind(0, out.tex);
        self.gl.set_i(p.u("u_src"), 0);
        self.gl.set_f(p.u("u_src_size"), &[out.w as f32, out.h as f32]);
        self.gl.set_f(p.u("u_scale"), &[scale]);
        self.gl.set_f(p.u("u_win_h"), &[wh as f32]);
        self.gl.set_f(p.u("u_sharp"), &[if self.rows.sharp && !whole { 1.0 } else { 0.0 }]);
        self.gl.set_f(p.u("u_size"), &[ww as f32, wh as f32]);
        self.rect(0.0, 0.0, ww as f32, wh as f32);
        self.time_end();
        if self.offscreen.is_none() {
            self.ctx.swap();
        }
        self.cpu_us[SEC_UPSCALE] = t0.elapsed().as_micros() as u32;
        Ok(())
    }

    /// Draws one rect `(x0, y0)..(x1, y1)` in the target's px with the program in use.
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.rect_v.clear();
        self.rect_v.extend_from_slice(&[x0, y0, x1, y0, x0, y1, x1, y1]);
        self.gl.vertices(self.bufs.rect, &self.rect_v, &sh::RECT_SIZES);
        self.gl.draw_quads(0, 1);
        self.calls += 1;
    }

    /// Makes the targets for a `w x h` canvas at the rows' light scale, if they are not so.
    fn fit(&mut self, canvas: (u32, u32)) -> Result<(), String> {
        let scale = if self.rows.half_light { 2 } else { 1 };
        if self.targets.as_ref().is_some_and(|t| t.canvas == canvas && t.scale == scale) {
            return Ok(());
        }
        if let Some(t) = self.targets.take() {
            t.free(&self.gl);
        }
        self.targets = Some(Targets::new(&self.gl, canvas, scale)?);
        Ok(())
    }

    /// Uploads the chunk slots the frame draws whose generation the GPU does not hold.
    fn upload_chunks(&mut self, frame: &Frame) {
        let side = CHUNK_PX as u32;
        for k in 0..self.prep.chunk_slots.len() {
            let (slot, generation) = self.prep.chunk_slots[k];
            let s = usize::from(slot);
            if s >= self.held.len() || self.held[s] == Some(generation) {
                continue;
            }
            let Some(l) = frame.layers.get(s) else { continue };
            let (x, y) = ((slot as u32 % SLOTS_ACROSS) * side, (slot as u32 / SLOTS_ACROSS) * side);
            let buf = &mut self.scratch;
            argb_bytes(buf, &l.albedo);
            self.gl.upload(self.chunk_tex[0], x, y, side, side, Format::Rgba8, buf);
            buf.clear();
            if l.lit() {
                for (n, &h) in l.normal.iter().zip(&l.height) {
                    buf.extend_from_slice(&[n[0], n[1], h, 2]);
                }
            } else {
                buf.resize(l.albedo.len() * 4, 0);
                for p in buf.chunks_exact_mut(4) {
                    p.copy_from_slice(&[128, 128, 0, 0]);
                }
            }
            self.gl.upload(self.chunk_tex[1], x, y, side, side, Format::Rgba8, buf);
            if l.lit() {
                // The emissive's alpha carries the surface byte (§1.8): how the px takes rain, its
                // depth in water, or beyond the zone, where the sky shows.
                argb_bytes(buf, &l.emissive);
                for (p, &s) in buf.chunks_exact_mut(4).zip(&l.surface) {
                    p[3] = s;
                }
            } else {
                buf.clear();
                buf.resize(l.albedo.len() * 4, 0);
            }
            self.gl.upload(self.chunk_tex[2], x, y, side, side, Format::Rgba8, buf);
            self.held[s] = Some(generation);
        }
    }

    fn time_begin(&mut self, section: usize) {
        let slot = self.slot;
        if let Some(t) = &mut self.timer
            && !t.open
        {
            let (q, _) = t.q[slot][section];
            self.gl.time_begin(q);
            t.q[slot][section].1 = true;
            t.open = true;
        }
    }

    fn time_end(&mut self) {
        if let Some(t) = &mut self.timer
            && t.open
        {
            self.gl.time_end();
            t.open = false;
        }
    }

    /// Reads the times of the frame that last wrote this frame's slot of the ring (`RING`
    /// frames back) into the stats, if they are in; if not, they are dropped.
    fn collect(&mut self) {
        let slot = self.slot;
        let Some(t) = &mut self.timer else {
            return;
        };
        if !t.q[slot].iter().any(|q| q.1) {
            return;
        }
        let mut pass = [0u32; StatPass::COUNT];
        let (mut whole, mut all) = (0, true);
        for (k, (q, begun)) in t.q[slot].iter_mut().enumerate() {
            if !*begun {
                continue;
            }
            *begun = false;
            match self.gl.time_read(*q) {
                Some(ns) => {
                    let us = ns / 1000;
                    pass[SECTION_PASS[k] as usize] += us;
                    whole += us;
                }
                None => all = false,
            }
        }
        if all {
            self.times.push_passes(whole, pass);
        }
    }

    /// Binds atlas page `page`'s three layers to units 0 to 2 and the CLUT to 3, and sets the
    /// sprite program's `u_page`.
    fn bind_page(&self, page: u8, u_page: Option<&Uniform>) {
        if let Some(p) = self.pages_gl.get(usize::from(page)) {
            self.gl.bind(0, p.alb);
            self.gl.bind(1, p.nh);
            self.gl.bind(2, p.em);
            self.gl.set_f(u_page, &p.shape);
        }
        if let Some(c) = self.clut {
            self.gl.bind(3, c);
        }
    }

    /// The sprite program in use, the sprite quads pointed at, its samplers set.
    fn sprite_program(&mut self, canvas: (u32, u32), mode: f32) {
        let p = &self.progs.sprite;
        self.gl.use_program(p.p);
        self.gl.set_f(p.u("u_canvas"), &[canvas.0 as f32, canvas.1 as f32]);
        for (k, n) in ["u_alb", "u_pnh", "u_pem", "u_clut", "u_snap"].iter().enumerate() {
            self.gl.set_i(p.u(n), k as i32);
        }
        self.gl.set_f(p.u("u_ao"), &AO_TINT.map(f32::from));
        self.gl.set_f(p.u("u_mode"), &[mode]);
        let verts = self.prep.sprite_v.len() / 12;
        self.gl.point(self.bufs.sprite, &sh::SPRITE_SIZES, verts);
    }

    /// The chunk program in use over `tex`, the chunk quads pointed at.
    fn chunk_program(&mut self, canvas: (u32, u32), tex: Texture) {
        let p = &self.progs.chunk;
        self.gl.use_program(p.p);
        self.gl.set_f(p.u("u_canvas"), &[canvas.0 as f32, canvas.1 as f32]);
        self.gl.set_f(p.u("u_tex"), &[(SLOTS_ACROSS * 256) as f32, (SLOT_ROWS * 256) as f32]);
        self.gl.set_i(p.u("u_src"), 0);
        self.gl.bind(0, tex);
        let verts = self.prep.chunk_v.len() / 4;
        self.gl.point(self.bufs.chunk, &sh::CHUNK_SIZES, verts);
    }

    /// Step 1: the albedo target.
    fn albedo(&mut self, t: &Targets, clear: u32) {
        let c = t.canvas;
        // The silhouettes climb what the terrain raises: its heights into the normal target first
        // (step 2 draws that target again from the start).
        if self.prep.steps.iter().any(|s| matches!(s, Step::Silhouette { .. })) {
            self.gl.target(Some(t.nh.fbo), c.0, c.1);
            self.gl.clear([128.0 / 255.0, 128.0 / 255.0, 0.0, 0.0]);
            self.gl.blend(Blend::Off);
            if !self.prep.solid_chunks.is_empty() {
                self.chunk_program(c, self.chunk_tex[1]);
                for k in 0..self.prep.solid_chunks.len() {
                    let r = self.prep.solid_chunks[k].clone();
                    self.gl.draw_quads(r.start, r.len());
                    self.calls += 1;
                }
            }
        }
        self.gl.target(Some(t.alb.fbo), c.0, c.1);
        let [_, r, g, b] = clear.to_be_bytes().map(|v| f32::from(v) / 255.0);
        self.gl.clear([r, g, b, 1.0]);
        let steps = std::mem::take(&mut self.prep.steps);
        let mut i = 0;
        // The terrain first (its own section), then the rest.
        let t_chunks = Instant::now();
        self.time_begin(0);
        if matches!(steps.first(), Some(Step::Chunks(_))) {
            self.chunk_program(c, self.chunk_tex[0]);
            self.gl.blend(Blend::Off);
            while let Some(Step::Chunks(r)) = steps.get(i) {
                self.gl.draw_quads(r.start, r.len());
                self.calls += 1;
                i += 1;
            }
        }
        self.time_end();
        self.cpu_us[0] = t_chunks.elapsed().as_micros() as u32;
        let t_list = Instant::now();
        self.time_begin(1);
        let mut current: Option<(u8, bool)> = None;
        for step in &steps[i..] {
            match step {
                Step::Chunks(r) => {
                    self.chunk_program(c, self.chunk_tex[0]);
                    self.gl.blend(Blend::Off);
                    self.gl.draw_quads(r.start, r.len());
                    self.calls += 1;
                    current = None;
                }
                Step::Copy(x, y, w, h) => self.gl.copy_to(t.snap, *x, *y, *w, *h),
                Step::Sprites { page, quads, mode, blend } => {
                    if current.is_none() {
                        self.sprite_program(c, *mode);
                        self.gl.bind(4, t.snap);
                    }
                    if current.map(|p| p.0) != Some(*page) {
                        self.bind_page(*page, self.progs.sprite.u("u_page"));
                    }
                    current = Some((*page, true));
                    self.gl.set_f(self.progs.sprite.u("u_mode"), &[*mode]);
                    self.gl.blend(*blend);
                    self.gl.draw_quads(quads.start, quads.len());
                    self.calls += 1;
                }
                Step::Silhouette { spans, mask, apply, k, feather } => {
                    self.silhouette(t, spans.clone(), *mask, *apply, *k, *feather);
                    current = None;
                }
            }
        }
        self.prep.steps = steps;
        self.gl.blend(Blend::Off);
        // Steps 2: the normal and height, and the emissive, of the same terrain and sprites.
        for (target, tex, mode, clear) in [
            (t.nh, self.chunk_tex[1], 4.0, [128.0 / 255.0, 128.0 / 255.0, 0.0, 0.0]),
            // Its alpha is the surface byte: 255 a thing (every sprite writes it), and where
            // nothing is drawn, 254: beyond the zone.
            (t.emi, self.chunk_tex[2], 5.0, [0.0, 0.0, 0.0, 254.0 / 255.0]),
        ] {
            self.gl.target(Some(target.fbo), c.0, c.1);
            self.gl.clear(clear);
            if !self.prep.solid_chunks.is_empty() {
                self.chunk_program(c, tex);
                for k in 0..self.prep.solid_chunks.len() {
                    let r = self.prep.solid_chunks[k].clone();
                    self.gl.draw_quads(r.start, r.len());
                    self.calls += 1;
                }
            }
            if !self.prep.solid.is_empty() {
                self.sprite_program(c, mode);
                let mut page = None;
                for k in 0..self.prep.solid.len() {
                    let (p, r) = self.prep.solid[k].clone();
                    if page != Some(p) {
                        self.bind_page(p, self.progs.sprite.u("u_page"));
                        page = Some(p);
                    }
                    self.gl.draw_quads(r.start, r.len());
                    self.calls += 1;
                }
            }
        }
        self.time_end();
        self.cpu_us[1] = t_list.elapsed().as_micros() as u32;
    }

    /// The silhouettes: the spans into the mask, the largest kept, then the mask applied to the
    /// albedo from a snapshot of it, each px at the ground under it (the terrain's heights in the
    /// normal target).
    fn silhouette(
        &mut self,
        t: &Targets,
        spans: std::ops::Range<usize>,
        mask: (i32, i32, i32, i32),
        (x, y, w, h): (i32, i32, i32, i32),
        k: [f32; 3],
        feather: i32,
    ) {
        let c = t.canvas;
        self.gl.target(Some(t.sil.fbo), c.0, c.1);
        self.gl.scissor(Some((x, y, w, h)));
        self.gl.clear([0.0; 4]);
        self.gl.scissor(None);
        let p = &self.progs.span;
        self.gl.use_program(p.p);
        self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
        self.gl.point(self.bufs.span, &sh::SPAN_SIZES, self.prep.span_v.len() / 4);
        self.gl.blend(if self.gl.info.minmax { Blend::Max } else { Blend::Off });
        self.gl.draw_quads(spans.start, spans.len());
        self.calls += 1;
        self.gl.target(Some(t.alb.fbo), c.0, c.1);
        self.gl.copy_to(t.snap, x, y, w, h);
        let p = &self.progs.silhouette;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Off);
        self.gl.bind(0, t.sil.tex);
        self.gl.bind(1, t.snap);
        self.gl.bind(2, t.nh.tex);
        self.gl.set_i(p.u("u_mask"), 0);
        self.gl.set_i(p.u("u_snap"), 1);
        self.gl.set_i(p.u("u_height"), 2);
        self.gl.set_f(p.u("u_size"), &[c.0 as f32, c.1 as f32]);
        self.gl.set_f(p.u("u_k"), &k);
        self.gl.set_f(p.u("u_feather"), &[feather as f32]);
        self.gl.set_f(p.u("u_box"), &[mask.0 as f32, mask.1 as f32, mask.2 as f32, mask.3 as f32]);
        self.rect(x as f32, y as f32, (x + w) as f32, (y + h) as f32);
    }

    /// Step 3: the point lights' shadow masks, four lights a mask, a channel each.
    fn shadows(&mut self, t: &Targets) {
        let c = t.canvas;
        let draws = std::mem::take(&mut self.prep.shadow_draws);
        if !draws.is_empty() {
            let p = &self.progs.span;
            for (mask, slots) in [(t.mask_a, 1..=4u8), (t.mask_b, 5..=8)] {
                self.gl.target(Some(mask.fbo), mask.w, mask.h);
                self.gl.clear([0.0; 4]);
                if !draws.iter().any(|(s, _)| slots.contains(s)) {
                    continue;
                }
                self.gl.use_program(p.p);
                self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
                self.gl.point(self.bufs.span, &sh::SPAN_SIZES, self.prep.span_v.len() / 4);
                self.gl.blend(if self.gl.info.minmax { Blend::Max } else { Blend::Off });
                for (s, r) in draws.iter().filter(|(s, _)| slots.contains(s)) {
                    let ch = usize::from((s - 1) % 4);
                    let mut m = [false; 4];
                    m[ch] = true;
                    self.gl.color_mask(m);
                    self.gl.draw_quads(r.start, r.len());
                    self.calls += 1;
                }
                self.gl.color_mask([true; 4]);
            }
        }
        self.prep.shadow_draws = draws;
        self.gl.blend(Blend::Off);
    }

    /// Step 4: the light target.
    fn light(&mut self, t: &Targets) {
        let c = t.canvas;
        let l = t.light;
        self.gl.target(Some(l.fbo), l.w, l.h);
        let normals = if self.rows.normal_light { 1.0 } else { 0.0 };
        let scale = t.scale as f32;
        let Some(sky) = self.prep.sky else {
            // No light pass (a T0 frame): lit flat, no glint.
            self.gl.clear([0.5, 0.5, 0.5, 0.0]);
            return;
        };
        let p = &self.progs.ambient;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Off);
        self.gl.bind(0, t.nh.tex);
        self.gl.set_i(p.u("u_nh"), 0);
        self.gl.set_f(p.u("u_size"), &[l.w as f32, l.h as f32]);
        self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
        self.gl.set_f(p.u("u_scale"), &[scale]);
        self.gl.set_f(p.u("u_normals"), &[normals]);
        self.gl.set_f(p.u("u_fill"), &sky.fill);
        match sky.sun {
            Some((d, col)) => {
                self.gl.set_f(p.u("u_sun"), &[d[0], d[1], d[2], 1.0]);
                self.gl.set_f(p.u("u_suncol"), &col);
            }
            None => self.gl.set_f(p.u("u_sun"), &[0.0, 0.0, 1.0, 0.0]),
        }
        self.rect(0.0, 0.0, l.w as f32, l.h as f32);
        if self.prep.n_lights > 0 {
            let p = &self.progs.point;
            self.gl.use_program(p.p);
            self.gl.blend(Blend::Add);
            self.gl.bind(0, t.nh.tex);
            self.gl.bind(1, t.mask_a.tex);
            self.gl.bind(2, t.mask_b.tex);
            self.gl.bind(3, t.emi.tex);
            self.gl.set_i(p.u("u_nh"), 0);
            self.gl.set_i(p.u("u_mask_a"), 1);
            self.gl.set_i(p.u("u_mask_b"), 2);
            self.gl.set_i(p.u("u_emi"), 3);
            self.gl.set_f(p.u("u_wet"), &[f32::from(self.prep.atmos.wet) / 255.0]);
            self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
            self.gl.set_f(p.u("u_scale"), &[scale]);
            self.gl.set_f(p.u("u_normals"), &[normals]);
            self.gl.point(self.bufs.light, &sh::LIGHT_SIZES, self.prep.light_v.len() / 14);
            self.gl.draw_quads(0, self.prep.n_lights);
            self.calls += 1;
            self.gl.blend(Blend::Off);
        }
    }

    /// The frame's T1 grade: its tint and lift (none without a `Post` pass).
    fn grade(&self) -> ([f32; 3], [f32; 3]) {
        match self.prep.post {
            Some(post) => (post.tint.map(|v| f32::from(v) / 255.0), post.lift.map(|v| f32::from(v) / 255.0)),
            None => ([1.0; 3], [0.0; 3]),
        }
    }

    /// Sets the sky's uniforms of program `p` (in use) from the frame's backdrop.
    fn sky_uniforms(&self, p: &Prog) {
        let Some(s) = self.prep.backdrop else { return };
        let lin = |c: jane_present::Rgb| c.map(prep::linear);
        self.gl.set_f(p.u("u_zenith"), &lin(s.zenith));
        self.gl.set_f(p.u("u_horizon"), &lin(s.horizon));
        let [r, g, b] = lin(s.glow);
        self.gl.set_f(p.u("u_glow"), &[r, g, b, f32::from(s.glow_amount) / 255.0]);
        self.gl.set_f(p.u("u_glow_x"), &[f32::from(s.glow_x)]);
    }

    /// The sky backdrop into its target (§1.9): the gradient, the stars and the moon over it, then
    /// the far things standing on its horizon.
    fn backdrop(&mut self, t: &Targets) {
        if self.prep.backdrop.is_none() {
            return;
        }
        let s = t.sky;
        self.gl.target(Some(s.fbo), s.w, s.h);
        self.gl.blend(Blend::Off);
        let p = &self.progs.sky;
        self.gl.use_program(p.p);
        self.sky_uniforms(&self.progs.sky);
        self.gl.set_f(self.progs.sky.u("u_size"), &[s.w as f32, s.h as f32]);
        self.rect(0.0, 0.0, s.w as f32, s.h as f32);
        let quads = self.prep.sky_shapes.clone();
        if !quads.is_empty() {
            let p = &self.progs.shape;
            self.gl.use_program(p.p);
            self.gl.set_f(p.u("u_canvas"), &[s.w as f32, s.h as f32]);
            self.gl.set_f(p.u("u_lit"), &[0.0]);
            self.gl.set_f(p.u("u_tint"), &[1.0; 3]);
            self.gl.set_f(p.u("u_lift"), &[0.0; 3]);
            self.gl.point(self.bufs.shape, &sh::SHAPE_SIZES, self.prep.shape_v.len() / 12);
            self.gl.blend(Blend::Over);
            self.gl.draw_quads(quads.start, quads.len());
            self.calls += 1;
        }
        if !self.prep.far_draws.is_empty() {
            let p = &self.progs.far;
            self.gl.use_program(p.p);
            self.sky_uniforms(&self.progs.far);
            let p = &self.progs.far;
            self.gl.set_f(p.u("u_canvas"), &[s.w as f32, s.h as f32]);
            self.gl.set_i(p.u("u_alb"), 0);
            self.gl.set_i(p.u("u_pem"), 2);
            self.gl.set_i(p.u("u_clut"), 3);
            self.gl.set_f(p.u("u_fill"), &self.prep.fill.map(prep::linear));
            self.gl.set_f(p.u("u_egain"), &[EMISSIVE_GAIN]);
            self.gl.point(self.bufs.far, &sh::SPRITE_SIZES, self.prep.far_v.len() / 12);
            self.gl.blend(Blend::Off);
            for k in 0..self.prep.far_draws.len() {
                let (page, r, haze) = self.prep.far_draws[k].clone();
                self.bind_page(page, self.progs.far.u("u_page"));
                self.gl.set_f(self.progs.far.u("u_haze"), &[haze]);
                self.gl.draw_quads(r.start, r.len());
                self.calls += 1;
            }
        }
    }

    /// Step 5: the canvas: the light by the albedo, the emissive, the water, the wet ground, the
    /// sky beyond the zone's top edge, the grade (`COMPOSE_FS`).
    fn compose(&mut self, t: &Targets, frame: &Frame) {
        let c = t.canvas;
        self.gl.target(Some(t.out.fbo), c.0, c.1);
        let p = &self.progs.compose;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Off);
        for (unit, tex) in [t.alb.tex, t.light.tex, t.emi.tex, t.nh.tex, t.sky.tex].into_iter().enumerate() {
            self.gl.bind(unit as u32, tex);
        }
        for (unit, name) in ["u_alb", "u_light", "u_emi", "u_nh", "u_sky"].into_iter().enumerate() {
            self.gl.set_i(p.u(name), unit as i32);
        }
        self.gl.set_f(p.u("u_size"), &[c.0 as f32, c.1 as f32]);
        let (tint, lift) = self.grade();
        self.gl.set_f(p.u("u_tint"), &tint);
        self.gl.set_f(p.u("u_lift"), &lift);
        self.gl.set_f(p.u("u_egain"), &[EMISSIVE_GAIN]);
        let (top, sky) = self.prep.backdrop.map_or((0.0, 0.0), |s| (s.zone.1 as f32, 1.0));
        let reflect = if self.rows.reflect { 1.0 } else { 0.0 };
        self.gl.set_f(p.u("u_info"), &[top, sky, if self.prep.water { 1.0 } else { 0.0 }, reflect]);
        let a = self.prep.atmos;
        // The tick wraps well inside a float's whole numbers; the swell skips once a half hour.
        let tick = (frame.tick % 100_000) as f32;
        self.gl
            .set_f(p.u("u_weather"), &[f32::from(a.rain) / 255.0, f32::from(a.wet) / 255.0, f32::from(a.wind), tick]);
        self.gl.set_f(p.u("u_cam"), &[frame.camera.0 as f32, frame.camera.1 as f32]);
        self.gl.set_f(p.u("u_fill"), &self.prep.fill.map(|v| f32::from(v) / 255.0));
        self.rect(0.0, 0.0, c.0 as f32, c.1 as f32);
    }

    /// What lies over the composed canvas, in the frame's order: the particles (lit by the light
    /// target where they lie under the light) and the fog.
    fn after(&mut self, t: &Targets, frame: &Frame) {
        if self.prep.after.is_empty() {
            return;
        }
        let c = t.canvas;
        self.gl.target(Some(t.out.fbo), c.0, c.1);
        let (tint, lift) = self.grade();
        let steps = std::mem::take(&mut self.prep.after);
        for step in &steps {
            match step {
                After::Parts { quads, lit } => {
                    let p = &self.progs.shape;
                    self.gl.use_program(p.p);
                    self.gl.bind(0, t.light.tex);
                    self.gl.set_i(p.u("u_light"), 0);
                    self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
                    self.gl.set_f(p.u("u_lit"), &[if *lit { 1.0 } else { 0.0 }]);
                    self.gl.set_f(p.u("u_tint"), &tint);
                    self.gl.set_f(p.u("u_lift"), &lift);
                    self.gl.point(self.bufs.shape, &sh::SHAPE_SIZES, self.prep.shape_v.len() / 12);
                    self.gl.blend(Blend::Over);
                    self.gl.draw_quads(quads.start, quads.len());
                    self.calls += 1;
                }
                After::Fog => {
                    let Some(mist) = self.mist else { continue };
                    let p = &self.progs.fog;
                    self.gl.use_program(p.p);
                    for (unit, (name, tex)) in
                        [("u_nh", t.nh.tex), ("u_light", t.light.tex), ("u_mist", mist)].into_iter().enumerate()
                    {
                        self.gl.bind(unit as u32, tex);
                        self.gl.set_i(p.u(name), unit as i32);
                    }
                    self.gl.set_f(p.u("u_size"), &[c.0 as f32, c.1 as f32]);
                    self.gl.set_f4s(p.u("u_vrect[0]"), &self.prep.fog_rect);
                    self.gl.set_f4s(p.u("u_vcol[0]"), &self.prep.fog_col);
                    self.gl.set_f4s(p.u("u_vshape[0]"), &self.prep.fog_shape);
                    self.gl.set_f(p.u("u_n"), &[(self.prep.fog_rect.len() / 4) as f32]);
                    let d = self.prep.drift;
                    let cam = frame.camera;
                    self.gl.set_f(p.u("u_move"), &[cam.0 as f32, cam.1 as f32, f32::from(d.0), f32::from(d.1)]);
                    self.gl.set_f(p.u("u_base"), &self.prep.ambient.map(|v| f32::from(v) / 255.0));
                    self.gl.set_f(p.u("u_tint"), &tint);
                    self.gl.set_f(p.u("u_lift"), &lift);
                    self.gl.blend(Blend::Over);
                    self.rect(0.0, 0.0, c.0 as f32, c.1 as f32);
                }
            }
        }
        self.prep.after = steps;
        self.gl.blend(Blend::Off);
    }

    /// Uploads what changed of the frame's UI images.
    fn ui_images(&mut self, frame: &Frame) {
        if self.images.len() < frame.ui_images.len() {
            self.images.resize(frame.ui_images.len(), None);
        }
        for (i, img) in frame.ui_images.iter().enumerate() {
            if img.w == 0 || img.h == 0 || self.images[i].is_some_and(|h| h.0 == img.generation) {
                continue;
            }
            let tex = match self.images[i] {
                Some((_, tex, size)) if size == (img.w, img.h) => tex,
                other => {
                    if let Some((_, tex, _)) = other {
                        self.gl.delete_texture(tex);
                    }
                    match self.gl.texture(u32::from(img.w), u32::from(img.h), Format::Rgba8, false) {
                        Ok(t) => t,
                        Err(_) => continue,
                    }
                }
            };
            self.scratch.clear();
            for &p in &img.argb {
                let [a, r, g, b] = p.to_be_bytes();
                self.scratch.extend_from_slice(&[r, g, b, a]);
            }
            self.gl.upload(tex, 0, 0, u32::from(img.w), u32::from(img.h), Format::Rgba8, &self.scratch);
            self.images[i] = Some((img.generation, tex, (img.w, img.h)));
        }
    }

    /// Step 6: the `Ui` pass over the canvas.
    fn ui(&mut self, t: &Targets, frame: &Frame) {
        if frame.ui.is_empty() {
            return;
        }
        self.ui_images(frame);
        let images = &self.images;
        ui::build(frame, &mut self.ui_v, &mut self.ui_draws, |s| {
            images.get(usize::from(s)).is_some_and(Option::is_some)
        });
        if self.ui_draws.is_empty() {
            return;
        }
        let c = t.canvas;
        self.gl.target(Some(t.out.fbo), c.0, c.1);
        let p = &self.progs.ui;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Over);
        self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
        self.gl.set_i(p.u("u_alb"), 0);
        self.gl.set_i(p.u("u_clut"), 3);
        self.gl.set_i(p.u("u_img"), 5);
        if let Some(cl) = self.clut {
            self.gl.bind(3, cl);
        }
        self.gl.vertices(self.bufs.ui, &self.ui_v, &sh::UI_SIZES);
        let mut page = None;
        for k in 0..self.ui_draws.len() {
            let d = self.ui_draws[k];
            let (x0, y0) = (i32::from(d.clip.x).clamp(0, c.0 as i32), i32::from(d.clip.y).clamp(0, c.1 as i32));
            let x1 = d.clip.right().clamp(0, c.0 as i32);
            let y1 = d.clip.bottom().clamp(0, c.1 as i32);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            self.gl.scissor(Some((x0, y0, x1 - x0, y1 - y0)));
            if page != Some(d.page) {
                if let Some(pg) = self.pages_gl.get(usize::from(d.page)) {
                    self.gl.bind(0, pg.alb);
                    self.gl.set_f(self.progs.ui.u("u_page"), &pg.shape);
                }
                page = Some(d.page);
            }
            if let Some((_, tex, (w, h))) = d.image.and_then(|s| self.images.get(usize::from(s)).copied().flatten()) {
                self.gl.bind(5, tex);
                self.gl.set_f(self.progs.ui.u("u_img_size"), &[f32::from(w), f32::from(h)]);
            }
            self.gl.draw_quads(d.first, d.count);
            self.calls += 1;
        }
        self.gl.scissor(None);
        self.gl.blend(Blend::Off);
    }
}

/// `0xAARRGGBB` as RGBA bytes.
fn argb_bytes(out: &mut Vec<u8>, px: &[u32]) {
    out.clear();
    out.reserve(px.len() * 4);
    for &p in px {
        let [a, r, g, b] = p.to_be_bytes();
        out.extend_from_slice(&[r, g, b, a]);
    }
}

impl Backend for Gl2 {
    fn caps(&self) -> Caps {
        Caps {
            tier: Tier::T1,
            max_lights: self.rows.max_lights,
            has_readback: true,
            max_texture: self.gl.info.max_texture.max(0) as u32,
            name: "gl2",
        }
    }

    /// Also forgets which chunks the GPU holds: a new presenter (a new game) numbers its chunk
    /// generations from the start again.
    fn upload_atlas(&mut self, pages: &AtlasPages) {
        self.held.fill(None);
        let gl = &self.gl;
        for p in self.pages_gl.drain(..) {
            for t in [p.alb, p.nh, p.em] {
                gl.delete_texture(t);
            }
        }
        if let Some(c) = self.clut.take() {
            gl.delete_texture(c);
        }
        let mut clut = pages.clut.clone();
        clut.resize(CLUT_LEN, 0xff00_0000);
        let mut bytes = Vec::with_capacity(CLUT_LEN * 4);
        argb_bytes(&mut bytes, &clut);
        if let Ok(t) = gl.texture(CLUT_LEN as u32, 1, Format::Rgba8, false) {
            gl.upload(t, 0, 0, CLUT_LEN as u32, 1, Format::Rgba8, &bytes);
            self.clut = Some(t);
        }
        // The mist tile (§1.9): 256 on a side, repeating, its alpha as luminance.
        if let Some(m) = self.mist.take() {
            gl.delete_texture(m);
        }
        // `jane_art::weather::MIST_SIDE`.
        let side = 256u32;
        if pages.mist.len() == (side * side) as usize
            && let Ok(t) = gl.texture(side, side, Format::La8, true)
        {
            bytes.clear();
            for &m in &pages.mist {
                bytes.extend_from_slice(&[m, 255]);
            }
            gl.upload(t, 0, 0, side, side, Format::La8, &bytes);
            gl.repeat(t);
            self.mist = Some(t);
        }
        self.pages.clear();
        let max = gl.info.max_texture.clamp(2048, 8192) as usize;
        for p in &pages.pages {
            self.pages.push(PageCpu::new(p.w, p.h, &p.albedo));
            let (w, h) = (usize::from(p.w).max(1), usize::from(p.h).max(1));
            // A page taller than a texture may be is laid in strips side by side.
            let strips = h.div_ceil(max);
            let sh = if strips > 1 { max } else { h };
            let (tw, th) = (w * strips, sh);
            let lit = p.lit();
            let n = w * h;
            let Ok(alb) = gl.texture(tw as u32, th as u32, Format::La8, false) else { continue };
            let Ok(nh) = gl.texture(tw as u32, th as u32, Format::Rgba8, false) else { continue };
            let Ok(em) = gl.texture(tw as u32, th as u32, Format::La8, false) else { continue };
            for s in 0..strips {
                let (y0, y1) = (s * sh, ((s + 1) * sh).min(h));
                if y0 >= y1 {
                    continue;
                }
                let rows = y0 * w..y1 * w;
                let (sw, shh) = (w as u32, (y1 - y0) as u32);
                bytes.clear();
                for &a in p.albedo.get(rows.clone()).unwrap_or(&[]) {
                    bytes.extend_from_slice(&a.to_le_bytes());
                }
                bytes.resize(sw as usize * shh as usize * 2, 0);
                gl.upload(alb, (s * w) as u32, 0, sw, shh, Format::La8, &bytes);
                bytes.clear();
                for i in rows.clone() {
                    let (nx, ny) = if lit && i < n { (p.normal[i][0], p.normal[i][1]) } else { (128, 128) };
                    let hh = if lit && i < n { p.height[i] } else { u8::from(p.albedo.get(i).is_some_and(|&a| a > 1)) };
                    bytes.extend_from_slice(&[nx, ny, hh, 0]);
                }
                gl.upload(nh, (s * w) as u32, 0, sw, shh, Format::Rgba8, &bytes);
                bytes.clear();
                for i in rows {
                    let e = if lit && i < n { p.emissive[i] } else { 0 };
                    bytes.extend_from_slice(&e.to_le_bytes());
                }
                gl.upload(em, (s * w) as u32, 0, sw, shh, Format::La8, &bytes);
            }
            self.pages_gl.push(PageGl { alb, nh, em, shape: [w as f32, sh as f32, tw as f32, th as f32] });
        }
    }

    fn draw(&mut self, frame: &Frame) {
        let t0 = Instant::now();
        self.calls = 0;
        self.slot = (self.frames as usize) % RING;
        self.collect();
        let canvas = (u32::from(frame.canvas.0).max(1), u32::from(frame.canvas.1).max(1));
        if self.fit(canvas).is_err() {
            return;
        }
        let rows = self.rows;
        self.prep.build(frame, &self.pages, &rows);
        self.upload_chunks(frame);
        let Some(t) = self.targets.take() else { return };
        if !self.prep.chunk_v.is_empty() {
            self.gl.vertices(self.bufs.chunk, &self.prep.chunk_v, &sh::CHUNK_SIZES);
        }
        if !self.prep.sprite_v.is_empty() {
            self.gl.vertices(self.bufs.sprite, &self.prep.sprite_v, &sh::SPRITE_SIZES);
        }
        if !self.prep.span_v.is_empty() {
            self.gl.vertices(self.bufs.span, &self.prep.span_v, &sh::SPAN_SIZES);
        }
        if !self.prep.light_v.is_empty() {
            self.gl.vertices(self.bufs.light, &self.prep.light_v, &sh::LIGHT_SIZES);
        }
        if !self.prep.shape_v.is_empty() {
            self.gl.vertices(self.bufs.shape, &self.prep.shape_v, &sh::SHAPE_SIZES);
        }
        if !self.prep.far_v.is_empty() {
            self.gl.vertices(self.bufs.far, &self.prep.far_v, &sh::SPRITE_SIZES);
        }
        self.albedo(&t, frame.clear);
        let at = Instant::now();
        self.time_begin(2);
        self.shadows(&t);
        self.time_end();
        self.cpu_us[2] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(3);
        self.light(&t);
        self.time_end();
        self.cpu_us[3] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(SEC_SKY);
        self.backdrop(&t);
        self.time_end();
        self.cpu_us[SEC_SKY] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(SEC_GRADE);
        self.compose(&t, frame);
        self.time_end();
        self.cpu_us[SEC_GRADE] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(SEC_AFTER);
        self.after(&t, frame);
        self.time_end();
        self.cpu_us[SEC_AFTER] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(SEC_UI);
        self.ui(&t, frame);
        self.time_end();
        self.cpu_us[SEC_UI] = at.elapsed().as_micros() as u32;
        self.targets = Some(t);
        if self.timer.is_none() {
            let mut pass = [0u32; StatPass::COUNT];
            for (k, us) in self.cpu_us.iter().enumerate() {
                pass[SECTION_PASS[k] as usize] += us;
            }
            self.times.push_passes(t0.elapsed().as_micros() as u32 + self.cpu_us[SEC_UPSCALE], pass);
        }
        self.frames += 1;
        self.times.set_counts(self.calls, self.prep.n_lights as u32, self.prep.casters as u32, 0);
    }

    fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16) {
        let Some(t) = &self.targets else {
            out.clear();
            return (0, 0);
        };
        let o = t.out;
        self.read(o, out)
    }

    fn stats(&self) -> Option<FrameStats> {
        Some(self.times.stats())
    }

    /// The rows T1 draws itself (§1.3): N dot L, the upscale, and the counts and silhouettes its
    /// `Rows` also hold, so a frame from a presenter that kept more is held to them. The
    /// backend's own settings (the light target's size, the albedo mode, the reflection) stay.
    fn set_features(&mut self, f: &Features) {
        self.rows = Rows {
            normal_light: f.normal_light,
            shadows: f.shadows.min(8),
            silhouettes: f.silhouettes,
            sharp: f.sharp,
            max_lights: f.max_lights.min(Rows::T1.max_lights),
            ..self.rows
        };
    }
}

impl Drop for Gl2 {
    fn drop(&mut self) {
        if let Some(t) = self.targets.take() {
            t.free(&self.gl);
        }
        if let Some(t) = self.offscreen_target.take() {
            t.free(&self.gl);
        }
    }
}
