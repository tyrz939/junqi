//! The bake (PORT.md §13.4): the PC's synth rendered once into the consoles' tracker module
//! (`tracker`): every song as its patterns over a shared bank of samples, each instrument's notes
//! a few semitones apart, every sound effect and a loop of each bed. Host only, at build time
//! (`jane bake --target psp` writes `jane-psp.jau`); deterministic, the same library gives the
//! same bytes.
//!
//! Every sound is rendered at the PC's 48 kHz by the PC's own voices, patches and beds, then
//! resampled (a windowed sinc) to the lowest rate that keeps it (11 025, 16 000 or 22 050 Hz: the
//! one whose top band holds under 0.3% of its energy), and stored as 4-bit ADPCM near full scale.
//! A held note loops a stretch of its sustain; a struck or plucked note past a second loops a
//! stretch of its tail, flattened, and the player lets it fall at the rate it was falling.

use std::f64::consts::PI;

use crate::bed::{Bed, BedVoice};
use crate::model::{Instrument, Library, TrackKind, VoiceKind};
use crate::pattern::{self, Step};
use crate::seq::{SongData, voice_chord};
use crate::tracker::{self as t, Bank, Codec, Header};
use crate::voice::{Prepared, Voice};

/// The rate everything is rendered at first (the PC's).
const SR: u32 = 48_000;
/// The rates a sound may be stored at.
const RATES: [u32; 3] = [11_025, 16_000, 22_050];
/// Semitones one instrument sample serves (it is played at most half this far off its root).
const SPAN: i32 = 12;
/// The same for a voice with formants (a choir): its bands stay put in Hz on the PC and are
/// narrower than a semitone's move of its harmonics, so each note it sings is its own sample.
const FORMANT_SPAN: i32 = 2;
/// A struck note keeps this many seconds whole before its tail loops.
const STRUCK_SECS: f64 = 0.45;
/// A held note's loop, seconds.
const HOLD_LOOP: f64 = 0.25;
/// The seed effects are rendered with (`engine::SFX_SEED`).
const SFX_SEED: u32 = 0x5eed;
/// ADPCM under this signal-to-noise (dB) is stored companded at eight bits instead.
const MIN_SNR: f64 = 18.0;

/// How a sound is stored.
#[derive(Clone, Copy, Debug)]
struct Opts {
    /// On the Memory Stick, loaded when wanted.
    far: bool,
    /// The share of energy a rate may leave out above 0.45 of it.
    hf: f64,
    /// ADPCM under this signal-to-noise (dB) goes companded instead.
    snr: f64,
    /// The highest rate it may be stored at (the PSP-1000's RAM, PORT.md §13.2).
    top: u32,
}

const NEAR: Opts = Opts { far: false, hf: 0.01, snr: MIN_SNR, top: 16_000 };
/// A long effect (over a second and a half): kept at 11 kHz.
const LONG: Opts = Opts { top: 11_025, ..NEAR };

/// Points on an instrument's velocity curve (`t::Inst::vel`: 0 to 1.5 by eighths).
const VELS: usize = 13;

/// What the bake made, for `jane bake`'s report.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub bytes: usize,
    pub header: usize,
    pub music: usize,
    pub sfx: usize,
    pub beds: usize,
    /// Of `bytes`, on the Memory Stick only.
    pub far: usize,
    pub samples: usize,
    pub zones: usize,
    /// The worst sample's ADPCM signal-to-noise, dB, and which.
    pub worst_snr: (f64, String),
    pub lines: Vec<String>,
}

