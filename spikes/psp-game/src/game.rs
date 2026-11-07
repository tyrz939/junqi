//! The game thread (PORT.md §13.12): its stack and budgets, `World`, and `run`, the title,
//! the loading screen and play in one loop (one long fn, left whole); with the Graphics
//! page's settings applied (`apply_graphics`) and the presenter made from its tables.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::sync::atomic::Ordering;

use alloc::format;

use crate::shell::{PspPad, Scene, Shell};
use crate::{ahead, bench, county_cache, diag, loader, perf, shell};
use jane_core::{Angle, ZoneId};
use jane_present::{Features, Frame, Present, Tier};
use jane_render_psp::ge::Ge;
use jane_render_psp::{capture, Lister, Pack, UiLister};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim};

use crate::files::{
    append_file, capture_header, file_exists, find, read_baked, save_capture, shot, write_file, Stick, BAKED_DIR,
    CAPTURE_AFTER_US, DASH_FRAMES, SETTINGS,
};
use crate::log::{log_window, Window};
use crate::platform::{
    free_mem, full_clock, heap_init, let_go_log, now_ns, now_us, power_thread, HEAP, RESUMES, SMALL_ARENA,
};
use crate::script::{
    at_reached, crossed, hold_ballast, pad, script_atmos, script_flags, script_weather, walk_at, At, Script, SESSION,
};
use crate::workers::{
    clock_seed, paint_jobs, stage, start_build, start_thread, start_workers, take_built, Boost, BuildJob, PAINTER,
    STAGES,
};
use crate::Line;
use psp::sys;

// ---------------------------------------------------------------- the game

/// The game thread's stack: the sim's and the presenter's deepest calls fit in it, with room.
/// (PORT.md §13.12: 35 KB of it used at most in play and with the map open, `sceKernelGetThreadStackFreeSize`.)
pub(crate) const MAIN_STACK: i32 = 128 * 1024;
const TICK_US: u32 = 1_000_000 / jane_core::num::TICK_RATE;
const CANVAS: (u16, u16) = (480, 272);
/// Pages held in RAM at most (the rest stay on the Memory Stick until drawn).
/// (1.25 MB: a frame draws from 0.8 to 1.4 MB of pages, the map open 1.7; a frame that needs
/// more holds them all past the budget until the next. Was 1.5 MB.)
const PAGE_RAM: u32 = 5 * 256 * 1024;
/// Terrain chunk slots: a 480 x 272 view straddles up to 4 x 3 chunks, and the paint-ahead
/// band one more column or row the way she walks.
const CHUNK_SLOTS: usize = 12;

/// The world being played and what draws it.
pub(crate) struct World {
    pub(crate) sim: alloc::boxed::Box<Sim>,
    pub(crate) present: Present,
}

