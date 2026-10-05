//! `jane bench --mem` (PLAY-PLAN.md §7): what the game holds in memory, by system. The heap is
//! counted by `cap`'s allocator (main.rs's `ALLOC`): each system is built in turn and its share is
//! what the heap grew by while it was built, so nothing is guessed. The presenter's biggest items
//! (the atlas, the chunk cache) are also reported by size, and the process's resident set (what
//! the OS holds, the GPU driver's own allocations included) is sampled beside.
//!
//! Two moments are recorded: at the Halt (New Game, a few seconds in) and after a long walk: a
//! tour of the county's ground and every other zone, drawn as it goes, with no bot (a bot's own
//! maps would be counted as the game's).

// Megabytes for a person to read: small numbers, exact enough in an f64.
#![allow(clippy::cast_precision_loss)]

use jane_core::ZoneId;
use jane_core::angle::Angle;
use jane_present::Present;
use jane_sim::input::DevOp;
use jane_sim::{Blueprints, Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

use crate::scene::{Bench, GlOpts, Hud, Which};

pub const USAGE: &str = "  bench --mem [--backend soft|gl2|wgpu] [--seed N] [--hops N] [--json]
                                      memory by system (PLAY-PLAN.md §7): heap per system as each is
                                      built, the atlas and the chunk cache by size, resident set and
                                      peak; at the Halt and after a tour of N hops (default 40)
";

const MB: f64 = 1024.0 * 1024.0;

fn heap() -> usize {
    crate::ALLOC.allocated()
}

fn heap_peak() -> usize {
    crate::ALLOC.max_allocated()
}

fn rss() -> usize {
    memory_stats::memory_stats().map_or(0, |m| m.physical_mem)
}

/// What was measured.
#[derive(Debug, Default)]
pub struct MemReport {
    pub backend: String,
    pub seed: u32,
    /// `(system, heap bytes)` as each was built.
    pub built: Vec<(String, usize)>,
    /// The heap's peak so far after each `took` stage (where the transients are).
    pub peaks: Vec<(String, usize)>,
    /// The presenter's sized items, at the Halt and after the walk.
    pub sized_halt: Vec<(String, usize)>,
    pub sized_walk: Vec<(String, usize)>,
    /// Worldgen's transient peak over what the blueprints keep.
    pub worldgen_scratch: usize,
    pub heap_halt: usize,
    pub heap_walk: usize,
    pub heap_peak: usize,
    pub rss_halt: usize,
    pub rss_walk: usize,
    pub rss_peak: usize,
    pub peak_by: String,
}

struct Rig {
    sim: Sim,
    present: Present,
    hud: Hud,
    bench: Bench,
    canvas: (u16, u16),
    seq: u16,
    rss_peak: usize,
    /// What last raised the heap's peak, and by how much over the heap it found.
    peak_by: String,
    /// `--verbose`: say each presenter tick that allocated over 4 MB.
    trace: bool,
}

impl Rig {
    fn step(&mut self, frame: InputFrame, cmds: &[Command], draw: bool) {
        let seat = Seat(0);
        let stamped: Vec<StampedCommand> = cmds
            .iter()
            .map(|c| {
                self.seq = self.seq.wrapping_add(1);
                StampedCommand { seat: Some(seat), seq: self.seq, cmd: *c }
            })
            .collect();
        let mut frames = [InputFrame::IDLE; 4];
        frames[0] = frame;
        let watch = |rig: &mut Rig, phase: &str, before: (usize, usize)| {
            let p = heap_peak();
            if p > before.1 {
                let zone = rig.sim.view(seat).map_or("-", |v| v.zone().name());
                rig.peak_by = format!(
                    "{phase} at tick {} in {zone}, {:.2} MB over the heap then",
                    rig.sim.state().tick.0,
                    (p - before.0) as f64 / MB
                );
            }
        };
        let before = (heap(), heap_peak());
        self.sim.step(&StepInput { frames, commands: &stamped });
        watch(self, "sim step", before);
        let events = self.sim.drain_events().to_vec();
        let before = (heap(), heap_peak());
        let churn = crate::ALLOC.total_allocated();
        if let Some(v) = self.sim.view(seat) {
            self.present.tick(&v, &events);
            self.hud.bufs.tick(&v, &events);
        }
        watch(self, "presenter tick", before);
        let churn = crate::ALLOC.total_allocated() - churn;
        if churn > 4 << 20 && self.trace {
            eprintln!("  presenter tick {} allocated {:.2} MB", self.sim.state().tick.0, churn as f64 / MB);
        }
        if draw {
            let before = (heap(), heap_peak());
            self.present.draw(255, self.canvas);
            self.hud.draw(&mut self.present, self.canvas);
            watch(self, "frame built", before);
            let before = (heap(), heap_peak());
            self.bench.backend().draw(self.present.frame());
            let _ = self.bench.finish();
            watch(self, "backend draw", before);
            if self.trace && heap() > before.0 + (4 << 20) {
                let f = self.present.frame();
                eprintln!(
                    "  backend draw at tick {} kept {:.2} MB more ({} sprites, {} casters, {} lights)",
                    self.sim.state().tick.0,
                    (heap() - before.0) as f64 / MB,
                    f.sprites.len(),
                    f.casters.len(),
                    f.lights.len()
                );
            }
        }
        if self.sim.state().tick.0 % 300 == 0 {
            self.rss_peak = self.rss_peak.max(rss());
        }
    }
}

fn took(r: &mut MemReport, mark: &mut usize, name: &str) {
    let now = heap();
    r.built.push((name.into(), now.saturating_sub(*mark)));
    r.peaks.push((name.into(), heap_peak()));
    *mark = now;
}

/// Measures `backend` on `seed`, touring `hops` places after the Halt.
pub fn measure(backend: Which, seed: u32, hops: u32, verbose: bool) -> Result<MemReport, String> {
    let mut r = MemReport { backend: backend.name().into(), seed, ..MemReport::default() };
    let mut rss_peak = rss();
    let mut mark = heap();

    // The blueprints, zone by zone (worldgen's scratch is freed as each finishes).
    let before = heap();
    let mut zones = Vec::with_capacity(jane_core::ZONE_COUNT);
    let mut kept = before;
    let mut peak = 0;
    for z in ZoneId::ALL {
        let bp = jane_sim::blueprints::build_one(z, seed).map_err(|e| format!("seed {seed}: {e}"))?;
        peak = peak.max(heap_peak().saturating_sub(kept));
        zones.push(std::sync::Arc::new(bp));
        kept = heap();
        if z == ZoneId::County {
            took(&mut r, &mut mark, "blueprint: county");
        }
    }
    r.worldgen_scratch = peak;
    took(&mut r, &mut mark, "blueprints: the other 12");
    let bps = Blueprints::from_parts(seed, zones.try_into().unwrap_or_else(|_| unreachable!("thirteen zones")));
    rss_peak = rss_peak.max(rss());

    let mut sim = Sim::new_game_with(bps, "Jane");
    took(&mut r, &mut mark, "sim state and runtime grids");
    let canvas = (768, 432);
    let mut present = Present::new(backend.tier());
    present.set_canvas(canvas);
    let sized = present.mem();
    let sized_sum: usize = sized.iter().map(|s| s.1).sum();
    let p = heap() - mark;
    for (n, b) in &sized {
        r.built.push(((*n).into(), *b));
    }
    r.built.push(("presenter: the rest (looks, tables)".into(), p.saturating_sub(sized_sum)));
    mark = heap();
    let hud = Hud::new(&present);
    took(&mut r, &mut mark, "ui");
    let rss0 = rss();
    let mut bench = Bench::new(backend, (1536, 864), GlOpts::default())?;
    bench.backend().upload_atlas(present.atlas());
    took(&mut r, &mut mark, &format!("renderer {} (heap)", backend.name()));
    r.built.push((format!("renderer {} (resident, driver incl.)", backend.name()), rss().saturating_sub(rss0)));
    // The backend has the px: the presenter lets go of its own (soft and gl2 share the albedo).
    present.release_atlas();
    r.built.push(("atlas px the presenter let go (freed)".into(), mark.saturating_sub(heap())));
    mark = heap();
    let mut engine = jane_audio::Engine::new(jane_audio::library(), 48000.0, seed);
    let mut buf = vec![0.0f32; 2048];
    engine.render(&mut buf);
    took(&mut r, &mut mark, "audio engine");
    rss_peak = rss_peak.max(rss());
    drop(buf);

    // At the Halt: New Game, a few seconds in, drawn.
    let god = Command::Dev(DevOp::God(true));
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &[] });
    let mut rig = Rig { sim, present, hud, bench, canvas, seq: 0, rss_peak, peak_by: String::new(), trace: verbose };
    rig.step(InputFrame::IDLE, &[god], true);
    for _ in 0..240 {
        rig.step(InputFrame::IDLE, &[], true);
    }
    r.heap_halt = heap();
    r.rss_halt = rss();
    r.peaks.push(("at the Halt".into(), heap_peak()));
    r.sized_halt = rig.present.mem().into_iter().chain(rig.sim.mem()).map(|(n, b)| (n.into(), b)).collect();

    // The walk: hops across the county, and into every other zone in turn.
    let county = rig.sim.blueprint(ZoneId::County).clone();
    let (w, h) = (county.w() as i32, county.h() as i32);
    let mut lcg = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut next = |n: i32| {
        lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((lcg >> 8) % n.max(1) as u32) as i32
    };
    for hop in 0..hops {
        let (zone, at) = if hop % 4 == 3 {
            (ZoneId::ALL[1 + (hop as usize / 4) % (jane_core::ZONE_COUNT - 1)], None)
        } else {
            (ZoneId::County, Some(jane_core::num::Vec2::centre(16 + next(w - 32), 16 + next(h - 32))))
        };
        let mark = crate::scene::mark_in(&rig.sim, zone, None)?;
        rig.sim.state_mut().players[0].travel = Some(jane_sim::state::TravelRequest { zone, mark, at });
        let dir = Angle((hop as u16).wrapping_mul(22_000));
        for k in 0..600u32 {
            let d = if k < 300 { dir } else { dir.wrapping_add(16384) };
            rig.step(InputFrame::walk(d), &[], k % 2 == 0);
        }
        if verbose {
            let p = rig.present.mem();
            eprintln!(
                "  hop {hop:>2} {:<10} heap {:>7.2} MB, chunk layers {:>6.2} MB",
                zone.name(),
                heap() as f64 / MB,
                p.iter().find(|r| r.0 == "terrain chunk cache").map_or(0, |r| r.1) as f64 / MB
            );
        }
    }
    r.heap_walk = heap();
    r.rss_walk = rss();
    r.sized_walk = rig.present.mem().into_iter().chain(rig.sim.mem()).map(|(n, b)| (n.into(), b)).collect();
    r.heap_peak = heap_peak();
    r.peak_by.clone_from(&rig.peak_by);
    r.rss_peak = rig.rss_peak.max(r.rss_walk).max(r.rss_halt);
    Ok(r)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let num = |name: &str, d: u32| {
        flag(name).map_or(Ok(d), |s| s.parse::<u32>().map_err(|_| format!("{name}: not a number: {s}")))
    };
    let backend = Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft, gl2 or wgpu")?;
    let r = measure(backend, num("--seed", 1)?, num("--hops", 40)?, args.iter().any(|a| a == "--verbose"))?;
    let mb = |b: usize| b as f64 / MB;
    if args.iter().any(|a| a == "--json") {
        let rows = |v: &[(String, usize)]| {
            serde_json::Value::Object(v.iter().map(|(n, b)| (n.clone(), serde_json::json!(b))).collect())
        };
        let j = serde_json::json!({
            "backend": r.backend, "seed": r.seed,
            "built": rows(&r.built), "sized_halt": rows(&r.sized_halt), "sized_walk": rows(&r.sized_walk),
            "worldgen_scratch": r.worldgen_scratch,
            "heap_halt": r.heap_halt, "heap_walk": r.heap_walk, "heap_peak": r.heap_peak,
            "peak_by": r.peak_by, "rss_halt": r.rss_halt, "rss_walk": r.rss_walk, "rss_peak": r.rss_peak,
        });
        println!("{j}");
        return Ok(());
    }
    println!("jane bench --mem: {} on seed {} (MB)", r.backend, r.seed);
    println!("  heap, as each system was built:");
    for (n, b) in &r.built {
        println!("    {n:<44} {:>8.2}", mb(*b));
    }
    println!("    {:<44} {:>8.2}  (transient, freed)", "worldgen scratch peak", mb(r.worldgen_scratch));
    println!("  heap peak so far, after:");
    for (n, b) in &r.peaks {
        println!("    {n:<44} {:>8.2}", mb(*b));
    }
    println!("  sized items                                   at Halt   after walk");
    for ((n, a), (_, b)) in r.sized_halt.iter().zip(&r.sized_walk) {
        println!("    {n:<42} {:>8.2} {:>10.2}", mb(*a), mb(*b));
    }
    println!(
        "  heap total                                 {:>10.2} {:>10.2}   peak {:.2}",
        mb(r.heap_halt),
        mb(r.heap_walk),
        mb(r.heap_peak)
    );
    println!(
        "  resident set (process)                     {:>10.2} {:>10.2}   peak {:.2}",
        mb(r.rss_halt),
        mb(r.rss_walk),
        mb(r.rss_peak)
    );
    println!("  the peak was set by: {}", r.peak_by);
    Ok(())
}