/// Bakes `lib` into a module's bytes.
pub fn module(lib: &Library) -> (Vec<u8>, Stats) {
    let mut b = Builder {
        head: Header::default(),
        data: Vec::new(),
        far: Vec::new(),
        stats: Stats { worst_snr: (99.0, String::new()), ..Stats::default() },
    };
    let inst_names: std::collections::BTreeMap<&str, usize> =
        lib.instruments.iter().enumerate().map(|(i, x)| (x.name.as_str(), i)).collect();
    let songs: Vec<SongData> =
        lib.songs.iter().map(|s| SongData::new(s, &|n: &str| inst_names.get(n).copied().unwrap_or(0))).collect();

    // The notes each instrument is ever asked for.
    let mut wanted: Vec<std::collections::BTreeSet<i32>> =
        vec![std::collections::BTreeSet::default(); lib.instruments.len()];
    let mut gates: Vec<Vec<f64>> = vec![Vec::new(); lib.instruments.len()];
    for s in &songs {
        notes_of(s, &lib.instruments, &mut wanted, &mut gates);
    }
    // Only the instruments a song plays, in the library's order.
    let mut remap = vec![u8::MAX; lib.instruments.len()];
    for (i, inst) in lib.instruments.iter().enumerate() {
        if wanted[i].is_empty() {
            continue;
        }
        remap[i] = b.head.insts.len() as u8;
        let mark = b.data.len();
        let zones = b.instrument(inst, &wanted[i]);
        b.stats.zones += zones.len();
        b.head.insts.push(t::Inst {
            name: inst.name.clone(),
            zones,
            release_ms: inst.env.r.max(6.0).round() as u32,
            send: q12(inst.send),
            own: own_midi(inst.hz) as i8,
            pluck: inst.voice == VoiceKind::Pluck,
            attack_ms: if held(inst) { inst.env.a.max(1.5).round() as u32 } else { 0 },
            decay_ms: if held(inst) { inst.env.d.max(1.0).round() as u32 } else { 0 },
            sustain: if held(inst) { q12(inst.env.s.clamp(0.0, 1.0)) } else { 4096 },
            vel: vec![4096; VELS],
            lo: wanted[i].first().copied().unwrap_or(0).clamp(0, 127) as u8,
            gains: Vec::new(),
        });
        b.stats.music += b.data.len() - mark;
    }
    for s in &songs {
        b.head.songs.push(song(s, &remap));
    }
    // The effects: variant 0, the others as its rate a few cents off (as the PC renders them).
    for p in &lib.sfx {
        let mark = b.data.len() + b.far.len();
        let r = crate::patch::render(p, SR as f32, SFX_SEED);
        let x: Vec<f64> = r.variants[0].iter().map(|&v| f64::from(v)).collect();
        // A lesson's cues (PRESENTATION.md §2.1) wait on the Memory Stick; a bell's or the
        // thunder's long fall is a tail, looped; the rest are kept whole.
        // So do the long, rare phrases (a rest, a waking, a save, a quest's, the boots, the train,
        // the night's turn and dawn's: one of them twice a day, NIGHT.md §6.4).
        let lesson = p.name.starts_with("learn_")
            || p.name.starts_with("grow_")
            || p.name.starts_with("turn_")
            || p.name == "dawn_turn"
            || matches!(
                p.name.as_str(),
                "quest_given" | "quest_done" | "rest" | "respawn" | "save" | "boots" | "train_whistle"
            );
        let tolls = matches!(p.name.as_str(), "bell_far" | "bell_within" | "bell_near" | "church_bell" | "thunder");
        let long = x.len() > 3 * SR as usize / 2;
        let sample = if lesson {
            b.sample(&p.name, &x, None, 0, 1.0, Opts { far: true, ..LONG })
        } else if tolls {
            b.struck(&p.name, &x, 1.5, 0.4)
        } else {
            // A short one (a step, a hit, a tick of the UI) keeps its brightness: 22 kHz.
            let short = x.len() < 4 * SR as usize / 5;
            b.sample(
                &p.name,
                &x,
                None,
                0,
                1.0,
                if long {
                    LONG
                } else if short {
                    Opts { top: 22_050, ..NEAR }
                } else {
                    NEAR
                },
            )
        };
        let tunes = (0..usize::from(p.variants.max(1)))
            .map(|v| {
                let mut rng =
                    crate::dsp::Rng::new(SFX_SEED ^ (v as u32).wrapping_mul(0x85eb_ca6b) ^ crate::patch::hash(&p.name));
                let cents = if v == 0 { 0.0 } else { f64::from(rng.bi()) * 40.0 };
                (2f64.powf(cents / 1200.0) * 65_536.0).round() as u32
            })
            .collect();
        b.head.sfx.push(t::Sfx { name: p.name.clone(), sample, send: q12(p.send), tunes });
        b.stats.sfx += b.data.len() + b.far.len() - mark;
    }
    for bed in Bed::ALL {
        let mark = b.data.len();
        let l = b.bed(bed);
        b.head.beds.push(l);
        b.stats.beds += b.data.len() - mark;
    }
    b.bed_levels();
    // Each instrument's usual note, seconds: the median of what the songs hold it for.
    let usual: Vec<f64> = lib
        .instruments
        .iter()
        .enumerate()
        .filter(|(i, _)| !wanted[*i].is_empty())
        .map(|(i, _)| {
            let mut g = gates[i].clone();
            g.sort_by(f64::total_cmp);
            g.get(g.len() / 2).copied().unwrap_or(1.0)
        })
        .collect();
    let notes: Vec<Vec<i32>> = wanted.iter().filter(|w| !w.is_empty()).map(|w| w.iter().copied().collect()).collect();
    b.velocities(lib, &usual, &notes);
    b.stats.samples = b.head.samples.len();
    let bytes = Bank::write(&b.head, &b.data, &b.far);
    b.stats.bytes = bytes.len();
    b.stats.far = b.far.len();
    b.stats.header = bytes.len() - b.data.len() - b.far.len();
    (bytes, b.stats)
}

struct Builder {
    head: Header,
    data: Vec<u8>,
    far: Vec<u8>,
    stats: Stats,
}

fn q12(x: f32) -> u16 {
    (f64::from(x) * 4096.0).round().clamp(0.0, 65_535.0) as u16
}

/// A voice that holds while its note does: its samples are its sustain, looped, and the player
/// draws its attack and decay.
fn held(inst: &Instrument) -> bool {
    inst.env.s > 0.02 && matches!(inst.voice, VoiceKind::Fm | VoiceKind::Table | VoiceKind::Noise)
}

