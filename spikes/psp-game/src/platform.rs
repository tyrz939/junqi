//! The platform under the game: the heap (a talc pool for small allocations, a kernel block
//! each for big ones, PORT.md §13.10, §13.12), the partition's free RAM, the clocks, sleep and
//! resume (the power and exit callbacks, PORT.md §13.13) and the CPU's full clock.

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, UnsafeCell};
use core::fmt::Write;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use alloc::format;

use crate::diag;

use crate::Line;
use psp::sys;
use talc::{ErrOnOom, Span, Talc};

// ---------------------------------------------------------------- heap
// As §13.10's spike: a talc pool for small allocations, a kernel block each for big ones, live
// and peak bytes counted as requested.

pub(crate) struct Heap {
    talc: UnsafeCell<Talc<ErrOnOom>>,
    lo: Cell<usize>,
    hi: Cell<usize>,
    pub(crate) live: Cell<usize>,
    pub(crate) peak: Cell<usize>,
    allocs: Cell<u32>,
    /// Bytes live in the small pool (talc's), and their peak: what the pool must hold.
    pub(crate) small_live: Cell<usize>,
    pub(crate) small_peak: Cell<usize>,
    /// The kernel blocks live, (address, bytes asked), the first [`BIGS`] of them: what is left
    /// lying in the partition when a world is let go (`big_map`, the log's).
    bigs: UnsafeCell<[(usize, usize); BIGS]>,
}

const BIG: usize = 64 * 1024;
const BIGS: usize = 1024;
/// The small pool: the build's peak of small allocations (4.9 MB with the sound's made at the
/// title) and a little over; play holds 3.2 to 3.9 MB of it (PORT.md §13.12). Was 6 MB.
pub(crate) const SMALL_ARENA: u32 = 5 * 1024 * 1024;

/// A small allocation of a KB or more is 16-aligned: a chunk's `T8` px and heights, which the GE
/// reads where they lie only when they are (else they are copied, or a height layer is skipped).
fn widen(layout: Layout) -> Layout {
    if layout.size() >= 1024 && layout.align() < 16 {
        Layout::from_size_align(layout.size(), 16).unwrap_or(layout)
    } else {
        layout
    }
}

// SAFETY: one thread allocates.
unsafe impl Sync for Heap {}

#[global_allocator]
pub(crate) static HEAP: Heap = Heap {
    talc: UnsafeCell::new(Talc::new(ErrOnOom)),
    lo: Cell::new(0),
    hi: Cell::new(0),
    live: Cell::new(0),
    peak: Cell::new(0),
    allocs: Cell::new(0),
    small_live: Cell::new(0),
    small_peak: Cell::new(0),
    bigs: UnsafeCell::new([(0, 0); BIGS]),
};

impl Heap {
    unsafe fn small(&self, layout: Layout) -> *mut u8 {
        (*self.talc.get()).malloc(layout).map_or(core::ptr::null_mut(), NonNull::as_ptr)
    }

    unsafe fn big(&self, layout: Layout) -> *mut u8 {
        // 64-aligned at least: a big buffer may be a GE texture (a chunk's albedo).
        let align = layout.align().max(64);
        let size = layout.size() + core::mem::size_of::<sys::SceUid>() + align;
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
        let pad = 1 + p.add(1).align_offset(align);
        *p.add(pad - 1) = pad as u8;
        let p = p.add(pad);
        if let Some(e) = (*self.bigs.get()).iter_mut().find(|e| e.0 == 0) {
            *e = (p as usize, layout.size());
        }
        p
    }
}

/// The kernel blocks the heap holds, by address: `n` blocks, `bytes`, and the first few as
/// `address:KB` (the partition's lie when a world is let go: what splits its free RAM).
fn big_map() -> String {
    let mut v: Vec<(usize, usize)> = Vec::with_capacity(BIGS);
    {
        let _hold = Hold::new();
        // SAFETY: under the hold, the allocator's own table.
        v.extend(unsafe { &*HEAP.bigs.get() }.iter().copied().filter(|e| e.0 != 0));
    }
    v.sort_unstable();
    let bytes: usize = v.iter().map(|e| e.1).sum();
    let mut s = format!("n {} bytes {bytes}", v.len());
    for (a, n) in v.iter().take(30) {
        let _ = write!(s, " {a:x}:{}", n.div_ceil(1024));
    }
    s
}

/// Set once the painter's worker thread runs: from then on an allocation holds the scheduler
/// (the pools are not thread-safe, and a spin lock would never yield to a lower priority).
pub(crate) static THREADED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Hold(Option<i32>);

