//! `jane audio` and `jane sheet audio` (PRESENTATION.md §5, §6): every patch, bed, song and the
//! bell at nine rendered to WAV under `sheets/`, measured (peak, loudness, DC, clicks, the key it
//! sounds in against the key it was written in), and a few drawn as a waveform over a
//! spectrogram. Nobody on the build can hear; this is how the sound is looked at.

#![allow(clippy::cast_precision_loss)]

use std::path::{Path, PathBuf};

use jane_art::sheet::Image;
use jane_audio::analysis::{self, to_db};
use jane_audio::{Bed, Cmd, Engine, RATE};

pub const USAGE: &str = "  audio list                          every sound effect, bed and song, with each song's key and mood
  audio render <what> [--secs S] [--seed N] [--out FILE.wav] [--png]
                                      one thing to a WAV (sheets/audio/<what>.wav): sfx:<name>, bed:<name>,
                                      inst:<name> (a dry phrase), song:<name> (one pass of its form), or scene:nine (the bell at nine
                                      over its cue and into the night), scene:six, scene:combat; --png
                                      draws it too
  audio render play:<seed> [--hour H[:MM]] [--warm T] [--model reader|rusher] [--secs S]
                                      the game's own sound: a model plays the seed from New Game (T ticks
                                      unheard first), the clock is set (default 20:58), and the real cue
                                      table drives the synth for S seconds (default 40): footsteps, beds,
                                      the bell at nine, whatever the model runs into
  audio check [--secs S]              measure every song and patch: peak, gated loudness, DC, clicks, the
                                      key heard against the key written; exits 1 on a failure";

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let seed = arg(args, "--seed").map_or(Ok(1), str::parse::<u32>).map_err(|e| format!("--seed: {e}"))?;
    let secs = arg(args, "--secs").map(str::parse::<f32>).transpose().map_err(|e| format!("--secs: {e}"))?;
    match args.first().map(String::as_str) {
        Some("list") => {
            list();
            Ok(())
        }
        Some("render") => {
            let what = args.get(1).ok_or("render what? sfx:<name>, bed:<name>, song:<name>, scene:nine")?;
            let out = arg(args, "--out").map_or_else(|| PathBuf::from("sheets/audio").join(format!("{}.wav", what.replace(':', "-"))), PathBuf::from);
            let v = render(what, seed, secs, args)?;
            write_wav(&out, &v)?;
            if args.iter().any(|a| a == "--png") {
                let png = out.with_extension("png");
                std::fs::write(&png, picture(&v, what).png()).map_err(|e| format!("{}: {e}", png.display()))?;
                println!("{}", png.display());
            }
            report(what, &v);
            Ok(())
        }
        Some("check") => check(seed, secs),
        _ => Err(format!("audio: list, render or check\n{USAGE}")),
    }
}

