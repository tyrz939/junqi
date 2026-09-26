//! The loop (PRESENTATION.md §1.11; ARCHITECTURE.md §4): events into devices once a frame, the
//! devices into one `InputFrame` and some `Command`s, then as many fixed 60 Hz ticks as the
//! clock has accumulated (the sim steps unless paused; the presenter ticks regardless), then one
//! frame drawn at `alpha` between the last tick and the next, through the backend the probe picked
//! (PRESENTATION.md §1.3): `wgpu` at T2 where an adapter can draw it, else `soft` at T0.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use jane_present::input::{
    Context, Edge, GameAction, Input, Mode, UiAction, canvas_size, canvas_to_world, pick, world_to_canvas,
};
use jane_present::{Backend, Present};
use jane_render_soft::Soft;
use jane_render_wgpu::Wgpu;
use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame, StampedCommand, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Blueprints, Seat, Sim};

use crate::devices::{Devices, Happened};
use crate::handle::SdlWindow;
use crate::screen::{self, Target};
use crate::shot;
use crate::{Args, BackendChoice};

/// A tick is `1/60` s; the accumulator counts nanoseconds times 60, so a tick is exactly 1e9.
const TICK: u64 = 1_000_000_000;
/// At most this many ticks a frame; beyond it the time is dropped (and counted), so a stall does
/// not become a fast-forward.
const MAX_CATCH_UP: u64 = 5;
/// The window while the county builds, and the stub scene's clear.
const LOADING: u32 = 0xff10_1014;
/// The seat this window plays.
const ME: Seat = Seat(0);

/// A second's worth of counts for the title bar.
#[derive(Debug, Default)]
struct Stats {
    since: Option<Instant>,
    frames: u32,
    ticks: u32,
    tick_time: Duration,
    draw_time: Duration,
}

/// Where a frame goes: a backend and the window it shows in.
trait Screen {
    fn backend(&mut self) -> &mut dyn Backend;
    fn window_mut(&mut self) -> &mut sdl2::video::Window;
    /// The window's size in px.
    fn size(&self) -> (u32, u32);
    /// The window was resized.
    fn resized(&mut self, _win: (u32, u32)) {}
    /// Shows the frame the backend last drew.
    fn show(&mut self, win: (u32, u32)) -> Result<(), String>;
    /// `soft`, or `wgpu, Vulkan, <adapter>`: the title bar.
    fn describe(&self) -> String;
}

/// T0: `soft` into a streaming texture on an SDL renderer, nearest upscale.
struct SoftScreen<'a> {
    canvas: sdl2::render::WindowCanvas,
    target: Target<'a>,
    soft: Soft,
}

impl Screen for SoftScreen<'_> {
    fn backend(&mut self) -> &mut dyn Backend {
        &mut self.soft
    }

    fn window_mut(&mut self) -> &mut sdl2::video::Window {
        self.canvas.window_mut()
    }

    fn size(&self) -> (u32, u32) {
        self.canvas.window().size()
    }

    fn show(&mut self, win: (u32, u32)) -> Result<(), String> {
        let (px, w, h) = self.soft.pixels();
        self.target.upload(px, w, h)?;
        self.canvas.clear();
        self.target.blit(&mut self.canvas, win)?;
        self.canvas.present();
        Ok(())
    }

    fn describe(&self) -> String {
        "soft".into()
    }
}

/// T2: `wgpu` on the window's own surface, sharp bilinear upscale.
struct GpuScreen {
    window: sdl2::video::Window,
    wgpu: Box<Wgpu>,
}

impl Screen for GpuScreen {
    fn backend(&mut self) -> &mut dyn Backend {
        self.wgpu.as_mut()
    }

    fn window_mut(&mut self) -> &mut sdl2::video::Window {
        &mut self.window
    }

    fn size(&self) -> (u32, u32) {
        self.window.size()
    }

    fn resized(&mut self, win: (u32, u32)) {
        self.wgpu.resize(win);
    }

    fn show(&mut self, _win: (u32, u32)) -> Result<(), String> {
        self.wgpu.present()
    }

    fn describe(&self) -> String {
        self.wgpu.describe().to_owned()
    }
}

/// The probe (§1.3): `wgpu` at T2 when asked or when `auto` finds an adapter that can draw it,
/// else `soft`. `Err` only when `wgpu` was asked for by name and cannot be had.
fn probe(choice: BackendChoice, window: &sdl2::video::Window) -> Result<Option<Wgpu>, String> {
    if choice == BackendChoice::Soft {
        return Ok(None);
    }
    let target = wgpu::SurfaceTarget::from(SdlWindow::new(window));
    match Wgpu::for_window(target, window.size(), true) {
        Ok(w) => Ok(Some(w)),
        Err(e) if choice == BackendChoice::Wgpu => Err(format!("--backend wgpu: {e}")),
        Err(e) => {
            println!("jane-app: no T2 ({e}); drawing with soft");
            Ok(None)
        }
    }
}

