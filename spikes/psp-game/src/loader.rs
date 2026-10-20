//! The baked county's loader (PLAN-PSP-SPEED.md §5, P1c). `county.jck` beside the program holds
//! every county chunk as the console's painter leaves it, each compressed on its own; this reads
//! the chunks the presenter asks for (`Present::baked_wants`) on a thread of its own, with
//! `sceIoReadAsync`, decodes them there (zstd, `ruzstd`) into a spare slot's layers, and hands
//! each to the game, which swaps it into the chunk cache (`Present::land_baked`): no read, no
//! decode and no copy on the game's thread. The chunks to land come first; then it reads ahead
//! (the blobs only, into a small cache) along her way.
//!
//! RAM: the pack's index (about 36 KB), [`CACHE`] blob buffers of the largest blob (24 KB in
//! the county: about 144 KB), one spare slot (about 130 KB), the decoder's state (ruzstd's
//! window and buffers, about 100 KB) and the thread's stack ([`STACK`]).
//!
//! Hand-offs, each one way: the game posts its wishes under a sequence count (a seqlock; the
//! loader reads them again when the count moved), the loader posts one decoded chunk at a time
//! (`READY`), the game gives the slot it swapped out back (`SPARE`), and a chunk the loader could
//! not give is named in `FAILED` (the painter takes it).

use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering};

use alloc::boxed::Box;
use alloc::vec::Vec;

use jane_present::Present;
use jane_present::chunk_pack::{Decoder, Index, Landing};
use jane_present::frame::ChunkId;
use psp::sys;

use super::Line;

/// Blob buffers read ahead (and read into before a decode).
const CACHE: usize = 6;
/// The loader's stack: ruzstd's decode keeps its state on the heap.
const STACK: i32 = 48 * 1024;
/// Its priority: above the game's (32), below the audio's. Below it (40), an unpaced game (the
/// bench's `fps:free`, the Graphics page's Unlocked) or a busy one never slept and the loader
/// starved: every frame of the owner's 2026-10-21 bench showed stand-ins. It mostly waits on a
/// read or the semaphore; an lz4 chunk's decode (about 5 ms on the PSP) is its only long run.
const PRIO: i32 = 30;
/// The most chunks a wish names (`Present`'s `BAKED_WANTS` + `BAKED_AHEAD`).
const WISH: usize = 18;
/// A chunk posted (or failed) is not decoded or read again for this long, microseconds: the game
/// lands it, or no longer wants it, within a frame or two.
const RETRY_US: u32 = 100_000;

static OPEN: AtomicBool = AtomicBool::new(false);
static GRID: AtomicU32 = AtomicU32::new(0);
static SEMA: AtomicI32 = AtomicI32::new(-1);
static STATE: AtomicPtr<Loader> = AtomicPtr::new(core::ptr::null_mut());
/// The wishes: a seqlock's count (odd while the game writes), how many to land and to read ahead
/// (`land << 16 | ahead`), and the chunks (`cx << 16 | cy`).
static WISH_SEQ: AtomicU32 = AtomicU32::new(0);
static WISH_N: AtomicU32 = AtomicU32::new(0);
static WISHES: [AtomicU32; WISH] = [const { AtomicU32::new(0) }; WISH];
static READY: AtomicPtr<Landing> = AtomicPtr::new(core::ptr::null_mut());
static SPARE: AtomicPtr<Landing> = AtomicPtr::new(core::ptr::null_mut());
/// A chunk the loader could not give, `cx << 16 | cy` + 1 (0: none).
static FAILED: AtomicU32 = AtomicU32::new(0);
/// Counts for the log: blobs read, their microseconds, decodes and theirs, chunks landed, posted
/// and not landed, failed.
pub static READS: AtomicU32 = AtomicU32::new(0);
pub static READ_US: AtomicU32 = AtomicU32::new(0);
pub static DECODES: AtomicU32 = AtomicU32::new(0);
pub static DECODE_US: AtomicU32 = AtomicU32::new(0);
pub static LANDED: AtomicU32 = AtomicU32::new(0);
pub static REFUSED: AtomicU32 = AtomicU32::new(0);
pub static FAILS: AtomicU32 = AtomicU32::new(0);
/// Frames that ended with a baked chunk to land still not landed (the presenter's `land` wishes
/// not empty after this frame's landing): the bench's `baked_wait`.
pub static WAIT_FRAMES: AtomicU32 = AtomicU32::new(0);

