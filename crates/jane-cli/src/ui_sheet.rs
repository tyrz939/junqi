//! `jane sheet ui`: the UI's screens drawn headless through the presenter and `soft`, one PNG
//! each, in states a player reaches only by playing (the death veil, a fight's target frame and
//! chips, three toasts at once, a tooltip, a popover, a drag in flight, a choice). The review
//! tool for the UI as `jane sheet scene` is for the world (PRESENTATION.md §3, §6).

use std::path::Path;

use jane_present::input::Bindings;
use jane_present::text::Tone;
use jane_present::ui::core::{DragPayload, UiInput};
use jane_present::ui::dialogue::{self, DialogueBox};
use jane_present::ui::hud::{self, HudCtx};
use jane_present::ui::lan::{self, HostInfo, HostState, JoinInfo, JoinState, LanRow};
use jane_present::ui::menus::{self, MenuState, PauseInfo, SlotMode, SlotRow};
use jane_present::ui::title::{self, TitleInfo, TitleState};
use jane_present::ui::window::{self, WindowState};
use jane_present::ui::{Ui, UiOut};
use jane_present::view::{DialogueView, SlotData, StatusChip, StoreView, TargetFrame, ViewBuffers};
use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::{InputFrame, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Seat, Sim};

/// The screens, by name.
pub const SCREENS: [&str; 30] = [
    "hud",
    "map-day",
    "banner",
    "dead",
    "choice",
    "tooltip",
    "popover",
    "drag",
    "pause",
    "host",
    "join",
    "table",
    "controls",
    "display",
    "loading",
    "store",
    "long",
    "saved",
    "slots-save",
    "slots-confirm",
    "slots-grey",
    "slots-load",
    "quests-log",
    "quests-log-pad",
    "quests-abandon",
    "quests-main",
    "quests-day-1",
    "quests-day-4",
    "quests-night-1",
    "quests-night-4",
];

/// Side quests the quest sheets give her, beside the letter New Game gives.
const SHEET_QUESTS: [&str; 3] = ["ames_spectacles", "the_nurses_round", "hurst_camp"];

struct Rig {
    sim: Sim,
    present: Present,
    soft: Soft,
    ui: Ui,
    bufs: ViewBuffers,
}

/// The sim with the tick's events kept, for a bot to play and the buffers to read.
struct Played {
    sim: Sim,
    events: Vec<jane_sim::Event>,
}

impl jane_bot::Host for Played {
    fn sim(&self) -> &Sim {
        &self.sim
    }

    fn view(&self, seat: Seat) -> Option<jane_sim::View<'_>> {
        self.sim.view(seat)
    }

    fn step(&mut self, input: &StepInput<'_>) -> jane_sim::Stepped {
        self.sim.step(input)
    }

    fn drain_events(&mut self) -> &[jane_sim::Event] {
        self.events.clear();
        self.events.extend_from_slice(self.sim.drain_events());
        &self.events
    }
}

