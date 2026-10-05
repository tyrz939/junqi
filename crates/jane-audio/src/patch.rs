//! Sound effects rendered at boot (PRESENTATION.md §5): each `SfxPatch` row of
//! `data/audio/sfx.json` becomes a few mono buffers (its variants), so playing one in the game is
//! a copy, never a synthesis.

use crate::dsp::{DcBlock, Modal, Pink, Pluck, Rng, Svf, poly_blep, sin_cycles};
use crate::model::{Layer, SfxPatch, Wave};

/// A struck tower bell's modes over its prime (strike note), after the partials measured on
/// English church bells: the hum an octave under, the prime, the minor-third tierce that makes a
/// bell sound sad, the quint, the nominal an octave over, and the high partials that make the
/// clang. `[ratio, amplitude, seconds to fall 60 dB]` for a bell of about 147 Hz; a bigger bell
/// rings longer. Not one of them is a whole multiple of the hum but the octaves.
pub const TOWER_BELL: [[f32; 3]; 11] = [
    [0.5, 0.55, 9.0],
    [1.0, 0.50, 6.0],
    [1.19, 0.45, 5.0],
    [1.505, 0.22, 3.2],
    [2.0, 0.62, 3.8],
    [2.51, 0.20, 2.2],
    [2.66, 0.14, 1.9],
    [3.01, 0.17, 1.6],
    [4.16, 0.11, 1.1],
    [5.43, 0.07, 0.7],
    [6.80, 0.05, 0.45],
];

/// A patch's variants, ready to play.
#[derive(Clone, Debug)]
pub struct Rendered {
    pub name: String,
    pub variants: Vec<Vec<f32>>,
    pub send: f32,
}

/// A patch's variants as the engine keeps them: 16-bit PCM, half the memory of `f32` and 96 dB
/// under full scale, past hearing (PLAY-PLAN.md §7). Full scale is 1.0.
#[derive(Clone, Debug)]
pub struct Stored {
    pub name: String,
    pub variants: Vec<Vec<i16>>,
    pub send: f32,
}

/// One full-scale step of a [`Stored`] sample.
pub const PCM16: f32 = 1.0 / 32767.0;

impl Stored {
    pub fn of(r: Rendered) -> Stored {
        let q = |x: f32| (x * 32767.0).round().clamp(-32767.0, 32767.0) as i16;
        Stored {
            name: r.name,
            variants: r.variants.iter().map(|v| v.iter().map(|&x| q(x)).collect()).collect(),
            send: r.send,
        }
    }

    /// The bytes its samples hold (`jane bench --mem`).
    pub fn bytes(&self) -> usize {
        self.variants.iter().map(|v| v.len() * 2).sum()
    }
}

/// Renders every variant of `p` at `sr`; the same seed gives the same samples.
pub fn render(p: &SfxPatch, sr: f32, seed: u32) -> Rendered {
    let n = usize::from(p.variants.max(1));
    let variants = (0..n)
        .map(|v| {
            let mut rng = Rng::new(seed ^ (v as u32).wrapping_mul(0x85eb_ca6b) ^ hash(&p.name));
            // Each variant a few cents off the first, so a repeat is never the same sound.
            let cents = if v == 0 { 0.0 } else { rng.bi() * 40.0 };
            let tune = 2f32.powf(cents / 1200.0);
            let len = p.layers.iter().map(layer_len_ms).fold(0.0, f32::max);
            let total = ((len + 20.0) * 0.001 * sr) as usize;
            let mut buf = vec![0.0f32; total];
            for l in &p.layers {
                layer(l, sr, tune, &mut rng, &mut buf);
            }
            finish(&mut buf, sr, p.peak);
            buf
        })
        .collect();
    Rendered { name: p.name.clone(), variants, send: p.send }
}

