//! `jane sheet scene` (PRESENTATION.md §6): one whole frame, headless. A player model plays a
//! seed from New Game as `jane play` does, the presenter ticks beside it every frame, and at the
//! chosen point one frame is drawn through `soft` (or, with the `gpu` feature, `gl2` or `wgpu`) and
//! written as a PNG. `jane bench frames` plays to the same point and times frames there.

use std::time::Instant;

use jane_bot::{Bot, Host, Model};
use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::DevOp;
use jane_sim::{Blueprints, Command, Event, InputFrame, Seat, Sim, StampedCommand, StepInput, Stepped, View};

/// The backend a frame is drawn through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Soft,
    Gl2,
    Wgpu,
}

impl Which {
    pub fn parse(s: &str) -> Option<Which> {
        match s {
            "soft" | "t0" => Some(Which::Soft),
            "gl2" | "t1" => Some(Which::Gl2),
            "wgpu" | "t2" => Some(Which::Wgpu),
            _ => None,
        }
    }

    /// The tier its frames are built for.
    pub fn tier(self) -> Tier {
        match self {
            Which::Soft => Tier::T0,
            Which::Gl2 => Tier::T1,
            Which::Wgpu => Tier::T2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Which::Soft => "t0",
            Which::Gl2 => "t1",
            Which::Wgpu => "t2",
        }
    }
}

/// `gl2`'s settings from the command line (PRESENTATION.md §1.3's rows, and the context's API):
/// each `None` keeps what the backend chose for the machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlOpts {
    /// Ask for OpenGL ES 2.0 (what the Pi runs) instead of OpenGL 2.1.
    pub es: bool,
    /// Point lights that cast (`shadows`; 0 is the row off).
    pub shadows: Option<u8>,
    /// The light target at half the canvas each way.
    pub half_light: Option<bool>,
    /// The exact albedo pass, or the fast one.
    pub exact: Option<bool>,
    /// `normal_light`.
    pub normals: Option<bool>,
    /// Debug (PRESENTATION.md §1.7): `--layers` writes the frame's terrain and sprite heights and
    /// the T2 height field beside the shot; `--show-sun` draws wgpu's sun term alone (red: the
    /// sun that reaches a px through the field, green: its N dot L, blue: the albedo).
    pub layers: bool,
    pub show_sun: bool,
}

impl GlOpts {
    /// Reads `--es`, `--shadows N|off`, `--half-light`, `--full-light`, `--fast`, `--exact`, `--flat`.
    pub fn parse(args: &[String]) -> Result<GlOpts, String> {
        let has = |f: &str| args.iter().any(|a| a == f);
        let shadows = match args.iter().position(|a| a == "--shadows").and_then(|i| args.get(i + 1)) {
            Some(v) if v == "off" => Some(0),
            Some(v) => Some(v.parse::<u8>().map_err(|_| format!("--shadows: a count or off, not {v}"))?),
            None => None,
        };
        let pick = |on: &str, off: &str| {
            if has(on) {
                Some(true)
            } else if has(off) {
                Some(false)
            } else {
                None
            }
        };
        Ok(GlOpts {
            es: has("--es"),
            shadows,
            half_light: pick("--half-light", "--full-light"),
            exact: pick("--exact", "--fast"),
            normals: if has("--flat") { Some(false) } else { None },
            layers: has("--layers"),
            show_sun: has("--show-sun"),
        })
    }

    #[cfg(feature = "gpu")]
    fn apply(self, g: &mut jane_render_gl2::Gl2) {
        let mut r = g.rows();
        r.shadows = self.shadows.unwrap_or(r.shadows);
        r.half_light = self.half_light.unwrap_or(r.half_light);
        r.exact = self.exact.unwrap_or(r.exact);
        r.normal_light = self.normals.unwrap_or(r.normal_light);
        g.set_rows(r);
    }

    #[cfg(feature = "gpu")]
    fn api(self) -> jane_render_gl2::Api {
        if self.es { jane_render_gl2::Api::Es } else { jane_render_gl2::Api::Auto }
    }
}

/// The backend, headless.
pub fn backend(which: Which, gl: GlOpts) -> Result<Box<dyn Backend>, String> {
    let _ = gl;
    match which {
        Which::Soft => Ok(Box::new(Soft::new())),
        #[cfg(feature = "gpu")]
        Which::Gl2 => {
            let mut g = jane_render_gl2::Gl2::headless(gl.api())?;
            gl.apply(&mut g);
            Ok(Box::new(g))
        }
        #[cfg(feature = "gpu")]
        Which::Wgpu => {
            let mut w = jane_render_wgpu::Wgpu::headless()?;
            w.show_sun(gl.show_sun);
            Ok(Box::new(w))
        }
        #[cfg(not(feature = "gpu"))]
        Which::Gl2 | Which::Wgpu => Err(format!(
            "{}: this jane was built without the gpu feature (cargo build -p jane-cli --features gpu)",
            which.name()
        )),
    }
}

