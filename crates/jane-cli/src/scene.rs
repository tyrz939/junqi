//! `jane sheet scene` (PRESENTATION.md §6): one whole frame, headless. A player model plays a
//! seed from New Game as `jane play` does, the presenter ticks beside it every frame, and at the
//! chosen point one frame is drawn through `soft` and written as a PNG.

use jane_bot::{Bot, Host, Model};
use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::DevOp;
use jane_sim::{Blueprints, Command, Event, InputFrame, Seat, Sim, StampedCommand, StepInput, Stepped, View};

/// What to render.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: u32,
    /// Frames (ticks) the model plays before the frame is drawn.
    pub ticks: u32,
    pub model: Model,
    /// Set the clock to this hour after the play (`--night` is 22).
    pub hour: Option<u8>,
    pub canvas: (u16, u16),
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

/// Plays `o` from New Game on `bps` and draws one frame.
pub fn render(bps: Blueprints, o: &Opts) -> Result<Shot, String> {
    let mut host = Tap { sim: Sim::new_game_with(bps, "Jane"), events: Vec::new() };
    let mut bot = Bot::story(o.model);
    let mut present = Present::new(Tier::T0);
    present.set_canvas(o.canvas);
    let mut soft = Soft::new();
    soft.upload_atlas(present.atlas());
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
    if let Some(hour) = o.hour {
        let cmd = [StampedCommand { seat: Some(seat), seq: u16::MAX, cmd: Command::Dev(DevOp::Time { hour }) }];
        let frames = [InputFrame::IDLE; 4];
        host.sim.step(&StepInput { frames, commands: &cmd });
        let events = host.sim.drain_events().to_vec();
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        present.tick(&v, &events);
    }
    let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
    let (clock, day) = v.clock();
    let line = format!(
        "{} seed {}: {played} ticks; {} day {day} {:02}:{:02}{}; {} units and {} props near; {} chunks painted",
        o.model.name(),
        o.seed,
        v.zone().name(),
        clock / 7200,
        clock % 7200 / 120,
        if v.is_night() { " night" } else { "" },
        present.seen().0,
        present.seen().1,
        present.chunks_painted(),
    );
    let frame = present.draw(255, o.canvas);
    soft.draw(frame);
    let mut px = Vec::new();
    let (w, h) = soft.read_back(&mut px);
    Ok(Shot { w, h, px, line })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_inputs_draw_the_same_bytes() {
        let bps = Blueprints::build(3).expect("seed 3 builds");
        let o = Opts { seed: 3, ticks: 240, model: Model::Reader, hour: Some(22), canvas: (768, 432) };
        let a = render(bps.clone(), &o).unwrap();
        let b = render(bps, &o).unwrap();
        assert_eq!((a.w, a.h), (768, 432));
        assert_eq!(a.png(), b.png());
        // Something was drawn: more than one colour on the canvas.
        assert!(a.px.iter().any(|&p| p != a.px[0]));
    }
}
