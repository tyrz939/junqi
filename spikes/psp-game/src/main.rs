#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

//! PORT.md §13.12: the first playable PSP build. Boots, reads the presenter's tables
//! (`present.jpt`) and the PSP pack's tables (`jane-psp.jpk`) from beside the program (or
//! `host0:/`), builds the county from the seed (packed blueprints), starts New Game, and runs:
//! the pad to the sim's input, the sim at its 60 ticks a second, the `Frame` from
//! `jane-present`, drawn by `jane-render-psp` on the GE, at the display's vblank. Prints its fps,
//! timings and heap to fd 1 every two seconds. With a `script.txt` beside it (a tick count), it
//! walks her along a fixed path through the town instead of reading the pad, and exits at that
//! tick showing the frame (what the headless runner's screenshot catches). Integers only; the
//! only unsafe is the platform glue.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, UnsafeCell};
use core::fmt::Write;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use jane_core::{Angle, ZoneId};
use jane_present::terrain::PaintJob;
use jane_present::{Features, Present, Tier};
use jane_render_psp::ge::Ge;
use jane_render_psp::{Lister, Pack};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};
use psp::sys;
use talc::{ErrOnOom, Span, Talc};

// ---------------------------------------------------------------- output

struct Line {
    buf: [u8; 512],
    len: usize,
}

impl Line {
    const fn new() -> Line {
        Line { buf: [0; 512], len: 0 }
    }
    fn flush(&mut self) {
        // SAFETY: fd 1 is the headless runner's stdout; the buffer outlives the call.
        unsafe { sys::sceIoWrite(sys::SceUid(1), self.buf.as_ptr().cast(), self.len) };
        self.len = 0;
    }
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let n = b.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&b[..n]);
        self.len += n;
        Ok(())
    }
}

macro_rules! say {
    ($($t:tt)*) => {{
        let mut l = Line::new();
        let _ = write!(l, $($t)*);
        let _ = l.write_str("\n");
        l.flush();
    }};
}

#[no_mangle]
pub fn __spike_panic(info: &core::panic::PanicInfo<'_>) -> ! {
    say!("GAME panic: {} live={} peak_heap={}", info, HEAP.live.get(), HEAP.peak.get());
    // SAFETY: plain syscall; it does not return.
    unsafe { sys::sceKernelExitGame() };
    loop {}
}

// ---------------------------------------------------------------- heap
// As §13.10's spike: a talc pool for small allocations, a kernel block each for big ones, live
// and peak bytes counted as requested.

struct Heap {
    talc: UnsafeCell<Talc<ErrOnOom>>,
    lo: Cell<usize>,
    hi: Cell<usize>,
    live: Cell<usize>,
    peak: Cell<usize>,
    allocs: Cell<u32>,
}

const BIG: usize = 64 * 1024;
const SMALL_ARENA: u32 = 6 * 1024 * 1024;

// SAFETY: one thread allocates.
unsafe impl Sync for Heap {}

#[global_allocator]
static HEAP: Heap = Heap {
    talc: UnsafeCell::new(Talc::new(ErrOnOom)),
    lo: Cell::new(0),
    hi: Cell::new(0),
    live: Cell::new(0),
    peak: Cell::new(0),
    allocs: Cell::new(0),
};

impl Heap {
    unsafe fn small(&self, layout: Layout) -> *mut u8 {
        (*self.talc.get()).malloc(layout).map_or(core::ptr::null_mut(), NonNull::as_ptr)
    }

    unsafe fn big(&self, layout: Layout) -> *mut u8 {
        let size = layout.size() + core::mem::size_of::<sys::SceUid>() + layout.align();
        let id = sys::sceKernelAllocPartitionMemory(
            sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
            b"big\0".as_ptr(),
            sys::SceSysMemBlockTypes::High,
            size as u32,
            core::ptr::null_mut(),
        );
        if id.0 < 0 {
            return core::ptr::null_mut();
        }
        let mut p: *mut u8 = sys::sceKernelGetBlockHeadAddr(id).cast();
        p.cast::<sys::SceUid>().write_unaligned(id);
        p = p.add(core::mem::size_of::<sys::SceUid>());
        let pad = 1 + p.add(1).align_offset(layout.align());
        *p.add(pad - 1) = pad as u8;
        p.add(pad)
    }
}

