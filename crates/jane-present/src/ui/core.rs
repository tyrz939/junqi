//! The immediate-mode core (PRESENTATION.md §3.1): each frame the screens call widgets in draw
//! order; the widgets read this frame's input, push [`UiCmd`]s and report what happened; `finish`
//! hands the list to the frame. Nothing here calls into the sim: what leaves is a [`UiOut`].
//!
//! Input is eaten by the top layer only: a screen sets [`Ui::interactive`] before it draws, so
//! the HUD under an open window draws but does not answer. The pointer over any panel drawn this
//! frame is the UI's, not the world's ([`Ui::wants_pointer`]).

use jane_art::font::Face;
use jane_art::palette::Ix;
use jane_core::{ItemId, SpellId};
use jane_sim::input::Command;

use crate::frame::{Frame, Src};
use crate::input::{KeySet, UiAction, sc};
use crate::ui::art::{Mark, UiArt};
use crate::ui::cmd::{Rect, UiCmd, UiImage};
use crate::ui::style::{self, argb, fade};

/// A widget's identity: stable across frames for the same widget.
pub type WidgetId = u32;

/// A widget id from a name and an index (FNV-1a).
pub const fn wid(name: &str, index: u32) -> WidgetId {
    let b = name.as_bytes();
    let mut h: u32 = 0x811c_9dc5;
    let mut i = 0;
    while i < b.len() {
        h ^= b[i] as u32;
        h = h.wrapping_mul(0x0100_0193);
        i += 1;
    }
    let mut k = 0;
    while k < 4 {
        h ^= (index >> (8 * k)) & 0xff;
        h = h.wrapping_mul(0x0100_0193);
        k += 1;
    }
    h
}

/// What the UI hears this frame, device-free. The app fills it from `DeviceState` and the
/// `Edge`s; a test fills it by hand.
#[derive(Clone, Debug, Default)]
pub struct UiInput {
    /// The pointer in canvas px; `None` outside the window or while a pad drives.
    pub pointer: Option<(i32, i32)>,
    /// The primary button: held now, went down, came up (since the last frame).
    pub held: bool,
    pub pressed: bool,
    pub released: bool,
    /// The secondary button went down.
    pub right_pressed: bool,
    /// The middle button went down.
    pub middle_pressed: bool,
    /// Pad buttons and triggers that went down (press-to-rebind reads them).
    pub pad_pressed: PadPress,
    /// Wheel notches, up positive.
    pub wheel: i32,
    /// Navigation presses, in order (a pad or the keyboard in a screen).
    pub actions: Vec<UiAction>,
    /// Text typed this frame (a text field reads it).
    pub typed: String,
    /// Keys that went down this frame (a text field's editing keys, press-to-rebind).
    pub keys: KeySet,
    /// The last device used was a pad: pad glyphs in the hints.
    pub pad: bool,
}

impl UiInput {
    pub fn has(&self, a: UiAction) -> bool {
        self.actions.contains(&a)
    }
}

/// Pad inputs that went down this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadPress {
    /// By [`crate::input::pad`] bit.
    pub buttons: u32,
    pub lt: bool,
    pub rt: bool,
}

/// How a run of text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ink {
    pub face: Face,
    pub ix: Ix,
    pub bold: bool,
    /// A second pass in `k` down and right.
    pub shadow: bool,
    /// A four-way `k` rim.
    pub outline: bool,
    pub alpha: u8,
}

impl Ink {
    pub const fn new(face: Face, ix: Ix) -> Ink {
        Ink { face, ix, bold: false, shadow: false, outline: false, alpha: 255 }
    }
    pub const fn fine(ix: Ix) -> Ink {
        Ink::new(Face::Fine, ix)
    }
    pub const fn small(ix: Ix) -> Ink {
        Ink::new(Face::Small, ix)
    }
    pub const fn head(ix: Ix) -> Ink {
        Ink::new(Face::Head, ix)
    }
    pub const fn title(ix: Ix) -> Ink {
        Ink::new(Face::Title, ix)
    }
    pub const fn shadow(self) -> Ink {
        Ink { shadow: true, ..self }
    }
    pub const fn outline(self) -> Ink {
        Ink { outline: true, ..self }
    }
    pub const fn bold(self) -> Ink {
        Ink { bold: true, ..self }
    }
    pub const fn alpha(self, a: u8) -> Ink {
        Ink { alpha: a, ..self }
    }
    pub const fn ix(self, ix: Ix) -> Ink {
        Ink { ix, ..self }
    }
}

/// The advance of `face` in px.
pub const fn advance(face: Face) -> i32 {
    face.cell().0
}

/// The line height of `face` in px.
pub const fn line_h(face: Face) -> i32 {
    face.cell().1
}

/// The width of `s` in `face`, px.
pub fn text_w(face: Face, s: &str) -> i32 {
    s.chars().count() as i32 * advance(face)
}

/// `s` broken greedily on spaces into lines of at most `cols` characters, a word longer than a
/// line on a line of its own; an explicit `\n` always breaks. Allocates nothing.
pub fn wrap_lines(s: &str, cols: usize) -> impl Iterator<Item = &str> {
    let cols = cols.max(1);
    s.split('\n').flat_map(move |para| Wrap { rest: para, cols })
}

struct Wrap<'a> {
    rest: &'a str,
    cols: usize,
}

impl<'a> Iterator for Wrap<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let s = self.rest.trim_start_matches(' ');
        if s.is_empty() {
            self.rest = s;
            return None;
        }
        // Walk words while they fit.
        let mut end = 0; // byte end of the last word that fits
        let mut n = 0; // chars up to `end`
        let mut chars = 0;
        let mut at = 0;
        for (i, c) in s.char_indices() {
            if c == ' ' {
                if chars <= self.cols {
                    end = i;
                    n = chars;
                } else {
                    break;
                }
            }
            chars += 1;
            at = i + c.len_utf8();
        }
        if chars <= self.cols {
            end = at;
            n = chars;
        }
        if n == 0 {
            // One word longer than a line: it takes the line whole.
            end = s.find(' ').unwrap_or(s.len());
        }
        self.rest = &s[end..];
        Some(s[..end].trim_end_matches(' '))
    }
}

/// How a panel is framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelStyle {
    /// A HUD plate: dark glass, a thin rim, no ornament.
    Hud,
    /// A window or a menu: the full frame with gold corner studs.
    Window,
    /// A tooltip or a popover: tight, lighter.
    Tip,
    /// The debug overlays: flat, cool, dense.
    Debug,
}

/// A button's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// A menu row: full width, gold when focused.
    Menu,
    /// A small chrome button (Bag, Book).
    Chip,
    /// A tab.
    Tab { on: bool },
}

/// What a dragged thing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPayload {
    Bag(u8),
    Craft(u8),
    Bar(u8),
    Spell(SpellId),
}