fn own_midi(hz: f32) -> i32 {
    (69.0 + 12.0 * (f64::from(hz) / 440.0).log2()).round() as i32
}

/// Every note a song can ask of each instrument (any county, any loop): its patterns over its
/// chords, on the track's instrument and every alternative.
fn notes_of(
    s: &SongData,
    insts: &[Instrument],
    wanted: &mut [std::collections::BTreeSet<i32>],
    gates: &mut [Vec<f64>],
) {
    let step = f64::from(s.step_ticks) / 60.0;
    for (ti, track) in s.tracks.iter().enumerate() {
        let base = 12 * (track.octave + 1) + s.tonic;
        let add = |wanted: &mut [std::collections::BTreeSet<i32>], m: i32| {
            for &i in &track.insts {
                wanted[i].insert(m);
            }
        };
        for sec in &s.sections {
            for alt in &sec.pats[ti] {
                for (k, tok) in alt.iter().enumerate() {
                    let chord = sec.chords[((k as u32 / s.bar) * 2 + (k as u32 % s.bar) * 2 / s.bar) as usize];
                    if matches!(tok, Step::Note { .. } | Step::Hit { .. }) {
                        let holds = alt[k + 1..].iter().take_while(|t| matches!(t, Step::Hold)).count() as f64;
                        let len = match &track.drift {
                            Some((_, _, len)) if track.kind == TrackKind::Drift => {
                                f64::from(len[0] + len[1]) / 2.0 - 1.0
                            }
                            _ => holds,
                        };
                        for &i in &track.insts {
                            gates[i].push((len + f64::from(track.gate)) * step);
                        }
                    }
                    match (*tok, track.kind) {
                        (Step::Note { deg, acc, oct, .. }, TrackKind::Melody | TrackKind::Drum) => {
                            add(wanted, base + pattern::degree(s.mode, deg) + acc + 12 * oct);
                        }
                        (Step::Note { deg, acc, oct, .. }, _) => {
                            add(wanted, base + chord.tone(s.mode, deg) + acc + 12 * oct);
                        }
                        (Step::Hit { .. }, TrackKind::Chord) => {
                            for m in voice_chord(&chord.tones(s.mode), base, track.voices) {
                                add(wanted, m);
                            }
                        }
                        (Step::Hit { .. }, TrackKind::Drift) => {
                            if let Some((pool, ..)) = &track.drift {
                                for &d in pool {
                                    add(wanted, base + chord.tone(s.mode, d));
                                }
                            }
                        }
                        (Step::Hit { .. }, _) => {
                            for &i in &track.insts {
                                let own = own_midi(insts[i].hz);
                                let up = (s.tonic - own).rem_euclid(12);
                                wanted[i].insert(if up <= 6 { own + up } else { own + up - 12 });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn song(s: &SongData, remap: &[u8]) -> t::Song {
    let steps = s.mode.steps().map(|x| x as i8);
    let tracks = s
        .tracks
        .iter()
        .map(|tr| t::Track {
            kind: match tr.kind {
                TrackKind::Melody => t::Kind::Melody,
                TrackKind::Chord => t::Kind::Chord,
                TrackKind::Bass => t::Kind::Bass,
                TrackKind::Arp => t::Kind::Arp,
                TrackKind::Drum => t::Kind::Drum,
                TrackKind::Drift => t::Kind::Drift,
            },
            insts: tr.insts.iter().map(|&i| remap[i]).collect(),
            octave: tr.octave as i8,
            vel: q12(tr.vel),
            pan: (f64::from(tr.pan) * 4096.0).round() as i16,
            voices: tr.voices,
            gate: q12(tr.gate),
            drift: tr.drift.as_ref().map(|(pool, gap, len)| t::Drift {
                pool: pool.iter().map(|&d| d as i8).collect(),
                gap: *gap,
                len: *len,
            }),
        })
        .collect();
    let sections = s
        .sections
        .iter()
        .map(|sec| t::Section {
            bars: sec.bars,
            chords: sec
                .chords
                .iter()
                .map(|c| t::Chord {
                    tone: std::array::from_fn(|k| c.tone(s.mode, k as i32) as i8),
                    tones: c.tones(s.mode).iter().map(|&x| x as i8).collect(),
                    acc: c.acc != 0,
                    borrowed: c.acc != 0 || c.third.is_some(),
                })
                .collect(),
            pats: sec
                .pats
                .iter()
                .map(|alts| {
                    alts.iter()
                        .map(|p| {
                            p.iter()
                                .map(|st| match *st {
                                    Step::Rest => t::Step { tag: t::REST, ..t::Step::default() },
                                    Step::Hold => t::Step { tag: t::HOLD, ..t::Step::default() },
                                    Step::Note { deg, acc, oct, vel, maybe } => t::Step {
                                        tag: t::NOTE,
                                        maybe,
                                        deg: deg as i8,
                                        acc: acc as i8,
                                        oct: oct as i8,
                                        vel: q12(vel),
                                    },
                                    Step::Hit { vel, maybe } => {
                                        t::Step { tag: t::HIT, maybe, vel: q12(vel), ..t::Step::default() }
                                    }
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        })
        .collect();
    assert!(
        s.tracks.len() <= t::MAX_TRACKS && s.sections.len() <= t::MAX_SECTIONS,
        "{}: too big for the player",
        s.name
    );
    t::Song {
        name: s.name.clone(),
        hash: crate::patch::hash(&s.name),
        tonic: s.tonic as i8,
        steps,
        step_ticks: s.step_ticks,
        bar: s.bar,
        gain: q12(s.gain),
        looped: s.looped,
        hum_us: (f64::from(s.humanize_ms) * 1000.0).round() as u32,
        tracks,
        sections,
        forms: s.forms.iter().map(|f| f.iter().map(|&x| x as u8).collect()).collect(),
    }
}

impl Builder {
    /// An instrument's zones: notes `SPAN` semitones apart, each rendered at its root.
    fn instrument(&mut self, inst: &Instrument, wanted: &std::collections::BTreeSet<i32>) -> Vec<t::Zone> {
        let prep = Prepared::new(inst.clone());
        let notes: Vec<i32> = wanted.iter().copied().collect();
        let mut zones = Vec::new();
        let span = if inst.formants.is_empty() { SPAN } else { FORMANT_SPAN };
        let mut k = 0;
        while k < notes.len() {
            let lo = notes[k];
            let mut hi = lo;
            while k < notes.len() && notes[k] < lo + span {
                hi = notes[k];
                k += 1;
            }
            let root = (lo + hi + 1) / 2;
            let sample = self.note(inst, &prep, root);
            zones.push(t::Zone { top: hi.clamp(0, 127) as u8, root: root.clamp(0, 127) as u8, sample });
        }
        zones
    }

    /// Each zone's level and each instrument's velocity curve, measured: the PC's voice (a
    /// second held, then let go, rung out; each side's power) against the module's note played by
    /// the module's own mixer. A note's own draws (its unison's phases, a string's burst) move the
    /// PC's level a few dB, so a zone is matched to the mean of six notes; the curve's shape is
    /// the middle zone's (louder notes are brighter on the PC, and the module keeps one velocity).
    fn velocities(&mut self, lib: &Library, usual: &[f64], notes: &[Vec<i32>]) {
        let rate = 22_050;
        let rms = |x: &[f64]| (x.iter().map(|s| s * s).sum::<f64>() / x.len().max(1) as f64).sqrt();
        let refv = t::REF_VEL as f32 / 4096.0;
        for k in 0..self.head.insts.len() {
            let bytes = Bank::write(&self.head, &self.data, &self.far);
            let inst = self.head.insts[k].clone();
            let src = lib.instruments.iter().find(|i| i.name == inst.name).expect("an instrument");
            let prep = Prepared::new(src.clone());
            // Held as long as the songs usually hold it.
            let gate = usual[k].clamp(0.05, 4.0);
            let secs = gate + (f64::from(src.env.r) / 1000.0).min(2.5) + 0.3;
            let pc = |midi: i32, vel: f32, seed: u32| {
                let hz = crate::dsp::midi_hz(midi as f32);
                let mut v =
                    Voice::new(&prep, 0, 0, 0, hz, vel, 0.0, 0, Some((gate * f64::from(SR)) as u32), SR as f32, seed);
                let n = (secs * f64::from(SR)) as usize;
                let (mut l, mut r, mut sl, mut sr) =
                    (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
                v.render(&prep, SR as f32, [&mut l, &mut r], [&mut sl, &mut sr]);
                let both: Vec<f64> = l.iter().chain(&r).map(|&s| f64::from(s)).collect();
                rms(&both)
            };
            for z in &inst.zones {
                let midi = i32::from(z.root);
                let mut m = t::Mixer::new(Bank::parse(bytes.clone()).expect("the bake reads back"), rate, 1);
                m.audition(k, midi, t::REF_VEL, (gate * f64::from(rate)) as u32);
                let mut out = vec![0i16; 2 * (secs * f64::from(rate)) as usize / t::BLOCK * t::BLOCK + 2 * t::BLOCK];
                m.render(&mut out);
                let ours = rms(&out.iter().map(|&s| f64::from(s) / 32_767.0).collect::<Vec<_>>());
                let level =
                    ((0..6u32).map(|s| pc(midi, refv, 0x5eed ^ s.wrapping_mul(0x9e37_79b9)).powi(2)).sum::<f64>()
                        / 6.0)
                        .sqrt();
                let s = &mut self.head.samples[usize::from(z.sample)];
                s.level = (f64::from(s.level) * level / ours.max(1e-12)).round().clamp(1.0, f64::from(u32::MAX)) as u32;
            }
            // Each note against its zone's root, the PC's and ours alike (two notes' draws).
            let bytes = Bank::write(&self.head, &self.data, &self.far);
            let mut gains = Vec::new();
            let lo = notes[k].first().copied().unwrap_or(0);
            let hi = notes[k].last().copied().unwrap_or(0);
            for midi in lo..=hi {
                let Some(z) = inst.zones.iter().find(|z| i32::from(z.top) >= midi).or(inst.zones.last()) else {
                    continue;
                };
                let root = i32::from(z.root);
                if !notes[k].contains(&midi) || midi == root {
                    gains.push(4096);
                    continue;
                }
                let two = |m: i32| {
                    ((0..2u32).map(|s| pc(m, refv, 0x1dea ^ s.wrapping_mul(0x9e37_79b9)).powi(2)).sum::<f64>() / 2.0)
                        .sqrt()
                };
                let ours = |m: i32| {
                    let mut mx = t::Mixer::new(Bank::parse(bytes.clone()).expect("the bake reads back"), rate, 1);
                    mx.audition(k, m, t::REF_VEL, (gate * f64::from(rate)) as u32);
                    let mut out =
                        vec![0i16; 2 * (secs * f64::from(rate)) as usize / t::BLOCK * t::BLOCK + 2 * t::BLOCK];
                    mx.render(&mut out);
                    rms(&out.iter().map(|&s| f64::from(s) / 32_767.0).collect::<Vec<_>>())
                };
                let g = (two(midi) / two(root).max(1e-12)) / (ours(midi) / ours(root).max(1e-12)).max(1e-12);
                gains.push((g * 4096.0).round().clamp(1.0, 65_535.0) as u16);
            }
            self.head.insts[k].gains = gains;
            let midi = i32::from(inst.zones[inst.zones.len() / 2].root);
            let seed = 0x7a11 ^ midi as u32;
            let at_ref = pc(midi, refv, seed).max(1e-12);
            self.head.insts[k].vel = (0..VELS)
                .map(|i| {
                    if i == 0 {
                        0
                    } else {
                        (pc(midi, i as f32 / 8.0, seed) / at_ref * 4096.0).round().min(65_535.0) as u16
                    }
                })
                .collect();
        }
    }
    /// One note of `inst` at `midi`, as the PC's voice plays it at the reference velocity.
    fn note(&mut self, inst: &Instrument, prep: &Prepared, midi: i32) -> u16 {
        let hz = crate::dsp::midi_hz(midi as f32);
        let held = held(inst);
        // Held: rendered at full sustain from the first frame (the player draws the attack and
        // decay); what changes in its timbre (the FM index, the filter's sweep) settles, then a
        // loop of the sustain. Struck: its fall, as it is.
        let flat;
        let prep = if held {
            let mut i = inst.clone();
            i.env = crate::model::Env { a: 1.5, d: 1.0, s: 1.0, r: inst.env.r };
            flat = Prepared::new(i);
            &flat
        } else {
            prep
        };
        let mut v =
            Voice::new(prep, 0, 0, 0, hz, t::REF_VEL as f32 / 4096.0, 0.0, 0, None, SR as f32, 0x7a11 ^ midi as u32);
        let sweep = inst.filter.map_or(0.0, |f| if f.env == 0.0 { 0.0 } else { 1.2 * f.env_ms });
        let index = if inst.voice == VoiceKind::Fm { 0.8 * inst.index_ms } else { 0.0 };
        let settle = f64::from(sweep.max(index)).clamp(60.0, 350.0) / 1000.0;
        let secs = if held { settle + 2.0 * HOLD_LOOP + 0.1 } else { 8.0 };
        let n = (secs * f64::from(SR)) as usize;
        let mut x = vec![0.0f64; n];
        let block = 4800;
        let (mut l, mut r, mut sl, mut sr) =
            (vec![0.0f32; block], vec![0.0f32; block], vec![0.0f32; block], vec![0.0f32; block]);
        let mut at = 0;
        while at < n && !v.done() {
            for b in [&mut l, &mut r, &mut sl, &mut sr] {
                b.fill(0.0);
            }
            v.render(prep, SR as f32, [&mut l, &mut r], [&mut sl, &mut sr]);
            for k in 0..block.min(n - at) {
                // Centre-panned: each side is the voice times 0.707.
                x[at + k] = f64::from(l[k] + r[k]) * 0.5 / std::f64::consts::FRAC_1_SQRT_2;
            }
            at += block;
        }
        let name = format!("{}@{midi}", inst.name);
        if held {
            let start = (settle * f64::from(SR)) as usize;
            let len = (HOLD_LOOP * hz.max(20.0) as f64).round() / f64::from(hz.max(20.0));
            self.sample(&name, &x, Some((start, len)), 0, 1.0, NEAR)
        } else {
            self.struck(&name, &x, STRUCK_SECS, 0.3)
        }
    }

    /// A struck or plucked sound: whole to its quiet end, or whole for `STRUCK_SECS` and then a
    /// flattened loop of its tail falling at its own rate.
    fn struck(&mut self, name: &str, x: &[f64], keep_secs: f64, loop_len: f64) -> u16 {
        let env = envelope(x, SR);
        let peak = env.iter().copied().fold(0.0, f64::max);
        let quiet = env.iter().rposition(|&e| e > peak * 10f64.powf(-50.0 / 20.0)).map_or(1, |k| k + 1);
        let end = (quiet * SR as usize / 100).min(x.len());
        let keep = (keep_secs * f64::from(SR)) as usize;
        if end <= keep + SR as usize / 4 {
            return self.sample(name, &x[..end], None, 0, 1.0, NEAR);
        }
        // The fall's rate from `keep` to its quiet end (two seconds at most): dB a second. A
        // struck sound falls fast and then slowly; the slow part is what the loop carries on.
        let (a, b) = (
            keep * 100 / SR as usize,
            (keep * 100 / SR as usize + 200).min(quiet.saturating_sub(1)).min(env.len() - 1),
        );
        let b = b.max(a + 1).min(env.len() - 1);
        let (ea, eb) = (env[a].max(1e-9), env[b].max(1e-9));
        let db_per_s = (20.0 * (ea / eb).log10() / ((b - a) as f64 / 100.0)).max(3.0);
        // The tail's fall is the one that leaves as much energy after `keep` as the PC's: an
        // exponential from the loop's level holds its mean square times T / (6 ln 10).
        let after: f64 = x[keep..end].iter().map(|s| s * s).sum::<f64>() / f64::from(SR);
        let a2 = env[a] * env[a];
        let tail_ms = if a2 > 1e-18 {
            (after * 6.0 * std::f64::consts::LN_10 / a2 * 1000.0).clamp(100.0, 60_000.0).round() as u32
        } else {
            (60.0 / db_per_s * 1000.0).round() as u32
        };
        // Flatten the loop: undo the fall over it, so each pass is as loud as the last and the
        // player's envelope carries the fall.
        let lo = keep;
        let mut y = x[..keep + (loop_len * f64::from(SR)) as usize + 10].to_vec();
        for (k, s) in y.iter_mut().enumerate().skip(lo) {
            let secs = (k - lo) as f64 / f64::from(SR);
            *s *= 10f64.powf(db_per_s * secs / 20.0);
        }
        self.sample(name, &y, Some((keep, loop_len)), tail_ms, 1.0, NEAR)
    }

    /// A bed: a loop of it at its usual level, made seamless, at full level.
    fn bed(&mut self, bed: Bed) -> t::BedLoop {
        let lv = bed_level(bed) as f32;
        let (secs, wide) = match bed {
            Bed::Rain | Bed::RainRoof | Bed::Crickets | Bed::Hum => (2.0, true),
            Bed::Wind | Bed::Lake | Bed::Cave | Bed::Fire | Bed::Drone | Bed::Machinery => (3.0, true),
            Bed::Birds => (4.0, true),
            Bed::Clock => (2.0, false),
        };
        let mut v = BedVoice::new(bed, SR as f32, 1);
        v.level = lv;
        v.target = lv;
        let warm = 2 * SR as usize;
        let n = ((secs + 0.35) * f64::from(SR)) as usize;
        let total = warm + n;
        let (mut l, mut r, mut sl, mut sr) =
            (vec![0.0f32; total], vec![0.0f32; total], vec![0.0f32; total], vec![0.0f32; total]);
        v.render(SR as f32, [&mut l, &mut r], [&mut sl, &mut sr]);
        let side = |c: &[f32]| c[warm..].iter().map(|&s| f64::from(s) / f64::from(lv)).collect::<Vec<f64>>();
        let (l, r, sl, sr) = (side(&l), side(&r), side(&sl), side(&sr));
        let mono: Vec<f64> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
        let rms = |x: &[f64]| (x.iter().map(|s| s * s).sum::<f64>() / x.len().max(1) as f64).sqrt();
        // Each side as loud as the PC's: a wide bed's copies play one a side, a narrow one in the
        // middle (0.707 a side).
        let side_rms = 0.5 * (rms(&l) + rms(&r));
        let gain = if wide {
            side_rms / rms(&mono).max(1e-12)
        } else {
            side_rms / (rms(&mono) * std::f64::consts::FRAC_1_SQRT_2).max(1e-12)
        };
        let send = (rms(&sl.iter().zip(&sr).map(|(a, b)| a + b).collect::<Vec<_>>())
            / rms(&l.iter().zip(&r).map(|(a, b)| a + b).collect::<Vec<_>>()).max(1e-12))
        .clamp(0.0, 1.0);
        let sample = self.sample(
            bed.name(),
            &mono,
            Some(((0.3 * f64::from(SR)) as usize, secs)),
            0,
            gain,
            Opts {
                far: false,
                hf: 0.02,
                snr: 0.0,
                top: if matches!(bed, Bed::Birds | Bed::Crickets) { 16_000 } else { 11_025 },
            },
        );
        let s = &self.head.samples[usize::from(sample)];
        let half = s.loop_start + s.loop_len / 2;
        let half_state = self.state_at(sample, half);
        t::BedLoop { name: bed.name().into(), sample, wide, send: q12(send as f32), half, half_state }
    }

    /// Each bed's loop matched to the PC's bed by each side's loudness over twelve seconds at
    /// its usual level (a loop of grains holds more or fewer of them than the live bed's mean).
    fn bed_levels(&mut self) {
        let rate = 22_050u32;
        let sides = |l: &[f32], r: &[f32], sr: f32| {
            0.5 * f64::from(crate::analysis::loudness(l, sr) + crate::analysis::loudness(r, sr))
        };
        for (k, bed) in Bed::ALL.iter().enumerate() {
            let lv = bed_level(*bed);
            let mut v = BedVoice::new(*bed, SR as f32, 7);
            v.level = lv as f32;
            v.target = lv as f32;
            let n = 14 * SR as usize;
            let (mut l, mut r, mut sl, mut sr) = (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
            v.render(SR as f32, [&mut l, &mut r], [&mut sl, &mut sr]);
            let w = 2 * SR as usize;
            let pc = sides(&l[w..], &r[w..], SR as f32);
            let mut m = t::Mixer::new(
                Bank::parse(Bank::write(&self.head, &self.data, &self.far)).expect("reads back"),
                rate,
                7,
            );
            m.handle(t::Cmd::Bed { bed: k as u8, level: (lv * 255.0).round() as u8 });
            let mut out = vec![0i16; 2 * 15 * rate as usize / t::BLOCK * t::BLOCK];
            m.render(&mut out);
            let skip = 2 * 3 * rate as usize;
            let ol: Vec<f32> = out[skip..].iter().step_by(2).map(|&s| f32::from(s) / 32_767.0).collect();
            let or: Vec<f32> = out[skip + 1..].iter().step_by(2).map(|&s| f32::from(s) / 32_767.0).collect();
            let ours = sides(&ol, &or, rate as f32);
            let s = &mut self.head.samples[usize::from(self.head.beds[k].sample)];
            s.level = (f64::from(s.level) * 10f64.powf((pc - ours) / 20.0)).round() as u32;
        }
    }

    /// The decoder's state before frame `at` of a sample (packed, `vag_pack`).
    fn state_at(&self, sample: u16, at: u32) -> i32 {
        let s = &self.head.samples[usize::from(sample)];
        if s.codec != Codec::Adpcm {
            return 0;
        }
        let frames = if s.far { &self.far } else { &self.data };
        let frames = &frames[s.at as usize..];
        let (mut o, mut oo) = (0, 0);
        for i in 0..at as usize {
            let (b, k) = (i / t::VAG_FRAMES * t::VAG_BYTES, i % t::VAG_FRAMES);
            let n = frames[b + 1 + k / 2];
            t::vag_decode(frames[b], if k % 2 == 0 { n & 15 } else { n >> 4 }, &mut o, &mut oo);
        }
        t::vag_pack(o, oo)
    }
    /// Stores `x` (48 kHz, full scale 1) at the lowest rate that keeps it; `looped` is (where the
    /// loop starts, frames at 48 kHz; its length, seconds), crossfaded so it is seamless.
    fn sample(&mut self, name: &str, x: &[f64], looped: Option<(usize, f64)>, tail_ms: u32, gain: f64, o: Opts) -> u16 {
        let rate = pick_rate(x, o.hf).min(o.top);
        let y = resample(x, rate);
        let ratio = f64::from(rate) / f64::from(SR);
        let (mut y, loop_at) = match looped {
            Some((start, secs)) => {
                let s = (start as f64 * ratio).round() as usize;
                let n = (secs * f64::from(rate)).round() as usize;
                let fade = (n / 4).min((0.05 * f64::from(rate)) as usize).min(s.max(1));
                let mut z = y[..(s + n).min(y.len())].to_vec();
                let n = z.len() - s;
                // The loop's last `fade` frames turn into what came before its start.
                for i in 0..fade {
                    let w = (i as f64 + 0.5) / fade as f64 * PI / 2.0;
                    let at = s + n - fade + i;
                    let before = if s + i >= fade { y[s + i - fade] } else { 0.0 };
                    z[at] = z[at] * w.cos() + before * w.sin();
                }
                (z, Some((s as u32, n as u32)))
            }
            None => (y, None),
        };
        for v in &mut y {
            *v *= gain;
        }
        let peak = y.iter().fold(0.0f64, |a, &b| a.max(b.abs())).max(1e-9);
        let scale = 32_767.0 * 0.98 / peak;
        let pcm: Vec<i16> = y.iter().map(|&v| (v * scale).round().clamp(-32_767.0, 32_767.0) as i16).collect();
        let marks: Vec<usize> = loop_at.map(|(s, _)| vec![s as usize]).unwrap_or_default();
        let (bytes, states) = t::vag_encode(&pcm, &marks);
        // How near the code came; under `MIN_SNR` the frames go companded instead.
        let (mut h1, mut h2) = (0, 0);
        let (mut sig, mut err) = (0.0f64, 0.0f64);
        for (i, &s) in pcm.iter().enumerate() {
            let (b, k) = (i / t::VAG_FRAMES * t::VAG_BYTES, i % t::VAG_FRAMES);
            let n = bytes[b + 1 + k / 2];
            let d = t::vag_decode(bytes[b], if k % 2 == 0 { n & 15 } else { n >> 4 }, &mut h1, &mut h2);
            sig += f64::from(s) * f64::from(s);
            err += f64::from(d - i32::from(s)).powi(2);
        }
        let mut snr = 10.0 * (sig / err.max(1.0)).log10();
        let (codec, bytes) = if snr >= o.snr {
            (Codec::Adpcm, bytes)
        } else {
            let mu: Vec<u8> = pcm.iter().map(|&s| t::mu8_encode(s)).collect();
            let err: f64 = pcm.iter().zip(&mu).map(|(&s, &u)| f64::from(t::mu8_decode(u) - i32::from(s)).powi(2)).sum();
            snr = 10.0 * (sig / err.max(1.0)).log10();
            (Codec::Mu8, mu)
        };
        if snr < self.stats.worst_snr.0 {
            self.stats.worst_snr = (snr, name.into());
        }
        let store = if o.far { &mut self.far } else { &mut self.data };
        while store.len() % 4 != 0 {
            store.push(0);
        }
        let at = store.len() as u32;
        store.extend_from_slice(&bytes);
        let (loop_start, loop_len) = loop_at.unwrap_or((0, 0));
        let loop_state = if codec == Codec::Adpcm { states.first().copied().unwrap_or(0) } else { 0 };
        self.stats.lines.push(format!(
            "{name}: {rate} Hz, {:.2} s{}, {} B {codec:?}{}, snr {snr:.0} dB",
            pcm.len() as f64 / f64::from(rate),
            if loop_len > 0 { format!(", loop {:.2} s", f64::from(loop_len) / f64::from(rate)) } else { String::new() },
            bytes.len(),
            if o.far { " far" } else { "" },
        ));
        self.head.samples.push(t::Sample {
            at,
            len: pcm.len() as u32,
            rate,
            codec,
            level: (peak / 0.98 * 65_536.0).round() as u32,
            loop_start,
            loop_len,
            loop_state,
            tail_ms,
            far: o.far,
        });
        (self.head.samples.len() - 1) as u16
    }
}

/// The level a bed is usually asked for (PRESENTATION.md §5.1), 0 to 1: its loop is made there.
fn bed_level(bed: Bed) -> f64 {
    match bed {
        Bed::Rain => 0.67,
        Bed::RainRoof => 0.6,
        Bed::Wind => 0.4,
        _ => 0.8,
    }
}

/// An RMS envelope a hundredth of a second.
fn envelope(x: &[f64], sr: u32) -> Vec<f64> {
    x.chunks(sr as usize / 100).map(|c| (c.iter().map(|s| s * s).sum::<f64>() / c.len() as f64).sqrt()).collect()
}

/// The lowest rate whose band above 0.45 of it holds under `share` of the sound's energy.
fn pick_rate(x: &[f64], share: f64) -> u32 {
    let all: f64 = x.iter().map(|s| s * s).sum::<f64>().max(1e-30);
    for &r in &RATES[..RATES.len() - 1] {
        let fc = 0.45 * f64::from(r);
        let mut hp = [Biquad::highpass(fc, 0.541_196), Biquad::highpass(fc, 1.306_563)];
        let high: f64 = x
            .iter()
            .map(|&s| {
                let y = hp[0].run(s);
                let y = hp[1].run(y);
                y * y
            })
            .sum();
        if high / all < share {
            return r;
        }
    }
    RATES[RATES.len() - 1]
}

struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    fn highpass(fc: f64, q: f64) -> Biquad {
        let w = 2.0 * PI * fc / f64::from(SR);
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        Biquad {
            b: [f64::midpoint(1.0, c) / a0, -(1.0 + c) / a0, f64::midpoint(1.0, c) / a0],
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
            z: [0.0; 2],
        }
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// `x` at 48 kHz to `rate`: a Blackman-windowed sinc, its cut at 0.46 of the new rate.
fn resample(x: &[f64], rate: u32) -> Vec<f64> {
    let r = f64::from(rate) / f64::from(SR);
    let fc = 0.46 * r;
    let half = (8.0 / r).ceil() as i64;
    let n = (x.len() as f64 * r).ceil() as usize;
    (0..n)
        .map(|k| {
            let c = k as f64 / r;
            let ci = c.floor() as i64;
            let mut acc = 0.0;
            for j in ci - half + 1..=ci + half {
                if j < 0 || j as usize >= x.len() {
                    continue;
                }
                let d = c - j as f64;
                let w = 0.42 + 0.5 * (PI * d / half as f64).cos() + 0.08 * (2.0 * PI * d / half as f64).cos();
                let s = if d.abs() < 1e-12 { 2.0 * fc } else { (2.0 * PI * fc * d).sin() / (PI * d) };
                acc += x[j as usize] * s * w;
            }
            acc
        })
        .collect()
}
