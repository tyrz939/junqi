//! The SDL2 window as a wgpu surface target, with no unsafe code (PRESENTATION.md §1.3).
//!
//! wgpu asks for a window handle source that is `Send + Sync`; SDL's `Window` is neither (it
//! holds an `Rc`), and a `WindowHandle` built from raw parts needs `unsafe`. So the source here
//! is a zero-sized token, `Send + Sync` by having nothing in it, that lends the handle of a
//! window kept on this thread: a clone of the game window, leaked once so it lives as long as
//! the process (the one window the game has), and reached through a thread-local. Asked from
//! another thread it has no window and says so; wgpu asks on this one, when the surface is made.

use std::cell::Cell;

use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use sdl2::video::Window;

thread_local! {
    static WINDOW: Cell<Option<&'static Window>> = const { Cell::new(None) };
}

/// The game window's handles, for `wgpu::Instance::create_surface`.
#[derive(Clone, Copy, Debug)]
pub struct SdlWindow;

impl SdlWindow {
    /// Lends `window`'s handles from now on, on this thread.
    pub fn new(window: &Window) -> SdlWindow {
        let kept: &'static Window = Box::leak(Box::new(window.clone()));
        WINDOW.with(|w| w.set(Some(kept)));
        SdlWindow
    }

    fn window() -> Result<&'static Window, HandleError> {
        WINDOW.with(Cell::get).ok_or(HandleError::Unavailable)
    }
}

impl HasWindowHandle for SdlWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        SdlWindow::window()?.window_handle()
    }
}

impl HasDisplayHandle for SdlWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        SdlWindow::window()?.display_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_token_can_cross_to_wgpu() {
        send_sync::<SdlWindow>();
        // With no window lent on this thread, there is no handle, and no panic.
        assert!(SdlWindow.window_handle().is_err());
    }
}
