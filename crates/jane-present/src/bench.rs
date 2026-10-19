//! The built-in benchmark: seven scripted scenes run back to back on a fresh game that is never
//! saved, each measured, one line each in `bench.txt`. The owner runs it in place of playing, on a
//! PSP (the title's Benchmark row, or a `script.txt` holding `bench`) or on a PC (`jane bench
//! scenes`), and the same scenes write the same lines on both.
//!
//! A scene is data: the PSP script's words (`spikes/psp-game`'s `script.txt`, as
//! `tools/psp-walk-test.py` writes them: `stick:`, `hold:`, `press:`, `clock:`, `tp:`, the end
//! tick) and the bench's own few, which [`Plan`] reads:
//!
//! - `at:<zone>:<mark>[:<x>,<y>][@p<tick>]`: she travels there (to the free cell nearest that cell
//!   of the zone, else to the mark), at tick 0 unless a tick is given;
//! - `route:<x>,<y>/<x>,<y>/..[@p<tick>]`: she walks to those cells in turn by the sim's click to
//!   move ([`Steer`]);
//!   `loop:` the same, back and forth for the rest of the scene;
//! - `spawn:<unit>:<n>[@p<tick>]`: `n` of the unit made beside her, one a tick (the sim's own
//!   `Dev(Spawn)` command, so the run stays the sim's to replay);
//! - `every:<button>:<period>@p<a>-p<b>`: the button pressed once each period meanwhile;
//! - `tough`: her health filled every [`TOUGH_EVERY`] ticks, so a mob does not end the scene (not
//!   `god`, which the foes do not notice at all);
//! - `settle:<ticks>`: the scene's first ticks not counted in its frame rate (the travel's own
//!   frame; default [`SETTLE`]).
//!
//! Dev and bench only: nothing here runs in a player's game unless the Benchmark row is picked,
//! and the spawn word reaches the sim only through a script or the bench.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use jane_core::{Angle, ZoneId};
use jane_sim::input::DevOp;
use jane_sim::state::TravelRequest;
use jane_sim::{Command, Seat, Sim};

use crate::pad_psp::psp;

/// The county the scenes were laid out on (the walk tests' seed; the county is the bake's, so the
/// cells hold on any seed, but the dungeons are the seed's).
pub const SEED: u32 = 570_944_055;

/// The sim's ticks a second.
pub const TICKS_PER_SECOND: u32 = jane_core::num::TICK_RATE;

/// Ticks a scene leaves out of its frame rate unless it says otherwise.
pub const SETTLE: u32 = 60;

/// `tough`: her health filled this often, ticks.
pub const TOUGH_EVERY: u32 = 10;

/// A stall this long (microseconds) is a hang: the watchdog writes what it knows.
pub const HANG_US: u32 = 3_000_000;

/// One scene of the bench.
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    pub name: &'static str,
    /// What it stresses.
    pub about: &'static str,
    pub words: &'static str,
}

