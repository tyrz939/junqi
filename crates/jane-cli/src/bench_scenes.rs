//! `jane bench scenes` and `jane bench report`: the built-in benchmark (`jane_present::bench`) on a
//! PC, and any `bench.txt` (a PSP's or a PC's) as a table.
//!
//! The PC runs the PSP's seven scenes from the same words on a fresh game that is never saved: two
//! sim ticks a frame (a steady 30's), the presenter ticked after each, one frame drawn through the
//! backend at the PSP's canvas, the whole frame timed. A PC has no vblank, display list guard or
//! small heap, so those columns read `-` or 0; the frame rate is the uncapped one. A watchdog
//! thread writes `bench.txt` and `crash.txt` beside `--out` if a frame stalls past
//! [`jane_present::bench::HANG_US`], so a freeze leaves evidence here too.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jane_present::bench::{self as b, Plan, Sample, Stats, Steer};
use jane_present::pad_psp::psp;
use jane_present::{Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

use crate::scene::{Bench, GlOpts, Which};

pub const USAGE: &str = "  bench scenes [--backend soft|gl2|wgpu] [--only NAME,..] [--out FILE] [--words W] [--trace]
                                      the built-in benchmark's seven scenes (sprint, wood, town-night,
                                      cellar, mob, churn, lake) on a fresh, unsaved game, a line each to
                                      stdout and FILE (default bench.txt), as a PSP writes them;
                                      --words runs one scene of your own words, --trace says where
                                      she is every two seconds
  bench report [BASE] FILE [--profiles [--out DIR]]
                                      a bench.txt as a table; with BASE the deltas a scene and key;
                                      --profiles runs psp-profile on each capture-<scene>.bin beside FILE
                                      into DIR/<scene>/ (default progress/<date>_bench)";

/// The PSP's canvas: the frames are the console's size.
const CANVAS: (u16, u16) = (480, 272);

/// What the watchdog reads: frames made, the scene running, the lines so far.
#[derive(Default)]
struct Watch {
    beat: AtomicU32,
    done: AtomicBool,
    hangs: AtomicU32,
    text: Mutex<(String, String)>,
}

