//! `jane play` and `jane replay` (ARCHITECTURE.md §8; VERIFICATION.md §3.2): a player model of
//! `jane-bot` plays a seed headless, one line per milestone, optionally recorded to a `.jrp`;
//! tapes are re-simulated and held to their hash streams, and two tapes are compared to the
//! frame and the part of the state where they part.

use std::path::Path;
use std::time::Instant;

use jane_bot::crawl::Crawl;
use jane_bot::{Bot, Model, Plan};
use jane_core::ZoneId;
use jane_sim::replay::{Recorder, Tape, diff_states, input_at, verify_tape};
use jane_sim::{Blueprints, Seat, Sim, StepInput};

pub const USAGE: &str = "  play --model reader|rusher --seed N [--minutes M] [--dungeon ZONE] [--tape OUT.jrp]
       [--snap OUT.png [--snap-every S]] [--ending hold|hill|train] [--profile]
       [--explain] [--explain-every S]
                                      a player model plays a seed headless from New Game (or a dungeon from
                                      its door, the console setting up the kit): one line per milestone;
                                      --snap draws the world round her at the end (and every S seconds of
                                      play as OUT-0001.png ...): the sim's view as a map, not the renderer;
                                      --ending: which of the three the bot chooses at Yours to Say;
                                      --profile: where the time went, the sim's phases and the bot's;
                                      --explain: what the bot holds, is doing and is blocked by, at the end
                                      (and every S seconds of play)
  play --fixture PATH                 write the bot-session hash fixture (seeds 1 to 3, both models, 5 min)
  replay verify FILE...               re-simulate each tape and hold it to its hash stream
  replay record --model M --seed N [--minutes M] [--dungeon ZONE] OUT.jrp
                                      a model's session, recorded
  replay diff A.jrp B.jrp             where two tapes part: headers, input, then the state (zone and part)";

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn num(args: &[String], name: &str, default: u32) -> Result<u32, String> {
    flag(args, name).map_or(Ok(default), |s| s.parse().map_err(|_| format!("{name}: not a number: {s}")))
}