/// The map after a day of play: the reader plays seed `seed` from the train to the next evening
/// (or as far as it gets in `budget` frames), the buffers reading every tick, so what the map
/// holds is what she rested, read and walked into; then the chart, with three pins of hers and the
/// view's middle on a sign she read.
fn map_day(dir: &Path, seed: u32, cx: HudCtx<'_>) -> Result<(), String> {
    const BUDGET: u32 = 900_000;
    let mut host = Played { sim: Sim::new_game(seed, "Jane"), events: Vec::new() };
    let mut bot = jane_bot::Bot::story(jane_bot::Model::Reader);
    let mut bufs = ViewBuffers::new();
    let start = host.sim.state().day;
    let mut n = 0;
    while n < BUDGET && !bot.done() {
        bot.step(&mut host);
        let v = host.sim.view(Seat(0)).expect("seat 0");
        bufs.tick(&v, &host.events);
        n += 1;
        let s = host.sim.state();
        let evening = s.day > start && s.hour() >= 17;
        if evening && v.zone() == jane_core::ZoneId::County && !v.indoor() && bufs.memory.pending.is_empty() {
            break;
        }
    }
    let s = host.sim.state();
    println!(
        "map-day: {n} frames, day {} {:02}:00, {} marks inked ({} fires, {} signs, {} names)",
        s.day + 1,
        s.hour(),
        bufs.memory.inked.len(),
        bufs.memory.inked.iter().filter(|m| matches!(m.note, jane_present::memory::Note::Fire(_))).count(),
        bufs.memory.inked.iter().filter(|m| matches!(m.note, jane_present::memory::Note::Sign(_))).count(),
        bufs.memory.inked.iter().filter(|m| matches!(m.note, jane_present::memory::Note::Name(_))).count(),
    );
    let mut rig = Rig::from(host.sim, bufs);
    let v = rig.sim.view(Seat(0)).expect("seat 0");
    let zone = v.zone();
    let her = v.body().pos.cell();
    // The view's middle on a sign she read, nearest her of those the view can centre on (the
    // chart's window stops at its edges), and three pins of hers about it.
    let (w, h) = rig.sim.view(Seat(0)).expect("seat 0").size();
    let safe = |c: (i32, i32)| c.0 > 640 && c.0 < w as i32 - 640 && c.1 > 360 && c.1 < h as i32 - 360;
    let sign = rig
        .bufs
        .memory
        .of(zone)
        .filter(|m| matches!(m.note, jane_present::memory::Note::Sign(_)) && safe(m.at))
        .min_by_key(|m| (m.at.0 - her.0).pow(2) + (m.at.1 - her.1).pow(2))
        .map(|m| m.at);
    let mid = sign.unwrap_or(her);
    for (dx, dy) in [(-210, -90), (170, -120), (90, 130)] {
        rig.bufs.memory.toggle_pin(zone, (mid.0 + dx, mid.1 + dy));
    }
    let mut st = WindowState::on(3);
    let mut tick = rig.bufs.tick;
    // A first frame paints the chart; a second, after the fog's look, composes it.
    for k in 0..2 {
        st.map.pan =
            sign.map(|c| (c.0 / jane_present::ui::map::OUT_STEP as i32, c.1 / jane_present::ui::map::OUT_STEP as i32));
        tick += 40 * k;
        let v = rig.sim.view(Seat(0)).expect("seat 0");
        let canvas = (768, 432);
        rig.present.draw(128, canvas);
        rig.ui.begin(UiInput::default(), tick, canvas);
        window::draw(&mut rig.ui, &mut st, &rig.bufs, Some(&v), cx);
        rig.ui.finish(rig.present.frame_mut());
        rig.soft.draw(rig.present.frame());
    }
    rig.write(dir, "map-day")
}

impl Rig {
    /// A rig on a sim already played, and the buffers that watched it.
    fn from(sim: Sim, bufs: ViewBuffers) -> Rig {
        let mut present = Present::new(Tier::T0);
        let mut soft = Soft::new();
        soft.upload_atlas(present.atlas());
        let ui = Ui::new(present.ui_art().clone());
        let v = sim.view(Seat(0)).expect("seat 0");
        for _ in 0..3 {
            present.tick(&v, &[]);
        }
        Rig { sim, present, soft, ui, bufs }
    }

    fn new(seed: u32) -> Rig {
        let mut sim = Sim::new_game(seed, "Jane");
        let mut present = Present::new(Tier::T0);
        let mut soft = Soft::new();
        soft.upload_atlas(present.atlas());
        let ui = Ui::new(present.ui_art().clone());
        let mut bufs = ViewBuffers::new();
        for _ in 0..90 {
            sim.step(&StepInput { frames: [InputFrame::IDLE; MAX_PLAYERS], commands: &[] });
            let events = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0");
            present.tick(&v, &events);
            bufs.tick(&v, &events);
        }
        Rig { sim, present, soft, ui, bufs }
    }

