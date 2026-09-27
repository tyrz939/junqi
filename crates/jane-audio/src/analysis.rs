//! Listening by numbers (PRESENTATION.md §5): nobody on the build can hear, so every sound is
//! judged by its levels, its spectrum, its pitch, its key and its edges. The tests and
//! `jane audio check` both read these.

use std::f32::consts::PI;

/// The largest sample, absolute.
pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>() / x.len() as f64).sqrt() as f32
}

/// The mean: what a speaker cone would be pushed by and never pushed back.
pub fn dc(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| f64::from(*v)).sum::<f64>() / x.len() as f64) as f32
}

/// Decibels of a linear level (floor -120).
pub fn to_db(x: f32) -> f32 {
    20.0 * x.max(1e-6).log10()
}

/// Gated loudness in dBFS: the power of 400 ms blocks, blocks under -60 dBFS left out, then
/// blocks more than 20 dB under that mean left out (quiet bars of a sparse cue do not pull it
/// down). A plain cousin of LUFS without its weighting curve.
pub fn loudness(mono: &[f32], sr: f32) -> f32 {
    let n = (0.4 * sr) as usize;
    let blocks: Vec<f32> = mono.chunks(n).filter(|c| c.len() == n).map(|c| rms(c).powi(2)).collect();
    let gate1: Vec<f32> = blocks.into_iter().filter(|p| to_db(p.sqrt()) > -60.0).collect();
    if gate1.is_empty() {
        return -120.0;
    }
    let mean = gate1.iter().sum::<f32>() / gate1.len() as f32;
    let rel = mean * 0.01;
    let gate2: Vec<f32> = gate1.into_iter().filter(|&p| p > rel).collect();
    to_db((gate2.iter().sum::<f32>() / gate2.len().max(1) as f32).sqrt())
}

/// Left and right of an interleaved stereo buffer, averaged.
pub fn mono(stereo: &[f32]) -> Vec<f32> {
    stereo.chunks(2).map(|c| 0.5 * (c[0] + c.get(1).copied().unwrap_or(c[0]))).collect()
}

/// Where the signal jumps: the second difference against the local level of the second
/// difference around it. A smooth, band-limited sound turns gently from sample to sample; a
/// click is a turn far sharper than its neighbourhood's. Returns the sample indices.
pub fn clicks(x: &[f32], sr: f32) -> Vec<usize> {
    if x.len() < 8 {
        return Vec::new();
    }
    let d2: Vec<f32> = (2..x.len()).map(|i| (x[i] - 2.0 * x[i - 1] + x[i - 2]).abs()).collect();
    let half = (0.005 * sr) as usize;
    // A running sum of squares for the neighbourhood's level.
    let mut pre = vec![0.0f64; d2.len() + 1];
    for (i, v) in d2.iter().enumerate() {
        pre[i + 1] = pre[i] + f64::from(*v) * f64::from(*v);
    }
    let mut out = Vec::new();
    let mut last = 0usize;
    for (i, &v) in d2.iter().enumerate() {
        if v < 0.01 {
            continue;
        }
        let a = i.saturating_sub(half);
        let b = (i + half + 1).min(d2.len());
        let sum = pre[b] - pre[a] - f64::from(v) * f64::from(v);
        let local = (sum / (b - a - 1).max(1) as f64).sqrt() as f32;
        if v > 10.0 * local.max(1e-4) && (out.is_empty() || i > last + half) {
            out.push(i + 2);
            last = i;
        }
    }
    out
}

/// An in-place radix-2 FFT; `re.len()` a power of two.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// The magnitude spectrum of `x` (the first power of two of it, Hann-windowed): `n/2` bins of
/// `sr / n` Hz.
pub fn spectrum(x: &[f32], n: usize) -> Vec<f32> {
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for (i, r) in re.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos();
        *r = x.get(i).copied().unwrap_or(0.0) * w;
    }
    fft(&mut re, &mut im);
    (0..n / 2).map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt()).collect()
}