/// FNV-1a of a name: a patch's own noise, whatever its row's place.
pub fn hash(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

fn layer_len_ms(l: &Layer) -> f32 {
    let notes = l.notes.len().max(1) as f32;
    l.delay_ms + (notes - 1.0) * l.every_ms + l.attack_ms + l.hold_ms + l.decay_ms
}

/// Takes the DC out, fades the last few milliseconds to exact zero and scales to `peak`.
fn finish(buf: &mut [f32], sr: f32, peak: f32) {
    let mut dc = DcBlock::new(sr);
    for x in buf.iter_mut() {
        *x = dc.run(*x);
    }
    let fade = ((0.006 * sr) as usize).min(buf.len());
    let n = buf.len();
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        buf[n - 1 - i] *= g * g;
    }
    let m = crate::analysis::peak(buf);
    if m > 1e-6 {
        let k = peak / m;
        for x in buf.iter_mut() {
            *x *= k;
        }
    }
}

/// One layer, added into `buf`.
fn layer(l: &Layer, sr: f32, tune: f32, rng: &mut Rng, buf: &mut [f32]) {
    let notes: Vec<f32> = if l.notes.is_empty() { vec![0.0] } else { l.notes.clone() };
    for (j, semi) in notes.iter().enumerate() {
        let start = ((l.delay_ms + j as f32 * l.every_ms) * 0.001 * sr) as usize;
        let k = tune * 2f32.powf(semi / 12.0);
        let at = start.min(buf.len());
        note(l, sr, k, rng, &mut buf[at..]);
    }
}

