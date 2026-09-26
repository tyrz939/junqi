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
                                      a player model plays a seed headless from New Game (or a dungeon from
                                      its door, the console setting up the kit): one line per milestone
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
    let sim = Sim::new_game_with(bps, "Jane");
    let mut bot = match dungeon {
        Some(z) => {
            let mut b = Bot::new(model, Plan::Crawl(Crawl::new(z)));
            b.setup = jane_bot::crawl::setup(sim.blueprints(), z);
            b
        }
        None => Bot::story(model),
    };
    let frames = minutes * 60 * 60;
    let mut rec = Recorder::new(sim);
    let t0 = Instant::now();
    let mut shown = 0;
    let mut played: u32 = 0;
    for _ in 0..frames {
        if bot.done() {
            break;
        }
        bot.step(&mut rec);
        played += 1;
        while shown < bot.log.len() {
            println!("{}", bot.log[shown].line());
            shown += 1;
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
