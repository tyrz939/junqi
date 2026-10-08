//! Rare fields out of line (PORT.md §13.3, phase 3): a record whose few seldom-set fields would
//! cost every row their bytes keeps them in one box, made only when one is set. The record reads
//! and writes them as its own through `Deref` to the rare part; a record that never sets one
//! holds a null pointer.
//!
//! Equal, hashed and shown as the value it reads as: a record given a box and set back to the
//! defaults is the record it was, boxed or not. [`Rare::settle`] lets such a box go.

use alloc::boxed::Box;

/// The rare part of a record: what a record without a box reads ([`Thin::none`]).
pub trait Thin: Clone + PartialEq + 'static {
    /// Nothing set; a `static` the type owns.
    fn none() -> &'static Self;
}

/// A record's rare fields: `None` until one is set.
pub struct Rare<T: Thin>(Option<Box<T>>);

impl<T: Thin> Rare<T> {
    /// Nothing set, no box.
    pub const fn empty() -> Self {
        Rare(None)
    }

    /// The fields, read.
    #[inline]
    pub fn get(&self) -> &T {
        self.0.as_deref().unwrap_or_else(|| T::none())
    }

    /// The fields to write: the box is made here if there is none.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_or_insert_with(|| Box::new(T::none().clone()))
    }

    /// Whether a box is held.
    pub fn is_boxed(&self) -> bool {
        self.0.is_some()
    }

    /// The box let go if it holds only the defaults.
    pub fn settle(&mut self) {
        if self.0.as_deref().is_some_and(|r| r == T::none()) {
            self.0 = None;
        }
    }

    /// The box, if any, to tidy in place.
    pub fn boxed_mut(&mut self) -> Option<&mut T> {
        self.0.as_deref_mut()
    }
}

impl<T: Thin> Clone for Rare<T> {
    fn clone(&self) -> Self {
        Rare(self.0.clone())
    }
}

impl<T: Thin> Default for Rare<T> {
    fn default() -> Self {
        Rare(None)
    }
}

impl<T: Thin> PartialEq for Rare<T> {
    fn eq(&self, o: &Self) -> bool {
        self.get() == o.get()
    }
}

impl<T: Thin + Eq> Eq for Rare<T> {}

impl<T: Thin + core::hash::Hash> core::hash::Hash for Rare<T> {
    fn hash<H: core::hash::Hasher>(&self, h: &mut H) {
        self.get().hash(h);
    }
}

impl<T: Thin + core::fmt::Debug> core::fmt::Debug for Rare<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

/// As an `Option` of the fields: none without a box, so it reads back as it was held.
#[cfg(feature = "serde")]
impl<T: Thin + serde::Serialize> serde::Serialize for Rare<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.as_deref().serialize(s)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: Thin + serde::Deserialize<'de>> serde::Deserialize<'de> for Rare<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Rare(Option::<T>::deserialize(d)?.map(Box::new)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Eq, Debug)]
    struct Few {
        a: u32,
        b: Option<u8>,
    }

    impl Thin for Few {
        fn none() -> &'static Self {
            static N: Few = Few { a: 0, b: None };
            &N
        }
    }

    #[test]
    fn a_rare_reads_the_default_until_set_and_compares_as_it_reads() {
        let mut r: Rare<Few> = Rare::empty();
        assert_eq!(r.get().a, 0);
        assert!(!r.is_boxed());
        r.get_mut().a = 3;
        assert!(r.is_boxed());
        assert_ne!(r, Rare::empty());
        r.get_mut().a = 0;
        assert_eq!(r, Rare::empty(), "a box of defaults is no box");
        r.settle();
        assert!(!r.is_boxed());
    }
}
