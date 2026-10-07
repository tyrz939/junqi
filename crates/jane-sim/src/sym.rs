//! Runtime names (ARCHITECTURE.md §3.1). The table is pre-seeded with the catalog's names, so
//! `Sym(n) == NameId(n)` below `names.len()`; generator-only names (a blueprint's
//! `Key::Local(i)`) append. A save stores only the tail, as strings.
//!
//! Interning is not the hot path: it happens when a zone's state is first made and when a save
//! loads. The catalog's names are looked up in a `BTreeMap`; the tail (the county's tens of
//! thousands of place names) is held end to end in one string, found through a small hashed index
//! of its own (PORT.md §13.3: a `String` and a map entry a name were a megabyte).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use jane_core::{Key, NameId, Sym};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug)]
pub struct SymTable {
    base: &'static [&'static str],
    /// Authoritative: every name interned beyond the catalog's, in the order it was interned,
    /// end to end; `ends[i]` is where name `i` ends.
    text: String,
    ends: Vec<u32>,
    /// Derived: every catalog name to its sym.
    base_index: BTreeMap<&'static str, Sym>,
    /// Derived: the tail by hash, open addressing (linear probing): a slot holds a tail index
    /// plus one, 0 empty; never fuller than half. Only ever asked "is this name here".
    slots: Vec<u32>,
}

impl PartialEq for SymTable {
    fn eq(&self, o: &Self) -> bool {
        self.text == o.text && self.ends == o.ends && self.base.len() == o.base.len()
    }
}

impl Eq for SymTable {}

/// FNV-1a over a name: where it starts looking in the slots.
fn hash(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

impl SymTable {
    /// The catalog's names, and nothing more.
    pub fn new(base: &'static [&'static str]) -> Self {
        let mut base_index = BTreeMap::new();
        for (i, n) in base.iter().enumerate() {
            // A name is in the catalog once; the first row wins should that ever change.
            base_index.entry(*n).or_insert(Sym(i as u32));
        }
        Self { base, text: String::new(), ends: Vec::new(), base_index, slots: Vec::new() }
    }

    /// The table a save's tail makes, over this build's catalog names.
    pub fn from_tail(tail: &[String]) -> Self {
        let mut t = Self::new(jane_data::catalog().names);
        for n in tail {
            t.push(n);
        }
        t
    }

    /// Tail name `i`.
    fn tail_name(&self, i: usize) -> &str {
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        &self.text[start..self.ends[i] as usize]
    }

    /// The tail index of `name`, if it is in the tail.
    fn tail_find(&self, name: &str) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut at = hash(name) as usize & mask;
        loop {
            match self.slots[at] {
                0 => return None,
                s if self.tail_name(s as usize - 1) == name => return Some(s as usize - 1),
                _ => at = (at + 1) & mask,
            }
        }
    }

    /// Append a name not yet in the table.
    fn push(&mut self, name: &str) {
        self.text.push_str(name);
        self.ends.push(self.text.len() as u32);
        let n = self.ends.len();
        if n * 2 > self.slots.len() {
            self.slots = alloc::vec![0; (n * 2).next_power_of_two().max(64)];
            for i in 0..n {
                self.slot(i);
            }
        } else {
            self.slot(n - 1);
        }
    }

    /// Put tail name `i` in its slot.
    fn slot(&mut self, i: usize) {
        let mask = self.slots.len() - 1;
        let mut at = hash(self.tail_name(i)) as usize & mask;
        while self.slots[at] != 0 {
            at = (at + 1) & mask;
        }
        self.slots[at] = i as u32 + 1;
    }

    /// The sym of a name, interning it if it is new.
    pub fn intern(&mut self, name: &str) -> Sym {
        if let Some(s) = self.find(name) {
            return s;
        }
        let s = Sym((self.base.len() + self.ends.len()) as u32);
        self.push(name);
        s
    }

    /// The sym of a name already interned.
    pub fn find(&self, name: &str) -> Option<Sym> {
        self.base_index
            .get(name)
            .copied()
            .or_else(|| self.tail_find(name).map(|i| Sym((self.base.len() + i) as u32)))
    }

    pub fn name(&self, s: Sym) -> &str {
        let i = s.0 as usize;
        if i < self.base.len() {
            self.base[i]
        } else if i - self.base.len() < self.ends.len() {
            self.tail_name(i - self.base.len())
        } else {
            "?"
        }
    }

    /// Whether `s` is what `name` interns to. A tail name is never a catalog name, so past the
    /// catalog this is one string compare (the save asks it of every spawn it hashes).
    pub fn is(&self, s: Sym, name: &str) -> bool {
        if (s.0 as usize) < self.base.len() { self.find(name) == Some(s) } else { self.name(s) == name }
    }

    /// Names beyond the catalog's.
    pub fn tail(&self) -> Tail<'_> {
        Tail(self)
    }

    pub fn len(&self) -> u32 {
        (self.base.len() + self.ends.len()) as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The names beyond the catalog's, in the order they were interned ([`SymTable::tail`]).
#[derive(Clone, Copy, Debug)]
pub struct Tail<'a>(&'a SymTable);

impl<'a> Tail<'a> {
    pub fn len(self) -> usize {
        self.0.ends.len()
    }

    pub fn is_empty(self) -> bool {
        self.0.ends.is_empty()
    }

    pub fn get(self, i: usize) -> Option<&'a str> {
        (i < self.len()).then(|| self.0.tail_name(i))
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a str> {
        (0..self.len()).map(move |i| self.0.tail_name(i))
    }

    pub fn to_vec(self) -> Vec<String> {
        self.iter().map(String::from).collect()
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
    /// The tail as a sequence of strings: the bytes a `Vec<String>` of it wrote.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.tail().iter())
    }
}

impl<'de> Deserialize<'de> for SymTable {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(SymTable::from_tail(&Vec::<String>::deserialize(d)?))
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
        let back = SymTable::from_tail(&t.tail().to_vec());
        assert_eq!(back.find("gate_the_first_of_its_kind"), Some(a));
        assert_eq!(back, t);
    }

    /// Thousands of names, through the index's growth: each is found as the sym it was given,
    /// the tail reads back in order, and its bytes are a `Vec<String>`'s.
    #[test]
    fn a_long_tail_finds_every_name_and_writes_as_strings() {
        let mut t = SymTable::default();
        let names: Vec<String> = (0..5000).map(|i| alloc::format!("county_rock_{}_{}", i * 7 % 2000, i)).collect();
        let syms: Vec<Sym> = names.iter().map(|n| t.intern(n)).collect();
        for (n, s) in names.iter().zip(&syms) {
            assert_eq!(t.find(n), Some(*s));
            assert_eq!(t.intern(n), *s);
            assert_eq!(t.name(*s), n);
        }
        assert_eq!(t.find("county_rock_none"), None);
        assert_eq!(t.tail().to_vec(), names);
        assert_eq!(postcard::to_allocvec(&t).unwrap(), postcard::to_allocvec(&names).unwrap());
    }
}
