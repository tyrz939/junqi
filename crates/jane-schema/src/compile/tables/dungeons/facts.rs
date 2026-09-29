//! What the mission lint needs to know about other tables, read from their raw rows the way
//! `combat` reads quests (never through another group's model): prop footprints and what a
//! prop answers, the `opens` tag of an item, what a unit gives when it dies, what a dialogue
//! tree can hand over. Rows that do not type are the owning group's errors; here they are skipped.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::diag::Diagnostics;
use crate::compile::fraction::Num;
use crate::compile::lists::RawAction;
use crate::compile::source::Source;

/// A prop row's footprint and verb.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropFact {
    pub w: i64,
    pub h: i64,
    /// The spell it answers to, if any.
    pub answers: Option<String>,
    /// Nothing to push, carry, use, rest or craft at, no light, no gate or plate, nothing it
    /// hides from sight: what a set piece may be made of (DUNGEONS.md §2.10).
    pub inert: bool,
}

/// One thing a list, a death or a talk can give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gain {
    Item(String, i64),
    Learn(String),
    Flag(String),
}

#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub props: BTreeMap<String, PropFact>,
    pub item_opens: BTreeMap<String, String>,
    pub unit_gains: BTreeMap<String, Vec<Gain>>,
    pub dialogue_gains: BTreeMap<String, Vec<Gain>>,
}

/// `eachAction` over a list as raw JSON.
pub fn value_gains(list: Option<&Value>, out: &mut Vec<Gain>) {
    for a in list.and_then(Value::as_array).into_iter().flatten() {
        let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_owned);
        match a.get("do").and_then(Value::as_str) {
            Some("give") => {
                if let Some(item) = s("item") {
                    out.push(Gain::Item(item, a.get("qty").and_then(Value::as_i64).unwrap_or(1)));
                }
            }
            Some("learn") => out.extend(s("spell").map(Gain::Learn)),
            Some("flag") => out.extend(s("flag").map(Gain::Flag)),
            Some("if") => {
                value_gains(a.get("then"), out);
                value_gains(a.get("else"), out);
            }
            Some("send") => value_gains(a.get("then"), out),
            _ => {}
        }
    }
}

/// `eachAction` over a typed list.
pub fn action_gains(list: Option<&[RawAction]>, out: &mut Vec<Gain>) {
    for a in list.into_iter().flatten() {
        match a {
            RawAction::Give { item, qty } => out.push(Gain::Item(item.clone(), i64::from(qty.unwrap_or(1)))),
            RawAction::Learn { spell } => out.push(Gain::Learn(spell.clone())),
            RawAction::Flag { flag, .. } => out.push(Gain::Flag(flag.clone())),
            RawAction::If { then, els, .. } => {
                action_gains(Some(then), out);
                action_gains(els.as_deref(), out);
            }
            RawAction::Send { then, .. } => action_gains(then.as_deref(), out),
            _ => {}
        }
    }
}

impl Facts {
    pub fn read(src: &Source) -> Facts {
        let mut quiet = Diagnostics::default();
        let mut f = Facts::default();
        for (id, row) in src.table("props", &mut quiet) {
            let v = &row.value;
            let set = |k: &str| v.get(k).is_some_and(|x| !x.is_null() && x != &Value::Bool(false));
            let inert = ![
                "push", "carry", "bench", "store", "answers", "once", "gate", "plate", "rest", "light", "prompt",
                "blockLos",
            ]
            .iter()
            .any(|k| set(k));
            f.props.insert(
                id,
                PropFact {
                    w: v.get("w").and_then(Value::as_i64).unwrap_or(0),
                    h: v.get("h").and_then(Value::as_i64).unwrap_or(0),
                    answers: v.get("answers").and_then(Value::as_str).map(str::to_owned),
                    inert,
                },
            );
        }
        for (id, row) in src.table("items", &mut quiet) {
            if let Some(tag) = row.value.get("opens").and_then(Value::as_str) {
                f.item_opens.insert(id, tag.to_owned());
            }
        }
        for (id, row) in src.table("units", &mut quiet) {
            let mut g = Vec::new();
            for l in row.value.get("loot").and_then(Value::as_array).into_iter().flatten() {
                let chance = l.get("chance").and_then(|c| serde_json::from_value::<Num>(c.clone()).ok());
                let sure = chance.and_then(|c| c.permille().ok()).is_some_and(|p| p.0 >= 1000);
                if let (true, Some(item)) = (sure, l.get("item").and_then(Value::as_str)) {
                    g.push(Gain::Item(item.to_owned(), l.get("qty").and_then(Value::as_i64).unwrap_or(1)));
                }
            }
            value_gains(row.value.get("onDeath"), &mut g);
            f.unit_gains.insert(id, g);
        }
        for (id, row) in src.table("dialogue", &mut quiet) {
            let mut g = Vec::new();
            for n in row.value.get("nodes").and_then(Value::as_object).into_iter().flat_map(|o| o.values()) {
                value_gains(n.get("actions"), &mut g);
                for o in n.get("options").and_then(Value::as_array).into_iter().flatten() {
                    value_gains(o.get("actions"), &mut g);
                }
            }
            f.dialogue_gains.insert(id, g);
        }
        f
    }

    pub fn prop(&self, id: &str) -> Option<&PropFact> {
        self.props.get(id)
    }

    /// The four wall rows of a lamp family (`gas_lamp` -> `gas_lamp_n`, `_e`, `_s`, `_w`), when
    /// `id` is not a row itself and all four are.
    pub fn family(&self, id: &str) -> Option<[String; 4]> {
        if self.props.contains_key(id) {
            return None;
        }
        let rows = ["n", "e", "s", "w"].map(|s| format!("{id}_{s}"));
        rows.iter().all(|r| self.props.contains_key(r)).then_some(rows)
    }
}
