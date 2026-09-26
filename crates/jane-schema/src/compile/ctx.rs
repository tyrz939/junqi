//! The compile's shared state: row ids for every table (sorted-string order, fixed before any
//! row is compiled, so a reference to any table resolves whatever order tables compile in),
//! the name, text and sprite interners, and the list pools.

use indexmap::IndexMap;
use jane_core::action::{Action, Cond, CondsRef, ListRef, NamesRef, TextRef};
use jane_core::ids::*;

use super::diag::Diagnostics;
use super::source::Source;

/// A table's row ids: key to index, in sorted-string order.
#[derive(Clone, Debug, Default)]
pub struct RowIds {
    by_key: IndexMap<String, u16>,
}

impl RowIds {
    pub fn from_keys<'a>(keys: impl IntoIterator<Item = &'a str>) -> Self {
        let mut v: Vec<&str> = keys.into_iter().collect();
        v.sort();
        v.dedup();
        Self { by_key: v.into_iter().enumerate().map(|(i, k)| (k.to_owned(), i as u16)).collect() }
    }

    pub fn get(&self, key: &str) -> Option<u16> {
        self.by_key.get(key).copied()
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// Keys in id order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.by_key.keys().map(String::as_str)
    }
}

/// Row ids for every table a reference can name.
#[derive(Clone, Debug, Default)]
pub struct Ids {
    pub spells: RowIds,
    pub effects: RowIds,
    pub items: RowIds,
    pub units: RowIds,
    pub props: RowIds,
    pub quests: RowIds,
    pub dialogue: RowIds,
    pub triggers: RowIds,
    pub stories: RowIds,
    pub dungeons: RowIds,
    /// `data/consequences.json`'s ids, sorted: the order the living world's table is built in.
    pub consequences: RowIds,
}

impl Ids {
    /// Every keyed table's keys, before anything is typed.
    pub fn collect(src: &Source, diag: &mut Diagnostics) -> Ids {
        let keys = |t: &str, d: &mut Diagnostics| RowIds::from_keys(src.table(t, d).keys().map(String::as_str));
        // Duplicates are reported once, when the owning group reads its table.
        let mut quiet = Diagnostics::default();
        let ids = Ids {
            spells: keys("spells", &mut quiet),
            effects: keys("effects", &mut quiet),
            items: keys("items", &mut quiet),
            units: keys("units", &mut quiet),
            props: keys("props", &mut quiet),
            quests: keys("quests", &mut quiet),
            dialogue: keys("dialogue", &mut quiet),
            triggers: keys("triggers", &mut quiet),
            stories: {
                let rows = src.list("stories", &mut quiet);
                let ids: Vec<String> =
                    rows.iter().filter_map(|r| r.value.get("id").and_then(|v| v.as_str()).map(str::to_owned)).collect();
                RowIds::from_keys(ids.iter().map(String::as_str))
            },
            dungeons: {
                let stems: Vec<String> = src
                    .files_in("dungeons")
                    .map(|(f, _)| f.trim_start_matches("dungeons/").trim_end_matches(".json").to_owned())
                    .collect();
                RowIds::from_keys(stems.iter().map(String::as_str))
            },
            consequences: {
                let rows = src.list("consequences", &mut quiet);
                let ids: Vec<String> =
                    rows.iter().filter_map(|r| r.value.get("id").and_then(|v| v.as_str()).map(str::to_owned)).collect();
                RowIds::from_keys(ids.iter().map(String::as_str))
            },
        };
        let _ = diag;
        ids
    }
}

/// First-seen interning: the same string, the same index.
#[derive(Clone, Debug, Default)]
pub struct Interner {
    map: IndexMap<String, u32>,
}

impl Interner {
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.map.get(s) {
            return i;
        }
        let i = self.map.len() as u32;
        self.map.insert(s.to_owned(), i);
        i
    }

    pub fn get(&self, s: &str) -> Option<u32> {
        self.map.get(s).copied()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn strings(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }
}

