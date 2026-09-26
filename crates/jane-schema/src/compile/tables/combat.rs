//! Compile the `combat` group into [`model::Combat`]: spells, effects, units, items, recipes.
//!
//! Every check `validateCatalog` made for these tables is made here, plus the ones the port
//! adds (ARCHITECTURE.md §6): phases in falling `hpBelow`, a ground spell's pool only on a ground
//! spell, `dayOnly` and `nightOnly` never together. Units convert as the model's doc comments say.

use serde::Deserialize;

use jane_core::Angle;
use jane_core::action::School;
use jane_core::ids::{ItemId, SpellId, UnitDefId};
use jane_core::num::{Fx, Milli, Permille, Tick};

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::fraction::Num;
use crate::compile::lists::{self, RawAction, RawSchool, RawStat};
use crate::compile::source::{Source, typed};
use crate::model::{
    self, BoltSplash, BossPhase, BySchool, CastAnim, Controller, EffectDef, EffectPulse, Faction, GroundPool, ItemDef,
    LootRoll, OnMelee, RecipeDef, ScheduleRow, ScheduleSlot, SnakeBody, SpellDef, SpellKind, SpellPower, UnitDef,
    UnitGlow, UnitSight, WorldSpell,
};

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Combat {
    let items = items(src, cx);
    let recipes = recipes(src, cx);
    let spells = spells(src, cx);
    let effects = effects(src, cx);
    let units = units(src, cx);
    model::Combat { spells, effects, units, items, recipes }
}

/// Rows in id order. A row that failed to type leaves a hole; the diagnostics already say why
/// and no catalog is produced, so the table is left empty rather than padded.
fn placed<T>(rows: Vec<Option<T>>) -> &'static [T] {
    let all: Option<Vec<T>> = rows.into_iter().collect();
    leak(all.unwrap_or_default())
}

/// A unit conversion's value, or its default after recording the error at `at.field`.
fn conv<T: Default>(cx: &mut Ctx, at: &str, field: &str, r: Result<T, String>) -> T {
    r.unwrap_or_else(|e| {
        cx.diag.error(format!("{at}.{field}"), e);
        T::default()
    })
}

/// A divisor content writes as a number (`1.25`), in thousandths (1250).
fn thousandths(n: Num) -> Result<u32, String> {
    let m = n.milli()?.0;
    u32::try_from(m).map_err(|_| format!("{m} thousandths is not a divisor"))
}

/// `"#rrggbb"` to `0xRRGGBB`.
fn rgb(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// A per-school table as content writes it: `{"frost": 0.5}`. An unknown school is a serde error.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawBySchool {
    heal: Option<Num>,
    physical: Option<Num>,
    frost: Option<Num>,
    fire: Option<Num>,
    nature: Option<Num>,
    blast: Option<Num>,
    shock: Option<Num>,
}

impl RawBySchool {
    /// In `School` order, each school left out as `default`.
    fn compile(&self, cx: &mut Ctx, at: &str, default: Permille) -> BySchool {
        let given = [self.heal, self.physical, self.frost, self.fire, self.nature, self.blast, self.shock];
        let mut out = [default; 7];
        for (slot, (school, n)) in out.iter_mut().zip(School::ALL.iter().zip(given)) {
            if let Some(n) = n {
                *slot = conv(cx, at, &format!("resist.{}", school_name(*school)), n.permille());
            }
        }
        out
    }
}

fn school_name(s: School) -> &'static str {
    match s {
        School::Heal => "heal",
        School::Physical => "physical",
        School::Frost => "frost",
        School::Fire => "fire",
        School::Nature => "nature",
        School::Blast => "blast",
        School::Shock => "shock",
    }
}

// --- spells ---------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawPower {
    stat: RawStat,
    div: Num,
    var_div: Num,
    flat: Option<Num>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSplash {
    radius: Num,
    div: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSpell {
    name: String,
    description: String,
    icon: String,
    kind: SpellKind,
    school: RawSchool,
    mp: Num,
    energy: Num,
    range: Num,
    cooldown: Num,
    gcd_immune: bool,
    needs_target: bool,
    needs_enemy: bool,
    needs_los: bool,
    anim: CastAnim,
    power: Option<RawPower>,
    speed: Option<Num>,
    count: Option<u8>,
    fan: Option<Num>,
    splash: Option<RawSplash>,
    effect: Option<String>,
    restore_energy: Option<Num>,
    radius: Option<Num>,
    duration: Option<Num>,
    pulse: Option<Num>,
    delay: Option<Num>,
    world: Option<WorldSpell>,
    /// Ticks, as written.
    stop: Option<u32>,
    glow: Option<Num>,
    touch: Option<Num>,
}

fn spells(src: &Source, cx: &mut Ctx) -> &'static [SpellDef] {
    let rows = src.table("spells", &mut cx.diag);
    let mut out: Vec<Option<SpellDef>> = vec![None; cx.ids.spells.len()];
    for (id, row) in &rows {
        let Some(r) = typed::<RawSpell>(row, &format!("spells.{id}"), &mut cx.diag) else { continue };
        let def = spell(cx, &format!("{}: spells.{id}", row.file), id, &r);
        out[cx.ids.spells.get(id).map_or(0, usize::from)] = Some(def);
    }
    placed(out)
}

