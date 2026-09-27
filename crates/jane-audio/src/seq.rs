//! The music (PRESENTATION.md §5): a sequencer that steps on the game's tick (a step is a whole
//! number of ticks, a tick 1/60 s), reading songs from `data/audio/songs`. It is deterministic:
//! the county's seed picks the form, the patterns and the instruments, and each loop draws its
//! small changes (a note that may or may not sound, a drift's wanderings) from the seed and the
//! loop's number, so the same county always plays the same evening.

use crate::dsp::Rng;
use crate::model::{Mode, Song, TrackKind};
use crate::pattern::{self, Chord, Step};
use crate::voice::{Prepared, Voice};

/// A track ready to play.
#[derive(Clone, Debug)]
pub struct TrackData {
    pub name: String,
    pub kind: TrackKind,
    /// The instrument and its alternatives, by index into the engine's instruments.
    pub insts: Vec<usize>,
    pub octave: i32,
    pub vel: f32,
    pub pan: f32,
    pub voices: u8,
    pub gate: f32,
    pub drift: Option<(Vec<i32>, [u32; 2], [u32; 2])>,
}

/// A section ready to play.
#[derive(Clone, Debug)]
pub struct SecData {
    pub name: String,
    pub bars: u32,
    /// Two a bar.
    pub chords: Vec<Chord>,
    /// Per track: its patterns (empty: the track rests here).
    pub pats: Vec<Vec<Vec<Step>>>,
}

/// A song ready to play.
#[derive(Clone, Debug)]
pub struct SongData {
    pub name: String,
    pub mood: String,
    pub key: String,
    pub tonic: i32,
    pub mode: Mode,
    pub step_ticks: u32,
    pub bar: u32,
    pub gain: f32,
    pub looped: bool,
    pub humanize_ms: f32,
    pub tracks: Vec<TrackData>,
    pub sections: Vec<SecData>,
    pub forms: Vec<Vec<usize>>,
}

impl SongData {
    /// Compiles a checked song against the engine's instrument list.
    pub fn new(s: &Song, inst_index: &dyn Fn(&str) -> usize) -> SongData {
        let tracks: Vec<TrackData> = s
            .tracks
            .iter()
            .map(|t| TrackData {
                name: t.name.clone(),
                kind: t.kind,
                insts: std::iter::once(&t.inst).chain(&t.alt).map(|n| inst_index(n)).collect(),
                octave: t.octave,
                vel: t.vel,
                pan: t.pan,
                voices: t.voices,
                gate: t.gate,
                drift: t.drift.as_ref().map(|d| (pattern::degrees(&d.pool).unwrap_or_default(), d.gap, d.len)),
            })
            .collect();
        let names: Vec<&String> = s.sections.keys().collect();
        let sections = s
            .sections
            .iter()
            .map(|(name, sec)| SecData {
                name: name.clone(),
                bars: sec.bars,
                chords: pattern::chords(&sec.chords, s.mode).unwrap_or_default(),
                pats: tracks
                    .iter()
                    .map(|t| {
                        sec.play.get(&t.name).map_or_else(Vec::new, |p| {
                            p.alts().into_iter().map(|a| pattern::parse(a, t.kind).unwrap_or_default()).collect()
                        })
                    })
                    .collect(),
            })
            .collect();
        let forms = s
            .forms
            .iter()
            .map(|f| f.split_whitespace().filter_map(|n| names.iter().position(|x| x.as_str() == n)).collect())
            .collect();
        SongData {
            name: s.name.clone(),
            mood: s.mood.clone(),
            key: s.key.clone(),
            tonic: crate::model::pitch_class(&s.key).unwrap_or(0),
            mode: s.mode,
            step_ticks: s.step_ticks,
            bar: s.bar,
            gain: s.gain,
            looped: s.looped,
            humanize_ms: s.humanize_ms,
            tracks,
            sections,
            forms,
        }
    }

    /// Steps in one pass of form `f`.
    pub fn form_steps(&self, f: usize) -> u32 {
        self.forms[f].iter().map(|&s| self.sections[s].bars * self.bar).sum()
    }
}

/// A note the sequencer played: what the tests check the key against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteOn {
    pub track: u16,
    pub midi: i32,
    /// The pattern asked for it outside the scale (`#`, `b`, a forced third, a borrowed root).
    pub chromatic: bool,
    /// Samples from the song's start.
    pub at: u64,
}

/// One song playing.
#[derive(Clone, Debug)]
pub struct Player {
    pub song: usize,
    seed: u32,
    pub loop_n: u32,
    form: usize,
    form_pos: usize,
    step_in_sec: u32,
    /// Per section, per track: which pattern.
    choice: Vec<Vec<usize>>,
    /// Per track: which instrument.
    inst: Vec<usize>,
    step_len: u64,
    next_at: u64,
    started: u64,
    pub gain: f32,
    target: f32,
    fade_step: f32,
    voices: Vec<Voice>,
    drift_wait: Vec<u32>,
    pub ended: bool,
    rng: Rng,
    born: u64,
    /// Filled when asked (tests and `jane audio`): every note played.
    pub log: Option<Vec<NoteOn>>,
    buf: [Vec<f32>; 4],
}

