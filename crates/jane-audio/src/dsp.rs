//! The building blocks (PRESENTATION.md §5): a sine by polynomial, band-limited edges, noise,
//! envelopes, a state-variable filter, a plucked string, a bell's modes, the shared reverb and
//! the limiter. Floats throughout: audio is presentation and never feeds the sim.

use std::f32::consts::{PI, TAU};

/// A small, fast, seeded noise source (xorshift32). Deterministic: the same seed makes the same
/// sound on every machine, so a rendered patch is the same bytes everywhere.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        // Never zero; mix the seed so neighbours differ from the first draw.
        let mut s = seed.wrapping_mul(0x9e37_79b9) ^ 0x6a09_e667;
        if s == 0 {
            s = 0x1234_5678;
        }
        let mut r = Rng(s);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform in [-1, 1).
    pub fn bi(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }

    /// Uniform in `[lo, hi]`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo { lo } else { lo + self.next_u32() % (hi - lo + 1) }
    }
}

/// `sin(2 pi p)` for `p` in cycles, by symmetry and an odd polynomial (error under 1e-6).
#[inline]
pub fn sin_cycles(p: f32) -> f32 {
    let p = p - p.floor();
    let x = if p < 0.25 {
        p
    } else if p < 0.75 {
        0.5 - p
    } else {
        p - 1.0
    };
    let x = x * TAU;
    let x2 = x * x;
    x * (1.0
        + x2 * (-1.0 / 6.0
            + x2 * (1.0 / 120.0 + x2 * (-1.0 / 5040.0 + x2 * (1.0 / 362_880.0 + x2 * (-1.0 / 39_916_800.0))))))
}

/// The PolyBLEP correction at phase `t` (cycles) for a phase step `dt`: rounds a saw's or a
/// pulse's jump so it does not alias.
#[inline]
pub fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Hz of a MIDI note.
pub fn midi_hz(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

/// Linear gain of decibels.
pub fn db(x: f32) -> f32 {
    10f32.powf(x / 20.0)
}

/// Per-sample multiplier that falls 60 dB in `ms`.
pub fn fall_coef(ms: f32, sr: f32) -> f32 {
    let n = (ms.max(0.1) * 0.001 * sr).max(1.0);
    0.001f32.powf(1.0 / n)
}

/// Equal-power pan: -1 left, 1 right.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos(), a.sin())
}

/// Pink noise from white (Paul Kellet's economy filter): wind, surf, breath.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pink {
    b: [f32; 3],
}

impl Pink {
    pub fn run(&mut self, white: f32) -> f32 {
        self.b[0] = 0.99765 * self.b[0] + white * 0.099_046;
        self.b[1] = 0.963 * self.b[1] + white * 0.296_516_4;
        self.b[2] = 0.57 * self.b[2] + white * 1.052_691_3;
        (self.b[0] + self.b[1] + self.b[2] + white * 0.1848) * 0.2
    }
}

/// An ADSR with a straight attack and exponential decay and release: it starts from zero and
/// ends at zero, so no note clicks on or off.
#[derive(Clone, Copy, Debug)]
pub struct Adsr {
    attack_step: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    pub level: f32,
    pub stage: Stage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Attack,
    Decay,
    Release,
    Done,
}

/// The shortest attack and release anything gets: under this a step is a click.
pub const MIN_EDGE_MS: f32 = 1.5;

impl Adsr {
    pub fn new(a_ms: f32, d_ms: f32, s: f32, r_ms: f32, sr: f32) -> Adsr {
        let a = (a_ms.max(MIN_EDGE_MS) * 0.001 * sr).max(1.0);
        Adsr {
            attack_step: 1.0 / a,
            decay: fall_coef(d_ms, sr),
            sustain: s.clamp(0.0, 1.0),
            release: fall_coef(r_ms.max(MIN_EDGE_MS * 4.0), sr),
            level: 0.0,
            stage: Stage::Attack,
        }
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Done {
            self.stage = Stage::Release;
        }
    }

