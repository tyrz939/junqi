//! Compile the `story` group into [`model::Story`]: props, quests, dialogue, triggers, the clock
//! and the start, with every check `validateCatalog` made on them, plus:
//!
//! - `{place:<story>}` in any of this group's English names a story that exists (the TypeScript
//!   left an unknown one verbatim on screen);
//! - a quest's `returnTo` is a declared, non-empty field (the TypeScript read it through a cast);
//! - a location step asks for exactly one visit (more can never be counted);
//! - a clock row holds no player-scoped verb, however deep (it runs with no actor,
//!   ARCHITECTURE.md §5.2): an error, since no row has one today;
//! - every quest is given (`quest` verb or `start.quests`) and handed in (`handin` verb) by some
//!   list anywhere in `/data`: a warning, since the lists live in every group's tables;
//! - a dialogue node nothing leads to, a `while` trigger with no `when`, and a bar item the start
//!   kit does not carry are warnings.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use jane_core::action::{Action, FactKey, ListRef, Stack, Thing};
use jane_core::blueprint::{Trigger, TriggerMode};
use jane_core::ids::{NameId, QuestId, TextId};

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::fraction::Num;
use crate::compile::lists::{self, RawAction, RawCond};
use crate::compile::source::{Row, Source, typed};
use crate::model::{
    self, Answers, BAR_SLOTS, BarSlot, ClockDef, DialogueLine, DialogueNode, DialogueOption, DialogueStart,
    DialogueTree, FEET_SIDE_SLACK, Light, PropDef, QuestDef, QuestReq, ReqTarget, StartDef, TriggerDef,
};

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Story {
    let props = props(src, cx);
    let quests = quests(src, cx);
    let dialogue = dialogue(src, cx);
    let triggers = triggers(src, cx);
    let clock = clock(src, cx);
    let start = start(src, cx);
    let omens = omens(src, cx);
    quests_given_and_handed_in(src, cx, &start);
    model::Story { props, quests, dialogue, triggers, clock, start, omens }
}

/// Rows indexed by their table's ids. A row that failed to type leaves a hole, the diagnostics
/// say why and no catalog is produced, so the table is left empty rather than padded.
fn by_id<T>(rows: Vec<Option<T>>) -> &'static [T] {
    rows.into_iter().collect::<Option<Vec<T>>>().map_or(&[], leak)
}

/// English, with every `{place:<story>}` in it checked against the stories.
fn text(cx: &mut Ctx, at: &str, s: &str) -> TextId {
    let mut rest = s;
    while let Some(i) = rest.find("{place:") {
        let after = &rest[i + "{place:".len()..];
        let Some(end) = after.find('}') else {
            cx.diag.error(at, format!("unclosed {{place:...}} in \"{s}\""));
            break;
        };
        let id = &after[..end];
        let well_formed =
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !well_formed || cx.ids.stories.get(id).is_none() {
            cx.diag.error(at, format!("{{place:{id}}} names no story"));
        }
        rest = &after[end + 1..];
    }
    cx.text(s)
}

/// A list that is always there.
fn list(cx: &mut Ctx, at: &str, raw: &[RawAction]) -> ListRef {
    match lists::list(cx, at, Some(raw)) {
        Some(l) => l,
        None => cx.push_list(Vec::new()),
    }
}

fn name(cx: &mut Ctx, at: &str, what: &str, s: &str) -> NameId {
    cx.diag.need(!s.is_empty(), at, format!("{what} is empty"));
    cx.name(s)
}

