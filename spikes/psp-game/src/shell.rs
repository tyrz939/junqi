//! The game around the world (PORT.md §13.13): what `jane-app`'s `App` is on the PC, for the PSP.
//! The title, the loading screen, play with the HUD, the window (bag, book, log, map), dialogue,
//! the pause menu, the save and load lists, the Controls screen; the pad to the presenter's input
//! mapper (`jane_present::input::Input`, the bindings' pad column) and its edges to the sim's
//! commands and the UI's actions; saves and their notes through [`Saves`]. No platform call: the
//! glue in `main.rs` hands it the pad, the world and the files.
//!
//! As the PC: a menu or the window holds the world (alone, as she always is here); a press in the
//! bag is still stepped, one tick with her stick idle; a rest at a bed or a fire writes the slot
//! last used; Save is offered only in reach of rest (the sim's `can_save`).

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use jane_core::ZoneId;
use jane_present::input::{Context, DeviceState, Edge, GameAction, Input, Mode, PadStyle, UiAction};
use jane_present::text;
use jane_present::ui::controls::{self, ControlsInfo, ControlsState};
use jane_present::ui::core::{AppIntent, PadPress, UiInput, UiOut};
use jane_present::ui::dialogue::{self, DialogueBox};
use jane_present::ui::hud::{self, HudCtx};
use jane_present::ui::lesson as lesson_ui;
use jane_present::ui::loading::{self, LoadingState};
use jane_present::ui::marks as quest_marks;
use jane_present::ui::menus::{self, MenuLights, MenuState, PauseInfo, SlotMode, SlotRow};
use jane_present::ui::saved as saved_ui;
use jane_present::ui::title::{self, TitleInfo, TitleState};
use jane_present::ui::window::{self, WindowState};
use jane_present::ui::{Ui, UiArt};
use jane_present::view::ViewBuffers;
use jane_present::Present;
use jane_present::{Frame, Tier};
use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame, StampedCommand, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Seat, Sim};

/// Save slots, as the PC has.
pub const SLOTS: u8 = 3;
/// The clear behind the title and the loading screen (the PC's).
pub const DARK: u32 = 0xff10_1014;

// The PSP pad and its routing onto the standard mapping live in `jane_present::pad_psp` (host
// tests drive them there).
pub use jane_present::pad_psp::{psp, PspPad};
use jane_present::pad_psp::{PspRouter, Settings};

/// Where the slots and their notes live: the Memory Stick on a PSP.
pub trait Saves {
    fn read(&mut self, n: u8) -> Option<Vec<u8>>;
    fn write(&mut self, n: u8, bytes: &[u8]) -> Result<(), String>;
    fn read_note(&mut self, n: u8) -> Option<String>;
    fn write_note(&mut self, n: u8, note: &str) -> Result<(), String>;
}

/// A slot's note (`slotN.txt` beside `slotN.jane`): what the PC keeps in `slotN.meta.json` and
/// `config.json`'s slot seeds, as lines of `key value` (no JSON library on the console). The
/// save itself is the sim's bytes, the PC's exactly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// The county's seed (the PC's `config.json` keeps it per slot).
    pub seed: u32,
    /// The order it was written in, for Continue (the PSP keeps no wall clock here).
    pub serial: u32,
    pub place: String,
    pub day: u32,
    pub clock: String,
    pub night: bool,
    pub quest: String,
    pub step: String,
    pub tracked: Vec<String>,
    pub seen: Vec<String>,
    /// The map's ink and pins: `zone x y kind burn pending text`.
    pub map: Vec<String>,
}

impl Note {
    pub fn write(&self) -> String {
        use core::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(s, "seed {}", self.seed);
        let _ = writeln!(s, "serial {}", self.serial);
        let _ = writeln!(s, "place {}", self.place);
        let _ = writeln!(s, "day {}", self.day);
        let _ = writeln!(s, "clock {}", self.clock);
        let _ = writeln!(s, "night {}", u8::from(self.night));
        let _ = writeln!(s, "quest {}", self.quest);
        let _ = writeln!(s, "step {}", self.step);
        for t in &self.tracked {
            let _ = writeln!(s, "tracked {t}");
        }
        for t in &self.seen {
            let _ = writeln!(s, "seen {t}");
        }
        for m in &self.map {
            let _ = writeln!(s, "map {m}");
        }
        s
    }

