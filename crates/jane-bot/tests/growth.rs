//! Growth, measured (PLAN.md §2.6, DUNGEONS.md "Growth on the story's path"): the Reader plays
//! the whole story on seeds 1 to 5, and every hour of play (and on first coming into each
//! dungeon and each region of the county) her strength, spirit and health are read off her,
//! with what the fight is at those numbers:
//!
//! - **hits to kill**: a foe's health (its row at its ground's phase) over her average blow
//!   (melee, and the best bolt she has learned), its resist counted, the 1 in 20 crit counted;
//! - **its blow**: the foe's heaviest average blow as a share of her health (per mille);
//! - what she was actually hit for that hour (the trace's `Hurt`s), and deaths.
//!
//! The early foes are the Lowfields' rat, skeleton and spider at phase 1; a region's or a
//! dungeon's own are every hostile row its blueprint spawns (bosses apart), at their phase.
//!
//! The targets it holds (PLAN.md §2.6's curve): at New Game an early foe takes her 2 to 4 blows
//! and lands a blow worth at least 5 % of her; by the end each dies to one blow and its blow is
//! under 2 % of her; on arrival in each dungeon its own foes take at least two blows of her
//! best on average (nothing new is a one-blow thing).
//!
//! A whole story on five seeds is minutes of release time, so the run is `#[ignore]`d, with the
//! story test: `cargo test --release -p jane-bot --test growth -- --ignored --nocapture`.

// The report prints means and shares: a test's arithmetic on small counts, never the sim's.
#![allow(clippy::cast_precision_loss)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use jane_bot::run::Session;
use jane_bot::{Bot, Ending, Model};
use jane_core::action::Stat;
use jane_core::{SpellId, UnitDefId, ZoneId};
use jane_data::{SpellKind, SpellPower, UnitDef};
use jane_sim::trace::{Ev, Kind};
use jane_sim::{Blueprints, Seat, Sim};

/// Frames an hour of play.
const HOUR: u32 = 60 * 60 * 60;
const STORY_FRAMES: u32 = 40 * HOUR;
const PHASE_SCALE: [u16; 7] = [1, 1, 2, 3, 4, 6, 8];
const EARLY: [&str; 3] = ["rat", "skeleton", "spider"];

/// Her stats at a moment.
#[derive(Clone, Debug, Default)]
struct Her {
    strength: u16,
    spirit: u16,
    learned: Vec<SpellId>,
}

impl Her {
    fn hp(&self) -> i64 {
        i64::from(self.strength) * 5
    }
    fn stat(&self, s: Stat) -> i64 {
        i64::from(match s {
            Stat::Strength => self.strength,
            Stat::Spirit => self.spirit,
        })
    }
}

/// A power's average roll at `stat`, milli-points, with the 1 in 20 double.
fn avg(p: &SpellPower, stat: i64) -> i64 {
    let fixed = stat * 1_000_000 / i64::from(p.div.max(1));
    let var = stat * 1000 / i64::from(p.var_div.max(1)) * 1000 / 2;
    (fixed + var + i64::from(p.flat.0)) * 21 / 20
}

/// A foe as spawned: its row and its phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Foe {
    def: UnitDefId,
    phase: u8,
}

impl Foe {
    fn row(self) -> &'static UnitDef {
        jane_data::catalog().combat.unit(self.def)
    }
    fn scale(self) -> i64 {
        i64::from(PHASE_SCALE[usize::from(self.phase).min(6)])
    }
    fn hp(self) -> i64 {
        i64::from(self.row().strength) * self.scale() * 5 * 1000
    }
    /// Its heaviest average blow, milli-points.
    fn blow(self) -> i64 {
        let cat = jane_data::catalog();
        let r = self.row();
        r.book
            .iter()
            .filter_map(|&s| cat.combat.spell(s).power.as_ref())
            .map(|p| {
                let stat = i64::from(match p.stat {
                    Stat::Strength => r.strength,
                    Stat::Spirit => r.spirit,
                }) * self.scale();
                avg(p, stat)
            })
            .max()
            .unwrap_or(0)
    }
}

