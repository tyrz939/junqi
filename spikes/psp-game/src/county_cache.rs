//! The county kept on the Memory Stick (PORT.md §13.13): `jane_sim::zone_cache` over sceIo in
//! `ms0:/PSP/SAVEDATA/JANE00001/cache/`, one file a zone (`<seed>-<zone>.bp`). Continue, Load and a
//! New Game on a seed built before read the zones back (seconds) instead of building them (a
//! minute); a zone not kept, or refused, is built and written as soon as it is built, a few KB at
//! a time. Called from the builder thread only.

use alloc::vec::Vec;
use core::fmt::Write as _;

use alloc::sync::Arc;

use jane_core::{Blueprint, ZoneId};
use jane_sim::blueprints::{BuildError, ZoneSource};
use jane_sim::zone_cache::{CacheSource, Sink, Store, ZoneCache};
use jane_sim::Blueprints;
use psp::sys;

use super::{cpath, now_us, File, Hold, Line, HEAP};

const DIR: &str = "ms0:/PSP/SAVEDATA/JANE00001/cache/";

/// The cache's directory on the stick (made once by [`Stick::new`]; [`stick`] after).
pub struct Stick;

/// The stick's cache, its directory made already.
fn stick() -> Stick {
    Stick
}

impl Stick {
    fn new() -> Stick {
        for d in ["ms0:/PSP", "ms0:/PSP/SAVEDATA", "ms0:/PSP/SAVEDATA/JANE00001", "ms0:/PSP/SAVEDATA/JANE00001/cache"] {
            let z = cpath(d);
            // SAFETY: a NUL-terminated path; an existing directory is an error we ignore.
            unsafe { sys::sceIoMkdir(z.as_ptr(), 0o777) };
        }
        Stick
    }
}

/// An open file, written through.
struct Out(sys::SceUid);

impl Sink for Out {
    fn put(&mut self, bytes: &[u8]) -> bool {
        // SAFETY: an open descriptor; `bytes` outlives the call.
        let n = unsafe { sys::sceIoWrite(self.0, bytes.as_ptr().cast(), bytes.len()) };
        n >= 0 && n as usize == bytes.len()
    }
}

impl Store for Stick {
    fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        File::open(&alloc::format!("{DIR}{name}")).and_then(|f| f.read_all())
    }

    fn write(&mut self, name: &str, fill: &mut dyn FnMut(&mut dyn Sink) -> bool) -> bool {
        let (path, tmp) = (alloc::format!("{DIR}{name}"), alloc::format!("{DIR}{name}.tmp"));
        let (zp, zt) = (cpath(&path), cpath(&tmp));
        // SAFETY: NUL-terminated paths; the descriptor is closed before the rename.
        unsafe {
            let fd = sys::sceIoOpen(
                zt.as_ptr(),
                sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
                0o777,
            );
            if fd.0 < 0 {
                return false;
            }
            let ok = fill(&mut Out(fd));
            let closed = sys::sceIoClose(fd) >= 0;
            if !(ok && closed) {
                sys::sceIoRemove(zt.as_ptr());
                return false;
            }
            sys::sceIoRemove(zp.as_ptr());
            sys::sceIoRename(zt.as_ptr(), zp.as_ptr()) >= 0
        }
    }

    fn remove(&mut self, name: &str) {
        let z = cpath(&alloc::format!("{DIR}{name}"));
        // SAFETY: a NUL-terminated path; a missing file is an error we ignore.
        unsafe { sys::sceIoRemove(z.as_ptr()) };
    }
}

/// `seed`'s zones on demand (PORT.md §13.3): the county now, read from the stick where kept and
/// built (and kept) where not; every other zone the same way when the sim first needs it, or
/// ahead of her ([`zone`]). The same blueprints `Blueprints::build_packed_with` builds. Logs what
/// was read and built, the time, and the heap's peak during it (the program's own peak is kept
/// as it was, the larger).
pub fn blueprints(seed: u32, report: jane_world::Report<'_>) -> Result<Blueprints, BuildError> {
    // The counts are the allocator's, kept under its hold (the game thread allocates too).
    let (t, peak_before) = {
        let _hold = Hold::new();
        let before = HEAP.peak.get();
        HEAP.peak.set(HEAP.live.get());
        (now_us(), before)
    };
    let source = Arc::new(CacheSource::new(stick as fn() -> Stick));
    let r = ZoneCache::new(Stick::new()).on_demand(seed, &source, report);
    let peak = {
        let _hold = Hold::new();
        let peak = HEAP.peak.get();
        HEAP.peak.set(peak.max(peak_before));
        peak
    };
    match &r {
        Ok((_, tally)) => say!(
            "GAME county cache seed={seed} read={} built={} unwritten={} us={} peak={peak}",
            tally.read,
            tally.built,
            tally.unwritten,
            now_us().wrapping_sub(t),
        ),
        Err(e) => say!("GAME county cache seed={seed} build error {e}"),
    }
    r.map(|(b, _)| b)
}

/// Zone `z` of `seed` read from the stick, else built and kept: for the loader building ahead of
/// her at a door (the builder thread; never the game's). Logs whether it was read, and the time.
pub fn zone(seed: u32, z: ZoneId) -> Result<Blueprint, BuildError> {
    let t = now_us();
    let source = CacheSource::new(stick as fn() -> Stick);
    let r = source.zone(seed, z, &mut |_| {});
    let tally = source.tally();
    say!(
        "GAME ahead zone={} read={} built={} unwritten={} us={}",
        z.name(),
        tally.read,
        tally.built,
        tally.unwritten,
        now_us().wrapping_sub(t)
    );
    r
}

/// Whether `seed`'s county is kept (a guess for the loading screen's words; not read or checked).
pub fn has(seed: u32) -> bool {
    ZoneCache::new(Stick::new()).has_seed(seed)
}