    pub fn read(s: &str) -> Note {
        let mut n = Note::default();
        for line in s.lines() {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            match k {
                "seed" => n.seed = v.parse().unwrap_or(0),
                "serial" => n.serial = v.parse().unwrap_or(0),
                "place" => n.place = v.to_owned(),
                "day" => n.day = v.parse().unwrap_or(0),
                "clock" => n.clock = v.to_owned(),
                "night" => n.night = v == "1",
                "quest" => n.quest = v.to_owned(),
                "step" => n.step = v.to_owned(),
                "tracked" => n.tracked.push(v.to_owned()),
                "seen" => n.seen.push(v.to_owned()),
                "map" => n.map.push(v.to_owned()),
                _ => {}
            }
        }
        n
    }
}

/// The map's memory as note lines (`jane-app`'s `map_rows`, as text).
fn map_lines(m: &jane_present::memory::MapMemory) -> Vec<String> {
    use jane_present::memory::{FireState, Note as N};
    let mark = |k: &jane_present::memory::MapMark, pending: bool| {
        let (kind, txt, burn) = match &k.note {
            N::Fire(FireState::Kept) => ("fire", "", 0),
            N::Fire(FireState::Made { burn }) => ("made", "", *burn),
            N::Fire(FireState::Cold) => ("cold", "", 0),
            N::Sign(t) => ("sign", t.as_str(), 0),
            N::Name(t) => ("name", t.as_str(), 0),
        };
        format!("{} {} {} {kind} {burn} {} {txt}", k.zone.name(), k.at.0, k.at.1, u8::from(pending))
    };
    let mut out: Vec<String> = m.inked.iter().map(|k| mark(k, false)).collect();
    out.extend(m.pending.iter().map(|k| mark(k, true)));
    out.extend(m.pins.iter().map(|p| format!("{} {} {} pin 0 0 ", p.zone.name(), p.at.0, p.at.1)));
    out
}

/// Note lines as the map's memory (`jane-app`'s `map_of`).
fn map_of(lines: &[String]) -> jane_present::memory::MapMemory {
    use jane_present::memory::{FireState, MapMark, MapMemory, Note as N, Pin, PINS};
    let mut m = MapMemory::default();
    for l in lines {
        let mut w = l.splitn(7, ' ');
        let (Some(z), Some(x), Some(y), Some(kind), Some(burn), Some(pending)) =
            (w.next(), w.next(), w.next(), w.next(), w.next(), w.next())
        else {
            continue;
        };
        let txt = w.next().unwrap_or("").to_owned();
        let (Some(zone), Ok(x), Ok(y)) = (ZoneId::from_name(z), x.parse::<i32>(), y.parse::<i32>()) else { continue };
        let at = (x, y);
        let note = match kind {
            "fire" => N::Fire(FireState::Kept),
            "made" => N::Fire(FireState::Made { burn: burn.parse().unwrap_or(0) }),
            "cold" => N::Fire(FireState::Cold),
            "sign" => N::Sign(txt),
            "name" => N::Name(txt),
            "pin" => {
                if m.pins.len() < PINS {
                    m.pins.push(Pin { zone, at });
                }
                continue;
            }
            _ => continue,
        };
        let k = MapMark { zone, at, note };
        if pending == "1" {
            m.pending.push(k)
        } else {
            m.inked.push(k)
        }
    }
    m
}

/// Where the player is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Title,
    Loading,
    Play,
}

/// A screen over the scene, top one last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Pause,
    Slots(SlotMode),
    ConfirmTitle,
    Overwrite(u8),
    Controls,
    Graphics,
}

/// What the shell asks the glue to do between frames: build a world, load one, leave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// New Game: a county from a seed nobody chose.
    NewGame,
    /// Load slot `n`: its bytes, and the seed its note keeps.
    Load {
        slot: u8,
        bytes: Vec<u8>,
        seed: u32,
    },
    /// Back to the title: the world let go.
    ToTitle,
    Quit,
}