/// Her blows at `her`: melee, and the best learned bolt (by its average on `foe`).
fn hits(her: &Her, foe: Foe) -> (i64, Option<i64>) {
    let cat = jane_data::catalog();
    let on = |s: SpellId| {
        let sp = cat.combat.spell(s);
        let p = sp.power.as_ref()?;
        let resist = i64::from(foe.row().resist_of(sp.school).0);
        let a = avg(p, her.stat(p.stat)) * (1000 - resist) / 1000;
        (a > 0).then(|| (foe.hp() + a - 1) / a)
    };
    let melee = cat.combat.spell_id("melee_player").and_then(on).unwrap_or(i64::MAX);
    let bolt =
        her.learned.iter().filter(|&&s| cat.combat.spell(s).kind == SpellKind::Bolt).filter_map(|&s| on(s)).min();
    (melee, bolt)
}

/// Its blow as per mille of her health.
fn bite(her: &Her, foe: Foe) -> i64 {
    foe.blow() / her.hp().max(1)
}

/// Every hostile row a zone spawns (bosses and harmless things apart), with how many; the
/// county's by region (0..=2 as `zone` 100 + region).
fn foes(bps: &Blueprints) -> BTreeMap<u16, BTreeMap<Foe, u32>> {
    let cat = jane_data::catalog();
    let mut out: BTreeMap<u16, BTreeMap<Foe, u32>> = BTreeMap::new();
    let zones = std::iter::once(ZoneId::County).chain(jane_bot::crawl::ORDER);
    for z in zones {
        let bp = bps.get(z);
        for u in &bp.units {
            let r = cat.combat.unit(u.def);
            if r.boss
                || r.aggro.0 == 0
                || !jane_sim::combat::is_enemy(jane_data::Faction::Friendly, r.faction)
                || !r.book.iter().any(|&s| cat.combat.spell(s).power.is_some())
            {
                continue;
            }
            let key = if z == ZoneId::County {
                100 + u16::from(bp.region_at(i32::from(u.cell.x), i32::from(u.cell.y)).unwrap_or(0))
            } else {
                z.index() as u16
            };
            *out.entry(key).or_default().entry(Foe { def: u.def, phase: u.phase.max(1) }).or_insert(0) += 1;
        }
    }
    out
}

fn place_name(key: u16) -> String {
    match key {
        100 => "Lowfields".into(),
        101 => "Waters".into(),
        102 => "Works".into(),
        k => jane_bot::crawl::ORDER.iter().find(|z| z.index() as u16 == k).map_or("?".into(), |z| z.name().into()),
    }
}

/// A place's foes at `her`: mean hits to kill with her best, the mean phase, and the mean blow
/// per mille, each weighted by count.
fn place(her: &Her, fs: &BTreeMap<Foe, u32>) -> (i64, i64, i64, i64) {
    let n: i64 = fs.values().map(|&c| i64::from(c)).sum::<i64>().max(1);
    let (mut melee, mut best, mut bite_, mut phase) = (0, 0, 0, 0);
    for (&f, &c) in fs {
        let c = i64::from(c);
        let (m, b) = hits(her, f);
        melee += m.min(99) * c;
        best += m.min(b.unwrap_or(i64::MAX)).min(99) * c;
        bite_ += bite(her, f) * c;
        phase += i64::from(f.phase) * c;
    }
    (melee * 10 / n, best * 10 / n, bite_ / n, phase * 10 / n)
}

struct Hour {
    her: Her,
    zone: u16,
    deaths: u32,
    hurt: i64,
    blows: u32,
}

struct Run {
    seed: u32,
    /// Her at New Game.
    start: Her,
    hours: Vec<Hour>,
    /// First coming into a place (a dungeon, or a county region): the place, the hour, her.
    arrivals: Vec<(u16, u32, Her)>,
    kit: BTreeMap<u16, (i32, i32)>,
    foes: BTreeMap<u16, BTreeMap<Foe, u32>>,
    deaths: u32,
    the_end: u8,
}

