//! The instruments' voices (PRESENTATION.md §5): FM, additive tables with unison, Karplus-Strong
//! strings, struck modes, a pitched drum and filtered noise, each through its own filter and
//! envelope. A voice is one note; the sequencer starts it with a delay in samples, so a note
//! lands on its sample, not on the block it falls in.

use crate::dsp::{Adsr, Modal, Pink, Pluck, Rng, Svf, Tables, pan_gains, read_table, sin_cycles};
use crate::model::{FilterKind, Instrument, VoiceKind};

/// An instrument ready to play: its tables built.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub inst: Instrument,
    tables: Option<Tables>,
}

impl Prepared {
    pub fn new(inst: Instrument) -> Prepared {
        let tables = (inst.voice == VoiceKind::Table).then(|| {
            let amps: Vec<f32> = if inst.harmonics.is_empty() {
                (1..=inst.partials.clamp(1, 32))
                    .map(|h| if inst.odd && h % 2 == 0 { 0.0 } else { 1.0 / f32::from(h).powf(inst.rolloff) })
                    .collect()
            } else {
                inst.harmonics.clone()
            };
            Tables::new(&amps)
        });
        Prepared { inst, tables }
    }
}

#[derive(Clone, Debug)]
enum Osc {
    Fm {
        pc: f32,
        pm: f32,
        pm2: f32,
        fb: f32,
        i1: f32,
        i1_floor: f32,
        i1_coef: f32,
        i2: f32,
        i2_coef: f32,
    },
    Table {
        phases: [f32; 4],
        ratios: [f32; 4],
        pans: [(f32, f32); 4],
        n: usize,
        breath: Svf,
        pink: Pink,
        formants: Vec<Svf>,
    },
    Pluck(Box<Pluck>),
    Bell {
        modes: Box<Modal>,
        strike: f32,
        strike_coef: f32,
        strike_f: Svf,
    },
    Drum {
        phase: f32,
        bend: f32,
        bend_coef: f32,
        click: f32,
        click_coef: f32,
        click_f: Svf,
    },
    Noise {
        pink: Pink,
    },
}

/// One sounding note.
#[derive(Clone, Debug)]
pub struct Voice {
    /// Which instrument (its index in the engine's list).
    pub inst: usize,
    /// Which track started it, and when (for stealing the oldest).
    pub track: u16,
    pub born: u64,
    /// Samples to wait before it starts.
    delay: u32,
    /// Samples it sounds before its release; `None` until let go.
    gate: Option<u32>,
    env: Adsr,
    osc: Osc,
    filter: Option<(Svf, FilterKind)>,
    cutoff: f32,
    fenv: f32,
    fenv_coef: f32,
    hz: f32,
    amp: f32,
    gl: f32,
    gr: f32,
    pub send: f32,
    t: u32,
    vib_phase: f32,
    lfo_phase: f32,
    rng: Rng,
}

