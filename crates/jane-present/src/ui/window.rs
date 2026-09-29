//! The window (PRESENTATION.md §3.2): one tabbed panel, Bag (the 8 x 3 bag, the craft strip at a
//! bench, her stats), Book (the spells the party has learned), Log (what she has been asked to do)
//! and Map (§3.5). Drag and drop moves things (§3.4): bag to bag, bag or book to the bar, bag to
//! the craft strip, the bar off itself; dropping a thing off every panel asks once and destroys it.
//! A right click (or a second press on a pad) opens the slot's popover. The keys move a focus ring
//! over the bag, pick up with confirm and put down with confirm.
//!
//! At a cupboard (USE on one, `jane_sim::store`) the Bag tab is two panels, her bag and the
//! cupboard, 6 x 4 each: drag either way; shift-click or a right click moves a thing across to
//! wherever it fits; the ring walks across both, confirm carries and puts down, X (or 2) moves the
//! lit thing across and Y (or 3) puts the whole bag away. Every move is a sim command.

use std::fmt::Write as _;

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_core::SpellId;
use jane_data::SpellKind;
use jane_sim::View;
use jane_sim::input::{BarSlotWire, Command};
use jane_sim::tuning::{BAG_SLOTS, BAR_SLOTS, STORE_SLOTS};

use crate::input::UiAction;
use crate::text;
use crate::ui::cmd::Rect;
use crate::ui::core::{
    ButtonKind, DragPayload, DropTarget, Ink, PanelStyle, SlotView, Ui, advance, line_h, wid, wrap_lines,
};
use crate::ui::hud::{self, HudCtx};
use crate::ui::map::{self, MapChart};
use crate::ui::menus::{self, MenuState};
use crate::ui::style::{self, argb};
use crate::view::{SlotData, StoreView, ViewBuffers};

/// The tabs, in order.
pub const TABS: [&str; 4] = ["Bag", "Book", "Log", "Map"];
/// The window's size (§3.2 says 640 x 380; it is shorter here so the bar below stays a drop
/// target while it is open).
pub const W: i32 = 640;
pub const H: i32 = 350;
const SLOT: i32 = 36;
const GAP: i32 = 4;

/// The window's own state between frames.
#[derive(Clone, Debug, Default)]
pub struct WindowState {
    pub tab: usize,
    /// The bag slot the keys have lit, and one picked up to be put down. At a cupboard, 0..24 are
    /// the bag's and 24..48 the cupboard's.
    pub focus: u8,
    pub carried: Option<u8>,
    /// The book's and the log's lit rows.
    pub book: usize,
    pub log: usize,
    pub map: MapChart,
    /// A bag slot waiting on "destroy it?".
    pub destroy: Option<u8>,
    confirm: MenuState,
    /// The bag slot whose popover is open.
    popover: Option<u8>,
}

impl WindowState {
    pub fn on(tab: usize) -> WindowState {
        WindowState { tab, ..WindowState::default() }
    }
}

fn slot_view(s: &SlotData) -> SlotView {
    SlotView {
        icon: s.icon,
        count: s.count,
        item: s.item,
        spell: s.spell,
        cooldown: s.cooldown,
        gcd: s.gcd,
        usable: true,
        label: None,
        flash: 0,
        flash_bad: false,
    }
}

/// The window's rect on this canvas.
pub fn rect(canvas: (i32, i32)) -> Rect {
    Rect::new((canvas.0 - W) / 2, 14, W, H)
}