/// The game around the world.
#[derive(Debug)]
pub struct Shell {
    pub ui: Ui,
    pub input: Input,
    pub dev: DeviceState,
    pub scene: Scene,
    pub loading: Option<LoadingState>,
    pub title: TitleState,
    pub menus: Vec<Menu>,
    lights: MenuLights,
    dialogue: DialogueBox,
    pub bufs: ViewBuffers,
    pub win: WindowState,
    pub win_open: bool,
    controls: ControlsState,
    gfx_page: jane_present::ui::graphics::GraphicsState,
    /// The Graphics page's settings (`settings.txt`); the glue reads them every frame.
    pub graphics: jane_present::gfx_psp::Graphics,
    /// This seat's presses, for the next step.
    pub pending: Vec<Command>,
    seq: u16,
    pub slot_rows: Vec<SlotRow>,
    /// The slot last saved to or loaded from.
    pub slot: Option<u8>,
    notes: Vec<Option<Note>>,
    pub asks: Vec<Ask>,
    /// Frames since boot: the UI's clock outside play.
    pub ticks: u32,
    pad_was: u32,
    /// What the glue says when something fails (a load refused): a toast in play, the log.
    pub said: Vec<String>,
    /// The world stepped this frame rested (a bed, a fire): the slot is written after the step.
    pub(crate) rested: bool,
    /// The glue's clock (µs), and the last step's parts: the sim, the presenter's tick, the
    /// buffers' tick.
    pub clock: Option<fn() -> u32>,
    pub times: [u32; 3],
    /// The PSP pad onto the standard mapping, with the player's pad settings.
    pub router: PspRouter,
    /// The three volumes (the Controls page's; `settings.txt`).
    pub volumes: jane_present::audio::Volumes,
    /// The Controls page changed a setting: the glue writes `settings.txt` and hears the volumes.
    pub settings_changed: bool,
    /// L + R + SELECT (SELECT pressed while both are held): the glue flips its performance
    /// overlay and clears this. That SELECT does not open the bag.
    pub perf_toggle: bool,
    /// The performance overlay's lines (empty when it is off: nothing drawn), top left over
    /// everything, in the UI's fine face.
    pub overlay: Vec<String>,
    /// L + R + START (START pressed while both are held): the glue starts a detailed capture
    /// (PORT.md §13.13) and clears this. That START does not pause.
    pub capture_toggle: bool,
    /// L + R + SQUARE: the glue saves the screen to the stick and clears this.
    pub screen_toggle: bool,
    square_masked: bool,
    /// A line over everything, top middle (the capture's "Capturing...").
    pub banner: String,
    start_masked: bool,
    /// The raw buttons last frame, and SELECT held since it toggled the overlay (kept from the
    /// mapper until it is let go).
    raw_was: u32,
    select_masked: bool,
}

impl Shell {
    pub fn new(art: UiArt) -> Shell {
        let mut ui = Ui::new(art);
        ui.pad_style = PadStyle::Psp;
        ui.move_images = true;
        let mut input = Input::new();
        input.assist = Some(jane_sim::input::AssistProfile::Pad);
        Shell {
            ui,
            input,
            dev: DeviceState::default(),
            scene: Scene::Title,
            loading: None,
            title: TitleState { name: "Jane".to_owned(), ..TitleState::default() },
            menus: Vec::new(),
            lights: MenuLights::default(),
            dialogue: DialogueBox::default(),
            bufs: ViewBuffers::new(),
            win: WindowState::default(),
            win_open: false,
            controls: ControlsState::default(),
            gfx_page: jane_present::ui::graphics::GraphicsState::default(),
            graphics: jane_present::gfx_psp::Graphics::default(),
            pending: Vec::new(),
            seq: 0,
            slot_rows: Vec::new(),
            slot: None,
            notes: Vec::new(),
            asks: Vec::new(),
            ticks: 0,
            pad_was: 0,
            said: Vec::new(),
            rested: false,
            clock: None,
            times: [0; 3],
            router: PspRouter::default(),
            volumes: jane_present::audio::Volumes::default(),
            settings_changed: false,
            perf_toggle: false,
            overlay: Vec::new(),
            capture_toggle: false,
            screen_toggle: false,
            square_masked: false,
            banner: String::new(),
            start_masked: false,
            raw_was: 0,
            select_masked: false,
        }
    }

    /// The settings in force (`settings.txt`).
    pub fn settings(&self) -> Settings {
        Settings { pad: self.router.settings, volumes: self.volumes, graphics: self.graphics }
    }

    /// Takes the settings read at boot (or a script's).
    pub fn set_settings(&mut self, s: Settings) {
        self.router.settings = s.pad;
        self.volumes = s.volumes;
        self.graphics = s.graphics;
        self.ui.pad_style = s.pad.style();
    }

    /// Who has the pad.
    pub fn mode(&self) -> Mode {
        match self.scene {
            Scene::Play if self.menus.is_empty() && !self.talking() && !self.win_open => Mode::Play,
            _ => Mode::Ui,
        }
    }

    fn talking(&self) -> bool {
        self.bufs.dialogue.is_some() && self.scene == Scene::Play
    }

    /// The world is held: a menu or the window is up, or a spell just learned holds its breath.
    pub fn world_held(&self, present: Option<&Present>) -> bool {
        !self.menus.is_empty() || self.win_open || present.is_some_and(|p| p.lessons().holds_world(true))
    }

