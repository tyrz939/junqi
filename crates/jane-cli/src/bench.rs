//! `jane bench gen [--zones <all|dungeons|id,id..>] [--seeds A..B | --seed N] [--json]`: time
//! worldgen stage by stage against PORT.md §9.4 (release, one thread), before anything is made
//! faster (PORT.md §6.j: measure first).
//!
//! What it times, per seed:
//!
//! - `skeleton`: the county's first valid skeleton, every attempt it took; `skeleton_attempt`:
//!   that valid attempt alone, built again from its own attempt number (the §9.4 row).
//! - `county.<stage>`: each of `county::STAGES` over skeleton attempt 0; `county.build` their sum,
//!   `county.solve` the solver's verdict on it, `county_build_solve` the two (the §9.4 row).
//! - `dungeon.<zone>`: the proven build as `jane_world::build_zone` makes it, every re-roll
//!   included, split into `.build` (the candidates), `.solve` (the traced base solve), `.walk` (the
//!   first completion C7 and C8 read) and `.C1` .. `.C12`, each summed over the attempts; `dungeon`
//!   pools every generated dungeon's total (the §9.4 row).
//! - `interior.<zone>`: a hand-built interior, proven.
//! - `new_game`: all thirteen zones through `build_zone`, one after another (the §9.4 row), only
//!   with `--zones all`.
//!
//! Each metric prints its sample count, median, p99 (nearest rank) and worst. `--json` prints the
//! same in the shape of `tools/perf/thresholds.json`: `gen` maps each gated metric to its p99 in
//! microseconds, `detail` has every metric. The table shows the gate from that file beside each
//! gated row.

use std::fmt::Write as _;
use std::time::Instant;

use jane_core::ZoneId;
use jane_world::county::{County, STAGES, county_skeleton};
use jane_world::dungeon::checks::{self, Ctx, ORDER, first_completion, rules_of};
use jane_world::skeleton::{SkeletonRows, build_skeleton};
use jane_world::solve::{Options, ZoneRules, solve_kept, validate};

use crate::gen_cmd;

pub const USAGE: &str = "  bench gen [--zones <all|dungeons|id,id..>] [--seeds A..B | --seed N] [--json]
                                      time worldgen stage by stage (skeleton, county stages, solver,
                                      each dungeon's build, solve and checks, New Game) against
                                      tools/perf/thresholds.json; --json prints that file's shape
  bench frames [--backend soft|gl2|wgpu] [--frames N] [--seed N] [--ticks T] [--hour H] [--wide]
               [--output WxH] [gl2: --es --shadows N|off --half-light|--full-light --fast|--exact --flat]
                                      play to a frame (default: the town at 22:00 after 600 ticks), then
                                      time N frames there (default 600): the Frame built, the backend's
                                      draw, the whole frame to the last pixel at the output size
                                      (wgpu and gl2: default 3840x2160, an offscreen 4K target), and
                                      the GPU's own clock (PRESENTATION.md §1.12); gl2's rows can be
                                      turned down to stand in for old hardware";

/// The rows PORT.md §9.4 gates, by metric name.
const GATED: [&str; 4] = ["skeleton_attempt", "county_build_solve", "dungeon", "new_game"];

/// Samples by metric, in the order first recorded.
#[derive(Default)]
struct Bench {
    rows: Vec<(String, Vec<u64>)>,
}

impl Bench {
    /// Record `ns` nanoseconds under `name`.
    fn add(&mut self, name: &str, ns: u64) {
        match self.rows.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => v.push(ns),
            None => self.rows.push((name.to_owned(), vec![ns])),
        }
    }
}

fn ns(t: Instant) -> u64 {
    t.elapsed().as_nanos() as u64
}

/// n, median, p99 (nearest rank), worst, mean; nanoseconds.
fn stats(v: &[u64]) -> (usize, u64, u64, u64, u64) {
    let mut s = v.to_vec();
    s.sort();
    let n = s.len();
    let rank = |p: usize| s[(n * p).div_ceil(100).clamp(1, n) - 1];
    (n, rank(50), rank(99), s[n - 1], s.iter().sum::<u64>() / n as u64)
}

