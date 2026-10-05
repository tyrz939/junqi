//! Play telemetry (PLAY-PLAN.md 0.1): what a run of a model was like, as CSV tables a person
//! can chart. The overnight gameplay audit's harness, kept: every frame it reads the sim's own
//! events and her view and writes down
//!
//! - every kill, with the blows she landed on it (each damage event: splash and burning count);
//! - every blow she took, and its size as a share of her max health;
//! - every heal, and whether food went down with it;
//! - every rest;
//! - a line a minute: health, and the seconds moving, fighting, talking, standing and dead;
//! - the bot's milestones, and the minute each chapter's quest was handed in;
//! - every death: what last hurt her, the zone, the cell and the minute.
//!
//! It only looks: nothing here presses or changes anything, so a run with telemetry steps and
//! hashes as one without. Minutes are frames played / 3600 (the clock jumps when she sleeps;
//! the frames do not). Everything is integers, in frame order, so two runs write the same bytes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use jane_sim::event::{Event, EventKind};
use jane_sim::ids::UnitId;
use jane_sim::{Seat, Sim, View};

use crate::{Bot, Mark, Model, sense};

/// Frames in a minute of play.
const MINUTE: u32 = 60 * 60;
/// A frame within this many of a blow given or taken is a frame spent fighting.
const FIGHTING_FOR: u32 = 120;
/// The columns every event table starts with.
const RUN: &str = "model,seed,frame,min";

/// The story's chapters, by the quest whose hand-in ends each (the Prologue ends at the rats).
pub const CHAPTERS: [(&str, &str); 8] = [
    ("prologue", "rats_below"),
    ("mine", "the_mine"),
    ("museum", "the_museum"),
    ("forest", "the_forest"),
    ("factory", "the_factory"),
    ("burial", "the_burial"),
    ("school", "the_school"),
    ("choice", "the_choice"),
];

/// Her blows on one foe so far.
#[derive(Debug, Default)]
struct OnFoe {
    events: u32,
    dmg: i64,
    /// Damage events by school.
    schools: BTreeMap<&'static str, u32>,
}

/// One minute's tally, in frames.
#[derive(Debug, Default)]
struct Minute {
    frames: u32,
    hp_sum: u64,
    hp_min: u32,
    moving: u32,
    fighting: u32,
    talking: u32,
    idle: u32,
    dead: u32,
}

/// A run's telemetry, gathered frame by frame.
#[derive(Debug)]
pub struct Telemetry {
    pub model: String,
    pub seed: u32,
    seat: Seat,
    frame: u32,
    on: BTreeMap<UnitId, OnFoe>,
    last_hurt: Option<&'static str>,
    last_fight: Option<u32>,
    last_pos: Option<jane_core::Vec2>,
    food: u32,
    minute: Minute,
    pub kills: Vec<String>,
    pub blows: Vec<String>,
    pub heals: Vec<String>,
    pub rests: Vec<String>,
    pub minutes: Vec<String>,
    pub deaths: Vec<String>,
    /// The biggest blow, per mille of her max health, and what dealt it.
    pub worst: (u32, &'static str),
    pub food_heals: u32,
    dmg_taken: i64,
    /// Fire chores (`jane_sim::fire`): fires she lit, deadwood gathered, lightings the rain refused.
    pub fires_lit: u32,
    pub gathers: u32,
    pub wet: u32,
}

fn csv(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_owned() }
}

/// Tenths, written `12.3`.
fn tenths(per_mille: u32) -> String {
    format!("{}.{}", per_mille / 10, per_mille % 10)
}

fn food(v: &View<'_>) -> u32 {
    ["apple", "grape"].into_iter().map(|n| sense::holds(v, sense::item(n))).sum()
}