/// `jane sheet audio`: everything, into `out/audio`.
pub fn sheet(out: &Path) -> Result<(), String> {
    let dir = out.join("audio");
    let lib = jane_audio::library();
    let mut all: Vec<String> = lib.sfx.iter().map(|p| format!("sfx:{}", p.name)).collect();
    all.extend(Bed::ALL.iter().map(|b| format!("bed:{}", b.name())));
    all.extend(lib.songs.iter().map(|s| format!("song:{}", s.name)));
    all.extend(["scene:nine", "scene:six", "scene:combat", "play:7"].map(String::from));
    for what in &all {
        let v = render(what, 1, None, &[])?;
        let name = what.replace(':', "-");
        write_wav(&dir.join(format!("{name}.wav")), &v)?;
        if what.starts_with("song:")
            || what.starts_with("scene:")
            || what.starts_with("play:")
            || what == "sfx:bell_far"
            || what == "sfx:bell_near"
        {
            let img = picture(&v, what);
            let path = dir.join(format!("{name}.png"));
            std::fs::write(&path, img.png()).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        report(what, &v);
    }
    Ok(())
}

fn list() {
    let lib = jane_audio::library();
    println!("sound effects ({}):", lib.sfx.len());
    for p in &lib.sfx {
        println!("  {:16} {} layer(s), {} variant(s)", p.name, p.layers.len(), p.variants);
    }
    println!("beds ({}):", Bed::ALL.len());
    for b in Bed::ALL {
        println!("  {}", b.name());
    }
    println!("songs ({}):", lib.songs.len());
    for s in &lib.songs {
        let bpm = 1800.0 / s.step_ticks as f32;
        println!("  {:16} {} {}, {:.0} eighths a minute{}: {}", s.name, s.key, s.mode.name(), bpm * 2.0, if s.looped { "" } else { ", once" }, s.mood);
    }
}

/// The engine at the game's rate, `seed`'s county.
fn engine(seed: u32) -> Engine {
    Engine::new(jane_audio::library(), RATE as f32, seed)
}

/// Seconds of one pass of a song's first form, plus its ring.
fn song_secs(e: &Engine, i: usize) -> f32 {
    let s = &e.songs()[i];
    let steps = s.form_steps(0) as f32;
    (steps * s.step_ticks as f32 / 60.0 + 4.0).min(150.0)
}

/// Renders `what` (interleaved stereo at `RATE`).
pub fn render(what: &str, seed: u32, secs: Option<f32>, args: &[String]) -> Result<Vec<f32>, String> {
    let mut e = engine(seed);
    let (kind, name) = what.split_once(':').unwrap_or(("song", what));
    match kind {
        "sfx" => {
            let id = e.sfx_index(name).ok_or_else(|| format!("no sound effect {name}"))?;
            let len = e.sfx()[id].variants[0].len() as f32 / RATE as f32;
            e.handle(Cmd::Sfx { id, gain: 1.0, pan: 0.0, send: 0.0, rate: 1.0 });
            Ok(e.render_secs(secs.unwrap_or(len + 1.5)))
        }
        "bed" => {
            let bed = Bed::from_name(name).ok_or_else(|| format!("no bed {name}"))?;
            e.handle(Cmd::Bed { bed, level: 1.0 });
            Ok(e.render_secs(secs.unwrap_or(20.0)))
        }
        "song" => {
            let i = e.song_index(name).ok_or_else(|| format!("no song {name}"))?;
            let s = secs.unwrap_or_else(|| song_secs(&e, i));
            e.handle(Cmd::Music { song: Some(i), fade_out_ms: 0.0, fade_in_ms: 0.0 });
            Ok(e.render_secs(s))
        }
        "scene" => scene(&mut e, name, secs),
        "play" => {
            let county = name.parse::<u32>().map_err(|_| format!("play:<seed>, not play:{name}"))?;
            play(county, secs.unwrap_or(40.0), args)
        }
        "inst" => {
            // A phrase up the instrument's middle: C3 E3 G3 C4 E4 G4 C5, the last held.
            let notes = [(48, 0.5), (52, 0.5), (55, 0.5), (60, 0.5), (64, 0.5), (67, 0.5), (72, 1.5)];
            let m = jane_audio::engine::audition(jane_audio::library(), name, &notes, RATE as f32)
                .ok_or_else(|| format!("no instrument {name}"))?;
            Ok(m.iter().flat_map(|s| [*s, *s]).collect())
        }
        _ => Err(format!("{what}: sfx:, bed:, song: or scene:")),
    }
}

/// A few seconds of the game as the cue table would play them.
fn scene(e: &mut Engine, name: &str, secs: Option<f32>) -> Result<Vec<f32>, String> {
    let song = |e: &Engine, n: &str| e.song_index(n).ok_or_else(|| format!("no song {n}"));
    let sfx = |e: &Engine, n: &str| e.sfx_index(n).ok_or_else(|| format!("no sound {n}"));
    // (seconds, command)
    let mut plan: Vec<(f32, Cmd)> = Vec::new();
    let total = match name {
        // Dusk in the Lowfields, then the bell at nine: nine strikes over its cue, and the night.
        "nine" | "six" => {
            let (strikes, before, after) =
                if name == "nine" { (9, "lowfields_day", "lowfields_night") } else { (6, "lowfields_night", "lowfields_day") };
            plan.push((0.0, Cmd::Music { song: Some(song(e, before)?), fade_out_ms: 0.0, fade_in_ms: 0.0 }));
            plan.push((0.0, Cmd::Bed { bed: if name == "nine" { Bed::Birds } else { Bed::Crickets }, level: 0.6 }));
            plan.push((8.0, Cmd::Music { song: Some(song(e, "bell")?), fade_out_ms: 2500.0, fade_in_ms: 1500.0 }));
            let bell = sfx(e, "bell_far")?;
            for k in 0..strikes {
                plan.push((9.0 + k as f32 * crate::audio_cmd::STRIKE_SECS, Cmd::Sfx { id: bell, gain: 1.0, pan: 0.0, send: 0.0, rate: 1.0 }));
            }
            let end = 9.0 + strikes as f32 * STRIKE_SECS + 5.0;
            plan.push((end - 5.0, Cmd::Bed { bed: if name == "nine" { Bed::Birds } else { Bed::Crickets }, level: 0.0 }));
            plan.push((end - 3.0, Cmd::Bed { bed: if name == "nine" { Bed::Crickets } else { Bed::Birds }, level: 0.6 }));
            plan.push((end, Cmd::Music { song: Some(song(e, after)?), fade_out_ms: 3000.0, fade_in_ms: 2000.0 }));
            end + 20.0
        }
        // A walk interrupted: the zone's music, then a fight, then its tail back to the zone.
        "combat" => {
            plan.push((0.0, Cmd::Music { song: Some(song(e, "lowfields_night")?), fade_out_ms: 0.0, fade_in_ms: 0.0 }));
            plan.push((0.0, Cmd::Bed { bed: Bed::Crickets, level: 0.5 }));
            plan.push((6.0, Cmd::Music { song: Some(song(e, "combat")?), fade_out_ms: 700.0, fade_in_ms: 300.0 }));
            let swing = sfx(e, "swing")?;
            let hit = sfx(e, "strike")?;
            for k in 0..10 {
                let t = 7.0 + k as f32 * 1.7;
                plan.push((t, Cmd::Sfx { id: swing, gain: 0.8, pan: 0.1, send: 0.0, rate: 1.0 }));
                plan.push((t + 0.15, Cmd::Sfx { id: hit, gain: 0.8, pan: -0.2, send: 0.0, rate: 1.0 }));
            }
            plan.push((34.0, Cmd::Music { song: Some(song(e, "lowfields_night")?), fade_out_ms: 2500.0, fade_in_ms: 2500.0 }));
            50.0
        }
        _ => return Err(format!("no scene {name}: nine, six or combat")),
    };
    let total = secs.unwrap_or(total);
    let mut out = Vec::with_capacity((total * RATE as f32) as usize * 2);
    let chunk = 0.05;
    let mut t = 0.0f32;
    while t < total {
        for (_, c) in plan.iter().filter(|(at, _)| *at >= t && *at < t + chunk) {
            e.handle(*c);
        }
        out.extend(e.render_secs(chunk));
        t += chunk;
    }
    Ok(out)
}

/// The sim with the tick's events kept, for the model and the cue table both.
struct Heard {
    sim: jane_sim::Sim,
    events: Vec<jane_sim::Event>,
}

impl jane_bot::Host for Heard {
    fn sim(&self) -> &jane_sim::Sim {
        &self.sim
    }

    fn view(&self, seat: jane_sim::Seat) -> Option<jane_sim::View<'_>> {
        self.sim.view(seat)
    }

    fn step(&mut self, input: &jane_sim::StepInput<'_>) -> jane_sim::Stepped {
        self.sim.step(input)
    }

    fn drain_events(&mut self) -> &[jane_sim::Event] {
        self.events.clear();
        self.events.extend_from_slice(self.sim.drain_events());
        &self.events
    }
}