/// What to render.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: u32,
    /// Frames (ticks) the model plays before the frame is drawn.
    pub ticks: u32,
    pub model: Model,
    /// Set the clock to this hour after the play (`--night` is 22).
    pub hour: Option<u8>,
    /// Then let this many minutes of the clock pass, the world idle (18:40 is hour 18, 40).
    pub minute: u8,
    pub canvas: (u16, u16),
    pub backend: Which,
    /// After the play, travel to this zone (by name), at this named mark or the zone's way in,
    /// with god on and the world let settle a second: a frame inside a dungeon. A name that is
    /// no zone is a mark of the county (`--at lake_bank`).
    pub at: Option<(String, Option<String>)>,
    /// Hold the sky to this weather, the ground wet as after an hour of it (`--weather rain`).
    pub weather: Option<jane_present::WeatherKind>,
    /// Cast this spell east after the rest, and draw the frame so many ticks later (`--cast icebolt:12`).
    pub cast: Option<String>,
    /// After the rest, she walks up to the unit of this catalog name and talks to it, and the
    /// frame is drawn as its first line is said (`--talk town_sweeper`): the emotes (ART-PLAN B3).
    pub talk: Option<String>,
    /// With `--cast`: first put this unit (by its catalog name) a few cells east of her, the
    /// console's `spawn`, so the bolt has a body to hit (`--spawn skeleton`).
    pub spawn: Option<String>,
    /// Quests given her before the frame (`--quest the_last_name,roberts_cap`): the marks and
    /// sparkles of a quest under way (PRESENTATION.md §3.8).
    pub quests: Vec<String>,
    /// `Features` rows set by key (`--rows fog=off,god_rays=off`, PRESENTATION.md §1.3).
    pub rows: Vec<(String, String)>,
    pub gl: GlOpts,
    /// A lesson's moment to look at (PRESENTATION.md §2.1, §3.2).
    pub lesson: LessonOpts,
    /// `--film` only: she walks each way for so many ticks in turn, then stands (`--walk
    /// south:60,east:600`), the camera with her; a `push-` leg leans on USE as she goes, pushing
    /// what she walks into (`--walk push-east:600`: the Burial's great torch).
    pub walk: Vec<(jane_core::Angle, u32, bool)>,
}

/// `--knows`, `--learn`, `--grow` and `--ui`: a spell learned (or a jar found) after the rest,
/// its moment filmed with the HUD and its card drawn over the frame.
#[derive(Clone, Debug, Default)]
pub struct LessonOpts {
    /// Spells she knows already, learned out of sight (the presenter never hears of them): so a
    /// `--learn` after them is not her first.
    pub knows: Vec<String>,
    /// Spells learned after the rest, all in one tick (their moments queue).
    pub learn: Vec<String>,
    /// A jar or a page's moment (`strength` or `spirit`): the presenter is told of it as the
    /// sim tells it of a finding; the sim is not touched.
    pub grow: Option<jane_core::action::Stat>,
    /// Draw the HUD and the lesson's card over the frame.
    pub ui: bool,
}

impl LessonOpts {
    /// Reads `--knows a,b`, `--learn a,b`, `--grow strength|spirit` and `--ui`.
    pub fn parse(args: &[String]) -> Result<LessonOpts, String> {
        let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
        let list = |name: &str| -> Vec<String> {
            flag(name)
                .map(|s| s.split(',').map(|x| x.trim().to_owned()).filter(|x| !x.is_empty()).collect())
                .unwrap_or_default()
        };
        let grow = match flag("--grow") {
            None => None,
            Some("strength") => Some(jane_core::action::Stat::Strength),
            Some("spirit") => Some(jane_core::action::Stat::Spirit),
            Some(g) => return Err(format!("--grow: strength or spirit, not {g}")),
        };
        let (knows, learn) = (list("--knows"), list("--learn"));
        let ui = args.iter().any(|a| a == "--ui") || !learn.is_empty() || grow.is_some();
        Ok(LessonOpts { knows, learn, grow, ui })
    }
}

/// The HUD and the lesson's card drawn over a scene's frame, from the buffers it keeps.
struct Hud {
    ui: jane_present::ui::Ui,
    bufs: jane_present::view::ViewBuffers,
}

impl Hud {
    fn new(present: &Present) -> Hud {
        Hud { ui: jane_present::ui::Ui::new(present.ui_art().clone()), bufs: jane_present::view::ViewBuffers::new() }
    }

    /// Draws the HUD and the moment into the presenter's last frame.
    fn draw(&mut self, present: &mut Present, canvas: (u16, u16)) {
        let bind = jane_present::input::Bindings::default();
        let cx = jane_present::ui::hud::HudCtx { bindings: &bind, pad: false, window_open: false };
        self.ui.begin(jane_present::ui::core::UiInput::default(), self.bufs.tick, canvas);
        jane_present::ui::marks::draw(&mut self.ui, present.marks(), present.dark(), present.ticks());
        jane_present::ui::marks::draw_emotes(&mut self.ui, present.emotes(), present.ticks());
        jane_present::ui::hud::draw(&mut self.ui, &self.bufs, cx);
        jane_present::ui::lesson::draw(&mut self.ui, present.lessons(), &self.bufs, false);
        self.ui.finish(present.frame_mut());
    }
}

/// The open cell nearest the middle of a rect of `zone` named `<zone>_<asked>_room` (a generated
/// dungeon's room by its node) or `asked`, if there is one and `asked` is no mark.
fn room_in(sim: &Sim, zone: jane_core::ids::ZoneId, asked: &str) -> Option<jane_core::num::Vec2> {
    // A cell (`--at burial:40,12`): a board by a door, a gate.
    if let Some((x, y)) = asked.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))) {
        return Some(jane_core::num::Vec2::centre(x, y));
    }
    let bp = sim.blueprint(zone);
    let syms = &sim.state().syms;
    let name_of = |k: &jane_core::Key| match *k {
        jane_core::Key::Name(n) => Some(syms.name(jane_sim::sym::of_name(n)).to_owned()),
        jane_core::Key::Local(_) => None,
    };
    if bp.marks.keys().any(|k| name_of(k).as_deref() == Some(asked)) {
        return None;
    }
    // A node's floor is its template's first rect, under whatever name the mission bound it to.
    let props = jane_data::catalog();
    let floors: Vec<jane_core::Key> = props
        .dungeons
        .mission_of(zone)
        .and_then(|m| m.nodes.iter().find(|n| n.id == asked))
        .map(|n| n.names.iter().filter_map(|t| t.rects.first()).map(|&r| jane_core::Key::Name(r)).collect())
        .unwrap_or_default();
    let r = bp
        .rects
        .iter()
        .find(|(k, _)| floors.contains(k))
        .or_else(|| bp.rects.iter().find(|(k, _)| name_of(k).as_deref() == Some(asked)))?
        .1;
    let blocked = |x: i32, y: i32| {
        bp.tiles.get(x, y).is_none_or(|t| t.flags() & jane_core::tile::F_SOLID != 0)
            || bp.props.iter().any(|p| {
                let d = props.story.prop(p.def);
                let (px, py) = (i32::from(p.cell.x), i32::from(p.cell.y));
                d.solid && x >= px && y >= py && x < px + i32::from(d.w) && y < py + i32::from(d.h)
            })
    };
    let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
    let mut best: Option<(i32, i32, i32)> = None;
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            let d = (x - cx).abs() + (y - cy).abs();
            if !blocked(x, y) && best.is_none_or(|b| d < b.0) {
                best = Some((d, x, y));
            }
        }
    }
    best.map(|(_, x, y)| jane_core::num::Vec2::centre(x, y))
}