    /// The slots as the lists show them, read through `saves`.
    pub fn read_slots(&mut self, saves: &mut dyn Saves) {
        self.notes = (0..SLOTS).map(|n| saves.read_note(n).map(|s| Note::read(&s))).collect();
        let latest = self.latest();
        self.slot_rows = (0..SLOTS)
            .map(|n| {
                let Some(bytes) = saves.read(n) else { return SlotRow { empty: true, ..SlotRow::default() } };
                let Ok((h, _)) = jane_sim::save::read_header(&bytes) else {
                    return SlotRow { empty: true, ..SlotRow::default() };
                };
                let s = &h.summary;
                let mut row = SlotRow {
                    empty: false,
                    zone: text::zone_name(s.zone, jane_data::Region::Lowfields).into(),
                    when: format!("Day {} · {:02}:00", s.day + 1, s.hour),
                    night: !(6..18).contains(&s.hour),
                    hp: format!("{} of {}", s.hp.points(), s.max_hp.points()),
                    latest: latest == Some(n),
                    ..SlotRow::default()
                };
                if let Some(m) = self.notes.get(usize::from(n)).and_then(Option::as_ref) {
                    row.when = format!("Day {} · {}", m.day, m.clock);
                    row.zone.clone_from(&m.place);
                    row.night = m.night;
                    row.quest.clone_from(&m.quest);
                    row.step.clone_from(&m.step);
                }
                row
            })
            .collect();
    }

    /// The slot written last (its note's serial), else the first with a save.
    fn latest(&self) -> Option<u8> {
        self.notes
            .iter()
            .enumerate()
            .filter_map(|(n, m)| m.as_ref().map(|m| (n as u8, m.serial)))
            .max_by_key(|e| e.1)
            .map(|e| e.0)
    }

    fn has_save(&self) -> bool {
        self.slot_rows.iter().any(|r| !r.empty)
    }

    /// The UI's input this frame from the PSP pad (and the script's presses, ORed in by the
    /// glue): the held frame for the sim, and her presses queued as edges, handled here.
    pub fn sample(&mut self, p: PspPad, sim: Option<&Sim>) -> (InputFrame, UiInput) {
        let mode = self.mode();
        // L + R + SELECT: the performance overlay, not the bag (SELECT kept from the mapper
        // until it is let go).
        let mut p = p;
        let rose = p.buttons & !self.raw_was;
        self.raw_was = p.buttons;
        let lr = psp::L | psp::R;
        if rose & psp::SELECT != 0 && p.buttons & lr == lr {
            self.perf_toggle = true;
            self.select_masked = true;
        }
        if p.buttons & psp::SELECT == 0 {
            self.select_masked = false;
        }
        if self.select_masked {
            p.buttons &= !psp::SELECT;
        }
        // L + R + START: a detailed capture, not the pause.
        if rose & psp::START != 0 && p.buttons & lr == lr {
            self.capture_toggle = true;
            self.start_masked = true;
        }
        if p.buttons & psp::START == 0 {
            self.start_masked = false;
        }
        if self.start_masked {
            p.buttons &= !psp::START;
        }
        // L + R + SQUARE: the screen to the stick, not bar 2.
        if rose & psp::SQUARE != 0 && p.buttons & lr == lr {
            self.screen_toggle = true;
            self.square_masked = true;
        }
        if p.buttons & psp::SQUARE == 0 {
            self.square_masked = false;
        }
        if self.square_masked {
            p.buttons &= !psp::SQUARE;
        }
        self.dev.pad = Some(self.router.route(p, mode));
        let mut held = self.input.sample(&self.dev, &Context { mode, feet: None });
        let pad_now = self.dev.pad.map_or(0, |p| p.held);
        let pad_pressed = PadPress { buttons: pad_now & !self.pad_was, lt: false, rt: false };
        self.pad_was = pad_now;
        self.dev.end_sample();
        if let (Some(st), true) = (self.loading.as_mut(), pad_pressed.buttons != 0) {
            st.press();
        }
        let edges: Vec<Edge> = self.input.drain().collect();
        let mut actions = Vec::new();
        let view = sim.and_then(|s| s.view(Seat(0)));
        for e in edges {
            self.edge(e, view.as_ref(), &mut actions);
        }
        if let Some(v) = view.as_ref() {
            jane_present::target::settle(&mut self.input, v);
        }
        if mode == Mode::Play {
            held.target = self.input.target;
        }
        let ui_input = UiInput { pad_pressed, actions, pad: true, ..UiInput::default() };
        (held, ui_input)
    }