/// The cue table's asks, straight into an engine: what the app's `Sound` does over its channel.
struct EngineBus<'a> {
    e: &'a mut Engine,
    cue: Option<jane_present::audio::MusicCue>,
}

impl jane_present::audio::AudioBus for EngineBus<'_> {
    fn music(&mut self, cue: jane_present::audio::MusicCue) {
        let (out, fade_in) = jane_present::audio::fades(self.cue, cue);
        self.cue = Some(cue);
        let song = cue.song().and_then(|n| self.e.song_index(n));
        self.e.handle(Cmd::Music { song, fade_out_ms: f32::from(out), fade_in_ms: f32::from(fade_in) });
    }

    fn sfx(&mut self, kind: jane_present::audio::SfxKind, at: jane_present::audio::At, listener: jane_present::audio::At) {
        let (Some(id), Some(p)) = (self.e.sfx_index(kind.name()), jane_present::audio::place(at, listener)) else { return };
        self.e.handle(Cmd::Sfx { id, gain: p.gain, pan: p.pan, send: p.send, rate: 1.0 });
    }

    fn bed(&mut self, bed: jane_present::audio::Bed, level: u8) {
        if let Some(b) = Bed::from_name(bed.name()) {
            self.e.handle(Cmd::Bed { bed: b, level: f32::from(level) / 255.0 });
        }
    }

    fn tick(&mut self) {}
}