/// The scenes, in the order they run (about five minutes in all).
pub const SCENES: [Scene; 7] = [
    Scene {
        name: "sprint",
        about: "running flat out from the Halt up the station road, through Castle and out east: ground painting, chunk streaming",
        words: "at:county:start:1000,1528 clock:14:13 tough hold:r@p1-p3600 \
                route:994,1528/988,1528/982,1528/976,1528/970,1528/964,1523/958,1517/952,1511/946,1505/940,1499/\
                934,1493/928,1487/922,1481/916,1475/910,1469/904,1463/898,1457/892,1451/886,1445/880,1439/874,1433/\
                868,1427/862,1421/856,1415/850,1409/844,1403/838,1397/832,1391/826,1385/820,1379/814,1373/808,1368/\
                803,1362/797,1356/791,1350/785,1344/781,1338/781,1332/781,1326/775,1322/769,1322/763,1322/757,1320/\
                751,1320/745,1320/739,1320/738,1314/738,1308/738,1302/738,1296/738,1290/742,1284/742,1278/742,1272/\
                742,1266/741,1260/741,1254/741,1248/741,1242/741,1236/741,1230/741,1224/741,1218/741,1212/741,1206/\
                742,1200/742,1194/742,1188/742,1182/742,1176/742,1170/742,1164/742,1158/743,1152/749,1146/750,1140/\
                756,1134/759,1128/759,1122/759,1116/759,1110/759,1104/759,1098/759,1092/759,1086/759,1080/759,1074/\
                759,1068/756,1062/750,1056/744,1050/744,1044/744,1038/743,1032/743,1026/743,1020/746,1014/752,1009/\
                757,1003/763,1000/769,1000/775,1000/781,997/787,997/788,991/795,997/797,991/803,987 \
                3600",
    },
    Scene {
        name: "wood",
        about: "Blackthorn Holt at noon: sun shadows under the trees, the heaviest display list (about 600 casters)",
        words: "at:county:start:690,1395 clock:12:00 tough \
                loop:696,1394/708,1397/710,1409/714,1421/702,1426/690,1425/678,1414/666,1413/678,1414/690,1415/\
                702,1426/714,1422/712,1410/710,1398/698,1394 2400",
    },
    Scene {
        name: "town-night",
        about: "Castle at half past nine: lamps, the Night Shift about, lights on every wall",
        words: "at:county:town_square clock:21:30 tough spawn:night_soldier:6@p30 \
                loop:762,995/750,995/738,998/726,998/714,998/726,998/738,998/750,995/762,995/774,996/786,998/\
                798,998/804,998/798,998/786,998/774,993/768,989/762,988/758,982/758,970/756,982/755,994/750,1000/\
                750,1012/755,1006/761,996 2400",
    },
    Scene {
        name: "cellar",
        about: "Julie's cellar: its rats and five more in a tight room, bodies in each other's way (the freeze case)",
        words: "at:cellar:stair_a:18,33 tough spawn:rat:5@p20 every:right:120@p40-p2400 every:cross:20@p50-p2400 \
                loop:16,30/21,30 2400",
    },
    Scene {
        name: "mob",
        about: "sixteen hostiles after her over the Castle Viaduct's deck and the terrace edge under it: the chase and the height paths",
        words: "at:county:start:940,948 tough spawn:skeleton:8@p10 spawn:night_hound:4@p30 spawn:bandit:4@p50 \
                loop:946,949/952,950/958,950/964,953/970,953/976,953/982,953/988,953 2700",
    },
    Scene {
        name: "churn",
        about: "in and out of the house, the cellar, the Arms, the church and the Library, then over the viaduct: zone loads, caches, memory",
        words: "settle:0 tough tp:house:front@p30 stick:270@p40-p200 tp:county:house_front@p240 tp:cellar:stair_a@p450 \
                stick:270@p460-p640 tp:county:house_front@p660 tp:arms:entry@p870 stick:270@p880-p1060 \
                tp:county:arms_front@p1080 tp:church:entry@p1290 stick:270@p1300-p1480 tp:county:church_door@p1500 \
                tp:library:entry@p1710 stick:90@p1720-p1900 tp:county:library_mouth@p1920 at:county:start:940,948@p2130 \
                route:946,949/952,950/958,950/964,953/970,953/976,953/982,953/988,953/994,953/1000,956@p2140 2400",
    },
    Scene {
        name: "lake",
        about: "by the lake at the eel-path camp in the afternoon, water at both edges of the view: the water's passes (its mark, sky, crests and mirrors)",
        words: "at:county:eel_path_camp clock:15:00 tough 1800",
    },
];

/// A scene by name.
pub fn scene(name: &str) -> Option<&'static Scene> {
    SCENES.iter().find(|s| s.name == name)
}

/// `at:`: a travel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Travel {
    pub zone: ZoneId,
    pub mark: String,
    pub cell: Option<(i32, i32)>,
    pub tick: u32,
}

/// `route:` or `loop:`: cells to walk through from a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub from: u32,
    pub cells: Vec<(i32, i32)>,
    /// Back and forth along the cells until the scene ends.
    pub looped: bool,
}

/// `spawn:`: `n` of `unit`, one a tick from `tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub unit: String,
    pub n: u32,
    pub tick: u32,
}

