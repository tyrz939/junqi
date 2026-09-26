//! The game as the player meets it (PRESENTATION.md §3): the title, the loading screen, play
//! with its HUD, dialogue, windows, pause and menus, the terminal and the overlays, save slots and
//! `config.json`, driven by one fixed 60 Hz loop (§1.11).
//!
//! Each frame: SDL events into the devices; the devices into one held `InputFrame`, the presses
//! into `Edge`s, and the pointer and the navigation presses into a `UiInput`; as many ticks as
//! the clock has accumulated (the sim steps unless the world is held; the presenter and the view
//! buffers tick regardless); the world's frame at `alpha`; the UI's layers over it, bottom to top;
//! the backend draws the lot. What the UI hands back is `Command`s for the sim and `AppIntent`s
//! for this file.

use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use jane_present::input::{
    Context, Edge, GameAction, Input, KeySet, Mode, UiAction, canvas_size, canvas_to_world, pick, world_to_canvas,
};
use jane_present::text;
use jane_present::ui::Ui;
use jane_present::ui::core::{AppIntent, UiInput, UiOut};
use jane_present::ui::dialogue::{self, DialogueBox};
use jane_present::ui::hud::{self, HudCtx};
use jane_present::ui::loading::{self, Card, LoadingState};
use jane_present::ui::menus::{self, MenuState, PauseInfo, SlotMode, SlotRow};
use jane_present::ui::title::{self, TitleInfo, TitleState};
use jane_present::view::ViewBuffers;
use jane_present::{Frame, Present};
use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame, StampedCommand, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Blueprints, Seat, Sim};

use crate::config::Config;
use crate::devices::{Devices, Happened};
use crate::game::Screen;
use crate::saves::{self, Dirs, SLOTS};
use crate::script::{Script, Step};
use crate::{Args, shot};

/// A tick is `1/60` s; the accumulator counts nanoseconds times 60, so a tick is exactly 1e9.
const TICK: u64 = 1_000_000_000;
/// At most this many ticks a frame (at 1x); beyond it the time is dropped (and counted).
const MAX_CATCH_UP: u64 = 5;
/// The seat this window plays.
const ME: Seat = Seat(0);
/// The clear behind the title and the loading screen.
const DARK: u32 = 0xff10_1014;

/// What the loader thread says.
enum Loaded {
    Card(Box<Card>),
    Sim(Result<Box<Sim>, String>),
}

/// Where the player is.
enum Scene {
    Title,
    Loading { rx: Receiver<Loaded>, st: LoadingState, sim: Option<Box<Sim>>, slot: Option<u8> },
    Play,
}

/// A screen over play, top one first in the list of what eats input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    Pause,
    Slots(SlotMode),
    /// A question the app asked: quit to title and lose what is unsaved.
    ConfirmTitle,
}

/// How fast the world runs (F7, F8) and whether it is held for stepping (F6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Speed {
    /// Ticks per real tick, in quarters: 1 slow, 4 normal, 16 fast.
    pub quarters: u32,
    pub held: bool,
    pub step: bool,
}

/// The whole app between frames.
struct App<'a> {
    args: &'a Args,
    dirs: Dirs,
    config: Config,
    present: Present,
    ui: Ui,
    bufs: ViewBuffers,
    input: Input,
    scene: Scene,
    sim: Option<Box<Sim>>,
    title: TitleState,
    menus: Vec<Menu>,
    menu_state: MenuState,
    dialogue: DialogueBox,
    pending: Vec<StampedCommand>,
    seq: u16,
    camera: (i32, i32),
    speed: Speed,
    /// Presenter ticks since the app started (title included): what scripts count.
    ticks: u64,
    dropped: u64,
    quit: bool,
    /// A message for the player that is not a toast (a save written, a load refused).
    note: Option<(String, u32)>,
    /// The slot rows as last read from disk.
    slot_rows: Vec<SlotRow>,
    /// The slot the game last saved to or loaded from: quick save's.
    slot: Option<u8>,
    shots: u32,
}

