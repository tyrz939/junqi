//! The consoles' module (PORT.md §13.4) against the PC's synth: the same notes from the same
//! seed, each cue about as loud, in its key, every effect and bed there.

use std::sync::OnceLock;

use jane_audio::analysis;
use jane_audio::tracker::{self, Bank, Mixer};
use jane_audio::{Cmd, Engine, library};

/// The rate the console mixes at.
const RATE: u32 = 22_050;

fn baked() -> &'static (Vec<u8>, jane_audio::bake::Stats) {
    static B: OnceLock<(Vec<u8>, jane_audio::bake::Stats)> = OnceLock::new();
    B.get_or_init(|| jane_audio::bake::module(library()))
}

fn mixer(seed: u32) -> Mixer {
    Mixer::new(Bank::parse(baked().0.clone()).expect("the bake reads back"), RATE, seed)
}

fn render(m: &mut Mixer, secs: f32) -> Vec<f32> {
    let n = ((secs * RATE as f32) as usize).div_ceil(tracker::BLOCK) * tracker::BLOCK;
    let mut out = vec![0i16; 2 * n];
    m.render(&mut out);
    out.iter().map(|&s| f32::from(s) / 32_767.0).collect()
}

#[test]
fn the_module_is_small_and_reads_back() {
    let (bytes, stats) = baked();
    for l in &stats.lines {
        println!("  {l}");
    }
    println!(
        "module {} B: header {}, music {}, sfx {}, beds {}, of it far {}; {} samples, {} zones; worst snr {:.1} dB ({})",
        stats.bytes,
        stats.header,
        stats.music,
        stats.sfx,
        stats.beds,
        stats.far,
        stats.samples,
        stats.zones,
        stats.worst_snr.0,
        stats.worst_snr.1
    );
    // PORT.md §13.2: tracker patterns plus one shared bank under 1.5 MB, what the console holds
    // (the resident part, the far slot, the room's lines).
    let resident = bytes[..Bank::ram_len(bytes).expect("a module")].to_vec();
    let m = Mixer::new(Bank::parse(resident).expect("the resident part reads alone"), RATE, 1);
    println!("in RAM: {} B", m.ram());
    assert!(m.ram() < 1_500_000, "the console holds {} bytes", m.ram());
    let again = jane_audio::bake::module(library()).0;
    assert!(again == *bytes, "the bake is deterministic");
    let bank = Bank::parse(bytes.clone()).expect("reads back");
    assert_eq!(bank.head.songs.len(), library().songs.len());
    assert_eq!(bank.head.sfx.len(), library().sfx.len());
}

#[test]
fn the_tracker_plays_the_pcs_notes_from_the_same_seed() {
    let lib = library();
    for seed in [1u32, 7] {
        let mut pc = Engine::new(lib, 48_000.0, seed);
        pc.log_notes = true;
        let mut tr = mixer(seed);
        tr.log_notes = true;
        for (i, song) in lib.songs.iter().enumerate() {
            pc.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
            tr.handle(tracker::Cmd::Music { song: Some(i as u16), fade_out_ms: 0, fade_in_ms: 0 });
            // About 200 steps (a few loops of the short songs).
            let secs = (200 * song.step_ticks) as f32 / 60.0;
            let _ = pc.render_secs(secs);
            let _ = render(&mut tr, secs);
            // What both heard well inside the window (a step on its edge may fall either side).
            let edge = f64::from(secs) - 0.3;
            let a: Vec<_> = pc.note_log().expect("pc log").iter().filter(|n| (n.at as f64) < edge * 48_000.0).copied().collect();
            let b: Vec<_> =
                tr.note_log().expect("tracker log").iter().filter(|n| (n.at as f64) < edge * f64::from(RATE)).copied().collect();
            let n = a.len().min(b.len());
            assert!(n > 20, "{}: {} and {} notes", song.name, a.len(), b.len());
            assert!(a.len().abs_diff(b.len()) <= 1, "{}: {} notes on the PC, {} on the tracker", song.name, a.len(), b.len());
            // Humanising draws the same number at another rate: a few ms either way.
            let slack = song.humanize_ms * 3.0 + 2.0;
            for (k, (x, y)) in a.iter().zip(&b).take(n).enumerate() {
                assert_eq!((x.track, x.midi, x.chromatic), (y.track, y.midi, y.chromatic), "{} seed {seed}: note {k}", song.name);
                let ms = (x.at as f32 / 48.0 - y.at as f32 * 1000.0 / RATE as f32).abs();
                assert!(ms <= slack, "{} seed {seed}: note {k} {ms} ms off", song.name);
            }
        }
    }
}

