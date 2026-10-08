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

use alloc::alloc::{Layout, alloc, dealloc};
use alloc::vec::Vec;
use core::ffi::c_void;

use psp::sys::{self, ClearBuffer, GuPrimitive, GuState, TexturePixelFormat, VertexType};

use crate::cache::{Lru, Slots};
use crate::list::{Lister, Mode, Tex};
use crate::pack::Pack;
use jane_present::Frame;
use jane_present::frame::CHUNK_PX;

const BUF_W: i32 = 512;
const SCR_W: i32 = 480;
const SCR_H: i32 = 272;
const FB_BYTES: u32 = (BUF_W * SCR_H * 4) as u32;
/// A VRAM slot: a 256 x 256 `T8` page and its CLUT.
const SLOT_BYTES: u32 = 1024 + 256 * 256;
/// The lightmap's render target after the framebuffers: 128 x 128 `8888`.
const RT_OFFSET: u32 = 2 * FB_BYTES;
const RT_BYTES: u32 = 128 * 128 * 4;
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

/// The display list: a frame's state changes and its vertices.
const LIST_WORDS: usize = 256 * 1024 / 4;

#[repr(C, align(16))]
struct List([u32; LIST_WORDS]);
static mut LIST: List = List([0; LIST_WORDS]);

/// A sprite vertex: 16-bit texel, `8888` colour, 16-bit position (through mode).
#[repr(C)]
#[derive(Clone, Copy)]
struct TexVertex {
    u: u16,
    v: u16,
    colour: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

/// A flat vertex: `8888` colour, 16-bit position.
#[repr(C)]
#[derive(Clone, Copy)]
struct FlatVertex {
    colour: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
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
    /// This frame's terrain patches, copied where the GE may read them.
    patches: Vec<Buf>,
    /// The lightmap's texture.
    light: Option<Buf>,
    /// This frame's light CLUT for the normal pages (16 entries).
    relief: Option<Buf>,
    /// This frame's lamp relief CLUTs (`Lister::lamp_reliefs`), 64 bytes each.
    lamp_cluts: Option<Buf>,
    /// The halo disc's texture, and the pool's.
    disc: Option<Buf>,
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
    /// The framebuffer drawn into: 0 or 1.
    back: u32,
    pub stats: DrawStats,
}

impl core::fmt::Debug for Ge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ge").field("stats", &self.stats).finish_non_exhaustive()
    }
}

fn list_ptr() -> *mut c_void {
    // The display list's static; only the game's one thread touches it.
    core::ptr::addr_of_mut!(LIST).cast()
}