/// Draws the window and answers it. `v` is the view (the map reads it); `None` in a test.
pub fn draw(ui: &mut Ui, st: &mut WindowState, b: &ViewBuffers, v: Option<&View<'_>>, cx: HudCtx<'_>) {
    let r = rect(ui.canvas);
    let (x, y) = (i32::from(r.x), i32::from(r.y));
    ui.panel(r, PanelStyle::Window);
    ui.drop_area(r, DropTarget::Window);
    // The tabs.
    let live = ui.interactive && st.destroy.is_none() && !ui.popover_open();
    for (i, t) in TABS.iter().enumerate() {
        let tr = Rect::new(x + 14 + i as i32 * 92, y + 10, 88, 24);
        if ui.button(wid("tab", i as u32), tr, t, ButtonKind::Tab { on: st.tab == i }, true, false) {
            st.tab = i;
        }
    }
    if live {
        for a in &ui.input.actions {
            match a {
                UiAction::TabLeft => st.tab = (st.tab + TABS.len() - 1) % TABS.len(),
                UiAction::TabRight => st.tab = (st.tab + 1) % TABS.len(),
                _ => {}
            }
        }
        if ui.input.keys.has(crate::input::sc::TAB) && ui.input.pad {
            st.tab = (st.tab + 1) % TABS.len();
        }
    }
    ui.fill(Rect::new(x + 10, y + 34, W - 20, 1), argb(style::gold_deep(), 160));
    let esc = if cx.pad { "B closes" } else { "Esc closes" };
    ui.text_right(x + W - 16, y + 16, esc, Ink::fine(style::quiet()).shadow());
    let body = Rect::new(x + 12, y + 42, W - 24, H - 54);
    match st.tab {
        0 => match &b.window.store {
            Some(s) => store_tab(ui, st, b, s, body, cx, live),
            None => {
                if usize::from(st.focus) >= BAG_SLOTS {
                    st.focus = 0;
                }
                if st.carried.is_some_and(|c| usize::from(c) >= BAG_SLOTS) {
                    st.carried = None;
                }
                bag_tab(ui, st, b, body, cx, live);
            }
        },
        1 => book_tab(ui, st, b, body, live),
        2 => log_tab(ui, st, b, body, live),
        _ => match v {
            Some(v) => map::draw(
                ui,
                &mut st.map,
                v,
                Rect::new(i32::from(body.x), i32::from(body.y), i32::from(body.w), i32::from(body.h) - 16),
            ),
            None => {
                ui.text_in(body, "No map here", Ink::small(style::quiet()));
            }
        },
    }
    drops(ui, st, b);
    if let Some(slot) = st.destroy {
        let name = b
            .window
            .bag
            .get(usize::from(slot))
            .and_then(|s| s.item)
            .map_or("that", |i| text::text(jane_data::catalog().combat.item(i).name));
        let q = format!("Destroy the {}?", name.to_lowercase());
        ui.interactive = true;
        if let Some(yes) = menus::confirm(ui, &mut st.confirm, &q) {
            if yes {
                ui.command(Command::BagDestroy { slot });
            }
            st.destroy = None;
        }
    }
}

/// A drag that ended this frame, as the one command it means (§3.4).
fn drops(ui: &mut Ui, st: &mut WindowState, b: &ViewBuffers) {
    let Some((payload, target)) = ui.take_drop() else { return };
    let cmd = match (payload, target) {
        (DragPayload::Bag(from), DropTarget::Bag(to)) => Some(Command::BagMove { from, to }),
        (DragPayload::Bag(bag), DropTarget::Craft(slot)) => Some(Command::CraftPut { bag, slot }),
        (DragPayload::Bag(bag), DropTarget::Bar(slot)) => b
            .window
            .bag
            .get(usize::from(bag))
            .and_then(|s| s.item)
            .map(|i| Command::Bind { slot, to: BarSlotWire::Item(i) }),
        (DragPayload::Bag(bag), DropTarget::Outside) => {
            st.destroy = Some(bag);
            st.confirm = MenuState { focus: 1 };
            None
        }
        (DragPayload::Spell(s), DropTarget::Bar(slot)) => Some(Command::Bind { slot, to: BarSlotWire::Spell(s) }),
        (DragPayload::Bar(a), DropTarget::Bar(bb)) => Some(Command::BarSwap { a, b: bb }),
        (DragPayload::Bar(slot), DropTarget::Outside | DropTarget::Window | DropTarget::BarBackground) => {
            Some(Command::Unbind { slot })
        }
        (DragPayload::Craft(slot), DropTarget::Bag(_) | DropTarget::Window | DropTarget::Outside) => {
            Some(Command::CraftClear { slot })
        }
        // At a cupboard: either way, onto a slot or anywhere on the other panel.
        (DragPayload::Bag(bag), DropTarget::Store(to)) => {
            b.window.store.as_ref().map(|s| Command::StorePut { prop: s.prop, bag, to: Some(to) })
        }
        (DragPayload::Bag(bag), DropTarget::StorePanel) => {
            b.window.store.as_ref().map(|s| Command::StorePut { prop: s.prop, bag, to: None })
        }
        (DragPayload::Store(slot), DropTarget::Bag(to)) => {
            b.window.store.as_ref().map(|s| Command::StoreTake { prop: s.prop, slot, to: Some(to) })
        }
        (DragPayload::Store(slot), DropTarget::BagPanel) => {
            b.window.store.as_ref().map(|s| Command::StoreTake { prop: s.prop, slot, to: None })
        }
        (DragPayload::Store(from), DropTarget::Store(to)) => {
            b.window.store.as_ref().map(|s| Command::StoreMove { prop: s.prop, from, to })
        }
        _ => None,
    };
    if let Some(c) = cmd {
        ui.command(c);
    }
}