/// The average magnitude spectrum over the whole signal, frames of `n` with half overlap.
pub fn mean_spectrum(x: &[f32], n: usize) -> Vec<f32> {
    let mut acc = vec![0.0f32; n / 2];
    let mut frames = 0;
    let mut at = 0;
    while at + n <= x.len() {
        for (a, s) in acc.iter_mut().zip(spectrum(&x[at..at + n], n)) {
            *a += s;
        }
        frames += 1;
        at += n / 2;
    }
    if frames == 0 {
        return spectrum(x, n);
    }
    for a in &mut acc {
        *a /= frames as f32;
    }
    acc
}

/// The `count` strongest local maxima of a magnitude spectrum, in Hz (parabolic interpolation),
/// strongest first; `min_hz` skips the rumble below.
pub fn peaks(spec: &[f32], sr: f32, count: usize, min_hz: f32) -> Vec<(f32, f32)> {
    let n = spec.len() * 2;
    let bin = sr / n as f32;
    let mut found: Vec<(f32, f32)> = Vec::new();
    for i in 2..spec.len() - 2 {
        let (a, b, c) = (spec[i - 1], spec[i], spec[i + 1]);
        if b > a && b >= c && b > spec[i - 2] && b >= spec[i + 2] && i as f32 * bin >= min_hz {
            let (la, lb, lc) = (a.max(1e-9).ln(), b.max(1e-9).ln(), c.max(1e-9).ln());
            let d = la - 2.0 * lb + lc;
            let off = if d.abs() > 1e-9 { 0.5 * (la - lc) / d } else { 0.0 };
            found.push(((i as f32 + off) * bin, b));
        }
    }
    found.sort_by(|x, y| y.1.total_cmp(&x.1));
    found.truncate(count);
    found
}

/// The fundamental of a steady tone by YIN (the cumulative mean normalised difference), with a
/// parabola through the dip; `None` if nothing periodic between 40 Hz and 2 kHz.
pub fn pitch(x: &[f32], sr: f32) -> Option<f32> {
    let w = (sr / 40.0) as usize;
    let max_tau = w;
    let min_tau = (sr / 2000.0) as usize;
    if x.len() < w * 2 {
        return None;
    }
    let mut d = vec![0.0f32; max_tau + 1];
    for (tau, dt) in d.iter_mut().enumerate().skip(1) {
        let mut s = 0.0f32;
        for i in 0..w {
            let e = x[i] - x[i + tau];
            s += e * e;
        }
        *dt = s;
    }
    let mut cm = vec![1.0f32; max_tau + 1];
    let mut run = 0.0;
    for tau in 1..=max_tau {
        run += d[tau];
        cm[tau] = d[tau] * tau as f32 / run.max(1e-12);
    }
    let mut tau = min_tau.max(2);
    while tau < max_tau {
        if cm[tau] < 0.15 {
            while tau + 1 < max_tau && cm[tau + 1] < cm[tau] {
                tau += 1;
            }
            let (a, b, c) = (cm[tau - 1], cm[tau], cm[tau + 1]);
            let den = a - 2.0 * b + c;
            let off = if den.abs() > 1e-12 { 0.5 * (a - c) / den } else { 0.0 };
            return Some(sr / (tau as f32 + off));
        }
        tau += 1;
    }
    None
}

/// Energy per pitch class (C = 0) over the signal, from 55 Hz to 2 kHz.
pub fn chroma(x: &[f32], sr: f32) -> [f32; 12] {
    let n = 8192;
    let spec = mean_spectrum(x, n);
    let bin = sr / n as f32;
    let mut c = [0.0f32; 12];
    for (i, m) in spec.iter().enumerate().skip(1) {
        let hz = i as f32 * bin;
        if !(55.0..2000.0).contains(&hz) {
            continue;
        }
        let midi = 69.0 + 12.0 * (hz / 440.0).log2();
        let pc = (midi.round() as i32).rem_euclid(12) as usize;
        // Weight a bin by how near it sits to the pitch's centre.
        let near = 1.0 - (midi - midi.round()).abs() * 2.0;
        c[pc] += m * m * near.max(0.0);
    }
    c
}