fn her_of(sim: &Sim) -> Her {
    let v = sim.view(Seat(0)).expect("seat 0");
    let b = v.body();
    Her { strength: b.strength, spirit: b.spirit, learned: v.learned().to_vec() }
}

fn place_of(sim: &Sim) -> u16 {
    let v = sim.view(Seat(0)).expect("seat 0");
    match v.zone() {
        ZoneId::County => 100 + v.region() as u16,
        z => z.index() as u16,
    }
}

fn play(seed: u32) -> Run {
    let bps = Blueprints::build(seed).expect("the seed builds");
    let foes = foes(&bps);
    let kit =
        jane_bot::crawl::ORDER.iter().map(|&z| (z.index() as u16, jane_bot::crawl::growth_before(&bps, z))).collect();
    let mut sim = Sim::new_game_with(bps, "Jane");
    let start = her_of(&sim);
    let mut bot = Bot::story(Model::Reader);
    bot.ctx.ending = Some([Ending::Hold, Ending::Hill, Ending::Train][(seed as usize + 2) % 3]);
    let mut sess = Session::new(bot, &sim, STORY_FRAMES / 3600);
    let mut hours: Vec<Hour> = Vec::new();
    let mut arrivals: Vec<(u16, u32, Her)> = Vec::new();
    let mut here: BTreeMap<u16, u32> = BTreeMap::new();
    let mut frame = 0;
    while frame < STORY_FRAMES && !sess.bot.done() {
        sess.step(&mut sim);
        frame += 1;
        if frame % 60 == 0 {
            let p = place_of(&sim);
            *here.entry(p).or_insert(0) += 1;
            if !arrivals.iter().any(|a| a.0 == p) {
                arrivals.push((p, frame / HOUR, her_of(&sim)));
            }
        }
        if frame % HOUR == 0 {
            let zone = here.iter().max_by_key(|e| *e.1).map_or(100, |e| *e.0);
            here.clear();
            hours.push(Hour { her: her_of(&sim), zone, deaths: 0, hurt: 0, blows: 0 });
        }
    }
    let zone = here.iter().max_by_key(|e| *e.1).map_or(100, |e| *e.0);
    hours.push(Hour { her: her_of(&sim), zone, deaths: 0, hurt: 0, blows: 0 });
    let (bot, trace) = sess.finish(&sim);
    for r in &trace.records {
        if r.seat != Some(0) {
            continue;
        }
        let last = hours.len() - 1;
        let h = &mut hours[((r.frame / HOUR) as usize).min(last)];
        match r.kind {
            Kind::Event(Ev::Died { .. }) => h.deaths += 1,
            Kind::Event(Ev::Hurt { amount, .. }) => {
                h.hurt += i64::from(amount);
                h.blows += 1;
            }
            _ => {}
        }
    }
    let v = sim.view(Seat(0)).expect("seat 0");
    Run { seed, start, deaths: bot.deaths.len() as u32, the_end: v.the_end(), hours, arrivals, kit, foes }
}

fn early(foes: &BTreeMap<u16, BTreeMap<Foe, u32>>) -> Vec<Foe> {
    let cat = jane_data::catalog();
    EARLY
        .iter()
        .map(|n| Foe { def: cat.combat.unit_id(n).expect("an early row"), phase: 1 })
        .inspect(|f| assert!(foes.values().any(|fs| fs.keys().any(|g| g.def == f.def)), "no {:?} spawns", f.def))
        .collect()
}

fn bolt(b: Option<i64>) -> String {
    b.map_or("-".into(), |b| b.to_string())
}

