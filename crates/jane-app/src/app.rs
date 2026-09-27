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

use jane_core::ZoneId;
use jane_net::Session;
use jane_present::input::{
    Context, Edge, GameAction, Input, KeySet, Mode, UiAction, canvas_size, canvas_to_world, pick, sc, world_to_canvas,
};
use jane_present::text;
use jane_present::ui::Ui;
use jane_present::ui::console::{self as term, Console, LineKind};
use jane_present::ui::controls::{self, ControlsInfo, ControlsState};
use jane_present::ui::core::{AppIntent, PadPress, UiInput, UiOut};
use jane_present::ui::dialogue::{self, DialogueBox};
use jane_present::ui::hud::{self, HudCtx};
use jane_present::ui::lan::{self as lan_ui, HostChoice, HostInfo, JoinInfo};
use jane_present::ui::loading::{self, Card, LoadingState};
use jane_present::ui::menus::{self, MenuState, PauseInfo, SlotMode, SlotRow};
use jane_present::ui::perf::{self, FrameInfo, PerfLog, PerfView, SimTally, TopLine};
use jane_present::ui::title::{self, TitleInfo, TitleState};
use jane_present::ui::window::{self, WindowState};
use jane_present::ui::world::{self, WorldDebug};
use jane_present::view::ViewBuffers;
use jane_present::{Frame, Present};
use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame};
use jane_sim::{Blueprints, Seat, Sim};

use crate::config::Config;
use crate::devices::{Devices, Happened};
use crate::game::Screen;
use crate::lan::Lan;
use crate::saves::{self, Dirs, SLOTS};
use crate::script::{Script, Step};
use crate::{Args, shot};

/// A tick is `1/60` s; the accumulator counts nanoseconds times 60, so a tick is exactly 1e9.
const TICK: u64 = 1_000_000_000;
/// The most the clock may fall behind (at 1x), in ticks: a second. Beyond it the time is
/// dropped (and counted). A slow frame (a present that blocks, an occluded window, a GPU asleep)
/// never drops a tick: the ticks it owes run on the next frame, which is drawn once.
const MAX_BEHIND: u64 = 60;
/// Tick work one frame does at most before it draws: a sim slower than real time still shows,
/// and what it owes waits for the next frame (then for the second's cap).
const TICK_BUDGET: Duration = Duration::from_millis(50);
/// A guest this many frames behind the host steps more than one a tick to catch up.
const BEHIND: u32 = 2;
/// The clear behind the title and the loading screen.
const DARK: u32 = 0xff10_1014;

/// What the loader thread says.
enum Loaded {
    /// A stage of the build has started (`jane_world::Report`).
    Stage(&'static str),
    Card(Box<Card>),
    Sim(Result<Box<Sim>, String>),
}

/// Where the player is.
enum Scene {
    Title,
    /// `host`: the world opens to the LAN once built (the title's Host).
    Loading {
        rx: Receiver<Loaded>,
        st: Box<LoadingState>,
        sim: Option<Box<Sim>>,
        slot: Option<u8>,
        host: Option<HostChoice>,
    },
    Play,
}

/// A screen over play, top one first in the list of what eats input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    Pause,
    Slots(SlotMode),
    /// A question the app asked: quit to title and lose what is unsaved.
    ConfirmTitle,
    Controls,
    /// The title's Host and Join (P8).
    Host,
    Join,
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
    /// The world and how it is stepped: alone, hosting or joined (`jane_net::Session`).
    session: Option<Session>,
    title: TitleState,
    menus: Vec<Menu>,
    menu_state: MenuState,
    dialogue: DialogueBox,
    /// This seat's presses, for the next step.
    pending: Vec<Command>,
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
    /// F2: 0 off, 1 compact, 2 full.
    perf_level: u8,
    perf: PerfLog,
    /// This frame's stages so far, µs: sim, present, build, backend, wait.
    stages: [u32; 5],
    /// F3.
    world_dbg: WorldDebug,
    /// The state hash and the tick it was taken on (it is dear: every half second while shown).
    hash: (u64, u64),
    /// `Sim::metrics()` over the steps, for F2.
    tally: SimTally,
    started: Instant,
    backend_name: String,
    /// A headless player on the seat (`--bot`, a script's `bot`): it plays, the UI shows it.
    bot: Option<jane_bot::Bot>,
    /// Give the seat back the moment a conversation opens (a script's `bot talk`).
    bot_until_talk: bool,
    /// The window (bag, book, log, map) and whether it is open; its state (the map's chart)
    /// outlives a close.
    win: WindowState,
    win_open: bool,
    /// The terminal (backquote).
    console: Console,
    /// Where the reticle is drawn this frame.
    reticle: Option<(i32, i32)>,
    controls: ControlsState,
    /// The Controls screen turned a `Features` row: the backend is told before the next draw.
    features_changed: bool,
    /// Playing together: the Host and Join screens, a join under way, the table.
    lan: Lan,
    /// What the bot heard of the steps since it last acted.
    bot_heard: Vec<Event>,
    /// The sound device and the cue table that drives it (PRESENTATION.md §5).
    sound: crate::audio::Sound,
    soundtrack: jane_present::audio::Soundtrack,
}