pub fn run(args: &Args) -> Result<(), String> {
    // Real pixels on a scaled desktop, so 2x is 2x and the nearest upscale stays square.
    sdl2::hint::set("SDL_WINDOWS_DPI_AWARENESS", "permonitorv2");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    // No pad subsystem is no pad, never no game.
    let pads = sdl.game_controller().ok();
    let usable = video.display_usable_bounds(0).ok().map(|r| (r.width(), r.height()));
    let k = screen::start_scale(usable, args.scale);
    let window = screen::open(&video, "Jane", k)?;
    // SDL starts with text input on; the console turns it on when it opens.
    video.text_input().stop();
    let mut pump = sdl.event_pump()?;
    match probe(args.backend, &window)? {
        Some(wgpu) => {
            println!("jane-app: {}", wgpu.describe());
            play(args, &mut pump, pads, &mut GpuScreen { window, wgpu: Box::new(wgpu) })
        }
        None => {
            let mut canvas = screen::canvas(window)?;
            screen::clear(&mut canvas, LOADING);
            let tc = canvas.texture_creator();
            println!("jane-app: soft");
            play(args, &mut pump, pads, &mut SoftScreen { canvas, target: Target::new(&tc), soft: Soft::new() })
        }
    }
}

/// Writes the canvas the backend last drew to `path` as a PNG.
fn save_shot(screen: &mut dyn Screen, px: &mut Vec<u32>, path: &str) -> Result<(u16, u16), String> {
    let (w, h) = screen.backend().read_back(px);
    shot::write(path, px, w, h)?;
    Ok((w, h))
}