/// What a scene's words ask. The PSP's script reads the words it knows itself (`stick:`, `hold:`,
/// `press:`, `clock:`, `tp:`, the end) and this the bench's; a PC driver reads them all from here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Ticks the scene runs.
    pub end: u32,
    pub settle: u32,
    /// `clock:HH:MM`, in ticks of the day.
    pub clock: Option<u32>,
    pub tough: bool,
    pub travels: Vec<Travel>,
    pub tps: Vec<(ZoneId, String, u32)>,
    pub routes: Vec<Route>,
    /// `stick:<degrees>@p<a>-p<b>`.
    pub sticks: Vec<(i32, u32, u32)>,
    /// `hold:` and `press:` (six ticks) and `every:`: `(PSP button bits, from, to, period)`;
    /// period 0 for a plain hold.
    pub buttons: Vec<(u32, u32, u32, u32)>,
    pub spawns: Vec<Spawn>,
}

/// `p<n>` (a play tick).
fn tick_of(s: &str) -> Option<u32> {
    s.strip_prefix('p')?.parse().ok()
}

/// `<word>@p<n>`: the word and its tick (0 without one).
fn split_at_tick(w: &str) -> Option<(&str, u32)> {
    match w.rsplit_once('@') {
        Some((v, at)) => Some((v, tick_of(at)?)),
        None => Some((w, 0)),
    }
}