fn bag_tab(ui: &mut Ui, st: &mut WindowState, b: &ViewBuffers, body: Rect, cx: HudCtx<'_>, live: bool) {
    let (x0, y0) = (i32::from(body.x) + 8, i32::from(body.y) + 4);
    ui.text(x0, y0, "Bag", Ink::small(style::gold()).shadow());
    let gy = y0 + 22;
    // Keys move the ring over the grid; confirm picks up and puts down.
    if live {
        for a in ui.input.actions.clone() {
            let f = i32::from(st.focus);
            let f = match a {
                UiAction::Left => (f + BAG_SLOTS as i32 - 1) % BAG_SLOTS as i32,
                UiAction::Right => (f + 1) % BAG_SLOTS as i32,
                UiAction::Up => (f + BAG_SLOTS as i32 - 8) % BAG_SLOTS as i32,
                UiAction::Down => (f + 8) % BAG_SLOTS as i32,
                UiAction::Confirm => {
                    match st.carried.take() {
                        Some(from) if from != st.focus => ui.command(Command::BagMove { from, to: st.focus }),
                        None if b.window.bag[usize::from(st.focus)].item.is_some() => st.carried = Some(st.focus),
                        Some(_) | None => {}
                    }
                    f
                }
                _ => f,
            };
            st.focus = f as u8;
        }
    }
    for i in 0..BAG_SLOTS {
        let (col, row) = (i as i32 % 8, i as i32 / 8);
        let sr = Rect::new(x0 + col * (SLOT + GAP), gy + row * (SLOT + GAP), SLOT, SLOT);
        let s = &b.window.bag[i];
        let mut v = slot_view(s);
        v.usable = st.carried != Some(i as u8);
        let payload = s.item.is_some().then_some(DragPayload::Bag(i as u8));
        let out = ui.slot(wid("bag", i as u32), sr, &v, payload, DropTarget::Bag(i as u8), st.focus == i as u8);
        if out.hovered {
            st.focus = i as u8;
        }
        if let Some(item) = s.item {
            ui.tip(wid("bag-tip", i as u32), sr, |t| hud::slot_tip(t, Some(item), None));
            if out.right_clicked {
                st.popover = Some(i as u8);
                if let Some(p) = ui.input.pointer {
                    ui.open_popover(wid("bag-pop", 0), p);
                }
            }
            if out.clicked && ui.input.pad {
                st.popover = Some(i as u8);
                ui.open_popover(wid("bag-pop", 0), (sr.right(), i32::from(sr.y)));
            }
        }
    }
    // The popover on a bag slot: use, put on the bar, destroy.
    if let Some(slot) = st.popover {
        let item = b.window.bag.get(usize::from(slot)).and_then(|s| s.item);
        let usable = b.window.bag.get(usize::from(slot)).is_some_and(|s| s.usable);
        let labels: &[&str] =
            if usable { &["Use", "Put on the bar", "Destroy"] } else { &["Put on the bar", "Destroy"] };
        match ui.popover(wid("bag-pop", 0), labels) {
            Some(k) => {
                let what = labels[k];
                match (what, item) {
                    ("Use", Some(i)) => ui.command(Command::Item(i)),
                    ("Put on the bar", Some(i)) => {
                        let empty = b.hud.bar.iter().position(|s| s.icon.is_none()).unwrap_or(BAR_SLOTS - 1);
                        ui.command(Command::Bind { slot: empty as u8, to: BarSlotWire::Item(i) });
                    }
                    ("Destroy", Some(_)) => {
                        st.destroy = Some(slot);
                        st.confirm = MenuState { focus: 1 };
                    }
                    _ => {}
                }
                st.popover = None;
            }
            None if !ui.popover_open() => st.popover = None,
            None => {}
        }
    }

    // The craft strip, at a bench.
    let cy = gy + 3 * (SLOT + GAP) + 14;
    ui.text(x0, cy, "Bench", Ink::small(if b.window.at_bench { style::gold() } else { style::dim() }).shadow());
    if b.window.at_bench {
        let sy = cy + 22;
        for k in 0..3 {
            let sr = Rect::new(x0 + k * (SLOT + GAP), sy, SLOT, SLOT);
            let s = &b.window.craft[k as usize];
            let payload = s.item.is_some().then_some(DragPayload::Craft(k as u8));
            let out = ui.slot(wid("craft", k as u32), sr, &slot_view(s), payload, DropTarget::Craft(k as u8), false);
            if out.clicked && s.item.is_some() {
                ui.command(Command::CraftClear { slot: k as u8 });
            }
        }
        let ax = x0 + 3 * (SLOT + GAP) + 6;
        ui.text(ax, sy + 9, "→", Ink::small(style::gold()).shadow());
        let or = Rect::new(ax + 26, sy, SLOT, SLOT);
        let out_view = b.window.craft_out.as_ref().map(slot_view).unwrap_or_default();
        let out = ui.slot(wid("craft-out", 0), or, &out_view, None, DropTarget::Window, false);
        if let Some(o) = &b.window.craft_out {
            let item = o.item;
            ui.tip(wid("craft-tip", 0), or, |t| hud::slot_tip(t, item, None));
        }
        let make = Rect::new(ax + 26 + SLOT + 10, sy + 6, 70, 24);
        let can = b.window.craft_out.is_some();
        if ui.button(wid("craft-make", 0), make, "Make", ButtonKind::Chip, can, false) || (out.clicked && can) {
            ui.command(Command::CraftTake);
        }
    } else {
        let s = "Crafting wants a bench within reach";
        ui.text(x0, cy + 24, s, Ink::fine(style::quiet()).shadow());
    }

    // Her stats, on a card to the right.
    let card = Rect::new(x0 + 8 * (SLOT + GAP) + 16, gy - 22, i32::from(body.w) - 8 * (SLOT + GAP) - 32, 190);
    ui.well(card, false);
    let (kx, ky) = (i32::from(card.x) + 12, i32::from(card.y) + 10);
    ui.text(kx, ky, &b.heroine, Ink::small(style::text_bright()).shadow());
    let s = &b.window.stats;
    let rows: [(&str, String); 6] = [
        ("Strength", s.strength.to_string()),
        ("Spirit", s.spirit.to_string()),
        ("Health", format!("{} of {}", b.hud.hp.now.max(0), s.hp_max)),
        ("Mana", format!("{} of {}", b.hud.mp.now.max(0), s.mp_max)),
        ("Day", b.hud.day.to_string()),
        ("Fallen", s.deaths.to_string()),
    ];
    for (i, (k, val)) in rows.iter().enumerate() {
        let yy = ky + 28 + i as i32 * 18;
        ui.text(kx, yy, k, Ink::fine(style::quiet()).shadow());
        ui.text_right(card.right() - 12, yy, val, Ink::fine(style::text()).shadow());
        ui.fill(Rect::new(kx, yy + 13, i32::from(card.w) - 24, 1), argb(style::INK, 60));
    }
    let tally = format!("{} put down, {} cast", s.kills, s.casts);
    ui.text(kx, ky + 28 + 6 * 18 + 4, &tally, Ink::fine(style::dim()).shadow());

    // Hints at the foot.
    let hint = if cx.pad {
        "A picks up and puts down · LB RB tabs"
    } else {
        "Drag to move or to the bar · right click for more · drop outside to destroy"
    };
    ui.text(x0, body.bottom() - 12, hint, Ink::fine(style::quiet()).shadow());
}

