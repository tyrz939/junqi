//! The mixer (PRESENTATION.md §5): music, sound effects and beds into one stereo stream through
//! the shared reverb, the volumes, a DC blocker and the limiter. It owns no device: the app's
//! audio callback calls [`Engine::render`], and `jane audio render` calls it into a file.

use std::collections::BTreeMap;

use crate::bed::{Bed, BedVoice};
use crate::dsp::{DcBlock, Limiter, Reverb, pan_gains};
use crate::model::Library;
use crate::patch::{self, PCM16, Stored};
use crate::seq::{Player, SongData};
use crate::voice::Prepared;

/// What the game asks of the engine. Indices, not names: the audio thread never looks a name up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    /// Change the music: `None` fades to silence. The same song again is nothing.
    Music { song: Option<usize>, fade_out_ms: f32, fade_in_ms: f32 },
    /// Play sound `id` at `gain`, panned (-1 to 1), with `send` to the reverb and at `rate`
    /// (1 as rendered).
    Sfx { id: usize, gain: f32, pan: f32, send: f32, rate: f32 },
    /// A bed to a level (0 to 1); it fades there.
    Bed { bed: Bed, level: f32 },
    /// Master, music and effects, 0 to 1.
    Volume { master: f32, music: f32, sfx: f32 },
    /// A new county: the music's choices are drawn from its seed from the next cue on.
    Seed(u32),
    /// The music and the beds to this share of their level (1 as set; the world held, less),
    /// over a quarter of a second. Effects are never ducked.
    Duck(f32),
}

/// A sound effect playing.
#[derive(Clone, Copy, Debug)]
struct SfxVoice {
    id: usize,
    variant: usize,
    pos: f64,
    rate: f64,
    gl: f32,
    gr: f32,
    send: f32,
}

/// The block size the engine mixes in.
pub const BLOCK: usize = 64;
/// At most this many effects at once; the oldest gives way.
const MAX_SFX: usize = 32;

#[derive(Debug)]
pub struct Engine {
    pub sr: f32,
    seed: u32,
    insts: Vec<Prepared>,
    songs: Vec<SongData>,
    /// Each effect's samples; `None` for a long one not yet wanted (`Engine::lazy`).
    sfx: Vec<Option<Stored>>,
    /// The long effects (bells, thunder, a spell's first learning) made on first want and let
    /// go when they have played (PLAY-PLAN.md §7), and the thread that makes them.
    loader: Option<Loader>,
    sfx_names: BTreeMap<String, usize>,
    song_names: BTreeMap<String, usize>,
    round: Vec<usize>,
    players: Vec<Player>,
    current: Option<usize>,
    voices: Vec<SfxVoice>,
    beds: Vec<BedVoice>,
    reverb: Reverb,
    limiter: Limiter,
    dc: [DcBlock; 2],
    vol: [f32; 3],
    vol_want: [f32; 3],
    duck: f32,
    duck_want: f32,
    now: u64,
    /// Log every note the next song plays (tests, `jane audio`).
    pub log_notes: bool,
    /// The log of the last song that played through and was let go.
    done_log: Option<Vec<crate::seq::NoteOn>>,
    bufs: Vec<Vec<f32>>,
}

/// The loudest the mix may be: a little under full scale, so no converter ever clips.
pub const CEILING: f32 = 0.93;

/// The seed every effect is rendered with (its variants differ by it, the county never).
const SFX_SEED: u32 = 0x5eed;

/// An effect whose longest variant runs past this many seconds is made when first wanted
/// ([`Engine::lazy`]): the bells, the thunder, a spell's first learning.
const LAZY_SECS: f32 = 3.0;

/// A play asked of an effect still being made, started when it arrives.
#[derive(Clone, Copy, Debug)]
struct Waiting {
    id: usize,
    gain: f32,
    pan: f32,
    send: f32,
    rate: f32,
}