fn cell_of(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn button(name: &str) -> Option<u32> {
    psp::NAMES.iter().find(|e| e.0 == name).map(|e| e.1)
}

impl Plan {
    /// The plan of a scene's words. Words it does not know are left to the script.
    pub fn parse(words: &str) -> Plan {
        let mut p = Plan { settle: SETTLE, ..Plan::default() };
        let mut end = None;
        for w in words.split_whitespace() {
            if let Ok(n) = w.parse::<u32>() {
                end = end.or(Some(n));
            } else if w == "tough" {
                p.tough = true;
            } else if let Some(n) = w.strip_prefix("settle:") {
                p.settle = n.parse().unwrap_or(SETTLE);
            } else if let Some(c) = w.strip_prefix("clock:") {
                let mut hm = c.split(':').map(|v| v.parse::<u32>().ok());
                if let (Some(Some(h)), Some(Some(m))) = (hm.next(), hm.next()) {
                    let hour = jane_core::num::TICKS_PER_HOUR;
                    p.clock = Some((h % 24) * hour + m.min(59) * hour / 60);
                }
            } else if let Some(rest) = w.strip_prefix("at:") {
                let Some((v, tick)) = split_at_tick(rest) else { continue };
                let mut parts = v.splitn(3, ':');
                let (Some(z), Some(mark)) = (parts.next().and_then(ZoneId::from_name), parts.next()) else { continue };
                let cell = parts.next().and_then(cell_of);
                p.travels.push(Travel { zone: z, mark: String::from(mark), cell, tick });
            } else if let Some((rest, looped)) =
                w.strip_prefix("route:").map(|r| (r, false)).or_else(|| w.strip_prefix("loop:").map(|r| (r, true)))
            {
                let Some((v, tick)) = split_at_tick(rest) else { continue };
                let cells: Vec<(i32, i32)> = v.split('/').filter_map(cell_of).collect();
                if !cells.is_empty() {
                    p.routes.push(Route { from: tick.max(1), cells, looped });
                }
            } else if let Some(rest) = w.strip_prefix("spawn:") {
                let Some((v, tick)) = split_at_tick(rest) else { continue };
                let Some((unit, n)) = v.split_once(':') else { continue };
                if let Ok(n) = n.parse() {
                    p.spawns.push(Spawn { unit: String::from(unit), n, tick });
                }
            } else if let Some(rest) = w.strip_prefix("tp:") {
                let Some((zm, at)) = rest.split_once('@') else { continue };
                let Some((z, m)) = zm.split_once(':') else { continue };
                if let (Some(z), Some(t)) = (ZoneId::from_name(z), tick_of(at)) {
                    p.tps.push((z, String::from(m), t));
                }
            } else if let Some(rest) = w.strip_prefix("stick:") {
                let Some((d, at)) = rest.split_once('@') else { continue };
                let Some((a, b)) = at.split_once('-') else { continue };
                if let (Ok(d), Some(a), Some(b)) = (d.parse(), tick_of(a), tick_of(b)) {
                    p.sticks.push((d, a, b));
                }
            } else if let Some(rest) = w.strip_prefix("hold:").or_else(|| w.strip_prefix("press:")) {
                let Some((b, at)) = rest.split_once('@') else { continue };
                let Some(bit) = button(b) else { continue };
                if w.starts_with("hold:") {
                    let Some((a, z)) = at.split_once('-') else { continue };
                    if let (Some(a), Some(z)) = (tick_of(a), tick_of(z)) {
                        p.buttons.push((bit, a, z, 0));
                    }
                } else if let Some(a) = tick_of(at) {
                    p.buttons.push((bit, a, a + 6, 0));
                }
            } else if let Some(rest) = w.strip_prefix("every:") {
                let Some((bp, at)) = rest.split_once('@') else { continue };
                let Some((b, period)) = bp.split_once(':') else { continue };
                let Some((a, z)) = at.split_once('-') else { continue };
                if let (Some(bit), Ok(period), Some(a), Some(z)) =
                    (button(b), period.parse::<u32>(), tick_of(a), tick_of(z))
                {
                    p.buttons.push((bit, a, z, period.max(8)));
                }
            }
        }
        p.end = end.unwrap_or(600);
        p
    }

    /// The bench's own buttons (`every:`) held at `tick`: a press of four ticks each period.
    pub fn every_held(&self, tick: u32) -> u32 {
        self.buttons
            .iter()
            .filter(|&&(_, a, z, period)| period > 0 && tick >= a && tick < z && (tick - a) % period < 4)
            .fold(0, |m, b| m | b.0)
    }

    /// Every button held at `tick` (holds, presses and `every:`): a PC driver's.
    pub fn held(&self, tick: u32) -> u32 {
        self.buttons
            .iter()
            .filter(|&&(_, a, z, period)| tick >= a && tick < z && (period == 0 || (tick - a) % period < 4))
            .fold(0, |m, b| m | b.0)
    }

    /// The stick's `stick:` lean at `tick`.
    pub fn stick(&self, tick: u32) -> Option<Angle> {
        self.sticks.iter().find(|&&(_, a, b)| tick >= a && tick < b).map(|&(d, _, _)| Angle::from_degrees(d))
    }

    /// The units to make at `tick`: one a tick of each `spawn:` running.
    pub fn spawns_at(&self, tick: u32) -> impl Iterator<Item = &str> + '_ {
        self.spawns.iter().filter(move |s| tick >= s.tick && tick < s.tick + s.n).map(|s| s.unit.as_str())
    }

    /// The `at:` travel due at `tick`.
    pub fn travel_at(&self, tick: u32) -> Option<&Travel> {
        self.travels.iter().find(|t| t.tick == tick)
    }

    /// The sim commands due at `tick`: `tough`'s health every [`TOUGH_EVERY`] ticks, one unit a
    /// tick of each `spawn:` running. Dev commands, sent through the sim's input as any command is.
    pub fn commands_at(&self, tick: u32) -> Vec<Command> {
        let mut out = Vec::new();
        if self.tough && tick % TOUGH_EVERY == 0 {
            out.push(Command::Dev(DevOp::Hp(i32::MAX)));
        }
        let cat = jane_data::catalog();
        out.extend(self.spawns_at(tick).filter_map(|u| cat.combat.unit_id(u)).map(|d| Command::Dev(DevOp::Spawn(d))));
        out
    }

    /// The travel `t` as the sim takes one (her seat's `travel`, as a script's stand sets it):
    /// to the free cell nearest `t.cell` in the zone, else to the mark.
    pub fn travel_request(sim: &Sim, t: &Travel) -> Option<TravelRequest> {
        let mark = sim.view(Seat(0))?.sym(&t.mark)?;
        Some(TravelRequest { zone: t.zone, mark, at: t.cell.map(|(x, y)| jane_core::num::Vec2::centre(x, y)) })
    }

    /// The `tp:` due at `tick`.
    pub fn tp_at(&self, tick: u32) -> Option<(ZoneId, &str)> {
        self.tps.iter().find(|t| t.2 == tick).map(|t| (t.0, t.1.as_str()))
    }

    /// The route in force at `tick` (the last begun), and its index.
    fn route_at(&self, tick: u32) -> Option<(usize, &Route)> {
        self.routes.iter().enumerate().filter(|(_, r)| r.from <= tick).max_by_key(|(_, r)| r.from)
    }
}

