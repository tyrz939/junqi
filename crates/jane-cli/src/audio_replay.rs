//! `jane audio replay` (PORT.md §13.4): what a PSP played, played again on the host. A scripted
//! PSP run writes every command its mixer applied, with the frame it applied it at
//! (`psp-cmds.txt`), and what it put out (`psp-audio.wav`, 22.05 kHz). This plays the same
//! commands at the same frames through the host's tracker (the same integer code, so it must come
//! out bit for bit as the PSP's) and through the PC's synth at 48 kHz (what the module was baked
//! from), and writes each with its spectrogram.

#![allow(clippy::cast_precision_loss)]

use std::path::{Path, PathBuf};

use jane_audio::analysis;
use jane_audio::tracker::{self, Bank, Mixer};
use jane_audio::{Bed, Cmd, Engine};

pub const USAGE: &str =
    "  audio replay <psp-cmds.txt> [--jau FILE] [--wav psp-audio.wav] [--secs S] [--seed N] [--out DIR]
                                      a PSP run's commands again, through the host's tracker and the PC's
                                      synth: tracker.wav, pc.wav and their spectrograms in DIR (default
                                      beside the commands); with --wav, the PSP's own capture compared";

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// The PSP's mixer rate (`jane_audio_psp::psp::RATE`).
const PSP_RATE: u32 = 22_050;

pub fn run(args: &[String]) -> Result<(), String> {
    let cmds_path = PathBuf::from(args.first().ok_or("replay what? a psp-cmds.txt")?);
    let seed = arg(args, "--seed").map_or(Ok(1), str::parse::<u32>).map_err(|e| format!("--seed: {e}"))?;
    let jau = arg(args, "--jau").map_or_else(|| PathBuf::from("target/bake/jane-psp.jau"), PathBuf::from);
    let out =
        arg(args, "--out").map_or_else(|| cmds_path.parent().unwrap_or(Path::new(".")).to_path_buf(), PathBuf::from);
    let text = std::fs::read_to_string(&cmds_path).map_err(|e| format!("{}: {e}", cmds_path.display()))?;
    let mut cmds = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let [f, a, b] = w[..] else { continue };
        let parse = |s: &str| u64::from_str_radix(s, 16).map_err(|e| format!("line {}: {e}", n + 1));
        let frame: u64 = f.parse().map_err(|e| format!("line {}: {e}", n + 1))?;
        if let Some(c) = tracker::unpack(parse(a)?, parse(b)?) {
            cmds.push((frame, c));
        }
    }
    let psp = arg(args, "--wav").map(|p| read_wav16(Path::new(p))).transpose()?;
    let secs = match arg(args, "--secs") {
        Some(s) => s.parse::<f64>().map_err(|e| format!("--secs: {e}"))?,
        None => psp.as_ref().map_or_else(
            || cmds.last().map_or(10.0, |c| c.0 as f64 / f64::from(PSP_RATE) + 5.0),
            |v| (v.len() / 2) as f64 / f64::from(PSP_RATE),
        ),
    };
    println!("  {} commands, {secs:.1} s", cmds.len());
    // The host's tracker: the PSP's own code.
    let bytes = std::fs::read(&jau).map_err(|e| format!("{}: {e}", jau.display()))?;
    let mut m = Mixer::new(Bank::parse(bytes).map_err(String::from)?, PSP_RATE, seed);
    let frames = (secs * f64::from(PSP_RATE)) as usize / 512 * 512;
    let mut tr = vec![0i16; 2 * frames];
    let mut next = 0;
    for block in tr.chunks_mut(2 * 512) {
        while next < cmds.len() && cmds[next].0 <= m.now {
            m.handle(cmds[next].1);
            next += 1;
        }
        m.render(block);
    }
    let trf: Vec<f32> = tr.iter().map(|&s| f32::from(s) / 32_767.0).collect();
    // The PC's synth at 48 kHz, each command at the same moment.
    let lib = jane_audio::library();
    let mut e = Engine::new(lib, jane_audio::RATE as f32, seed);
    let total = (secs * f64::from(jane_audio::RATE)) as usize;
    let mut pc = vec![0.0f32; 2 * total];
    let mut at = 0usize;
    for (frame, c) in &cmds {
        let to = ((*frame as f64 * f64::from(jane_audio::RATE) / f64::from(PSP_RATE)) as usize).min(total);
        if to > at {
            e.render(&mut pc[2 * at..2 * to]);
            at = to;
        }
        if let Some(c) = to_pc(*c) {
            e.handle(c);
        }
    }
    if at < total {
        e.render(&mut pc[2 * at..]);
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let save = |name: &str, v: &[f32], sr: u32| -> Result<(), String> {
        let p = out.join(format!("{name}.wav"));
        std::fs::write(&p, jane_audio::wav::wav(v, 2, sr)).map_err(|e| format!("{}: {e}", p.display()))?;
        let png = out.join(format!("{name}.png"));
        std::fs::write(&png, crate::audio_cmd::picture_at(v, name, sr).png())
            .map_err(|e| format!("{}: {e}", png.display()))?;
        println!("  {} {}", p.display(), png.display());
        Ok(())
    };
    save("tracker", &trf, PSP_RATE)?;
    save("pc", &pc, jane_audio::RATE)?;
    let line = |name: &str, v: &[f32], sr: u32| {
        let mono = analysis::mono(v);
        let c = analysis::chroma(&mono, sr as f32);
        let k = analysis::keys(&c)[0];
        println!(
            "  {name:8} loudness {:6.1} dB  peak {:6.1} dB  key {}{}",
            analysis::loudness(&mono, sr as f32),
            analysis::to_db(analysis::peak(v)),
            k.0,
            if k.1 { "m" } else { "" }
        );
        c
    };
    let cp = line("pc", &pc, jane_audio::RATE);
    let ct = line("tracker", &trf, PSP_RATE);
    println!("  chroma alike (pc, tracker): {:.3}", alike(&cp, &ct));
    if let Some(p) = psp {
        let n = p.len().min(tr.len());
        let differ = p[..n].iter().zip(&tr[..n]).filter(|(a, b)| a != b).count();
        println!("  psp capture: {} frames; against the host's tracker {differ} of {n} samples differ", p.len() / 2);
        let pf: Vec<f32> = p.iter().map(|&s| f32::from(s) / 32_767.0).collect();
        save("psp", &pf, PSP_RATE)?;
        let cs = line("psp", &pf, PSP_RATE);
        println!("  chroma alike (pc, psp): {:.3}", alike(&cp, &cs));
    }
    Ok(())
}

