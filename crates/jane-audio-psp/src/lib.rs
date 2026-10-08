//! The PSP's sound (PORT.md §13.4): the PC's cue table (`jane_present::audio::Soundtrack`) asks
//! a bus for music, placed effects and beds, exactly as on PC; this bus turns each ask into a
//! `jane_audio::tracker::Cmd` in integers and hands it to the audio thread through a lock-free
//! ring, where the tracker mixer plays the baked module (`jane-psp.jau`).
//!
//! - [`Ring`]: the commands from the game's thread to the audio thread, single producer and
//!   single consumer, atomics only (safe code, no lock, never waits);
//! - [`Bus`]: the cue table's asks as commands (the PC's `Sound` in integers: the placing, the
//!   footsteps' few cents, her growth heard, the duck), and the far effects loaded on want;
//! - [`Sound`]: the cue table and the bus, ticked with the game;
//! - `psp` (PSP only): the audio thread on sceAudio, the module read from the Memory Stick, the
//!   capture a scripted run writes beside the program.
//!
//! `no_std` plus `alloc` without `std`; integers on the console path.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use jane_audio::tracker::{Cmd, Header};
use jane_present::audio::{At, AudioBus, Bed, MusicCue, SfxKind, Soundtrack, Volumes, fades, place};

#[cfg(target_os = "psp")]
#[allow(unsafe_code)]
pub mod psp;

// ---------------------------------------------------------------- the ring

/// Commands waiting at most (a tick asks a handful; the audio thread drains it every block).
pub const RING: usize = 256;