impl Telemetry {
    pub fn new(model: &str, seed: u32) -> Telemetry {
        Telemetry {
            model: model.to_owned(),
            seed,
            seat: Seat(0),
            frame: 0,
            on: BTreeMap::new(),
            last_hurt: None,
            last_fight: None,
            last_pos: None,
            food: 0,
            minute: Minute { hp_min: 100, ..Minute::default() },
            kills: Vec::new(),
            blows: Vec::new(),
            heals: Vec::new(),
            rests: Vec::new(),
            minutes: Vec::new(),
            deaths: Vec::new(),
            worst: (0, ""),
            food_heals: 0,
            dmg_taken: 0,
            fires_lit: 0,
            gathers: 0,
            wet: 0,
        }
    }

    fn head(&self) -> String {
        format!("{},{},{},{}", self.model, self.seed, self.frame, self.frame / MINUTE)
    }

    /// One frame played: `events` are the step's (all of them; hers are picked out).
    pub fn observe(&mut self, sim: &Sim, events: &[Event]) {
        self.frame += 1;
        let Some(v) = sim.view(self.seat) else { return };
        let cat = jane_data::catalog();
        let me = v.body();
        let my = me.id;
        let max = jane_sim::units::max_hp(me).points().max(1);
        let zone = v.zone().name();
        let food_now = food(&v);
        let mut fought = false;
        for e in events.iter().filter(|e| e.to.is_none_or(|s| s == self.seat)) {
            match e.kind {
                EventKind::Damage { unit, from, amount, school, .. } if unit == my => {
                    fought = true;
                    let by = from.and_then(|f| v.unit(f)).map_or("?", |u| cat.combat.unit(u.def).id);
                    self.last_hurt = Some(by);
                    let amount = amount.points();
                    self.dmg_taken += i64::from(amount);
                    let pm = (amount.max(0) as u32) * 1000 / max as u32;
                    if pm > self.worst.0 {
                        self.worst = (pm, by);
                    }
                    self.blows.push(format!(
                        "{},{zone},{by},{},{amount},{},{max},{}",
                        self.head(),
                        school_name(school),
                        me.hp.points() + amount,
                        tenths(pm)
                    ));
                }
                EventKind::Damage { unit, from: Some(f), amount, school, .. } if f == my => {
                    fought = true;
                    let o = self.on.entry(unit).or_default();
                    o.events += 1;
                    o.dmg += i64::from(amount.points());
                    *o.schools.entry(school_name(school)).or_insert(0) += 1;
                }
                EventKind::Heal { unit, amount, .. } if unit == my => {
                    let ate = food_now < self.food;
                    self.food_heals += u32::from(ate);
                    self.heals.push(format!(
                        "{},{zone},{},{max},{},{},{}",
                        self.head(),
                        amount.points(),
                        u8::from(ate),
                        self.food,
                        food_now
                    ));
                }
                EventKind::Death { unit, def, .. } if unit != my => {
                    if let Some(o) = self.on.remove(&unit) {
                        let row = cat.combat.unit(def);
                        let mix: Vec<String> = o.schools.iter().map(|(k, n)| format!("{k}:{n}")).collect();
                        self.kills.push(format!(
                            "{},{zone},{},{},{},{},{},{},{max},{}",
                            self.head(),
                            row.id,
                            u8::from(row.boss),
                            o.events,
                            o.dmg,
                            me.strength,
                            me.spirit,
                            mix.join(" ")
                        ));
                    }
                }
                EventKind::PlayerDied => {
                    let (x, y) = me.pos.cell();
                    self.deaths.push(format!(
                        "{},{zone},{x},{y},{},{max},{},{}",
                        self.head(),
                        self.last_hurt.take().unwrap_or("?"),
                        food_now,
                        v.hour()
                    ));
                }
                EventKind::Rest => {
                    self.rests.push(format!(
                        "{},{zone},{},{max},{},{food_now}",
                        self.head(),
                        me.hp.points(),
                        v.clock().1
                    ));
                }
                EventKind::Toast(jane_sim::event::ToastKind::FireLit { by, .. }) if by == self.seat => {
                    self.fires_lit += 1;
                }
                EventKind::Toast(jane_sim::event::ToastKind::FireWants(jane_sim::event::FireWant::Wet)) => {
                    self.wet += 1;
                }
                EventKind::Loot { item, .. } if Some(item) == jane_sim::fire::deadwood() => self.gathers += 1,
                _ => {}
            }
        }
        // A foe left behind in another zone is not a kill to come: its tally goes when she does.
        if events.iter().any(|e| matches!(e.kind, EventKind::Zone { .. })) {
            self.on.clear();
        }
        self.food = food_now;
        if fought {
            self.last_fight = Some(self.frame);
        }
        // The minute's line.
        let m = &mut self.minute;
        m.frames += 1;
        let hp = (me.hp.points().max(0) as u32 * 100 / max as u32).min(100);
        m.hp_sum += u64::from(hp);
        m.hp_min = m.hp_min.min(hp);
        if !me.alive {
            m.dead += 1;
        } else if v.me().dialogue.is_some() {
            m.talking += 1;
        } else if self.last_fight.is_some_and(|f| self.frame - f <= FIGHTING_FOR) {
            m.fighting += 1;
        } else if self.last_pos.is_some_and(|p| p != me.pos) {
            m.moving += 1;
        } else {
            m.idle += 1;
        }
        self.last_pos = Some(me.pos);
        if self.frame % MINUTE == 0 {
            let line = format!(
                "{},{},{},{zone},{},{},{},{},{},{},{},{},{},{max},{food_now}",
                self.model,
                self.seed,
                self.frame / MINUTE - 1,
                m.hp_sum / u64::from(m.frames.max(1)),
                m.hp_min,
                m.moving / 60,
                m.fighting / 60,
                m.talking / 60,
                m.idle / 60,
                m.dead / 60,
                me.strength,
                me.spirit
            );
            self.minutes.push(line);
            self.minute = Minute { hp_min: 100, ..Minute::default() };
        }
    }

