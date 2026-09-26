//! The loop (PRESENTATION.md §1.11; ARCHITECTURE.md §4): events into devices once a frame, the
//! devices into one `InputFrame` and some `Command`s, then as many fixed 60 Hz ticks as the
//! clock has accumulated (the sim steps unless paused; the presenter ticks regardless), then one
//! frame drawn at `alpha` between the last tick and the next.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use jane_present::input::{
    Context, Edge, GameAction, Input, Mode, UiAction, canvas_size, canvas_to_world, pick, world_to_canvas,
};
use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::Seat;
use jane_sim::event::Event;
use jane_sim::input::Command;

use crate::Args;
use crate::devices::{Devices, Happened};
use crate::screen::{self, Target};
use crate::{session, shot};

/// A tick is `1/60` s; the accumulator counts nanoseconds times 60, so a tick is exactly 1e9.
const TICK: u64 = 1_000_000_000;
/// At most this many ticks a frame; beyond it the time is dropped (and counted), so a stall does
/// not become a fast-forward.
const MAX_CATCH_UP: u64 = 5;
/// The window while the county builds, and the stub scene's clear.
const LOADING: u32 = 0xff10_1014;
/// A guest this many frames behind the host steps extra frames to catch up.
const BEHIND: u32 = 2;

/// A second's worth of counts for the title bar.
#[derive(Debug, Default)]
struct Stats {
    since: Option<Instant>,
    frames: u32,
    ticks: u32,
    tick_time: Duration,
    draw_time: Duration,
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
    let mut canvas = screen::open(&video, "Jane", k)?;
    // SDL starts with text input on; the console turns it on when it opens.
    video.text_input().stop();
    let mut pump = sdl.event_pump()?;

    // New Game: the county before the loop, with the window already a colour and not a frozen
    // white rect. A real loading screen that draws the skeleton is P6's.
    screen::clear(&mut canvas, LOADING);
    let _ = canvas.window_mut().set_title(&format!("Jane: seed {}, building the county", args.seed));
    pump.pump_events();
    let t0 = Instant::now();
    // The session's clock, in milliseconds from here.
    let clock = t0;
    // Alone or hosting, the world is built (or loaded) here; joining, it comes from the host.
    let mut session = match &args.join {
        Some(addr) => session::join(args, addr, clock, |title| {
            let _ = canvas.window_mut().set_title(title);
            !pump.poll_iter().any(|e| matches!(e, sdl2::event::Event::Quit { .. }))
        })?,
        None => session::start(session::load_or_new(args)?, args)?,
    };
    let me = session.seat().unwrap_or(Seat::HOST);
    let seed = session.sim().map_or(args.seed, |s| s.state().seed);
    println!("jane-app: seed {seed}: the world ready in {} ms, seat {}", t0.elapsed().as_millis(), me.0);

    let mut present = Present::new(Tier::T0);
    let mut soft = Soft::new();
    soft.upload_atlas(present.atlas());
    let tc = canvas.texture_creator();
    let mut target = Target::new(&tc);
    let mut win = canvas.window().size();
    let mut devices = Devices::new(pads, win.1);
    let mut input = Input::new();
    let mut canvas_px = canvas_size(win.0, win.1);

    let mut paused = false;
    let mut pending: Vec<Command> = Vec::new();
    let mut events: Vec<Event> = Vec::with_capacity(64);
    let mut edges: Vec<Edge> = Vec::with_capacity(8);
    let mut camera = (0, 0);
    let mut ticks: u64 = 0;
    let mut dropped: u64 = 0;
    let mut shots = 0;
    let mut stats = Stats::default();
    let mut acc: u64 = 0;
    let mut last = Instant::now();

    'run: loop {
        let frame_start = Instant::now();
        for e in pump.poll_iter() {
            match devices.event(&e) {
                Happened::Quit => break 'run,
                Happened::Resized => {
                    win = canvas.window().size();
                    devices.set_window_height(win.1);
                    canvas_px = canvas_size(win.0, win.1);
                }
                Happened::Nothing => {}
            }
        }
        devices.poll_pad();