pub fn scenes(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let which = Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft, gl2 or wgpu")?;
    let out = std::path::PathBuf::from(flag("--out").unwrap_or("bench.txt"));
    let only: Vec<&str> = flag("--only").map(|s| s.split(',').collect()).unwrap_or_default();
    for o in &only {
        b::scene(o).ok_or_else(|| format!("--only: no scene {o}"))?;
    }
    let bps = jane_sim::Blueprints::build(b::SEED).map_err(|e| format!("seed {}: {e}", b::SEED))?;
    let mut sim = Sim::new_game_with(bps, "Jane");
    let tier: Tier = which.tier();
    let mut present = Present::new(tier);
    let mut backend = Bench::new(which, (u32::from(CANVAS.0) * 2, u32::from(CANVAS.1) * 2), GlOpts::parse(args)?)?;
    backend.backend().upload_atlas(present.atlas());
    let header = b::header(&format!("pc-{}", which.name()), &crate_commit(), "tpf 2 uncapped");
    println!("{header}");

    let watch = Arc::new(Watch::default());
    *watch.text.lock().map_err(|e| e.to_string())? = (String::new(), format!("{header}\n"));
    let dog = {
        let (watch, out) = (Arc::clone(&watch), out.clone());
        std::thread::spawn(move || watchdog(&watch, &out))
    };

    let mut seq = 0u16;
    let mut lines = vec![header];
    // `--words "<scene words>"`: one scene of one's own in place of the list (laying a scene out).
    let own =
        flag("--words").map(|w| b::Scene { name: "words", about: "", words: Box::leak(w.to_owned().into_boxed_str()) });
    let trace = args.iter().any(|a| a == "--trace");
    let list: Vec<b::Scene> = match own {
        Some(s) => vec![s],
        None => b::SCENES.iter().filter(|s| only.is_empty() || only.contains(&s.name)).copied().collect(),
    };
    for scene in &list {
        if let Ok(mut t) = watch.text.lock() {
            scene.name.clone_into(&mut t.0);
        }
        let plan = Plan::parse(scene.words);
        let mut stats = Stats::default();
        let mut steer = Steer::default();
        let hangs0 = watch.hangs.load(Ordering::Relaxed);
        let mut tick = 0u32;
        let mut held_was = 0u32;
        let mut hits = 0u32;
        // `--trace`: the worst ticks by sim time, with the path work each did.
        let mut worst: Vec<(u32, u32, u64, u64)> = Vec::new();
        while tick < plan.end {
            let t0 = Instant::now();
            for _ in 0..2 {
                if tick >= plan.end {
                    break;
                }
                let p0 = sim.path_stats();
                let t = Instant::now();
                step(&mut sim, &plan, &mut steer, tick, &mut held_was, &mut seq)?;
                let sim_us = t.elapsed().as_micros() as u32;
                stats.tick(sim_us);
                if trace {
                    let p = sim.path_stats();
                    worst.push((sim_us, tick, p.expanded - p0.expanded, p.searches - p0.searches));
                }
                let events = sim.drain_events().to_vec();
                hits += events.iter().filter(|e| matches!(e.kind, jane_sim::EventKind::Damage { .. })).count() as u32;
                let v = sim.view(Seat(0)).ok_or("seat 0 is not in the world")?;
                present.tick(&v, &events);
                tick += 1;
            }
            let frame = present.draw(0, CANVAS);
            let (casters, sprites) = (frame.casters.len() as u32, frame.sprites.len() as u32);
            backend.backend().draw(frame);
            backend.finish()?;
            let us = t0.elapsed().as_micros() as u32;
            stats.frame(&Sample { us, settling: tick <= plan.settle, casters, sprites, ..Sample::default() });
            watch.beat.fetch_add(1, Ordering::Relaxed);
            if trace && tick % 120 == 0 {
                let near = sim.view(Seat(0)).map_or(0, |v| {
                    let (x, y) = v.body().pos.cell();
                    v.units_in(jane_core::Rect::new(x - 6, y - 6, 13, 13)).count()
                });
                let talking = sim.view(Seat(0)).is_some_and(|v| v.me().dialogue.is_some());
                eprintln!(
                    "  {} tick {tick} at {} casters {casters} sprites {sprites} units_near {near} hits {hits}{}",
                    scene.name,
                    where_she_is(&sim),
                    if talking { " talking" } else { "" }
                );
            }
        }
        if trace {
            let nodes = worst.iter().map(|w| w.2).max().unwrap_or(0);
            let total: u64 = worst.iter().map(|w| w.2).sum();
            let heavy = worst.iter().filter(|w| w.2 >= 1024).count();
            eprintln!("  {} path nodes {total} in {} ticks, {heavy} ticks of 1024 or more", scene.name, worst.len());
            worst.sort_unstable_by(|a, b| b.cmp(a));
            eprintln!(
                "  {} path nodes worst tick {nodes}; slowest ticks (us, tick, nodes, searches): {:?}",
                scene.name,
                &worst[..worst.len().min(8)]
            );
        }
        stats.hangs = watch.hangs.load(Ordering::Relaxed) - hangs0;
        stats.at = where_she_is(&sim);
        let line = stats.line(scene.name);
        println!("{line}");
        if let Ok(mut t) = watch.text.lock() {
            t.1.push_str(&line);
            t.1.push('\n');
        }
        lines.push(line);
    }
    watch.done.store(true, Ordering::Relaxed);
    let _ = dog.join();
    let mut text = lines.join("\n");
    text.push('\n');
    std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

/// One tick of a scene: its travels, clock and commands, a route's click to move, the
/// `stick:`'s lean and the sprint, stepped.
fn step(
    sim: &mut Sim,
    plan: &Plan,
    steer: &mut Steer,
    tick: u32,
    held_was: &mut u32,
    seq: &mut u16,
) -> Result<(), String> {
    if tick == 0 {
        if let Some(c) = plan.clock {
            sim.state_mut().clock = c;
        }
    }
    if let Some(t) = plan.travel_at(tick) {
        let req = Plan::travel_request(sim, t).ok_or_else(|| format!("at: no mark {}", t.mark))?;
        sim.state_mut().players[0].travel = Some(req);
    }
    let mut cmds = plan.commands_at(tick);
    if let Some((zone, mark)) = plan.tp_at(tick) {
        let m = sim.view(Seat(0)).and_then(|v| v.sym(mark)).ok_or_else(|| format!("tp: no mark {mark}"))?;
        cmds.push(Command::Dev(DevOp::Tp { zone, mark: m }));
    }
    let held = plan.held(tick);
    // Bar 1 on each press of cross (the PSP's A); the target buttons are the PSP's alone.
    if held & !*held_was & psp::CROSS != 0 {
        cmds.push(Command::Bar { slot: 0, on: None });
    }
    *held_was = held;
    let her = sim.view(Seat(0)).map(|v| v.body().pos.cell());
    cmds.extend(her.and_then(|h| steer.goto(plan, tick, h)));
    let mut f = plan.stick(tick).map_or(InputFrame::IDLE, InputFrame::walk);
    f.sprint = held & psp::R != 0;
    let mut frames = [InputFrame::IDLE; jane_sim::tuning::MAX_PLAYERS];
    frames[0] = f;
    let commands: Vec<StampedCommand> = cmds
        .into_iter()
        .map(|cmd| {
            *seq = seq.wrapping_add(1);
            StampedCommand { seat: Some(Seat(0)), seq: *seq, cmd }
        })
        .collect();
    sim.step(&StepInput { frames, commands: &commands });
    Ok(())
}

fn where_she_is(sim: &Sim) -> String {
    sim.view(Seat(0))
        .and_then(|v| v.unit(v.me().unit).map(|u| (v.zone(), u.pos.cell())))
        .map_or_else(|| "-".into(), |(z, (x, y))| format!("{}:{x},{y}", z.name()))
}

/// The build's commit, as the PSP's header has it (`git rev-parse`, else `unknown`).
fn crate_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown".into(), |s| s.trim().to_owned())
}