/// Columns of each panel at a cupboard.
const STORE_COLS: i32 = 6;

/// The command that moves cell `from` onto cell `to` at a cupboard (cells: 0..24 the bag, 24..48
/// the cupboard; `to: None` is across, wherever it fits).
fn store_move(prop: jane_sim::ids::PropId, from: u8, to: Option<u8>) -> Option<Command> {
    let bag = BAG_SLOTS as u8;
    Some(match (from < bag, to) {
        (true, None) => Command::StorePut { prop, bag: from, to: None },
        (false, None) => Command::StoreTake { prop, slot: from - bag, to: None },
        (true, Some(t)) if t < bag => Command::BagMove { from, to: t },
        (true, Some(t)) => Command::StorePut { prop, bag: from, to: Some(t - bag) },
        (false, Some(t)) if t < bag => Command::StoreTake { prop, slot: from - bag, to: Some(t) },
        (false, Some(t)) => Command::StoreMove { prop, from: from - bag, to: t - bag },
    })
    .filter(|c| !matches!(*c, Command::BagMove { from, to } | Command::StoreMove { from, to, .. } if from == to))
}

/// The ring one step along `a` over the two panels, side by side.
fn store_step(focus: u8, a: UiAction) -> u8 {
    let (n, cols) = (BAG_SLOTS as i32, STORE_COLS);
    let rows = n / cols;
    let f = i32::from(focus);
    let (side, i) = (f / n, f % n);
    let (col, row) = (i % cols + side * cols, i / cols);
    let (col, row) = match a {
        UiAction::Left => ((col + 2 * cols - 1) % (2 * cols), row),
        UiAction::Right => ((col + 1) % (2 * cols), row),
        UiAction::Up => (col, (row + rows - 1) % rows),
        UiAction::Down => (col, (row + 1) % rows),
        _ => (col, row),
    };
    ((col / cols) * n + row * cols + col % cols) as u8
}

