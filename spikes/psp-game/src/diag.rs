//! Crash diagnostics for real hardware: `crash.txt` beside `perf.txt` when the program panics
//! (and an allocation failure is a panic here: `handle_alloc_error` ends in the panic handler),
//! the run header for `perf.txt`, and the paint-failure count.
//!
//! The panic path allocates nothing: the text is written into a static buffer and out through
//! `sceIo*`. A true CPU exception handler (EPC, BadVAddr, cause) is not here: user mode reaches
//! `sceKernelRegisterDefaultExceptionHandler` only through an import stub rust-psp does not
//! have, and PPSSPP stubs it out, so it could not be tried; a bus error is a hang or a reset.
//! The report has the panic's file and line, the return address, the memory, the place in the
//! world and the last [`RING`] stage events.
//!
//! A freeze has no panic: the [`watchdog`] thread writes `freeze.txt` instead when the frames
//! stand still (where the game's thread stopped, and which thread spins or waits on what).
//! A CPU fault with no handler halts the whole PSP, the watchdog with it: a freeze that leaves no
//! `freeze.txt` points at one.

use core::cell::UnsafeCell;
use core::fmt::Write;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use psp::sys;

/// The events kept: a stage name and the frame it was on.
const RING: usize = 32;
const REPORT: usize = 3072;

struct Slots {
    path: [u8; 160],
    zone: [u8; 24],
    zone_len: usize,
    cell: (i32, i32),
    clock: (u32, u32),
    seed: u32,
    events: [(u32, &'static str); RING],
    report: [u8; REPORT],
}

struct Shared(UnsafeCell<Slots>);

// SAFETY: written by the game's thread (context, events) and read where the program dies; a
// torn read in a crash report is accepted.
unsafe impl Sync for Shared {}

static S: Shared = Shared(UnsafeCell::new(Slots {
    path: [0; 160],
    zone: [0; 24],
    zone_len: 0,
    cell: (0, 0),
    clock: (0, 0),
    seed: 0,
    events: [(0, ""); RING],
    report: [0; REPORT],
}));

static FRAME: AtomicU32 = AtomicU32::new(0);
static HEAD: AtomicU32 = AtomicU32::new(0);
/// Allocations that returned null (all, then those made for the painter: its thread, or the
/// game's while it makes a job's snapshot), and the size of the last.
static ALLOC_FAILS: AtomicU32 = AtomicU32::new(0);
static PAINT_FAILS: AtomicU32 = AtomicU32::new(0);
static LAST_FAIL: AtomicU32 = AtomicU32::new(0);
static PAINTER: AtomicI32 = AtomicI32::new(-1);
static IN_JOB: AtomicU32 = AtomicU32::new(0);

/// Where `crash.txt` goes (the save folder) and the painter's thread.
pub fn init(dir: &str, painter: i32) {
    // SAFETY: the game's thread, before play.
    let s = unsafe { &mut *S.0.get() };
    let n = dir.len().min(s.path.len() - 11);
    s.path[..n].copy_from_slice(&dir.as_bytes()[..n]);
    s.path[n..n + 10].copy_from_slice(b"crash.txt\0");
    PAINTER.store(painter, Ordering::Relaxed);
}

/// A new frame begins.
pub fn frame() {
    FRAME.fetch_add(1, Ordering::Relaxed);
}

/// A stage reached, on this frame.
pub fn note(what: &'static str) {
    let k = HEAD.fetch_add(1, Ordering::Relaxed) as usize % RING;
    // SAFETY: see `Shared`.
    unsafe { (*S.0.get()).events[k] = (FRAME.load(Ordering::Relaxed), what) };
}

/// Where she is, for the report (a second's old at worst).
pub fn context(sim: &jane_sim::Sim) {
    // SAFETY: see `Shared`.
    let s = unsafe { &mut *S.0.get() };
    s.seed = sim.state().seed;
    if let Some(v) = sim.view(jane_sim::Seat(0)) {
        let name = v.zone().name().as_bytes();
        let n = name.len().min(s.zone.len());
        s.zone[..n].copy_from_slice(&name[..n]);
        s.zone_len = n;
        s.clock = v.clock();
        if let Some(u) = v.unit(v.me().unit) {
            s.cell = u.pos.cell();
        }
    }
}

/// Whether the game's thread is making a paint job's snapshot (its allocations count as the
/// painter's).
pub fn job_scope(on: bool) {
    IN_JOB.store(u32::from(on), Ordering::Relaxed);
}

/// An allocation of `size` returned null.
pub fn alloc_failed(size: usize) {
    ALLOC_FAILS.fetch_add(1, Ordering::Relaxed);
    LAST_FAIL.store(size as u32, Ordering::Relaxed);
    // SAFETY: plain syscall.
    let me = unsafe { sys::sceKernelGetThreadId() };
    if me == PAINTER.load(Ordering::Relaxed) || IN_JOB.load(Ordering::Relaxed) != 0 {
        PAINT_FAILS.fetch_add(1, Ordering::Relaxed);
    }
}

/// Paint jobs that hit an allocation failure.
pub fn paint_fails() -> u32 {
    PAINT_FAILS.load(Ordering::Relaxed)
}

struct Buf<'a>(&'a mut [u8], usize);

impl Write for Buf<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.0.len() - self.1);
        self.0[self.1..self.1 + n].copy_from_slice(&s.as_bytes()[..n]);
        self.1 += n;
        Ok(())
    }
}

