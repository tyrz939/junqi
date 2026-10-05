//! The window and the present (PRESENTATION.md, the window; §1.3 T0): a resizable window, a
//! streaming ARGB8888 texture the size of the canvas, shown at the largest whole multiple that
//! fits with the theme's dark bars round it, or (the `fill` row) filling the window's height by
//! sharp bilinear: nearest to the next whole multiple up, then a smooth fit down.

use jane_present::input::Fit;
use jane_present::{CANVAS_H, CANVAS_W};
use sdl2::VideoSubsystem;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::{Window, WindowContext};

/// Room the window's own title bar and frame take above the canvas, in desktop px.
const DECOR_PX: u32 = 64;

/// The window's starting multiple of 640 x 360: `asked`, else the largest whole multiple that
/// fits the desktop's usable area (3x at 1080p, 4x at 1440p, 6x at 4K, the title bar allowed
/// for), never below 1.
pub fn start_scale(usable: Option<(u32, u32)>, asked: Option<u32>) -> u32 {
    if let Some(k) = asked {
        return k.max(1);
    }
    let Some((w, h)) = usable else { return 2 };
    let fits = (w / u32::from(CANVAS_W)).min(h.saturating_sub(DECOR_PX) / u32::from(CANVAS_H));
    fits.max(1)
}

/// How the game starts on a desktop of `desktop` px whose usable area (less the task bar) is
/// `usable`: `asked` times the canvas in a window; else, where the desktop is a larger whole
/// multiple than a window can be (1080p is 3x, 1440p 4x, 4K 6x, and a window with its title bar
/// is one less), filling the desktop (`true`: borderless fullscreen, F11 to leave); else a window
/// at the largest multiple that fits.
pub fn start(desktop: Option<(u32, u32)>, usable: Option<(u32, u32)>, asked: Option<u32>) -> (u32, bool) {
    let windowed = start_scale(usable, asked);
    if asked.is_some() {
        return (windowed, false);
    }
    let full = desktop.map_or(0, |(w, h)| (w / u32::from(CANVAS_W)).min(h / u32::from(CANVAS_H)));
    if full > windowed { (full, true) } else { (windowed, false) }
}

/// A window of `k` times the canvas, centred, resizable. `soft` puts an SDL renderer on it
/// ([`canvas`]); `wgpu` makes its surface on it; `gl2` its context, on a window made `opengl`
/// (with the context's attributes set before, as `jane_render_gl2::attributes` does).
pub fn open(video: &VideoSubsystem, title: &str, k: u32, opengl: bool) -> Result<Window, String> {
    // T0 upscales nearest (PRESENTATION.md, the window); set before any texture is made.
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
    let mut b = video.window(title, u32::from(CANVAS_W) * k, u32::from(CANVAS_H) * k);
    b.position_centered().resizable();
    if opengl {
        b.opengl();
    }
    let mut window = b.build().map_err(|e| e.to_string())?;
    window.set_minimum_size(u32::from(CANVAS_W), u32::from(CANVAS_H)).map_err(|e| e.to_string())?;
    Ok(window)
}

/// The window's SDL renderer, waiting for vsync: what `soft` presents through.
pub fn canvas(window: Window) -> Result<WindowCanvas, String> {
    window.into_canvas().present_vsync().build().map_err(|e| e.to_string())
}

/// Fill the whole window with one colour and show it (the wait while the county builds).
pub fn clear(canvas: &mut WindowCanvas, argb: u32) {
    let [_, r, g, b] = argb.to_be_bytes();
    canvas.set_draw_color(sdl2::pixels::Color::RGB(r, g, b));
    canvas.clear();
    canvas.present();
}

/// The texture the canvas is uploaded into, remade when the canvas changes size; and, filling
/// the window, the whole multiple it is drawn up to before the last smooth fit.
pub struct Target<'a> {
    tc: &'a TextureCreator<WindowContext>,
    tex: Option<Texture<'a>>,
    size: (u32, u32),
    up: Option<Texture<'a>>,
    up_size: (u32, u32),
}

impl std::fmt::Debug for Target<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target").field("size", &self.size).finish_non_exhaustive()
    }
}

