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
use core::fmt::Write;

mod diag;
mod perf;
mod shell;
use psp::sys;

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

mod files;
mod game;
mod log;
mod platform;
mod script;
mod workers;
use files::{capture_header, cpath, write_file, File};
use game::{run, MAIN_STACK};
use platform::{free_mem, now_us, Hold, HEAP, THREADED};
use script::Script;
use workers::start_thread;

mod county_cache;

// Zones built ahead of her at a door (PORT.md §13.3, phase 4).
mod ahead;

// The built-in benchmark: the title's Benchmark row or a script's `bench` (its hooks below are
// marked "the bench").
mod bench;
mod micro;

// The baked county's chunks read and decoded off the game's thread (PLAN-PSP-SPEED.md P1c).
mod loader;

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