fn alike(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()).max(1e-12)
}

/// A tracker command as the PC engine's.
fn to_pc(c: tracker::Cmd) -> Option<Cmd> {
    let q12 = |x: i32| x as f32 / 4096.0;
    Some(match c {
        tracker::Cmd::Music { song, fade_out_ms, fade_in_ms } => Cmd::Music {
            song: song.map(usize::from),
            fade_out_ms: f32::from(fade_out_ms),
            fade_in_ms: f32::from(fade_in_ms),
        },
        tracker::Cmd::Sfx { id, gain, pan, send, rate } => Cmd::Sfx {
            id: usize::from(id),
            gain: q12(i32::from(gain)),
            pan: q12(i32::from(pan)),
            send: q12(i32::from(send)),
            rate: rate as f32 / 65_536.0,
        },
        tracker::Cmd::Bed { bed, level } => {
            Cmd::Bed { bed: *Bed::ALL.get(usize::from(bed))?, level: f32::from(level) / 255.0 }
        }
        tracker::Cmd::Volume { master, music, sfx } => {
            Cmd::Volume { master: q12(i32::from(master)), music: q12(i32::from(music)), sfx: q12(i32::from(sfx)) }
        }
        tracker::Cmd::Seed(s) => Cmd::Seed(s),
        tracker::Cmd::Duck(d) => Cmd::Duck(f32::from(d) / 255.0),
    })
}

/// A 16-bit stereo WAV's frames, to its end (a capture's header may say less than it holds).
fn read_wav16(p: &Path) -> Result<Vec<i16>, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if b.len() < 44 || &b[..4] != b"RIFF" {
        return Err(format!("{}: not a WAV", p.display()));
    }
    Ok(b[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
}