pub(crate) fn run(dirs: &[String]) {
    // The PSP at its full clock, as games run it (PPSSPP starts it at 222 MHz), read back.
    let (set, cpu, bus) = full_clock();
    say!("GAME clock set={set:x} cpu={cpu} bus={bus}");
    let free = heap_init();
    // The workers first, while their stacks can still be had; each waits for its first job.
    start_workers();
    let _ = start_thread(b"power\0", power_thread, 17, 8 * 1024);
    // Above every other thread, asleep but once a second: a freeze writes `freeze.txt`.
    diag::set_main();
    let _ = start_thread(b"watchdog\0", diag::watchdog, 15, 16 * 1024);
    // The bench's hang watchdog (bench.rs), asleep until a bench runs.
    bench::start_watchdog();
    say!("GAME heap user_free={free} small_pool={SMALL_ARENA}");
    let Some((jpt_path, jpt)) = find(dirs, "present.jpt") else {
        say!("GAME error: no present.jpt in {:?}", dirs);
        return;
    };
    let Some((jpk_path, jpk)) = find(dirs, "jane-psp.jpk") else {
        say!("GAME error: no jane-psp.jpk in {:?}", dirs);
        return;
    };
    // The baked zones (`assets/world/<zone>.bp`, the county so far): read from beside the pack at each
    // New Game, never built.
    let Some((county_path, _)) = find(dirs, "county.bp") else {
        say!("GAME error: no county.bp in {:?}", dirs);
        return;
    };
    let dir = String::from(&county_path[..county_path.len() - "county.bp".len()]);
    let pack_path = format!("{dir}county.jck");
    BAKED_DIR.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(dir)), Ordering::Release);
    jane_sim::baked::set_reader(read_baked);
    let script_path = find(dirs, "script.txt").map(|(p, _)| p);
    let script_text: Option<String> =
        find(dirs, "script.txt").and_then(|(_, f)| f.read_all()).and_then(|b| String::from_utf8(b).ok());
    let mut script: Option<Script> = script_text.as_deref().map(Script::parse);
    // Shots go beside the script.
    let shot_dir: String =
        script_path.as_deref().and_then(|p| p.rfind('/').map(|i| String::from(&p[..=i]))).unwrap_or_default();
    say!("GAME files {jpt_path} {jpk_path} script={script:?}");
    // The county's chunks baked (P1c), read and decoded on the loader's thread.
    if script.as_ref().is_some_and(|s| s.nobake) || loader::open(&pack_path).is_none() {
        say!("GAME baked: none (the painter paints the county)");
    }

    // The pack's tables (its pages stay in the file).
    let mut head = [0u8; jane_render_psp::pack::HEADER];
    if !jpk.read_at(0, &mut head) {
        say!("GAME error: short pack");
        return;
    }
    let pack = match Pack::head_len(&head).and_then(|n| {
        let mut b = alloc::vec![0u8; n];
        if jpk.read_at(0, &mut b) {
            Pack::head(&b)
        } else {
            Err(jane_render_psp::pack::PackError("short"))
        }
    }) {
        Ok(p) => p,
        Err(e) => {
            say!("GAME error: pack: {e:?}");
            return;
        }
    };
    // The UI's page table alone: the title and the loading screen come before any world.
    let art = match jpt.read_all().map(|b| Present::ui_art_from_tables(&b)) {
        Some(Ok(a)) => a,
        _ => {
            say!("GAME error: no UI table in present.jpt");
            return;
        }
    };
    let mut ui_lister = UiLister::new(&art, &pack);
    say!(
        "GAME pack pages={} recs={} refs={} ui_pics={} live={}",
        pack.pages.len(),
        pack.recs.len(),
        pack.refs.len(),
        ui_lister.len(),
        HEAP.live.get()
    );
    // The world's lister once a presenter exists; outside play an empty one draws the clear.
    let mut lister = Lister::new(&[], &pack);
    let mut lister_blank = Lister::new(&[], &pack);
    let mut lister_real = false;
    let script_atmos_bits = script_atmos(script_text.as_deref());
    let mut ge = Ge::new(pack, PAGE_RAM);
    // The CPU's next frame beside the GE's drawing of this one (PORT.md §13.13).
    ge.pipelined = !script.as_ref().is_some_and(|s| s.nopipe);
    if let Some(p) = script.as_ref().and_then(|s| s.pacing) {
        ge.pacing = p;
    }
    // `listcap:KB`: the display list's room lowered (inert until the list guard reads it).
    if let Some(kb) = script.as_ref().and_then(|s| s.list_cap_kb) {
        ge.list_cap = kb as usize * 1024;
        say!("GAME listcap {} KB", kb);
    }
    let mut shell = Shell::new(art);
    shell.clock = Some(now_us);
    let mut stick = Stick::new();
    diag::init(&stick.dir, PAINTER.load(Ordering::Relaxed));
    shell.read_slots(&mut stick);
    // The player's settings (the pad, the volumes), and a script's over them.
    let mut settings = stick
        .get(SETTINGS)
        .and_then(|b| String::from_utf8(b).ok())
        .map_or_else(Default::default, |t| jane_present::pad_psp::Settings::read(&t));
    if let Some(s) = &script {
        settings.pad.walk = s.walk_with.unwrap_or(settings.pad.walk);
        settings.pad.dead = s.dead.unwrap_or(settings.pad.dead);
    }
    say!("GAME settings {:?}", settings);
    shell.set_settings(settings);
    // The bench: a script's `bench` runs it from the start, each scene a script (bench.rs).
    let mut bench = bench::Bench::from_script(script_text.as_deref(), &stick.dir, &jpk_path, &shell);
    if let Some(bn) = &bench {
        script = Some(bn.script());
    }
    // The volumes last sent to the mixer (sent again when they change or the mixer is new).
    let mut vol_sent: Option<jane_present::audio::Volumes> = None;
    let mut said_target = None;
    // The frame outside play: the dark clear and the UI.
    let mut blank = Frame::new(Tier::T0);
    blank.canvas = CANVAS;
    blank.clear = shell::DARK;
    // It draws the UI alone: the world's lists' room let go (a PC frame reserves thousands).
    (blank.passes, blank.chunks, blank.sprites, blank.lights, blank.casters, blank.blocks) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    (blank.water, blank.fog, blank.parts, blank.stars) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    blank.ui = Vec::with_capacity(512);
    // SAFETY: the pad's set-up, once.
    unsafe {
        sys::sceCtrlSetSamplingCycle(0);
        sys::sceCtrlSetSamplingMode(sys::CtrlMode::Analog);
    }
    let mut load = |at: u32, buf: &mut [u8]| jpk.read_at(at, buf);
    let mut world: Option<World> = None;
    let mut built_slot: Option<u8> = None;
    let mut loading_seen = 0usize;
    let mut job_out = false;
    let mut events: Vec<jane_sim::event::Event> = Vec::with_capacity(64);
    // Frames since the title came up, and play ticks: the script's two clocks.
    let (mut title_frames, mut play_ticks, mut sessions) = (0u32, 0u32, 0u32);
    let mut acc = 0u32;
    let mut last = now_us();
    let mut w = Window { start: now_us(), ..Window::default() };
    let mut quit = false;
    let mut travel_sent = false;
    let mut shots_taken: Vec<At> = Vec::new();
    let mut ends = [0u32; 121];
    let mut ends_n = 0usize;
    // The sound from the title on (PORT.md §13.4): the module, the mixer and its audio thread,
    // made once; its 0.8 MB now fits beside a New Game's build. The county's seed reaches it
    // when the world is made. A scripted run captures what it plays beside the program.
    // The VFPU spike (PLAN-PSP-SPEED.md): group pan on the VFPU is on by default (2.5-2.8x on
    // hardware, maxdiff 0); a script's `novfpu` turns it off, read by the audio thread every
    // buffer. `vfpubench` prints its micro-bench now (this thread has the VFPU). A bench's first
    // scene script is already in `script`, so the flag stored here is the one the run uses.
    if let Some(s) = &script {
        jane_audio_psp::psp::VFPU_PAN.store(!s.novfpu, core::sync::atomic::Ordering::Relaxed);
        if s.vfpubench {
            say!("GAME {}", jane_audio_psp::vfpu::bench(now_us).trim_end());
        }
    }
    if let Some(bn) = bench.as_mut() {
        bn.stamp_vfpu();
    }
    let mut sound: Option<jane_audio_psp::Sound<jane_audio_psp::psp::PspHost>> =
        jane_audio_psp::psp::start(dirs, 0, script.is_some());
    let mut resumes_seen = 0u32;
    // The performance overlay (L + R + SELECT) and its window's counts.
    let mut perf = perf::Perf::default();
    if script.as_ref().is_some_and(|s| s.perf) {
        perf.toggle(now_us());
    }
    let perf_log = format!("{}perf.txt", stick.dir);
    let mut run_header = true;
    let (mut perf_up, mut perf_counts, mut perf_mix) =
        (0u32, ge.page_counts(), jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed));
    // The flips and missed vblanks (the GE's totals) at the overlay's window start, and the
    // lists the guard cut within it.
    let (mut perf_flips, mut perf_missed, mut perf_full) = (0u32, 0u32, 0u32);
    // The dashcam (PORT.md §13.13): every frame's record in a ring of the last `DASH_FRAMES`;
    // L + R + START saves it five seconds later (the ten before the press and the five after).
    // Beside it: the counts at the last frame (audio, free RAM, chunks, view jumps, fence waits,
    // lamp builds, underruns), the save's time and the press's frame, the banner's end.
    let mut dash = capture::Recorder::new(DASH_FRAMES);
    say!("GAME dashcam frames={DASH_FRAMES} bytes={}", dash.as_ref().map_or(0, capture::Recorder::bytes));
    let mut cap_mix = 0u32;
    // When `cap_mix` was last read: the wall span its mix delta covers (the bench's audio share).
    let mut cap_mix_at = 0u32;
    let mut mix_wall = 1u32;
    let mut cap_blocks = 0u32;
    let mut cap_free = (0u32, 0u32);
    let mut cap_counts = [0u32; 6];
    let mut save_at: Option<(u32, u32)> = None;
    let mut banner_until = 0u32;
    // The display's clock (PSP v0.4.1 plan, A): the vblanks the last flip took are the ticks the
    // next frame steps; flips and misses as last seen (the log's and the capture's per-frame
    // counts); the chunks painted and landed as last seen (swatches are the difference).
    let mut owed_vb = 0u32;
    let (mut flips_seen, mut missed_seen) = (0u32, 0u32);
    let (mut painted_seen, mut landed_seen) = (0u32, 0u32);
    // The work (the top of a frame to its draw) of the last eight frames, microseconds. Until
    // eight are known there is no room to assume: the painter's extra vblank waits for them.
    let mut work_ring = [16_667u32; 8];
    let mut work_n = 0usize;
    // The painter's starvation guard, and the phase of the frame before (a script's moments
    // that `tpf` steps over).
    let mut boost = Boost::default();
    let mut prev_phase = At::Title(0);
    // A script's ballast taken (once).
    let mut ballast_held = false;
    while !quit {
        diag::frame();
        #[cfg(feature = "crashtest")]
        if script.as_ref().and_then(|s| s.hang_at).is_some_and(|n| diag::frames() >= n) {
            diag::note("hangtest");
            loop {
                core::hint::spin_loop();
            }
        }
        #[cfg(feature = "crashtest")]
        if script.as_ref().and_then(|s| s.crash_at).is_some_and(|n| diag::frames() >= n) {
            panic!("crashtest");
        }
        // This frame's sim steps and presenter ticks, microseconds (the overlay's), the buffers'
        // part of them, the ticks, and the presenter's tick parts at the frame's top.
        let (mut fr_sim, mut fr_tick, mut fr_bufs, mut fr_ticks) = (0u32, 0u32, 0u32, 0u32);
        let prof0 = world.as_ref().map_or([0; 12], |wd| wd.present.prof);
        let now = now_us();
        let t_top = now;
        let dt = now.wrapping_sub(last);
        last = now;
        // The pad, with the script's presses and its walk.
        let phase = if shell.scene == Scene::Play {
            let clock = if script.as_ref().is_some_and(|s| s.framed) { ends_n as u32 } else { play_ticks };
            At::Play(sessions * SESSION + clock)
        } else {
            At::Title(title_frames)
        };
        // A script's sleep: what the hardware's does that the emulator's does not.
        if let Some(s) = &script {
            for &(at, resume) in &s.sleeps {
                if crossed(at, prev_phase, phase) {
                    say!("GAME script sleep resume={resume} tick={play_ticks}");
                    ge.spoil_vram();
                    // SAFETY: the pack's handle closed under its `File`, as a sleep leaves it stale.
                    unsafe { sys::sceIoClose(jpk.fd.get()) };
                    if resume {
                        RESUMES.fetch_add(1, Ordering::Release);
                    }
                }
            }
            for &at in &s.stales {
                if crossed(at, prev_phase, phase) {
                    // The handle stale and every page let go: each loads again through
                    // `File::read_at`'s second try on the file opened again.
                    say!("GAME script stale tick={play_ticks}");
                    // SAFETY: as above.
                    unsafe { sys::sceIoClose(jpk.fd.get()) };
                    ge.drop_pages();
                }
            }
        }
        prev_phase = phase;
        // A script's ballast set for a tick of play (the world's, in the loading branch, else).
        if let Some((b, Some(at))) = script.as_ref().and_then(|s| s.ballast) {
            if !ballast_held && at_reached(at, phase) {
                hold_ballast(b);
                ballast_held = true;
            }
        }
        // Back from a sleep (PORT.md §13.13): the GE and the display set again, VRAM's slots and
        // pools empty, the pack opened again, the clock set again.
        let resumes = RESUMES.load(Ordering::Acquire);
        let resumed_now = resumes != resumes_seen;
        if resumes != resumes_seen {
            resumes_seen = resumes;
            ge.resumed();
            lister.lamps.forget_all();
            let reopened = jpk.reopen();
            let (set, cpu, bus) = full_clock();
            say!("GAME resumed n={resumes} pack_reopened={reopened} clock set={set:x} cpu={cpu} bus={bus}");
        }
        let mut p = if script.is_some() { PspPad { buttons: 0, lx: 128, ly: 128 } } else { pad() };
        if let Some(s) = &script {
            p.buttons |= s.held(phase);
            if shell.scene == Scene::Play && s.walk {
                (p.lx, p.ly) = walk_at(play_ticks);
            }
            if let Some(&(d, _, _)) = s.sticks.iter().find(|(_, a, z)| at_reached(*a, phase) && !at_reached(*z, phase))
            {
                let a = Angle::from_degrees(d);
                let (c, sn) = (jane_core::angle::cos_q15(a).0, jane_core::angle::sin_q15(a).0);
                (p.lx, p.ly) = ((128 + c * 127 / 32_768) as u8, (128 + sn * 127 / 32_768) as u8);
            }
            let again = sessions == 0 || (sessions == 1 && !s.retitles.is_empty());
            if shell.scene == Scene::Title && s.new && again && shell.asks.is_empty() && title_frames == 2 {
                shell.asks.push(shell::Ask::NewGame);
            }
        }
        // The bench: its presses, and its results closed (bench.rs).
        bench::pad(&mut bench, &mut p, shell.scene == Scene::Play, play_ticks, &mut shell);
        let a = now_us();
        let (held, ui_input) = shell.sample(p, world.as_ref().map(|wd| &*wd.sim));
        // The title's theme on the title and while a county is built; in play, the music and
        // the beds step back while the world is held (a menu, the window), as the PC's.
        if let Some(s) = sound.as_mut() {
            match shell.scene {
                Scene::Title | Scene::Loading => s.title(),
                Scene::Play => s.bus.set_held(shell.world_held(world.as_ref().map(|wd| &wd.present))),
            }
        }
        match shell.scene {
            Scene::Title => {
                title_frames += 1;
                shell.ticks = now_us() / (1_000_000 / 60);
            }
            Scene::Loading => {
                title_frames += 1;
                // The builder's stages to the screen, and the world when it is done.
                let n = STAGES.load(Ordering::Acquire);
                if let Some(st) = shell.loading.as_mut() {
                    while loading_seen < n {
                        st.stage(stage(loading_seen));
                        loading_seen += 1;
                    }
                }
                if let Some(out) = take_built() {
                    match out {
                        Ok((mut sim, seed)) => {
                            // The sim's phases timed (`Sim::metrics`, the log's `simparts`).
                            sim.set_wall_clock(Some(now_ns));
                            say!(
                                "GAME built seed={seed} live={} peak={} hash={:016x}",
                                HEAP.live.get(),
                                HEAP.peak.get(),
                                sim.hash()
                            );
                            if let Some(st) = shell.loading.as_mut() {
                                st.seed = seed;
                                st.finish();
                            }
                            world = Some(World {
                                sim,
                                present: match make_presenter(dirs) {
                                    Some(p) => p,
                                    None => return,
                                },
                            });
                            if let (false, Some(wd)) = (lister_real, &world) {
                                lister = Lister::new(&wd.present.sprites().refs, ge.pack());
                                lister.clock = Some(now_us);
                                if let Some(o) = script.as_ref().and_then(|s| s.order) {
                                    lister.order_passes = o;
                                }
                                lister_real = true;
                            }
                            if let Some(wd) = world.as_mut() {
                                wd.present.clock = Some(now_us);
                            }
                            if let Some(e) = script.as_ref().and_then(|s| s.effects) {
                                lister.effects = e;
                            }

                            // The atmosphere's hooks: the mist tile to the GE, a script's weather.
                            if let Some(wd) = world.as_mut() {
                                ge.set_mist(&wd.present.take_mist());
                                script_weather(script_text.as_deref(), &mut wd.present, &mut wd.sim);
                            }
                            match sound.as_mut() {
                                None => {
                                    sound = jane_audio_psp::psp::start(dirs, seed, script.is_some());
                                    vol_sent = None;
                                }
                                Some(s) => s.bus.set_seed(seed),
                            }
                            say!("GAME presenter live={} peak={}", HEAP.live.get(), HEAP.peak.get());
                            // The presenter's first look at the zone (its terrain, atmosphere and
                            // the first painting of the chunks in view: 2.3 s on a PSP-1000) made
                            // here under the loading screen, not in the first play tick, which
                            // froze the game after it. Presentation only: no sim state moves.
                            // A script's stand is travelled to first (one idle step, scripts
                            // only), so the view primed is the view she plays in.
                            if let Some(wd) = world.as_mut() {
                                let t = now_us();
                                if let Some((x, y)) = script.as_ref().and_then(|s| s.stand) {
                                    let cat = jane_data::catalog();
                                    let start = cat.name(cat.story.start.mark);
                                    if let Some(mark) = wd.sim.view(Seat(0)).and_then(|v| v.sym(start)) {
                                        let at = Some(jane_core::num::Vec2::centre(x, y));
                                        wd.sim.state_mut().players[0].travel =
                                            Some(jane_sim::state::TravelRequest { zone: ZoneId::County, mark, at });
                                        let frames = [InputFrame::IDLE; jane_sim::tuning::MAX_PLAYERS];
                                        wd.sim.step(&jane_sim::input::StepInput { frames, commands: &[] });
                                        let _ = wd.sim.drain_events();
                                    }
                                }
                                if let Some(v) = wd.sim.view(Seat(0)) {
                                    wd.present.tick(&v, &[]);
                                }
                                say!("GAME primed us={}", now_us().wrapping_sub(t));
                            }
                            (painted_seen, landed_seen) = (0, 0);
                            // A script's squeeze, only now the world is built: the build needs
                            // the RAM the ballast takes.
                            if let Some((b, None)) = script.as_ref().and_then(|s| s.ballast) {
                                hold_ballast(b);
                                ballast_held = true;
                            }
                        }
                        Err(e) => {
                            say!("GAME error: {e}");
                            shell.said.push(e);
                            shell.to_title(&mut stick);
                        }
                    }
                }
                let t = now_us() / (1_000_000 / 60);
                shell.ticks = t;
                if let Some(st) = shell.loading.as_mut() {
                    st.tick(t);
                    if world.is_some() && st.done(t) {
                        shell.begin_play(built_slot);
                        let_go_log("play");
                        sessions += 1;
                        blank.ui_images.clear();
                        play_ticks = 0;
                        acc = 0;
                        travel_sent = false;
                    }
                }
            }
            Scene::Play => {}
        }
        let b = now_us();
        let _ = a;
        w.pre += b.wrapping_sub(t_top);
        // The world's ticks owed, at most four a frame; past that the clock lets go. Locked at 30
        // or 60 the clock is the display's own (the PSP v0.4.1 plan, A): a tick for each vblank
        // the last flip took, two a frame on cadence, so a position is sampled at the flip and
        // not at a loop top that jitters by 5 ms (stepping 3 ticks then 1 was the microstutter).
        // Unlocked has no vblank to count: the wall clock. A script steps exactly `tpf` ticks a
        // frame (default 1): its pad is read once a frame, so ticks caught up after a slow frame
        // would all walk with that frame's stick, and a run's state would hang on the wall
        // clock (a zone built on the step, 1.4 s, against one read back, 60 ms: two hashes).
        let by_tpf = script.as_ref().is_some_and(|s| !s.vclock);
        let wall = !by_tpf && ge.pacing == jane_render_psp::ge::Pacing::Unlocked;
        if let (Scene::Play, Some(wd)) = (shell.scene, world.as_mut()) {
            let owed = match &script {
                Some(s) if by_tpf => s.tpf.unwrap_or(1),
                _ if wall => {
                    acc = acc.saturating_add(dt);
                    (acc / TICK_US).min(4)
                }
                _ => owed_vb.min(4),
            };
            let mut n = 0;
            while n < owed {
                // The scripted dev travel: only a script's, never a player's (PORT.md §13.13).
                if !travel_sent {
                    travel_sent = true;
                    if let Some(s) = &script {
                        let mut seq = 0x7000;
                        let to = s
                            .place
                            .as_ref()
                            .and_then(|(z, mark)| wd.sim.view(Seat(0)).and_then(|v| v.sym(mark)).map(|m| (*z, m)));
                        if let Some((zone, m)) = to {
                            shell.pending.push(Command::Dev(DevOp::Tp { zone, mark: m }));
                            seq += 1;
                        }
                        // At the script's cell of the county, on a dev ground or not (the travel's
                        // own rule: the nearest free cell).
                        if let Some((x, y)) = s.stand {
                            let cat = jane_data::catalog();
                            let start = cat.name(cat.story.start.mark);
                            if let Some(mark) = wd.sim.view(Seat(0)).and_then(|v| v.sym(start)) {
                                let at = Some(jane_core::num::Vec2::centre(x, y));
                                wd.sim.state_mut().players[0].travel =
                                    Some(jane_sim::state::TravelRequest { zone: ZoneId::County, mark, at });
                            }
                        }
                        if let Some(c) = s.clock {
                            wd.sim.state_mut().clock = c;
                        } else if let Some(hour) = s.hour {
                            shell.pending.push(Command::Dev(DevOp::Time { hour }));
                        }
                        script_flags(&s.flags, &mut wd.sim);
                        let _ = seq;
                    }
                }
                // A script's timed dev travels.
                let mut tp_now = false;
                if let Some(s) = &script {
                    let now = At::Play(sessions * SESSION + play_ticks);
                    for (sp, at) in &s.learns {
                        if let (true, Some(id)) = (*at == now, jane_data::catalog().combat.spell_id(sp)) {
                            say!("GAME learn {sp} tick={play_ticks}");
                            shell.pending.push(Command::Dev(DevOp::Learn(id)));
                        }
                    }
                    if s.rests.contains(&now) {
                        say!("GAME rest save tick={play_ticks}");
                        shell.rested = true;
                    }
                    if s.retitles.contains(&now) {
                        say!("GAME retitle tick={play_ticks}");
                        shell.asks.push(shell::Ask::ToTitle);
                    }
                    for (zone, mark, at) in &s.tps {
                        if *at != now {
                            continue;
                        }
                        // `tp:<zone>:<mark>:X,Y@...`: by the mark's door, set down at that cell.
                        let (mark, cell) = match mark.split_once(':') {
                            Some((m, c)) => {
                                (m, c.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))))
                            }
                            None => (mark.as_str(), None),
                        };
                        if let Some(m) = wd.sim.view(Seat(0)).and_then(|v| v.sym(mark)) {
                            say!("GAME tp zone={} mark={mark} cell={cell:?} tick={play_ticks}", zone.name());
                            match cell {
                                Some((x, y)) => {
                                    let at = Some(jane_core::num::Vec2::centre(x, y));
                                    wd.sim.state_mut().players[0].travel =
                                        Some(jane_sim::state::TravelRequest { zone: *zone, mark: m, at });
                                }
                                None => shell.pending.push(Command::Dev(DevOp::Tp { zone: *zone, mark: m })),
                            }
                            tp_now = true;
                        }
                    }
                }
                // A script's run says what the pad sent and whom she targets (the input's proofs).
                if script.is_some() {
                    for c in shell.pending.iter().filter(|c| !matches!(c, Command::Dev(_))) {
                        say!("GAME cmd tick={play_ticks} {c:?}");
                    }
                    if shell.input.target != said_target {
                        said_target = shell.input.target;
                        say!("GAME target tick={play_ticks} {said_target:?}");
                    }
                }
                // The bench: the scene's travel and dev commands for this tick (bench.rs).
                if let Some(bn) = bench.as_mut() {
                    bn.tick(&mut wd.sim, play_ticks, &mut shell.pending);
                }
                let t0 = now_us();
                diag::note("step");
                let stepped = shell.step(
                    &mut wd.sim,
                    &mut wd.present,
                    if script.as_ref().is_some_and(|s| s.still) { InputFrame::IDLE } else { held },
                    &mut events,
                );
                if let (true, Some(s), Some(v)) = (stepped, sound.as_mut(), wd.sim.view(Seat(0))) {
                    diag::note("sound");
                    s.tick(&v, &events, &wd.present);
                }
                let t1 = now_us();
                if tp_now {
                    say!("GAME tp step us={} live={} peak={}", shell.times[0], HEAP.live.get(), HEAP.peak.get());
                }
                if stepped {
                    let [sim_us, tick_us, bufs_us] = shell.times;
                    fr_sim += sim_us;
                    fr_tick += tick_us + bufs_us;
                    fr_bufs += bufs_us;
                    fr_ticks += 1;
                    w.sim += sim_us;
                    w.sim_worst = w.sim_worst.max(sim_us);
                    let m = wd.sim.metrics();
                    for (a, ns) in w.sim_phases.iter_mut().zip(m.phase_ns) {
                        *a += ns / 1000;
                    }
                    w.tick += tick_us;
                    w.tick_worst = w.tick_worst.max(tick_us);
                    w.bufs += bufs_us;
                    w.bufs_worst = w.bufs_worst.max(bufs_us);
                    // The bench: the step's sim time; no slot written (bench.rs).
                    if let Some(bn) = bench.as_mut() {
                        bn.stepped(sim_us, &mut shell);
                    }
                }
                w.step += t1.wrapping_sub(t0);
                w.step_worst = w.step_worst.max(t1.wrapping_sub(t0));
                if wall {
                    acc -= TICK_US;
                }
                play_ticks += 1;
                w.ticks += 1;
                n += 1;
                if stepped && !script.as_ref().is_some_and(|s| s.noahead) {
                    diag::note("ahead");
                    ahead::poll(&mut wd.sim, play_ticks);
                }
            }
            if !wall || acc >= 4 * TICK_US {
                acc = 0;
            }
            let tp = now_us();
            diag::note("steps");
            if w.frames % 30 == 0 {
                diag::context(&wd.sim);
            }
            shell.after_steps(&wd.sim, &mut stick);
            diag::note("baked");
            loader::frame(&mut wd.present);
            if !script.as_ref().is_some_and(|s| s.nopaint) {
                if let Some(v) = wd.sim.view(Seat(0)) {
                    boost.zone(v.zone());
                }
                job_out = paint_jobs(&mut wd.present, &wd.sim, job_out, &mut boost);
                if boost.frame(wd.present.chunks_waiting(), job_out) {
                    w.boosts += 1;
                }
            }
            let pd = now_us().wrapping_sub(tp);
            w.post += pd;
            w.post_worst = w.post_worst.max(pd);
        }
        // The frame: the world (in play), then the UI over it. On the display's clock a frame is
        // drawn at the tick just stepped (no part of the next to blend in).
        let c = now_us();
        let alpha = if wall { (acc * 256 / TICK_US).min(255) as u8 } else { 0 };
        // In play the presenter's frame is drawn into (the UI needs the presenter shared and the
        // frame mutable: the frame is swapped out for the UI's turn and back).
        let in_play = shell.scene == Scene::Play && world.is_some();
        if let (true, Some(wd)) = (in_play, world.as_mut()) {
            diag::note("draw");
            wd.present.draw(alpha, CANVAS);
            core::mem::swap(wd.present.frame_mut(), &mut blank);
        }
        let d = now_us();
        {
            let (pres, sim) = match world.as_ref() {
                Some(wd) if in_play => (Some(&wd.present), Some(&*wd.sim)),
                _ => (None, None),
            };
            diag::note("ui");
            shell.draw(ui_input, pres, sim, &mut blank);
        }
        if let (true, Some(wd)) = (in_play, world.as_mut()) {
            core::mem::swap(wd.present.frame_mut(), &mut blank);
        }
        shell.outs(world.as_ref().map(|wd| &*wd.sim), &mut stick);
        // The Controls page's settings: kept on the stick; the volumes to the mixer.
        if core::mem::take(&mut shell.settings_changed) {
            let r = stick.put(SETTINGS, shell.settings().write().as_bytes());
            say!("GAME settings saved {:?} {r:?}", shell.settings());
        }
        if let Some(s) = sound.as_mut().filter(|_| vol_sent != Some(shell.volumes)) {
            s.bus.set_volume(shell.volumes);
            vol_sent = Some(shell.volumes);
        }
        // L + R + SELECT: the performance overlay (the flag is the shell's; the overlay the glue's).
        if core::mem::take(&mut shell.perf_toggle) {
            perf.toggle(now_us());
            (perf_up, perf_counts) = (0, ge.page_counts());
            perf_mix = jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed);
            (perf_flips, perf_missed, perf_full) = (ge.flips, ge.missed_vblanks, 0);
            if !perf.on {
                shell.overlay.clear();
            }
            say!("GAME perf overlay on={}", perf.on);
        }
        // L + R + START: the dashcam saved five seconds from now (again: now).
        if core::mem::take(&mut shell.capture_toggle) {
            match (&dash, save_at) {
                (None, _) => {
                    shell.banner = String::from("No room to capture");
                    banner_until = now_us().wrapping_add(3_000_000);
                }
                (Some(_), Some((_, press))) => save_at = Some((now_us(), press)),
                (Some(r), None) => {
                    save_at = Some((now_us().wrapping_add(CAPTURE_AFTER_US), r.pushed));
                    shell.banner = String::from("Capturing...");
                    say!("GAME capture pressed");
                }
            }
        }
        // L + R + SQUARE: the screen as it is to the stick.
        if core::mem::take(&mut shell.screen_toggle) {
            ge.flush();
            let n = (0..1000u32).find(|n| !file_exists(&format!("{}screen-{n}.bmp", stick.dir))).unwrap_or(999);
            shot(&ge, &format!("{}screen-{n}.bmp", stick.dir));
            shell.banner = format!("Screen saved {n}");
            banner_until = now_us().wrapping_add(3_000_000);
        }
        // The GE's signals time each pass for the dashcam and the overlay (a SIGNAL a pass).
        ge.timing = dash.is_some() || perf.on;
        // The Graphics page (and a script's words over it), every frame: a turn takes at once.
        let gfx = script.as_ref().map_or(shell.graphics, |s| s.gfx(shell.graphics));
        apply_graphics(gfx, &mut lister, &mut ge, world.as_mut().map(|wd| &mut wd.present));
        if let Some(p) = script.as_ref().and_then(|s| s.pacing) {
            ge.pacing = p;
        }
        // A script's `effects` bits and `atmos=N` passes left out, over the page (a bench's).
        if let Some(fx) = script.as_ref().and_then(|s| s.effects) {
            lister.effects &= fx;
        }
        lister.atmos_off |= script_atmos_bits;
        let e = now_us();
        let frame: &Frame = match world.as_ref() {
            Some(wd) if in_play => wd.present.frame(),
            _ => &blank,
        };
        let world_lister = if in_play { &mut lister } else { &mut lister_blank };
        diag::note("list");
        let lp0 = world_lister.prof;
        world_lister.build_with(frame, &mut ge.pages(&mut load));
        let mut lp_d = world_lister.prof;
        for (o, b) in lp_d.iter_mut().zip(lp0) {
            *o = o.wrapping_sub(b);
        }
        let f_world = now_us();
        ui_lister.build(frame);
        let f = now_us();
        if in_play {
            work_ring[work_n % work_ring.len()] = f.wrapping_sub(t_top);
            work_n += 1;
        }
        diag::note("ge draw");
        ge.draw(frame, if in_play { &lister } else { &lister_blank }, &ui_lister.quads, &mut load);
        let g = now_us();
        // The chunk slots the list just sent reads: a paint into one waits for the GE first.
        if let Some(wd) = world.as_mut().filter(|_| in_play && ge.pipelined) {
            let mask = lister.chunks.iter().fold(0u32, |m, c| m | 1u32.checked_shl(u32::from(c.0)).unwrap_or(0));
            wd.present.set_chunk_fence(mask, jane_render_psp::ge::wait_idle);
        }
        // A pipelined draw first waited for the last list and showed the last frame: not the
        // list's build.
        let shown_in_draw = if ge.pipelined { ge.stats.show_us } else { 0 };
        // The UI's images are the GE's now: the frame's px let go (one copy held, PORT.md §13.13).
        let drawn: &mut Frame = match world.as_mut() {
            Some(wd) if in_play => wd.present.frame_mut(),
            _ => &mut blank,
        };
        for (k, im) in drawn.ui_images.iter_mut().enumerate() {
            if !im.argb.is_empty() && ge.holds_image(k, im.generation) {
                im.argb = Vec::new();
            }
        }
        let tg = now_us();
        ge.show();
        w.show += now_us().wrapping_sub(tg) + shown_in_draw;
        w.sync += ge.stats.sync_us;
        w.list_most = w.list_most.max(ge.stats.list_bytes);
        w.pages_most = w.pages_most.max(ge.stats.frame_page_bytes);
        // The flip this frame's draw showed (the frame before's picture, pipelined): the vblanks
        // it took are the ticks the next frame steps, and those past the pacing's target are
        // misses. The loop's own period says nothing of a miss: it jitters round 33 ms with the
        // list's build after the flip.
        let vb = ge.take_flip_vblanks();
        let (flipped, missed_now) = (ge.flips.wrapping_sub(flips_seen), ge.missed_vblanks.wrapping_sub(missed_seen));
        (flips_seen, missed_seen, owed_vb) = (ge.flips, ge.missed_vblanks, vb);
        if in_play {
            (w.flips, w.vblanks, w.missed) = (w.flips + flipped, w.vblanks + vb, w.missed + missed_now);
            w.flip_max = w.flip_max.max(vb);
            w.list_full += ge.stats.list_full;
            let (qd, ps, pk) = lister.left_out();
            (w.quads_dropped, w.polys_skipped, w.patches_skipped) =
                (w.quads_dropped + qd, w.polys_skipped + ps, w.patches_skipped + pk);
            if let Some(wd) = world.as_ref() {
                // Swatches painted: the chunks painted that the painter did not land.
                let (p, l) = (wd.present.chunks_painted(), wd.present.chunks_landed());
                w.rough += p.wrapping_sub(painted_seen).saturating_sub(l.wrapping_sub(landed_seen));
                (painted_seen, landed_seen) = (p, l);
                w.waiting_max = w.waiting_max.max(wd.present.chunks_waiting() as u32);
            }
        }
        // The bench: the frame's measures (bench.rs).
        if let Some(bn) = bench.as_mut() {
            bn.frame(in_play, missed_now, vb, ge.stats.list_full, world.as_ref().map(|wd| &wd.present));
        }
        // Ground on screen still swatches: a vblank more to the painter's thread (30 fps a
        // moment rather than squares of colour; PORT.md §13.12). Only at a locked 30, and only
        // when the frames have room: the wait cuts the next frame's budget to one vblank, and a
        // frame with more than that of work then misses its flip (19 of 20 missed flips in the
        // owner's calm captures were such frames; PSP v0.4.1 plan, A). At 60 the wait would halve
        // the rate, and unlocked it is no use. While the county builds, every other vblank is the
        // builder's.
        if let Some(wd) = world.as_ref().filter(|_| shell.scene == Scene::Play) {
            w.wait_run = if wd.present.chunks_waiting() > 0 { w.wait_run + 1 } else { 0 };
            w.wait_run_max = w.wait_run_max.max(w.wait_run);
            let mush = wd.present.chunks_mush() > 0;
            (w.mush, w.mush_run) = (w.mush + u32::from(mush), if mush { w.mush_run + 1 } else { 0 });
            w.mush_run_max = w.mush_run_max.max(w.mush_run);
            if wd.present.chunks_waiting() > 0 {
                w.wait += 1;
                let work = work_ring.iter().copied().max().unwrap_or(0);
                if job_out && ge.pacing == jane_render_psp::ge::Pacing::Locked30 && work + 2_000 < 16_667 {
                    // SAFETY: a plain syscall.
                    unsafe { sys::sceDisplayWaitVblankStart() };
                    w.extra += 1;
                }
            }
        } else if shell.scene == Scene::Loading {
            // SAFETY: a plain syscall.
            unsafe { sys::sceDisplayWaitVblankStart() };
        }
        // The script's shots, once each.
        if let Some(s) = &script {
            for &at in &s.shots {
                if !shots_taken.contains(&at) && at_reached(at, phase) {
                    shots_taken.push(at);
                    let name = match at {
                        At::Title(n) => format!("{shot_dir}shot-t{n}.bmp"),
                        At::Play(n) => {
                            format!("{shot_dir}shot-{}{}.bmp", if n / SESSION == 1 { "p" } else { "q" }, n % SESSION)
                        }
                    };
                    ge.flush();
                    shot(&ge, &name);
                    say!("GAME shot-mem live={} free={:?}", HEAP.live.get(), free_mem());
                }
            }
        }
        w.frames += 1;
        // The last two seconds' frames, in play: what a script's end reports.
        if shell.scene == Scene::Play {
            ends[ends_n % ends.len()] = now_us();
            ends_n += 1;
        } else {
            ends_n = 0;
        }
        w.draw += d.wrapping_sub(c);
        w.ui += e.wrapping_sub(d);
        w.list += f.wrapping_sub(e);
        w.ge += g.wrapping_sub(f);
        w.worst = w.worst.max(g.wrapping_sub(c));
        // The presenter's tick parts this frame: the buffers', then its own (`Present::prof`).
        let mut tparts = [0u32; perf::TPARTS];
        tparts[0] = fr_bufs;
        if let Some(wd) = world.as_ref() {
            for k in 0..10 {
                tparts[k + 1] = wd.present.prof[k].wrapping_sub(prof0[k]);
            }
        }
        // The dashcam, and a bench's summary (it counts even when the ring was not allocated).
        if dash.is_some() || bench.is_some() {
            let tc = now_us();
            let st = ge.stats;
            let mut fl = [0u32; capture::field::N];
            {
                use capture::field as F;
                fl[F::FRAME] = tc.wrapping_sub(t_top);
                fl[F::SIM] = fr_sim;
                fl[F::TICK] = fr_tick - fr_bufs;
                fl[F::BUFS] = fr_bufs;
                fl[F::T_UNITS..=F::T_LIGHTS].copy_from_slice(&tparts[1..]);
                fl[F::DRAW] = d.wrapping_sub(c);
                fl[F::UI] = e.wrapping_sub(d);
                fl[F::LIST] = f.wrapping_sub(e);
                fl[F::GE_BUILD] = g.wrapping_sub(f).saturating_sub(st.sync_us + shown_in_draw);
                fl[F::GE_WAIT] = st.sync_us;
                fl[F::VBLANK] = tc.wrapping_sub(tg) + shown_in_draw;
                let mix = jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed);
                fl[F::AUDIO] = mix.wrapping_sub(cap_mix);
                cap_mix = mix;
                mix_wall = tc.wrapping_sub(cap_mix_at).max(1);
                cap_mix_at = tc;
                let blocks = jane_audio_psp::psp::BLOCKS.load(Ordering::Relaxed);
                fl[F::AUDIO_WORST] = jane_audio_psp::psp::take_worst_block_us();
                fl[F::AUDIO_BLOCKS] = blocks.wrapping_sub(cap_blocks);
                cap_blocks = blocks;
                fl[F::GE_TOTAL] = st.ge_total;
                fl[F::QUADS] = st.quads;
                fl[F::BATCHES] = st.batches;
                fl[F::BINDS] = st.binds;
                fl[F::CLUT_LOADS] = st.clut_loads;
                fl[F::MODES] = st.modes;
                // Counts since the last frame: chunks painted and landed, view jumps, fence
                // waits, lamp builds, underruns.
                let now_counts = [
                    world.as_ref().map_or(0, |wd| wd.present.chunks_painted()),
                    world.as_ref().map_or(0, |wd| wd.present.chunks_landed()),
                    world.as_ref().map_or(0, |wd| wd.present.view_jumps),
                    world.as_ref().map_or(0, |wd| wd.present.fence_waits()),
                    lister.lamps.builds,
                    jane_audio_psp::psp::UNDERRUNS.load(Ordering::Relaxed),
                ];
                let mut dc = [0u32; 6];
                for k in 0..6 {
                    dc[k] = now_counts[k].wrapping_sub(cap_counts[k]);
                }
                cap_counts = now_counts;
                if let Some(wd) = world.as_ref().filter(|_| in_play) {
                    let fr = wd.present.frame();
                    fl[F::LIGHTS] = fr.lights.len() as u32;
                    fl[F::CASTERS] = fr.casters.len() as u32;
                    fl[F::PARTICLES] = fr.parts.len() as u32;
                    fl[F::EV_PLACEHOLDERS] = wd.present.chunks_waiting() as u32;
                }
                fl[F::PAINTED] = dc[0];
                fl[F::LANDED] = dc[1];
                fl[F::EV_ROUGH] = dc[0].wrapping_sub(dc[1]);
                fl[F::EV_LANDED] = dc[1];
                fl[F::EV_VIEW_JUMP] = dc[2];
                fl[F::EV_FENCE] = dc[3];
                fl[F::EV_LAMP_BUILD] = dc[4];
                fl[F::EV_UNDERRUN] = dc[5];
                fl[F::EV_LAMP_UPLOAD] = lister.lamps.uploads.len() as u32;
                fl[F::EV_EVICT] = st.evicts;
                fl[F::EV_SLOT_UPLOAD] = st.uploads;
                fl[F::EV_RT] = st.rts;
                fl[F::EV_STENCIL] = st.stencil;
                fl[F::EV_GE_ERROR] = st.ge_error;
                fl[F::EV_WB] = st.wb;
                fl[F::EV_WB_KB] = st.wb_bytes / 1024;
                fl[F::EV_RESUME] = u32::from(resumed_now);
                fl[F::EV_CHUNK_NEW] = st.chunks_converted;
                fl[F::SLABS] = lister.slab_count;
                fl[F::PAGE_LOADS] = st.page_loads;
                fl[F::UPLOADS] = st.uploads;
                // Free RAM read every 16th frame (the call walks the kernel's lists).
                if dash.as_ref().is_some_and(|rec| rec.frames() % 16 == 15) {
                    cap_free = free_mem();
                }
                (fl[F::LARGEST], fl[F::FREE]) = cap_free;
                fl[F::TICKS] = fr_ticks;
                // The flip this frame's draw showed: its vblanks, and a miss when it took more
                // than the pacing's target (the loop period is no sign of one).
                fl[F::VBLANKS] = vb;
                fl[F::LATE] = u32::from(missed_now > 0);
                fl[F::LIST_KB] = st.list_bytes / 1024;
                fl[F::EV_LIST_FULL] = st.list_full;
                fl[F::OVERHEAD] = now_us().wrapping_sub(tc);
            }
            if let Some(rec) = dash.as_mut() {
                rec.push(&fl, &st.passes);
                if let Some((at, press)) = save_at.filter(|s| now_us().wrapping_sub(s.0) < 0x8000_0000) {
                    save_at = None;
                    let mut header = capture_header(&shell, world.as_ref().map(|wd| &*wd.sim), script_text.as_deref());
                    let _ = writeln!(header, "press_frame {}", rec.pushed.wrapping_sub(press).min(rec.frames() as u32));
                    let _ = writeln!(header, "pacing {:?} pipelined {}", ge.pacing, ge.pipelined);
                    let _ = at;
                    save_capture(rec, &header, &stick, &mut shell);
                    banner_until = now_us().wrapping_add(3_000_000);
                }
            }
            if let Some(bn) = bench.as_mut() {
                // The frame's CPU parts (`capture::summary::PARTS`): the world's draw and the
                // UI's, then the world list's own timers.
                let parts = {
                    let top = lp_d[1] + lp_d[5] + lp_d[6] + lp_d[10] + lp_d[12];
                    let world_us = f_world.wrapping_sub(e);
                    [
                        d.wrapping_sub(c),
                        e.wrapping_sub(d),
                        f.wrapping_sub(f_world),
                        lp_d[1],
                        lp_d[3],
                        lp_d[4],
                        lp_d[5],
                        lp_d[8],
                        lp_d[9],
                        lp_d[6],
                        lp_d[12],
                        lp_d[10],
                        world_us.saturating_sub(top),
                    ]
                };
                // The wall time the frame's mix delta covers (the last read of `cap_mix` to this
                // one), as the overlay divides its mix by its own window's span.
                bn.record(dash.as_ref(), &fl, &st.passes, &parts, in_play, mix_wall);
            }
        }
        if banner_until != 0 && now_us().wrapping_sub(banner_until) < 0x8000_0000 {
            shell.banner.clear();
            banner_until = 0;
        }
        if perf.on {
            let now = now_us();
            let sync = ge.stats.sync_us;
            perf.detail(&ge.stats.passes, ge.stats.ge_total, &tparts);
            let parts = [
                fr_sim,
                fr_tick,
                d.wrapping_sub(c),
                e.wrapping_sub(d),
                f.wrapping_sub(e),
                g.wrapping_sub(f).saturating_sub(sync + shown_in_draw),
                sync,
                now.wrapping_sub(tg) + shown_in_draw,
            ];
            perf_up += ge.stats.uploads;
            perf_full += ge.stats.list_full;
            if perf.frame(now, parts) {
                let (cpu, bus) =
                    // SAFETY: plain syscalls.
                    unsafe { (sys::scePowerGetCpuClockFrequencyInt(), sys::scePowerGetBusClockFrequencyInt()) };
                let (largest, free) = free_mem();
                let counts = ge.page_counts();
                let mix = jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed);
                let facts = perf::Facts {
                    cpu,
                    bus,
                    mix_us: mix.wrapping_sub(perf_mix),
                    free,
                    largest,
                    slots: ge.slots_filled(),
                    uploads: perf_up,
                    hits: counts.0.wrapping_sub(perf_counts.0),
                    loads: counts.1.wrapping_sub(perf_counts.1),
                    load_fails: ge.stats.page_load_fails,
                    waiting: world.as_ref().map_or(0, |wd| wd.present.chunks_waiting()),
                    job_out,
                    zones: world.as_ref().map_or(0, |wd| {
                        jane_core::ZoneId::ALL.iter().filter(|&&z| wd.sim.blueprints().held_now(z).is_some()).count()
                    }),
                    ahead: ahead::state(),
                    resumes: RESUMES.load(Ordering::Relaxed),
                    reopens: jpk.reopens.get(),
                    paint_fails: diag::paint_fails(),
                    flips: ge.flips.wrapping_sub(perf_flips),
                    missed: ge.missed_vblanks.wrapping_sub(perf_missed),
                    boosts: boost.total,
                    list_full: perf_full,
                };
                (perf_up, perf_counts, perf_mix) = (0, counts, mix);
                (perf_flips, perf_missed, perf_full) = (ge.flips, ge.missed_vblanks, 0);
                if let Some(text) = perf.close(now, &facts) {
                    if perf.log_bytes < perf::LOG_MOST {
                        let first = perf.log_bytes == 0;
                        // Each run's lines begin with who it was (the build, the seed, the settings).
                        let text = if run_header {
                            run_header = false;
                            let seed = world.as_ref().map_or(0, |wd| wd.sim.state().seed);
                            let mut set = String::new();
                            for l in shell.settings().write().lines() {
                                set.push_str(l);
                                set.push(' ');
                            }
                            format!(
                                "run commit {} stamp {:016x} seed {seed} settings {}
{text}",
                                env!("JANE_COMMIT"),
                                jane_world::SOURCE_STAMP,
                                set.trim_end()
                            )
                        } else {
                            text
                        };
                        perf.log_bytes = append_file(&perf_log, text.as_bytes()).unwrap_or(perf::LOG_MOST);
                        // A log full from an earlier run begins again (this run's numbers kept).
                        if first && perf.log_bytes >= perf::LOG_MOST && write_file(&perf_log, text.as_bytes()).is_ok() {
                            perf.log_bytes = text.len() as u32;
                        }
                    }
                }
                shell.overlay.clone_from(&perf.lines);
            }
        }
        // The bench: the title's Benchmark row is New Game on the bench's script (bench.rs).
        if let Some(mut bn) = bench::start_asked(&mut shell, &stick.dir, &jpk_path) {
            script = Some(bn.script());
            // The flag was stored at boot (no scene script yet). Stamp the header from it.
            bn.stamp_vfpu();
            bench = Some(bn);
            sessions = 0;
        }
        // The asks: a world to build or load, the title, the end.
        // The GE done with the frame's data first: a world let go frees what its list reads.
        if !shell.asks.is_empty() {
            ge.flush();
        }
        for ask in core::mem::take(&mut shell.asks) {
            match ask {
                shell::Ask::NewGame => {
                    world = None;
                    loading_seen = 0;
                    built_slot = None;
                    let seed = script.as_ref().and_then(|s| s.seed).unwrap_or_else(clock_seed);
                    ge.drop_pages();
                    shell.begin_loading(seed, "New Game");
                    blank.ui_images.clear();
                    say!("GAME new game seed={seed} live={}", HEAP.live.get());
                    loader::renew();
                    let_go_log("new game");
                    start_build(BuildJob {
                        seed,
                        reroll: true,
                        save: None,
                        ground: script.as_ref().and_then(|s| s.ground),
                    });
                }
                shell::Ask::Load { slot, bytes, seed } => {
                    world = None;
                    loading_seen = 0;
                    built_slot = Some(slot);
                    shell.begin_loading(seed, if county_cache::has(seed) { "Reading the county" } else { "Load" });
                    ge.drop_pages();
                    blank.ui_images.clear();
                    say!("GAME load slot={} seed={seed} bytes={} live={}", slot + 1, bytes.len(), HEAP.live.get());
                    loader::renew();
                    let_go_log("load");
                    start_build(BuildJob { seed, reroll: false, save: Some(bytes), ground: None });
                }
                shell::Ask::ToTitle => {
                    world = None;
                    ge.drop_pages();
                    loader::renew();
                    let_go_log("title");
                    shell.to_title(&mut stick);
                    title_frames = 0;
                }
                shell::Ask::Quit => quit = true,
                // Made New Game by `bench::start_asked` above.
                shell::Ask::Bench => {}
            }
        }
        for s in core::mem::take(&mut shell.said) {
            say!("GAME said: {s}");
        }
        let span = now_us().wrapping_sub(w.start);
        let done = script.as_ref().is_some_and(|s| {
            shell.scene == Scene::Play && (play_ticks >= s.end || s.frames.is_some_and(|n| ends_n as u32 >= n))
        });
        // The bench: a scene's end is the next one's start, the last one's the results (bench.rs).
        let wsim = world.as_ref().map(|wd| &*wd.sim);
        let done = done
            && bench::scene_end(
                &mut bench,
                &mut script,
                wsim,
                &mut shell,
                &mut play_ticks,
                &mut travel_sent,
                dash.as_ref(),
                &format!("pacing {:?} pipelined {}\n", ge.pacing, ge.pipelined),
            );
        if span >= 2_000_000 || done {
            let amb = world.as_mut().map_or([0; 4], |wd| wd.present.take_ambient_prof());
            w.at_tick = play_ticks;
            log_window(&w, span, &ge, &lister, &ui_lister, world.as_ref(), shell.scene, amb);
            lister.prof = [0; 13];
            ge.parts = [0; 4];
            if let Some(wd) = world.as_mut() {
                wd.present.prof = [0; 12];
            }
            w = Window {
                start: now_us(),
                mix0: jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed),
                wait_run: w.wait_run,
                mush_run: w.mush_run,
                ..Window::default()
            };
        }
        if done {
            let n = ends.len();
            if ends_n >= n {
                let (last, first) = (ends[(ends_n - 1) % n], ends[ends_n % n]);
                let us = last.wrapping_sub(first).max(1);
                say!(
                    "GAME fps2s={}.{} over={}us",
                    (n as u32 - 1) * 1_000_000 / us,
                    (n as u32 - 1) * 10_000_000 / us % 10,
                    us
                );
            }
            // The night as latched (NIGHT.md §3.2): its stage and the tick it turned.
            let night = world.as_ref().map(|wd| wd.sim.state().night).unwrap_or_default();
            say!(
                "GAME done ticks={play_ticks} peak={} hash={:016x} night_stage={} turned_at={} pack_reopens={} load_fails={}",
                HEAP.peak.get(),
                world.as_ref().map_or(0, |wd| wd.sim.hash()),
                night.stage,
                night.turned_at.0,
                jpk.reopens.get(),
                ge.stats.page_load_fails
            );
            return;
        }
    }
}

