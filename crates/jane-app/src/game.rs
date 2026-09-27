//! The window and the backend the probe picks (PRESENTATION.md §1.3): `wgpu` at T2 where an
//! adapter can draw it, else `soft` at T0. The loop itself is `app.rs`.

use jane_present::Backend;
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
    /// Shows the frame the backend last drew.
    fn show(&mut self, win: (u32, u32)) -> Result<(), String>;
    /// `soft`, or `wgpu, Vulkan, <adapter>`: the title bar.
    fn describe(&self) -> String;
}

/// T0: `soft` into a streaming texture on an SDL renderer, nearest upscale.
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

    fn show(&mut self, win: (u32, u32)) -> Result<(), String> {
        let (px, w, h) = self.soft.pixels();
        self.target.upload(px, w, h)?;
        self.canvas.clear();
        self.target.blit(&mut self.canvas, win)?;
        self.canvas.present();
        Ok(())
    }

    fn describe(&self) -> String {
        "soft".into()
    }
}

/// T2: `wgpu` on the window's own surface, sharp bilinear upscale.
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

    fn show(&mut self, _win: (u32, u32)) -> Result<(), String> {
        self.wgpu.present()
    }

    fn describe(&self) -> String {
        self.wgpu.describe().to_owned()
    }
}

/// The probe (§1.3): `wgpu` at T2 when asked or when `auto` finds an adapter that can draw it,
/// else `soft`. `Err` only when `wgpu` was asked for by name and cannot be had.
fn probe(choice: BackendChoice, window: &sdl2::video::Window) -> Result<Option<Wgpu>, String> {
    if choice == BackendChoice::Soft {
        return Ok(None);
    }
    let target = wgpu::SurfaceTarget::from(SdlWindow::new(window));
    match Wgpu::for_window(target, window.size(), true) {
        Ok(w) => Ok(Some(w)),
        Err(e) if choice == BackendChoice::Wgpu => Err(format!("--backend wgpu: {e}")),
        Err(e) => {
            println!("jane-app: no T2 ({e}); drawing with soft");
            Ok(None)
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
    let k = screen::start_scale(usable, args.scale);
    let window = screen::open(&video, "Jane", k)?;
    // SDL starts with text input on; the console turns it on when it opens.
    let text_in = video.text_input();
    text_in.stop();
    // The UI draws its own pointer: the arrow, the hand, the reticle at the assisted aim.
    sdl.mouse().show_cursor(false);
    let mut pump = sdl.event_pump()?;
    // The command line's backend, else the one the Controls screen chose last time.
    let saved = crate::config::Config::load(&crate::saves::Dirs::find(args.data_dir.as_deref())).backend;
    let choice = match (args.backend, saved.as_deref()) {
        (BackendChoice::Auto, Some("soft")) => BackendChoice::Soft,
        (BackendChoice::Auto, Some("wgpu")) => BackendChoice::Wgpu,
        (b, _) => b,
    };
    match probe(choice, &window)? {
        Some(wgpu) => {
            println!("jane-app: {}", wgpu.describe());
            crate::app::run(args, &mut pump, pads, &text_in, &mut GpuScreen { window, wgpu: Box::new(wgpu) })
        }
        None => {
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
            )
        }
    }
}
