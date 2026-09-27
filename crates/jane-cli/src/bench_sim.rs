//! `jane bench sim`: a player model plays a seed from New Game for a long session, and the time
//! goes into buckets of play minutes (3600 frames each), the sim's step, the bot's thinking and
//! the hash a tape takes every [`HASH_EVERY`] frames each apart, beside what might grow with the
//! game (units, props, the journal, path searches and their nodes, the bot's plans, events, live
//! runtimes). A late game slower than an early one shows here as a row, and which side of the
//! frame it is on as a column.
//!
//! The late rows are the last quarter of the frames played. `tools/perf/thresholds.json`'s `sim`
//! gates them: `late_tick_p50` and `late_tick_p99` are PORT.md §9.4's median and p99 tick for the
//! step alone, `late_frame_mean` the whole frame (step, bot and the tape's hash), so a player
//! model that slows down late (as the Reader did with nothing left to do) fails too. `--gate`
//! fails the command when a seed's late rows are over; `--json` prints the worst in that file's
//! shape.

use std::time::Instant;

use jane_bot::{Bot, Host, Model};
use jane_core::ZoneId;
use jane_sim::replay::HASH_EVERY;
use jane_sim::{Blueprints, Event, Seat, Sim, StepInput, Stepped, View};

pub const USAGE: &str = "
  bench sim [--model reader|rusher] [--seeds A,B,..] [--minutes M] [--bucket B] [--json] [--gate]
                                      a model plays each seed (default 7) M minutes (default 120) from New
                                      Game; per B-minute bucket (default 10): frames/s, the step's p50, p99,
                                      max and mean us, the bot's, the tape's hash, units, props, the journal,
                                      path searches, the bot's plans, events; the late rows (the last
                                      quarter) against thresholds.json's `sim` (--gate fails over them)";

/// The rows `thresholds.json`'s `sim` holds, in microseconds.
const GATED: [&str; 3] = ["late_tick_p50", "late_tick_p99", "late_frame_mean"];

/// A host that times its steps and counts what they emit.
struct Timed {
    sim: Sim,
    step_ns: u64,
    events: u64,
}

impl Host for Timed {
    fn view(&self, seat: Seat) -> Option<View<'_>> {
        self.sim.view(seat)
    }

    fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        let t = Instant::now();
        let out = self.sim.step(input);
        self.step_ns = t.elapsed().as_nanos() as u64;
        out
    }

    fn drain_events(&mut self) -> &[Event] {
        let ev = self.sim.drain_events();
        self.events += ev.len() as u64;
        ev
    }

    fn sim(&self) -> &Sim {
        &self.sim
    }
}

/// One bucket's samples (nanoseconds), what it did, and the counts at its end.
#[derive(Default)]
struct Bucket {
    step: Vec<u64>,
    bot: Vec<u64>,
    hash_ns: u64,
    hashes: u32,
    wall_ns: u64,
    ticks: u32,
    events: u64,
    searches: u64,
    expanded: u64,
    plans: u64,
    tick: u32,
    zone: Option<ZoneId>,
    awake: u32,
    units: u32,
    props: u32,
    journal: u32,
    live: u32,
    made: u32,
    done: u32,
}

/// The late rows of one session, microseconds.
#[derive(Clone, Copy, Default)]
struct Late {
    tick_p50: u64,
    tick_p99: u64,
    tick_max: u64,
    frame_mean: u64,
}

impl Late {
    fn row(&self, name: &str) -> u64 {
        match name {
            "late_tick_p50" => self.tick_p50,
            "late_tick_p99" => self.tick_p99,
            _ => self.frame_mean,
        }
    }
}

/// Nearest rank.
fn rank(sorted: &[u64], p: usize) -> u64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    sorted[(n * p).div_ceil(100).clamp(1, n) - 1]
}