    /// One press, as the PC's app hears it (no pointer: no click to select or walk).
    fn edge(&mut self, edge: Edge, view: Option<&jane_sim::View<'_>>, actions: &mut Vec<UiAction>) {
        match edge {
            Edge::Game(g) => {
                if self.scene != Scene::Play || !self.menus.is_empty() {
                    return;
                }
                let cmd = match g {
                    GameAction::Use => Command::Use,
                    GameAction::Hop => Command::Hop,
                    GameAction::Bar(slot) => Command::Bar { slot, on: None },
                    GameAction::Tab { back } => {
                        if let Some(v) = view {
                            jane_present::target::tab(&mut self.input, v, None, back);
                        }
                        return;
                    }
                    GameAction::Select | GameAction::Goto => return,
                };
                self.pending.push(cmd);
            }
            Edge::Ui(a) => match a {
                UiAction::Pause => {
                    // START first lets go of her target and stops what she is doing; with
                    // nothing to stop, it pauses (as Esc on the PC).
                    let busy = view.is_some_and(jane_present::target::busy);
                    if self.scene == Scene::Play && self.menus.is_empty() && (busy || self.input.target.is_some()) {
                        jane_present::target::clear(&mut self.input);
                        if busy {
                            self.pending.push(Command::Halt);
                        }
                    } else if self.scene == Scene::Play && self.menus.is_empty() {
                        self.menus.push(Menu::Pause);
                        self.lights.push(MenuState::default());
                    }
                }
                UiAction::Cancel => {
                    if self.ui.popover_open() {
                        actions.push(a);
                    } else if !self.menus.is_empty() {
                        self.menus.pop();
                        self.lights.pop();
                    } else if self.win_open {
                        if self.win.asking() {
                            actions.push(a);
                        } else {
                            self.win_open = false;
                        }
                    } else {
                        actions.push(a);
                    }
                }
                UiAction::Bags => self.window_key(0),
                UiAction::Book => self.window_key(1),
                UiAction::Quests => self.window_key(2),
                UiAction::Map => self.window_key(3),
                UiAction::Console
                | UiAction::Debug
                | UiAction::Grid
                | UiAction::Shot
                | UiAction::Step
                | UiAction::Slow
                | UiAction::Fast
                | UiAction::QuickSave
                | UiAction::QuickLoad => {}
                _ => actions.push(a),
            },
        }
    }

    fn window_key(&mut self, tab: usize) {
        if self.scene != Scene::Play || !self.menus.is_empty() {
            return;
        }
        if self.win_open && self.win.tab == tab {
            self.win_open = false;
        } else {
            self.win_open = true;
            self.win.tab = tab;
        }
    }

    /// Steps the world one tick (or holds it), the presenter and the buffers fed. Returns
    /// whether it stepped.
    pub fn step(&mut self, sim: &mut Sim, present: &mut Present, held: InputFrame, events: &mut Vec<Event>) -> bool {
        let paused = self.world_held(Some(present));
        if paused && self.pending.is_empty() {
            if let Some(v) = sim.view(Seat(0)) {
                present.tick(&v, &[]);
            }
            return false;
        }
        let frame = if paused { InputFrame::IDLE } else { held };
        let cmds: Vec<StampedCommand> = self
            .pending
            .drain(..)
            .map(|cmd| {
                self.seq = self.seq.wrapping_add(1);
                StampedCommand { seat: Some(Seat::HOST), seq: self.seq, cmd }
            })
            .collect();
        let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
        frames[0] = frame;
        let now = || self.clock.map_or(0, |c| c());
        let t0 = now();
        sim.step(&StepInput { frames, commands: &cmds });
        let t1 = now();
        events.clear();
        events.extend_from_slice(sim.drain_events());
        if events.iter().any(|e| e.kind == jane_sim::EventKind::Rest) {
            self.rested = true;
        }
        if let Some(v) = sim.view(Seat(0)) {
            present.tick(&v, events);
            let t2 = now();
            self.bufs.tick(&v, events);
            let t3 = now();
            self.times = [t1.wrapping_sub(t0), t2.wrapping_sub(t1), t3.wrapping_sub(t2)];
            // Use on a cupboard or a bench opens the window on her bag; closing it lets go.
            if self.bufs.window.take_opened() && self.menus.is_empty() {
                self.win_open = true;
                self.win.tab = 0;
            }
            if !self.win_open {
                self.bufs.window.close_store();
            }
        }
        true
    }

    /// After the frame's steps: a rest writes the slot last used (the PC's autosave).
    pub fn after_steps(&mut self, sim: &Sim, saves: &mut dyn Saves) {
        if core::mem::take(&mut self.rested) {
            let n = self.slot.unwrap_or(0);
            self.write_slot(n, sim, saves);
        }
    }

    /// Writes slot `n`: the sim's save bytes (the PC's), then its note.
    fn write_slot(&mut self, n: u8, sim: &Sim, saves: &mut dyn Saves) {
        let bytes = sim.save();
        match saves.write(n, &bytes) {
            Ok(()) => {
                let note = self.note(sim);
                let _ = saves.write_note(n, &note.write());
                self.slot = Some(n);
                self.bufs.saved(&format!("Saved to slot {}", n + 1), true);
                self.read_slots(saves);
            }
            Err(e) => {
                self.bufs.saved(&format!("Couldn't save: {e}"), false);
                self.said.push(e);
            }
        }
    }