/// A single-producer, single-consumer ring of commands, each packed into four 32-bit words (the
/// PSP's MIPS has no 64-bit atomics).
#[derive(Debug)]
pub struct Ring {
    slots: [[AtomicU32; 4]; RING],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl Default for Ring {
    fn default() -> Ring {
        Ring::new()
    }
}

impl Ring {
    pub const fn new() -> Ring {
        Ring {
            slots: [const { [const { AtomicU32::new(0) }; 4] }; RING],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// The game's side: false when full (the command is dropped; the audio thread is behind).
    pub fn push(&self, c: Cmd) -> bool {
        let h = self.head.load(Ordering::Relaxed);
        if h.wrapping_sub(self.tail.load(Ordering::Acquire)) >= RING {
            return false;
        }
        let (a, b) = pack(c);
        let s = &self.slots[h % RING];
        for (w, v) in s.iter().zip([a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32]) {
            w.store(v, Ordering::Relaxed);
        }
        self.head.store(h.wrapping_add(1), Ordering::Release);
        true
    }

    /// The audio thread's side.
    pub fn pop(&self) -> Option<Cmd> {
        let t = self.tail.load(Ordering::Relaxed);
        if t == self.head.load(Ordering::Acquire) {
            return None;
        }
        let s = &self.slots[t % RING];
        let w = |k: usize| u64::from(s[k].load(Ordering::Relaxed));
        let c = unpack(w(0) | w(1) << 32, w(2) | w(3) << 32);
        self.tail.store(t.wrapping_add(1), Ordering::Release);
        c
    }
}

pub use jane_audio::tracker::{pack, unpack};

// ---------------------------------------------------------------- the bus

/// Where the commands go and how a far effect is fetched: the audio thread's ring and the
/// Memory Stick on a PSP, a mixer in hand in the tests.
pub trait Host {
    fn send(&mut self, c: Cmd);
    /// Puts far sample `sample` (its frames `len` bytes from byte `at` of the module's file) in
    /// the mixer's slot before it is asked to play; false if it could not.
    fn load_far(&mut self, sample: u16, at: usize, len: usize) -> bool;
}

/// A placing in Q12: gain, pan, send (the PC's `place`, in integers either way).
fn placed(at: At, listener: At) -> Option<(i32, i32, i32)> {
    let p = place(at, listener)?;
    #[cfg(feature = "std")]
    {
        let q = |x: f32| (x * 4096.0).round() as i32;
        Some((q(p.gain), q(p.pan), q(p.send)))
    }
    #[cfg(not(feature = "std"))]
    {
        Some((p.gain, p.pan, p.send))
    }
}

/// The volumes as Q12 gains.
fn gains(v: Volumes) -> (u16, u16, u16) {
    let (m, mu, fx) = v.gains();
    #[cfg(feature = "std")]
    {
        let q = |x: f32| (x * 4096.0).round().clamp(0.0, 4096.0) as u16;
        (q(m), q(mu), q(fx))
    }
    #[cfg(not(feature = "std"))]
    {
        let q = |x: i32| x.clamp(0, 4096) as u16;
        (q(m), q(mu), q(fx))
    }
}

/// The cue table's asks as tracker commands: the PC's `Sound` (jane-app) in integers.
#[derive(Debug)]
pub struct Bus<H: Host> {
    pub host: H,
    /// Each `SfxKind`'s effect in the module, by its place in `SfxKind::ALL`.
    sfx: Vec<Option<u16>>,
    /// Each `Bed`'s loop, by its place in `Bed::ALL`.
    beds: Vec<Option<u8>>,
    songs: Vec<String>,
    /// Per module effect: its far sample, where its frames are in the file and how many bytes.
    far: Vec<Option<(u16, usize, usize)>>,
    loaded: Option<u16>,
    cue: Option<MusicCue>,
    held: bool,
    hush: u8,
    sent: u8,
}

impl<H: Host> Bus<H> {
    /// A bus over a module's header; `far_at` is where its far frames start in the file.
    pub fn new(host: H, head: &Header, far_at: usize) -> Bus<H> {
        let sfx =
            SfxKind::ALL.iter().map(|k| head.sfx.iter().position(|s| s.name == k.name()).map(|i| i as u16)).collect();
        let beds =
            Bed::ALL.iter().map(|b| head.beds.iter().position(|x| x.name == b.name()).map(|i| i as u8)).collect();
        let far = head
            .sfx
            .iter()
            .map(|s| {
                let x = &head.samples[usize::from(s.sample)];
                x.far.then(|| (s.sample, far_at + x.at as usize, jane_audio::tracker::Bank::sample_bytes(x)))
            })
            .collect();
        Bus {
            host,
            sfx,
            beds,
            songs: head.songs.iter().map(|s| s.name.clone()).collect(),
            far,
            loaded: None,
            cue: None,
            held: false,
            hush: 255,
            sent: 255,
        }
    }

    pub fn set_volume(&mut self, v: Volumes) {
        let (master, music, sfx) = gains(v);
        self.host.send(Cmd::Volume { master, music, sfx });
    }

    /// The world held still (alone, a menu up): the music and the beds step back.
    pub fn set_held(&mut self, held: bool) {
        self.held = held;
        self.send_duck();
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.host.send(Cmd::Seed(seed));
    }

    fn send_duck(&mut self) {
        let share = if self.held { 89 } else { 255 }.min(self.hush);
        if share.abs_diff(self.sent) >= 2 || (share == 255 && self.sent != 255) {
            self.sent = share;
            self.host.send(Cmd::Duck(share));
        }
    }

    fn play(&mut self, kind: SfxKind, gain: i32, pan: i32, send: i32, rate: u32) {
        let Some(id) = SfxKind::ALL.iter().position(|k| *k == kind).and_then(|i| self.sfx[i]) else { return };
        if let Some(Some((sample, at, len))) = self.far.get(usize::from(id)).copied() {
            if self.loaded != Some(sample) {
                if !self.host.load_far(sample, at, len) {
                    return;
                }
                self.loaded = Some(sample);
            }
        }
        self.host.send(Cmd::Sfx {
            id,
            gain: gain.clamp(0, 16_383) as u16,
            pan: pan.clamp(-4096, 4096) as i16,
            send: send.clamp(0, 4096) as u16,
            rate,
        });
    }
}

impl<H: Host> AudioBus for Bus<H> {
    fn music(&mut self, cue: MusicCue) {
        let (out, fade_in) = fades(self.cue, cue);
        self.cue = Some(cue);
        let song = cue.song().and_then(|n| self.songs.iter().position(|s| s == n)).map(|i| i as u16);
        self.host.send(Cmd::Music { song, fade_out_ms: out, fade_in_ms: fade_in });
    }

    fn sfx(&mut self, kind: SfxKind, at: At, listener: At) {
        let Some((gain, pan, send)) = placed(at, listener) else { return };
        // Footsteps a few cents apart (0.97 to 1.03), so a walk is not a machine.
        let rate =
            if kind.name().starts_with("step_") { 63_570 + 3_932 * at.0.0.rem_euclid(97) as u32 / 97 } else { 65_536 };
        self.play(kind, gain, pan, send, rate);
    }

    /// Her growth heard (PLAN.md §2.6): half as loud again, a little wetter and some two
    /// semitones deeper at the end of the game.
    fn sfx_with(&mut self, kind: SfxKind, at: At, listener: At, might: u16) {
        let Some((gain, pan, send)) = placed(at, listener) else { return };
        let grown = i32::from(might.clamp(256, 512) - 256);
        let gain = gain * (512 + grown) / 512;
        let send = send + 410 * grown / 256;
        let rate = (65_536 - 7_209 * grown / 256) as u32;
        self.play(kind, gain, pan, send, rate);
    }

    fn bed(&mut self, bed: Bed, level: u8) {
        let Some(b) = Bed::ALL.iter().position(|x| *x == bed).and_then(|i| self.beds[i]) else { return };
        self.host.send(Cmd::Bed { bed: b, level });
    }

    fn tick(&mut self) {}

    fn duck(&mut self, share: u8) {
        self.hush = share;
        self.send_duck();
    }
}

// ---------------------------------------------------------------- the game's side

/// The cue table and its bus: what the game ticks.
#[derive(Debug)]
pub struct Sound<H: Host> {
    pub track: Soundtrack,
    pub bus: Bus<H>,
}

impl<H: Host> Sound<H> {
    pub fn new(bus: Bus<H>) -> Sound<H> {
        Sound { track: Soundtrack::new(), bus }
    }

    /// One tick of play: this seat's view and the tick's events, and the presenter's lesson
    /// (its cue and its hush).
    pub fn tick(&mut self, view: &jane_sim::View<'_>, events: &[jane_sim::Event], present: &jane_present::Present) {
        self.track.tick(view, events, &mut self.bus);
        self.track.lesson(present.lessons(), &mut self.bus);
    }

    /// The title or a loading screen: the theme.
    pub fn title(&mut self) {
        self.track.title(&mut self.bus);
    }

    /// A sound of the UI.
    pub fn ui(&mut self, kind: SfxKind) {
        self.track.ui(kind, &mut self.bus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_rides_the_ring_unchanged() {
        let r = Ring::new();
        let cmds = [
            Cmd::Music { song: Some(7), fade_out_ms: 700, fade_in_ms: 300 },
            Cmd::Music { song: None, fade_out_ms: 2500, fade_in_ms: 0 },
            Cmd::Sfx { id: 41, gain: 4000, pan: -3482, send: 1434, rate: 66_000 },
            Cmd::Bed { bed: 9, level: 200 },
            Cmd::Volume { master: 2621, music: 2007, sfx: 2621 },
            Cmd::Seed(0xdead_beef),
            Cmd::Duck(89),
        ];
        for _ in 0..100 {
            for c in cmds {
                assert!(r.push(c));
            }
            for c in cmds {
                assert_eq!(r.pop(), Some(c));
            }
        }
        assert_eq!(r.pop(), None);
        for _ in 0..RING {
            assert!(r.push(Cmd::Duck(1)));
        }
        assert!(!r.push(Cmd::Duck(2)), "a full ring drops, never waits");
    }
}