        // The devices, once a frame: one held frame for every tick of it, and the presses.
        let mode = if paused { Mode::Ui } else { Mode::Play };
        let feet =
            session.sim().and_then(|s| s.view(me)).map(|v| world_to_canvas(v.body().pos, camera)).filter(|&(x, y)| {
                (0.0..f32::from(canvas_px.0)).contains(&x) && (0.0..f32::from(canvas_px.1)).contains(&y)
            });
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
                    on: cursor.and_then(|c| {
                        session.sim().and_then(|s| s.view(me)).map(|v| pick(&v, canvas_to_world(c, camera)))
                    }),
                },
                Edge::Ui(UiAction::Pause) => {
                    // Alone, pause freezes the world: the sim does not step (PRESENTATION.md §1.11).
                    // With company (or open to it) her stick is idle and the world goes on.
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
                    let (px, w, h) = soft.pixels();
                    match shot::write(&path, px, w, h) {
                        Ok(()) => println!("jane-app: shot {path}"),
                        Err(e) => eprintln!("jane-app: shot: {e}"),
                    }
                    continue;
                }
                // Bags, book, quests, map, console, the overlays, save and load: P7's screens.
                Edge::Ui(_) => continue,
            };
            pending.push(cmd);
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
        // A guest behind the host steps what it has in hand beyond the clock's ticks.
        let extra = u64::from(session.backlog().saturating_sub(BEHIND)).min(MAX_CATCH_UP);
        let ms = clock.elapsed().as_millis() as u64;
        session.poll(ms);
        for i in 0..due + extra {
            let t = Instant::now();
            events.clear();
            // Alone this steps unless paused; at a table, when every seat's input has come.
            if session.try_step(ms, held, &mut pending, paused).is_some() {
                events.extend_from_slice(session.events());
            }
            // Every tick, the world frozen or not: toasts fade and cursors blink in a pause.
            if let Some(v) = session.sim().and_then(|s| s.view(me)) {
                present.tick(&v, &events);
            }
            if i < due {
                acc -= TICK;
                ticks += 1;
            }
            stats.ticks += 1;
            stats.tick_time += t.elapsed();
            if args.ticks.is_some_and(|n| ticks >= n) {
                done = true;
                break;
            }
        }
        session.poll(ms);
        if session.take_rested() {
            if let (Some(path), Some(sim)) = (&args.save, session.sim()) {
                session::write_save(std::path::Path::new(path), sim);
            }
        }

        // One frame at alpha.
        let t = Instant::now();
        let alpha = (acc * 256 / TICK).min(255) as u8;
        let frame = present.draw(alpha, canvas_px);
        camera = frame.camera;
        soft.draw(frame);
        let (px, w, h) = soft.pixels();
        target.upload(px, w, h)?;
        canvas.clear();
        target.blit(&mut canvas, win)?;
        // The draw, not the wait for vsync that present() does.
        stats.draw_time += t.elapsed();
        canvas.present();
        stats.frames += 1;
        title(&mut canvas, &mut stats, seed, paused, dropped, &mut session);
        if done {
            break;
        }
        // Without vsync (a minimised window, a driver that ignores it) do not spin a core.
        if due == 0 && frame_start.elapsed() < Duration::from_millis(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    if let Some(path) = &args.shot {
        let (px, w, h) = soft.pixels();
        shot::write(path, px, w, h)?;
        println!("jane-app: shot {path} ({w} x {h})");
    }
    if let (Some(path), Some(sim)) = (&args.save, session.sim()) {
        if !matches!(session, jane_net::Session::Guest(_)) {
            session::write_save(std::path::Path::new(path), sim);
        }
    }
    if let Some(sim) = session.sim() {
        println!(
            "jane-app: seed {seed}: {ticks} ticks, {dropped} dropped, frame {}, hash {:016x}",
            sim.state().frame,
            sim.hash()
        );
    }
    if let jane_net::Session::Host(h) = &session {
        let c = h.checks();
        println!("jane-app: hash checks with guests: {} agreed, {} differed, last at frame {}", c.ok, c.bad, c.last);
    }
    session.close();
    Ok(())
}

/// Once a second: frames a second and the average tick and draw, until the F2 overlay exists;
/// hosting or joined, who is at the table and whether it waits for anyone.
fn title(
    canvas: &mut sdl2::render::WindowCanvas,
    s: &mut Stats,
    seed: u32,
    paused: bool,
    dropped: u64,
    session: &mut jane_net::Session,
) {
    let since = *s.since.get_or_insert_with(Instant::now);
    let el = since.elapsed();
    if el < Duration::from_secs(1) {
        return;
    }
    let per = |d: Duration, n: u32| d.as_micros() / u128::from(n.max(1));
    let fps = u128::from(s.frames) * 1000 / el.as_millis().max(1);
    let mut t = format!(
        "Jane: seed {seed}, {fps} fps, tick {} us, draw {} us",
        per(s.tick_time, s.ticks),
        per(s.draw_time, s.frames)
    );
    if dropped > 0 {
        let _ = write!(t, ", {dropped} ticks dropped");
    }
    if paused {
        t += ", paused";
    }
    let st = session.status();
    if st.role != "alone" {
        let _ = write!(t, ", {} ({} at the table", st.role, st.seats);
        if let Some(p) = session.port() {
            let _ = write!(t, ", port {p}");
        }
        t += ")";
    }
    if let Some(stall) = st.stall {
        let who: Vec<String> = (0..4).filter(|i| stall.seats & (1 << i) != 0).map(|i| i.to_string()).collect();
        let _ = write!(t, ", waiting for seat {} ({} s)", who.join(" and "), stall.waited_ms / 1000);
    }
    if let Some(end) = st.ended {
        let _ = write!(t, ", {end}");
    }
    // Who sat down, got up or was dropped; a desync's report among them (a toast, once the HUD
    // has one).
    for n in &st.notes {
        println!("jane-app: {n}");
    }
    if let (Some(r), true) = (st.desync, st.notes.is_empty()) {
        eprintln!("jane-app: {r}");
    }
    let _ = canvas.window_mut().set_title(&t);
    *s = Stats { since: Some(Instant::now()), ..Stats::default() };
}