/// The thread that renders a long effect when it is first wanted: asked by id, it answers with
/// the samples. The audio thread never renders and never waits; the sound starts when they come
/// (a tenth of a second or so after it was asked, at most).
#[derive(Debug)]
struct Loader {
    ask: std::sync::mpsc::Sender<usize>,
    made: std::sync::mpsc::Receiver<(usize, Stored)>,
    /// By effect: made on want, let go when played.
    lazy: Vec<bool>,
    asked: Vec<bool>,
    waiting: Vec<Waiting>,
}

impl Engine {
    /// [`Engine::new`] for the game (PLAY-PLAN.md §7): the long effects are not kept, but made on
    /// a thread of their own when first wanted and let go once they have played: some 11 MB of
    /// samples not held. Offline renders and tests keep [`Engine::new`], which has every effect.
    pub fn lazy(lib: &Library, sr: f32, seed: u32) -> Engine {
        let lazy: Vec<bool> = lib.sfx.iter().map(|p| patch::secs(p) > LAZY_SECS).collect();
        let mut e = Engine::with(lib, sr, seed, &lazy);
        let (ask, asked_rx) = std::sync::mpsc::channel::<usize>();
        let (made_tx, made) = std::sync::mpsc::channel();
        let patches = lib.sfx.clone();
        let spawned = std::thread::Builder::new().name("jane-sfx".into()).spawn(move || {
            for id in asked_rx {
                let Some(p) = patches.get(id) else { continue };
                if made_tx.send((id, Stored::of(patch::render(p, sr, SFX_SEED)))).is_err() {
                    break;
                }
            }
        });
        if spawned.is_err() {
            // No thread: every effect made now, as `new` would.
            return Engine::new(lib, sr, seed);
        }
        {
            let n = lazy.len();
            e.loader = Some(Loader { ask, made, lazy, asked: vec![false; n], waiting: Vec::with_capacity(8) });
        }
        e
    }

    /// The bytes the effects' samples hold now (`jane bench --mem`).
    pub fn sfx_bytes(&self) -> usize {
        self.sfx.iter().flatten().map(Stored::bytes).sum()
    }

    /// Takes in what the loader has made, starts what waited for it, and lets go of a long
    /// effect nobody is playing.
    fn load(&mut self) {
        let Some(l) = &mut self.loader else { return };
        let mut start = Vec::new();
        while let Ok((id, s)) = l.made.try_recv() {
            if let Some(slot) = self.sfx.get_mut(id) {
                *slot = Some(s);
            }
            l.asked[id] = false;
            l.waiting.retain(|w| {
                let go = w.id == id;
                if go {
                    start.push(*w);
                }
                !go
            });
        }
        for (id, s) in self.sfx.iter_mut().enumerate() {
            if l.lazy[id] && s.is_some() && !self.voices.iter().any(|v| v.id == id) && !start.iter().any(|w| w.id == id)
            {
                *s = None;
            }
        }
        for w in start {
            self.handle(Cmd::Sfx { id: w.id, gain: w.gain, pan: w.pan, send: w.send, rate: w.rate });
        }
    }
    /// Renders every patch and prepares every instrument and song of `lib` at `sr`; `seed` is
    /// the county's (it varies the music, never the sound effects).
    pub fn new(lib: &Library, sr: f32, seed: u32) -> Engine {
        Engine::with(lib, sr, seed, &[])
    }