fn spell(cx: &mut Ctx, at: &str, id: &str, r: &RawSpell) -> SpellDef {
    let kind = r.kind;
    for (field, n) in [
        ("range", r.range),
        ("mp", r.mp),
        ("energy", r.energy),
        ("restoreEnergy", r.restore_energy.unwrap_or(Num::from_int(0))),
    ] {
        cx.diag.need(!n.is_negative(), format!("{at}.{field}"), "negative number");
    }

    // catalog.ts: melee and bolt need power, and a power's divisors are > 0.
    cx.diag.need(!matches!(kind, SpellKind::Melee | SpellKind::Bolt) || r.power.is_some(), at, "needs power");
    let power = r.power.as_ref().map(|p| {
        let div = conv(cx, at, "power.div", thousandths(p.div));
        let var_div = conv(cx, at, "power.varDiv", thousandths(p.var_div));
        cx.diag.need(div > 0 && var_div > 0, format!("{at}.power"), "power divisors must be > 0");
        let flat = p.flat.map(|f| conv(cx, at, "power.flat", f.milli())).unwrap_or_default();
        let stat = match p.stat {
            RawStat::Strength => jane_core::action::Stat::Strength,
            RawStat::Spirit => jane_core::action::Stat::Spirit,
        };
        SpellPower { stat, div, var_div, flat }
    });

    // A bolt needs speed; touch is a bolt's.
    cx.diag.need(
        kind != SpellKind::Bolt || r.speed.is_some_and(Num::is_positive),
        format!("{at}.speed"),
        "bolt needs speed",
    );
    cx.diag.need(r.speed.is_none_or(|s| !s.is_negative()), format!("{at}.speed"), "negative number");
    let speed = r.speed.map(|s| conv(cx, at, "speed", s.fx_px()));
    if let Some(t) = r.touch {
        cx.diag.need(kind == SpellKind::Bolt && t.is_positive(), format!("{at}.touch"), "touch is a bolt's, in px");
    }
    let touch = r.touch.map(|t| conv(cx, at, "touch", t.fx_px()));

    // Ground: the pool, and only on a ground spell.
    let ground = if kind == SpellKind::Ground {
        cx.diag.need(
            r.radius.is_some_and(Num::is_positive) && r.duration.is_some_and(Num::is_positive),
            at,
            "ground needs radius and duration",
        );
        let zero = Num::from_int(0);
        Some(GroundPool {
            radius: conv(cx, at, "radius", r.radius.unwrap_or(zero).fx_metres()),
            duration: conv(cx, at, "duration", r.duration.unwrap_or(zero).ticks()),
            pulse: r.pulse.map_or(Tick(30), |p| conv(cx, at, "pulse", p.ticks())),
            delay: r.delay.map_or(Tick(1), |d| Tick(conv(cx, at, "delay", d.ticks()).0.max(1))),
        })
    } else {
        cx.diag.need(
            r.radius.is_none() && r.duration.is_none() && r.pulse.is_none() && r.delay.is_none(),
            at,
            "radius, duration, pulse and delay are a ground spell's",
        );
        None
    };

    // World: the verb, and only on a world spell.
    if kind == SpellKind::World {
        cx.diag.need(r.world.is_some(), at, "world spell needs a verb");
    } else {
        cx.diag.need(r.world.is_none(), format!("{at}.world"), "a verb is a world spell's");
    }

    cx.diag.need(r.count != Some(0), format!("{at}.count"), "count is at least 1");
    cx.diag.need(r.fan.is_none_or(|f| !f.is_negative()), format!("{at}.fan"), "negative number");
    let splash = r.splash.as_ref().map(|s| {
        cx.diag.need(
            s.div >= 1 && s.radius.is_positive(),
            format!("{at}.splash"),
            "splash needs a radius and div >= 1",
        );
        BoltSplash { radius: conv(cx, at, "splash.radius", s.radius.fx_px()), div: s.div }
    });

    SpellDef {
        id: leak_str(id),
        name: cx.text(&r.name),
        description: cx.text(&r.description),
        icon: cx.sprite(&r.icon),
        kind,
        school: r.school.into(),
        mp: conv(cx, at, "mp", r.mp.milli()),
        energy: conv(cx, at, "energy", r.energy.milli()),
        range: conv(cx, at, "range", r.range.fx_metres()),
        cooldown: conv(cx, at, "cooldown", r.cooldown.ticks()),
        gcd_immune: r.gcd_immune,
        needs_target: r.needs_target,
        needs_enemy: r.needs_enemy,
        needs_los: r.needs_los,
        anim: r.anim,
        power,
        speed,
        count: r.count.unwrap_or(1),
        fan: r.fan.map_or(Angle(0), |f| conv(cx, at, "fan", f.angle())),
        splash,
        effect: r.effect.as_deref().and_then(|e| cx.effect(&format!("{at}.effect"), e)),
        restore_energy: r.restore_energy.map(|e| conv(cx, at, "restoreEnergy", e.milli())).unwrap_or_default(),
        ground,
        world: r.world,
        stop: Tick(r.stop.unwrap_or(0)),
        glow: r.glow.map(|g| conv(cx, at, "glow", g.fx_px())),
        touch,
    }
}

// --- effects --------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPulse {
    amount: Num,
    every: Num,
    school: RawSchool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOnMelee {
    school: RawSchool,
    amount: Num,
    effect: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawEffect {
    name: String,
    icon: String,
    duration: Num,
    harmful: bool,
    speed: Option<Num>,
    #[serde(default)]
    stun: bool,
    pulse: Option<RawPulse>,
    heal: Option<Num>,
    mana: Option<Num>,
    resist: Option<RawBySchool>,
    mana_shield: Option<Num>,
    lifesteal: Option<Num>,
    crit_one_in: Option<u16>,
    on_melee: Option<RawOnMelee>,
    mana_on_hit: Option<Num>,
    only_if_weak: Option<RawSchool>,
    #[serde(default)]
    no_resist: bool,
}

fn effects(src: &Source, cx: &mut Ctx) -> &'static [EffectDef] {
    let rows = src.table("effects", &mut cx.diag);
    let mut out: Vec<Option<EffectDef>> = vec![None; cx.ids.effects.len()];
    for (id, row) in &rows {
        let Some(r) = typed::<RawEffect>(row, &format!("effects.{id}"), &mut cx.diag) else { continue };
        let def = effect(cx, &format!("{}: effects.{id}", row.file), id, &r);
        out[cx.ids.effects.get(id).map_or(0, usize::from)] = Some(def);
    }
    placed(out)
}

