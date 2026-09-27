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
use jane_present::{AO_TINT, AtlasPages, Backend, CLUT_LEN, Caps, Frame, FrameStats, FrameTimes, StatPass, Tier};

use crate::gl::{Blend, Buffer, Fbo, Format, Gl, Program, Query, Texture, Uniform};
use crate::prep::{PageCpu, Prep, SLOT_ROWS, SLOTS_ACROSS, Step};
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
}

impl Progs {
    fn new(gl: &Gl) -> Result<Progs, String> {
        let light = |fs: &str| format!("{}{fs}", sh::LIGHT_COMMON);
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
                &["u_size", "u_mask", "u_snap", "u_k"],
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
                &["u_nh", "u_canvas", "u_scale", "u_normals", "u_mask_a", "u_mask_b"],
            )?,
            compose: Prog::new(
                gl,
                sh::RECT_VS,
                sh::COMPOSE_FS,
                &sh::RECT_ATTRS,
                &["u_size", "u_alb", "u_light", "u_emi", "u_tint", "u_lift", "u_egain"],
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
        })
    }

    fn free(self, gl: &Gl) {
        for t in [self.alb, self.nh, self.emi, self.sil, self.light, self.mask_a, self.mask_b, self.out] {
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
const SECTIONS: usize = 7;
const SECTION_PASS: [StatPass; SECTIONS] = [
    StatPass::Chunks,
    StatPass::List,
    StatPass::Shadows,
    StatPass::Light,
    StatPass::Grade,
    StatPass::Ui,
    StatPass::Upscale,
];
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
        let rows = Rows { exact: !tiled, half_light: tiled, ..Rows::T1 };
        let times = FrameTimes::new(timer.is_some());
        Ok(Gl2 {
            gl,
            ctx,
            rows,
            progs,
            bufs,
            clut: None,
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
        self.time_begin(6);
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
        self.cpu_us[6] = t0.elapsed().as_micros() as u32;
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
                argb_bytes(buf, &l.emissive);
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
                Step::Silhouette { spans, apply, k } => {
                    self.silhouette(t, spans.clone(), *apply, *k);
                    current = None;
                }
            }
        }
        self.prep.steps = steps;
        self.gl.blend(Blend::Off);
        // Steps 2: the normal and height, and the emissive, of the same terrain and sprites.
        for (target, tex, mode, clear) in [
            (t.nh, self.chunk_tex[1], 4.0, [128.0 / 255.0, 128.0 / 255.0, 0.0, 0.0]),
            (t.emi, self.chunk_tex[2], 5.0, [0.0, 0.0, 0.0, 1.0]),
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
    /// albedo from a snapshot of it.
    fn silhouette(
        &mut self,
        t: &Targets,
        spans: std::ops::Range<usize>,
        (x, y, w, h): (i32, i32, i32, i32),
        k: [f32; 3],
    ) {
        let c = t.canvas;
        self.gl.target(Some(t.sil.fbo), c.0, c.1);
        self.gl.scissor(Some((x, y, w, h)));
        self.gl.clear([0.0; 4]);
        self.gl.scissor(None);
        let p = &self.progs.span;
        self.gl.use_program(p.p);
        self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
        self.gl.point(self.bufs.span, &sh::SPAN_SIZES, self.prep.span_v.len() / 3);
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
        self.gl.set_i(p.u("u_mask"), 0);
        self.gl.set_i(p.u("u_snap"), 1);
        self.gl.set_f(p.u("u_size"), &[c.0 as f32, c.1 as f32]);
        self.gl.set_f(p.u("u_k"), &k);
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
                self.gl.point(self.bufs.span, &sh::SPAN_SIZES, self.prep.span_v.len() / 3);
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
            // No light pass (a T0 frame): lit flat.
            self.gl.clear([0.5, 0.5, 0.5, 1.0]);
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
            self.gl.set_i(p.u("u_nh"), 0);
            self.gl.set_i(p.u("u_mask_a"), 1);
            self.gl.set_i(p.u("u_mask_b"), 2);
            self.gl.set_f(p.u("u_canvas"), &[c.0 as f32, c.1 as f32]);
            self.gl.set_f(p.u("u_scale"), &[scale]);
            self.gl.set_f(p.u("u_normals"), &[normals]);
            self.gl.point(self.bufs.light, &sh::LIGHT_SIZES, self.prep.light_v.len() / 14);
            self.gl.draw_quads(0, self.prep.n_lights);
            self.calls += 1;
            self.gl.blend(Blend::Off);
        }
    }

    /// Step 5: the canvas.
    fn compose(&mut self, t: &Targets) {
        let c = t.canvas;
        self.gl.target(Some(t.out.fbo), c.0, c.1);
        let p = &self.progs.compose;
        self.gl.use_program(p.p);
        self.gl.blend(Blend::Off);
        self.gl.bind(0, t.alb.tex);
        self.gl.bind(1, t.light.tex);
        self.gl.bind(2, t.emi.tex);
        self.gl.set_i(p.u("u_alb"), 0);
        self.gl.set_i(p.u("u_light"), 1);
        self.gl.set_i(p.u("u_emi"), 2);
        self.gl.set_f(p.u("u_size"), &[c.0 as f32, c.1 as f32]);
        let (tint, lift) = match self.prep.post {
            Some(post) => (post.tint.map(|v| f32::from(v) / 255.0), post.lift.map(|v| f32::from(v) / 255.0)),
            None => ([1.0; 3], [0.0; 3]),
        };
        self.gl.set_f(p.u("u_tint"), &tint);
        self.gl.set_f(p.u("u_lift"), &lift);
        self.gl.set_f(p.u("u_egain"), &[EMISSIVE_GAIN]);
        self.rect(0.0, 0.0, c.0 as f32, c.1 as f32);
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
        Caps { tier: Tier::T1, max_lights: self.rows.max_lights, has_readback: true, name: "gl2" }
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
        self.time_begin(4);
        self.compose(&t);
        self.time_end();
        self.cpu_us[4] = at.elapsed().as_micros() as u32;
        let at = Instant::now();
        self.time_begin(5);
        self.ui(&t, frame);
        self.time_end();
        self.cpu_us[5] = at.elapsed().as_micros() as u32;
        self.targets = Some(t);
        if self.timer.is_none() {
            let mut pass = [0u32; StatPass::COUNT];
            for (k, us) in self.cpu_us.iter().enumerate() {
                pass[SECTION_PASS[k] as usize] += us;
            }
            self.times.push_passes(t0.elapsed().as_micros() as u32 + self.cpu_us[6], pass);
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