    /// [`Engine::new`] with the effects `skip` says left unmade (`None`).
    fn with(lib: &Library, sr: f32, seed: u32, skip: &[bool]) -> Engine {
        let insts: Vec<Prepared> = lib.instruments.iter().cloned().map(Prepared::new).collect();
        let inst_names: BTreeMap<&str, usize> =
            lib.instruments.iter().enumerate().map(|(i, x)| (x.name.as_str(), i)).collect();
        let songs: Vec<SongData> =
            lib.songs.iter().map(|s| SongData::new(s, &|n: &str| inst_names.get(n).copied().unwrap_or(0))).collect();
        let sfx: Vec<Option<Stored>> = lib
            .sfx
            .iter()
            .enumerate()
            .map(|(i, p)| (!skip.get(i).copied().unwrap_or(false)).then(|| Stored::of(patch::render(p, sr, SFX_SEED))))
            .collect();
        let sfx_names = lib.sfx.iter().enumerate().map(|(i, p)| (p.name.clone(), i)).collect();
        let song_names = songs.iter().enumerate().map(|(i, s)| (s.name.clone(), i)).collect();
        let beds = Bed::ALL.iter().map(|&b| BedVoice::new(b, sr, seed)).collect();
        Engine {
            sr,
            seed,
            round: vec![0; sfx.len()],
            insts,
            songs,
            sfx,
            loader: None,
            sfx_names,
            song_names,
            players: Vec::new(),
            current: None,
            voices: Vec::with_capacity(MAX_SFX),
            beds,
            reverb: Reverb::new(sr, 2.4, 1.25),
            limiter: Limiter::new(sr, CEILING),
            dc: [DcBlock::new(sr), DcBlock::new(sr)],
            vol: [1.0; 3],
            vol_want: [1.0; 3],
            duck: 1.0,
            duck_want: 1.0,
            now: 0,
            log_notes: false,
            done_log: None,
            bufs: vec![vec![0.0; BLOCK]; 12],
        }
    }

