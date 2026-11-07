//! The threads below the game's: the terrain painter's worker (PORT.md §13.12) with its
//! starvation guard `Boost` (`PAINTER_BOOST_FRAMES`, `PAINTER_ENTRY_FRAMES`), `start_thread`,
//! and the builder that makes a New Game, Continue or Load (PORT.md §13.13) and reports its
//! stages to the loading screen.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::sync::atomic::{AtomicPtr, Ordering};

use alloc::format;

use crate::{county_cache, diag};
use jane_present::terrain::PaintJob;
use jane_present::Present;
use jane_sim::{Seat, Sim};

use crate::platform::{let_go_log, now_us, HEAP, THREADED};
use crate::Line;
use psp::sys;

// ---------------------------------------------------------------- the painter's worker
// PORT.md §13.12: a chunk takes the terrain painter about 100 ms on a PSP, so it runs on a thread
// below the game's priority, in the time the game waits for the vblank and the GE. The game
// hands it one job at a time (`Present::take_paint_job`) and lands it when it is back.

static TODO: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static DONE: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static SEMA: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);
/// The painter's thread (its stack's use, in the log).
pub(crate) static PAINTER: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);
/// The painter's jobs run and their wall time, microseconds (its own time and the game's
/// between).
pub(crate) static JOBS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub(crate) static JOB_US: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// `slowpaint:PCT`: a job holds the painter's thread this share of its own time, percent.
pub(crate) static SLOW_PAINT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(100);

extern "C" fn worker(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    loop {
        // SAFETY: a semaphore made before this thread started.
        unsafe { sys::sceKernelWaitSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1, core::ptr::null_mut()) };
        let p = TODO.swap(core::ptr::null_mut(), Ordering::AcqRel);
        if !p.is_null() {
            let t = now_us();
            // SAFETY: the game gave up the job when it stored it, and takes it back from DONE.
            unsafe { (*p).run() };
            // `slowpaint`: the thread held on, spinning below the game as the work would.
            let slow = SLOW_PAINT.load(Ordering::Relaxed);
            if slow > 100 {
                let until = now_us().wrapping_sub(t) * (slow - 100) / 100;
                let s = now_us();
                while now_us().wrapping_sub(s) < until {}
            }
            JOB_US.fetch_add(now_us().wrapping_sub(t), Ordering::Relaxed);
            JOBS.fetch_add(1, Ordering::Relaxed);
            DONE.store(p, Ordering::Release);
        }
    }
}

/// A thread running `entry`, made once at boot while its stack can still be had.
pub(crate) fn start_thread(
    name: &[u8],
    entry: extern "C" fn(usize, *mut core::ffi::c_void) -> i32,
    prio: i32,
    stack: i32,
) -> i32 {
    // SAFETY: a thread, made once; it runs for the program's life.
    unsafe {
        let id = sys::sceKernelCreateThread(
            name.as_ptr(),
            entry,
            prio,
            stack,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        );
        let started = sys::sceKernelStartThread(id, 0, core::ptr::null_mut());
        say!(
            "GAME thread {} id={:x} start={started:x}",
            core::str::from_utf8(&name[..name.len() - 1]).unwrap_or("?"),
            id.0
        );
        id.0
    }
}

pub(crate) fn start_workers() {
    // SAFETY: semaphores made once, before the threads that wait on them.
    unsafe {
        let sema = sys::sceKernelCreateSema(b"paint\0".as_ptr(), 0, 0, 1, core::ptr::null_mut());
        SEMA.store(sema.0, Ordering::Release);
    }
    // The terrain painter's job uses a few KB of stack (its scratch is the heap's).
    PAINTER.store(start_thread(b"painter\0", worker, PAINTER_PRIO, 32 * 1024), Ordering::Relaxed);
}

/// The painter's thread priority, and the one it is raised to when starved (the game's thread is
/// 32, a lower number runs first; the audio thread, 16, stays above both).
const PAINTER_PRIO: i32 = 48;
const PAINTER_BOOST_PRIO: i32 = 31;
/// Frames in a row with swatches showing and a job out, without it landing, before the painter
/// is raised above the game's thread for the rest of that job (about 3 s at a steady 30).
const PAINTER_BOOST_FRAMES: u32 = 90;
/// Frames after she enters a zone other than the county (painted at runtime, not baked) in
/// which the painter is raised at once while ground shows as swatches: the room comes in a
/// few slow frames (about 100 ms a chunk on the PSP) instead of seconds of swatches, the
/// painter running only while the game slept (the owner, 2026-10-27: "indoor painting is
/// pretty slow still").
const PAINTER_ENTRY_FRAMES: u32 = 600;

