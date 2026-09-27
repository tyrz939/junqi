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
    /// With `--cast`: first put this unit (by its catalog name) a few cells east of her, the
    /// console's `spawn`, so the bolt has a body to hit (`--spawn skeleton`).
    pub spawn: Option<String>,
    /// `Features` rows set by key (`--rows fog=off,god_rays=off`, PRESENTATION.md §1.3).
    pub rows: Vec<(String, String)>,
    pub gl: GlOpts,
}

/// The open cell nearest the middle of a rect of `zone` named `<zone>_<asked>_room` (a generated
/// dungeon's room by its node) or `asked`, if there is one and `asked` is no mark.
fn room_in(sim: &Sim, zone: jane_core::ids::ZoneId, asked: &str) -> Option<jane_core::num::Vec2> {
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
    Ok((host, present, played))
}

/// Plays `o` from New Game on `bps` and draws one frame.
pub fn render(bps: Blueprints, o: &Opts) -> Result<Shot, String> {
    let mut b = backend(o.backend, o.gl)?;
    let (host, mut present, played) = play(bps, o, o.backend.tier())?;
    b.upload_atlas(present.atlas());
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
    for k in 0..n {
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
        let events = host.sim.drain_events().to_vec();
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
        if k % every.max(1) != 0 {
            continue;
        }
        let frame = present.draw(255, o.canvas);
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
            spawn: None,
            rows: Vec::new(),
            gl: GlOpts::default(),
        };
        let a = render(bps.clone(), &o).unwrap();
        let b = render(bps, &o).unwrap();
        assert_eq!((a.w, a.h), (768, 432));
        assert_eq!(a.png(), b.png());
        // Something was drawn: more than one colour on the canvas.
        assert!(a.px.iter().any(|&p| p != a.px[0]));
    }
}