fn effect(cx: &mut Ctx, at: &str, id: &str, r: &RawEffect) -> EffectDef {
    // Optional amounts: absent is 0, which the sim reads as "none" (the TypeScript's `if (def.heal)`).
    let opt = |cx: &mut Ctx, field: &str, n: Option<Num>, f: fn(Num) -> Result<Permille, String>| {
        n.map(|n| conv(cx, at, field, f(n))).unwrap_or_default()
    };
    let lifesteal = opt(cx, "lifesteal", r.lifesteal, Num::permille);
    let mana_shield = opt(cx, "manaShield", r.mana_shield, Num::permille);
    let milli = |cx: &mut Ctx, field: &str, n: Option<Num>| -> Milli {
        n.map(|n| conv(cx, at, field, n.milli())).unwrap_or_default()
    };
    let heal = milli(cx, "heal", r.heal);
    let mana = milli(cx, "mana", r.mana);
    let mana_on_hit = milli(cx, "manaOnHit", r.mana_on_hit);
    for (field, n) in [
        ("lifesteal", r.lifesteal),
        ("manaShield", r.mana_shield),
        ("heal", r.heal),
        ("mana", r.mana),
        ("manaOnHit", r.mana_on_hit),
    ] {
        cx.diag.need(n.is_none_or(|n| !n.is_negative()), format!("{at}.{field}"), "negative number");
    }

    cx.diag.need(
        r.speed.is_none_or(|s| !s.is_negative() && s.at_most_one()),
        format!("{at}.speed"),
        "speed is a movement multiplier, 0..1",
    );
    let speed = r.speed.map_or(Permille::ONE, |s| conv(cx, at, "speed", s.permille()));

    let pulse = r.pulse.as_ref().map(|p| EffectPulse {
        amount: conv(cx, at, "pulse.amount", p.amount.milli()),
        // catalog.ts: a pulse comes at most once a tick.
        every: Tick(conv(cx, at, "pulse.every", p.every.ticks()).0.max(1)),
        school: p.school.into(),
    });

    let on_melee = r.on_melee.as_ref().map(|m| OnMelee {
        school: m.school.into(),
        amount: conv(cx, at, "onMelee.amount", m.amount.milli()),
        effect: m.effect.as_deref().and_then(|e| cx.effect(&format!("{at}.onMelee.effect"), e)),
    });

    cx.diag.need(r.only_if_weak != Some(RawSchool::Heal), format!("{at}.onlyIfWeak"), "bad onlyIfWeak school \"heal\"");

    EffectDef {
        id: leak_str(id),
        name: cx.text(&r.name),
        icon: cx.sprite(&r.icon),
        duration: conv(cx, at, "duration", r.duration.ticks()),
        harmful: r.harmful,
        speed,
        stun: r.stun,
        pulse,
        heal,
        mana,
        resist: r.resist.as_ref().map_or([Permille::ONE; 7], |b| b.compile(cx, at, Permille::ONE)),
        mana_shield,
        lifesteal,
        crit_one_in: r.crit_one_in.unwrap_or(0),
        on_melee,
        mana_on_hit,
        only_if_weak: r.only_if_weak.map(School::from),
        no_resist: r.no_resist,
    }
}

// --- units ----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLoot {
    item: String,
    qty: u16,
    chance: Num,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGlow {
    radius: Num,
    color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBody {
    segments: u16,
    spacing: Num,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawPhase {
    hp_below: Num,
    book: Vec<String>,
    run: Option<Num>,
    on_enter: Option<Vec<RawAction>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawUnit {
    name: String,
    faction: Faction,
    controller: Controller,
    strength: u16,
    spirit: u16,
    walk: Num,
    run: Num,
    aggro: Num,
    leash: Num,
    bounds: Option<Num>,
    book: Vec<String>,
    respawn: Num,
    auto_regen: bool,
    loot: Vec<RawLoot>,
    resist: Option<RawBySchool>,
    sprite: String,
    on_death: Option<Vec<RawAction>>,
    talk: Option<String>,
    #[serde(default)]
    boss: bool,
    glow: Option<RawGlow>,
    #[serde(default)]
    day_only: bool,
    day_only_after: Option<String>,
    #[serde(default)]
    night_only: bool,
    bait: Option<String>,
    body: Option<RawBody>,
    #[serde(default)]
    phases: Vec<RawPhase>,
    sight: Option<UnitSight>,
    #[serde(default)]
    shuns_light: bool,
    #[serde(default)]
    hunts: Vec<String>,
    #[serde(default)]
    flees: Vec<String>,
    #[serde(default)]
    schedule: Vec<RawSlot>,
}

/// One row of a unit's hours (ARCHITECTURE.md §4.6.a): `{"from": 9, "to": 21, "mark": "arms_front"}`,
/// or `"inside": "<prop>"`, `"patrol": true`, `"absent": true`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    from: u8,
    to: u8,
    mark: Option<String>,
    inside: Option<String>,
    #[serde(default)]
    patrol: bool,
    #[serde(default)]
    absent: bool,
}

/// The unit rows' hours, compiled after every group that names a mark or a prop (the chunks):
/// interning a mark's name with the units would move every name interned after it, and with them
/// every blueprint's keys (`compile::build_source` calls this after the chunks).
pub fn late_schedules(src: &Source, cx: &mut Ctx, units: &'static [UnitDef]) -> &'static [UnitDef] {
    let rows = src.table("units", &mut crate::compile::diag::Diagnostics::default());
    let mut out = units.to_vec();
    for (id, row) in &rows {
        let Ok(r) = serde_json::from_value::<RawUnit>(row.value.clone()) else { continue };
        let (Some(i), false) = (cx.ids.units.get(id), r.schedule.is_empty()) else { continue };
        if let Some(def) = out.get_mut(usize::from(i)) {
            def.schedule = schedule(cx, &format!("{}: units.{id}", row.file), &r);
        }
    }
    leak(out)
}

/// A schedule: every hour of the day in exactly one row, each row exactly one slot; never with
/// `dayOnly` or `nightOnly`, which are its shorthand.
fn schedule(cx: &mut Ctx, at: &str, r: &RawUnit) -> &'static [ScheduleRow] {
    if r.schedule.is_empty() {
        return &[];
    }
    let at = format!("{at}.schedule");
    cx.diag.need(!r.day_only && !r.night_only, &at, "a schedule says the hours itself: no dayOnly or nightOnly");
    let mut hours = [0u8; 24];
    let mut out = Vec::with_capacity(r.schedule.len());
    for (n, s) in r.schedule.iter().enumerate() {
        let at = format!("{at}[{n}]");
        if s.from >= 24 || s.to >= 24 {
            cx.diag.error(&at, "hours are 0..=23");
            continue;
        }
        let given = usize::from(s.mark.is_some())
            + usize::from(s.inside.is_some())
            + usize::from(s.patrol)
            + usize::from(s.absent);
        cx.diag.need(given == 1, &at, "a slot is exactly one of mark, inside, patrol, absent");
        let slot = if let Some(m) = &s.mark {
            ScheduleSlot::Mark(cx.name(m))
        } else if let Some(p) = &s.inside {
            ScheduleSlot::Inside(cx.name(p))
        } else if s.patrol {
            ScheduleSlot::Patrol
        } else {
            ScheduleSlot::Absent
        };
        for (h, n) in hours.iter_mut().enumerate() {
            if model::in_span(h as u8, s.from, s.to) {
                *n += 1;
            }
        }
        out.push(ScheduleRow { hour_from: s.from, hour_to: s.to, slot });
    }
    for (h, n) in hours.iter().enumerate() {
        match n {
            1 => {}
            0 => cx.diag.error(&at, format!("{h:02}:00 is in no row: every hour needs a slot (WORLD.md §10.2)")),
            _ => cx.diag.error(&at, format!("{h:02}:00 is in {n} rows")),
        }
    }
    leak(out)
}

fn units(src: &Source, cx: &mut Ctx) -> &'static [UnitDef] {
    let rows = src.table("units", &mut cx.diag);
    let mut out: Vec<Option<UnitDef>> = vec![None; cx.ids.units.len()];
    for (id, row) in &rows {
        let Some(r) = typed::<RawUnit>(row, &format!("units.{id}"), &mut cx.diag) else { continue };
        let def = unit(cx, &format!("{}: units.{id}", row.file), id, &r);
        out[cx.ids.units.get(id).map_or(0, usize::from)] = Some(def);
    }
    placed(out)
}

fn book(cx: &mut Ctx, at: &str, ids: &[String]) -> &'static [SpellId] {
    leak(ids.iter().filter_map(|s| cx.spell(at, s)).collect())
}

