//! `jane sheet scene` (PRESENTATION.md §6): one whole frame, headless. A player model plays a
//! seed from New Game as `jane play` does, the presenter ticks beside it every frame, and at the
//! chosen point one frame is drawn through `soft` (or, with the `gpu` feature, `wgpu`) and
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
    Wgpu,
}

impl Which {
    pub fn parse(s: &str) -> Option<Which> {
        match s {
            "soft" | "t0" => Some(Which::Soft),
            "wgpu" | "t2" => Some(Which::Wgpu),
            _ => None,
        }
    }

    /// The tier its frames are built for.
    pub fn tier(self) -> Tier {
        match self {
            Which::Soft => Tier::T0,
            Which::Wgpu => Tier::T2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Which::Soft => "t0",
            Which::Wgpu => "t2",
        }
    }
}

/// The backend, headless.
pub fn backend(which: Which) -> Result<Box<dyn Backend>, String> {
    match which {
        Which::Soft => Ok(Box::new(Soft::new())),
        #[cfg(feature = "gpu")]
        Which::Wgpu => Ok(Box::new(jane_render_wgpu::Wgpu::headless()?)),
        #[cfg(not(feature = "gpu"))]
        Which::Wgpu => {
            Err("wgpu: this jane was built without the gpu feature (cargo build -p jane-cli --features gpu)".into())
        }
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
    /// with god on and the world let settle a second: a frame inside a dungeon.
    pub at: Option<(String, Option<String>)>,
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
}

impl Shot {
    pub fn png(&self) -> Vec<u8> {
        let rgba: Vec<u8> =
            self.px.iter().flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]).collect();
        jane_art::sheet::png(u32::from(self.w), u32::from(self.h), &rgba)
    }
}

/// Plays `o` from New Game on `bps` to the frame asked for: the host and a presenter at `tier`
/// that has ticked beside it, and how many ticks the model played.
fn play(bps: Blueprints, o: &Opts, tier: Tier) -> Result<(Tap, Present, u32), String> {
    let mut host = Tap { sim: Sim::new_game_with(bps, "Jane"), events: Vec::new() };
    let mut bot = Bot::story(o.model);
    let mut present = Present::new(tier);
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
        let z = jane_core::ids::ZoneId::from_name(zone).ok_or_else(|| format!("--at: no zone {zone}"))?;
        let mark = mark_in(&host.sim, z, mark.as_deref())?;
        let cmds = [
            StampedCommand { seat: Some(seat), seq: u16::MAX - 2, cmd: Command::Dev(DevOp::God(true)) },
            StampedCommand { seat: Some(seat), seq: u16::MAX - 1, cmd: Command::Dev(DevOp::Tp { zone: z, mark }) },
        ];
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &cmds });
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
    Ok((host, present, played))
}

/// Plays `o` from New Game on `bps` and draws one frame.
pub fn render(bps: Blueprints, o: &Opts) -> Result<Shot, String> {
    let mut b = backend(o.backend)?;
    let (host, mut present, played) = play(bps, o, o.backend.tier())?;
    b.upload_atlas(present.atlas());
    let seat = Seat(0);
    let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
    let (clock, day) = v.clock();
    let line = format!(
        "{} seed {}: {played} ticks; {} day {day} {:02}:{:02}{}; {} units and {} props near; {} chunks painted; {}",
        o.model.name(),
        o.seed,
        v.zone().name(),
        clock / 7200,
        clock % 7200 / 120,
        if v.is_night() { " night" } else { "" },
        present.seen().0,
        present.seen().1,
        present.chunks_painted(),
        b.caps().name,
    );
    let frame = present.draw(255, o.canvas);
    b.draw(frame);
    let mut px = Vec::new();
    let (w, h) = b.read_back(&mut px);
    if px.is_empty() {
        return Err(format!("{}: nothing read back", b.caps().name));
    }
    Ok(Shot { w, h, px, line })
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
    let mut b = Bench::new(o.backend, output)?;
    b.backend().upload_atlas(present.atlas());
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
    Wgpu(Box<jane_render_wgpu::Wgpu>),
}

impl Bench {
    #[cfg_attr(not(feature = "gpu"), allow(clippy::unnecessary_wraps))]
    fn new(which: Which, output: (u32, u32)) -> Result<Bench, String> {
        let _ = output;
        match which {
            Which::Soft => Ok(Bench::Soft(Box::new(Soft::new()))),
            #[cfg(feature = "gpu")]
            Which::Wgpu => Ok(Bench::Wgpu(Box::new(jane_render_wgpu::Wgpu::headless_output(output)?))),
            #[cfg(not(feature = "gpu"))]
            Which::Wgpu => Err(backend(which).err().unwrap_or_default()),
        }
    }

    fn backend(&mut self) -> &mut dyn Backend {
        match self {
            Bench::Soft(s) => s.as_mut(),
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
        };
        let a = render(bps.clone(), &o).unwrap();
        let b = render(bps, &o).unwrap();
        assert_eq!((a.w, a.h), (768, 432));
        assert_eq!(a.png(), b.png());
        // Something was drawn: more than one colour on the canvas.
        assert!(a.px.iter().any(|&p| p != a.px[0]));
    }
}
