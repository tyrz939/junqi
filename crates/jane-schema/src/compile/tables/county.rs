//! Compile the `county` group into [`model::County`]: `zones.json`, the skeleton's rows (`sites`,
//! `pois`, `anchors`, `areas`), `paths`, `doors`, `names`, `placements/*.json`, `stories/*.json`.
//!
//! Every check the TypeScript made on these rows that needs only data is made here (PORT.md §5.3):
//! the zone file is in tick order, every door leads into one of the thirteen zones and arrives at a
//! mark that zone provides, every reference between rows resolves (and, where the builder walks the
//! rows in order, to an earlier row), a skeleton patch a story leans on is `required`, name pools are
//! unique and no name hides inside another, story ids are unique and every story's quests exist and
//! belong to it alone, every `{place:...}` in any text is a story, and the mechanical VOICE.md rules
//! (no en or em dash, nobody's name baked in) hold for every English string these tables hold.
//!
//! What these tables provide to the provider check is [`model::County::provides`] (ARCHITECTURE.md
//! §5.3); a name provided twice in the county (two props with one key) is an error here.

use serde::Deserialize;
use serde_json::Value;

use jane_core::action::{Facing, Stack};
use jane_core::ids::{NameId, StoryId, ZoneId};

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::diag::Diagnostics;
use crate::compile::fraction::Num;
use crate::compile::lists::{self, RawAction, RawCond};
use crate::compile::source::{Row, Source, typed};
use crate::model::{
    self, After, AnchorDef, AnchorWhere, Apart, AreaDef, AreaWhere, Contract, DistBy, DistRule, DoorAt, DoorDef, Edge,
    Hides, NameKind, NamePool, NearAnchor, NightLockDef, PathDef, PlaceAt, PlaceKind, PlacedRect, PlacedUnit,
    PlacementDef, PoiDef, PoiWhere, PropEdit, PropTemplate, ProvidedBy, Region, Rim, RuinKind, SiteBand, SiteDef,
    SiteWhere, SolveStateDef, Spreads, StoryDef, StoryNear, Terrain, Via, ZoneDef, ZoneKind,
};

pub fn compile(src: &Source, cx: &mut Ctx) -> model::County {
    let zones = zones(src, cx);
    let sites = sites(src, cx);
    let areas = areas(src, cx, &sites);
    let pois = pois(src, cx);
    let anchors = anchors(src, cx, &sites, &areas, &pois);
    let paths = paths(src, cx, &sites, &areas, &anchors);
    let doors = doors(src, cx, &zones, &sites);
    let stories = stories(src, cx);
    let names = names(src, cx, &stories);
    let placements = placements(src, cx, &sites, &areas, &anchors, &pois);
    story_rows(cx, &stories, &placements);
    place_refs(src, cx);
    let promised = promised(&placements);

    let mut story_ix = vec![0u16; cx.ids.stories.len()];
    for (n, s) in stories.defs.iter().enumerate() {
        story_ix[s.id.index()] = n as u16;
    }
    let county = model::County {
        zones: leak(zones),
        promised,
        sites: leak(sites.defs),
        pois: leak(pois.defs),
        anchors: leak(anchors.defs),
        areas: leak(areas.defs),
        paths: leak(paths),
        doors: leak(doors),
        names: leak(names),
        placements: leak(placements),
        stories: leak(stories.defs),
        story_ix: leak(story_ix),
    };
    once_each(cx, &county);
    county
}

// --- shared ----------------------------------------------------------------------------------

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawRegion {
    Lowfields,
    Waters,
    Works,
}

