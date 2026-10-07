//! The C2 backend (PORT.md §13.5, §13.12): the PSP's GE draws the same `Frame` every tier draws.
//!
//! - [`pack`] reads the PSP pack's tables (`JPK2`); its pages stay on the Memory Stick until a
//!   frame names one.
//! - [`list`] turns a `Frame` into quads: sprites resolved from the presenter's atlas rects to
//!   the pack's trimmed rects on its `T8` pages, chunks as textures, the ambient light as a
//!   multiply. Pure and integer, so a PC test reads what the PSP draws.
//! - [`cache`] decides which pages are held in RAM (an LRU under a byte budget) and which in
//!   VRAM (an LRU of slots).
//! - `ge` (PSP only) sends the quads to the GE through rust-psp's `sceGu*`: the one module with
//!   unsafe code, as every console's platform glue is.
//!
//! `no_std` plus `alloc` without the `std` feature; integer only.

#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

pub mod cache;
#[cfg(target_os = "psp")]
#[allow(unsafe_code)]
pub mod ge;
pub mod light;
pub mod list;
pub mod pack;

pub use list::{Lister, Mode, Quad, Tex};
pub use pack::Pack;

/// A terrain chunk's albedo (`0xAARRGGBB`) into `out` as the GE reads an `8888` texture
/// (`0xAABBGGRR`).
pub fn chunk_abgr(albedo: &[u32], out: &mut [u32]) {
    for (o, &c) in out.iter_mut().zip(albedo) {
        *o = list::abgr(c);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn argb_to_abgr_swaps_red_and_blue() {
        assert_eq!(super::list::abgr(0xff11_2233), 0xff33_2211);
        let mut out = [0u32; 2];
        super::chunk_abgr(&[0x8001_0203, 0xffff_0000], &mut out);
        assert_eq!(out, [0x8003_0201, 0xff00_00ff]);
    }
}