fn ms(ns: u64) -> String {
    let us = ns / 1000;
    format!("{}.{:03}", us / 1000, us % 1000)
}

/// `"key": integer` pairs from the thresholds file: enough JSON for a flat file this tool writes.
fn read_gates(text: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(q) = rest.find('"') {
        rest = &rest[q + 1..];
        let Some(end) = rest.find('"') else { break };
        let key = &rest[..end];
        rest = &rest[end + 1..];
        let after = rest.trim_start();
        if let Some(v) = after.strip_prefix(':') {
            let v = v.trim_start();
            let digits: String = v.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse() {
                out.push((key.to_owned(), n));
            }
        }
    }
    out
}

fn skeleton(b: &mut Bench, seed: u32) -> Result<(), String> {
    let rows = SkeletonRows::catalog();
    let t = Instant::now();
    let sk = build_skeleton(seed, &rows, 0).map_err(|e| e.to_string())?;
    b.add("skeleton", ns(t));
    let t = Instant::now();
    let again = build_skeleton(seed, &rows, sk.attempt).map_err(|e| e.to_string())?;
    b.add("skeleton_attempt", ns(t));
    if again.attempt != sk.attempt {
        return Err(format!("seed {seed}: skeleton attempt {} did not hold alone", sk.attempt));
    }
    Ok(())
}

fn county(b: &mut Bench, seed: u32) -> Result<(), String> {
    let sk = county_skeleton(seed, 0).map_err(|e| e.to_string())?;
    let mut c = County::new(&sk, 0);
    let mut build = 0;
    for (name, stage) in STAGES {
        let t = Instant::now();
        stage(&mut c);
        let d = ns(t);
        build += d;
        b.add(&format!("county.{name}"), d);
    }
    let t = Instant::now();
    let bp = c.done();
    build += ns(t);
    b.add("county.build", build);
    let rules = ZoneRules::for_zone(ZoneId::County);
    let t = Instant::now();
    let report = validate(&bp, &rules);
    let solve = ns(t);
    b.add("county.solve", solve);
    b.add("county_build_solve", build + solve);
    if !report.ok() {
        eprintln!("jane bench: seed {seed}: the solver refused county attempt 0 (timed all the same)");
    }
    Ok(())
}

/// `dungeon::build`'s loop with `checks::check` inside it, timed piece by piece.
fn dungeon(b: &mut Bench, zone: ZoneId, seed: u32) {
    let name = zone.name();
    let mut parts: Vec<(String, u64)> = Vec::new();
    let mut part = |k: &str, d: u64| match parts.iter_mut().find(|(n, _)| n == k) {
        Some((_, v)) => *v += d,
        None => parts.push((k.to_owned(), d)),
    };
    let t_all = Instant::now();
    let mut attempt = 0u8;
    loop {
        let t = Instant::now();
        let built = jane_world::dungeon::build_candidate(zone, seed, attempt);
        part("build", ns(t));
        let ok = if built.info.errors.is_empty() && built.info.layout.is_some() {
            let (bp, info) = (&built.blueprint, &built.info);
            let rules = rules_of(info.mission);
            let opts = Options { trace: true, ..Options::default() };
            let t = Instant::now();
            let (base, trail) = solve_kept(bp, &rules, &opts);
            part("solve", ns(t));
            if base.ok() {
                let t = Instant::now();
                let walk = first_completion(bp, info);
                part("walk", ns(t));
                let c = Ctx { bp, info, m: info.mission, cat: jane_data::catalog(), rules, opts, base, trail, walk };
                let mut faults = 0;
                for check in ORDER {
                    let t = Instant::now();
                    faults += checks::run(check, &c).len();
                    part(check.name(), ns(t));
                }
                faults == 0
            } else {
                false
            }
        } else {
            false
        };
        if ok || attempt + 1 >= jane_core::blueprint::ZONE_ATTEMPTS {
            break;
        }
        attempt += 1;
    }
    let total = ns(t_all);
    b.add(&format!("dungeon.{name}"), total);
    b.add("dungeon", total);
    b.add(&format!("dungeon.{name}.attempts"), u64::from(attempt) + 1);
    for (k, d) in parts {
        b.add(&format!("dungeon.{name}.{k}"), d);
    }
}

