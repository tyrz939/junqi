//! The sound device (PRESENTATION.md §5): SDL2's audio at 48 kHz stereo, its callback running
//! `jane-audio`'s engine. The game's [`AudioBus`] is [`Sound`]: it turns the cue table's asks
//! into engine commands and sends them over a channel, so the audio thread never waits on the
//! game and the game never waits on it. With no device (a server, a machine with no sound) the
//! bus is silent and the game plays on.

// A device rate, a fade in milliseconds, a footstep's nudge: small numbers, exact in an f32.
#![allow(clippy::cast_precision_loss)]

use std::sync::mpsc::{Receiver, Sender, channel};

use jane_audio::{Cmd, Engine};
use jane_present::audio::{At, AudioBus, Bed, MusicCue, SfxKind, Volumes, fades, place};

/// The audio thread's half: the engine and the commands it has not yet heard.
struct Callback {
    engine: Engine,
    rx: Receiver<Cmd>,
}

impl sdl2::audio::AudioCallback for Callback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        while let Ok(c) = self.rx.try_recv() {
            self.engine.handle(c);
        }
        self.engine.render(out);
    }
}

/// The game's half: the bus.
pub struct Sound {
    device: Option<sdl2::audio::AudioDevice<Callback>>,
    tx: Option<Sender<Cmd>>,
    /// Each `SfxKind`'s patch, by its place in `SfxKind::ALL`.
    sfx: Vec<Option<usize>>,
    /// Each `Bed`'s bed, by its place in `Bed::ALL`.
    beds: Vec<Option<jane_audio::Bed>>,
    cue: Option<MusicCue>,
    held: bool,
    /// A lesson's hush: the music's and the beds' share, of 255.
    hush: u8,
    /// The share last sent to the engine.
    sent: u8,
}

impl std::fmt::Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound").field("open", &self.device.is_some()).field("cue", &self.cue).finish_non_exhaustive()
    }
}

/// Each sound effect's patch in the library, by name (the engine keeps the library's order).
fn tables() -> (Vec<Option<usize>>, Vec<Option<jane_audio::Bed>>) {
    let lib = jane_audio::library();
    let sfx = SfxKind::ALL.iter().map(|k| lib.sfx.iter().position(|p| p.name == k.name())).collect();
    let beds = Bed::ALL.iter().map(|b| jane_audio::Bed::from_name(b.name())).collect();
    (sfx, beds)
}

fn song(cue: MusicCue) -> Option<usize> {
    let name = cue.song()?;
    jane_audio::library().songs.iter().position(|s| s.name == name)
}

impl Sound {
    /// No device: every ask is dropped.
    pub fn silent() -> Sound {
        let (sfx, beds) = tables();
        Sound { device: None, tx: None, sfx, beds, cue: None, held: false, hush: 255, sent: 255 }
    }

    /// Opens the default playback device at 48 kHz stereo, or falls back to silence with a line
    /// on stdout. `seed` is the county's (the music varies by it).
    pub fn open(sdl: &sdl2::Sdl, vol: Volumes, seed: u32) -> Sound {
        let mut s = Sound::silent();
        let audio = match sdl.audio() {
            Ok(a) => a,
            Err(e) => {
                println!("jane-app: no audio ({e}); silent");
                return s;
            }
        };
        let desired = sdl2::audio::AudioSpecDesired {
            freq: Some(jane_audio::RATE as i32),
            channels: Some(2),
            // About 21 ms a buffer at 48 kHz: short enough that a footstep lands on its step.
            samples: Some(1024),
        };
        let (tx, rx) = channel();
        let t0 = std::time::Instant::now();
        let opened = audio.open_playback(None, &desired, |spec| {
            let mut engine = Engine::new(jane_audio::library(), spec.freq as f32, seed);
            let (m, mu, fx) = vol.gains();
            engine.handle(Cmd::Volume { master: m, music: mu, sfx: fx });
            Callback { engine, rx }
        });
        match opened {
            Ok(d) => {
                let spec = d.spec();
                println!(
                    "jane-app: audio {} Hz, {} channel(s), sound made in {} ms",
                    spec.freq,
                    spec.channels,
                    t0.elapsed().as_millis()
                );
                d.resume();
                s.device = Some(d);
                s.tx = Some(tx);
            }
            Err(e) => println!("jane-app: no audio device ({e}); silent"),
        }
        s
    }

    fn send(&self, c: Cmd) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    pub fn set_volume(&mut self, v: Volumes) {
        let (master, music, sfx) = v.gains();
        self.send(Cmd::Volume { master, music, sfx });
    }