/// The hang watchdog: no frame for [`b::HANG_US`] writes `bench.txt` (the lines so far and the
/// hang) and `crash.txt` beside `out`, once a stall.
fn watchdog(w: &Watch, out: &std::path::Path) {
    let (mut last, mut since, mut said) = (w.beat.load(Ordering::Relaxed), Instant::now(), false);
    while !w.done.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(250));
        let beat = w.beat.load(Ordering::Relaxed);
        if beat != last {
            (last, since, said) = (beat, Instant::now(), false);
            continue;
        }
        let ms = since.elapsed().as_millis() as u32;
        if !said && ms * 1000 >= b::HANG_US {
            said = true;
            w.hangs.fetch_add(1, Ordering::Relaxed);
            let (scene, text) = w.text.lock().map(|t| t.clone()).unwrap_or_default();
            let note = format!("hang scene {scene} stalled_ms {ms} frame {beat}\n");
            eprint!("{note}");
            let _ = std::fs::write(out, format!("{text}{note}"));
            let _ = std::fs::write(out.with_file_name("crash.txt"), note);
        }
    }
}

/// `jane bench report [BASE] FILE [--profiles [--out DIR]]`.
pub fn report(args: &[String]) -> Result<(), String> {
    let mut files = Vec::new();
    let mut profiles = false;
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--profiles" => profiles = true,
            "--out" => {
                i += 1;
                out = Some(args.get(i).map(String::as_str).ok_or("--out needs a directory")?);
            }
            a if a.starts_with('-') => return Err(format!("unknown argument \"{a}\"\n{USAGE}")),
            a => files.push(a),
        }
        i += 1;
    }
    if out.is_some() && !profiles {
        return Err("--out needs --profiles".into());
    }
    let (base, file) = match files.as_slice() {
        [file] => (None, *file),
        [base, file] => (Some(*base), *file),
        _ => return Err("bench report: which bench.txt?".into()),
    };
    let new_text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    print!("{}", table(&new_text));
    if let Some(base) = base {
        std::fs::metadata(base).map_err(|e| format!("{base}: {e}"))?;
        print!("{}", deltas(base, file));
    }
    if profiles {
        run_profiles(file, out)?;
    }
    Ok(())
}

