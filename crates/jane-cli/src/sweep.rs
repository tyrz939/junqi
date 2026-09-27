//! `jane sweep` and the L4 table (VERIFICATION.md §3.2): every model on every seed, traced;
//! their experience metrics, the L5 audits, and a report the owner can read.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use jane_bot::experience::{Experience, FIRST_HOUR, MINUTE, first_hour_value, in_band};

/// A unit row's id, or `?`.
pub fn unit_name(d: Option<u16>) -> &'static str {
    d.map_or("?", |d| jane_data::catalog().combat.unit(jane_core::UnitDefId(d)).id)
}

pub fn quest_name(q: u16) -> &'static str {
    jane_data::catalog().story.quest(jane_core::QuestId(q)).id
}

pub const REGIONS: [&str; 3] = ["Lowfields", "Waters", "Works"];

/// The L4 table for one experience, as text.
pub fn l4_table(x: &Experience) -> String {
    let m = Experience::min;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "L4 {} seed {} seat {}: {} min played, {} quests done, {} deaths ({}.{} an hour)",
        x.model,
        x.seed,
        x.seat,
        m(x.frames),
        x.quests.iter().filter(|q| q.done.is_some()).count(),
        x.deaths.len(),
        x.deaths_per_hour_tenths() / 10,
        x.deaths_per_hour_tenths() % 10
    );
    let _ = writeln!(
        s,
        "  no objective {} min, looking {} min (first hour {} + {}); walked {} cells, backtracked {} ({}%); walk:play {}.{:02}; night unlit, first night {} min",
        m(x.idle_by_hour.iter().sum()),
        m(x.search_by_hour.iter().sum()),
        m(x.idle_in(60)),
        m(x.searching_in(60)),
        x.walked_cells,
        x.backtrack_cells,
        x.backtrack_cells * 100 / x.walked_cells.max(1),
        x.walk_to_play() / 100,
        x.walk_to_play() % 100,
        m(x.night_unlit_first * 60)
    );
    let long: Vec<_> = x.stretches_over(60).collect();
    let _ = writeln!(
        s,
        "  empty walks (60 s or more, nothing new in view): {}; over 2 min {}; longest {} s; bare (nothing in view) {}",
        long.len(),
        x.stretches_over(120).count(),
        long.iter().map(|s| s.secs).max().unwrap_or(0),
        long.iter().filter(|s| s.bare).count()
    );
    for b in FIRST_HOUR.iter().filter(|b| b.models.contains(&x.model.as_str())) {
        let v = first_hour_value(x, b);
        let _ = writeln!(
            s,
            "  §4.1 {:<48} {:>8} min  band {}..{}  {}",
            b.claim,
            v.map_or("never".into(), m),
            b.lo.map_or(String::new(), m),
            b.hi.map_or(String::new(), m),
            if in_band(b, v) { "ok" } else { "OUT" }
        );
    }
    let _ = MINUTE;
    s
}

/// L5's step table for a seed, as Markdown rows: every step whose verdict is bad (or all, with
/// `all`).
pub fn truth_rows(steps: &[jane_bot::audit::StepAudit], all: bool) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "| Quest | Step | Words | Where | To road | Verdict |");
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- |");
    for a in steps.iter().filter(|a| all || a.verdict.bad()) {
        let step = if a.step == 255 { "back to".to_owned() } else { format!("{}", a.step + 1) };
        let at = match (a.zone, a.at) {
            (Some(z), Some(c)) => format!("{} {},{}", z.name(), c.0, c.1),
            _ => "-".into(),
        };
        let _ = writeln!(
            s,
            "| {} | {step} | {} | {at} | {} | {} |",
            a.quest,
            a.text.replace('|', "/"),
            a.to_road.map_or("-".into(), |d| d.to_string()),
            a.verdict.word().replace('|', "/")
        );
    }
    s
}