    /// A new county: its music varies from here on.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn sfx_index(&self, name: &str) -> Option<usize> {
        self.sfx_names.get(name).copied()
    }

    pub fn song_index(&self, name: &str) -> Option<usize> {
        self.song_names.get(name).copied()
    }

    pub fn songs(&self) -> &[SongData] {
        &self.songs
    }

    /// Each effect's samples (`None`: a long one an [`Engine::lazy`] has not made yet).
    pub fn sfx(&self) -> &[Option<Stored>] {
        &self.sfx
    }

    /// The song playing (not fading out), if any.
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The notes the current song has played, when `log_notes` was on as it started.
    pub fn note_log(&self) -> Option<&[crate::seq::NoteOn]> {
        self.players.iter().rev().find(|p| p.target() > 0.0).and_then(|p| p.log.as_deref()).or(self.done_log.as_deref())
    }

    /// Whether the current song has played through (a song that does not loop).
    pub fn song_ended(&self) -> bool {
        self.players.iter().rev().find(|p| p.target() > 0.0).is_none_or(|p| p.ended)
    }

    pub fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Music { song, fade_out_ms, fade_in_ms } => {
                if song == self.current && song.is_some() {
                    return;
                }
                for p in &mut self.players {
                    if p.target() > 0.0 {
                        p.fade(0.0, fade_out_ms, self.sr);
                    }
                }
                self.current = song;
                if let Some(s) = song.filter(|&s| s < self.songs.len()) {
                    let mut p = Player::new(s, &self.songs[s], self.seed, self.now, self.sr);
                    if self.log_notes {
                        p.log = Some(Vec::new());
                    }
                    if fade_in_ms > 0.0 {
                        p.gain = 0.0;
                        p.fade(1.0, fade_in_ms, self.sr);
                    }
                    self.players.push(p);
                }
            }
            Cmd::Sfx { id, gain, pan, send, rate } => {
                if gain <= 0.0 {
                    return;
                }
                let Some(slot) = self.sfx.get(id) else { return };
                let Some(r) = slot else {
                    // A long effect not made yet: asked of the loader, and started when it comes.
                    if let Some(l) = &mut self.loader
                        && l.waiting.len() < l.waiting.capacity()
                    {
                        if !l.asked[id] {
                            l.asked[id] = l.ask.send(id).is_ok();
                        }
                        l.waiting.push(Waiting { id, gain, pan, send, rate });
                    }
                    return;
                };
                if self.voices.len() >= MAX_SFX {
                    // The one furthest through gives way.
                    if let Some((i, _)) = self.voices.iter().enumerate().max_by(|a, b| a.1.pos.total_cmp(&b.1.pos)) {
                        self.voices.swap_remove(i);
                    }
                }
                let variant = self.round[id] % r.variants.len();
                self.round[id] += 1;
                let (gl, gr) = pan_gains(pan);
                self.voices.push(SfxVoice {
                    id,
                    variant,
                    pos: 0.0,
                    rate: f64::from(rate.clamp(0.25, 4.0)),
                    gl: gl * gain,
                    gr: gr * gain,
                    send: send.max(r.send),
                });
            }
            Cmd::Bed { bed, level } => {
                if let Some(b) = self.beds.iter_mut().find(|b| b.bed == bed) {
                    b.target = level.clamp(0.0, 1.0);
                }
            }
            Cmd::Seed(seed) => self.seed = seed,
            Cmd::Duck(d) => self.duck_want = d.clamp(0.0, 1.0),
            Cmd::Volume { master, music, sfx } => {
                self.vol_want = [master.clamp(0.0, 1.0), music.clamp(0.0, 1.0), sfx.clamp(0.0, 1.0)];
            }
        }
    }

    /// Fills `out`, interleaved stereo.
    pub fn render(&mut self, out: &mut [f32]) {
        self.load();
        for chunk in out.chunks_mut(BLOCK * 2) {
            self.block(chunk);
        }
    }

    /// Renders `secs` of whatever is playing, interleaved stereo.
    pub fn render_secs(&mut self, secs: f32) -> Vec<f32> {
        let mut v = vec![0.0; (secs * self.sr) as usize * 2];
        self.render(&mut v);
        v
    }

    fn block(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        for b in &mut self.bufs {
            b[..n].fill(0.0);
        }
        let [ml, mr, msl, msr, fl, fr, fsl, fsr, al, ar, asl, asr] = &mut self.bufs[..] else { unreachable!() };
        let (ml, mr, msl, msr) = (&mut ml[..n], &mut mr[..n], &mut msl[..n], &mut msr[..n]);
        let (fl, fr, fsl, fsr) = (&mut fl[..n], &mut fr[..n], &mut fsl[..n], &mut fsr[..n]);
        let (al, ar, asl, asr) = (&mut al[..n], &mut ar[..n], &mut asl[..n], &mut asr[..n]);
        // The music.
        for p in &mut self.players {
            p.render(&self.songs[p.song], &self.insts, self.now, self.sr, [ml, mr], [msl, msr]);
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.finished() && p.log.is_some() && p.target() > 0.0) {
            self.done_log = p.log.take();
        }
        self.players.retain(|p| !p.finished());
        // The effects.
        let sfx = &self.sfx;
        self.voices.retain_mut(|v| {
            let Some(Some(s)) = sfx.get(v.id) else { return false };
            let buf = &s.variants[v.variant];
            for k in 0..n {
                let i = v.pos as usize;
                if i + 1 >= buf.len() {
                    return false;
                }
                let f = (v.pos - i as f64) as f32;
                let (a, b) = (f32::from(buf[i]) * PCM16, f32::from(buf[i + 1]) * PCM16);
                let s = a + (b - a) * f;
                fl[k] += s * v.gl;
                fr[k] += s * v.gr;
                fsl[k] += s * v.gl * v.send;
                fsr[k] += s * v.gr * v.send;
                v.pos += v.rate;
            }
            true
        });
        // The beds.
        for b in &mut self.beds {
            b.render(self.sr, [al, ar], [asl, asr]);
        }
        // Volumes move over a block, never jump.
        let prev = self.vol;
        let prev_duck = self.duck;
        for i in 0..3 {
            self.vol[i] += (self.vol_want[i] - self.vol[i]) * 0.05;
        }
        self.duck += (self.duck_want - self.duck) * (n as f32 / (0.25 * self.sr)).min(1.0);
        for k in 0..n {
            let t = k as f32 / n as f32;
            let v = |i: usize| prev[i] + (self.vol[i] - prev[i]) * t;
            let d = prev_duck + (self.duck - prev_duck) * t;
            let (vm, vmu, vs) = (v(0), v(1) * d, v(2));
            let va = vs * d;
            let (wl, wr) =
                self.reverb.run(msl[k] * vmu + fsl[k] * vs + asl[k] * va, msr[k] * vmu + fsr[k] * vs + asr[k] * va);
            let l = (ml[k] * vmu + fl[k] * vs + al[k] * va + wl) * vm;
            let r = (mr[k] * vmu + fr[k] * vs + ar[k] * va + wr) * vm;
            let (l, r) = (self.dc[0].run(l), self.dc[1].run(r));
            let (l, r) = self.limiter.run(l, r);
            out[2 * k] = l;
            out[2 * k + 1] = r;
        }
        self.now += n as u64;
    }
}