pub fn run(
    args: &Args,
    pump: &mut sdl2::EventPump,
    pads: Option<sdl2::GameControllerSubsystem>,
    text_in: &sdl2::keyboard::TextInputUtil,
    screen: &mut dyn Screen,
) -> Result<(), String> {
    let dirs = Dirs::find(args.data_dir.as_deref());
    let config = Config::load(&dirs);
    let mut present = Present::new(screen.backend().caps().tier);
    screen.backend().upload_atlas(present.atlas());
    let ui = Ui::new(present.ui_art().clone());
    let describe = screen.describe();
    let mut win = screen.size();
    let mut devices = Devices::new(pads, win.1);
    let mut canvas_px = canvas_size(win.0, win.1);
    present.set_canvas(canvas_px);
    let mut app = App {
        args,
        title: TitleState {
            name: if config.name.is_empty() { args.name.clone() } else { config.name.clone() },
            ..TitleState::default()
        },
        slot: config.last_slot,
        dirs,
        config,
        present,
        ui,
        bufs: ViewBuffers::new(),
        input: Input::new(),
        scene: Scene::Title,
        sim: None,
        menus: Vec::new(),
        menu_state: MenuState::default(),
        dialogue: DialogueBox::default(),
        pending: Vec::new(),
        seq: 0,
        camera: (0, 0),
        speed: Speed { quarters: 4, held: false, step: false },
        ticks: 0,
        dropped: 0,
        quit: false,
        note: None,
        slot_rows: Vec::new(),
        shots: 0,
    };
    app.read_slots();
    if args.new {
        app.new_game(args.name.clone(), args.seed);
    }
    let mut script = match &args.script {
        Some(s) => Some(Script::parse(s)?),
        None => None,
    };
    let mut events: Vec<Event> = Vec::with_capacity(64);
    let mut edges: Vec<Edge> = Vec::with_capacity(16);
    let mut typed = String::new();
    let mut shot_px: Vec<u32> = Vec::new();
    let mut acc: u64 = 0;
    let mut last = Instant::now();
    let mut held_left = false;
    let mut title_clock = (Instant::now(), 0u32, Duration::ZERO, Duration::ZERO, 0u32);
    let mut typing = false;
    // Script presses to let go of next frame.
    let mut release: Vec<u16> = Vec::new();

    while !app.quit {
        let frame_start = Instant::now();
        for code in release.drain(..) {
            devices.state.key(code, false);
        }
        for e in pump.poll_iter() {
            if let sdl2::event::Event::TextInput { text, .. } = &e {
                typed.push_str(text);
            }
            match devices.event(&e) {
                Happened::Quit => app.quit = true,
                Happened::Resized => {
                    win = screen.size();
                    screen.resized(win);
                    devices.set_window_height(win.1);
                    canvas_px = canvas_size(win.0, win.1);
                }
                Happened::Nothing => {}
            }
        }
        devices.poll_pad();
        // The script's inputs, as a device would give them.
        let mut script_shot = None;
        if let Some(s) = &mut script {
            for step in s.due(app.ticks) {
                match step {
                    Step::Key(k) => {
                        devices.state.key(k, true);
                        release.push(k);
                    }
                    Step::Down(k) => devices.state.key(k, true),
                    Step::Up(k) => devices.state.key(k, false),
                    Step::Move(x, y) => {
                        devices.state.mouse.pos = Some((f32::from(x as i16), f32::from(y as i16)));
                        devices.state.mouse.moved = true;
                    }
                    Step::Click(x, y) | Step::RightClick(x, y) => {
                        devices.state.mouse.pos = Some((f32::from(x as i16), f32::from(y as i16)));
                        let b = if matches!(step, Step::Click(..)) {
                            jane_present::input::MouseButton::Left
                        } else {
                            jane_present::input::MouseButton::Right
                        };
                        devices.state.button(b, true);
                        devices.state.button(b, false);
                    }
                    Step::Type(t) => typed.push_str(&t),
                    Step::Shot(p) => script_shot = Some(p),
                }
            }
        }

        // What the UI hears this frame.
        let mode = app.mode(typing);
        if typing {
            text_in.start();
        } else {
            text_in.stop();
            typed.clear();
        }
        let left = jane_present::input::MouseButton::Left;
        let now_held = devices.state.mouse.is_held(left);
        let pressed = devices.state.mouse.was_pressed(left);
        let released = (held_left || pressed) && !now_held;
        held_left = now_held;
        let pointer = devices.state.mouse.pos.map(|(x, y)| (x as i32, y as i32));
        // A press over the UI is the UI's, not the world's.
        if mode == Mode::Play && pressed && app.ui.wants_pointer() {
            devices.state.mouse.pressed &= !1;
        }
        let feet =
            app.sim.as_ref().and_then(|s| s.view(ME)).map(|v| world_to_canvas(v.body().pos, app.camera)).filter(
                |&(x, y)| (0.0..f32::from(canvas_px.0)).contains(&x) && (0.0..f32::from(canvas_px.1)).contains(&y),
            );
        let held = app.input.sample(&devices.state, &Context { mode, feet });
        let cursor = devices.state.mouse.pos.filter(|_| app.input.aiming_with_mouse());
        let keys: KeySet = devices.state.pressed;
        let wheel = devices.state.mouse.wheel;
        let right = devices.state.mouse.was_pressed(jane_present::input::MouseButton::Right);
        devices.state.end_sample();
        edges.extend(app.input.drain());
        let mut actions = Vec::new();
        for edge in edges.drain(..) {
            app.edge(edge, cursor, &mut actions, screen, &mut shot_px);
        }
        let ui_input = UiInput {
            pointer,
            held: now_held,
            pressed,
            released,
            right_pressed: right,
            wheel,
            actions,
            typed: std::mem::take(&mut typed),
            keys,
            pad: app.input.pad_active(),
        };

        // The clock: whole ticks due since the last frame, scaled by the speed.
        let now = Instant::now();
        acc += (now - last).as_nanos() as u64 * 60 * u64::from(app.speed.quarters) / 4;
        last = now;
        let cap = MAX_CATCH_UP * u64::from(app.speed.quarters.max(4)) / 4;
        let mut due = acc / TICK;
        if due > cap {
            app.dropped += due - cap;
            acc -= (due - cap) * TICK;
            due = cap;
        }
        let t_ticks = Instant::now();
        for _ in 0..due {
            events.clear();
            app.tick(held, &mut events);
            acc -= TICK;
            app.ticks += 1;
            title_clock.4 += 1;
        }
        let tick_time = t_ticks.elapsed();

        // One frame at alpha: the world, then the UI over it.
        let t_draw = Instant::now();
        let alpha = (acc * 256 / TICK).min(255) as u8;
        if matches!(app.scene, Scene::Play) && app.sim.is_some() {
            app.camera = app.present.draw(alpha, canvas_px).camera;
        } else {
            blank(app.present.frame_mut(), canvas_px);
        }
        app.ui.begin(ui_input, app.present.ticks().max(app.ticks as u32), canvas_px);
        app.draw_ui();
        typing = app.ui.typing;
        app.ui.finish(app.present.frame_mut());
        screen.backend().draw(app.present.frame());
        let draw_time = t_draw.elapsed();
        screen.show(win)?;
        for out in std::mem::take(&mut app.ui.out) {
            match out {
                UiOut::Command(c) => app.command(c),
                UiOut::Intent(i) => app.intent(i),
            }
        }
        if let Some(p) = script_shot {
            match save_shot(screen, &mut shot_px, &p) {
                Ok(_) => println!("jane-app: shot {p}"),
                Err(e) => eprintln!("jane-app: shot: {e}"),
            }
        }
        title_clock.1 += 1;
        title_clock.2 += tick_time;
        title_clock.3 += draw_time;
        if title_clock.0.elapsed() >= Duration::from_secs(1) {
            let secs = title_clock.0.elapsed().as_millis().max(1);
            let fps = u128::from(title_clock.1) * 1000 / secs;
            let t = format!("Jane: {describe}, {fps} fps");
            let _ = screen.window_mut().set_title(&t);
            title_clock = (Instant::now(), 0, Duration::ZERO, Duration::ZERO, 0);
        }
        if args.ticks.is_some_and(|n| app.ticks >= n) {
            break;
        }
        if due == 0 && frame_start.elapsed() < Duration::from_millis(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if let Some(path) = &args.shot {
        let (w, h) = save_shot(screen, &mut shot_px, path)?;
        println!("jane-app: shot {path} ({w} x {h})");
    }
    if let Some(sim) = &app.sim {
        println!(
            "jane-app: seed {}: {} ticks, {} dropped, frame {}, hash {:016x}",
            sim.state().seed,
            app.ticks,
            app.dropped,
            sim.state().frame,
            sim.hash()
        );
    }
    Ok(())
}

/// An empty frame at `canvas`: nothing of the world, the dark clear.
fn blank(f: &mut Frame, canvas: (u16, u16)) {
    f.passes.clear();
    f.canvas = canvas;
    f.clear = DARK;
}

/// Writes the canvas the backend last drew to `path` as a PNG.
pub fn save_shot(screen: &mut dyn Screen, px: &mut Vec<u32>, path: &str) -> Result<(u16, u16), String> {
    let (w, h) = screen.backend().read_back(px);
    shot::write(path, px, w, h)?;
    Ok((w, h))
}

impl App<'_> {
    /// Who has the keyboard.
    fn mode(&self, typing: bool) -> Mode {
        if typing {
            return Mode::Text;
        }
        match self.scene {
            Scene::Play if self.menus.is_empty() && !self.talking() => Mode::Play,
            _ => Mode::Ui,
        }
    }

    fn talking(&self) -> bool {
        self.bufs.dialogue.is_some() && matches!(self.scene, Scene::Play)
    }

    /// The world is held: a menu is up with nobody else here, or F6 holds it.
    fn world_held(&self) -> bool {
        !self.menus.is_empty() || (self.speed.held && !self.speed.step)
    }

    fn tick(&mut self, held: InputFrame, events: &mut Vec<Event>) {
        match &mut self.scene {
            Scene::Title => {}
            Scene::Loading { rx, st, sim, slot } => {
                while let Ok(m) = rx.try_recv() {
                    match m {
                        Loaded::Card(c) => {
                            st.card = Some(*c);
                            st.since = self.ticks as u32;
                        }
                        Loaded::Sim(Ok(s)) => {
                            *sim = Some(s);
                            st.built = true;
                        }
                        Loaded::Sim(Err(e)) => {
                            eprintln!("jane-app: {e}");
                            self.note = Some((e, self.ticks as u32));
                            self.scene = Scene::Title;
                            return;
                        }
                    }
                }
                if st.done(self.ticks as u32) && sim.is_some() {
                    let loaded_slot = *slot;
                    self.sim = sim.take();
                    self.scene = Scene::Play;
                    self.bufs = ViewBuffers::new();
                    self.dialogue.reset();
                    self.menus.clear();
                    self.pending.clear();
                    if loaded_slot.is_some() {
                        self.slot = loaded_slot;
                    }
                }
            }
            Scene::Play => {
                let held_still = self.world_held();
                let Some(sim) = self.sim.as_mut() else { return };
                if !held_still {
                    let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
                    frames[ME.index()] = held;
                    self.pending.sort_by_key(|c| (c.seat, c.seq));
                    sim.step(&StepInput { frames, commands: &self.pending });
                    self.pending.clear();
                    events.extend_from_slice(sim.drain_events());
                    self.speed.step = false;
                }
                if let Some(v) = sim.view(ME) {
                    self.present.tick(&v, events);
                    self.bufs.tick(&v, events);
                    if events.iter().any(|e| matches!(e.kind, jane_sim::EventKind::Rest)) {
                        self.autosave();
                    }
                }
            }
        }
    }

    /// One press, as the app hears it.
    fn edge(
        &mut self,
        edge: Edge,
        cursor: Option<(f32, f32)>,
        actions: &mut Vec<UiAction>,
        screen: &mut dyn Screen,
        shot_px: &mut Vec<u32>,
    ) {
        match edge {
            Edge::Game(g) => {
                if !matches!(self.scene, Scene::Play) || !self.menus.is_empty() {
                    return;
                }
                let cmd = match g {
                    GameAction::Use => Command::Use,
                    GameAction::Bar(slot) => Command::Bar {
                        slot,
                        on: cursor.and_then(|c| {
                            self.sim
                                .as_ref()
                                .and_then(|s| s.view(ME))
                                .map(|v| pick(&v, canvas_to_world(c, self.camera)))
                        }),
                    },
                };
                self.command(cmd);
            }
            Edge::Ui(a) => match a {
                UiAction::Pause => {
                    if matches!(self.scene, Scene::Play) && self.menus.is_empty() {
                        self.menus.push(Menu::Pause);
                        self.menu_state = MenuState::default();
                    }
                }
                UiAction::Cancel => {
                    if self.ui.popover_open() || self.title.naming {
                        actions.push(a);
                    } else if !self.menus.is_empty() {
                        self.menus.pop();
                        self.menu_state = MenuState::default();
                    } else if self.talking() {
                        actions.push(a);
                    }
                }
                UiAction::Shot => {
                    self.shots += 1;
                    let path = shot::numbered(self.args.shot.as_deref().unwrap_or("jane-shot.png"), self.shots);
                    match save_shot(screen, shot_px, &path) {
                        Ok(_) => println!("jane-app: shot {path}"),
                        Err(e) => eprintln!("jane-app: shot: {e}"),
                    }
                }
                UiAction::QuickSave => self.save_to(self.slot.unwrap_or(0)),
                UiAction::QuickLoad => {
                    if let Some(n) = self.slot.or_else(|| saves::latest(&self.dirs)) {
                        self.load(n);
                    }
                }
                UiAction::Step => {
                    if self.speed.held {
                        self.speed.step = true;
                    } else {
                        self.speed.held = true;
                    }
                }
                UiAction::Slow => self.speed.quarters = if self.speed.quarters == 1 { 4 } else { 1 },
                UiAction::Fast => self.speed.quarters = if self.speed.quarters == 16 { 4 } else { 16 },
                _ => actions.push(a),
            },
        }
    }

    fn command(&mut self, cmd: Command) {
        if self.sim.is_none() {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        self.pending.push(StampedCommand { seat: Some(ME), seq: self.seq, cmd });
    }

    fn intent(&mut self, i: AppIntent) {
        match i {
            AppIntent::NewGame { name } => {
                self.config.name.clone_from(&name);
                let _ = self.config.save(&self.dirs);
                let seed = if self.args.seed_given { self.args.seed } else { crate::clock_seed() };
                self.new_game(name, seed);
            }
            AppIntent::Continue => {
                if let Some(n) = saves::latest(&self.dirs) {
                    self.load(n);
                }
            }
            AppIntent::Load(n) => self.load(n),
            AppIntent::Save(n) => {
                self.save_to(n);
                self.menus.retain(|m| !matches!(m, Menu::Slots(_)));
            }
            AppIntent::LoadMenu => {
                self.read_slots();
                self.menus.push(Menu::Slots(SlotMode::Load));
                self.menu_state = MenuState::default();
            }
            AppIntent::SaveMenu => {
                self.read_slots();
                self.menus.push(Menu::Slots(SlotMode::Save));
                self.menu_state = MenuState::default();
            }
            AppIntent::ToTitle => {
                self.menus.push(Menu::ConfirmTitle);
                self.menu_state = MenuState { focus: 1 };
            }
            AppIntent::Quit => self.quit = true,
            AppIntent::Resume => self.menus.clear(),
            AppIntent::Pause => {
                if self.menus.is_empty() {
                    self.menus.push(Menu::Pause);
                    self.menu_state = MenuState::default();
                }
            }
            AppIntent::Back => {
                self.menus.pop();
                self.menu_state = MenuState::default();
            }
            _ => {}
        }
    }

    fn read_slots(&mut self) {
        let latest = saves::latest(&self.dirs);
        self.slot_rows = (0..SLOTS)
            .map(|n| match saves::info(&self.dirs, n) {
                Some(i) => {
                    let s = &i.summary;
                    SlotRow {
                        empty: false,
                        zone: text::zone_name(s.zone, jane_data::Region::Lowfields).into(),
                        when: format!("Day {}, {:02}:00", s.day + 1, s.hour),
                        hp: format!("HP {} of {}", s.hp.points(), s.max_hp.points()),
                        age: saves::age(i.modified),
                        latest: latest == Some(n),
                    }
                }
                None => SlotRow { empty: true, ..SlotRow::default() },
            })
            .collect();
    }

    /// New Game: the county built on a thread while the loading screen draws its skeleton.
    fn new_game(&mut self, name: String, seed: u32) {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            if let Ok(s) = jane_world::skeleton::skeleton(seed) {
                let _ = tx.send(Loaded::Card(Box::new(Card::from_skeleton(&s))));
            }
            let t0 = Instant::now();
            let sim = Blueprints::build(seed)
                .map(|bps| Box::new(Sim::new_game_with(bps, &name)))
                .map_err(|e| format!("seed {seed}: {e}"));
            println!("jane-app: seed {seed}: the county built in {} ms", t0.elapsed().as_millis());
            let _ = tx.send(Loaded::Sim(sim));
        });
        let st = LoadingState { card: None, since: self.ticks as u32, built: false, seed, verb: "New Game" };
        self.scene = Scene::Loading { rx, st, sim: None, slot: None };
        self.sim = None;
    }

    /// Load slot `n`: the county rebuilt from its seed on a thread, the save's deltas over it.
    fn load(&mut self, n: u8) {
        let bytes = match saves::read(&self.dirs, n) {
            Ok(b) => b,
            Err(e) => {
                self.say(&e);
                return;
            }
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let sim = Sim::from_save(&bytes).map(Box::new).map_err(|e| format!("slot {}: {e:?}", n + 1));
            let _ = tx.send(Loaded::Sim(sim));
        });
        let st = LoadingState { card: None, since: self.ticks as u32, built: false, seed: 0, verb: "Load" };
        self.scene = Scene::Loading { rx, st, sim: None, slot: Some(n) };
        self.sim = None;
        self.menus.clear();
        self.config.last_slot = Some(n);
        let _ = self.config.save(&self.dirs);
    }

    fn save_to(&mut self, n: u8) {
        let Some(sim) = &self.sim else { return };
        if !self.bufs.me.can_save {
            self.bufs.push_toast("I can only save by a bed or a fire", jane_present::text::Tone::Refused);
            return;
        }
        match saves::write(&self.dirs, n, &sim.save()) {
            Ok(()) => {
                self.slot = Some(n);
                self.config.last_slot = Some(n);
                let _ = self.config.save(&self.dirs);
                self.bufs.push_toast(&format!("Saved to slot {}", n + 1), jane_present::text::Tone::Good);
                self.read_slots();
            }
            Err(e) => self.say(&e),
        }
    }

    /// She rested: the game writes the slot it last used (§3.2 `Event::Rest`).
    fn autosave(&mut self) {
        let Some(sim) = &self.sim else { return };
        let n = self.slot.unwrap_or(0);
        if saves::write(&self.dirs, n, &sim.save()).is_ok() {
            self.slot = Some(n);
        }
    }

    fn say(&mut self, s: &str) {
        eprintln!("jane-app: {s}");
        self.bufs.push_toast(s, jane_present::text::Tone::Refused);
    }

    /// The UI's layers, bottom to top; only the top one answers.
    fn draw_ui(&mut self) {
        let pad = self.input.pad_active();
        let cx = HudCtx { bindings: &self.input.bindings, pad, window_open: false };
        match &mut self.scene {
            Scene::Title => {
                self.ui.interactive = self.menus.is_empty();
                let info = TitleInfo { has_save: self.slot_rows.iter().any(|s| !s.empty) };
                title::draw(&mut self.ui, &mut self.title, info);
            }
            Scene::Loading { st, .. } => {
                self.ui.interactive = false;
                loading::draw(&mut self.ui, st);
            }
            Scene::Play => {
                let top_is_hud = self.menus.is_empty() && self.bufs.dialogue.is_none();
                self.ui.interactive = top_is_hud;
                hud::draw(&mut self.ui, &self.bufs, cx);
                if let Some(d) = &self.bufs.dialogue {
                    self.ui.interactive = self.menus.is_empty();
                    dialogue::draw(&mut self.ui, &mut self.dialogue, d, cx);
                } else {
                    self.dialogue.reset();
                }
            }
        }
        // The menus over whichever scene.
        let n = self.menus.len();
        for (k, m) in self.menus.clone().into_iter().enumerate() {
            self.ui.interactive = k + 1 == n;
            match m {
                Menu::Pause => {
                    let (clock, day) = self.sim.as_ref().map_or((0, 0), |s| (s.state().clock, s.state().day));
                    let mut when = format!("Day {}, ", day + 1);
                    text::clock(clock, &mut when);
                    let zone = self.bufs.hud.zone_name;
                    let info = PauseInfo { can_save: self.bufs.me.can_save, when: &when, zone, company: false };
                    menus::pause(&mut self.ui, &mut self.menu_state, &info);
                }
                Menu::Slots(mode) => menus::slots(&mut self.ui, &mut self.menu_state, mode, &self.slot_rows),
                Menu::ConfirmTitle => {
                    if let Some(yes) = menus::confirm(&mut self.ui, &mut self.menu_state, "Quit to the title?") {
                        self.menus.pop();
                        if yes {
                            self.menus.clear();
                            self.sim = None;
                            self.scene = Scene::Title;
                            self.title.naming = false;
                            self.read_slots();
                        }
                    }
                }
            }
        }
    }
}