/// A route's progress: which cell is next, since when, and when she was last sent.
///
/// A route is walked by the sim's own click to move (`Command::Goto`, PLAY-PLAN Â§2.1): her path
/// is the sim's A* round what is in the way, decks and ledges too, on any platform, and the
/// search is part of what the scene measures. The stick is left alone meanwhile (a held move
/// cancels a Goto).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Steer {
    route: Option<usize>,
    next: usize,
    back: bool,
    since: u32,
    sent: Option<(usize, u32)>,
}

/// Within this many cells (each way) of a route's cell is reached it.
const REACHED: i32 = 2;
/// A route's cell not reached in this many ticks is given up (no way there).
const GIVE_UP: u32 = 1200;
/// The Goto sent again this often while she walks to one cell (one a path cut short ended).
const RESEND: u32 = 120;

impl Steer {
    /// The click to move to send at `tick` with her at `her` (a cell of her zone), if a route is
    /// in force, not done, and its cell is new or due again.
    pub fn goto(&mut self, plan: &Plan, tick: u32, her: (i32, i32)) -> Option<Command> {
        let (k, r) = plan.route_at(tick)?;
        if self.route != Some(k) {
            *self = Steer { route: Some(k), since: tick, ..Steer::default() };
        }
        let (x, y) = self.toward(r, tick, her)?;
        if self.sent.is_some_and(|(n, at)| n == self.next && tick.wrapping_sub(at) < RESEND) {
            return None;
        }
        self.sent = Some((self.next, tick));
        Some(Command::Goto(jane_sim::input::Goto::Ground(jane_core::num::Vec2::centre(x, y))))
    }

    /// The route's next cell not yet reached.
    fn toward(&mut self, r: &Route, tick: u32, her: (i32, i32)) -> Option<(i32, i32)> {
        let n = r.cells.len();
        for _ in 0..=2 * n {
            if self.next >= n {
                return None;
            }
            let (x, y) = r.cells[self.next];
            let (dx, dy) = (x - her.0, y - her.1);
            if (dx.abs() > REACHED || dy.abs() > REACHED) && tick.wrapping_sub(self.since) < GIVE_UP {
                return Some((x, y));
            }
            // Reached (or given up): the next cell, turning at the ends of a loop.
            self.since = tick;
            if r.looped && n > 1 {
                if self.back && self.next == 0 {
                    self.back = false;
                } else if !self.back && self.next + 1 == n {
                    self.back = true;
                }
                self.next = if self.back { self.next - 1 } else { self.next + 1 };
            } else {
                self.next += 1;
            }
        }
        None
    }
}

/// One frame's measures (what the PC does not have is `None` or 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    /// The frame's whole time, microseconds (the loop's period on a PSP).
    pub us: u32,
    /// Within the scene's settle: not counted in its rates.
    pub settling: bool,
    pub missed: u32,
    /// The flip's vblanks (0 on a PC).
    pub vblanks: u32,
    /// Ground in view still flat swatches.
    pub swatch: bool,
    /// Lists the display list's guard cut.
    pub sheds: u32,
    /// Free RAM and the largest block, bytes, when read this frame.
    pub mem: Option<(u32, u32)>,
    pub casters: u32,
    pub sprites: u32,
}

/// A scene's measures as it runs.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub frames: u32,
    times: Vec<u32>,
    us: u64,
    pub worst_us: u32,
    pub missed: u32,
    pub flip_max: u32,
    pub swatch: u32,
    pub free_min: Option<u32>,
    pub largest_min: Option<u32>,
    pub sheds: u32,
    pub paint_fails: u32,
    pub sim_worst_us: u32,
    pub hangs: u32,
    pub casters_max: u32,
    pub sprites_max: u32,
    /// Where she was at the end: `zone:x,y`.
    pub at: String,
}

/// PASS, WARN or FAIL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

impl Verdict {
    pub const fn name(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Warn => "WARN",
            Verdict::Fail => "FAIL",
        }
    }
}

