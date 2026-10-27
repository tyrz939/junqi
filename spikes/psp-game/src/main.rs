#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

//! PORT.md §13.12, §13.13: the PSP build of the game. Boots, reads the presenter's tables
//! (`present.jpt`) and the PSP pack's tables (`jane-psp.jpk`) from beside the program (or
//! `host0:/`), and shows the title: New Game reads the baked county (`county.bp`)
//! on a thread while the loading screen tells it, Continue and Load read it again and lay the
//! slot's save over it; then play, as the PC plays (`shell`): the pad to the sim's
//! input through the bindings' pad column, the sim at its 60 ticks a second, the `Frame` from
//! `jane-present` with its UI, drawn by `jane-render-psp` on the GE at the display's vblank.
//! Prints its fps, timings and heap to fd 1 every two seconds. With a `script.txt` beside it,
//! the pad is the script's (presses, a walk, shots to BMP files) and it exits at its tick.
//! Integers only; the only unsafe is the platform glue.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, UnsafeCell};
use core::fmt::Write;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use alloc::format;

use jane_core::{Angle, ZoneId};
use jane_present::terrain::PaintJob;
use jane_present::{Features, Frame, Present, Tier};
use jane_render_psp::ge::Ge;
use jane_render_psp::{capture, Lister, Pack, UiLister};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim};
use shell::{PspPad, Saves, Scene, Shell};

mod diag;
mod perf;
mod shell;
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
    diag::crash(info, HEAP.live.get(), HEAP.peak.get());
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
    /// Bytes live in the small pool (talc's), and their peak: what the pool must hold.
    small_live: Cell<usize>,
    small_peak: Cell<usize>,
    /// The kernel blocks live, (address, bytes asked), the first [`BIGS`] of them: what is left
    /// lying in the partition when a world is let go (`big_map`, the log's).
    bigs: UnsafeCell<[(usize, usize); BIGS]>,
}