/// The counts above, in that order and with [`WAIT_FRAMES`] last: the bench takes a scene's
/// difference of two.
pub fn counts() -> [u32; 8] {
    [&READS, &READ_US, &DECODES, &DECODE_US, &LANDED, &REFUSED, &FAILS, &WAIT_FRAMES].map(|c| c.load(Ordering::Relaxed))
}
/// Bytes the loader holds (the log's).
pub static RAM: AtomicU32 = AtomicU32::new(0);

fn pack(id: ChunkId) -> u32 {
    u32::from(id.cx) << 16 | u32::from(id.cy)
}

fn unpack(w: u32) -> ChunkId {
    ChunkId { cx: (w >> 16) as u16, cy: w as u16 }
}

fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

struct Entry {
    id: Option<ChunkId>,
    used: u32,
    len: usize,
    buf: Vec<u8>,
}

struct Loader {
    path: Vec<u8>,
    fd: sys::SceUid,
    index: Index,
    cache: Vec<Entry>,
    decoder: Decoder,
    /// The loader's own clock (each unit of work), for the cache's LRU.
    clock: u32,
    /// The chunks posted lately and when (`now_us`).
    posted: [(u32, u32); 8],
    posted_at: usize,
    /// The wishes as last read: their count, to land, to read ahead.
    seq: u32,
    land: Vec<ChunkId>,
    ahead: Vec<ChunkId>,
}

