//! Verbs and conditions as content writes them (`{"do": "give", "item": "potion"}`), and their
//! compile to `jane_core::Action` and `Cond`. Every table with a list goes through here, so the
//! checks `validateCatalog` made per verb are made once for every list in the game.
//!
//! A nested list (`if`'s branches, `send`'s `then`) becomes a list of its own in the pool,
//! written before the verb that names it.

use serde::Deserialize;

use jane_core::action::{
    Action, CameraMode, Cond, Condition, CondsRef, FlagKey, FlagOp, FlagTest, Heal, ListRef, School, Stack, Stat,
};
use jane_core::ids::Key;
use jane_core::tile::{Material, Tile};

use super::ctx::Ctx;
use super::fraction::Num;

/// A verb as written.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "do", rename_all = "lowercase", deny_unknown_fields)]
pub enum RawAction {
    Quest {
        quest: String,
    },
    Handin {
        quest: String,
    },
    Flag {
        flag: String,
        value: Option<i32>,
        add: Option<i32>,
    },
    Rest {
        until: Option<u8>,
    },
    Grow {
        stat: RawStat,
        amount: i16,
        id: String,
    },
    Give {
        item: String,
        qty: Option<u16>,
    },
    Take {
        item: String,
        qty: Option<u16>,
    },
    Learn {
        spell: String,
    },
    Toast {
        text: String,
    },
    Read {
        text: String,
    },
    Lock {
        prop: String,
    },
    Unlock {
        prop: String,
    },
    Show {
        prop: String,
    },
    Hide {
        prop: String,
    },
    Switch {
        prop: String,
        on: Option<bool>,
    },
    Spawn {
        unit: String,
        def: String,
        at: String,
    },
    Despawn {
        unit: String,
    },
    Aggro {
        unit: String,
    },
    Location {
        name: String,
    },
    Fill {
        rect: String,
        tile: u8,
    },
    Strike {
        rect: String,
        amount: Num,
        school: RawSchool,
        effect: Option<String>,
        #[serde(rename = "hitsFriends", default)]
        hits_friends: bool,
    },
    Status {
        effect: String,
    },
    Heal {
        amount: Num,
    },
    Travel {
        zone: String,
        mark: String,
    },
    Talk {
        tree: String,
    },
    Throw {
        item: String,
    },
    Shake {
        amount: u8,
    },
    Camera {
        mode: RawCameraMode,
        rect: Option<String>,
    },
    If {
        when: Vec<RawCond>,
        then: Vec<RawAction>,
        #[serde(rename = "else")]
        els: Option<Vec<RawAction>>,
    },
    Send {
        unit: String,
        to: String,
        then: Option<Vec<RawAction>>,
    },
    Reveal {
        rects: Vec<String>,
    },
    Place {
        def: String,
        item: String,
    },
    /// `{ "do": "switchAll", "def": "lamp_east", "on": false }`: every prop of a row in the zone.
    #[serde(rename = "switchAll")]
    SwitchAll {
        def: String,
        on: Option<bool>,
    },
    /// `{ "do": "nightLock", "prop", "says", "hours"?: [from, to], "keyed"?: bool }`.
    #[serde(rename = "nightLock")]
    NightLock {
        prop: String,
        says: String,
        hours: Option<[u8; 2]>,
        #[serde(default)]
        keyed: bool,
    },
    #[serde(rename = "nightUnlock")]
    NightUnlock {
        prop: String,
    },
}