/// One instrument alone, dry: `notes` of (MIDI note, seconds held), one after another, each
/// given a second to ring. Mono. For `jane audio render inst:<name>` and the instrument tests.
pub fn audition(lib: &Library, inst: &str, notes: &[(i32, f32)], sr: f32) -> Option<Vec<f32>> {
    let prep = Prepared::new(lib.instruments.iter().find(|i| i.name == inst)?.clone());
    let total: f32 = notes.iter().map(|n| n.1).sum::<f32>() + 2.0;
    let len = (total * sr) as usize;
    let (mut l, mut r, mut sl, mut sr_) = (vec![0.0; len], vec![0.0; len], vec![0.0; len], vec![0.0; len]);
    let mut at = 0.0f32;
    for (k, &(midi, secs)) in notes.iter().enumerate() {
        let start = (at * sr) as usize;
        let mut v = crate::voice::Voice::new(
            &prep,
            0,
            0,
            k as u64,
            crate::dsp::midi_hz(midi as f32),
            0.9,
            0.0,
            0,
            Some((secs * sr) as u32),
            sr,
            k as u32 + 1,
        );
        v.render(&prep, sr, [&mut l[start..], &mut r[start..]], [&mut sl[start..], &mut sr_[start..]]);
        at += secs;
    }
    Some(l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lazy_engine_makes_a_long_effect_when_wanted_plays_it_and_lets_it_go() {
        let lib = crate::library();
        let mut e = Engine::lazy(lib, 48_000.0, 1);
        let id = e.sfx_index("bell_far").expect("a bell");
        assert!(e.sfx()[id].is_none(), "a long effect is not made up front");
        assert!(e.sfx_bytes() < Engine::new(lib, 48_000.0, 1).sfx_bytes() / 2);
        e.handle(Cmd::Sfx { id, gain: 1.0, pan: 0.0, send: 0.0, rate: 1.0 });
        // It comes from the loader's thread: render on until it has (two seconds at most).
        let mut heard = false;
        for _ in 0..200 {
            if crate::analysis::peak(&e.render_secs(0.01)) > 1e-3 {
                heard = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(heard, "the bell rang");
        let secs = patch::secs(&lib.sfx[id]);
        let _ = e.render_secs(secs + 0.1);
        let _ = e.render_secs(0.01);
        assert!(e.sfx()[id].is_none(), "played through, it is let go");
    }

    #[test]
    fn a_quiet_engine_is_silent_and_an_effect_is_heard_where_it_is_panned() {
        let lib = crate::library();
        let mut e = Engine::new(lib, 48_000.0, 1);
        let quiet = e.render_secs(0.5);
        assert!(crate::analysis::peak(&quiet) < 1e-6);
        let id = (0..e.sfx().len()).next().unwrap();
        e.handle(Cmd::Sfx { id, gain: 1.0, pan: -1.0, send: 0.0, rate: 1.0 });
        let v = e.render_secs(1.0);
        let l: Vec<f32> = v.iter().step_by(2).copied().collect();
        let r: Vec<f32> = v.iter().skip(1).step_by(2).copied().collect();
        // The right hears only the reverb's return.
        assert!(crate::analysis::rms(&l) > 3.0 * crate::analysis::rms(&r).max(1e-7), "hard left is left");
    }
}