    /// The world held still (alone, a menu up): the music and the beds step back until it goes
    /// on. With company the world is never held, so this is never set.
    pub fn set_held(&mut self, held: bool) {
        self.held = held;
        self.send_duck();
    }

    /// The deeper of the held world's step back and a lesson's hush, sent when it moves.
    fn send_duck(&mut self) {
        // Of 255: the held world's 0.35, or the hush's share, whichever is deeper.
        let share = if self.held { 89 } else { 255 }.min(self.hush);
        if share.abs_diff(self.sent) >= 2 || (share == 255 && self.sent != 255) {
            self.sent = share;
            self.send(Cmd::Duck(f32::from(share) / 255.0));
        }
    }

    /// A new county: its music from the next cue.
    pub fn set_seed(&mut self, seed: u32) {
        self.send(Cmd::Seed(seed));
    }
}

impl AudioBus for Sound {
    fn music(&mut self, cue: MusicCue) {
        let (out, fade_in) = fades(self.cue, cue);
        self.cue = Some(cue);
        self.send(Cmd::Music { song: song(cue), fade_out_ms: f32::from(out), fade_in_ms: f32::from(fade_in) });
    }

    fn sfx(&mut self, kind: SfxKind, at: At, listener: At) {
        let Some(id) = SfxKind::ALL.iter().position(|k| *k == kind).and_then(|i| self.sfx[i]) else { return };
        let Some(p) = place(at, listener) else { return };
        // Footsteps a few cents apart, so a walk is not a machine.
        let rate =
            if kind.name().starts_with("step_") { 0.97 + 0.06 * (at.0.0.rem_euclid(97) as f32 / 97.0) } else { 1.0 };
        self.send(Cmd::Sfx { id, gain: p.gain, pan: p.pan, send: p.send, rate });
    }

    /// Her growth heard (PLAN.md §2.6): at the end of the game her casts and blows are half as
    /// loud again, a little wetter, and some two semitones deeper than at New Game.
    fn sfx_with(&mut self, kind: SfxKind, at: At, listener: At, might: u16) {
        let Some(id) = SfxKind::ALL.iter().position(|k| *k == kind).and_then(|i| self.sfx[i]) else { return };
        let Some(p) = place(at, listener) else { return };
        let grown = f32::from(might.clamp(256, 512) - 256) / 256.0;
        let gain = p.gain * (1.0 + 0.5 * grown);
        let send = p.send + 0.1 * grown;
        self.send(Cmd::Sfx { id, gain, pan: p.pan, send, rate: 1.0 - 0.11 * grown });
    }

    fn bed(&mut self, bed: Bed, level: u8) {
        let Some(b) = Bed::ALL.iter().position(|x| *x == bed).and_then(|i| self.beds[i]) else { return };
        self.send(Cmd::Bed { bed: b, level: f32::from(level) / 255.0 });
    }

    fn tick(&mut self) {}

    fn duck(&mut self, share: u8) {
        self.hush = share;
        self.send_duck();
    }
}

/// The little sound a menu makes for a pick: a screen opening or closing, a choice made.
pub fn intent_sound(i: &jane_present::ui::core::AppIntent) -> Option<SfxKind> {
    use jane_present::ui::core::AppIntent as I;
    Some(match i {
        I::Pause | I::Controls | I::LoadMenu | I::SaveMenu | I::OpenWindow(_) | I::HostMenu | I::JoinMenu => {
            SfxKind::UiOpen
        }
        I::Back | I::Resume | I::CloseWindow => SfxKind::UiClose,
        I::NewGame { .. }
        | I::Continue
        | I::Load(_)
        | I::ToTitle
        | I::Assist(_)
        | I::ResetBindings
        | I::Host(_)
        | I::Join(_)
        | I::OpenToLan => SfxKind::UiConfirm,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_the_presentation_asks_for_is_in_the_library() {
        let (sfx, beds) = tables();
        for (k, id) in SfxKind::ALL.iter().zip(&sfx) {
            assert!(id.is_some(), "no patch {} in data/audio/sfx.json", k.name());
        }
        for (b, id) in Bed::ALL.iter().zip(&beds) {
            assert!(id.is_some(), "no bed {}", b.name());
        }
        for cue in MusicCue::all() {
            if let Some(name) = cue.song() {
                assert!(song(cue).is_some(), "no song {name} in data/audio/songs");
            }
        }
    }

    #[test]
    fn a_silent_bus_takes_every_ask() {
        let mut s = Sound::silent();
        let me = (jane_core::Fx(0), jane_core::Fx(0));
        s.music(MusicCue::Title);
        s.sfx(SfxKind::BellFar, me, me);
        s.bed(Bed::Rain, 200);
        s.set_volume(Volumes::default());
        s.tick();
    }
}