/// The keys a delta table walks before the summary's own, in order.
const DELTA_KEYS: [&str; 11] = [
    "fps",
    "low1",
    "worst_ms",
    "sim_worst_ms",
    "missed",
    "swatch",
    "largest_kb",
    "casters",
    "sprites",
    "baked_wait",
    "refused",
];

/// `BASE` against `NEW`: one row a scene and key. Absent or `-` is `None`.
fn delta_rows(base: &str, new: &str) -> Vec<(String, String, Option<f64>, Option<f64>)> {
    let base_scenes = scenes_of(base);
    let new_scenes = scenes_of(new);
    let mut names: Vec<String> = new_scenes.iter().map(|s| s.0.clone()).collect();
    for (name, _) in &base_scenes {
        if !names.iter().any(|n| n == name) {
            names.push(name.clone());
        }
    }
    let mut keys: Vec<String> = DELTA_KEYS.iter().map(|k| (*k).to_owned()).collect();
    keys.extend(jane_render_psp::capture::summary::line_keys());
    let mut rows = Vec::new();
    for name in names {
        let b = base_scenes.iter().find(|s| s.0 == name);
        let n = new_scenes.iter().find(|s| s.0 == name);
        for key in &keys {
            let bv = b.and_then(|s| number(&s.1, key));
            let nv = n.and_then(|s| number(&s.1, key));
            if bv.is_none() && nv.is_none() {
                continue;
            }
            rows.push((name.clone(), key.clone(), bv, nv));
        }
    }
    rows
}

/// The two files compared: headers, then a scene's keys with base, new, delta and percent.
/// A scene in only one file shows `-` on the other side.
pub fn deltas(base: &str, new: &str) -> String {
    let base_text = std::fs::read_to_string(base).unwrap_or_default();
    let new_text = std::fs::read_to_string(new).unwrap_or_default();
    format_deltas(&base_text, &new_text)
}

fn format_deltas(base: &str, new: &str) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "base {}", header_line(base));
    let _ = writeln!(s, "new  {}", header_line(new));
    let rows = delta_rows(base, new);
    let base_scenes = scenes_of(base);
    let new_scenes = scenes_of(new);
    let mut at = 0;
    while at < rows.len() {
        let scene = rows[at].0.clone();
        let end = rows[at..].iter().position(|r| r.0 != scene).map_or(rows.len(), |n| at + n);
        let bp = base_scenes.iter().find(|sc| sc.0 == scene);
        let np = new_scenes.iter().find(|sc| sc.0 == scene);
        let mut cells =
            vec![["key".to_owned(), "base".to_owned(), "new".to_owned(), "delta".to_owned(), "pct".to_owned()]];
        for row in &rows[at..end] {
            let btxt = bp.map_or_else(|| "-".into(), |sc| raw(&sc.1, &row.1).to_owned());
            let ntxt = np.map_or_else(|| "-".into(), |sc| raw(&sc.1, &row.1).to_owned());
            let (d, p) = delta_text(row.2, row.3);
            cells.push([row.1.clone(), btxt, ntxt, d, p]);
        }
        at = end;
        let _ = writeln!(s, "scene {scene}");
        let widths: Vec<usize> = (0..5).map(|c| cells.iter().map(|r| r[c].len()).max().unwrap_or(0)).collect();
        for r in &cells {
            let _ = write!(s, "  {:<w$}", r[0], w = widths[0]);
            for c in 1..5 {
                let _ = write!(s, "  {:>w$}", r[c], w = widths[c]);
            }
            s.push('\n');
        }
    }
    s
}