/// `jane audit truth --seed N [--all]`: L5 on the built world.
pub fn audit(args: &[String]) -> Result<(), String> {
    let flag = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).map(String::as_str);
    let seed: u32 = flag("--seed").unwrap_or("1").parse().map_err(|_| "--seed: a number")?;
    let all = args.iter().any(|a| a == "--all");
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let steps = jane_bot::audit::steps(&bps);
    let bad = steps.iter().filter(|a| a.verdict.bad()).count();
    println!("seed {seed}: {} steps and hand-ins, {bad} with a problem", steps.len());
    print!("{}", truth_rows(&steps, all));
    let os = jane_bot::audit::omens(&bps);
    for o in &os {
        println!(
            "omen {:<18} {:<9} {} {} posted {}",
            o.id,
            o.region,
            if o.true_here { "TRUE " } else { "false" },
            if o.lethal { "lethal" } else { "      " },
            o.posted
                .iter()
                .map(|(z, c, w)| format!("{} {},{} ({w})", z.name(), c.0, c.1))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    for p in jane_bot::audit::omen_problems(&os) {
        println!("PROBLEM {p}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// `jane sweep` and `jane dossier`.

pub const USAGE: &str = "  sweep [--seeds A..B] [--models all|reader,lost,...] [--minutes M] [--short M] [--threads N]
        [--out DIR] [--traces DIR]
                                      every model on every seed, traced (VERIFICATION.md §3.2): L4 per run,
                                      L5 per seed, and a report ranking the quests the Lost could not find,
                                      the emptiest walks and the deaths (DIR/README.md, default sheets/sweep);
                                      --minutes: the story models' cap (default 2400, to the end); --short:
                                      the Explorer's and the pairs' (default 120)
  dossier <seed> [--models reader,lost] [--minutes M] [--out DIR]
                                      one seed's walkthrough (VERIFICATION.md L7): each model's timeline, its L4
                                      table, the seed's L5 audit and omens, and a few --snap pictures
  audit [--seed N] [--all]            L5 on the built world: every quest step's words against where its thing
                                      stands; the omens";

/// A step the Lost looked for on a seed: (seed, frames looking, gave up, frames from given to first on screen).
type Looked = (u32, u32, bool, Option<u32>);

/// Deaths to one killer in one place: how many, on which seeds, by which models.
type Killed = (u32, std::collections::BTreeSet<u32>, std::collections::BTreeSet<String>);

/// A step's verdict on a seed: (seed, the verdict, the words).
type Verdicted = (u32, String, String);

/// A seed's L5: its steps and its omens.
type SeedAudit = (u32, Vec<jane_bot::audit::StepAudit>, Vec<jane_bot::audit::OmenAudit>);

/// Every model the sweep knows, in its order.
pub const ALL_MODELS: [&str; 7] = ["reader", "rusher", "explorer", "cautious", "lost", "pair:together", "pair:split"];

/// What one run came to.
pub struct Run {
    pub seed: u32,
    pub model: String,
    pub trace: jane_sim::trace::Trace,
    /// Each seat's experience.
    pub xs: Vec<Experience>,
    pub log: Vec<String>,
    pub deaths: Vec<String>,
    pub the_end: u8,
    pub ms: u128,
    pub desyncs: usize,
    /// Pictures taken on the way (frame, png bytes, why).
    pub snaps: Vec<(u32, Vec<u8>, String)>,
}

/// The ending a seed's story run chooses: the three in turn (the story test's rule).
pub fn ending_for(seed: u32) -> jane_bot::Ending {
    [jane_bot::Ending::Hold, jane_bot::Ending::Hill, jane_bot::Ending::Train][(seed as usize + 2) % 3]
}

/// Play one model on one seed for at most `minutes`, traced. `snap_every`: frames between
/// pictures (0: none), and one whenever the Lost gives up looking.
pub fn run_one(seed: u32, model: &str, minutes: u32, snap_every: u32) -> Result<Run, String> {
    let t0 = std::time::Instant::now();
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    if let Some(mode) = jane_bot::pair::Mode::parse(model) {
        let r = crate::pair::play(&bps, mode, minutes);
        let xs = vec![jane_bot::experience::measure(&r.trace, 0), jane_bot::experience::measure(&r.trace, 1)];
        let log = r.logs[0]
            .iter()
            .map(|l| format!("seat 0 {l}"))
            .chain(r.logs[1].iter().map(|l| format!("seat 1 {l}")))
            .collect();
        return Ok(Run {
            seed,
            model: model.to_owned(),
            trace: r.trace,
            xs,
            log,
            deaths: Vec::new(),
            the_end: 0,
            ms: t0.elapsed().as_millis(),
            desyncs: r.desyncs,
            snaps: Vec::new(),
        });
    }
    let m = jane_bot::Model::parse(model).ok_or_else(|| format!("no model {model}"))?;
    let mut sim = jane_sim::Sim::new_game_with(bps, "Jane");
    let mut bot = jane_bot::Bot::story(m);
    bot.ctx.ending = Some(ending_for(seed));
    let mut sess = jane_bot::run::Session::new(bot, &sim, minutes);
    let mut snaps = Vec::new();
    let mut gave_ups = 0;
    for f in 0..minutes * 60 * 60 {
        if sess.bot.done() {
            break;
        }
        sess.step(&mut sim);
        let gave = sess
            .bot
            .log
            .iter()
            .filter(|l| matches!(&l.mark, jane_bot::Mark::Note(n) if n.starts_with("lost: gave up")))
            .count();
        let why = if gave > gave_ups {
            gave_ups = gave;
            sess.bot.log.last().map(jane_bot::Milestone::line)
        } else if snap_every > 0 && f > 0 && f % snap_every == 0 {
            Some(format!("{} min", f / MINUTE))
        } else {
            None
        };
        if let Some(why) = why {
            if let Some(v) = sim.view(jane_sim::Seat(0)) {
                snaps.push((f, crate::snap::snap(&v).png(), why));
            }
        }
    }
    let (bot, trace) = sess.finish(&sim);
    let the_end = sim.view(jane_sim::Seat(0)).map_or(0, |v| v.the_end());
    let xs = vec![jane_bot::experience::measure(&trace, 0)];
    Ok(Run {
        seed,
        model: model.to_owned(),
        trace,
        xs,
        log: bot.log.iter().map(jane_bot::Milestone::line).collect(),
        deaths: bot.deaths.iter().map(jane_bot::Death::line).collect(),
        the_end,
        ms: t0.elapsed().as_millis(),
        desyncs: 0,
        snaps,
    })
}

fn parse_seeds(s: &str) -> Result<Vec<u32>, String> {
    if let Some((a, b)) = s.split_once("..") {
        let a: u32 = a.parse().map_err(|_| format!("--seeds: {s}"))?;
        let b: u32 = b.parse().map_err(|_| format!("--seeds: {s}"))?;
        return Ok((a..=b).collect());
    }
    s.split(',').map(|x| x.parse().map_err(|_| format!("--seeds: {s}"))).collect()
}

fn flag<'a>(args: &'a [String], n: &str) -> Option<&'a str> {
    args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Run `jobs` on `threads` threads, in any order; results in job order.
fn run_all(jobs: &[(u32, String, u32)], threads: usize, snap_every: u32) -> Vec<Result<Run, String>> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<Result<Run, String>>>> =
        std::sync::Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((seed, model, minutes)) = jobs.get(i) else { break };
                    let r = run_one(*seed, model, *minutes, snap_every);
                    match &r {
                        Ok(r) => eprintln!(
                            "  seed {seed} {model}: {} min played, {} quests, {} deaths, {} ms",
                            Experience::min(r.xs[0].frames),
                            r.xs[0].quests.iter().filter(|q| q.done.is_some()).count(),
                            r.xs.iter().map(|x| x.deaths.len()).sum::<usize>(),
                            r.ms
                        ),
                        Err(e) => eprintln!("  seed {seed} {model}: {e}"),
                    }
                    out.lock().expect("results")[i] = Some(r);
                }
            });
        }
    });
    out.into_inner().expect("results").into_iter().map(|r| r.expect("every job ran")).collect()
}