const BIG: usize = 64 * 1024;
const BIGS: usize = 1024;
/// The small pool: the build's peak of small allocations (4.9 MB with the sound's made at the
/// title) and a little over; play holds 3.2 to 3.9 MB of it (PORT.md §13.12). Was 6 MB.
const SMALL_ARENA: u32 = 5 * 1024 * 1024;

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
static HEAP: Heap = Heap {
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
fn let_go_log(why: &str) {
    let (largest, free) = free_mem();
    say!(
        "GAME let go {why} live={} small_live={} free={free} largest={largest} holes{}",
        HEAP.live.get(),
        HEAP.small_live.get(),
        holes()
    );
    say!("GAME let go {why} bigs {}", big_map());
}

fn free_mem() -> (u32, u32) {
    // SAFETY: plain syscalls.
    unsafe { (sys::sceKernelMaxFreeMemSize() as u32, sys::sceKernelTotalFreeMemSize() as u32) }
}

fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// The sim's wall clock (`Sim::set_wall_clock`): the microsecond clock in ns. It wraps every 71
/// minutes, which a phase's lap (a difference) does not notice.
fn now_ns() -> u64 {
    u64::from(now_us()) * 1000
}

// ---------------------------------------------------------------- files

/// A file, open for reading, and its path: a read that fails is tried once more on the file
/// opened again (a Memory Stick handle goes stale across a sleep: the owner's PSP-1000 lost
/// its text after a resume, PORT.md §13.13).
struct File {
    fd: Cell<sys::SceUid>,
    path: Vec<u8>,
    /// Times it was opened again.
    reopens: Cell<u32>,
}

fn open_fd(z: &[u8]) -> Option<sys::SceUid> {
    // SAFETY: a NUL-terminated path that outlives the call.
    let fd = unsafe { sys::sceIoOpen(z.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    (fd.0 >= 0).then_some(fd)
}

impl File {
    fn open(path: &str) -> Option<File> {
        let mut z = Vec::with_capacity(path.len() + 1);
        z.extend_from_slice(path.as_bytes());
        z.push(0);
        let fd = open_fd(&z)?;
        Some(File { fd: Cell::new(fd), path: z, reopens: Cell::new(0) })
    }

    fn size(&self) -> u32 {
        let fd = self.fd.get();
        // SAFETY: an open fd.
        unsafe {
            let n = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
            sys::sceIoLseek(fd, 0, sys::IoWhence::Set);
            n as u32
        }
    }

    /// The file closed and opened again (after a resume, or a read that failed).
    fn reopen(&self) -> bool {
        // SAFETY: our fd, closed once; a stale one's close fails harmlessly.
        unsafe { sys::sceIoClose(self.fd.get()) };
        self.reopens.set(self.reopens.get() + 1);
        match open_fd(&self.path) {
            Some(fd) => {
                self.fd.set(fd);
                true
            }
            None => false,
        }
    }

    /// Fills `buf` from byte `at`, the whole of it; on a failed or short read the file is
    /// opened again and the read tried once more.
    fn read_at(&self, at: u32, buf: &mut [u8]) -> bool {
        self.read_once(at, buf) || (self.reopen() && self.read_once(at, buf))
    }

    fn read_once(&self, at: u32, buf: &mut [u8]) -> bool {
        let fd = self.fd.get();
        // SAFETY: an open fd; `buf` is ours for the call.
        unsafe {
            if sys::sceIoLseek(fd, i64::from(at), sys::IoWhence::Set) != i64::from(at) {
                return false;
            }
            let mut done = 0usize;
            while done < buf.len() {
                let n = sys::sceIoRead(fd, buf[done..].as_mut_ptr().cast(), (buf.len() - done) as u32);
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
        unsafe { sys::sceIoClose(self.fd.get()) };
    }
}

// ---------------------------------------------------------------- sleep and resume
// PORT.md §13.13: the power switch to sleep and back. A callback thread (sleeping with callbacks)
// hears the power events; the game thread acts on a resume at the top of its next frame: the
// GE's base state and display set again, every VRAM page slot and lamp pool marked empty (they
// upload again from RAM), the CLUTs rebuilt as every frame does, the pack and the sound's file
// opened again, the clock set again.

/// Resumes the power callback has seen (the game compares with the count it has acted on).
static RESUMES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
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

extern "C" fn power_thread(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
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
fn full_clock() -> (i32, i32, i32) {
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

/// The directory the baked zones lie in (the pack's): set once, never freed.
static BAKED_DIR: AtomicPtr<String> = AtomicPtr::new(core::ptr::null_mut());

/// A baked zone's file, whole, from beside the pack (`jane_sim::baked`'s reader).
fn read_baked(zone: ZoneId) -> Option<Vec<u8>> {
    let p = BAKED_DIR.load(Ordering::Acquire);
    if p.is_null() {
        return None;
    }
    // SAFETY: stored once from a leaked box, never freed or written again.
    let dir = unsafe { &*p };
    File::open(&format!("{dir}{}", jane_sim::baked::file_name(zone))).and_then(|f| f.read_all())
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

// ---------------------------------------------------------------- the pad and the script

/// The scripted walk through the town: `(heading in degrees, or none to stand; ticks)`.
const WALK: &[(Option<i32>, u32)] = &[(None, 60), (Some(180), 330), (Some(270), 260), (Some(0), 400), (Some(90), 200)];

/// The walk's stick at play tick `tick`, as the PSP's stick would lean.
fn walk_at(tick: u32) -> (u8, u8) {
    let mut t = tick;
    for &(dir, n) in WALK {
        if t < n {
            return dir.map_or((128, 128), |d| {
                let a = Angle::from_degrees(d);
                let (c, s) = (jane_core::angle::cos_q15(a).0, jane_core::angle::sin_q15(a).0);
                ((128 + c * 127 / 32_768) as u8, (128 + s * 127 / 32_768) as u8)
            });
        }
        t -= n;
    }
    (128, 128)
}

/// The pad, one sample.
fn pad() -> PspPad {
    let mut d = sys::SceCtrlData { timestamp: 0, buttons: sys::CtrlButtons::empty(), lx: 128, ly: 128, rsrv: [0; 6] };
    // SAFETY: one sample into our struct.
    unsafe { sys::sceCtrlPeekBufferPositive(&mut d, 1) };
    PspPad { buttons: d.buttons.bits(), lx: d.lx, ly: d.ly }
}

/// A play session's ticks in [`At::Play`]: session `k`'s tick `n` is `k * SESSION + n`.
const SESSION: u32 = 1_000_000;

/// When a script's word acts: frames since the title came up (`t`), or ticks of play (`p` in the
/// first session, `q` in the second).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    Title(u32),
    Play(u32),
}

impl At {
    fn parse(s: &str) -> Option<At> {
        let (k, n) = s.split_at(1);
        let n = n.parse().ok()?;
        match k {
            "t" => Some(At::Title(n)),
            // Play sessions: `p` the first, `q` the second (after a load or a new game).
            "p" => Some(At::Play(SESSION + n)),
            "q" => Some(At::Play(2 * SESSION + n)),
            _ => None,
        }
    }
}

/// What a script's `ballast:` holds back from the heap, to run short of RAM on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ballast {
    /// `ballast:KB`: this many KB, in 64 KB kernel blocks.
    Kb(u32),
    /// `ballast:autoKB`: blocks until this many KB of the kernel's RAM are free (the overlay's
    /// `free`), as when the world has just been built.
    Free(u32),
}

/// `script.txt` (PORT.md §13.12, §13.13): `ticks [hour [effects]] [zone:mark] [still]`, and the
/// presses and shots: `press:<button>@t<frame>` or `@p<tick>` (a press of six frames),
/// `hold:<button>@p<from>-<to>`, `shot@p<tick>` (the screen to `shot-p<tick>.bmp` beside the
/// program), `new` (New Game at once from the title, else the title waits for presses).
/// The pacing and the squeeze (the PSP v0.4.1 plan): `tpf:N` steps N ticks a frame (1 to 4,
/// default 1; `tpf:2` is a steady 30; `vclock` has the display's vblanks step the sim instead,
/// as a player's game does), `stand:X,Y` puts her at that county cell, `ballast:KB`
/// or `ballast:autoKB` (`...@p<tick>`: at that tick of play) holds RAM back once the world is
/// built, `listcap:KB` lowers the display list's cap (`Ge::list_cap`), `slowpaint:PCT` has each
/// paint job hold the painter's thread PCT% of its own time (the real PSP's slower painter on
/// an emulator: about 330), `perf` turns the overlay
/// and `perf.txt` on. `retitle@p<tick>` quits to the title then and starts New Game again: the
/// memory test of a county built in the RAM a played world left (`GAME let go` lines log the
/// partition's free holes and the heap's kernel blocks at each world let go).
#[derive(Debug, Default)]
struct Script {
    /// Play ticks to stop at.
    end: u32,
    hour: Option<u8>,
    effects: Option<u8>,
    place: Option<(ZoneId, String)>,
    still: bool,
    walk: bool,
    new: bool,
    /// `seed:N`: New Game's seed (else the clock's).
    seed: Option<u32>,
    /// `clock:HH:MM`: the clock set to that minute at the scripted travel, in place of the
    /// hour's (a frame a second short of nine: `clock:20:59`, the night's turn, NIGHT.md §2.2).
    clock: Option<u32>,
    /// `flag:NAME`: a world flag (or the spine's consequence of the name) set at the travel, as
    /// the sim's tests set one (`flag:mine_quiet`: the night at N2).
    flags: Vec<String>,
    /// `tp:<zone>:<mark>@p<tick>`: a dev travel at that tick (PORT.md §13.3's on-demand runs);
    /// `tp:<zone>:<mark>:X,Y@p<tick>` sets her down at that cell of the zone instead of the mark.
    tps: Vec<(ZoneId, String, At)>,
    /// `rest@p<tick>`: the slot written then, as a rest writes it (a scripted save anywhere).
    rests: Vec<At>,
    /// `retitle@p<tick>`: back to the title then (as the pause menu's Quit to the title), and New
    /// Game again from it: a county built in the RAM a played world left (the `q` session).
    retitles: Vec<At>,
    /// `learn:<spell>@p<tick>`: the spell learned then (a dev learn, as the sim's tests teach one).
    learns: Vec<(String, At)>,
    /// `noahead`: no zone built ahead of her (`ahead`), to measure without it.
    noahead: bool,
    /// `novfpu`: the mixer's group pan and send in integers. On by default (2.5-2.8x on
    /// hardware, maxdiff 0); `vfpu` is still accepted and turns it on. `vfpubench`: its
    /// micro-bench printed at boot (`micro vfpu`).
    novfpu: bool,
    vfpubench: bool,
    /// `nopaint`: the painter's thread handed no job (the ground new to the view stays the
    /// stand-in's; what a starved painter shows, to judge the fallback).
    nopaint: bool,
    /// `nobake`: the baked county (`county.jck`) not opened: the painter paints it, as before P1c.
    nobake: bool,
    /// `nopipe`: each frame waits for its own list (no CPU and GE overlap), as before.
    nopipe: bool,
    /// `fps:60|30|free`: the frame rate's pacing for this run.
    pacing: Option<jane_render_psp::ge::Pacing>,
    /// `gfx:full|balanced|fast` (a preset), `gfx:palette|readback` (the grade's way),
    /// `gfx-<effect>`, `gfx+<effect>` (`gfx_psp::Effect`
    /// keys): the Graphics page for this run.
    gfx_words: Vec<String>,
    /// `sleep@p<n>` and `resume@p<n>`: what a sleep does that an emulator does not (VRAM's
    /// slots and pools overwritten, the pages in RAM let go, the pack's handle closed under it),
    /// then for `resume` the power callback's resume as the hardware sends it.
    sleeps: Vec<(At, bool)>,
    /// `perf`: the performance overlay on from the start (as L + R + SELECT turns it on).
    perf: bool,
    /// `stale@p<n>`: the pack's handle closed under it and every page let go (the reads' retry).
    stales: Vec<At>,
    /// `crashtest:N` (the `crashtest` feature only): panic at frame N.
    #[cfg(feature = "crashtest")]
    crash_at: Option<u32>,
    /// `hangtest:N` (the `crashtest` feature only): spin for ever at frame N (the watchdog's test).
    #[cfg(feature = "crashtest")]
    hang_at: Option<u32>,
    /// `frames:N`: stop after N frames of play (a held world's ticks stand still: the map open).
    frames: Option<u32>,
    /// `framed`: the `p` clock counts frames of play, not ticks (presses in a held world).
    framed: bool,
    presses: Vec<(u32, At, At)>,
    /// `stick:<degrees>@p<a>-p<b>`: the stick leaned that way (0 east, 90 south) meanwhile.
    sticks: Vec<(i32, At, At)>,
    shots: Vec<At>,
    /// `walk:stick|dpad|both`, `dead:low|medium|high`: the pad settings for this run (not
    /// written to the stick unless the Controls page turns one).
    walk_with: Option<jane_present::pad_psp::WalkWith>,
    dead: Option<jane_present::pad_psp::DeadZone>,
    /// `ground:terraces|viaduct` (MAP.md §9 R3): New Game on a dev ground in the county's place
    /// (`jane_sim::dev_ground`); `stand:X,Y`: she starts at that cell of it, or of the county.
    ground: Option<jane_sim::dev_ground::Ground>,
    stand: Option<(i32, i32)>,
    /// `tpf:N`: the sim's ticks a frame (1 to 4, default 1). A script's pad is read once a frame,
    /// so the ticks cannot follow the display's vblanks (a run's state would hang on the
    /// emulator's timing); this is the fixed count that does, `tpf:2` the steady 30's.
    tpf: Option<u32>,
    /// `vclock`: the sim steps on the display's clock as a player's game does (a tick a vblank
    /// the last flip took), not by `tpf`: to see that clock on an emulator. The run's state then
    /// hangs on the emulator's timing (a script's hash is not repeatable).
    vclock: bool,
    /// `ballast:KB` or `ballast:autoKB`: RAM held back once the world is built (the crash's
    /// short heap, reproduced); with `@p<tick>` at that tick of play instead, when the page
    /// cache and the chunks are warm and `auto` means the free RAM of play, not of the build.
    ballast: Option<(Ballast, Option<At>)>,
    /// `listcap:KB`: the display list's cap, lowered to see the list guard act.
    list_cap_kb: Option<u32>,
    /// `order:N`: the passes whose draws are grouped by texture (`Lister::order_passes`, bit a
    /// pass id; 0 none): to weigh what the grouping costs against the binds it saves.
    order: Option<u32>,
}

impl Script {
    /// The Graphics page with this script's `gfx` words over `g`.
    fn gfx(&self, g: jane_present::gfx_psp::Graphics) -> jane_present::gfx_psp::Graphics {
        use jane_present::gfx_psp::{Effect, Preset};
        let mut g = g;
        for w in &self.gfx_words {
            if let Some(p) = w.strip_prefix("gfx:").and_then(Preset::from_key) {
                g.take_preset(p);
            } else if w == "gfx:palette" || w == "gfx:readback" {
                g.set(Effect::Grade, true);
                g.grade_full = w == "gfx:readback";
            } else if let Some((on, k)) =
                w.strip_prefix("gfx+").map(|k| (true, k)).or(w.strip_prefix("gfx-").map(|k| (false, k)))
            {
                if let Some(e) = Effect::ALL.into_iter().find(|e| e.key() == k) {
                    g.set(e, on);
                }
            }
        }
        g
    }

    fn parse(text: &str) -> Script {
        let mut s = Script::default();
        let mut nums = Vec::new();
        let mut words = 0;
        for w in text.split_whitespace() {
            words += 1;
            if let Ok(n) = w.parse::<u32>() {
                nums.push(n);
            } else if w == "still" {
                s.still = true;
            } else if w == "noahead" {
                s.noahead = true;
            } else if w == "novfpu" {
                s.novfpu = true;
            } else if w == "vfpu" {
                s.novfpu = false;
            } else if w == "vfpubench" {
                s.vfpubench = true;
            } else if w == "nopaint" {
                s.nopaint = true;
            } else if w == "nobake" {
                s.nobake = true;
            } else if w == "nopipe" {
                s.nopipe = true;
            } else if w.starts_with("gfx") {
                s.gfx_words.push(String::from(w));
            } else if let Some(v) = w.strip_prefix("fps:") {
                use jane_render_psp::ge::Pacing;
                s.pacing = match v {
                    "30" => Some(Pacing::Locked30),
                    "free" => Some(Pacing::Unlocked),
                    _ => Some(Pacing::Vsync),
                };
            } else if w == "perf" {
                s.perf = true;
            } else if w == "framed" {
                s.framed = true;
            } else if w == "new" {
                s.new = true;
            } else if let Some(rest) = w.strip_prefix("press:").or_else(|| w.strip_prefix("hold:")) {
                let hold = w.starts_with("hold:");
                let Some((b, at)) = rest.split_once('@') else { continue };
                let Some(bit) = shell::psp::NAMES.iter().find(|e| e.0 == b).map(|e| e.1) else { continue };
                if hold {
                    let Some((a, z)) = at.split_once('-') else { continue };
                    let (Some(a), Some(z)) = (At::parse(a), At::parse(z)) else { continue };
                    s.presses.push((bit, a, z));
                } else if let Some(a) = At::parse(at) {
                    let z = match a {
                        At::Title(n) => At::Title(n + 6),
                        At::Play(n) => At::Play(n + 6),
                    };
                    s.presses.push((bit, a, z));
                }
            } else if let Some(rest) = w.strip_prefix("stick:") {
                let Some((d, at)) = rest.split_once('@') else { continue };
                let Some((a, z)) = at.split_once('-') else { continue };
                if let (Ok(d), Some(a), Some(z)) = (d.parse(), At::parse(a), At::parse(z)) {
                    s.sticks.push((d, a, z));
                }
            } else if let Some((resume, at)) =
                w.strip_prefix("sleep@").map(|a| (false, a)).or_else(|| w.strip_prefix("resume@").map(|a| (true, a)))
            {
                if let Some(a) = At::parse(at) {
                    s.sleeps.push((a, resume));
                }
            } else if let Some(at) = w.strip_prefix("stale@") {
                if let Some(a) = At::parse(at) {
                    s.stales.push(a);
                }
            } else if let Some(at) = w.strip_prefix("rest@") {
                if let Some(a) = At::parse(at) {
                    s.rests.push(a);
                }
            } else if let Some(at) = w.strip_prefix("retitle@") {
                if let Some(a) = At::parse(at) {
                    s.retitles.push(a);
                }
            } else if let Some(rest) = w.strip_prefix("learn:") {
                if let Some((sp, a)) = rest.split_once('@').and_then(|(sp, at)| Some((sp, At::parse(at)?))) {
                    s.learns.push((String::from(sp), a));
                }
            } else if let Some(rest) = w.strip_prefix("tp:") {
                let Some((zm, at)) = rest.split_once('@') else { continue };
                let Some((z, m)) = zm.split_once(':') else { continue };
                if let (Some(zz), Some(a)) = (ZoneId::ALL.into_iter().find(|zz| zz.name() == z), At::parse(at)) {
                    s.tps.push((zz, String::from(m), a));
                }
            } else if let Some(n) = w.strip_prefix("crashtest:").filter(|_| cfg!(feature = "crashtest")) {
                #[cfg(feature = "crashtest")]
                {
                    s.crash_at = n.parse().ok();
                }
                #[cfg(not(feature = "crashtest"))]
                let _ = n;
            } else if let Some(n) = w.strip_prefix("hangtest:").filter(|_| cfg!(feature = "crashtest")) {
                #[cfg(feature = "crashtest")]
                {
                    s.hang_at = n.parse().ok();
                }
                #[cfg(not(feature = "crashtest"))]
                let _ = n;
            } else if let Some(n) = w.strip_prefix("frames:") {
                s.frames = n.parse().ok();
            } else if let Some(v) = w.strip_prefix("walk:") {
                s.walk_with = jane_present::pad_psp::WalkWith::from_key(v);
            } else if let Some(v) = w.strip_prefix("dead:") {
                s.dead = jane_present::pad_psp::DeadZone::from_key(v);
            } else if let Some(n) = w.strip_prefix("seed:") {
                s.seed = n.parse().ok();
            } else if let Some(c) = w.strip_prefix("clock:") {
                let mut hm = c.split(':').map(|v| v.parse::<u32>().ok());
                if let (Some(Some(h)), Some(Some(m))) = (hm.next(), hm.next()) {
                    let hour = jane_sim::tuning::TICKS_PER_HOUR;
                    s.clock = Some((h % 24) * hour + m.min(59) * hour / 60);
                }
            } else if let Some(f) = w.strip_prefix("flag:") {
                s.flags.push(String::from(f));
            } else if let Some(g) = w.strip_prefix("ground:") {
                s.ground = jane_sim::dev_ground::Ground::parse(g);
            } else if let Some(n) = w.strip_prefix("tpf:") {
                s.tpf = n.parse::<u32>().ok().map(|n| n.clamp(1, 4));
            } else if w == "vclock" {
                s.vclock = true;
            } else if let Some(v) = w.strip_prefix("ballast:") {
                let (v, at) = v.split_once('@').map_or((v, None), |(v, at)| (v, At::parse(at)));
                let b = match v.strip_prefix("auto") {
                    Some(kb) => kb.parse().ok().map(Ballast::Free),
                    None => v.parse().ok().map(Ballast::Kb),
                };
                s.ballast = b.map(|b| (b, at));
            } else if let Some(pc) = w.strip_prefix("slowpaint:").and_then(|v| v.parse::<u32>().ok()) {
                SLOW_PAINT.store(pc.max(100), Ordering::Relaxed);
            } else if let Some(kb) = w.strip_prefix("listcap:") {
                s.list_cap_kb = kb.parse().ok();
            } else if let Some(n) = w.strip_prefix("order:") {
                s.order = n.parse().ok();
            } else if let Some((x, y)) = w.strip_prefix("stand:").and_then(|v| v.split_once(',')) {
                if let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                    s.stand = Some((x, y));
                }
            } else if let Some(at) = w.strip_prefix("shot@") {
                if let Some(a) = At::parse(at) {
                    s.shots.push(a);
                }
            } else if let Some((z, m)) = w.split_once(':') {
                if let Some(zz) = ZoneId::ALL.into_iter().find(|zz| zz.name() == z) {
                    s.place = Some((zz, String::from(m)));
                }
            }
        }
        let _ = words;
        s.end = nums.first().copied().unwrap_or(600);
        s.hour = nums.get(1).map(|&h| h.min(23) as u8);
        s.effects = nums.get(2).map(|&e| e as u8);
        // The old scripts walk the town from the square; one that presses its way stands.
        s.walk = !s.still && s.presses.is_empty() && s.sticks.is_empty();
        s.new = s.new || !s.presses.iter().any(|p| matches!(p.1, At::Title(_)));
        s
    }

    /// The buttons the script holds at `now` (and in which phase).
    fn held(&self, now: At) -> u32 {
        self.presses
            .iter()
            .filter(|(_, a, z)| match (now, *a, *z) {
                (At::Title(t), At::Title(a), At::Title(z)) | (At::Play(t), At::Play(a), At::Play(z)) => t >= a && t < z,
                _ => false,
            })
            .fold(0, |m, p| m | p.0)
    }
}

// ---------------------------------------------------------------- the painter's worker
// PORT.md §13.12: a chunk takes the terrain painter about 100 ms on a PSP, so it runs on a thread
// below the game's priority, in the time the game waits for the vblank and the GE. The game
// hands it one job at a time (`Present::take_paint_job`) and lands it when it is back.

static TODO: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static DONE: AtomicPtr<PaintJob> = AtomicPtr::new(core::ptr::null_mut());
static SEMA: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);
/// The painter's thread (its stack's use, in the log).
static PAINTER: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);
/// The painter's jobs run and their wall time, microseconds (its own time and the game's
/// between).
static JOBS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static JOB_US: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// `slowpaint:PCT`: a job holds the painter's thread this share of its own time, percent.
static SLOW_PAINT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(100);

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
fn start_thread(name: &[u8], entry: extern "C" fn(usize, *mut core::ffi::c_void) -> i32, prio: i32, stack: i32) -> i32 {
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

fn start_workers() {
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
struct Boost {
    /// Frames running with swatches showing, a job out and none landed.
    starved: u32,
    /// The painter is raised now.
    on: bool,
    /// Times raised, in the run.
    total: u32,
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
    fn frame(&mut self, waiting: usize, out: bool) -> bool {
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
    fn zone(&mut self, zone: jane_core::ZoneId) {
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
fn paint_jobs(present: &mut Present, sim: &Sim, out: bool, boost: &mut Boost) -> bool {
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

mod county_cache;

// Zones built ahead of her at a door (PORT.md §13.3, phase 4).
mod ahead;

// The built-in benchmark: the title's Benchmark row or a script's `bench` (its hooks below are
// marked "the bench").
mod bench;
mod micro;

// The baked county's chunks read and decoded off the game's thread (PLAN-PSP-SPEED.md P1c).
mod loader;

/// The builder's stack: worldgen's deepest recursion fits in it, with room.
const BUILD_STACK: i32 = 384 * 1024;

/// What to build: a county from a seed (re-rolled while it does not prove, for a seed nobody
/// chose), and a save to load over it.
struct BuildJob {
    seed: u32,
    reroll: bool,
    save: Option<Vec<u8>>,
    /// A script's dev ground in the county's place.
    ground: Option<jane_sim::dev_ground::Ground>,
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
static STAGES: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

fn report(stage: &'static str) {
    let n = STAGES.load(Ordering::Acquire);
    if n < STAGE_MAX {
        STAGE_PTR[n].store(stage.as_ptr().cast_mut(), Ordering::Relaxed);
        STAGE_LEN[n].store(stage.len(), Ordering::Relaxed);
        STAGES.store(n + 1, Ordering::Release);
    }
}

/// Stage `i` as reported.
fn stage(i: usize) -> &'static str {
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
fn start_build(job: BuildJob) {
    STAGES.store(0, Ordering::Release);
    THREADED.store(true, Ordering::Release);
    BUILD_TODO.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(job)), Ordering::Release);
    let _ = start_thread(b"builder\0", builder, 40, BUILD_STACK);
}

fn take_built() -> Option<Built> {
    let p = BUILD_DONE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    // SAFETY: the builder stored it and no longer touches it.
    (!p.is_null()).then(|| *unsafe { alloc::boxed::Box::from_raw(p) })
}

/// A seed nobody chose: the clock's.
fn clock_seed() -> u32 {
    let mut t = 0u64;
    // SAFETY: one read into our u64.
    unsafe { sys::sceRtcGetCurrentTick(&mut t) };
    (t ^ (t >> 32)) as u32 | 1
}

// ---------------------------------------------------------------- saves and shots

/// Saves on the Memory Stick (PORT.md §13.13): `ms0:/PSP/SAVEDATA/JANE00001/slotN.jane`, the
/// sim's bytes as the PC writes them, and `slotN.txt`, the note. Written to a temporary file, then
/// renamed over the slot, so a power cut mid-write leaves the old save.
struct Stick {
    dir: String,
}

/// The player's settings beside the saves (`jane_present::pad_psp::Settings`).
const SETTINGS: &str = "settings.txt";

fn cpath(s: &str) -> Vec<u8> {
    let mut z = Vec::with_capacity(s.len() + 1);
    z.extend_from_slice(s.as_bytes());
    z.push(0);
    z
}

/// `bytes` added to the end of `path` (made if missing); its length after, or none.
fn append_file(path: &str, bytes: &[u8]) -> Option<u32> {
    let z = cpath(path);
    // SAFETY: a NUL-terminated path; `bytes` outlives the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            z.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 < 0 {
            return None;
        }
        let n = sys::sceIoWrite(fd, bytes.as_ptr().cast(), bytes.len());
        let len = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
        sys::sceIoClose(fd);
        (n >= 0).then_some(len as u32)
    }
}

fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    let z = cpath(path);
    // SAFETY: a NUL-terminated path; `bytes` outlives the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            z.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 < 0 {
            return Err(format!("cannot write {path} ({:x})", fd.0));
        }
        let n = sys::sceIoWrite(fd, bytes.as_ptr().cast(), bytes.len());
        sys::sceIoClose(fd);
        if n < 0 || n as usize != bytes.len() {
            return Err(format!("{path}: short write"));
        }
    }
    Ok(())
}

impl Stick {
    fn new() -> Stick {
        for d in ["ms0:/PSP", "ms0:/PSP/SAVEDATA", "ms0:/PSP/SAVEDATA/JANE00001"] {
            let z = cpath(d);
            // SAFETY: a NUL-terminated path; an existing directory is an error we ignore.
            unsafe { sys::sceIoMkdir(z.as_ptr(), 0o777) };
        }
        Stick { dir: String::from("ms0:/PSP/SAVEDATA/JANE00001/") }
    }

    fn put(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (path, tmp) = (format!("{}{name}", self.dir), format!("{}{name}.tmp", self.dir));
        write_file(&tmp, bytes)?;
        let (zt, zp) = (cpath(&tmp), cpath(&path));
        // SAFETY: NUL-terminated paths.
        unsafe {
            sys::sceIoRemove(zp.as_ptr());
            if sys::sceIoRename(zt.as_ptr(), zp.as_ptr()) < 0 {
                return Err(format!("cannot rename {tmp}"));
            }
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Vec<u8>> {
        File::open(&format!("{}{name}", self.dir)).and_then(|f| f.read_all())
    }
}

impl Saves for Stick {
    fn read(&mut self, n: u8) -> Option<Vec<u8>> {
        self.get(&format!("slot{}.jane", n + 1))
    }
    fn write(&mut self, n: u8, bytes: &[u8]) -> Result<(), String> {
        let r = self.put(&format!("slot{}.jane", n + 1), bytes);
        say!("GAME save slot={} bytes={} {:?}", n + 1, bytes.len(), r);
        r
    }
    fn read_note(&mut self, n: u8) -> Option<String> {
        self.get(&format!("slot{}.txt", n + 1)).and_then(|b| String::from_utf8(b).ok())
    }
    fn write_note(&mut self, n: u8, note: &str) -> Result<(), String> {
        self.put(&format!("slot{}.txt", n + 1), note.as_bytes())
    }
}

/// The screen as a BMP at `path` (a script's shot).
fn shot(ge: &Ge, path: &str) {
    let px = ge.shown();
    let (w, h) = (480usize, 272usize);
    let mut b = Vec::with_capacity(54 + w * h * 3);
    let size = (54 + w * h * 3) as u32;
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
    b.extend_from_slice(&(w as u32).to_le_bytes());
    b.extend_from_slice(&(h as u32).to_le_bytes());
    b.extend_from_slice(&[1, 0, 24, 0, 0, 0, 0, 0]);
    b.extend_from_slice(&((w * h * 3) as u32).to_le_bytes());
    b.extend_from_slice(&[0; 16]);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = px[y * 512 + x];
            b.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
        }
    }
    say!("GAME shot {path} {:?}", write_file(path, &b));
}

// ---------------------------------------------------------------- the game

/// The game thread's stack: the sim's and the presenter's deepest calls fit in it, with room.
/// (PORT.md §13.12: 35 KB of it used at most in play and with the map open, `sceKernelGetThreadStackFreeSize`.)
const MAIN_STACK: i32 = 128 * 1024;
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
struct World {
    sim: alloc::boxed::Box<Sim>,
    present: Present,
}

/// Timings and counts over the log's two-second window.
#[derive(Default)]
struct Window {
    frames: u32,
    ticks: u32,
    step: u32,
    step_worst: u32,
    bufs: u32,
    bufs_worst: u32,
    sim: u32,
    sim_worst: u32,
    tick: u32,
    tick_worst: u32,
    draw: u32,
    list: u32,
    ui: u32,
    ge: u32,
    worst: u32,
    wait: u32,
    start: u32,
    /// The pad and the shell's sample; the shell's after-steps and the painter's hand-off; the
    /// GE's fill waited for (inside `ge`); the vblank waited for; the audio thread's mixing.
    pre: u32,
    post: u32,
    sync: u32,
    show: u32,
    mix0: u32,
    /// The window's largest display list and page working set, bytes.
    list_most: u32,
    pages_most: u32,
    post_worst: u32,
    /// In play: flips shown, the vblanks they took, those past the pacing's target (missed), and
    /// the longest flip (`Ge::present`).
    flips: u32,
    vblanks: u32,
    missed: u32,
    flip_max: u32,
    /// Chunks painted as swatches (painted less landed), the most chunks waiting on screen at
    /// once, the extra vblanks the painter was given, its boosts, and lists the guard cut.
    rough: u32,
    waiting_max: u32,
    /// Frames in a row with swatches showing, now (carried into the next window) and at most.
    wait_run: u32,
    wait_run_max: u32,
    /// Frames with a swatch on screen that is mostly flat colour (`Present::chunks_mush`), and
    /// the most in a row as above.
    mush: u32,
    mush_run: u32,
    mush_run_max: u32,
    extra: u32,
    boosts: u32,
    list_full: u32,
    /// Quads, polygons and terrain patches the lister left out for want of room, summed over the
    /// window's frames (`Lister::left_out`).
    quads_dropped: u32,
    polys_skipped: u32,
    patches_skipped: u32,
    /// The play tick at the window's end.
    at_tick: u32,
    /// The sim's phases summed, microseconds (`jane_sim::metrics::Phase`).
    sim_phases: [u32; jane_sim::metrics::PHASES],
}

fn run(dirs: &[String]) {
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

/// The dashcam's frames (ten seconds at 60, more at less; 90 words x 4 = 360 B a frame,
/// 600 frames = 216000 B (211 KiB)), and how long after L + R + START it is saved.
const DASH_FRAMES: usize = 600;
const CAPTURE_AFTER_US: u32 = 5_000_000;

/// The capture's header: the build, the settings, where she is and the clocks.
pub(crate) fn capture_header(shell: &Shell, sim: Option<&Sim>, script: Option<&str>) -> String {
    use core::fmt::Write as _;
    let mut h = String::with_capacity(512);
    let _ = writeln!(h, "commit {}", env!("JANE_COMMIT"));
    let _ = writeln!(h, "source_stamp {:016x}", jane_world::SOURCE_STAMP);
    // SAFETY: plain syscalls.
    let (cpu, bus) = unsafe { (sys::scePowerGetCpuClockFrequencyInt(), sys::scePowerGetBusClockFrequencyInt()) };
    let _ = writeln!(h, "clock cpu {cpu} bus {bus}");
    let _ = writeln!(h, "time_us {}", now_us());
    for l in shell.settings().write().lines() {
        let _ = writeln!(h, "setting {l}");
    }
    if let Some(sim) = sim {
        let _ = writeln!(h, "seed {}", sim.state().seed);
        let _ = writeln!(h, "state_hash {:016x}", sim.hash());
        if let Some(v) = sim.view(Seat(0)) {
            let (clock, day) = v.clock();
            let _ = writeln!(h, "zone {}", v.zone().name());
            if let Some(u) = v.unit(v.me().unit) {
                let (x, y) = u.pos.cell();
                let _ = writeln!(h, "cell {x} {y}");
            }
            let mut when = String::new();
            jane_present::text::clock(clock, &mut when);
            let _ = writeln!(h, "game_time day {} {when}", day + 1);
            let _ = writeln!(h, "weather {:?}", v.weather().kind);
        }
    }
    if let Some(t) = script {
        let _ = writeln!(h, "script {}", t.lines().next().unwrap_or(""));
    }
    h
}

/// The dashcam's frames to `capture-<n>.bin` (the first number not taken), and the banner says
/// so.
fn save_capture(rec: &capture::Recorder, header: &str, stick: &Stick, shell: &mut Shell) {
    let bytes = rec.encode(header);
    let n = (0..1000u32).find(|n| !file_exists(&format!("{}capture-{n}.bin", stick.dir))).unwrap_or(999);
    let path = format!("{}capture-{n}.bin", stick.dir);
    let r = write_file(&path, &bytes);
    say!("GAME capture saved {path} frames={} bytes={} {r:?}", rec.frames(), bytes.len());
    shell.banner = if r.is_ok() { format!("Capture saved {n}") } else { String::from("Capture not saved") };
}

/// Whether a file is there.
fn file_exists(path: &str) -> bool {
    open_fd(&cpath(path)).is_some_and(|fd| {
        // SAFETY: our handle, closed once.
        unsafe { sys::sceIoClose(fd) };
        true
    })
}

fn at_reached(at: At, now: At) -> bool {
    match (at, now) {
        (At::Title(a), At::Title(n)) | (At::Play(a), At::Play(n)) => n >= a,
        _ => false,
    }
}

/// Whether a script's moment `at` fell in the frame that took the clock from `prev` to `now`: at
/// `now`, or between the two (a frame steps `tpf` ticks, so `now` can skip a tick).
fn crossed(at: At, prev: At, now: At) -> bool {
    match (at, prev, now) {
        (At::Play(a), At::Play(p), At::Play(n)) => a == n || (p < a && a < n),
        _ => at == now,
    }
}

/// The ballast's block: kernel RAM held for the run, never freed.
const BALLAST_BLOCK: u32 = 64 * 1024;

/// A script's `ballast:` (the v0.4.1 plan, E): kernel blocks of 64 KB taken and held, so the run
/// is short of RAM as the owner's was (2.0 MB free, the largest block 1.4 MB). `Free(kb)` takes
/// blocks until `kb` KB of the kernel's RAM are free, and a last smaller one for the remainder.
/// Taken from the top, where the heap's own big blocks come from.
fn hold_ballast(b: Ballast) {
    let take = |bytes: u32| -> bool {
        // SAFETY: plain syscalls; the block is never freed.
        let id = unsafe {
            sys::sceKernelAllocPartitionMemory(
                sys::SceSysMemPartitionId::SceKernelPrimaryUserPartition,
                b"ballast\0".as_ptr(),
                sys::SceSysMemBlockTypes::High,
                bytes,
                core::ptr::null_mut(),
            )
        };
        id.0 >= 0
    };
    let before = free_mem();
    let (mut blocks, mut held) = (0u32, 0u32);
    match b {
        Ballast::Kb(kb) => {
            let mut left = kb * 1024;
            while left > 0 {
                let n = left.min(BALLAST_BLOCK);
                if !take(n) {
                    break;
                }
                (blocks, held, left) = (blocks + 1, held + n, left - n);
            }
        }
        Ballast::Free(kb) => {
            let keep = kb * 1024;
            while free_mem().1 > keep + BALLAST_BLOCK {
                if !take(BALLAST_BLOCK) {
                    break;
                }
                (blocks, held) = (blocks + 1, held + BALLAST_BLOCK);
            }
            // The remainder in one block, rounded down to the kernel's 256-byte grain with room
            // for its header.
            let rest = free_mem().1.saturating_sub(keep).saturating_sub(512) & !255;
            if rest >= 4096 && take(rest) {
                (blocks, held) = (blocks + 1, held + rest);
            }
        }
    }
    let after = free_mem();
    say!(
        "GAME ballast {:?} blocks={blocks} held={held} free {}K->{}K largest {}K->{}K",
        b,
        before.1 / 1024,
        after.1 / 1024,
        before.0 / 1024,
        after.0 / 1024
    );
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

fn log_window(
    w: &Window,
    span: u32,
    ge: &Ge,
    lister: &Lister,
    ui: &UiLister,
    world: Option<&World>,
    scene: Scene,
    amb: [u32; 4],
) {
    let (maxf, totf) = free_mem();
    let st = ge.stats;
    let per = |us: u32, n: u32| us / n.max(1);
    say!(
        "GAME scene={scene:?} fps={}.{} ticks/s={} step={}us step_worst={}us sim={}us sim_worst={}us tick={}us tick_worst={}us bufs={}us bufs_worst={}us draw={}us ui={}us list={}us ge={}us worst={}us quads={} ui_quads={} ui_misses={} batches={} misses={} pages_ram={} loads={} uploads={} images={} live={} peak={} free={totf} maxfree={maxf}",
        w.frames * 100 / (span / 100_000).max(1) / 10,
        w.frames * 100 / (span / 100_000).max(1) % 10,
        w.ticks * 1000 / (span / 1000).max(1),
        per(w.step, w.ticks),
        w.step_worst,
        per(w.sim, w.ticks),
        w.sim_worst,
        per(w.tick, w.ticks),
        w.tick_worst,
        per(w.bufs, w.ticks),
        w.bufs_worst,
        per(w.draw, w.frames),
        per(w.ui, w.frames),
        per(w.list, w.frames),
        per(w.ge, w.frames),
        w.worst,
        st.quads,
        ui.quads.len(),
        ui.misses,
        st.batches,
        lister.misses,
        st.ram_pages_bytes,
        st.page_loads,
        st.uploads,
        ge.image_bytes(),
        HEAP.live.get(),
        HEAP.peak.get(),
    );
    // SAFETY: plain syscalls.
    let (stack_main, stack_painter) = unsafe {
        (
            sys::sceKernelGetThreadStackFreeSize(sys::SceUid(0)),
            sys::sceKernelGetThreadStackFreeSize(sys::SceUid(PAINTER.load(Ordering::Relaxed))),
        )
    };
    say!(
        "GAME small live={} peak={} pool={SMALL_ARENA} list_most={} pages_most={} stack_free main={stack_main} painter={stack_painter} patches={:?} {:?}",
        HEAP.small_live.get(),
        HEAP.small_peak.get(),
        w.list_most,
        w.pages_most,
        lister.patch_bytes(),
        ge.patch_bytes()
    );
    let mix = jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed).wrapping_sub(w.mix0);
    let f = w.frames.max(1);
    say!(
        "GAME parts missed={} post_worst={}us pre={}us post={}us sync={}us show={}us audio={}us/frame ({}%) lister=[water {} pools {} n {} casters {} blocks {} silh {} sprites {} water2 {} silh_casters {} silh_blocks {} order {} lamp_polys {}] ge=[prep {} plan {} world {} ui {}]",
        w.missed,
        w.post_worst,
        w.pre / f,
        w.post / f,
        w.sync / f,
        w.show / f,
        mix / f,
        mix / (span / 100).max(1),
        lister.prof[0] / f,
        lister.prof[1] / f,
        lister.prof[2] / f,
        lister.prof[3] / f,
        lister.prof[4] / f,
        lister.prof[5] / f,
        lister.prof[6] / f,
        lister.prof[7] / f,
        lister.prof[8] / f,
        lister.prof[9] / f,
        lister.prof[10] / f,
        lister.prof[11] / f,
        ge.parts[0] / f,
        ge.parts[1] / f,
        ge.parts[2] / f,
        ge.parts[3] / f,
    );
    // The window's pacing in play, one line for a walk test to read (`tools/psp-walk-test.py`):
    // flips shown, the vblanks they took, those past the pacing's target (missed), the longest
    // flip; swatches (chunks painted that the painter did not land: the first view's too) and
    // the frames they showed (`wait_frames`) and most at once; the painter's extra vblanks and
    // boosts; lists the guard cut; paint jobs that hit an allocation failure (the run's).
    say!(
        "GAME pace scene={scene:?} tick={} ticks={} flips={} vblanks={} missed={} flip_max={} swatches={} wait_frames={} wait_max={} wait_streak={} mush_frames={} mush_streak={} extra_waits={} boosts={} list_full={} paint_fail={} quads_dropped={} polys_skipped={} patches_skipped={}",
        w.at_tick,
        w.ticks,
        w.flips,
        w.vblanks,
        w.missed,
        w.flip_max,
        w.rough,
        w.wait,
        w.waiting_max,
        w.wait_run_max,
        w.mush,
        w.mush_run_max,
        w.extra,
        w.boosts,
        w.list_full,
        diag::paint_fails(),
        w.quads_dropped,
        w.polys_skipped,
        w.patches_skipped
    );
    if let Some(wd) = world {
        let p = &wd.present.prof;
        let t = w.ticks.max(1);
        say!(
            "GAME tickparts units={} emotes={} props={} lights={} paint={} walls={} sky_atmos={} fx={} ambient={} head={} props_scan={} lights_scan={} amb=[blocks {} flocks {} water {} cap {}] rows_built={} casters={} blocks={} sprites={} parts={}",
            p[0] / t,
            p[1] / t,
            p[8] / t,
            p[9] / t,
            p[2] / t,
            p[3] / t,
            p[4] / t,
            p[5] / t,
            p[6] / t,
            p[7] / t,
            p[10] / t,
            p[11] / t,
            amb[0] / t,
            amb[1] / t,
            amb[2] / t,
            amb[3] / t,
            lister.prof[2],
            wd.present.frame().casters.len(),
            wd.present.frame().blocks.len(),
            wd.present.frame().sprites.len(),
            wd.present.frame().parts.len(),
        );
        // The sim's phases (`jane_sim::Phase`), microseconds a tick.
        let mut line = String::new();
        for (p, us) in jane_sim::Phase::ALL.iter().zip(w.sim_phases) {
            let _ = write!(line, " {}={}", p.name().replace(' ', "_"), us / t);
        }
        say!("GAME simparts{line}");
        say!(
            "GAME jobs n={} wall_us={} frames_waiting={} painted={} landed={}",
            JOBS.load(Ordering::Relaxed),
            JOB_US.load(Ordering::Relaxed) / JOBS.load(Ordering::Relaxed).max(1),
            w.wait,
            wd.present.chunks_painted(),
            wd.present.chunks_landed()
        );
        let ld = |a: &core::sync::atomic::AtomicU32| a.load(Ordering::Relaxed);
        say!(
            "GAME baked landed={} refused={} fails={} reads={} read_us={} decodes={} decode_us={} stand_ins_shown={} ram={}",
            ld(&loader::LANDED),
            ld(&loader::REFUSED),
            ld(&loader::FAILS),
            ld(&loader::READS),
            ld(&loader::READ_US) / ld(&loader::READS).max(1),
            ld(&loader::DECODES),
            ld(&loader::DECODE_US) / ld(&loader::DECODES).max(1),
            wd.present.stand_ins_shown,
            ld(&loader::RAM)
        );
        if let Some(v) = wd.sim.view(Seat(0)) {
            if let Some(u) = v.unit(v.me().unit) {
                let (x, y) = u.pos.cell();
                say!("GAME her zone={} cell=({x}, {y})", v.zone().name());
            }
        }
    }
}

/// A script's `atmos=N` word: the atmosphere's passes left out (`jane_render_psp::list::atmos_fx`
/// bits), to measure each one's cost.
fn script_atmos(text: Option<&str>) -> u16 {
    text.and_then(|t| t.split_whitespace().find_map(|w| w.strip_prefix("atmos=")?.parse().ok())).unwrap_or(0)
}

/// A script's weather word (`clear`, `mist`, `rain` or `storm`): the sky held to it, and in rain
/// or a storm the sim's sky and the county's ground too, as `jane sheet scene --weather` holds
/// them, so a scripted frame matches a PC one at the same state (the atmosphere's shots).
fn script_weather(text: Option<&str>, present: &mut Present, sim: &mut Sim) {
    use jane_present::WeatherKind as K;
    let Some(kind) = text.and_then(|t| {
        t.split_whitespace().find_map(|w| match w {
            "clear" => Some(K::Clear),
            "mist" => Some(K::Mist),
            "rain" => Some(K::Rain),
            "storm" => Some(K::Storm),
            _ => None,
        })
    }) else {
        return;
    };
    let wet = if matches!(kind, K::Rain | K::Storm) { 255 } else { 0 };
    present.atmos_mut().force(Some((kind, wet)));
    if wet > 0 {
        use jane_sim::state::{WeatherKind as Sky, WeatherState};
        let sky = if kind == K::Storm { Sky::Storm } else { Sky::Rain };
        let now = sim.state().tick;
        let st = sim.state_mut();
        st.weather = st.weather.map(|_| WeatherState { kind: sky, since: now, until: jane_core::Tick(u32::MAX) });
        if let Some(z) = st.zone_mut(ZoneId::County) {
            z.wetness = z.wetness.map(|_| 255);
        }
    }
}

/// A script's `flag:NAME` words: each world flag set to 1, and the spine's consequence of the
/// name marked done (`mine_quiet`, `works_dark`, `burial_quiet`: the night's stage, NIGHT.md
/// §3.2), as the sim's own tests set them. A dev script's, never a player's.
fn script_flags(names: &[String], sim: &mut Sim) {
    use jane_sim::state::FlagKey;
    for name in names {
        if let Some(c) = jane_data::catalog().living.consequence_id(name) {
            sim.state_mut().consequences_done.set(u32::from(c.0), true);
        }
        if let Some(k) = sim.state().syms.find(name) {
            sim.state_mut().flags.insert(FlagKey::Named(k), 1);
        }
        say!("GAME flag {name}");
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
    say!("GAME exit");
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
                super::MAIN_STACK,
                sys::ThreadAttributes::USER | sys::ThreadAttributes::VFPU,
                core::ptr::null_mut(),
            );
            sys::sceKernelStartThread(id, argc_bytes, argv);
        }
        0
    }
}