/// The painter's starvation guard: it runs only while the game's thread sleeps, so a game that
/// never sleeps (every frame late) leaves ground as swatches for seconds. After
/// [`PAINTER_BOOST_FRAMES`] frames of that the painter outranks the game for one job (at most
/// the 100 ms or so a chunk takes), which bounds the stall at one job.
#[derive(Debug, Default)]
pub(crate) struct Boost {
    /// Frames running with swatches showing, a job out and none landed.
    starved: u32,
    /// The painter is raised now.
    on: bool,
    /// Times raised, in the run.
    pub(crate) total: u32,
    /// Frames left in which a zone just entered raises the painter at once
    /// ([`PAINTER_ENTRY_FRAMES`]); 0 once its view is painted.
    entry: u32,
    /// The zone seen last frame.
    zone: Option<jane_core::ZoneId>,
}

impl Boost {
    fn set(prio: i32) {
        // SAFETY: a plain syscall on the painter's thread (made at boot).
        unsafe { sys::sceKernelChangeThreadPriority(sys::SceUid(PAINTER.load(Ordering::Relaxed)), prio) };
    }

    /// A frame ended: swatches `waiting` on screen with a job `out`. Raises the painter once
    /// the stall is [`PAINTER_BOOST_FRAMES`] long; returns true on the frame it does.
    pub(crate) fn frame(&mut self, waiting: usize, out: bool) -> bool {
        self.entry = self.entry.saturating_sub(1);
        if !(out && waiting > 0) {
            self.starved = 0;
            if waiting == 0 {
                self.entry = 0;
            }
            return false;
        }
        self.starved += 1;
        if self.on || (self.entry == 0 && self.starved < PAINTER_BOOST_FRAMES) {
            return false;
        }
        Self::set(PAINTER_BOOST_PRIO);
        diag::note("painter boost");
        (self.on, self.total) = (true, self.total + 1);
        say!("GAME painter boost n={} after {} frames", self.total, self.starved);
        true
    }

    /// She is in `zone` this frame: one entered other than the county starts the entry's frames.
    pub(crate) fn zone(&mut self, zone: jane_core::ZoneId) {
        if self.zone.replace(zone) != Some(zone) && zone != jane_core::ZoneId::County {
            self.entry = PAINTER_ENTRY_FRAMES;
        }
    }

    /// A job landed: the stall is over, and the painter goes back below the game.
    fn landed(&mut self) {
        self.starved = 0;
        if core::mem::take(&mut self.on) {
            Self::set(PAINTER_PRIO);
        }
    }
}

/// Lands a job the worker finished and hands it the next; true while one is out. `boost` keeps
/// the painter from starving.
pub(crate) fn paint_jobs(present: &mut Present, sim: &Sim, out: bool, boost: &mut Boost) -> bool {
    let mut out = out;
    let done = DONE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !done.is_null() {
        // SAFETY: the worker stored it and no longer touches it.
        diag::note("paint land");
        present.land(*unsafe { alloc::boxed::Box::from_raw(done) });
        out = false;
        boost.landed();
    }
    if !out {
        if let Some(v) = sim.view(Seat(0)) {
            diag::job_scope(true);
            let job = present.take_paint_job(&v);
            diag::job_scope(false);
            if let Some(job) = job {
                diag::note("paint job out");
                THREADED.store(true, Ordering::Release);
                TODO.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(job)), Ordering::Release);
                // SAFETY: the semaphore the worker waits on.
                unsafe { sys::sceKernelSignalSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1) };
                out = true;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- the builder
// PORT.md §13.13: a New Game, Continue or Load is made on a thread below the game's, so the
// loading screen draws and moves meanwhile; each stage it reports is queued for the screen. The
// county is read from its baked file (`county.bp`, never built); any other zone a save names is read
// from the stick's cache or built (`county_cache`).

/// The builder's stack: worldgen's deepest recursion fits in it, with room.
const BUILD_STACK: i32 = 384 * 1024;

/// What to build: a county from a seed (re-rolled while it does not prove, for a seed nobody
/// chose), and a save to load over it.
pub(crate) struct BuildJob {
    pub(crate) seed: u32,
    pub(crate) reroll: bool,
    pub(crate) save: Option<Vec<u8>>,
    /// A script's dev ground in the county's place.
    pub(crate) ground: Option<jane_sim::dev_ground::Ground>,
}

/// What came back: the world and its seed, or why not.
type Built = Result<(alloc::boxed::Box<Sim>, u32), String>;

static BUILD_TODO: AtomicPtr<BuildJob> = AtomicPtr::new(core::ptr::null_mut());
static BUILD_DONE: AtomicPtr<Built> = AtomicPtr::new(core::ptr::null_mut());
/// The stages reported, as `&'static str` pointers and lengths, and how many.
const STAGE_MAX: usize = 64;
static STAGE_PTR: [AtomicPtr<u8>; STAGE_MAX] = [const { AtomicPtr::new(core::ptr::null_mut()) }; STAGE_MAX];
static STAGE_LEN: [core::sync::atomic::AtomicUsize; STAGE_MAX] =
    [const { core::sync::atomic::AtomicUsize::new(0) }; STAGE_MAX];
pub(crate) static STAGES: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

fn report(stage: &'static str) {
    let n = STAGES.load(Ordering::Acquire);
    if n < STAGE_MAX {
        STAGE_PTR[n].store(stage.as_ptr().cast_mut(), Ordering::Relaxed);
        STAGE_LEN[n].store(stage.len(), Ordering::Relaxed);
        STAGES.store(n + 1, Ordering::Release);
    }
}

/// Stage `i` as reported.
pub(crate) fn stage(i: usize) -> &'static str {
    let (p, n) = (STAGE_PTR[i].load(Ordering::Relaxed), STAGE_LEN[i].load(Ordering::Relaxed));
    // SAFETY: `report` stored a `&'static str`'s pointer and length.
    unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(p, n)) }
}

