//! The immediate-mode UI (PRESENTATION.md §3): its core and widgets, the screens built on them,
//! and the `Ui` pass they draw into (`cmd`, the contract with every backend).
//!
//! Layers, top one eats input: debug, terminal, title, pause, window, dialogue, HUD and world.

pub mod art;
pub mod cmd;
pub mod console;
pub mod controls;
pub mod core;
pub mod dialogue;
pub mod hud;
pub mod icons;
pub mod lan;
pub mod loading;
pub mod map;
pub mod menus;
pub mod perf;
pub mod style;
pub mod title;
pub mod window;
pub mod world;

pub use art::{Mark, UiArt};
pub use cmd::{Rect, UiCmd, UiImage};
pub use core::{
    AppIntent, ButtonKind, DragPayload, DropTarget, FieldOut, Ink, PanelStyle, SlotOut, SlotView, Ui, UiInput, UiOut,
    WidgetId, wid,
};
