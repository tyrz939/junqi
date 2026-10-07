#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

//! PORT.md §13.10: the real jane-sim on the PSP. Decodes a tape recorded on the PC (`tape.jrp`,
//! made by `host/`), builds its seed's blueprints, replays it frame by frame holding it to the
//! tape's hash stream (a hash every 60 ticks), and prints
//! `SIM hash=<hex> ticks=<n> peak_heap=<bytes>` to fd 1. A desync prints the first frame that
//! differs. Integers only; the only unsafe is the platform glue below.

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::{Cell, UnsafeCell};
use core::fmt::Write;
use core::ptr::NonNull;

use psp::sys;
use talc::{ErrOnOom, Span, Talc};

static TAPE: &[u8] = include_bytes!("../tape.jrp");
/// Print the heap at every worldgen stage too.
const VERBOSE: bool = true;

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

// The panic handler is rust-psp's under `stub-only` (a spin loop; a second `#[panic_handler]` is
// a duplicate lang item). A panic therefore shows as the headless run timing out after the last
// `SIM` progress line, which names the phase. For diagnosis, a local copy of rust-psp whose
// stub-only handler calls this symbol prints the message (see PORT.md §13.10).
#[no_mangle]
pub fn __spike_panic(info: &core::panic::PanicInfo<'_>) -> ! {
    // SAFETY: plain syscalls.
    let (max, total) = unsafe { (sys::sceKernelMaxFreeMemSize(), sys::sceKernelTotalFreeMemSize()) };
    say!(
        "SIM panic: {} live={} peak_heap={} kernel_max_free={max} kernel_total_free={total}",
        info,
        HEAP.live.get(),
        HEAP.peak.get()
    );
    // SAFETY: plain syscall; it does not return.
    unsafe { sys::sceKernelExitGame() };
    loop {}
}

// ---------------------------------------------------------------- heap

/// Two pools, counting live and peak bytes as requested by the program (allocator overhead is
/// not counted). Allocations under [`BIG`] go to one block of the user partition carved by
/// `talc`; big ones (worldgen grids) get their own kernel block, so the two never fragment each
/// other. Either pool falls back to the other. Single-threaded: only the main thread allocates.
struct Heap {
    talc: UnsafeCell<Talc<ErrOnOom>>,
    lo: Cell<usize>,
    hi: Cell<usize>,
    live: Cell<usize>,
    peak: Cell<usize>,
    allocs: Cell<u32>,
}

/// Allocations of this many bytes or more get their own kernel block.
const BIG: usize = 64 * 1024;
/// The small pool.
const SMALL_ARENA: u32 = 3 * 1024 * 1024;

// SAFETY: the PSP program runs one thread that allocates; nothing else touches the heap.
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

    /// A kernel block laid out `[id][pad..][pad count][data]`, as rust-psp's own allocator.
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

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
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

/// Claim the small pool from the user partition. Returns its size and the partition's largest
/// free block before it.
fn heap_init() -> (u32, u32) {
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
            say!("SIM error: cannot allocate the small pool ({:x})", id.0);
            return (0, max);
        }
        let base: *mut u8 = sys::sceKernelGetBlockHeadAddr(id).cast();
        HEAP.lo.set(base as usize);
        HEAP.hi.set(base as usize + SMALL_ARENA as usize);
        let _ = (*HEAP.talc.get()).claim(Span::from_base_size(base, SMALL_ARENA as usize));
        (SMALL_ARENA, max)
    }
}

fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

// ---------------------------------------------------------------- the spike

fn run() {
    let (small, free) = heap_init();
    say!("SIM heap user_free={free} small_pool={small} tape={}", TAPE.len());
    let t0 = now_us();
    let tape = match jane_sim::replay::Tape::decode(TAPE) {
        Ok(t) => t,
        Err(e) => {
            say!("SIM error: decode: {e}");
            return;
        }
    };
    say!(
        "SIM tape seed={} frames={} hashes={} content={:016x} peak_heap={}",
        tape.header.seed,
        tape.frames,
        tape.hashes.len(),
        jane_data::catalog().content_hash,
        HEAP.peak.get()
    );
    // Zone by zone (what `Blueprints::build` does), so the heap is seen per zone and per stage.
    // First the twelve small zones, each built, hashed against the PC's and dropped; then the
    // county (the big one, alone in the heap); then the twelve again, kept, for the replay.
    let seed = tape.header.seed;
    for z in jane_core::ZoneId::ALL.into_iter().filter(|&z| z != jane_core::ZoneId::County) {
        if build(z, seed, false).is_none() {
            return;
        }
    }
    let mut zones = alloc::vec::Vec::new();
    for z in jane_core::ZoneId::ALL {
        match build(z, seed, VERBOSE && z == jane_core::ZoneId::County) {
            Some(bp) => zones.push(alloc::sync::Arc::new(bp)),
            None => return,
        }
    }
    let Ok(zones) = zones.try_into() else { return };
    let bps = jane_sim::Blueprints::from_parts(seed, zones);
    let t1 = now_us();
    say!("SIM built blueprints us={} live={} peak_heap={}", t1.wrapping_sub(t0), HEAP.live.get(), HEAP.peak.get());
    // From here the peak is the running sim's: blueprints resident plus state, scratch, journal.
    let build_peak = HEAP.peak.get();
    HEAP.peak.set(HEAP.live.get());
    say!("SIM build_peak={build_peak}");
    match jane_sim::replay::verify_tape(&tape, bps) {
        Ok(v) => {
            let t2 = now_us();
            say!(
                "SIM replay us={} live={} allocs={} hashes_checked={}",
                t2.wrapping_sub(t1),
                HEAP.live.get(),
                HEAP.allocs.get(),
                v.hashes
            );
            say!("SIM hash={:016x} ticks={} peak_heap={}", v.final_hash, v.ticks, HEAP.peak.get());
        }
        Err(e) => say!("SIM error: {e} peak_heap={}", HEAP.peak.get()),
    }
}

/// One zone's blueprint, its hash and the heap after it printed (and every stage if `stages`).
fn build(z: jane_core::ZoneId, seed: u32, stages: bool) -> Option<jane_core::Blueprint> {
    let mut stage = |s: &'static str| {
        if stages {
            say!("SIM   stage {s} live={} peak_heap={}", HEAP.live.get(), HEAP.peak.get());
        }
    };
    let tz = now_us();
    match jane_sim::blueprints::build_one_with(z, seed, &mut stage) {
        Ok(bp) => {
            say!(
                "SIM zone {} bp={:016x} us={} live={} peak_heap={}",
                z.name(),
                jane_world::hash::hash(&bp),
                now_us().wrapping_sub(tz),
                HEAP.live.get(),
                HEAP.peak.get()
            );
            Some(bp)
        }
        Err(e) => {
            say!("SIM error: {e}");
            None
        }
    }
}

fn psp_main() {
    run();
    // SAFETY: plain syscall; ends the program so the headless runner exits.
    unsafe { sys::sceKernelExitGame() };
}

// ---------------------------------------------------------------- module glue
// `psp::module!` calls rust-psp's `catch_unwind`, which `stub-only` leaves out, so the module
// header is spelled out here (from psp 0.3.14 `lib.rs`), calling `psp_main` directly.

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
        mod_name: sys::SceModuleInfo::name("psp_sim"),
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
        extern "C" fn main_thread(_argc: usize, _argv: *mut c_void) -> i32 {
            super::psp_main();
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
