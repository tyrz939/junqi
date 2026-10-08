//! `jane bench heap` (PORT.md §13.2, §13.3): the sim's and worldgen's heap, measured, for the
//! memory diet. Bytes are *requested* bytes as `cap`'s allocator counts them (main.rs's `ALLOC`),
//! the same measure as the PSP spike's (§13.10), so the numbers compare.
//!
//! Three tables:
//!
//! - **Stages.** Every county stage (and each other zone, and the solver) as `Blueprints::build`
//!   runs them: live bytes when it ends and the peak while it ran. `cap` cannot reset its peak, so
//!   each stage starts with a *ballast* allocation that lifts the live count to the peak so far
//!   (never touched, so the OS commits nothing): whatever the stage then adds raises the peak by
//!   exactly its own high-water mark, which is read back and the ballast freed.
//! - **Blueprints.** Each finished blueprint, retained (what the heap grew by while it was built)
//!   and by field, each field sized by cloning it alone (a clone is exact-capacity, so retained
//!   minus the fields' sum is slack: spare `Vec` capacity, map tables).
//! - **Sim.** `Sim::new_game_with` over the blueprints, then idle ticks: resident and peak.
//!
//! Megabytes are for reading; `--json` gives exact bytes for the CI gate (`tests/heap_budget.rs`).

// Megabytes for a person to read: small numbers, exact enough in an f64.
#![allow(clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::sync::Arc;

use jane_core::{Blueprint, ZoneId};
use jane_sim::{Blueprints, InputFrame, Sim, StepInput};

pub const USAGE: &str = "  bench heap [--seed N] [--ticks N] [--json]
                                      worldgen's and the sim's heap (PORT.md §13.3): live and peak per
                                      county stage and zone, each blueprint by field, the sim resident
                                      after New Game and N idle ticks (default 600)
";

/// Decimal megabytes, as PORT.md §13.2 and §13.10 count them.
const MB: f64 = 1_000_000.0;

fn live() -> usize {
    crate::ALLOC.allocated()
}

fn peak() -> usize {
    crate::ALLOC.max_allocated()
}

/// A ballast that lifts the live count to the peak so far, so the next region's own high-water
/// mark is `peak() - top` on top of the live bytes it started from.
struct Window {
    ballast: Vec<u8>,
    start: usize,
    top: usize,
}

impl Window {
    fn open() -> Self {
        let start = live();
        let ballast = Vec::with_capacity(peak().saturating_sub(start));
        Self { ballast, start, top: peak() }
    }

    /// Live bytes now, the ballast not counted.
    fn live(&self) -> usize {
        live() - self.ballast.capacity()
    }

    /// `(live now, peak while open)`, ballast freed.
    fn close(self) -> (usize, usize) {
        let rise = peak().saturating_sub(self.top);
        drop(self.ballast);
        (live(), self.start + rise)
    }
}

/// One row of the stage table.
#[derive(Debug, Clone)]
pub struct StageRow {
    pub name: &'static str,
    pub live: usize,
    pub peak: usize,
}

