//! The GL context, made through SDL2 (PRESENTATION.md §1.3 T1): OpenGL 2.1 (compatibility, so a
//! 2006 driver and a modern one both give it) or OpenGL ES 2.0 (the Pi, and anything whose
//! desktop GL is missing). The game window's, or a hidden window's for sheets, tests and the
//! bench.

use sdl2::video::{GLContext, GLProfile, SwapInterval, Window};
use sdl2::{Sdl, VideoSubsystem};

use crate::gl::Gl;

/// Which GL to ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Api {
    /// OpenGL 2.1, else OpenGL ES 2.0.
    Auto,
    /// OpenGL 2.1 only.
    Desktop,
    /// OpenGL ES 2.0 only (GLSL ES 1.00): what the Pi runs, testable on a desktop whose driver
    /// makes ES contexts.
    Es,
}

/// Sets the attributes a T1 context is made with. Call before the window is made (on X11 the
/// window's visual comes from them) and again before each context is asked for.
pub fn attributes(video: &VideoSubsystem, es: bool) {
    let a = video.gl_attr();
    if es {
        a.set_context_profile(GLProfile::GLES);
        a.set_context_version(2, 0);
    } else {
        a.set_context_profile(GLProfile::Compatibility);
        a.set_context_version(2, 1);
    }
    a.set_red_size(8);
    a.set_green_size(8);
    a.set_blue_size(8);
    a.set_alpha_size(0);
    a.set_depth_size(0);
    a.set_stencil_size(0);
    a.set_double_buffer(true);
}

/// A context and what keeps it alive: the window it draws into, and SDL itself when this crate
/// started it.
pub struct Context {
    pub window: Window,
    _video: VideoSubsystem,
    _gl: GLContext,
    _sdl: Option<Sdl>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").field("window", &self.window.size()).finish_non_exhaustive()
    }
}

/// Makes a context on `window` (made with `.opengl()`), current on this thread.
pub fn context(video: &VideoSubsystem, window: &Window, api: Api) -> Result<(GLContext, Gl), String> {
    let tries: &[bool] = match api {
        Api::Auto => &[false, true],
        Api::Desktop => &[false],
        Api::Es => &[true],
    };
    let mut why = Vec::new();
    for &es in tries {
        attributes(video, es);
        match window.gl_create_context() {
            Ok(c) => {
                window.gl_make_current(&c)?;
                let gl = Gl::load(video);
                if gl.info.es != es && api != Api::Auto {
                    why.push(format!(
                        "asked for {} and got {}",
                        if es { "GLES" } else { "GL" },
                        gl.info.version_string
                    ));
                    continue;
                }
                return Ok((c, gl));
            }
            Err(e) => why.push(format!("{}: {e}", if es { "OpenGL ES 2.0" } else { "OpenGL 2.1" })),
        }
    }
    Err(why.join("; "))
}

impl Context {
    /// The game window's context (the window made with `.opengl()`), waiting for vsync or not.
    pub fn for_window(video: &VideoSubsystem, window: &Window, api: Api, vsync: bool) -> Result<(Context, Gl), String> {
        let (c, gl) = context(video, window, api)?;
        let interval = if vsync { SwapInterval::VSync } else { SwapInterval::Immediate };
        // Not every driver lets the interval be set; the loop's own pacing covers it.
        let _ = video.gl_set_swap_interval(interval);
        Ok((Context { window: window.clone(), _video: video.clone(), _gl: c, _sdl: None }, gl))
    }

    /// A hidden window's context: no window shows, frames are drawn into textures and read back.
    pub fn headless(api: Api) -> Result<(Context, Gl), String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        attributes(&video, api == Api::Es);
        let window = video.window("jane gl2", 64, 64).opengl().hidden().build().map_err(|e| e.to_string())?;
        let (c, gl) = context(&video, &window, api)?;
        Ok((Context { window, _video: video, _gl: c, _sdl: Some(sdl) }, gl))
    }

    /// The window's size in px.
    pub fn size(&self) -> (u32, u32) {
        self.window.drawable_size()
    }

    pub fn swap(&self) {
        self.window.gl_swap_window();
    }
}
