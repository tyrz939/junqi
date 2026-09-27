//! Compile the atmosphere's layers (WORLD.md §5.3, PRESENTATION.md §1.9) into
//! [`model::Atmosphere`]: `data/atmosphere.json` (and `data/atmosphere/*.json`), keyed by layer id.
//!
//! The checks: every layer is keyed to somewhere (an area, a rect, a region or a zone); a zone
//! and a region are ones that exist; hours are 0..=23; it needs at least one kind of sky; the
//! colour is `#rrggbb`; density is 1..=1000 per mille; a ground layer has a `top`. That an area
//! or a rect is one the county places needs a seed: `jane-present`'s test.
//!
//! A source with no `atmosphere.json` (a fixture of another group) compiles to no layers.

use serde::Deserialize;

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::source::{Source, typed};
use crate::model::{self, AtmosLayer, FogHeight, Region};

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawRegion {
    Lowfields,
    Waters,
    Works,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawSky {
    Clear,
    Mist,
    Rain,
    Storm,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawHeight {
    Ground,
    Full,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawLayer {
    #[serde(default)]
    areas: Vec<String>,
    #[serde(default)]
    rects: Vec<String>,
    #[serde(default)]
    regions: Vec<RawRegion>,
    #[serde(default)]
    zones: Vec<String>,
    hours: [u8; 2],
    needs: Vec<RawSky>,
    colour: String,
    density: u16,
    height: RawHeight,
    #[serde(default)]
    top: u8,
    #[serde(default)]
    drift: [i8; 2],
    #[serde(default)]
    spread: u8,
}

fn hex(s: &str) -> Option<u32> {
    let h = s.strip_prefix('#')?;
    (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten()
}

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Atmosphere {
    let table = "atmosphere";
    if src.file("atmosphere.json").is_none() {
        return model::Atmosphere::EMPTY;
    }
    let rows = src.table(table, &mut cx.diag);
    let mut layers = Vec::with_capacity(rows.len());
    for (id, row) in &rows {
        let at = format!("{}: {table}.{id}", row.file);
        let Some(r) = typed::<RawLayer>(row, &at, &mut cx.diag) else { continue };
        let zones: Vec<_> = r.zones.iter().filter_map(|z| cx.zone(&format!("{at}.zones"), z)).collect();
        cx.diag.need(
            !(r.areas.is_empty() && r.rects.is_empty() && r.regions.is_empty() && zones.is_empty()),
            &at,
            "a layer is keyed to somewhere: areas, rects, regions or zones",
        );
        cx.diag.need(r.hours.iter().all(|&h| h < 24), format!("{at}.hours"), "hours are 0..=23");
        cx.diag.need(!r.needs.is_empty(), format!("{at}.needs"), "a layer shows under at least one sky");
        let colour = hex(&r.colour);
        cx.diag.need(colour.is_some(), format!("{at}.colour"), "is #rrggbb");
        cx.diag.need((1..=1000).contains(&r.density), format!("{at}.density"), "is 1..=1000 per mille");
        cx.diag.need(r.height != RawHeight::Ground || r.top > 0, format!("{at}.top"), "a ground layer stands `top` px");
        let mut needs = [false; 4];
        for s in &r.needs {
            needs[*s as usize] = true;
        }
        layers.push(AtmosLayer {
            id: leak_str(id),
            areas: leak(r.areas.iter().map(|s| leak_str(s)).collect()),
            rects: leak(r.rects.iter().map(|s| leak_str(s)).collect()),
            regions: leak(
                r.regions
                    .iter()
                    .map(|g| match g {
                        RawRegion::Lowfields => Region::Lowfields,
                        RawRegion::Waters => Region::Waters,
                        RawRegion::Works => Region::Works,
                    })
                    .collect(),
            ),
            zones: leak(zones),
            hour_from: r.hours[0],
            hour_to: r.hours[1],
            needs,
            colour: colour.unwrap_or(0),
            density: r.density,
            height: match r.height {
                RawHeight::Ground => FogHeight::Ground,
                RawHeight::Full => FogHeight::Full,
            },
            top: r.top,
            drift: (r.drift[0], r.drift[1]),
            spread: r.spread,
        });
    }
    layers.sort_by_key(|l| l.id);
    model::Atmosphere { layers: leak(layers) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::diag::Diagnostics;

    fn build(json: &str) -> (model::Atmosphere, Diagnostics) {
        let src = Source::from_files(&[("atmosphere.json", json)]).unwrap();
        let mut cx = Ctx::default();
        let a = compile(&src, &mut cx);
        (a, cx.diag)
    }

    #[test]
    fn a_layer_is_checked_and_compiled() {
        let ok = r##"{ "fen": { "areas": ["eel_beds"], "hours": [19, 8], "needs": ["clear", "mist"],
            "colour": "#d8e2e8", "density": 500, "height": "ground", "top": 24, "drift": [4, 0], "spread": 3 } }"##;
        let (a, d) = build(ok);
        assert!(d.is_ok(), "{d}");
        let l = a.layers[0];
        assert_eq!((l.id, l.areas, l.colour, l.top, l.drift), ("fen", &["eel_beds"][..], 0xd8e2e8, 24, (4, 0)));
        assert!(l.shows(22, 0) && l.shows(3, 1) && !l.shows(12, 0) && !l.shows(22, 2));
        let (_, d) = build(&ok.replace(r#""areas": ["eel_beds"], "#, ""));
        assert!(!d.is_ok(), "keyed to nowhere");
        let (_, d) = build(&ok.replace("#d8e2e8", "blue"));
        assert!(!d.is_ok(), "a bad colour");
        let (_, d) = build(&ok.replace(r#""top": 24, "#, ""));
        assert!(!d.is_ok(), "a ground layer with no top");
        let (_, d) = build(&ok.replace("\"drift\"", "\"drfit\""));
        assert!(!d.is_ok(), "an unknown field");
    }
}