fn unit_refs(cx: &mut Ctx, at: &str, ids: &[String]) -> &'static [UnitDefId] {
    leak(ids.iter().filter_map(|s| cx.unit(at, s)).collect())
}

fn unit(cx: &mut Ctx, at: &str, id: &str, r: &RawUnit) -> UnitDef {
    cx.diag.need(r.strength > 0, format!("{at}.strength"), "strength must be > 0");
    // What bites looks only at the party's bodies for a target (sim/ai.ts nearestEnemy): a
    // friendly row that fights would be invisible to it.
    cx.diag.need(
        r.faction != Faction::Friendly || matches!(r.controller, Controller::Npc | Controller::Player),
        at,
        "a friendly unit is a person or the player (npc or player controller)",
    );
    for (field, n) in [("walk", r.walk), ("run", r.run), ("aggro", r.aggro), ("leash", r.leash)] {
        cx.diag.need(!n.is_negative(), format!("{at}.{field}"), "negative number");
    }
    cx.diag.need(r.bounds.is_none_or(Num::is_positive), format!("{at}.bounds"), "bounds must be > 0");

    let loot = r
        .loot
        .iter()
        .enumerate()
        .filter_map(|(n, l)| {
            let at = format!("{at}.loot[{n}]");
            cx.diag.need(l.qty >= 1, &at, "qty < 1");
            cx.diag.need(
                l.chance.is_positive() && l.chance.at_most_one(),
                &at,
                "chance is a fraction, above 0 and at most 1",
            );
            let chance = conv(cx, &at, "chance", l.chance.permille());
            let item = cx.item(&at, &l.item)?;
            Some(LootRoll { item, qty: l.qty, chance })
        })
        .collect();

    let glow = r.glow.as_ref().map(|g| {
        let color = rgb(&g.color).unwrap_or_else(|| {
            cx.diag.error(format!("{at}.glow.color"), format!("\"{}\" is not a colour \"#rrggbb\"", g.color));
            0
        });
        cx.diag.need(g.radius.is_positive(), format!("{at}.glow.radius"), "radius must be > 0");
        UnitGlow { radius: conv(cx, at, "glow.radius", g.radius.fx_px()), color }
    });

    // The snake has a body; nothing else does.
    let snake = r.controller == Controller::Snake;
    if snake {
        cx.diag.need(r.body.is_some(), at, "snake controller needs body");
    } else {
        cx.diag.need(r.body.is_none(), format!("{at}.body"), "a body is a snake controller's");
    }
    let body = r.body.as_ref().map(|b| {
        cx.diag.need(
            b.segments >= 1 && b.spacing.is_positive(),
            format!("{at}.body"),
            "body needs segments and spacing",
        );
        SnakeBody { segments: b.segments, spacing: conv(cx, at, "body.spacing", b.spacing.fx_px()) }
    });

    let mut phases = Vec::with_capacity(r.phases.len());
    for (n, p) in r.phases.iter().enumerate() {
        let pat = format!("{at}.phases[{n}]");
        cx.diag.need(
            p.hp_below.is_positive() && p.hp_below.at_most_one(),
            &pat,
            format!("phase {n}: hpBelow is a fraction of full health"),
        );
        let phase = BossPhase {
            hp_below: conv(cx, &pat, "hpBelow", p.hp_below.permille()),
            book: book(cx, &format!("{pat}.book"), &p.book),
            run: p.run.map(|v| conv(cx, &pat, "run", v.fx_px())),
            on_enter: lists::list(cx, &format!("{pat}.onEnter"), p.on_enter.as_deref()),
        };
        cx.diag.need(p.run.is_none_or(|v| !v.is_negative()), format!("{pat}.run"), "negative number");
        // ARCHITECTURE.md §6: phases in falling hpBelow. The snake reads its table by its own clock.
        if let Some(prev) = phases.last().map(|q: &BossPhase| q.hp_below) {
            cx.diag.need(snake || phase.hp_below < prev, &pat, "phases fall in hpBelow");
        }
        phases.push(phase);
    }

    cx.diag.need(!(r.day_only && r.night_only), at, "a unit cannot be both dayOnly and nightOnly");
    cx.diag.need(
        r.day_only_after.is_none() || r.day_only,
        format!("{at}.dayOnlyAfter"),
        "dayOnlyAfter without dayOnly",
    );

    UnitDef {
        id: leak_str(id),
        name: cx.text(&r.name),
        faction: r.faction,
        controller: r.controller,
        strength: r.strength,
        spirit: r.spirit,
        walk: conv(cx, at, "walk", r.walk.fx_px()),
        run: conv(cx, at, "run", r.run.fx_px()),
        aggro: conv(cx, at, "aggro", r.aggro.fx_metres()),
        leash: conv(cx, at, "leash", r.leash.fx_metres()),
        bounds: r.bounds.map_or(Fx(jane_core::num::METRE_FX), |b| conv(cx, at, "bounds", b.fx_metres())),
        book: book(cx, &format!("{at}.book"), &r.book),
        respawn: conv(cx, at, "respawn", r.respawn.ticks()),
        auto_regen: r.auto_regen,
        loot: leak(loot),
        resist: r.resist.as_ref().map_or([Permille::ZERO; 7], |b| b.compile(cx, at, Permille::ZERO)),
        sprite: cx.sprite(&r.sprite),
        on_death: lists::list(cx, &format!("{at}.onDeath"), r.on_death.as_deref()),
        talk: r.talk.as_deref().and_then(|t| cx.dialogue(&format!("{at}.talk"), t)),
        boss: r.boss,
        glow,
        day_only: r.day_only,
        day_only_after: r.day_only_after.as_deref().and_then(|q| cx.quest(&format!("{at}.dayOnlyAfter"), q)),
        night_only: r.night_only,
        bait: r.bait.as_deref().and_then(|b| cx.item(&format!("{at}.bait"), b)),
        body,
        phases: leak(phases),
        sight: r.sight.unwrap_or(UnitSight::Any),
        shuns_light: r.shuns_light,
        // Named by `late_schedules`, once the chunks have named the marks and doors.
        schedule: &[],
        hunts: unit_refs(cx, &format!("{at}.hunts"), &r.hunts),
        flees: unit_refs(cx, &format!("{at}.flees"), &r.flees),
    }
}