impl Player {
    pub fn new(song: usize, data: &SongData, seed: u32, now: u64, sr: f32) -> Player {
        let mut county = Rng::new(seed ^ crate::patch::hash(&data.name));
        let inst = data.tracks.iter().map(|t| t.insts[county.range(0, t.insts.len() as u32 - 1) as usize]).collect();
        let tick = (sr / 60.0).round() as u64;
        let mut p = Player {
            song,
            seed,
            loop_n: 0,
            form: 0,
            form_pos: 0,
            step_in_sec: 0,
            choice: Vec::new(),
            inst,
            step_len: tick * u64::from(data.step_ticks),
            next_at: now,
            started: now,
            gain: 1.0,
            target: 1.0,
            fade_step: 0.0,
            voices: Vec::with_capacity(64),
            drift_wait: vec![0; data.tracks.len()],
            ended: false,
            rng: county,
            born: 0,
            log: None,
            buf: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        };
        p.pick(data);
        p
    }

    /// The loop's draws: its form and patterns, from the seed and the loop's number.
    fn pick(&mut self, data: &SongData) {
        let mut r = Rng::new(self.seed ^ crate::patch::hash(&data.name) ^ self.loop_n.wrapping_mul(0x2545_f491));
        self.form = r.range(0, data.forms.len() as u32 - 1) as usize;
        self.choice = data
            .sections
            .iter()
            .map(|s| s.pats.iter().map(|alts| if alts.len() > 1 { r.range(0, alts.len() as u32 - 1) as usize } else { 0 }).collect())
            .collect();
        self.rng = r;
    }

    /// Fades toward `target` over `ms`.
    pub fn fade(&mut self, target: f32, ms: f32, sr: f32) {
        self.target = target;
        self.fade_step = (target - self.gain).abs() / (ms.max(1.0) * 0.001 * sr);
        if ms <= 0.0 {
            self.gain = target;
        }
    }