fn open_fd(z: &[u8]) -> Option<sys::SceUid> {
    // SAFETY: a NUL-terminated path that outlives the call.
    let fd = unsafe { sys::sceIoOpen(z.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    (fd.0 >= 0).then_some(fd)
}

/// `buf` filled from byte `at` of `fd`; false on a failed or short read. A plain blocking read:
/// only this thread sleeps on it (the game and the mixer run meanwhile). Not `sceIoReadAsync`:
/// its work runs on an I/O thread at the priority of the thread that opened the fd (the game's,
/// at boot), which an unpaced or busy game, never sleeping, starved; reads took 0.5–1.1 s each
/// in the 2026-10-21 bench's scenes against a few ms for the same reads made in sync.
fn read_async(fd: sys::SceUid, at: u32, buf: &mut [u8]) -> bool {
    // SAFETY: an open fd only this thread reads; `buf` is ours for the call.
    unsafe {
        if sys::sceIoLseek(fd, i64::from(at), sys::IoWhence::Set) != i64::from(at) {
            return false;
        }
        sys::sceIoRead(fd, buf.as_mut_ptr().cast(), buf.len() as u32) == buf.len() as i32
    }
}

/// Opens `path` (the pack) and starts the loader; the pack's chunk grid, or `None` if there is
/// no pack (the painter paints the county, as before). At boot: its index is read here.
pub fn open(path: &str) -> Option<(u16, u16)> {
    let mut z = Vec::with_capacity(path.len() + 1);
    z.extend_from_slice(path.as_bytes());
    z.push(0);
    let fd = open_fd(&z)?;
    // SAFETY: an open fd.
    let size = unsafe {
        let n = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
        sys::sceIoLseek(fd, 0, sys::IoWhence::Set);
        n
    };
    let close = |why: &str| {
        say!("GAME baked: {path} {why}");
        // SAFETY: our fd.
        unsafe { sys::sceIoClose(fd) };
        None
    };
    let Ok(size) = u32::try_from(size) else { return close("size") };
    let mut head = alloc::vec![0u8; Index::HEAD];
    if !read_async(fd, 0, &mut head) {
        return close("header unread");
    }
    let Ok(n) = Index::head_len(&head) else { return close("not a pack") };
    head.resize(n, 0);
    if !read_async(fd, 0, &mut head) {
        return close("index unread");
    }
    let index = match Index::read(&head, size) {
        Ok(i) => i,
        Err(e) => {
            say!("GAME baked: {e:?}");
            return close("index refused");
        }
    };
    drop(head);
    let most = (index.most() as usize).next_multiple_of(64);
    let (cols, rows) = (index.cols, index.rows);
    let ram = n + CACHE * most + 2 * 64 * 1024 + 2 * 1024 + 100 * 1024;
    RAM.store(ram as u32, Ordering::Relaxed);
    let cache = (0..CACHE).map(|_| Entry { id: None, used: 0, len: 0, buf: alloc::vec![0; most] }).collect();
    let loader = Loader {
        path: z,
        fd,
        index,
        cache,
        decoder: Decoder::new(),
        clock: 0,
        posted: [(u32::MAX, 0); 8],
        posted_at: 0,
        seq: u32::MAX,
        land: Vec::with_capacity(WISH),
        ahead: Vec::with_capacity(WISH),
    };
    STATE.store(Box::into_raw(Box::new(loader)), Ordering::Release);
    SPARE.store(Box::into_raw(Box::new(Landing::new())), Ordering::Release);
    // SAFETY: a semaphore made once, before the thread that waits on it.
    let sema = unsafe { sys::sceKernelCreateSema(b"baked\0".as_ptr(), 0, 0, 1, core::ptr::null_mut()) };
    SEMA.store(sema.0, Ordering::Release);
    // From here an allocation holds the scheduler (the loader's decode allocates).
    crate::THREADED.store(true, Ordering::Release);
    crate::start_thread(b"baked\0", run, PRIO, STACK);
    GRID.store(u32::from(cols) << 16 | u32::from(rows), Ordering::Relaxed);
    OPEN.store(true, Ordering::Release);
    say!("GAME baked: {path} {cols}x{rows} chunks, blob most {most} B, loader ram {ram} B");
    Some((cols, rows))
}

/// The pack's chunk grid, if it is open.
pub fn grid() -> Option<(u16, u16)> {
    OPEN.load(Ordering::Acquire).then(|| {
        let g = GRID.load(Ordering::Relaxed);
        ((g >> 16) as u16, g as u16)
    })
}

/// The game's side, once a frame after the tick: lands what the loader decoded, hands the slot
/// swapped out back, tells the presenter of a chunk the loader failed on, and posts its wishes.
pub fn frame(present: &mut Present) {
    if !OPEN.load(Ordering::Acquire) {
        return;
    }
    let mut wake = false;
    let p = READY.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !p.is_null() {
        // SAFETY: the loader stored it and no longer touches it.
        let mut l = unsafe { Box::from_raw(p) };
        if present.land_baked(&mut l) {
            LANDED.fetch_add(1, Ordering::Relaxed);
        } else {
            REFUSED.fetch_add(1, Ordering::Relaxed);
        }
        SPARE.store(Box::into_raw(l), Ordering::Release);
        wake = true;
    }
    let f = FAILED.swap(0, Ordering::AcqRel);
    if f != 0 {
        present.baked_failed(unpack(f - 1));
        wake = true;
    }
    let (land, ahead) = present.baked_wants();
    if !land.is_empty() {
        WAIT_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    let (nl, na) = (land.len().min(WISH), ahead.len().min(WISH - land.len().min(WISH)));
    let n = (nl as u32) << 16 | na as u32;
    let same = WISH_N.load(Ordering::Relaxed) == n
        && land.iter().chain(&ahead[..na]).zip(&WISHES).all(|(&id, w)| w.load(Ordering::Relaxed) == pack(id));
    if !same {
        let s = WISH_SEQ.load(Ordering::Relaxed);
        WISH_SEQ.store(s.wrapping_add(1), Ordering::Release);
        WISH_N.store(n, Ordering::Release);
        for (w, &id) in WISHES.iter().zip(land[..nl].iter().chain(&ahead[..na])) {
            w.store(pack(id), Ordering::Release);
        }
        WISH_SEQ.store(s.wrapping_add(2), Ordering::Release);
    }
    if wake || (!same && n != 0) {
        // SAFETY: the semaphore the loader waits on (a signal past its max is refused, harmless).
        unsafe { sys::sceKernelSignalSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1) };
    }
}

/// A world let go (the title, New Game, Load): the slot the loader holds for the next landing
/// made again, now. That slot is one the last world's chunk cache swapped out (two 64 KB blocks,
/// about 129 KB), so it lay in the middle of the RAM the world gave back and split it: after a
/// played world the largest free block was 3 MB smaller and the next county's build ran out
/// (PPSSPP as a PSP-1000, `retitle`, 2026-10-21). Made after the world is gone, the new one
/// takes the top of the free RAM. A slot the loader is decoding into is left as it is.
pub fn renew() {
    if !OPEN.load(Ordering::Acquire) {
        return;
    }
    // At most one slot is about: given back (SPARE), posted (READY, for the old world's chunk:
    // let go unlanded) or in the loader's hands.
    let spare = SPARE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    let ready =
        if spare.is_null() { READY.swap(core::ptr::null_mut(), Ordering::AcqRel) } else { core::ptr::null_mut() };
    for p in [spare, ready] {
        if !p.is_null() {
            // SAFETY: taken from the hand-off; the loader no longer touches it.
            drop(unsafe { Box::from_raw(p) });
        }
    }
    if !spare.is_null() || !ready.is_null() {
        SPARE.store(Box::into_raw(Box::new(Landing::new())), Ordering::Release);
        // SAFETY: the semaphore the loader waits on (it may have been waiting for a slot).
        unsafe { sys::sceKernelSignalSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1) };
    }
}

extern "C" fn run(_argc: usize, _argv: *mut core::ffi::c_void) -> i32 {
    // SAFETY: stored once before this thread started; only this thread touches it after.
    let me = unsafe { &mut *STATE.load(Ordering::Acquire) };
    loop {
        if !me.step() {
            // SAFETY: a semaphore made before this thread started.
            unsafe { sys::sceKernelWaitSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1, core::ptr::null_mut()) };
        }
    }
}

