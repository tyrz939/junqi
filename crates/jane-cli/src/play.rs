//! `jane play` and `jane replay` (ARCHITECTURE.md §8; VERIFICATION.md §3.2): a player model of
//! `jane-bot` plays a seed headless, one line per milestone, optionally recorded to a `.jrp`;
//! tapes are re-simulated and held to their hash streams, and two tapes are compared to the
//! frame and the part of the state where they part.

use std::path::Path;
use std::time::Instant;

use jane_bot::crawl::Crawl;
use jane_bot::telemetry::Telemetry;
use jane_bot::{Bot, Model, Plan};
use jane_core::ZoneId;
use jane_sim::replay::{Recorder, Tape, diff_states, input_at, verify_tape};
use jane_sim::{Blueprints, Seat, Sim, StepInput};

pub const USAGE: &str =
    "  play --model reader|rusher|explorer|cautious|lost --seed N [--minutes M] [--dungeon ZONE] [--tape OUT.jrp]
       [--trace OUT.jtr] [--l4] [--telemetry DIR]
       [--snap OUT.png [--snap-every S]] [--ending hold|hill|train] [--profile]
       [--explain] [--explain-every S] [--from ACT] [--deaths] [--growth]
                                      a player model plays a seed headless from New Game (or a dungeon from
                                      its door, the console setting up the kit): one line per milestone;
                                      --snap draws the world round her at the end (and every S seconds of
                                      play as OUT-0001.png ...): the sim's view as a map, not the renderer;
                                      --ending: which of the three the bot chooses at Yours to Say;
                                      --profile: where the time went, the sim's phases and the bot's;
                                      --explain: what the bot holds, is doing and is blocked by, at the end
                                      (and every S seconds of play); --from: the story from an act
                                      (mine museum forest factory burial school choice), the acts
                                      before it written in by the console: for looking, never a tape;
                                      --trace: the session's trace (VERIFICATION.md §3.1) and its L4
                                      table; --l4: only the table; --telemetry: the run's CSVs (kills,
                                      blows, heals, rests, minutes, milestones, chapters, deaths,
                                      summary) into DIR (PLAY-PLAN.md 0.1)
  telemetry [--models M,M] [--seeds A..B | --seed N] [--minutes M] [--threads T] --out DIR
                                      the story from New Game for each model on each seed, in parallel:
                                      every run's CSVs and one summary.csv, the summary printed
  play --fixture PATH                write the bot-session hash fixture (seeds 1 to 3, both models, 5 min)
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
        ("telemetry", _) => telemetry(args),
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
    let model = Model::parse(flag(args, "--model").unwrap_or("reader"))
        .ok_or("--model: reader, rusher, explorer, cautious or lost")?;
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
    if let Some(act) = flag(args, "--from") {
        if tape.is_some() || dungeon.is_some() {
            return Err("--from: a story started part way is for looking at, not a tape or a crawl".into());
        }
        bot.setup = jane_bot::console::start_at(&mut sim, act)?;
    }
    if let Some(e) = flag(args, "--ending") {
        bot.ctx.ending = Some(jane_bot::Ending::parse(e).ok_or("--ending: hold, hill or train")?);
    }
    let frames = minutes * 60 * 60;
    let tele_dir = flag(args, "--telemetry");
    let mut tele = tele_dir.map(|_| Telemetry::new(model.name(), seed));
    let mut sess = jane_bot::run::Session::new(bot, &sim, minutes);
    let mut rec = Recorder::new(sim);
    let t0 = Instant::now();
    let mut shown = 0;
    let mut played: u32 = 0;
    let snap = flag(args, "--snap");
    let every = num(args, "--snap-every", 0)? * 60;
    let mut shots = 0;
    let mut prof = Profile::default();
    let explain_every = num(args, "--explain-every", 0)? * 60;
    let growth = args.iter().any(|a| a == "--growth");
    for _ in 0..frames {
        if sess.bot.done() {
            break;
        }
        let f0 = if profile { wall_ns() } else { 0 };
        sess.step(&mut rec);
        if let Some(t) = tele.as_mut() {
            t.observe(rec.sim(), sess.bot.events());
        }
        if profile {
            prof.add(wall_ns() - f0, &rec.sim().metrics());
        }
        played += 1;
        if explain_every > 0 && played % explain_every == 0 {
            if let Some(v) = rec.view(Seat(0)) {
                println!("{}", sess.bot.explain(&v));
            }
        }
        while shown < sess.bot.log.len() {
            let line = sess.bot.log[shown].line();
            println!("{line}");
            shown += 1;
            if growth && line.ends_with("quest given: the_burial") {
                print!("{}", growth_report(rec.sim()));
            }
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
    let (bot, trace) = sess.finish(rec.sim());
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
    if let (Some(t), Some(dir)) = (&tele, tele_dir) {
        let row = t.write(Path::new(dir), &bot).map_err(|e| format!("{dir}: {e}"))?;
        println!("telemetry: {dir}\n{}", table(&[row]));
    }
    if profile {
        prof.print(played);
    }
    // Every death, and a count by where and to what.
    if args.iter().any(|a| a == "--deaths" || a == "--deaths-why") {
        let cat = jane_data::catalog();
        let mut by: std::collections::BTreeMap<(String, String), u32> = std::collections::BTreeMap::new();
        for d in &bot.deaths {
            println!("death {}", d.line());
            if args.iter().any(|a| a == "--deaths-why") {
                for l in &d.before {
                    println!("      {l}");
                }
            }
            let who = d.by.map_or("?".to_owned(), |u| cat.combat.unit(u).id.to_owned());
            *by.entry((d.zone.name().to_owned(), who)).or_insert(0) += 1;
        }
        let mut rows: Vec<_> = by.into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        println!("deaths: {} in all", bot.deaths.len());
        for ((z, who), n) in rows {
            println!("  {n:>4} {z:<8} {who}");
        }
    }
    if growth {
        print!("{}", growth_report(&sim));
    }
    if args.iter().any(|a| a == "--explain") {
        println!("{}", bot.explain(&v));
    }
    // The ground round a prop of the zone she ends in, by key.
    if let Some(key) = flag(args, "--show-prop") {
        match v.sym(key).and_then(|k| jane_bot::sense::prop_by_key(&v, k)) {
            Some(p) => println!("{key} at {:?}:\n{}", p.cell, jane_bot::ascii(&v, jane_bot::sense::prop_rect(p), 6)),
            None => println!("{key}: not in the {}", v.zone().name()),
        }
    }
    if let Some(path) = snap {
        write_snap(&v, path)?;
    }
    if let Some(path) = flag(args, "--trace") {
        let bytes = trace.encode();
        std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
        println!("trace: {path}, {} records, {} bytes", trace.records.len(), bytes.len());
    }
    if args.iter().any(|a| a == "--l4" || a == "--trace") {
        print!("{}", crate::sweep::l4_table(&jane_bot::experience::measure(&trace, 0)));
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

/// `jane telemetry`: each model on each seed from New Game, in parallel, every run's tables and
/// one `summary.csv` written to `--out`, and the summary printed as a table.
fn telemetry(args: &[String]) -> Result<(), String> {
    let models = flag(args, "--models")
        .unwrap_or("reader")
        .split(',')
        .map(|m| Model::parse(m).ok_or_else(|| format!("--models: no model {m}")))
        .collect::<Result<Vec<_>, _>>()?;
    let seeds: Vec<u32> = if flag(args, "--seed").or(flag(args, "--seeds")).is_some() {
        crate::gen_cmd::seeds(args)?.collect()
    } else {
        (1..=3).collect()
    };
    let minutes = num(args, "--minutes", 900)?;
    let out = Path::new(flag(args, "--out").ok_or("--out DIR")?);
    let jobs: Vec<(Model, u32)> = models.iter().flat_map(|&m| seeds.iter().map(move |&s| (m, s))).collect();
    let threads = num(args, "--threads", std::thread::available_parallelism().map_or(4, |n| n.get() as u32))?.max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done: Vec<std::sync::Mutex<Option<Result<String, String>>>> =
        jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.min(jobs.len() as u32) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(m, seed)) = jobs.get(i) else { break };
                    let r = jane_bot::telemetry::play(m, seed, minutes)
                        .and_then(|(bot, t)| t.write(out, &bot).map_err(|e| format!("{}: {e}", out.display())));
                    eprintln!("{} seed {seed}: {}", m.name(), if r.is_ok() { "written" } else { "failed" });
                    *done[i].lock().expect("a run's slot") = Some(r);
                }
            });
        }
    });
    let mut rows = Vec::new();
    for d in done {
        rows.push(d.into_inner().expect("a run's slot").expect("every job ran")?);
    }
    let mut csv = format!("{}\n", jane_bot::telemetry::SUMMARY_HEADER);
    for r in &rows {
        csv.push_str(r);
        csv.push('\n');
    }
    let path = out.join("summary.csv");
    std::fs::write(&path, csv).map_err(|e| format!("{}: {e}", path.display()))?;
    print!("{}", table(&rows));
    println!("written: {}", out.display());
    Ok(())
}