impl Ge {
    /// Starts the GE and the display over `pack`'s tables; pages held in RAM up to `ram_budget`
    /// bytes.
    pub fn new(pack: Pack, ram_budget: u32) -> Ge {
        let pages = pack.pages.len();
        // SAFETY: the GU's start-up sequence, once, before any draw.
        unsafe {
            sys::sceGuInit();
            sys::sceGuStart(sys::GuContextType::Direct, list_ptr());
            sys::sceGuDrawBuffer(sys::DisplayPixelFormat::Psm8888, core::ptr::null_mut(), BUF_W);
            sys::sceGuDispBuffer(SCR_W, SCR_H, FB_BYTES as *mut c_void, BUF_W);
            // No depth buffer: nothing is depth tested, and no write may land in the slots.
            sys::sceGuDepthMask(1);
            sys::sceGuOffset(2048 - (SCR_W as u32 / 2), 2048 - (SCR_H as u32 / 2));
            sys::sceGuScissor(0, 0, SCR_W, SCR_H);
            sys::sceGuEnable(GuState::ScissorTest);
            sys::sceGuDisable(GuState::DepthTest);
            sys::sceGuShadeModel(sys::ShadingModel::Flat);
            sys::sceGuEnable(GuState::Blend);
            sys::sceGuEnable(GuState::AlphaTest);
            sys::sceGuAlphaFunc(sys::AlphaFunc::Greater, 0, 0xff);
            sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
            sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
            sys::sceGuFinish();
            sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait);
            sys::sceDisplayWaitVblankStart();
            sys::sceGuDisplay(true);
        }
        Ge {
            pack,
            ram: (0..2 * pages).map(|_| None).collect(),
            lru: Lru::new(ram_budget),
            slots: Slots::new(SLOTS),
            chunks: Vec::new(),
            patches: Vec::new(),
            light: Buf::new(crate::light::SIDE * crate::light::SIDE * 4),
            relief: Buf::new(64),
            glow: (0..pages).map(|_| None).collect(),
            lamp_cluts: Buf::new(crate::list::LAMP_RELIEFS * 64),
            grey: Buf::new(1024).map(|mut b| {
                for (k, w) in b.words()[..256].iter_mut().enumerate() {
                    *w = (k as u32) << 24 | 0x00ff_ffff;
                }
                // SAFETY: our buffer, written once.
                unsafe { sys::sceKernelDcacheWritebackRange(b.ptr.cast(), 1024) };
                b
            }),
            pool: Buf::new(crate::light::POOL * crate::light::POOL * 4).map(|mut b| {
                let d = crate::light::pool_disc();
                b.words()[..d.len()].copy_from_slice(&d);
                // SAFETY: our buffer, written once.
                unsafe { sys::sceKernelDcacheWritebackRange(b.ptr.cast(), (d.len() * 4) as u32) };
                b
            }),
            disc: Buf::new(crate::light::DISC * crate::light::DISC * 4).map(|mut b| {
                let d = crate::light::disc();
                b.words()[..d.len()].copy_from_slice(&d);
                // SAFETY: our buffer, written once.
                unsafe { sys::sceKernelDcacheWritebackRange(b.ptr.cast(), (d.len() * 4) as u32) };
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
            back: 0,
            stats: DrawStats::default(),
        }
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
            self.ram[ram_ix(self.pack.pages.len(), e)] = None;
            self.slots.forget(e);
        }
        let Some(mut buf) = Buf::new(bytes as usize) else {
            self.stats.page_load_fails += 1;
            return false;
        };
        if !load(offset, &mut buf.bytes()[..bytes as usize]) {
            self.stats.page_load_fails += 1;
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
        unsafe { sys::sceKernelDcacheWritebackRange(b.ptr.cast(), 1024) };
        self.glow[i] = Some(b);
        true
    }

    /// Binds page `p` as the texture: from its VRAM slot (copied there first if it is not), or
    /// from RAM when it is larger than a slot or no slot is free this frame. Inside a display
    /// list.
    fn bind_page(&mut self, p: u16) {
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
                    // Through the uncached mirror, so the GE reads what was written.
                    let dst = (vram as usize | 0x4000_0000) as *mut u8;
                    core::ptr::copy_nonoverlapping(buf.ptr, dst, info.bytes() as usize);
                    self.stats.uploads += 1;
                    sys::sceGuTexFlush();
                }
                base = vram;
            }
            sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, i32::from(info.swizzled));
            sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
            sys::sceGuClutLoad(32, base.cast());
            let (w, h) = (i32::from(info.w), i32::from(info.h));
            sys::sceGuTexImage(sys::MipmapLevel::None, w, h, w, base.add(1024).cast());
        }
    }

    /// Binds a chunk slot's texture, converting the frame's layer first when it is new. Inside a
    /// display list.
    fn bind_chunk(&mut self, frame: &Frame, slot: u16, generation: u32) {
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
            if self.direct[s].as_ref().is_none_or(|d| d.0 != generation) {
                let (clut, px) = match self.direct[s].take() {
                    Some((_, c, p)) => (Some(c), p),
                    None => (Buf::new(1024), None),
                };
                let Some(mut clut) = clut else { return };
                clut.words()[..256].copy_from_slice(&l.clut[..256]);
                let px = if aligned {
                    None
                } else {
                    let Some(mut p) = px.or_else(|| Buf::new(l.albedo.len() * 4)) else { return };
                    p.words()[..l.albedo.len()].copy_from_slice(&l.albedo);
                    Some(p)
                };
                // SAFETY: our buffers; a command into the open list.
                unsafe {
                    sys::sceKernelDcacheWritebackRange(clut.ptr.cast(), 1024);
                    if let Some(p) = &px {
                        sys::sceKernelDcacheWritebackRange(p.ptr.cast(), p.len as u32);
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
                sys::sceGuClutLoad(32, clut.ptr.cast());
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
                sys::sceKernelDcacheWritebackRange(b.ptr.cast(), (n * 4) as u32);
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

    /// Draws `lister`'s quads for `frame` (built from it) and shows them at the next vblank.
    /// Pages are loaded through `load(offset, bytes)`: the pack file read at that offset.
    pub fn draw(&mut self, frame: &Frame, lister: &Lister, load: &mut dyn FnMut(u32, &mut [u8]) -> bool) {
        let mut st = DrawStats { quads: lister.quads.len() as u32, ..DrawStats::default() };
        let (loads, uploads) = (self.stats.page_loads, self.slots.uploads);
        self.stats = DrawStats::default();
        self.lru.next_frame();
        self.slots.next_frame();
        // Every page this frame draws, held in RAM before the list starts.
        for q in &lister.quads {
            match q.tex {
                Tex::Page(p) | Tex::Glow(p) => {
                    self.hold(p, load);
                }
                Tex::Normal(p, _) => {
                    self.hold(p | NORMAL, load);
                }
                _ => {}
            }
        }
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
        if let (Some(c), Some(b)) = (lister.relief, self.relief.as_mut()) {
            b.words()[..16].copy_from_slice(&c);
        }
        // SAFETY: one display list, built and run here; vertices are taken from it.
        unsafe {
            sys::sceKernelDcacheWritebackAll();
            sys::sceGuStart(sys::GuContextType::Direct, list_ptr());
            sys::sceGuClearColor(lister.clear);
            sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT);
            let mut i = 0;
            let quads = &lister.quads;
            let mut bound: Option<Tex> = None;
            let mut mode: Option<Mode> = None;
            while i < quads.len() {
                let q = quads[i];
                // Into the lightmap's target and back: commands, not quads.
                if q.mode == Mode::RtBegin {
                    sys::sceGuDrawBufferList(sys::DisplayPixelFormat::Psm8888, RT_OFFSET as *mut c_void, 128);
                    sys::sceGuScissor(0, 0, i32::from(q.x1), i32::from(q.y1));
                    // Cleared by a flat rect, colour and stencil (alpha) written as they are
                    // (`sceGuClear` covers the screen's size, past this small target).
                    sys::sceGuDisable(GuState::Blend);
                    sys::sceGuDisable(GuState::AlphaTest);
                    sys::sceGuDisable(GuState::Texture2D);
                    let v = sys::sceGuGetMemory((2 * core::mem::size_of::<FlatVertex>()) as i32).cast::<FlatVertex>();
                    let c = q.colour & 0x00ff_ffff;
                    v.write(FlatVertex { colour: c, x: 0, y: 0, z: 0, _pad: 0 });
                    v.add(1).write(FlatVertex { colour: c, x: q.x1, y: q.y1, z: 0, _pad: 0 });
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
                    sys::sceGuDisable(GuState::StencilTest);
                    sys::sceGuPixelMask(0);
                    let fb = if self.back == 0 { 0 } else { FB_BYTES };
                    sys::sceGuDrawBufferList(sys::DisplayPixelFormat::Psm8888, fb as *mut c_void, BUF_W);
                    sys::sceGuScissor(0, 0, SCR_W, SCR_H);
                    sys::sceGuTexFlush();
                    sys::sceGuTexSync();
                    mode = None;
                    bound = None;
                    i += 1;
                    continue;
                }
                // A batch: the quads after it with its texture and mode.
                let mut j = i + 1;
                while j < quads.len() && quads[j].tex == q.tex && quads[j].mode == q.mode {
                    j += 1;
                }
                if bound != Some(q.tex) {
                    match q.tex {
                        Tex::None | Tex::Poly(_) => sys::sceGuDisable(GuState::Texture2D),
                        Tex::Page(p) => {
                            if self.ram[ram_ix(self.pack.pages.len(), p)].is_none() {
                                i = j;
                                continue;
                            }
                            sys::sceGuEnable(GuState::Texture2D);
                            self.bind_page(p);
                        }
                        Tex::Patch(k) => {
                            let Some(p) = lister.patches.get(usize::from(k)) else {
                                i = j;
                                continue;
                            };
                            let k = usize::from(k);
                            let bytes = p.px.len() * 4;
                            if self.patches.len() <= k || self.patches[k].len < bytes {
                                let Some(b) = Buf::new(bytes.max(16 * 1024)) else {
                                    i = j;
                                    continue;
                                };
                                if self.patches.len() <= k {
                                    self.patches.push(b);
                                } else {
                                    self.patches[k] = b;
                                }
                            }
                            let b = &mut self.patches[k];
                            b.words()[..p.px.len()].copy_from_slice(&p.px);
                            sys::sceKernelDcacheWritebackRange(b.ptr.cast(), bytes as u32);
                            sys::sceGuTexFlush();
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                            let (tw, th) = (i32::from(p.tw), i32::from(p.th));
                            sys::sceGuTexImage(sys::MipmapLevel::None, tw, th, tw, b.ptr.cast());
                        }
                        Tex::Glow(p) => {
                            if self.ram[ram_ix(self.pack.pages.len(), p)].is_none() || !self.hold_glow(p, load) {
                                i = j;
                                continue;
                            }
                            sys::sceGuEnable(GuState::Texture2D);
                            self.bind_page(p);
                            // The page's own CLUT swapped for its glow CLUT.
                            if let Some(Some(g)) = self.glow.get(usize::from(p)) {
                                sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                                sys::sceGuClutLoad(32, g.ptr.cast());
                            }
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
                            sys::sceGuClutLoad(2, cb.cast());
                            let (w, h) = (i32::from(info.w), i32::from(info.h));
                            sys::sceGuTexImage(sys::MipmapLevel::None, w, h, w, nb.ptr.cast());
                        }
                        Tex::Height(slot) => {
                            let Some(l) = frame.layers.get(usize::from(slot)).filter(|l| l.has_height()) else {
                                i = j;
                                continue;
                            };
                            let (Some(m), true) = (self.height_mask.as_ref(), (l.height.as_ptr() as usize) % 16 == 0)
                            else {
                                i = j;
                                continue;
                            };
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                            sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                            sys::sceGuClutLoad(32, m.ptr.cast());
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
                            let at =
                                (0x0400_0000 + LAMP_OFFSET + u32::from(slot) * (side * side) as u32) as *const c_void;
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::PsmT8, 0, 0, 0);
                            sys::sceGuClutMode(sys::ClutPixelFormat::Psm8888, 0, 0xff, 0);
                            sys::sceGuClutLoad(32, g.ptr.cast());
                            sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, at);
                            sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                            sys::sceGuTexFlush();
                        }
                        Tex::LightRt => {
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                            let rt = (0x0400_0000 + RT_OFFSET) as *const c_void;
                            sys::sceGuTexImage(sys::MipmapLevel::None, 128, 128, 128, rt);
                            sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                        }
                        Tex::Disc => {
                            let Some(b) = self.disc.as_ref() else {
                                i = j;
                                continue;
                            };
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                            let side = crate::light::DISC as i32;
                            sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, b.ptr.cast());
                            sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                        }
                        Tex::Lightmap => {
                            let Some(b) = self.light.as_mut() else {
                                i = j;
                                continue;
                            };
                            let px = &lister.light.px;
                            b.words()[..px.len()].copy_from_slice(px);
                            sys::sceKernelDcacheWritebackRange(b.ptr.cast(), (px.len() * 4) as u32);
                            sys::sceGuTexFlush();
                            sys::sceGuEnable(GuState::Texture2D);
                            sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
                            let side = crate::light::SIDE as i32;
                            sys::sceGuTexImage(sys::MipmapLevel::None, side, side, side, b.ptr.cast());
                            sys::sceGuTexFilter(sys::TextureFilter::Linear, sys::TextureFilter::Linear);
                        }
                        Tex::Chunk(slot) => {
                            let generation = lister.chunks.iter().find(|c| c.0 == slot).map_or(0, |c| c.1);
                            sys::sceGuEnable(GuState::Texture2D);
                            self.bind_chunk(frame, slot, generation);
                        }
                    }
                    if matches!(bound, Some(Tex::Lightmap | Tex::Disc | Tex::Pool | Tex::LightRt | Tex::LampTex(_))) {
                        sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
                    }
                    bound = Some(q.tex);
                }
                if mode != Some(q.mode) {
                    let stencil = |m: Mode| {
                        matches!(
                            m,
                            Mode::StencilClear | Mode::StencilMark | Mode::ShadowBand | Mode::PoolLit | Mode::PoolShade
                        )
                    };
                    if mode.is_some_and(stencil) && !stencil(q.mode) {
                        sys::sceGuDisable(GuState::StencilTest);
                        sys::sceGuPixelMask(0);
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
                        // Where the stencil is not 1: shaded, and set to 1.
                        Mode::ShadowBand => {
                            sys::sceGuEnable(GuState::StencilTest);
                            sys::sceGuPixelMask(0);
                            sys::sceGuStencilFunc(sys::StencilFunc::NotEqual, 1, 0xff);
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
                if let Tex::Poly(_) = q.tex {
                    // Each polygon its own fan of flat vertices.
                    for q in &quads[i..j] {
                        let Tex::Poly(pi) = q.tex else { continue };
                        let Some((pts, n)) = lister.polys.get(usize::from(pi)) else { continue };
                        let n = usize::from(*n);
                        if n < 3 {
                            continue;
                        }
                        let v =
                            sys::sceGuGetMemory((n * core::mem::size_of::<FlatVertex>()) as i32).cast::<FlatVertex>();
                        for (k, &(x, y)) in pts[..n].iter().enumerate() {
                            v.add(k).write(FlatVertex { colour: q.colour, x, y, z: 0, _pad: 0 });
                        }
                        sys::sceGuDrawArray(
                            GuPrimitive::TriangleFan,
                            VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                            n as i32,
                            core::ptr::null(),
                            v.cast(),
                        );
                    }
                } else if q.tex == Tex::None {
                    let v =
                        sys::sceGuGetMemory((n * 2 * core::mem::size_of::<FlatVertex>()) as i32).cast::<FlatVertex>();
                    for (k, q) in quads[i..j].iter().enumerate() {
                        v.add(2 * k).write(FlatVertex { colour: q.colour, x: q.x0, y: q.y0, z: 0, _pad: 0 });
                        v.add(2 * k + 1).write(FlatVertex { colour: q.colour, x: q.x1, y: q.y1, z: 0, _pad: 0 });
                    }
                    sys::sceGuDrawArray(
                        GuPrimitive::Sprites,
                        VertexType::COLOR_8888 | VertexType::VERTEX_16BIT | VertexType::TRANSFORM_2D,
                        (2 * n) as i32,
                        core::ptr::null(),
                        v.cast(),
                    );
                } else {
                    let v = sys::sceGuGetMemory((n * 2 * core::mem::size_of::<TexVertex>()) as i32).cast::<TexVertex>();
                    for (k, q) in quads[i..j].iter().enumerate() {
                        let c = q.colour;
                        v.add(2 * k).write(TexVertex { u: q.u0, v: q.v0, colour: c, x: q.x0, y: q.y0, z: 0, _pad: 0 });
                        v.add(2 * k + 1).write(TexVertex {
                            u: q.u1,
                            v: q.v1,
                            colour: c,
                            x: q.x1,
                            y: q.y1,
                            z: 0,
                            _pad: 0,
                        });
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
                i = j;
            }
            sys::sceGuFinish();
            sys::sceGuSync(sys::GuSyncMode::Finish, sys::GuSyncBehavior::Wait);
        }
        st.page_loads = self.stats.page_loads;
        st.page_load_fails = self.stats.page_load_fails;
        st.uploads = self.slots.uploads - uploads;
        st.chunks_converted = self.stats.chunks_converted;
        st.ram_pages_bytes = self.lru.bytes();
        st.chunk_bytes = self.chunks.iter().flatten().map(|c| c.1.len as u32).sum();
        let _ = loads;
        self.stats = st;
    }

    /// The pack pages' texels for the lister (a caster's silhouette), loaded through `load`.
    pub fn pages<'a>(&'a mut self, load: &'a mut dyn FnMut(u32, &mut [u8]) -> bool) -> GePx<'a> {
        GePx { ge: self, load, last: None }
    }

    /// Shows the frame drawn at the next vblank (waits for it).
    pub fn show(&mut self) {
        // SAFETY: plain syscalls; the list has finished.
        unsafe {
            sys::sceDisplayWaitVblankStart();
            sys::sceGuSwapBuffers();
        }
        self.back ^= 1;
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
