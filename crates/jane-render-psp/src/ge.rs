//! The GE glue (PSP only): the [`Lister`]'s quads to the screen through rust-psp's `sceGu*`.
//! The one module with unsafe code: it writes the display list, VRAM and the GE's registers.
//!
//! VRAM (2 MB): two `8888` framebuffers 512 px wide (1.06 MB), then [`SLOTS`] page slots of
//! 66 560 bytes (a 256 x 256 `T8` page and its CLUT). A page is loaded from the pack into RAM
//! when a frame first names it (an LRU under a byte budget, [`Lru`]), copied into a VRAM slot
//! when drawn (an LRU of slots, [`Slots`]); a page over 256 x 256 is drawn from RAM. A terrain
//! chunk is drawn from RAM as an `8888` texture, converted from the frame's layer when its
//! generation changes. No depth buffer: the frame is in draw order. Integer only: the quads
//! are 16-bit through-mode vertices.
//!
//! **The CPU's data cache and the GE (PORT.md §13.13's audit).** The GE reads RAM, not the
//! CPU's write-back data cache, and runs the list while it is written (each draw moves the
//! stall address). So: the list and its vertices are written through the uncached mirror
//! (rust-psp's `sceGuStart` and `sceGuGetMemory`); everything the CPU wrote before a list (pages
//! read from the stick, chunks landed, CLUTs, UI images) is written back by one
//! `sceKernelDcacheWritebackAll` before `sceGuStart` (the frame's terrain patches too: one copy
//! into one arena); what is written while the list runs (a chunk's CLUT, a glow CLUT) is written
//! back by range before its command; VRAM is written through the uncached mirror (page slots,
//! lamp pools). A buffer written while the list runs is one no earlier command of that list
//! reads (a slot not drawn this frame, a chunk's first bind), and nothing the list reads is
//! freed or rewritten until `draw` has waited for it to finish (no frame is pipelined, so no
//! double buffer is needed). The texture cache is flushed at each list's start, after each VRAM
//! upload and new chunk, and before the frame or the lightmap's target is read as a texture (with
//! a sync). The CPU reads GE-written memory only through the uncached mirror (`shown`).
//!
//! **The list's room.** The list is a static of 256 KB and the GU checks no write against its end,
//! so `draw` never writes a frame it has not planned to fit [`Ge::list_cap`] (`list::guard`: the
//! frame's cost, and the passes given up first to last when it would not fit) and tests the room
//! before every batch it does write. Both are counted in [`DrawStats::list_full`].

use alloc::alloc::{Layout, alloc, dealloc};
use alloc::vec::Vec;
use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

use psp::sys::{self, ClearBuffer, GuPrimitive, GuState, TexturePixelFormat, VertexType};

use crate::cache::{Lru, Slots};
use crate::list::guard::{self, Shed};
use crate::list::{Lister, Mode, PATCH_WORDS, Tex};
use crate::pack::Pack;
use jane_present::Frame;
use jane_present::frame::CHUNK_PX;

const BUF_W: i32 = 512;
const SCR_W: i32 = 480;
const SCR_H: i32 = 272;
const FB_BYTES: u32 = (BUF_W * SCR_H * 4) as u32;
/// A VRAM slot: a 256 x 256 `T8` page and its CLUT.
const SLOT_BYTES: u32 = 1024 + 256 * 256;
/// The lightmap's render target after the framebuffers: 256 wide, `light::ROWS` rows, `8888`.
const RT_OFFSET: u32 = 2 * FB_BYTES;
const RT_BYTES: u32 = (crate::light::SIDE * crate::light::ROWS * 4) as u32;
/// The lamp cache's pools after the target (`lamps::SLOTS` of `lamps::TEX` squared bytes).
const LAMP_OFFSET: u32 = RT_OFFSET + RT_BYTES;
const LAMP_BYTES: u32 = (crate::lamps::SLOTS * crate::lamps::TEX * crate::lamps::TEX) as u32;
/// VRAM slots after the framebuffers, the lightmap's target and the lamp cache.
pub const SLOTS: usize = ((0x20_0000 - LAMP_OFFSET - LAMP_BYTES) / SLOT_BYTES) as usize;
/// Set on a page index: its normal page (RAM keys and `hold`).
const NORMAL: u16 = 0x4000;
/// A page key's place in the RAM table: the albedo pages, then their normal pages.
fn ram_ix(pages: usize, key: u16) -> usize {
    if key & NORMAL != 0 { pages + usize::from(key & !NORMAL) } else { usize::from(key) }
}

/// The mist tile's side (`jane_art::weather::MIST_SIDE`).
const MIST_SIDE: usize = 256;

/// The power of two at or over `n` (a texture's declared side; the GE reads only the rows and
/// columns the quads name).
fn pow2(n: u16) -> i32 {
    i32::from(n.max(1).next_power_of_two().min(512))
}

/// The display list: a frame's state changes and its vertices.
const LIST_WORDS: usize = 256 * 1024 / 4;
/// Bytes of the list kept free at its end: the state put back after the world's quads and after
/// the UI's, the finish, and a few signals. No batch is begun that would reach into them.
const LIST_TAIL: usize = 2048;
/// Bytes the list holds before its first batch (the flush, the clear, a signal).
const LIST_HEAD: usize = 256;

#[repr(C, align(16))]
struct List([u32; LIST_WORDS]);
/// Two: while the GE runs one frame's list, the next is written into the other (pipelined).
///
/// The list is written with `sceGuGetMemory`, which checks nothing: a frame past the end writes
/// over whatever follows the static (the GU's own state, once: the owner's crash and the walk
/// test's faults at a wood). So `draw` plans the frame to fit [`Ge::list_cap`] before it writes
/// ([`guard::plan`]) and tests the room before every batch it writes, whatever the plan said.
static mut LIST: [List; 2] = [List([0; LIST_WORDS]), List([0; LIST_WORDS])];

/// How a drawn frame is shown (the Graphics page's frame rate).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pacing {
    /// At the vblank after it is drawn: 60 when it fits.
    #[default]
    Vsync,
    /// Every second vblank at the soonest: a steady 30.
    Locked30,
    /// As soon as it is drawn, mid-scan (a tear line), as fast as it goes.
    Unlocked,
}

/// A sprite vertex: 16-bit texel, `8888` colour, 16-bit position (through mode). The layout
/// [`put_tex`] writes as words.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct TexVertex {
    u: u16,
    v: u16,
    colour: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

/// A flat vertex: `8888` colour, 16-bit position. The layout [`put_flat`] writes as words.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct FlatVertex {
    colour: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

/// A textured vertex written into the list as four words (the list is uncached: a store each,
/// where the struct's fields took seven).
///
/// # Safety
/// `p` points into the list's memory, room for one vertex, 4-byte aligned.
#[inline(always)]
unsafe fn put_tex(p: *mut TexVertex, (u, v): (u16, u16), colour: u32, (x, y): (i16, i16)) {
    const _: () = assert!(core::mem::size_of::<TexVertex>() == 16);
    let w = p.cast::<u32>();
    // SAFETY: as the caller's; the words are the struct's bytes (little-endian, z and pad 0).
    unsafe {
        w.write(u32::from(u) | u32::from(v) << 16);
        w.add(1).write(colour);
        w.add(2).write(u32::from(x as u16) | u32::from(y as u16) << 16);
        w.add(3).write(0);
    }
}

/// A flat vertex written as three words (where its fields took five stores).
///
/// # Safety
/// As [`put_tex`].
#[inline(always)]
unsafe fn put_flat(p: *mut FlatVertex, colour: u32, (x, y): (i16, i16)) {
    const _: () = assert!(core::mem::size_of::<FlatVertex>() == 12);
    let w = p.cast::<u32>();
    // SAFETY: as the caller's.
    unsafe {
        w.write(colour);
        w.add(1).write(u32::from(x as u16) | u32::from(y as u16) << 16);
        w.add(2).write(0);
    }
}

/// A block of RAM aligned for the GE (64 bytes: the pack's own alignment).
struct Buf {
    ptr: *mut u8,
    len: usize,
}