/// `crash.txt`: written from the panic handler, allocating nothing.
pub fn crash(info: &core::panic::PanicInfo<'_>, live: usize, peak: usize) {
    let ra: u32;
    // SAFETY: reads a register.
    unsafe { core::arch::asm!(".set push", ".set noat", "move {0}, $ra", ".set pop", out(reg) ra) };
    report(format_args!("panic {info}"), ra, live, peak);
}

/// `crash.txt` for a hang (the bench's watchdog, `bench.rs`): the game's thread stalled, written
/// from the watchdog's thread, allocating nothing. The last events say where it stalled.
pub fn hang(scene: &str, stalled_ms: u32, live: usize, peak: usize) {
    report(format_args!("hang scene {scene} stalled_ms {stalled_ms} (the game's thread made no frame)"), 0, live, peak);
}

/// Frames begun.
pub fn frames_now() -> u32 {
    FRAME.load(Ordering::Relaxed)
}

fn report(head: core::fmt::Arguments<'_>, ra: u32, live: usize, peak: usize) {
    // SAFETY: plain syscalls.
    let (big, free, thread) =
        unsafe { (sys::sceKernelMaxFreeMemSize(), sys::sceKernelTotalFreeMemSize(), sys::sceKernelGetThreadId()) };
    // SAFETY: see `Shared`.
    let s = unsafe { &mut *S.0.get() };
    let (path, zone, zone_len, cell, clock, seed) = (s.path, s.zone, s.zone_len, s.cell, s.clock, s.seed);
    let events = s.events;
    let mut b = Buf(&mut s.report, 0);
    let _ = writeln!(b, "commit {} stamp {:016x}", env!("JANE_COMMIT"), jane_world::SOURCE_STAMP);
    let _ = writeln!(b, "{head}");
    let _ = writeln!(b, "ra {ra:08x} (the hook's) epc n/a badvaddr n/a cause n/a (no exception handler in user mode)");
    let _ = writeln!(b, "thread {thread:x} painter {:x}", PAINTER.load(Ordering::Relaxed));
    let _ = writeln!(b, "free {free} largest {big} heap_live {live} heap_peak {peak}");
    let _ = writeln!(
        b,
        "alloc_fail {} last_size {} paint_fail {}",
        ALLOC_FAILS.load(Ordering::Relaxed),
        LAST_FAIL.load(Ordering::Relaxed),
        PAINT_FAILS.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        b,
        "zone {} cell {} {} seed {seed} clock {} day {} frame {}",
        core::str::from_utf8(&zone[..zone_len]).unwrap_or("?"),
        cell.0,
        cell.1,
        clock.0,
        clock.1,
        FRAME.load(Ordering::Relaxed)
    );
    let head = HEAD.load(Ordering::Relaxed) as usize;
    let _ = writeln!(b, "last events, oldest first");
    for k in 0..RING {
        let (f, name) = events[(head + k) % RING];
        if !name.is_empty() {
            let _ = writeln!(b, "  f{f} {name}");
        }
    }
    let len = b.1;
    if path[0] != 0 {
        // SAFETY: a NUL-terminated path in a static; the report outlives the calls.
        unsafe {
            let fd = sys::sceIoOpen(
                path.as_ptr(),
                sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
                0o777,
            );
            if fd.0 >= 0 {
                sys::sceIoWrite(fd, s.report.as_ptr().cast(), len);
                sys::sceIoClose(fd);
            }
        }
    }
}