/// `jane sweep ...`
pub fn sweep(args: &[String]) -> Result<(), String> {
    let seeds = parse_seeds(flag(args, "--seeds").unwrap_or("1..8"))?;
    let models: Vec<String> = match flag(args, "--models").unwrap_or("all") {
        "all" => ALL_MODELS.iter().map(|s| (*s).to_owned()).collect(),
        m => m.split(',').map(str::to_owned).collect(),
    };
    let long: u32 = flag(args, "--minutes").unwrap_or("2400").parse().map_err(|_| "--minutes: a number")?;
    let short: u32 = flag(args, "--short").unwrap_or("120").parse().map_err(|_| "--short: a number")?;
    let threads: usize = flag(args, "--threads").map_or_else(
        || std::thread::available_parallelism().map_or(4, std::num::NonZero::get).saturating_sub(2).max(1),
        |t| t.parse().unwrap_or(4),
    );
    let out = std::path::PathBuf::from(flag(args, "--out").unwrap_or("sheets/sweep"));
    let traces =
        flag(args, "--traces").map_or_else(|| std::path::PathBuf::from("target/sweep"), std::path::PathBuf::from);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::create_dir_all(&traces).map_err(|e| format!("{}: {e}", traces.display()))?;
    let mut jobs = Vec::new();
    // Longest first: the story models, then the short ones.
    for m in &models {
        for &s in &seeds {
            let minutes = if m == "explorer" || m.starts_with("pair") { short } else { long };
            jobs.push((s, m.clone(), minutes));
        }
    }
    jobs.sort_by_key(|(s, m, min)| (std::cmp::Reverse(*min), m.starts_with("pair"), m.clone(), *s));
    eprintln!("sweep: {} runs on {threads} threads", jobs.len());
    let t0 = std::time::Instant::now();
    let results = run_all(&jobs, threads, 0);
    let mut runs = Vec::new();
    for r in results {
        match r {
            Ok(r) => {
                let name = format!("{}-{}.jtr", r.seed, r.model.replace(':', "-"));
                std::fs::write(traces.join(&name), r.trace.encode()).map_err(|e| format!("{name}: {e}"))?;
                runs.push(r);
            }
            Err(e) => return Err(e),
        }
    }
    runs.sort_by(|a, b| (a.seed, model_ix(&a.model)).cmp(&(b.seed, model_ix(&b.model))));
    let audits: Vec<SeedAudit> = seeds
        .iter()
        .map(|&s| {
            let bps = jane_sim::Blueprints::build(s).expect("the seed builds");
            (s, jane_bot::audit::steps(&bps), jane_bot::audit::omens(&bps))
        })
        .collect();
    let report = report(&runs, &audits, t0.elapsed().as_secs(), long, short);
    let path = out.join("README.md");
    std::fs::write(&path, &report).map_err(|e| format!("{}: {e}", path.display()))?;
    println!(
        "sweep: {} runs in {} s; report {}; traces in {}",
        runs.len(),
        t0.elapsed().as_secs(),
        path.display(),
        traces.display()
    );
    Ok(())
}