    /// The run's tables, written as `<kind>_<model>_<seed>.csv` in `dir`, with the bot's
    /// milestones and chapter times; returns the summary row (see [`SUMMARY_HEADER`]).
    pub fn write(&self, dir: &Path, bot: &Bot) -> std::io::Result<String> {
        std::fs::create_dir_all(dir)?;
        let name = |kind: &str| dir.join(format!("{kind}_{}_{}.csv", self.model, self.seed));
        let table = |kind: &str, header: &str, rows: &[String]| -> std::io::Result<()> {
            let mut s = String::with_capacity(64 * (rows.len() + 1));
            s.push_str(header);
            s.push('\n');
            for r in rows {
                s.push_str(r);
                s.push('\n');
            }
            std::fs::write(name(kind), s)
        };
        table(
            "kills",
            &format!("{RUN},zone,target,boss,hits,dmg_total,her_str,her_spi,her_maxhp,schools"),
            &self.kills,
        )?;
        table("blows", &format!("{RUN},zone,by,school,amount,her_hp_before,her_maxhp,pct_of_max"), &self.blows)?;
        table("heals", &format!("{RUN},zone,amount,her_maxhp,food,food_before,food_after"), &self.heals)?;
        table("rests", &format!("{RUN},zone,hp,maxhp,day,food"), &self.rests)?;
        table(
            "minutes",
            "model,seed,minute,zone,hp_pct_mean,hp_pct_min,moving_s,fighting_s,talking_s,idle_s,dead_s,her_str,her_spi,her_maxhp,food",
            &self.minutes,
        )?;
        table("deaths", &format!("{RUN},zone,cell_x,cell_y,by,her_maxhp,food,hour"), &self.deaths)?;
        let milestones: Vec<String> = bot
            .log
            .iter()
            .map(|m| format!("{},{},{},{},{}", self.model, self.seed, m.frame, m.frame / MINUTE, csv(&m.line())))
            .collect();
        table("milestones", "model,seed,frame,min,line", &milestones)?;
        let chapters = self.chapters(bot);
        let rows: Vec<String> =
            chapters.iter().map(|(c, done, took)| format!("{},{},{c},{done},{took}", self.model, self.seed)).collect();
        table("chapters", "model,seed,chapter,done_min,minutes", &rows)?;
        let summary = self.summary(bot);
        table("summary", SUMMARY_HEADER, std::slice::from_ref(&summary))?;
        Ok(summary)
    }

