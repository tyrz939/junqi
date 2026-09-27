//! A 16-bit PCM WAV writer: forty lines, no crate (PORT.md §3.5). For `jane audio render` and
//! `jane sheet audio`; the game writes no files of sound.

/// Interleaved samples (`channels` of them a frame) as a WAV file's bytes, with triangular
/// dither on the way to 16 bits.
pub fn wav(samples: &[f32], channels: u16, sr: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sr.to_le_bytes());
    out.extend_from_slice(&(sr * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    let mut rng = crate::dsp::Rng::new(7);
    for &s in samples {
        let d = (rng.f() - rng.f()) / 32768.0;
        let v = ((s + d).clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_wav_has_its_header_and_its_samples() {
        let w = super::wav(&[0.0, 0.5, -0.5, 1.0], 2, 48_000);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(w.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 48_000);
        let last = i16::from_le_bytes([w[50], w[51]]);
        assert!(last >= 32766);
    }
}
