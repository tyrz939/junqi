//! Runtime names (ARCHITECTURE.md §3.1). The table is pre-seeded with the catalog's names, so
//! `Sym(n) == NameId(n)` below `names.len()`; generator-only names (a blueprint's
//! `Key::Local(i)`) append. A save stores only the tail, as strings.
//!
//! Interning is not the hot path: it happens when a zone's state is first made and when a save
//! loads. The lookups are `BTreeMap`s, which can be asked with a `&str`.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use jane_core::{Key, NameId, Sym};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug)]
pub struct SymTable {
    base: &'static [&'static str],
    /// Authoritative: every name interned beyond the catalog's, in the order it was interned.
    tail: Vec<String>,
    /// Derived: every name to its sym.
    base_index: BTreeMap<&'static str, Sym>,
    tail_index: BTreeMap<String, Sym>,
}

impl PartialEq for SymTable {
    fn eq(&self, o: &Self) -> bool {
        self.tail == o.tail && self.base.len() == o.base.len()
    }
}

impl Eq for SymTable {}

impl SymTable {
    /// The catalog's names, and nothing more.
    pub fn new(base: &'static [&'static str]) -> Self {
        Self::with_tail(base, Vec::new())
    }

    fn with_tail(base: &'static [&'static str], tail: Vec<String>) -> Self {
        let mut base_index = BTreeMap::new();
        for (i, n) in base.iter().enumerate() {
            // A name is in the catalog once; the first row wins should that ever change.
            base_index.entry(*n).or_insert(Sym(i as u32));
        }
        let mut tail_index = BTreeMap::new();
        for (i, n) in tail.iter().enumerate() {
            tail_index.insert(n.clone(), Sym((base.len() + i) as u32));
        }
        Self { base, tail, base_index, tail_index }
    }

    /// The table a save's tail makes, over this build's catalog names.
    pub fn from_tail(tail: Vec<String>) -> Self {
        Self::with_tail(jane_data::catalog().names, tail)
    }

    /// The sym of a name, interning it if it is new.
    pub fn intern(&mut self, name: &str) -> Sym {
        if let Some(s) = self.find(name) {
            return s;
        }
        let s = Sym((self.base.len() + self.tail.len()) as u32);
        self.tail.push(name.to_owned());
        self.tail_index.insert(name.to_owned(), s);
        s
    }

    /// The sym of a name already interned.
    pub fn find(&self, name: &str) -> Option<Sym> {
        self.base_index.get(name).or_else(|| self.tail_index.get(name)).copied()
    }

    pub fn name(&self, s: Sym) -> &str {
        let i = s.0 as usize;
        if i < self.base.len() { self.base[i] } else { self.tail.get(i - self.base.len()).map_or("?", String::as_str) }
    }

    /// Whether `s` is what `name` interns to. A tail name is never a catalog name, so past the
    /// catalog this is one string compare (the save asks it of every spawn it hashes).
    pub fn is(&self, s: Sym, name: &str) -> bool {
        if (s.0 as usize) < self.base.len() { self.find(name) == Some(s) } else { self.name(s) == name }
    }

    /// Names beyond the catalog's.
    pub fn tail(&self) -> &[String] {
        &self.tail
    }

    pub fn len(&self) -> u32 {
        (self.base.len() + self.tail.len()) as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SymTable {
    fn default() -> Self {
        Self::new(jane_data::catalog().names)
    }
}

/// A content name is its own sym.
pub const fn of_name(n: NameId) -> Sym {
    Sym(n.0 as u32)
}

/// A blueprint key to a sym, given that blueprint's locals as interned (`ZoneRuntime::locals`).
pub fn of_key(k: Key, locals: &[Sym]) -> Sym {
    match k {
        Key::Name(n) => of_name(n),
        Key::Local(i) => locals[i as usize],
    }
}

impl Serialize for SymTable {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.tail.serialize(s)
    }
}

impl<'de> Deserialize<'de> for SymTable {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(SymTable::from_tail(Vec::<String>::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_names_are_their_own_syms_and_locals_append() {
        let cat = jane_data::catalog();
        let mut t = SymTable::default();
        let start = cat.name_id("start").unwrap();
        assert_eq!(t.find("start"), Some(of_name(start)));
        assert_eq!(t.intern("start"), of_name(start));
        let a = t.intern("gate_the_first_of_its_kind");
        assert_eq!(a.0 as usize, cat.names.len());
        assert_eq!(t.intern("gate_the_first_of_its_kind"), a);
        assert_eq!(t.name(a), "gate_the_first_of_its_kind");
        assert_eq!(t.tail().len(), 1);
        let back = SymTable::from_tail(t.tail().to_vec());
        assert_eq!(back.find("gate_the_first_of_its_kind"), Some(a));
        assert_eq!(back, t);
    }
}