impl Stats {
    pub fn frame(&mut self, s: &Sample) {
        if let Some((free, largest)) = s.mem {
            self.free_min = Some(self.free_min.map_or(free, |m| m.min(free)));
            self.largest_min = Some(self.largest_min.map_or(largest, |m| m.min(largest)));
        }
        self.sheds += s.sheds;
        if s.settling {
            return;
        }
        self.frames += 1;
        self.times.push(s.us);
        self.us += u64::from(s.us);
        self.worst_us = self.worst_us.max(s.us);
        self.missed += s.missed;
        self.flip_max = self.flip_max.max(s.vblanks);
        self.swatch += u32::from(s.swatch);
        self.casters_max = self.casters_max.max(s.casters);
        self.sprites_max = self.sprites_max.max(s.sprites);
    }

    /// A sim step's time.
    pub fn tick(&mut self, us: u32) {
        self.sim_worst_us = self.sim_worst_us.max(us);
    }

    /// Frames a second, tenths.
    pub fn fps10(&self) -> u32 {
        (u64::from(self.frames) * 10_000_000 / self.us.max(1)) as u32
    }

    /// The 1% low: the frame rate at the 99th percentile frame time (the frames slower than it
    /// are the slowest hundredth), tenths.
    pub fn low10(&self) -> u32 {
        let mut t = self.times.clone();
        t.sort_unstable_by(|a, b| b.cmp(a));
        t.get(t.len() / 100).map_or(0, |&us| 10_000_000 / us.max(1))
    }

    /// The verdict against the thresholds (a locked 30 is the PSP's aim; a PC's uncapped rates
    /// pass them easily) and the reasons for anything short of PASS.
    pub fn verdict(&self) -> (Verdict, String) {
        let mut v = Verdict::Pass;
        let mut why = String::new();
        let mut flag = |level: Verdict, bad: bool, what: &str| {
            if bad {
                v = v.max(level);
                if !why.is_empty() {
                    why.push(',');
                }
                why.push_str(what);
            }
        };
        let (fps, low) = (self.fps10(), self.low10());
        let n = self.frames.max(1);
        flag(Verdict::Fail, self.hangs > 0, "hang");
        flag(Verdict::Fail, self.paint_fails > 0, "paint_fail");
        flag(Verdict::Fail, self.frames == 0, "no_frames");
        flag(Verdict::Fail, self.worst_us >= 1_000_000, "worst>=1s");
        flag(Verdict::Fail, fps < 200, "fps<20");
        flag(Verdict::Fail, low < 100, "low1<10");
        flag(Verdict::Fail, self.largest_min.is_some_and(|b| b < 256 * 1024), "largest<256K");
        flag(Verdict::Warn, (200..280).contains(&fps), "fps<28");
        flag(Verdict::Warn, (100..200).contains(&low), "low1<20");
        flag(Verdict::Warn, (200_000..1_000_000).contains(&self.worst_us), "worst>=200ms");
        flag(Verdict::Warn, self.missed * 20 > n, "missed>5%");
        flag(Verdict::Warn, self.swatch * 10 > n, "swatch>10%");
        flag(Verdict::Warn, self.sheds > 0, "list_shed");
        flag(Verdict::Warn, self.sim_worst_us >= 16_000, "sim>=16ms");
        flag(Verdict::Warn, self.largest_min.is_some_and(|b| (256 * 1024..1024 * 1024).contains(&b)), "largest<1M");
        (v, why)
    }

    /// The scene's line in `bench.txt`: `key value` pairs, `-` where the platform has no measure.
    pub fn line(&self, name: &str) -> String {
        let (v, why) = self.verdict();
        let kb = |b: Option<u32>| b.map_or(String::from("-"), |b| alloc::format!("{}", b / 1024));
        let (fps, low) = (self.fps10(), self.low10());
        let mut l = String::with_capacity(256);
        let _ = write!(
            l,
            "scene {name} fps {}.{} low1 {}.{} worst_ms {} missed {} flip_max {} swatch {} frames {} free_kb {} largest_kb {} sheds {} paint_fail {} sim_worst_ms {}.{} hangs {} casters {} sprites {} at {} verdict {}",
            fps / 10,
            fps % 10,
            low / 10,
            low % 10,
            self.worst_us / 1000,
            self.missed,
            self.flip_max,
            self.swatch,
            self.frames,
            kb(self.free_min),
            kb(self.largest_min),
            self.sheds,
            self.paint_fails,
            self.sim_worst_us / 1000,
            self.sim_worst_us / 100 % 10,
            self.hangs,
            self.casters_max,
            self.sprites_max,
            if self.at.is_empty() { "-" } else { &self.at },
            v.name()
        );
        if !why.is_empty() {
            let _ = write!(l, " why {why}");
        }
        l
    }
}

