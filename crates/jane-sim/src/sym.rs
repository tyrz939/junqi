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

use jane_core::{Key, NameId, Names, Sym};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug)]
pub struct SymTable {
    base: &'static [&'static str],
    /// Authoritative: every name interned beyond the catalog's, in the order it was interned, in
    /// runs. A zone's run is its blueprint's `local_names` itself, shared (PORT.md §13.3 phase 3:
    /// not a second copy of the county's names), less the few that were already interned (its
    /// `skips`); the table's own run holds names interned one at a time. `starts[r]` is the tail
    /// index run `r` begins at.
    runs: Vec<Run>,
    starts: Vec<u32>,
    /// Whether the last run is the table's own (one it appends to), not a zone's.
    own_last: bool,
    /// Names in the tail.
    len: u32,
    /// Derived: every catalog name to its sym.
    base_index: BTreeMap<&'static str, Sym>,
    /// Derived: the tail by hash, open addressing (linear probing): a slot holds a tail index
    /// plus one, 0 empty; never fuller than half. Only ever asked "is this name here".
    slots: Vec<u32>,
}

/// A run of the tail: `names` but for the positions in `skips` (ascending).
#[derive(Clone, Debug)]
struct Run {
    names: Names,
    skips: Vec<u32>,
}

impl Run {
    /// The run's `k`-th name in the tail.
    fn get(&self, k: usize) -> &str {
        let mut pos = k;
        for &s in &self.skips {
            if s as usize <= pos {
                pos += 1;
            } else {
                break;
            }
        }
        &self.names[pos]
    }
}

impl PartialEq for SymTable {
    fn eq(&self, o: &Self) -> bool {
        self.base.len() == o.base.len() && self.len == o.len && self.tail().iter().eq(o.tail().iter())
    }
}

impl Eq for SymTable {}

/// FNV-1a over a name: where it starts looking in the slots.
fn hash(name: &str) -> u32 {
    jane_core::names::name_hash(name)
}

impl SymTable {
    /// The catalog's names, and nothing more.
    pub fn new(base: &'static [&'static str]) -> Self {
        let mut base_index = BTreeMap::new();
        for (i, n) in base.iter().enumerate() {
            // A name is in the catalog once; the first row wins should that ever change.
            base_index.entry(*n).or_insert(Sym(i as u32));
        }
        Self { base, runs: Vec::new(), starts: Vec::new(), own_last: false, len: 0, base_index, slots: Vec::new() }
    }

    /// The table a save's tail makes, over this build's catalog names.
    pub fn from_tail(tail: &[String]) -> Self {
        let mut t = Self::new(jane_data::catalog().names);
        for n in tail {
            t.push(n);
        }
        t
    }

    /// Tail name `i` (`i` below the tail's length).
    fn tail_name(&self, i: usize) -> &str {
        let r = self.starts.partition_point(|&s| s as usize <= i) - 1;
        self.runs[r].get(i - self.starts[r] as usize)
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
        if !self.own_last {
            self.runs.push(Run { names: Names::new(), skips: Vec::new() });
            self.starts.push(self.len);
            self.own_last = true;
        }
        self.runs.last_mut().expect("a run").names.push(name);
        self.len += 1;
        self.grown();
    }

    /// The tail grew by one name: it slotted (every name, if the slots grew).
    fn grown(&mut self) {
        let n = self.len as usize;
        if n * 2 > self.slots.len() {
            self.slots = alloc::vec![0; (n * 2).next_power_of_two().max(64)];
            for i in 0..n {
                self.slot(i);
            }
        } else {
            self.slot(n - 1);
        }
    }

    /// Intern every name of `names` (a zone's `local_names`) in order, as [`intern`](Self::intern)
    /// one at a time would: their syms. The table keeps `names` itself as the run, sharing its
    /// bytes, and notes the positions of those already interned.
    pub fn intern_all(&mut self, names: &Names) -> Vec<Sym> {
        let mut out = Vec::with_capacity(names.len());
        let was_own = self.own_last;
        let r = self.runs.len();
        self.runs.push(Run { names: names.clone(), skips: Vec::new() });
        self.starts.push(self.len);
        self.own_last = false;
        for (p, n) in names.iter().enumerate() {
            if let Some(s) = self.find(n) {
                self.runs[r].skips.push(p as u32);
                out.push(s);
                continue;
            }
            out.push(Sym((self.base.len() + self.len as usize) as u32));
            self.len += 1;
            self.grown();
        }
        if self.runs[r].skips.len() == names.len() {
            // Nothing appended: no run.
            self.runs.pop();
            self.starts.pop();
            self.own_last = was_own;
        }
        out
    }
    /// Put tail name `i` in its slot (never one already slotted).
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
        let s = Sym((self.base.len() + self.len as usize) as u32);
        self.push(name);
        s
    }

    /// The sym of a name already interned.
    pub fn find(&self, name: &str) -> Option<Sym> {
        self.base_index.get(name).copied().or_else(|| self.tail_find(name).map(|i| Sym((self.base.len() + i) as u32)))
    }

    pub fn name(&self, s: Sym) -> &str {
        let i = s.0 as usize;
        if i < self.base.len() {
            self.base[i]
        } else if i - self.base.len() < self.len as usize {
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

    /// The tail's runs (a zone's names, shared with its blueprint, or the table's own), for a
    /// measure of what the table holds.
    pub fn runs(&self) -> impl Iterator<Item = &Names> {
        self.runs.iter().map(|r| &r.names)
    }

    /// Names beyond the catalog's.
    pub fn tail(&self) -> Tail<'_> {
        Tail(self)
    }

    pub fn len(&self) -> u32 {
        (self.base.len() + self.len as usize) as u32
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
        self.0.len as usize
    }

    pub fn is_empty(self) -> bool {
        self.0.len == 0
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

    /// A zone's names interned whole are the table one at a time would make: catalog names,
    /// names already in, and a name twice in the list are skipped; the rest share its bytes.
    #[test]
    fn interning_a_zones_names_whole_is_interning_them_one_by_one() {
        let mut names: Vec<String> = (0..300).map(|i| alloc::format!("county_rock_{i}")).collect();
        names.insert(7, "start".into());
        names.insert(50, "county_rock_3".into());
        names.insert(90, "already".into());
        let list: Names = names.iter().collect();
        let (mut a, mut b) = (SymTable::default(), SymTable::default());
        for t in [&mut a, &mut b] {
            t.intern("already");
            t.intern("before");
        }
        let whole = a.intern_all(&list);
        let one: Vec<Sym> = names.iter().map(|n| b.intern(n)).collect();
        assert_eq!(whole, one);
        assert_eq!(a, b);
        assert_eq!(a.intern("after"), b.intern("after"));
        for n in names.iter().chain(["after".to_owned()].iter()) {
            assert_eq!(a.find(n), b.find(n), "{n}");
        }
        assert_eq!(a.tail().to_vec(), b.tail().to_vec());
        assert!(a.runs().any(|r| r.same(&list)), "the zone's names are shared, not copied");
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