/// The mark `asked` in `zone`, or its way in (the console's `tp` rule): its first named mark
/// among start, front, entry, a stair, a mouth, a gate. An unknown mark names the ones it has.
fn mark_in(sim: &Sim, zone: jane_core::ids::ZoneId, asked: Option<&str>) -> Result<jane_core::Sym, String> {
    let syms = &sim.state().syms;
    let named: Vec<(&str, jane_core::Sym)> = sim
        .blueprint(zone)
        .marks
        .keys()
        .filter_map(|k| match *k {
            jane_core::Key::Name(n) => {
                let s = jane_sim::sym::of_name(n);
                Some((syms.name(s), s))
            }
            jane_core::Key::Local(_) => None,
        })
        .collect();
    if let Some(a) = asked {
        return named.iter().find(|(n, _)| *n == a).map(|(_, s)| *s).ok_or_else(|| {
            let mut all: Vec<&str> = named.iter().map(|(n, _)| *n).collect();
            all.sort_unstable();
            format!("{}: no mark {a}; it has {}", zone.name(), all.join(" "))
        });
    }
    for want in ["start", "front", "entry", "stair_a", "mouth", "gate"] {
        if let Some(s) = syms.find(want).filter(|s| named.iter().any(|(_, n)| n == s)) {
            return Ok(s);
        }
    }
    named.first().map(|(_, s)| *s).ok_or_else(|| format!("{} has no named mark", zone.name()))
}

/// `--rows fog=off,god_rays=off`: `Features` rows by their `config.json` key.
pub fn rows(arg: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let Some(a) = arg else { return Ok(Vec::new()) };
    a.split(',')
        .map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .ok_or_else(|| format!("--rows: key=value, not {kv}"))
        })
        .collect()
}

/// `--weather`'s word.
pub fn weather(s: &str) -> Result<jane_present::WeatherKind, String> {
    use jane_present::WeatherKind as W;
    match s {
        "clear" => Ok(W::Clear),
        "mist" => Ok(W::Mist),
        "rain" => Ok(W::Rain),
        "storm" => Ok(W::Storm),
        _ => Err(format!("--weather: clear, mist, rain or storm, not {s}")),
    }
}

/// The sim with this frame's events kept for the presenter: the bot drains the host, so the host
/// keeps a copy.
struct Tap {
    sim: Sim,
    events: Vec<Event>,
}

impl Host for Tap {
    fn sim(&self) -> &Sim {
        &self.sim
    }

    fn view(&self, seat: Seat) -> Option<View<'_>> {
        self.sim.view(seat)
    }

    fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        self.sim.step(input)
    }

    fn drain_events(&mut self) -> &[Event] {
        self.events.clear();
        self.events.extend_from_slice(self.sim.drain_events());
        &self.events
    }
}

/// What came of a render: the frame's pixels and a line about where it was taken.
#[derive(Debug)]
pub struct Shot {
    pub w: u16,
    pub h: u16,
    /// `0xAARRGGBB` rows.
    pub px: Vec<u32>,
    pub line: String,
    /// With `--layers`: PNGs of the frame's heights and of the T2 height field.
    pub layers: Option<(Vec<u8>, Vec<u8>)>,
}

impl Shot {
    pub fn png(&self) -> Vec<u8> {
        let rgba: Vec<u8> =
            self.px.iter().flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]).collect();
        jane_art::sheet::png(u32::from(self.w), u32::from(self.h), &rgba)
    }

    /// The rect `(x, y, w, h)` of the frame, each px drawn `zoom` px square: the art director's
    /// close look at 3 or 4x (ART.md §3.1).
    pub fn crop(&self, (x, y, w, h): (u16, u16, u16, u16), zoom: u16) -> Shot {
        let (x, y) = (x.min(self.w.saturating_sub(1)), y.min(self.h.saturating_sub(1)));
        let (w, h) = (w.min(self.w - x).max(1), h.min(self.h - y).max(1));
        let z = zoom.clamp(1, 16);
        let (ow, oh) = (w * z, h * z);
        let mut px = Vec::with_capacity(usize::from(ow) * usize::from(oh));
        for oy in 0..oh {
            let sy = usize::from(y + oy / z);
            for ox in 0..ow {
                px.push(self.px[sy * usize::from(self.w) + usize::from(x + ox / z)]);
            }
        }
        Shot { w: ow, h: oh, px, line: self.line.clone(), layers: None }
    }
}