    /// The note beside a save of `sim`, read off the buffers as they are now.
    fn note(&self, sim: &Sim) -> Note {
        let h = &self.bufs.hud;
        let line = h.tracker.first();
        let (tracked, seen) = self.bufs.track.ids();
        Note {
            seed: sim.state().seed,
            serial: self.notes.iter().flatten().map(|m| m.serial).max().unwrap_or(0) + 1,
            place: h.zone_name.to_owned(),
            day: h.day,
            clock: h.clock.clone(),
            night: h.night,
            quest: line.map(|l| l.title.clone()).unwrap_or_default(),
            step: line.map(|l| l.step.clone()).unwrap_or_default(),
            tracked,
            seen,
            map: map_lines(&self.bufs.memory),
        }
    }

    /// Into play with a world just built or loaded (`slot`: the one it came from).
    pub fn begin_play(&mut self, slot: Option<u8>) {
        self.scene = Scene::Play;
        self.loading = None;
        self.bufs = ViewBuffers::new();
        self.bufs.no_ways = true;
        self.dialogue.reset();
        self.menus.clear();
        self.lights.clear();
        self.pending.clear();
        self.win_open = false;
        self.win = WindowState::default();
        // The county's chart at 256 px a side at most (8 cells a px): 1 MB less than the PC's.
        self.win.map.most_px = 256;
        self.input.target = None;
        if let Some(n) = slot {
            self.slot = Some(n);
            if let Some(m) = self.notes.get(usize::from(n)).and_then(Option::as_ref) {
                if !m.tracked.is_empty() || !m.seen.is_empty() {
                    self.bufs.track = jane_present::view::Tracking::from_ids(&m.tracked, &m.seen);
                }
                self.bufs.memory = map_of(&m.map);
            }
        }
        // The title's backdrop and the loading card are let go: play needs the RAM.
        self.ui.drop_images();
    }

    /// The loading screen for a build under way (`verb`: "New Game" or "Load").
    pub fn begin_loading(&mut self, seed: u32, verb: &'static str) {
        self.scene = Scene::Loading;
        self.menus.clear();
        self.lights.clear();
        self.win_open = false;
        self.ui.drop_images();
        self.loading = Some(LoadingState::new(loading::Mode::Scroll, seed, verb, self.ticks));
    }

    /// Back at the title (the world let go by the glue).
    pub fn to_title(&mut self, saves: &mut dyn Saves) {
        self.scene = Scene::Title;
        self.loading = None;
        self.menus.clear();
        self.lights.clear();
        self.win_open = false;
        self.title.naming = false;
        self.title.painted = (0, 0);
        self.bufs = ViewBuffers::new();
        self.read_slots(saves);
    }

