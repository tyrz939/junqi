//! Compile the `dungeons` group into [`model::Dungeons`]: the missions in
//! `data/dungeons/*.json` and the room templates in `data/rooms/<dungeon>/*.room`.
//!
//! Rooms are parsed, linted (rules 1-6 of `room.ts`) and turned into every transform they
//! allow; pools are checked to be interfaces; missions are typed with `deny_unknown_fields`
//! and linted (`lintDef`, C3, node order, pools, binds, placeholders, edges, states, the
//! fallback stamp) before they are compiled into the model.
//!
//! **For the zones table.** The eight dungeon zones are mission-driven: their contract, given
//! keys and verbs, and states come from the mission, not from `zones.json`. Two ways to read
//! them: `CATALOG.dungeons.mission_of(zone)` (`MissionDef::contract`, `given_keys`,
//! `given_verbs`, `states`, `provides`, as ids), or [`derived_zones`] (the same as strings,
//! straight from a `Source`, callable from any table's compile in any order and silent about
//! errors, which this module reports once).

mod check;
mod facts;
mod fallback;
mod names;
mod pools;
mod raw;
mod room;
mod typed;

use crate::compile::ctx::{Ctx, leak};
use crate::compile::diag::Diagnostics;
use crate::compile::source::{Row, Source, typed as type_row};
use crate::model::{self, MissionNameWhat};

use facts::Facts;
use pools::Library;
use raw::RawMission;

pub use room::{Room, lint as lint_room, parse as parse_room, shapes_of};

/// Every mission, typed, in file order (which is `DungeonId` order).
fn read_missions(src: &Source, diag: &mut Diagnostics) -> Vec<(String, RawMission)> {
    let mut out = Vec::new();
    for (file, v) in src.files_in("dungeons") {
        let row = Row { file: file.to_owned(), value: v.clone() };
        if let Some(m) = type_row::<RawMission>(&row, "mission", diag) {
            out.push((file.to_owned(), m));
        }
    }
    out
}

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Dungeons {
    let lib = Library::read(src, &mut cx.diag);
    pools::check_interfaces(&lib, &mut cx.diag);
    let missions = read_missions(src, &mut cx.diag);
    let fx = Facts::read(src);
    for (file, m) in &missions {
        check::check_mission(file, m, &lib, &fx, &mut cx.diag);
    }
    if lib.rooms.is_empty() && missions.is_empty() {
        return typed::empty();
    }
    let (template_ids, templates, pool_ids, pools) = typed::rooms(cx, &fx, &lib);
    let missions: Vec<model::MissionDef> =
        missions.iter().map(|(file, m)| typed::mission(cx, &fx, &lib, (&template_ids, &pool_ids), file, m)).collect();
    model::Dungeons { missions: leak(missions), templates, pools }
}

/// A dungeon zone's rows, derived from its mission (`dungeon/index.ts` `dungeonZone`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedZone {
    /// The zone id (`"mine"`).
    pub zone: String,
    /// `contractOf`: binds of critical nodes, `gateAs`/`propAs` of edges between them.
    pub units: Vec<String>,
    pub props: Vec<String>,
    pub marks: Vec<String>,
    pub rects: Vec<String>,
    pub given_keys: Vec<String>,
    pub given_verbs: Vec<String>,
    /// `solveOptionsOf(def).states`: (state id, the flag it lives in).
    pub states: Vec<(String, String)>,
    /// Every name a blueprint of the mission can hold (the provider check's source).
    pub provides: Vec<(String, MissionNameWhat)>,
}

/// Every mission's derived zone, as strings, read quietly from the source. A mission that
/// does not type is left out (the dungeons compile reports it).
pub fn derived_zones(src: &Source) -> Vec<DerivedZone> {
    let mut quiet = Diagnostics::default();
    let lib = Library::read(src, &mut quiet);
    read_missions(src, &mut quiet)
        .into_iter()
        .map(|(_, m)| {
            let c = names::contract_of(&m);
            let min_cost = m.budget.enemies.values().copied().min().map_or(0, i64::from);
            DerivedZone {
                zone: m.id.clone(),
                units: c.units,
                props: c.props,
                marks: c.marks,
                rects: c.rects,
                given_keys: m.given_keys.clone(),
                given_verbs: m.given_verbs.clone(),
                states: m.states.iter().map(|s| (s.id.clone(), s.flag.clone())).collect(),
                provides: names::provides(&m, &lib, i64::from(m.budget.base_heat), min_cost),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