    /// Sets a new release time and lets go (a stolen voice fades out fast).
    pub fn release_in(&mut self, ms: f32, sr: f32) {
        self.release = fall_coef(ms, sr);
        self.release();
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            Stage::Attack => {
                // A raised curve over the straight ramp: the first samples move gently.
                self.level += self.attack_step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
                let x = self.level;
                return x * x * (3.0 - 2.0 * x);
            }
            Stage::Decay => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay;
                if self.sustain == 0.0 && self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Done;
                }
            }
            Stage::Release => {
                self.level *= self.release;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Done;
                }
            }
            Stage::Done => self.level = 0.0,
        }
        self.level
    }

    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }
}

/// Andrew Simper's trapezoidal state-variable filter: stable under fast sweeps, low-, band- and
/// high-pass from one state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    ic1: f32,
    ic2: f32,
}

impl Svf {
    pub fn new(hz: f32, q: f32, sr: f32) -> Svf {
        let mut f = Svf::default();
        f.set(hz, q, sr);
        f
    }

    pub fn set(&mut self, hz: f32, q: f32, sr: f32) {
        let hz = hz.clamp(10.0, sr * 0.45);
        let g = (PI * hz / sr).tan();
        self.k = 1.0 / q.max(0.1);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    /// (low, band, high).
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - self.k * v1 - v2)
    }

    #[inline]
    pub fn run(&mut self, x: f32, kind: crate::model::FilterKind) -> f32 {
        let (l, b, h) = self.tick(x);
        match kind {
            crate::model::FilterKind::Lp => l,
            crate::model::FilterKind::Bp => b,
            crate::model::FilterKind::Hp => h,
        }
    }
}

/// A one-pole low-pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    pub a: f32,
    pub y: f32,
}