    /// Each chapter handed in: (chapter, the minute, the minutes since the last).
    pub fn chapters(&self, bot: &Bot) -> Vec<(&'static str, u32, u32)> {
        let cat = jane_data::catalog();
        let mut out = Vec::new();
        let mut last = 0;
        for (c, q) in CHAPTERS {
            let Some(id) = cat.story.quest_id(q) else { continue };
            let Some(m) = bot.log.iter().find(|m| m.mark == Mark::QuestDone(id)) else { break };
            let at = m.frame / MINUTE;
            out.push((c, at, at - last));
            last = at;
        }
        out
    }

    /// The run in one row.
    pub fn summary(&self, bot: &Bot) -> String {
        let chapters = self.chapters(bot);
        let mins = self.frame / MINUTE;
        let first_death = self.deaths.first().and_then(|d| d.split(',').nth(3)).unwrap_or("");
        let hits: u64 = self.kills.iter().filter_map(|k| k.split(',').nth(7)?.parse::<u64>().ok()).sum();
        let per_kill = if self.kills.is_empty() { 0 } else { hits * 10 / self.kills.len() as u64 };
        let per_hour = |n: usize| if mins == 0 { 0 } else { n as u64 * 600 / u64::from(mins) };
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{mins},{},{},{},{first_death},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.model,
            self.seed,
            chapters.len(),
            chapters.last().map_or("", |c| c.0),
            self.deaths.len(),
            self.kills.len(),
            tenths(per_kill as u32),
            self.blows.len(),
            tenths(self.worst.0),
            self.worst.1,
            self.heals.len(),
            self.food_heals,
            self.rests.len(),
            tenths(per_hour(self.rests.len()) as u32),
            self.fires_lit,
            self.gathers,
            self.wet,
            tenths(per_hour((self.fires_lit + self.gathers) as usize) as u32),
        );
        s
    }
}

/// The story from New Game: `model` on `seed` for up to `minutes` of play (or until it stops),
/// its telemetry gathered as it goes.
pub fn play(model: Model, seed: u32, minutes: u32) -> Result<(Bot, Telemetry), String> {
    play_with(model, seed, minutes, None)
}

/// [`play`], with made fires (`jane_sim::fire`) set on or off for the run (`None`: as New Game
/// has it, `tuning::FIRES_MADE`).
pub fn play_with(model: Model, seed: u32, minutes: u32, fires: Option<bool>) -> Result<(Bot, Telemetry), String> {
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let mut sim = Sim::new_game_with(bps, "Jane");
    let mut bot = Bot::story(model);
    if let Some(on) = fires {
        bot.setup.push(jane_sim::Command::Dev(jane_sim::DevOp::Fires(on)));
    }
    let mut t = Telemetry::new(model.name(), seed);
    for _ in 0..minutes.saturating_mul(MINUTE) {
        if bot.done() {
            break;
        }
        bot.step(&mut sim);
        t.observe(&sim, bot.events());
    }
    Ok((bot, t))
}

/// The summary table's columns: minutes played, chapters handed in and the last, deaths and
/// the first one's minute, kills and her blows per kill, blows taken and the biggest (per cent of
/// her max health, and by what), heals and those that were food, rests and rests an hour.
pub const SUMMARY_HEADER: &str = "model,seed,minutes,chapters,last_chapter,deaths,first_death_min,kills,hits_per_kill,blows_taken,worst_blow_pct,worst_blow_by,heals,food_heals,rests,rests_per_hour,fires_lit,gathers,wet_refusals,chores_per_hour";

fn school_name(s: jane_core::action::School) -> &'static str {
    use jane_core::action::School as S;
    match s {
        S::Heal => "heal",
        S::Physical => "physical",
        S::Frost => "frost",
        S::Fire => "fire",
        S::Nature => "nature",
        S::Blast => "blast",
        S::Shock => "shock",
    }
}