fn model_ix(m: &str) -> usize {
    ALL_MODELS.iter().position(|x| *x == m).unwrap_or(99)
}

fn step_name(q: u16, i: u8) -> String {
    if i == 255 { format!("{} (back to)", quest_name(q)) } else { format!("{} step {}", quest_name(q), i + 1) }
}

fn step_words(q: u16, i: u8, seed: u32) -> String {
    let cat = jane_data::catalog();
    let def = cat.story.quest(jane_core::QuestId(q));
    let t = if i == 255 { cat.text(def.return_to) } else { cat.text(def.requirements[usize::from(i)].text) };
    jane_bot::audit::expand(t, seed)
}

/// The whole report, as Markdown.
#[allow(clippy::too_many_lines)]
pub fn report(runs: &[Run], audits: &[SeedAudit], secs: u64, long: u32, short: u32) -> String {
    let cat = jane_data::catalog();
    let m = Experience::min;
    let mut s = String::new();
    let seeds: Vec<u32> = audits.iter().map(|a| a.0).collect();
    let _ = writeln!(s, "# Jane: the sweep\n");
    let _ = writeln!(
        s,
        "Seeds {:?} x {} models, written by `jane sweep` (VERIFICATION.md §3.2) in {secs} s; content {:016x}. The story models (Reader, Rusher, Cautious, Lost) play from New Game to an ending or {long} minutes; the Explorer and the pairs {short} minutes. Times are real minutes of play (a night slept is not play). Traces are `target/sweep/<seed>-<model>.jtr`.\n",
        seeds,
        runs.iter().map(|r| r.model.as_str()).collect::<std::collections::BTreeSet<_>>().len(),
        cat.content_hash
    );
    let _ = writeln!(
        s,
        "How to read it: section 1 ranks what the owner asked about (confusing quests, empty walks, where it is hard); the tables after it are the evidence, per seed and model.\n"
    );

    // ---- 1a. The Lost: what it could not find.
    let _ = writeln!(s, "## 1. Findings\n");
    let _ = writeln!(s, "### 1a. Quests the Lost could not find from the words\n");
    let _ = writeln!(
        s,
        "The Lost (VERIFICATION.md L3) uses a quest's thing only once it has been on her screen; until then she looks from the words (a landmark they name, the roads, the map's edge), and after 20 minutes looking she is told where it is: that step failed \"from the text alone\". Ranked by seeds given up on, then minutes looking. *Audit* is L5's verdict for the step on that seed; *why* weighs the two.\n"
    );
    let mut by_step: BTreeMap<(u16, u8), Vec<Looked>> = BTreeMap::new();
    for r in runs.iter().filter(|r| r.model == "lost") {
        for st in &r.xs[0].steps {
            if st.searched > 0 || st.gave_up {
                let sight = st.sighted.map(|(f, _, _)| f.saturating_sub(st.given));
                by_step.entry((st.quest, st.step)).or_default().push((r.seed, st.searched, st.gave_up, sight));
            }
        }
    }
    let mut ranked: Vec<_> = by_step.into_iter().collect();
    ranked.sort_by_key(|((q, i), v)| {
        let gave = v.iter().filter(|x| x.2).count();
        let mins: u32 = v.iter().map(|x| x.1).sum();
        (std::cmp::Reverse(gave), std::cmp::Reverse(mins), *q, *i)
    });
    let _ = writeln!(
        s,
        "| # | Step | Words (seed of the first row) | Gave up on seeds | Looked, min (seed: min) | Audit | Why |"
    );
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- | --- |");
    for (n, ((q, i), v)) in ranked.iter().take(25).enumerate() {
        let gave: Vec<String> = v.iter().filter(|x| x.2).map(|x| x.0.to_string()).collect();
        let looked: Vec<String> = v.iter().map(|x| format!("{}: {}", x.0, m(x.1))).collect();
        let seed0 = v[0].0;
        let audit: Vec<String> = v
            .iter()
            .filter_map(|x| {
                let a = audits
                    .iter()
                    .find(|a| a.0 == x.0)?
                    .1
                    .iter()
                    .find(|a| cat.story.quest_id(a.quest).map(|qq| qq.0) == Some(*q) && a.step == *i)?;
                Some((x.0, a))
            })
            .map(|(sd, a)| format!("{sd}: {}", a.verdict.word()))
            .collect();
        let why = why(v, audits, *q, *i);
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} |",
            n + 1,
            step_name(*q, *i),
            step_words(*q, *i, seed0).replace('|', "/"),
            if gave.is_empty() { "-".into() } else { gave.join(", ") },
            looked.join("; "),
            audit.join("; ").replace('|', "/"),
            why
        );
    }
    if ranked.is_empty() {
        let _ = writeln!(s, "(The Lost looked for nothing: every step's thing was on screen before she needed it.)");
    }
    let _ = writeln!(s);

    // ---- 1b. Empty walks.
    let _ = writeln!(s, "### 1b. Where the world is empty\n");
    let _ = writeln!(
        s,
        "Runs of walking out of doors (not talking, not fighting) with nothing new on the 48 x 27 screen (no prop with a verb, no creature or person, no patch's edge not seen before) for 60 s or more (PLAN.md §2.4: something visible every 20 to 30 s, a deliberate empty stretch at most about 2 minutes). *Bare*: nothing at all on screen the whole way. Longest first, one per place (a 64-cell square), every model.\n"
    );
    let mut all: Vec<(&Run, &jane_bot::experience::Stretch, u8)> = Vec::new();
    for r in runs {
        for x in &r.xs {
            for st in &x.stretches {
                all.push((r, st, x.seat));
            }
        }
    }
    all.sort_by_key(|(r, st, _)| (std::cmp::Reverse(st.secs), r.seed, st.frame));
    let mut seen_places: std::collections::BTreeSet<(u32, i32, i32)> = std::collections::BTreeSet::new();
    let _ = writeln!(s, "| # | Seed | Model | At min | Secs | From | To | Region | Bare | Doing |");
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut n = 0;
    for (r, st, seat) in &all {
        let key = (r.seed, st.from.0 / 64, st.from.1 / 64);
        if !seen_places.insert(key) {
            continue;
        }
        n += 1;
        if n > 30 {
            break;
        }
        let _ = writeln!(
            s,
            "| {n} | {} | {}{} | {} | {} | {},{} | {},{} | {} | {} | {} |",
            r.seed,
            r.model,
            if r.model.starts_with("pair") { format!(" seat {seat}") } else { String::new() },
            m(st.frame),
            st.secs,
            st.from.0,
            st.from.1,
            st.to.0,
            st.to.1,
            REGIONS[usize::from(st.region).min(2)],
            if st.bare { "yes" } else { "" },
            st.doing
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Per seed and model: empty walks of 60 s or more, of 2 minutes or more, and the share of walking seconds spent in them.\n"
    );
    let _ = writeln!(s, "| Seed | {} |", ALL_MODELS.join(" | "));
    let _ = writeln!(s, "| --- |{}", " --- |".repeat(ALL_MODELS.len()));
    for &sd in &seeds {
        let mut row = format!("| {sd} |");
        for md in ALL_MODELS {
            match runs.iter().find(|r| r.seed == sd && r.model == md) {
                Some(r) => {
                    let x = &r.xs[0];
                    let secs: u32 = x.stretches.iter().map(|s| s.secs).sum();
                    let _ = write!(
                        row,
                        " {} / {} / {}% |",
                        x.stretches_over(60).count(),
                        x.stretches_over(120).count(),
                        secs * 100 / x.walk_samples.max(1)
                    );
                }
                None => row.push_str(" - |"),
            }
        }
        let _ = writeln!(s, "{row}");
    }
    let _ = writeln!(s);

    // ---- 1c. Difficulty.
    let _ = writeln!(s, "### 1c. Where it is hard\n");
    let _ = writeln!(
        s,
        "Deaths by what killed her and where, over every run (a death a real hour is the rate; the Cautious backs off at half her health, so a place that kills the Cautious too is a world spike, not a bot's mistake: VERIFICATION.md §8).\n"
    );
    let mut killers: BTreeMap<(String, String), Killed> = BTreeMap::new();
    for r in runs {
        for x in &r.xs {
            for d in &x.deaths {
                let zone = jane_core::ZoneId::ALL.get(usize::from(d.zone)).map_or("?", |z| z.name());
                let where_ =
                    if zone == "county" { REGIONS[usize::from(d.region).min(2)].to_owned() } else { zone.to_owned() };
                let e = killers.entry((unit_name(d.by).to_owned(), where_)).or_default();
                e.0 += 1;
                e.1.insert(r.seed);
                e.2.insert(r.model.clone());
            }
        }
    }
    let mut kr: Vec<_> = killers.into_iter().collect();
    kr.sort_by_key(|(k, v)| (std::cmp::Reverse(v.0), k.clone()));
    let _ = writeln!(s, "| # | Killer | Where | Deaths | Seeds | Models |");
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- |");
    for (n, ((who, whr), (c, sds, mds))) in kr.iter().take(20).enumerate() {
        let _ = writeln!(
            s,
            "| {} | {who} | {whr} | {c} | {} | {} |",
            n + 1,
            sds.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
            mds.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "Deaths by hour of play (all story models, every seed): the spikes.\n");
    let mut by_hour: std::collections::BTreeMap<(String, usize), u32> = std::collections::BTreeMap::new();
    let mut hours = 0usize;
    for r in runs.iter().filter(|r| !r.model.starts_with("pair") && r.model != "explorer") {
        for d in &r.xs[0].deaths {
            let h = (d.frame / (60 * MINUTE)) as usize;
            hours = hours.max(h + 1);
            *by_hour.entry((r.model.clone(), h)).or_default() += 1;
        }
    }
    let hours = hours.min(12);
    let _ = writeln!(s, "| Model | {} |", (0..hours).map(|h| format!("h{}", h + 1)).collect::<Vec<_>>().join(" | "));
    let _ = writeln!(s, "| --- |{}", " --- |".repeat(hours));
    for md in ["reader", "rusher", "cautious", "lost"] {
        let row: Vec<String> =
            (0..hours).map(|h| by_hour.get(&(md.to_owned(), h)).copied().unwrap_or(0).to_string()).collect();
        let _ = writeln!(s, "| {md} | {} |", row.join(" | "));
    }
    let _ = writeln!(s);

    // ---- 2. The runs.
    let _ = writeln!(s, "## 2. Every run\n");
    let _ = writeln!(
        s,
        "| Seed | Model | Ended | Played, min | Quests done | Deaths (an hour) | No objective, min | Looking, min | Backtrack | Empty walks >= 60 s | Walk:play | ms |"
    );
    let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for r in runs {
        for x in &r.xs {
            let _ = writeln!(
                s,
                "| {} | {}{} | {} | {} | {} | {} ({}.{}) | {} | {} | {}% | {} | {}.{:02} | {} |",
                r.seed,
                r.model,
                if r.xs.len() > 1 { format!(" seat {}", x.seat) } else { String::new() },
                ["-", "shield held", "Ball in the hill", "Sunday train"][usize::from(r.the_end.min(3))],
                m(x.frames),
                x.quests.iter().filter(|q| q.done.is_some()).count(),
                x.deaths.len(),
                x.deaths_per_hour_tenths() / 10,
                x.deaths_per_hour_tenths() % 10,
                m(x.idle_by_hour.iter().sum()),
                m(x.search_by_hour.iter().sum()),
                x.backtrack_cells * 100 / x.walked_cells.max(1),
                x.stretches_over(60).count(),
                x.walk_to_play() / 100,
                x.walk_to_play() % 100,
                r.ms
            );
        }
    }
    let _ = writeln!(s);

    // ---- 3. §4.1 bands.
    let _ = writeln!(s, "## 3. VERIFICATION.md §4.1, the first hour\n");
    let _ = writeln!(
        s,
        "Each row that gives a number, per seed (minutes; *never*: it did not happen in the run). Recorded, not enforced, except where `crates/jane-bot/tests/experience.rs` says so (§6: a band is enforced only after it is recorded on 64 seeds and read by the owner).\n"
    );
    let _ = writeln!(
        s,
        "| Claim | Model | Band | {} | In band |",
        seeds.iter().map(|s| format!("s{s}")).collect::<Vec<_>>().join(" | ")
    );
    let _ = writeln!(s, "| --- | --- | --- |{} --- |", " --- |".repeat(seeds.len()));
    for b in &FIRST_HOUR {
        for md in b.models {
            let mut ok = 0;
            let mut cells = Vec::new();
            for &sd in &seeds {
                match runs.iter().find(|r| r.seed == sd && r.model == *md) {
                    Some(r) => {
                        let v = first_hour_value(&r.xs[0], b);
                        if in_band(b, v) {
                            ok += 1;
                        }
                        cells.push(format!(
                            "{}{}",
                            v.map_or("never".into(), m),
                            if in_band(b, v) { "" } else { " **x**" }
                        ));
                    }
                    None => cells.push("-".into()),
                }
            }
            let _ = writeln!(
                s,
                "| {} | {md} | {}..{} | {} | {ok}/{} |",
                b.claim,
                b.lo.map_or(String::new(), m),
                b.hi.map_or(String::new(), m),
                cells.join(" | "),
                seeds.len()
            );
        }
    }
    let _ = writeln!(s);

    // ---- 4. L5.
    let _ = writeln!(s, "## 4. L5: the words against the world\n");
    let _ = writeln!(
        s,
        "Every quest step and every \"Back to ...\" whose words do not hold on a seed (`jane audit --seed N` prints one seed's; `--all` every row). FAR: what the words name is built, but not within the preposition's reach of the thing (by/at 10 cells, in/on 40, near 48, from/past/up 150, a named place's edge allowed 16). UNBUILT: nothing in the county is called that. OFF-ROAD: more than a half-screen from any road or path, and so is the landmark named. NOTHING: no instance on the seed.\n"
    );
    let mut rows: BTreeMap<(String, u8), Vec<Verdicted>> = BTreeMap::new();
    for (sd, steps, _) in audits {
        for a in steps.iter().filter(|a| a.verdict.bad()) {
            rows.entry((a.quest.to_owned(), a.step)).or_default().push((*sd, a.verdict.word(), a.text.clone()));
        }
    }
    let mut rows: Vec<_> = rows.into_iter().collect();
    rows.sort_by_key(|(k, v)| (std::cmp::Reverse(v.len()), k.clone()));
    let _ = writeln!(s, "| Step | Seeds | Words (first seed) | Verdict by seed |");
    let _ = writeln!(s, "| --- | --- | --- | --- |");
    for ((q, i), v) in &rows {
        let step = if *i == 255 { format!("{q} (back to)") } else { format!("{q} step {}", i + 1) };
        let _ = writeln!(
            s,
            "| {step} | {}/{} | {} | {} |",
            v.len(),
            seeds.len(),
            v[0].2.replace('|', "/"),
            v.iter().map(|(sd, w, _)| format!("{sd}: {w}")).collect::<Vec<_>>().join("; ").replace('|', "/")
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "### Omens (STORY.md §6)\n");
    let _ = writeln!(
        s,
        "True on each seed (marked only here, never in the game), and whether the claim is posted or said anywhere in the built world.\n"
    );
    let _ = writeln!(
        s,
        "| Omen | Region | Lethal | {} | Posted |",
        seeds.iter().map(|s| format!("s{s}")).collect::<Vec<_>>().join(" | ")
    );
    let _ = writeln!(s, "| --- | --- | --- |{} --- |", " --- |".repeat(seeds.len()));
    if let Some((_, _, first)) = audits.first() {
        for (oi, o) in first.iter().enumerate() {
            let cells: Vec<&str> = audits.iter().map(|(_, _, os)| if os[oi].true_here { "TRUE" } else { "" }).collect();
            let posted = audits.iter().filter(|(_, _, os)| !os[oi].posted.is_empty()).count();
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {posted}/{} seeds |",
                o.id,
                o.region,
                if o.lethal { "yes" } else { "" },
                cells.join(" | "),
                seeds.len()
            );
        }
    }
    for (sd, _, os) in audits {
        for p in jane_bot::audit::omen_problems(os) {
            let _ = writeln!(s, "- seed {sd}: {p}");
        }
    }
    let _ = writeln!(s);

    // ---- 5. Pairs.
    if runs.iter().any(|r| r.model.starts_with("pair")) {
        let _ = writeln!(s, "## 5. The Co-op pair\n");
        let _ = writeln!(
            s,
            "Two seats over lockstep (`jane-net`'s in-memory links, host and guest in one process). A death is *apart* when the other seat was in another zone or more than a screen away at the time.\n"
        );
        let _ = writeln!(s, "| Seed | Mode | Seat | Played, min | Quests done | Deaths | Apart | Desyncs |");
        let _ = writeln!(s, "| --- | --- | --- | --- | --- | --- | --- | --- |");
        for r in runs.iter().filter(|r| r.model.starts_with("pair")) {
            for x in &r.xs {
                let other = r.xs.iter().find(|o| o.seat != x.seat);
                let apart = other.map_or(0, |o| apart_deaths(&r.trace, x, o.seat));
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} | {} | {apart} | {} |",
                    r.seed,
                    r.model,
                    x.seat,
                    m(x.frames),
                    x.quests.iter().filter(|q| q.done.is_some()).count(),
                    x.deaths.len(),
                    r.desyncs
                );
            }
        }
        let _ = writeln!(s);
    }
    s
}