fn note(l: &Layer, sr: f32, k: f32, rng: &mut Rng, out: &mut [f32]) {
    let attack = (l.attack_ms.max(crate::dsp::MIN_EDGE_MS) * 0.001 * sr).max(1.0);
    let hold = l.hold_ms * 0.001 * sr;
    let decay_n = l.decay_ms * 0.001 * sr;
    let total = (attack + hold + decay_n) as usize;
    let fall = crate::dsp::fall_coef(l.decay_ms, sr);
    let sweep_n = l.pitch_ms.map_or(total as f32, |ms| ms * 0.001 * sr).max(1.0);
    let (p0, p1) = (l.pitch[0] * k, l.pitch[1] * k);
    let ratio = p1 / p0;
    let mut phase = rng.f();
    let mut mphase = 0.0f32;
    let mut pink = Pink::default();
    let mut filt = l.filter.map(|f| (Svf::new(f.cutoff[0], f.q, sr), f));
    let mut pluck =
        (l.wave == Wave::Pluck).then(|| Pluck::new(p0, sr, l.duty.clamp(0.0, 1.0), l.decay_ms * 0.001, 0.15, rng));
    let mut bell = (l.wave == Wave::Bell).then(|| Modal::new(p0, &TOWER_BELL, 0.8, sr, l.decay_ms / 6000.0));
    let mut decay = 1.0f32;
    let vib = l.vibrato;
    let trem = l.tremolo;
    for (i, o) in out.iter_mut().enumerate().take(total) {
        let t = i as f32;
        let env = if t < attack {
            let x = t / attack;
            x * x * (3.0 - 2.0 * x)
        } else if t < attack + hold {
            1.0
        } else {
            decay *= fall;
            decay
        };
        let sweep = (t / sweep_n).min(1.0);
        let mut hz = p0 * ratio.powf(sweep);
        if vib[1] > 0.0 {
            hz *= 2f32.powf(vib[1] * sin_cycles(vib[0] * t / sr) / 12.0);
        }
        let dt = hz / sr;
        let s = match l.wave {
            Wave::Sine => sin_cycles(phase),
            Wave::Tri => {
                let x = 4.0 * (phase - (phase + 0.5).floor()).abs() - 1.0;
                -x
            }
            Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
            Wave::Square => {
                let duty = if l.duty > 0.0 { l.duty.clamp(0.05, 0.95) } else { 0.5 };
                let mut v = if phase < duty { 1.0 } else { -1.0 };
                v += poly_blep(phase, dt);
                let q = (phase - duty).rem_euclid(1.0);
                v -= poly_blep(q, dt);
                v
            }
            Wave::Noise => rng.bi(),
            Wave::Pink => pink.run(rng.bi()) * 3.0,
            Wave::Fm => {
                mphase += dt * l.ratio;
                let index = l.duty * (0.25 + 0.75 * env);
                sin_cycles(phase + index * sin_cycles(mphase) / std::f32::consts::TAU)
            }
            Wave::Pluck => pluck.as_mut().map_or(0.0, Pluck::tick),
            Wave::Bell => bell.as_mut().map_or(0.0, Modal::tick),
        };
        phase += dt;
        if phase >= 1.0 {
            phase -= phase.floor();
        }
        let mut s = s;
        if let Some((f, row)) = &mut filt {
            if i % 16 == 0 {
                let c = row.cutoff[0] * (row.cutoff[1] / row.cutoff[0]).powf(sweep);
                f.set(c, row.q, sr);
            }
            s = f.run(s, row.kind);
        }
        if trem[1] > 0.0 {
            s *= 1.0 - trem[1] * (0.5 + 0.5 * sin_cycles(trem[0] * t / sr));
        }
        // A bell and a string carry their own decay; the envelope only shapes their ends, the
        // last quarter a gentle fade under what is left of the ring.
        let shaped = match l.wave {
            Wave::Bell | Wave::Pluck => {
                if t < attack {
                    env
                } else {
                    ((total as f32 - t) / (0.25 * total as f32)).min(1.0)
                }
            }
            _ => env,
        };
        *o += s * shaped * l.gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SfxFilter;

    fn layer_of(wave: Wave) -> Layer {
        Layer {
            wave,
            pitch: [440.0, 440.0],
            pitch_ms: None,
            attack_ms: 2.0,
            hold_ms: 0.0,
            decay_ms: 400.0,
            duty: 0.5,
            ratio: 1.0,
            vibrato: [0.0, 0.0],
            tremolo: [0.0, 0.0],
            filter: None,
            delay_ms: 0.0,
            gain: 1.0,
            notes: Vec::new(),
            every_ms: 0.0,
        }
    }

    #[test]
    fn a_patch_starts_and_ends_at_zero_at_its_peak() {
        for wave in [Wave::Sine, Wave::Tri, Wave::Saw, Wave::Square, Wave::Noise, Wave::Fm, Wave::Pluck, Wave::Bell] {
            let mut l = layer_of(wave);
            l.filter = Some(SfxFilter { kind: crate::model::FilterKind::Lp, cutoff: [4000.0, 800.0], q: 0.9 });
            let p = SfxPatch { name: "t".into(), layers: vec![l], peak: 0.5, variants: 2, send: 0.1 };
            let r = render(&p, 48_000.0, 1);
            for v in &r.variants {
                assert!((crate::analysis::peak(v) - 0.5).abs() < 1e-3, "{wave:?}");
                assert!(v[0].abs() < 0.02, "{wave:?} starts at {}", v[0]);
                assert!(v[v.len() - 1].abs() < 1e-4, "{wave:?}");
                assert!(crate::analysis::dc(v).abs() < 0.01, "{wave:?}");
            }
            assert_ne!(r.variants[0], r.variants[1], "{wave:?}: variants differ");
            assert_eq!(render(&p, 48_000.0, 1).variants, r.variants, "{wave:?}: deterministic");
        }
    }

    #[test]
    fn the_tower_bell_is_inharmonic_and_minor() {
        let mut l = layer_of(Wave::Bell);
        l.pitch = [146.83, 146.83];
        l.decay_ms = 6000.0;
        let p = SfxPatch { name: "bell".into(), layers: vec![l], peak: 0.5, variants: 1, send: 0.1 };
        let r = render(&p, 48_000.0, 1);
        let spec = crate::analysis::spectrum(&r.variants[0][2400..], 32_768);
        let peaks = crate::analysis::peaks(&spec, 48_000.0, 8, 40.0);
        let has = |ratio: f32| peaks.iter().any(|(hz, _)| (hz / (146.83 * ratio) - 1.0).abs() < 0.01);
        // Hum, prime, the minor third, the nominal.
        for ratio in [0.5, 1.0, 1.19, 2.0] {
            assert!(has(ratio), "{ratio}: {peaks:?}");
        }
        // The tierce is no harmonic of the hum: 2.38 hums.
        assert!((1.19f32 / 0.5).fract() > 0.3);
    }
}
