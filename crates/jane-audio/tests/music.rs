//! The score, judged by numbers (PRESENTATION.md §5): every cue as loud as every other, clean at
//! its edges, in the key it was written in, and the same county always hearing the same music.
//! Nobody on the build can hear it; these are what stand in for ears.

#![allow(clippy::cast_precision_loss)]

use jane_audio::analysis::{self, to_db};
use jane_audio::model::Mode;
use jane_audio::{Cmd, Engine, LOUDNESS, RATE};

/// How far a cue may sit from the loudness every cue is matched to.
const TOLERANCE_DB: f32 = 2.0;

struct Heard {
    name: String,
    mono: Vec<f32>,
    stereo: Vec<f32>,
    notes: Vec<jane_audio::seq::NoteOn>,
}

/// Every song, a pass of its first form (at most 40 s), seed 1, in parallel.
fn hear_all() -> Vec<Heard> {
    let lib = jane_audio::library();
    std::thread::scope(|s| {
        let handles: Vec<_> = lib
            .songs
            .iter()
            .map(|song| {
                s.spawn(move || {
                    let mut e = Engine::new(lib, RATE as f32, 1);
                    e.log_notes = true;
                    let i = e.song_index(&song.name).unwrap();
                    let d = &e.songs()[i];
                    let secs = (d.form_steps(0) as f32 * d.step_ticks as f32 / 60.0 + 3.0).min(40.0);
                    e.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
                    let stereo = e.render_secs(secs);
                    Heard {
                        name: song.name.clone(),
                        mono: analysis::mono(&stereo),
                        notes: e.note_log().unwrap_or_default().to_vec(),
                        stereo,
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// The keys a tune in `mode` on `tonic` may be heard in: its own tonic, its parent major, or that
/// major's relative minor (a Lydian tune is heard in the major it borrows its notes from).
fn key_fits(tonic: i32, mode: Mode, t: i32, minor: bool) -> bool {
    let down = match mode {
        Mode::Major => 0,
        Mode::Dorian => 2,
        Mode::Phrygian => 4,
        Mode::Lydian => 5,
        Mode::Mixolydian => 7,
        Mode::Minor | Mode::Harmonic => 9,
        Mode::Locrian => 11,
    };
    let parent = (tonic - down).rem_euclid(12);
    t == tonic || (!minor && t == parent) || (minor && t == (parent + 9) % 12)
}

#[test]
fn every_cue_is_matched_clean_and_in_its_key() {
    let lib = jane_audio::library();
    let sr = RATE as f32;
    let mut bad = Vec::new();
    for h in hear_all() {
        let song = lib.songs.iter().find(|s| s.name == h.name).unwrap();
        // Loudness, matched across the score.
        let loud = analysis::loudness(&h.mono, sr);
        let want = LOUDNESS - song.under;
        if (loud - want).abs() > TOLERANCE_DB {
            bad.push(format!(
                "{}: {loud:.1} dBFS, wants {want:.1} (gain x{:.2})",
                h.name,
                10f32.powf((want - loud) / 20.0)
            ));
        }
        // Never at the ceiling, never clipped.
        let peak = analysis::peak(&h.stereo);
        if peak > jane_audio::CEILING {
            bad.push(format!("{}: peaks at {:.1} dBFS", h.name, to_db(peak)));
        }
        // No DC.
        let dc = analysis::dc(&h.mono);
        if dc.abs() > 1e-3 {
            bad.push(format!("{}: DC {dc}", h.name));
        }
        // No clicks, even turned up 6 dB so a quiet one would show.
        let loud2: Vec<f32> = h.mono.iter().map(|x| x * 2.0).collect();
        let clicks = analysis::clicks(&loud2, sr);
        if !clicks.is_empty() {
            bad.push(format!(
                "{}: clicks at {:?} s",
                h.name,
                clicks.iter().take(4).map(|i| *i as f32 / sr).collect::<Vec<_>>()
            ));
        }
        // Heard in the key it was written in (among the three likeliest).
        let tonic = jane_audio::model::pitch_class(&song.key).unwrap();
        let keys = analysis::keys(&analysis::chroma(&h.mono, sr));
        if !keys.iter().take(3).any(|&(t, m, _)| key_fits(tonic, song.mode, t, m)) {
            bad.push(format!("{}: written in {} {}, heard in {:?}", h.name, song.key, song.mode.name(), &keys[..3]));
        }
        // Every note the score plays is in its scale, unless the pattern asked for it outside.
        let steps = song.mode.steps();
        for n in &h.notes {
            let pc = (n.midi - tonic).rem_euclid(12);
            if !n.chromatic && !steps.contains(&pc) {
                bad.push(format!(
                    "{}: note {} is outside {} {} and was not asked for",
                    h.name,
                    n.midi,
                    song.key,
                    song.mode.name()
                ));
                break;
            }
        }
        if h.notes.is_empty() {
            bad.push(format!("{}: played nothing", h.name));
        }
        // Not drowned in its own low end: under 70 per cent of the power below 150 Hz (a mix
        // that is all drum and drone has nothing left for the tune once it is matched).
        let (bands, _) = analysis::balance(&h.mono, sr);
        if bands[0] > 0.70 {
            bad.push(format!("{}: {:.0}% of its power under 150 Hz", h.name, 100.0 * bands[0]));
        }
    }
    assert!(bad.is_empty(), "\n  {}", bad.join("\n  "));
}

#[test]
fn a_county_always_hears_the_same_evening_and_another_county_another() {
    let lib = jane_audio::library();
    let play = |seed: u32| {
        let mut e = Engine::new(lib, RATE as f32, seed);
        let i = e.song_index("lowfields_night").unwrap();
        e.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
        e.render_secs(12.0)
    };
    let a = play(7);
    assert_eq!(a, play(7), "same seed, same samples");
    assert_ne!(a, play(8), "another county drifts another way");
}

#[test]
fn a_cue_change_crossfades_without_a_click() {
    let lib = jane_audio::library();
    let sr = RATE as f32;
    let mut e = Engine::new(lib, sr, 3);
    let day = e.song_index("lowfields_day").unwrap();
    let fight = e.song_index("combat").unwrap();
    e.handle(Cmd::Music { song: Some(day), fade_out_ms: 0.0, fade_in_ms: 0.0 });
    let mut v = e.render_secs(4.0);
    e.handle(Cmd::Music { song: Some(fight), fade_out_ms: 700.0, fade_in_ms: 300.0 });
    v.extend(e.render_secs(4.0));
    e.handle(Cmd::Music { song: None, fade_out_ms: 1500.0, fade_in_ms: 0.0 });
    v.extend(e.render_secs(8.0));
    let m = analysis::mono(&v);
    assert!(analysis::clicks(&m, sr).is_empty());
    // Silence at the end: the fade finished and the voices rang out.
    let tail = &m[m.len() - (sr as usize) / 2..];
    assert!(analysis::peak(tail) < 1e-3, "{}", analysis::peak(tail));
}

#[test]
fn every_pitched_instrument_is_in_tune() {
    let lib = jane_audio::library();
    let sr = RATE as f32;
    for inst in &lib.instruments {
        use jane_audio::model::VoiceKind;
        if matches!(inst.voice, VoiceKind::Bell | VoiceKind::Drum | VoiceKind::Noise) || inst.name == "tick" {
            continue;
        }
        // A3 held: long enough to find its period after the attack.
        let m = jane_audio::engine::audition(lib, &inst.name, &[(57, 1.5)], sr).unwrap();
        let from = (0.5 * sr) as usize;
        let got = analysis::pitch(&m[from..from + (0.5 * sr) as usize], sr);
        let got = got.unwrap_or_else(|| panic!("{}: no pitch found", inst.name));
        let cents = 1200.0 * (got / 220.0).log2();
        // A vibrato is allowed its own swing (the School's music box wows on purpose).
        let allowed = 10.0 + 100.0 * inst.vibrato[1];
        assert!(cents.abs() < allowed, "{}: {got:.1} Hz, {cents:.1} cents from A3", inst.name);
    }
}

#[test]
fn every_sound_effect_starts_and_ends_at_rest() {
    let lib = jane_audio::library();
    let sr = RATE as f32;
    for p in &lib.sfx {
        let r = jane_audio::patch::render(p, sr, 0x5eed);
        for (i, v) in r.variants.iter().enumerate() {
            let who = format!("{} ({i})", p.name);
            assert!(analysis::peak(v) <= p.peak + 1e-4, "{who}: over its peak");
            assert!(analysis::dc(v).abs() < 0.01, "{who}: DC");
            assert!(v[0].abs() < 0.02 && v[v.len() - 1].abs() < 1e-4, "{who}: ends");
            assert!(analysis::clicks(v, sr).is_empty(), "{who}: clicks at {:?}", analysis::clicks(v, sr));
        }
    }
}

/// A learned spell's cues (PRESENTATION.md §5.1): each school's, and its first-spell phrase, in
/// the school's own key (a minor key for the schools that hurt, a major one for mending, growing
/// and healing), clean at its edges and with no click, and about as loud as one another.
#[test]
fn every_learn_cue_is_clean_and_in_its_schools_key() {
    let lib = jane_audio::library();
    let sr = RATE as f32;
    // (school, tonic pitch class, minor)
    let keys = [
        ("frost", 2, true),
        ("fire", 7, true),
        ("blast", 9, true),
        ("shock", 0, true),
        ("nature", 4, false),
        ("physical", 5, false),
        ("heal", 10, false),
    ];
    let mut bad = Vec::new();
    let mut levels = Vec::new();
    for (school, tonic, minor) in keys {
        for name in [format!("learn_{school}"), format!("learn_first_{school}")] {
            let p = lib.sfx.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no patch {name}"));
            let v = &jane_audio::patch::render(p, sr, 0x5eed).variants[0];
            let loud2: Vec<f32> = v.iter().map(|x| x * 2.0).collect();
            if !analysis::clicks(&loud2, sr).is_empty() {
                bad.push(format!(
                    "{name}: clicks at {:?} s",
                    analysis::clicks(&loud2, sr).iter().map(|i| *i as f32 / sr).collect::<Vec<_>>()
                ));
            }
            let heard = analysis::keys(&analysis::chroma(v, sr));
            if heard[0].0 != tonic || heard[0].1 != minor {
                bad.push(format!(
                    "{name}: wants {tonic} {}, heard {:?}",
                    if minor { "minor" } else { "major" },
                    &heard[..3]
                ));
            }
            let l = analysis::loudness(v, sr);
            eprintln!("{name}: {l:.1} dBFS, peak {:.1}, {:.1} s", to_db(analysis::peak(v)), v.len() as f32 / sr);
            levels.push((name, l));
        }
    }
    let (lo, hi) = levels.iter().fold((f32::MAX, f32::MIN), |(a, b), (_, l)| (a.min(*l), b.max(*l)));
    if hi - lo > 6.0 {
        bad.push(format!("levels spread {:.1} dB: {levels:?}", hi - lo));
    }
    assert!(bad.is_empty(), "\n  {}", bad.join("\n  "));
}