/// Where a dragged thing can land (PRESENTATION.md §3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Bag(u8),
    Craft(u8),
    Bar(u8),
    /// The bar's plate between the slots.
    BarBackground,
    /// Anywhere on an open window that is not a slot.
    Window,
    /// Off every panel: asks to destroy.
    Outside,
}

/// A drag in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drag {
    pub payload: DragPayload,
    pub origin: DropTarget,
    pub start: (i32, i32),
    pub moved: bool,
    /// The icon the ghost shows.
    pub ghost: Option<jane_core::ids::SpriteId>,
}

/// What a slot shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotView {
    pub icon: Option<jane_core::ids::SpriteId>,
    pub count: u16,
    pub item: Option<ItemId>,
    pub spell: Option<SpellId>,
    /// Permille of a cooldown still to run, and of the GCD.
    pub cooldown: u16,
    pub gcd: u16,
    pub usable: bool,
    /// A small label in the corner (the bar's key).
    pub label: Option<&'static str>,
    /// Lit for a moment: a fail flash (red) or a press (gold), ticks left of 12.
    pub flash: u8,
    pub flash_bad: bool,
}

/// What happened to a slot this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotOut {
    pub hovered: bool,
    /// Pressed and released without moving: a click.
    pub clicked: bool,
    pub right_clicked: bool,
}

/// What a text field did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldOut {
    pub changed: bool,
    /// Return was pressed.
    pub submitted: bool,
}

/// What the app does for a screen (PRESENTATION.md §3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppIntent {
    NewGame {
        name: String,
    },
    Continue,
    Load(u8),
    Save(u8),
    ToTitle,
    Quit,
    Resume,
    /// Open the pause menu (the HUD's menu button).
    Pause,
    /// Open the Controls screen (from the title or the pause menu).
    Controls,
    /// Open the load list (from the title or the pause menu).
    LoadMenu,
    /// Open the save list (the pause menu, in reach of rest).
    SaveMenu,
    /// Back out of the screen on top.
    Back,
    /// Capture the next press for a binding (`action` row, `col` 0 and 1 keys, 2 mouse, 3 pad).
    Rebind {
        row: u8,
        col: u8,
    },
    /// Every binding back to `data/bindings.json`.
    ResetBindings,
    /// The Controls row's aim assist.
    Assist(jane_sim::input::AssistProfile),
    /// A line typed into the terminal.
    Console(String),
    /// The bag, book, quests or map window, on a tab.
    OpenWindow(u8),
    CloseWindow,
}

/// What the UI hands the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiOut {
    Command(Command),
    Intent(AppIntent),
}

/// A popover open on a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Popover {
    id: WidgetId,
    at: (i32, i32),
    focus: u8,
    opened: u32,
}

/// The UI.
#[derive(Debug)]
pub struct Ui {
    pub art: UiArt,
    pub input: UiInput,
    pub tick: u32,
    /// The canvas, px.
    pub canvas: (i32, i32),
    /// Widgets answer only while this is set: the screen on top sets it.
    pub interactive: bool,
    /// The focus ring shows (the last device was a pad or the keyboard in a screen).
    pub nav: bool,
    pub cmds: Vec<UiCmd>,
    pub out: Vec<UiOut>,
    hot: WidgetId,
    active: WidgetId,
    /// The widget the press that ended this frame began on.
    released_on: WidgetId,
    /// Rects the pointer belongs to the UI over, this frame and last.
    solid: Vec<Rect>,
    solid_last: Vec<Rect>,
    drops: Vec<(Rect, DropTarget)>,
    drops_last: Vec<(Rect, DropTarget)>,
    pub drag: Option<Drag>,
    dropped: Option<(DragPayload, DropTarget)>,
    /// A click that ended a drag that never moved: the slot that armed it was clicked.
    click: Option<WidgetId>,
    tip_want: Option<(WidgetId, Rect)>,
    tip_text: String,
    tip: (WidgetId, u32),
    popover: Option<Popover>,
    /// A text field has the keyboard.
    pub typing: bool,
    images: Vec<(UiImage, bool)>,
    clip: Rect,
    /// The pointer as it was last frame, to see it move.
    last_pointer: Option<(i32, i32)>,
    /// Where the reticle goes this frame (the assisted aim, §4), set by the app in play; the
    /// pointer's own mark otherwise. The OS cursor is hidden and the UI draws its own.
    pub reticle: Option<(i32, i32)>,
    /// Draw the pointer's mark (the app hides the OS cursor).
    pub draw_cursor: bool,
}

/// Ticks under the pointer before a tooltip shows (§3.2).
pub const TIP_TICKS: u32 = 20;
/// Canvas px the pointer moves before a press becomes a drag (§3.4).
pub const DRAG_START_PX: i32 = 4;

impl Ui {
    pub fn new(art: UiArt) -> Ui {
        Ui {
            art,
            input: UiInput::default(),
            tick: 0,
            canvas: (768, 432),
            interactive: true,
            nav: false,
            cmds: Vec::with_capacity(4096),
            out: Vec::with_capacity(16),
            hot: 0,
            active: 0,
            released_on: 0,
            solid: Vec::with_capacity(64),
            solid_last: Vec::with_capacity(64),
            drops: Vec::with_capacity(64),
            drops_last: Vec::with_capacity(64),
            drag: None,
            dropped: None,
            click: None,
            tip_want: None,
            tip_text: String::new(),
            tip: (0, 0),
            popover: None,
            typing: false,
            images: Vec::new(),
            clip: Rect::CANVAS,
            last_pointer: None,
            reticle: None,
            draw_cursor: false,
        }
    }

    /// Starts a frame: this frame's input, the presenter's tick, the canvas size.
    pub fn begin(&mut self, input: UiInput, tick: u32, canvas: (u16, u16)) {
        self.input = input;
        self.tick = tick;
        self.canvas = (i32::from(canvas.0), i32::from(canvas.1));
        self.cmds.clear();
        self.out.clear();
        self.interactive = true;
        self.hot = 0;
        self.typing = false;
        std::mem::swap(&mut self.solid, &mut self.solid_last);
        self.solid.clear();
        std::mem::swap(&mut self.drops, &mut self.drops_last);
        self.drops.clear();
        self.dropped = None;
        self.click = None;
        self.tip_want = None;
        self.clip = Rect::CANVAS;
        self.reticle = None;
        let moved = self.input.pointer.is_some() && self.input.pointer != self.last_pointer;
        if !self.input.actions.is_empty() || self.input.pad {
            self.nav = true;
        }
        if moved || self.input.pressed {
            self.nav = self.input.pad && !moved;
        }
        self.last_pointer = self.input.pointer;
        // The drag: start once moved far enough, land on release.
        if let Some(d) = &mut self.drag {
            if let Some(p) = self.input.pointer
                && !d.moved
                && ((p.0 - d.start.0).abs() >= DRAG_START_PX || (p.1 - d.start.1).abs() >= DRAG_START_PX)
            {
                d.moved = true;
            }
            if !self.input.held {
                let d = *d;
                self.drag = None;
                if d.moved {
                    let at = self.input.pointer.unwrap_or(d.start);
                    let target =
                        self.drops_last.iter().rev().find(|(r, _)| r.contains(at)).map_or(DropTarget::Outside, |e| e.1);
                    if target != d.origin {
                        self.dropped = Some((d.payload, target));
                    }
                } else {
                    self.click = Some(self.active);
                }
                self.active = 0;
            }
        } else if !self.input.held {
            self.released_on = if self.input.released { self.active } else { 0 };
            self.active = 0;
        }
    }