/// The Bag tab at a cupboard: her bag on the left, the cupboard on the right.
fn store_tab(
    ui: &mut Ui,
    st: &mut WindowState,
    b: &ViewBuffers,
    s: &StoreView,
    body: Rect,
    cx: HudCtx<'_>,
    live: bool,
) {
    let cell = SLOT + GAP;
    let panel_w = STORE_COLS * cell - GAP;
    let rows = BAG_SLOTS as i32 / STORE_COLS;
    let between = 84;
    let lx = i32::from(body.x) + (i32::from(body.w) - 2 * panel_w - between) / 2;
    let rx = lx + panel_w + between;
    let y0 = i32::from(body.y) + 6;
    let gy = y0 + 26;
    let total = (BAG_SLOTS + STORE_SLOTS) as u8;
    if usize::from(st.focus) >= usize::from(total) {
        st.focus = 0;
    }
    let item_at = |c: u8| -> Option<jane_core::ItemId> {
        let c = usize::from(c);
        if c < BAG_SLOTS {
            b.window.bag.get(c).and_then(|d| d.item)
        } else {
            s.slots.get(c - BAG_SLOTS).and_then(|d| d.item)
        }
    };

    // The keys and the pad: the ring walks both panels; confirm carries and puts down.
    if live {
        for a in ui.input.actions.clone() {
            match a {
                UiAction::Left | UiAction::Right | UiAction::Up | UiAction::Down => st.focus = store_step(st.focus, a),
                UiAction::Confirm => match st.carried.take() {
                    Some(from) => {
                        if let Some(c) = store_move(s.prop, from, Some(st.focus)) {
                            ui.command(c);
                        }
                    }
                    None if item_at(st.focus).is_some() => st.carried = Some(st.focus),
                    None => {}
                },
                UiAction::Quick => {
                    st.carried = None;
                    if item_at(st.focus).is_some() {
                        if let Some(c) = store_move(s.prop, st.focus, None) {
                            ui.command(c);
                        }
                    }
                }
                UiAction::QuickAll => {
                    st.carried = None;
                    ui.command(Command::StorePutAll { prop: s.prop });
                }
                _ => {}
            }
        }
    }

    // The two panels' headings, and a well under each grid.
    let grid_h = rows * cell - GAP;
    ui.text(lx, y0, "Bag", Ink::small(style::gold()).shadow());
    let held = b.window.bag.iter().filter(|d| d.item.is_some()).count();
    ui.text_right(lx + panel_w, y0 + 3, &format!("{held} of {BAG_SLOTS}"), Ink::fine(style::quiet()).shadow());
    ui.text(rx, y0, &s.name, Ink::small(style::gold()).shadow());
    ui.text_right(rx + panel_w, y0 + 3, &format!("{} of {STORE_SLOTS}", s.used), Ink::fine(style::quiet()).shadow());
    let lwell = Rect::new(lx - 6, gy - 6, panel_w + 12, grid_h + 12);
    let rwell = Rect::new(rx - 6, gy - 6, panel_w + 12, grid_h + 12);
    ui.well(lwell, false);
    ui.well(rwell, false);
    ui.drop_area(lwell, DropTarget::BagPanel);
    ui.drop_area(rwell, DropTarget::StorePanel);
    // Between them, the way things go.
    let mid = lx + panel_w + between / 2;
    ui.text(mid - 6, gy + grid_h / 2 - 16, "→", Ink::small(style::gold_deep()).shadow());
    ui.text(mid - 6, gy + grid_h / 2 + 2, "←", Ink::small(style::gold_deep()).shadow());

    for c in 0..total {
        let store = usize::from(c) >= BAG_SLOTS;
        let i = i32::from(c) % BAG_SLOTS as i32;
        let x = if store { rx } else { lx };
        let sr = Rect::new(x + i % STORE_COLS * cell, gy + i / STORE_COLS * cell, SLOT, SLOT);
        let d = if store { &s.slots[i as usize] } else { &b.window.bag[i as usize] };
        let mut v = slot_view(d);
        v.usable = st.carried != Some(c);
        let (payload, target) = if store {
            (d.item.is_some().then_some(DragPayload::Store(i as u8)), DropTarget::Store(i as u8))
        } else {
            (d.item.is_some().then_some(DragPayload::Bag(i as u8)), DropTarget::Bag(i as u8))
        };
        let out = ui.slot(wid("store-cell", u32::from(c)), sr, &v, payload, target, st.focus == c);
        if out.hovered {
            st.focus = c;
        }
        if let Some(item) = d.item {
            ui.tip(wid("store-tip", u32::from(c)), sr, |t| hud::slot_tip(t, Some(item), None));
            // Shift-click, or a right click: across, wherever it fits.
            if (out.clicked && ui.input.shift) || out.right_clicked {
                if let Some(cmd) = store_move(s.prop, c, None) {
                    ui.command(cmd);
                }
            }
        }
    }

    // Everything in the bag, away.
    let by = gy + grid_h + 14;
    let all = Rect::new(lx + panel_w - 120, by, 120, 24);
    if ui.button(wid("store-all", 0), all, "Put all away", ButtonKind::Chip, held > 0, false) {
        ui.command(Command::StorePutAll { prop: s.prop });
    }
    let note = "Kept here for the whole party";
    ui.text_right(rx + panel_w, by + 6, note, Ink::fine(style::dim()).shadow());

    let hint = if cx.pad {
        "A picks up and puts down · X moves it across · Y puts all away · B closes"
    } else {
        "Drag between them · shift-click or right click moves a thing across"
    };
    ui.text(lx, body.bottom() - 12, hint, Ink::fine(style::quiet()).shadow());
}