pub const USAGE_TUNE: &str = "
  bench tune [--backend soft|gl2|wgpu] [--gate MS] [--frames N] [--output WxH] [--at ZONE[:MARK]]
             [--weather W] [--hour H] [--config PATH | --write [--data-dir DIR]]
                                      PRESENTATION.md §1.12's degrade ladder on this machine: time the
                                      frame (default: the town at 22:00) with the tier's rows, and while
                                      its p99 is over the tier's gate (T2 6 ms, T1 12, T0 25) turn the
                                      next row down and time it again; then write the rows that differ
                                      from the tier's own under present in config.json (--config PATH,
                                      or --write: the game's own, beside the exe when it is portable,
                                      else the user's data folder), or print them";

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("gen") => gen_bench(&args[1..]),
        Some("frames") => frames(&args[1..]),
        Some("tune") => tune(&args[1..]),
        _ => Err(format!("usage:\n{USAGE}{USAGE_TUNE}")),
    }
}

/// A step of the ladder: what it turns down, and how.
type Rung = (&'static str, fn(&mut jane_present::Features, jane_present::Tier));

/// The steps of §1.12's degrade ladder a tier can take, in order: each turns rows down from
/// those in force. Step 3 (`fog` to one layer) and step 7 (`half_res`) are not built, and
/// `frame_skip` (step 8), the last resort, is `tune`'s own last step.
fn ladder(tier: jane_present::Tier) -> Vec<Rung> {
    use jane_present::Tier;
    let mut steps: Vec<Rung> = Vec::new();
    // Every tier blooms since 2026-09-27, and T1 and T2 draw light shafts; T0's lit windows go
    // with its bloom (the glow is what the bloom is made of).
    if tier > Tier::T0 {
        steps.push(("god_rays and bloom off", |f, t| {
            f.set(t, "god_rays", "off");
            f.set(t, "bloom", "off");
        }));
    } else {
        steps.push(("bloom and glow off", |f, t| {
            f.set(t, "bloom", "off");
            f.set(t, "glow", "off");
        }));
    }
    steps.push(("max_lights halved, shadows to 4", |f, t| {
        let half = (f.max_lights / 2).max(1).to_string();
        f.set(t, "max_lights", &half);
        let four = f.shadows.min(4).to_string();
        f.set(t, "shadows", &four);
    }));
    if tier > Tier::T0 {
        steps.push(("shadows off", |f, t| {
            f.set(t, "shadows", "off");
        }));
    }
    steps.push(("max_particles halved", |f, t| {
        let half = (f.max_particles / 2).to_string();
        f.set(t, "max_particles", &half);
    }));
    if tier == Tier::T1 {
        steps.push(("normal_light off", |f, t| {
            f.set(t, "normal_light", "off");
        }));
    }
    steps
}

/// The game's `config.json` (jane-app's `Dirs::find`): `--data-dir`, else beside the exe when a
/// file called `portable` is there, else the user's data folder.
fn game_config(data_dir: Option<&str>) -> std::path::PathBuf {
    use std::path::{Path, PathBuf};
    if let Some(d) = data_dir {
        return PathBuf::from(d).join("config.json");
    }
    let beside = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    if let Some(b) = beside.as_ref().filter(|b| b.join("portable").exists()) {
        return b.join("config.json");
    }
    let home = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let root = if cfg!(windows) {
        home("APPDATA").map(|p| p.join("Jane"))
    } else if cfg!(target_os = "macos") {
        home("HOME").map(|p| p.join("Library/Application Support/Jane"))
    } else {
        home("XDG_DATA_HOME").map(|p| p.join("jane")).or_else(|| home("HOME").map(|p| p.join(".local/share/jane")))
    };
    root.or(beside).unwrap_or_else(|| PathBuf::from(".")).join("config.json")
}

/// `jane bench tune` (PRESENTATION.md §1.12): the degrade ladder, measured, and the rows it ends
/// on written as this machine's defaults.
fn tune(args: &[String]) -> Result<(), String> {
    use crate::scene::{Opts, Which, bench};
    use jane_present::{Features, Tier};
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let num = |name: &str, d: u32| {
        flag(name).map_or(Ok(d), |s| s.parse::<u32>().map_err(|_| format!("{name}: not a number: {s}")))
    };
    let backend = Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft, gl2 or wgpu")?;
    let tier = backend.tier();
    let gate_us = match flag("--gate") {
        Some(s) => (s.parse::<f64>().map_err(|_| format!("--gate: not a number of ms: {s}"))? * 1000.0) as u32,
        None => match tier {
            Tier::T2 => 6000,
            Tier::T1 => 12_000,
            Tier::T0 => 25_000,
        },
    };
    let output = match flag("--output") {
        Some(s) => {
            let (w, h) = s.split_once('x').ok_or("--output: WxH")?;
            (w.parse::<u32>().map_err(|_| "--output: WxH")?, h.parse::<u32>().map_err(|_| "--output: WxH")?)
        }
        None => (3840, 2160),
    };
    let seed = num("--seed", 1)?;
    let n = num("--frames", 300)?;
    let mut rows = Features::of(tier);
    let time = |rows: &Features| -> Result<u32, String> {
        let o = Opts {
            seed,
            ticks: 600,
            model: jane_bot::Model::Reader,
            hour: Some(u8::try_from(num("--hour", 22)? % 24).unwrap_or(22)),
            minute: 0,
            canvas: (768, 432),
            backend,
            at: flag("--at").map(|a| match a.split_once(':') {
                Some((z, m)) => (z.to_string(), Some(m.to_string())),
                None => (a.to_string(), None),
            }),
            weather: flag("--weather").map(crate::scene::weather).transpose()?,
            cast: None,
            spawn: None,
            rows: Features::KEYS.iter().filter_map(|k| rows.get(k).map(|v| ((*k).to_owned(), v))).collect(),
            gl: crate::scene::GlOpts::default(),
        };
        let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
        Ok(bench(bps, &o, n, output)?.whole.1)
    };
    let ms = |us: u32| f64::from(us) / 1000.0;
    println!("{}: the gate is p99 < {:.2} ms", backend.name(), ms(gate_us));
    let mut p99 = time(&rows)?;
    println!("  {:<32} p99 {:>6.2} ms", "the tier's own rows", ms(p99));
    for (what, step) in ladder(tier) {
        if p99 < gate_us {
            break;
        }
        let before = rows;
        step(&mut rows, tier);
        if rows == before {
            continue;
        }
        p99 = time(&rows)?;
        println!("  {what:<32} p99 {:>6.2} ms", ms(p99));
    }
    if p99 >= gate_us {
        rows.set(tier, "frame_skip", "on");
        println!("  still over: frame_skip on, the backend draws every other tick");
    }
    // The rows that differ from the tier's own, as config.json's `present` holds them.
    let own = Features::of(tier);
    let mut present = serde_json::Map::new();
    for k in Features::KEYS {
        if let Some(v) = rows.get(k).filter(|v| Some(v) != own.get(k).as_ref()) {
            let value = v.parse::<u64>().map_or(serde_json::Value::String(v), serde_json::Value::from);
            present.insert(k.to_owned(), value);
        }
    }
    let path = match (flag("--config"), args.iter().any(|a| a == "--write")) {
        (Some(p), _) => Some(std::path::PathBuf::from(p)),
        (None, true) => Some(game_config(flag("--data-dir"))),
        (None, false) => None,
    };
    let Some(path) = path else {
        println!("present: {}", serde_json::Value::Object(present));
        println!("(--config PATH or --write keeps these in config.json)");
        return Ok(());
    };
    // Laid into the file's `present`, the ladder's rows set or cleared, everything else kept.
    let mut doc: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(_) => serde_json::json!({}),
    };
    let obj = doc.as_object_mut().ok_or_else(|| format!("{}: not a JSON object", path.display()))?;
    let slot = obj.entry("present").or_insert_with(|| serde_json::json!({}));
    let into = slot.as_object_mut().ok_or_else(|| format!("{}: present is not an object", path.display()))?;
    for k in Features::KEYS {
        into.remove(k);
    }
    into.extend(present);
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("wrote present: {} to {}", doc["present"], path.display());
    Ok(())
}