    /// Ends the frame into `frame`: the draw list (with the tooltip, the drag ghost and the
    /// pointer's mark on top) and any image that changed.
    pub fn finish(&mut self, frame: &mut Frame) {
        self.draw_tip();
        if let Some(d) = self.drag.filter(|d| d.moved)
            && let (Some(p), Some(icon)) = (self.input.pointer, d.ghost)
        {
            self.set_clip(Rect::CANVAS);
            let src = self.art.icon(icon);
            self.sprite(src, Rect::new(p.0 - 16, p.1 - 16, 32, 32), 0, 179);
        }
        self.draw_pointer();
        std::mem::swap(&mut frame.ui, &mut self.cmds);
        self.cmds.clear();
        if frame.ui_images.len() < self.images.len() {
            frame.ui_images.resize(self.images.len(), UiImage::default());
        }
        for (i, (img, dirty)) in self.images.iter_mut().enumerate() {
            if *dirty {
                frame.ui_images[i].clone_from(img);
                *dirty = false;
            }
        }
    }

    /// The pointer's mark: the reticle at the assisted aim in play (with a faint dot where the
    /// hand really is when the assist has pulled it), the arrow over the UI, the hand in a drag.
    fn draw_pointer(&mut self) {
        let Some(p) = self.input.pointer.filter(|_| self.draw_cursor) else { return };
        self.set_clip(Rect::CANVAS);
        let over_ui = self.wants_pointer();
        match self.reticle {
            Some(r) if !over_ui => {
                if (r.0 - p.0).abs() + (r.1 - p.1).abs() > 3 {
                    self.fill(Rect::new(p.0 - 1, p.1 - 1, 2, 2), argb(style::gold(), 120));
                }
                self.mark(Mark::Reticle, r.0 - 7, r.1 - 7, 255);
            }
            _ if self.drag.is_some_and(|d| d.moved) => self.mark(Mark::Hand, p.0 - 6, p.1 - 4, 255),
            _ => self.mark(Mark::Arrow, p.0 - 1, p.1 - 1, 255),
        }
    }

    /// The pointer is over something the UI drew last frame: a click there is not the world's.
    pub fn wants_pointer(&self) -> bool {
        self.drag.is_some()
            || self.input.pointer.is_some_and(|p| self.solid_last.iter().chain(&self.solid).any(|r| r.contains(p)))
    }

    /// What was dropped this frame, if a drag ended on a target.
    pub fn take_drop(&mut self) -> Option<(DragPayload, DropTarget)> {
        self.dropped.take()
    }

    pub fn command(&mut self, c: Command) {
        self.out.push(UiOut::Command(c));
    }

    pub fn intent(&mut self, i: AppIntent) {
        self.out.push(UiOut::Intent(i));
    }

    /// The UI's run-time picture in `slot`, made `w x h` if it is not; mark it changed with
    /// [`Ui::image_changed`] after writing it.
    pub fn image_mut(&mut self, slot: u16, w: u16, h: u16) -> &mut UiImage {
        let i = usize::from(slot);
        if self.images.len() <= i {
            self.images.resize(i + 1, (UiImage::default(), false));
        }
        let e = &mut self.images[i];
        if e.0.w != w || e.0.h != h {
            let gen_ = e.0.generation;
            e.0 = UiImage::new(w, h);
            e.0.generation = gen_.wrapping_add(1);
            e.1 = true;
        }
        &mut e.0
    }

    pub fn image_changed(&mut self, slot: u16) {
        if let Some(e) = self.images.get_mut(usize::from(slot)) {
            e.0.generation = e.0.generation.wrapping_add(1);
            e.1 = true;
        }
    }

    // --- raw draws -------------------------------------------------------------------------

    pub fn set_clip(&mut self, r: Rect) {
        if r != self.clip {
            self.clip = r;
            self.cmds.push(UiCmd::Clip(r));
        }
    }

    pub fn fill(&mut self, dst: Rect, argb: u32) {
        if !dst.is_empty() && argb >> 24 != 0 {
            self.cmds.push(UiCmd::Fill { dst, argb });
        }
    }

    /// A fill in palette index `ix` at coverage `a`.
    pub fn fill_ix(&mut self, dst: Rect, ix: Ix, a: u8) {
        self.fill(dst, argb(ix, a));
    }

    pub fn sprite(&mut self, src: Src, dst: Rect, ink: u16, alpha: u8) {
        if alpha != 0 && !dst.is_empty() {
            self.cmds.push(UiCmd::Sprite { page: self.art.page, src, dst, ink, alpha, mirror: false });
        }
    }

    pub fn image(&mut self, slot: u16, src: Src, dst: Rect, alpha: u8) {
        if alpha != 0 && !dst.is_empty() {
            self.cmds.push(UiCmd::Image { slot, src, dst, alpha });
        }
    }

    /// A mark at `(x, y)` (its top-left), in its own colours.
    pub fn mark(&mut self, m: Mark, x: i32, y: i32, alpha: u8) {
        let s = self.art.mark(m);
        self.sprite(s, Rect::new(x, y, i32::from(s.w), i32::from(s.h)), 0, alpha);
    }

    /// A mark flattened to one ink.
    pub fn mark_ink(&mut self, m: Mark, x: i32, y: i32, ink: Ix, alpha: u8) {
        let s = self.art.mark(m);
        self.sprite(s, Rect::new(x, y, i32::from(s.w), i32::from(s.h)), ink.0, alpha);
    }

    pub fn icon(&mut self, icon: jane_core::ids::SpriteId, x: i32, y: i32, small: bool, alpha: u8) {
        let s = if small { self.art.icon_small(icon) } else { self.art.icon(icon) };
        self.sprite(s, Rect::new(x, y, i32::from(s.w), i32::from(s.h)), 0, alpha);
    }