/// `jane play ...` and `jane replay ...`.
pub fn main(cmd: &str, args: &[String]) -> std::process::ExitCode {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    match run(cmd, args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("jane {cmd}: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

pub fn run(cmd: &str, args: &[String]) -> Result<(), String> {
    match (cmd, args.first().map(String::as_str)) {
        ("play", _) if flag(args, "--fixture").is_some() => fixture(flag(args, "--fixture").expect("a path")),
        ("play", _) => play(args, flag(args, "--tape")),
        ("replay", Some("verify")) => verify(&args[1..]),
        ("replay", Some("record")) => {
            let out =
                args.last().filter(|a| a.ends_with(".jrp")).ok_or("replay record: the last argument is OUT.jrp")?;
            play(&args[1..], Some(out))
        }
        ("replay", Some("diff")) => match &args[1..] {
            [a, b] => diff(a, b),
            _ => Err("replay diff: two tapes".into()),
        },
        _ => Err(format!("{cmd}: see `jane help`")),
    }
}

fn build(seed: u32) -> Result<Blueprints, String> {
    let t0 = Instant::now();
    let bps = Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    eprintln!("seed {seed}: 13 zones built in {} ms", t0.elapsed().as_millis());
    Ok(bps)
}

fn play(args: &[String], tape: Option<&str>) -> Result<(), String> {
    let model = Model::parse(flag(args, "--model").unwrap_or("reader")).ok_or("--model: reader or rusher")?;
    let seed = num(args, "--seed", 1)?;
    let minutes = num(args, "--minutes", 5)?;
    let dungeon = flag(args, "--dungeon")
        .map(|z| ZoneId::from_name(z).ok_or_else(|| format!("--dungeon: no zone {z}")))
        .transpose()?;
    let bps = build(seed)?;
    let mut sim = Sim::new_game_with(bps, "Jane");
    let profile = args.iter().any(|a| a == "--profile");
    if profile {
        sim.set_wall_clock(Some(wall_ns));
    }
    let mut bot = match dungeon {
        Some(z) => {
            let mut b = Bot::new(model, Plan::Crawl(Crawl::new(z)));
            b.setup = jane_bot::crawl::setup(sim.blueprints(), z);
            b
        }
        None => Bot::story(model),
    };
    if let Some(e) = flag(args, "--ending") {
        bot.ending = Some(jane_bot::Ending::parse(e).ok_or("--ending: hold, hill or train")?);
    }
    let frames = minutes * 60 * 60;
    let mut rec = Recorder::new(sim);
    let t0 = Instant::now();
    let mut shown = 0;
    let mut played: u32 = 0;
    let snap = flag(args, "--snap");
    let every = num(args, "--snap-every", 0)? * 60;
    let mut shots = 0;
    let mut prof = Profile::default();
    let explain_every = num(args, "--explain-every", 0)? * 60;
    for _ in 0..frames {
        if bot.done() {
            break;
        }
        let f0 = if profile { wall_ns() } else { 0 };
        bot.step(&mut rec);
        if profile {
            prof.add(wall_ns() - f0, &rec.sim().metrics());
        }
        played += 1;
        if explain_every > 0 && played % explain_every == 0 {
            if let Some(v) = rec.view(Seat(0)) {
                println!("{}", bot.explain(&v));
            }
        }
        while shown < bot.log.len() {
            println!("{}", bot.log[shown].line());
            shown += 1;
        }
        if let Some(path) = snap
            && every > 0
            && played % every == 0
        {
            shots += 1;
            let v = rec.view(Seat(0)).ok_or("seat 0 is not in the world")?;
            write_snap(&v, &numbered(path, shots))?;
        }
    }
    let us = t0.elapsed().as_micros().max(1);
    let (sim, t) = rec.finish();
    let v = sim.view(Seat(0)).ok_or("seat 0 is not in the world")?;
    let fps = u128::from(played) * 1_000_000 / us;
    println!(
        "{} seed {seed}: {played} frames ({} ticks) in {} ms, {fps} frames/s; {} in the {}, hp {}/{}; {} quests done; hash {:016x}",
        model.name(),
        sim.state().tick.0,
        us / 1000,
        if bot.done() { "stopped" } else { "still going" },
        v.zone().name(),
        v.body().hp.points(),
        jane_sim::units::max_hp(v.body()).points(),
        v.quests_done().len(),
        sim.hash()
    );
    if let Some(why) = bot.stuck() {
        println!("last stuck: {why}");
    }
    if profile {
        prof.print(played);
    }
    if args.iter().any(|a| a == "--explain") {
        println!("{}", bot.explain(&v));
    }
    if let Some(path) = snap {
        write_snap(&v, path)?;
    }
    if let Some(path) = tape {
        let bytes = t.encode();
        std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        println!(
            "tape: {path}, {} frames in {} runs, {} hashes, {} bytes",
            t.frames,
            t.runs.len(),
            t.hashes.len(),
            bytes.len()
        );
    }
    Ok(())
}

/// Monotonic nanoseconds since the first call: the wall clock lent to the sim for its metrics.
fn wall_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

/// Where the frames' time went: the sim's phases, and the bot (the frame less its step).
#[derive(Default)]
struct Profile {
    frame_ns: u64,
    step_ns: u64,
    phase_ns: [u64; jane_sim::metrics::PHASES],
    paths: u64,
    expanded: u64,
    awake: u64,
    total: u64,
    /// The slowest steps.
    worst: Vec<jane_sim::SimMetrics>,
    steps: Vec<u32>,
}

/// `num / den` with two decimals, in integers.
fn ratio(num: u64, den: u64) -> String {
    let h = num * 100 / den.max(1);
    format!("{}.{:02}", h / 100, h % 100)
}

impl Profile {
    fn add(&mut self, frame_ns: u64, m: &jane_sim::SimMetrics) {
        self.frame_ns += frame_ns;
        self.step_ns += u64::from(m.step_ns);
        for (a, b) in self.phase_ns.iter_mut().zip(m.phase_ns) {
            *a += u64::from(b);
        }
        self.paths += u64::from(m.path_searches);
        self.expanded += u64::from(m.path_expanded);
        self.awake += u64::from(m.units_awake);
        self.total += u64::from(m.units_total);
        self.steps.push(m.step_ns);
        self.worst.push(*m);
        if self.worst.len() > 64 {
            self.worst.sort_unstable_by_key(|m| std::cmp::Reverse(m.step_ns));
            self.worst.truncate(8);
        }
    }

    fn print(&mut self, frames: u32) {
        let n = u64::from(frames.max(1));
        self.steps.sort_unstable();
        let pct = |p: usize| u64::from(self.steps.get(self.steps.len().saturating_sub(1) * p / 100).copied().unwrap_or(0));
        println!(
            "profile: {frames} frames; per frame {} us = sim {} us + bot {} us; sim median {} us, p99 {} us",
            ratio(self.frame_ns, n * 1000),
            ratio(self.step_ns, n * 1000),
            ratio(self.frame_ns.saturating_sub(self.step_ns), n * 1000),
            ratio(pct(50), 1000),
            ratio(pct(99), 1000),
        );
        for p in jane_sim::Phase::ALL {
            println!("  {:<13} {:>8} us/frame", p.name(), ratio(self.phase_ns[p.index()], n * 1000));
        }
        println!(
            "  paths {}/frame ({} nodes each); units awake {} of {}",
            ratio(self.paths, n),
            ratio(self.expanded, self.paths.max(1)),
            ratio(self.awake, n),
            ratio(self.total, n)
        );
        self.worst.sort_unstable_by_key(|m| std::cmp::Reverse(m.step_ns));
        for m in self.worst.iter().take(3) {
            let top: Vec<String> = {
                let mut ph: Vec<(u32, jane_sim::Phase)> =
                    jane_sim::Phase::ALL.iter().map(|&p| (m.phase_ns[p.index()], p)).collect();
                ph.sort_unstable_by_key(|&(t, p)| (std::cmp::Reverse(t), p));
                ph.iter().take(3).map(|(t, p)| format!("{} {} us", p.name(), t / 1000)).collect()
            };
            println!("  slow step at frame {}: {} us ({})", m.frame, m.step_ns / 1000, top.join(", "));
        }
    }
}

/// `snap.png`, 3 -> `snap-0003.png`.
fn numbered(path: &str, n: u32) -> String {
    let (stem, ext) = path.rsplit_once('.').unwrap_or((path, "png"));
    format!("{stem}-{n:04}.{ext}")
}

fn write_snap(v: &jane_sim::view::View<'_>, path: &str) -> Result<(), String> {
    std::fs::write(path, crate::snap::snap(v).png()).map_err(|e| format!("{path}: {e}"))?;
    println!("snap: {path}");
    Ok(())
}

fn fixture(path: &str) -> Result<(), String> {
    let t0 = Instant::now();
    let text = jane_bot::fixture::text(|seed| Blueprints::build(seed).expect("the seed builds"));
    if let Some(dir) = Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, &text).map_err(|e| format!("{path}: {e}"))?;
    println!("{path}: {} lines in {} ms", text.lines().count(), t0.elapsed().as_millis());
    Ok(())
}

fn verify(files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        return Err("replay verify: which tapes?".into());
    }
    let mut bad = 0;
    for f in files {
        let bytes = std::fs::read(f).map_err(|e| format!("{f}: {e}"))?;
        let t0 = Instant::now();
        let r = Tape::decode(&bytes).and_then(|tape| {
            let bps = Blueprints::build(tape.header.seed)
                .map_err(|e| jane_sim::replay::ReplayError::Build(jane_sim::SaveError::Build(e)))?;
            verify_tape(&tape, bps)
        });
        match r {
            Ok(v) => println!(
                "ok: {f}: {} frames, {} ticks, {} hashes, final {:016x} ({} ms)",
                v.frames,
                v.ticks,
                v.hashes,
                v.final_hash,
                t0.elapsed().as_millis()
            ),
            Err(e) => {
                bad += 1;
                println!("FAIL: {f}: {e}");
            }
        }
    }
    if bad > 0 { Err(format!("{bad} of {} tapes failed", files.len())) } else { Ok(()) }
}

fn diff(a: &str, b: &str) -> Result<(), String> {
    let read = |p: &str| -> Result<Tape, String> {
        let bytes = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        Tape::decode_unchecked(&bytes).map_err(|e| format!("{p}: {e}"))
    };
    let (ta, tb) = (read(a)?, read(b)?);
    if ta.header != tb.header {
        println!("headers differ:\n  {a}: {:?}\n  {b}: {:?}", ta.header, tb.header);
    }
    let n = ta.frames.min(tb.frames);
    match (0..n).find(|&f| input_at(&ta, f) != input_at(&tb, f)) {
        Some(f) => println!("input first differs at frame {f}"),
        None if ta.frames == tb.frames => println!("input: the same {n} frames"),
        None => println!("input: the same for {n} frames; one tape is longer ({} / {})", ta.frames, tb.frames),
    }
    let ours = jane_data::catalog().content_hash;
    if ta.header.content_hash != ours || tb.header.content_hash != ours || ta.header.seed != tb.header.seed {
        println!("not re-simulated: another seed or other content than this build's ({ours:016x})");
        return Ok(());
    }
    let bps = build(ta.header.seed)?;
    let fresh = || (ta.new_game_with(bps.clone()), tb.new_game_with(bps.clone()));
    let step = |s: &mut Sim, t: &Tape, f: u32| {
        if let Some((frames, commands)) = input_at(t, f) {
            s.step(&StepInput { frames, commands });
        }
    };
    // Hash every 60 frames to find the stretch, then frame by frame inside it.
    let (mut sa, mut sb) = fresh();
    let mut good = 0;
    let mut bad = None;
    for f in 0..n {
        step(&mut sa, &ta, f);
        step(&mut sb, &tb, f);
        if (f + 1) % 60 == 0 || f + 1 == n {
            if sa.hash() == sb.hash() {
                good = f + 1;
            } else {
                bad = Some(f + 1);
                break;
            }
        }
    }
    let Some(_) = bad else {
        println!("state: the same after all {n} common frames (final {:016x})", sa.hash());
        return Ok(());
    };
    let (mut sa, mut sb) = fresh();
    for f in 0..n {
        step(&mut sa, &ta, f);
        step(&mut sb, &tb, f);
        if f + 1 > good && sa.hash() != sb.hash() {
            println!("state first differs after frame {f} (tick {} / {})", sa.state().tick.0, sb.state().tick.0);
            let (za, zb) = (sa.zone_hashes(), sb.zone_hashes());
            let zones: Vec<&str> =
                ZoneId::ALL.iter().filter(|z| za[z.index()] != zb[z.index()]).map(|z| z.name()).collect();
            println!("zones that differ: {zones:?}");
            for p in diff_states(sa.state(), sb.state()) {
                println!("  {p}");
            }
            return Ok(());
        }
    }
    Ok(())
}