impl Hold {
    pub(crate) fn new() -> Hold {
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
        let layout = widen(layout);
        let p = if layout.size() >= BIG {
            let p = self.big(layout);
            if p.is_null() {
                self.small(layout)
            } else {
                p
            }
        } else {
            let p = self.small(layout);
            if p.is_null() {
                self.big(layout)
            } else {
                p
            }
        };
        if !p.is_null() {
            let a = p as usize;
            if a >= self.lo.get() && a < self.hi.get() {
                let sl = self.small_live.get() + layout.size();
                self.small_live.set(sl);
                self.small_peak.set(self.small_peak.get().max(sl));
            }
            let live = self.live.get() + layout.size();
            self.live.set(live);
            if live > self.peak.get() {
                self.peak.set(live);
            }
            self.allocs.set(self.allocs.get().wrapping_add(1));
        } else {
            diag::alloc_failed(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _hold = Hold::new();
        let layout = widen(layout);
        let a = ptr as usize;
        if a >= self.lo.get() && a < self.hi.get() {
            self.small_live.set(self.small_live.get() - layout.size());
            (*self.talc.get()).free(NonNull::new_unchecked(ptr), layout);
        } else {
            if let Some(e) = (*self.bigs.get()).iter_mut().find(|e| e.0 == a) {
                *e = (0, 0);
            }
            let pad = *ptr.sub(1) as usize;
            let id = ptr.sub(pad + core::mem::size_of::<sys::SceUid>()).cast::<sys::SceUid>().read_unaligned();
            sys::sceKernelFreePartitionMemory(id);
        }
        self.live.set(self.live.get() - layout.size());
    }
}

pub(crate) fn heap_init() -> u32 {
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

/// The partition's free holes, largest first, as `address:KB` (each taken whole in turn, then
/// all given back): where the RAM a build needs lies.
fn holes() -> String {
    let mut got: Vec<(sys::SceUid, usize, u32)> = Vec::with_capacity(16);
    // No other thread runs (and allocates) while the partition is taken.
    let hold = Hold::new();
    for _ in 0..16 {
        // SAFETY: plain syscalls; every block taken is freed below.
        let (id, n) = unsafe {
            let n = sys::sceKernelMaxFreeMemSize() as u32;
            if n < 64 * 1024 {
                break;
            }
            let id = sys::sceKernelAllocPartitionMemory(
                sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
                b"hole\0".as_ptr(),
                sys::SceSysMemBlockTypes::Low,
                n,
                core::ptr::null_mut(),
            );
            (id, n)
        };
        if id.0 < 0 {
            break;
        }
        // SAFETY: a block just taken.
        let at = unsafe { sys::sceKernelGetBlockHeadAddr(id) } as usize;
        got.push((id, at, n));
    }
    for &(id, ..) in &got {
        // SAFETY: our block, freed once.
        unsafe { sys::sceKernelFreePartitionMemory(id) };
    }
    drop(hold);
    let mut s = String::new();
    for &(_, at, n) in &got {
        let _ = write!(s, " {at:x}:{}", n / 1024);
    }
    s
}

/// The heap and the partition once a world is let go (or before one is built): what the next
/// county's build has to work in.
pub(crate) fn let_go_log(why: &str) {
    let (largest, free) = free_mem();
    say!(
        "GAME let go {why} live={} small_live={} free={free} largest={largest} holes{}",
        HEAP.live.get(),
        HEAP.small_live.get(),
        holes()
    );
    say!("GAME let go {why} bigs {}", big_map());
}

pub(crate) fn free_mem() -> (u32, u32) {
    // SAFETY: plain syscalls.
    unsafe { (sys::sceKernelMaxFreeMemSize() as u32, sys::sceKernelTotalFreeMemSize() as u32) }
}

pub(crate) fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// The sim's wall clock (`Sim::set_wall_clock`): the microsecond clock in ns. It wraps every 71
/// minutes, which a phase's lap (a difference) does not notice.
pub(crate) fn now_ns() -> u64 {
    u64::from(now_us()) * 1000
}

// ---------------------------------------------------------------- sleep and resume
// PORT.md §13.13: the power switch to sleep and back. A callback thread (sleeping with callbacks)
// hears the power events; the game thread acts on a resume at the top of its next frame: the
// GE's base state and display set again, every VRAM page slot and lamp pool marked empty (they
// upload again from RAM), the CLUTs rebuilt as every frame does, the pack and the sound's file
// opened again, the clock set again.

/// Resumes the power callback has seen (the game compares with the count it has acted on).
pub(crate) static RESUMES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Suspends seen, and the last event's bits (the overlay's line).
static SUSPENDS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static POWER_BITS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

unsafe extern "C" fn power_cb(_count: i32, info: i32, _arg: *mut core::ffi::c_void) -> i32 {
    let f = sys::PowerInfo::from_bits_truncate(info as u32);
    POWER_BITS.store(info as u32, Ordering::Relaxed);
    if f.contains(sys::PowerInfo::SUSPENDING) {
        SUSPENDS.fetch_add(1, Ordering::Release);
    }
    if f.contains(sys::PowerInfo::RESUME_COMPLETE) {
        RESUMES.fetch_add(1, Ordering::Release);
    }
    0
}

unsafe extern "C" fn exit_cb(_a: i32, _b: i32, _arg: *mut core::ffi::c_void) -> i32 {
    // SAFETY: the HOME menu's Quit: the game ends here.
    unsafe { sys::sceKernelExitGame() };
    0
}

pub(crate) extern "C" fn power_thread(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    // SAFETY: callbacks made once on this thread, which sleeps with callbacks for the program's
    // life so the kernel can run them here.
    unsafe {
        let cb = sys::sceKernelCreateCallback(b"jane-power\0".as_ptr(), power_cb, core::ptr::null_mut());
        let r = sys::scePowerRegisterCallback(-1, cb);
        let ex = sys::sceKernelCreateCallback(b"jane-exit\0".as_ptr(), exit_cb, core::ptr::null_mut());
        let e = sys::sceKernelRegisterExitCallback(ex);
        say!("GAME power callback={:x} slot={r:x} exit={e:x}", cb.0);
        loop {
            sys::sceKernelSleepThreadCB();
        }
    }
}

/// The CPU and bus at the PSP's full clock (333 / 166 MHz, as games run it); the clocks in
/// effect after, read back.
pub(crate) fn full_clock() -> (i32, i32, i32) {
    // SAFETY: plain syscalls.
    unsafe {
        let r = sys::scePowerSetClockFrequency(333, 333, 166);
        if sys::scePowerGetCpuClockFrequencyInt() < 333 {
            sys::scePowerSetCpuClockFrequency(333);
            sys::scePowerSetBusClockFrequency(166);
        }
        (r, sys::scePowerGetCpuClockFrequencyInt(), sys::scePowerGetBusClockFrequencyInt())
    }
}