impl OnePole {
    pub fn new(hz: f32, sr: f32) -> OnePole {
        OnePole { a: 1.0 - (-TAU * hz / sr).exp(), y: 0.0 }
    }

    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

/// Takes the DC out (a 5 Hz high-pass).
#[derive(Clone, Copy, Debug)]
pub struct DcBlock {
    r: f32,
    x1: f32,
    y1: f32,
}

impl DcBlock {
    pub fn new(sr: f32) -> DcBlock {
        DcBlock { r: 1.0 - TAU * 5.0 / sr, x1: 0.0, y1: 0.0 }
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// Karplus-Strong: a noise burst in a tuned delay line, averaged each pass, so the high partials
/// die first as a real string's do. An all-pass takes up the fraction of a sample the tuning
/// needs, so a string is in tune at every pitch.
#[derive(Clone, Debug)]
pub struct Pluck {
    buf: Vec<f32>,
    idx: usize,
    last: f32,
    c: f32,
    ap_x: f32,
    ap_y: f32,
    g: f32,
}

impl Pluck {
    /// A string at `hz`; `bright` 0 to 1 colours the pluck; `t60` is seconds to fall 60 dB;
    /// `pick` 0 to 0.5 is where it is plucked (a comb in the burst).
    pub fn new(hz: f32, sr: f32, bright: f32, t60: f32, pick: f32, rng: &mut Rng) -> Pluck {
        let d = (sr / hz.max(20.0) - 0.5).max(2.2);
        let mut n = d.floor();
        let mut frac = d - n;
        if frac < 0.2 {
            n -= 1.0;
            frac += 1.0;
        }
        let n = n as usize;
        let c = (1.0 - frac) / (1.0 + frac);
        // Loop gain for the decay, over the averaging filter's own loss at this pitch.
        let trips = (t60.max(0.05) * hz).max(1.0);
        let h = (PI * hz / sr).cos().max(0.05);
        let g = (0.001f32.powf(1.0 / trips) / h).min(0.9995);
        // The burst: noise through a one-pole whose corner the brightness sets, then the comb.
        let mut lp = OnePole::new(200.0 + 9000.0 * bright * bright, sr);
        let mut buf: Vec<f32> = (0..n).map(|_| lp.lp(rng.bi())).collect();
        let p = ((pick.clamp(0.0, 0.5) * n as f32) as usize).max(1);
        if pick > 0.0 && p < n {
            let orig = buf.clone();
            for i in 0..n {
                buf[i] = orig[i] - orig[(i + n - p) % n];
            }
        }
        let mean = buf.iter().sum::<f32>() / n as f32;
        let peak = buf.iter().map(|x| (x - mean).abs()).fold(1e-6, f32::max);
        for x in &mut buf {
            *x = (*x - mean) / peak;
        }
        Pluck { buf, idx: 0, last: 0.0, c, ap_x: 0.0, ap_y: 0.0, g }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        let out = self.buf[self.idx];
        let avg = 0.5 * (out + self.last);
        self.last = out;
        let ap = self.c * avg + self.ap_x - self.c * self.ap_y;
        self.ap_x = avg;
        self.ap_y = ap;
        self.buf[self.idx] = ap * self.g;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        out
    }
}

/// One decaying mode: a complex phasor turned and shrunk each sample.
#[derive(Clone, Copy, Debug)]
pub struct Mode {
    re: f32,
    im: f32,
    cr: f32,
    ci: f32,
    amp: f32,
}

impl Mode {
    pub fn new(hz: f32, amp: f32, t60: f32, sr: f32) -> Mode {
        let w = TAU * hz / sr;
        let r = 0.001f32.powf(1.0 / (t60.max(0.005) * sr));
        // Starts at phase zero: the sine term is zero, so the strike does not step.
        Mode { re: 1.0, im: 0.0, cr: r * w.cos(), ci: r * w.sin(), amp }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        let re = self.re * self.cr - self.im * self.ci;
        let im = self.re * self.ci + self.im * self.cr;
        self.re = re;
        self.im = im;
        im * self.amp
    }
}

/// A struck bell's modes: each partial a pair a few tenths of a hertz apart, so it beats as a
/// bell does while it hums.
#[derive(Clone, Debug)]
pub struct Modal {
    modes: Vec<Mode>,
}

impl Modal {
    /// `modes` are `[ratio, amplitude, t60 seconds]` over `hz`; modes over the Nyquist are left out.
    pub fn new(hz: f32, modes: &[[f32; 3]], beat: f32, sr: f32, decay_scale: f32) -> Modal {
        let mut v = Vec::with_capacity(modes.len() * 2);
        for &[ratio, amp, t60] in modes {
            let f = hz * ratio;
            if f >= sr * 0.45 {
                continue;
            }
            let t = t60 * decay_scale;
            if beat > 0.0 {
                let b = beat * (0.6 + 0.4 * ratio.min(3.0) / 3.0);
                v.push(Mode::new(f - b * 0.5, amp * 0.5, t, sr));
                v.push(Mode::new(f + b * 0.5, amp * 0.5, t * 0.93, sr));
            } else {
                v.push(Mode::new(f, amp, t, sr));
            }
        }
        Modal { modes: v }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        self.modes.iter_mut().map(Mode::tick).sum()
    }
}

/// A single-cycle table of harmonics, band-limited: one per harmonic count, so a high note
/// reads a table with fewer harmonics and nothing folds back.
#[derive(Clone, Debug)]
pub struct Tables {
    /// (harmonics kept, samples).
    levels: Vec<(u32, Vec<f32>)>,
}

pub const TABLE_LEN: usize = 2048;

impl Tables {
    /// `amps[h-1]` is harmonic h's amplitude.
    pub fn new(amps: &[f32]) -> Tables {
        let n = amps.len().max(1) as u32;
        let mut counts = vec![n];
        let mut c = n;
        while c > 1 {
            c = c.div_ceil(2);
            counts.push(c);
        }
        let levels = counts
            .into_iter()
            .map(|keep| {
                let mut t = vec![0.0f32; TABLE_LEN + 1];
                for (h, &a) in amps.iter().enumerate().take(keep as usize) {
                    if a == 0.0 {
                        continue;
                    }
                    let h1 = (h + 1) as f32;
                    for (i, s) in t.iter_mut().enumerate().take(TABLE_LEN) {
                        *s += a * sin_cycles(h1 * i as f32 / TABLE_LEN as f32);
                    }
                }
                let peak = t.iter().fold(1e-6f32, |m, x| m.max(x.abs()));
                for s in &mut t {
                    *s /= peak;
                }
                t[TABLE_LEN] = t[0];
                (keep, t)
            })
            .collect();
        Tables { levels }
    }

    /// The table for a note at `hz`: the most harmonics that stay under the Nyquist.
    pub fn for_hz(&self, hz: f32, sr: f32) -> &[f32] {
        let allowed = (sr * 0.45 / hz.max(1.0)) as u32;
        self.levels.iter().find(|(k, _)| *k <= allowed).unwrap_or(&self.levels[self.levels.len() - 1]).1.as_slice()
    }
}

/// Reads a table at `phase` (cycles), linearly.
#[inline]
pub fn read_table(t: &[f32], phase: f32) -> f32 {
    let p = (phase - phase.floor()) * TABLE_LEN as f32;
    let i = p as usize;
    let f = p - i as f32;
    let a = t[i.min(TABLE_LEN - 1)];
    let b = t[(i + 1).min(TABLE_LEN)];
    a + (b - a) * f
}

/// A delay line of whole samples.
#[derive(Clone, Debug)]
struct Delay {
    buf: Vec<f32>,
    idx: usize,
}

impl Delay {
    fn new(n: usize) -> Delay {
        Delay { buf: vec![0.0; n.max(1)], idx: 0 }
    }

    #[inline]
    fn read(&self) -> f32 {
        self.buf[self.idx]
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.idx] = x;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
    }
}

/// A Schroeder all-pass: diffuses without colouring.
#[derive(Clone, Debug)]
struct AllPass {
    d: Delay,
    g: f32,
}

impl AllPass {
    fn new(n: usize, g: f32) -> AllPass {
        AllPass { d: Delay::new(n), g }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let z = self.d.read();
        let v = x + self.g * z;
        self.d.write(v);
        z - self.g * v
    }
}

/// The shared reverb: a small room with a long, dark tail. Eight delay lines under a Householder
/// feedback matrix (a feedback delay network), each damped by a one-pole so the tail darkens
/// as it dies, fed through a pre-delay and two diffusers. One for the whole mix: every sound sits
/// in the same air.
#[derive(Clone, Debug)]
pub struct Reverb {
    pre: Delay,
    diff: [AllPass; 2],
    lines: Vec<Delay>,
    gains: Vec<f32>,
    damp: Vec<OnePole>,
    hp: [DcBlock; 2],
    tone: [OnePole; 2],
}

impl Reverb {
    /// `t60` seconds of tail; `size` scales the lines (1 a parlour, 1.6 a church).
    pub fn new(sr: f32, t60: f32, size: f32) -> Reverb {
        // Mutually prime lengths in milliseconds.
        const MS: [f32; 8] = [31.1, 37.3, 41.9, 47.3, 53.1, 59.9, 67.3, 73.1];
        let lines: Vec<Delay> = MS.iter().map(|m| Delay::new((m * size * 0.001 * sr) as usize)).collect();
        let gains = lines.iter().map(|d| 0.001f32.powf(d.buf.len() as f32 / (t60 * sr))).collect();
        let damp = (0..8).map(|_| OnePole::new(5200.0, sr)).collect();
        Reverb {
            pre: Delay::new((0.018 * sr) as usize),
            diff: [AllPass::new((0.0071 * sr) as usize, 0.62), AllPass::new((0.0113 * sr) as usize, 0.58)],
            lines,
            gains,
            damp,
            hp: [DcBlock::new(sr), DcBlock::new(sr)],
            tone: [OnePole::new(7000.0, sr), OnePole::new(7000.0, sr)],
        }
    }