    /// `s` with the top-left of its first cell at `(x, y)`; returns the x after it.
    pub fn text(&mut self, x: i32, y: i32, s: &str, ink: Ink) -> i32 {
        let adv = advance(ink.face);
        let off = ink.face.shadow();
        if ink.shadow {
            self.glyphs(x + off, y + off, s, ink, style::INK, ink.alpha);
        }
        if ink.outline {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                self.glyphs(x + dx, y + dy, s, ink, style::INK, ink.alpha);
            }
        }
        self.glyphs(x, y, s, ink, ink.ix, ink.alpha);
        x + s.chars().count() as i32 * adv
    }

    fn glyphs(&mut self, x: i32, y: i32, s: &str, ink: Ink, ix: Ix, alpha: u8) {
        let adv = advance(ink.face);
        let (w, h) = self.canvas;
        let line = line_h(ink.face);
        if y + line < 0 || y > h {
            return;
        }
        for (k, c) in s.chars().enumerate() {
            if c == ' ' {
                continue;
            }
            let cx = x + k as i32 * adv;
            if cx > w {
                break;
            }
            let (src, (ix0, iy0)) = self.art.glyph(ink.face, ink.bold, c);
            let dst = Rect::new(cx + i32::from(ix0), y + i32::from(iy0), i32::from(src.w), i32::from(src.h));
            self.sprite(src, dst, ix.0, alpha);
        }
    }

    /// `s` centred in `r` (both ways).
    pub fn text_in(&mut self, r: Rect, s: &str, ink: Ink) -> i32 {
        let w = text_w(ink.face, s);
        let y = i32::from(r.y) + (i32::from(r.h) - line_h(ink.face)) / 2;
        self.text(i32::from(r.x) + (i32::from(r.w) - w) / 2, y, s, ink)
    }

    /// `s` ending at `right`.
    pub fn text_right(&mut self, right: i32, y: i32, s: &str, ink: Ink) {
        let w = text_w(ink.face, s);
        self.text(right - w, y, s, ink);
    }

    /// `s` wrapped into `r`'s width, from its top; returns the lines drawn. Lines below `r` are
    /// not drawn.
    pub fn wrapped(&mut self, r: Rect, s: &str, ink: Ink) -> i32 {
        self.wrapped_reveal(r, s, ink, usize::MAX)
    }

    /// As [`Ui::wrapped`], showing only the first `shown` characters: the whole text is laid
    /// out, so the lines never reflow as it types (§3.1).
    pub fn wrapped_reveal(&mut self, r: Rect, s: &str, ink: Ink, shown: usize) -> i32 {
        let cols = (i32::from(r.w) / advance(ink.face)).max(1) as usize;
        let lh = line_h(ink.face);
        let mut left = shown;
        let mut n = 0;
        for line in wrap_lines(s, cols) {
            let y = i32::from(r.y) + n * lh;
            n += 1;
            if y + lh > r.bottom() + 1 {
                continue;
            }
            if left == 0 {
                continue;
            }
            let len = line.chars().count();
            let take = len.min(left);
            let cut = line.char_indices().nth(take).map_or(line.len(), |(i, _)| i);
            self.text(i32::from(r.x), y, &line[..cut], ink);
            left = left.saturating_sub(len + 1);
        }
        n
    }

    // --- chrome ----------------------------------------------------------------------------

    /// A framed panel. The pointer over it is the UI's.
    pub fn panel(&mut self, r: Rect, style: PanelStyle) {
        self.solid.push(r);
        let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
        let (top, bottom, a) = match style {
            PanelStyle::Hud => (style::panel_top(), style::panel_bottom(), 200),
            PanelStyle::Window => (style::panel_top(), style::panel_bottom(), style::PANEL_A),
            PanelStyle::Tip => (jane_art::Ramp::UiPanel.at(jane_art::Tone::Base), style::panel_top(), 240),
            PanelStyle::Debug => {
                (jane_art::Ramp::UiSlot.at(jane_art::Tone::Mid), jane_art::Ramp::UiSlot.at(jane_art::Tone::Shade), 242)
            }
        };
        // The body: a two-tone vertical gradient in bands (a calm fill, never a dither).
        let (t, b) = (argb(top, a), argb(bottom, a));
        let bands = 4.min(h - 4).max(1);
        for k in 0..bands {
            let y0 = y + 2 + (h - 4) * k / bands;
            let y1 = y + 2 + (h - 4) * (k + 1) / bands;
            self.fill(Rect::new(x + 2, y0, w - 4, y1 - y0), lerp_argb(t, b, k * 255 / (bands - 1).max(1)));
        }
        // The rim: `k`, cut at the corners by a px.
        let ink = argb(style::INK, 255);
        self.fill(Rect::new(x + 2, y, w - 4, 1), ink);
        self.fill(Rect::new(x + 2, y + h - 1, w - 4, 1), ink);
        self.fill(Rect::new(x, y + 2, 1, h - 4), ink);
        self.fill(Rect::new(x + w - 1, y + 2, 1, h - 4), ink);
        self.fill(Rect::new(x + 1, y + 1, 1, 1), ink);
        self.fill(Rect::new(x + w - 2, y + 1, 1, 1), ink);
        self.fill(Rect::new(x + 1, y + h - 2, 1, 1), ink);
        self.fill(Rect::new(x + w - 2, y + h - 2, 1, 1), ink);
        // Inside the rim: a lit top edge, a darker bottom, the sides between.
        let lit = argb(style::panel_lit(), if style == PanelStyle::Debug { 70 } else { 150 });
        let shade = argb(style::INK, 120);
        self.fill(Rect::new(x + 2, y + 1, w - 4, 1), lit);
        self.fill(Rect::new(x + 1, y + 2, 1, h - 4), fade(lit, 150));
        self.fill(Rect::new(x + 2, y + h - 2, w - 4, 1), shade);
        self.fill(Rect::new(x + w - 2, y + 2, 1, h - 4), fade(shade, 180));
        if style == PanelStyle::Window {
            // Gold studs at the corners and a gold hairline under the lit edge.
            self.fill(Rect::new(x + 6, y + 3, w - 12, 1), argb(style::gold_deep(), 90));
            for (sx, sy) in [(x + 2, y + 2), (x + w - 9, y + 2), (x + 2, y + h - 9), (x + w - 9, y + h - 9)] {
                self.mark(Mark::Diamond, sx, sy, 255);
            }
        }
    }

    /// A thin rule from `x0` to `x1` at `y`: gold at the middle, fading to nothing at the ends.
    pub fn rule(&mut self, x0: i32, x1: i32, y: i32, ix: Ix) {
        let w = x1 - x0;
        let steps: i32 = 6;
        for k in 0..steps {
            let a = 40 + 140 * (steps - 1 - (2 * k - (steps - 1)).abs()) / (steps - 1);
            let xa = x0 + w * k / steps;
            let xb = x0 + w * (k + 1) / steps;
            self.fill(Rect::new(xa, y, xb - xa, 1), argb(ix, a.clamp(0, 255) as u8));
        }
    }

    /// A bar: a sunk track, the fill in `ramp` lit along its top, the damage-lag tail, ticks at
    /// the quarters. `frac` and `lag` are permille of the track.
    pub fn bar(&mut self, r: Rect, frac: u16, lag: u16, ramp: jane_art::Ramp) {
        use jane_art::Tone;
        let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
        self.fill(r, argb(style::INK, 255));
        let inner = Rect::new(x + 1, y + 1, w - 2, h - 2);
        self.fill(inner, argb(jane_art::Ramp::UiSlot.at(Tone::Shade), 255));
        let iw = i32::from(inner.w);
        let fw = iw * i32::from(frac.min(1000)) / 1000;
        let lw = iw * i32::from(lag.min(1000)) / 1000;
        let ih = i32::from(inner.h);
        if lw > fw {
            // The lag: a ghost of the bar in its own pale tone, so it reads as what was just lost.
            self.fill(Rect::new(x + 1 + fw, y + 1, lw - fw, ih), argb(ramp.at(Tone::High), 150));
            self.fill(Rect::new(x + 1 + fw, y + 1, lw - fw, 1), argb(jane_art::Ramp::UiLag.at(Tone::Light), 200));
        }
        if fw > 0 {
            // Bands down the fill: a bright lip, the body, a shaded foot.
            let rows = [
                (Tone::High, 1),
                (Tone::Light, (ih - 3).max(1) / 2),
                (Tone::Base, (ih - 3).max(1) - (ih - 3).max(1) / 2),
                (Tone::Mid, 1),
                (Tone::Shade, 1),
            ];
            let mut yy = y + 1;
            for (tone, n) in rows {
                let n = n.min(y + 1 + ih - yy);
                if n > 0 {
                    self.fill(Rect::new(x + 1, yy, fw, n), argb(ramp.at(tone), 255));
                }
                yy += n;
            }
            // The leading edge catches the light.
            self.fill(Rect::new(x + fw, y + 1, 1, ih), argb(ramp.at(Tone::Glint), 200));
        }
        for q in 1..4 {
            self.fill(Rect::new(x + 1 + iw * q / 4, y + 1, 1, ih), argb(style::INK, 90));
        }
    }

    /// A slot's well at `r` (36 or 20 px square).
    pub fn well(&mut self, r: Rect, lit: bool) {
        let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
        self.fill(Rect::new(x + 1, y, w - 2, h), argb(style::INK, 255));
        self.fill(Rect::new(x, y + 1, w, h - 2), argb(style::INK, 255));
        self.fill(Rect::new(x + 1, y + 1, w - 2, h - 2), argb(style::well(), 235));
        // Sunk: the shadow on the top and left, the lit lip on the bottom and right.
        self.fill(Rect::new(x + 1, y + 1, w - 2, 1), argb(style::INK, 160));
        self.fill(Rect::new(x + 1, y + 2, 1, h - 3), argb(style::INK, 110));
        self.fill(Rect::new(x + 2, y + h - 2, w - 3, 1), argb(style::well_lit(), if lit { 255 } else { 150 }));
        self.fill(Rect::new(x + w - 2, y + 2, 1, h - 4), argb(style::well_lit(), if lit { 200 } else { 110 }));
    }

    /// The focus ring: gold brackets at the corners of `r`, breathing with the tick.
    pub fn focus_ring(&mut self, r: Rect) {
        let (x, y, w, h) = (i32::from(r.x) - 2, i32::from(r.y) - 2, i32::from(r.w) + 4, i32::from(r.h) + 4);
        let breathe = (self.tick / 4 % 16) as i32;
        let a = (170 + 5 * (breathe - 8).abs()) as u8;
        let g = argb(style::gold(), a);
        let n = 5;
        for (cx, cy, dx, dy) in
            [(x, y, 1, 1), (x + w - 1, y, -1, 1), (x, y + h - 1, 1, -1), (x + w - 1, y + h - 1, -1, -1)]
        {
            let hx = if dx > 0 { cx } else { cx - n + 1 };
            let vy = if dy > 0 { cy } else { cy - n + 1 };
            self.fill(Rect::new(hx, cy, n, 1), g);
            self.fill(Rect::new(cx, vy, 1, n), g);
            self.fill(Rect::new(hx, cy - dy, n, 1), argb(style::INK, a / 2));
        }
    }

    // --- widgets ---------------------------------------------------------------------------

    /// The pointer is over `r` and this layer answers.
    pub fn hover(&self, r: Rect) -> bool {
        self.interactive && self.drag.is_none_or(|d| !d.moved) && self.input.pointer.is_some_and(|p| r.contains(p))
    }

    /// A button: drawn, and `true` the frame it is clicked (or confirmed while `focused`).
    pub fn button(
        &mut self,
        id: WidgetId,
        r: Rect,
        label: &str,
        kind: ButtonKind,
        enabled: bool,
        focused: bool,
    ) -> bool {
        self.solid.push(r);
        let over = enabled && self.hover(r);
        if over {
            self.hot = id;
            if self.input.pressed {
                self.active = id;
            }
        }
        let held = over && self.active == id && self.input.held;
        let clicked = enabled
            && ((over && self.input.released && self.active_was(id))
                || (focused && self.interactive && self.input.has(UiAction::Confirm)));
        // A menu row shows its light whatever the device (the rows follow the pointer); the
        // other kinds only while the keys or a pad drive.
        let lit = over || (focused && (self.nav || kind == ButtonKind::Menu));
        let (x, y, w, h) = (i32::from(r.x), i32::from(r.y), i32::from(r.w), i32::from(r.h));
        let down = i32::from(held);
        match kind {
            ButtonKind::Menu => {
                if lit {
                    // A gold band behind the words, fading out to the sides.
                    let g = style::gold_deep();
                    self.fill(
                        Rect::new(x + 12, y + 1, w - 24, h - 2),
                        argb(jane_art::Ramp::UiPanel.at(jane_art::Tone::Light), 70),
                    );
                    self.rule(x, x + w, y, g);
                    self.rule(x, x + w, y + h - 1, g);
                    self.mark(Mark::Diamond, x + 4, y + (h - 7) / 2, 255);
                    self.mark(Mark::Diamond, x + w - 11, y + (h - 7) / 2, 255);
                }
                let ink = if !enabled {
                    style::dim()
                } else if lit {
                    style::text_bright()
                } else {
                    style::text()
                };
                self.text_in(Rect::new(x, y + down, w, h), label, Ink::small(ink).shadow());
            }
            ButtonKind::Chip => {
                self.panel(r, PanelStyle::Hud);
                if lit {
                    self.fill(r.inset(2), argb(jane_art::Ramp::UiPanel.at(jane_art::Tone::Light), 60));
                }
                let ink = if enabled { if lit { style::gold() } else { style::text() } } else { style::dim() };
                self.text_in(Rect::new(x, y + down, w, h), label, Ink::fine(ink).shadow());
            }
            ButtonKind::Tab { on } => {
                let fill = if on { style::panel_top() } else { style::panel_bottom() };
                self.fill(Rect::new(x + 1, y, w - 2, h), argb(style::INK, 255));
                self.fill(Rect::new(x + 2, y + 1, w - 4, h - 1), argb(fill, 240));
                if on {
                    self.fill(Rect::new(x + 2, y + 1, w - 4, 1), argb(style::gold(), 200));
                }
                let ink = if on {
                    style::gold()
                } else if lit {
                    style::text_bright()
                } else {
                    style::quiet()
                };
                self.text_in(Rect::new(x, y + 1, w, h), label, Ink::small(ink).shadow());
            }
        }
        if focused && self.nav && kind != ButtonKind::Menu {
            self.focus_ring(r);
        }
        clicked
    }

    fn active_was(&self, id: WidgetId) -> bool {
        self.active == id || self.released_on == id || self.click == Some(id)
    }

    /// A slot: the well, the icon, the count, the sweeps and the label; a drag source when
    /// `drag` names what it holds, and a drop target as `target`.
    pub fn slot(
        &mut self,
        id: WidgetId,
        r: Rect,
        v: &SlotView,
        drag: Option<DragPayload>,
        target: DropTarget,
        focused: bool,
    ) -> SlotOut {
        self.solid.push(r);
        self.drops.push((r, target));
        let size = i32::from(r.w);
        let over = self.hover(r);
        let mut out = SlotOut { hovered: over, ..SlotOut::default() };
        if over {
            self.hot = id;
            if self.input.pressed {
                self.active = id;
                if let Some(p) = drag
                    && let Some(at) = self.input.pointer
                {
                    self.drag = Some(Drag { payload: p, origin: target, start: at, moved: false, ghost: v.icon });
                }
            }
            if self.input.right_pressed {
                out.right_clicked = true;
            }
        }
        if self.click == Some(id) || (over && self.input.released && self.active_was(id) && self.drag.is_none()) {
            out.clicked = true;
        }
        let dragging_me = self.drag.is_some_and(|d| d.moved && Some(d.payload) == drag && drag.is_some());
        let (x, y) = (i32::from(r.x), i32::from(r.y));
        self.well(r, over);
        if let Some(icon) = v.icon {
            let a = if dragging_me {
                70
            } else if v.usable {
                255
            } else {
                150
            };
            if size >= 32 {
                self.icon(icon, x + (size - 32) / 2, y + (size - 32) / 2, false, a);
            } else {
                self.icon(icon, x + (size - 16) / 2, y + (size - 16) / 2, true, a);
            }
        }
        // The cooldown veil: dark, with a bright edge where it will end; the GCD a pale wash.
        if v.cooldown > 0
            && let Some(src) = self.art.sweep(size, v.cooldown)
        {
            self.sprite(src, Rect::new(x, y, size, size), style::INK.0, 190);
        } else if v.gcd > 0
            && let Some(src) = self.art.sweep(size, v.gcd)
        {
            self.sprite(src, Rect::new(x, y, size, size), jane_art::Ramp::UiVeil.at(jane_art::Tone::Base).0, 77);
        }
        if v.flash > 0 {
            let ix = if v.flash_bad { style::bad() } else { style::gold() };
            let a = (u32::from(v.flash) * 16).min(200) as u8;
            self.fill(r.inset(2), argb(ix, a / 2));
            let ring = argb(ix, a);
            self.fill(Rect::new(x + 1, y + 1, size - 2, 1), ring);
            self.fill(Rect::new(x + 1, y + size - 2, size - 2, 1), ring);
            self.fill(Rect::new(x + 1, y + 1, 1, size - 2), ring);
            self.fill(Rect::new(x + size - 2, y + 1, 1, size - 2), ring);
        }
        if v.count > 1 {
            let mut buf = [0u8; 6];
            let s = fmt_u32(u32::from(v.count), &mut buf);
            self.text_right(x + size - 2, y + size - 13, s, Ink::fine(style::text_bright()).outline());
        }
        if let Some(l) = v.label {
            self.text(x + 3, y + 2, l, Ink::fine(style::quiet()).outline());
        }
        if focused && self.nav {
            self.focus_ring(r);
        }
        // A drop target lights under a moving drag.
        if self.drag.is_some_and(|d| d.moved) && self.input.pointer.is_some_and(|p| r.contains(p)) {
            self.focus_ring(r);
        }
        out
    }

    /// Asks for a tooltip on widget `id` over `r`: shown once the pointer (or the focus) has
    /// stayed `TIP_TICKS`. `text` is only read when it shows; `\n` breaks a line, and the first
    /// line is the title.
    pub fn tip(&mut self, id: WidgetId, r: Rect, text: impl FnOnce(&mut String)) {
        if self.drag.is_some_and(|d| d.moved) || self.popover.is_some() {
            return;
        }
        if self.tip.0 != id {
            self.tip = (id, self.tick);
        }
        self.tip_want = Some((id, r));
        if self.tick.wrapping_sub(self.tip.1) >= TIP_TICKS {
            self.tip_text.clear();
            text(&mut self.tip_text);
        }
    }

    fn draw_tip(&mut self) {
        let Some((id, r)) = self.tip_want else {
            self.tip = (0, 0);
            return;
        };
        if self.tip.0 != id || self.tick.wrapping_sub(self.tip.1) < TIP_TICKS || self.tip_text.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.tip_text);
        let cols = 40;
        let mut lines = 0;
        let mut widest = 0;
        for l in wrap_lines(&text, cols) {
            lines += 1;
            widest = widest.max(l.chars().count() as i32);
        }
        let (fw, lh) = (advance(Face::Fine), line_h(Face::Fine));
        let w = widest * fw + 16;
        let h = lines * lh + 10;
        let (cw, ch) = self.canvas;
        let mut x = self.input.pointer.map_or(i32::from(r.x) + i32::from(r.w) + 6, |p| p.0 + 14);
        let mut y = self.input.pointer.map_or(i32::from(r.y), |p| p.1 + 10);
        if x + w > cw - 4 {
            x = (cw - 4 - w).max(4);
        }
        if y + h > ch - 4 {
            y = (ch - 4 - h).max(4);
        }
        self.set_clip(Rect::CANVAS);
        let appear = self.tick.wrapping_sub(self.tip.1).saturating_sub(TIP_TICKS);
        let a = (appear * 48).min(255) as u8;
        self.panel(Rect::new(x, y, w, h), PanelStyle::Tip);
        for (i, l) in wrap_lines(&text, cols).enumerate() {
            let ink = if i == 0 { Ink::fine(style::gold()).alpha(a) } else { Ink::fine(style::text()).alpha(a) };
            self.text(x + 8, y + 5 + i as i32 * lh, l, ink.shadow());
        }
        self.tip_text = text;
    }

    /// A single-line text field of at most `max` characters, with a blinking caret. Typing goes
    /// to it while this layer is interactive.
    pub fn text_field(&mut self, r: Rect, buf: &mut String, max: usize, ink: Ink) -> FieldOut {
        let mut out = FieldOut::default();
        self.solid.push(r);
        if self.interactive {
            self.typing = true;
            for c in self.input.typed.chars() {
                if buf.chars().count() < max && (c == ' ' || c.is_ascii_graphic()) {
                    buf.push(c);
                    out.changed = true;
                }
            }
            if self.input.keys.has(sc::BACKSPACE) && buf.pop().is_some() {
                out.changed = true;
            }
            out.submitted = self.input.keys.has(sc::RETURN) || self.input.keys.has(sc::KP_ENTER);
        }
        self.well(r, true);
        let y = i32::from(r.y) + (i32::from(r.h) - line_h(ink.face)) / 2;
        let end = self.text(i32::from(r.x) + 6, y, buf, ink);
        if self.interactive && (self.tick / 20) % 2 == 0 {
            self.fill(Rect::new(end + 1, y + 1, 2, line_h(ink.face) - 3), argb(style::gold(), 230));
        }
        out
    }

    /// A popover of `items` at `at`, opened by [`Ui::open_popover`] for widget `id`; the index
    /// chosen the frame it is chosen. Any press outside closes it.
    pub fn popover(&mut self, id: WidgetId, items: &[&str]) -> Option<usize> {
        let p = self.popover.filter(|p| p.id == id)?;
        let fw = advance(Face::Small);
        let lh = 22;
        let w = items.iter().map(|s| s.chars().count() as i32).max().unwrap_or(4) * fw + 28;
        let h = items.len() as i32 * lh + 8;
        let (cw, ch) = self.canvas;
        let x = p.at.0.min(cw - w - 4);
        let y = p.at.1.min(ch - h - 4);
        let r = Rect::new(x, y, w, h);
        self.set_clip(Rect::CANVAS);
        self.panel(r, PanelStyle::Tip);
        let mut chosen = None;
        let mut focus = p.focus;
        for (i, s) in items.iter().enumerate() {
            let row = Rect::new(x + 3, y + 4 + i as i32 * lh, w - 6, lh);
            let over = self.input.pointer.is_some_and(|q| row.contains(q));
            if over {
                focus = i as u8;
            }
            let lit = focus == i as u8;
            if lit {
                self.fill(row, argb(jane_art::Ramp::UiPanel.at(jane_art::Tone::Light), 90));
                self.mark(Mark::Diamond, x + 6, i32::from(row.y) + 7, 255);
            }
            let ink = if lit { style::text_bright() } else { style::text() };
            self.text(x + 18, i32::from(row.y) + 2, s, Ink::small(ink).shadow());
            if over && self.input.released && self.tick != p.opened {
                chosen = Some(i);
            }
        }
        let n = items.len() as u8;
        for a in self.input.actions.clone() {
            match a {
                UiAction::Up => focus = (focus + n - 1) % n.max(1),
                UiAction::Down => focus = (focus + 1) % n.max(1),
                UiAction::Confirm => chosen = Some(usize::from(focus)),
                UiAction::Cancel => self.popover = None,
                _ => {}
            }
        }
        if self.input.pressed && self.tick != p.opened && !self.input.pointer.is_some_and(|q| r.contains(q)) {
            self.popover = None;
        }
        if let Some(pp) = &mut self.popover {
            pp.focus = focus;
        }
        if chosen.is_some() {
            self.popover = None;
        }
        self.solid.push(r);
        chosen
    }

    /// Open a popover for widget `id` at `at` (canvas px).
    pub fn open_popover(&mut self, id: WidgetId, at: (i32, i32)) {
        self.popover = Some(Popover { id, at, focus: 0, opened: self.tick });
    }

    pub fn popover_open(&self) -> bool {
        self.popover.is_some()
    }

    pub fn close_popover(&mut self) {
        self.popover = None;
    }

    /// Registers `r` as where a drag can land as `target` (a window's body).
    pub fn drop_area(&mut self, r: Rect, target: DropTarget) {
        self.drops.push((r, target));
    }

    /// Registers `r` as the UI's for the pointer without drawing.
    pub fn claim(&mut self, r: Rect) {
        self.solid.push(r);
    }

    pub fn hot(&self) -> WidgetId {
        self.hot
    }
}

