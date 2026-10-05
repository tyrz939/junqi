//! The window and the backend the probe picks (PRESENTATION.md §1.3): `wgpu` at T2 where an
//! adapter can draw it, else `gl2` at T1 where an OpenGL 2.1 or GLES 2 context can, else `soft`
//! at T0. The loop itself is `app.rs`.

use jane_present::Backend;
use jane_render_gl2::Gl2;
use jane_render_soft::Soft;
use jane_render_wgpu::Wgpu;

use crate::handle::SdlWindow;
use crate::screen::{self, Target};
use crate::{Args, BackendChoice};

/// The window while the first frame is made.
const LOADING: u32 = 0xff10_1014;

/// Where a frame goes: a backend and the window it shows in.
pub trait Screen {
    fn backend(&mut self) -> &mut dyn Backend;
    fn window_mut(&mut self) -> &mut sdl2::video::Window;
    /// The window's size in px.
    fn size(&self) -> (u32, u32);
    /// The window was resized.
    fn resized(&mut self, _win: (u32, u32)) {}
    /// Shows the frame the backend last drew, laid on the window as `fit` says.
    fn show(&mut self, fit: &jane_present::input::Fit) -> Result<(), String>;
    /// `soft`, or `wgpu, Vulkan, <adapter>`: the title bar.
    fn describe(&self) -> String;
    /// Whether anyone can see the window: not minimised, not hidden.
    fn visible(&mut self) -> bool {
        use sdl2::sys::SDL_WindowFlags::{SDL_WINDOW_HIDDEN, SDL_WINDOW_MINIMIZED};
        let flags = self.window_mut().window_flags();
        flags & (SDL_WINDOW_HIDDEN as u32 | SDL_WINDOW_MINIMIZED as u32) == 0
    }
    /// How often a frame is shown, when the present does not wait for the display itself: the
    /// loop paces its frames to it.
    fn frame_interval(&mut self) -> Option<std::time::Duration> {
        None
    }
}

/// One refresh of the display the window is on (60 Hz if SDL cannot say).
fn refresh(window: &sdl2::video::Window) -> std::time::Duration {
    let hz = window.display_mode().map_or(60, |m| m.refresh_rate).clamp(24, 480);
    std::time::Duration::from_micros(1_000_000 / hz as u64)
}

/// T0: `soft` into a streaming texture on an SDL renderer: a whole multiple, nearest, or filling
/// the window by sharp bilinear (the `fill` row).
struct SoftScreen<'a> {
    canvas: sdl2::render::WindowCanvas,
    target: Target<'a>,
    soft: Soft,
}

impl Screen for SoftScreen<'_> {
    fn backend(&mut self) -> &mut dyn Backend {
        &mut self.soft
    }

    fn window_mut(&mut self) -> &mut sdl2::video::Window {
        self.canvas.window_mut()
    }

    fn size(&self) -> (u32, u32) {
        self.canvas.window().size()
    }

    fn show(&mut self, fit: &jane_present::input::Fit) -> Result<(), String> {
        let (px, w, h) = self.soft.pixels();
        self.target.upload(px, w, h)?;
        let [_, r, g, b] = jane_present::input::BARS.to_be_bytes();
        self.canvas.set_draw_color(sdl2::pixels::Color::RGB(r, g, b));
        self.canvas.clear();
        self.target.blit(&mut self.canvas, fit)?;
        self.canvas.present();
        Ok(())
    }

    fn describe(&self) -> String {
        "soft".into()
    }
}

/// T2: `wgpu` on the window's own surface: a whole multiple, or filling by sharp bilinear.
struct GpuScreen {
    window: sdl2::video::Window,
    wgpu: Box<Wgpu>,
}

impl Screen for GpuScreen {
    fn backend(&mut self) -> &mut dyn Backend {
        self.wgpu.as_mut()
    }

    fn window_mut(&mut self) -> &mut sdl2::video::Window {
        &mut self.window
    }

    fn size(&self) -> (u32, u32) {
        self.window.size()
    }

    fn resized(&mut self, win: (u32, u32)) {
        self.wgpu.resize(win);
    }

    fn show(&mut self, fit: &jane_present::input::Fit) -> Result<(), String> {
        self.wgpu.set_fit(Some(*fit));
        self.wgpu.present()
    }

    fn describe(&self) -> String {
        self.wgpu.describe().to_owned()
    }

    fn frame_interval(&mut self) -> Option<std::time::Duration> {
        self.wgpu.paced_by_caller().then(|| refresh(&self.window))
    }
}

/// T1: `gl2` through an OpenGL 2.1 or GLES 2 context on the window: a whole multiple, or
/// filling by sharp bilinear.
struct GlScreen {
    window: sdl2::video::Window,
    gl2: Box<Gl2>,
}

impl Screen for GlScreen {
    fn backend(&mut self) -> &mut dyn Backend {
        self.gl2.as_mut()
    }

    fn window_mut(&mut self) -> &mut sdl2::video::Window {
        &mut self.window
    }

    fn size(&self) -> (u32, u32) {
        self.window.size()
    }

    fn show(&mut self, fit: &jane_present::input::Fit) -> Result<(), String> {
        self.gl2.set_fit(Some(*fit));
        self.gl2.present()
    }

    fn describe(&self) -> String {
        self.gl2.describe().to_owned()
    }
}

/// What the probe picked, and the window it draws into.
enum Picked {
    Wgpu(sdl2::video::Window, Box<Wgpu>),
    Gl2(sdl2::video::Window, Box<Gl2>),
    Soft(sdl2::video::Window),
}