/// Plays `o` from New Game on `bps` to the frame asked for: the host and a presenter at `tier`
/// that has ticked beside it, and how many ticks the model played.
fn play(bps: Blueprints, o: &Opts, tier: Tier) -> Result<(Tap, Present, u32), String> {
    let mut host = Tap { sim: Sim::new_game_with(bps, "Jane"), events: Vec::new() };
    let mut bot = Bot::story(o.model);
    let mut present = Present::new(tier);
    if let Some(k) = o.weather {
        let wet = if matches!(k, jane_present::WeatherKind::Rain | jane_present::WeatherKind::Storm) { 255 } else { 0 };
        present.atmos_mut().force(Some((k, wet)));
        // The sim's sky and ground too, so what the rain puts out (a campfire's `douse`) is out
        // in the frame as it is in the game: a frame never draws a fire lit that the sim has out.
        if wet > 0 {
            use jane_sim::state::{WeatherKind as Sky, WeatherState};
            let kind = if matches!(k, jane_present::WeatherKind::Storm) { Sky::Storm } else { Sky::Rain };
            let now = host.sim.state().tick;
            let st = host.sim.state_mut();
            st.weather = st.weather.map(|_| WeatherState { kind, since: now, until: jane_core::Tick(u32::MAX) });
            if let Some(z) = st.zone_mut(jane_core::ZoneId::County) {
                z.wetness = z.wetness.map(|_| 255);
            }
        }
    }
    for (k, v) in &o.rows {
        if !present.atmos_mut().features.set(tier, k, v) {
            return Err(format!("--rows: no row {k}={v} (keys: {})", jane_present::Features::KEYS.join(", ")));
        }
    }
    present.set_canvas(o.canvas);
    let seat = Seat(0);
    let mut played = 0;
    for _ in 0..o.ticks {
        if bot.done() {
            break;
        }
        bot.step(&mut host);
        played += 1;
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &host.events);
    }
    if let Some((zone, mark)) = &o.at {
        // A name that is no zone is a mark of the county.
        let (z, mark) = match jane_core::ids::ZoneId::from_name(zone) {
            Some(z) => (z, mark.as_deref()),
            None if mark.is_none() => (jane_core::ids::ZoneId::County, Some(zone.as_str())),
            None => return Err(format!("--at: no zone {zone}")),
        };
        // A generated dungeon's room by its node (`--at mine:store`), or any rect by its name: she
        // arrives at the open cell nearest its middle, by way of the zone's way in.
        let room = mark.and_then(|m| room_in(&host.sim, z, m));
        let mark = mark_in(&host.sim, z, if room.is_some() { None } else { mark })?;
        let cmds = [
            StampedCommand { seat: Some(seat), seq: u16::MAX - 2, cmd: Command::Dev(DevOp::God(true)) },
            StampedCommand { seat: Some(seat), seq: u16::MAX - 1, cmd: Command::Dev(DevOp::Tp { zone: z, mark }) },
        ];
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        if let Some(at) = room {
            let req = jane_sim::state::TravelRequest { zone: z, mark, at: Some(at) };
            host.sim.state_mut().players[0].travel = Some(req);
            host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        }
        for _ in 0..60 {
            let events = host.sim.drain_events().to_vec();
            let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
            present.tick(&v, &events);
            host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        }
    }
    if let Some(hour) = o.hour {
        let cmd = [StampedCommand { seat: Some(seat), seq: u16::MAX, cmd: Command::Dev(DevOp::Time { hour }) }];
        let frames = [InputFrame::IDLE; 4];
        host.sim.step(&StepInput { frames, commands: &cmd });
        let events = host.sim.drain_events().to_vec();
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
        // Two ticks of the clock a second (7200 an hour): 120 a minute.
        for _ in 0..u32::from(o.minute) * 120 {
            host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
            let events = host.sim.drain_events().to_vec();
            let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
            present.tick(&v, &events);
        }
    }
    // `--cast SPELL[:TICKS]`: she learns it, has the mana, and casts it east; the frame is drawn
    // TICKS later (12 by default: a bolt in flight).
    if let Some(c) = &o.cast {
        let (name, after) = c.split_once(':').map_or((c.as_str(), 12), |(s, t)| (s, t.parse().unwrap_or(12)));
        let spell = jane_data::catalog().combat.spell_id(name).ok_or_else(|| format!("--cast: no spell \"{name}\""))?;
        let dev = |seq: u16, op| StampedCommand { seat: Some(seat), seq, cmd: Command::Dev(op) };
        let setup = [
            dev(u16::MAX - 3, DevOp::Learn(spell)),
            dev(u16::MAX - 2, DevOp::Mp(9999)),
            dev(u16::MAX - 4, DevOp::God(true)),
        ];
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &setup });
        if let Some(u) = &o.spawn {
            let def = jane_data::catalog().combat.unit_id(u).ok_or_else(|| format!("--spawn: no unit \"{u}\""))?;
            let spawn = [dev(u16::MAX - 6, DevOp::Spawn(def))];
            host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &spawn });
        }
        let aim = InputFrame { aim: Some(jane_core::Angle::EAST), ..InputFrame::IDLE };
        let cast = [StampedCommand { seat: Some(seat), seq: u16::MAX - 5, cmd: Command::Cast { spell, on: None } }];
        host.sim.step(&StepInput { frames: [aim; 4], commands: &cast });
        for _ in 0..after {
            let events = host.sim.drain_events().to_vec();
            let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
            present.tick(&v, &events);
            host.sim.step(&StepInput { frames: [aim; 4], commands: &[] });
        }
        let events = host.sim.drain_events().to_vec();
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
    }
    // `--talk NAME`: she walks to NAME (steered, a cell short) and presses USE.
    if let Some(name) = &o.talk {
        let def = jane_data::catalog().combat.unit_id(name).ok_or_else(|| format!("--talk: no unit \"{name}\""))?;
        let mut pressed: u16 = 0;
        for _ in 0..900 {
            let me = host.sim.state().players[0].unit;
            let z = host.sim.state().players[0].zone;
            let zs = host.sim.state().zone(z).ok_or("her zone")?;
            let her = zs.unit(me).ok_or("her body")?.pos;
            let them = zs.units.iter().filter(|u| u.def == def && u.alive).map(|u| u.pos).next();
            let Some(them) = them else { return Err(format!("--talk: no {name} here")) };
            let (dx, dy) = (them.x.0 - her.x.0, them.y.0 - her.y.0);
            let near = dx.abs() < 24 << 7 && dy.abs() < 24 << 7;
            let frame = if near { InputFrame::IDLE } else { InputFrame::walk(jane_core::angle::bearing(her, them)) };
            let press = [StampedCommand { seat: Some(seat), seq: 2000 + pressed, cmd: Command::Use }];
            let cmds: &[StampedCommand] = if near && pressed < 2 {
                pressed += 1;
                &press
            } else {
                &[]
            };
            host.sim.step(&StepInput {
                frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE],
                commands: cmds,
            });
            let events = host.sim.drain_events().to_vec();
            let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
            present.tick(&v, &events);
            if pressed >= 1 && v.dialogue().is_some() {
                // The line said, the emote risen.
                for _ in 0..30 {
                    host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
                    let events = host.sim.drain_events().to_vec();
                    let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
                    present.tick(&v, &events);
                }
                break;
            }
        }
    }
    // `--quest`: given her, and the world let run a second so the sparkles have risen.
    if !o.quests.is_empty() {
        let dev = |seq: u16, op| StampedCommand { seat: Some(seat), seq, cmd: Command::Dev(op) };
        let cmds = o
            .quests
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let id = jane_data::catalog().story.quest_id(q).ok_or_else(|| format!("--quest: no quest \"{q}\""))?;
                Ok(dev(900 + i as u16, DevOp::Quest(id)))
            })
            .collect::<Result<Vec<_>, String>>()?;
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        for _ in 0..70 {
            let events = host.sim.drain_events().to_vec();
            let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
            present.tick(&v, &events);
            host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        }
    }
    // `--knows`: learned out of sight; `--learn`: learned now, the moments to come; `--grow`: a
    // finding's word to the presenter alone.
    let l = &o.lesson;
    let spell = |name: &String| {
        jane_data::catalog().combat.spell_id(name).ok_or_else(|| format!("--learn/--knows: no spell \"{name}\""))
    };
    let dev = |seq: u16, op| StampedCommand { seat: Some(seat), seq, cmd: Command::Dev(op) };
    if !l.knows.is_empty() {
        let cmds = l
            .knows
            .iter()
            .enumerate()
            .map(|(i, n)| Ok(dev(1000 + i as u16, DevOp::Learn(spell(n)?))))
            .collect::<Result<Vec<_>, String>>()?;
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        host.sim.drain_events();
    }
    if !l.learn.is_empty() || l.grow.is_some() {
        let mut cmds = vec![dev(1100, DevOp::God(true))];
        for (i, n) in l.learn.iter().enumerate() {
            cmds.push(dev(1101 + i as u16, DevOp::Learn(spell(n)?)));
        }
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
        let mut events = host.sim.drain_events().to_vec();
        if let Some(stat) = l.grow {
            let k = match stat {
                jane_core::action::Stat::Strength => jane_sim::event::ToastKind::Stronger,
                jane_core::action::Stat::Spirit => jane_sim::event::ToastKind::WordsStay,
            };
            events.push(Event { to: None, in_zone: None, kind: jane_sim::event::EventKind::Toast(k) });
        }
        host.events = events;
    }
    Ok((host, present, played))
}