/// Summary rows as an aligned table under their header.
fn table(rows: &[String]) -> String {
    let all: Vec<Vec<&str>> = std::iter::once(jane_bot::telemetry::SUMMARY_HEADER)
        .chain(rows.iter().map(String::as_str))
        .map(|r| r.split(',').collect())
        .collect();
    let cols = all.iter().map(Vec::len).max().unwrap_or(0);
    let width: Vec<usize> =
        (0..cols).map(|c| all.iter().filter_map(|r| r.get(c)).map(|s| s.len()).max().unwrap_or(0)).collect();
    let mut s = String::new();
    for r in &all {
        let cells: Vec<String> = r.iter().enumerate().map(|(i, c)| format!("{c:>w$}", w = width[i])).collect();
        s.push_str(&cells.join(" "));
        s.push('\n');
    }
    s
}

/// `--growth`: her health and strength, and every finding that grows her in the story's dungeons
/// and quests, found or not; for one not found, the prop as the zone has it (a zone never
/// entered has no state).
fn growth_report(sim: &Sim) -> String {
    use std::fmt::Write as _;
    let cat = jane_data::catalog();
    let st = sim.state();
    let g = &st.growth;
    let mut s = String::new();
    let hp = sim.view(Seat(0)).map_or(0, |v| jane_sim::units::max_hp(v.body()).points());
    let _ =
        writeln!(s, "growth: max hp {hp}; grown strength {} spirit {}; {} found", g.strength, g.spirit, g.found.len());
    let key_name = |bp: &jane_core::Blueprint, k: jane_core::Key| match k {
        jane_core::Key::Name(n) => cat.name(n).to_owned(),
        jane_core::Key::Local(i) => bp.local_names.get(i as usize).cloned().unwrap_or_default(),
    };
    let (mut all, mut got) = ((0, 0), (0, 0));
    // The dungeons the story walks before the Burial.
    let before = jane_bot::crawl::ORDER.iter().take_while(|&&z| z != jane_core::ZoneId::Burial);
    for &z in before {
        let bp = sim.blueprints().get(z);
        let zs = st.zones[z.index()].as_deref();
        let later = jane_bot::crawl::rooms_for_later(bp);
        for (i, p) in bp.props.iter().enumerate() {
            let for_later = later.iter().any(|r| r.contains(i32::from(p.cell.x), i32::from(p.cell.y)));
            for (stat, amount, id) in jane_bot::crawl::grows_of(bp, p) {
                let sym = match id {
                    jane_core::Key::Name(n) => Some(jane_sim::sym::of_name(n)),
                    jane_core::Key::Local(j) => bp.local_names.get(j as usize).and_then(|n| st.syms.find(n)),
                };
                let found = sym.is_some_and(|y| g.found.contains(&y));
                let str_ = stat == jane_core::action::Stat::Strength;
                let a = i32::from(amount);
                let (all_, got_) = if str_ { (&mut all.0, &mut got.0) } else { (&mut all.1, &mut got.1) };
                if !for_later {
                    *all_ += a;
                }
                if found {
                    *got_ += a;
                }
                let rooms: Vec<String> = bp
                    .rects
                    .iter()
                    .filter(|(_, r)| r.contains(i32::from(p.cell.x), i32::from(p.cell.y)))
                    .map(|(k, _)| key_name(bp, *k))
                    .collect();
                let state = match zs.and_then(|zs| zs.props.iter().find(|q| q.spawn == Some(i as u16))) {
                    None => "zone never entered".to_owned(),
                    Some(q) => format!("hidden {} locked {} used {}", q.hidden, q.locked, q.used),
                };
                let _ = writeln!(
                    s,
                    "  {} {:<8} {:<16} {:<8} {}{} at ({},{}) in {:?}: {}",
                    if found {
                        "got "
                    } else if for_later {
                        "LATE"
                    } else {
                        "MISS"
                    },
                    z.name(),
                    cat.story.prop(p.def).id,
                    key_name(bp, p.key),
                    if str_ { "str +" } else { "spi +" },
                    amount,
                    p.cell.x,
                    p.cell.y,
                    rooms,
                    if found { String::new() } else { state }
                );
            }
        }
    }
    let _ = writeln!(
        s,
        "  before the Burial: strength {}/{} spirit {}/{} (LATE: behind a verb had only later, not counted)",
        got.0, all.0, got.1, all.1
    );
    s
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
        let pct =
            |p: usize| u64::from(self.steps.get(self.steps.len().saturating_sub(1) * p / 100).copied().unwrap_or(0));
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