fn close(b: &mut Bucket, sim: &Sim) {
    let s = sim.state();
    b.tick = s.tick.0;
    b.zone = s.players.first().map(|p| p.zone);
    for z in s.zones.iter().flatten() {
        b.made += 1;
        b.units += z.units.len() as u32;
        b.awake += z.units.iter().filter(|u| u.awake).count() as u32;
        b.props += z.props.len() as u32;
    }
    b.live = ZoneId::ALL.iter().filter(|&&z| sim.runtime(z).is_some()).count() as u32;
    b.journal = s.journal.entries.len() as u32;
    b.done = s.quests.done.len() as u32;
}

fn session(model: Model, seed: u32, minutes: u32, bucket_min: u32) -> Result<(Vec<Bucket>, Late), String> {
    let bps = Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let mut host = Timed { sim: Sim::new_game_with(bps, "Jane"), step_ns: 0, events: 0 };
    let mut bot = Bot::story(model);
    let per = bucket_min * 3600;
    let frames = minutes * 3600;
    let late_from = frames - frames / 4;
    let (mut late_steps, mut late_wall, mut late_frames) = (Vec::new(), 0u64, 0u64);
    let mut out = Vec::new();
    let mut b = Bucket::default();
    let mut last_paths = host.sim.path_stats();
    let mut last_plans = bot.ctx.nav.plans;
    let mut last_tick = 0;
    for f in 0..frames {
        if bot.done() {
            break;
        }
        let t = Instant::now();
        bot.step(&mut host);
        let mut wall = t.elapsed().as_nanos() as u64;
        b.step.push(host.step_ns);
        b.bot.push(wall.saturating_sub(host.step_ns));
        if (f + 1) % HASH_EVERY == 0 {
            // What a tape pays: `Recorder` hashes the state every `HASH_EVERY` ticks.
            let t = Instant::now();
            std::hint::black_box(host.sim.hash());
            let h = t.elapsed().as_nanos() as u64;
            b.hash_ns += h;
            b.hashes += 1;
            wall += h;
        }
        b.wall_ns += wall;
        if f >= late_from {
            late_steps.push(host.step_ns);
            late_wall += wall;
            late_frames += 1;
        }
        let last = (f + 1) % per == 0 || f + 1 == frames || bot.done();
        if last {
            let p = host.sim.path_stats();
            b.searches = p.searches - last_paths.searches;
            b.expanded = p.expanded - last_paths.expanded;
            last_paths = p;
            b.plans = bot.ctx.nav.plans - last_plans;
            last_plans = bot.ctx.nav.plans;
            b.events = std::mem::take(&mut host.events);
            let tick = host.sim.state().tick.0;
            b.ticks = tick - last_tick;
            last_tick = tick;
            close(&mut b, &host.sim);
            out.push(std::mem::take(&mut b));
        }
    }
    late_steps.sort();
    let late = Late {
        tick_p50: rank(&late_steps, 50) / 1000,
        tick_p99: rank(&late_steps, 99) / 1000,
        tick_max: late_steps.last().copied().unwrap_or(0) / 1000,
        frame_mean: late_wall / late_frames.max(1) / 1000,
    };
    eprintln!(
        "{} seed {seed}: {} frames, tick {}, {} quests done, bot {}",
        model.name(),
        host.sim.state().frame,
        host.sim.state().tick.0,
        host.sim.state().quests.done.len(),
        if bot.done() { "stopped" } else { "still going" }
    );
    Ok((out, late))
}