/// Plays `o` from New Game on `bps` and draws one frame.
pub fn render(bps: Blueprints, o: &Opts) -> Result<Shot, String> {
    let mut b = backend(o.backend, o.gl)?;
    let (mut host, mut present, played) = play(bps, o, o.backend.tier())?;
    b.upload_atlas(present.atlas());
    // With the HUD: a tick for its buffers (and for a lesson's events, heard now).
    let mut hud = o.lesson.ui.then(|| Hud::new(&present));
    if let Some(h) = &mut hud {
        let events = std::mem::take(&mut host.events);
        let v = host.sim.view(Seat(0)).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
        h.bufs.tick(&v, &events);
    }
    // `--rows` the backend draws itself (`normal_light`, `sharp`): after gl2's own flags only when asked.
    if !o.rows.is_empty() {
        b.set_features(&present.features());
    }
    let seat = Seat(0);
    let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
    let (clock, day) = v.clock();
    let line = format!(
        "{} seed {}: {played} ticks; {} day {day} {:02}:{:02}{}; {} units and {} props near; {} chunks painted; {} fx and {} weather parts; {}",
        o.model.name(),
        o.seed,
        v.zone().name(),
        clock / 7200,
        clock % 7200 / 120,
        if v.is_night() { " night" } else { "" },
        present.seen().0,
        present.seen().1,
        present.chunks_painted(),
        present.fx_count().0,
        present.fx_count().1,
        b.caps().name,
    );
    present.draw(255, o.canvas);
    if let Some(h) = &mut hud {
        h.draw(&mut present, o.canvas);
    }
    let layers = o.gl.layers.then(|| {
        let g = crate::layers::heights(present.frame(), present.atlas());
        let (top, floor) = crate::layers::field(&g);
        (crate::layers::png(&g.h, None, g.w, g.rows), crate::layers::png(&top, Some(&floor), g.w, g.rows))
    });
    let frame = present.frame();
    b.draw(frame);
    let mut px = Vec::new();
    let (w, h) = b.read_back(&mut px);
    if px.is_empty() {
        return Err(format!("{}: nothing read back", b.caps().name));
    }
    Ok(Shot { w, h, px, line, layers })
}