/// `bench.txt`'s first line: the build and how it ran.
pub fn header(platform: &str, commit: &str, settings: &str) -> String {
    alloc::format!("bench v1 platform {platform} commit {commit} seed {SEED} settings {}", settings.trim())
}

/// A `bench.txt` line's `key value` pairs (a `scene` line's or the header's).
pub fn fields(line: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut it = line.split_whitespace();
    while let (Some(k), Some(v)) = (it.next(), it.next()) {
        out.push((k, v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_parses_to_a_plan_with_an_end_and_a_start() {
        for s in &SCENES {
            let p = Plan::parse(s.words);
            assert!(p.end >= 1800, "{}: {} ticks", s.name, p.end);
            assert!(!p.travels.is_empty() || !p.tps.is_empty(), "{} goes nowhere", s.name);
            for sp in &p.spawns {
                assert!(jane_data::catalog().combat.unit_id(&sp.unit).is_some(), "{}: no unit {}", s.name, sp.unit);
            }
        }
        let total: u32 = SCENES.iter().map(|s| Plan::parse(s.words).end).sum();
        assert!(total / TICKS_PER_SECOND < 6 * 60, "the bench runs {} s", total / TICKS_PER_SECOND);
    }

    #[test]
    fn the_words_read_as_meant() {
        let p =
            Plan::parse("at:cellar:stair_a:18,33@p5 spawn:rat:3@p20 every:cross:20@p50-p100 loop:1,2/3,4 tough 900");
        assert_eq!(p.end, 900);
        assert!(p.tough);
        assert_eq!(p.commands_at(20).len(), 2, "health and a rat");
        assert_eq!(
            p.travels[0],
            Travel { zone: ZoneId::Cellar, mark: "stair_a".into(), cell: Some((18, 33)), tick: 5 }
        );
        assert_eq!(p.spawns_at(19).count(), 0);
        assert_eq!(p.spawns_at(22).count(), 1);
        assert_eq!(p.spawns_at(23).count(), 0);
        assert_eq!(p.every_held(50), psp::CROSS);
        assert_eq!(p.every_held(54), 0);
        assert_eq!(p.every_held(70), psp::CROSS);
        assert!(p.routes[0].looped);
    }

    #[test]
    fn a_loop_turns_at_its_ends() {
        let to = |x: i32| Some(Command::Goto(jane_sim::input::Goto::Ground(jane_core::num::Vec2::centre(x, 0))));
        let p = Plan::parse("loop:10,0/20,0 100");
        let mut s = Steer::default();
        assert_eq!(s.goto(&p, 1, (0, 0)), to(10));
        assert_eq!(s.goto(&p, 2, (5, 0)), None, "sent, not due again");
        assert_eq!(s.goto(&p, 3, (10, 0)), to(20));
        assert_eq!(s.goto(&p, 4, (20, 0)), to(10), "turned at the end");
        assert_eq!(s.goto(&p, 5, (10, 0)), to(20), "and at the start");
        assert_eq!(s.goto(&p, 5 + RESEND, (15, 0)), to(20), "sent again");
        let r = Plan::parse("route:10,0 100");
        let mut s = Steer::default();
        assert_eq!(s.goto(&r, 1, (10, 0)), None);
    }

    #[test]
    fn a_line_says_its_verdict_and_reads_back() {
        let mut st = Stats::default();
        for k in 0..300 {
            st.frame(&Sample { us: if k == 7 { 250_000 } else { 33_333 }, ..Sample::default() });
        }
        let l = st.line("x");
        let f = fields(&l);
        assert_eq!(f[0], ("scene", "x"));
        assert!(f.contains(&("verdict", "WARN")), "{l}");
        assert!(f.contains(&("why", "worst>=200ms")), "{l}");
        st.hangs = 1;
        assert_eq!(st.verdict().0, Verdict::Fail);
    }
}