fn header_line(text: &str) -> &str {
    text.lines().find(|l| l.starts_with("bench ")).unwrap_or("")
}

fn scenes_of(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.starts_with("scene ") {
            continue;
        }
        let f = b::fields(line);
        let name = f.iter().find(|p| p.0 == "scene").map(|p| p.1.to_owned()).unwrap_or_default();
        let pairs = f.into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
        out.push((name, pairs));
    }
    out
}

fn number(pairs: &[(String, String)], key: &str) -> Option<f64> {
    pairs.iter().find(|p| p.0 == key).map(|p| p.1.as_str()).filter(|v| *v != "-").and_then(|v| v.parse().ok())
}

fn raw<'a>(pairs: &'a [(String, String)], key: &str) -> &'a str {
    pairs.iter().find(|p| p.0 == key).map(|p| p.1.as_str()).filter(|v| *v != "-").unwrap_or("-")
}

fn delta_text(base: Option<f64>, new: Option<f64>) -> (String, String) {
    match (base, new) {
        // A zero base has no percent.
        (Some(b), Some(n)) if b != 0.0 => {
            let d = n - b;
            (format!("{d:+.1}"), format!("{:+.0}%", d / b * 100.0))
        }
        _ => ("-".into(), "-".into()),
    }
}

/// Days since 1970-01-01 to `YYYY-MM-DD` (Howard Hinnant's civil_from_days).
fn civil(days_since_epoch: i64) -> String {
    let z = days_since_epoch + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn today() -> String {
    let days =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0);
    civil(i64::try_from(days).unwrap_or(0))
}

/// `psp-profile` on each `capture-<scene>.bin` beside `file`. A missing file, a capture that
/// does not decode, or one with no frames is a note, and the report goes on.
fn run_profiles(file: &str, out: Option<&str>) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let bench = std::path::Path::new(file);
    let dir = bench.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new("."));
    let out_dir = match out {
        Some(d) => std::path::PathBuf::from(d),
        None => std::path::PathBuf::from(format!("progress/{}_bench", today())),
    };
    for line in text.lines() {
        if !line.starts_with("scene ") {
            continue;
        }
        let f = b::fields(line);
        let scene = f.iter().find(|p| p.0 == "scene").map_or("", |p| p.1);
        if scene.is_empty() {
            continue;
        }
        let cap = dir.join(format!("capture-{scene}.bin"));
        if !cap.is_file() {
            println!("profile {scene}: no capture");
            continue;
        }
        let dest = out_dir.join(scene);
        match crate::psp_profile::profile(&cap, Some(&dest)) {
            Ok((summary, html)) => {
                if let Err(e) = std::fs::write(dest.join("summary.txt"), summary) {
                    println!("profile {scene}: skipped (summary.txt: {e})");
                    continue;
                }
                println!("profile {scene} -> {}", html.display());
            }
            Err(e) => println!("profile {scene}: skipped ({e})"),
        }
    }
    Ok(())
}