/// A condition as written; `not` negates any of them.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "if", rename_all = "camelCase", deny_unknown_fields)]
pub enum RawCond {
    Flag {
        flag: String,
        eq: Option<i32>,
        min: Option<i32>,
        #[serde(default)]
        not: bool,
    },
    Night {
        #[serde(default)]
        not: bool,
    },
    QuestActive {
        quest: String,
        #[serde(default)]
        not: bool,
    },
    QuestReady {
        quest: String,
        #[serde(default)]
        not: bool,
    },
    QuestDone {
        quest: String,
        #[serde(default)]
        not: bool,
    },
    HasItem {
        item: String,
        qty: Option<u16>,
        #[serde(default)]
        not: bool,
    },
    /// Knows a spell (the TypeScript's `knows`). The journal's `Knows` is `knowsFact`.
    Knows {
        spell: String,
        #[serde(default)]
        not: bool,
    },
    Dead {
        unit: String,
        #[serde(default)]
        not: bool,
    },
    /// A journal fact (ARCHITECTURE.md §3.7): `{"if": "knowsFact", "place": "the_mill"}`.
    KnowsFact {
        place: Option<String>,
        person: Option<String>,
        item: Option<String>,
        claim: Option<String>,
        route: Option<[String; 2]>,
        danger: Option<String>,
        rumour: Option<String>,
        #[serde(default)]
        not: bool,
    },
    Heard {
        claim: String,
        #[serde(default)]
        not: bool,
    },
    SpeakerKnows {
        story: String,
        #[serde(default)]
        not: bool,
    },
    /// The clock's hour is `from` up to `to`, wrapping midnight: `{"if": "hours", "from": 16,
    /// "to": 10}` (a door's `nightHours` rule).
    Hours {
        from: u8,
        to: u8,
        #[serde(default)]
        not: bool,
    },
    /// `{"if": "weekday", "day": "tuesday"}`.
    Weekday {
        day: RawWeekday,
        #[serde(default)]
        not: bool,
    },
}

/// A day of the week as content writes it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawWeekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawSchool {
    Heal,
    Physical,
    Frost,
    Fire,
    Nature,
    Blast,
    Shock,
}