    /// `n` frames stepped, the commands on the first, the presenter and the buffers fed.
    fn steps(&mut self, n: u32, cmds: &[jane_sim::Command]) {
        for i in 0..n {
            let stamped: Vec<jane_sim::StampedCommand> = if i == 0 {
                cmds.iter()
                    .enumerate()
                    .map(|(k, &cmd)| jane_sim::StampedCommand { seat: Some(Seat(0)), seq: k as u16, cmd })
                    .collect()
            } else {
                Vec::new()
            };
            self.sim.step(&StepInput { frames: [InputFrame::IDLE; MAX_PLAYERS], commands: &stamped });
            let events = self.sim.drain_events().to_vec();
            let v = self.sim.view(Seat(0)).expect("seat 0");
            self.present.tick(&v, &events);
            self.bufs.tick(&v, &events);
        }
    }

    /// Draws one frame: the world, then `ui_fn` over it at presenter tick `tick`.
    fn frame(&mut self, input: UiInput, tick: u32, ui_fn: impl FnOnce(&mut Ui, &ViewBuffers, &Sim)) -> Vec<UiOut> {
        let canvas = (768, 432);
        self.present.draw(128, canvas);
        self.ui.begin(input, tick, canvas);
        self.ui.draw_cursor = true;
        ui_fn(&mut self.ui, &self.bufs, &self.sim);
        let out = self.ui.out.clone();
        self.ui.finish(self.present.frame_mut());
        self.soft.draw(self.present.frame());
        out
    }