// ---------------------------------------------------------------------------------------------
// Props

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLight {
    radius: Num,
    color: String,
    flicker: Num,
    #[serde(default)]
    cold: bool,
    #[serde(default)]
    sky: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawProp {
    name: String,
    sprite: String,
    w: u8,
    h: u8,
    base: Option<u8>,
    solid: bool,
    block_los: bool,
    #[serde(default)]
    push: bool,
    #[serde(default)]
    carry: bool,
    #[serde(default)]
    bench: bool,
    #[serde(default)]
    store: bool,
    answers: Option<String>,
    #[serde(default)]
    once: bool,
    #[serde(default)]
    gate: bool,
    #[serde(default)]
    plate: bool,
    #[serde(default)]
    light_when_on: bool,
    #[serde(default)]
    night_only: bool,
    #[serde(default)]
    day_only: bool,
    #[serde(default)]
    rest: bool,
    #[serde(default)]
    hide_when_used: bool,
    #[serde(default)]
    regrow: bool,
    light: Option<RawLight>,
    douse: Option<u8>,
    #[serde(default)]
    shows_loot: bool,
    #[serde(default)]
    flat: bool,
    prompt: Option<String>,
    locked_says: Option<String>,
}

fn answers(s: &str) -> Option<Answers> {
    Some(match s {
        "repair" => Answers::Repair,
        "grow" => Answers::Grow,
        "physical" => Answers::Physical,
        "frost" => Answers::Frost,
        "fire" => Answers::Fire,
        "nature" => Answers::Nature,
        "blast" => Answers::Blast,
        "shock" => Answers::Shock,
        _ => return None,
    })
}

/// `"#rrggbb"` to `0xRRGGBB`.
fn color(s: &str) -> Result<u32, String> {
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    hex.and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or_else(|| format!("colour \"{s}\" is not #rrggbb"))
}

fn light(cx: &mut Ctx, at: &str, l: &RawLight) -> Option<Light> {
    let at = format!("{at}.light");
    cx.diag.need(l.radius.is_positive(), &at, "radius must be > 0");
    let radius = l.radius.fx_px().map_err(|e| cx.diag.error(&at, e)).ok()?;
    let color = color(&l.color).map_err(|e| cx.diag.error(&at, e)).ok()?;
    let flicker = l.flicker.permille().map_err(|e| cx.diag.error(&at, e)).ok()?;
    cx.diag.need((0..=1000).contains(&flicker.0), &at, "flicker is a fraction of full brightness, 0..1");
    Some(Light { radius, color, flicker, cold: l.cold, sky: l.sky })
}

/// A prop's `feet` (x, y, w, h in sixteenths of a cell) must lie in the rows it stamps and stand
/// on its front edge. What is pushed, carried, a gate or answers a verb is part of a way or a
/// puzzle the solver proves by its cells: its feet hold the footprint's width, leaving at most
/// [`FEET_SIDE_SLACK`] open at either side, so none side by side lets her slip between, and its
/// back is a notch she steps into from the north only (`PropDef::solid_parts`).
fn feet_fit(cx: &mut Ctx, at: &str, f: [u8; 4], r: &RawProp, base: u8) {
    let [x, y, w, h] = f.map(i32::from);
    let (fw, fh) = (i32::from(r.w) * 16, i32::from(r.h) * 16);
    let top = (i32::from(r.h) - i32::from(base)) * 16;
    cx.diag.need(r.solid && !r.flat, at, "only a solid prop that stands up has feet");
    cx.diag.need(w >= 1 && h >= 1 && x + w <= fw, at, "feet are at least a sixteenth each way, in the footprint");
    cx.diag.need(y >= top && y + h == fh, at, "feet stand on the front edge, within the rows it blocks");
    let wide = x <= FEET_SIDE_SLACK && x + w >= fw - FEET_SIDE_SLACK;
    cx.diag.need(
        wide || !(r.push || r.carry || r.gate || r.answers.is_some()),
        at,
        "a way's or a puzzle's feet leave at most 4 sixteenths of a cell open at either side",
    );
}

fn props(src: &Source, cx: &mut Ctx) -> &'static [PropDef] {
    let rows = src.table("props", &mut cx.diag);
    let feet_rows = src.table("prop_feet", &mut cx.diag);
    for id in feet_rows.keys().filter(|id| !rows.contains_key(*id)) {
        cx.diag.error(format!("prop_feet.{id}"), "no prop row has this id");
    }
    let mut out: Vec<Option<PropDef>> = vec![None; cx.ids.props.len()];
    for (id, row) in &rows {
        let at = format!("props.{id}");
        let Some(r) = typed::<RawProp>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need(r.w >= 1 && r.h >= 1, &at, "footprint < 1");
        let base = r.base.unwrap_or(r.h);
        cx.diag.need((1..=r.h).contains(&base), &at, "base is 1 to h rows of the footprint");
        cx.diag.need(
            base == r.h || (r.solid && !r.push && !r.carry && !r.gate),
            &at,
            "only a solid thing that stays put blocks less than its footprint",
        );
        let ans = match r.answers.as_deref() {
            None => None,
            Some(s) => {
                let a = answers(s);
                if a.is_none() {
                    cx.diag.error(&at, format!("answers \"{s}\" is not a world verb or a damage school"));
                }
                a
            }
        };
        let feet = feet_rows.get(id).and_then(|row| {
            let at = format!("prop_feet.{id}");
            let f = typed::<[u8; 4]>(row, &at, &mut cx.diag)?;
            feet_fit(cx, &at, f, &r, base);
            Some(f)
        });
        let flagged = r.light_when_on || r.night_only || r.day_only;
        cx.diag.need(!flagged || r.light.is_some(), &at, "light flag without a light");
        cx.diag.need(!(r.night_only && r.day_only), &at, "a light cannot be both nightOnly and dayOnly");
        cx.diag.need(r.douse.is_none() || r.light.is_some(), &at, "douse without a light");
        let lit = match &r.light {
            Some(l) => light(cx, &at, l),
            None => None,
        };
        let def = PropDef {
            id: leak_str(id),
            name: text(cx, &at, &r.name),
            sprite: cx.sprite(&r.sprite),
            w: r.w,
            h: r.h,
            base,
            feet,
            solid: r.solid,
            block_los: r.block_los,
            push: r.push,
            carry: r.carry,
            bench: r.bench,
            store: r.store,
            answers: ans,
            once: r.once,
            gate: r.gate,
            plate: r.plate,
            light_when_on: r.light_when_on,
            night_only: r.night_only,
            day_only: r.day_only,
            rest: r.rest,
            hide_when_used: r.hide_when_used,
            regrow: r.regrow,
            light: lit,
            douse: r.douse,
            shows_loot: r.shows_loot,
            flat: r.flat,
            prompt: r.prompt.as_deref().map(|p| text(cx, &at, p)),
            locked_says: r.locked_says.as_deref().map(|p| text(cx, &at, p)),
        };
        let i = cx.ids.props.get(id).map_or(0, usize::from);
        out[i] = Some(def);
    }
    by_id(out)
}