impl Loader {
    /// The game's wishes read again if they moved (a seqlock: read until the count holds still
    /// and even).
    fn read_wishes(&mut self) {
        loop {
            let s = WISH_SEQ.load(Ordering::Acquire);
            if s == self.seq {
                return;
            }
            if s & 1 == 1 {
                core::hint::spin_loop();
                // SAFETY: plain syscall: the game is mid-write; sleep so it can finish (this
                // thread outranks it).
                unsafe { sys::sceKernelDelayThread(100) };
                continue;
            }
            let n = WISH_N.load(Ordering::Acquire);
            let (nl, na) = ((n >> 16) as usize, (n & 0xffff) as usize);
            self.land.clear();
            self.ahead.clear();
            for (i, w) in WISHES.iter().enumerate().take((nl + na).min(WISH)) {
                let id = unpack(w.load(Ordering::Acquire));
                if i < nl { self.land.push(id) } else { self.ahead.push(id) }
            }
            if WISH_SEQ.load(Ordering::Acquire) == s {
                self.seq = s;
                return;
            }
        }
    }

    /// One unit of work: a chunk decoded and posted, or a blob read ahead. False when there is
    /// nothing to do.
    fn step(&mut self) -> bool {
        self.read_wishes();
        self.clock = self.clock.wrapping_add(1);
        // A chunk to land, while the game has taken the last one and given a slot back.
        if READY.load(Ordering::Acquire).is_null() && !SPARE.load(Ordering::Acquire).is_null() {
            let pick = self.land.iter().copied().find(|&id| !self.recent(id));
            if let Some(id) = pick {
                if let Some(e) = self.blob(id) {
                    self.post(id, e);
                } else {
                    self.fail(id);
                }
                return true;
            }
        }
        // A blob to read ahead: the chunks to land not read yet, then those further along, as
        // many as the cache holds but one (so a read ahead never evicts another's blob).
        let next = self.near().find(|&id| self.held(id).is_none() && !self.recent(id));
        match next {
            Some(id) => {
                if self.blob(id).is_none() {
                    self.fail(id);
                }
                true
            }
            None => false,
        }
    }