/// The probe (§1.3): `wgpu` at T2 when asked, or when `auto` finds an adapter that can draw it;
/// else `gl2` at T1 when asked, or when `auto` can make an OpenGL 2.1 or GLES 2 context with
/// framebuffer objects, eight texture units and the T1 shaders; else `soft`. `Err` only when a
/// GPU backend was asked for by name and cannot be had.
///
/// A GL context wants a window made for OpenGL and wgpu's surface a plain one, so `auto` asks
/// wgpu for an adapter first with no window at all, then opens the window the winner needs.
fn probe(choice: BackendChoice, video: &sdl2::VideoSubsystem, k: u32) -> Result<Picked, String> {
    if choice == BackendChoice::Soft {
        return Ok(Picked::Soft(screen::open(video, "Jane", k, false)?));
    }
    let wgpu_first = match choice {
        BackendChoice::Wgpu => true,
        BackendChoice::Auto => match jane_render_wgpu::probe() {
            Ok(_) => true,
            Err(e) => {
                println!("jane-app: no T2 ({e})");
                false
            }
        },
        BackendChoice::Soft | BackendChoice::Gl2 => false,
    };
    if wgpu_first {
        let mut window = screen::open(video, "Jane", k, false)?;
        let target = wgpu::SurfaceTarget::from(SdlWindow::new(&window));
        match Wgpu::for_window(target, window.size(), true) {
            Ok(w) => return Ok(Picked::Wgpu(window, Box::new(w))),
            Err(e) if choice == BackendChoice::Wgpu => return Err(format!("--backend wgpu: {e}")),
            Err(e) => {
                // The window stays lent to wgpu's handle token; it goes out of sight for the GL one.
                println!("jane-app: no T2 ({e})");
                window.hide();
            }
        }
    }
    jane_render_gl2::attributes(video, false);
    let window = screen::open(video, "Jane", k, true)?;
    match Gl2::for_window(video, &window, jane_render_gl2::Api::Auto, true) {
        Ok(g) => Ok(Picked::Gl2(window, Box::new(g))),
        Err(e) if choice == BackendChoice::Gl2 => Err(format!("--backend gl2: {e}")),
        Err(e) => {
            println!("jane-app: no T1 ({e}); drawing with soft");
            Ok(Picked::Soft(window))
        }
    }
}

pub fn run(args: &Args) -> Result<(), String> {
    // Real pixels on a scaled desktop, so 2x is 2x and the nearest upscale stays square.
    sdl2::hint::set("SDL_WINDOWS_DPI_AWARENESS", "permonitorv2");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    // No pad subsystem is no pad, never no game.
    let pads = sdl.game_controller().ok();
    let usable = video.display_usable_bounds(0).ok().map(|r| (r.width(), r.height()));
    let desktop = video.display_bounds(0).ok().map(|r| (r.width(), r.height()));
    // A run the tests and scripts drive stays a window.
    let asked = args.scale.or(args.ticks.map(|_| 2));
    let (k, full) = screen::start(desktop, usable, asked);
    // The command line's backend, else the one the Controls screen chose last time.
    let saved = crate::config::Config::load(&crate::saves::Dirs::find(args.data_dir.as_deref())).backend;
    let choice = match (args.backend, saved.as_deref()) {
        (BackendChoice::Auto, Some("soft")) => BackendChoice::Soft,
        (BackendChoice::Auto, Some("gl2")) => BackendChoice::Gl2,
        (BackendChoice::Auto, Some("wgpu")) => BackendChoice::Wgpu,
        (b, _) => b,
    };
    let mut picked = probe(choice, &video, k)?;
    if full {
        let w = match &mut picked {
            Picked::Wgpu(w, _) | Picked::Gl2(w, _) | Picked::Soft(w) => w,
        };
        if let Err(e) = w.set_fullscreen(sdl2::video::FullscreenType::Desktop) {
            println!("jane-app: no fullscreen ({e}); a window");
        }
    }
    // SDL starts with text input on; the console turns it on when it opens.
    let text_in = video.text_input();
    text_in.stop();
    // The UI draws its own pointer: the arrow, the hand, the reticle at the assisted aim.
    sdl.mouse().show_cursor(false);
    let mut pump = sdl.event_pump()?;
    // The sound device, or silence without one (PRESENTATION.md §5).
    let volumes = crate::config::Config::load(&crate::saves::Dirs::find(args.data_dir.as_deref())).volumes();
    let sound = crate::audio::Sound::open(&sdl, volumes, args.seed);
    match picked {
        Picked::Wgpu(window, wgpu) => {
            println!("jane-app: {}", wgpu.describe());
            crate::app::run(args, &mut pump, pads, &text_in, &mut GpuScreen { window, wgpu }, sound)
        }
        Picked::Gl2(window, gl2) => {
            println!("jane-app: {}", gl2.describe());
            crate::app::run(args, &mut pump, pads, &text_in, &mut GlScreen { window, gl2 }, sound)
        }
        Picked::Soft(window) => {
            let mut canvas = screen::canvas(window)?;
            screen::clear(&mut canvas, LOADING);
            let tc = canvas.texture_creator();
            println!("jane-app: soft");
            crate::app::run(
                args,
                &mut pump,
                pads,
                &text_in,
                &mut SoftScreen { canvas, target: Target::new(&tc), soft: Soft::new() },
                sound,
            )
        }
    }
}