fn build(job: &BuildJob) -> Built {
    let mut seed = job.seed;
    let mut tries = 1;
    let t = now_us();
    // The county alone, read from the stick or built; every other zone as she walks in (or
    // ahead of her), and let go once she has left (PORT.md §13.3).
    let bps = loop {
        match county_cache::blueprints(seed, &mut report) {
            Err(jane_sim::blueprints::BuildError(_, jane_world::ZoneError::Unproven(_)))
                if job.reroll && tries < jane_sim::blueprints::REROLLS =>
            {
                seed = jane_sim::blueprints::next_seed(seed);
                tries += 1;
            }
            Err(e) => return Err(format!("seed {seed}: {e}")),
            Ok(b) => break b,
        }
    };
    let bps = match job.ground {
        Some(g) => jane_sim::dev_ground::stand_in(bps, g),
        None => bps,
    };
    say!("GAME world seed={seed} us={} live={} peak={}", now_us().wrapping_sub(t), HEAP.live.get(), HEAP.peak.get());
    let sim = match &job.save {
        Some(bytes) => Sim::from_save_with(bytes, bps).map_err(|e| format!("{e:?}"))?,
        None => Sim::new_game_with(bps, "Jane"),
    };
    Ok((alloc::boxed::Box::new(sim), seed))
}

/// One build, then the thread ends and its stack is given back: play has the RAM.
extern "C" fn builder(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    let p = BUILD_TODO.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        // SAFETY: the game gave up the job when it stored it.
        let job = unsafe { alloc::boxed::Box::from_raw(p) };
        let out = build(&job);
        drop(job);
        let_go_log("built");
        BUILD_DONE.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(out)), Ordering::Release);
    }
    // SAFETY: this thread's own end; nothing of it is used after.
    unsafe { sys::sceKernelExitDeleteThread(0) };
    0
}

/// A build on a thread of its own, made now (the world let go, its RAM free for the stack).
pub(crate) fn start_build(job: BuildJob) {
    STAGES.store(0, Ordering::Release);
    THREADED.store(true, Ordering::Release);
    BUILD_TODO.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(job)), Ordering::Release);
    let _ = start_thread(b"builder\0", builder, 40, BUILD_STACK);
}

pub(crate) fn take_built() -> Option<Built> {
    let p = BUILD_DONE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    // SAFETY: the builder stored it and no longer touches it.
    (!p.is_null()).then(|| *unsafe { alloc::boxed::Box::from_raw(p) })
}

/// A seed nobody chose: the clock's.
pub(crate) fn clock_seed() -> u32 {
    let mut t = 0u64;
    // SAFETY: one read into our u64.
    unsafe { sys::sceRtcGetCurrentTick(&mut t) };
    (t ^ (t >> 32)) as u32 | 1
}