/// What every table compile writes into.
#[derive(Debug, Default)]
pub struct Ctx {
    pub ids: Ids,
    pub names: Interner,
    pub texts: Interner,
    pub sprites: Interner,
    pub lists: Vec<Vec<Action>>,
    pub conds: Vec<Vec<Cond>>,
    pub name_lists: Vec<Vec<Key>>,
    pub diag: Diagnostics,
}

impl Ctx {
    pub fn name(&mut self, s: &str) -> NameId {
        NameId(self.names.intern(s) as u16)
    }

    pub fn key(&mut self, s: &str) -> Key {
        Key::Name(self.name(s))
    }

    pub fn text(&mut self, s: &str) -> TextId {
        TextId(self.texts.intern(s) as u16)
    }

    pub fn text_ref(&mut self, s: &str) -> TextRef {
        TextRef::Text(self.text(s))
    }

    pub fn sprite(&mut self, s: &str) -> SpriteId {
        SpriteId(self.sprites.intern(s) as u16)
    }

    pub fn push_list(&mut self, list: Vec<Action>) -> ListRef {
        self.lists.push(list);
        ListRef::Catalog(self.lists.len() as u16 - 1)
    }

    pub fn push_conds(&mut self, conds: Vec<Cond>) -> CondsRef {
        self.conds.push(conds);
        CondsRef::Catalog(self.conds.len() as u16 - 1)
    }

    pub fn push_names(&mut self, names: Vec<Key>) -> NamesRef {
        self.name_lists.push(names);
        NamesRef::Catalog(self.name_lists.len() as u16 - 1)
    }

    /// Resolve a row reference, or record an error and return `None`.
    pub fn row<T>(
        &mut self,
        at: &str,
        what: &str,
        key: &str,
        f: impl Fn(&Ids) -> &RowIds,
        wrap: fn(u16) -> T,
    ) -> Option<T> {
        match f(&self.ids).get(key) {
            Some(i) => Some(wrap(i)),
            None => {
                self.diag.error(at, format!("unknown {what} \"{key}\""));
                None
            }
        }
    }

    pub fn spell(&mut self, at: &str, key: &str) -> Option<SpellId> {
        self.row(at, "spell", key, |i| &i.spells, SpellId)
    }

    pub fn effect(&mut self, at: &str, key: &str) -> Option<EffectId> {
        self.row(at, "effect", key, |i| &i.effects, EffectId)
    }

    pub fn item(&mut self, at: &str, key: &str) -> Option<ItemId> {
        self.row(at, "item", key, |i| &i.items, ItemId)
    }

    pub fn unit(&mut self, at: &str, key: &str) -> Option<UnitDefId> {
        self.row(at, "unit def", key, |i| &i.units, UnitDefId)
    }

    pub fn prop(&mut self, at: &str, key: &str) -> Option<PropDefId> {
        self.row(at, "prop def", key, |i| &i.props, PropDefId)
    }

    pub fn quest(&mut self, at: &str, key: &str) -> Option<QuestId> {
        self.row(at, "quest", key, |i| &i.quests, QuestId)
    }

    pub fn dialogue(&mut self, at: &str, key: &str) -> Option<DialogueId> {
        self.row(at, "dialogue", key, |i| &i.dialogue, DialogueId)
    }

    pub fn story(&mut self, at: &str, key: &str) -> Option<StoryId> {
        self.row(at, "story", key, |i| &i.stories, StoryId)
    }

    /// A consequence row by id: its index in the living world's table, which is sorted the same.
    pub fn consequence(&mut self, at: &str, key: &str) -> Option<ConsequenceId> {
        self.row(at, "consequence", key, |i| &i.consequences, ConsequenceId)
    }

    pub fn zone(&mut self, at: &str, key: &str) -> Option<ZoneId> {
        let z = ZoneId::from_name(key);
        if z.is_none() {
            self.diag.error(at, format!("unknown zone \"{key}\""));
        }
        z
    }
}

/// `Vec` to a `'static` slice. The compile leaks what it builds: once in a build script, once
/// at startup with `dev-data`.
pub fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

pub fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