/// `a` toward `b` by `t` of 255, all four channels.
pub fn lerp_argb(a: u32, b: u32, t: i32) -> u32 {
    let t = t.clamp(0, 255);
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 0xff) as i32, ((b >> s) & 0xff) as i32);
        ((x + (y - x) * t / 255) as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// `n` in decimal into `buf`, no allocation.
pub fn fmt_u32(n: u32, buf: &mut [u8]) -> &str {
    let mut i = buf.len();
    let mut v = n;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 || i == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui() -> Ui {
        Ui::new(UiArt::build(1).0)
    }

    fn at(p: (i32, i32), held: bool, pressed: bool, released: bool) -> UiInput {
        UiInput { pointer: Some(p), held, pressed, released, ..UiInput::default() }
    }

    #[test]
    fn wrap_is_greedy_on_spaces_and_breaks_on_newlines() {
        let v: Vec<&str> = wrap_lines("the lamp stops the train", 10).collect();
        assert_eq!(v, ["the lamp", "stops the", "train"]);
        let v: Vec<&str> = wrap_lines("a  b ", 10).collect();
        assert_eq!(v, ["a  b"]);
        let v: Vec<&str> = wrap_lines("one\ntwo three", 20).collect();
        assert_eq!(v, ["one", "two three"]);
        let v: Vec<&str> = wrap_lines("supercalifragilistic is long", 6).collect();
        assert_eq!(v, ["supercalifragilistic", "is", "long"]);
        assert_eq!(wrap_lines("", 5).count(), 0);
        // Exactly the width fits.
        let v: Vec<&str> = wrap_lines("abcde fghij", 11).collect();
        assert_eq!(v, ["abcde fghij"]);
    }

    #[test]
    fn a_revealed_line_is_laid_out_as_the_whole_line() {
        // Ten chars shown of a text that wraps: the glyphs drawn sit where the full layout puts
        // them, so the box never reflows as it types.
        let mut u = ui();
        u.begin(UiInput::default(), 1, (768, 432));
        let r = Rect::new(0, 0, 12 * 8, 200);
        let full = u.wrapped(r, "one two three four", Ink::small(style::text()));
        let all: Vec<UiCmd> = u.cmds.drain(..).collect();
        let part = u.wrapped_reveal(r, "one two three four", Ink::small(style::text()), 10);
        assert_eq!(full, part, "the same lines either way");
        let shown: Vec<UiCmd> = u.cmds.drain(..).collect();
        assert_eq!(shown.len(), 8, "\"one two th\": eight glyphs, the spaces skipped");
        assert!(shown.iter().all(|c| all.contains(c)), "each at its place in the whole layout");
    }

    #[test]
    fn a_button_clicks_on_release_over_it_and_confirms_when_focused() {
        let mut u = ui();
        let r = Rect::new(10, 10, 100, 20);
        let id = wid("b", 0);
        u.begin(at((20, 15), true, true, false), 1, (768, 432));
        assert!(!u.button(id, r, "Go", ButtonKind::Menu, true, false));
        u.begin(at((20, 15), false, false, true), 2, (768, 432));
        assert!(u.button(id, r, "Go", ButtonKind::Menu, true, false));
        // Pressed inside, released outside: no click.
        u.begin(at((20, 15), true, true, false), 3, (768, 432));
        u.button(id, r, "Go", ButtonKind::Menu, true, false);
        u.begin(at((300, 300), false, false, true), 4, (768, 432));
        assert!(!u.button(id, r, "Go", ButtonKind::Menu, true, false));
        // Confirm on the focused one; nothing on a disabled one.
        u.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 5, (768, 432));
        assert!(u.button(id, r, "Go", ButtonKind::Menu, true, true));
        u.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 6, (768, 432));
        assert!(!u.button(id, r, "Go", ButtonKind::Menu, false, true));
        // A layer that is not on top does not answer.
        u.begin(UiInput { actions: vec![UiAction::Confirm], ..UiInput::default() }, 7, (768, 432));
        u.interactive = false;
        assert!(!u.button(id, r, "Go", ButtonKind::Menu, true, true));
    }

    #[test]
    fn a_drag_starts_after_four_px_and_lands_on_the_last_target_under_the_pointer() {
        let mut u = ui();
        let a = Rect::new(0, 0, 36, 36);
        let b = Rect::new(100, 0, 36, 36);
        let v = SlotView { icon: None, ..SlotView::default() };
        let frame = |u: &mut Ui| {
            u.drop_area(Rect::new(0, 0, 400, 200), DropTarget::Window);
            u.slot(wid("s", 0), a, &v, Some(DragPayload::Bag(0)), DropTarget::Bag(0), false);
            u.slot(wid("s", 1), b, &v, Some(DragPayload::Bag(1)), DropTarget::Bag(1), false);
        };
        u.begin(at((10, 10), true, true, false), 1, (768, 432));
        frame(&mut u);
        assert!(u.drag.is_some_and(|d| !d.moved));
        u.begin(at((12, 11), true, false, false), 2, (768, 432));
        frame(&mut u);
        assert!(u.drag.is_some_and(|d| !d.moved), "three px is not a drag");
        u.begin(at((110, 10), true, false, false), 3, (768, 432));
        frame(&mut u);
        assert!(u.drag.is_some_and(|d| d.moved));
        u.begin(at((110, 10), false, false, true), 4, (768, 432));
        assert_eq!(u.take_drop(), Some((DragPayload::Bag(0), DropTarget::Bag(1))));
        assert!(u.drag.is_none());
        // Dropped on the window's body, off a slot; and off every panel.
        for (p, want) in [((300, 100), DropTarget::Window), ((600, 400), DropTarget::Outside)] {
            u.begin(at((10, 10), true, true, false), 5, (768, 432));
            frame(&mut u);
            u.begin(at(p, true, false, false), 6, (768, 432));
            frame(&mut u);
            u.begin(at(p, false, false, true), 7, (768, 432));
            assert_eq!(u.take_drop(), Some((DragPayload::Bag(0), want)));
        }
    }

    #[test]
    fn a_press_without_a_move_is_a_click_not_a_drop() {
        let mut u = ui();
        let a = Rect::new(0, 0, 36, 36);
        let v = SlotView::default();
        u.begin(at((10, 10), true, true, false), 1, (768, 432));
        u.slot(wid("s", 0), a, &v, Some(DragPayload::Bag(0)), DropTarget::Bag(0), false);
        u.begin(at((10, 10), false, false, true), 2, (768, 432));
        let out = u.slot(wid("s", 0), a, &v, Some(DragPayload::Bag(0)), DropTarget::Bag(0), false);
        assert!(out.clicked);
        assert_eq!(u.take_drop(), None);
    }

    #[test]
    fn a_tooltip_waits_twenty_ticks_under_the_pointer() {
        let mut u = ui();
        let r = Rect::new(0, 0, 36, 36);
        let mut frame = Frame::new(crate::Tier::T0);
        for t in 1..=25 {
            u.begin(at((5, 5), false, false, false), t, (768, 432));
            u.tip(wid("t", 0), r, |s| s.push_str("Brass key\nIt fits a lock somewhere."));
            u.finish(&mut frame);
            let tip_drawn = frame.ui.iter().any(|c| matches!(c, UiCmd::Fill { dst, .. } if dst.x > 5));
            assert_eq!(tip_drawn, t > 20, "tick {t}");
        }
    }

    #[test]
    fn a_text_field_takes_typing_up_to_its_limit() {
        let mut u = ui();
        let mut buf = String::from("Jan");
        let mut keys = KeySet::EMPTY;
        keys.set(sc::BACKSPACE, true);
        u.begin(UiInput { typed: "e".into(), ..UiInput::default() }, 1, (768, 432));
        let o = u.text_field(Rect::new(0, 0, 200, 24), &mut buf, 4, Ink::small(style::text()));
        assert!(o.changed && buf == "Jane");
        u.begin(UiInput { typed: "tte".into(), ..UiInput::default() }, 2, (768, 432));
        u.text_field(Rect::new(0, 0, 200, 24), &mut buf, 4, Ink::small(style::text()));
        assert_eq!(buf, "Jane", "four at most");
        u.begin(UiInput { keys, ..UiInput::default() }, 3, (768, 432));
        u.text_field(Rect::new(0, 0, 200, 24), &mut buf, 4, Ink::small(style::text()));
        assert_eq!(buf, "Jan");
    }

    #[test]
    fn widget_ids_differ_by_name_and_index() {
        assert_ne!(wid("bag", 0), wid("bag", 1));
        assert_ne!(wid("bag", 0), wid("bar", 0));
        let mut b = [0u8; 6];
        assert_eq!(fmt_u32(0, &mut b), "0");
        assert_eq!(fmt_u32(40213, &mut b), "40213");
    }
}
