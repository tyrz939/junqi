//! Ambient beds (PRESENTATION.md §5): the loops under everything, made live so they never loop.
//! Rain on open ground and on a roof, wind, birds at dusk, crickets at night, the lake lapping,
//! the Works' hum, a cave's drips, a fire, a clock. Each fades to the level the cue table asks for
//! and sleeps at zero.

use crate::dsp::{Modal, OnePole, Pink, Rng, Svf, pan_gains, sin_cycles};

/// The beds, in the order the engine keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bed {
    Rain,
    /// Rain heard from inside: on the roof and the glass.
    RainRoof,
    Wind,
    Birds,
    Crickets,
    Lake,
    /// The Works: the mains and the machines that nobody runs.
    Hum,
    /// Underground: a low room tone and water dripping.
    Cave,
    Fire,
    Clock,
}

impl Bed {
    pub const ALL: [Bed; 10] = [
        Bed::Rain,
        Bed::RainRoof,
        Bed::Wind,
        Bed::Birds,
        Bed::Crickets,
        Bed::Lake,
        Bed::Hum,
        Bed::Cave,
        Bed::Fire,
        Bed::Clock,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Bed::Rain => "rain",
            Bed::RainRoof => "rain_roof",
            Bed::Wind => "wind",
            Bed::Birds => "birds",
            Bed::Crickets => "crickets",
            Bed::Lake => "lake",
            Bed::Hum => "hum",
            Bed::Cave => "cave",
            Bed::Fire => "fire",
            Bed::Clock => "clock",
        }
    }

    pub fn from_name(s: &str) -> Option<Bed> {
        Bed::ALL.into_iter().find(|b| b.name() == s)
    }

    /// What brings each bed at full level to its place under the music: about -30 dBFS for the
    /// steady ones (rain a little louder), the birds' and crickets' calls peaking near -17.
    /// Measured by `every_bed_makes_sound_and_none_clips`.
    const fn trim(self) -> f32 {
        match self {
            Bed::Rain | Bed::Lake => 0.34,
            Bed::RainRoof => 0.21,
            Bed::Wind => 0.18,
            Bed::Birds => 1.6,
            Bed::Crickets => 1.2,
            Bed::Hum => 0.146,
            Bed::Cave => 0.075,
            Bed::Fire => 0.23,
            Bed::Clock => 0.32,
        }
    }
}

/// A short event inside a bed (a drop, a chirp, a drip, a pop): a tone or a noise under an
/// envelope, panned.
#[derive(Clone, Debug)]
struct Grain {
    delay: u32,
    t: u32,
    len: u32,
    attack: u32,
    hz0: f32,
    hz1: f32,
    phase: f32,
    am: f32,
    noise: bool,
    filt: Svf,
    gain: f32,
    gl: f32,
    gr: f32,
    send: f32,
}

impl Grain {
    #[inline]
    fn step(&mut self, sr: f32, rng: &mut Rng) -> Option<(f32, f32, f32)> {
        if self.delay > 0 {
            self.delay -= 1;
            return Some((0.0, 0.0, 0.0));
        }
        if self.t >= self.len {
            return None;
        }
        let x = self.t as f32 / self.len as f32;
        let env = if self.t < self.attack {
            let a = self.t as f32 / self.attack.max(1) as f32;
            a * a * (3.0 - 2.0 * a)
        } else {
            let d = (self.t - self.attack) as f32 / (self.len - self.attack).max(1) as f32;
            (1.0 - d).powi(3)
        };
        let hz = self.hz0 * (self.hz1 / self.hz0).powf(x);
        self.phase += hz / sr;
        let mut s = if self.noise { self.filt.tick(rng.bi()).1 } else { sin_cycles(self.phase) };
        if self.am > 0.0 {
            s *= 0.5 + 0.5 * sin_cycles(self.am * self.t as f32 / sr);
        }
        self.t += 1;
        let v = s * env * self.gain;
        Some((v * self.gl, v * self.gr, self.send))
    }
}