/// The wall clock lent to the sim for `Sim::metrics`' phase times: nanoseconds since its first
/// read. Presentation only: the sim reads it to fill the metrics and never to move, so each
/// machine at a table lends its own and the hashes still agree (`jane-sim/tests/metrics.rs`).
fn wall_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    u64::try_from(START.get_or_init(Instant::now).elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Lend a guest's world the wall clock: it comes with the host's welcome (and again with a
/// resync), after the session opened. A local or hosted world was lent it in `open_session`.
fn lend_wall_clock(s: &mut Session) {
    if let Session::Guest(g) = s
        && let Some(sim) = g.sim_mut()
    {
        sim.set_wall_clock(Some(wall_ns));
    }
}

/// The world, if there is one.
fn sim_of(s: Option<&Session>) -> Option<&Sim> {
    s.and_then(Session::sim)
}

/// The seat this window plays: seat 0 alone or hosting, the seat the host gave when joined.
fn me_of(s: Option<&Session>) -> Seat {
    s.and_then(Session::seat).unwrap_or(Seat::HOST)
}

pub fn run(
    args: &Args,
    pump: &mut sdl2::EventPump,
    pads: Option<sdl2::GameControllerSubsystem>,
    text_in: &sdl2::keyboard::TextInputUtil,
    screen: &mut dyn Screen,
    sound: crate::audio::Sound,
) -> Result<(), String> {
    let dirs = Dirs::find(args.data_dir.as_deref());
    let config = Config::load(&dirs);
    let mut present = Present::new(screen.backend().caps().tier);
    screen.backend().upload_atlas(present.atlas());
    // The Features rows (PRESENTATION.md §1.3): the tier's own with config.json's laid over
    // them; the presenter acts on most, the backend is handed the ones it draws itself.
    present.set_features(config.features(present.frame().tier));
    screen.backend().set_features(&present.features());
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
        session: None,
        menus: Vec::new(),
        menu_state: MenuState::default(),
        dialogue: DialogueBox::default(),
        pending: Vec::new(),
        camera: (0, 0),
        speed: Speed { quarters: 4, held: false, step: false },
        ticks: 0,
        dropped: 0,
        quit: false,
        note: None,
        slot_rows: Vec::new(),
        shots: 0,
        perf_level: 0,
        perf: PerfLog::default(),
        stages: [0; 5],
        world_dbg: WorldDebug::default(),
        hash: (0, u64::MAX),
        tally: SimTally::default(),
        started: Instant::now(),
        backend_name: describe.clone(),
        bot: args.bot.as_deref().and_then(jane_bot::Model::parse).map(jane_bot::Bot::story),
        bot_until_talk: false,
        win: WindowState::default(),
        win_open: false,
        console: Console::default(),
        reticle: None,
        controls: ControlsState::default(),
        features_changed: false,
        lan: Lan::new(args.port),
        bot_heard: Vec::new(),
        sound,
        soundtrack: jane_present::audio::Soundtrack::new(),
    };
    app.input.bindings = app.config.bindings();
    app.input.assist = app.config.assist();
    app.read_slots();
    if args.new {
        let host = args.host.then_some(HostChoice {
            slot: None,
            open: true,
            seats: args.seats,
            delay: args.delay,
            wait: args.wait,
            port: args.port,
        });
        app.new_game(args.name.clone(), args.seed, host);
    } else if let Some(addr) = &args.join {
        app.intent(AppIntent::JoinMenu);
        app.intent(AppIntent::Join(addr.clone()));
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
    let mut pad_was = (0u32, false, false);
    let mut title_clock = (Instant::now(), 0u32, Duration::ZERO, Duration::ZERO, 0u32);
    let mut typing = false;
    // The tick the backend last drew at (the `frame_skip` row).
    let mut drawn_at: u64 = 0;
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
                    Step::Bot(m) => {
                        app.bot_until_talk = m == "talk";
                        let model = if m == "talk" { "reader" } else { m.as_str() };
                        app.bot = jane_bot::Model::parse(model).map(jane_bot::Bot::story);
                    }
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
        // While F3 is up, 1 to 9 toggle its layers and press no bar slot.
        if app.world_dbg.on && mode == Mode::Play {
            for k in 0..9u16 {
                if devices.state.pressed.has(sc::N1 + k) {
                    app.world_dbg.toggle(usize::from(k));
                    devices.state.pressed.set(sc::N1 + k, false);
                }
            }
        }
        let feet = sim_of(app.session.as_ref())
            .and_then(|s| s.view(me_of(app.session.as_ref())))
            .map(|v| world_to_canvas(v.body().pos, app.camera))
            .filter(|&(x, y)| (0.0..f32::from(canvas_px.0)).contains(&x) && (0.0..f32::from(canvas_px.1)).contains(&y));
        let held = app.input.sample(&devices.state, &Context { mode, feet });
        let cursor = devices.state.mouse.pos.filter(|_| app.input.aiming_with_mouse());
        // The reticle: where the assist will send a bolt, at the cursor's distance from her chest.
        app.reticle = None;
        if mode == Mode::Play
            && let (Some(c), Some(f), Some(aim)) = (cursor, feet, held.aim)
            && let Some(v) = sim_of(app.session.as_ref()).and_then(|s| s.view(me_of(app.session.as_ref())))
        {
            let spell = v.me().bar.iter().find_map(|b| match b {
                Some(jane_data::BarSlot::Spell(s)) => Some(*s),
                _ => None,
            });
            let a = spell.and_then(|s| v.assisted_aim(&held, s)).unwrap_or(aim);
            let chest = (f.0, f.1 - jane_present::input::CHEST_PX);
            let d = ((c.0 - chest.0).powi(2) + (c.1 - chest.1).powi(2)).sqrt();
            let k = f32::from(jane_core::angle::cos_q15(a).0 as i16) / 32768.0;
            let s = f32::from(jane_core::angle::sin_q15(a).0 as i16) / 32768.0;
            app.reticle = Some(((chest.0 + k * d) as i32, (chest.1 + s * d) as i32));
        }
        let keys: KeySet = devices.state.pressed;
        let wheel = devices.state.mouse.wheel;
        let right = devices.state.mouse.was_pressed(jane_present::input::MouseButton::Right);
        let middle = devices.state.mouse.was_pressed(jane_present::input::MouseButton::Middle);
        let pad_now = devices.state.pad.map_or((0, false, false), |p| (p.held, p.axes[4] > 16_384, p.axes[5] > 16_384));
        let pad_pressed =
            PadPress { buttons: pad_now.0 & !pad_was.0, lt: pad_now.1 && !pad_was.1, rt: pad_now.2 && !pad_was.2 };
        pad_was = pad_now;
        devices.state.end_sample();
        // Any press once the county is built goes straight on to play.
        if let Scene::Loading { st, .. } = &mut app.scene
            && (!keys.is_empty() || pressed || right || pad_pressed.buttons != 0)
        {
            st.press();
        }
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
            middle_pressed: middle,
            pad_pressed,
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
        let cap = MAX_BEHIND * u64::from(app.speed.quarters.max(4)) / 4;
        let mut due = acc / TICK;
        if due > cap {
            app.dropped += due - cap;
            acc -= (due - cap) * TICK;
            due = cap;
        }
        let t_ticks = Instant::now();
        app.stages = [0; 5];
        // The network, before the ticks and after: what came, and what is due to go.
        app.net();
        for _ in 0..due {
            events.clear();
            app.tick(held, &mut events);
            acc -= TICK;
            app.ticks += 1;
            title_clock.4 += 1;
            if t_ticks.elapsed() >= TICK_BUDGET || args.ticks.is_some_and(|n| app.ticks >= n) {
                break;
            }
        }
        app.net();
        app.table_news();
        let tick_time = t_ticks.elapsed();
        // A window nobody can see (minimised, hidden) is not drawn: the clock ticks on, and no
        // present can hold the loop (PRESENTATION.md §1.11).
        if !screen.visible() && args.ticks.is_none() {
            std::thread::sleep(Duration::from_millis(4));
            continue;
        }

        // One frame at alpha: the world, then the UI over it.
        let t_draw = Instant::now();
        let alpha = (acc * 256 / TICK).min(255) as u8;
        if matches!(app.scene, Scene::Play) && sim_of(app.session.as_ref()).is_some() {
            app.camera = app.present.draw(alpha, canvas_px).camera;
        } else {
            blank(app.present.frame_mut(), canvas_px);
        }
        let stats = screen.backend().stats();
        app.ui.begin(ui_input, app.present.ticks().max(app.ticks as u32), canvas_px);
        app.ui.draw_cursor = true;
        app.ui.reticle = app.reticle;
        app.draw_ui(stats);
        typing = app.ui.typing;
        app.ui.finish(app.present.frame_mut());
        app.stages[2] = t_draw.elapsed().as_micros() as u32;
        if app.features_changed {
            screen.backend().set_features(&app.present.features());
            app.features_changed = false;
        }
        // The `frame_skip` row (§1.12 step 8): the backend draws and shows every other tick, 30
        // fps; the sim, the presenter and the UI still run every frame, so no press is lost.
        let skip = app.present.features().frame_skip && app.ticks < drawn_at + 2 && args.ticks.is_none();
        let t_backend = Instant::now();
        if !skip {
            screen.backend().draw(app.present.frame());
            drawn_at = app.ticks;
        }
        app.stages[3] = t_backend.elapsed().as_micros() as u32;
        let draw_time = t_draw.elapsed();
        let t_wait = Instant::now();
        if !skip {
            screen.show(win)?;
        }
        app.stages[4] = t_wait.elapsed().as_micros() as u32;
        app.perf.frame(app.stages, app.started.elapsed().as_millis() as u64, app.ticks);
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
        // A present that does not wait for the display (mailbox) is paced to its refresh here,
        // so the GPU draws the frames that are shown and no more.
        if let Some(every) = screen.frame_interval() {
            let left = every.saturating_sub(frame_start.elapsed()).saturating_sub(Duration::from_micros(500));
            if !left.is_zero() {
                std::thread::sleep(left);
            }
        } else if due == 0 && frame_start.elapsed() < Duration::from_millis(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if let Some(path) = &args.shot {
        let (w, h) = save_shot(screen, &mut shot_px, path)?;
        println!("jane-app: shot {path} ({w} x {h})");
    }
    if let Some(sim) = sim_of(app.session.as_ref()) {
        println!(
            "jane-app: seed {}: {} ticks, {} dropped, frame {}, hash {:016x}",
            sim.state().seed,
            app.ticks,
            app.dropped,
            sim.state().frame,
            sim.hash()
        );
    }
    if let Some(Session::Host(h)) = &app.session {
        let c = h.checks();
        println!("jane-app: hash checks with guests: {} agreed, {} differed, last at frame {}", c.ok, c.bad, c.last);
    }
    app.end_session();
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
        if typing || self.console.open || self.controls.capture.is_some() {
            return Mode::Text;
        }
        match self.scene {
            Scene::Play if self.menus.is_empty() && !self.talking() && !self.win_open => Mode::Play,
            _ => Mode::Ui,
        }
    }

    fn talking(&self) -> bool {
        self.bufs.dialogue.is_some() && matches!(self.scene, Scene::Play)
    }

    /// The world is held: a menu is up with nobody else here, or F6 holds it.
    fn world_held(&self) -> bool {
        !self.menus.is_empty() || self.win_open || (self.speed.held && !self.speed.step)
    }

    /// Opens the window on `tab`, switches to it, or closes the window when it is already there.
    fn window_key(&mut self, tab: usize) {
        if !matches!(self.scene, Scene::Play) || !self.menus.is_empty() {
            return;
        }
        if self.win_open && self.win.tab == tab {
            self.win_open = false;
        } else {
            self.win_open = true;
            self.win.tab = tab;
        }
    }

    fn tick(&mut self, held: InputFrame, events: &mut Vec<Event>) {
        match &mut self.scene {
            Scene::Title => self.soundtrack.title(&mut self.sound),
            Scene::Loading { rx, st, sim, slot, host } => {
                self.soundtrack.title(&mut self.sound);
                while let Ok(m) = rx.try_recv() {
                    match m {
                        Loaded::Stage(s) => st.stage(s),
                        Loaded::Card(c) => {
                            st.card = Some(*c);
                            st.since = self.ticks as u32;
                        }
                        Loaded::Sim(Ok(s)) => {
                            *sim = Some(s);
                            st.finish();
                        }
                        Loaded::Sim(Err(e)) => {
                            eprintln!("jane-app: {e}");
                            self.note = Some((e, self.ticks as u32));
                            self.scene = Scene::Title;
                            return;
                        }
                    }
                }
                st.tick(self.ticks as u32);
                if st.done(self.ticks as u32)
                    && let Some(s) = sim.take()
                {
                    let (loaded_slot, host) = (*slot, *host);
                    let session = self.open_session(*s, host);
                    self.begin_play(session);
                    if loaded_slot.is_some() {
                        self.slot = loaded_slot;
                    }
                }
            }
            Scene::Play => {
                // Alone, a menu or F6 holds the world; with company (or open to it) nothing
                // holds, and her stick is idle while her menu is up (ENGINE.md §4).
                let paused = self.world_held();
                let now = self.started.elapsed().as_millis() as u64;
                let me = me_of(self.session.as_ref());
                let Some(session) = self.session.as_mut() else { return };
                lend_wall_clock(session);
                if !(paused && session.pauses()) {
                    let t = Instant::now();
                    // A guest behind the host steps what it has in hand to catch up.
                    let mut budget = 1 + session.backlog().saturating_sub(BEHIND).min(5);
                    while budget > 0 {
                        budget -= 1;
                        let frame = match &mut self.bot {
                            Some(bot) => {
                                bot.seat = me;
                                let v = session.sim().and_then(|s| s.view(me));
                                let act = bot.act(v.as_ref(), &self.bot_heard);
                                self.pending.extend(act.cmds);
                                act.frame
                            }
                            None => held,
                        };
                        if session.try_step(now, frame, &mut self.pending, paused).is_none() {
                            break;
                        }
                        events.extend_from_slice(session.events());
                        if let Some(sim) = session.sim() {
                            self.tally.add(&sim.metrics());
                        }
                        self.bot_heard.clear();
                        self.bot_heard.extend_from_slice(session.events());
                        self.speed.step = false;
                    }
                    let us = t.elapsed().as_micros() as u32;
                    self.stages[0] += us;
                    self.perf.tick(us);
                }
                let rested = session.take_rested();
                // Alone and held, the music steps back; with company the world goes on, and so
                // does its sound (PRESENTATION.md §5.5).
                self.sound.set_held(paused && session.pauses());
                let t = Instant::now();
                if let Some(v) = session.sim().and_then(|s| s.view(me)) {
                    self.present.tick(&v, events);
                    self.bufs.tick(&v, events);
                    self.soundtrack.tick(&v, events, &mut self.sound);
                    self.stages[1] += t.elapsed().as_micros() as u32;
                    if self.bot_until_talk && self.bufs.dialogue.is_some() {
                        self.bot = None;
                        self.bot_until_talk = false;
                    }
                }
                // Anyone at the table rested: the world is written where it lives (a guest's
                // rest saves the host's world; a guest writes nothing).
                if rested {
                    self.autosave();
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
                            sim_of(self.session.as_ref())
                                .and_then(|s| s.view(me_of(self.session.as_ref())))
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
                    if self.console.open {
                        self.console.open = false;
                    } else if self.controls.capture.is_some() || self.ui.popover_open() || self.title.naming {
                        actions.push(a);
                    } else if !self.menus.is_empty() {
                        self.menus.pop();
                        self.menu_state = MenuState::default();
                    } else if self.win_open {
                        if self.win.destroy.is_some() {
                            actions.push(a);
                        } else {
                            self.win_open = false;
                        }
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
                        self.load(n, None);
                    }
                }
                // Speed and stepping are a lone player's: a table keeps the host's time.
                UiAction::Step | UiAction::Slow | UiAction::Fast if !self.alone() => {}
                UiAction::Step => {
                    if self.speed.held {
                        self.speed.step = true;
                    } else {
                        self.speed.held = true;
                    }
                }
                UiAction::Console => self.console.toggle(self.ticks as u32),
                UiAction::Bags => self.window_key(0),
                UiAction::Book => self.window_key(1),
                UiAction::Quests => self.window_key(2),
                UiAction::Map => self.window_key(3),
                UiAction::Debug => self.perf_level = (self.perf_level + 1) % 3,
                UiAction::Grid => self.world_dbg.on = !self.world_dbg.on,
                UiAction::Slow => self.speed.quarters = if self.speed.quarters == 1 { 4 } else { 1 },
                UiAction::Fast => self.speed.quarters = if self.speed.quarters == 16 { 4 } else { 16 },
                _ => {
                    // A step through a menu is heard; in play the same keys walk.
                    if matches!(a, UiAction::Up | UiAction::Down | UiAction::Left | UiAction::Right)
                        && self.mode(false) != Mode::Play
                    {
                        self.soundtrack.ui(jane_present::audio::SfxKind::UiMove, &mut self.sound);
                    }
                    actions.push(a);
                }
            },
        }
    }

    fn command(&mut self, cmd: Command) {
        if sim_of(self.session.as_ref()).is_none() {
            return;
        }
        self.pending.push(cmd);
    }

    fn intent(&mut self, i: AppIntent) {
        if let Some(k) = crate::audio::intent_sound(&i) {
            self.soundtrack.ui(k, &mut self.sound);
        }
        match i {
            AppIntent::NewGame { name } => {
                self.config.name.clone_from(&name);
                let _ = self.config.save(&self.dirs);
                let seed = if self.args.seed_given { self.args.seed } else { crate::clock_seed() };
                self.new_game(name, seed, None);
            }
            AppIntent::Continue => {
                if let Some(n) = saves::latest(&self.dirs) {
                    self.load(n, None);
                }
            }
            AppIntent::Load(n) => self.load(n, None),
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
            AppIntent::Controls => {
                self.menus.push(Menu::Controls);
                self.controls = ControlsState::default();
            }
            AppIntent::Pause => {
                if self.menus.is_empty() {
                    self.menus.push(Menu::Pause);
                    self.menu_state = MenuState::default();
                }
            }
            AppIntent::Console(line) => self.console_line(&line),
            AppIntent::OpenWindow(tab) => self.window_key(usize::from(tab)),
            AppIntent::CloseWindow => self.win_open = false,
            AppIntent::Back => {
                if self.menus.last() == Some(&Menu::Join) {
                    if self.lan.joining.is_some() {
                        // Back while knocking is Cancel; the screen stays.
                        self.lan.cancel_join();
                        self.lan.status = Some(("Cancelled".to_owned(), false));
                        return;
                    }
                    self.lan.close_finder();
                }
                self.menus.pop();
                self.menu_state = MenuState::default();
            }
            AppIntent::HostMenu => {
                self.read_slots();
                self.lan.host_form.choice.seats = self.args.seats;
                self.lan.host_form.choice.delay = self.args.delay;
                self.lan.host_form.choice.wait = self.args.wait;
                self.lan.host_form.choice.port = self.lan.port;
                self.lan.host_form.port_text = self.lan.port.to_string();
                self.menus.push(Menu::Host);
            }
            AppIntent::JoinMenu => {
                self.lan.open_finder(self.config.last_host.as_deref());
                self.menus.push(Menu::Join);
            }
            AppIntent::Host(choice) => {
                self.menus.clear();
                self.lan.port = choice.port;
                match choice.slot {
                    Some(n) => self.load(n, Some(choice)),
                    None => {
                        let name = if self.title.name.trim().is_empty() {
                            self.args.name.clone()
                        } else {
                            self.title.name.clone()
                        };
                        let seed = if self.args.seed_given { self.args.seed } else { crate::clock_seed() };
                        self.new_game(name, seed, Some(choice));
                    }
                }
            }
            AppIntent::Join(addr) => {
                let token = crate::lan::token(self.args.token, &mut self.config, &self.dirs);
                self.config.last_host = Some(addr.clone());
                let _ = self.config.save(&self.dirs);
                let now = self.started.elapsed().as_millis() as u64;
                self.lan.start_join(&addr, token, now);
            }
            AppIntent::OpenToLan => self.open_to_lan(),
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

    /// The loading screen `--loading` asked for: the scroll, or the developer's map.
    fn loading_mode(&self) -> loading::Mode {
        if self.args.loading_map { loading::Mode::Map } else { loading::Mode::Scroll }
    }

    /// New Game: the county built on a thread while the loading screen says each stage of it
    /// (or, `--loading map`, draws its skeleton).
    /// `host`: the world is opened to the LAN once built (the title's Host).
    fn new_game(&mut self, name: String, seed: u32, host: Option<HostChoice>) {
        let (tx, rx) = channel();
        let mode = self.loading_mode();
        std::thread::spawn(move || {
            if mode == loading::Mode::Map
                && let Ok(s) = jane_world::skeleton::skeleton(seed)
            {
                let _ = tx.send(Loaded::Card(Box::new(Card::from_skeleton(&s))));
            }
            let t0 = Instant::now();
            let sim = Blueprints::build_with(seed, &mut |s| {
                let _ = tx.send(Loaded::Stage(s));
            })
            .map(|bps| Box::new(Sim::new_game_with(bps, &name)))
            .map_err(|e| format!("seed {seed}: {e}"));
            println!("jane-app: seed {seed}: the county built in {} ms", t0.elapsed().as_millis());
            let _ = tx.send(Loaded::Sim(sim));
        });
        let st = Box::new(LoadingState::new(mode, seed, "New Game", self.ticks as u32));
        self.end_session();
        self.scene = Scene::Loading { rx, st, sim: None, slot: None, host };
    }

    /// Load slot `n`: the county rebuilt from its seed on a thread, the save's deltas over it.
    /// `host`: the loaded world is opened to the LAN (the title's Host on a slot).
    fn load(&mut self, n: u8, host: Option<HostChoice>) {
        let bytes = match saves::read(&self.dirs, n) {
            Ok(b) => b,
            Err(e) => {
                self.say(&e);
                return;
            }
        };
        let (tx, rx) = channel();
        let seed = self.config.slot_seeds.get(usize::from(n)).copied().flatten();
        let mode = self.loading_mode();
        std::thread::spawn(move || {
            // With the seed known the stages are said as the county is rebuilt (or it forms
            // on the map card); a seed remembered wrong falls back to the save's own, and with
            // none the save builds its own county, said as one stage.
            let sim = match seed {
                Some(seed) => {
                    if mode == loading::Mode::Map
                        && let Ok(s) = jane_world::skeleton::skeleton(seed)
                    {
                        let _ = tx.send(Loaded::Card(Box::new(Card::from_skeleton(&s))));
                    }
                    Blueprints::build_with(seed, &mut |s| {
                        let _ = tx.send(Loaded::Stage(s));
                    })
                    .ok()
                    .and_then(|bps| Sim::from_save_with(&bytes, bps).ok())
                    .map_or_else(|| Sim::from_save(&bytes), Ok)
                }
                None => {
                    let _ = tx.send(Loaded::Stage("skeleton"));
                    Sim::from_save(&bytes)
                }
            };
            let _ = tx.send(Loaded::Sim(sim.map(Box::new).map_err(|e| format!("slot {}: {e:?}", n + 1))));
        });
        let st = Box::new(LoadingState::new(mode, seed.unwrap_or(0), "Load", self.ticks as u32));
        self.end_session();
        self.scene = Scene::Loading { rx, st, sim: None, slot: Some(n), host };
        self.menus.clear();
        self.config.last_slot = Some(n);
        let _ = self.config.save(&self.dirs);
    }

    fn save_to(&mut self, n: u8) {
        if matches!(self.session, Some(Session::Guest(_))) {
            self.bufs.push_toast("The world is the host's to save", jane_present::text::Tone::Refused);
            return;
        }
        let Some(sim) = sim_of(self.session.as_ref()) else { return };
        if !self.bufs.me.can_save {
            self.bufs.push_toast("I can only save by a bed or a fire", jane_present::text::Tone::Refused);
            return;
        }
        let (bytes, seed) = (sim.save(), sim.state().seed);
        match saves::write(&self.dirs, n, &bytes) {
            Ok(()) => {
                self.slot = Some(n);
                self.config.last_slot = Some(n);
                self.config.set_slot_seed(n, seed);
                let _ = self.config.save(&self.dirs);
                self.bufs.push_toast(&format!("Saved to slot {}", n + 1), jane_present::text::Tone::Good);
                self.soundtrack.ui(jane_present::audio::SfxKind::Save, &mut self.sound);
                self.read_slots();
            }
            Err(e) => self.say(&e),
        }
    }

    /// She rested: the game writes the slot it last used (§3.2 `Event::Rest`).
    fn autosave(&mut self) {
        if matches!(self.session, Some(Session::Guest(_))) {
            return;
        }
        let Some((bytes, seed)) = sim_of(self.session.as_ref()).map(|s| (s.save(), s.state().seed)) else { return };
        let n = self.slot.unwrap_or(0);
        if saves::write(&self.dirs, n, &bytes).is_ok() {
            self.slot = Some(n);
            self.config.set_slot_seed(n, seed);
            let _ = self.config.save(&self.dirs);
        }
    }

    fn say(&mut self, s: &str) {
        eprintln!("jane-app: {s}");
        self.bufs.push_toast(s, jane_present::text::Tone::Refused);
    }

    // --- playing together (ARCHITECTURE.md §7) ---------------------------------------------

    /// Alone: pause holds the world, and the speed keys work.
    fn alone(&self) -> bool {
        self.session.as_ref().is_none_or(Session::pauses)
    }

    /// The session a built or loaded world is played through: alone, or hosted on the LAN.
    /// A port that cannot be listened on leaves her alone, saying why.
    fn open_session(&mut self, mut sim: Sim, host: Option<HostChoice>) -> Session {
        sim.set_wall_clock(Some(wall_ns));
        let Some(choice) = host else { return Session::local(sim) };
        let cfg = crate::lan::host_config(&sim.state().name, choice);
        let local = Session::local(sim);
        match local.open_to_lan(cfg, self.lan.port) {
            Ok(mut s) => {
                if let Session::Host(h) = &mut s {
                    h.set_open(choice.open);
                }
                println!("jane-app: hosting on port {}", self.lan.port);
                s
            }
            Err((s, e)) => {
                self.say(&format!("Could not host on port {}: {e}", self.lan.port));
                s
            }
        }
    }

    /// Into play with `session`: the buffers fresh, the menus gone.
    fn begin_play(&mut self, session: Session) {
        self.end_session();
        // The county's music is the county's seed's, on every machine at the table.
        if let Some(sim) = session.sim() {
            self.sound.set_seed(sim.state().seed);
        }
        self.session = Some(session);
        self.scene = Scene::Play;
        self.bufs = ViewBuffers::new();
        self.dialogue.reset();
        self.menus.clear();
        self.pending.clear();
        self.bot_heard.clear();
        self.tally = SimTally::default();
        self.lan.seats = 0;
        self.lan.stall = None;
        if let Some(Session::Host(_)) = &self.session {
            let s = format!("Open to the LAN on port {}", self.lan.port);
            self.bufs.push_toast(&s, jane_present::text::Tone::Good);
        }
    }

    /// Leave the table (a guest says goodbye, a host closes it) and drop the world.
    fn end_session(&mut self) {
        if let Some(mut s) = self.session.take() {
            s.close();
            // Let the goodbye go before the socket is dropped.
            s.poll(self.started.elapsed().as_millis() as u64);
        }
        self.lan.stall = None;
    }

    /// The pause menu's "Open to LAN": the world she is playing alone becomes the host's.
    fn open_to_lan(&mut self) {
        if !matches!(self.session, Some(Session::Local(_))) {
            return;
        }
        let Some(s) = self.session.take() else { return };
        let choice = HostChoice {
            seats: self.args.seats,
            delay: self.args.delay,
            wait: self.args.wait,
            ..HostChoice::default()
        };
        let name = s.sim().map_or_else(|| self.args.name.clone(), |x| x.state().name.clone());
        match s.open_to_lan(crate::lan::host_config(&name, choice), self.lan.port) {
            Ok(s) => {
                self.session = Some(s);
                self.lan.seats = 0;
                self.menus.clear();
                let line = format!("Open to the LAN on port {}", self.lan.port);
                println!("jane-app: {line}");
                self.bufs.push_toast(&line, jane_present::text::Tone::Good);
            }
            Err((s, e)) => {
                self.session = Some(s);
                self.say(&format!("Could not host on port {}: {e}", self.lan.port));
            }
        }
    }

    /// The network, once or twice a frame: the session's traffic, the Join screen's list and a
    /// join under way (into play when the host welcomes it).
    fn net(&mut self) {
        let now = self.started.elapsed().as_millis() as u64;
        if let Some(s) = &mut self.session {
            s.poll(now);
        }
        if self.menus.contains(&Menu::Join) {
            self.lan.poll_finder(self.ticks);
        }
        if let Some(s) = self.lan.poll_join(now) {
            let seat = s.seat().map_or(0, |x| x.0);
            println!("jane-app: joined in seat {seat}");
            self.lan.close_finder();
            self.begin_play(s);
        }
    }

    /// What the table's changes say: the coats that sat down and got up, whom it waits for, a
    /// desync's report, and a host that went.
    fn table_news(&mut self) {
        let Some(s) = &mut self.session else { return };
        if s.pauses() {
            self.lan.stall = None;
            return;
        }
        let st = s.status();
        self.lan.stall = st.stall;
        let mut says = Vec::new();
        if let Some(sim) = s.sim() {
            self.lan.seat_changes(sim, |line, good| says.push((line, good)));
        }
        for (line, good) in says {
            let tone = if good { jane_present::text::Tone::Good } else { jane_present::text::Tone::Plain };
            self.bufs.push_toast(&line, tone);
        }
        for n in &st.notes {
            match n {
                jane_net::Note::Dropped { .. } | jane_net::Note::Refused { .. } | jane_net::Note::Desync(_) => {
                    println!("jane-app: {n}");
                    self.console.say(&n.to_string(), LineKind::Out);
                }
                _ => {}
            }
        }
        if let Some(r) = &st.desync {
            self.bufs.push_toast("Our worlds parted; the host set them right", jane_present::text::Tone::Refused);
            eprintln!("jane-app: {r}");
            self.console.say(&r.to_string(), LineKind::Error);
        }
        if let Some(why) = st.ended {
            // The host went: the table is gone, and so is her world (it was the host's).
            self.end_session();
            self.scene = Scene::Title;
            self.menus.clear();
            self.read_slots();
            self.note = Some((why.clone(), self.ticks as u32));
            self.say(&format!("The table is gone: {why}"));
        }
    }

    /// The UI's layers, bottom to top; only the top one answers.
    fn draw_ui(&mut self, stats: Option<jane_present::FrameStats>) {
        let pad = self.input.pad_active();
        let cx = HudCtx { bindings: &self.input.bindings, pad, window_open: self.win_open };
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
                if self.world_dbg.on
                    && let Some(v) = sim_of(self.session.as_ref()).and_then(|s| s.view(me_of(self.session.as_ref())))
                {
                    self.ui.interactive = false;
                    world::draw(&mut self.ui, &mut self.world_dbg, &v, &self.present, self.present.frame());
                }
                let top_is_hud = self.menus.is_empty() && self.bufs.dialogue.is_none();
                self.ui.interactive = top_is_hud;
                hud::draw(&mut self.ui, &self.bufs, cx);
                // At a table: who sits at it, and whom it waits for.
                if let Some(sess) = self.session.as_ref().filter(|s| !s.pauses()) {
                    let me = me_of(self.session.as_ref());
                    let hosting = matches!(sess, Session::Host(_));
                    let chips = !self.bufs.hud.statuses.is_empty();
                    if !self.win_open {
                        lan_ui::table(&mut self.ui, self.lan.seats, me.0, hosting, chips);
                    }
                    if let Some(st) = self.lan.stall {
                        lan_ui::stall(&mut self.ui, st.seats, st.waited_ms, st.wait);
                    }
                }
                if self.win_open && self.bufs.dialogue.is_none() {
                    self.ui.interactive = self.menus.is_empty();
                    let v = sim_of(self.session.as_ref()).and_then(|s| s.view(me_of(self.session.as_ref())));
                    window::draw(&mut self.ui, &mut self.win, &self.bufs, v.as_ref(), cx);
                }
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
                    let (clock, day) =
                        sim_of(self.session.as_ref()).map_or((0, 0), |s| (s.state().clock, s.state().day));
                    let mut when = format!("Day {}, ", day + 1);
                    text::clock(clock, &mut when);
                    let zone = self.bufs.hud.zone_name;
                    // Alone, "Open to LAN"; hosting, where others dial; joined, nothing.
                    let lan = match &self.session {
                        Some(Session::Local(_)) => Some(("Open to LAN".to_owned(), true)),
                        Some(Session::Host(h)) => Some((format!("Hosting on port {}", h.port()), false)),
                        // The door, the saves and who may sit are the host's.
                        Some(Session::Guest(_)) => Some(("The host keeps this table".to_owned(), false)),
                        None => None,
                    };
                    let guest = matches!(self.session, Some(Session::Guest(_)));
                    let info = PauseInfo {
                        can_save: self.bufs.me.can_save && !guest,
                        when: &when,
                        zone,
                        // Only with someone else actually sitting at the table.
                        company: !self.alone()
                            && sim_of(self.session.as_ref()).is_some_and(|s| s.state().party_size() > 1),
                        lan: lan.as_ref().map(|(l, on)| (l.as_str(), *on)),
                        guest,
                    };
                    menus::pause(&mut self.ui, &mut self.menu_state, &info);
                }
                Menu::Slots(mode) => menus::slots(&mut self.ui, &mut self.menu_state, mode, &self.slot_rows),
                Menu::Host => {
                    let slots: Vec<Option<String>> =
                        self.slot_rows.iter().map(|r| (!r.empty).then(|| format!("{} · {}", r.zone, r.when))).collect();
                    let info = HostInfo { slots: &slots, port: self.lan.port };
                    lan_ui::host(&mut self.ui, &mut self.lan.host_form, &info);
                }
                Menu::Join => {
                    let status = self.lan.status.as_ref().map(|(s, bad)| (s.as_str(), *bad));
                    let info = JoinInfo { found: &self.lan.found, status, joining: self.lan.joining.is_some() };
                    lan_ui::join(&mut self.ui, &mut self.lan.join_form, &info);
                }
                Menu::Controls => {
                    let backend = self.config.backend.clone().unwrap_or_else(|| "auto".into());
                    let info = ControlsInfo {
                        assist: self.input.assist,
                        backend: &backend,
                        volumes: self.config.volumes(),
                        rows: self.present.features(),
                        tier: self.present.frame().tier,
                    };
                    let out = controls::draw(&mut self.ui, &mut self.controls, &mut self.input.bindings, info);
                    let mut save = out.bindings;
                    // A row turned: the presenter at once, the backend before the next draw.
                    if let Some((rows, key)) = out.rows {
                        self.present.set_features(rows);
                        self.config.set_feature(self.present.frame().tier, &self.present.features(), key);
                        self.features_changed = true;
                        save = true;
                    }
                    if out.bindings {
                        self.config.set_bindings(&self.input.bindings);
                    }
                    if let Some(a) = out.assist {
                        self.input.assist = a;
                        self.config.set_assist(a);
                        save = true;
                    }
                    if let Some(b) = out.backend {
                        self.config.backend = Some(b.to_owned());
                        save = true;
                    }
                    if let Some(v) = out.volumes {
                        self.config.set_volumes(v);
                        self.sound.set_volume(v);
                        save = true;
                    }
                    if save && let Err(e) = self.config.save(&self.dirs) {
                        eprintln!("jane-app: {e}");
                    }
                }
                Menu::ConfirmTitle => {
                    if let Some(yes) = menus::confirm(&mut self.ui, &mut self.menu_state, "Quit to the title?") {
                        self.menus.pop();
                        if yes {
                            self.menus.clear();
                            self.end_session();
                            self.scene = Scene::Title;
                            self.title.naming = false;
                            self.read_slots();
                        }
                    }
                }
            }
        }
        if self.console.open {
            self.ui.interactive = true;
            term::draw(&mut self.ui, &mut self.console);
        }
        if self.perf_level > 0 || self.world_dbg.on {
            self.ui.interactive = false;
            self.overlays(stats);
        }
    }

    /// A line from the terminal, run.
    fn console_line(&mut self, line: &str) {
        let me = me_of(self.session.as_ref());
        for r in crate::console::run(line, sim_of(self.session.as_ref()), me) {
            match r {
                crate::console::Run::Command(c) => {
                    self.command(c);
                    self.console.say("done", LineKind::Good);
                }
                crate::console::Run::Say(s, k) => self.console.say(&s, k),
                crate::console::Run::Save(n) => {
                    // The terminal saves anywhere: it is a dev's tool.
                    if let Some((bytes, seed)) = sim_of(self.session.as_ref()).map(|s| (s.save(), s.state().seed)) {
                        match saves::write(&self.dirs, n, &bytes) {
                            Ok(()) => {
                                self.config.set_slot_seed(n, seed);
                                let _ = self.config.save(&self.dirs);
                                self.console.say(&format!("saved to slot {}", n + 1), LineKind::Good);
                            }
                            Err(e) => self.console.say(&e, LineKind::Error),
                        }
                        self.read_slots();
                    }
                }
                crate::console::Run::Load(n) => self.load(n, None),
                crate::console::Run::Seat(seat, cmd) => {
                    let r = self.session.as_mut().map_or(Err("no world yet"), |s| s.inject(seat, cmd));
                    match r {
                        Ok(()) => self.console.say("done", LineKind::Good),
                        Err(e) => self.console.say(e, LineKind::Error),
                    }
                }
                crate::console::Run::Speed(q) => {
                    self.speed.quarters = q;
                    self.speed.held = false;
                }
                crate::console::Run::Hold => self.speed.held = true,
                crate::console::Run::Step => {
                    self.speed.held = true;
                    self.speed.step = true;
                }
                crate::console::Run::Title => {
                    self.menus.clear();
                    self.end_session();
                    self.scene = Scene::Title;
                    self.read_slots();
                }
                crate::console::Run::Clear => self.console.clear(),
            }
        }
    }

    /// F2 and the top line.
    fn overlays(&mut self, stats: Option<jane_present::FrameStats>) {
        let (seed, zone, tick, clock) = match sim_of(self.session.as_ref()) {
            Some(s) => {
                let st = s.state();
                let mut c = String::new();
                text::clock(st.clock, &mut c);
                (st.seed, self.bufs.hud.zone.map_or("-", ZoneId::name), st.tick.0, format!("day {} {c}", st.day + 1))
            }
            None => (0, "-", 0, String::new()),
        };
        if let Some(s) = sim_of(self.session.as_ref())
            && self.ticks.wrapping_sub(self.hash.1) >= 30
        {
            self.hash = (s.hash(), self.ticks);
        }
        let speed = match (self.speed.held, self.speed.quarters) {
            (true, _) => "held: F6 steps",
            (false, 1) => "x0.25",
            (false, 16) => "x4",
            _ => "",
        };
        let top = TopLine { seed, zone, tick, clock: &clock, hash: self.hash.0, speed };
        perf::top_line(&mut self.ui, &top);
        if self.perf_level == 0 {
            return;
        }
        let sim_info = self.tally.info();
        let f = self.present.frame();
        let mut passes = [None; 8];
        for (i, p) in f.passes.iter().take(8).enumerate() {
            passes[i] = Some(p.name());
        }
        let frame = FrameInfo {
            sprites: f.sprites.len() as u32,
            lights: f.lights.len() as u32,
            casters: f.casters.len() as u32,
            chunks_live: f.chunks.len() as u32,
            chunks_painted: self.present.chunks_painted(),
            ui_cmds: self.ui.cmds.len() as u32,
            atlas_pages: self.present.atlas().pages.len() as u32,
            passes,
        };
        let v = PerfView {
            log: &self.perf,
            backend: stats,
            backend_name: &self.backend_name,
            tier: f.tier,
            dropped: self.dropped,
            sim: sim_info,
            frame,
            top,
        };
        perf::draw(&mut self.ui, self.perf_level, &v);
    }
}
