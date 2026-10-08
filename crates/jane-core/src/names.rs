//! A list of names held end to end in one string (PORT.md §13.3, phase 3): a blueprint's
//! generator-made names (`county_rock_812_40`, tens of thousands in the county), where a `String`
//! each cost a 24-byte header and an allocation on top of its bytes.
//!
//! Reads as a list of `&str`: `names[i]`, [`Names::get`], [`Names::iter`]. [`NameIndex`] finds a
//! name's position by hash, for a builder that interns thousands.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Names in the order pushed. Shared on clone (copied on the first write after): the sim's name
/// table holds a zone's names as its blueprint does, without a second copy.
#[derive(Clone, Default)]
pub struct Names(Arc<Inner>);

/// Name `i` is `text[ends[i - 1]..ends[i]]`.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
struct Inner {
    text: String,
    ends: Vec<u32>,
}

impl PartialEq for Names {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0) || self.0 == o.0
    }
}

impl Eq for Names {}

impl core::hash::Hash for Names {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}

impl Names {
    pub fn new() -> Self {
        Names::default()
    }

    pub fn len(&self) -> usize {
        self.0.ends.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.ends.is_empty()
    }

    /// Name `i`.
    #[inline]
    pub fn get(&self, i: usize) -> Option<&str> {
        let n = &*self.0;
        let end = *n.ends.get(i)? as usize;
        let start = if i == 0 { 0 } else { n.ends[i - 1] as usize };
        Some(&n.text[start..end])
    }

    /// Append a name (a duplicate is the caller's to refuse).
    pub fn push(&mut self, name: &str) {
        let n = Arc::make_mut(&mut self.0);
        // Grown by a quarter, not doubled: a builder's list of tens of thousands keeps little spare.
        if n.text.capacity() - n.text.len() < name.len() {
            n.text.reserve_exact((n.text.len() / 4).max(1024).max(name.len()));
        }
        if n.ends.len() == n.ends.capacity() {
            n.ends.reserve_exact((n.ends.len() / 4).max(256));
        }
        n.text.push_str(name);
        n.ends.push(n.text.len() as u32);
    }

    /// Whether `o` is this very list (a clone of it, not written since).
    pub fn same(&self, o: &Names) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }

    /// Bytes held (once, however many share them).
    pub fn heap_bytes(&self) -> usize {
        self.0.text.capacity() + self.0.ends.capacity() * 4 + 2 * core::mem::size_of::<usize>()
    }

    /// The first position of `name`, by a scan: [`NameIndex`] for many.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.iter().position(|n| n == name)
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter { names: self, at: 0 }
    }

    /// Exactly the bytes held, no spare.
    pub fn shrink_to_fit(&mut self) {
        if self.0.text.capacity() == self.0.text.len() && self.0.ends.capacity() == self.0.ends.len() {
            return;
        }
        let n = Arc::make_mut(&mut self.0);
        n.text.shrink_to_fit();
        n.ends.shrink_to_fit();
    }

    /// The names' bytes, end to end.
    pub fn text_len(&self) -> usize {
        self.0.text.len()
    }
}

impl core::ops::Index<usize> for Names {
    type Output = str;
    fn index(&self, i: usize) -> &str {
        self.get(i).unwrap_or_else(|| panic!("name {i} of {}", self.len()))
    }
}

impl core::fmt::Debug for Names {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<S: AsRef<str>> FromIterator<S> for Names {
    fn from_iter<I: IntoIterator<Item = S>>(it: I) -> Self {
        let mut n = Names::new();
        for s in it {
            n.push(s.as_ref());
        }
        n
    }
}

/// As the one string and the ends: read back only if the ends climb, stay in the string and fall
/// on characters' edges, so every name reads.
#[cfg(feature = "serde")]
impl serde::Serialize for Names {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (&self.0.text, &self.0.ends).serialize(s)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Names {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (text, ends) = <(String, Vec<u32>)>::deserialize(d)?;
        let mut at = 0;
        for &e in &ends {
            if e < at || !text.is_char_boundary(e as usize) {
                return Err(serde::de::Error::custom("a name's end is out of place"));
            }
            at = e;
        }
        Ok(Names(Arc::new(Inner { text, ends })))
    }
}

/// The names in order.
#[derive(Clone, Debug)]
pub struct Iter<'a> {
    names: &'a Names,
    at: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let n = self.names.get(self.at)?;
        self.at += 1;
        Some(n)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.names.len() - self.at;
        (left, Some(left))
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl<'a> IntoIterator for &'a Names {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// FNV-1a over a name: where it starts looking in a [`NameIndex`].
pub fn name_hash(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

/// A [`Names`]' positions by hash: open addressing, linear probing; a slot holds a position plus
/// one, 0 empty, never fuller than half. Kept beside the names it indexes, told of each push.
#[derive(Clone, Debug, Default)]
pub struct NameIndex {
    slots: Vec<u32>,
}

impl NameIndex {
    /// The position of `name` in `names`.
    pub fn find(&self, names: &Names, name: &str) -> Option<usize> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut at = name_hash(name) as usize & mask;
        loop {
            match self.slots[at] {
                0 => return None,
                s if names.get(s as usize - 1) == Some(name) => return Some(s as usize - 1),
                _ => at = (at + 1) & mask,
            }
        }
    }

    /// `names` just had its last name pushed.
    pub fn pushed(&mut self, names: &Names) {
        let n = names.len();
        if n * 2 > self.slots.len() {
            self.slots = alloc::vec![0; (n * 2).next_power_of_two().max(64)];
            for i in 0..n {
                self.slot(names, i);
            }
        } else {
            self.slot(names, n - 1);
        }
    }

    fn slot(&mut self, names: &Names, i: usize) {
        let mask = self.slots.len() - 1;
        let mut at = name_hash(&names[i]) as usize & mask;
        while self.slots[at] != 0 {
            at = (at + 1) & mask;
        }
        self.slots[at] = i as u32 + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_back_in_order_and_the_index_finds_each() {
        let mut n = Names::new();
        let mut ix = NameIndex::default();
        let all: Vec<String> = (0..3000).map(|i| alloc::format!("county_rock_{}_{}", i * 7 % 2000, i)).collect();
        for s in &all {
            assert_eq!(ix.find(&n, s), None);
            n.push(s);
            ix.pushed(&n);
        }
        n.push("");
        ix.pushed(&n);
        assert_eq!(n.len(), 3001);
        for (i, s) in all.iter().enumerate() {
            assert_eq!(&n[i], s.as_str());
            assert_eq!(ix.find(&n, s), Some(i));
        }
        assert_eq!(n.get(3000), Some(""));
        assert_eq!(ix.find(&n, ""), Some(3000));
        assert_eq!(n.get(3001), None);
        assert!(n.iter().map(String::from).eq(all.iter().cloned().chain([String::new()])));
        assert_eq!(n.position("county_rock_7_1"), Some(1));
        let back: Names = all.iter().collect();
        assert_eq!(back.len(), 3000);
    }
}