/// Set once the painter's worker thread runs: from then on an allocation holds the scheduler
/// (the pools are not thread-safe, and a spin lock would never yield to a lower priority).
static THREADED: AtomicBool = AtomicBool::new(false);

struct Hold(Option<i32>);

impl Hold {
    fn new() -> Hold {
        // SAFETY: plain syscall; resumed in `drop`.
        Hold(THREADED.load(Ordering::Relaxed).then(|| unsafe { sys::sceKernelSuspendDispatchThread() }))
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        if let Some(st) = self.0 {
            // SAFETY: the state `new` took.
            unsafe { sys::sceKernelResumeDispatchThread(st) };
        }
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _hold = Hold::new();
        let p = if layout.size() >= BIG {
            let p = self.big(layout);
            if p.is_null() { self.small(layout) } else { p }
        } else {
            let p = self.small(layout);
            if p.is_null() { self.big(layout) } else { p }
        };
        if !p.is_null() {
            let live = self.live.get() + layout.size();
            self.live.set(live);
            if live > self.peak.get() {
                self.peak.set(live);
            }
            self.allocs.set(self.allocs.get().wrapping_add(1));
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _hold = Hold::new();
        let a = ptr as usize;
        if a >= self.lo.get() && a < self.hi.get() {
            (*self.talc.get()).free(NonNull::new_unchecked(ptr), layout);
        } else {
            let pad = *ptr.sub(1) as usize;
            let id = ptr.sub(pad + core::mem::size_of::<sys::SceUid>()).cast::<sys::SceUid>().read_unaligned();
            sys::sceKernelFreePartitionMemory(id);
        }
        self.live.set(self.live.get() - layout.size());
    }
}

fn heap_init() -> u32 {
    // SAFETY: syscalls; the block is never freed and is handed whole to talc.
    unsafe {
        let max = sys::sceKernelMaxFreeMemSize() as u32;
        let id = sys::sceKernelAllocPartitionMemory(
            sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
            b"jane-heap\0".as_ptr(),
            sys::SceSysMemBlockTypes::Low,
            SMALL_ARENA,
            core::ptr::null_mut(),
        );
        if id.0 < 0 {
            say!("GAME error: cannot allocate the small pool ({:x})", id.0);
            return max;
        }
        let base: *mut u8 = sys::sceKernelGetBlockHeadAddr(id).cast();
        HEAP.lo.set(base as usize);
        HEAP.hi.set(base as usize + SMALL_ARENA as usize);
        let _ = (*HEAP.talc.get()).claim(Span::from_base_size(base, SMALL_ARENA as usize));
        max
    }
}

fn free_mem() -> (u32, u32) {
    // SAFETY: plain syscalls.
    unsafe { (sys::sceKernelMaxFreeMemSize() as u32, sys::sceKernelTotalFreeMemSize() as u32) }
}

fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

// ---------------------------------------------------------------- files

/// A file, open for reading.
struct File(sys::SceUid);

impl File {
    fn open(path: &str) -> Option<File> {
        let mut z = Vec::with_capacity(path.len() + 1);
        z.extend_from_slice(path.as_bytes());
        z.push(0);
        // SAFETY: a NUL-terminated path that outlives the call.
        let fd = unsafe { sys::sceIoOpen(z.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        (fd.0 >= 0).then_some(File(fd))
    }

    fn size(&self) -> u32 {
        // SAFETY: an open fd.
        unsafe {
            let n = sys::sceIoLseek(self.0, 0, sys::IoWhence::End);
            sys::sceIoLseek(self.0, 0, sys::IoWhence::Set);
            n as u32
        }
    }

    /// Fills `buf` from byte `at`.
    fn read_at(&self, at: u32, buf: &mut [u8]) -> bool {
        // SAFETY: an open fd; `buf` is ours for the call.
        unsafe {
            if sys::sceIoLseek(self.0, i64::from(at), sys::IoWhence::Set) != i64::from(at) {
                return false;
            }
            let mut done = 0usize;
            while done < buf.len() {
                let n = sys::sceIoRead(self.0, buf[done..].as_mut_ptr().cast(), (buf.len() - done) as u32);
                if n <= 0 {
                    return false;
                }
                done += n as usize;
            }
            true
        }
    }

    fn read_all(&self) -> Option<Vec<u8>> {
        let mut v = alloc::vec![0u8; self.size() as usize];
        self.read_at(0, &mut v).then_some(v)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // SAFETY: an open fd, closed once.
        unsafe { sys::sceIoClose(self.0) };
    }
}

/// The first of `dirs` holding `name`.
fn find(dirs: &[String], name: &str) -> Option<(String, File)> {
    for d in dirs {
        let mut p = d.clone();
        p.push_str(name);
        if let Some(f) = File::open(&p) {
            return Some((p, f));
        }
    }
    None
}

// ---------------------------------------------------------------- the pad

/// The scripted walk through the town: `(heading in degrees, or none to stand; ticks)`.
const WALK: &[(Option<i32>, u32)] = &[
    (None, 90),
    (Some(270), 100),
    (Some(180), 150),
    (Some(90), 200),
    (Some(0), 300),
    (Some(270), 200),
    (Some(180), 120),
];

fn walk_at(tick: u32) -> InputFrame {
    let mut t = tick;
    for &(dir, n) in WALK {
        if t < n {
            return dir.map_or(InputFrame::IDLE, |d| InputFrame::walk(Angle::from_degrees(d)));
        }
        t -= n;
    }
    InputFrame::IDLE
}

/// The pad as her input: the stick (or the d-pad, full tilt) walks her, Cross uses, R sprints.
fn pad() -> InputFrame {
    let mut d = sys::SceCtrlData { timestamp: 0, buttons: sys::CtrlButtons::empty(), lx: 128, ly: 128, rsrv: [0; 6] };
    // SAFETY: one sample into our struct.
    unsafe { sys::sceCtrlPeekBufferPositive(&mut d, 1) };
    let b = d.buttons;
    let (mut dx, mut dy) = (i32::from(d.lx) - 128, i32::from(d.ly) - 128);
    if dx * dx + dy * dy < 40 * 40 {
        (dx, dy) = (0, 0);
    }
    let dpad = |on: bool, v: i32| if on { v } else { 0 };
    let (px, py) = (
        dpad(b.contains(sys::CtrlButtons::RIGHT), 127) - dpad(b.contains(sys::CtrlButtons::LEFT), 127),
        dpad(b.contains(sys::CtrlButtons::DOWN), 127) - dpad(b.contains(sys::CtrlButtons::UP), 127),
    );
    if px != 0 || py != 0 {
        (dx, dy) = (px, py);
    }
    let mut f = InputFrame::IDLE;
    if dx != 0 || dy != 0 {
        f.mv_dir = jane_core::angle::iatan2(dy, dx);
        let m2 = dx * dx + dy * dy;
        f.mv_mag = (isqrt(m2 as u32) as i32).clamp(0, 127) as u8;
    }
    f.use_held = b.contains(sys::CtrlButtons::CROSS);
    f.sprint = b.contains(sys::CtrlButtons::RTRIGGER);
    f
}

fn isqrt(n: u32) -> u32 {
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ---------------------------------------------------------------- the painter's worker
// PORT.md §13.12: a chunk takes the terrain painter about 100 ms on a PSP, so it runs on a thread
// below the game's priority, in the time the game waits for the vblank and the GE. The game
// hands it one job at a time (`Present::take_paint_job`) and lands it when it is back.

static TODO: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static DONE: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static SEMA: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);

extern "C" fn worker(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    loop {
        // SAFETY: a semaphore made before this thread started.
        unsafe { sys::sceKernelWaitSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1, core::ptr::null_mut()) };
        let p = TODO.swap(core::ptr::null_mut(), Ordering::AcqRel);
        if !p.is_null() {
            // SAFETY: the game gave up the job when it stored it, and takes it back from DONE.
            unsafe { (*p).run() };
            DONE.store(p, Ordering::Release);
        }
    }
}

fn start_worker() {
    // SAFETY: a semaphore and a thread, made once; the thread runs `worker` for the program's life.
    unsafe {
        let sema = sys::sceKernelCreateSema(b"paint\0".as_ptr(), 0, 0, 1, core::ptr::null_mut());
        SEMA.store(sema.0, Ordering::Release);
        THREADED.store(true, Ordering::Release);
        let id = sys::sceKernelCreateThread(
            b"painter\0".as_ptr(),
            worker,
            48,
            256 * 1024,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        );
        sys::sceKernelStartThread(id, 0, core::ptr::null_mut());
    }
}

/// Lands a job the worker finished and hands it the next; true while one is out.
fn paint_jobs(present: &mut Present, sim: &Sim, out: bool) -> bool {
    let mut out = out;
    let done = DONE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !done.is_null() {
        // SAFETY: the worker stored it and no longer touches it.
        present.land(*unsafe { alloc::boxed::Box::from_raw(done) });
        out = false;
    }
    if !out {
        if let Some(v) = sim.view(Seat(0)) {
            if let Some(job) = present.take_paint_job(&v) {
                TODO.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(job)), Ordering::Release);
                // SAFETY: the semaphore the worker waits on.
                unsafe { sys::sceKernelSignalSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1) };
                out = true;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- the game

const TICK_US: u32 = 1_000_000 / jane_core::num::TICK_RATE;
const CANVAS: (u16, u16) = (480, 272);
/// Pages held in RAM at most (the rest stay on the Memory Stick until drawn).
const PAGE_RAM: u32 = 6 * 1024 * 1024;

fn run(dirs: &[String]) {
    // The PSP at its full clock, as games run it (PPSSPP starts it at 222 MHz).
    // SAFETY: plain syscall.
    unsafe { sys::scePowerSetClockFrequency(333, 333, 166) };
    let free = heap_init();
    say!("GAME heap user_free={free} small_pool={SMALL_ARENA}");
    let Some((jpt_path, jpt)) = find(dirs, "present.jpt") else {
        say!("GAME error: no present.jpt in {:?}", dirs);
        return;
    };
    let Some((jpk_path, jpk)) = find(dirs, "jane-psp.jpk") else {
        say!("GAME error: no jane-psp.jpk in {:?}", dirs);
        return;
    };
    // `script.txt`: the tick to stop at, and an hour to set the clock to first.
    let words: Vec<u32> = find(dirs, "script.txt")
        .and_then(|(_, f)| f.read_all())
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.split_whitespace().filter_map(|w| w.parse().ok()).collect())
        .unwrap_or_default();
    let script: Option<u32> = words.first().copied();
    let hour: Option<u8> = words.get(1).map(|&h| h.min(23) as u8);
    say!("GAME files {jpt_path} {jpk_path} script={script:?}");

    // The pack's tables (its pages stay in the file).
    let mut head = [0u8; jane_render_psp::pack::HEADER];
    if !jpk.read_at(0, &mut head) {
        say!("GAME error: short pack");
        return;
    }
    let pack = match Pack::head_len(&head).and_then(|n| {
        let mut b = alloc::vec![0u8; n];
        if jpk.read_at(0, &mut b) { Pack::head(&b) } else { Err(jane_render_psp::pack::PackError("short")) }
    }) {
        Ok(p) => p,
        Err(e) => {
            say!("GAME error: pack: {e:?}");
            return;
        }
    };
    say!("GAME pack pages={} recs={} refs={} live={}", pack.pages.len(), pack.recs.len(), pack.refs.len(), HEAP.live.get());

    // The presenter from its tables: no generator runs but the terrain painter's own.
    let t0 = now_us();
    let mut present = match jpt.read_all().map(|b| Present::from_tables(Tier::T0, &b)) {
        Some(Ok(p)) => p,
        Some(Err(e)) => {
            say!("GAME error: tables: {e:?}");
            return;
        }
        None => {
            say!("GAME error: cannot read present.jpt");
            return;
        }
    };
    drop(jpt);
    present.set_features(Features::c2());
    present.set_canvas(CANVAS);
    let lister = Lister::new(&present.sprites().refs, &pack);
    say!("GAME present us={} live={} peak={}", now_us().wrapping_sub(t0), HEAP.live.get(), HEAP.peak.get());

    // The world from the seed, each blueprint packed as it lands (PORT.md §13.3).
    let seed = 1u32;
    let t1 = now_us();
    let mut zones = Vec::new();
    for z in ZoneId::ALL {
        let tz = now_us();
        match jane_sim::blueprints::build_one_with(z, seed, &mut |_| {}) {
            Ok(mut bp) => {
                bp.pack();
                say!("GAME zone {} us={} live={} peak={}", z.name(), now_us().wrapping_sub(tz), HEAP.live.get(), HEAP.peak.get());
                zones.push(alloc::sync::Arc::new(bp));
            }
            Err(e) => {
                say!("GAME error: {e}");
                return;
            }
        }
    }
    let Ok(zones) = zones.try_into() else { return };
    let bps = jane_sim::Blueprints::from_parts(seed, zones);
    let mut sim = Sim::new_game_with(bps, "Jane");
    let build_peak = HEAP.peak.get();
    say!("GAME world us={} live={} build_peak={build_peak}", now_us().wrapping_sub(t1), HEAP.live.get());
    HEAP.peak.set(HEAP.live.get());

    let mut ge = Ge::new(pack, PAGE_RAM);
    let mut lister = lister;
    present.set_deferred_paint(true);
    start_worker();
    let mut job_out = false;
    // New Game wakes her at the farm on the county's west edge, a thousand cells from Castle; the
    // first tick takes her to the town square (the dev travel), where the town is.
    let to_town: Vec<StampedCommand> = sim
        .view(Seat(0))
        .and_then(|v| v.sym("town_square"))
        .map(|mark| StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Tp { zone: ZoneId::County, mark }) })
        .into_iter()
        .chain(hour.map(|hour| StampedCommand { seat: Some(Seat(0)), seq: 2, cmd: Command::Dev(DevOp::Time { hour }) }))
        .collect();
    // SAFETY: the pad's set-up, once.
    unsafe {
        sys::sceCtrlSetSamplingCycle(0);
        sys::sceCtrlSetSamplingMode(sys::CtrlMode::Analog);
    }