// --- items and recipes ----------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawItem {
    name: String,
    description: String,
    icon: String,
    max_stack: u16,
    usable: bool,
    cooldown: Num,
    #[serde(rename = "use")]
    use_list: Option<Vec<RawAction>>,
    #[serde(default)]
    keep: bool,
    opens: Option<String>,
    #[serde(default)]
    bound: bool,
}

/// Items a quest asks the party to acquire, read from the quests' raw rows: they are story items.
fn acquired(src: &Source, cx: &mut Ctx) -> Vec<String> {
    let mut quiet = crate::compile::diag::Diagnostics::default();
    let mut out = Vec::new();
    for row in src.table("quests", &mut quiet).values() {
        for req in row.value.get("requirements").and_then(|r| r.as_array()).into_iter().flatten() {
            if req.get("type").and_then(|t| t.as_str()) == Some("acquire") {
                if let Some(t) = req.get("target").and_then(|t| t.as_str()) {
                    out.push(t.to_owned());
                }
            }
        }
    }
    let _ = cx;
    out
}

fn items(src: &Source, cx: &mut Ctx) -> &'static [ItemDef] {
    let rows = src.table("items", &mut cx.diag);
    let acquired = acquired(src, cx);
    let mut out: Vec<Option<ItemDef>> = vec![None; cx.ids.items.len()];
    for (id, row) in &rows {
        let at = format!("items.{id}");
        let Some(r) = typed::<RawItem>(row, &at, &mut cx.diag) else { continue };
        let cooldown = r.cooldown.ticks().map_err(|e| cx.diag.error(&at, e)).unwrap_or_default();
        cx.diag.need(r.max_stack >= 1, &at, "maxStack < 1");
        cx.diag.need(
            !r.usable || r.use_list.is_some() || r.opens.is_some(),
            &at,
            "usable but has neither use[] nor opens",
        );
        let def = ItemDef {
            id: leak_str(id),
            name: cx.text(&r.name),
            description: cx.text(&r.description),
            icon: cx.sprite(&r.icon),
            max_stack: r.max_stack,
            usable: r.usable,
            cooldown,
            use_list: lists::list(cx, &format!("{at}.use"), r.use_list.as_deref()),
            keep: r.keep,
            opens: r.opens.as_deref().map(|o| cx.name(o)),
            bound: r.bound,
            story: r.opens.is_some() || acquired.iter().any(|a| a == id),
        };
        let i = cx.ids.items.get(id).map_or(0, usize::from);
        out[i] = Some(def);
    }
    // A row that failed to type leaves a hole; the diagnostics already say why, and no catalog
    // is produced, so the hole is never read.
    leak(out.into_iter().map(|d| d.unwrap_or_else(placeholder_item)).collect())
}