    /// Draws the UI's layers into `frame`, bottom to top; only the top one answers.
    pub fn draw(&mut self, input: UiInput, present: Option<&Present>, sim: Option<&Sim>, frame: &mut Frame) {
        let canvas = frame.canvas;
        let tick = present.map_or(self.ticks, Present::ticks);
        self.ui.begin(input, tick, canvas);
        let view = sim.and_then(|s| s.view(Seat(0)));
        let cx = HudCtx {
            bindings: &self.input.bindings,
            pad: true,
            window_open: self.win_open,
            style: self.router.settings.style(),
        };
        match self.scene {
            Scene::Title => {
                self.ui.interactive = self.menus.is_empty();
                let info = TitleInfo { has_save: self.has_save(), console: true };
                title::draw(&mut self.ui, &mut self.title, info);
            }
            Scene::Loading => {
                self.ui.interactive = false;
                if let Some(st) = self.loading.as_mut() {
                    loading::draw(&mut self.ui, st);
                }
            }
            Scene::Play => {
                if let Some(p) = present {
                    self.ui.interactive = false;
                    quest_marks::draw(&mut self.ui, p.marks(), p.dark(), p.ticks());
                    jane_present::ui::fight::draw(&mut self.ui, p.fight());
                    quest_marks::draw_emotes(&mut self.ui, p.emotes(), p.ticks());
                }
                self.ui.interactive = self.menus.is_empty() && self.bufs.dialogue.is_none();
                hud::draw(&mut self.ui, &self.bufs, cx);
                saved_ui::draw(&mut self.ui, &self.bufs);
                if let Some(p) = present {
                    lesson_ui::draw(&mut self.ui, p.lessons(), &self.bufs, self.win_open);
                }
                if self.win_open && self.bufs.dialogue.is_none() {
                    self.ui.interactive = self.menus.is_empty();
                    window::draw(&mut self.ui, &mut self.win, &self.bufs, view.as_ref(), cx);
                    for (zone, at) in core::mem::take(&mut self.win.map.pin_edits) {
                        if self.bufs.memory.toggle_pin(zone, at) == jane_present::memory::PinEdit::Full {
                            let s = format!("All {} pins are on the map", jane_present::memory::PINS);
                            self.bufs.push_toast(&s, jane_present::text::Tone::Refused);
                        }
                    }
                }
                if let Some(d) = &self.bufs.dialogue {
                    self.ui.interactive = self.menus.is_empty();
                    dialogue::draw(&mut self.ui, &mut self.dialogue, d, cx);
                } else {
                    self.dialogue.reset();
                }
            }
        }
        let n = self.menus.len();
        self.lights.sync(n);
        for (k, m) in self.menus.clone().into_iter().enumerate() {
            self.ui.interactive = k + 1 == n;
            match m {
                Menu::Pause => {
                    let (clock, day) = sim.map_or((0, 0), |s| (s.state().clock, s.state().day));
                    let mut when = format!("Day {}, ", day + 1);
                    text::clock(clock, &mut when);
                    let info = PauseInfo {
                        can_save: self.bufs.me.can_save,
                        when: &when,
                        zone: self.bufs.hud.zone_name,
                        company: false,
                        lan: None,
                        guest: false,
                        graphics: true,
                    };
                    menus::pause(&mut self.ui, self.lights.layer(k), &info);
                }
                Menu::Slots(mode) => menus::slots(&mut self.ui, self.lights.layer(k), mode, &self.slot_rows),
                Menu::Controls => {
                    let info = ControlsInfo {
                        assist: self.input.assist,
                        backend: "ge",
                        volumes: self.volumes,
                        rows: present.map_or_else(|| jane_present::Features::c2(), Present::features),
                        tier: Tier::T0,
                    };
                    let out = controls::draw_console(
                        &mut self.ui,
                        &mut self.controls,
                        &self.input.bindings,
                        info,
                        self.router.settings,
                    );
                    if let Some(pad) = out.pad {
                        self.router.settings = pad;
                        self.ui.pad_style = pad.style();
                        self.settings_changed = true;
                    }
                    if let Some(v) = out.volumes {
                        self.volumes = v;
                        self.settings_changed = true;
                    }
                }
                Menu::Graphics => {
                    if let Some(g) =
                        jane_present::ui::graphics::draw_console(&mut self.ui, &mut self.gfx_page, self.graphics)
                    {
                        self.graphics = g;
                        self.settings_changed = true;
                    }
                }
                Menu::Overwrite(n) => {
                    let row = self.slot_rows.get(usize::from(n)).cloned().unwrap_or_default();
                    let question = format!("Save over slot {}?", n + 1);
                    let detail = menus::overwrite_detail(&row);
                    let ask = menus::Ask { question: &question, detail: &detail, yes: "Save over", no: "Keep it" };
                    if let Some(yes) = menus::ask(&mut self.ui, self.lights.layer(k), &ask) {
                        self.menus.pop();
                        self.lights.pop();
                        if yes {
                            self.ui.intent(AppIntent::Save(n));
                        }
                    }
                }
                Menu::ConfirmTitle => {
                    if let Some(yes) = menus::confirm(&mut self.ui, self.lights.layer(k), "Quit to the title?") {
                        self.menus.pop();
                        self.lights.pop();
                        if yes {
                            self.asks.push(Ask::ToTitle);
                        }
                    }
                }
            }
        }
        if !self.overlay.is_empty() {
            use jane_present::ui::core::Ink;
            use jane_present::ui::{style, Rect};
            // The fine face's cell: 8 x 12.
            let lh = 12;
            let w = self.overlay.iter().map(|l| l.chars().count() as i32 * 8).max().unwrap_or(0) + 6;
            let (x, y) = (2, 44);
            self.ui.fill(Rect::new(x, y, w, lh * self.overlay.len() as i32 + 4), 0xb000_0000);
            for (k, l) in self.overlay.iter().enumerate() {
                self.ui.text(x + 3, y + 2 + k as i32 * lh, l, Ink::fine(style::text_bright()));
            }
        }
        if !self.banner.is_empty() {
            use jane_present::ui::core::Ink;
            use jane_present::ui::{style, Rect};
            let w = self.banner.chars().count() as i32 * 8 + 10;
            let x = (i32::from(canvas.0) - w) / 2;
            self.ui.fill(Rect::new(x, 2, w, 16), 0xc000_0000);
            self.ui.text(x + 5, 4, &self.banner, Ink::fine(style::gold()));
        }
        self.ui.finish(frame);
        // The map closed: its chart let go, here and on the GE (painted again when it opens).
        let map_shown = self.scene == Scene::Play && self.win_open && self.win.tab == 3;
        if !map_shown && self.win.map.held() {
            self.win.map.release();
            if let Some(im) = frame.ui_images.get_mut(usize::from(jane_present::ui::map::CHART)) {
                *im = jane_present::ui::UiImage::default();
            }
        }
    }