    fn write(&mut self, dir: &Path, name: &str) -> Result<(), String> {
        let mut px = Vec::new();
        let (w, h) = self.soft.read_back(&mut px);
        let mut rgba = Vec::with_capacity(px.len() * 4);
        for p in &px {
            let [_, r, g, b] = p.to_be_bytes();
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(format!("ui-sheet-{name}.png"));
        std::fs::write(&path, jane_art::sheet::png(u32::from(w), u32::from(h), &rgba))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{}", path.display());
        Ok(())
    }
}

fn at(p: (i32, i32)) -> UiInput {
    UiInput { pointer: Some(p), ..UiInput::default() }
}

/// Writes every screen (or those named) into `dir`.
pub fn run(dir: &Path, names: &[String]) -> Result<(), String> {
    if let Some(bad) = names.iter().find(|n| !SCREENS.contains(&n.as_str())) {
        return Err(format!("no screen {bad}: {}", SCREENS.join(", ")));
    }
    let want = |s: &str| names.is_empty() || names.iter().any(|n| n == s);
    let bind = Bindings::default();
    let cx = HudCtx { bindings: &bind, pad: false, window_open: false };
    let win_cx = HudCtx { window_open: true, ..cx };
    let mut rig = Rig::new(7);
    // A fight's HUD: a target, two chips, three toasts at once, a flash on the bar, the lag.
    {
        let b = &mut rig.bufs;
        b.hud.target = Some(TargetFrame { name: "Yard skeleton".into(), hp: b.hud.hp, hostile: true, boss: false });
        if let Some(t) = &mut b.hud.target {
            t.hp.frac = 420;
            t.hp.lag = 610;
        }
        let cat = jane_data::catalog();
        for (k, id) in ["chilled", "burning"].iter().enumerate() {
            if let Some(e) = cat.combat.effect_id(id) {
                let d = cat.combat.effect(e);
                b.hud.statuses.push(StatusChip {
                    icon: d.icon,
                    name: d.name,
                    ticks_left: 200 + k as u32 * 300,
                    total: 600,
                    harmful: true,
                });
            }
        }
        b.push_toast("Rats 3 of 5", Tone::Good);
        b.push_toast("My bag is full", Tone::Refused);
        b.push_toast("It shifts a little. Hold to push it.", Tone::Plain);
        b.push_toast("My bag is full", Tone::Refused);
        b.hud.hp.frac = 310;
        b.hud.hp.lag = 540;
        b.hud.hp.now = 46;
        b.hud.bar[0].flash = 8;
        b.hud.bar[0].cooldown = 620;
        b.hud.bar[6].gcd = 400;
        b.hud.prompt = Some(jane_present::view::Prompt { verb: "Talk", label: "The dog".into(), hold: false });
    }
    // The toasts arrived a few ticks ago: risen and faded in.
    rig.bufs.tick += 12;
    let tick = rig.bufs.tick;
    if want("hud") {
        rig.frame(at((520, 170)), tick, |ui, b, _| hud::draw(ui, b, cx));
        rig.write(dir, "hud")?;
    }
    if want("dead") {
        let mut b = rig.bufs.clone();
        b.me.dead = true;
        b.me.respawn_ticks = 185;
        b.hud.hp.frac = 0;
        b.hud.toasts.clear();
        rig.frame(UiInput::default(), tick, |ui, _, _| hud::draw(ui, &b, cx));
        rig.write(dir, "dead")?;
    }
    if want("choice") {
        let d = DialogueView {
            speaker: "The dog".into(),
            text: "You keep coming back to look. That is a city habit. Will you take the key or not?".into(),
            options: vec!["I'll take it.".into(), "Not yet.".into()],
            choosing: true,
            more: false,
            key: (1, 2, 3, 4),
        };
        let mut bx = DialogueBox::default();
        let mut b = rig.bufs.clone();
        b.hud.toasts.clear();
        b.hud.prompt = None;
        rig.frame(UiInput::default(), 1000, |ui, _, _| {
            hud::draw(ui, &b, cx);
            dialogue::draw(ui, &mut bx, &d, cx);
        });
        rig.frame(at((300, 330)), 1100, |ui, _, _| {
            hud::draw(ui, &b, cx);
            dialogue::draw(ui, &mut bx, &d, cx);
        });
        rig.write(dir, "choice")?;
    }
    let mut b = rig.bufs.clone();
    b.hud.toasts.clear();
    // The bag's first slot, as the window lays it out.
    let slot0 =
        (i32::from(window::rect((768, 432)).x) + 12 + 8 + 18, i32::from(window::rect((768, 432)).y) + 42 + 4 + 22 + 18);
    if want("tooltip") {
        let mut st = WindowState::default();
        for t in 0..30 {
            rig.frame(at(slot0), 2000 + t, |ui, _, sim| {
                hud::draw(ui, &b, win_cx);
                let v = sim.view(Seat(0));
                window::draw(ui, &mut st, &b, v.as_ref(), win_cx);
            });
        }
        rig.write(dir, "tooltip")?;
    }
    if want("popover") {
        let mut st = WindowState::default();
        let right = UiInput { pointer: Some(slot0), right_pressed: true, ..UiInput::default() };
        rig.frame(right, 3000, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        rig.frame(at((slot0.0 + 20, slot0.1 + 30)), 3001, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        rig.write(dir, "popover")?;
    }
    if want("drag") {
        let mut st = WindowState::default();
        let press = UiInput { pointer: Some(slot0), held: true, pressed: true, ..UiInput::default() };
        rig.frame(press, 4000, |ui, _, sim| {
            hud::draw(ui, &b, win_cx);
            window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
        });
        let over_bar = hud::bar_rect((768, 432));
        let to = (i32::from(over_bar.x) + 8 + 3 * 40 + 18, i32::from(over_bar.y) + 6 + 18);
        for (k, p) in [(slot0.0 + 30, slot0.1 + 40), (to.0 - 40, to.1 - 60), to].iter().enumerate() {
            let hold = UiInput { pointer: Some(*p), held: true, ..UiInput::default() };
            rig.frame(hold, 4001 + k as u32, |ui, _, sim| {
                hud::draw(ui, &b, win_cx);
                window::draw(ui, &mut st, &b, sim.view(Seat(0)).as_ref(), win_cx);
            });
        }
        let dragging = rig.ui.drag.is_some_and(|d| d.moved && d.payload == DragPayload::Bag(0));
        if !dragging {
            return Err("the drag did not start".into());
        }
        rig.write(dir, "drag")?;
    }
    // The save slots: the picker with used and empty slots, the question before a save is written
    // over, Save grey away from rest, and the load picker (sheets/slots/ in the docs).
    {
        let line = rig.bufs.hud.tracker.first().cloned().unwrap_or_default();
        let rows = vec![
            SlotRow {
                empty: false,
                zone: "The Lowfields".into(),
                when: "Day 3 · 21:14".into(),
                night: true,
                quest: line.title.clone(),
                step: line.step.clone(),
                hp: "34 of 40".into(),
                age: "5 min ago".into(),
                latest: true,
            },
            SlotRow { empty: true, ..SlotRow::default() },
            SlotRow {
                empty: false,
                zone: "Auntie's House".into(),
                when: "Day 1 · 17:40".into(),
                night: false,
                quest: "Under the House".into(),
                step: "The cellar's two iron doors, and the rats behind them".into(),
                hp: "40 of 40".into(),
                age: "yesterday".into(),
                latest: false,
            },
        ];
        let mut quiet = rig.bufs.clone();
        quiet.hud.toasts.clear();
        quiet.hud.target = None;
        quiet.hud.statuses.clear();
        if want("slots-save") {
            let mut st = MenuState { focus: 0 };
            rig.frame(UiInput::default(), 6000, |ui, _, _| {
                hud::draw(ui, &quiet, cx);
                ui.interactive = true;
                menus::slots(ui, &mut st, SlotMode::Save, &rows);
            });
            rig.write(dir, "slots-save")?;
        }
        if want("slots-confirm") {
            let mut under = MenuState { focus: 2 };
            let mut st = MenuState { focus: 1 };
            let detail = menus::overwrite_detail(&rows[2]);
            let ask = menus::Ask { question: "Save over slot 3?", detail: &detail, yes: "Save over", no: "Keep it" };
            rig.frame(UiInput::default(), 6010, |ui, _, _| {
                hud::draw(ui, &quiet, cx);
                ui.interactive = false;
                menus::slots(ui, &mut under, SlotMode::Save, &rows);
                ui.interactive = true;
                menus::ask(ui, &mut st, &ask);
            });
            rig.write(dir, "slots-confirm")?;
        }
        if want("slots-grey") {
            let mut st = MenuState::default();
            let info = PauseInfo {
                can_save: false,
                when: "Day 3, 21:14",
                zone: "The Lowfields",
                company: false,
                lan: Some(("Open to LAN", true)),
                guest: false,
            };
            rig.frame(UiInput::default(), 6020, |ui, _, _| {
                hud::draw(ui, &quiet, cx);
                ui.interactive = true;
                menus::pause(ui, &mut st, &info);
            });
            rig.write(dir, "slots-grey")?;
        }
        if want("slots-load") {
            let mut st = MenuState { focus: 2 };
            rig.frame(UiInput::default(), 6030, |ui, _, _| {
                ui.fill(jane_present::ui::cmd::Rect::new(0, 0, 768, 432), 0xff10_1014);
                ui.interactive = true;
                menus::slots(ui, &mut st, SlotMode::Load, &rows);
            });
            rig.write(dir, "slots-load")?;
        }
    }
    if want("saved") {
        // The save card: writing, then shut with its glint (another seat's rest), then a failure.
        for (name, text, ok, age) in [
            ("saved", "Saved", true, 20),
            ("saved-by", "Saved by the teal coat", true, 56),
            ("save-failed", "Couldn't save: the disk is full", false, 40),
        ] {
            let mut b = rig.bufs.clone();
            b.hud.toasts.clear();
            b.hud.prompt = None;
            b.hud.banner = None;
            b.saved(text, ok);
            b.tick += age;
            rig.frame(UiInput::default(), tick, |ui, _, _| {
                hud::draw(ui, &b, cx);
                jane_present::ui::saved::draw(ui, &b);
            });
            rig.write(dir, name)?;
        }
    }
    if want("long") {
        // The longest words a toast and the prompt carry: a mine chest's "has no keyhole" ran off
        // both edges of the canvas (2026-10-01).
        let mut b = rig.bufs.clone();
        b.hud.toasts.clear();
        b.push_toast("Chest has no keyhole. Something under the floor holds the lid down", Tone::Refused);
        b.push_toast("The Nameless Stone, cut clean and set in the ground by somebody's careful hand", Tone::Plain);
        b.hud.prompt = Some(jane_present::view::Prompt {
            verb: "Read",
            label: "The noticeboard outside the Castle Arms, papered over three times".into(),
            hold: false,
        });
        // Risen and faded in.
        b.tick += 12;
        b.hud.banner = None;
        rig.frame(UiInput::default(), tick, |ui, _, _| hud::draw(ui, &b, cx));
        rig.write(dir, "long")?;
    }
    if want("map-day") {
        map_day(dir, 7, win_cx)?;
    }
    if want("banner") {
        // She walks into the Long Hedge for the first time today: its name goes up.
        let mut r = Rig::new(7);
        let place = r
            .bufs
            .crossings
            .places
            .iter()
            .find(|p| p.name == "The Long Hedge")
            .or_else(|| r.bufs.crossings.places.first())
            .cloned()
            .ok_or("no named places")?;
        let jane_present::memory::Shape::Ring { c, r: rad } = place.shape else { return Err("not a ring".into()) };
        let stand = |r: &mut Rig, x: i32, y: i32| {
            let (z, id) = (r.sim.state().players[0].zone, r.sim.state().players[0].unit);
            if let Some(u) = r.sim.state_mut().zone_mut(z).and_then(|zs| zs.unit_mut(id)) {
                u.pos = jane_core::Vec2::centre(x, y);
            }
            r.sim.rebuild_runtimes();
        };
        stand(&mut r, c.0 + rad + 20, c.1);
        r.steps(20, &[]);
        r.bufs.hud.banner = None;
        stand(&mut r, c.0 + rad - 6, c.1);
        r.steps(70, &[]);
        let t = r.bufs.tick;
        r.frame(UiInput::default(), t, |ui, b, _| hud::draw(ui, b, cx));
        r.write(dir, "banner")?;
    }
    if want("store") {
        // At Julie's dresser: her bag beside it, a few things put away, a drag in flight from the
        // bag into the dresser.
        let cat = jane_data::catalog();
        let mut slots = vec![SlotData::default(); jane_sim::tuning::STORE_SLOTS];
        for (i, (name, qty)) in
            [("key_basement", 1), ("gold_bar", 16), ("gold_bar", 5), ("key_generic", 1), ("apple", 6), ("wood", 4)]
                .into_iter()
                .enumerate()
        {
            let item = cat.combat.item_id(name).ok_or(format!("no item {name}"))?;
            let d = cat.combat.item(item);
            slots[[0, 1, 2, 3, 6, 7][i]] =
                SlotData { item: Some(item), icon: Some(d.icon), count: qty, usable: d.usable, ..SlotData::default() };
        }
        let used = slots.iter().filter(|s| s.item.is_some()).count();
        let prop = jane_sim::ids::PropId(std::num::NonZeroU32::new(1).expect("one"));
        let mut sb = b.clone();
        sb.window.store = Some(StoreView { prop, name: "Dresser".into(), slots, used });
        let mut st = WindowState::default();
        let r = window::rect((768, 432));
        // The bag's first slot and the dresser's fifth, as the two panels lay them out.
        let lx = i32::from(r.x) + 12 + (i32::from(r.w) - 24 - 2 * (6 * 40 - 4) - 84) / 2;
        let gy = i32::from(r.y) + 42 + 6 + 26;
        let from = (lx + 18, gy + 18);
        let to = (lx + (6 * 40 - 4) + 84 + 4 * 40 + 18, gy + 18);
        let press = UiInput { pointer: Some(from), held: true, pressed: true, ..UiInput::default() };
        rig.frame(press, 4500, |ui, _, sim| {
            hud::draw(ui, &sb, win_cx);
            window::draw(ui, &mut st, &sb, sim.view(Seat(0)).as_ref(), win_cx);
        });
        for (k, p) in [(from.0 + 40, from.1 + 30), (to.0 - 60, to.1 + 20), to].iter().enumerate() {
            let hold = UiInput { pointer: Some(*p), held: true, ..UiInput::default() };
            rig.frame(hold, 4501 + k as u32, |ui, _, sim| {
                hud::draw(ui, &sb, win_cx);
                window::draw(ui, &mut st, &sb, sim.view(Seat(0)).as_ref(), win_cx);
            });
        }
        rig.write(dir, "store")?;
    }
    if want("pause") {
        let mut st = MenuState::default();
        let info = PauseInfo {
            can_save: true,
            when: "Day 1, 18:42",
            zone: "The Lowfields",
            company: false,
            lan: Some(("Open to LAN", true)),
            guest: false,
        };
        rig.frame(at((384, 180)), 5000, |ui, _, _| {
            hud::draw(ui, &b, cx);
            ui.interactive = true;
            menus::pause(ui, &mut st, &info);
        });
        rig.write(dir, "pause")?;
    }
    // The Controls screen's two pages: the bindings, and Display with T1's rows, two turned down.
    for (name, page) in [("controls", 0u8), ("display", 1)] {
        if !want(name) {
            continue;
        }
        let mut st = jane_present::ui::controls::ControlsState { page, drow: 1, ..Default::default() };
        let mut rows = jane_present::Features::of(Tier::T1);
        rows.cycle(Tier::T1, "shadows");
        rows.cycle(Tier::T1, "fog");
        let info = jane_present::ui::controls::ControlsInfo {
            assist: None,
            backend: "auto",
            volumes: jane_present::audio::Volumes::default(),
            rows,
            tier: Tier::T1,
        };
        let mut b = Bindings::default();
        rig.frame(at((384, 60)), 5000, |ui, _, _| {
            ui.interactive = true;
            jane_present::ui::controls::draw(ui, &mut st, &mut b, info);
        });
        rig.write(dir, name)?;
    }
    // Playing together (P8): the title's Host and Join, and the table on the HUD.
    let slots = [Some("The Lowfields · Day 2, 21:00".to_owned()), None, Some("Julie's house · Day 5, 08:00".into())];
    if want("host") {
        let mut t = TitleState { name: "Tess".into(), ..TitleState::default() };
        let mut st = HostState { port_text: "7777".into(), ..HostState::default() };
        st.choice.slot = Some(0);
        st.choice.seats = 3;
        for k in 0..2 {
            rig.frame(at((330, 262)), 6000 + k, |ui, _, _| {
                ui.interactive = false;
                title::draw(ui, &mut t, TitleInfo { has_save: true });
                ui.interactive = true;
                lan::host(ui, &mut st, &HostInfo { slots: &slots, port: 7777 });
            });
        }
        rig.write(dir, "host")?;
    }
    if want("join") {
        let mut t = TitleState::default();
        let mut st = JoinState { addr: "192.168.1.20".into(), ..JoinState::default() };
        let found = [
            LanRow { name: "Tess's world".into(), addr: "192.168.1.20:7777".into(), seats: "2 of 4".into(), ok: true },
            LanRow {
                name: "Jane's world (served)".into(),
                addr: "192.168.1.31:7777".into(),
                seats: "1 of 4".into(),
                ok: true,
            },
            LanRow { name: "Mo's world".into(), addr: "192.168.1.44:7777".into(), seats: "4 of 4".into(), ok: false },
        ];
        let refused = "Refused: the host's content is a3516ef3a75d37a1, this build's is a3516ef3a75de90c: both must run the same data";
        let info = JoinInfo { found: &found, status: Some((refused, true)), joining: false };
        rig.frame(at((300, 150)), 7000, |ui, _, _| {
            ui.interactive = false;
            title::draw(ui, &mut t, TitleInfo { has_save: true });
            ui.interactive = true;
            lan::join(ui, &mut st, &info);
        });
        rig.write(dir, "join")?;
    }
    if want("table") {
        rig.frame(UiInput::default(), 8000, |ui, b, _| {
            hud::draw(ui, b, cx);
            lan::table(ui, 0b0111, 1, false, true);
            lan::stall(ui, 0b0100, 3400, false);
        });
        rig.write(dir, "table")?;
    }
    // New Game's loading screen mid-scroll: every stage reported at once, as on a fast machine.
    if want("loading") {
        use jane_present::ui::loading::{self, LoadingState, Mode};
        let mut st = LoadingState::new(Mode::Scroll, 7, "New Game", 0);
        for s in jane_world::build_stages() {
            st.stage(s);
        }
        st.finish();
        let at_tick = 100;
        for now in 0..=at_tick {
            st.tick(now);
        }
        rig.frame(UiInput::default(), at_tick, |ui, _, _| loading::draw(ui, &mut st));
        rig.write(dir, "loading")?;
    }
    if names.is_empty() || names.iter().any(|n| n.starts_with("quests-")) {
        quests(dir, &want, cx)?;
    }
    Ok(())
}

/// The Log with its tick boxes and keys, "Abandon ...?", the main line's grey Abandon, and the
/// tracker in play by day and by night with one quest and with four.
fn quests(dir: &Path, want: &dyn Fn(&str) -> bool, cx: HudCtx<'_>) -> Result<(), String> {
    let cat = jane_data::catalog();
    let side: Vec<_> = SHEET_QUESTS.iter().map(|q| cat.story.quest_id(q).expect("a sheet quest")).collect();
    let give: Vec<jane_sim::Command> =
        side.iter().map(|&q| jane_sim::Command::Dev(jane_sim::input::DevOp::Quest(q))).collect();
    for (hour, label) in [(17, "day"), (22, "night")] {
        let mut rig = Rig::new(7);
        rig.sim.state_mut().clock = hour * jane_sim::tuning::TICKS_PER_HOUR;
        rig.steps(30, &[]);
        for (n, cmds) in [(1, &[][..]), (4, &give[..])] {
            let name = format!("quests-{label}-{n}");
            rig.steps(30, cmds);
            if want(&name) {
                let mut b = rig.bufs.clone();
                b.hud.toasts.clear();
                b.hud.banner = None;
                rig.frame(UiInput::default(), 9000, |ui, _, _| hud::draw(ui, &b, cx));
                rig.write(dir, &name)?;
            }
        }
        if label != "day" {
            continue;
        }
        // The Log, with the fourth quest untracked by hand and a side quest lit.
        rig.bufs.track.toggle(side[2]);
        rig.steps(2, &[]);
        let mut b = rig.bufs.clone();
        b.hud.toasts.clear();
        let lit = b.window.quests.iter().position(|r| r.id == Some(side[0])).unwrap_or(0);
        let main = b.window.quests.iter().position(|r| r.main).unwrap_or(0);
        let win_cx = HudCtx { window_open: true, ..cx };
        let shots: [(&str, usize, bool, bool); 4] = [
            ("quests-log", lit, false, false),
            ("quests-log-pad", lit, true, false),
            ("quests-abandon", lit, false, true),
            ("quests-main", main, false, false),
        ];
        for (name, row, pad, asking) in shots {
            if !want(name) {
                continue;
            }
            let mut st = WindowState::on(2);
            st.log = row;
            if let Some(q) = b.window.quests[row].id.filter(|_| asking) {
                st.ask_abandon(q);
            }
            let wcx = HudCtx { pad, ..win_cx };
            let input = if pad { UiInput { pad: true, ..UiInput::default() } } else { UiInput::default() };
            rig.frame(input, 9100, |ui, _, _| {
                hud::draw(ui, &b, wcx);
                window::draw(ui, &mut st, &b, None, wcx);
            });
            rig.write(dir, name)?;
        }
    }
    Ok(())
}