fn report(r: &Run) -> String {
    let mut s = String::new();
    let ef = early(&r.foes);
    let _ = writeln!(s, "seed {} (the_end {}, {} deaths)", r.seed, r.the_end, r.deaths);
    let _ = writeln!(
        s,
        "  hr |   hp str spi | rat m/b bite | skel m/b bite | spider m/b bite | where       own: melee best bite ph | taken/blow deaths"
    );
    let e: Vec<String> = ef
        .iter()
        .map(|&f| {
            let (m, b) = hits(&r.start, f);
            format!("{:>3}/{:<3} {:>3}", m, bolt(b), bite(&r.start, f))
        })
        .collect();
    let _ = writeln!(
        s,
        "  NG | {:>4} {:>3} {:>3} | {} | {}  | {}",
        r.start.hp(),
        r.start.strength,
        r.start.spirit,
        e[0],
        e[1],
        e[2]
    );
    for (i, h) in r.hours.iter().enumerate() {
        let e: Vec<String> = ef
            .iter()
            .map(|&f| {
                let (m, b) = hits(&h.her, f);
                format!("{:>3}/{:<3} {:>3}", m, bolt(b), bite(&h.her, f))
            })
            .collect();
        let own = r.foes.get(&h.zone).map(|fs| place(&h.her, fs)).unwrap_or_default();
        let _ = writeln!(
            s,
            "  {:>2} | {:>4} {:>3} {:>3} | {} | {}  | {}    | {:<11} {:>4.1} {:>4.1} {:>4} {:.1} | {:>4} {:>6}",
            i,
            h.her.hp(),
            h.her.strength,
            h.her.spirit,
            e[0],
            e[1],
            e[2],
            place_name(h.zone),
            own.0 as f32 / 10.0,
            own.1 as f32 / 10.0,
            own.2,
            own.3 as f32 / 10.0,
            if h.blows > 0 { h.hurt * 1000 / i64::from(h.blows) / h.her.hp().max(1) } else { 0 },
            h.deaths,
        );
    }
    let _ =
        writeln!(s, "  arrivals: place (hour): hp str spi, kit str+ spi+ | own foes: melee best bite(per mille) phase");
    for (p, hr, her) in &r.arrivals {
        let Some(fs) = r.foes.get(p) else { continue };
        let own = place(her, fs);
        let kit = r.kit.get(p).copied().unwrap_or_default();
        let _ = writeln!(
            s,
            "    {:<10} ({:>2}): {:>4} {:>3} {:>3}, kit +{} +{} | {:>4.1} {:>4.1} {:>4} {:.1}",
            place_name(*p),
            hr,
            her.hp(),
            her.strength,
            her.spirit,
            kit.0,
            kit.1,
            own.0 as f32 / 10.0,
            own.1 as f32 / 10.0,
            own.2,
            own.3 as f32 / 10.0,
        );
    }
    s
}