// ---------------------------------------------------------------- the watchdog
// A freeze writes no crash.txt: a loop that never ends and a wait that is never answered raise
// nothing, and a CPU fault in user mode has no handler to run. The watchdog is a thread above the
// others that wakes once a second; when the frames have stood still for [`FREEZE_SECS`] it
// writes `freeze.txt` beside `crash.txt`: where she was, the last stage events (where the game's
// thread stopped), and every thread's state twice a second apart: how long each ran meanwhile
// (a thread that spins runs all of it; a deadlock's run none and wait), its wait and its stack
// left. Nothing written means every thread stopped (a CPU fault with no handler halts the PSP).

/// Seconds the frames may stand still before the watchdog writes (the longest wait the game
/// makes on its own thread is a zone built at her step, about 1.3 s emulated).
pub const FREEZE_SECS: u32 = 8;
/// Threads listed, at most.
const THREADS: usize = 24;

/// The freeze's report, the watchdog's alone.
struct FreezeBuf(UnsafeCell<[u8; REPORT]>);

// SAFETY: only the watchdog's thread touches it.
unsafe impl Sync for FreezeBuf {}

static FREEZE_REPORT: FreezeBuf = FreezeBuf(UnsafeCell::new([0; REPORT]));
/// The game's thread.
static MAIN: AtomicI32 = AtomicI32::new(-1);

/// The watchdog's thread: started once at boot, from the game's thread.
pub extern "C" fn watchdog(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    let mut last = FRAME.load(Ordering::Relaxed);
    let (mut still, mut written) = (0u32, false);
    loop {
        // SAFETY: plain syscall.
        unsafe { sys::sceKernelDelayThread(1_000_000) };
        let f = FRAME.load(Ordering::Relaxed);
        if f != last {
            (last, still, written) = (f, 0, false);
            continue;
        }
        still += 1;
        if still >= FREEZE_SECS && !written {
            freeze(still);
            written = true;
        }
    }
}

/// The game's thread, for the watchdog's report (called on it).
pub fn set_main() {
    // SAFETY: plain syscall.
    MAIN.store(unsafe { sys::sceKernelGetThreadId() }, Ordering::Relaxed);
}

/// One thread's state: (id, status, wait type, wait id, priority, run clocks, stack free, name).
type ThreadRow = (i32, i32, i32, i32, i32, u64, i32, [u8; 32]);

fn threads(out: &mut [ThreadRow; THREADS]) -> usize {
    let mut ids = [sys::SceUid(0); THREADS];
    let mut n = 0i32;
    // SAFETY: a buffer of THREADS ids and a count, ours.
    unsafe {
        sys::sceKernelGetThreadmanIdList(sys::SceKernelIdListType::Thread, ids.as_mut_ptr(), THREADS as i32, &mut n)
    };
    let n = (n.max(0) as usize).min(THREADS);
    for (row, &id) in out.iter_mut().zip(&ids[..n]) {
        // The info holds a function pointer, so it is never made whole here: zeroed, its size
        // set, the kernel fills it, and only its name is read (through the raw pointer).
        let mut info = core::mem::MaybeUninit::<sys::SceKernelThreadInfo>::zeroed();
        // SAFETY: zeroed plain-old-data the kernel fills, its size set.
        let mut run: sys::SceKernelThreadRunStatus = unsafe { core::mem::zeroed() };
        run.size = core::mem::size_of::<sys::SceKernelThreadRunStatus>();
        // SAFETY: our own buffers, their sizes set; the name is plain bytes, zero if unread.
        let (free, name) = unsafe {
            let p = info.as_mut_ptr();
            core::ptr::addr_of_mut!((*p).size).write(core::mem::size_of::<sys::SceKernelThreadInfo>());
            sys::sceKernelReferThreadStatus(id, p);
            sys::sceKernelReferThreadRunStatus(id, &mut run);
            (sys::sceKernelGetThreadStackFreeSize(id), core::ptr::addr_of!((*p).name).read())
        };
        let clocks = u64::from(run.run_clocks.hi) << 32 | u64::from(run.run_clocks.low);
        *row = (id.0, run.status, run.wait_type, run.wait_id, run.current_priority, clocks, free, name);
    }
    n
}

