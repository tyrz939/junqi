//! Compile the living world (ARCHITECTURE.md §4.6, §6; WORLD.md) into [`model::Living`]:
//! `data/weather.json`, `data/ecology.json`, `data/consequences.json` and
//! `data/tuning/sim.json`. Compiled after every other group, so the areas, quests, units and
//! texts it names are already known.
//!
//! The checks (§6 "Schema additions for the living world"):
//!
//! - **weather**: one row per region, in region order; every zone under exactly one sky; in each
//!   season every hour in exactly one band; each band's weights sum to 1000; a kind lasts at least
//!   an hour, `min <= max`.
//! - **ecology**: an area the skeleton has; a unit row that exists, once per area; `cap`,
//!   `weight` and `hold` at least 1.
//! - **consequences**: `on` a flag, a quest done or a named death, never negated; edits drawn from
//!   the world verbs only (`show`, `hide`, `lock`, `unlock`, `nightLock`, `nightUnlock`, `switch`,
//!   `spawn`, `despawn`, `fill`,
//!   `send`, `flag`, through `send`'s `then`: [`Action::is_world_verb`]), and at least one that is
//!   not a flag (WORLD.md §6: a flag alone
//!   fails the cohesion test); a claim confirmed or contradicted is a line some row already says,
//!   and not both. That a flag `on` is set somewhere, and that a `dead` name is provided, needs every
//!   list: `integrate.rs`.
//! - **tuning**: the journal keeps at least one entry per kind; the rain ramp steps at least
//!   every tick.

use serde::Deserialize;

use jane_core::action::{Action, Condition, ListRef};
use jane_core::ids::ZoneId;
use jane_core::num::Tick;

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::fraction::Num;
use crate::compile::lists::{self, RawAction, RawCond};
use crate::compile::source::{Row, Source, typed};
use crate::model::{
    self, ConsequenceDef, County, EcologyDef, Population, Region, RegionWeather, Season, SimTuning, Sky, SkyBand,
    WetnessTuning, in_span,
};

pub fn compile(src: &Source, cx: &mut Ctx, county: &County) -> model::Living {
    let weather = weather(src, cx);
    let ecology = ecology(src, cx, county);
    let consequences = consequences(src, cx);
    let tuning = tuning(src, cx);
    model::Living { weather, ecology, consequences, tuning }
}

/// The three regions, in region order.
const REGIONS: [Region; 3] = [Region::Lowfields, Region::Waters, Region::Works];

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawRegion {
    Lowfields,
    Waters,
    Works,
}