/// `jane sheet scene --film N[:EVERY]` (PRESENTATION.md §6, `jane film` in small): plays to the
/// frame `o` asks for, then `n` more ticks with the world idle, drawing every `every`-th at
/// `alpha = 1`: the rain falling, a mist drifting, a lightning strike, a bolt in flight. Each
/// shot is handed to `out` with its tick.
pub fn film(
    bps: Blueprints,
    o: &Opts,
    n: u32,
    every: u32,
    mut out: impl FnMut(u32, &Shot) -> Result<(), String>,
) -> Result<(), String> {
    let mut b = backend(o.backend, o.gl)?;
    let (mut host, mut present, _) = play(bps, o, o.backend.tier())?;
    b.upload_atlas(present.atlas());
    // `--rows` the backend draws itself (`normal_light`, `sharp`): after gl2's own flags only when asked.
    if !o.rows.is_empty() {
        b.set_features(&present.features());
    }
    let seat = Seat(0);
    let mut px = Vec::new();
    // A lesson's events (`--learn`, `--grow`) are heard on the film's first tick.
    let mut first = std::mem::take(&mut host.events);
    let mut hud = o.lesson.ui.then(|| Hud::new(&present));
    for k in 0..n {
        let mut frames = [InputFrame::IDLE; 4];
        let mut left = k;
        for &(dir, ticks, push) in &o.walk {
            if left < ticks {
                frames[0] = InputFrame { use_held: push, ..InputFrame::walk(dir) };
                break;
            }
            left -= ticks;
        }
        host.sim.step(&StepInput { frames, commands: &[] });
        let mut events = std::mem::take(&mut first);
        events.extend_from_slice(host.sim.drain_events());
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
        if let Some(h) = &mut hud {
            h.bufs.tick(&v, &events);
        }
        if k % every.max(1) != 0 {
            continue;
        }
        present.draw(255, o.canvas);
        if let Some(h) = &mut hud {
            h.draw(&mut present, o.canvas);
        }
        let frame = present.frame();
        b.draw(frame);
        let (w, h) = b.read_back(&mut px);
        out(k, &Shot { w, h, px: px.clone(), line: String::new(), layers: None })?;
    }
    Ok(())
}

/// What `jane bench frames` measured.
#[derive(Debug)]
pub struct FrameBench {
    pub frames: u32,
    /// Per frame, microseconds: the presenter's `draw` (the Frame built), the backend's `draw`
    /// (submitted), and the whole frame to the GPU's last pixel (output included).
    pub build: (u32, u32),
    pub submit: (u32, u32),
    pub whole: (u32, u32),
    /// The backend's own clock, if it has one.
    pub stats: Option<jane_present::FrameStats>,
    pub line: String,
}

fn p50_p99(v: &mut [u32]) -> (u32, u32) {
    v.sort_unstable();
    if v.is_empty() {
        return (0, 0);
    }
    (v[(v.len() - 1) / 2], v[(v.len() - 1) * 99 / 100])
}

/// Plays to the frame `o` asks for, then draws `frames` frames there, each after a presenter
/// tick with the sim idle, through `o.backend`, upscaled to `output` px where the backend has an
/// output (wgpu: an offscreen texture of the window's size, so 3840 x 2160 is a 4K frame).
pub fn bench(bps: Blueprints, o: &Opts, frames: u32, output: (u32, u32)) -> Result<FrameBench, String> {
    let (mut host, mut present, _) = play(bps, o, o.backend.tier())?;
    let seat = Seat(0);
    let (mut build, mut submit, mut whole) = (Vec::new(), Vec::new(), Vec::new());
    let mut b = Bench::new(o.backend, output, o.gl)?;
    b.backend().upload_atlas(present.atlas());
    if !o.rows.is_empty() {
        b.backend().set_features(&present.features());
    }
    for k in 0..frames + 30 {
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        let events = host.sim.drain_events().to_vec();
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
        let t1 = Instant::now();
        let frame = present.draw(200, o.canvas);
        let t2 = Instant::now();
        b.backend().draw(frame);
        let t3 = Instant::now();
        b.finish()?;
        let t4 = Instant::now();
        // The first 30 frames warm the caches and the pipelines.
        if k >= 30 {
            build.push((t2 - t1).as_micros() as u32);
            submit.push((t3 - t2).as_micros() as u32);
            whole.push((t4 - t1).as_micros() as u32);
        }
    }
    Ok(FrameBench {
        frames,
        build: p50_p99(&mut build),
        submit: p50_p99(&mut submit),
        whole: p50_p99(&mut whole),
        stats: b.backend().stats(),
        line: b.describe(),
    })
}

/// The backend a bench drives, and how a frame ends on it.
enum Bench {
    Soft(Box<Soft>),
    #[cfg(feature = "gpu")]
    Gl2(Box<jane_render_gl2::Gl2>),
    #[cfg(feature = "gpu")]
    Wgpu(Box<jane_render_wgpu::Wgpu>),
}

impl Bench {
    #[cfg_attr(not(feature = "gpu"), allow(clippy::unnecessary_wraps))]
    fn new(which: Which, output: (u32, u32), gl: GlOpts) -> Result<Bench, String> {
        let _ = output;
        match which {
            Which::Soft => Ok(Bench::Soft(Box::new(Soft::new()))),
            #[cfg(feature = "gpu")]
            Which::Gl2 => {
                let mut g = jane_render_gl2::Gl2::headless_output(gl.api(), output)?;
                gl.apply(&mut g);
                Ok(Bench::Gl2(Box::new(g)))
            }
            #[cfg(feature = "gpu")]
            Which::Wgpu => Ok(Bench::Wgpu(Box::new(jane_render_wgpu::Wgpu::headless_output(output)?))),
            #[cfg(not(feature = "gpu"))]
            Which::Gl2 | Which::Wgpu => Err(backend(which, gl).err().unwrap_or_default()),
        }
    }