#[test]
#[ignore = "slow: a whole story on five seeds, minutes each in release"]
fn the_growth_curve_on_seeds_1_to_5() {
    let runs: Vec<Run> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=5).map(|s| sc.spawn(move || play(s))).collect();
        hs.into_iter().map(|h| h.join().expect("a story run")).collect()
    });
    for r in &runs {
        println!("{}", report(r));
    }
    println!("{}", curve(&runs));
    let mut problems = Vec::new();
    for r in &runs {
        let ef = early(&r.foes);
        let end = &r.hours.last().expect("an hour").her;
        for &f in &ef {
            let name = f.row().id;
            let (m, _) = hits(&r.start, f);
            if !(2..=4).contains(&m) {
                problems.push(format!("seed {}: at New Game a {name} takes {m} blows, not 2 to 4", r.seed));
            }
            if bite(&r.start, f) < 50 {
                problems.push(format!("seed {}: at New Game a {name}'s blow is {}‰ of her", r.seed, bite(&r.start, f)));
            }
            let (m, b) = hits(end, f);
            if m.min(b.unwrap_or(m)) > 1 {
                problems.push(format!("seed {}: at the end a {name} takes {m} blows", r.seed));
            }
            if bite(end, f) > 20 {
                problems.push(format!("seed {}: at the end a {name}'s blow is {}‰ of her", r.seed, bite(end, f)));
            }
        }
        for (p, _, her) in r.arrivals.iter().filter(|a| a.0 < 100) {
            let Some(fs) = r.foes.get(p) else { continue };
            let own = place(her, fs);
            if own.1 < 20 {
                problems.push(format!(
                    "seed {}: on arrival the {}'s own foes take {} blows of her best",
                    r.seed,
                    place_name(*p),
                    own.1 as f32 / 10.0
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{}",
        problems.join(
            "
"
        )
    );
}

/// The five seeds as one curve, hour by hour (a seed's last hour counts until the longest ends):
/// means of her health, strength and spirit, the early foes' blows to kill (melee / her best)
/// and their blow in per mille of her, her own ground's foes' blows of her best and their blow,
/// what she was hit for a blow, and deaths (summed); `n` the seeds still playing that hour, and
/// `end` each seed's last state.
fn curve(runs: &[Run]) -> String {
    let ef = early(&runs[0].foes);
    let mut s = String::new();
    let _ = writeln!(
        s,
        "curve (seeds 1-5): hour n | hp str spi | rat skel spider: melee/best bite‰ | own ground: best bite‰ | hit for‰ | deaths"
    );
    let row = |s: &mut String, label: &str, hers: &[&Her], own: Option<(f32, f32)>, taken: Option<f32>, deaths: u32| {
        let n = hers.len().max(1) as f32;
        let mean = |f: &dyn Fn(&Her) -> f32| hers.iter().map(|h| f(h)).sum::<f32>() / n;
        let mut e = String::new();
        for &f in &ef {
            let m = mean(&|h| hits(h, f).0 as f32);
            let b = mean(&|h| hits(h, f).1.map_or(hits(h, f).0, |b| b.min(hits(h, f).0)) as f32);
            let _ = write!(e, " {m:>3.1}/{b:<3.1} {:>3.0} |", mean(&|h| bite(h, f) as f32));
        }
        let own = own.map_or("   -        ".into(), |(b, t)| format!("{b:>4.1} {t:>4.0}   "));
        let _ = writeln!(
            s,
            "  {label:>4} {} | {:>4.0} {:>3.0} {:>3.0} |{e} {own}| {:>5} | {deaths}",
            hers.len(),
            mean(&|h| h.hp() as f32),
            mean(&|h| f32::from(h.strength)),
            mean(&|h| f32::from(h.spirit)),
            taken.map_or("-".into(), |t| format!("{t:.0}")),
        );
    };
    let starts: Vec<&Her> = runs.iter().map(|r| &r.start).collect();
    row(&mut s, "NG", &starts, None, None, 0);
    let longest = runs.iter().map(|r| r.hours.len()).max().unwrap_or(0);
    for i in 0..longest {
        let hs: Vec<&Hour> = runs.iter().filter_map(|r| r.hours.get(i)).collect();
        let hers: Vec<&Her> = hs.iter().map(|h| &h.her).collect();
        let owns: Vec<(i64, i64)> = runs
            .iter()
            .filter_map(|r| r.hours.get(i).and_then(|h| r.foes.get(&h.zone)).map(|fs| (r, fs, &r.hours[i])))
            .map(|(_, fs, h)| {
                let p = place(&h.her, fs);
                (p.1, p.2)
            })
            .collect();
        let own = (!owns.is_empty()).then(|| {
            let n = owns.len() as f32;
            (owns.iter().map(|o| o.0 as f32 / 10.0).sum::<f32>() / n, owns.iter().map(|o| o.1 as f32).sum::<f32>() / n)
        });
        let blows: u32 = hs.iter().map(|h| h.blows).sum();
        let taken = (blows > 0)
            .then(|| hs.iter().map(|h| h.hurt as f32 * 1000.0 / h.her.hp().max(1) as f32).sum::<f32>() / blows as f32);
        row(&mut s, &format!("{}", i + 1), &hers, own, taken, hs.iter().map(|h| h.deaths).sum());
    }
    let ends: Vec<&Her> = runs.iter().filter_map(|r| r.hours.last()).map(|h| &h.her).collect();
    row(&mut s, "end", &ends, None, None, runs.iter().map(|r| r.deaths).sum());
    s
}