impl From<RawRegion> for Region {
    fn from(r: RawRegion) -> Region {
        match r {
            RawRegion::Lowfields => Region::Lowfields,
            RawRegion::Waters => Region::Waters,
            RawRegion::Works => Region::Works,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The weather

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawSeason {
    Autumn,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBand {
    season: RawSeason,
    from: u8,
    to: u8,
    clear: u16,
    mist: u16,
    rain: u16,
    storm: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLast {
    clear: [u8; 2],
    mist: [u8; 2],
    rain: [u8; 2],
    storm: [u8; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWeather {
    region: RawRegion,
    zones: Vec<String>,
    bands: Vec<RawBand>,
    last: RawLast,
}

fn weather(src: &Source, cx: &mut Ctx) -> &'static [RegionWeather] {
    let rows = src.list("weather", &mut cx.diag);
    // A source with no weather at all (a test fixture of another group): a sky that never changes.
    if rows.is_empty() {
        return &[];
    }
    let mut out: Vec<RegionWeather> = Vec::with_capacity(rows.len());
    let mut under = [0u8; jane_core::ids::ZONE_COUNT];
    for (n, row) in rows.iter().enumerate() {
        let at = format!("{}: weather[{n}]", row.file);
        let Some(r) = typed::<RawWeather>(row, &at, &mut cx.diag) else { continue };
        let region = Region::from(r.region);
        let zones: Vec<ZoneId> = r.zones.iter().filter_map(|z| cx.zone(&format!("{at}.zones"), z)).collect();
        for z in &zones {
            under[z.index()] += 1;
        }
        let mut hours = [0u8; 24];
        let mut bands = Vec::with_capacity(r.bands.len());
        for (b, band) in r.bands.iter().enumerate() {
            let at = format!("{at}.bands[{b}]");
            if band.from >= 24 || band.to >= 24 {
                cx.diag.error(&at, "hours are 0..=23");
                continue;
            }
            let weights = [band.clear, band.mist, band.rain, band.storm];
            let sum: u32 = weights.iter().map(|w| u32::from(*w)).sum();
            cx.diag.need(sum == 1000, &at, format!("the weights sum to {sum}, not 1000"));
            for (h, c) in hours.iter_mut().enumerate() {
                if in_span(h as u8, band.from, band.to) {
                    *c += 1;
                }
            }
            let season = match band.season {
                RawSeason::Autumn => Season::Autumn,
            };
            bands.push(SkyBand { season, hour_from: band.from, hour_to: band.to, weights });
        }
        for (h, c) in hours.iter().enumerate() {
            cx.diag.need(*c == 1, &at, format!("{h:02}:00 is in {c} bands of the autumn: every hour in exactly one"));
        }
        let mut last = [(0u8, 0u8); 4];
        for (i, (kind, [lo, hi])) in
            [("clear", r.last.clear), ("mist", r.last.mist), ("rain", r.last.rain), ("storm", r.last.storm)]
                .into_iter()
                .enumerate()
        {
            cx.diag.need(
                lo >= 1 && lo <= hi,
                format!("{at}.last.{kind}"),
                "a kind lasts [min, max] hours, 1 <= min <= max",
            );
            last[i] = (lo, hi);
        }
        out.push(RegionWeather { region, zones: leak(zones), bands: leak(bands), last });
    }
    let order: Vec<Region> = out.iter().map(|w| w.region).collect();
    cx.diag.need(order == REGIONS, "weather", "one row per region, in region order: lowfields, waters, works");
    for z in ZoneId::ALL {
        let n = under[z.index()];
        cx.diag.need(
            n == 1,
            "weather",
            format!("zone \"{}\" is under {n} skies: every zone under exactly one", z.name()),
        );
    }
    leak(out)
}

// ---------------------------------------------------------------------------------------------
// Ecology

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPopulation {
    unit: String,
    cap: u8,
    weight: u16,
    hold: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEcology {
    area: String,
    recover: u16,
    populations: Vec<RawPopulation>,
}

fn ecology(src: &Source, cx: &mut Ctx, county: &County) -> &'static [EcologyDef] {
    let rows = src.list("ecology", &mut cx.diag);
    let mut out: Vec<EcologyDef> = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let at = format!("{}: ecology[{n}]", row.file);
        let Some(r) = typed::<RawEcology>(row, &at, &mut cx.diag) else { continue };
        let area = cx.name(&r.area);
        cx.diag.need(
            county.areas.iter().any(|a| a.id == area),
            &at,
            format!("\"{}\" is no area of the skeleton (data/areas.json)", r.area),
        );
        cx.diag.need(out.iter().all(|e| e.area != area), &at, format!("area \"{}\" has two rows", r.area));
        let mut pops: Vec<Population> = Vec::with_capacity(r.populations.len());
        for (i, p) in r.populations.iter().enumerate() {
            let at = format!("{at}.populations[{i}]");
            let Some(unit) = cx.unit(&at, &p.unit) else { continue };
            cx.diag.need(pops.iter().all(|q| q.unit != unit), &at, format!("\"{}\" twice in one area", p.unit));
            cx.diag.need(p.cap >= 1 && p.weight >= 1 && p.hold >= 1, &at, "cap, weight and hold are at least 1");
            pops.push(Population { unit, cap: p.cap, weight: p.weight, hold: p.hold });
        }
        out.push(EcologyDef { area, recover: r.recover, populations: leak(pops) });
    }
    leak(out)
}

// ---------------------------------------------------------------------------------------------
// Consequences

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConsequence {
    id: String,
    on: RawCond,
    #[serde(default = "county")]
    zone: String,
    edits: Vec<RawAction>,
    confirms: Option<String>,
    contradicts: Option<String>,
}

fn county() -> String {
    "county".to_owned()
}

/// Every verb of a compiled list that is not a world verb, through `send`'s `then`.
fn not_world(cx: &Ctx, l: ListRef, out: &mut Vec<String>) {
    let ListRef::Catalog(i) = l else { return };
    let Some(list) = cx.lists.get(usize::from(i)) else { return };
    for a in list {
        if !a.is_world_verb() {
            out.push(format!("{a:?}").split([' ', '(', '{']).next().unwrap_or("?").to_owned());
        }
        if let Action::Send { then: Some(t), .. } = *a {
            not_world(cx, t, out);
        }
    }
}

fn claim(cx: &mut Ctx, at: &str, s: Option<&str>) -> Option<jane_core::ids::TextId> {
    let s = s?;
    // The claim is a line some row says: a confirmation of nothing anybody said is a bug.
    if cx.texts.get(s).is_none() {
        cx.diag.error(at, format!("\"{s}\" is not a line any row says"));
        return None;
    }
    Some(cx.text(s))
}

fn consequences(src: &Source, cx: &mut Ctx) -> &'static [ConsequenceDef] {
    let rows = src.list("consequences", &mut cx.diag);
    let mut typed_rows: Vec<(String, RawConsequence)> = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let at = format!("{}: consequences[{n}]", row.file);
        if let Some(r) = typed::<RawConsequence>(row, &at, &mut cx.diag) {
            typed_rows.push((row_at(row, &r.id), r));
        }
    }
    // Ids in sorted order (ARCHITECTURE.md §3.1).
    typed_rows.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    let mut out: Vec<ConsequenceDef> = Vec::with_capacity(typed_rows.len());
    for (at, r) in &typed_rows {
        cx.diag.need(!r.id.is_empty(), at, "id is empty");
        cx.diag.need(out.last().is_none_or(|p: &ConsequenceDef| p.id != r.id), at, "defined twice");
        let on = lists::cond(cx, &format!("{at}.on"), &r.on);
        let Some(on) = on else { continue };
        cx.diag.need(!on.not, format!("{at}.on"), "a consequence fires on something happening, never on its absence");
        cx.diag.need(
            matches!(on.c, Condition::Flag { .. } | Condition::QuestDone(_) | Condition::Dead(_)),
            format!("{at}.on"),
            "on is a flag, a quest done or a named death",
        );
        let Some(zone) = cx.zone(&format!("{at}.zone"), &r.zone) else { continue };
        let edits = lists::list(cx, &format!("{at}.edits"), Some(&r.edits)).unwrap_or(ListRef::Catalog(0));
        let mut bad = Vec::new();
        not_world(cx, edits, &mut bad);
        for v in bad {
            cx.diag.error(
                format!("{at}.edits"),
                format!("\"{v}\" is not a world verb: a consequence shows, hides, locks, unlocks, night-locks, night-unlocks, switches, spawns, despawns, fills, sends and sets flags"),
            );
        }
        let only_flags = r.edits.iter().all(|a| matches!(a, RawAction::Flag { .. }));
        cx.diag.need(!only_flags, format!("{at}.edits"), "a consequence is never a flag alone (WORLD.md §6)");
        cx.diag.need(
            r.confirms.is_none() || r.contradicts.is_none(),
            at,
            "a consequence confirms a claim or contradicts it, not both",
        );
        let confirms = claim(cx, &format!("{at}.confirms"), r.confirms.as_deref());
        let contradicts = claim(cx, &format!("{at}.contradicts"), r.contradicts.as_deref());
        out.push(ConsequenceDef { id: leak_str(&r.id), on: on.c, zone, edits, confirms, contradicts });
    }
    leak(out)
}

fn row_at(row: &Row, id: &str) -> String {
    format!("{}: consequences.{id}", row.file)
}

// ---------------------------------------------------------------------------------------------
// Tuning

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWetness {
    /// Seconds.
    every: Num,
    rise: u8,
    fall: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTuning {
    journal_ring: u16,
    wetness: RawWetness,
    clear_hours: u8,
}

/// With no `tuning/sim.json` (a test fixture): the defaults of ARCHITECTURE.md §12.
pub const DEFAULT_TUNING: SimTuning =
    SimTuning { journal_ring: 512, wetness: WetnessTuning { every: Tick(60), rise: 3, fall: 1 }, clear_hours: 2 };

fn tuning(src: &Source, cx: &mut Ctx) -> SimTuning {
    let file = "tuning/sim.json";
    let Some(v) = src.file(file) else { return DEFAULT_TUNING };
    let row = Row { file: file.to_owned(), value: v.clone() };
    let Some(r) = typed::<RawTuning>(&row, file, &mut cx.diag) else { return DEFAULT_TUNING };
    cx.diag.need(r.journal_ring >= 1, file, "journalRing: the journal keeps at least one entry per kind");
    let every = r.wetness.every.ticks().unwrap_or_else(|e| {
        cx.diag.error(format!("{file}.wetness.every"), e);
        Tick(60)
    });
    cx.diag.need(every.0 >= 1, format!("{file}.wetness.every"), "the ramp steps at least every tick");
    SimTuning {
        journal_ring: r.journal_ring,
        wetness: WetnessTuning { every, rise: r.wetness.rise, fall: r.wetness.fall },
        clear_hours: r.clear_hours,
    }
}

/// The sky kinds by index, for the sim (`Sky::ALL[i]`).
pub const SKIES: [Sky; 4] = Sky::ALL;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ctx::Ids;
    use crate::compile::diag::Diagnostics;
    use crate::model::{AreaDef, AreaWhere, Contract};

    /// The living group over in-memory files: a quest `q`, a unit `u`, an area `field`, and a line
    /// "Hello." that some row says.
    fn build(extra: &[(&str, &str)]) -> (model::Living, Ctx) {
        let mut files: Vec<(&str, &str)> =
            vec![("quests.json", r#"{"q": {}}"#), ("units.json", r#"{"u": {}}"#), ("items.json", r#"{"i": {}}"#)];
        files.extend_from_slice(extra);
        let src = Source::from_files(&files).unwrap();
        let mut cx = Ctx { ids: Ids::collect(&src, &mut Diagnostics::default()), ..Ctx::default() };
        cx.text("Hello.");
        let field = cx.name("field");
        let area = AreaDef {
            id: field,
            name: cx.text("The Field"),
            region: Region::Lowfields,
            threat: 1,
            radius: 50,
            rest: false,
            required: false,
            at: AreaWhere { near: None, near_min: None, near_max: None, terrain: None, off_road: None, on_road: false },
        };
        let areas: &'static [AreaDef] = leak(vec![area]);
        let county = County {
            zones: &[],
            promised: Contract::EMPTY,
            sites: &[],
            pois: &[],
            anchors: &[],
            areas,
            paths: &[],
            doors: &[],
            names: &[],
            placements: &[],
            stories: &[],
            story_ix: &[],
        };
        let living = compile(&src, &mut cx, &county);
        (living, cx)
    }

    fn errors(cx: &Ctx) -> Vec<String> {
        cx.diag.errors.iter().map(ToString::to_string).collect()
    }

    fn has(b: &(model::Living, Ctx), needle: &str) -> bool {
        errors(&b.1).iter().any(|e| e.contains(needle))
    }

    const WEATHER_OK: &str = r#"[
      {"region": "lowfields", "zones": ["county", "house", "cellar", "mine", "arms", "church"],
       "bands": [{"season": "autumn", "from": 6, "to": 21, "clear": 700, "mist": 100, "rain": 200, "storm": 0},
                 {"season": "autumn", "from": 21, "to": 6, "clear": 500, "mist": 300, "rain": 200, "storm": 0}],
       "last": {"clear": [2, 5], "mist": [1, 3], "rain": [1, 3], "storm": [1, 2]}},
      {"region": "waters", "zones": ["forest", "library", "museum"],
       "bands": [{"season": "autumn", "from": 0, "to": 0, "clear": 400, "mist": 400, "rain": 200, "storm": 0}],
       "last": {"clear": [2, 5], "mist": [1, 3], "rain": [1, 3], "storm": [1, 2]}},
      {"region": "works", "zones": ["burial", "factory", "pipes", "school"],
       "bands": [{"season": "autumn", "from": 0, "to": 0, "clear": 400, "mist": 200, "rain": 200, "storm": 200}],
       "last": {"clear": [2, 5], "mist": [1, 3], "rain": [1, 3], "storm": [1, 2]}}
    ]"#;

    #[test]
    fn weather_rows_are_checked() {
        let (l, cx) = build(&[("weather.json", WEATHER_OK)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        assert_eq!(l.weather.len(), 3);
        assert_eq!(l.region_of(ZoneId::Museum), Region::Waters);
        assert_eq!(l.sky(Region::Lowfields).unwrap().band_at(22).unwrap().weights, [500, 300, 200, 0]);

        let bad = WEATHER_OK.replace(r#""clear": 700, "mist": 100"#, r#""clear": 700, "mist": 200"#);
        assert!(has(&build(&[("weather.json", &bad)]), "sum to 1100, not 1000"));
        let gap = WEATHER_OK.replace(r#""from": 21, "to": 6"#, r#""from": 22, "to": 6"#);
        assert!(has(&build(&[("weather.json", &gap)]), "21:00 is in 0 bands"));
        let twice = WEATHER_OK.replace(r#""zones": ["forest", "#, r#""zones": ["mine", "forest", "#);
        assert!(has(&build(&[("weather.json", &twice)]), "zone \"mine\" is under 2 skies"));
        let order = WEATHER_OK.replacen(r#""region": "lowfields""#, r#""region": "waters""#, 1);
        assert!(has(&build(&[("weather.json", &order)]), "in region order"));
        let short = WEATHER_OK.replacen(r#""mist": [1, 3]"#, r#""mist": [0, 3]"#, 1);
        assert!(has(&build(&[("weather.json", &short)]), "1 <= min <= max"));
    }

    #[test]
    fn consequences_are_world_verbs_and_more_than_a_flag() {
        let ok = r#"[
          {"id": "b_second", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "hide", "prop": "p"}, {"do": "flag", "flag": "f"}]},
          {"id": "a_first", "on": {"if": "dead", "unit": "u"}, "zone": "mine", "edits": [{"do": "show", "prop": "p"}], "confirms": "Hello."}
        ]"#;
        let (l, cx) = build(&[("consequences.json", ok)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        let ids: Vec<&str> = l.consequences.iter().map(|r| r.id).collect();
        assert_eq!(ids, ["a_first", "b_second"], "ids in sorted order");
        assert_eq!(l.consequences[0].zone, ZoneId::Mine);
        assert_eq!(l.consequences[1].zone, ZoneId::County);
        assert!(l.consequences[0].confirms.is_some());
        assert!(matches!(l.consequences[0].on, Condition::Dead(_)));

        let give = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "give", "item": "i", "qty": 1}, {"do": "show", "prop": "p"}]}]"#;
        assert!(has(&build(&[("consequences.json", give)]), "is not a world verb"));
        // A lock or an unlock is a world edit: a door shut for good, a gate left open.
        let lock = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "lock", "prop": "p"}, {"do": "unlock", "prop": "g"}]}]"#;
        let (l, cx) = build(&[("consequences.json", lock)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        assert_eq!(l.consequences.len(), 1);
        let deep_lock = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "send", "unit": "u", "to": "m", "then": [{"do": "lock", "prop": "p"}]}]}]"#;
        assert!(errors(&build(&[("consequences.json", deep_lock)]).1).is_empty(), "through send's then too");
        // A door's hours are a world edit too, the bell's when left out.
        let night = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "nightLock", "prop": "p", "says": "Not now.", "keyed": true}, {"do": "nightUnlock", "prop": "g"}]}]"#;
        let (l, cx) = build(&[("consequences.json", night)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        assert_eq!(l.consequences.len(), 1);
        let hours = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "nightLock", "prop": "p", "says": "Not now.", "hours": [4, 4]}]}]"#;
        assert!(has(&build(&[("consequences.json", hours)]), "two different hours"));
        let toast = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "toast", "text": "Hello."}, {"do": "lock", "prop": "p"}]}]"#;
        assert!(has(&build(&[("consequences.json", toast)]), "\"Toast\" is not a world verb"));
        let flag = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "flag", "flag": "f"}]}]"#;
        assert!(has(&build(&[("consequences.json", flag)]), "never a flag alone"));
        let not = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q", "not": true}, "edits": [{"do": "show", "prop": "p"}]}]"#;
        assert!(has(&build(&[("consequences.json", not)]), "never on its absence"));
        let night = r#"[{"id": "x", "on": {"if": "night"}, "edits": [{"do": "show", "prop": "p"}]}]"#;
        assert!(has(&build(&[("consequences.json", night)]), "on is a flag, a quest done or a named death"));
        let quest =
            r#"[{"id": "x", "on": {"if": "questDone", "quest": "nope"}, "edits": [{"do": "show", "prop": "p"}]}]"#;
        assert!(has(&build(&[("consequences.json", quest)]), "unknown quest \"nope\""));
        let said = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "show", "prop": "p"}], "contradicts": "Nobody says this."}]"#;
        assert!(has(&build(&[("consequences.json", said)]), "is not a line any row says"));
        let both = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "show", "prop": "p"}], "confirms": "Hello.", "contradicts": "Hello."}]"#;
        assert!(has(&build(&[("consequences.json", both)]), "not both"));
        let deep = r#"[{"id": "x", "on": {"if": "questDone", "quest": "q"}, "edits": [{"do": "send", "unit": "u", "to": "m", "then": [{"do": "rest"}]}]}]"#;
        assert!(has(&build(&[("consequences.json", deep)]), "\"Rest\" is not a world verb"));
    }

    #[test]
    fn ecology_names_an_area_and_a_unit() {
        let ok =
            r#"[{"area": "field", "recover": 10, "populations": [{"unit": "u", "cap": 4, "weight": 50, "hold": 60}]}]"#;
        let (l, mut cx) = build(&[("ecology.json", ok)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        let e = l.ecology_of(cx.name("field")).unwrap();
        assert_eq!((e.recover, e.populations[0].cap, e.populations[0].hold), (10, 4, 60));
        let area = ok.replace(r#""area": "field""#, r#""area": "nowhere""#);
        assert!(has(&build(&[("ecology.json", &area)]), "\"nowhere\" is no area"));
        let unit = ok.replace(r#""unit": "u""#, r#""unit": "ghost""#);
        assert!(has(&build(&[("ecology.json", &unit)]), "unknown unit def \"ghost\""));
        let zero = ok.replace(r#""hold": 60"#, r#""hold": 0"#);
        assert!(has(&build(&[("ecology.json", &zero)]), "at least 1"));
    }

    #[test]
    fn tuning_has_defaults_and_is_checked() {
        assert_eq!(build(&[]).0.tuning, DEFAULT_TUNING);
        let t = r#"{"journalRing": 64, "wetness": {"every": 0.5, "rise": 4, "fall": 2}, "clearHours": 1}"#;
        let tu = build(&[("tuning/sim.json", t)]).0.tuning;
        assert_eq!((tu.journal_ring, tu.wetness.every, tu.wetness.rise, tu.clear_hours), (64, Tick(30), 4, 1));
        let bad = r#"{"journalRing": 0, "wetness": {"every": 0, "rise": 4, "fall": 2}, "clearHours": 1}"#;
        let b = build(&[("tuning/sim.json", bad)]);
        assert!(has(&b, "journalRing") && has(&b, "at least every tick"));
    }
}