// ---------------------------------------------------------------------------------------------
// Quests

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawReqKind {
    Kill,
    Acquire,
    Location,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReq {
    #[serde(rename = "type")]
    kind: RawReqKind,
    target: String,
    qty: u16,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawQuest {
    name: String,
    description: String,
    completion: String,
    return_to: String,
    requirements: Vec<RawReq>,
    rewards: Vec<RawAction>,
}

fn quests(src: &Source, cx: &mut Ctx) -> &'static [QuestDef] {
    let rows = src.table("quests", &mut cx.diag);
    let mut out: Vec<Option<QuestDef>> = vec![None; cx.ids.quests.len()];
    for (id, row) in &rows {
        let at = format!("quests.{id}");
        let Some(r) = typed::<RawQuest>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need(!r.requirements.is_empty(), &at, "no requirements");
        cx.diag.need(!r.return_to.trim().is_empty(), &at, "returnTo is empty: nobody says who takes it back");
        let mut reqs = Vec::with_capacity(r.requirements.len());
        let mut ok = true;
        for (n, q) in r.requirements.iter().enumerate() {
            let rat = format!("{at}.requirements[{n}]");
            cx.diag.need(q.qty >= 1, &rat, "qty < 1");
            let target = match q.kind {
                RawReqKind::Kill => {
                    cx.row(&rat, "kill target", &q.target, |i| &i.units, jane_core::UnitDefId).map(ReqTarget::Kill)
                }
                RawReqKind::Acquire => cx.item(&rat, &q.target).map(ReqTarget::Acquire),
                RawReqKind::Location => {
                    cx.diag.need(q.qty == 1, &rat, "a location is visited once: qty must be 1");
                    Some(ReqTarget::Location(name(cx, &rat, "location", &q.target)))
                }
            };
            let text = text(cx, &rat, &q.text);
            match target {
                Some(target) => reqs.push(QuestReq { target, qty: q.qty, text }),
                None => ok = false,
            }
        }
        let def = QuestDef {
            id: leak_str(id),
            name: text(cx, &at, &r.name),
            description: text(cx, &at, &r.description),
            completion: text(cx, &at, &r.completion),
            return_to: text(cx, &format!("{at}.returnTo"), &r.return_to),
            requirements: leak(reqs),
            rewards: list(cx, &format!("{at}.rewards"), &r.rewards),
        };
        if ok {
            let i = cx.ids.quests.get(id).map_or(0, usize::from);
            out[i] = Some(def);
        }
    }
    by_id(out)
}

/// Every `{"do": "quest"}` and `{"do": "handin"}` in any JSON file, however deep.
fn quest_verbs(v: &Value, given: &mut Vec<String>, handed: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if let (Some(verb), Some(Value::String(q))) = (o.get("do").and_then(Value::as_str), o.get("quest")) {
                match verb {
                    "quest" => given.push(q.clone()),
                    "handin" => handed.push(q.clone()),
                    _ => {}
                }
            }
            for x in o.values() {
                quest_verbs(x, given, handed);
            }
        }
        Value::Array(a) => {
            for x in a {
                quest_verbs(x, given, handed);
            }
        }
        _ => {}
    }
}