fn gen_bench(args: &[String]) -> Result<(), String> {
    let which = args.iter().position(|a| a == "--zones").and_then(|i| args.get(i + 1)).map_or("all", String::as_str);
    let json = args.iter().any(|a| a == "--json");
    let zones = gen_cmd::zones(which)?;
    let range = if args.iter().any(|a| a == "--seeds" || a == "--seed") { gen_cmd::seeds(args)? } else { 1..=16 };
    // The catalog is built once, lazily: not a cost of any one zone.
    let _ = jane_data::catalog();
    let mut b = Bench::default();
    for seed in range.clone() {
        if zones.contains(&ZoneId::County) {
            skeleton(&mut b, seed)?;
            county(&mut b, seed)?;
        }
        for &z in &zones {
            if z == ZoneId::County {
                continue;
            }
            if jane_world::interiors::is_interior(z) {
                let t = Instant::now();
                let _ = jane_world::build_zone(z, seed);
                b.add(&format!("interior.{}", z.name()), ns(t));
            } else if jane_data::catalog().dungeons.mission_of(z).is_some() {
                dungeon(&mut b, z, seed);
            }
        }
        if which == "all" {
            let t = Instant::now();
            for z in ZoneId::ALL {
                let _ = jane_world::build_zone(z, seed);
            }
            b.add("new_game", ns(t));
        }
        if !json {
            eprint!("\rjane bench: seed {seed}");
        }
    }
    if !json {
        eprintln!();
    }
    let gates_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/perf/thresholds.json");
    let gates = std::fs::read_to_string(&gates_path).map(|t| read_gates(&t)).unwrap_or_default();
    let gate = |k: &str| gates.iter().find(|(n, _)| n == k).map(|&(_, v)| v);
    let source = format!("jane bench gen --zones {which} --seeds {}..{}", range.start(), range.end());
    if json {
        let mut s = String::new();
        let _ = writeln!(s, "{{\n  \"source\": \"{source}\",\n  \"class\": \"measured\",\n  \"unit\": \"us\",");
        let _ = writeln!(s, "  \"stat\": \"p99\",\n  \"gen\": {{");
        let gated: Vec<String> = GATED
            .iter()
            .filter_map(|&g| b.rows.iter().find(|(n, _)| n == g))
            .map(|(n, v)| format!("    \"{n}\": {}", stats(v).2 / 1000))
            .collect();
        let _ = writeln!(s, "{}\n  }},\n  \"detail\": {{", gated.join(",\n"));
        let detail: Vec<String> = b
            .rows
            .iter()
            .map(|(n, v)| {
                let (count, med, p99, max, mean) = stats(v);
                let us = |x: u64| if n.ends_with(".attempts") { x } else { x / 1000 };
                format!(
                    "    \"{n}\": {{ \"n\": {count}, \"median\": {}, \"p99\": {}, \"max\": {}, \"mean\": {} }}",
                    us(med),
                    us(p99),
                    us(max),
                    us(mean)
                )
            })
            .collect();
        let _ = writeln!(s, "{}\n  }}\n}}", detail.join(",\n"));
        print!("{s}");
        return Ok(());
    }
    println!("# {source} (ms; p99 is nearest rank, so the worst of fewer than 100)");
    println!("{:<34} {:>4} {:>10} {:>10} {:>10} {:>10}  gate", "metric", "n", "median", "p99", "max", "mean");
    for (n, v) in &b.rows {
        let (count, med, p99, max, mean) = stats(v);
        if n.ends_with(".attempts") {
            println!("{n:<34} {count:>4} {med:>10} {p99:>10} {max:>10} {mean:>10}");
            continue;
        }
        let verdict = gate(n)
            .map_or(String::new(), |g| format!("< {} ms {}", g / 1000, if p99 / 1000 < g { "met" } else { "MISSED" }));
        println!("{n:<34} {count:>4} {:>10} {:>10} {:>10} {:>10}  {verdict}", ms(med), ms(p99), ms(max), ms(mean));
    }
    Ok(())
}