/// Why the Lost could not find a step, weighing the audit: the words (they name something not
/// built, or name it far off), the placement (off any road), or both; "the words hold" when the
/// audit found nothing wrong and she still did not find it (too little to go on, or too far).
fn why(v: &[Looked], audits: &[SeedAudit], q: u16, i: u8) -> String {
    let cat = jane_data::catalog();
    let mut text = 0;
    let mut place = 0;
    let mut far = Vec::new();
    for x in v {
        let Some(a) = audits
            .iter()
            .find(|a| a.0 == x.0)
            .and_then(|a| a.1.iter().find(|a| cat.story.quest_id(a.quest).map(|qq| qq.0) == Some(q) && a.step == i))
        else {
            continue;
        };
        match &a.verdict {
            jane_bot::audit::Verdict::Unbuilt(_) | jane_bot::audit::Verdict::Far(_) => text += 1,
            jane_bot::audit::Verdict::OffRoad(_) => place += 1,
            _ => {}
        }
        if let Some(d) = a.to_road.filter(|&d| d > jane_bot::audit::HALF_SCREEN) {
            place += 1;
            far.push(format!("{} cells off the road on seed {}", d, x.0));
        }
    }
    let base = match (text > 0, place > 0) {
        (true, true) => "both: the words and where it stands",
        (true, false) => "text: the words name what is not there",
        (false, true) => "placement: off the roads",
        (false, false) => "the words hold, but name too little to find it by",
    };
    if far.is_empty() { base.to_owned() } else { format!("{base} ({})", far.join(", ")) }
}