/// ARCHITECTURE.md §6 "every quest givable and handable by some row". A warning: the lists that
/// give and take quests live in every group's tables (dialogue, triggers, placements, items),
/// and whether one is reachable on a seed is the solver's to say.
fn quests_given_and_handed_in(src: &Source, cx: &mut Ctx, start: &StartDef) {
    let (mut given, mut handed) = (Vec::new(), Vec::new());
    for (_, v) in src.json_files() {
        quest_verbs(v, &mut given, &mut handed);
    }
    let keys: Vec<String> = cx.ids.quests.keys().map(str::to_owned).collect();
    for (i, q) in keys.iter().enumerate() {
        let at = format!("quests.{q}");
        let at_start = start.quests.contains(&QuestId(i as u16));
        if !at_start && !given.iter().any(|g| g == q) {
            cx.diag.warn(&at, "no list gives it (no \"quest\" verb names it and the start does not)");
        }
        if !handed.iter().any(|h| h == q) {
            cx.diag.warn(&at, "no list hands it in (no \"handin\" verb names it)");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Dialogue

/// A fact a line tells: exactly one of these.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFact {
    place: Option<String>,
    person: Option<String>,
    item: Option<String>,
    prop: Option<String>,
    claim: Option<String>,
    route: Option<[String; 2]>,
    danger: Option<String>,
    rumour: Option<String>,
}

fn fact(cx: &mut Ctx, at: &str, f: &RawFact) -> Option<FactKey> {
    let given = [
        f.place.is_some(),
        f.person.is_some(),
        f.item.is_some(),
        f.prop.is_some(),
        f.claim.is_some(),
        f.route.is_some(),
        f.danger.is_some(),
        f.rumour.is_some(),
    ];
    if given.iter().filter(|g| **g).count() != 1 {
        cx.diag.error(at, "a fact names exactly one of place, person, item, prop, claim, route, danger, rumour");
        return None;
    }
    Some(if let Some(p) = &f.place {
        FactKey::Place(cx.key(p))
    } else if let Some(p) = &f.person {
        FactKey::Person(cx.key(p))
    } else if let Some(i) = &f.item {
        FactKey::Thing(Thing::Item(cx.item(at, i)?))
    } else if let Some(p) = &f.prop {
        FactKey::Thing(Thing::Prop(cx.prop(at, p)?))
    } else if let Some(c) = &f.claim {
        FactKey::Claim(text(cx, at, c))
    } else if let Some([a, b]) = &f.route {
        FactKey::Route(cx.key(a), cx.key(b))
    } else if let Some(d) = &f.danger {
        FactKey::Danger(cx.key(d))
    } else {
        FactKey::Rumour(cx.story(at, f.rumour.as_deref().unwrap_or_default())?)
    })
}

/// A line: plain English, or English and the facts saying it tells (ARCHITECTURE.md §3.7).
#[derive(Deserialize)]
#[serde(untagged)]
enum RawLine {
    Plain(String),
    Tells(RawTold),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTold {
    text: String,
    tells: Vec<RawFact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOption {
    label: String,
    goto: Option<String>,
    actions: Option<Vec<RawAction>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    lines: Vec<RawLine>,
    #[serde(default)]
    options: Vec<RawOption>,
    actions: Option<Vec<RawAction>>,
    goto: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStart {
    when: Option<Vec<RawCond>>,
    node: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTree {
    speaker: String,
    start: Vec<RawStart>,
    /// Sorted by name: a node's index is its place in this order.
    nodes: BTreeMap<String, RawNode>,
}

fn tree(cx: &mut Ctx, id: &str, r: &RawTree) -> Option<DialogueTree> {
    let at = format!("dialogue.{id}");
    let index: BTreeMap<&str, u16> = r.nodes.keys().enumerate().map(|(i, k)| (k.as_str(), i as u16)).collect();
    cx.diag.need(u16::try_from(index.len()).is_ok(), &at, "more nodes than a u16 can index");
    let mut ok = true;
    let mut goto = |cx: &mut Ctx, at: &str, what: &str, to: &str| -> Option<u16> {
        let n = index.get(to).copied();
        if n.is_none() {
            cx.diag.error(at, format!("{what} unknown node \"{to}\""));
            ok = false;
        }
        n
    };

    cx.diag.need(!r.start.is_empty(), &at, "no start");
    cx.diag.need(r.start.last().is_none_or(|s| s.when.is_none()), &at, "last start entry must be unconditional");
    let mut start = Vec::with_capacity(r.start.len());
    let mut reached = vec![false; r.nodes.len()];
    for (n, s) in r.start.iter().enumerate() {
        let sat = format!("{at}.start[{n}]");
        let node = goto(cx, &sat, "start ->", &s.node);
        let when = lists::conds(cx, &sat, s.when.as_deref());
        if let Some(n) = node {
            reached[usize::from(n)] = true;
        }
        start.push(DialogueStart { when, node: node.unwrap_or(0) });
    }

    let mut nodes = Vec::with_capacity(r.nodes.len());
    for (nid, node) in &r.nodes {
        let nat = format!("{at}.{nid}");
        cx.diag.need(!node.lines.is_empty(), &nat, "no lines");
        cx.diag.need(node.options.len() <= 2, &nat, "at most 2 options");
        let mut lines = Vec::with_capacity(node.lines.len());
        for (n, l) in node.lines.iter().enumerate() {
            let lat = format!("{nat}.lines[{n}]");
            let (t, tells) = match l {
                RawLine::Plain(t) => (t.as_str(), &[][..]),
                RawLine::Tells(RawTold { text, tells }) => {
                    cx.diag.need(!tells.is_empty(), &lat, "tells is empty: write the line as a string");
                    (text.as_str(), tells.as_slice())
                }
            };
            let text = text(cx, &lat, t);
            let tells: Vec<FactKey> =
                tells.iter().enumerate().filter_map(|(k, f)| fact(cx, &format!("{lat}.tells[{k}]"), f)).collect();
            lines.push(DialogueLine { text, tells: leak(tells) });
        }
        let node_goto = node.goto.as_deref().and_then(|g| goto(cx, &nat, "goto", g));
        let mut options = Vec::with_capacity(node.options.len());
        for (n, o) in node.options.iter().enumerate() {
            let oat = format!("{nat}.options[{n}]");
            let og = o.goto.as_deref().and_then(|g| goto(cx, &oat, "option ->", g));
            options.push(DialogueOption {
                label: text(cx, &oat, &o.label),
                goto: og,
                actions: lists::list(cx, &format!("{oat}.actions"), o.actions.as_deref()),
            });
        }
        for g in node_goto.iter().chain(options.iter().filter_map(|o| o.goto.as_ref())) {
            reached[usize::from(*g)] = true;
        }
        nodes.push(DialogueNode {
            id: leak_str(nid),
            lines: leak(lines),
            options: leak(options),
            actions: lists::list(cx, &format!("{nat}.actions"), node.actions.as_deref()),
            goto: node_goto,
        });
    }
    if ok {
        for (nid, r) in r.nodes.keys().zip(&reached) {
            if !r {
                cx.diag.warn(format!("{at}.{nid}"), "no start row or goto leads here");
            }
        }
    }
    let speaker = text(cx, &format!("{at}.speaker"), &r.speaker);
    ok.then(|| DialogueTree { id: leak_str(id), speaker, start: leak(start), nodes: leak(nodes) })
}

fn dialogue(src: &Source, cx: &mut Ctx) -> &'static [DialogueTree] {
    let rows = src.table("dialogue", &mut cx.diag);
    let mut out: Vec<Option<DialogueTree>> = vec![None; cx.ids.dialogue.len()];
    for (id, row) in &rows {
        let Some(r) = typed::<RawTree>(row, &format!("dialogue.{id}"), &mut cx.diag) else { continue };
        let i = cx.ids.dialogue.get(id).map_or(0, usize::from);
        out[i] = tree(cx, id, &r);
    }
    by_id(out)
}

// ---------------------------------------------------------------------------------------------
// Triggers

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawMode {
    Enter,
    While,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrigger {
    zone: String,
    rect: String,
    mode: Option<RawMode>,
    once: bool,
    when: Option<Vec<RawCond>>,
    actions: Vec<RawAction>,
    reset: Option<Vec<RawAction>>,
}

fn triggers(src: &Source, cx: &mut Ctx) -> &'static [TriggerDef] {
    let rows = src.table("triggers", &mut cx.diag);
    let mut out: Vec<Option<TriggerDef>> = vec![None; cx.ids.triggers.len()];
    for (id, row) in &rows {
        let at = format!("triggers.{id}");
        let Some(r) = typed::<RawTrigger>(row, &at, &mut cx.diag) else { continue };
        let mode = match r.mode {
            None | Some(RawMode::Enter) => TriggerMode::Enter,
            Some(RawMode::While) => TriggerMode::While,
        };
        if mode == TriggerMode::While && r.when.as_ref().is_none_or(Vec::is_empty) {
            cx.diag
                .warn(&at, "a \"while\" trigger with no \"when\" fires whenever anyone is inside: is it an \"enter\"?");
        }
        let zone = cx.zone(&at, &r.zone);
        let rect = cx.name(&r.rect);
        cx.diag.need(!r.rect.is_empty(), &at, "rect is empty");
        let trigger = Trigger {
            rect: jane_core::Key::Name(rect),
            mode,
            once: r.once,
            when: lists::conds(cx, &at, r.when.as_deref()),
            actions: list(cx, &at, &r.actions),
            reset: lists::list(cx, &format!("{at}.reset"), r.reset.as_deref()),
        };
        let Some(zone) = zone else { continue };
        let i = cx.ids.triggers.get(id).map_or(0, usize::from);
        out[i] = Some(TriggerDef { id: leak_str(id), zone, trigger });
    }
    by_id(out)
}

// ---------------------------------------------------------------------------------------------
// The clock

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClock {
    hour: u8,
    #[serde(default)]
    minute: u8,
    actions: Vec<RawAction>,
}

fn verb_name(a: &Action) -> &'static str {
    match a {
        Action::Give(_) => "give",
        Action::Take(_) => "take",
        Action::Rest { .. } => "rest",
        Action::Travel { .. } => "travel",
        Action::Talk(_) => "talk",
        Action::Read(_) => "read",
        _ => "?",
    }
}

/// Every player-scoped verb in a compiled list, through `if` branches and `send`'s `then`.
fn actor_verbs(cx: &Ctx, l: ListRef, out: &mut Vec<&'static str>) {
    let ListRef::Catalog(i) = l else { return };
    let Some(list) = cx.lists.get(usize::from(i)) else { return };
    for a in list {
        if a.needs_actor() {
            out.push(verb_name(a));
        }
        match *a {
            Action::If { then, els, .. } => {
                actor_verbs(cx, then, out);
                if let Some(e) = els {
                    actor_verbs(cx, e, out);
                }
            }
            Action::Send { then: Some(t), .. } => actor_verbs(cx, t, out),
            _ => {}
        }
    }
}

fn clock(src: &Source, cx: &mut Ctx) -> &'static [ClockDef] {
    let rows = src.list("clock", &mut cx.diag);
    let mut out = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let at = format!("clock[{n}]");
        let Some(r) = typed::<RawClock>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need(r.hour < 24, &at, "hour out of range");
        cx.diag.need(r.minute < 60 && r.minute % 10 == 0, &at, "minute is a ten-minute mark, 0 to 50");
        let actions = list(cx, &at, &r.actions);
        let mut found = Vec::new();
        actor_verbs(cx, actions, &mut found);
        for v in found {
            cx.diag
                .error(&at, format!("\"{v}\" needs a player, and a clock row runs with none (ARCHITECTURE.md §5.2)"));
        }
        out.push(ClockDef { hour: r.hour, minute: r.minute, actions });
    }
    leak(out)
}

// ---------------------------------------------------------------------------------------------
// The omens

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawOmenRegion {
    Lowfields,
    Waters,
    Works,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOmen {
    id: String,
    /// Out of 1000.
    chance: u16,
    region: RawOmenRegion,
    #[serde(default)]
    lethal: bool,
    /// The flag it sets: `omen:<id>`.
    sets: String,
    claim: String,
}

/// `data/omens.json`, in row order: an id once, a chance of 1 to 1000, a flag `omen:<id>` set by
/// no other row. That something reads the flag (an omen nobody acts on is a claim the county
/// never makes good) needs every list: `integrate.rs`.
fn omens(src: &Source, cx: &mut Ctx) -> &'static [model::OmenDef] {
    let rows = src.list("omens", &mut cx.diag);
    let mut out: Vec<model::OmenDef> = Vec::with_capacity(rows.len());
    for (n, row) in rows.iter().enumerate() {
        let at = format!("{}: omens[{n}]", row.file);
        let Some(r) = typed::<RawOmen>(row, &at, &mut cx.diag) else { continue };
        cx.diag.need(!r.id.is_empty(), &at, "id is empty");
        cx.diag.need(out.iter().all(|o| o.id != r.id), &at, format!("\"{}\" is defined twice", r.id));
        cx.diag.need((1..=1000).contains(&r.chance), &at, format!("chance {} is not 1 to 1000", r.chance));
        cx.diag.need(r.sets == format!("omen:{}", r.id), &at, format!("an omen sets omen:{}, not {}", r.id, r.sets));
        let region = match r.region {
            RawOmenRegion::Lowfields => model::Region::Lowfields,
            RawOmenRegion::Waters => model::Region::Waters,
            RawOmenRegion::Works => model::Region::Works,
        };
        let flag = cx.name(&r.sets);
        let claim = cx.text(&r.claim);
        out.push(model::OmenDef { id: leak_str(&r.id), chance: r.chance, region, lethal: r.lethal, flag, claim });
    }
    leak(out)
}

// ---------------------------------------------------------------------------------------------
// The start

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStack {
    item: String,
    qty: u16,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawSource {
    Spell,
    Item,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBar {
    source: RawSource,
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStartDef {
    items: Vec<RawStack>,
    bar: Vec<Option<RawBar>>,
    quests: Vec<String>,
    mark: String,
}

fn empty_start() -> StartDef {
    StartDef { items: &[], bar: [None; BAR_SLOTS], quests: &[], mark: NameId(0) }
}

fn start(src: &Source, cx: &mut Ctx) -> StartDef {
    let at = "start";
    let Some(v) = src.file("start.json") else {
        cx.diag.error("start.json", "missing: New Game needs a start kit and a mark");
        return empty_start();
    };
    let row = Row { file: "start.json".to_owned(), value: v.clone() };
    let Some(r) = typed::<RawStartDef>(&row, at, &mut cx.diag) else { return empty_start() };
    let mut items = Vec::with_capacity(r.items.len());
    for (n, s) in r.items.iter().enumerate() {
        let sat = format!("{at}.items[{n}]");
        cx.diag.need(s.qty >= 1, &sat, "qty < 1");
        if let Some(item) = cx.item(&sat, &s.item) {
            items.push(Stack { item, qty: s.qty });
        }
    }
    cx.diag.need(r.bar.len() == BAR_SLOTS, at, format!("bar has {} slots, not {BAR_SLOTS}", r.bar.len()));
    let mut bar = [None; BAR_SLOTS];
    for (n, b) in r.bar.iter().enumerate().take(BAR_SLOTS) {
        let Some(b) = b else { continue };
        let bat = format!("{at}.bar[{n}]");
        bar[n] = match b.source {
            RawSource::Spell => cx.spell(&bat, &b.id).map(BarSlot::Spell),
            RawSource::Item => {
                let item = cx.item(&bat, &b.id);
                if let Some(i) = item {
                    if !items.iter().any(|s| s.item == i) {
                        cx.diag.warn(&bat, format!("\"{}\" is on the bar but not in the start kit", b.id));
                    }
                }
                item.map(BarSlot::Item)
            }
        };
    }
    let quests: Vec<QuestId> = r.quests.iter().filter_map(|q| cx.quest(at, q)).collect();
    let mark = name(cx, at, "mark", &r.mark);
    StartDef { items: leak(items), bar, quests: leak(quests), mark }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ctx::Ids;
    use crate::compile::diag::Diagnostics;

    /// Compile a story group from in-memory files; everything the group names is declared.
    fn build(extra: &[(&str, &str)]) -> (model::Story, Ctx) {
        let mut files: Vec<(&str, &str)> = vec![
            ("items.json", r#"{"apple": {}, "letter": {}}"#),
            ("spells.json", r#"{"melee": {}}"#),
            ("units.json", r#"{"skeleton": {}}"#),
            ("stories/tales.json", r#"[{"id": "hollins"}]"#),
            (
                "start.json",
                r#"{"items": [{"item": "apple", "qty": 3}], "bar": [{"source": "spell", "id": "melee"}, null, null, null, null, null, {"source": "item", "id": "apple"}, null], "quests": ["letter"], "mark": "start"}"#,
            ),
        ];
        let has = |f: &str, files: &[(&str, &str)]| extra.iter().chain(files.iter()).any(|(n, _)| *n == f);
        if !has("quests.json", &files) {
            files.push((
                "quests.json",
                r#"{"letter": {"name": "A Letter", "description": "d", "completion": "c", "returnTo": "the step",
                    "requirements": [{"type": "location", "target": "stoop", "qty": 1, "text": "The stoop"}],
                    "rewards": [{"do": "handin", "quest": "letter"}]}}"#,
            ));
        }
        files.retain(|(n, _)| !extra.iter().any(|(e, _)| e == n));
        files.extend_from_slice(extra);
        let src = Source::from_files(&files).unwrap();
        let mut cx = Ctx { ids: Ids::collect(&src, &mut Diagnostics::default()), ..Ctx::default() };
        let story = compile(&src, &mut cx);
        (story, cx)
    }

    fn errors(cx: &Ctx) -> Vec<String> {
        cx.diag.errors.iter().map(ToString::to_string).collect()
    }

    fn has_error(cx: &Ctx, needle: &str) -> bool {
        cx.diag.errors.iter().any(|d| d.to_string().contains(needle))
    }

    /// `omens.json`: row order kept, a chance out of 1000, the flag it sets `omen:<id>`, an id once.
    #[test]
    fn omens_keep_their_order_and_set_their_own_flag() {
        let ok = r#"[
          {"id": "b_later", "chance": 333, "region": "works", "sets": "omen:b_later", "claim": "It rings twice."},
          {"id": "a_first", "chance": 1000, "region": "lowfields", "lethal": true, "sets": "omen:a_first", "claim": "No exit."}
        ]"#;
        let (s, cx) = build(&[("omens.json", ok)]);
        assert!(errors(&cx).is_empty(), "{:?}", errors(&cx));
        let ids: Vec<&str> = s.omens.iter().map(|o| o.id).collect();
        assert_eq!(ids, ["b_later", "a_first"], "rolled in row order, so kept in it");
        assert!(s.omens[1].lethal && s.omens[1].chance == 1000 && s.omens[1].region == model::Region::Lowfields);
        for (bad, why) in [
            (r#"[{"id": "x", "chance": 0, "region": "works", "sets": "omen:x", "claim": "c"}]"#, "not 1 to 1000"),
            (r#"[{"id": "x", "chance": 1001, "region": "works", "sets": "omen:x", "claim": "c"}]"#, "not 1 to 1000"),
            (r#"[{"id": "x", "chance": 5, "region": "works", "sets": "x", "claim": "c"}]"#, "sets omen:x"),
            (
                r#"[{"id": "x", "chance": 5, "region": "works", "sets": "omen:x", "claim": "c"}, {"id": "x", "chance": 5, "region": "works", "sets": "omen:x", "claim": "c"}]"#,
                "defined twice",
            ),
        ] {
            let (_, cx) = build(&[("omens.json", bad)]);
            assert!(has_error(&cx, why), "{bad}: {:?}", errors(&cx));
        }
    }

    #[test]
    fn a_small_story_compiles() {
        let (s, cx) = build(&[
            (
                "props.json",
                r##"{"lamp": {"name": "Lamp Post", "sprite": "lamp", "w": 1, "h": 2, "solid": true, "blockLos": false,
                     "nightOnly": true, "light": {"radius": 72, "color": "#ffc070", "flicker": 0.05}, "answers": "frost"}}"##,
            ),
            (
                "dialogue.json",
                r#"{"dog": {"speaker": "The Dog", "start": [{"when": [{"if": "night"}], "node": "b"}, {"node": "a"}],
                    "nodes": {"b": {"lines": ["Woof."], "goto": "a"},
                              "a": {"lines": ["Hello.", {"text": "The mill is shut.", "tells": [{"claim": "The mill is shut."}]}],
                                    "options": [{"label": "Bye", "actions": [{"do": "quest", "quest": "letter"}]}, {"label": "Again", "goto": "b"}]}}}}"#,
            ),
            (
                "triggers.json",
                r#"{"arrive": {"zone": "county", "rect": "stoop", "once": true, "actions": [{"do": "location", "name": "stoop"}]}}"#,
            ),
            ("clock.json", r#"[{"hour": 21, "actions": [{"do": "toast", "text": "Nine."}]}]"#),
        ]);
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        assert!(cx.diag.warnings.is_empty(), "{}", cx.diag);
        let lamp = &s.props[0];
        assert_eq!((lamp.w, lamp.h), (1, 2));
        let l = lamp.light.unwrap();
        assert_eq!((l.radius, l.color, l.flicker), (jane_core::Fx(72 * 256), 0x00ff_c070, jane_core::Permille(50)));
        assert_eq!(lamp.answers, Some(Answers::Frost));
        assert!(lamp.light_shows(false, true) && !lamp.light_shows(false, false));

        let dog = &s.dialogue[0];
        // Nodes by sorted name: "a" is 0, "b" is 1.
        assert_eq!(dog.node_index("a"), Some(0));
        assert_eq!(dog.start[0].node, 1);
        assert!(dog.start[0].when.is_some() && dog.start[1].when.is_none());
        assert_eq!(dog.node(1).goto, Some(0));
        assert_eq!(dog.node(0).options[1].goto, Some(1));
        assert!(matches!(dog.node(0).lines[1].tells, [FactKey::Claim(_)]));

        assert_eq!(s.triggers[0].zone, jane_core::ZoneId::County);
        assert_eq!(s.triggers[0].trigger.mode, TriggerMode::Enter);
        assert_eq!(s.clock_at(21, 0).count(), 1);
        assert_eq!(s.start.bar[0], Some(BarSlot::Spell(jane_core::SpellId(0))));
        assert_eq!(s.start.bar[6], Some(BarSlot::Item(jane_core::ItemId(0))));
        assert_eq!(s.start.quests, &[QuestId(0)]);
        assert!(matches!(s.quests[0].requirements[0].target, ReqTarget::Location(_)));
    }

    #[test]
    fn dialogue_rules() {
        let (_, cx) = build(&[(
            "dialogue.json",
            r#"{"t": {"speaker": "", "start": [{"node": "a"}, {"when": [{"if": "night"}], "node": "zz"}],
                 "nodes": {"a": {"lines": [], "goto": "nope",
                                 "options": [{"label": "1"}, {"label": "2"}, {"label": "3", "goto": "gone"}]}}}}"#,
        )]);
        let e = errors(&cx);
        assert!(has_error(&cx, "last start entry must be unconditional"), "{e:?}");
        assert!(has_error(&cx, "start -> unknown node \"zz\""), "{e:?}");
        assert!(has_error(&cx, "no lines"), "{e:?}");
        assert!(has_error(&cx, "at most 2 options"), "{e:?}");
        assert!(has_error(&cx, "goto unknown node \"nope\""), "{e:?}");
        assert!(has_error(&cx, "option -> unknown node \"gone\""), "{e:?}");
        let (_, cx) = build(&[("dialogue.json", r#"{"t": {"speaker": "", "start": [], "nodes": {}}}"#)]);
        assert!(has_error(&cx, "no start"));
    }

    #[test]
    fn an_orphan_node_is_a_warning() {
        let (_, cx) = build(&[(
            "dialogue.json",
            r#"{"t": {"speaker": "", "start": [{"node": "a"}], "nodes": {"a": {"lines": ["x"]}, "lost": {"lines": ["y"]}}}}"#,
        )]);
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        assert!(cx.diag.warnings.iter().any(|w| w.at == "dialogue.t.lost"), "{}", cx.diag);
    }

    #[test]
    fn prop_rules() {
        let (_, cx) = build(&[(
            "props.json",
            r#"{"a": {"name": "A", "sprite": "a", "w": 0, "h": 1, "solid": true, "blockLos": true, "answers": "heal"},
                "b": {"name": "B", "sprite": "b", "w": 1, "h": 1, "solid": true, "blockLos": true, "nightOnly": true, "dayOnly": true},
                "c": {"name": "C", "sprite": "c", "w": 1, "h": 1, "solid": true, "blockLos": true, "light": {"radius": 8, "color": "red", "flicker": 0}},
                "d": {"name": "D", "sprite": "d", "w": 1, "h": 1, "solid": true, "blockLos": true, "glows": true}}"#,
        )]);
        let e = errors(&cx);
        assert!(has_error(&cx, "footprint < 1"), "{e:?}");
        assert!(has_error(&cx, "answers \"heal\" is not a world verb or a damage school"), "{e:?}");
        assert!(has_error(&cx, "light flag without a light"), "{e:?}");
        assert!(has_error(&cx, "both nightOnly and dayOnly"), "{e:?}");
        assert!(has_error(&cx, "colour \"red\" is not #rrggbb"), "{e:?}");
        assert!(has_error(&cx, "unknown field `glows`"), "{e:?}");
    }

    #[test]
    fn quest_rules() {
        let (_, cx) = build(&[(
            "quests.json",
            r#"{"letter": {"name": "n", "description": "At {place:nowhere}.", "completion": "c", "returnTo": " ",
                 "requirements": [{"type": "kill", "target": "dragon", "qty": 0, "text": "t"},
                                  {"type": "location", "target": "stoop", "qty": 2, "text": "t"}],
                 "rewards": []},
                "empty": {"name": "n", "description": "At {place:hollins}.", "completion": "c", "returnTo": "the dog",
                 "requirements": [], "rewards": [{"do": "handin", "quest": "empty"}]}}"#,
        )]);
        let e = errors(&cx);
        assert!(has_error(&cx, "unknown kill target \"dragon\""), "{e:?}");
        assert!(has_error(&cx, "qty < 1"), "{e:?}");
        assert!(has_error(&cx, "qty must be 1"), "{e:?}");
        assert!(has_error(&cx, "returnTo is empty"), "{e:?}");
        assert!(has_error(&cx, "{place:nowhere} names no story"), "{e:?}");
        assert!(has_error(&cx, "quests.empty: no requirements"), "{e:?}");
        assert_eq!(cx.diag.errors.iter().filter(|d| d.msg.contains("names no story")).count(), 1, "{e:?}");
        // "letter" is given by the start but nothing hands it in; "empty" hands itself in but nothing gives it.
        let w: Vec<String> = cx.diag.warnings.iter().map(ToString::to_string).collect();
        assert!(w.iter().any(|w| w.contains("quests.letter") && w.contains("hands it in")), "{w:?}");
        assert!(w.iter().any(|w| w.contains("quests.empty") && w.contains("gives it")), "{w:?}");
        assert!(!w.iter().any(|w| w.contains("quests.letter") && w.contains("gives it")), "{w:?}");
    }

    #[test]
    fn clock_rows_have_no_player() {
        let (_, cx) = build(&[(
            "clock.json",
            r#"[{"hour": 24, "actions": []},
                {"hour": 6, "actions": [{"do": "if", "when": [{"if": "night"}], "then": [{"do": "give", "item": "apple"}]},
                                        {"do": "send", "unit": "u", "to": "m", "then": [{"do": "travel", "zone": "mine", "mark": "m"}]},
                                        {"do": "show", "prop": "p"}]}]"#,
        )]);
        let e = errors(&cx);
        assert!(has_error(&cx, "clock[0]: hour out of range"), "{e:?}");
        assert!(has_error(&cx, "clock[1]: \"give\" needs a player"), "{e:?}");
        assert!(has_error(&cx, "clock[1]: \"travel\" needs a player"), "{e:?}");
        assert_eq!(cx.diag.errors.len(), 3, "{e:?}");
    }

    #[test]
    fn trigger_rules() {
        let (s, cx) = build(&[(
            "triggers.json",
            r#"{"a": {"zone": "icehouse", "rect": "r", "once": true, "actions": []},
                "b": {"zone": "burial", "rect": "all", "mode": "while", "once": true, "actions": [], "reset": [{"do": "unlock", "prop": "g"}]}}"#,
        )]);
        assert!(has_error(&cx, "unknown zone \"icehouse\""), "{}", cx.diag);
        assert!(cx.diag.warnings.iter().any(|w| w.at == "triggers.b" && w.msg.contains("while")), "{}", cx.diag);
        assert!(s.triggers.is_empty(), "a table with a failed row is left empty");
    }

    #[test]
    fn start_rules() {
        let (_, cx) = build(&[(
            "start.json",
            r#"{"items": [{"item": "pear", "qty": 0}], "bar": [{"source": "item", "id": "letter"}, {"source": "spell", "id": "fly"}],
                "quests": ["nope"], "mark": ""}"#,
        )]);
        let e = errors(&cx);
        assert!(has_error(&cx, "unknown item \"pear\""), "{e:?}");
        assert!(has_error(&cx, "qty < 1"), "{e:?}");
        assert!(has_error(&cx, "bar has 2 slots, not 8"), "{e:?}");
        assert!(has_error(&cx, "unknown spell \"fly\""), "{e:?}");
        assert!(has_error(&cx, "unknown quest \"nope\""), "{e:?}");
        assert!(has_error(&cx, "mark is empty"), "{e:?}");
        assert!(cx.diag.warnings.iter().any(|w| w.at == "start.bar[0]"), "{}", cx.diag);
    }

    #[test]
    fn colours() {
        assert_eq!(color("#000000"), Ok(0));
        assert_eq!(color("#FFa048"), Ok(0x00ff_a048));
        assert!(color("#fff").is_err());
        assert!(color("ff a048").is_err());
    }
}