/// One blueprint's sizes.
#[derive(Debug, Clone)]
pub struct BpRow {
    pub zone: &'static str,
    pub retained: usize,
    pub fields: Vec<(&'static str, usize)>,
}

#[derive(Debug, Default)]
pub struct HeapReport {
    pub seed: u32,
    pub stages: Vec<StageRow>,
    /// The highest peak over the whole build.
    pub build_peak: usize,
    /// Live once all thirteen are built (the blueprints and whatever else stayed).
    pub build_resident: usize,
    pub blueprints: Vec<BpRow>,
    pub sim_new_game: usize,
    /// The sim's own heap at New Game by part (each sized by a lone clone): the rest of
    /// `sim_new_game` past these and the blueprints is the runtimes' lookups and buckets.
    pub sim_parts: Vec<(&'static str, usize)>,
    pub sim_resident: usize,
    pub sim_peak: usize,
    pub ticks: u32,
    /// The console form (`Blueprints::packed`, PORT.md §13.3): the thirteen blueprints packed,
    /// the county's tile and paint planes, and the sim over them as above.
    pub packed_blueprints: usize,
    /// The console form's build (`build_one_packed_with`): its stages and their highest peak.
    pub packed_stages: Vec<StageRow>,
    pub packed_build_peak: usize,
    pub packed_county: Vec<(&'static str, usize)>,
    pub packed_sim_new_game: usize,
    pub packed_sim_parts: Vec<(&'static str, usize)>,
    pub packed_sim_resident: usize,
    pub packed_sim_peak: usize,
}

/// The sim over a set of blueprints: at New Game (and by part), after the idle ticks, its peak.
struct SimRow {
    new_game: usize,
    parts: Vec<(&'static str, usize)>,
    resident: usize,
    peak: usize,
}

fn sim_row(bps: Blueprints, base: usize, ticks: u32) -> SimRow {
    let w = Window::open();
    let mut sim = Sim::new_game_with(bps, "Jane");
    let new_game = w.live() - base;
    let frames = [InputFrame::IDLE; 4];
    for _ in 0..ticks {
        sim.step(&StepInput { frames, commands: &[] });
        let _ = sim.drain_events();
    }
    let (l, p) = w.close();
    // Sized after the window, so the clones are not in the peak.
    let mut parts = Vec::new();
    let st = sim.state();
    // A clone shares the name runs (`Names` is shared on clone): the table's own runs, those no
    // blueprint holds, are added at their size.
    let own_names: usize = st
        .syms
        .runs()
        .filter(|r| !ZoneId::ALL.iter().any(|&z| sim.blueprint(z).local_names.same(r)))
        .map(jane_core::Names::heap_bytes)
        .sum();
    parts.push(("state (zones' rows, syms, journal)", sized(st) + own_names));
    parts.push(("  of it: names interned (syms)", sized(&st.syms) + own_names));
    let zone_rows = |f: &dyn Fn(&jane_sim::ZoneState) -> usize| st.zones.iter().flatten().map(|z| f(z)).sum::<usize>();
    parts.push(("  of it: units", zone_rows(&|z| sized(&z.units))));
    parts.push(("  of it: props", zone_rows(&|z| sized(&z.props))));
    let grids: usize = ZoneId::ALL.iter().filter_map(|&z| sim.runtime(z)).map(|rt| sized(&rt.grid)).sum();
    parts.push(("runtime grids (flags, parts, occupancy)", grids));
    let rts = || ZoneId::ALL.iter().filter_map(|&z| sim.runtime(z));
    parts.push(("runtime prop buckets", rts().map(|rt| sized(&rt.props)).sum()));
    parts.push((
        "runtime lookups (names, marks, rects, locals)",
        rts().map(|rt| sized(&rt.names) + sized(&rt.unit_names) + sized(&rt.marks) + sized(&rt.rects) + sized(&rt.locals)).sum(),
    ));
    parts.push((
        "runtime lists (awake, triggers, regions)",
        rts()
            .map(|rt| {
                sized(&rt.awake_props)
                    + sized(&rt.awake_prop_list)
                    + sized(&rt.awake_units)
                    + sized(&rt.unit_blocks)
                    + sized(&rt.triggers)
                    + sized(&rt.regions)
                    + sized(&rt.prev_pos)
            })
            .sum(),
    ));
    let before = live();
    let path = jane_sim::path::PathScratch::default();
    parts.push(("path scratch (A* window)", live().saturating_sub(before)));
    drop(path);
    drop(sim);
    SimRow { new_game, parts, resident: l - base, peak: p - base }
}

/// The heap a clone of `v` asks for.
fn sized<T: Clone>(v: &T) -> usize {
    let before = live();
    let c = v.clone();
    let n = live().saturating_sub(before);
    drop(c);
    n
}

fn fields(bp: &Blueprint) -> Vec<(&'static str, usize)> {
    vec![
        ("tiles", sized(&bp.tiles)),
        ("units", sized(&bp.units)),
        ("props", sized(&bp.props)),
        ("marks", sized(&bp.marks)),
        ("rects", sized(&bp.rects)),
        ("triggers", sized(&bp.triggers)),
        ("stories", sized(&bp.stories)),
        ("paint", sized(&bp.paint)),
        ("lists", sized(&bp.lists)),
        ("conds", sized(&bp.conds)),
        ("name_lists", sized(&bp.name_lists)),
        ("texts", sized(&bp.texts)),
        ("local_names", bp.local_names.heap_bytes()),
        ("areas", sized(&bp.areas)),
        ("regions", sized(&bp.regions)),
        ("sanctuary", sized(&bp.sanctuary)),
    ]
}

/// The thirteen zones built one form or the other, each stage's live and peak into `stages`,
/// each blueprint's bytes into `rows`.
fn build_all(
    seed: u32,
    base: usize,
    packed: bool,
    stages: &mut Vec<StageRow>,
    rows: &mut Vec<BpRow>,
) -> Result<Vec<Arc<Blueprint>>, String> {
    let mut zones: Vec<Arc<Blueprint>> = Vec::with_capacity(jane_core::ZONE_COUNT);
    for z in ZoneId::ALL {
        let before = live();
        let mut open: Option<(&'static str, Window)> = None;
        let mut report = |name: &'static str| {
            if let Some((n, w)) = open.take() {
                let (l, p) = w.close();
                stages.push(StageRow { name: n, live: l - base, peak: p - base });
            }
            open = Some((name, Window::open()));
        };
        let built = if packed {
            jane_sim::blueprints::build_one_packed_with(z, seed, &mut report)
        } else {
            jane_sim::blueprints::build_one_with(z, seed, &mut report)
        };
        let bp = built.map_err(|e| format!("seed {seed}: {e}"))?;
        if let Some((n, w)) = open.take() {
            let (l, p) = w.close();
            let n = if z == ZoneId::County { "solve" } else { n };
            stages.push(StageRow { name: n, live: l - base, peak: p - base });
        }
        let retained = live().saturating_sub(before);
        rows.push(BpRow { zone: z.name(), retained, fields: fields(&bp) });
        zones.push(Arc::new(bp));
    }
    Ok(zones)
}

/// Measures the build of `seed`'s thirteen zones, then the sim over them for `ticks` idle ticks;
/// then the same in the console form (built packed).
pub fn measure(seed: u32, ticks: u32) -> Result<HeapReport, String> {
    let mut r = HeapReport { seed, ticks, ..HeapReport::default() };
    let _ = jane_data::catalog(); // the catalog is static: built before anything is counted
    r.stages.reserve(64);
    r.packed_stages.reserve(64);
    let base = live();

    let zones = build_all(seed, base, false, &mut r.stages, &mut r.blueprints)?;
    r.build_peak = r.stages.iter().map(|s| s.peak).max().unwrap_or(0);
    r.build_resident = live() - base;

    let all = |zones: &[Arc<Blueprint>]| -> [Arc<Blueprint>; jane_core::ZONE_COUNT] {
        core::array::from_fn(|i| Arc::clone(&zones[i]))
    };
    let pc = sim_row(Blueprints::from_parts(seed, all(&zones)), base, ticks);
    (r.sim_new_game, r.sim_parts, r.sim_resident, r.sim_peak) = (pc.new_game, pc.parts, pc.resident, pc.peak);
    drop(zones);

    // The console form: built packed, and the sim over it.
    let mut rows = Vec::new();
    let zones = build_all(seed, base, true, &mut r.packed_stages, &mut rows)?;
    r.packed_build_peak = r.packed_stages.iter().map(|s| s.peak).max().unwrap_or(0);
    r.packed_blueprints = live() - base;
    if let Some(p) = &zones[ZoneId::County.index()].packed {
        r.packed_county = vec![("tiles", p.tiles.heap_bytes()), ("paint", p.paint.heap_bytes())];
    }
    let packed = sim_row(Blueprints::from_parts(seed, all(&zones)), base, ticks);
    (r.packed_sim_new_game, r.packed_sim_parts, r.packed_sim_resident, r.packed_sim_peak) =
        (packed.new_game, packed.parts, packed.resident, packed.peak);
    Ok(r)
}

/// The county's build stage by stage, what each part of the `County` holds when the stage ends
/// (`--county`): where the build's floor is.
pub fn county_parts(seed: u32) -> Result<String, String> {
    use jane_world::county::{County, STAGES, county_skeleton};
    let _ = jane_data::catalog();
    let base = live();
    let sk = county_skeleton(seed, 0).map_err(|e| format!("{e:?}"))?;
    let mut c = County::new(&sk, 0);
    let mut s = String::new();
    let names = [
        "live", "tiles", "claims", "props", "units", "names", "paint", "bp rest", "trodden", "earth", "reached", "ground",
        "before", "lines", "places", "ways", "claims2", "perims", "country", "rest",
    ];
    for n in &names {
        let _ = write!(s, "{:>8}", &n[..n.len().min(8)]);
    }
    let _ = writeln!(s, "  stage");
    for &(name, stage) in STAGES {
        let w = Window::open();
        stage(&mut c);
        c.release_after(name);
        let (l, p) = w.close();
        let bp = c.k.blueprint();
        let bp_rest = sized(&bp.marks)
            + sized(&bp.rects)
            + sized(&bp.lists)
            + sized(&bp.conds)
            + sized(&bp.name_lists)
            + sized(&bp.texts)
            + sized(&bp.stories)
            + sized(&bp.triggers);
        let row = [
            l - base,
            sized(&bp.tiles),
            sized(c.k.claims()),
            sized(&bp.props),
            sized(&bp.units),
            bp.local_names.heap_bytes(),
            sized(&bp.paint),
            bp_rest,
            sized(&c.trodden),
            sized(&c.wild_earth),
            sized(&c.reached),
            sized(&c.ground),
            sized(&c.before),
            sized(&c.lines) + sized(&c.lit),
            sized(&c.places),
            sized(&c.ways),
            sized(&c.story_claims),
            sized(&c.perimeters),
            sized(&c.country.d_road) + sized(&c.country.d_first),
        ];
        let rest = row[0].saturating_sub(row[1..].iter().sum::<usize>());
        for v in row.iter().chain([rest].iter()) {
            let _ = write!(s, "{:>8}", mb(*v).trim());
        }
        let _ = writeln!(s, "  {name} (peak {})", mb(p - base).trim());
    }
    Ok(s)
}

fn mb(n: usize) -> String {
    format!("{:7.2}", n as f64 / MB)
}

impl HeapReport {
    pub fn table(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "seed {}: heap in MB (requested bytes)\n", self.seed);
        let _ = writeln!(s, "{:<18} {:>7} {:>7} {:>7}", "stage", "live", "peak", "+peak");
        let mut prev = 0;
        for row in &self.stages {
            let _ =
                writeln!(s, "{:<18} {} {} {}", row.name, mb(row.live), mb(row.peak), mb(row.peak.saturating_sub(prev)));
            prev = row.live;
        }
        let _ = writeln!(s, "build peak {} MB, resident after {} MB\n", mb(self.build_peak), mb(self.build_resident));
        let names: Vec<&str> = self.blueprints[0].fields.iter().map(|f| f.0).collect();
        let _ = write!(s, "{:<10} {:>7}", "blueprint", "kept");
        for n in &names {
            let _ = write!(s, " {:>7}", &n[..n.len().min(7)]);
        }
        let _ = writeln!(s);
        let mut total = vec![0usize; names.len()];
        let mut kept = 0;
        for b in &self.blueprints {
            let _ = write!(s, "{:<10} {}", b.zone, mb(b.retained));
            kept += b.retained;
            for (i, f) in b.fields.iter().enumerate() {
                total[i] += f.1;
                let _ = write!(s, " {}", mb(f.1));
            }
            let _ = writeln!(s);
        }
        let _ = write!(s, "{:<10} {}", "all 13", mb(kept));
        for t in &total {
            let _ = write!(s, " {}", mb(*t));
        }
        let _ = writeln!(s, "\n");
        let _ = writeln!(
            s,
            "sim: new game {} MB live, after {} idle ticks {} MB, peak {} MB",
            mb(self.sim_new_game),
            self.ticks,
            mb(self.sim_resident),
            mb(self.sim_peak)
        );
        for (n, b) in &self.sim_parts {
            let _ = writeln!(s, "  {n:<40} {}", mb(*b));
        }
        let _ = writeln!(s, "\npacked (the console form, built packed): build peak {} MB, blueprints {} MB", mb(self.packed_build_peak), mb(self.packed_blueprints));
        let mut worst: Vec<&StageRow> = self.packed_stages.iter().collect();
        worst.sort_by_key(|r| core::cmp::Reverse(r.peak));
        let _ = writeln!(
            s,
            "  highest stages: {}",
            worst.iter().take(5).map(|r| format!("{} {}", r.name, mb(r.peak).trim())).collect::<Vec<_>>().join(", ")
        );
        for (n, b) in &self.packed_county {
            let _ = writeln!(s, "  county {n:<33} {}", mb(*b));
        }
        let _ = writeln!(
            s,
            "sim: new game {} MB live, after {} idle ticks {} MB, peak {} MB",
            mb(self.packed_sim_new_game),
            self.ticks,
            mb(self.packed_sim_resident),
            mb(self.packed_sim_peak)
        );
        for (n, b) in &self.packed_sim_parts {
            let _ = writeln!(s, "  {n:<40} {}", mb(*b));
        }
        s
    }

    pub fn json(&self) -> String {
        let stages: Vec<serde_json::Value> =
            self.stages.iter().map(|r| serde_json::json!({ "name": r.name, "live": r.live, "peak": r.peak })).collect();
        let bps: Vec<serde_json::Value> = self
            .blueprints
            .iter()
            .map(|b| {
                let f: serde_json::Map<String, serde_json::Value> =
                    b.fields.iter().map(|(n, v)| ((*n).to_owned(), (*v).into())).collect();
                serde_json::json!({ "zone": b.zone, "retained": b.retained, "fields": f })
            })
            .collect();
        serde_json::json!({
            "seed": self.seed,
            "stages": stages,
            "build_peak": self.build_peak,
            "build_resident": self.build_resident,
            "blueprints_retained": self.blueprints.iter().map(|b| b.retained).sum::<usize>(),
            "blueprints": bps,
            "sim_new_game": self.sim_new_game,
            "sim_resident": self.sim_resident,
            "sim_peak": self.sim_peak,
            "ticks": self.ticks,
            "packed_blueprints": self.packed_blueprints,
            "packed_build_peak": self.packed_build_peak,
            "packed_sim_new_game": self.packed_sim_new_game,
            "packed_sim_resident": self.packed_sim_resident,
            "packed_sim_peak": self.packed_sim_peak,
        })
        .to_string()
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut seed = 1u32;
    let mut ticks = 600u32;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()).ok_or("--seed N")?,
            "--ticks" => ticks = it.next().and_then(|s| s.parse().ok()).ok_or("--ticks N")?,
            "--json" => json = true,
            "--county" => {
                print!("{}", county_parts(seed)?);
                return Ok(());
            }
            o => return Err(format!("unknown option {o}\n{USAGE}")),
        }
    }
    let r = measure(seed, ticks)?;
    if json {
        println!("{}", r.json());
    } else {
        print!("{}", r.table());
    }
    Ok(())
}