/// Deaths of `x`'s seat with the other seat in another zone or more than a screen away, by the
/// samples nearest before each.
fn apart_deaths(t: &jane_sim::trace::Trace, x: &Experience, other: u8) -> usize {
    use jane_sim::trace::Kind;
    x.deaths
        .iter()
        .filter(|d| {
            let o = t
                .records
                .iter()
                .filter(|r| r.seat == Some(other) && r.frame <= d.frame)
                .filter_map(|r| match &r.kind {
                    Kind::Sample(s) => Some(*s),
                    _ => None,
                })
                .next_back();
            o.is_none_or(|o| o.zone != d.zone || (o.cell.0 - d.cell.0).abs() > 24 || (o.cell.1 - d.cell.1).abs() > 13)
        })
        .count()
}

/// `jane dossier <seed> ...`: one seed's walkthrough.
pub fn dossier(args: &[String]) -> Result<(), String> {
    let seed: u32 = args.first().and_then(|a| a.parse().ok()).ok_or("dossier: which seed?")?;
    let models: Vec<String> = flag(args, "--models").unwrap_or("reader,lost").split(',').map(str::to_owned).collect();
    let minutes: u32 = flag(args, "--minutes").unwrap_or("240").parse().map_err(|_| "--minutes: a number")?;
    let out = std::path::PathBuf::from(flag(args, "--out").unwrap_or("sheets/dossier")).join(format!("seed-{seed}"));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let jobs: Vec<(u32, String, u32)> = models.iter().map(|m| (seed, m.clone(), minutes)).collect();
    let runs: Vec<Run> = run_all(&jobs, jobs.len(), 30 * MINUTE).into_iter().collect::<Result<_, _>>()?;
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let steps = jane_bot::audit::steps(&bps);
    let os = jane_bot::audit::omens(&bps);
    let mut s = String::new();
    let _ = writeln!(s, "# Seed {seed}: the dossier\n");
    let _ = writeln!(
        s,
        "Written by `jane dossier {seed}` (VERIFICATION.md L7): what each model did on this seed, minute by minute, what it was like in numbers, what the words claim against what was built, and which omens are true (marked only here). Pictures are the sim's view as a map (`--snap`), not the game's renderer.\n"
    );
    let _ = writeln!(s, "## Omens on this seed\n");
    for o in &os {
        let _ = writeln!(
            s,
            "- **{}** ({}{}): {}; claim {}",
            o.id,
            o.region,
            if o.lethal { ", lethal" } else { "" },
            if o.true_here { "TRUE" } else { "false" },
            if o.posted.is_empty() {
                "posted nowhere".to_owned()
            } else {
                format!(
                    "posted at {}",
                    o.posted
                        .iter()
                        .map(|(z, c, w)| format!("{} {},{} ({w})", z.name(), c.0, c.1))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            }
        );
    }
    let _ = writeln!(s);
    for r in &runs {
        let _ = writeln!(s, "## {}\n", r.model);
        let _ = writeln!(s, "```\n{}```\n", r.xs.iter().map(l4_table).collect::<String>());
        if !r.snaps.is_empty() {
            for (f, png, why) in &r.snaps {
                let name = format!("{}-{:06}.png", r.model.replace(':', "-"), f / 60);
                std::fs::write(out.join(&name), png).map_err(|e| format!("{name}: {e}"))?;
                let _ = writeln!(s, "![{why}]({name})\n\n*{} min: {why}*\n", f / MINUTE);
            }
        }
        let _ = writeln!(s, "### Timeline\n\n```");
        for l in r.log.iter().filter(|l| !l.contains(" killed ")) {
            let _ = writeln!(s, "{l}");
        }
        let _ = writeln!(s, "```\n");
        if !r.deaths.is_empty() {
            let _ = writeln!(s, "### Deaths\n\n```");
            for d in &r.deaths {
                let _ = writeln!(s, "{d}");
            }
            let _ = writeln!(s, "```\n");
        }
        if r.xs[0].steps.iter().any(|st| st.searched > 0) {
            let _ = writeln!(
                s,
                "### Looked for\n\n| Step | Words | Looked, min | First on screen, min after given | Gave up |\n| --- | --- | --- | --- | --- |"
            );
            for st in r.xs[0].steps.iter().filter(|st| st.searched > 0) {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} |",
                    step_name(st.quest, st.step),
                    step_words(st.quest, st.step, seed),
                    Experience::min(st.searched),
                    st.sighted.map_or("never".into(), |(f, _, _)| Experience::min(f.saturating_sub(st.given))),
                    if st.gave_up { "yes" } else { "" }
                );
            }
            let _ = writeln!(s);
        }
    }
    let _ = writeln!(s, "## L5 on this seed\n");
    s.push_str(&truth_rows(&steps, false));
    let path = out.join("README.md");
    std::fs::write(&path, &s).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("dossier: {}", path.display());
    Ok(())
}