/// Krumhansl and Kessler's key profiles.
const MAJOR: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

fn correlate(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut s, mut sa, mut sb) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        s += (a[i] - ma) * (b[i] - mb);
        sa += (a[i] - ma).powi(2);
        sb += (b[i] - mb).powi(2);
    }
    s / (sa * sb).sqrt().max(1e-12)
}

/// The key a chroma sounds in: (tonic pitch class, minor, correlation), best first.
pub fn keys(c: &[f32; 12]) -> Vec<(i32, bool, f32)> {
    let mut out = Vec::with_capacity(24);
    for tonic in 0..12 {
        for (minor, prof) in [(false, &MAJOR), (true, &MINOR)] {
            let mut rot = [0.0f32; 12];
            for i in 0..12 {
                rot[(i + tonic) % 12] = prof[i];
            }
            out.push((tonic as i32, minor, correlate(c, &rot)));
        }
    }
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out
}

/// A spectrogram as rows of dB (top the highest), `w` frames by `h` log-spaced bands from 40 Hz
/// to 16 kHz.
pub fn spectrogram(x: &[f32], sr: f32, w: usize, h: usize) -> Vec<Vec<f32>> {
    let n = 2048;
    let bin = sr / n as f32;
    let hop = (x.len().saturating_sub(n) / w.max(1)).max(1);
    let (lo, hi) = (40f32.ln(), 16_000f32.min(sr * 0.5).ln());
    let mut rows = vec![vec![-120.0f32; w]; h];
    for col in 0..w {
        let at = col * hop;
        if at + n > x.len() {
            break;
        }
        let s = spectrum(&x[at..at + n], n);
        for (row, r) in rows.iter_mut().enumerate() {
            let f0 = (lo + (hi - lo) * (h - row - 1) as f32 / h as f32).exp();
            let f1 = (lo + (hi - lo) * (h - row) as f32 / h as f32).exp();
            let (b0, b1) = ((f0 / bin) as usize, ((f1 / bin) as usize).max((f0 / bin) as usize + 1));
            let m = s[b0.min(s.len() - 1)..b1.min(s.len())].iter().fold(0.0f32, |a, v| a.max(*v));
            r[col] = to_db(m / (n as f32 / 4.0));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| crate::dsp::sin_cycles(hz * i as f32 / sr) * 0.5).collect()
    }

    #[test]
    fn a_sine_measures_as_itself() {
        let sr = 48_000.0;
        let x = tone(440.0, sr, 48_000);
        assert!((peak(&x) - 0.5).abs() < 1e-3);
        assert!((rms(&x) - 0.5 / 2f32.sqrt()).abs() < 1e-3);
        assert!(dc(&x).abs() < 1e-3);
        assert!((pitch(&x, sr).unwrap() - 440.0).abs() < 0.5);
        let p = peaks(&spectrum(&x, 8192), sr, 1, 20.0);
        assert!((p[0].0 - 440.0).abs() < 2.0, "{p:?}");
        assert!(clicks(&x, sr).is_empty());
    }

    #[test]
    fn a_step_is_a_click() {
        let sr = 48_000.0;
        let mut x = tone(220.0, sr, 9600);
        for v in &mut x[4800..] {
            *v += 0.3;
        }
        let c = clicks(&x, sr);
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((4799..=4802).contains(&c[0]));
    }

    #[test]
    fn a_triad_is_heard_in_its_key() {
        let sr = 48_000.0;
        // C E G: C major.
        let mut x = vec![0.0f32; 48_000];
        for hz in [261.63f32, 329.63, 392.0, 130.81] {
            for (i, v) in x.iter_mut().enumerate() {
                *v += crate::dsp::sin_cycles(hz * i as f32 / sr) * 0.2;
            }
        }
        let k = keys(&chroma(&x, sr));
        assert_eq!((k[0].0, k[0].1), (0, false), "{:?}", &k[..3]);
    }
}