    /// What the UI handed out this frame: commands for the sim, intents for the shell.
    pub fn outs(&mut self, sim: Option<&Sim>, saves: &mut dyn Saves) {
        for out in core::mem::take(&mut self.ui.out) {
            match out {
                UiOut::Command(c) => {
                    if sim.is_some() {
                        self.pending.push(c);
                    }
                }
                UiOut::Intent(i) => self.intent(i, sim, saves),
            }
        }
    }

    fn intent(&mut self, i: AppIntent, sim: Option<&Sim>, saves: &mut dyn Saves) {
        match i {
            AppIntent::NewGame { .. } => self.asks.push(Ask::NewGame),
            AppIntent::Continue => {
                if let Some(n) = self.latest().or_else(|| (0..SLOTS).find(|&n| !self.slot_rows[usize::from(n)].empty)) {
                    self.load(n, saves);
                }
            }
            AppIntent::Load(n) => self.load(n, saves),
            AppIntent::Save(n) => {
                if let Some(s) = sim {
                    if self.bufs.me.can_save {
                        self.write_slot(n, s, saves);
                    } else {
                        self.bufs.push_toast(menus::REST_TO_SAVE, jane_present::text::Tone::Refused);
                    }
                }
                self.menus.retain(|m| !matches!(m, Menu::Slots(_) | Menu::Overwrite(_)));
                self.lights.sync(self.menus.len());
            }
            AppIntent::Overwrite(n) => {
                self.menus.push(Menu::Overwrite(n));
                self.lights.push(MenuState { focus: 1 });
            }
            AppIntent::LoadMenu => {
                self.read_slots(saves);
                self.menus.push(Menu::Slots(SlotMode::Load));
                self.lights.push(MenuState::default());
            }
            AppIntent::SaveMenu => {
                self.read_slots(saves);
                self.menus.push(Menu::Slots(SlotMode::Save));
                self.lights.push(MenuState::default());
            }
            AppIntent::ToTitle => {
                self.menus.push(Menu::ConfirmTitle);
                self.lights.push(MenuState { focus: 1 });
            }
            AppIntent::Quit => self.asks.push(Ask::Quit),
            AppIntent::Resume => {
                self.menus.clear();
                self.lights.clear();
            }
            AppIntent::Controls => {
                self.menus.push(Menu::Controls);
                self.lights.push(MenuState::default());
                self.controls = ControlsState::default();
            }
            AppIntent::Graphics => {
                self.menus.push(Menu::Graphics);
                self.lights.push(MenuState::default());
                self.gfx_page = jane_present::ui::graphics::GraphicsState::default();
            }
            AppIntent::Pause => {
                if self.menus.is_empty() {
                    self.menus.push(Menu::Pause);
                    self.lights.push(MenuState::default());
                }
            }
            AppIntent::OpenWindow(tab) => self.window_key(usize::from(tab)),
            AppIntent::CloseWindow => self.win_open = false,
            AppIntent::Track(q) => self.bufs.track.toggle(q),
            AppIntent::Back => {
                self.menus.pop();
                self.lights.pop();
            }
            _ => {}
        }
    }

    /// Load slot `n`: its bytes and its note's seed to the glue, which rebuilds the county.
    fn load(&mut self, n: u8, saves: &mut dyn Saves) {
        let Some(bytes) = saves.read(n) else {
            self.said.push(format!("slot {}: nothing to load", n + 1));
            return;
        };
        let seed = self.notes.get(usize::from(n)).and_then(Option::as_ref).map(|m| m.seed);
        let Some(seed) = seed else {
            self.said.push(format!("slot {}: no note to say its county's seed", n + 1));
            self.bufs.push_toast("That save's note is missing", jane_present::text::Tone::Refused);
            return;
        };
        self.menus.clear();
        self.lights.clear();
        self.asks.push(Ask::Load { slot: n, bytes, seed });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_reads_back() {
        let n = Note {
            seed: 7,
            serial: 3,
            place: "The Lowfields".into(),
            day: 2,
            clock: "21:14".into(),
            night: true,
            quest: "A Letter".into(),
            step: "Go to the house".into(),
            tracked: alloc::vec!["a".into(), "b".into()],
            seen: alloc::vec!["c".into()],
            map: alloc::vec!["county 1 2 sign 0 0 The way to Castle".into()],
        };
        assert_eq!(Note::read(&n.write()), n);
    }
}