/// A `bench.txt` as a table: the header's facts, then a row a scene, then any hang.
pub fn table(text: &str) -> String {
    use std::fmt::Write as _;
    const COLS: [(&str, &str); 13] = [
        ("scene", "scene"),
        ("fps", "fps"),
        ("low1", "1% low"),
        ("worst_ms", "worst ms"),
        ("missed", "missed"),
        ("flip_max", "flip"),
        ("swatch", "swatch"),
        ("free_kb", "free KB"),
        ("largest_kb", "block KB"),
        ("sheds", "sheds"),
        ("paint_fail", "paint!"),
        ("sim_worst_ms", "sim ms"),
        ("verdict", "verdict"),
    ];
    let mut s = String::new();
    let mut rows: Vec<Vec<String>> = vec![COLS.iter().map(|c| c.1.to_owned()).collect()];
    let mut notes = Vec::new();
    for line in text.lines() {
        if line.starts_with("bench ") {
            let _ = writeln!(s, "{line}");
        } else if line.starts_with("scene ") {
            let f = b::fields(line);
            let get = |k: &str| f.iter().find(|p| p.0 == k).map_or("-", |p| p.1).to_owned();
            rows.push(COLS.iter().map(|c| get(c.0)).collect());
            let why = get("why");
            if why != "-" {
                notes.push(format!("{}: {why} (at {})", get("scene"), get("at")));
            }
        } else if !line.trim().is_empty() {
            notes.push(line.to_owned());
        }
    }
    let widths: Vec<usize> = (0..COLS.len()).map(|c| rows.iter().map(|r| r[c].len()).max().unwrap_or(0)).collect();
    for r in &rows {
        for (c, cell) in r.iter().enumerate() {
            if c == 0 {
                let _ = write!(s, "{cell:<w$}", w = widths[c]);
            } else {
                let _ = write!(s, "  {cell:>w$}", w = widths[c]);
            }
        }
        s.push('\n');
    }
    for n in notes {
        let _ = writeln!(s, "  {n}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_tables_each_scene_and_says_why() {
        let t = table(
            "bench v1 platform psp commit abc seed 1 settings x\n\
             scene sprint fps 29.8 low1 21.0 worst_ms 84 missed 3 flip_max 3 swatch 12 frames 1800 free_kb 2100 largest_kb 1400 sheds 0 paint_fail 0 sim_worst_ms 5.4 hangs 0 casters 9 sprites 40 at county:760,1000 verdict WARN why low1<20 tick_ms 9.8 tick_p95 13.2 audio_pct 9.1 audio_p95 12.4\n\
             hang scene cellar stalled_ms 3250 frame 900\n",
        );
        assert!(t.contains("sprint") && t.contains("29.8") && t.contains("WARN"), "{t}");
        assert!(t.contains("sprint: low1<20 (at county:760,1000)"), "{t}");
        assert!(t.contains("hang scene cellar"), "{t}");
    }

    #[test]
    fn deltas_name_each_scene_and_key() {
        let base = "\
bench v1 platform psp commit 5371eef9b5
scene wood fps 32.7 low1 17.7 worst_ms 69
scene cellar fps 20.0 low1 11.0 worst_ms 80
";
        let new = "\
bench v1 platform psp commit abc1234 vfpu_pan on
scene wood fps 35.1 low1 19.0 worst_ms 60 tick_ms 9.8 tick_p95 13.2
scene lake fps 30.0 low1 20.0 worst_ms 40
";
        let rows = delta_rows(base, new);
        assert!(rows.contains(&("wood".into(), "fps".into(), Some(32.7), Some(35.1))));
        assert!(rows.contains(&("wood".into(), "tick_ms".into(), None, Some(9.8))));
        assert!(rows.iter().any(|r| r.0 == "cellar" && r.1 == "fps" && r.2 == Some(20.0) && r.3.is_none()));
        assert!(rows.iter().any(|r| r.0 == "lake" && r.1 == "fps" && r.2.is_none() && r.3 == Some(30.0)));
        let text = format_deltas(base, new);
        assert!(text.contains("+2.4"), "{text}");
        assert!(text.contains("scene cellar"), "{text}");
        assert!(text.contains("scene lake"), "{text}");
        assert!(!text.contains("not in new"), "{text}");
        assert!(text.starts_with("base bench v1 platform psp commit 5371eef9b5\nnew  bench v1"), "{text}");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_000), "2024-10-04");
        assert_eq!(civil(20_752), "2026-10-26");
    }
}