/// `play:<seed>`: a model plays the county and the game's own cue table drives the engine, a tick
/// of sound for each tick of the sim.
fn play(seed: u32, secs: f32, args: &[String]) -> Result<Vec<f32>, String> {
    use jane_sim::input::{Command, DevOp, InputFrame, StampedCommand, StepInput};
    let model = arg(args, "--model").map_or(Some(jane_bot::Model::Reader), jane_bot::Model::parse).ok_or("--model: reader or rusher")?;
    let warm = arg(args, "--warm").map_or(Ok(0), str::parse::<u32>).map_err(|e| format!("--warm: {e}"))?;
    let (hour, minute) = match arg(args, "--hour").unwrap_or("20:58").split_once(':') {
        Some((h, m)) => (h.parse::<u8>().map_err(|e| format!("--hour: {e}"))?, m.parse::<u8>().map_err(|e| format!("--hour: {e}"))?),
        None => (arg(args, "--hour").unwrap_or("20").parse::<u8>().map_err(|e| format!("--hour: {e}"))?, 0),
    };
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let mut host = Heard { sim: jane_sim::Sim::new_game_with(bps, "Jane"), events: Vec::new() };
    let mut bot = jane_bot::Bot::story(model);
    let seat = jane_sim::Seat(0);
    for _ in 0..warm {
        if bot.done() {
            break;
        }
        bot.step(&mut host);
    }
    let idle = |host: &mut Heard, commands: &[StampedCommand]| {
        host.sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands });
        host.events.clear();
        host.events.extend_from_slice(host.sim.drain_events());
    };
    let cmd = [StampedCommand { seat: Some(seat), seq: u16::MAX, cmd: Command::Dev(DevOp::Time { hour }) }];
    idle(&mut host, &cmd);
    for _ in 0..u32::from(minute) * 120 {
        idle(&mut host, &[]);
    }
    let mut e = engine(seed);
    let mut track = jane_present::audio::Soundtrack::new();
    let per_tick = (RATE / 60) as usize;
    let ticks = (secs * 60.0) as u32;
    let mut out = Vec::with_capacity(ticks as usize * per_tick * 2);
    let mut buf = vec![0.0f32; per_tick * 2];
    let mut heard: Vec<String> = Vec::new();
    for t in 0..ticks {
        if bot.done() {
            idle(&mut host, &[]);
        } else {
            bot.step(&mut host);
        }
        let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
        track.tick(&v, &host.events, &mut EngineBus { e: &mut e, cue: track.cue() });
        let now = format!("{:?}", track.cue().and_then(jane_present::audio::MusicCue::song));
        if heard.last().is_none_or(|l| !l.ends_with(&now)) {
            heard.push(format!("{:.1}s {now}", t as f32 / 60.0));
        }
        e.render(&mut buf);
        out.extend_from_slice(&buf);
    }
    let v = host.sim.view(seat).ok_or("seat 0 is not in the world")?;
    let (clock, day) = v.clock();
    println!(
        "  play:{seed}: {} in the {}, day {day} {:02}:{:02} at the end; the music: {}",
        model.name(),
        v.zone().name(),
        clock / 7200,
        clock % 7200 / 120,
        heard.join(", ")
    );
    Ok(out)
}