/// `jane bench frames`.
fn frames(args: &[String]) -> Result<(), String> {
    use crate::scene::{Opts, Which, bench};
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let num = |name: &str, d: u32| {
        flag(name).map_or(Ok(d), |s| s.parse::<u32>().map_err(|_| format!("{name}: not a number: {s}")))
    };
    let backend = Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft, gl2 or wgpu")?;
    let seed = num("--seed", 1)?;
    let output = match flag("--output") {
        Some(s) => {
            let (w, h) = s.split_once('x').ok_or("--output: WxH")?;
            (w.parse::<u32>().map_err(|_| "--output: WxH")?, h.parse::<u32>().map_err(|_| "--output: WxH")?)
        }
        None => (3840, 2160),
    };
    let canvas = if args.iter().any(|a| a == "--wide") { (1008, 432) } else { (768, 432) };
    let o = Opts {
        seed,
        ticks: num("--ticks", 600)?,
        model: jane_bot::Model::Reader,
        hour: Some(u8::try_from(num("--hour", 22)? % 24).unwrap_or(22)),
        minute: 0,
        canvas,
        backend,
        at: flag("--at").map(|a| match a.split_once(':') {
            Some((z, m)) => (z.to_string(), Some(m.to_string())),
            None => (a.to_string(), None),
        }),
        weather: flag("--weather").map(crate::scene::weather).transpose()?,
        cast: flag("--cast").map(str::to_owned),
        spawn: flag("--spawn").map(str::to_owned),
        rows: crate::scene::rows(flag("--rows"))?,
        gl: crate::scene::GlOpts::parse(args)?,
    };
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let n = num("--frames", 600)?;
    let r = bench(bps, &o, n, output)?;
    let us =
        |(a, b): (u32, u32)| format!("p50 {:>6.2} ms  p99 {:>6.2} ms", f64::from(a) / 1000.0, f64::from(b) / 1000.0);
    println!(
        "{}: {} frames of {} x {} at {:02}:00, output {} x {}",
        r.line,
        r.frames,
        canvas.0,
        canvas.1,
        o.hour.unwrap_or(0),
        output.0,
        output.1
    );
    println!("  frame built (present.draw)   {}", us(r.build));
    println!("  backend draw (submitted)     {}", us(r.submit));
    println!("  whole frame, to last pixel   {}", us(r.whole));
    if let Some(s) = r.stats {
        println!(
            "  {} {}",
            if s.gpu_clock { "GPU clock, whole frame      " } else { "backend's own clock, whole  " },
            us((s.p50_us, s.p99_us))
        );
        for p in jane_present::StatPass::ALL {
            let i = p as usize;
            if s.pass_p99_us[i] > 0 {
                println!("    {:<26} {}", p.name(), us((s.pass_p50_us[i], s.pass_p99_us[i])));
            }
        }
        println!(
            "  last frame: {} draw calls, {} lights, {} casters, {} px written",
            s.draw_calls, s.lights, s.casters, s.pixels_written
        );
    }
    Ok(())
}