fn spell_line(id: SpellId) -> String {
    let d = jane_data::catalog().combat.spell(id);
    let mut s = String::new();
    if d.mp.points() > 0 {
        let _ = write!(s, "{} mana", d.mp.points());
    }
    if d.energy.points() > 0 {
        if !s.is_empty() {
            s.push_str(", ");
        }
        let _ = write!(s, "{} energy", d.energy.points());
    }
    if s.is_empty() {
        s.push_str("Free");
    }
    let m = d.range.0 / jane_core::num::CELL_FX;
    let reach = match d.kind {
        SpellKind::OnSelf => "on herself".to_string(),
        SpellKind::Ally => format!("a friend within {m} m"),
        SpellKind::Melee => "at arm's length".to_string(),
        _ => format!("{m} m"),
    };
    s.push_str(" · ");
    s.push_str(&reach);
    s
}

fn book_tab(ui: &mut Ui, st: &mut WindowState, b: &ViewBuffers, body: Rect, live: bool) {
    let (x0, y0) = (i32::from(body.x) + 8, i32::from(body.y) + 4);
    let book = &b.window.book;
    if book.is_empty() {
        ui.text(x0, y0, "No spells yet", Ink::small(style::quiet()).shadow());
        return;
    }
    if live {
        for a in &ui.input.actions {
            match a {
                UiAction::Up => st.book = (st.book + book.len() - 1) % book.len(),
                UiAction::Down => st.book = (st.book + 1) % book.len(),
                _ => {}
            }
        }
    }
    st.book = st.book.min(book.len() - 1);
    let row_h = 42;
    let lw = 300;
    for (i, row) in book.iter().enumerate().take(((i32::from(body.h) - 30) / row_h) as usize) {
        let d = jane_data::catalog().combat.spell(row.id);
        let rr = Rect::new(x0, y0 + i as i32 * row_h, lw, row_h - 4);
        let lit = st.book == i;
        if ui.hover(rr) && ui.input.pointer.is_some() {
            st.book = i;
        }
        if lit {
            ui.fill(rr, argb(Ramp::UiPanel.at(Tone::Light), 55));
            ui.fill(Rect::new(i32::from(rr.x), i32::from(rr.y), 2, row_h - 4), argb(style::gold(), 220));
        }
        let sr = Rect::new(x0 + 4, i32::from(rr.y) + 1, SLOT, SLOT);
        let v = SlotView { icon: Some(d.icon), usable: true, spell: Some(row.id), ..SlotView::default() };
        ui.slot(wid("book", i as u32), sr, &v, Some(DragPayload::Spell(row.id)), DropTarget::Window, false);
        ui.text(
            x0 + 48,
            i32::from(rr.y) + 3,
            text::text(d.name),
            Ink::small(if lit { style::text_bright() } else { style::text() }).shadow(),
        );
        ui.text(x0 + 48, i32::from(rr.y) + 22, &spell_line(row.id), Ink::fine(style::quiet()).shadow());
        if let Some(k) = row.bound {
            let label = format!("{}", k + 1);
            let br = Rect::new(rr.right() - 22, i32::from(rr.y) + 10, 16, 16);
            ui.fill(br, argb(style::gold_deep(), 255));
            ui.text(i32::from(br.x) + 4, i32::from(br.y) + 2, &label, Ink::fine(style::INK));
        }
        if lit && live && ui.input.has(UiAction::Confirm) && row.bound.is_none() {
            let empty = b.hud.bar.iter().position(|s| s.icon.is_none()).unwrap_or(BAR_SLOTS - 1);
            ui.command(Command::Bind { slot: empty as u8, to: BarSlotWire::Spell(row.id) });
        }
    }
    // The lit spell, whole.
    let row = &book[st.book];
    let d = jane_data::catalog().combat.spell(row.id);
    let dx = x0 + lw + 20;
    let dw = body.right() - dx - 12;
    ui.text(dx, y0, text::text(d.name), Ink::small(style::gold()).shadow());
    ui.rule(dx, dx + dw, y0 + 20, style::gold_deep());
    let cols = (dw / advance(Face::Fine)) as usize;
    let mut yy = y0 + 28;
    for l in wrap_lines(text::text(d.description), cols) {
        ui.text(dx, yy, l, Ink::fine(style::text()).shadow());
        yy += line_h(Face::Fine);
    }
    yy += 8;
    ui.text(dx, yy, &spell_line(row.id), Ink::fine(style::quiet()).shadow());
    if d.cooldown.0 > 0 {
        let mut c = String::from("Again after ");
        text::span(d.cooldown.0, &mut c);
        ui.text(dx, yy + 14, &c, Ink::fine(style::quiet()).shadow());
    }
    let hint = "Drag a spell to the bar to use it";
    ui.text(x0, body.bottom() - 12, hint, Ink::fine(style::quiet()).shadow());
}