impl Voice {
    /// A note of `prep` at `hz`, `vel` (0 to 1.3), after `delay` samples, let go after `gate`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prep: &Prepared,
        inst: usize,
        track: u16,
        born: u64,
        hz: f32,
        vel: f32,
        pan: f32,
        delay: u32,
        gate: Option<u32>,
        sr: f32,
        seed: u32,
    ) -> Voice {
        let i = &prep.inst;
        let mut rng = Rng::new(seed);
        let osc = match i.voice {
            VoiceKind::Fm => {
                let c = crate::dsp::fall_coef(i.index_ms.max(1.0), sr);
                let c2 = crate::dsp::fall_coef(i.mod2[2].max(1.0), sr);
                // A louder note is brighter: more index.
                let lift = 0.6 + 0.4 * vel;
                Osc::Fm {
                    pc: 0.0,
                    pm: 0.0,
                    pm2: 0.0,
                    fb: 0.0,
                    i1: i.index * lift,
                    i1_floor: i.index * i.index_sustain * lift,
                    i1_coef: c,
                    i2: i.mod2[1] * lift,
                    i2_coef: c2,
                }
            }
            VoiceKind::Table => {
                let n = usize::from(i.unison.clamp(1, 4));
                let mut phases = [0.0; 4];
                let mut ratios = [1.0; 4];
                let mut pans = [(0.707, 0.707); 4];
                for k in 0..n {
                    phases[k] = rng.f();
                    let spread = if n == 1 { 0.0 } else { k as f32 / (n - 1) as f32 * 2.0 - 1.0 };
                    ratios[k] = 2f32.powf(spread * i.detune / 1200.0);
                    pans[k] = pan_gains((pan + spread * 0.6).clamp(-1.0, 1.0));
                }
                let formants = i.formants.iter().map(|f| Svf::new(f[0], f[1], sr)).collect();
                Osc::Table {
                    phases,
                    ratios,
                    pans,
                    n,
                    breath: Svf::new(hz * 2.0, 1.5, sr),
                    pink: Pink::default(),
                    formants,
                }
            }
            VoiceKind::Pluck => {
                let t60 = i.sustain_s * (261.63 / hz).powf(0.5);
                Osc::Pluck(Box::new(Pluck::new(hz, sr, (i.bright * (0.7 + 0.3 * vel)).min(1.0), t60, i.pick, &mut rng)))
            }
            VoiceKind::Bell => Osc::Bell {
                modes: Box::new(Modal::new(hz, &i.modes, i.beat, sr, 1.0)),
                strike: i.strike * vel,
                strike_coef: crate::dsp::fall_coef(25.0, sr),
                strike_f: Svf::new((hz * 6.0).min(9000.0), 1.2, sr),
            },
            VoiceKind::Drum => Osc::Drum {
                phase: 0.0,
                bend: 2f32.powf(i.drop / 12.0) - 1.0,
                bend_coef: crate::dsp::fall_coef(i.drop_ms, sr),
                click: i.click * vel,
                click_coef: crate::dsp::fall_coef(12.0, sr),
                click_f: Svf::new(3000.0, 0.8, sr),
            },
            VoiceKind::Noise => Osc::Noise { pink: Pink::default() },
        };
        let (gl, gr) = pan_gains(pan);
        let (filter, cutoff) = match &i.filter {
            Some(f) => {
                let key = (hz / 261.63).powf(f.key);
                let v = 2f32.powf(f.vel * (vel - 0.8));
                let c = f.cutoff * key * v;
                (Some((Svf::new(c * 2f32.powf(f.env), f.q, sr), f.kind)), c)
            }
            None => (None, 0.0),
        };
        let fenv_coef = i.filter.map_or(0.0, |f| crate::dsp::fall_coef(f.env_ms, sr));
        Voice {
            inst,
            track,
            born,
            delay,
            gate,
            env: Adsr::new(i.env.a, i.env.d, i.env.s, i.env.r, sr),
            osc,
            filter,
            cutoff,
            fenv: 1.0,
            fenv_coef,
            hz,
            amp: vel.clamp(0.0, 1.5).powf(1.4) * i.gain,
            gl,
            gr,
            send: i.send,
            t: 0,
            vib_phase: rng.f(),
            lfo_phase: rng.f(),
            rng,
        }
    }

    /// Lets the note go now.
    pub fn release(&mut self) {
        if self.delay > 0 {
            // Never started: it simply does not.
            self.env = Adsr::new(0.0, 0.0, 0.0, 0.0, 48_000.0);
            self.env.release();
            self.delay = 0;
            self.gate = None;
            self.amp = 0.0;
            return;
        }
        // A string keeps ringing under the release; the release is how long it is let ring.
        self.gate = None;
        self.env.release();
    }

    /// Fades out in a few milliseconds (a voice taken for a newer note).
    pub fn steal(&mut self, sr: f32) {
        self.gate = None;
        self.env.release_in(8.0, sr);
    }

    pub fn done(&self) -> bool {
        self.delay == 0 && self.env.done()
    }

    pub fn level(&self) -> f32 {
        self.env.level * self.amp
    }

    /// Adds the voice into the dry and send buses.
    pub fn render(&mut self, prep: &Prepared, sr: f32, out: [&mut [f32]; 2], send: [&mut [f32]; 2]) {
        let i = &prep.inst;
        let [ol, or] = out;
        let [sl, sr_] = send;
        let n = ol.len();
        let vib_rate = i.vibrato[0] / sr;
        let vib_depth = i.vibrato[1];
        let vib_delay = (i.vibrato[2] * 0.001 * sr) as u32;
        let ramp = (0.3 * sr) as u32;
        let inv_sr = 1.0 / sr;
        let lfo = i.filter.map_or([0.0, 0.0], |f| f.lfo);
        let table_ix = prep.tables.as_ref().map(|t| t.for_hz(self.hz, sr));
        for k in 0..n {
            if self.delay > 0 {
                self.delay -= 1;
                continue;
            }
            if let Some(g) = &mut self.gate {
                if *g == 0 {
                    self.release();
                } else {
                    *g -= 1;
                }
            }
            let e = self.env.tick();
            if self.env.done() {
                break;
            }
            // Vibrato, easing in after its delay.
            let mut hz = self.hz;
            if vib_depth > 0.0 && self.t > vib_delay {
                let depth = vib_depth * ((self.t - vib_delay) as f32 / ramp as f32).min(1.0);
                self.vib_phase += vib_rate;
                hz *= 2f32.powf(depth * sin_cycles(self.vib_phase) / 12.0);
            }
            let dt = hz * inv_sr;
            let (mut l, mut r) = match &mut self.osc {
                Osc::Fm { pc, pm, pm2, fb, i1, i1_floor, i1_coef, i2, i2_coef } => {
                    *pm += dt * i.ratio;
                    *pm2 += dt * i.mod2[0];
                    *pc += dt;
                    let m1 = sin_cycles(*pm + *fb * i.feedback);
                    *fb = m1;
                    let m2 = if *i2 > 1e-4 { sin_cycles(*pm2) * *i2 } else { 0.0 };
                    let s = sin_cycles(*pc + (m1 * *i1 + m2) / std::f32::consts::TAU);
                    *i1 = *i1_floor + (*i1 - *i1_floor) * *i1_coef;
                    *i2 *= *i2_coef;
                    wrap(pc);
                    wrap(pm);
                    wrap(pm2);
                    (s, s)
                }
                Osc::Table { phases, ratios, pans, n, breath, pink, formants } => {
                    let t = table_ix.unwrap_or(&[]);
                    let (mut l, mut r) = (0.0, 0.0);
                    for u in 0..*n {
                        phases[u] += dt * ratios[u];
                        wrap(&mut phases[u]);
                        let s = if t.is_empty() { sin_cycles(phases[u]) } else { read_table(t, phases[u]) };
                        l += s * pans[u].0;
                        r += s * pans[u].1;
                    }
                    let norm = 1.0 / (*n as f32).sqrt();
                    l *= norm;
                    r *= norm;
                    if i.breath > 0.0 {
                        let b = breath.tick(pink.run(self.rng.bi())).1 * i.breath * 4.0;
                        l += b;
                        r += b;
                    }
                    if !formants.is_empty() {
                        let (mut fl, mut fr) = (0.0, 0.0);
                        let m = 0.5 * (l + r);
                        let side = 0.5 * (l - r);
                        for (f, row) in formants.iter_mut().zip(&i.formants) {
                            // A band's peak gain is its q: take it back out, so `gain` is the formant's own.
                            let b = f.tick(m).1 * row[2] / row[1];
                            fl += b;
                            fr += b;
                        }
                        l = fl + side * 0.5;
                        r = fr - side * 0.5;
                    }
                    (l, r)
                }
                Osc::Pluck(p) => {
                    let s = p.tick();
                    (s, s)
                }
                Osc::Bell { modes, strike, strike_coef, strike_f } => {
                    let mut s = modes.tick();
                    if *strike > 1e-4 {
                        s += strike_f.tick(self.rng.bi()).1 * *strike;
                        *strike *= *strike_coef;
                    }
                    (s, s)
                }
                Osc::Drum { phase, bend, bend_coef, click, click_coef, click_f } => {
                    *phase += dt * (1.0 + *bend);
                    *bend *= *bend_coef;
                    wrap(phase);
                    let mut s = sin_cycles(*phase);
                    if *click > 1e-4 {
                        s += click_f.tick(self.rng.bi()).1 * *click;
                        *click *= *click_coef;
                    }
                    (s, s)
                }
                Osc::Noise { pink } => {
                    let s = pink.run(self.rng.bi()) * 3.0;
                    (s, s)
                }
            };
            if let Some((f, kind)) = &mut self.filter {
                if self.t % 16 == 0 {
                    let fi = i.filter.unwrap_or_default();
                    let mut c = self.cutoff * 2f32.powf(fi.env * self.fenv);
                    if lfo[1] > 0.0 {
                        self.lfo_phase += lfo[0] * 16.0 * inv_sr;
                        c *= 2f32.powf(lfo[1] * sin_cycles(self.lfo_phase));
                    }
                    f.set(c, fi.q, sr);
                }
                self.fenv *= self.fenv_coef;
                if matches!(self.osc, Osc::Table { .. }) {
                    // A stereo voice's filter runs on the middle; the sides keep their width.
                    let m = 0.5 * (l + r);
                    let side = 0.5 * (l - r);
                    let fm = f.run(m, *kind);
                    l = fm + side * 0.6;
                    r = fm - side * 0.6;
                } else {
                    l = f.run(l, *kind);
                    r = l;
                }
            }
            let g = e * self.amp;
            // A table voice panned each of its unison voices already.
            let (vl, vr) =
                if matches!(self.osc, Osc::Table { .. }) { (l * g, r * g) } else { (l * g * self.gl, r * g * self.gr) };
            ol[k] += vl;
            or[k] += vr;
            sl[k] += vl * self.send;
            sr_[k] += vr * self.send;
            self.t += 1;
        }
    }
}

#[inline]
fn wrap(p: &mut f32) {
    if *p >= 1.0 {
        *p -= p.floor();
    }
}