    /// Nothing more to hear: faded out, or played through and rung out.
    pub fn finished(&self) -> bool {
        (self.gain <= 0.0 && self.target <= 0.0) || (self.ended && self.voices.is_empty())
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    /// Renders `n` samples from sample `now` into the dry and send buffers.
    pub fn render(&mut self, data: &SongData, insts: &[Prepared], now: u64, sr: f32, out: [&mut [f32]; 2], send: [&mut [f32]; 2]) {
        let n = out[0].len();
        while !self.ended && self.next_at < now + n as u64 {
            let off = self.next_at.saturating_sub(now) as u32;
            self.step(data, insts, off, sr);
            self.next_at += self.step_len;
        }
        for b in &mut self.buf {
            b.clear();
            b.resize(n, 0.0);
        }
        let [bl, br, bsl, bsr] = &mut self.buf;
        for v in &mut self.voices {
            v.render(&insts[v.inst], sr, [bl, br], [bsl, bsr]);
        }
        self.voices.retain(|v| !v.done());
        let [ol, or] = out;
        let [sl, sr_] = send;
        let g0 = data.gain;
        for k in 0..n {
            if self.gain < self.target {
                self.gain = (self.gain + self.fade_step).min(self.target);
            } else if self.gain > self.target {
                self.gain = (self.gain - self.fade_step).max(self.target);
            }
            let g = self.gain * self.gain * g0;
            ol[k] += bl[k] * g;
            or[k] += br[k] * g;
            sl[k] += bsl[k] * g;
            sr_[k] += bsr[k] * g;
        }
    }

    fn step(&mut self, data: &SongData, insts: &[Prepared], off: u32, sr: f32) {
        let sec_ix = data.forms[self.form][self.form_pos];
        let sec = &data.sections[sec_ix];
        let s = self.step_in_sec;
        let bar = data.bar;
        let chord = sec.chords[((s / bar) * 2 + (s % bar) * 2 / bar) as usize];
        let hum = (data.humanize_ms * 0.001 * sr) as u32;
        for (ti, track) in data.tracks.iter().enumerate() {
            self.drift_wait[ti] = self.drift_wait[ti].saturating_sub(1);
            let alts = &sec.pats[ti];
            if alts.is_empty() {
                continue;
            }
            let pat = &alts[self.choice[sec_ix][ti].min(alts.len() - 1)];
            let Some(&tok) = pat.get(s as usize) else { continue };
            // How long a note begun here lasts: its step and the holds after it.
            let holds = pat[s as usize + 1..].iter().take_while(|t| matches!(t, Step::Hold)).count() as u32;
            let base = 12 * (track.octave + 1) + data.tonic;
            let (notes, vel, chromatic): (Vec<i32>, f32, bool) = match (tok, track.kind) {
                (Step::Rest | Step::Hold, _) => continue,
                (Step::Note { maybe: true, .. } | Step::Hit { maybe: true, .. }, _) if self.rng.f() < 0.5 => continue,
                (Step::Note { deg, acc, oct, vel, .. }, TrackKind::Melody | TrackKind::Drum) => {
                    (vec![base + pattern::degree(data.mode, deg) + acc + 12 * oct], vel, acc != 0)
                }
                (Step::Note { deg, acc, oct, vel, .. }, _) => {
                    let borrowed = chord.acc != 0 || chord.third.is_some();
                    (vec![base + chord.tone(data.mode, deg) + acc + 12 * oct], vel, acc != 0 || borrowed)
                }
                (Step::Hit { vel, .. }, TrackKind::Chord) => {
                    let tones = chord.tones(data.mode);
                    (voice_chord(&tones, base, track.voices), vel, chord.acc != 0 || chord.third.is_some())
                }
                (Step::Hit { vel, .. }, TrackKind::Drift) => {
                    let Some((pool, gap, len)) = &track.drift else { continue };
                    if self.drift_wait[ti] > 0 || pool.is_empty() {
                        continue;
                    }
                    let k = pool[self.rng.range(0, pool.len() as u32 - 1) as usize];
                    let l = self.rng.range(len[0], len[1]);
                    self.drift_wait[ti] = self.rng.range(gap[0], gap[1]);
                    let midi = base + chord.tone(data.mode, k);
                    let gate = ((l as f32 - 1.0 + track.gate) * self.step_len as f32) as u32;
                    let v = track.vel * vel * (0.75 + 0.25 * self.rng.f());
                    let h = self.rng.range(0, hum * 3);
                    self.play(insts, ti as u16, self.inst[ti], midi, v, track.pan, off + h, gate, sr, chord.acc != 0);
                    continue;
                }
                (Step::Hit { vel, .. }, _) => {
                    // A drum hit at the instrument's own pitch.
                    let hz = insts[self.inst[ti]].inst.hz;
                    let midi = (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32;
                    (vec![midi], vel, false)
                }
            };
            let gate = ((holds as f32 + track.gate) * self.step_len as f32) as u32;
            let strum = if insts[self.inst[ti]].inst.voice == crate::model::VoiceKind::Pluck && notes.len() > 1 {
                (0.016 * sr) as u32
            } else {
                0
            };
            for (j, &midi) in notes.iter().enumerate() {
                let h = if hum > 0 { self.rng.range(0, hum) } else { 0 };
                let v = track.vel * vel * (1.0 + 0.12 * (self.rng.f() - 0.5));
                self.play(insts, ti as u16, self.inst[ti], midi, v, track.pan, off + h + strum * j as u32, gate, sr, chromatic);
            }
        }
        self.step_in_sec += 1;
        if self.step_in_sec == sec.bars * bar {
            self.step_in_sec = 0;
            self.form_pos += 1;
            if self.form_pos == data.forms[self.form].len() {
                self.form_pos = 0;
                if data.looped {
                    self.loop_n += 1;
                    self.pick(data);
                } else {
                    self.ended = true;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn play(&mut self, insts: &[Prepared], track: u16, inst: usize, midi: i32, vel: f32, pan: f32, delay: u32, gate: u32, sr: f32, chromatic: bool) {
        if let Some(log) = &mut self.log {
            log.push(NoteOn { track, midi, chromatic, at: self.next_at - self.started + u64::from(delay) });
        }
        // Past 48 voices the oldest gives way (a fast fade, never a cut).
        if self.voices.iter().filter(|v| v.level() > 0.0).count() >= 48
            && let Some(old) = self.voices.iter_mut().min_by_key(|v| v.born)
        {
            old.steal(sr);
        }
        self.born += 1;
        let hz = crate::dsp::midi_hz(midi as f32);
        let seed = self.rng.next_u32();
        self.voices.push(Voice::new(&insts[inst], inst, track, self.born, hz, vel, pan, delay, Some(gate), sr, seed));
    }

    /// How many voices are sounding.
    pub fn voices(&self) -> usize {
        self.voices.len()
    }
}

/// A chord's notes near `base` (the track's octave): each tone in the octave window from a
/// fourth under the octave's third, so a progression moves by the nearest step; a fourth voice
/// doubles the root below.
pub fn voice_chord(tones: &[i32], base: i32, voices: u8) -> Vec<i32> {
    let centre = base + 4;
    let mut out: Vec<i32> = tones
        .iter()
        .map(|&t| {
            let mut p = base + t;
            while p < centre - 6 {
                p += 12;
            }
            while p >= centre + 6 {
                p -= 12;
            }
            p
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    if usize::from(voices) > out.len() && !tones.is_empty() {
        let lo = out[0];
        let mut root = base + tones[0];
        while root >= lo {
            root -= 12;
        }
        while root + 12 < lo {
            root += 12;
        }
        out.insert(0, root);
    }
    out.truncate(usize::from(voices.max(1)));
    out
}