    fn backend(&mut self) -> &mut dyn Backend {
        match self {
            Bench::Soft(s) => s.as_mut(),
            #[cfg(feature = "gpu")]
            Bench::Gl2(g) => g.as_mut(),
            #[cfg(feature = "gpu")]
            Bench::Wgpu(g) => g.as_mut(),
        }
    }

    /// The frame to the screen (wgpu: upscaled to the output) and waited for.
    #[cfg_attr(not(feature = "gpu"), allow(clippy::unnecessary_wraps))]
    fn finish(&mut self) -> Result<(), String> {
        match self {
            Bench::Soft(_) => Ok(()),
            #[cfg(feature = "gpu")]
            Bench::Gl2(g) => {
                g.present()?;
                g.finish();
                Ok(())
            }
            #[cfg(feature = "gpu")]
            Bench::Wgpu(g) => {
                g.present()?;
                g.finish();
                Ok(())
            }
        }
    }

    fn describe(&self) -> String {
        match self {
            Bench::Soft(_) => "soft".into(),
            #[cfg(feature = "gpu")]
            Bench::Gl2(g) => {
                let r = g.rows();
                format!(
                    "{}; shadows {}, light target {}, albedo {}, normals {}",
                    g.describe(),
                    r.shadows,
                    if r.half_light { "half" } else { "full" },
                    if r.exact { "exact" } else { "fast" },
                    if r.normal_light { "on" } else { "off" }
                )
            }
            #[cfg(feature = "gpu")]
            Bench::Wgpu(g) => g.describe().to_owned(),
        }
    }
}

