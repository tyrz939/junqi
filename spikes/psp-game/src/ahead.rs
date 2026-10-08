//! Zones built ahead of her (PORT.md §13.3, phase 4). Built on demand, a zone her step through a
//! door would build stalls that step (the Burial: 1.3 s emulated built, about 60 ms read back from
//! the stick's cache). Every half second of play this asks the sim which zones the doors near her
//! lead to (`Sim::zones_ahead`) and hands the nearest not held to the painter's thread, which
//! reads it from the cache or builds it (`county_cache::zone`) after its paint job, in the time the
//! game waits for the vblank; the game hands it in (`Sim::offer_blueprint`) when it is back.
//!
//! No thread or stack of its own: a stack made in play is RAM the town may not have (a load into
//! the square leaves about 130 KB in one block). One zone ahead at most, and only while the
//! kernel's largest free block keeps [`RESERVE`] for the renderer. A zone she reaches first is
//! built by her step as before. Invisible to the sim: an offered zone is the zone her step would
//! have built.

use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};

use alloc::boxed::Box;
use alloc::sync::Arc;

use jane_core::{Blueprint, ZoneId};
use jane_sim::{Seat, Sim};
use psp::sys;

use super::{county_cache, Line, SEMA};

/// How near a door must be, in cells (a few strides).
const NEAR: i32 = 14;
/// Ask every this many play ticks.
const EVERY: u32 = 30;
/// The kernel's largest free block a build ahead leaves alone: the renderer's and the presenter's
/// big buffers come from it, and a build's own big ones (64 KB and over) would take from it.
const RESERVE: usize = 512 * 1024;

/// The job: the seed and the zone's index; `ASKED` while it waits or runs.
static SEED: AtomicU32 = AtomicU32::new(0);
static ZONE: AtomicU32 = AtomicU32::new(0);
static ASKED: AtomicBool = AtomicBool::new(false);
static DONE: AtomicPtr<Blueprint> = AtomicPtr::new(core::ptr::null_mut());

/// On the painter's thread, after its paint job: the zone asked for, if one is.
pub fn work() {
    if !ASKED.load(Ordering::Acquire) {
        return;
    }
    let seed = SEED.load(Ordering::Acquire);
    if let Some(z) = ZoneId::ALL.get(ZONE.load(Ordering::Acquire) as usize).copied() {
        if let Ok(bp) = county_cache::zone(seed, z) {
            DONE.store(Box::into_raw(Box::new(bp)), Ordering::Release);
        }
    }
    ASKED.store(false, Ordering::Release);
}

/// Once a play tick, after the step: hands in a zone built ahead, and asks for the next.
pub fn poll(sim: &mut Sim, play_tick: u32) {
    let done = DONE.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if !done.is_null() {
        // SAFETY: the painter stored it and no longer touches it.
        let bp = *unsafe { Box::from_raw(done) };
        let z = bp.zone;
        let held = sim.blueprints().held_now(z).is_some();
        sim.offer_blueprint(Arc::new(bp));
        say!("GAME ahead offered zone={}{}", z.name(), if held { " (held already)" } else { "" });
    }
    if play_tick % EVERY != 0 || ASKED.load(Ordering::Acquire) {
        return;
    }
    let Some(her) = sim.state().player(Seat(0)).map(|p| p.zone) else { return };
    // One ahead at most: nothing held but the county and her zone.
    let others = ZoneId::ALL
        .iter()
        .filter(|&&z| z != ZoneId::County && z != her && sim.blueprints().held_now(z).is_some())
        .count();
    if others > 0 {
        return;
    }
    let Some(&z) = sim.zones_ahead(Seat(0), NEAR).first() else { return };
    // SAFETY: a query of the kernel's free memory.
    let free = unsafe { sys::sceKernelMaxFreeMemSize() };
    if free < RESERVE {
        if play_tick % (EVERY * 20) == 0 {
            say!("GAME ahead zone={} waits: maxfree={free}", z.name());
        }
        return;
    }
    SEED.store(sim.blueprints().seed(), Ordering::Release);
    ZONE.store(z.index() as u32, Ordering::Release);
    ASKED.store(true, Ordering::Release);
    say!("GAME ahead zone={} asked maxfree={free}", z.name());
    // SAFETY: the semaphore the painter waits on.
    unsafe { sys::sceKernelSignalSema(sys::SceUid(SEMA.load(Ordering::Acquire)), 1) };
}