impl Buf {
    fn new(len: usize) -> Option<Buf> {
        let layout = Layout::from_size_align(len.max(64), 64).ok()?;
        // SAFETY: a non-zero size; freed with the same layout in `drop`.
        let ptr = unsafe { alloc(layout) };
        (!ptr.is_null()).then_some(Buf { ptr, len: len.max(64) })
    }

    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` holds `len` bytes we own.
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    fn words(&mut self) -> &mut [u32] {
        // SAFETY: 64-aligned, `len` a multiple of 4 for every word buffer made here.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.cast::<u32>(), self.len / 4) }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        // SAFETY: allocated in `new` with this layout.
        unsafe { dealloc(self.ptr, Layout::from_size_align_unchecked(self.len, 64)) }
    }
}

/// The GE's signals this list (PORT.md §13.13's per-pass timing): each one's pass id and the
/// time the GE reached it, written by the signal callback (an interrupt), read after the sync.
const SIGNALS: usize = 64;
static SIG_N: AtomicU32 = AtomicU32::new(0);
static SIG_T: [AtomicU32; SIGNALS] = [const { AtomicU32::new(0) }; SIGNALS];
static SIG_ID: [AtomicU32; SIGNALS] = [const { AtomicU32::new(0) }; SIGNALS];
/// The id the list's last signal carries: the end of the last pass.
const SIG_END: u8 = 0x7f;
/// A sub-pass's mark when timed, at most: its signal and the batch it ends.
const SUB_MARK_BYTES: usize = guard::SIGNAL + guard::BIND + guard::MODE + guard::DRAW;

extern "C" fn on_signal(id: i32, _arg: *mut c_void) {
    // SAFETY: a plain syscall (the timer), fine in an interrupt.
    let t = unsafe { sys::sceKernelGetSystemTimeLow() };
    let n = SIG_N.load(Ordering::Relaxed) as usize;
    if n < SIGNALS {
        SIG_T[n].store(t, Ordering::Relaxed);
        SIG_ID[n].store(id as u32 & 0xff, Ordering::Relaxed);
        SIG_N.store(n as u32 + 1, Ordering::Release);
    }
}

/// A signal into the open list: the GE runs on (`Continue`) and calls `on_signal` with `id`
/// (offset past the ids the GU keeps for itself).
///
/// # Safety
/// Inside an open display list.
unsafe fn signal(id: u8) {
    // The GE reads SIGNAL's behaviour from bits 16 to 23 and the id from the low 16 (as
    // PPSSPP and the hardware do); rust-psp's `sceGuSignal` (pspsdk's order) puts the id high,
    // which the GE takes for a jump or an end. So the two commands by hand: SIGNAL, then END.
    // SAFETY: commands into the open list.
    unsafe {
        sys::sceGuSendCommandi(
            sys::GeCommand::Signal,
            (sys::SignalBehavior::Continue as i32) << 16 | (i32::from(id) + 0x10),
        );
        sys::sceGuSendCommandi(sys::GeCommand::End, 0);
    }
}

/// The GE's transfer rows a page upload is cut into: 32-bit px a row (a buffer width the transfer
/// takes: a multiple of 8, at most 1024).
const UPLOAD_ROW: usize = 512;

/// `bytes` of a page from RAM (`src`, written back: the list's `sceKernelDcacheWritebackAll`
/// came after its load) into its VRAM slot (`vram`), by the GE in the list's order (PLAN-PSP-
/// SPEED.md Q4): the CPU no longer stores it through the uncached mirror. As 32-bit px, rows of
/// [`UPLOAD_ROW`], then the rest as one short row; the texture read after it waits for it and
/// reads it fresh. A page not 16-byte aligned or not whole words is copied by the CPU as before.
///
/// # Safety
/// Inside an open display list; `src` holds `bytes` that stay unwritten until the list has run
/// (the frame's pages are held to the next `draw`), and no command of this list before this one
/// reads the slot (`Slots::place`).
unsafe fn upload(src: *const u8, vram: *mut u8, bytes: usize) {
    // SAFETY: as the caller's.
    unsafe {
        if (src as usize | vram as usize | bytes) % 16 != 0 {
            let dst = (vram as usize | 0x4000_0000) as *mut u8;
            core::ptr::copy_nonoverlapping(src, dst, bytes);
            sys::sceGuTexFlush();
            return;
        }
        let words = bytes / 4;
        let (rows, rest) = (words / UPLOAD_ROW, words % UPLOAD_ROW);
        let w = UPLOAD_ROW as i32;
        let (s, d) = (src.cast_mut().cast::<c_void>(), vram.cast::<c_void>());
        if rows > 0 {
            sys::sceGuCopyImage(sys::DisplayPixelFormat::Psm8888, 0, 0, w, rows as i32, w, s, 0, 0, w, d);
        }
        if rest > 0 {
            let y = rows as i32;
            sys::sceGuCopyImage(sys::DisplayPixelFormat::Psm8888, 0, y, rest as i32, 1, w, s, 0, y, w, d);
        }
        sys::sceGuTexSync();
        sys::sceGuTexFlush();
    }
}

/// Back from the lightmap's target to the frame being drawn (`back`: the framebuffer's index).
///
/// # Safety
/// Inside an open display list.
unsafe fn end_rt(back: u32) {
    // SAFETY: commands into the open list.
    unsafe {
        sys::sceGuDisable(GuState::StencilTest);
        sys::sceGuPixelMask(0);
        let fb = if back == 0 { 0 } else { FB_BYTES };
        sys::sceGuDrawBufferList(sys::DisplayPixelFormat::Psm8888, fb as *mut c_void, BUF_W);
        sys::sceGuScissor(0, 0, SCR_W, SCR_H);
        sys::sceGuTexFlush();
        sys::sceGuTexSync();
    }
}

/// Data-cache write-backs and their bytes (a capture's events).
static WB: AtomicU32 = AtomicU32::new(0);
static WB_BYTES: AtomicU32 = AtomicU32::new(0);

/// `sceKernelDcacheWritebackRange`, counted.
///
/// # Safety
/// `p` names `n` bytes the caller owns.
unsafe fn wb_range(p: *const c_void, n: u32) {
    WB.fetch_add(1, Ordering::Relaxed);
    WB_BYTES.fetch_add(n, Ordering::Relaxed);
    // SAFETY: as the caller's.
    unsafe { sys::sceKernelDcacheWritebackRange(p, n) };
}

/// `sceKernelDcacheWritebackAll`, counted (as the cache's 16 KB).
///
/// # Safety
/// A plain syscall.
unsafe fn wb_all() {
    WB.fetch_add(1, Ordering::Relaxed);
    WB_BYTES.fetch_add(16 * 1024, Ordering::Relaxed);
    // SAFETY: a plain syscall.
    unsafe { sys::sceKernelDcacheWritebackAll() };
}

/// Waits until the GE has run every list sent (a chunk's paint into a slot a running list may
/// read: `Present::set_chunk_fence`).
pub fn wait_idle() {
    // SAFETY: a plain syscall.
    unsafe { sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait) };
}

/// CLUT loads sent this list (a count for the capture).
static CLUT_LOADS: AtomicU32 = AtomicU32::new(0);
/// The CLUT this list loaded last: its address and blocks (0: none yet).
static CLUT_AT: AtomicU32 = AtomicU32::new(0);
static CLUT_BLOCKS: AtomicU32 = AtomicU32::new(0);

/// `sceGuClutLoad`, counted, unless it is the CLUT this list loaded last (PLAN-PSP-SPEED.md Q6):
/// the GE's CLUT still holds it, as nothing a list reads is rewritten after its first command
/// (a page's CLUT, graded or not, a chunk's, the lamps' grey: each written before the list or
/// before its own first load, and kept until the list has run).
///
/// # Safety
/// Inside an open display list; `clut` lives until the list has run.
unsafe fn clut_load(blocks: i32, clut: *const c_void) {
    let at = clut as u32;
    if CLUT_AT.load(Ordering::Relaxed) == at && CLUT_BLOCKS.load(Ordering::Relaxed) == blocks as u32 {
        return;
    }
    CLUT_AT.store(at, Ordering::Relaxed);
    CLUT_BLOCKS.store(blocks as u32, Ordering::Relaxed);
    CLUT_LOADS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: as the caller's.
    unsafe { sys::sceGuClutLoad(blocks, clut) };
}

/// The display list's bytes between moves of its stall address: the GE runs this far behind
/// the list at most while it is written, where each batch once moved it (two syscalls a batch).
const STALL_STEP: usize = 6 * 1024;

/// What the last frame took, for the game's log line.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawStats {
    pub quads: u32,
    pub batches: u32,
    pub page_loads: u32,
    pub page_load_fails: u32,
    pub uploads: u32,
    pub chunks_converted: u32,
    pub ram_pages_bytes: u32,
    pub chunk_bytes: u32,
    /// Microseconds the CPU waited for the GE to finish the list (its fill, as the emulator
    /// times it).
    pub sync_us: u32,
    /// Microseconds waited to show the last frame (the vblank, or two for a steady 30).
    pub show_us: u32,
    /// The display list's bytes, and the bytes of the pages this frame drew from.
    pub list_bytes: u32,
    pub frame_page_bytes: u32,
    /// Texture binds, CLUT loads and blend-mode changes sent.
    pub binds: u32,
    pub clut_loads: u32,
    pub modes: u32,
    /// Pages the RAM cache let go, render-target switches, stencil-mode changes, the sync's
    /// value when not done (0), data-cache write-backs and their bytes (a capture's events).
    pub evicts: u32,
    pub rts: u32,
    pub stencil: u32,
    pub ge_error: u32,
    pub wb: u32,
    pub wb_bytes: u32,
    /// With [`Ge::timing`]: the GE's microseconds by pass (`capture::pass`), and its whole list
    /// from the first signal to the last.
    pub passes: [u32; crate::capture::pass::N],
    pub ge_total: u32,
    /// 1 when the frame did not fit the display list's room ([`Ge::list_cap`]) and the list
    /// guard acted: it left out passes ([`DrawStats::list_shed`]) or, past those, stopped the
    /// world's batches short. A capture's `ev_list_full`.
    pub list_full: u32,
    /// The guard's rung ([`guard::Shed::level`]; 0: nothing left out), 100 more when its limit
    /// before a batch stopped the world short of the plan.
    pub list_shed: u32,
    /// The bytes the guard planned the frame's list at (the world's and the UI's), to compare
    /// with `list_bytes`: it is a bound, not under what is written.
    pub list_est: u32,
}

/// The GE, its framebuffers, the pages held and the chunks converted.
pub struct Ge {
    pack: Pack,
    /// Each page's RAM copy, by page index.
    ram: Vec<Option<Buf>>,
    lru: Lru,
    slots: Slots,
    /// Each presenter chunk slot's texture and the generation it holds.
    chunks: Vec<Option<(u32, Buf)>>,
    /// This frame's terrain patches (`Lister::patch_px`), copied where the GE may read them: one
    /// arena of up to [`PATCH_WORDS`] words, made (and grown) as a frame has patches and kept.
    patch_arena: Option<Buf>,
    /// This frame's light CLUT for the normal pages (16 entries).
    relief: Option<Buf>,
    /// This frame's lamp relief CLUTs (`Lister::lamp_reliefs`), 64 bytes each.
    lamp_cluts: Option<Buf>,
    /// The halo disc's texture, and the pool's.
    disc: Option<Buf>,
    /// A glowing particle's disc (`light::spot`).
    spot: Option<Buf>,
    /// The lamp cache's CLUT: entry `i` white at alpha `i`.
    grey: Option<Buf>,
    pool: Option<Buf>,
    /// Each page's glow CLUT once read from the pack (1 KB each).
    glow: Vec<Option<Buf>>,
    /// The CLUT that reads a chunk's height layer as the raised terrain's mask: clear at ground
    /// height (`shadow::GROUND` and under), opaque over it.
    height_mask: Option<Buf>,
    /// Each `T8` slot's generation last bound and its CLUT, copied where the GE may load it.
    direct: Vec<Option<(u32, Buf, Option<Buf>)>>,
    evicted: Vec<u16>,
    /// The UI's ink CLUT: entry 0 clear, every other white (`Tex::Ink`).
    white: Option<Buf>,
    /// Each UI image slot's texture (`Tex::Image`): its generation, size and `8888` texels.
    images: Vec<Option<(u32, u16, u16, Buf)>>,
    /// The lister's CLUTs for `Tex::Frame` (the grade's), copied where the GE may load them,
    /// and the rebuild they hold.
    cluts: Option<Buf>,
    cluts_gen: u32,
    /// The mist tile (`Ge::set_mist`, 64 KB in RAM, `T8`) and this frame's fog CLUT for it.
    mist: Option<Buf>,
    fog_clut: Option<Buf>,
    /// The puddles' noise tile (`water::noise_tile`, 64 KB in RAM, `T8`) and this frame's
    /// threshold CLUT for it.
    noise: Option<Buf>,
    noise_clut: Option<Buf>,
    /// The framebuffer drawn into: 0 or 1.
    back: u32,
    pub stats: DrawStats,
    /// Signals between the passes, so [`DrawStats::passes`] times each (the overlay's second
    /// page and a capture; off, the list has none).
    pub timing: bool,
    /// The CPU's next frame runs while the GE draws this one ([`Ge::draw`] returns without
    /// waiting; the next draw waits, then shows it). Off: each frame waits for its own list.
    pub pipelined: bool,
    pub pacing: Pacing,
    /// A list sent and not yet waited for; a frame drawn and not yet shown; the list in use.
    pending: bool,
    unshown: bool,
    list_ix: usize,
    /// The vblank count at the last show (`Pacing::Locked30`).
    last_vcount: u32,
    /// Frames shown ([`Ge::present`]) and, of the vblanks they took, those past the pacing's
    /// target ([`Ge::flip_target`]): the run's totals. A flip that missed took more vblanks than
    /// the pacing asks (3 or more at a locked 30, 2 or more at 60).
    pub flips: u32,
    pub missed_vblanks: u32,
    /// The vblanks the flips took since [`Ge::take_flip_vblanks`] last took them.
    flip_vblanks: u32,
    /// Microseconds of `draw`'s own work by part, summed until read (the game's log window):
    /// before the list (pages held, CLUTs and patches copied), the list guard's plan, the world's
    /// quads into the list, the UI's.
    pub parts: [u32; 4],
    /// Bytes of display list a frame may use, at most the list's 256 KB (and at least a few
    /// KB): the list guard plans each frame to fit it. A script lowers it (`listcap:KB`) to
    /// force the guard.
    pub list_cap: usize,
    /// Pages let go while a list may still read them: freed once it has run.
    grave: Vec<Buf>,
    /// A read-back quad's slices (scratch).
    sliced: Vec<crate::list::Quad>,
    /// The palette grade's CLUTs (`list::Palette`): each page's and each glow CLUT's graded
    /// copy and the grade it is of; each chunk slot's CLUT's grade (`None` ungraded).
    pal_pages: Vec<Option<(u32, Buf)>>,
    pal_glow: Vec<Option<(u32, Buf)>>,
    direct_pal: Vec<Option<u32>>,
    /// The waits [`Ge::settle`] and [`Ge::present`] took since the last draw's stats, the
    /// sync's last value other than done, pages let go since.
    ge_error: u32,
    evicts: u32,
    wait_sync: u32,
    wait_show: u32,
}

impl core::fmt::Debug for Ge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ge").field("stats", &self.stats).finish_non_exhaustive()
    }
}

fn list_ptr(k: usize) -> *mut c_void {
    // The display lists' static; only the game's one thread touches it.
    // SAFETY: the address of one of the two, not a reference.
    unsafe { core::ptr::addr_of_mut!(LIST[k & 1]).cast() }
}

/// The GE's state every list builds on, and the display: drawn into the first framebuffer,
/// shown from the second. At start-up and again after a resume (PORT.md §13.13).
fn base_state() {
    // SAFETY: a list of state commands, run and waited for; nothing else is drawing.
    unsafe {
        sys::sceGuStart(sys::GuContextType::Direct, list_ptr(0));
        sys::sceGuDrawBuffer(sys::DisplayPixelFormat::Psm8888, core::ptr::null_mut(), BUF_W);
        sys::sceGuDispBuffer(SCR_W, SCR_H, FB_BYTES as *mut c_void, BUF_W);
        // No depth buffer: nothing is depth tested, and no write may land in the slots.
        sys::sceGuDepthMask(1);
        sys::sceGuOffset(2048 - (SCR_W as u32 / 2), 2048 - (SCR_H as u32 / 2));
        sys::sceGuScissor(0, 0, SCR_W, SCR_H);
        sys::sceGuEnable(GuState::ScissorTest);
        sys::sceGuDisable(GuState::DepthTest);
        // Smooth: a strip's and a line's colours run between their ends (a sprite takes its
        // second vertex's either way).
        sys::sceGuShadeModel(sys::ShadingModel::Smooth);
        sys::sceGuEnable(GuState::Blend);
        sys::sceGuEnable(GuState::AlphaTest);
        sys::sceGuAlphaFunc(sys::AlphaFunc::Greater, 0, 0xff);
        sys::sceGuDisable(GuState::StencilTest);
        sys::sceGuPixelMask(0);
        sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
        sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
        sys::sceGuTexFlush();
        sys::sceGuFinish();
        sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait);
        sys::sceDisplayWaitVblankStart();
        sys::sceGuDisplay(true);
    }
}

impl Ge {
    /// Starts the GE and the display over `pack`'s tables; pages held in RAM up to `ram_budget`
    /// bytes.
    pub fn new(pack: Pack, ram_budget: u32) -> Ge {
        let pages = pack.pages.len();
        // SAFETY: the GU's start-up, once, before any draw.
        unsafe {
            sys::sceGuInit();
            sys::sceGuSetCallback(sys::GuCallbackId::Signal, Some(on_signal));
        }
        base_state();
        Ge {
            pack,
            ram: (0..2 * pages).map(|_| None).collect(),
            lru: Lru::new(ram_budget),
            slots: Slots::new(SLOTS),
            chunks: Vec::new(),
            patch_arena: None,
            relief: Buf::new(64),
            glow: (0..pages).map(|_| None).collect(),
            lamp_cluts: Buf::new(crate::list::LAMP_RELIEFS * 64),
            grey: Buf::new(1024).map(|mut b| {
                for (k, w) in b.words()[..256].iter_mut().enumerate() {
                    *w = (k as u32) << 24 | 0x00ff_ffff;
                }
                // SAFETY: our buffer, written once.
                unsafe { wb_range(b.ptr.cast(), 1024) };
                b
            }),
            pool: Buf::new(crate::light::POOL * crate::light::POOL * 4).map(|mut b| {
                let d = crate::light::pool_disc();
                b.words()[..d.len()].copy_from_slice(&d);
                // SAFETY: our buffer, written once.
                unsafe { wb_range(b.ptr.cast(), (d.len() * 4) as u32) };
                b
            }),
            disc: Buf::new(crate::light::DISC * crate::light::DISC * 4).map(|mut b| {
                let d = crate::light::disc();
                b.words()[..d.len()].copy_from_slice(&d);
                // SAFETY: our buffer, written once.
                unsafe { wb_range(b.ptr.cast(), (d.len() * 4) as u32) };
                b
            }),
            spot: Buf::new(crate::light::DISC * crate::light::DISC * 4).map(|mut b| {
                let d = crate::light::spot();
                b.words()[..d.len()].copy_from_slice(&d);
                // SAFETY: our buffer, written once.
                unsafe { wb_range(b.ptr.cast(), (d.len() * 4) as u32) };
                b
            }),
            height_mask: Buf::new(1024).map(|mut b| {
                for (k, w) in b.words()[..256].iter_mut().enumerate() {
                    *w = if k as i32 > jane_present::shadow::GROUND { 0xff00_0000 } else { 0 };
                }
                b
            }),
            direct: Vec::new(),
            evicted: Vec::new(),
            white: Buf::new(1024).map(|mut b| {
                for (k, w) in b.words()[..256].iter_mut().enumerate() {
                    *w = if k == 0 { 0 } else { 0xffff_ffff };
                }
                // SAFETY: our buffer, written once.
                unsafe { wb_range(b.ptr.cast(), 1024) };
                b
            }),
            images: Vec::new(),
            cluts: None,
            cluts_gen: u32::MAX,
            mist: None,
            fog_clut: Buf::new(1024),
            noise: {
                let t = crate::water::noise_tile();
                Buf::new(t.len()).map(|mut b| {
                    b.bytes()[..t.len()].copy_from_slice(&t);
                    // SAFETY: our buffer, written once.
                    unsafe { wb_range(b.ptr.cast(), t.len() as u32) };
                    b
                })
            },
            noise_clut: Buf::new(1024),
            back: 0,
            stats: DrawStats::default(),
            timing: false,
            pipelined: false,
            pacing: Pacing::Vsync,
            pending: false,
            unshown: false,
            list_ix: 0,
            last_vcount: 0,
            flips: 0,
            missed_vblanks: 0,
            flip_vblanks: 0,
            parts: [0; 4],
            list_cap: LIST_WORDS * 4,
            grave: Vec::new(),
            sliced: Vec::with_capacity(64),
            pal_pages: Vec::new(),
            pal_glow: Vec::new(),
            direct_pal: Vec::new(),
            wait_sync: 0,
            wait_show: 0,
            ge_error: 0,
            evicts: 0,
        }
    }

    /// The mist tile the fog drifts (the presenter's, `Present::atlas().mist`: 256 x 256 alpha),
    /// copied where the GE may read it. Once, after `new`; no fog is drawn without it.
    pub fn set_mist(&mut self, tile: &[u8]) {
        let side = MIST_SIDE;
        if tile.len() != side * side {
            return;
        }
        self.mist = Buf::new(tile.len()).map(|mut b| {
            b.bytes()[..tile.len()].copy_from_slice(tile);
            // SAFETY: our buffer, written once.
            unsafe { wb_range(b.ptr.cast(), tile.len() as u32) };
            b
        });
    }

    /// The pack's tables.
    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    /// Makes page `p` held in RAM, loading it through `load(offset, bytes)` on a miss.
    fn hold(&mut self, p: u16, load: &mut dyn FnMut(u32, &mut [u8]) -> bool) -> bool {
        // A normal page is keyed by its page with `NORMAL` set.
        let Some(info) = self.pack.pages.get(usize::from(p & !NORMAL)).copied() else { return false };
        let (offset, bytes) = if p & NORMAL != 0 {
            let Some(n) = info.normal else { return false };
            n
        } else {
            (info.offset, info.bytes())
        };
        if self.lru.want(p, bytes, &mut self.evicted) && self.ram[ram_ix(self.pack.pages.len(), p)].is_some() {
            return true;
        }
        for e in core::mem::take(&mut self.evicted) {
            // A list still running may read it: freed after it has run.
            if let Some(b) = self.ram[ram_ix(self.pack.pages.len(), e)].take() {
                self.grave.push(b);
                self.evicts += 1;
            }
            if let Some(Some((_, b))) = self.pal_pages.get_mut(usize::from(e)).map(Option::take) {
                self.grave.push(b);
            }
            self.slots.forget(e);
        }
        // A page that could not be had is not held: the LRU lets go of it (its bytes uncounted),
        // so the next frame that names it loads it again, and no slot ever holds it.
        let Some(mut buf) = Buf::new(bytes as usize) else {
            self.stats.page_load_fails += 1;
            self.lru.forget(p);
            return false;
        };
        if !load(offset, &mut buf.bytes()[..bytes as usize]) {
            self.stats.page_load_fails += 1;
            self.lru.forget(p);
            return false;
        }
        self.stats.page_loads += 1;
        self.ram[ram_ix(self.pack.pages.len(), p)] = Some(buf);
        true
    }

    /// Page `p`'s glow CLUT held (read from the pack the first time).
    fn hold_glow(&mut self, p: u16, load: &mut dyn FnMut(u32, &mut [u8]) -> bool) -> bool {
        let i = usize::from(p);
        if matches!(self.glow.get(i), Some(Some(_))) {
            return true;
        }
        let Some(off) = self.pack.pages.get(i).and_then(|pg| pg.glow) else { return false };
        let Some(mut b) = Buf::new(1024) else { return false };
        if !load(off, &mut b.bytes()[..1024]) {
            return false;
        }
        // SAFETY: our buffer; the GE reads it after the list starts.
        unsafe { wb_range(b.ptr.cast(), 1024) };
        self.glow[i] = Some(b);
        true
    }

    /// Binds page `p` as the texture: from its VRAM slot (copied there first if it is not), or
    /// from RAM when it is larger than a slot or no slot is free this frame, through `clut` (its
    /// own CLUT when `None`: one load, not its own and then the one wanted). Inside a display
    /// list.
    fn bind_page(&mut self, p: u16, clut: Option<*const c_void>) {
        let info = self.pack.pages[usize::from(p)];
        let Some(buf) = self.ram[ram_ix(self.pack.pages.len(), p)].as_mut() else { return };
        let mut base = buf.ptr.cast_const();
        // SAFETY: VRAM past the framebuffers is the slots'; the slot is not drawn from this
        // frame (`Slots::place`), so the GE is not reading it; the GU calls only add commands.
        unsafe {
            if info.bytes() <= SLOT_BYTES
                && let Some((slot, fresh)) = self.slots.place(p)
            {
                let vram = (0x0400_0000 + LAMP_OFFSET + LAMP_BYTES + slot as u32 * SLOT_BYTES) as *mut u8;
                if fresh {
                    upload(buf.ptr, vram, info.bytes() as usize);
                    self.stats.uploads += 1;
                }
                base = vram;
            }
            sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, i32::from(info.swizzled));
            sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
            clut_load(32, clut.unwrap_or(base.cast()));
            let (w, h) = (i32::from(info.w), i32::from(info.h));
            sys::sceGuTexImage(sys::MipmapLevel::None, w, h, w, base.add(1024).cast());
        }
    }

    /// Page `p`'s CLUT (`glow`: its glow CLUT) through the palette grade, made when the grade
    /// moved since; `None` when the source is not held. Inside a display list: a copy written
    /// now is one no earlier command of this list read (one grade a frame).
    fn graded_clut(&mut self, p: u16, glow: bool, pal: &crate::list::Palette) -> Option<*const c_void> {
        let i = usize::from(p);
        // Made for this grade already: no copy of the source (a KB a bind, once).
        let held = if glow { self.pal_glow.get(i) } else { self.pal_pages.get(i) };
        if let Some(Some((g, b))) = held
            && *g == pal.generation
        {
            return Some(b.ptr.cast_const().cast());
        }
        let src: [u32; 256] = {
            let b = if glow {
                self.glow.get_mut(i)?.as_mut()?
            } else {
                self.ram[ram_ix(self.pack.pages.len(), p)].as_mut()?
            };
            let mut s = [0u32; 256];
            s.copy_from_slice(&b.words()[..256]);
            s
        };
        let night_page = self.pack.pages.get(i).is_some_and(|pg| pg.cat == crate::pack::NIGHT);
        let cache = if glow { &mut self.pal_glow } else { &mut self.pal_pages };
        if cache.len() <= i {
            cache.resize_with(i + 1, || None);
        }
        if cache[i].as_ref().is_none_or(|e| e.0 != pal.generation) {
            let mut b = match cache[i].take() {
                Some((_, b)) => b,
                None => Buf::new(1024)?,
            };
            for (o, c) in b.words()[..256].iter_mut().zip(src) {
                // A world page's colours take the night (NIGHT.md §4.2); its glow, and the
                // night kit's own page (drawn in the night's colours), do not.
                *o = if glow || night_page { pal.colour(c) } else { pal.world(c) };
            }
            // SAFETY: our buffer, read by a command after this.
            unsafe { wb_range(b.ptr.cast(), 1024) };
            cache[i] = Some((pal.generation, b));
        }
        cache[i].as_ref().map(|e| e.1.ptr.cast_const().cast())
    }

    /// Binds a chunk slot's texture, converting the frame's layer first when it is new (and its
    /// CLUT through the palette grade, `pal`). Inside a display list.
    fn bind_chunk(&mut self, frame: &Frame, slot: u16, generation: u32, pal: Option<&crate::list::Palette>) {
        let s = usize::from(slot);
        // A console presenter lays its albedo in the GE's order: drawn where it is, no copy (the
        // write-back at the start of the list covers a fresh paint).
        // A console presenter's chunk is `T8` over its own CLUT: drawn where it is, the CLUT
        // copied where the GE may load it (the write-back at the start of the list covers a
        // fresh paint).
        if let Some(l) = frame.layers.get(s).filter(|l| l.is_t8())
            && l.albedo.len() * 4 >= (CHUNK_PX * CHUNK_PX) as usize
        {
            if self.direct.len() <= s {
                self.direct.resize_with(s + 1, || None);
            }
            // Px not 16-byte aligned (the allocator's small pool) are copied where the GE may
            // read them.
            let aligned = (l.albedo.as_ptr() as usize) % 16 == 0;
            if self.direct_pal.len() <= s {
                self.direct_pal.resize(s + 1, None);
            }
            let want_pal = pal.map(|p| p.generation);
            if self.direct[s].as_ref().is_none_or(|d| d.0 != generation) || self.direct_pal[s] != want_pal {
                let (clut, px) = match self.direct[s].take() {
                    Some((_, c, p)) => (Some(c), p),
                    None => (Buf::new(1024), None),
                };
                let Some(mut clut) = clut else { return };
                match pal {
                    Some(p) => {
                        for (o, &c) in clut.words()[..256].iter_mut().zip(&l.clut[..256]) {
                            *o = p.world(c);
                        }
                    }
                    None => clut.words()[..256].copy_from_slice(&l.clut[..256]),
                }
                self.direct_pal[s] = want_pal;
                let px = if aligned {
                    None
                } else {
                    let Some(mut p) = px.or_else(|| Buf::new(l.albedo.len() * 4)) else { return };
                    p.words()[..l.albedo.len()].copy_from_slice(&l.albedo);
                    Some(p)
                };
                // SAFETY: our buffers; a command into the open list.
                unsafe {
                    wb_range(clut.ptr.cast(), 1024);
                    if let Some(p) = &px {
                        wb_range(p.ptr.cast(), p.len as u32);
                    }
                    sys::sceGuTexFlush();
                }
                self.stats.chunks_converted += 1;
                self.direct[s] = Some((generation, clut, px));
            }
            let Some((_, clut, px)) = self.direct[s].as_ref() else { return };
            let texels = px.as_ref().map_or(l.albedo.as_ptr().cast::<u8>(), |p| p.ptr.cast_const());
            // SAFETY: commands into the open list; the layer lives in the frame and the CLUT
            // here, neither written until the list has run (`draw` waits for it).
            unsafe {
                sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                clut_load(32, clut.ptr.cast());
                sys::sceGuTexImage(sys::MipmapLevel::None, CHUNK_PX, CHUNK_PX, CHUNK_PX, texels.cast());
            }
            return;
        }
        if self.chunks.len() <= s {
            self.chunks.resize_with(s + 1, || None);
        }
        let n = (CHUNK_PX * CHUNK_PX) as usize;
        let stale = self.chunks[s].as_ref().is_none_or(|c| c.0 != generation);
        if stale {
            if self.chunks[s].is_none() {
                let Some(b) = Buf::new(n * 4) else { return };
                self.chunks[s] = Some((generation, b));
            }
            let Some((g, b)) = self.chunks[s].as_mut() else { return };
            let Some(layer) = frame.layers.get(s) else { return };
            crate::chunk_abgr(&layer.albedo, b.words());
            *g = generation;
            self.stats.chunks_converted += 1;
            // SAFETY: the buffer is ours and `n * 4` bytes long; the GE reads it after this.
            unsafe {
                wb_range(b.ptr.cast(), (n * 4) as u32);
                sys::sceGuTexFlush();
            }
        }
        let Some((_, b)) = self.chunks[s].as_ref() else { return };
        // SAFETY: commands into the open list; the buffer lives until the slot is converted again,
        // after the list has run (`draw` waits for it).
        unsafe {
            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
            sys::sceGuTexImage(sys::MipmapLevel::None, CHUNK_PX, CHUNK_PX, CHUNK_PX, b.ptr.cast());
        }
    }

    /// Draws `lister`'s quads for `frame` (built from it), then `ui`'s (its `Ui` pass), and shows
    /// them at the next vblank. Pages are loaded through `load(offset, bytes)`: the pack file
    /// read at that offset.
    pub fn draw(
        &mut self,
        frame: &Frame,
        lister: &Lister,
        ui: &[crate::list::Quad],
        load: &mut dyn FnMut(u32, &mut [u8]) -> bool,
    ) {
        // The last frame: its list waited for and the frame shown, so its buffers are free to
        // write and its framebuffer is the one drawn into next.
        self.settle();
        self.present();
        let mut st = DrawStats { quads: (lister.quads.len() + ui.len()) as u32, ..DrawStats::default() };
        (st.sync_us, st.show_us) = (core::mem::take(&mut self.wait_sync), core::mem::take(&mut self.wait_show));
        (st.ge_error, st.evicts) = (core::mem::take(&mut self.ge_error), core::mem::take(&mut self.evicts));
        (st.wb, st.wb_bytes) = (WB.swap(0, Ordering::Relaxed), WB_BYTES.swap(0, Ordering::Relaxed));
        // The last list's signals: each one's time to the next is its pass's. Read a frame late,
        // as an emulator may call the signals' handler after the sync has returned.
        let mut passes = [0u32; crate::capture::pass::N];
        let mut ge_total = 0;
        let n = (SIG_N.load(Ordering::Acquire) as usize).min(SIGNALS);
        for k in 0..n.saturating_sub(1) {
            let id = SIG_ID[k].load(Ordering::Relaxed).wrapping_sub(0x10) as usize;
            let dt = SIG_T[k + 1].load(Ordering::Relaxed).wrapping_sub(SIG_T[k].load(Ordering::Relaxed));
            if let Some(p) = passes.get_mut(id) {
                *p += dt;
            }
            // A sub-pass's time is its parent's too (the parent holds the whole pass).
            if let Some(parent) = crate::capture::pass::parent(id as u8) {
                passes[usize::from(parent)] += dt;
            }
        }
        if n >= 2 {
            ge_total = SIG_T[n - 1].load(Ordering::Relaxed).wrapping_sub(SIG_T[0].load(Ordering::Relaxed));
        }
        // SAFETY: a plain syscall (the clock).
        let now = || unsafe { sys::sceKernelGetSystemTimeLow() };
        let t_prep = now();
        let (loads, uploads) = (self.stats.page_loads, self.slots.uploads);
        self.stats = DrawStats::default();
        self.lru.next_frame();
        self.slots.next_frame();
        // Every page this frame draws, held in RAM before the list starts (a run of one texture
        // asked once).
        let mut prev = None;
        for q in lister.quads.iter().chain(ui) {
            if prev == Some(q.tex) {
                continue;
            }
            prev = Some(q.tex);
            match q.tex {
                Tex::Page(p) | Tex::Glow(p) | Tex::Ink(p) => {
                    self.hold(p, load);
                }
                Tex::Normal(p, _) => {
                    self.hold(p | NORMAL, load);
                }
                _ => {}
            }
        }
        self.images(frame, ui);
        // The lamp cache's new pools into their VRAM slots, through the uncached mirror.
        for (slot, tex) in &lister.lamps.uploads {
            let at = 0x4400_0000 + LAMP_OFFSET + u32::from(*slot) * (crate::lamps::TEX * crate::lamps::TEX) as u32;
            // SAFETY: the lamp cache's VRAM, past the target; the last list has run.
            unsafe { core::ptr::copy_nonoverlapping(tex.as_ptr(), at as *mut u8, tex.len()) };
        }
        if let Some(b) = self.lamp_cluts.as_mut() {
            for (k, c) in lister.lamp_reliefs.iter().enumerate() {
                b.words()[k * 16..k * 16 + 16].copy_from_slice(c);
            }
        }
        // The grade's CLUTs, when they were rebuilt.
        if lister.cluts_gen != self.cluts_gen || self.cluts.is_none() {
            let bytes = lister.cluts.len() * 1024;
            if self.cluts.as_ref().is_none_or(|b| b.len < bytes) {
                self.cluts = Buf::new(bytes.max(3 * 1024));
            }
            if let Some(b) = self.cluts.as_mut() {
                for (k, c) in lister.cluts.iter().enumerate() {
                    b.words()[k * 256..k * 256 + 256].copy_from_slice(c);
                }
                self.cluts_gen = lister.cluts_gen;
            }
        }
        if let Some(b) = self.fog_clut.as_mut() {
            b.words()[..256].copy_from_slice(&lister.fog_clut);
        }
        if let Some(b) = self.noise_clut.as_mut() {
            b.words()[..256].copy_from_slice(&lister.noise_clut);
        }
        if let (Some(c), Some(b)) = (lister.relief, self.relief.as_mut()) {
            b.words()[..16].copy_from_slice(&c);
        }
        // The frame's patches into the GE's arena, once (the last list has run; the write-back
        // below covers them). No arena to be had: no patch is laid, the frame goes on.
        if !lister.patches.is_empty() {
            let used = lister.patch_px.len().min(PATCH_WORDS);
            // As the lister's: made as a frame needs it (doubling from 16 KB) and then kept.
            if self.patch_arena.as_ref().is_none_or(|a| a.len < used * 4) {
                let bytes = (used * 4).next_power_of_two().clamp(16 * 1024, PATCH_WORDS * 4);
                self.patch_arena = Buf::new(bytes);
            }
            if let Some(a) = self.patch_arena.as_mut() {
                a.words()[..used].copy_from_slice(&lister.patch_px[..used]);
            }
        }
        // What the list has room for, and what the frame must give up to fit it: the world's
        // share is the cap less the head, the tail and what the UI will take (a UI past a
        // quarter of the cap is cut by its own limit, not at the world's cost).
        let t_plan = now();
        let cap = self.list_cap.clamp(4 * LIST_TAIL, LIST_WORDS * 4);
        let ui_cost = guard::cost(ui, &[], &[], &[], &Shed::NONE, false, &mut self.sliced);
        let world_limit = cap - LIST_TAIL - ui_cost.min(cap / 4);
        let plan = guard::plan(
            &lister.quads,
            &lister.marks,
            &lister.polys,
            &lister.strips,
            self.timing,
            // The sub-passes' signals and the batches they end, when timed.
            world_limit - LIST_HEAD - if self.timing { lister.sub_marks.len() * SUB_MARK_BYTES } else { 0 },
            &mut self.sliced,
        );
        st.list_full = u32::from(plan.cut());
        st.list_shed = u32::from(plan.shed.level);
        let t_emit = now();
        let mut t_ui = t_emit;
        st.list_est = (LIST_HEAD + plan.bytes + ui_cost) as u32;
        // SAFETY: one display list, built and run here; vertices are taken from it.
        unsafe {
            wb_all();
            self.list_ix ^= 1;
            sys::sceGuStart(sys::GuContextType::Direct, list_ptr(self.list_ix));
            // The GE's texture cache keeps lines by address across lists: a UI image converted
            // again into the same buffer (`images`, the map's chart) or a chunk's height layer
            // landed again would otherwise sample last frame's texels. One flush a frame.
            sys::sceGuTexFlush();
            SIG_N.store(0, Ordering::Release);
            CLUT_LOADS.store(0, Ordering::Relaxed);
            CLUT_AT.store(0, Ordering::Relaxed);
            // The stall address moves at each batch again (`STALL_DEFER` off). Moving it only every
            // `STALL_STEP` bytes (Q6) hung the GE on the owner's PSP (bench at 758dabc, town at
            // night: the list sent, `sceGuSync` never back) and PPSSPP never showed it; it stays
            // off until it is proven on the hardware.
            sys::jane_gu_stall_defer(false);
            let mut stalled = sys::sceGuCheckList() as usize;
            if self.timing {
                signal(crate::capture::pass::CLEAR);
            }
            sys::sceGuClearColor(lister.clear);
            // The stencil (the alpha) cleared with the frame when the sun's pass takes it so: then
            // nothing before that pass writes the alpha (`rest_mask`), as its clear quad would
            // have set it all to 0 (PLAN-PSP-SPEED.md Q1).
            let mut rest_mask = 0u32;
            if lister.stencil_from_clear.is_some() {
                sys::sceGuClearStencil(0);
                sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::STENCIL_BUFFER_BIT);
                rest_mask = 0xff00_0000;
                sys::sceGuPixelMask(rest_mask);
            } else {
                sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT);
            }
            // The list guard's last word: a limit reached before a batch stopped a pass short.
            let mut cut = false;
            for (world, quads) in [(true, &lister.quads[..]), (false, ui)] {
                let mut i = 0;
                let mut bound: Option<Tex> = None;
                let mut mode: Option<Mode> = None;
                // What the plan left out of the world, and the list's room to this pass's end:
                // the world's, less the UI's share; the UI's, all but the tail.
                let (shed, limit) = if world { (plan.shed, world_limit) } else { (Shed::NONE, cap - LIST_TAIL) };
                let marks: &[(u32, u8)] = if world { &lister.marks } else { &[] };
                // A signal at each pass mark when timed (and a batch ends there; so it does where
                // a pass is left out).
                let timed = world && self.timing;
                let stops = timed || shed.any();
                let mut m = 0;
                let subs: &[(u32, u8)] = if timed { &lister.sub_marks } else { &[] };
                let mut sm = 0;
                // Inside the lightmap's target: it is left whatever stops the pass.
                let mut in_rt = false;
                // This batch is the last: a run of quads drawn in part, the room used.
                let mut last = false;
                if !world && self.timing && !quads.is_empty() {
                    signal(crate::capture::pass::UI);
                }
                while i < quads.len() {
                    i = guard::first_kept(quads, marks, &shed, i);
                    if i >= quads.len() {
                        break;
                    }
                    // From the sun's pass on, the alpha is written again.
                    if world && rest_mask != 0 && lister.stencil_from_clear.is_some_and(|at| i >= at as usize) {
                        rest_mask = 0;
                        sys::sceGuPixelMask(0);
                    }
                    if timed {
                        while m < marks.len() && marks[m].0 as usize <= i {
                            signal(marks[m].1);
                            m += 1;
                        }
                        while sm < subs.len() && subs[sm].0 as usize <= i {
                            signal(subs[sm].1);
                            sm += 1;
                        }
                    }
                    let q = quads[i];
                    // The list's bytes so far: no batch is begun that could not be finished
                    // inside the limit (`guard`'s bounds), whatever the plan said.
                    let used = sys::sceGuCheckList() as usize;
                    // Into the lightmap's target and back: commands, not quads.
                    if q.mode == Mode::RtBegin {
                        if used + guard::RT_BEGIN + guard::RT_END > limit {
                            cut = true;
                            break;
                        }
                        in_rt = true;
                        st.rts += 1;
                        sys::sceGuDrawBufferList(
                            sys::DisplayPixelFormat::Psm8888,
                            RT_OFFSET as *mut c_void,
                            crate::light::SIDE as i32,
                        );
                        sys::sceGuScissor(0, 0, i32::from(q.x1), i32::from(q.y1));
                        // Cleared by a flat rect, colour and stencil (alpha) written as they are
                        // (`sceGuClear` covers the screen's size, past this small target).
                        sys::sceGuDisable(GuState::Blend);
                        sys::sceGuDisable(GuState::AlphaTest);
                        sys::sceGuDisable(GuState::Texture2D);
                        let v =
                            sys::sceGuGetMemory((2 * core::mem::size_of::<FlatVertex>()) as i32).cast::<FlatVertex>();
                        let c = q.colour & 0x00ff_ffff;
                        put_flat(v, c, (0, 0));
                        put_flat(v.add(1), c, (q.x1, q.y1));
                        sys::sceGuDrawArray(
                            GuPrimitive::Sprites,
                            VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                            2,
                            core::ptr::null(),
                            v.cast(),
                        );
                        sys::sceGuEnable(GuState::Blend);
                        sys::sceGuEnable(GuState::AlphaTest);
                        mode = None;
                        bound = None;
                        i += 1;
                        continue;
                    }
                    if q.mode == Mode::RtEnd {
                        end_rt(self.back);
                        in_rt = false;
                        mode = None;
                        bound = None;
                        i += 1;
                        continue;
                    }
                    // A batch: the quads after it with its texture and mode (the polygons all one).
                    let mut stop = if stops { guard::stop_at(marks, &shed, i) } else { usize::MAX };
                    if let Some(s) = subs.get(sm) {
                        stop = stop.min(s.0 as usize);
                    }
                    let mut j = guard::batch_end(quads, i, stop);
                    // The room it needs at most (inside the target, also what leaving it takes).
                    let keep = if in_rt { guard::RT_END } else { 0 };
                    let fixed = guard::BIND + guard::MODE + guard::DRAW + keep;
                    let need = fixed + guard::payload(&quads[i..j], &lister.polys, &lister.strips, &mut self.sliced);
                    if used + need > limit {
                        // Part of a plain run of quads may still fit; anything else stops here.
                        let per = match q.tex {
                            Tex::None | Tex::Line(_) => 2 * guard::FLAT,
                            Tex::Poly(_) | Tex::Strip(_) | Tex::Frame(..) | Tex::LightRt => 0,
                            _ => 2 * guard::TEXTURED,
                        };
                        if per == 0 || used + fixed + per > limit {
                            cut = true;
                            break;
                        }
                        j = i + (limit - used - fixed) / per;
                        last = true;
                    }
                    // What the texture state is: every untextured kind one (a flat quad, a
                    // polygon, a line, a flat strip: the texture off), so a stencil clear and the
                    // polygons marked after it take one bind, not two.
                    let key = match guard::batch_key(q.tex) {
                        Tex::Poly(_) | Tex::Line(_) => Tex::None,
                        Tex::Strip(k)
                            if lister.strips.get(usize::from(k)).map(|s| s.tex)
                                == Some(crate::list::StripTex::Flat) =>
                        {
                            Tex::None
                        }
                        t => t,
                    };
                    if bound != Some(key) {
                        st.binds += 1;
                        // The last texture's filter and wrap undone first, so this one's own stand.
                        if bound == Some(Tex::Noise)
                            || matches!(bound, Some(Tex::Strip(k))
                                if lister.strips.get(usize::from(k)).is_some_and(|s| s.tex == crate::list::StripTex::Mist))
                        {
                            sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
                        }
                        if matches!(bound, Some(Tex::Disc | Tex::Spot | Tex::Pool | Tex::LightRt | Tex::LampTex(_))) {
                            sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
                        }
                        match q.tex {
                            Tex::None | Tex::Poly(_) | Tex::Line(_) => sys::sceGuDisable(GuState::Texture2D),
                            Tex::Noise => {
                                let (Some(n), Some(c)) = (self.noise.as_ref(), self.noise_clut.as_ref()) else {
                                    i = j;
                                    continue;
                                };
                                let side = crate::water::NOISE as i32;
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                clut_load(32, c.ptr.cast());
                                sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, n.ptr.cast());
                                sys::sceGuTexWrap(sys::GuTexWrapMode::Repeat, sys::GuTexWrapMode::Repeat);
                            }
                            Tex::Strip(k) => match lister.strips.get(usize::from(k)).map(|s| s.tex) {
                                Some(crate::list::StripTex::Flat) => sys::sceGuDisable(GuState::Texture2D),
                                Some(crate::list::StripTex::Mist) => {
                                    let (Some(m), Some(c)) = (self.mist.as_ref(), self.fog_clut.as_ref()) else {
                                        i = j;
                                        continue;
                                    };
                                    let side = MIST_SIDE as i32;
                                    sys::sceGuEnable(GuState::Texture2D);
                                    sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                                    sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                    clut_load(32, c.ptr.cast());
                                    sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, m.ptr.cast());
                                    sys::sceGuTexWrap(sys::GuTexWrapMode::Repeat, sys::GuTexWrapMode::Repeat);
                                }
                                None => {
                                    i = j;
                                    continue;
                                }
                            },
                            Tex::Frame(c, k) => {
                                // The frame drawn so far, as `T32` through CLUT `k`, channel `c`.
                                let Some(cb) = self.cluts.as_ref().filter(|b| (usize::from(k) + 1) * 1024 <= b.len)
                                else {
                                    i = j;
                                    continue;
                                };
                                let fb = 0x0400_0000 + if self.back == 0 { 0 } else { FB_BYTES };
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::PsmT32, 0, 0, 0);
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 8 * u32::from(c), 0xff, 0);
                                clut_load(32, cb.ptr.add(usize::from(k) * 1024).cast());
                                sys::sceGuTexImage(sys::MipmapLevel::None, BUF_W, BUF_W, BUF_W, fb as *const c_void);
                                sys::sceGuTexFlush();
                                sys::sceGuTexSync();
                            }
                            Tex::Page(p) => {
                                if self.ram[ram_ix(self.pack.pages.len(), p)].is_none() {
                                    i = j;
                                    continue;
                                }
                                sys::sceGuEnable(GuState::Texture2D);
                                // The world's pages through the palette grade (never the UI's).
                                let graded = lister
                                    .palette
                                    .as_ref()
                                    .filter(|_| world)
                                    .and_then(|pal| self.graded_clut(p, false, pal));
                                self.bind_page(p, graded);
                            }
                            Tex::Patch(k) => {
                                // In the arena the frame's patches were copied to: its rows
                                // `stride` words apart (a multiple of 4), 16-byte aligned. No
                                // arena (the heap had none): no patch.
                                let (Some(p), Some(arena)) =
                                    (lister.patches.get(usize::from(k)), self.patch_arena.as_ref())
                                else {
                                    i = j;
                                    continue;
                                };
                                if (p.at as usize + usize::from(p.stride) * usize::from(p.h)) * 4 > arena.len {
                                    i = j;
                                    continue;
                                }
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                                let (tw, th) = (i32::from(p.tw), i32::from(p.th));
                                let at = arena.ptr.add(p.at as usize * 4);
                                sys::sceGuTexImage(sys::MipmapLevel::None, tw, th, i32::from(p.stride), at.cast());
                            }
                            Tex::Ink(p) => {
                                let Some(white) = self.white.as_ref().map(|b| b.ptr.cast_const()) else {
                                    i = j;
                                    continue;
                                };
                                if self.ram[ram_ix(self.pack.pages.len(), p)].is_none() {
                                    i = j;
                                    continue;
                                }
                                sys::sceGuEnable(GuState::Texture2D);
                                // Through the white CLUT, not its own: the ink's colour.
                                self.bind_page(p, Some(white.cast()));
                            }
                            Tex::Image(slot) => {
                                let Some(Some((_, w, h, b))) = self.images.get(usize::from(slot)) else {
                                    i = j;
                                    continue;
                                };
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                                let (tw, th) = (pow2(*w), pow2(*h));
                                sys::sceGuTexImage(sys::MipmapLevel::None, tw, th, i32::from(*w), b.ptr.cast());
                            }
                            Tex::Glow(p) => {
                                if self.ram[ram_ix(self.pack.pages.len(), p)].is_none() || !self.hold_glow(p, load) {
                                    i = j;
                                    continue;
                                }
                                sys::sceGuEnable(GuState::Texture2D);
                                // Through its glow CLUT (graded), not its own.
                                let graded = lister.palette.as_ref().and_then(|pal| self.graded_clut(p, true, pal));
                                let glow = graded.or_else(|| {
                                    self.glow
                                        .get(usize::from(p))
                                        .and_then(Option::as_ref)
                                        .map(|g| g.ptr.cast_const().cast())
                                });
                                self.bind_page(p, glow);
                            }
                            Tex::Normal(p, c) => {
                                let cb = if c == u16::MAX {
                                    self.relief.as_ref().map(|b| b.ptr.cast_const())
                                } else {
                                    self.lamp_cluts.as_ref().map(|b| b.ptr.add(usize::from(c) * 64).cast_const())
                                };
                                let (Some(nb), Some(cb)) =
                                    (self.ram[ram_ix(self.pack.pages.len(), p | NORMAL)].as_ref(), cb)
                                else {
                                    i = j;
                                    continue;
                                };
                                let info = self.pack.pages[usize::from(p)];
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::PsmT4, 0, 0, 1);
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                clut_load(2, cb.cast());
                                let (w, h) = (i32::from(info.w), i32::from(info.h));
                                sys::sceGuTexImage(sys::MipmapLevel::None, w, h, w, nb.ptr.cast());
                            }
                            Tex::Height(slot) => {
                                let Some(l) = frame.layers.get(usize::from(slot)).filter(|l| l.has_height()) else {
                                    i = j;
                                    continue;
                                };
                                let (Some(m), true) =
                                    (self.height_mask.as_ref(), (l.height.as_ptr() as usize) % 16 == 0)
                                else {
                                    i = j;
                                    continue;
                                };
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                clut_load(32, m.ptr.cast());
                                sys::sceGuTexImage(
                                    sys::MipmapLevel::None,
                                    CHUNK_PX,
                                    CHUNK_PX,
                                    CHUNK_PX,
                                    l.height.as_ptr().cast(),
                                );
                            }
                            Tex::Pool => {
                                let Some(b) = self.pool.as_ref() else {
                                    i = j;
                                    continue;
                                };
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                                let side = crate::light::POOL as i32;
                                sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, b.ptr.cast());
                                sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                            }
                            Tex::LampTex(slot) => {
                                let Some(g) = self.grey.as_ref() else {
                                    i = j;
                                    continue;
                                };
                                let side = crate::lamps::TEX as i32;
                                let at = (0x0400_0000 + LAMP_OFFSET + u32::from(slot) * (side * side) as u32)
                                    as *const c_void;
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                clut_load(32, g.ptr.cast());
                                sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, at);
                                sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                                sys::sceGuTexFlush();
                            }
                            Tex::LightRt => {
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                                let rt = (0x0400_0000 + RT_OFFSET) as *const c_void;
                                // Declared square (a power of two); only the rows drawn are read.
                                let side = crate::light::SIDE as i32;
                                sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, rt);
                                sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                            }
                            Tex::Disc | Tex::Spot => {
                                let Some(b) =
                                    (if q.tex == Tex::Disc { self.disc.as_ref() } else { self.spot.as_ref() })
                                else {
                                    i = j;
                                    continue;
                                };
                                sys::sceGuEnable(GuState::Texture2D);
                                sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                                let side = crate::light::DISC as i32;
                                sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, b.ptr.cast());
                                sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                            }
                            Tex::Chunk(slot) => {
                                let generation = lister.chunks.iter().find(|c| c.0 == slot).map_or(0, |c| c.1);
                                sys::sceGuEnable(GuState::Texture2D);
                                self.bind_chunk(frame, slot, generation, lister.palette.as_ref());
                            }
                        }
                        bound = Some(key);
                    }
                    if mode != Some(q.mode) {
                        st.modes += 1;
                        let stencil = |m: Mode| {
                            matches!(
                                m,
                                Mode::StencilClear
                                    | Mode::StencilMark
                                    | Mode::RaisedMark
                                    | Mode::ShadowBand
                                    | Mode::PoolLit
                                    | Mode::PoolShade
                                    | Mode::Mark { .. }
                                    | Mode::Masked(..)
                            )
                        };
                        if matches!(mode, Some(Mode::Mark { tag, .. }) if tag != 0) {
                            sys::sceGuAlphaFunc(sys::AlphaFunc::Greater, 0, 0xff);
                        }
                        st.stencil += u32::from(stencil(q.mode));
                        if mode.is_some_and(stencil) && !stencil(q.mode) {
                            sys::sceGuDisable(GuState::StencilTest);
                            sys::sceGuPixelMask(rest_mask);
                        }
                        if matches!(mode, Some(Mode::Lut(_))) && !matches!(q.mode, Mode::Lut(_)) {
                            sys::sceGuPixelMask(rest_mask);
                            sys::sceGuEnable(GuState::Blend);
                        }
                        match q.mode {
                            // The stencil is the framebuffer's alpha: colour masked, alpha written.
                            // Source and destination both whole: what glows added.
                            Mode::AddGlow => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Fix,
                                    sys::BlendFactor::Fix,
                                    0x00ff_ffff,
                                    0x00ff_ffff,
                                );
                            }
                            // Source factor 1 is one less the destination's colour.
                            Mode::PoolLit | Mode::PoolShade => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0);
                                let f = if q.mode == Mode::PoolLit {
                                    sys::StencilFunc::NotEqual
                                } else {
                                    sys::StencilFunc::Equal
                                };
                                sys::sceGuStencilFunc(f, 1, 0xff);
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                );
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::SrcAlpha,
                                    sys::BlendFactor::Fix,
                                    0,
                                    0x00ff_ffff,
                                );
                            }
                            Mode::RtBegin | Mode::RtEnd => {}
                            // The texel's colour alone: a console chunk's CLUT alpha is its marks.
                            Mode::Opaque => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgb);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::SrcAlpha,
                                    sys::BlendFactor::OneMinusSrcAlpha,
                                    0,
                                    0,
                                );
                            }
                            // The stencil's bits outside `keep` from `value`, where the texel's alpha
                            // is `tag` and (with `need`) the stencil has one of its bits.
                            Mode::Mark { tag, value, keep, need } => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0x00ff_ffff | u32::from(keep) << 24);
                                sys::sceGuTexFunc(sys::TextureEffect::Replace, sys::TextureColorComponent::Rgba);
                                if tag != 0 {
                                    sys::sceGuAlphaFunc(sys::AlphaFunc::Equal, i32::from(tag), 0xff);
                                }
                                match need {
                                    Some(n) => sys::sceGuStencilFunc(
                                        sys::StencilFunc::NotEqual,
                                        i32::from(value),
                                        i32::from(n),
                                    ),
                                    None => sys::sceGuStencilFunc(sys::StencilFunc::Always, i32::from(value), 0xff),
                                }
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Replace,
                                );
                            }
                            Mode::Masked(blend, at) => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0);
                                match at {
                                    crate::list::Where::Is(v) => {
                                        sys::sceGuStencilFunc(sys::StencilFunc::Equal, i32::from(v), 0xff);
                                    }
                                    crate::list::Where::Bits(v, m) => {
                                        sys::sceGuStencilFunc(sys::StencilFunc::Equal, i32::from(v), i32::from(m));
                                    }
                                    // `ref < stencil`: the stencil over `v`.
                                    crate::list::Where::Over(v) => {
                                        sys::sceGuStencilFunc(sys::StencilFunc::Less, i32::from(v), 0xff);
                                    }
                                }
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                );
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                match blend {
                                    crate::list::Blend::Alpha => sys::sceGuBlendFunc(
                                        sys::BlendOp::Add,
                                        sys::BlendFactor::SrcAlpha,
                                        sys::BlendFactor::OneMinusSrcAlpha,
                                        0,
                                        0,
                                    ),
                                    crate::list::Blend::Multiply => sys::sceGuBlendFunc(
                                        sys::BlendOp::Add,
                                        sys::BlendFactor::Color,
                                        sys::BlendFactor::Fix,
                                        0,
                                        0,
                                    ),
                                    crate::list::Blend::Glow => {
                                        sys::sceGuTexFunc(
                                            sys::TextureEffect::Modulate,
                                            sys::TextureColorComponent::Rgb,
                                        );
                                        sys::sceGuBlendFunc(
                                            sys::BlendOp::Add,
                                            sys::BlendFactor::Fix,
                                            sys::BlendFactor::Fix,
                                            0x00ff_ffff,
                                            0x00ff_ffff,
                                        );
                                    }
                                    crate::list::Blend::Add => sys::sceGuBlendFunc(
                                        sys::BlendOp::Add,
                                        sys::BlendFactor::SrcAlpha,
                                        sys::BlendFactor::Fix,
                                        0,
                                        0x00ff_ffff,
                                    ),
                                }
                            }
                            // Its channel alone written, the texel as it is.
                            Mode::Lut(c) => {
                                sys::sceGuDisable(GuState::Blend);
                                sys::sceGuTexFunc(sys::TextureEffect::Replace, sys::TextureColorComponent::Rgba);
                                sys::sceGuPixelMask(!(0xffu32 << (8 * u32::from(c))));
                            }
                            // The texel times the colour, plus what is there times one less it.
                            Mode::Desaturate => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgb);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Fix,
                                    sys::BlendFactor::Fix,
                                    0x00ff_ffff,
                                    0x00ff_ffff - (q.colour & 0x00ff_ffff),
                                );
                            }
                            Mode::Subtract => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgb);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::ReverseSubtract,
                                    sys::BlendFactor::Fix,
                                    sys::BlendFactor::Fix,
                                    0x00ff_ffff,
                                    0x00ff_ffff,
                                );
                            }
                            Mode::Halo => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::SrcAlpha,
                                    sys::BlendFactor::Fix,
                                    0,
                                    0x00ff_ffff,
                                );
                            }
                            Mode::Lift => {
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::OneMinusColor,
                                    sys::BlendFactor::Fix,
                                    0,
                                    0x00ff_ffff,
                                );
                            }
                            Mode::StencilClear => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0x00ff_ffff);
                                sys::sceGuStencilFunc(sys::StencilFunc::Always, 0, 0xff);
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Replace,
                                    sys::StencilOperation::Replace,
                                    sys::StencilOperation::Replace,
                                );
                            }
                            Mode::StencilMark => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0x00ff_ffff);
                                sys::sceGuTexFunc(sys::TextureEffect::Replace, sys::TextureColorComponent::Rgba);
                                sys::sceGuStencilFunc(sys::StencilFunc::Always, 1, 0xff);
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Replace,
                                );
                            }
                            Mode::RaisedMark => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0x00ff_ffff);
                                sys::sceGuTexFunc(sys::TextureEffect::Replace, sys::TextureColorComponent::Rgba);
                                sys::sceGuStencilFunc(sys::StencilFunc::Always, i32::from(crate::list::RAISED), 0xff);
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Replace,
                                );
                            }
                            // Where the stencil is clear (neither shaded yet nor raised): shaded,
                            // and set to 1.
                            Mode::ShadowBand => {
                                sys::sceGuEnable(GuState::StencilTest);
                                sys::sceGuPixelMask(0);
                                // `1 > stencil`: the stencil clear; the op writes the ref, 1.
                                sys::sceGuStencilFunc(sys::StencilFunc::Greater, 1, 0xff);
                                sys::sceGuStencilOp(
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Keep,
                                    sys::StencilOperation::Replace,
                                );
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Color,
                                    sys::BlendFactor::Fix,
                                    0,
                                    0,
                                );
                            }
                            Mode::Alpha => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::SrcAlpha,
                                    sys::BlendFactor::OneMinusSrcAlpha,
                                    0,
                                    0,
                                );
                            }
                            Mode::Add => {
                                sys::sceGuTexFunc(sys::TextureEffect::Add, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::SrcAlpha,
                                    sys::BlendFactor::OneMinusSrcAlpha,
                                    0,
                                    0,
                                );
                            }
                            // Source factor 0 is the destination's colour, the destination's a
                            // fixed 0: what is there, times the quad's colour.
                            // Both factors the other's colour: `src * dst + dst * src`, twice the
                            // multiply, as the lightmap holds half the light.
                            Mode::Multiply2Opaque => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgb);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Color,
                                    sys::BlendFactor::Color,
                                    0,
                                    0,
                                );
                            }
                            Mode::Multiply2 => {
                                sys::sceGuTexFunc(sys::TextureEffect::Modulate, sys::TextureColorComponent::Rgba);
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Color,
                                    sys::BlendFactor::Color,
                                    0,
                                    0,
                                );
                            }
                            Mode::Multiply => {
                                sys::sceGuBlendFunc(
                                    sys::BlendOp::Add,
                                    sys::BlendFactor::Color,
                                    sys::BlendFactor::Fix,
                                    0,
                                    0,
                                );
                            }
                        }
                        mode = Some(q.mode);
                    }
                    let n = j - i;
                    if let Tex::Strip(k) = q.tex {
                        // One strip, its vertices' colours run smooth.
                        if let Some(st) =
                            lister.strips.get(usize::from(k)).filter(|s| s.tex == crate::list::StripTex::Mist)
                        {
                            let vs = &lister.verts[st.start as usize..(st.start as usize + usize::from(st.len))];
                            let v = sys::sceGuGetMemory((vs.len() * core::mem::size_of::<TexVertex>()) as i32)
                                .cast::<TexVertex>();
                            for (k, p) in vs.iter().enumerate() {
                                put_tex(v.add(k), (p.u, p.v), p.colour, (p.x, p.y));
                            }
                            sys::sceGuDrawArray(
                                GuPrimitive::TriangleStrip,
                                VertexType::TEXTURE_16BIT
                                    | VertexType::COLOR_8888
                                    | VertexType::VERTEX_16BIT
                                    | VertexType::TRANSFORM_2D,
                                vs.len() as i32,
                                core::ptr::null(),
                                v.cast(),
                            );
                        } else if let Some(st) = lister.strips.get(usize::from(k)) {
                            let vs = &lister.verts[st.start as usize..(st.start as usize + usize::from(st.len))];
                            let v = sys::sceGuGetMemory((vs.len() * core::mem::size_of::<FlatVertex>()) as i32)
                                .cast::<FlatVertex>();
                            for (k, p) in vs.iter().enumerate() {
                                put_flat(v.add(k), p.colour, (p.x, p.y));
                            }
                            sys::sceGuDrawArray(
                                GuPrimitive::TriangleStrip,
                                VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                                vs.len() as i32,
                                core::ptr::null(),
                                v.cast(),
                            );
                        }
                    } else if let Tex::Line(fade) = q.tex {
                        let v = sys::sceGuGetMemory((n * 2 * core::mem::size_of::<FlatVertex>()) as i32)
                            .cast::<FlatVertex>();
                        for (k, q) in quads[i..j].iter().enumerate() {
                            let tail = if fade { q.colour & 0x00ff_ffff } else { q.colour };
                            put_flat(v.add(2 * k), q.colour, (q.x0, q.y0));
                            put_flat(v.add(2 * k + 1), tail, (q.x1, q.y1));
                        }
                        sys::sceGuDrawArray(
                            GuPrimitive::Lines,
                            VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                            (2 * n) as i32,
                            core::ptr::null(),
                            v.cast(),
                        );
                    } else if let Tex::Poly(_) = q.tex {
                        // The batch's polygons as triangles, one block of flat vertices and one
                        // draw a few thousand vertices (the draw's count is 16 bits): each fan,
                        // its first corner and each pair after, the same triangles in the same
                        // order as a fan drawn alone. One draw each cost 2 syscalls and 24 bytes.
                        let mut k = i;
                        while k < j {
                            let (mut e, mut verts) = (k, 0usize);
                            while e < j {
                                let nv = guard::poly_verts(&quads[e], &lister.polys);
                                if verts > 0 && verts + nv > guard::POLY_VERTS {
                                    break;
                                }
                                verts += nv;
                                e += 1;
                            }
                            if verts > 0 {
                                let v = sys::sceGuGetMemory((verts * core::mem::size_of::<FlatVertex>()) as i32)
                                    .cast::<FlatVertex>();
                                let mut w = 0;
                                for pq in &quads[k..e] {
                                    let Tex::Poly(pi) = pq.tex else { continue };
                                    let Some((pts, n)) = lister.polys.get(usize::from(pi)) else { continue };
                                    let n = usize::from(*n).min(pts.len());
                                    if n < 3 {
                                        continue;
                                    }
                                    for t in 1..n - 1 {
                                        for (x, y) in [pts[0], pts[t], pts[t + 1]] {
                                            put_flat(v.add(w), pq.colour, (x, y));
                                            w += 1;
                                        }
                                    }
                                }
                                sys::sceGuDrawArray(
                                    GuPrimitive::Triangles,
                                    VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                                    w as i32,
                                    core::ptr::null(),
                                    v.cast(),
                                );
                            }
                            k = e;
                        }
                    } else if q.tex == Tex::None {
                        let v = sys::sceGuGetMemory((n * 2 * core::mem::size_of::<FlatVertex>()) as i32)
                            .cast::<FlatVertex>();
                        for (k, q) in quads[i..j].iter().enumerate() {
                            put_flat(v.add(2 * k), q.colour, (q.x0, q.y0));
                            put_flat(v.add(2 * k + 1), q.colour, (q.x1, q.y1));
                        }
                        sys::sceGuDrawArray(
                            GuPrimitive::Sprites,
                            VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                            (2 * n) as i32,
                            core::ptr::null(),
                            v.cast(),
                        );
                    } else if matches!(q.tex, Tex::Frame(..) | Tex::LightRt) {
                        // A read-back of the frame or the lightmap: in slices the texture cache
                        // holds (`list::slices`).
                        for q in &quads[i..j] {
                            crate::list::slices(q, &mut self.sliced);
                            let m = self.sliced.len();
                            let v = sys::sceGuGetMemory((m * 2 * core::mem::size_of::<TexVertex>()) as i32)
                                .cast::<TexVertex>();
                            for (k, s) in self.sliced.iter().enumerate() {
                                put_tex(v.add(2 * k), (s.u0, s.v0), s.colour, (s.x0, s.y0));
                                put_tex(v.add(2 * k + 1), (s.u1, s.v1), s.colour, (s.x1, s.y1));
                            }
                            sys::sceGuDrawArray(
                                GuPrimitive::Sprites,
                                VertexType::TEXTURE_16BIT
                                    | VertexType::COLOR_8888
                                    | VertexType::VERTEX_16BIT
                                    | VertexType::TRANSFORM_2D,
                                (2 * m) as i32,
                                core::ptr::null(),
                                v.cast(),
                            );
                        }
                    } else {
                        let v =
                            sys::sceGuGetMemory((n * 2 * core::mem::size_of::<TexVertex>()) as i32).cast::<TexVertex>();
                        for (k, q) in quads[i..j].iter().enumerate() {
                            put_tex(v.add(2 * k), (q.u0, q.v0), q.colour, (q.x0, q.y0));
                            put_tex(v.add(2 * k + 1), (q.u1, q.v1), q.colour, (q.x1, q.y1));
                        }
                        sys::sceGuDrawArray(
                            GuPrimitive::Sprites,
                            VertexType::TEXTURE_16BIT
                                | VertexType::COLOR_8888
                                | VertexType::VERTEX_16BIT
                                | VertexType::TRANSFORM_2D,
                            (2 * n) as i32,
                            core::ptr::null(),
                            v.cast(),
                        );
                    }
                    st.batches += 1;
                    let at = sys::sceGuCheckList() as usize;
                    if at >= stalled + STALL_STEP {
                        sys::jane_gu_stall_flush();
                        stalled = at;
                    }
                    i = j;
                    if last {
                        cut = true;
                        break;
                    }
                }
                // A pass stopped inside the lightmap's target comes out of it.
                if in_rt {
                    end_rt(self.back);
                }
                // Between the world's quads and the UI's: nearest, clamped, every channel blended.
                let _ = bound;
                sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
                sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
                sys::sceGuDisable(GuState::StencilTest);
                sys::sceGuPixelMask(0);
                rest_mask = 0;
                sys::sceGuEnable(GuState::Blend);
                sys::sceGuAlphaFunc(sys::AlphaFunc::Greater, 0, 0xff);
                if world {
                    t_ui = now();
                }
            }
            if cut {
                st.list_full = 1;
                st.list_shed += 100;
            }
            // The GE's state outlives the list: every channel written and blended again, so the
            // next frame's clear and quads are whole.
            sys::sceGuPixelMask(0);
            sys::sceGuEnable(GuState::Blend);
            sys::sceGuDisable(GuState::StencilTest);
            sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
            if self.timing {
                signal(SIG_END);
            }
            st.list_bytes = sys::sceGuFinish() as u32;
        }
        let t_end = now();
        for (p, (a, b)) in
            self.parts.iter_mut().zip([(t_prep, t_plan), (t_plan, t_emit), (t_emit, t_ui), (t_ui, t_end)])
        {
            *p = p.wrapping_add(b.wrapping_sub(a));
        }
        self.pending = true;
        self.unshown = true;
        if !self.pipelined {
            self.settle();
            st.sync_us += core::mem::take(&mut self.wait_sync);
        }
        st.clut_loads = CLUT_LOADS.load(Ordering::Relaxed);
        (st.passes, st.ge_total) = (passes, ge_total);
        st.page_loads = self.stats.page_loads;
        st.page_load_fails = self.stats.page_load_fails;
        st.uploads = self.slots.uploads - uploads;
        st.chunks_converted = self.stats.chunks_converted;
        st.ram_pages_bytes = self.lru.bytes();
        st.frame_page_bytes = self.lru.frame_bytes();
        st.chunk_bytes = self.chunks.iter().flatten().map(|c| c.1.len as u32).sum();
        let _ = loads;
        self.stats = st;
    }

    /// Waits for the list sent last, if it has not been (its time to `DrawStats::sync_us`), and
    /// frees what it may have read.
    pub fn settle(&mut self) {
        if self.pending {
            // SAFETY: plain syscalls.
            unsafe {
                let t = sys::sceKernelGetSystemTimeLow();
                let r = sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait);
                let r = r as i32;
                if r != 0 {
                    self.ge_error = r as u32;
                }
                self.wait_sync += sys::sceKernelGetSystemTimeLow().wrapping_sub(t);
            }
            self.pending = false;
        }
        self.grave.clear();
    }

    /// Shows the frame drawn last, if it is not shown yet, by [`Ge::pacing`] (its wait to
    /// `DrawStats::show_us`). Its list must have run ([`Ge::settle`]). The vblanks since the
    /// flip before are read off the display's count and kept ([`Ge::take_flip_vblanks`]); the
    /// first flip has none before it to measure from.
    pub fn present(&mut self) {
        if !self.unshown {
            return;
        }
        // SAFETY: plain syscalls; the list has run.
        unsafe {
            let t = sys::sceKernelGetSystemTimeLow();
            match self.pacing {
                Pacing::Vsync => {
                    sys::sceDisplayWaitVblankStart();
                }
                Pacing::Locked30 => loop {
                    sys::sceDisplayWaitVblankStart();
                    if sys::sceDisplayGetVcount().wrapping_sub(self.last_vcount) >= 2 {
                        break;
                    }
                },
                Pacing::Unlocked => {}
            }
            sys::sceGuSwapBuffers();
            let v = sys::sceDisplayGetVcount();
            // A sleep or a long build leaves a gap of thousands: kept to a byte.
            let took = v.wrapping_sub(self.last_vcount).min(255);
            self.last_vcount = v;
            if self.flips > 0 {
                self.flip_vblanks = self.flip_vblanks.saturating_add(took);
                let target = self.flip_target();
                if target > 0 {
                    self.missed_vblanks = self.missed_vblanks.wrapping_add(took.saturating_sub(target));
                }
            }
            self.flips = self.flips.wrapping_add(1);
            self.wait_show += sys::sceKernelGetSystemTimeLow().wrapping_sub(t);
        }
        self.back ^= 1;
        self.unshown = false;
    }

    /// The vblanks a flip should take under [`Ge::pacing`]: 2 at a locked 30, 1 at 60, 0 when
    /// unlocked (no target: nothing is missed).
    pub fn flip_target(&self) -> u32 {
        match self.pacing {
            Pacing::Locked30 => 2,
            Pacing::Vsync => 1,
            Pacing::Unlocked => 0,
        }
    }

    /// The vblanks the flips took since the last call, and reset: the display's own count of the
    /// time to step the sim by (a tick a vblank), and what a capture records. 0 when nothing was
    /// shown; each flip's is at most 255.
    pub fn take_flip_vblanks(&mut self) -> u32 {
        core::mem::take(&mut self.flip_vblanks)
    }

    /// The frame drawn last waited for and shown now (a screenshot, leaving play, a sleep).
    pub fn flush(&mut self) {
        self.settle();
        self.present();
    }

    /// The UI images `ui` draws converted for the GE when their generation moved (`0xAARRGGBB`
    /// to `0xAABBGGRR`, rows padded to a multiple of 4 px); a slot the frame no longer holds is
    /// let go.
    fn images(&mut self, frame: &Frame, ui: &[crate::list::Quad]) {
        // A slot the frame no longer holds (or holds empty, 0 x 0) is let go; one whose px the
        // game has let go after they were converted (the console keeps one copy) is kept.
        for (k, e) in self.images.iter_mut().enumerate() {
            if frame.ui_images.get(k).is_none_or(|im| im.w == 0 || im.h == 0) {
                *e = None;
            }
        }
        for q in ui {
            let Tex::Image(slot) = q.tex else { continue };
            let s = usize::from(slot);
            let Some(im) = frame.ui_images.get(s).filter(|im| im.w > 0 && im.h > 0) else { continue };
            if self.images.len() <= s {
                self.images.resize_with(s + 1, || None);
            }
            let (w, h) = (im.w.next_multiple_of(4), im.h);
            if self.images[s].as_ref().is_some_and(|e| e.0 == im.generation && (e.1, e.2) == (w, h)) {
                continue;
            }
            if im.argb.len() < usize::from(im.w) * usize::from(im.h) {
                continue;
            }
            let n = usize::from(w) * usize::from(h);
            let buf = match self.images[s].take() {
                Some(e) if e.3.len >= n * 4 => Some(e.3),
                _ => Buf::new(n * 4),
            };
            let Some(mut b) = buf else { continue };
            let words = b.words();
            for y in 0..usize::from(im.h) {
                let row = &im.argb[y * usize::from(im.w)..(y + 1) * usize::from(im.w)];
                let out = &mut words[y * usize::from(w)..(y + 1) * usize::from(w)];
                for (o, &c) in out.iter_mut().zip(row) {
                    *o = crate::list::abgr(c);
                }
            }
            // SAFETY: our buffer; the GE reads it after the list starts.
            unsafe { wb_range(b.ptr.cast(), (n * 4) as u32) };
            self.images[s] = Some((im.generation, w, h, b));
        }
    }

    /// Lets go of every page held in RAM and every chunk texture (a console leaving play: the
    /// next build wants the RAM); pages load again as frames name them.
    pub fn drop_pages(&mut self) {
        self.flush();
        let budget = self.lru.budget();
        for r in &mut self.ram {
            *r = None;
        }
        for g in &mut self.glow {
            *g = None;
        }
        self.lru = Lru::new(budget);
        self.slots = Slots::new(self.slots.len());
        self.chunks.clear();
        self.direct.clear();
        self.direct_pal.clear();
        self.pal_pages.clear();
        self.pal_glow.clear();
        self.patch_arena = None;
    }

    /// After a sleep and resume (PORT.md §13.13): VRAM is not trusted, so every page slot is
    /// marked empty (each page uploads again from RAM when drawn; the caller empties the lamp
    /// cache's pools too), the GE's base state and the display are set again, and the texture
    /// cache flushed. The CLUTs and the lightmap's target are rebuilt by every frame anyway.
    pub fn resumed(&mut self) {
        self.flush();
        self.slots.clear();
        self.back = 0;
        base_state();
    }

    /// What a sleep may do to VRAM, for a scripted run on an emulator that keeps it: the page
    /// slots and the lamp pools overwritten (as the owner's PSP-1000 showed after a resume).
    pub fn spoil_vram(&mut self) {
        self.flush();
        let at = (0x4400_0000 + LAMP_OFFSET) as *mut u8;
        let n = (0x20_0000 - LAMP_OFFSET) as usize;
        // SAFETY: the lamp pools and page slots past the framebuffers, through the uncached
        // mirror; the last list has run.
        unsafe { core::ptr::write_bytes(at, 0x5a, n) };
    }

    /// VRAM page slots holding a page, of all.
    pub fn slots_filled(&self) -> (usize, usize) {
        (self.slots.filled(), self.slots.len())
    }

    /// The RAM page cache's hits and loads since start.
    pub fn page_counts(&self) -> (u32, u32) {
        (self.lru.hits, self.lru.loads)
    }

    /// Whether UI image `slot` at `generation` is held as a texture (its px may be let go).
    pub fn holds_image(&self, slot: usize, generation: u32) -> bool {
        self.images.get(slot).is_some_and(|e| e.as_ref().is_some_and(|e| e.0 == generation))
    }

    /// The patches' arena: 1 once the first frame with a patch made it (0 until), and its
    /// bytes. Flat from then on, whatever a frame lays.
    pub fn patch_bytes(&self) -> (usize, usize) {
        (usize::from(self.patch_arena.is_some()), self.patch_arena.as_ref().map_or(0, |b| b.len))
    }

    /// Bytes the UI images hold (the log's line).
    pub fn image_bytes(&self) -> u32 {
        self.images.iter().flatten().map(|e| e.3.len as u32).sum()
    }

    /// The pack pages' texels for the lister (a caster's silhouette), loaded through `load`.
    pub fn pages<'a>(&'a mut self, load: &'a mut dyn FnMut(u32, &mut [u8]) -> bool) -> GePx<'a> {
        GePx { ge: self, load, last: None }
    }

    /// The frame on the screen (after [`show`](Self::show)): 272 rows of 512 `0xAABBGGRR` px, the
    /// first 480 of each shown. What a script's screenshot reads (PORT.md §13.13).
    pub fn shown(&self) -> &[u32] {
        let off = if self.back == 1 { 0 } else { FB_BYTES };
        // SAFETY: the shown framebuffer, through the uncached mirror; the GE has finished with it
        // (`draw` waits for its list) and nothing writes it until the next swap.
        unsafe { core::slice::from_raw_parts((0x4400_0000 + off) as *const u32, (BUF_W * SCR_H) as usize) }
    }

    /// Shows the frame drawn by [`Ge::pacing`], unless pipelined: then the next draw shows it
    /// once its list has run.
    pub fn show(&mut self) {
        if !self.pipelined {
            self.present();
            self.stats.show_us = core::mem::take(&mut self.wait_show);
        }
    }
}

/// [`Ge::pages`]: the pages held in RAM as [`PagePx`](crate::list::PagePx), the last one asked
/// kept to hand.
pub struct GePx<'a> {
    ge: &'a mut Ge,
    load: &'a mut dyn FnMut(u32, &mut [u8]) -> bool,
    /// The last page read: its index, px (past the CLUT) and width.
    last: Option<(u16, *const u8, usize, usize)>,
}

impl core::fmt::Debug for GePx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GePx").finish_non_exhaustive()
    }
}

impl crate::list::PagePx for GePx<'_> {
    fn texel(&mut self, page: u16, u: u16, v: u16) -> u8 {
        let (ptr, w, len) = match self.last {
            Some((p, ptr, w, len)) if p == page => (ptr, w, len),
            _ => {
                if !self.ge.hold(page, self.load) {
                    return 0;
                }
                let pages = self.ge.pack.pages.len();
                let Some(info) = self.ge.pack.pages.get(usize::from(page)).copied() else { return 0 };
                let Some(buf) = self.ge.ram[ram_ix(pages, page)].as_ref() else { return 0 };
                // SAFETY: the page's buffer holds its CLUT (1024 bytes) and then its px.
                let ptr = unsafe { buf.ptr.add(1024).cast_const() };
                let (w, len) = (usize::from(info.w), info.px_bytes as usize);
                self.last = Some((page, ptr, w, len));
                (ptr, w, len)
            }
        };
        // The swizzle: 16-byte x 8-row blocks, row-major.
        let (u, v) = (usize::from(u), usize::from(v));
        let k = ((v / 8) * (w / 16) + u / 16) * 128 + (v % 8) * 16 + u % 16;
        if k >= len {
            return 0;
        }
        // SAFETY: `k` is inside the page's px.
        unsafe { *ptr.add(k) }
    }
}
