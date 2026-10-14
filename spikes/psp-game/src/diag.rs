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

/// Frames begun.
#[cfg(feature = "crashtest")]
pub fn frames() -> u32 {
    FRAME.load(Ordering::Relaxed)
}