    /// The wishes the cache holds blobs for: the first [`CACHE`] - 1, those to land first.
    fn near(&self) -> impl Iterator<Item = ChunkId> + '_ {
        self.land.iter().chain(&self.ahead).copied().take(CACHE - 1)
    }

    /// Whether chunk `id` was posted or failed within [`RETRY_US`].
    fn recent(&self, id: ChunkId) -> bool {
        let now = now_us();
        self.posted.iter().any(|&(p, at)| p == pack(id) && now.wrapping_sub(at) < RETRY_US)
    }

    fn mark(&mut self, id: ChunkId) {
        self.posted[self.posted_at] = (pack(id), now_us());
        self.posted_at = (self.posted_at + 1) % self.posted.len();
    }

    fn held(&self, id: ChunkId) -> Option<usize> {
        self.cache.iter().position(|e| e.id == Some(id))
    }

    /// Chunk `id`'s blob in the cache, read if it is not: its entry.
    fn blob(&mut self, id: ChunkId) -> Option<usize> {
        if let Some(i) = self.held(id) {
            self.cache[i].used = self.clock;
            return Some(i);
        }
        let (at, len) = self.index.span(id)?;
        let len = len as usize;
        // The least recently used entry not among the nearest wishes.
        let wished = |e: &Entry| e.id.is_some_and(|c| self.near().any(|n| n == c));
        let i = (0..self.cache.len()).min_by_key(|&i| (wished(&self.cache[i]), self.cache[i].used)).expect("a cache");
        let e = &mut self.cache[i];
        e.id = None;
        if len > e.buf.len() {
            return None;
        }
        let t = now_us();
        let mut ok = read_async(self.fd, at, &mut e.buf[..len]);
        if !ok {
            // A stale handle (a sleep and resume): the file opened again, the read tried once more.
            // SAFETY: our fd, closed once; a stale one's close fails harmlessly.
            unsafe { sys::sceIoClose(self.fd) };
            if let Some(fd) = open_fd(&self.path) {
                self.fd = fd;
                ok = read_async(self.fd, at, &mut e.buf[..len]);
            }
        }
        READS.fetch_add(1, Ordering::Relaxed);
        READ_US.fetch_add(now_us().wrapping_sub(t), Ordering::Relaxed);
        if !ok {
            return None;
        }
        (e.id, e.used, e.len) = (Some(id), self.clock, len);
        Some(i)
    }

    /// Entry `e`'s chunk decoded into the spare slot and posted to the game.
    fn post(&mut self, id: ChunkId, e: usize) {
        let p = SPARE.swap(core::ptr::null_mut(), Ordering::AcqRel);
        // SAFETY: the game gave it back and no longer touches it.
        let mut l = unsafe { Box::from_raw(p) };
        let t = now_us();
        let e = &self.cache[e];
        let ok = self.decoder.decode_landing(self.index.codec, &e.buf[..e.len], &mut l).is_ok();
        DECODES.fetch_add(1, Ordering::Relaxed);
        DECODE_US.fetch_add(now_us().wrapping_sub(t), Ordering::Relaxed);
        if !ok {
            SPARE.store(Box::into_raw(l), Ordering::Release);
            self.fail(id);
            return;
        }
        l.id = id;
        self.mark(id);
        READY.store(Box::into_raw(l), Ordering::Release);
    }

    /// Chunk `id` could not be given: the game is told (once it has taken the last).
    fn fail(&mut self, id: ChunkId) {
        FAILS.fetch_add(1, Ordering::Relaxed);
        say!("GAME baked: chunk {},{} failed", id.cx, id.cy);
        while FAILED.compare_exchange(0, pack(id) + 1, Ordering::AcqRel, Ordering::Acquire).is_err() {
            // SAFETY: plain syscall: the game takes the last failure within a frame.
            unsafe { sys::sceKernelDelayThread(10_000) };
        }
        // Not asked for again until the game has read it.
        self.mark(id);
    }
}