/// `jane sheet fire-scenes` (PLAY-PLAN.md §2.2's frames): made fires on, on seed `seed`, through
/// `soft`, written as PNGs into `out`: a cold pit; laying it (the laid frame, USE held); the
/// flare as it takes; the made fire lit at night with the Night Shift at its edge; the ash; and
/// the Halt trunk's card. The world is driven by hand (the console's verbs, her bag filled), not
/// by a model. Each written file's name.
pub fn fire_scenes(seed: u32, out: &std::path::Path) -> Result<Vec<String>, String> {
    use jane_core::ZoneId;
    let canvas = (768, 432);
    let bps = Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let mut sim = Sim::new_game_with(bps, "Jane");
    let mut present = Present::new(Tier::T0);
    present.set_canvas(canvas);
    let mut b = backend(Which::Soft, GlOpts::default())?;
    b.upload_atlas(present.atlas());
    let mut hud = Hud::new(&present);
    let seat = Seat(0);
    let mut seq = 0u16;
    let mut step = |sim: &mut Sim, present: &mut Present, hud: &mut Hud, frame: InputFrame, cmd: Option<Command>| {
        seq = seq.wrapping_add(1);
        let cmds: Vec<StampedCommand> =
            cmd.into_iter().map(|cmd| StampedCommand { seat: Some(seat), seq, cmd }).collect();
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = frame;
        sim.step(&StepInput { frames, commands: &cmds });
        let events = sim.drain_events().to_vec();
        if let Some(v) = sim.view(seat) {
            present.tick(&v, &events);
            hud.bufs.tick(&v, &events);
        }
    };
    let idle = InputFrame::IDLE;
    let mut written = Vec::new();
    let mut shoot = |present: &mut Present, hud: &mut Hud, name: &str| -> Result<(), String> {
        present.draw(255, canvas);
        hud.draw(present, canvas);
        b.draw(present.frame());
        let mut px = Vec::new();
        let (w, h) = b.read_back(&mut px);
        let shot = Shot { w, h, px, line: String::new(), layers: None };
        let path = out.join(format!("{name}.png"));
        std::fs::write(&path, shot.png()).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path.display().to_string());
        // And the fire close to, at 3x (ART.md §3.1's close look): the middle of the frame.
        let (cw, ch) = (176, 112);
        let close = shot.crop(((w - cw) / 2, (h - ch) / 2, cw, ch), 3);
        let path = out.join(format!("{name}-3x.png"));
        std::fs::write(&path, close.png()).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path.display().to_string());
        Ok(())
    };
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Fires(true))));
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::God(true))));

    let start = mark_in(&sim, ZoneId::County, None)?;
    let stand_at = |sim: &mut Sim, x: i32, y: i32| {
        let at = jane_core::Vec2::centre(x, y);
        sim.state_mut().players[0].travel =
            Some(jane_sim::state::TravelRequest { zone: ZoneId::County, mark: start, at: Some(at) });
    };
    let home = sim.view(seat).map_or((0, 0), |v| v.body().pos.cell());

    // A cold pit near where she starts, with a free cell east, west or south of it to stand on.
    let cat = jane_data::catalog();
    let pit_def = cat.story.prop_id("campfire_cold").ok_or("no fire pit row")?;
    let mut pits: Vec<(jane_sim::PropId, jane_core::Cell)> = sim
        .state()
        .zone(ZoneId::County)
        .map(|z| z.props.iter().filter(|p| p.def == pit_def && !p.hidden).map(|p| (p.id, p.cell)).collect())
        .unwrap_or_default();
    let far = |c: &jane_core::Cell| {
        let (dx, dy) = (i32::from(c.x) - home.0, i32::from(c.y) - home.1);
        dx * dx + dy * dy
    };
    pits.sort_by_key(|(id, c)| (far(c), *id));
    let mut found = None;
    'pits: for (id, c) in pits {
        let (x, y) = (i32::from(c.x), i32::from(c.y));
        // Out in the open: no walls round it (a ruin's hearth is a picture of a ruin).
        let walled = sim.runtime(ZoneId::County).map_or(0, |rt| {
            (-5..=6).flat_map(|j| (-5..=6).map(move |i| (x + i, y + j))).filter(|&(i, j)| rt.grid.solid(i, j)).count()
        });
        if walled > 10 {
            continue;
        }
        for (sx, sy, face) in [
            (x + 2, y + 1, jane_core::Angle::WEST),
            (x - 1, y + 1, jane_core::Angle::EAST),
            (x, y + 2, jane_core::Angle::NORTH),
        ] {
            if sim.runtime(ZoneId::County).is_some_and(|rt| rt.grid.solid(sx, sy)) {
                continue;
            }
            stand_at(&mut sim, sx, sy);
            step(&mut sim, &mut present, &mut hud, idle, None);
            step(&mut sim, &mut present, &mut hud, InputFrame { mv_dir: face, mv_mag: 20, ..idle }, None);
            let on_it = sim
                .view(seat)
                .and_then(|v| v.focus())
                .is_some_and(|f| f.target == jane_sim::interact::FocusRef::Prop(id));
            if on_it {
                found = Some(id);
                break 'pits;
            }
        }
    }
    let pit = found.ok_or("no pit to stand at")?;
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Time { hour: 17 })));
    for _ in 0..90 {
        step(&mut sim, &mut present, &mut hud, idle, None);
    }
    shoot(&mut present, &mut hud, "pit-cold")?;
    for (item, n) in [("deadwood", 4), ("match", 3)] {
        let item = cat.combat.item_id(item).ok_or("no such item")?;
        step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Give { item, qty: n })));
    }
    let held = InputFrame { use_held: true, ..idle };
    for _ in 0..40 {
        step(&mut sim, &mut present, &mut hud, held, None);
    }
    shoot(&mut present, &mut hud, "pit-laying")?;
    for _ in 0..21 {
        step(&mut sim, &mut present, &mut hud, held, None);
    }
    for _ in 0..6 {
        step(&mut sim, &mut present, &mut hud, idle, None);
    }
    shoot(&mut present, &mut hud, "pit-flare")?;

    // Night: the made fire burning, and the Night Shift kept at the edge of its light.
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Time { hour: 23 })));
    if let Some(shade) = cat.combat.unit_id("night_skeleton") {
        for _ in 0..3 {
            step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Spawn(shade))));
        }
        // Out in the dark round the fire, each its own way off: they come in to the edge of its
        // light and wait there (`shunsLight`).
        let me = sim.view(seat).map_or(jane_core::Vec2::ZERO, |v| v.body().pos);
        let cell = jane_core::num::CELL_FX;
        if let Some(z) = sim.state_mut().zone_mut(ZoneId::County) {
            let shades: Vec<_> = z.units.iter().filter(|u| u.def == shade).map(|u| u.id).collect();
            for (id, (dx, dy)) in shades.into_iter().zip([(12, -2), (-11, 3), (2, 11)]) {
                if let Some(u) = z.unit_mut(id) {
                    u.pos = jane_core::Vec2::new(jane_core::Fx(me.x.0 + dx * cell), jane_core::Fx(me.y.0 + dy * cell));
                    u.home = u.pos;
                }
            }
        }
        sim.rebuild_runtimes();
    }
    for _ in 0..240 {
        step(&mut sim, &mut present, &mut hud, idle, None);
    }
    shoot(&mut present, &mut hud, "pit-lit-night")?;

    // Burnt down: ash, by morning light.
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Kill)));
    let until = sim
        .state()
        .zone(ZoneId::County)
        .and_then(|z| z.props.iter().find(|p| p.id == pit))
        .and_then(|p| p.burns_until)
        .ok_or("the pit did not light")?;
    sim.state_mut().tick = jane_core::Tick(until.0.saturating_sub(1));
    for _ in 0..1300 {
        step(&mut sim, &mut present, &mut hud, idle, None);
    }
    step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Time { hour: 9 })));
    for _ in 0..60 {
        step(&mut sim, &mut present, &mut hud, idle, None);
    }
    shoot(&mut present, &mut hud, "pit-ash")?;

    // The Halt trunk, by the fire she steps off the train beside: its card, at five.
    let trunk = sim.state().zone(ZoneId::County).and_then(|z| {
        let key = sim.state().syms.find("halt_trunk")?;
        z.props.iter().find(|p| p.key == key).map(|p| p.cell)
    });
    if let Some(c) = trunk {
        step(&mut sim, &mut present, &mut hud, idle, Some(Command::Dev(DevOp::Time { hour: 17 })));
        stand_at(&mut sim, i32::from(c.x), i32::from(c.y) + 2);
        step(&mut sim, &mut present, &mut hud, idle, None);
        let north = InputFrame { mv_dir: jane_core::Angle::NORTH, mv_mag: 20, ..idle };
        step(&mut sim, &mut present, &mut hud, north, None);
        for _ in 0..240 {
            step(&mut sim, &mut present, &mut hud, idle, None);
        }
        step(&mut sim, &mut present, &mut hud, idle, Some(Command::Use));
        for _ in 0..20 {
            step(&mut sim, &mut present, &mut hud, idle, None);
        }
        shoot(&mut present, &mut hud, "halt-trunk-note")?;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_inputs_draw_the_same_bytes() {
        let bps = Blueprints::build(3).expect("seed 3 builds");
        let o = Opts {
            seed: 3,
            ticks: 240,
            model: Model::Reader,
            hour: Some(22),
            minute: 0,
            canvas: (768, 432),
            backend: Which::Soft,
            at: None,
            weather: None,
            cast: None,
            talk: None,
            spawn: None,
            quests: Vec::new(),
            rows: Vec::new(),
            gl: GlOpts::default(),
            lesson: LessonOpts::default(),
            walk: Vec::new(),
        };
        let a = render(bps.clone(), &o).unwrap();
        let b = render(bps, &o).unwrap();
        assert_eq!((a.w, a.h), (768, 432));
        assert_eq!(a.png(), b.png());
        // Something was drawn: more than one colour on the canvas.
        assert!(a.px.iter().any(|&p| p != a.px[0]));
    }
}