#[test]
fn each_cue_is_about_as_loud_as_the_pcs_and_in_its_key() {
    let lib = library();
    let mut worst = 0.0f32;
    let mut bad = Vec::new();
    for (i, song) in lib.songs.iter().enumerate() {
        let mut pc = Engine::new(lib, 48_000.0, 1);
        pc.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
        let sa = pc.render_secs(24.0);
        let a = analysis::mono(&sa);
        let mut tr = mixer(1);
        tr.handle(tracker::Cmd::Music { song: Some(i as u16), fade_out_ms: 0, fade_in_ms: 0 });
        let sb = render(&mut tr, 24.0);
        let b = analysis::mono(&sb);
        // As heard: each side's loudness (the PC's wide voices are mono on the console, at the
        // same power a side).
        let sides = |s: &[f32], sr: f32| {
            let l: Vec<f32> = s.iter().step_by(2).copied().collect();
            let r: Vec<f32> = s.iter().skip(1).step_by(2).copied().collect();
            0.5 * (analysis::loudness(&l, sr) + analysis::loudness(&r, sr))
        };
        let (la, lb) = (sides(&sa, 48_000.0), sides(&sb, RATE as f32));
        // Each side's pitches, as heard (a wide PC voice is partly out of its mono sum).
        let chroma = |s: &[f32], sr: f32| {
            let l: Vec<f32> = s.iter().step_by(2).copied().collect();
            let r: Vec<f32> = s.iter().skip(1).step_by(2).copied().collect();
            let (x, y) = (analysis::chroma(&l, sr), analysis::chroma(&r, sr));
            std::array::from_fn::<f32, 12, _>(|k| x[k] + y[k])
        };
        let (ca, cb) = (chroma(&sa, 48_000.0), chroma(&sb, RATE as f32));
        let _ = (&a, &b);
        let (ka, kb) = (analysis::keys(&ca)[0], analysis::keys(&cb)[0]);
        // How alike the twelve pitch classes are heard (cosine of the chroma).
        let dot: f32 = ca.iter().zip(&cb).map(|(x, y)| x * y).sum();
        let like = dot / (ca.iter().map(|x| x * x).sum::<f32>().sqrt() * cb.iter().map(|x| x * x).sum::<f32>().sqrt()).max(1e-12);
        println!(
            "  {:16} pc {la:6.1} dB  psp {lb:6.1} dB  ({:+.1})  key pc {}{} psp {}{}  chroma {like:.3}  voices {}",
            song.name,
            lb - la,
            ka.0,
            if ka.1 { "m" } else { "" },
            kb.0,
            if kb.1 { "m" } else { "" },
            tr.peak_voices
        );
        worst = worst.max((lb - la).abs());
        if analysis::peak(&b) > 0.94 {
            bad.push(format!("{}: over the ceiling", song.name));
        }
        // The same pitches heard: the chroma alike. (The likeliest key is printed, not held: a
        // close call between a minor key and its dominant falls either way on a short window.)
        if like < 0.97 {
            bad.push(format!("{}: heard in {} {} (chroma {like:.3})", song.name, kb.0, kb.1));
        }
        if (lb - la).abs() > 1.0 {
            bad.push(format!("{}: {:+.1} dB off the PC's", song.name, lb - la));
        }
    }
    println!("worst {worst:.1} dB");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Each side's loudness, for a stereo render.
fn sides(s: &[f32], sr: f32) -> f32 {
    let l: Vec<f32> = s.iter().step_by(2).copied().collect();
    let r: Vec<f32> = s.iter().skip(1).step_by(2).copied().collect();
    0.5 * (analysis::loudness(&l, sr) + analysis::loudness(&r, sr))
}

#[test]
fn every_effect_and_bed_is_about_as_loud_as_the_pcs() {
    let lib = library();
    let mut bad = Vec::new();
    let mut pc = Engine::new(lib, 48_000.0, 1);
    for (id, p) in lib.sfx.iter().enumerate() {
        let secs = jane_audio::patch::secs(p) + 0.5;
        pc.handle(Cmd::Sfx { id, gain: 0.8, pan: 0.3, send: 0.1, rate: 1.0 });
        let a = pc.render_secs(secs + 2.5);
        let mut m = mixer(1);
        m.handle(tracker::Cmd::Sfx { id: id as u16, gain: 3277, pan: 1229, send: 410, rate: 65_536 });
        let b = render(&mut m, secs + 2.5);
        let (la, lb) = (sides(&a, 48_000.0), sides(&b, RATE as f32));
        // 2.5 dB: a bright effect (frost's glassy tings) loses what lies over 11 kHz.
        if (la - lb).abs() > 2.5 {
            bad.push(format!("sfx {}: pc {la:.1} psp {lb:.1}", p.name));
        }
    }
    for (k, bed) in jane_audio::Bed::ALL.iter().enumerate() {
        let mut pc = Engine::new(lib, 48_000.0, 1);
        pc.handle(Cmd::Bed { bed: *bed, level: 200.0 / 255.0 });
        let a = pc.render_secs(14.0);
        let mut m = mixer(1);
        m.handle(tracker::Cmd::Bed { bed: k as u8, level: 200 });
        let b = render(&mut m, 14.0);
        // After the swell.
        let (la, lb) = (sides(&a[2 * 48_000 * 4..], 48_000.0), sides(&b[2 * RATE as usize * 4..], RATE as f32));
        println!("  bed {:10} pc {la:6.1} psp {lb:6.1} ({:+.1})", bed.name(), lb - la);
        if (la - lb).abs() > 1.5 {
            bad.push(format!("bed {}: pc {la:.1} psp {lb:.1}", bed.name()));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
#[test]
#[ignore = "a diagnosis: each instrument's note against the PC's"]
fn each_instrument_note_against_the_pcs() {
    let lib = library();
    let bank = Bank::parse(baked().0.clone()).unwrap();
    for (k, inst) in bank.head.insts.iter().enumerate() {
        println!("  {} curve {:?}", inst.name, inst.vel);
        for (midi, vel) in inst.zones.iter().map(|z| (i32::from(z.root), 0.5f32)) {
            let prep = jane_audio::voice::Prepared::new(lib.instruments.iter().find(|i| i.name == inst.name).unwrap().clone());
            let hz = jane_audio::dsp::midi_hz(midi as f32);
            let long = ["choir", "glass", "drone", "clarinet"].contains(&inst.name.as_str());
            let (gate_s, secs) = if long { (6.0f32, 9.0f32) } else { (1.0, 3.5) };
            let n = (48_000.0 * secs) as usize;
            let mut pc = Vec::new();
            for seed in 0..8u32 {
                let mut v = jane_audio::voice::Voice::new(&prep, 0, 0, 0, hz, vel, 0.0, 0, Some((48_000.0 * gate_s) as u32), 48_000.0, 9 + seed * 7919);
                let (mut l, mut r, mut sl, mut sr) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
                v.render(&prep, 48_000.0, [&mut l, &mut r], [&mut sl, &mut sr]);
                pc.extend(l.iter().chain(&r).copied());
            }
            let mut m = mixer(1);
            m.audition(k, midi, (vel * 4096.0) as u32, (RATE as f32 * gate_s) as u32);
            let st = render(&mut m, secs);
            let tr = analysis::mono(&st);
            let b = analysis::rms(&st);
            let a = analysis::rms(&pc);
            if inst.name == "choir" {
                let w = |x: &[f32], sr: usize| x.chunks(sr / 4).map(|c| format!("{:.0}", analysis::to_db(analysis::rms(c)))).collect::<Vec<_>>().join(" ");
                println!("    pc  {}", w(&pc[..n], 48_000));
                let _ = &tr;
                println!("    psp {}", w(&tr, RATE as usize));
                let tail = &tr[tr.len() - 2000..];
                println!("    voices {} dc {} first {:?}", m.voices(), analysis::dc(tail), &tail[..6]);
            }
            println!("  {:18} midi {midi:3} vel {vel}: pc {:6.1} dB  psp {:6.1} dB  ({:+.1})", inst.name, analysis::to_db(a), analysis::to_db(b), analysis::to_db(b / a));
        }
    }
}
#[test]
fn a_quiet_mixer_is_silent() {
    let mut m = mixer(1);
    let v = render(&mut m, 2.0);
    assert!(analysis::peak(&v) == 0.0, "peak {}", analysis::peak(&v));
}
#[test]
#[ignore = "a diagnosis: one song's level over time against the PC's"]
fn one_song_over_time() {
    let lib = library();
    let name = std::env::var("SONG").unwrap_or_else(|_| "waters_night".into());
    let i = lib.songs.iter().position(|s| s.name == name).unwrap();
    let mut pc = Engine::new(lib, 48_000.0, 1);
    pc.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
    let a = pc.render_secs(24.0);
    let mut tr = mixer(1);
    tr.handle(tracker::Cmd::Music { song: Some(i as u16), fade_out_ms: 0, fade_in_ms: 0 });
    let b = render(&mut tr, 24.0);
    let w = |x: &[f32], sr: usize| x.chunks(sr * 2 * 2).map(|c| format!("{:.0}", analysis::to_db(analysis::rms(c)))).collect::<Vec<_>>().join(" ");
    println!("pc  {}", w(&a, 48_000));
    println!("psp {}", w(&b, RATE as usize));
    let (ca, cb) = (analysis::chroma(&analysis::mono(&a), 48_000.0), analysis::chroma(&analysis::mono(&b), RATE as f32));
    println!("chroma pc  {:?}", ca.map(|x| (x * 100.0).round()));
    println!("chroma psp {:?}", cb.map(|x| (x * 100.0).round()));
    println!("keys pc  {:?}", &analysis::keys(&ca)[..3]);
    println!("keys psp {:?}", &analysis::keys(&cb)[..3]);
}

#[test]
#[ignore = "a diagnosis: one song's tracks alone against the PC's"]
fn one_song_track_by_track() {
    let name = std::env::var("SONG").unwrap_or_else(|_| "waters_night".into());
    let i = library().songs.iter().position(|s| s.name == name).unwrap();
    for keep in 0..library().songs[i].tracks.len() {
        let mut lib = library().clone();
        for (k, tr) in lib.songs[i].tracks.iter_mut().enumerate() {
            if k != keep {
                tr.vel = 0.0;
            }
        }
        let mut pc = Engine::new(&lib, 48_000.0, 1);
        pc.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
        let a = pc.render_secs(24.0);
        let mut m = Mixer::new(Bank::parse(jane_audio::bake::module(&lib).0).unwrap(), RATE, 1);
        m.handle(tracker::Cmd::Music { song: Some(i as u16), fade_out_ms: 0, fade_in_ms: 0 });
        let b = render(&mut m, 24.0);
        let (x, y) = (analysis::to_db(analysis::rms(&a)), analysis::to_db(analysis::rms(&b)));
        println!("  {:10} pc {x:6.1} psp {y:6.1} ({:+.1})", lib.songs[i].tracks[keep].name, y - x);
        let norm = |c: [f32; 12]| {
            let s: f32 = c.iter().sum::<f32>().max(1e-9);
            c.map(|x| (x / s * 100.0).round())
        };
        println!("    pc  {:?}", norm(analysis::chroma(&analysis::mono(&a), 48_000.0)));
        println!("    psp {:?}", norm(analysis::chroma(&analysis::mono(&b), RATE as f32)));
    }
}
#[test]
#[ignore = "a bench: the mixer's cost on the host (the PSP's is measured on PPSSPP)"]
fn bench_the_mixer() {
    let lib = library();
    let combat = lib.songs.iter().position(|s| s.name == "combat").unwrap() as u16;
    let mut m = mixer(1);
    m.handle(tracker::Cmd::Music { song: Some(combat), fade_out_ms: 0, fade_in_ms: 0 });
    for b in [0u8, 2, 3] {
        m.handle(tracker::Cmd::Bed { bed: b, level: 200 });
    }
    let mut out = vec![0i16; 2 * 512];
    let t = std::time::Instant::now();
    let blocks = 60 * RATE as usize / 512;
    for k in 0..blocks {
        if k % 10 == 0 {
            m.handle(tracker::Cmd::Sfx { id: (k % 40) as u16, gain: 3000, pan: 0, send: 400, rate: 65_536 });
        }
        m.render(&mut out);
    }
    let us = t.elapsed().as_micros() as f64 / blocks as f64;
    println!("mixer: {us:.1} us a 512-frame block on the host, peak voices {}, {:.1} voice-frames a frame", m.peak_voices, m.mixed as f64 / (blocks * 512) as f64);
    let mut idle = mixer(1);
    let t = std::time::Instant::now();
    for _ in 0..blocks {
        idle.render(&mut out);
    }
    println!("idle: {:.1} us a block", t.elapsed().as_micros() as f64 / blocks as f64);
}