/// One bed's state.
#[derive(Clone, Debug)]
pub struct BedVoice {
    pub bed: Bed,
    pub level: f32,
    pub target: f32,
    rng: Rng,
    pink: [Pink; 2],
    f: [Svf; 4],
    lp: [OnePole; 4],
    lfo: [f32; 4],
    walk: [f32; 4],
    countdown: u32,
    t: u64,
    grains: Vec<Grain>,
    clank: Option<(Modal, f32, f32)>,
}

impl BedVoice {
    pub fn new(bed: Bed, sr: f32, seed: u32) -> BedVoice {
        let (f, lp) = match bed {
            Bed::Rain => (
                [
                    Svf::new(500.0, 0.6, sr),
                    Svf::new(500.0, 0.6, sr),
                    Svf::new(6500.0, 0.6, sr),
                    Svf::new(6500.0, 0.6, sr),
                ],
                [OnePole::new(250.0, sr); 4],
            ),
            Bed::RainRoof => (
                [
                    Svf::new(180.0, 0.6, sr),
                    Svf::new(180.0, 0.6, sr),
                    Svf::new(1400.0, 0.7, sr),
                    Svf::new(1400.0, 0.7, sr),
                ],
                [OnePole::new(120.0, sr); 4],
            ),
            Bed::Wind => (
                [Svf::new(400.0, 0.9, sr), Svf::new(450.0, 0.9, sr), Svf::new(600.0, 5.0, sr), Svf::new(90.0, 0.7, sr)],
                [OnePole::new(1.0, sr); 4],
            ),
            Bed::Crickets => (
                [
                    Svf::new(4700.0, 3.0, sr),
                    Svf::new(4700.0, 3.0, sr),
                    Svf::new(100.0, 0.7, sr),
                    Svf::new(100.0, 0.7, sr),
                ],
                [OnePole::new(1.0, sr); 4],
            ),
            Bed::Lake => (
                [
                    Svf::new(700.0, 0.7, sr),
                    Svf::new(650.0, 0.7, sr),
                    Svf::new(1600.0, 2.0, sr),
                    Svf::new(120.0, 0.7, sr),
                ],
                [OnePole::new(2.0, sr); 4],
            ),
            Bed::Hum => (
                [
                    Svf::new(110.0, 0.7, sr),
                    Svf::new(2400.0, 4.0, sr),
                    Svf::new(100.0, 0.7, sr),
                    Svf::new(100.0, 0.7, sr),
                ],
                [OnePole::new(0.5, sr); 4],
            ),
            Bed::Cave => (
                [
                    Svf::new(140.0, 0.7, sr),
                    Svf::new(120.0, 0.7, sr),
                    Svf::new(100.0, 0.7, sr),
                    Svf::new(100.0, 0.7, sr),
                ],
                [OnePole::new(0.3, sr); 4],
            ),
            Bed::Fire => (
                [
                    Svf::new(380.0, 0.6, sr),
                    Svf::new(420.0, 0.6, sr),
                    Svf::new(3000.0, 0.7, sr),
                    Svf::new(100.0, 0.7, sr),
                ],
                [OnePole::new(3.0, sr); 4],
            ),
            Bed::Birds | Bed::Clock => ([Svf::new(100.0, 0.7, sr); 4], [OnePole::new(1.0, sr); 4]),
        };
        let mut rng = Rng::new(seed ^ crate::patch::hash(bed.name()));
        let lfo = [rng.f(), rng.f(), rng.f(), rng.f()];
        BedVoice {
            bed,
            level: 0.0,
            target: 0.0,
            rng,
            pink: [Pink::default(); 2],
            f,
            lp,
            lfo,
            walk: [0.5; 4],
            countdown: 0,
            t: 0,
            grains: Vec::new(),
            clank: None,
        }
    }

    /// Asleep: silent and wanted silent.
    pub fn idle(&self) -> bool {
        self.level < 1e-4 && self.target == 0.0 && self.grains.is_empty()
    }