fn log_tab(ui: &mut Ui, st: &mut WindowState, b: &ViewBuffers, body: Rect, live: bool) {
    let (x0, y0) = (i32::from(body.x) + 8, i32::from(body.y) + 4);
    let qs = &b.window.quests;
    if qs.is_empty() {
        ui.text(x0, y0, "Nothing asked of her yet", Ink::small(style::quiet()).shadow());
        return;
    }
    if live {
        for a in &ui.input.actions {
            match a {
                UiAction::Up => st.log = (st.log + qs.len() - 1) % qs.len(),
                UiAction::Down => st.log = (st.log + 1) % qs.len(),
                _ => {}
            }
        }
    }
    st.log = st.log.min(qs.len() - 1);
    let lw = 230;
    let row_h = 22;
    for (i, q) in qs.iter().enumerate().take(((i32::from(body.h) - 8) / row_h) as usize) {
        let rr = Rect::new(x0, y0 + i as i32 * row_h, lw, row_h - 2);
        if ui.hover(rr) && ui.input.pointer.is_some() {
            st.log = i;
        }
        let lit = st.log == i;
        if lit {
            ui.fill(rr, argb(Ramp::UiPanel.at(Tone::Light), 55));
            ui.fill(Rect::new(i32::from(rr.x), i32::from(rr.y), 2, row_h - 2), argb(style::gold(), 220));
        }
        let (mark, ink) = if q.done {
            ("✓", style::dim())
        } else if q.ready {
            ("◆", style::gold())
        } else {
            ("•", style::text())
        };
        ui.text(x0 + 6, i32::from(rr.y) + 4, mark, Ink::fine(ink).shadow());
        let title: String = q.title.chars().take(((lw - 24) / advance(Face::Fine)) as usize).collect();
        ui.text(
            x0 + 20,
            i32::from(rr.y) + 4,
            &title,
            Ink::fine(if q.done { style::dim() } else { style::text_bright() }).shadow(),
        );
        ui.claim(rr);
    }
    let q = &qs[st.log];
    let dx = x0 + lw + 18;
    let dw = body.right() - dx - 12;
    ui.fill(Rect::new(dx - 10, y0, 1, i32::from(body.h) - 16), argb(style::gold_deep(), 90));
    let cols_small = (dw / advance(Face::Small)) as usize;
    let mut yy = y0;
    for l in wrap_lines(&q.title, cols_small) {
        ui.text(dx, yy, l, Ink::small(style::gold()).shadow());
        yy += line_h(Face::Small);
    }
    yy += 6;
    let cols = (dw / advance(Face::Fine)) as usize;
    for l in wrap_lines(&q.body, cols) {
        ui.text(dx, yy, l, Ink::fine(style::text()).shadow());
        yy += line_h(Face::Fine);
    }
    yy += 8;
    for (s, done) in &q.steps {
        let (mark, ink) = if *done { ("✓", style::good()) } else { ("•", style::gold()) };
        ui.text(dx, yy, mark, Ink::fine(ink).shadow());
        for (k, l) in wrap_lines(s, cols - 3).enumerate() {
            ui.text(
                dx + 16,
                yy + k as i32 * line_h(Face::Fine),
                l,
                Ink::fine(if *done { style::quiet() } else { style::text_bright() }).shadow(),
            );
        }
        yy += wrap_lines(s, cols - 3).count() as i32 * line_h(Face::Fine) + 4;
    }
    if q.done {
        ui.text(dx, yy + 4, "Done", Ink::fine(style::good()).shadow());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Bindings;
    use crate::ui::core::UiInput;
    use crate::ui::{UiArt, UiOut};

    fn bufs() -> ViewBuffers {
        let sim = jane_sim::Sim::new_game(7, "Tess");
        let v = sim.view(jane_sim::Seat(0)).unwrap();
        let mut b = ViewBuffers::new();
        b.tick(&v, &[]);
        b
    }

    #[test]
    fn keys_carry_a_bag_thing_and_put_it_down_as_one_move() {
        let b = bufs();
        let from = b.window.bag.iter().position(|s| s.item.is_some()).expect("the start kit") as u8;
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = WindowState { focus: from, ..WindowState::default() };
        let bind = Bindings::default();
        let cx = HudCtx { bindings: &bind, pad: true, window_open: true };
        let press = |ui: &mut Ui, st: &mut WindowState, a: UiAction, t: u32| {
            ui.begin(UiInput { actions: vec![a], pad: true, ..UiInput::default() }, t, (768, 432));
            draw(ui, st, &b, None, cx);
            ui.out.clone()
        };
        assert!(press(&mut ui, &mut st, UiAction::Confirm, 1).is_empty());
        assert_eq!(st.carried, Some(from));
        press(&mut ui, &mut st, UiAction::Down, 2);
        let out = press(&mut ui, &mut st, UiAction::Confirm, 3);
        assert_eq!(out, vec![UiOut::Command(Command::BagMove { from, to: from + 8 })]);
        assert_eq!(st.carried, None);
    }

    #[test]
    fn at_a_cupboard_the_ring_walks_both_panels_and_every_move_is_a_command() {
        let mut b = bufs();
        let prop = jane_sim::ids::PropId(std::num::NonZeroU32::new(77).unwrap());
        let slots = vec![SlotData::default(); STORE_SLOTS];
        b.window.store = Some(StoreView { prop, name: "Dresser".into(), slots, used: 0 });
        let from = b.window.bag.iter().position(|s| s.item.is_some()).expect("the start kit") as u8;
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = WindowState { focus: from, ..WindowState::default() };
        let bind = Bindings::default();
        let cx = HudCtx { bindings: &bind, pad: true, window_open: true };
        let press = |ui: &mut Ui, st: &mut WindowState, a: UiAction, t: u32| {
            ui.begin(UiInput { actions: vec![a], pad: true, ..UiInput::default() }, t, (768, 432));
            draw(ui, st, &b, None, cx);
            ui.out.clone()
        };
        // X: across, wherever it fits.
        let out = press(&mut ui, &mut st, UiAction::Quick, 1);
        assert_eq!(out, vec![UiOut::Command(Command::StorePut { prop, bag: from, to: None })]);
        // Carried with A, walked right across the gap into the cupboard, put down with A.
        press(&mut ui, &mut st, UiAction::Confirm, 2);
        assert_eq!(st.carried, Some(from));
        let col = i32::from(from) % STORE_COLS;
        for t in 0..STORE_COLS - col {
            press(&mut ui, &mut st, UiAction::Right, 3 + t as u32);
        }
        assert!(usize::from(st.focus) >= BAG_SLOTS, "into the cupboard: {}", st.focus);
        let to = st.focus - BAG_SLOTS as u8;
        let out = press(&mut ui, &mut st, UiAction::Confirm, 20);
        assert_eq!(out, vec![UiOut::Command(Command::StorePut { prop, bag: from, to: Some(to) })]);
        // Y: the whole bag away.
        let out = press(&mut ui, &mut st, UiAction::QuickAll, 21);
        assert_eq!(out, vec![UiOut::Command(Command::StorePutAll { prop })]);
        // And back: a cupboard slot to the bag.
        assert_eq!(store_move(prop, 30, Some(2)), Some(Command::StoreTake { prop, slot: 6, to: Some(2) }));
        assert_eq!(store_move(prop, 30, None), Some(Command::StoreTake { prop, slot: 6, to: None }));
        assert_eq!(store_move(prop, 30, Some(31)), Some(Command::StoreMove { prop, from: 6, to: 7 }));
        assert_eq!(store_move(prop, 30, Some(30)), None);
        // Left from the cupboard's first column is the bag's last.
        assert_eq!(store_step(24, UiAction::Left), 5);
        assert_eq!(store_step(0, UiAction::Left), 24 + 5);
    }

    #[test]
    fn tabs_cycle_on_the_shoulders() {
        let b = bufs();
        let mut ui = Ui::new(UiArt::build(1).0);
        let mut st = WindowState::default();
        let bind = Bindings::default();
        let cx = HudCtx { bindings: &bind, pad: true, window_open: true };
        ui.begin(UiInput { actions: vec![UiAction::TabLeft], ..UiInput::default() }, 1, (768, 432));
        draw(&mut ui, &mut st, &b, None, cx);
        assert_eq!(st.tab, 3);
    }
}