impl From<RawSchool> for School {
    fn from(s: RawSchool) -> School {
        match s {
            RawSchool::Heal => School::Heal,
            RawSchool::Physical => School::Physical,
            RawSchool::Frost => School::Frost,
            RawSchool::Fire => School::Fire,
            RawSchool::Nature => School::Nature,
            RawSchool::Blast => School::Blast,
            RawSchool::Shock => School::Shock,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawStat {
    Strength,
    Spirit,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawCameraMode {
    Follow,
    Lock,
}

/// A flag's key from its string: `been:x` and `dead:x` are their own kinds.
pub fn flag_key(cx: &mut Ctx, s: &str) -> FlagKey {
    if let Some(rest) = s.strip_prefix("been:") {
        FlagKey::Been(cx.key(rest))
    } else if let Some(rest) = s.strip_prefix("dead:") {
        FlagKey::Dead(cx.key(rest))
    } else {
        FlagKey::Named(cx.key(s))
    }
}

fn nonempty(cx: &mut Ctx, at: &str, what: &str, s: &str) -> Key {
    cx.diag.need(!s.is_empty(), at, format!("{what} is empty"));
    cx.key(s)
}

/// Compile a list into the pool. `None` for an absent list; an empty list is still a list.
pub fn list(cx: &mut Ctx, at: &str, raw: Option<&[RawAction]>) -> Option<ListRef> {
    let raw = raw?;
    let mut out = Vec::with_capacity(raw.len());
    for (i, a) in raw.iter().enumerate() {
        if let Some(a) = action(cx, &format!("{at}[{i}]"), a) {
            out.push(a);
        }
    }
    Some(cx.push_list(out))
}

/// Compile a condition list into the pool.
pub fn conds(cx: &mut Ctx, at: &str, raw: Option<&[RawCond]>) -> Option<CondsRef> {
    let raw = raw?;
    let out = cond_vec(cx, at, raw);
    Some(cx.push_conds(out))
}

/// Compile conditions without pooling them.
pub fn cond_vec(cx: &mut Ctx, at: &str, raw: &[RawCond]) -> Vec<Cond> {
    raw.iter().enumerate().filter_map(|(i, c)| cond(cx, &format!("{at}.when[{i}]"), c)).collect()
}

pub fn action(cx: &mut Ctx, at: &str, a: &RawAction) -> Option<Action> {
    Some(match a {
        RawAction::Quest { quest } => Action::Quest(cx.quest(at, quest)?),
        RawAction::Handin { quest } => Action::HandIn(cx.quest(at, quest)?),
        RawAction::Flag { flag, value, add } => {
            cx.diag.need(!(value.is_some() && add.is_some()), at, "flag: value or add, not both");
            let key = flag_key(cx, flag);
            let op = match (value, add) {
                (_, Some(n)) => FlagOp::Add(*n),
                (Some(v), None) => FlagOp::Set(*v),
                (None, None) => FlagOp::Set(1),
            };
            Action::Flag { key, op }
        }
        RawAction::Rest { until } => {
            cx.diag.need(until.is_none_or(|h| h < 24), at, "rest: until is an hour, 0..23");
            Action::Rest { until: *until }
        }
        RawAction::Grow { stat, amount, id } => {
            cx.diag.need(*amount > 0, at, "grow: amount must be > 0");
            let id = nonempty(cx, at, "grow id", id);
            let stat = match stat {
                RawStat::Strength => Stat::Strength,
                RawStat::Spirit => Stat::Spirit,
            };
            Action::Grow { stat, amount: *amount, id }
        }
        RawAction::Give { item, qty } => Action::Give(stack(cx, at, item, *qty)?),
        RawAction::Take { item, qty } => Action::Take(stack(cx, at, item, *qty)?),
        RawAction::Learn { spell } => Action::Learn(cx.spell(at, spell)?),
        RawAction::Toast { text } => Action::Toast(cx.text_ref(text)),
        RawAction::Read { text } => Action::Read(cx.text_ref(text)),
        RawAction::Lock { prop } => Action::Lock(nonempty(cx, at, "prop", prop)),
        RawAction::Unlock { prop } => Action::Unlock(nonempty(cx, at, "prop", prop)),
        RawAction::Show { prop } => Action::Show(nonempty(cx, at, "prop", prop)),
        RawAction::Hide { prop } => Action::Hide(nonempty(cx, at, "prop", prop)),
        RawAction::Switch { prop, on } => Action::Switch { prop: nonempty(cx, at, "prop", prop), on: *on },
        RawAction::Spawn { unit, def, at: mark } => {
            let def = cx.unit(at, def)?;
            Action::Spawn { key: nonempty(cx, at, "unit", unit), def, at: nonempty(cx, at, "mark", mark) }
        }
        RawAction::Despawn { unit } => Action::Despawn(nonempty(cx, at, "unit", unit)),
        RawAction::Aggro { unit } => Action::Aggro(nonempty(cx, at, "unit", unit)),
        RawAction::Location { name } => Action::Location(nonempty(cx, at, "location", name)),
        RawAction::Fill { rect, tile } => {
            let Some(t) = Tile::from_id(*tile) else {
                let hint = if Material::from_ts_tile(*tile).is_some() { " (a render-only tile)" } else { "" };
                cx.diag.error(at, format!("fill: {tile} is not a tile{hint}"));
                return None;
            };
            Action::Fill { rect: nonempty(cx, at, "rect", rect), tile: t }
        }
        RawAction::Strike { rect, amount, school, effect, hits_friends } => {
            cx.diag.need(
                *school != RawSchool::Heal && amount.is_positive(),
                at,
                "strike needs an amount and a damage school",
            );
            let amount = amount.milli().map_err(|e| cx.diag.error(at, e)).ok()?;
            let effect = match effect {
                Some(e) => Some(cx.effect(at, e)?),
                None => None,
            };
            Action::Strike {
                rect: nonempty(cx, at, "rect", rect),
                amount,
                school: (*school).into(),
                effect,
                hits_friends: *hits_friends,
            }
        }
        RawAction::Status { effect } => Action::Status(cx.effect(at, effect)?),
        RawAction::Heal { amount } => {
            cx.diag.need(amount.is_positive(), at, "heal: amount must be > 0");
            // Below or at 1 is a fraction of max hp: the 2020 apple healed 25 % while its tooltip said 25.
            let h =
                if amount.at_most_one() { amount.permille().map(Heal::Pct) } else { amount.milli().map(Heal::Flat) };
            Action::Heal(h.map_err(|e| cx.diag.error(at, e)).ok()?)
        }
        RawAction::Travel { zone, mark } => {
            let zone = cx.zone(at, zone)?;
            Action::Travel { zone, mark: nonempty(cx, at, "mark", mark) }
        }
        RawAction::Talk { tree } => Action::Talk(cx.dialogue(at, tree)?),
        RawAction::Throw { item } => Action::Throw(cx.item(at, item)?),
        RawAction::Place { def, item } => Action::Place { prop: cx.prop(at, def)?, item: cx.item(at, item)? },
        RawAction::SwitchAll { def, on } => Action::SwitchAll { def: cx.prop(at, def)?, on: *on },
        RawAction::NightLock { prop, says, hours, keyed } => {
            let [from, to] = hours.unwrap_or([jane_core::NightLock::BELL.0, jane_core::NightLock::BELL.1]);
            cx.diag.need(from < 24 && to < 24 && from != to, at, "nightLock: two different hours, 0 to 23");
            let says = cx.text_ref(says);
            Action::NightLock {
                prop: nonempty(cx, at, "prop", prop),
                lock: jane_core::NightLock { says, from, to, keyed: *keyed },
            }
        }
        RawAction::NightUnlock { prop } => Action::NightUnlock(nonempty(cx, at, "prop", prop)),
        RawAction::Shake { amount } => Action::Shake(*amount),
        RawAction::Camera { mode, rect } => {
            let mode = match mode {
                RawCameraMode::Follow => CameraMode::Follow,
                RawCameraMode::Lock => CameraMode::Lock,
            };
            cx.diag.need(mode == CameraMode::Follow || rect.is_some(), at, "camera: lock needs a rect");
            Action::Camera { mode, rect: rect.as_deref().map(|r| cx.key(r)) }
        }
        RawAction::If { when, then, els } => {
            cx.diag.need(!when.is_empty(), at, "if needs a \"when\"");
            let w = cond_vec(cx, at, when);
            let when = cx.push_conds(w);
            let then = list(cx, &format!("{at}.then"), Some(then))?;
            let els = list(cx, &format!("{at}.else"), els.as_deref());
            Action::If { when, then, els }
        }
        RawAction::Send { unit, to, then } => {
            let then = list(cx, &format!("{at}.then"), then.as_deref());
            Action::Send { unit: nonempty(cx, at, "unit", unit), to: nonempty(cx, at, "mark", to), then }
        }
        RawAction::Reveal { rects } => {
            cx.diag.need(!rects.is_empty(), at, "reveal needs a list of rects");
            let keys = rects.iter().map(|r| nonempty(cx, at, "rect", r)).collect();
            Action::Reveal(cx.push_names(keys))
        }
    })
}

fn stack(cx: &mut Ctx, at: &str, item: &str, qty: Option<u16>) -> Option<Stack> {
    let q = qty.unwrap_or(1);
    cx.diag.need(q >= 1, at, "qty < 1");
    Some(Stack { item: cx.item(at, item)?, qty: q })
}

pub fn cond(cx: &mut Ctx, at: &str, c: &RawCond) -> Option<Cond> {
    let (not, c) = match c {
        RawCond::Flag { flag, eq, min, not } => {
            cx.diag.need(!(eq.is_some() && min.is_some()), at, "flag: eq or min, not both");
            let key = flag_key(cx, flag);
            let test = match (eq, min) {
                (Some(v), _) => FlagTest::Eq(*v),
                (None, Some(m)) => FlagTest::Min(*m),
                (None, None) => FlagTest::NonZero,
            };
            (*not, Condition::Flag { key, test })
        }
        RawCond::Night { not } => (*not, Condition::Night),
        RawCond::QuestActive { quest, not } => (*not, Condition::QuestActive(cx.quest(at, quest)?)),
        RawCond::QuestReady { quest, not } => (*not, Condition::QuestReady(cx.quest(at, quest)?)),
        RawCond::QuestDone { quest, not } => (*not, Condition::QuestDone(cx.quest(at, quest)?)),
        RawCond::HasItem { item, qty, not } => (*not, Condition::HasItem(stack(cx, at, item, *qty)?)),
        RawCond::Knows { spell, not } => (*not, Condition::HasSpell(cx.spell(at, spell)?)),
        RawCond::Dead { unit, not } => (*not, Condition::Dead(nonempty(cx, at, "unit", unit))),
        RawCond::KnowsFact { place, person, item, claim, route, danger, rumour, not } => {
            use jane_core::action::{FactKey, Thing};
            let given = [
                place.is_some(),
                person.is_some(),
                item.is_some(),
                claim.is_some(),
                route.is_some(),
                danger.is_some(),
                rumour.is_some(),
            ];
            if given.iter().filter(|g| **g).count() != 1 {
                cx.diag.error(at, "knowsFact names exactly one fact");
                return None;
            }
            let fact = if let Some(p) = place {
                FactKey::Place(cx.key(p))
            } else if let Some(p) = person {
                FactKey::Person(cx.key(p))
            } else if let Some(i) = item {
                FactKey::Thing(Thing::Item(cx.item(at, i)?))
            } else if let Some(t) = claim {
                FactKey::Claim(cx.text(t))
            } else if let Some([a, b]) = route {
                FactKey::Route(cx.key(a), cx.key(b))
            } else if let Some(d) = danger {
                FactKey::Danger(cx.key(d))
            } else {
                FactKey::Rumour(cx.story(at, rumour.as_deref().unwrap_or_default())?)
            };
            (*not, Condition::Knows(fact))
        }
        RawCond::Heard { claim, not } => (*not, Condition::Heard(cx.text(claim))),
        RawCond::SpeakerKnows { story, not } => (*not, Condition::SpeakerKnows(cx.story(at, story)?)),
        RawCond::Hours { from, to, not } => {
            cx.diag.need(
                *from < 24 && *to < 24 && from != to,
                at,
                format!("hours [{from}, {to}]: two different hours, 0 to 23"),
            );
            (*not, Condition::Hours { from: *from, to: *to })
        }
        RawCond::Weekday { day, not } => (*not, Condition::Weekday(*day as u8)),
    };
    Some(Cond { not, c })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::ctx::RowIds;

    fn cx() -> Ctx {
        let mut cx = Ctx::default();
        cx.ids.items = RowIds::from_keys(["potion", "key"]);
        cx.ids.quests = RowIds::from_keys(["letter"]);
        cx
    }

    fn parse(json: &str) -> Vec<RawAction> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn verbs_compile_and_nest() {
        let mut cx = cx();
        let raw = parse(
            r#"[{"do":"give","item":"potion"},{"do":"flag","flag":"been:mill","add":2},
               {"do":"if","when":[{"if":"questDone","quest":"letter","not":true}],"then":[{"do":"show","prop":"gate"}]},
               {"do":"heal","amount":0.25},{"do":"heal","amount":5}]"#,
        );
        let l = list(&mut cx, "t", Some(&raw)).unwrap();
        assert!(cx.diag.is_ok(), "{}", cx.diag);
        let ListRef::Catalog(i) = l else { panic!() };
        let v = &cx.lists[usize::from(i)];
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], Action::Give(Stack { item: jane_core::ItemId(1), qty: 1 }));
        assert!(matches!(v[1], Action::Flag { key: FlagKey::Been(_), op: FlagOp::Add(2) }));
        let Action::If { then: ListRef::Catalog(t), when: CondsRef::Catalog(w), .. } = v[2] else { panic!() };
        assert!(t < i, "a nested list is pooled before its parent");
        assert!(cx.conds[usize::from(w)][0].not);
        assert_eq!(v[3], Action::Heal(Heal::Pct(jane_core::Permille(250))));
        assert_eq!(v[4], Action::Heal(Heal::Flat(jane_core::Milli(5000))));
    }

    #[test]
    fn bad_rows_are_errors() {
        assert!(serde_json::from_str::<Vec<RawAction>>(r#"[{"do":"give","item":"x","qtty":2}]"#).is_err());
        assert!(serde_json::from_str::<Vec<RawAction>>(r#"[{"do":"dance"}]"#).is_err());
        let mut cx = cx();
        let raw = parse(
            r#"[{"do":"give","item":"nope"},{"do":"fill","rect":"r","tile":45},{"do":"travel","zone":"icehouse","mark":"m"}]"#,
        );
        list(&mut cx, "t", Some(&raw));
        assert_eq!(cx.diag.errors.len(), 3, "{}", cx.diag);
        assert!(cx.diag.errors[1].msg.contains("render-only"));
    }
}
