//! The window and the present (PRESENTATION.md, the window; §1.3 T0): a resizable window, a
//! streaming ARGB8888 texture the size of the canvas, nearest upscale, no bars.

use jane_present::{CANVAS_H, CANVAS_W};
use sdl2::VideoSubsystem;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::{Window, WindowContext};

/// Room the window's own title bar and frame take above the canvas, in desktop px.
const DECOR_PX: u32 = 64;

/// The window's starting multiple of 768 x 432: `asked`, else 2x where it fits the desktop's
/// usable area, else the largest multiple that does, never below 1.
pub fn start_scale(usable: Option<(u32, u32)>, asked: Option<u32>) -> u32 {
    if let Some(k) = asked {
        return k.max(1);
    }
    let Some((w, h)) = usable else { return 2 };
    let fits = (w / u32::from(CANVAS_W)).min(h.saturating_sub(DECOR_PX) / u32::from(CANVAS_H));
    fits.clamp(1, 2)
}

/// A window of `k` times the canvas, centred, resizable. `soft` puts an SDL renderer on it
/// ([`canvas`]); `wgpu` makes its surface on it.
pub fn open(video: &VideoSubsystem, title: &str, k: u32) -> Result<Window, String> {
    // T0 upscales nearest (PRESENTATION.md, the window); set before any texture is made.
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
    let mut window = video
        .window(title, u32::from(CANVAS_W) * k, u32::from(CANVAS_H) * k)
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| e.to_string())?;
    window.set_minimum_size(u32::from(CANVAS_W) / 2, u32::from(CANVAS_H) / 2).map_err(|e| e.to_string())?;
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

/// The texture the canvas is uploaded into, remade when the canvas changes size.
pub struct Target<'a> {
    tc: &'a TextureCreator<WindowContext>,
    tex: Option<Texture<'a>>,
    size: (u32, u32),
}

impl std::fmt::Debug for Target<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target").field("size", &self.size).finish_non_exhaustive()
    }
}

impl<'a> Target<'a> {
    pub fn new(tc: &'a TextureCreator<WindowContext>) -> Target<'a> {
        Target { tc, tex: None, size: (0, 0) }
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

    /// Draw the texture over the window at `s = win_h / 432`: the canvas's height fills the
    /// window's and its width covers it (the last column may be cut). No bars.
    pub fn blit(&self, canvas: &mut WindowCanvas, win: (u32, u32)) -> Result<(), String> {
        let Some(tex) = &self.tex else { return Ok(()) };
        let (cw, ch) = self.size;
        let dw = (u64::from(cw) * u64::from(win.1)).div_ceil(u64::from(ch)) as u32;
        canvas.copy(tex, None, Some(Rect::new(0, 0, dw.max(1), win.1.max(1))))
    }
}

#[cfg(test)]
mod tests {
    use super::start_scale;

    #[test]
    fn the_window_starts_at_2x_where_it_fits() {
        assert_eq!(start_scale(Some((2560, 1400)), None), 2);
        assert_eq!(start_scale(Some((3840, 2100)), None), 2);
        // 1366 x 728 usable: 2x does not fit.
        assert_eq!(start_scale(Some((1366, 728)), None), 1);
        assert_eq!(start_scale(Some((700, 400)), None), 1);
        assert_eq!(start_scale(None, None), 2);
        assert_eq!(start_scale(Some((1366, 728)), Some(3)), 3);
        assert_eq!(start_scale(None, Some(0)), 1);
    }
}