/// The Graphics page's settings to the lister's effects, the GE's pacing and the rain.
fn apply_graphics(g: jane_present::gfx_psp::Graphics, lister: &mut Lister, ge: &mut Ge, present: Option<&mut Present>) {
    use jane_present::gfx_psp::{Effect as E, FrameRate};
    use jane_render_psp::list::{atmos_fx as A, fx};
    let mut effects = fx::GLOW;
    for (e, bits) in [
        (E::Relief, fx::RELIEF | fx::LAMP_RELIEF),
        (E::SunShadows, fx::SHADOWS),
        (E::Lamps, fx::LAMPS),
        (E::LampShadows, fx::LAMP_SHADOWS),
    ] {
        if g.has(e) {
            effects |= bits;
        }
    }
    let mut off = 0;
    for (e, bits) in [
        (E::Grade, A::GRADE | A::SATURATION),
        (E::Fog, A::FOG),
        (E::Shafts, A::SHAFTS),
        (E::Reflections, A::REFLECT | A::PUDDLES | A::STREAKS),
        (E::Particles, A::PARTICLES),
    ] {
        if !g.has(e) {
            off |= bits;
        }
    }
    lister.effects = effects;
    lister.palette_grade = !g.grade_full;
    lister.atmos_off = off;
    ge.pacing = match g.rate {
        FrameRate::Sixty => jane_render_psp::ge::Pacing::Vsync,
        FrameRate::Thirty => jane_render_psp::ge::Pacing::Locked30,
        FrameRate::Unlocked => jane_render_psp::ge::Pacing::Unlocked,
    };
    if let Some(p) = present {
        p.set_rain(g.has(E::Rain));
        // The night's overlays: Full 96, Balanced 48, Fast 24 (NIGHT.md §4.3).
        p.set_night_cap(jane_present::night::console_cap(g));
    }
}

/// The presenter from its tables: no generator runs but the terrain painter's own.
fn make_presenter(dirs: &[String]) -> Option<Present> {
    let t0 = now_us();
    let bytes = find(dirs, "present.jpt").and_then(|(_, f)| f.read_all())?;
    let mut present = match Present::from_tables_console(Tier::T0, &bytes, CHUNK_SLOTS) {
        Ok(p) => p,
        Err(e) => {
            say!("GAME error: tables: {e:?}");
            return None;
        }
    };
    drop(bytes);
    present.set_features(Features::c2());
    present.set_canvas(CANVAS);
    if let Some((cols, rows)) = loader::grid() {
        present.set_baked(cols, rows);
    }
    say!(
        "GAME present us={} live={} peak={} mem={:?}",
        now_us().wrapping_sub(t0),
        HEAP.live.get(),
        HEAP.peak.get(),
        present.mem()
    );
    Some(present)
}