    /// One stereo sample in, the wet signal out.
    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let x = self.pre.read();
        self.pre.write((l + r) * 0.5);
        let x = self.diff[0].run(x);
        let x = self.diff[1].run(x);
        let mut outs = [0.0f32; 8];
        let mut sum = 0.0;
        for (i, o) in outs.iter_mut().enumerate() {
            *o = self.damp[i].lp(self.lines[i].read()) * self.gains[i];
            sum += *o;
        }
        let k = sum * (2.0 / 8.0);
        for (i, o) in outs.iter().enumerate() {
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            self.lines[i].write(o - k + x * sign * 0.35);
        }
        let wl = outs[0] - outs[2] + outs[4] - outs[6] + 0.5 * (outs[1] - outs[5]);
        let wr = outs[1] - outs[3] + outs[5] - outs[7] + 0.5 * (outs[2] - outs[6]);
        (self.tone[0].lp(self.hp[0].run(wl)) * 0.6, self.tone[1].lp(self.hp[1].run(wr)) * 0.6)
    }
}

/// A look-ahead peak limiter, linked across the two channels: the mix never passes the ceiling,
/// and it bends rather than clips.
#[derive(Clone, Debug)]
pub struct Limiter {
    ceiling: f32,
    look: usize,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    /// The gains wanted over the window, and a sliding minimum over them.
    want: Vec<f32>,
    idx: usize,
    gain: f32,
    release: f32,
}

impl Limiter {
    pub fn new(sr: f32, ceiling: f32) -> Limiter {
        let look = (0.004 * sr) as usize;
        Limiter {
            ceiling,
            look,
            buf_l: vec![0.0; look],
            buf_r: vec![0.0; look],
            want: vec![1.0; look],
            idx: 0,
            gain: 1.0,
            release: 1.0 - (-1.0 / (0.12 * sr)).exp(),
        }
    }

    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        let want = if peak > self.ceiling { self.ceiling / peak } else { 1.0 };
        let out_l = self.buf_l[self.idx];
        let out_r = self.buf_r[self.idx];
        self.buf_l[self.idx] = l;
        self.buf_r[self.idx] = r;
        self.want[self.idx] = want;
        self.idx = (self.idx + 1) % self.look;
        // The least gain anything in the window needs (the window is a few ms: a plain scan).
        let floor = self.want.iter().copied().fold(1.0f32, f32::min);
        if floor < self.gain {
            // Down within the look-ahead, so the gain is there before the peak is; what little
            // is left over at the peak the clamp below takes.
            self.gain += (floor - self.gain) * (8.0 / self.look as f32).min(1.0);
        } else {
            self.gain += (floor - self.gain) * self.release;
        }
        let g = self.gain;
        let c = self.ceiling;
        ((out_l * g).clamp(-c, c), (out_r * g).clamp(-c, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_polynomial_sine_is_a_sine() {
        for i in 0..1000 {
            let p = i as f32 / 997.0 - 0.3;
            assert!((sin_cycles(p) - (p * TAU).sin()).abs() < 2e-6, "{p}");
        }
    }

    #[test]
    fn an_envelope_starts_and_ends_at_zero_and_never_steps() {
        let sr = 48_000.0;
        let mut e = Adsr::new(0.0, 200.0, 0.5, 0.0, sr);
        let mut last = 0.0f32;
        let mut max_step = 0.0f32;
        for i in 0..20_000 {
            if i == 10_000 {
                e.release();
            }
            let v = e.tick();
            max_step = max_step.max((v - last).abs());
            last = v;
        }
        assert!(e.done());
        assert_eq!(last, 0.0);
        // The shortest attack is 1.5 ms: no step over 1/72 of full scale in a sample.
        assert!(max_step < 1.6 / (MIN_EDGE_MS * 0.001 * sr), "{max_step}");
    }

    #[test]
    fn a_plucked_string_is_in_tune() {
        let sr = 48_000.0;
        for hz in [82.41f32, 220.0, 659.26] {
            let mut p = Pluck::new(hz, sr, 0.5, 2.0, 0.2, &mut Rng::new(1));
            let x: Vec<f32> = (0..(sr as usize / 2)).map(|_| p.tick()).collect();
            let got = crate::analysis::pitch(&x[2000..], sr).unwrap();
            let cents = 1200.0 * (got / hz).log2();
            assert!(cents.abs() < 6.0, "{hz}: {got} ({cents} cents)");
        }
    }

    #[test]
    fn the_limiter_holds_the_ceiling() {
        let mut l = Limiter::new(48_000.0, 0.9);
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = sin_cycles(i as f32 * 440.0 / 48_000.0) * if i > 10_000 { 3.0 } else { 0.5 };
            let (a, b) = l.run(x, -x);
            peak = peak.max(a.abs()).max(b.abs());
        }
        assert!(peak <= 0.9 + 1e-6, "{peak}");
    }

    #[test]
    fn the_reverb_dies_away() {
        let mut r = Reverb::new(48_000.0, 1.5, 1.0);
        let mut e_late = 0.0f32;
        for i in 0..(48_000 * 4) {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (a, b) = r.run(x, x);
            if i > 48_000 * 3 {
                e_late = e_late.max(a.abs()).max(b.abs());
            }
        }
        assert!(e_late < 1e-3, "{e_late}");
    }
}