    /// Adds `out.len()` samples of the bed at its level into the stereo dry and send buffers.
    pub fn render(&mut self, sr: f32, out: [&mut [f32]; 2], send: [&mut [f32]; 2]) {
        if self.idle() {
            self.level = 0.0;
            return;
        }
        let [ol, or] = out;
        let [sl, sr_] = send;
        // About two seconds from silence to full, so a bed swells in rather than switches on.
        let ramp = 1.0 / (2.0 * sr);
        for k in 0..ol.len() {
            if self.level < self.target {
                self.level = (self.level + ramp).min(self.target);
            } else if self.level > self.target {
                self.level = (self.level - ramp).max(self.target);
            }
            let lv = self.level;
            let (l, r, send_amt) = self.sample(sr, lv);
            let (mut gl, mut gr, mut gs) = (l * lv, r * lv, send_amt);
            // Grains run at the level they were born at, so a drip does not fade mid-fall.
            let rng = &mut self.rng;
            self.grains.retain_mut(|g| match g.step(sr, rng) {
                Some((a, b, s)) => {
                    gl += a;
                    gr += b;
                    gs = gs.max(s);
                    true
                }
                None => false,
            });
            let trim = self.bed.trim();
            ol[k] += gl * trim;
            or[k] += gr * trim;
            sl[k] += gl * gs * trim;
            sr_[k] += gr * gs * trim;
            self.t += 1;
        }
    }

    /// Spawns a grain.
    #[allow(clippy::too_many_arguments)]
    fn grain(
        &mut self,
        sr: f32,
        ms: f32,
        attack_ms: f32,
        hz: (f32, f32),
        noise: Option<(f32, f32)>,
        gain: f32,
        pan: f32,
        send: f32,
    ) {
        if self.grains.len() >= 48 {
            return;
        }
        let (gl, gr) = pan_gains(pan);
        let (fc, q) = noise.unwrap_or((1000.0, 1.0));
        self.grains.push(Grain {
            delay: 0,
            t: 0,
            len: (ms * 0.001 * sr) as u32,
            attack: (attack_ms.max(1.0) * 0.001 * sr) as u32,
            hz0: hz.0,
            hz1: hz.1,
            phase: 0.0,
            am: 0.0,
            noise: noise.is_some(),
            filt: Svf::new(fc, q, sr),
            gain: gain * self.level,
            gl,
            gr,
            send,
        });
    }

    /// A slow random walk in 0..1, one per slot, stepped every sample.
    fn walk(&mut self, i: usize, speed: f32) -> f32 {
        let v = self.walk[i] + self.rng.bi() * speed;
        self.walk[i] = v.clamp(0.0, 1.0);
        self.walk[i]
    }