    let mut load = |at: u32, buf: &mut [u8]| jpk.read_at(at, buf);
    let (mut ticks, mut frames) = (0u32, 0u32);
    let mut acc = 0u32;
    let mut last = now_us();
    // Over the log's window: frames, ticks and each part's microseconds.
    let (mut w_frames, mut w_ticks, mut w_sim, mut w_tick, mut w_draw, mut w_list, mut w_ge, mut w_worst) =
        (0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    let mut w_start = now_us();
    let mut w_tick_worst = 0u32;
    loop {
        let now = now_us();
        acc = acc.saturating_add(now.wrapping_sub(last));
        last = now;
        // Every tick owed, at most four a frame; past that the clock lets go.
        let mut n = 0;
        while acc >= TICK_US && n < 4 {
            let input = match script {
                Some(_) => walk_at(ticks),
                None => pad(),
            };
            let a = now_us();
            let commands: &[StampedCommand] = if ticks == 0 { &to_town } else { &[] };
            sim.step(&StepInput { frames: [input, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands });
            let b = now_us();
            let events = sim.drain_events().to_vec();
            if let Some(v) = sim.view(Seat(0)) {
                present.tick(&v, &events);
            }
            let c = now_us();
            w_sim += b.wrapping_sub(a);
            w_tick += c.wrapping_sub(b);
            w_tick_worst = w_tick_worst.max(c.wrapping_sub(b));
            acc -= TICK_US;
            ticks += 1;
            w_ticks += 1;
            n += 1;
        }
        if acc >= 4 * TICK_US {
            acc = 0;
        }
        job_out = paint_jobs(&mut present, &sim, job_out);
        let a = now_us();
        let alpha = (acc * 256 / TICK_US).min(255) as u8;
        let frame = present.draw(alpha, CANVAS);
        let b = now_us();
        lister.build(frame);
        let c = now_us();
        ge.draw(frame, &lister, &mut load);
        let d = now_us();
        ge.show();
        frames += 1;
        w_frames += 1;
        w_draw += b.wrapping_sub(a);
        w_list += c.wrapping_sub(b);
        w_ge += d.wrapping_sub(c);
        w_worst = w_worst.max(d.wrapping_sub(a));
        let span = now_us().wrapping_sub(w_start);
        let done = script.is_some_and(|s| ticks >= s);
        if span >= 2_000_000 || done {
            let (maxf, totf) = free_mem();
            let st = ge.stats;
            let per = |us: u32, n: u32| us / n.max(1);
            say!(
                "GAME t={ticks} fps={}.{} ticks/s={} sim={}us tick={}us draw={}us list={}us ge={}us worst={}us worst_tick={}us painted={} landed={} seen={:?} quads={} batches={} misses={} pages_ram={} loads={} uploads={} chunk_tex={} live={} peak={} free={totf} maxfree={maxf}",
                w_frames * 100 / (span / 100_000).max(1) / 10,
                w_frames * 100 / (span / 100_000).max(1) % 10,
                w_ticks * 1000 / (span / 1000).max(1),
                per(w_sim, w_ticks),
                per(w_tick, w_ticks),
                per(w_draw, w_frames),
                per(w_list, w_frames),
                per(w_ge, w_frames),
                w_worst,
                w_tick_worst,
                present.chunks_painted(),
                present.chunks_landed(),
                present.seen(),
                st.quads,
                st.batches,
                lister.misses,
                st.ram_pages_bytes,
                st.page_loads,
                st.uploads,
                st.chunk_bytes,
                HEAP.live.get(),
                HEAP.peak.get(),
            );
            if let Some(v) = sim.view(Seat(0)) {
                if let Some(u) = v.unit(v.me().unit) {
                    let (x, y) = u.pos.cell();
                    say!("GAME her zone={} cell=({x}, {y})", v.zone().name());
                }
            }
            (w_frames, w_ticks, w_sim, w_tick, w_draw, w_list, w_ge, w_worst) = (0, 0, 0, 0, 0, 0, 0, 0);
            w_tick_worst = 0;
            w_start = now_us();
        }
        if done {
            // Hold the last frame a few vblanks for the screenshot.
            for _ in 0..8 {
                let frame = present.draw(0, CANVAS);
                lister.build(frame);
                ge.draw(frame, &lister, &mut load);
                ge.show();
            }
            say!("GAME done ticks={ticks} frames={frames} peak={}", HEAP.peak.get());
            return;
        }
    }
}

/// The directory the program was started from (its path in `argv[0]`, up to the last `/`), then
/// the host's root.
fn dirs_from(argv: &[u8]) -> Vec<String> {
    let first = argv.split(|&b| b == 0).next().unwrap_or(&[]);
    let mut out = Vec::new();
    if let Ok(s) = core::str::from_utf8(first) {
        say!("GAME argv0={s}");
        if let Some(i) = s.rfind('/') {
            out.push(String::from(&s[..=i]));
        }
    }
    out.push(String::from("host0:/"));
    out.push(String::from("ms0:/PSP/GAME/jane/"));
    out
}

fn psp_main(argv: &[u8]) {
    let dirs = dirs_from(argv);
    run(&dirs);
    // SAFETY: plain syscall; ends the program so the headless runner exits.
    unsafe { sys::sceKernelExitGame() };
}

// ---------------------------------------------------------------- module glue
// As §13.10's spike: `psp::module!` needs rust-psp's `catch_unwind`, which `stub-only` leaves
// out, so the module header is spelled out here.

core::arch::global_asm!(
    r#"
        .section .lib.ent.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_ent_top
    __lib_ent_top:
        .section .lib.ent.btm, "a", @progbits
        .align 2
    .global __lib_ent_bottom
    __lib_ent_bottom:
        .word 0

        .section .lib.stub.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_stub_top
    __lib_stub_top:
        .section .lib.stub.btm, "a", @progbits
        .align 2
    .global __lib_stub_bottom
    __lib_stub_bottom:
        .word 0
    "#
);

mod module {
    use core::ffi::c_void;
    use psp::sys;

    #[no_mangle]
    #[link_section = ".rodata.sceModuleInfo"]
    #[used]
    static MODULE_INFO: psp::Align16<sys::SceModuleInfo> = psp::Align16(sys::SceModuleInfo {
        mod_attribute: 0,
        mod_version: [1, 1],
        mod_name: sys::SceModuleInfo::name("jane"),
        terminal: 0,
        gp_value: unsafe { &_gp },
        stub_top: unsafe { &__lib_stub_top },
        stub_end: unsafe { &__lib_stub_bottom },
        ent_top: unsafe { &__lib_ent_top },
        ent_end: unsafe { &__lib_ent_bottom },
    });

    extern "C" {
        static _gp: u8;
        static __lib_ent_bottom: u8;
        static __lib_ent_top: u8;
        static __lib_stub_bottom: u8;
        static __lib_stub_top: u8;
    }

    #[no_mangle]
    #[link_section = ".lib.ent"]
    #[used]
    static LIB_ENT: sys::SceLibraryEntry = sys::SceLibraryEntry {
        name: core::ptr::null(),
        version: (1, 1),
        attribute: sys::SceLibAttr::SCE_LIB_IS_SYSLIB,
        entry_len: 4,
        var_count: 1,
        func_count: 1,
        entry_table: &LIB_ENT_TABLE,
    };

    #[no_mangle]
    #[link_section = ".rodata.sceResident"]
    #[used]
    static LIB_ENT_TABLE: sys::SceLibraryEntryTable = sys::SceLibraryEntryTable {
        module_start_nid: 0xd632acdb,
        module_info_nid: 0xf01d73a7,
        module_start,
        module_info: &MODULE_INFO.0,
    };

    #[no_mangle]
    extern "C" fn module_start(argc_bytes: usize, argv: *mut c_void) -> isize {
        extern "C" fn main_thread(argc: usize, argv: *mut c_void) -> i32 {
            // SAFETY: the kernel copied `argc` bytes of arguments to `argv` for this thread.
            let args = unsafe { core::slice::from_raw_parts(argv.cast::<u8>(), argc) };
            super::psp_main(args);
            0
        }
        // SAFETY: as rust-psp's `module!`; a 1 MB stack for the sim's deeper recursions.
        unsafe {
            let id = sys::sceKernelCreateThread(
                b"main_thread\0".as_ptr(),
                main_thread,
                32,
                1024 * 1024,
                sys::ThreadAttributes::USER | sys::ThreadAttributes::VFPU,
                core::ptr::null_mut(),
            );
            sys::sceKernelStartThread(id, argc_bytes, argv);
        }
        0
    }
}