fn table(model: Model, seed: u32, bucket: u32, rows: &[Bucket], late: Late) {
    println!(
        "\n{} seed {seed}, {bucket}-minute buckets (us; frames/s over the step, the bot and the hash)",
        model.name()
    );
    println!(
        "{:>5} {:>8} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>5} {:>6} {:>5} {:>4} {:>6} {:>5} {:>6} {:>5} {:>4} {:>4} {:>7} {:>7}",
        "min",
        "frames/s",
        "st p50",
        "st p99",
        "st max",
        "st avg",
        "bt avg",
        "bt p99",
        "bt max",
        "hash",
        "awake",
        "units",
        "props",
        "jrnl",
        "done",
        "srch/t",
        "nodes",
        "plans",
        "ev/t",
        "live",
        "made",
        "tick",
        "zone"
    );
    for (i, b) in rows.iter().enumerate() {
        let n = b.step.len().max(1) as u64;
        let (mut st, mut bt) = (b.step.clone(), b.bot.clone());
        st.sort();
        bt.sort();
        let ticks = u64::from(b.ticks.max(1));
        println!(
            "{:>5} {:>8} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>5} {:>6} {:>5} {:>4} {:>6} {:>5} {:>6} {:>5} {:>4} {:>4} {:>7} {:>7}",
            (i as u32 + 1) * bucket,
            b.step.len() as u64 * 1_000_000_000 / b.wall_ns.max(1),
            rank(&st, 50) / 1000,
            rank(&st, 99) / 1000,
            st.last().copied().unwrap_or(0) / 1000,
            st.iter().sum::<u64>() / n / 1000,
            bt.iter().sum::<u64>() / n / 1000,
            rank(&bt, 99) / 1000,
            bt.last().copied().unwrap_or(0) / 1000,
            b.hash_ns / u64::from(b.hashes.max(1)) / 1000,
            b.awake,
            b.units,
            b.props,
            b.journal,
            b.done,
            format!("{}.{:02}", b.searches / ticks, b.searches * 100 / ticks % 100),
            b.expanded / b.searches.max(1),
            b.plans,
            format!("{}.{:01}", b.events / ticks, b.events * 10 / ticks % 10),
            b.live,
            b.made,
            b.tick,
            b.zone.map_or("-", ZoneId::name),
        );
    }
    println!(
        "late (the last quarter): step p50 {} us, p99 {} us, max {} us; frame mean {} us",
        late.tick_p50, late.tick_p99, late.tick_max, late.frame_mean
    );
}

pub fn run(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let model = Model::parse(flag("--model").unwrap_or("reader")).ok_or("--model: reader or rusher")?;
    let seeds: Vec<u32> = flag("--seeds")
        .unwrap_or("7")
        .split(',')
        .map(|s| s.trim().parse().map_err(|_| format!("--seeds: not a number: {s}")))
        .collect::<Result<_, _>>()?;
    let minutes: u32 = flag("--minutes").unwrap_or("120").parse().map_err(|_| "--minutes: not a number")?;
    let bucket: u32 = flag("--bucket").unwrap_or("10").parse().map_err(|_| "--bucket: not a number")?;
    let (json, gate) = (args.iter().any(|a| a == "--json"), args.iter().any(|a| a == "--gate"));
    if minutes == 0 {
        return Err("--minutes: at least 1".into());
    }
    let gates_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/perf/thresholds.json");
    let gates = std::fs::read_to_string(&gates_path).map(|t| crate::bench::read_gates(&t)).unwrap_or_default();
    let gate_of = |name: &str| gates.iter().find(|(k, _)| k == name).map(|&(_, v)| v);
    let mut worst = Late::default();
    let mut over = Vec::new();
    for &seed in &seeds {
        let (rows, late) = session(model, seed, minutes, bucket.max(1))?;
        worst.tick_p50 = worst.tick_p50.max(late.tick_p50);
        worst.tick_p99 = worst.tick_p99.max(late.tick_p99);
        worst.tick_max = worst.tick_max.max(late.tick_max);
        worst.frame_mean = worst.frame_mean.max(late.frame_mean);
        for name in GATED {
            if let Some(g) = gate_of(name).filter(|&g| late.row(name) > g) {
                over.push(format!("seed {seed}: {name} {} us over {g} us", late.row(name)));
            }
        }
        if !json {
            table(model, seed, bucket.max(1), &rows, late);
        }
    }
    if json {
        println!(
            "{{\n  \"sim\": {{\n    \"late_tick_p50\": {},\n    \"late_tick_p99\": {},\n    \"late_frame_mean\": {}\n  }}\n}}",
            worst.tick_p50, worst.tick_p99, worst.frame_mean
        );
    } else {
        for name in GATED {
            let g = gate_of(name).map_or_else(|| "no gate".to_owned(), |g| format!("gate {g}"));
            println!("{name}: {} us ({g})", worst.row(name));
        }
    }
    if gate && !over.is_empty() {
        return Err(format!("over thresholds.json's sim rows: {}", over.join("; ")));
    }
    Ok(())
}