fn play(
    args: &Args,
    pump: &mut sdl2::EventPump,
    pads: Option<sdl2::GameControllerSubsystem>,
    screen: &mut dyn Screen,
) -> Result<(), String> {
    // New Game: the county before the loop, with the window already a colour and not a frozen
    // white rect. A real loading screen that draws the skeleton is P6's.
    let _ = screen.window_mut().set_title(&format!("Jane: seed {}, building the county", args.seed));
    pump.pump_events();
    let t0 = Instant::now();
    let bps = Blueprints::build(args.seed).map_err(|e| format!("seed {}: {e}", args.seed))?;
    let mut sim = Sim::new_game_with(bps, &args.name);
    println!("jane-app: seed {}: the county built in {} ms", args.seed, t0.elapsed().as_millis());

    let mut present = Present::new(screen.backend().caps().tier);
    screen.backend().upload_atlas(present.atlas());
    let describe = screen.describe();
    let mut win = screen.size();
    let mut devices = Devices::new(pads, win.1);
    let mut input = Input::new();
    let mut canvas_px = canvas_size(win.0, win.1);

    let mut paused = false;
    let mut pending: Vec<StampedCommand> = Vec::new();
    let mut seq: u16 = 0;
    let mut events: Vec<Event> = Vec::with_capacity(64);
    let mut edges: Vec<Edge> = Vec::with_capacity(8);
    let mut camera = (0, 0);
    let mut ticks: u64 = 0;
    let mut dropped: u64 = 0;
    let mut shots = 0;
    let mut shot_px: Vec<u32> = Vec::new();
    let mut stats = Stats::default();
    let mut acc: u64 = 0;
    let mut last = Instant::now();

    'run: loop {
        let frame_start = Instant::now();
        for e in pump.poll_iter() {
            match devices.event(&e) {
                Happened::Quit => break 'run,
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
        // The devices, once a frame: one held frame for every tick of it, and the presses.
        let mode = if paused { Mode::Ui } else { Mode::Play };
        let feet = sim
            .view(ME)
            .map(|v| world_to_canvas(v.body().pos, camera))
            .filter(|&(x, y)| (0.0..f32::from(canvas_px.0)).contains(&x) && (0.0..f32::from(canvas_px.1)).contains(&y));
        let held = input.sample(&devices.state, &Context { mode, feet });
        let cursor = devices.state.mouse.pos.filter(|_| input.aiming_with_mouse());
        devices.state.end_sample();
        edges.extend(input.drain());
        for edge in edges.drain(..) {
            let cmd = match edge {
                Edge::Game(GameAction::Use) => Command::Use,
                // With a cursor, `on` is what is under it (her own body over nobody); without
                // one (a pad) the sim picks the friend nearest the aim line.
                Edge::Game(GameAction::Bar(slot)) => Command::Bar {
                    slot,
                    on: cursor.and_then(|c| sim.view(ME).map(|v| pick(&v, canvas_to_world(c, camera)))),
                },
                Edge::Ui(UiAction::Pause) => {
                    // Alone, pause freezes the world: the sim does not step (PRESENTATION.md §1.11).
                    paused = true;
                    pending.clear();
                    continue;
                }
                Edge::Ui(UiAction::Cancel) => {
                    paused = false;
                    continue;
                }
                Edge::Ui(UiAction::Shot) => {
                    shots += 1;
                    let path = shot::numbered(args.shot.as_deref().unwrap_or("jane-shot.png"), shots);
                    match save_shot(screen, &mut shot_px, &path) {
                        Ok(_) => println!("jane-app: shot {path}"),
                        Err(e) => eprintln!("jane-app: shot: {e}"),
                    }
                    continue;
                }
                // Bags, book, quests, map, console, the overlays, save and load: P7's screens.
                Edge::Ui(_) => continue,
            };
            seq = seq.wrapping_add(1);
            pending.push(StampedCommand { seat: Some(ME), seq, cmd });
        }

        // The clock: whole ticks due since the last frame, at most MAX_CATCH_UP of them.
        let now = Instant::now();
        acc += (now - last).as_nanos() as u64 * 60;
        last = now;
        let mut due = acc / TICK;
        if due > MAX_CATCH_UP {
            dropped += due - MAX_CATCH_UP;
            acc -= (due - MAX_CATCH_UP) * TICK;
            due = MAX_CATCH_UP;
        }
        let mut done = false;
        for _ in 0..due {
            let t = Instant::now();
            events.clear();
            if !paused {
                let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
                frames[ME.index()] = held;
                sim.step(&StepInput { frames, commands: &pending });
                pending.clear();
                events.extend_from_slice(sim.drain_events());
            }
            // Every tick, the world frozen or not: toasts fade and cursors blink in a pause.
            if let Some(v) = sim.view(ME) {
                present.tick(&v, &events);
            }
            acc -= TICK;
            ticks += 1;
            stats.ticks += 1;
            stats.tick_time += t.elapsed();
            if args.ticks.is_some_and(|n| ticks >= n) {
                done = true;
                break;
            }
        }

        // One frame at alpha.
        let t = Instant::now();
        let alpha = (acc * 256 / TICK).min(255) as u8;
        let frame = present.draw(alpha, canvas_px);
        camera = frame.camera;
        screen.backend().draw(frame);
        // The draw, not the upload and the wait for vsync that show() does.
        stats.draw_time += t.elapsed();
        screen.show(win)?;
        stats.frames += 1;
        title(screen, &describe, &mut stats, args.seed, paused, dropped);
        if done {
            break;
        }
        // Without vsync (a minimised window, a driver that ignores it) do not spin a core.
        if due == 0 && frame_start.elapsed() < Duration::from_millis(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    if let Some(path) = &args.shot {
        let (w, h) = save_shot(screen, &mut shot_px, path)?;
        println!("jane-app: shot {path} ({w} x {h})");
    }
    println!(
        "jane-app: seed {}: {ticks} ticks, {dropped} dropped, frame {}, hash {:016x}",
        args.seed,
        sim.state().frame,
        sim.hash()
    );
    Ok(())
}

/// Once a second: the backend, frames a second, the average tick and draw, and the backend's own
/// frame times where it keeps them (§1.12), until the F2 overlay exists.
fn title(screen: &mut dyn Screen, describe: &str, s: &mut Stats, seed: u32, paused: bool, dropped: u64) {
    let since = *s.since.get_or_insert_with(Instant::now);
    let el = since.elapsed();
    if el < Duration::from_secs(1) {
        return;
    }
    let per = |d: Duration, n: u32| d.as_micros() / u128::from(n.max(1));
    let fps = u128::from(s.frames) * 1000 / el.as_millis().max(1);
    let mut t = format!(
        "Jane: seed {seed}, {describe}, {fps} fps, tick {} us, draw {} us",
        per(s.tick_time, s.ticks),
        per(s.draw_time, s.frames)
    );
    if let Some(f) = screen.backend().stats().filter(|f| f.frames > 0) {
        let _ = write!(
            t,
            ", {} p50 {:.2} ms p99 {:.2} ms",
            if f.gpu_clock { "gpu" } else { "frame" },
            f64::from(f.p50_us) / 1000.0,
            f64::from(f.p99_us) / 1000.0
        );
    }
    if dropped > 0 {
        let _ = write!(t, ", {dropped} ticks dropped");
    }
    if paused {
        t += ", paused";
    }
    let _ = screen.window_mut().set_title(&t);
    *s = Stats { since: Some(Instant::now()), ..Stats::default() };
}