/// Seconds between the bell's strikes (a hand-rung tower bell: a stroke, a swing back).
pub const STRIKE_SECS: f32 = 2.5;

fn write_wav(path: &Path, v: &[f32]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(path, jane_audio::wav::wav(v, 2, RATE)).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

/// Pitch classes by name, C first.
const NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// One line of numbers about a render.
fn report(what: &str, v: &[f32]) {
    let m = analysis::mono(v);
    let sr = RATE as f32;
    let k = analysis::keys(&analysis::chroma(&m, sr));
    let key = k.first().map_or(String::new(), |(t, minor, c)| format!("{} {} ({c:.2})", NAMES[*t as usize], if *minor { "minor" } else { "major" }));
    println!(
        "  {what}: {:.1} s, peak {:.1} dBFS, loudness {:.1} dBFS, dc {:.4}, {} click(s){}",
        m.len() as f32 / sr,
        to_db(analysis::peak(v)),
        analysis::loudness(&m, sr),
        analysis::dc(&m),
        analysis::clicks(&m, sr).len(),
        if what.starts_with("song:") { format!(", heard in {key}") } else { String::new() }
    );
    if what.starts_with("song:") || what.starts_with("play:") || what.starts_with("scene:") {
        let (b, centroid) = analysis::balance(&m, sr);
        let pct: Vec<String> = b.iter().map(|x| format!("{:.1}", 100.0 * x)).collect();
        println!("    balance: centroid {centroid:.0} Hz; power under 150, 500, 2k, 5k, 10k Hz and above, %: {}", pct.join(" "));
    }
    if what.starts_with("song:") {
        let c = analysis::chroma(&m, sr);
        let top = c.iter().fold(1e-12f32, |a, b| a.max(*b));
        let bars: Vec<String> = c.iter().enumerate().map(|(i, v)| format!("{}:{:.0}", NAMES[i], 9.0 * v / top)).collect();
        println!("    chroma {}", bars.join(" "));
    }
}

/// How far `jane audio check` lets a cue sit from its loudness (the tests hold it to 2 dB).
pub const LOUDNESS_TOLERANCE: f32 = 2.0;

fn check(seed: u32, secs: Option<f32>) -> Result<(), String> {
    let lib = jane_audio::library();
    let mut bad = 0;
    for s in &lib.songs {
        let v = render(&format!("song:{}", s.name), seed, secs, &[])?;
        let m = analysis::mono(&v);
        let sr = RATE as f32;
        let loud = analysis::loudness(&m, sr);
        let peak = analysis::peak(&v);
        let clicks = analysis::clicks(&m, sr).len();
        let tonic = jane_audio::model::pitch_class(&s.key).unwrap_or(0);
        let keys = analysis::keys(&analysis::chroma(&m, sr));
        let heard = keys.iter().take(3).any(|&(t, minor, _)| key_fits(tonic, s.mode, t, minor));
        let want = jane_audio::LOUDNESS - s.under;
        let ok_loud = (loud - want).abs() <= LOUDNESS_TOLERANCE;
        let ok = ok_loud && peak <= jane_audio::CEILING + 1e-4 && clicks == 0 && heard;
        if !ok {
            bad += 1;
        }
        if clicks > 0 {
            let at: Vec<String> = analysis::clicks(&m, sr).iter().take(4).map(|i| format!("{:.2} s", *i as f32 / sr)).collect();
            println!("     {} clicks at {}", s.name, at.join(", "));
        }
        println!(
            "{} {:16} {:>5.1} dBFS (gain x{:.2} to hit {want}), peak {:.1}, {clicks} click(s), key {} {}: heard {:?}",
            if ok { "ok  " } else { "FAIL" },
            s.name,
            loud,
            s.gain * jane_audio::dsp::db(want - loud),
            to_db(peak),
            s.key,
            s.mode.name(),
            &keys[..2]
        );
    }
    for p in &lib.sfx {
        let r = jane_audio::patch::render(p, RATE as f32, 0x5eed);
        for (i, v) in r.variants.iter().enumerate() {
            let dc = analysis::dc(v);
            let ok = analysis::peak(v) <= 1.0 && dc.abs() < 0.01 && v[0].abs() < 0.02 && v[v.len() - 1].abs() < 1e-3;
            if !ok {
                bad += 1;
                println!("FAIL sfx {} variant {i}: dc {dc}, first {}, last {}", p.name, v[0], v[v.len() - 1]);
            }
        }
    }
    if bad > 0 { Err(format!("{bad} failure(s)")) } else { Ok(()) }
}

/// Whether a key heard (`t`, `minor`) fits a song written on `tonic` in `mode`: its own tonic, or
/// the major scale its notes come from (a mode is heard as its parent major or that major's
/// relative minor as often as as itself).
pub fn key_fits(tonic: i32, mode: jane_audio::model::Mode, t: i32, minor: bool) -> bool {
    use jane_audio::model::Mode;
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

/// A waveform strip over a log-frequency spectrogram, `what` written in the corner.
fn picture(v: &[f32], what: &str) -> Image {
    let (w, sh, wh) = (1200u32, 300u32, 90u32);
    let m = analysis::mono(v);
    let mut img = Image::new(w, sh + wh + 24, [14, 12, 16, 255]);
    // The waveform: each column's min and max, amber on the dark.
    let per = (m.len() / w as usize).max(1);
    let mid = 24 + wh / 2;
    for x in 0..w {
        let a = (x as usize * per).min(m.len());
        let b = (a + per).min(m.len());
        let (lo, hi) = m[a..b].iter().fold((0.0f32, 0.0f32), |(l, h), s| (l.min(*s), h.max(*s)));
        let y0 = (mid as f32 - hi * (wh as f32 / 2.0)) as u32;
        let y1 = (mid as f32 - lo * (wh as f32 / 2.0)) as u32;
        for y in y0..=y1.max(y0) {
            img.set(x, y.min(24 + wh - 1), [232, 176, 92, 255]);
        }
        // The ceiling.
        img.set(x, mid - (jane_audio::CEILING * wh as f32 / 2.0) as u32, [120, 40, 40, 255]);
    }
    let rows = analysis::spectrogram(&m, RATE as f32, w as usize, sh as usize);
    for (y, row) in rows.iter().enumerate() {
        for (x, db) in row.iter().enumerate() {
            let t = ((db + 90.0) / 80.0).clamp(0.0, 1.0);
            img.set(x as u32, 24 + wh + y as u32, heat(t));
        }
    }
    // Octave lines at 55, 110, ... 14080 Hz.
    let (lo, hi) = (40f32.ln(), 16_000f32.ln());
    let mut hz = 55.0f32;
    while hz < 16_000.0 {
        let y = sh as f32 * (1.0 - (hz.ln() - lo) / (hi - lo));
        for x in (0..w).step_by(6) {
            img.set(x, 24 + wh + y as u32, [70, 70, 80, 255]);
        }
        hz *= 2.0;
    }
    let font = jane_art::Font::build();
    jane_art::sheet::label(&mut img, &font, 6, 4, what, jane_art::font::Face::Small, [230, 220, 200]);
    img
}

/// Dark violet to amber to near white.
fn heat(t: f32) -> [u8; 4] {
    let c = |a: f32, b: f32, x: f32| (a + (b - a) * x) as u8;
    let (r, g, b) = if t < 0.5 {
        let x = t / 0.5;
        (c(10.0, 150.0, x), c(8.0, 40.0, x), c(20.0, 90.0, x))
    } else {
        let x = (t - 0.5) / 0.5;
        (c(150.0, 255.0, x), c(40.0, 230.0, x), c(90.0, 170.0, x))
    };
    [r, g, b, 255]
}