fn placeholder_item() -> ItemDef {
    ItemDef {
        id: "",
        name: jane_core::TextId(0),
        description: jane_core::TextId(0),
        icon: jane_core::SpriteId(0),
        max_stack: 1,
        usable: false,
        cooldown: jane_core::Tick(0),
        use_list: None,
        keep: false,
        opens: None,
        bound: false,
        story: false,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    inputs: Vec<String>,
    output: String,
    qty: u16,
}

fn recipes(src: &Source, cx: &mut Ctx) -> &'static [RecipeDef] {
    let rows = src.list("recipes", &mut cx.diag);
    let mut out: Vec<RecipeDef> = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let at = format!("recipes[{n}]");
        let Some(r) = typed::<RawRecipe>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need((1..=3).contains(&r.inputs.len()), &at, "1..3 inputs");
        cx.diag.need(r.qty >= 1, &at, "qty < 1");
        let mut inputs: Vec<ItemId> = r.inputs.iter().filter_map(|i| cx.item(&at, i)).collect();
        inputs.sort();
        let Some(output) = cx.item(&at, &r.output) else { continue };
        if out.iter().any(|o| o.inputs == inputs.as_slice()) {
            cx.diag.error(&at, format!("duplicate recipe {}", r.inputs.join("+")));
        }
        out.push(RecipeDef { inputs: leak(inputs), output, qty: r.qty });
    }
    leak(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ctx::Ids;
    use jane_core::action::{Action, Stat};
    use jane_core::ids::EffectId;
    use jane_core::num::METRE_FX;

    const ITEMS: &str = r#"{"key": {"name": "Key", "description": "A key.", "icon": "item_key",
        "maxStack": 1, "usable": false, "cooldown": 0}}"#;

    const SPELLS: &str = r#"{
      "bolt": {"name": "Bolt", "description": "d", "icon": "spell_frost", "kind": "bolt", "school": "frost",
        "mp": 4, "energy": 0, "range": 15, "cooldown": 1.5, "gcdImmune": false, "needsTarget": true,
        "needsEnemy": true, "needsLos": true, "anim": "cast",
        "power": {"stat": "spirit", "div": 1.25, "varDiv": 32, "flat": 2},
        "speed": 2.5, "count": 15, "fan": 360, "splash": {"radius": 25, "div": 5}, "effect": "chill",
        "glow": 20, "touch": 28, "stop": 24},
      "pool": {"name": "Pool", "description": "d", "icon": "spell_nature", "kind": "ground", "school": "nature",
        "mp": 0, "energy": 0, "range": 10, "cooldown": 6, "gcdImmune": false, "needsTarget": false,
        "needsEnemy": false, "needsLos": false, "anim": "cast", "radius": 1.5, "duration": 2.5},
      "swing": {"name": "Swing", "description": "d", "icon": "spell_melee", "kind": "melee", "school": "physical",
        "mp": 0, "energy": 0, "range": 0.25, "cooldown": 1, "gcdImmune": true, "needsTarget": true,
        "needsEnemy": true, "needsLos": true, "anim": "attack", "restoreEnergy": 3,
        "power": {"stat": "strength", "div": 8, "varDiv": 32}}
    }"#;

    const EFFECTS: &str = r#"{
      "chill": {"name": "Chilled", "icon": "fx_chilled", "duration": 4, "harmful": true, "speed": 0.5,
        "resist": {"physical": 0.5, "fire": 2}, "pulse": {"amount": 4, "every": 0.001, "school": "frost"}},
      "fury": {"name": "Fury", "icon": "fx_fury", "duration": 0, "harmful": false, "lifesteal": 0.3,
        "critOneIn": 3, "manaShield": 1, "onMelee": {"school": "fire", "amount": 6, "effect": "chill"}}
    }"#;

    const UNITS: &str = r##"{
      "foreman": {"name": "Foreman", "faction": "bandit", "controller": "ai", "strength": 20, "spirit": 3,
        "walk": 0.55, "run": 1.2, "aggro": 16, "leash": 80, "book": ["swing"], "respawn": 600,
        "autoRegen": true, "loot": [{"item": "key", "qty": 1, "chance": 0.4}],
        "resist": {"shock": -0.5, "frost": 0.8}, "sprite": "foreman", "boss": true, "sight": "lit",
        "glow": {"radius": 24, "color": "#ff2020"},
        "phases": [{"hpBelow": 0.75, "book": ["bolt"], "run": 1.1, "onEnter": [{"do": "give", "item": "key"}]},
                   {"hpBelow": 0.25, "book": ["swing", "bolt"]}]},
      "snake": {"name": "Snake", "faction": "undead", "controller": "snake", "strength": 30, "spirit": 3,
        "walk": 1, "run": 2, "aggro": 12, "leash": 60, "bounds": 0.5, "book": [], "respawn": 0,
        "autoRegen": false, "loot": [], "sprite": "snake", "body": {"segments": 64, "spacing": 3},
        "phases": [{"hpBelow": 1, "book": ["bolt"]}, {"hpBelow": 1, "book": ["pool"]}], "hunts": ["foreman"]}
    }"##;

    fn build(files: &[(&str, &str)]) -> (model::Combat, Ctx) {
        let src = Source::from_files(files).unwrap();
        let mut cx = Ctx::default();
        cx.ids = Ids::collect(&src, &mut cx.diag);
        let combat = compile(&src, &mut cx);
        (combat, cx)
    }

    fn good() -> (model::Combat, Ctx) {
        build(&[("items.json", ITEMS), ("spells.json", SPELLS), ("effects.json", EFFECTS), ("units.json", UNITS)])
    }

    fn errors(cx: &Ctx) -> Vec<String> {
        cx.diag.errors.iter().map(ToString::to_string).collect()
    }

    /// Some error names `at` and says `msg`.
    fn has(errs: &[String], at: &str, msg: &str) -> bool {
        errs.iter().any(|e| e.contains(&format!("{at}:")) && e.contains(msg))
    }

    #[test]
    fn spells_convert_to_their_units() {
        let (c, cx) = good();
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        let bolt = c.spell(SpellId(0));
        assert_eq!(bolt.id, "bolt");
        assert_eq!(bolt.kind, SpellKind::Bolt);
        assert_eq!(bolt.school, School::Frost);
        assert_eq!(bolt.mp, Milli(4000));
        assert_eq!(bolt.range, Fx(15 * METRE_FX));
        assert_eq!(bolt.cooldown, Tick(90));
        assert_eq!(bolt.speed, Some(Fx(640)), "2.5 px per tick");
        assert_eq!(bolt.count, 15);
        assert_eq!(bolt.fan, Angle(u16::MAX), "360 degrees is the ring");
        assert_eq!(bolt.splash, Some(BoltSplash { radius: Fx(25 * 256), div: 5 }));
        assert_eq!(bolt.effect, Some(EffectId(0)));
        assert_eq!(bolt.glow, Some(Fx(20 * 256)));
        assert_eq!(bolt.touch, Some(Fx(28 * 256)));
        assert_eq!(bolt.stop, Tick(24), "stop is written in ticks");
        assert_eq!(bolt.power, Some(SpellPower { stat: Stat::Spirit, div: 1250, var_div: 32000, flat: Milli(2000) }));
        assert_eq!((bolt.ground, bolt.world), (None, None));

        // catalog.ts: a ground radius in metres, a lifetime in seconds; combat.ts pulses every 30 ticks by default.
        let pool = c.spell(SpellId(1));
        assert_eq!(
            pool.ground,
            Some(GroundPool { radius: Fx(3072), duration: Tick(150), pulse: Tick(30), delay: Tick(1) })
        );
        assert_eq!(pool.count, 1);
        assert_eq!(pool.fan, Angle(0));

        let swing = c.spell(SpellId(2));
        assert_eq!(swing.range, Fx(512), "a quarter metre");
        assert_eq!(swing.restore_energy, Milli(3000));
        assert_eq!(swing.stop, Tick(0));
        assert_eq!(swing.power.map(|p| p.flat), Some(Milli(0)));
    }

    #[test]
    fn effects_convert_to_their_units() {
        let (c, cx) = good();
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        let chill = c.effect(EffectId(0));
        assert_eq!(chill.duration, Tick(240));
        assert_eq!(chill.speed, Permille(500));
        assert_eq!(chill.resist_of(School::Physical), Permille(500));
        assert_eq!(chill.resist_of(School::Fire), Permille(2000));
        assert_eq!(chill.resist_of(School::Frost), Permille::ONE, "a school left out is no change");
        // catalog.ts buildCatalog: pulse.every = Math.max(1, seconds(every)).
        assert_eq!(chill.pulse, Some(EffectPulse { amount: Milli(4000), every: Tick(1), school: School::Frost }));

        let fury = c.effect(EffectId(1));
        assert_eq!(fury.duration, Tick(0));
        assert_eq!(fury.speed, Permille::ONE, "no speed row is no change");
        assert_eq!(fury.lifesteal, Permille(300));
        assert_eq!(fury.mana_shield, Permille(1000));
        assert_eq!(fury.crit_one_in, 3);
        assert_eq!(
            fury.on_melee,
            Some(OnMelee { school: School::Fire, amount: Milli(6000), effect: Some(EffectId(0)) })
        );
        assert_eq!(fury.resist, [Permille::ONE; 7]);
    }

    #[test]
    fn units_convert_to_their_units() {
        let (c, cx) = good();
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        let f = c.unit(UnitDefId(0));
        assert_eq!(f.id, "foreman");
        assert_eq!((f.walk, f.run), (Fx(141), Fx(307)), "ARCHITECTURE.md §2: .55 -> 141, 1.2 -> 307");
        assert_eq!((f.aggro, f.leash), (Fx(16 * METRE_FX), Fx(80 * METRE_FX)));
        assert_eq!(f.bounds, Fx(METRE_FX), "bounds ?? 1");
        assert_eq!(f.respawn, Tick(36000));
        assert_eq!(f.book, &[SpellId(2)]);
        assert_eq!(f.loot, &[LootRoll { item: ItemId(0), qty: 1, chance: Permille(400) }]);
        assert_eq!(f.resist_of(School::Shock), Permille(-500), "weak to shock");
        assert_eq!(f.resist_of(School::Frost), Permille(800));
        assert_eq!(f.resist_of(School::Fire), Permille(0), "a school left out is no reduction");
        assert_eq!(f.glow, Some(UnitGlow { radius: Fx(24 * 256), color: 0xff2020 }));
        assert_eq!(f.sight, UnitSight::Lit);
        assert_eq!(f.phases.len(), 2);
        assert_eq!(f.phases[0].hp_below, Permille(750));
        assert_eq!(f.phases[0].run, Some(Fx(282)));
        assert_eq!(f.phases[1].book, &[SpellId(2), SpellId(0)], "a book keeps its written order");
        let Some(jane_core::ListRef::Catalog(l)) = f.phases[0].on_enter else { panic!("onEnter is a list") };
        assert!(matches!(cx.lists[usize::from(l)][..], [Action::Give(_)]));

        let s = c.unit(UnitDefId(1));
        assert_eq!(s.body, Some(SnakeBody { segments: 64, spacing: Fx(768) }));
        assert_eq!(s.bounds, Fx(1024));
        assert_eq!(s.sight, UnitSight::Any);
        assert_eq!(s.hunts, &[UnitDefId(0)]);
        assert!(s.schedule.is_empty() && s.flees.is_empty());
    }

    #[test]
    fn every_spell_check_of_validate_catalog() {
        let bad = r#"{
          "a": {"name": "A", "description": "d", "icon": "i", "kind": "bolt", "school": "frost", "mp": -1,
            "energy": 0, "range": 1, "cooldown": 1, "gcdImmune": false, "needsTarget": true, "needsEnemy": true,
            "needsLos": true, "anim": "cast", "effect": "nope"},
          "b": {"name": "B", "description": "d", "icon": "i", "kind": "ground", "school": "nature", "mp": 0,
            "energy": 0, "range": 1, "cooldown": 1, "gcdImmune": false, "needsTarget": true, "needsEnemy": true,
            "needsLos": true, "anim": "cast", "radius": 1},
          "c": {"name": "C", "description": "d", "icon": "i", "kind": "world", "school": "nature", "mp": 0,
            "energy": 0, "range": 1, "cooldown": 1, "gcdImmune": false, "needsTarget": true, "needsEnemy": true,
            "needsLos": true, "anim": "cast", "touch": 14},
          "d": {"name": "D", "description": "d", "icon": "i", "kind": "melee", "school": "physical", "mp": 0,
            "energy": 0, "range": 1, "cooldown": 1, "gcdImmune": false, "needsTarget": true, "needsEnemy": true,
            "needsLos": true, "anim": "attack", "power": {"stat": "strength", "div": 0, "varDiv": 2}, "duration": 2},
          "e": {"name": "E", "description": "d", "icon": "i", "kind": "melee", "school": "physical", "mp": 0,
            "energy": 0, "range": 1, "cooldown": 1, "gcdImmune": false, "needsTarget": true, "needsEnemy": true,
            "needsLos": true, "anim": "attack"}
        }"#;
        let (_, cx) = build(&[("spells.json", bad)]);
        let e = errors(&cx);
        assert!(has(&e, "spells.a.mp", "negative number"), "{e:?}");
        assert!(has(&e, "spells.a.effect", "unknown effect \"nope\""), "{e:?}");
        assert!(has(&e, "spells.a", "needs power"), "{e:?}");
        assert!(has(&e, "spells.a.speed", "bolt needs speed"), "{e:?}");
        assert!(has(&e, "spells.b", "ground needs radius and duration"), "{e:?}");
        assert!(has(&e, "spells.c", "world spell needs a verb"), "{e:?}");
        assert!(has(&e, "spells.c.touch", "touch is a bolt's"), "{e:?}");
        assert!(has(&e, "spells.d.power", "power divisors must be > 0"), "{e:?}");
        assert!(has(&e, "spells.d", "a ground spell's"), "{e:?}");
        assert!(has(&e, "spells.e", "needs power"), "{e:?}");
        assert_eq!(e.len(), 10, "{e:?}");

        // A bad kind, school or anim: the enums refuse it, naming the row.
        let typo = SPELLS.replace("\"kind\": \"melee\"", "\"kind\": \"punch\"");
        let (_, cx) = build(&[("items.json", ITEMS), ("spells.json", &typo), ("effects.json", EFFECTS)]);
        assert!(has(&errors(&cx), "spells.swing", "punch"), "{}", cx.diag);
    }

    #[test]
    fn every_effect_check_of_validate_catalog() {
        let bad = r#"{
          "a": {"name": "A", "icon": "i", "duration": 1, "harmful": true, "onlyIfWeak": "heal",
            "onMelee": {"school": "fire", "amount": 1, "effect": "nope"}, "speed": 1.5},
          "b": {"name": "B", "icon": "i", "duration": 1, "harmful": true, "resist": {"cold": 0.5}},
          "c": {"name": "C", "icon": "i", "duration": 1, "harmful": true,
            "pulse": {"amount": 1, "every": 1, "school": "wind"}}
        }"#;
        let (_, cx) = build(&[("effects.json", bad)]);
        let e = errors(&cx);
        assert!(has(&e, "effects.a.onlyIfWeak", "bad onlyIfWeak school"), "{e:?}");
        assert!(has(&e, "effects.a.onMelee.effect", "unknown effect \"nope\""), "{e:?}");
        assert!(has(&e, "effects.a.speed", "0..1"), "{e:?}");
        assert!(has(&e, "effects.b", "cold"), "a bad resist school: {e:?}");
        assert!(has(&e, "effects.c", "wind"), "a bad pulse school: {e:?}");
        assert_eq!(e.len(), 5, "{e:?}");
    }

    #[test]
    fn every_unit_check_of_validate_catalog() {
        let bad = r#"{
          "a": {"name": "A", "faction": "friendly", "controller": "ai", "strength": 0, "spirit": 1, "walk": 1,
            "run": 1, "aggro": 0, "leash": 0, "book": ["nope"], "respawn": 0, "autoRegen": true,
            "loot": [{"item": "gone", "qty": 1, "chance": 1}], "sprite": "s", "talk": "nobody",
            "bait": "worm", "onDeath": [{"do": "learn", "spell": "nope"}], "glow": {"radius": 3, "color": "red"},
            "phases": [{"hpBelow": 0.5, "book": ["nada"], "onEnter": [{"do": "quest", "quest": "q"}]},
                       {"hpBelow": 0.75, "book": []}, {"hpBelow": 0, "book": []}]},
          "b": {"name": "B", "faction": "undead", "controller": "snake", "strength": 1, "spirit": 1, "walk": 1,
            "run": 1, "aggro": 0, "leash": 0, "book": [], "respawn": 0, "autoRegen": true, "loot": [],
            "sprite": "s", "dayOnly": true, "nightOnly": true, "dayOnlyAfter": "q"},
          "c": {"name": "C", "faction": "undead", "controller": "ai", "strength": 1, "spirit": 1, "walk": 1,
            "run": 1, "aggro": 0, "leash": 0, "book": [], "respawn": 0, "autoRegen": true, "loot": [],
            "sprite": "s", "sight": "dark"},
          "d": {"name": "D", "faction": "undead", "controller": "ai", "strength": 1, "spirit": 1, "walk": 1,
            "run": 1, "aggro": 0, "leash": 0, "book": [], "respawn": 0, "autoRegen": true, "loot": [],
            "sprite": "s", "resist": {"poison": 1}}
        }"#;
        let (_, cx) = build(&[("units.json", bad)]);
        let e = errors(&cx);
        assert!(has(&e, "units.a.strength", "strength must be > 0"), "{e:?}");
        assert!(has(&e, "units.a", "a friendly unit is a person or the player"), "{e:?}");
        assert!(has(&e, "units.a.book", "unknown spell \"nope\""), "{e:?}");
        assert!(has(&e, "units.a.phases[0].book", "unknown spell \"nada\""), "{e:?}");
        assert!(has(&e, "units.a.phases[0].onEnter[0]", "unknown quest \"q\""), "{e:?}");
        assert!(has(&e, "units.a.phases[1]", "phases fall in hpBelow"), "{e:?}");
        assert!(has(&e, "units.a.phases[2]", "hpBelow is a fraction of full health"), "{e:?}");
        assert!(has(&e, "units.a.loot[0]", "unknown item \"gone\""), "{e:?}");
        assert!(has(&e, "units.a.talk", "unknown dialogue \"nobody\""), "{e:?}");
        assert!(has(&e, "units.a.bait", "unknown item \"worm\""), "{e:?}");
        assert!(has(&e, "units.a.onDeath[0]", "unknown spell \"nope\""), "{e:?}");
        assert!(has(&e, "units.a.glow.color", "not a colour"), "{e:?}");
        assert!(has(&e, "units.b", "snake controller needs body"), "{e:?}");
        assert!(has(&e, "units.b", "both dayOnly and nightOnly"), "{e:?}");
        assert!(has(&e, "units.b.dayOnlyAfter", "unknown quest \"q\""), "{e:?}");
        assert!(has(&e, "units.c", "dark"), "sight is \"lit\" or nothing: {e:?}");
        assert!(has(&e, "units.d", "poison"), "a bad resist school: {e:?}");
        assert_eq!(e.len(), 17, "{e:?}");
    }

    #[test]
    fn colours_and_divisors() {
        assert_eq!(rgb("#ffd070"), Some(0xffd070));
        assert_eq!(rgb("#FFD070"), Some(0xffd070));
        assert_eq!(rgb("ffd070"), None);
        assert_eq!(rgb("#+fd070"), None);
        assert_eq!(rgb("#ffd07"), None);
        assert_eq!(thousandths(Num::from_int(64)), Ok(64000));
        assert!(thousandths(Num::from_int(-1)).is_err());
    }

    /// ARCHITECTURE.md §4.6.a: a row's hours cover the day once, each a single slot, and never
    /// beside `dayOnly` / `nightOnly`; the names are interned only in the late pass.
    #[test]
    fn schedules_cover_the_day_once() {
        let person = |extra: &str| {
            format!(
                r#"{{"cobb": {{"name": "Mr Cobb", "faction": "friendly", "controller": "npc", "strength": 10, "spirit": 1,
                  "walk": 0.35, "run": 0.7, "aggro": 0, "leash": 0, "book": [], "respawn": 0, "autoRegen": true,
                  "loot": [], "sprite": "s", {extra}}}}}"#
            )
        };
        let late = |extra: &str| {
            let units = person(extra);
            let src = Source::from_files(&[("units.json", &units)]).unwrap();
            let mut cx = Ctx::default();
            cx.ids = Ids::collect(&src, &mut cx.diag);
            let c = compile(&src, &mut cx);
            assert!(c.units[0].schedule.is_empty(), "nothing named before the late pass");
            let names = cx.names.len();
            let units = late_schedules(&src, &mut cx, c.units);
            (units[0].schedule, names, cx)
        };
        let ok = r#""schedule": [{"from": 21, "to": 6, "inside": "arms_door"}, {"from": 6, "to": 9, "mark": "yard"},
                     {"from": 9, "to": 21, "patrol": true}]"#;
        let (rows, before, cx) = late(ok);
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        assert_eq!(before, 0);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].slot, ScheduleSlot::Inside(jane_core::ids::NameId(0)));
        assert_eq!(rows[1].slot, ScheduleSlot::Mark(jane_core::ids::NameId(1)));
        assert_eq!((rows[2].hour_from, rows[2].hour_to, rows[2].slot), (9, 21, ScheduleSlot::Patrol));

        let gap = r#""schedule": [{"from": 6, "to": 21, "patrol": true}, {"from": 22, "to": 6, "absent": true}]"#;
        assert!(has(&errors(&late(gap).2), "units.cobb.schedule", "21:00 is in no row"));
        let twice = r#""schedule": [{"from": 0, "to": 0, "patrol": true}, {"from": 22, "to": 6, "absent": true}]"#;
        assert!(has(&errors(&late(twice).2), "units.cobb.schedule", "22:00 is in 2 rows"));
        let two = r#""schedule": [{"from": 0, "to": 0, "patrol": true, "absent": true}]"#;
        assert!(has(&errors(&late(two).2), "units.cobb.schedule[0]", "exactly one of mark, inside, patrol, absent"));
        let hour = r#""schedule": [{"from": 0, "to": 24, "patrol": true}]"#;
        assert!(has(&errors(&late(hour).2), "units.cobb.schedule[0]", "hours are 0..=23"));
        let sugar = r#""dayOnly": true, "schedule": [{"from": 0, "to": 0, "patrol": true}]"#;
        assert!(has(&errors(&late(sugar).2), "units.cobb.schedule", "no dayOnly or nightOnly"));
    }
}