    /// The continuous part of the bed, and how much of it goes to the reverb.
    fn sample(&mut self, sr: f32, lv: f32) -> (f32, f32, f32) {
        let t = self.t;
        match self.bed {
            Bed::Rain | Bed::RainRoof => {
                let roof = self.bed == Bed::RainRoof;
                let a = self.pink[0].run(self.rng.bi());
                let b = self.pink[1].run(self.rng.bi());
                let hl = self.f[0].tick(a).2;
                let hr = self.f[1].tick(b).2;
                let l = self.f[2].tick(hl).0;
                let r = self.f[3].tick(hr).0;
                // Heavy rain has a body under it.
                let w = self.rng.bi();
                let body = self.lp[0].lp(w) * 2.5 * (lv - 0.5).max(0.0);
                // Drops: more of them the harder it rains.
                let rate = if roof { 18.0 } else { 45.0 } * lv * lv;
                if self.rng.f() < rate / sr {
                    let hz = if roof { 700.0 + self.rng.f() * 900.0 } else { 1800.0 + self.rng.f() * 3500.0 };
                    let pan = self.rng.bi() * 0.9;
                    let g = 0.08 + self.rng.f() * 0.12;
                    let ms = 6.0 + self.rng.f() * 10.0;
                    self.grain(sr, ms, 0.5, (hz, hz * 0.8), Some((hz, 4.0)), g, pan, 0.2);
                }
                let k = if roof { 0.9 } else { 0.55 };
                (l * k + body, r * k + body, 0.12)
            }
            Bed::Wind => {
                // Gusts: a slow walk on the level and the colour; a whistle when it blows hard.
                if t % 32 == 0 {
                    let g = self.walk(0, 0.004);
                    let c = self.walk(1, 0.003);
                    let hz = 250.0 + 700.0 * c;
                    self.f[0].set(hz, 0.9, sr);
                    self.f[1].set(hz * 1.13, 0.9, sr);
                    let w = self.walk(2, 0.002);
                    self.f[2].set(420.0 + 500.0 * w, 7.0, sr);
                    self.lfo[0] = 0.35 + 0.65 * g;
                }
                let a = self.pink[0].run(self.rng.bi());
                let b = self.pink[1].run(self.rng.bi());
                let gust = self.lp[0].lp(self.lfo[0]);
                let low = self.f[3].tick(a + b).0 * 0.6;
                let whistle = self.f[2].tick(a).1 * 0.35 * (lv - 0.6).max(0.0) * 2.0;
                let l = (self.f[0].tick(a).1 * 1.4 + low + whistle) * gust;
                let r = (self.f[1].tick(b).1 * 1.4 + low + whistle) * gust;
                (l, r, 0.1)
            }
            Bed::Birds => {
                // A phrase every few seconds from somewhere in the hedges.
                if self.countdown == 0 {
                    self.countdown = (sr * (1.2 + self.rng.f() * 3.5)) as u32;
                    let pan = self.rng.bi() * 0.85;
                    let kind = self.rng.range(0, 2);
                    let mut at = 0.0f32;
                    let notes = self.rng.range(2, 6);
                    let base = 1700.0 + self.rng.f() * 1800.0;
                    for n in 0..notes {
                        let (ms, h0, h1, gap) = match kind {
                            // A thrush: fluting notes that step about.
                            0 => {
                                let h = base * (1.0 + 0.18 * self.rng.bi());
                                (90.0 + self.rng.f() * 70.0, h, h * (1.0 + 0.1 * self.rng.bi()), 60.0)
                            }
                            // A wren or a robin: quick falling chips.
                            1 => {
                                let h = base * 1.6;
                                (35.0 + self.rng.f() * 20.0, h * 1.25, h * 0.8, 40.0)
                            }
                            // A slow rising call.
                            _ => {
                                let h = base * 0.8;
                                (140.0, h * 0.85, h * 1.2, 110.0)
                            }
                        };
                        let g = 0.05 + 0.04 * self.rng.f();
                        let (gl, gr) = pan_gains(pan);
                        let len = (ms * 0.001 * sr) as u32;
                        self.grains.push(Grain {
                            delay: (at * 0.001 * sr) as u32,
                            t: 0,
                            len,
                            attack: (0.008 * sr) as u32,
                            hz0: h0,
                            hz1: h1,
                            phase: 0.0,
                            am: if kind == 0 && n % 2 == 1 { 28.0 } else { 0.0 },
                            noise: false,
                            filt: Svf::new(1000.0, 1.0, sr),
                            gain: g * lv,
                            gl,
                            gr,
                            send: 0.45,
                        });
                        at += ms + gap + self.rng.f() * 50.0;
                    }
                }
                self.countdown -= 1;
                (0.0, 0.0, 0.4)
            }
            Bed::Crickets => {
                // Three crickets, each chirping in its own time: a burst of three or four pulses
                // of a 4.5 kHz tone, over a thin hiss of the far ones.
                let mut l = 0.0;
                let mut r = 0.0;
                for c in 0..3usize {
                    let rate = [0.71, 0.83, 0.97][c];
                    let hz = [4420.0, 4710.0, 4550.0][c];
                    let pan = [-0.6, 0.5, 0.05][c];
                    let gain = [0.05, 0.035, 0.022][c];
                    let cyc = (t as f32 / sr * rate + self.lfo[c]).fract();
                    // The burst: four pulses at 30 Hz in the first 0.13 of a cycle.
                    let pulse = if cyc < 0.13 {
                        let p = (cyc / 0.13 * 4.0).fract();
                        (sin_cycles(p * 0.5)).powi(2)
                    } else {
                        0.0
                    };
                    let s = sin_cycles(hz * t as f32 / sr) * pulse * gain;
                    let (gl, gr) = pan_gains(pan);
                    l += s * gl;
                    r += s * gr;
                }
                let far = self.f[0].tick(self.rng.bi()).1 * 0.02;
                let far2 = self.f[1].tick(self.rng.bi()).1 * 0.02;
                (l + far, r + far2, 0.2)
            }
            Bed::Lake => {
                // Lapping: a wave's swell every couple of seconds, low and soft, and the small
                // splash at its top.
                if self.countdown == 0 {
                    self.countdown = (sr * (1.6 + self.rng.f() * 2.4)) as u32;
                    self.lfo[1] = 0.0;
                    self.lfo[2] = 0.3 + 0.7 * self.rng.f();
                    self.lfo[3] = self.rng.bi() * 0.6;
                }
                self.countdown -= 1;
                let wave_len = 1.4 * sr;
                self.lfo[1] += 1.0;
                let x = (self.lfo[1] / wave_len).min(1.0);
                let swell =
                    if x < 0.3 { (x / 0.3).powi(2) } else { (1.0 - (x - 0.3) / 0.7).max(0.0).powi(2) } * self.lfo[2];
                if (self.lfo[1] - (0.3 * wave_len)).abs() < 0.5 && self.rng.f() < 0.7 {
                    let hz = 900.0 + self.rng.f() * 800.0;
                    let pan = self.lfo[3];
                    self.grain(sr, 90.0, 6.0, (hz, hz), Some((hz, 1.5)), 0.12 * self.lfo[2], pan, 0.25);
                }
                let a = self.pink[0].run(self.rng.bi());
                let b = self.pink[1].run(self.rng.bi());
                let (pl, pr) = pan_gains(self.lfo[3]);
                let l = self.f[0].tick(a).0 * (0.25 + swell) * pl * 1.4;
                let r = self.f[1].tick(b).0 * (0.25 + swell) * pr * 1.4;
                (l, r, 0.15)
            }
            Bed::Hum => {
                // Mains hum and its harmonics, beating slowly, a rumble under it, and now and
                // then iron knocking somewhere nobody is.
                let ts = t as f32 / sr;
                let breathe = 0.8 + 0.2 * sin_cycles(0.07 * ts + self.lfo[0]);
                let h = sin_cycles(50.0 * ts) * 0.5
                    + sin_cycles(100.3 * ts) * 0.35
                    + sin_cycles(99.8 * ts) * 0.2
                    + sin_cycles(150.0 * ts) * 0.12
                    + sin_cycles(200.4 * ts) * 0.06;
                let h = (h * breathe * 0.9).tanh() * 0.3;
                let rumble = self.f[0].tick(self.pink[0].run(self.rng.bi())).0 * 0.9;
                let buzz = self.f[1].tick(self.rng.bi()).1 * 0.012 * breathe;
                if self.countdown == 0 {
                    self.countdown = (sr * (5.0 + self.rng.f() * 11.0)) as u32;
                    let hz = 90.0 + self.rng.f() * 80.0;
                    let modes = [[1.0, 0.5, 1.6], [2.76, 0.3, 0.9], [5.4, 0.2, 0.5], [8.93, 0.1, 0.3]];
                    self.clank =
                        Some((Modal::new(hz, &modes, 0.0, sr, 1.0), 0.14 + 0.08 * self.rng.f(), self.rng.bi() * 0.8));
                }
                self.countdown -= 1;
                let (mut cl, mut cr) = (0.0, 0.0);
                if let Some((m, g, pan)) = &mut self.clank {
                    let s = m.tick() * *g;
                    let (gl, gr) = pan_gains(*pan);
                    cl = s * gl;
                    cr = s * gr;
                }
                (h + rumble + buzz + cl, h + rumble - buzz + cr, 0.25)
            }
            Bed::Cave => {
                let ts = t as f32 / sr;
                let tone = 0.7 + 0.3 * sin_cycles(0.05 * ts + self.lfo[0]);
                let l = self.f[0].tick(self.pink[0].run(self.rng.bi())).0 * 1.6 * tone;
                let r = self.f[1].tick(self.pink[1].run(self.rng.bi())).0 * 1.6 * tone;
                if self.countdown == 0 {
                    self.countdown = (sr * (0.9 + self.rng.f() * 4.0)) as u32;
                    // A drip: a little rising "plip", far off and wet.
                    let hz = 700.0 + self.rng.f() * 900.0;
                    let pan = self.rng.bi() * 0.9;
                    let g = 0.06 + 0.06 * self.rng.f();
                    self.grain(sr, 45.0, 2.0, (hz, hz * 2.1), None, g, pan, 0.7);
                }
                self.countdown -= 1;
                (l, r, 0.3)
            }
            Bed::Fire => {
                let ts = t as f32 / sr;
                let roar = 0.6 + 0.4 * sin_cycles(0.3 * ts + self.lfo[0]) * sin_cycles(0.11 * ts);
                let a = self.pink[0].run(self.rng.bi());
                let b = self.pink[1].run(self.rng.bi());
                let l = self.f[0].tick(a).0 * roar * 0.9;
                let r = self.f[1].tick(b).0 * roar * 0.9;
                if self.rng.f() < 9.0 / sr {
                    // A crackle: a tiny bright pop, sometimes a cluster.
                    let n = self.rng.range(1, 3);
                    let pan = self.rng.bi() * 0.4;
                    for _ in 0..n {
                        let hz = 2000.0 + self.rng.f() * 4000.0;
                        let g = 0.05 + self.rng.f() * 0.15;
                        let ms = 2.0 + self.rng.f() * 5.0;
                        self.grain(sr, ms, 0.3, (hz, hz), Some((hz, 0.8)), g, pan, 0.1);
                    }
                }
                (l, r, 0.1)
            }
            Bed::Clock => {
                // Tick, tock: a second apart, woody.
                let period = sr as u64;
                if t % period == 0 {
                    let tock = (t / period) % 2 == 1;
                    let hz = if tock { 1900.0 } else { 2500.0 };
                    self.grain(sr, 18.0, 0.4, (hz, hz * 0.9), Some((hz, 6.0)), 0.25, 0.25, 0.35);
                }
                (0.0, 0.0, 0.35)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bed_makes_sound_and_none_clips() {
        let sr = 48_000.0;
        for bed in Bed::ALL {
            let mut b = BedVoice::new(bed, sr, 3);
            b.target = 1.0;
            b.level = 1.0;
            let n = (sr * 6.0) as usize;
            let (mut l, mut r, mut sl, mut sr_) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
            b.render(sr, [&mut l, &mut r], [&mut sl, &mut sr_]);
            let p = crate::analysis::peak(&l).max(crate::analysis::peak(&r));
            let rms = crate::analysis::rms(&l);
            println!("{bed:?}: peak {:.1} dB, rms {:.1} dB", crate::analysis::to_db(p), crate::analysis::to_db(rms));
            assert!(p < 0.25, "{bed:?} peaks {p}");
            assert!(
                (-40.0..-24.0).contains(&crate::analysis::to_db(rms)) || matches!(bed, Bed::Birds | Bed::Clock),
                "{bed:?}"
            );
            assert!(rms > 1e-5, "{bed:?} is silent");
            assert!(crate::analysis::dc(&l).abs() < 0.01, "{bed:?} has DC");
            assert_eq!(Bed::from_name(bed.name()), Some(bed));
        }
    }

    #[test]
    fn a_bed_asked_for_silence_falls_asleep() {
        let sr = 48_000.0;
        let mut b = BedVoice::new(Bed::Wind, sr, 1);
        b.level = 1.0;
        b.target = 0.0;
        let n = (sr * 3.0) as usize;
        let (mut l, mut r, mut sl, mut sr_) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        b.render(sr, [&mut l, &mut r], [&mut sl, &mut sr_]);
        assert!(b.idle());
        assert!(crate::analysis::clicks(&l, sr).is_empty(), "a fade, not a cut");
    }
}