impl<'a> Target<'a> {
    pub fn new(tc: &'a TextureCreator<WindowContext>) -> Target<'a> {
        Target { tc, tex: None, size: (0, 0), up: None, up_size: (0, 0) }
    }

    /// Copy `px` (`0xAARRGGBB` rows, `w` x `h`) into the texture.
    pub fn upload(&mut self, px: &[u32], w: u16, h: u16) -> Result<(), String> {
        let (w, h) = (u32::from(w), u32::from(h));
        if w == 0 || h == 0 || px.len() < (w * h) as usize {
            return Ok(());
        }
        if self.tex.is_none() || self.size != (w, h) {
            // The old texture goes before the new one is made.
            self.tex = None;
            let t = self.tc.create_texture_streaming(PixelFormatEnum::ARGB8888, w, h).map_err(|e| e.to_string())?;
            self.tex = Some(t);
            self.size = (w, h);
        }
        let tex = self.tex.as_mut().expect("made above");
        tex.with_lock(None, |buf, pitch| {
            for (y, row) in px.chunks_exact(w as usize).take(h as usize).enumerate() {
                let out = &mut buf[y * pitch..y * pitch + w as usize * 4];
                for (o, p) in out.chunks_exact_mut(4).zip(row) {
                    // ARGB8888 is a packed format: one native-endian u32 a pixel.
                    o.copy_from_slice(&p.to_ne_bytes());
                }
            }
        })
    }

    /// Draw the texture on the window as `fit` lays it: at a whole scale, nearest into its rect
    /// (the bars are the clear round it); else by sharp bilinear, nearest up to the next whole
    /// multiple in a texture of its own, then linear down to the window. A renderer that cannot
    /// draw into a texture fills by nearest.
    pub fn blit(&mut self, canvas: &mut WindowCanvas, fit: &Fit) -> Result<(), String> {
        let Some(tex) = &self.tex else { return Ok(()) };
        let dst = Rect::new(fit.origin.0, fit.origin.1, fit.size.0.max(1), fit.size.1.max(1));
        if fit.whole() {
            return canvas.copy(tex, None, Some(dst));
        }
        let k = (fit.scale.ceil() as u32).max(1);
        let want = (self.size.0 * k, self.size.1 * k);
        if self.up.is_none() || self.up_size != want {
            self.up = None;
            // Read when a texture is made: the multiple is sampled smoothly, the canvas never.
            sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
            let up = self.tc.create_texture_target(PixelFormatEnum::ARGB8888, want.0, want.1);
            sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
            self.up = up.ok();
            self.up_size = want;
        }
        let Some(up) = self.up.as_mut() else { return canvas.copy(tex, None, Some(dst)) };
        let drawn = canvas.with_texture_canvas(up, |c| {
            let _ = c.copy(tex, None, None);
        });
        if drawn.is_err() {
            return canvas.copy(tex, None, Some(dst));
        }
        canvas.copy(up, None, Some(dst))
    }
}

#[cfg(test)]
mod tests {
    use super::start_scale;

    #[test]
    fn the_window_starts_at_the_largest_whole_multiple_that_fits() {
        // Fullscreen-sized desktops: 3x at 1080p, 4x at 1440p, 6x at 4K (with room for a title bar
        // they are one less in a window, as a usable area gives them).
        assert_eq!(start_scale(Some((1920, 1080 + 64)), None), 3);
        assert_eq!(start_scale(Some((2560, 1440 + 64)), None), 4);
        assert_eq!(start_scale(Some((3840, 2160 + 64)), None), 6);
        assert_eq!(start_scale(Some((2560, 1400)), None), 3);
        assert_eq!(start_scale(Some((1920, 1040)), None), 2);
        // 1366 x 728 usable: 1x.
        assert_eq!(start_scale(Some((1366, 728)), None), 1);
        assert_eq!(start_scale(Some((700, 400)), None), 1);
        assert_eq!(start_scale(None, None), 2);
        assert_eq!(start_scale(Some((1366, 728)), Some(3)), 3);
        assert_eq!(start_scale(None, Some(0)), 1);
    }

    #[test]
    fn a_desktop_a_whole_multiple_bigger_than_a_window_starts_filled() {
        use super::start;
        let task_bar = |w: u32, h: u32| Some((w, h - 40));
        assert_eq!(start(Some((1920, 1080)), task_bar(1920, 1080), None), (3, true));
        assert_eq!(start(Some((2560, 1440)), task_bar(2560, 1440), None), (4, true));
        assert_eq!(start(Some((3840, 2160)), task_bar(3840, 2160), None), (6, true));
        // 1366 x 768 is 2x filled; 1280 x 1024 has room for a 2x window.
        assert_eq!(start(Some((1366, 768)), task_bar(1366, 768), None), (2, true));
        assert_eq!(start(Some((1280, 1024)), task_bar(1280, 1024), None), (2, false));
        // Asked for, it is a window.
        assert_eq!(start(Some((1920, 1080)), task_bar(1920, 1080), Some(2)), (2, false));
        assert_eq!(start(None, None, None), (2, false));
    }
}