/// `freeze.txt`: the frames stood still `secs` seconds. Allocates nothing.
fn freeze(secs: u32) {
    let mut a = [(0, 0, 0, 0, 0, 0u64, 0, [0u8; 32]); THREADS];
    let mut b = a;
    let na = threads(&mut a);
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelDelayThread(1_000_000) };
    let nb = threads(&mut b);
    // SAFETY: plain syscalls.
    let (big, free) = unsafe { (sys::sceKernelMaxFreeMemSize(), sys::sceKernelTotalFreeMemSize()) };
    // SAFETY: the crash report's context is read as `crash` reads it (a torn read is accepted);
    // the freeze's buffer is this thread's alone.
    let s = unsafe { &*S.0.get() };
    let r = unsafe { &mut *FREEZE_REPORT.0.get() };
    let mut w = Buf(r, 0);
    let _ = writeln!(w, "commit {} stamp {:016x}", env!("JANE_COMMIT"), jane_world::SOURCE_STAMP);
    let _ = writeln!(w, "freeze: no frame for {secs} s (the watchdog's report; no panic)");
    let _ = writeln!(
        w,
        "main {:x} painter {:x} free {free} largest {big} alloc_fail {} last_size {} paint_fail {}",
        MAIN.load(Ordering::Relaxed),
        PAINTER.load(Ordering::Relaxed),
        ALLOC_FAILS.load(Ordering::Relaxed),
        LAST_FAIL.load(Ordering::Relaxed),
        PAINT_FAILS.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        w,
        "zone {} cell {} {} seed {} clock {} day {} frame {}",
        core::str::from_utf8(&s.zone[..s.zone_len]).unwrap_or("?"),
        s.cell.0,
        s.cell.1,
        s.seed,
        s.clock.0,
        s.clock.1,
        FRAME.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        w,
        "threads: id name status (1 run 2 ready 4 wait 8 suspended 10 dormant 20 dead) wait_type wait_id prio stack_free ran_us_in_1s"
    );
    for row in &b[..nb] {
        let (id, status, wt, wid, prio, clocks, stack, name) = *row;
        let ran = a[..na].iter().find(|r| r.0 == id).map_or(u64::MAX, |r| clocks.wrapping_sub(r.5));
        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        let _ = writeln!(
            w,
            "  {id:x} {} {status:x} {wt} {wid:x} {prio} {stack} {}",
            core::str::from_utf8(&name[..len]).unwrap_or("?"),
            ran as i64
        );
    }
    let head = HEAD.load(Ordering::Relaxed) as usize;
    let _ = writeln!(w, "last events, oldest first");
    for k in 0..RING {
        let (f, name) = s.events[(head + k) % RING];
        if !name.is_empty() {
            let _ = writeln!(w, "  f{f} {name}");
        }
    }
    let len = w.1;
    let mut path = s.path;
    if path[0] == 0 {
        return;
    }
    // `crash.txt` there becomes `freeze.txt`.
    if let Some(at) = path.windows(9).position(|p| p == b"crash.txt") {
        path[at..at + 9].copy_from_slice(b"freeze.tx");
        path[at + 9] = b't';
        path[at + 10] = 0;
    }
    // SAFETY: a NUL-terminated path; the report outlives the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            path.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, r.as_ptr().cast(), len);
            sys::sceIoClose(fd);
        }
        // The headless runner's stdout too.
        sys::sceIoWrite(sys::SceUid(1), b"GAME freeze written\n".as_ptr().cast(), 20);
        sys::sceIoWrite(sys::SceUid(1), r.as_ptr().cast(), len);
    }
}

/// Frames begun.
#[cfg(feature = "crashtest")]
pub fn frames() -> u32 {
    FRAME.load(Ordering::Relaxed)
}
