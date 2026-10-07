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
/// VRAM slots after the two framebuffers.
pub const SLOTS: usize = ((0x20_0000 - 2 * FB_BYTES) / SLOT_BYTES) as usize;
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
            ram: (0..pages).map(|_| None).collect(),
            lru: Lru::new(ram_budget),
            slots: Slots::new(SLOTS),
            chunks: Vec::new(),
            patches: Vec::new(),
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
        let Some(info) = self.pack.pages.get(usize::from(p)).copied() else { return false };
        if self.lru.want(p, info.bytes(), &mut self.evicted) && self.ram[usize::from(p)].is_some() {
            return true;
        }
        for e in core::mem::take(&mut self.evicted) {
            self.ram[usize::from(e)] = None;
            self.slots.forget(e);
        }
        let Some(mut buf) = Buf::new(info.bytes() as usize) else {
            self.stats.page_load_fails += 1;
            return false;
        };
        if !load(info.offset, &mut buf.bytes()[..info.bytes() as usize]) {
            self.stats.page_load_fails += 1;
            return false;
        }
        self.stats.page_loads += 1;
        self.ram[usize::from(p)] = Some(buf);
        true
    }

    /// Binds page `p` as the texture: from its VRAM slot (copied there first if it is not), or
    /// from RAM when it is larger than a slot or no slot is free this frame. Inside a display
    /// list.
    fn bind_page(&mut self, p: u16) {
        let info = self.pack.pages[usize::from(p)];
        let Some(buf) = self.ram[usize::from(p)].as_mut() else { return };
        let mut base = buf.ptr.cast_const();
        // SAFETY: VRAM past the framebuffers is the slots'; the slot is not drawn from this
        // frame (`Slots::place`), so the GE is not reading it; the GU calls only add commands.
        unsafe {
            if info.bytes() <= SLOT_BYTES
                && let Some((slot, fresh)) = self.slots.place(p)
            {
                let vram = (0x0400_0000 + 2 * FB_BYTES + slot as u32 * SLOT_BYTES) as *mut u8;
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
            if let Tex::Page(p) = q.tex {
                self.hold(p, load);
            }
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
                // A batch: the quads after it with its texture and mode.
                let mut j = i + 1;
                while j < quads.len() && quads[j].tex == q.tex && quads[j].mode == q.mode {
                    j += 1;
                }
                if bound != Some(q.tex) {
                    match q.tex {
                        Tex::None => sys::sceGuDisable(GuState::Texture2D),
                        Tex::Page(p) => {
                            if self.ram[usize::from(p)].is_none() {
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
                        Tex::Chunk(slot) => {
                            let generation = lister.chunks.iter().find(|c| c.0 == slot).map_or(0, |c| c.1);
                            sys::sceGuEnable(GuState::Texture2D);
                            self.bind_chunk(frame, slot, generation);
                        }
                    }
                    bound = Some(q.tex);
                }
                if mode != Some(q.mode) {
                    match q.mode {
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
                if q.tex == Tex::None {
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
