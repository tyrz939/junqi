//! `jane bench scenes` and `jane bench report`: the built-in benchmark (`jane_present::bench`) on a
//! PC, and any `bench.txt` (a PSP's or a PC's) as a table.
//!
//! The PC runs the PSP's six scenes from the same words on a fresh game that is never saved: two
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
                                      the built-in benchmark's six scenes (sprint, wood, town-night,
                                      cellar, mob, churn) on a fresh, unsaved game, a line each to
                                      stdout and FILE (default bench.txt), as a PSP writes them;
                                      --words runs one scene of your own words, --trace says where
                                      she is every two seconds
  bench report FILE                   a bench.txt (a PSP's or a PC's) as a table";

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
        while tick < plan.end {
            let t0 = Instant::now();
            for _ in 0..2 {
                if tick >= plan.end {
                    break;
                }
                let t = Instant::now();
                step(&mut sim, &plan, &mut steer, tick, &mut held_was, &mut seq)?;
                let sim_us = t.elapsed().as_micros() as u32;
                stats.tick(sim_us);
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

/// `jane bench report FILE`.
pub fn report(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("bench report: which bench.txt?")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    print!("{}", table(&text));
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
             scene sprint fps 29.8 low1 21.0 worst_ms 84 missed 3 flip_max 3 swatch 12 frames 1800 free_kb 2100 largest_kb 1400 sheds 0 paint_fail 0 sim_worst_ms 5.4 hangs 0 casters 9 sprites 40 at county:760,1000 verdict WARN why low1<20\n\
             hang scene cellar stalled_ms 3250 frame 900\n",
        );
        assert!(t.contains("sprint") && t.contains("29.8") && t.contains("WARN"), "{t}");
        assert!(t.contains("sprint: low1<20 (at county:760,1000)"), "{t}");
        assert!(t.contains("hang scene cellar"), "{t}");
    }
}