fn region(r: RawRegion) -> Region {
    match r {
        RawRegion::Lowfields => Region::Lowfields,
        RawRegion::Waters => Region::Waters,
        RawRegion::Works => Region::Works,
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawTerrain {
    Foothill,
    Crown,
    Bank,
    Flat,
    Wood,
}

fn terrain(t: RawTerrain) -> Terrain {
    match t {
        RawTerrain::Foothill => Terrain::Foothill,
        RawTerrain::Crown => Terrain::Crown,
        RawTerrain::Bank => Terrain::Bank,
        RawTerrain::Flat => Terrain::Flat,
        RawTerrain::Wood => Terrain::Wood,
    }
}

/// A table's rows with their ids, read before any row is typed, so a reference resolves whatever
/// order the rows are compiled in.
struct Keyed<T> {
    ids: Vec<String>,
    defs: Vec<T>,
}

impl<T> Keyed<T> {
    /// The index of `id`, or an error naming what was looked for.
    fn ix(&self, cx: &mut Ctx, at: &str, what: &str, id: &str) -> Option<u8> {
        let i = self.ids.iter().position(|x| x == id);
        if i.is_none() {
            cx.diag.error(at, format!("unknown {what} \"{id}\""));
        }
        i.map(|i| i as u8)
    }

    /// Like `ix`, and the row must come before row `before`: the builder walks the rows in order.
    fn earlier(&self, cx: &mut Ctx, at: &str, what: &str, id: &str, before: usize) -> Option<u8> {
        let i = self.ix(cx, at, what, id)?;
        cx.diag.need(usize::from(i) < before, at, format!("{what} \"{id}\" must be an earlier row"));
        Some(i)
    }
}

/// The `id` of every row, and duplicates reported. A table indexed by `u8` holds at most 255 rows.
fn row_ids(cx: &mut Ctx, table: &str, rows: &[Row], field: &str) -> Vec<String> {
    let ids: Vec<String> =
        rows.iter().map(|r| r.value.get(field).and_then(Value::as_str).unwrap_or_default().to_owned()).collect();
    for (n, id) in ids.iter().enumerate() {
        if ids[..n].contains(id) {
            cx.diag.error(format!("{}: {table}[{n}]", rows[n].file), format!("{field} \"{id}\" is defined twice"));
        }
    }
    cx.diag.need(ids.len() <= 255, table, format!("{} rows: more than a u8 index holds", ids.len()));
    ids
}

/// English, interned, held to VOICE.md's mechanical rules: no en or em dash, nobody's name baked in.
/// A row's `nightLock` and `nightHours` (the bell's night when left out): hours 0..=23, not the
/// same hour twice, and never hours without the line.
pub(crate) fn night_lock(cx: &mut Ctx, at: &str, says: Option<&str>, hours: Option<[u8; 2]>) -> Option<NightLockDef> {
    let Some(says) = says else {
        cx.diag.need(hours.is_none(), at, "nightHours without a nightLock line");
        return None;
    };
    let [from, to] = hours.unwrap_or([jane_core::NightLock::BELL.0, jane_core::NightLock::BELL.1]);
    cx.diag.need(
        from < 24 && to < 24 && from != to,
        at,
        format!("nightHours [{from}, {to}]: two different hours, 0 to 23"),
    );
    Some(NightLockDef { says: say(cx, at, says), from, to })
}

fn say(cx: &mut Ctx, at: &str, s: &str) -> jane_core::TextId {
    cx.diag.need(!s.contains(['\u{2013}', '\u{2014}']), at, format!("an en or em dash in \"{s}\" (VOICE.md)"));
    cx.diag.need(!s.contains("Jane"), at, format!("\"Jane\" is baked into \"{s}\"; the player names her"));
    cx.text(s)
}

fn name(cx: &mut Ctx, at: &str, s: &str) -> NameId {
    cx.diag.need(!s.is_empty(), at, "an empty name");
    cx.name(s)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStack {
    item: String,
    qty: u16,
}

fn stacks(cx: &mut Ctx, at: &str, raw: &[RawStack]) -> &'static [Stack] {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        cx.diag.need(s.qty >= 1, at, format!("{} x{}: qty < 1", s.item, s.qty));
        if let Some(item) = cx.item(at, &s.item) {
            out.push(Stack { item, qty: s.qty });
        }
    }
    leak(out)
}

// --- zones -----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawZoneKind {
    County,
    Interior,
    Dungeon,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContract {
    #[serde(default)]
    units: Vec<String>,
    #[serde(default)]
    props: Vec<String>,
    #[serde(default)]
    marks: Vec<String>,
    #[serde(default)]
    rects: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSolveState {
    id: String,
    flag: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawZone {
    id: String,
    kind: RawZoneKind,
    mission: Option<String>,
    #[serde(default)]
    contract: RawContract,
    #[serde(default)]
    given_keys: Vec<String>,
    given_verbs: Option<Vec<String>>,
    #[serde(default)]
    states: Vec<RawSolveState>,
}

fn names_of(cx: &mut Ctx, at: &str, what: &str, list: &[String]) -> &'static [NameId] {
    for (n, s) in list.iter().enumerate() {
        if list[..n].contains(s) {
            cx.diag.error(at, format!("{what} \"{s}\" is listed twice"));
        }
    }
    leak(list.iter().map(|s| name(cx, at, s)).collect())
}

fn contract(cx: &mut Ctx, at: &str, c: &RawContract) -> Contract {
    Contract {
        units: names_of(cx, &format!("{at}.contract.units"), "unit", &c.units),
        props: names_of(cx, &format!("{at}.contract.props"), "prop", &c.props),
        marks: names_of(cx, &format!("{at}.contract.marks"), "mark", &c.marks),
        rects: names_of(cx, &format!("{at}.contract.rects"), "rect", &c.rects),
    }
}

/// `data/zones.json`: the thirteen zones in tick order, which must be `ZoneId::ALL`'s order.
fn zones(src: &Source, cx: &mut Ctx) -> Vec<ZoneDef> {
    let rows = src.list("zones", &mut cx.diag);
    let ids: Vec<&str> = rows.iter().map(|r| r.value.get("id").and_then(Value::as_str).unwrap_or_default()).collect();
    let want: Vec<&str> = ZoneId::ALL.iter().map(|z| z.name()).collect();
    cx.diag.need(
        ids == want,
        "zones.json",
        format!(
            "the zones must be in tick order, ZoneId::ALL: [{}]; the file has [{}]",
            want.join(", "),
            ids.join(", ")
        ),
    );
    let mut out: Vec<ZoneDef> = ZoneId::ALL
        .iter()
        .map(|&id| ZoneDef {
            id,
            kind: ZoneKind::Interior,
            mission: None,
            contract: Contract::EMPTY,
            given_keys: &[],
            given_verbs: None,
            states: &[],
        })
        .collect();
    for (n, row) in rows.iter().enumerate() {
        let at = format!("zones[{n}]");
        let Some(r) = typed::<RawZone>(row, &at, &mut cx.diag) else { continue };
        let at = format!("{}: zones.{}", row.file, r.id);
        let Some(id) = cx.zone(&at, &r.id) else { continue };
        let kind = match r.kind {
            RawZoneKind::County => ZoneKind::County,
            RawZoneKind::Interior => ZoneKind::Interior,
            RawZoneKind::Dungeon => ZoneKind::Dungeon,
        };
        cx.diag.need(
            (kind == ZoneKind::County) == (id == ZoneId::County),
            &at,
            "the county, and only the county, is kind \"county\"",
        );
        let mission = match (&r.mission, kind) {
            (Some(m), ZoneKind::Dungeon) => cx.row(&at, "mission", m, |i| &i.dungeons, jane_core::DungeonId),
            (None, ZoneKind::Dungeon) => {
                cx.diag.error(&at, "a dungeon names its mission (data/dungeons/<mission>.json)");
                None
            }
            (Some(_), _) => {
                cx.diag.error(&at, "only a dungeon has a mission");
                None
            }
            (None, _) => None,
        };
        if kind == ZoneKind::Dungeon {
            // The mission holds these; saying them twice is saying them two ways.
            cx.diag.need(r.given_keys.is_empty(), &at, "a dungeon's givenKeys are its mission's");
            cx.diag.need(r.given_verbs.is_none(), &at, "a dungeon's givenVerbs are its mission's");
            cx.diag.need(r.states.is_empty(), &at, "a dungeon's states are its mission's");
        }
        cx.diag.need(r.states.len() <= 3, &at, "at most three states (the stateful flood)");
        let def = ZoneDef {
            id,
            kind,
            mission,
            contract: contract(cx, &at, &r.contract),
            given_keys: names_of(cx, &format!("{at}.givenKeys"), "key tag", &r.given_keys),
            given_verbs: r
                .given_verbs
                .as_ref()
                .map(|v| leak(v.iter().filter_map(|s| cx.spell(&format!("{at}.givenVerbs"), s)).collect())),
            states: leak(
                r.states
                    .iter()
                    .map(|s| SolveStateDef { id: name(cx, &at, &s.id), flag: lists::flag_key(cx, &s.flag) })
                    .collect(),
            ),
        };
        out[id.index()] = def;
    }
    out
}

// --- sites -----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawEdge {
    West,
    East,
    North,
    South,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawDistBy {
    Road,
    Line,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDist {
    to: String,
    min: Option<u16>,
    max: Option<u16>,
    by: RawDistBy,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSiteWhere {
    edge: Option<RawEdge>,
    terrain: Option<RawTerrain>,
    across_river_from: Option<String>,
    max_height: Option<u8>,
    river_within: Option<u16>,
    lake_within: Option<u16>,
    off_road: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawSite {
    id: String,
    name: String,
    region: RawRegion,
    on_road: bool,
    #[serde(default)]
    dungeon: bool,
    #[serde(default)]
    rest: bool,
    hub: Option<u16>,
    #[serde(default, rename = "where")]
    at: RawSiteWhere,
    #[serde(default)]
    dist: Vec<RawDist>,
    road_from: Option<String>,
}

fn band(cx: &mut Ctx, at: &str, min: Option<u16>, max: Option<u16>) {
    if let (Some(a), Some(b)) = (min, max) {
        cx.diag.need(a <= b, at, format!("min {a} > max {b}"));
    }
}

/// `data/sites.json`, in row order: every reference is to an earlier row, because the skeleton
/// places them in order and aims each at the ones already standing.
fn sites(src: &Source, cx: &mut Ctx) -> Keyed<SiteDef> {
    let rows = src.list("sites", &mut cx.diag);
    let mut k = Keyed { ids: row_ids(cx, "sites", &rows, "id"), defs: Vec::new() };
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawSite>(row, &format!("sites[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: sites.{}", row.file, r.id);
        let mut dist = Vec::new();
        for d in &r.dist {
            band(cx, &at, d.min, d.max);
            cx.diag.need(
                d.min.is_some() || d.max.is_some(),
                &at,
                format!("a distance rule to {} with neither min nor max", d.to),
            );
            let Some(to) = k.earlier(cx, &at, "site", &d.to, n) else { continue };
            let by = match d.by {
                RawDistBy::Road => DistBy::Road,
                RawDistBy::Line => DistBy::Line,
            };
            dist.push(DistRule { to, min: d.min, max: d.max, by });
        }
        let w = &r.at;
        let def = SiteDef {
            id: leak_str(&r.id),
            name: say(cx, &at, &r.name),
            region: region(r.region),
            on_road: r.on_road,
            dungeon: r.dungeon,
            rest: r.rest,
            hub: r.hub,
            at: SiteWhere {
                edge: w.edge.as_ref().map(|e| match e {
                    RawEdge::West => Edge::West,
                    RawEdge::East => Edge::East,
                    RawEdge::North => Edge::North,
                    RawEdge::South => Edge::South,
                }),
                terrain: w.terrain.map(terrain),
                across_river_from: w.across_river_from.as_ref().and_then(|s| k.earlier(cx, &at, "site", s, n)),
                max_height: w.max_height,
                river_within: w.river_within,
                lake_within: w.lake_within,
                off_road: w.off_road,
            },
            dist: leak(dist),
            road_from: r.road_from.as_ref().and_then(|s| k.earlier(cx, &at, "site", s, n)),
        };
        cx.diag.need(r.road_from.is_none() || r.on_road, &at, "roadFrom on a site that is off the road");
        k.defs.push(def);
    }
    k
}

// --- areas -----------------------------------------------------------------------------------

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAreaWhere {
    near: Option<String>,
    near_max: Option<u16>,
    near_min: Option<u16>,
    terrain: Option<RawTerrain>,
    off_road: Option<u16>,
    #[serde(default)]
    on_road: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArea {
    id: String,
    #[serde(default)]
    required: bool,
    name: String,
    region: RawRegion,
    threat: u8,
    radius: u16,
    #[serde(default)]
    rest: bool,
    #[serde(default, rename = "where")]
    at: RawAreaWhere,
}

fn areas(src: &Source, cx: &mut Ctx, sites: &Keyed<SiteDef>) -> Keyed<AreaDef> {
    let rows = src.list("areas", &mut cx.diag);
    let mut k = Keyed { ids: row_ids(cx, "areas", &rows, "id"), defs: Vec::new() };
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawArea>(row, &format!("areas[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: areas.{}", row.file, r.id);
        cx.diag.need(r.threat <= 6, &at, format!("threat {} is past 6", r.threat));
        cx.diag.need(r.radius > 0, &at, "radius 0");
        let w = &r.at;
        band(cx, &at, w.near_min, w.near_max);
        cx.diag.need(
            w.near.is_some() || (w.near_min.is_none() && w.near_max.is_none()),
            &at,
            "nearMin or nearMax without near",
        );
        cx.diag.need(!(w.on_road && w.off_road.is_some()), &at, "onRoad and offRoad at once");
        let def = AreaDef {
            id: name(cx, &at, &r.id),
            name: say(cx, &at, &r.name),
            region: region(r.region),
            threat: r.threat,
            radius: r.radius,
            rest: r.rest,
            required: r.required,
            at: AreaWhere {
                near: w.near.as_ref().and_then(|s| sites.ix(cx, &at, "site", s)),
                near_min: w.near_min,
                near_max: w.near_max,
                terrain: w.terrain.map(terrain),
                off_road: w.off_road,
                on_road: w.on_road,
            },
        };
        k.defs.push(def);
    }
    k
}

/// An area a story leans on by name: it must be `required`, or a county that skipped it would
/// leave the story's thing nowhere (and a promised thing re-rolls the county for ever).
fn required_area(cx: &mut Ctx, at: &str, areas: &Keyed<AreaDef>, id: &str) -> Option<u8> {
    let i = areas.ix(cx, at, "area", id)?;
    if let Some(a) = areas.defs.get(usize::from(i)) {
        cx.diag.need(a.required, at, format!("area \"{id}\" is named here, so it must be \"required\""));
    }
    Some(i)
}

// --- pois ------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawPoiWhere {
    Roadside,
    Deep,
    Bank,
    Any,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPoi {
    kind: String,
    name: String,
    regions: Vec<RawRegion>,
    weight: u16,
    #[serde(rename = "where")]
    at: RawPoiWhere,
}

fn pois(src: &Source, cx: &mut Ctx) -> Keyed<PoiDef> {
    let rows = src.list("pois", &mut cx.diag);
    let mut k = Keyed { ids: row_ids(cx, "pois", &rows, "kind"), defs: Vec::new() };
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawPoi>(row, &format!("pois[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: pois.{}", row.file, r.kind);
        cx.diag.need(r.kind != "none", &at, "\"none\" is the anchors' bare spot, not a kind of small place");
        cx.diag.need(!r.regions.is_empty(), &at, "in no region");
        cx.diag.need(r.weight >= 1, &at, "weight 0");
        let def = PoiDef {
            kind: name(cx, &at, &r.kind),
            name: say(cx, &at, &r.name),
            regions: leak(r.regions.iter().copied().map(region).collect()),
            weight: r.weight,
            at: match r.at {
                RawPoiWhere::Roadside => PoiWhere::Roadside,
                RawPoiWhere::Deep => PoiWhere::Deep,
                RawPoiWhere::Bank => PoiWhere::Bank,
                RawPoiWhere::Any => PoiWhere::Any,
            },
        };
        k.defs.push(def);
    }
    k
}

// --- anchors ---------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBand {
    to: String,
    min: Option<u16>,
    max: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRim {
    area: String,
    toward: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAfter {
    anchor: String,
    steps: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApart {
    from: String,
    min: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNearAnchor {
    anchor: String,
    min: u16,
    max: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAnchorWhere {
    road: Option<[String; 2]>,
    dist: Option<RawBand>,
    area: Option<String>,
    rim: Option<RawRim>,
    near_area: Option<String>,
    #[serde(default)]
    last_lamp: bool,
    after: Option<RawAfter>,
    apart: Option<RawApart>,
    near_anchor: Option<RawNearAnchor>,
    nearest: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnchor {
    id: String,
    kind: String,
    name: Option<String>,
    region: Option<RawRegion>,
    #[serde(rename = "where")]
    at: RawAnchorWhere,
}

fn anchors(
    src: &Source,
    cx: &mut Ctx,
    sites: &Keyed<SiteDef>,
    areas: &Keyed<AreaDef>,
    pois: &Keyed<PoiDef>,
) -> Keyed<AnchorDef> {
    let rows = src.list("anchors", &mut cx.diag);
    let mut k = Keyed { ids: row_ids(cx, "anchors", &rows, "id"), defs: Vec::new() };
    for (n, id) in k.ids.iter().enumerate() {
        // A footpath goes `via` an anchor or an area by one id: it must mean one thing.
        cx.diag.need(
            !areas.ids.contains(id),
            format!("anchors[{n}]"),
            format!("\"{id}\" is both an anchor and an area"),
        );
    }
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawAnchor>(row, &format!("anchors[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: anchors.{}", row.file, r.id);
        let w = &r.at;
        let kind = if r.kind == "none" {
            None
        } else {
            cx.diag.need(
                pois.ids.contains(&r.kind),
                &at,
                format!("kind \"{}\" is not in pois.json (or \"none\")", r.kind),
            );
            Some(name(cx, &at, &r.kind))
        };
        let where_ = AnchorWhere {
            road: w
                .road
                .as_ref()
                .and_then(|[a, b]| Some((sites.ix(cx, &at, "site", a)?, sites.ix(cx, &at, "site", b)?))),
            dist: w.dist.as_ref().and_then(|d| {
                band(cx, &at, d.min, d.max);
                Some(SiteBand { to: sites.ix(cx, &at, "site", &d.to)?, min: d.min, max: d.max })
            }),
            area: w.area.as_ref().and_then(|a| required_area(cx, &at, areas, a)),
            rim: w.rim.as_ref().and_then(|x| {
                Some(Rim {
                    area: required_area(cx, &at, areas, &x.area)?,
                    toward: sites.ix(cx, &at, "site", &x.toward)?,
                })
            }),
            near_area: w.near_area.as_ref().and_then(|a| required_area(cx, &at, areas, a)),
            last_lamp: w.last_lamp,
            after: w.after.as_ref().and_then(|x| {
                cx.diag.need(x.steps >= 1, &at, "after: 0 steps");
                Some(After { anchor: k.earlier(cx, &at, "anchor", &x.anchor, n)?, steps: x.steps })
            }),
            apart: w
                .apart
                .as_ref()
                .and_then(|x| Some(Apart { from: k.earlier(cx, &at, "anchor", &x.from, n)?, min: x.min })),
            near_anchor: w.near_anchor.as_ref().and_then(|x| {
                band(cx, &at, Some(x.min), Some(x.max));
                Some(NearAnchor { anchor: k.earlier(cx, &at, "anchor", &x.anchor, n)?, min: x.min, max: x.max })
            }),
            nearest: w.nearest.as_ref().and_then(|s| sites.ix(cx, &at, "site", s)),
        };
        let def = AnchorDef {
            id: name(cx, &at, &r.id),
            kind,
            name: r.name.as_deref().map(|s| say(cx, &at, s)),
            region: r.region.map_or(Region::Lowfields, region),
            at: where_,
        };
        k.defs.push(def);
    }
    k
}

// --- paths -----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPath {
    id: String,
    from: String,
    via: String,
    to: String,
    width: u8,
    marks: [String; 2],
}

fn paths(
    src: &Source,
    cx: &mut Ctx,
    sites: &Keyed<SiteDef>,
    areas: &Keyed<AreaDef>,
    anchors: &Keyed<AnchorDef>,
) -> Vec<PathDef> {
    let rows = src.list("paths", &mut cx.diag);
    row_ids(cx, "paths", &rows, "id");
    let mut out = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawPath>(row, &format!("paths[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: paths.{}", row.file, r.id);
        cx.diag.need(r.width >= 1, &at, "width 0");
        cx.diag.need(r.marks[0] != r.marks[1], &at, "its two ends have one mark");
        let via = if anchors.ids.contains(&r.via) {
            anchors.ix(cx, &at, "anchor", &r.via).map(Via::Anchor)
        } else if areas.ids.contains(&r.via) {
            required_area(cx, &at, areas, &r.via).map(Via::Area)
        } else {
            cx.diag.error(&at, format!("via \"{}\" is neither an anchor nor an area", r.via));
            None
        };
        let (Some(from), Some(to), Some(via)) =
            (sites.ix(cx, &at, "site", &r.from), sites.ix(cx, &at, "site", &r.to), via)
        else {
            continue;
        };
        let marks = [name(cx, &at, &r.marks[0]), name(cx, &at, &r.marks[1])];
        out.push(PathDef { id: leak_str(&r.id), from, via, to, width: r.width, marks });
    }
    out
}

// --- doors -----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawDoor {
    zone: String,
    chunk: Option<String>,
    near: Option<String>,
    def: Option<String>,
    key: String,
    label: String,
    key_tag: Option<String>,
    night_lock: Option<String>,
    night_hours: Option<[u8; 2]>,
    /// The key does not lock the door: it answers the `nightLock` at the hours it is shut.
    #[serde(default)]
    keyed: bool,
    mark: Option<String>,
    #[serde(default)]
    from_below: bool,
}

/// The mark a door arrives at in its zone: `entry`.
const ENTRY: &str = "entry";

/// Does a mission bind a mark by this name on a critical node (what its derived contract holds)?
/// Read off the raw mission: the dungeons group owns the compile of missions.
fn mission_binds_mark(src: &Source, mission: &str, mark: &str) -> bool {
    let Some(def) = src.file(&format!("dungeons/{mission}.json")) else { return false };
    def.get("nodes").and_then(Value::as_array).into_iter().flatten().any(|node| {
        node.get("critical").and_then(Value::as_bool) == Some(true)
            && node.get("binds").and_then(Value::as_array).into_iter().flatten().any(|b| {
                b.get("what").and_then(Value::as_str) == Some("mark")
                    && b.get("as").and_then(Value::as_str) == Some(mark)
            })
    })
}

/// `data/doors.json`: every row applies, because every zone exists (PORT.md §6.l).
fn doors(src: &Source, cx: &mut Ctx, zones: &[ZoneDef], sites: &Keyed<SiteDef>) -> Vec<DoorDef> {
    let rows = src.list("doors", &mut cx.diag);
    row_ids(cx, "doors", &rows, "key");
    let mut out = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawDoor>(row, &format!("doors[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: doors.{}", row.file, r.key);
        let Some(zone) = cx.zone(&at, &r.zone) else { continue };
        cx.diag.need(zone != ZoneId::County, &at, "a door out of the county into the county");
        let place = match (&r.chunk, &r.near) {
            (Some(c), None) => sites.ix(cx, &at, "site", c).map(DoorAt::Chunk),
            (None, Some(s)) => sites.ix(cx, &at, "site", s).map(DoorAt::Near),
            _ => {
                cx.diag.error(&at, "exactly one of chunk (set into a landmark) or near (beside a site)");
                None
            }
        };
        cx.diag.need(!r.from_below || r.mark.is_some(), &at, "fromBelow with no mark: a way up that arrives nowhere");
        cx.diag.need(
            !r.keyed || (r.key_tag.is_some() && r.night_lock.is_some()),
            &at,
            "keyed: a key that answers a nightLock needs both a keyTag and a nightLock",
        );
        cx.diag.need(r.chunk.is_none() || r.def.is_none(), &at, "a door set into a landmark's face is a door: no def");
        // Where it leads must be there, whenever it can be known from data.
        let to = if r.from_below {
            None
        } else {
            let entry = cx.name(ENTRY);
            let z = &zones[zone.index()];
            let bound = match z.mission {
                Some(m) => {
                    let mission = cx.ids.dungeons.keys().nth(m.index()).unwrap_or_default().to_owned();
                    mission_binds_mark(src, &mission, ENTRY)
                }
                None => false,
            };
            let there = bound || z.contract.marks.contains(&entry);
            cx.diag.need(there, &at, format!("leads to \"{ENTRY}\" in {}, which provides no such mark", r.zone));
            Some(entry)
        };
        let def_name = r.def.as_deref().unwrap_or("door");
        let (Some(at_), Some(def)) = (place, cx.prop(&at, def_name)) else { continue };
        out.push(DoorDef {
            zone,
            at: at_,
            key: name(cx, &at, &r.key),
            def,
            label: say(cx, &at, &r.label),
            key_tag: r.key_tag.as_deref().map(|t| name(cx, &at, t)),
            night_lock: night_lock(cx, &at, r.night_lock.as_deref(), r.night_hours),
            keyed: r.keyed,
            mark: r.mark.as_deref().map(|m| name(cx, &at, m)),
            to,
        });
    }
    out
}

// --- stories ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawPlaceKind {
    Hamlet,
    Farmstead,
    Cottage,
    Inn,
    Woodcutter,
    Camp,
    Ruin,
}

fn place_kind(k: RawPlaceKind) -> PlaceKind {
    match k {
        RawPlaceKind::Hamlet => PlaceKind::Hamlet,
        RawPlaceKind::Farmstead => PlaceKind::Farmstead,
        RawPlaceKind::Cottage => PlaceKind::Cottage,
        RawPlaceKind::Inn => PlaceKind::Inn,
        RawPlaceKind::Woodcutter => PlaceKind::Woodcutter,
        RawPlaceKind::Camp => PlaceKind::Camp,
        RawPlaceKind::Ruin => PlaceKind::Ruin,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNear {
    story: String,
    max: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawRuin {
    House,
    Walls,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpreads {
    to: Vec<String>,
    /// Seconds.
    after: Num,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStory {
    id: String,
    kind: RawPlaceKind,
    region: Option<RawRegion>,
    threat: Option<[u8; 2]>,
    first: Option<[u16; 2]>,
    near: Option<RawNear>,
    hostile: Option<String>,
    count: Option<u8>,
    quests: Vec<String>,
    #[serde(default)]
    tale: bool,
    name: Option<String>,
    board: Option<String>,
    ruin: Option<RawRuin>,
    spreads: Option<RawSpreads>,
}

/// The people each story spreads to (ARCHITECTURE.md §4.6.e), named once every group that
/// provides a person has interned its names (`compile::build_source` calls it after the chunks).
pub fn late_spreads(src: &Source, cx: &mut Ctx, stories: &'static [StoryDef]) -> &'static [StoryDef] {
    #[derive(Deserialize)]
    struct Heard {
        id: String,
        spreads: Option<RawSpreads>,
    }
    let rows = src.list("stories", &mut Diagnostics::default());
    let mut out = stories.to_vec();
    for row in &rows {
        let Ok(h) = serde_json::from_value::<Heard>(row.value.clone()) else { continue };
        let (Some(sp), Some(def)) = (h.spreads, out.iter_mut().find(|d| d.key == h.id)) else { continue };
        let at = format!("{}: stories.{}.spreads", row.file, h.id);
        if let Some(d) = def.spreads.as_mut() {
            d.to = leak(sp.to.iter().map(|t| name(cx, &at, t)).collect());
        }
    }
    leak(out)
}

struct Stories {
    defs: Vec<StoryDef>,
    /// Each story's content id, by position in `defs`.
    keys: Vec<String>,
}

fn stories(src: &Source, cx: &mut Ctx) -> Stories {
    let rows = src.list("stories", &mut cx.diag);
    row_ids(cx, "stories", &rows, "id");
    let mut out = Stories { defs: Vec::new(), keys: Vec::new() };
    let mut told: Vec<(String, String)> = Vec::new(); // (quest, story)
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawStory>(row, &format!("stories[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: stories.{}", row.file, r.id);
        let Some(id) = cx.story(&at, &r.id) else { continue };
        let threat = r.threat.unwrap_or([0, 2]);
        cx.diag.need(
            threat[0] <= threat[1] && threat[1] <= 6,
            &at,
            format!("threat {threat:?}: 0 <= low <= high <= 6"),
        );
        if let Some(f) = r.first {
            cx.diag.need(f[0] <= f[1], &at, format!("first {f:?}: low > high"));
        }
        let near = r.near.as_ref().and_then(|x| {
            cx.diag.need(x.story != r.id, &at, "near itself");
            // A chain is followed back to its head; a loop never gets there.
            let mut seen = vec![r.id.as_str()];
            let mut cur = x.story.as_str();
            while let Some(next) = rows
                .iter()
                .find(|q| q.value.get("id").and_then(Value::as_str) == Some(cur))
                .and_then(|q| q.value.get("near")?.get("story")?.as_str())
            {
                if seen.contains(&cur) {
                    cx.diag.error(&at, format!("the chain of near stories loops at \"{cur}\""));
                    break;
                }
                seen.push(cur);
                cur = next;
            }
            Some(StoryNear { story: cx.story(&at, &x.story)?, max: x.max })
        });
        let hostile: Vec<_> = r
            .hostile
            .as_deref()
            .map(|h| h.split('|').filter_map(|u| cx.unit(&format!("{at}.hostile"), u)).collect())
            .unwrap_or_default();
        cx.diag.need(r.count.is_none() || r.hostile.is_some(), &at, "count with no hostile");
        cx.diag.need(r.count != Some(0), &at, "count 0");
        let mut quests = Vec::new();
        for q in &r.quests {
            if let Some((_, other)) = told.iter().find(|(t, _)| t == q) {
                cx.diag.error(&at, format!("quest \"{q}\" is told in two stories (and in \"{other}\")"));
            }
            told.push((q.clone(), r.id.clone()));
            if let Some(q) = cx.quest(&at, q) {
                quests.push(q);
            }
        }
        cx.diag.need(!r.tale || r.name.is_some(), &at, "a tale has a fixed name of its own");
        let spreads = r.spreads.as_ref().map(|s| {
            let after = s.after.ticks().map_err(|e| cx.diag.error(&at, e)).unwrap_or_default();
            cx.diag.need(!s.to.is_empty(), &at, "spreads to nobody");
            // Who hears is named by `late_spreads`, after the chunks have named everyone: interning
            // a name here would move every name after it.
            Spreads { to: &[], after }
        });
        let def = StoryDef {
            id,
            key: leak_str(&r.id),
            kind: place_kind(r.kind),
            region: r.region.map_or(Region::Lowfields, region),
            threat: (threat[0], threat[1]),
            first: r.first.map(|f| (f[0], f[1])),
            near,
            hostile: leak(hostile),
            count: r.count.unwrap_or(1),
            quests: leak(quests),
            tale: r.tale,
            name: r.name.as_deref().map(|s| say(cx, &at, s)),
            board: r.board.as_deref().map(|s| say(cx, &at, s)),
            ruin: r.ruin.map(|x| match x {
                RawRuin::House => RuinKind::House,
                RawRuin::Walls => RuinKind::Walls,
            }),
            place_name: name(cx, &at, &format!("story_{}", r.id)),
            spreads,
        };
        out.defs.push(def);
        out.keys.push(r.id);
    }
    out
}

/// Every story has rows at its place: a story with none is a place nobody uses (stories.test.ts).
fn story_rows(cx: &mut Ctx, stories: &Stories, placements: &[PlacementDef]) {
    for s in &stories.defs {
        let used = placements.iter().any(|p| matches!(p.at, PlaceAt::Place { story, .. } if story == s.id));
        cx.diag.need(used, format!("stories.{}", s.key), "no placement row puts anything at its place");
    }
}

/// Every `{place:<id>}` in any text names a story (stories.test.ts, tales.test.ts). Read off the raw
/// rows of every table that holds English, so the check does not wait for another group's compile.
fn place_refs(src: &Source, cx: &mut Ctx) {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                let mut rest = s.as_str();
                while let Some(i) = rest.find("{place:") {
                    rest = &rest[i + 7..];
                    let end = rest.find('}').unwrap_or(rest.len());
                    out.push(rest[..end].to_owned());
                    rest = &rest[end..];
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut quiet = Diagnostics::default();
    let mut found: Vec<(String, String)> = Vec::new(); // (file, story)
    let keyed = ["quests", "dialogue", "items", "triggers", "units", "props", "spells", "effects"];
    let listed = ["clock", "placements", "stories", "doors", "sites", "anchors", "areas", "pois"];
    let mut rows: Vec<Row> = keyed.iter().flat_map(|t| src.table(t, &mut quiet).into_values()).collect();
    rows.extend(listed.iter().flat_map(|t| src.list(t, &mut quiet)));
    rows.extend(src.files_in("dungeons").map(|(f, v)| Row { file: f.to_owned(), value: v.clone() }));
    for row in rows {
        let mut ids = Vec::new();
        walk(&row.value, &mut ids);
        found.extend(ids.into_iter().map(|id| (row.file.clone(), id)));
    }
    for (file, id) in found {
        cx.diag.need(cx.ids.stories.get(&id).is_some(), file, format!("{{place:{id}}} names no story"));
    }
}

// --- names -----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWoodcutter {
    roots: Vec<String>,
    ends: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoards {
    hamlet: Vec<String>,
    farmstead: Vec<String>,
    inn: Vec<String>,
    cottage: Vec<String>,
    woodcutter: Vec<String>,
    camp: Vec<String>,
    ruin: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNames {
    hamlet: Vec<String>,
    farmstead: Vec<String>,
    inn: Vec<String>,
    cottage: Vec<String>,
    woodcutter: RawWoodcutter,
    camp: Vec<String>,
    ruin: Vec<String>,
    boards: RawBoards,
}

/// The woodcutters' clearings: ends by roots, leaving out a root its end already holds.
fn clearings(w: &RawWoodcutter) -> Vec<String> {
    let mut out = Vec::new();
    for end in &w.ends {
        for root in &w.roots {
            if !end.contains(root.as_str()) {
                out.push(format!("{root} {end}"));
            }
        }
    }
    out
}

/// `data/names.json`: one pool per kind of place, every name its own (stories.test.ts "every name a
/// seed could paint on a board is its own, and no name hides inside another"), and enough of them
/// for the stories that draw from the front of the list with ten to spare.
fn names(src: &Source, cx: &mut Ctx, stories: &Stories) -> Vec<NamePool> {
    let Some(v) = src.file("names.json") else {
        cx.diag.error("names.json", "missing");
        return Vec::new();
    };
    let row = Row { file: "names.json".to_owned(), value: v.clone() };
    let Some(r) = typed::<RawNames>(&row, "names", &mut cx.diag) else { return Vec::new() };
    let lists: [(PlaceKind, Vec<String>, &Vec<String>); 7] = [
        (PlaceKind::Hamlet, r.hamlet.clone(), &r.boards.hamlet),
        (PlaceKind::Farmstead, r.farmstead.clone(), &r.boards.farmstead),
        (PlaceKind::Cottage, r.cottage.clone(), &r.boards.cottage),
        (PlaceKind::Inn, r.inn.clone(), &r.boards.inn),
        (PlaceKind::Woodcutter, clearings(&r.woodcutter), &r.boards.woodcutter),
        (PlaceKind::Camp, r.camp.clone(), &r.boards.camp),
        (PlaceKind::Ruin, r.ruin.clone(), &r.boards.ruin),
    ];
    let mut all: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (kind, list, boards) in &lists {
        let at = format!("names.json: {}", kind.name());
        cx.diag.need(!list.is_empty(), &at, "no names");
        cx.diag.need(!boards.is_empty(), &at, "no board lines");
        for (n, s) in list.iter().enumerate() {
            // Every inn is the Halfway House: the innkeepers say so (data/dialogue/country.json).
            if *kind != PlaceKind::Inn && list[..n].contains(s) {
                cx.diag.error(&at, format!("\"{s}\" is in the list twice"));
            }
            if !all.contains(&s.as_str()) {
                all.push(s);
            }
        }
        let wanted = stories.defs.iter().filter(|s| s.kind == *kind && s.name.is_none()).count();
        cx.diag.need(
            *kind == PlaceKind::Inn || list.len() >= wanted + 10,
            &at,
            format!("{} names for {wanted} stories: stories draw from the front, and ten more are wanted", list.len()),
        );
        out.push(NamePool {
            kind: *kind,
            names: leak(list.iter().map(|s| say(cx, &at, s)).collect()),
            boards: leak(boards.iter().map(|s| say(cx, &at, s)).collect()),
        });
    }
    // No name inside another: a board that says one must not be read as the other.
    let lower: Vec<String> = all.iter().map(|s| s.to_lowercase()).collect();
    for (i, a) in lower.iter().enumerate() {
        for (j, b) in lower.iter().enumerate() {
            if i != j && b.contains(a.as_str()) {
                cx.diag.error("names.json", format!("\"{}\" is inside \"{}\"", all[i], all[j]));
            }
        }
    }
    // The tales' own names: none alike, none the same as or inside a pool's name, nor the other way
    // (tales.test.ts).
    let mut tales: Vec<(&str, String)> = Vec::new();
    for s in stories.defs.iter().filter(|s| s.tale) {
        let Some(t) = s.name else { continue };
        let name = cx.texts.strings().nth(t.index()).unwrap_or_default().to_owned();
        let at = format!("stories.{}", s.key);
        if tales.iter().any(|(_, n)| *n == name) {
            cx.diag.error(&at, format!("tale name \"{name}\" is taken by another tale"));
        }
        let low = name.to_lowercase();
        for (b, bl) in all.iter().zip(&lower) {
            cx.diag.need(
                !bl.contains(&low) && !low.contains(bl.as_str()),
                &at,
                format!("tale name \"{name}\" and \"{b}\" hide in each other"),
            );
        }
        tales.push((s.key, name));
    }
    out
}

// --- placements ------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawFacing {
    East,
    South,
    West,
    North,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAt {
    mark: Option<String>,
    site: Option<String>,
    area: Option<String>,
    poi: Option<String>,
    near: Option<String>,
    anchor: Option<String>,
    slot: Option<String>,
    place: Option<String>,
    prop: Option<String>,
    within: Option<u16>,
    dx: Option<i16>,
    dy: Option<i16>,
    on: Option<String>,
    room: Option<[u8; 4]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawProp {
    def: String,
    #[serde(default)]
    locked: bool,
    key_tag: Option<String>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    on: bool,
    #[serde(default)]
    loot: Vec<RawStack>,
    #[serde(rename = "use")]
    use_list: Option<Vec<RawAction>>,
    release: Option<Vec<RawAction>>,
    #[serde(default)]
    needs: Vec<RawStack>,
    talk: Option<String>,
    label: Option<String>,
    night_lock: Option<String>,
    night_hours: Option<[u8; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawEdit {
    key: Option<String>,
    def: Option<String>,
    locked: Option<bool>,
    key_tag: Option<String>,
    hidden: Option<bool>,
    loot: Option<Vec<RawStack>>,
    #[serde(rename = "use")]
    use_list: Option<Vec<RawAction>>,
    talk: Option<String>,
    label: Option<String>,
    night_lock: Option<String>,
    night_hours: Option<[u8; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnit {
    def: String,
    phase: Option<u8>,
    facing: Option<RawFacing>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRect {
    name: String,
    w: u16,
    h: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHides {
    key: String,
    prop: RawProp,
    when: Option<Vec<RawCond>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawPlacement {
    key: String,
    count: Option<u16>,
    #[serde(default)]
    spread: bool,
    at: RawAt,
    edit: Option<RawEdit>,
    unit: Option<RawUnit>,
    prop: Option<RawProp>,
    rect: Option<RawRect>,
    mark: Option<String>,
    hides: Option<RawHides>,
    beside: Option<String>,
    /// No longer read: every row throws its own dice (placements.ts). Accepted so older rows parse.
    #[allow(dead_code)]
    own_dice: Option<bool>,
}

fn prop_template(cx: &mut Ctx, at: &str, p: &RawProp) -> Option<PropTemplate> {
    Some(PropTemplate {
        def: cx.prop(at, &p.def)?,
        locked: p.locked,
        key_tag: p.key_tag.as_deref().map(|t| name(cx, at, t)),
        hidden: p.hidden,
        on: p.on,
        loot: stacks(cx, &format!("{at}.loot"), &p.loot),
        use_list: lists::list(cx, &format!("{at}.use"), p.use_list.as_deref()),
        release: lists::list(cx, &format!("{at}.release"), p.release.as_deref()),
        needs: stacks(cx, &format!("{at}.needs"), &p.needs),
        talk: p.talk.as_deref().and_then(|t| cx.dialogue(at, t)),
        label: p.label.as_deref().map(|t| say(cx, at, t)),
        night_lock: night_lock(cx, at, p.night_lock.as_deref(), p.night_hours),
    })
}

/// An empty edit is allowed only at a story's place, with a slot: it changes nothing, and says the
/// story's place must offer that slot (`slotsWanted` in stories.ts: a tale that needs a ruin's inside).
fn prop_edit(cx: &mut Ctx, at: &str, e: &RawEdit, asks_slot: bool) -> PropEdit {
    let edit = PropEdit {
        key: e.key.as_deref().map(|k| name(cx, at, k)),
        def: e.def.as_deref().and_then(|d| cx.prop(at, d)),
        locked: e.locked,
        key_tag: e.key_tag.as_deref().map(|t| name(cx, at, t)),
        hidden: e.hidden,
        loot: e.loot.as_deref().map(|l| stacks(cx, &format!("{at}.loot"), l)),
        use_list: lists::list(cx, &format!("{at}.use"), e.use_list.as_deref()),
        talk: e.talk.as_deref().and_then(|t| cx.dialogue(at, t)),
        label: e.label.as_deref().map(|t| say(cx, at, t)),
        night_lock: night_lock(cx, at, e.night_lock.as_deref(), e.night_hours),
    };
    let empty = PropEdit {
        key: None,
        def: None,
        locked: None,
        key_tag: None,
        hidden: None,
        loot: None,
        use_list: None,
        talk: None,
        label: None,
        night_lock: None,
    };
    cx.diag.need(edit != empty || asks_slot, at, "an edit that changes nothing, and not at a story's slot");
    edit
}

/// The keys a row places: `key`, or `key_1` to `key_n`.
fn keys_of(key: &str, count: u16) -> Vec<String> {
    if count == 1 { vec![key.to_owned()] } else { (1..=count).map(|i| format!("{key}_{i}")).collect() }
}

/// `data/placements/*.json`, in file then row order.
fn placements(
    src: &Source,
    cx: &mut Ctx,
    sites: &Keyed<SiteDef>,
    areas: &Keyed<AreaDef>,
    anchors: &Keyed<AnchorDef>,
    pois: &Keyed<PoiDef>,
) -> Vec<PlacementDef> {
    let rows = src.list("placements", &mut cx.diag);
    let mut out = Vec::new();
    // (key, place) of every row so far, for `on`.
    let mut earlier: Vec<(String, Option<String>)> = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let Some(r) = typed::<RawPlacement>(row, &format!("placements[{n}]"), &mut cx.diag) else { continue };
        let at = format!("{}: placements.{}", row.file, r.key);
        let a = &r.at;
        let count = r.count.unwrap_or(1);
        cx.diag.need(count >= 1, &at, "count 0");

        // Where: exactly one way of saying it.
        let ways = [
            a.mark.is_some(),
            a.site.is_some(),
            a.area.is_some(),
            a.poi.is_some(),
            a.anchor.is_some(),
            a.prop.is_some(),
            a.place.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
            + usize::from(a.slot.is_some() && a.place.is_none());
        cx.diag.need(ways == 1, &at, "at: exactly one of mark, site, area, poi, anchor, slot, prop, place");
        cx.diag.need(a.near.is_none() || a.poi.is_some(), &at, "at.near is for a poi");
        let by_position = a.dx.is_some() || a.dy.is_some() || a.on.is_some();
        cx.diag.need(
            a.place.is_some() || (!by_position && a.room.is_none()),
            &at,
            "dx, dy, on and room are for a story's place",
        );
        cx.diag.need(a.room.is_none() || a.dx.is_some() || a.dy.is_some(), &at, "room goes with dx, dy");
        cx.diag.need(
            a.place.is_none() || a.slot.is_some() || by_position,
            &at,
            "at a story's place: a slot, or dx, dy or on",
        );
        if let Some(on) = &a.on {
            let ok = earlier.iter().any(|(k, p)| k == on && p == &a.place);
            cx.diag.need(ok, &at, format!("on \"{on}\": no earlier row at the same place has that key"));
        }
        let place_at = if let Some(m) = &a.mark {
            Some(PlaceAt::Mark(name(cx, &at, m)))
        } else if let Some(s) = &a.site {
            sites.ix(cx, &at, "site", s).map(PlaceAt::Site)
        } else if let Some(s) = &a.area {
            required_area(cx, &at, areas, s).map(PlaceAt::Area)
        } else if let Some(k) = &a.poi {
            cx.diag.need(pois.ids.contains(k), &at, format!("poi \"{k}\" is not a kind in pois.json"));
            let near = match &a.near {
                Some(s) => sites.ix(cx, &at, "site", s).map(Some),
                None => Some(None),
            };
            near.map(|near| PlaceAt::Poi { kind: name(cx, &at, k), near })
        } else if let Some(s) = &a.anchor {
            anchors.ix(cx, &at, "anchor", s).map(PlaceAt::Anchor)
        } else if let Some(p) = &a.prop {
            Some(PlaceAt::Prop(name(cx, &at, p)))
        } else if let Some(story) = &a.place {
            let slot = a.slot.as_deref().map(|s| name(cx, &at, s));
            cx.story(&at, story).map(|story: StoryId| PlaceAt::Place { story, slot })
        } else {
            a.slot.as_deref().map(|s| PlaceAt::Slot(name(cx, &at, s)))
        };
        earlier.push((r.key.clone(), a.place.clone()));

        // What: an edit alone, or something to put down.
        if r.edit.is_some() {
            let alone =
                r.unit.is_none() && r.prop.is_none() && r.rect.is_none() && r.mark.is_none() && r.hides.is_none();
            cx.diag.need(
                alone && count == 1 && r.beside.is_none(),
                &at,
                "an edit places nothing: no unit, prop, rect, mark, hides, beside or count",
            );
        } else {
            cx.diag.need(
                r.unit.is_some() || r.prop.is_some() || r.rect.is_some() || r.mark.is_some(),
                &at,
                "puts nothing down: a unit, prop, rect or mark, or an edit",
            );
        }
        cx.diag.need(
            r.hides.is_none() || r.prop.is_some(),
            &at,
            "hides: under what? a hidden thing lies under the row's prop",
        );
        cx.diag.need(r.beside.is_none() || r.prop.is_some(), &at, "beside moves the row's prop, and there is none");
        let unit = r.unit.as_ref().and_then(|u| {
            if let Some(p) = u.phase {
                cx.diag.need(p <= 6, &at, format!("phase {p} is past 6"));
            }
            Some(PlacedUnit {
                def: cx.unit(&at, &u.def)?,
                phase: u.phase,
                facing: u.facing.as_ref().map(|f| match f {
                    RawFacing::East => Facing::East,
                    RawFacing::South => Facing::South,
                    RawFacing::West => Facing::West,
                    RawFacing::North => Facing::North,
                }),
            })
        });
        let rect = r.rect.as_ref().map(|x| {
            cx.diag.need(x.w >= 1 && x.h >= 1, &at, "an empty rect");
            PlacedRect { name: name(cx, &at, &x.name), w: x.w, h: x.h }
        });
        let hides = r.hides.as_ref().and_then(|h| {
            Some(Hides {
                key: name(cx, &at, &h.key),
                prop: prop_template(cx, &format!("{at}.hides"), &h.prop)?,
                when: lists::conds(cx, &format!("{at}.hides"), h.when.as_deref()),
            })
        });
        let def = PlacementDef {
            key: name(cx, &at, &r.key),
            keys: leak(keys_of(&r.key, count.max(1)).iter().map(|k| name(cx, &at, k)).collect()),
            spread: r.spread,
            at: place_at.unwrap_or(PlaceAt::Mark(NameId(0))),
            within: a.within,
            dx: a.dx,
            dy: a.dy,
            on: a.on.as_deref().map(|s| name(cx, &at, s)),
            room: a.room,
            edit: r
                .edit
                .as_ref()
                .map(|e| prop_edit(cx, &format!("{at}.edit"), e, a.place.is_some() && a.slot.is_some())),
            unit,
            prop: r.prop.as_ref().and_then(|p| prop_template(cx, &format!("{at}.prop"), p)),
            rect,
            mark: r.mark.as_deref().map(|m| name(cx, &at, m)),
            hides,
            beside: r.beside.as_deref().map(|b| name(cx, &at, b)),
        };
        out.push(def);
    }
    out
}

/// What the placement rows promise the county on every seed (placements.ts `placementContract`):
/// not an edit (the chunk promised that prop), not a story's row (there only when it found a place).
fn promised(rows: &[PlacementDef]) -> Contract {
    let (mut units, mut props, mut marks, mut rects) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for r in rows.iter().filter(|r| r.edit.is_none() && !r.at_place()) {
        for k in r.keys {
            if r.unit.is_some() {
                units.push(*k);
            }
            if r.prop.is_some() {
                props.push(*k);
            }
        }
        if let Some(h) = r.hides {
            props.push(h.key);
        }
        if let Some(m) = r.mark {
            marks.push(m);
        }
        if let Some(x) = r.rect {
            rects.push(x.name);
        }
    }
    Contract { units: leak(units), props: leak(props), marks: leak(marks), rects: leak(rects) }
}

/// A thing is set down once: no two providers in a zone name the same unit, prop, mark or rect (a
/// second prop with one key fails the blueprint's "no duplicate keys" on every seed). An edit's new
/// key is the thing it edits, so it is not a second provider.
fn once_each(cx: &mut Ctx, c: &model::County) {
    let mut seen: Vec<(ZoneId, NameKind, NameId, ProvidedBy)> = Vec::new();
    let renames: Vec<NameId> = c.placements.iter().filter_map(|p| p.edit.and_then(|e| e.key)).collect();
    for p in c.provides() {
        if p.kind == NameKind::Prop && renames.contains(&p.name) && p.by != ProvidedBy::Zones {
            continue;
        }
        if let Some((_, _, _, by)) = seen.iter().find(|(z, k, n, _)| *z == p.zone && *k == p.kind && *n == p.name) {
            let what = cx.names.strings().nth(p.name.index()).unwrap_or_default().to_owned();
            cx.diag
                .error(p.zone.name(), format!("{:?} \"{what}\" is provided twice ({:?} and {:?})", p.kind, by, p.by));
            continue;
        }
        seen.push((p.zone, p.kind, p.name, p.by));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ctx::Ids;

    /// The thirteen zones in order, the county with `start`, every other zone an interior with `entry`.
    fn zones_json() -> String {
        let rows: Vec<String> = ZoneId::ALL
            .iter()
            .map(|z| match z {
                ZoneId::County => r#"{"id": "county", "kind": "county", "contract": {"marks": ["start"]}}"#.to_owned(),
                z => format!(r#"{{"id": "{}", "kind": "interior", "contract": {{"marks": ["entry"]}}}}"#, z.name()),
            })
            .collect();
        format!("[{}]", rows.join(",\n"))
    }

    /// Eleven names a kind (a story each for the cottages and the camps, and ten to spare), none inside another.
    fn names_json() -> String {
        let eleven = |w: &str| {
            format!(
                "[{}]",
                (0..11).map(|i| format!("\"{w} Q{}\"", char::from(b'a' + i))).collect::<Vec<_>>().join(", ")
            )
        };
        let boards = r#"{"hamlet": ["{NAME}."], "farmstead": ["{NAME}."], "inn": ["{NAME}."], "cottage": ["{NAME}."], "woodcutter": ["{NAME}."], "camp": ["{NAME}."], "ruin": ["{NAME}."]}"#;
        format!(
            r#"{{"hamlet": {}, "farmstead": {}, "inn": ["The Halfway House"], "cottage": {}, "woodcutter": {{"roots": {}, "ends": ["Copse"]}}, "camp": {}, "ruin": {}, "boards": {boards}}}"#,
            eleven("Hamlet"),
            eleven("Farm"),
            eleven("Cottage"),
            eleven("Root"),
            eleven("Camp"),
            eleven("Ruin")
        )
    }

    const SITES: &str = r#"[
        {"id": "station", "name": "Castle Halt", "region": "lowfields", "onRoad": true},
        {"id": "town", "name": "Castle", "region": "lowfields", "onRoad": true, "dist": [{"to": "station", "min": 100, "by": "road"}], "roadFrom": "station"}
    ]"#;
    const AREAS: &str = r#"[
        {"id": "top_field", "required": true, "name": "The Top Field", "region": "lowfields", "threat": 3, "radius": 140},
        {"id": "sidings", "name": "The Sidings", "region": "works", "threat": 5, "radius": 140}
    ]"#;
    const STORIES: &str = r#"[
        {"id": "ames", "kind": "cottage", "quests": ["ames_spectacles"]},
        {"id": "ames_camp", "kind": "camp", "near": {"story": "ames", "max": 420}, "hostile": "crow", "count": 2, "quests": []}
    ]"#;
    const PLACEMENTS: &str = r#"[
        {"key": "ames_door", "at": {"place": "ames", "slot": "house"}, "edit": {"talk": "ames"}},
        {"key": "camp_chest", "at": {"place": "ames_camp", "slot": "chest"}, "edit": {}},
        {"key": "pumpkin", "count": 3, "spread": true, "at": {"area": "top_field"}, "unit": {"def": "crow"}},
        {"key": "board", "at": {"mark": "start", "within": 4}, "prop": {"def": "sign", "label": "A board"}, "mark": "board_front"}
    ]"#;

    /// The fixture's files, with `edits` replacing or adding files by path.
    fn files(edits: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = vec![
            ("zones.json".into(), zones_json()),
            ("names.json".into(), names_json()),
            ("sites.json".into(), SITES.into()),
            ("areas.json".into(), AREAS.into()),
            ("pois.json".into(), r#"[{"kind": "well", "name": "A well", "regions": ["lowfields"], "weight": 3, "where": "roadside"}]"#.into()),
            ("anchors.json".into(), r#"[{"id": "halt_well", "kind": "well", "name": "The well", "where": {"road": ["station", "town"]}}]"#.into()),
            ("paths.json".into(), r#"[{"id": "p", "from": "town", "via": "halt_well", "to": "station", "width": 2, "marks": ["p_a", "p_b"]}]"#.into()),
            ("doors.json".into(), r#"[{"zone": "library", "near": "town", "key": "library_door", "label": "The library"}]"#.into()),
            ("stories/lowfields.json".into(), STORIES.into()),
            ("placements/lowfields.json".into(), PLACEMENTS.into()),
            ("props.json".into(), r#"{"door": {}, "sign": {}}"#.into()),
            ("units.json".into(), r#"{"crow": {}}"#.into()),
            ("quests.json".into(), r#"{"ames_spectacles": {}, "other": {}}"#.into()),
            ("dialogue.json".into(), r#"{"ames": {}}"#.into()),
        ];
        for (path, text) in edits {
            match out.iter_mut().find(|(p, _)| p == path) {
                Some(f) => f.1 = (*text).to_owned(),
                None => out.push(((*path).to_owned(), (*text).to_owned())),
            }
        }
        out
    }

    /// This group's compile alone over the fixture, so no other group's tables can speak.
    fn run(edits: &[(&str, &str)]) -> (model::County, Vec<String>) {
        let f = files(edits);
        let refs: Vec<(&str, &str)> = f.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let src = Source::from_files(&refs).expect("fixture JSON");
        let mut cx = Ctx::default();
        cx.ids = Ids::collect(&src, &mut cx.diag);
        let county = compile(&src, &mut cx);
        (county, cx.diag.errors.iter().map(ToString::to_string).collect())
    }

    fn fails(edits: &[(&str, &str)], needle: &str) {
        let (_, errors) = run(edits);
        assert!(errors.iter().any(|e| e.contains(needle)), "no error containing {needle:?} in {errors:#?}");
    }

    #[test]
    fn the_fixture_compiles_clean() {
        let (c, errors) = run(&[]);
        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(c.zones.len(), 13);
        assert!(c.zones.iter().zip(ZoneId::ALL).all(|(z, id)| z.id == id));
        assert_eq!(c.promised.units.len(), 3, "pumpkin_1..3");
        assert_eq!(c.promised.props.len(), 1);
        assert_eq!(c.doors[0].to, c.zone(ZoneId::Library).contract.marks.first().copied());
        let camp = &c.stories[1];
        assert_eq!((camp.count, camp.hostile.len()), (2, 1));
        assert_eq!(c.story(camp.id).key, "ames_camp");
        // Eleven a kind, and the woodcutters' eleven roots by one end.
        assert!(c.names.iter().all(|p| p.names.len() == if p.kind == PlaceKind::Inn { 1 } else { 11 }));
    }

    #[test]
    fn zones_are_in_tick_order() {
        let swapped = zones_json().replacen("\"house\"", "\"TMP\"", 1).replacen("\"cellar\"", "\"house\"", 1).replacen(
            "\"TMP\"",
            "\"cellar\"",
            1,
        );
        fails(&[("zones.json", &swapped)], "tick order");
    }

    #[test]
    fn a_dungeon_names_its_mission_and_nothing_its_mission_says() {
        let z = zones_json().replace(
            r#"{"id": "school", "kind": "interior", "contract": {"marks": ["entry"]}}"#,
            r#"{"id": "school", "kind": "dungeon", "givenKeys": ["x"]}"#,
        );
        fails(&[("zones.json", &z)], "names its mission");
        fails(&[("zones.json", &z)], "givenKeys are its mission's");
    }

    #[test]
    fn every_door_leads_into_one_of_the_thirteen_and_arrives_at_a_mark_there() {
        // PORT.md §6.l: the dead icehouse row is an error now, not a door that never grows.
        fails(
            &[("doors.json", r#"[{"zone": "icehouse", "chunk": "town", "key": "d", "label": "The ice house"}]"#)],
            "unknown zone \"icehouse\"",
        );
        let z = zones_json().replace(
            r#"{"id": "library", "kind": "interior", "contract": {"marks": ["entry"]}}"#,
            r#"{"id": "library", "kind": "interior"}"#,
        );
        fails(&[("zones.json", &z)], "provides no such mark");
        fails(&[("doors.json", r#"[{"zone": "library", "key": "d", "label": "x"}]"#)], "exactly one of chunk");
        fails(
            &[("doors.json", r#"[{"zone": "pipes", "near": "town", "key": "d", "label": "x", "fromBelow": true}]"#)],
            "arrives nowhere",
        );
    }

    #[test]
    fn a_mission_binds_a_mark_on_a_critical_node() {
        let src = Source::from_files(&[(
            "dungeons/pipes.json",
            r#"{"nodes": [{"critical": false, "binds": [{"what": "mark", "as": "side"}]}, {"critical": true, "binds": [{"what": "mark", "as": "entry"}, {"what": "prop", "as": "gate"}]}]}"#,
        )])
        .unwrap();
        assert!(mission_binds_mark(&src, "pipes", "entry"));
        assert!(!mission_binds_mark(&src, "pipes", "side"), "not critical: not in the contract");
        assert!(!mission_binds_mark(&src, "pipes", "gate"), "a prop, not a mark");
        assert!(!mission_binds_mark(&src, "school", "entry"));
    }

    #[test]
    fn sites_lean_only_on_earlier_rows() {
        let later = r#"[
            {"id": "station", "name": "Castle Halt", "region": "lowfields", "onRoad": true, "roadFrom": "town"},
            {"id": "town", "name": "Castle", "region": "lowfields", "onRoad": true}
        ]"#;
        fails(&[("sites.json", later)], "must be an earlier row");
        fails(&[("sites.json", &SITES.replace("\"to\": \"station\"", "\"to\": \"farm\""))], "unknown site \"farm\"");
    }

    #[test]
    fn a_patch_a_story_names_is_required() {
        let rows = PLACEMENTS.replace(r#""area": "top_field""#, r#""area": "sidings""#);
        fails(&[("placements/lowfields.json", &rows)], "must be \"required\"");
    }

    #[test]
    fn stories_are_whole() {
        let twice = r#"[{"id": "a", "kind": "cottage", "quests": ["ames_spectacles"]}, {"id": "b", "kind": "cottage", "quests": ["ames_spectacles"]}]"#;
        fails(&[("stories/lowfields.json", twice)], "told in two stories");
        let dup =
            r#"[{"id": "ames", "kind": "cottage", "quests": []}, {"id": "ames", "kind": "cottage", "quests": []}]"#;
        fails(&[("stories/lowfields.json", dup)], "defined twice");
        let looped = r#"[{"id": "ames", "kind": "cottage", "near": {"story": "ames_camp", "max": 9}, "quests": []}, {"id": "ames_camp", "kind": "camp", "near": {"story": "ames", "max": 9}, "quests": []}]"#;
        fails(&[("stories/lowfields.json", looped)], "loops");
        fails(&[("stories/lowfields.json", &STORIES.replace("ames_spectacles", "nope"))], "unknown quest \"nope\"");
        fails(&[("stories/lowfields.json", &STORIES.replace("\"crow\"", "\"crow|bat\""))], "unknown unit def \"bat\"");
        fails(
            &[("stories/lowfields.json", &STORIES.replace("\"kind\": \"cottage\"", "\"kind\": \"castle\""))],
            "unknown variant",
        );
        let unused = format!(
            "{}, {{\"id\": \"lonely\", \"kind\": \"ruin\", \"quests\": []}}]",
            STORIES.trim_end().trim_end_matches(']')
        );
        fails(&[("stories/lowfields.json", &unused)], "no placement row puts anything at its place");
        fails(
            &[("quests.json", r#"{"ames_spectacles": {"text": "At {place:nowhere}."}}"#)],
            "{place:nowhere} names no story",
        );
    }

    #[test]
    fn tales_have_names_of_their_own() {
        let tale = STORIES.replace(r#""kind": "cottage","#, r#""kind": "cottage", "tale": true,"#);
        fails(&[("stories/lowfields.json", &tale)], "fixed name of its own");
        let hidden =
            STORIES.replace(r#""kind": "cottage","#, r#""kind": "cottage", "tale": true, "name": "Cottage Qa","#);
        fails(&[("stories/lowfields.json", &hidden)], "hide in each other");
    }

    #[test]
    fn name_pools_are_unique_and_nothing_hides_inside_another() {
        let doubled = names_json().replacen("Hamlet Qb", "Hamlet Qa", 1);
        fails(&[("names.json", &doubled)], "in the list twice");
        let inside = names_json().replacen("Hamlet Qb", "Farm Qa End", 1);
        fails(&[("names.json", &inside)], "is inside");
        let short = names_json().replacen(", \"Cottage Qk\"", "", 1);
        fails(&[("names.json", &short)], "ten more are wanted");
        fails(&[("names.json", &names_json().replacen("Hamlet Qa", "Hamlet \u{2014} Qa", 1))], "en or em dash");
        fails(&[("names.json", &names_json().replacen("Hamlet Qa", "Jane's End", 1))], "\"Jane\" is baked");
    }

    #[test]
    fn placements_say_where_one_way_and_on_an_earlier_row() {
        let two = PLACEMENTS.replace(r#"{"mark": "start", "within": 4}"#, r#"{"mark": "start", "site": "town"}"#);
        fails(&[("placements/lowfields.json", &two)], "exactly one of");
        let on = r#"[{"key": "a", "at": {"place": "ames", "on": "b", "dx": 1}, "mark": "m"}, {"key": "b", "at": {"place": "ames", "slot": "house"}, "edit": {"talk": "ames"}}, {"key": "c", "at": {"place": "ames_camp", "slot": "chest"}, "edit": {}}]"#;
        fails(&[("placements/lowfields.json", on)], "no earlier row at the same place");
        let empty = PLACEMENTS.replace(r#""edit": {"talk": "ames"}"#, r#""edit": {}, "mark": "x""#);
        fails(&[("placements/lowfields.json", &empty)], "an edit places nothing");
        let nothing = PLACEMENTS
            .replace(r#", "mark": "board_front""#, "")
            .replace(r#", "prop": {"def": "sign", "label": "A board"}"#, "");
        fails(&[("placements/lowfields.json", &nothing)], "puts nothing down");
        let hides = PLACEMENTS.replace(
            r#""unit": {"def": "crow"}"#,
            r#""unit": {"def": "crow"}, "hides": {"key": "k", "prop": {"def": "sign"}}"#,
        );
        fails(&[("placements/lowfields.json", &hides)], "under what");
        fails(
            &[("placements/lowfields.json", &PLACEMENTS.replace("\"sign\"", "\"signe\""))],
            "unknown prop def \"signe\"",
        );
        fails(
            &[("placements/lowfields.json", &PLACEMENTS.replace("\"within\": 4", "\"withn\": 4"))],
            "unknown field `withn`",
        );
    }

    #[test]
    fn a_name_is_provided_once() {
        // The placement promises a prop the zone contract already names: two props with one key.
        let z = zones_json().replace(r#"{"marks": ["start"]}"#, r#"{"marks": ["start"], "props": ["board"]}"#);
        fails(&[("zones.json", &z)], "provided twice");
        // A path end that is also a placement's mark.
        fails(
            &[(
                "paths.json",
                r#"[{"id": "p", "from": "town", "via": "halt_well", "to": "station", "width": 2, "marks": ["board_front", "p_b"]}]"#,
            )],
            "provided twice",
        );
    }

    #[test]
    fn provides_names_every_source() {
        let (c, errors) = run(&[]);
        assert!(errors.is_empty(), "{errors:#?}");
        let by = |b: ProvidedBy| c.provides().into_iter().filter(|p| p.by == b).count();
        assert_eq!(by(ProvidedBy::Zones), 13, "one mark per zone");
        assert_eq!(by(ProvidedBy::Placement), 3 + 2, "three pumpkins, the board and its mark");
        assert_eq!(by(ProvidedBy::Story), 4, "two stories' rect and mark; their rows only edit");
        assert_eq!(by(ProvidedBy::Anchor), 2);
        assert_eq!(by(ProvidedBy::Area), 2);
        assert_eq!(by(ProvidedBy::Path), 2);
        assert_eq!(by(ProvidedBy::Door), 1);
    }

    #[test]
    fn woodcutters_are_ends_by_roots_less_the_doubled() {
        let w = RawWoodcutter {
            roots: vec!["Hazel".into(), "Coppice".into()],
            ends: vec!["Copse".into(), "Coppice Wood".into()],
        };
        assert_eq!(clearings(&w), ["Hazel Copse", "Coppice Copse", "Hazel Coppice Wood"]);
    }

    #[test]
    fn keys_expand_with_count() {
        assert_eq!(keys_of("stone", 1), ["stone"]);
        assert_eq!(keys_of("stone", 3), ["stone_1", "stone_2", "stone_3"]);
    }
}
