//! The presenter's tables as a pack (`JPT1`, PORT.md §13.4, §13.12): what `Present::new` learns
//! from the generators beside the atlas (each look's frames, glass rows, flora kinds and bends,
//! the sky's and the cues' sprites, the critters, the UI page's glyph metrics), written by
//! `jane bake` so a console builds its presenter from the pack and runs no generator but the
//! terrain painter (§13.7). [`Present::tables`](crate::Present::tables) writes it,
//! [`Present::from_tables`](crate::Present::from_tables) reads it.
//!
//! ```text
//! "JPT1"  u16 version (3: the decks' table after the night kit's)  u16 0  u32 atlas bytes
//! atlas   a JAT1 pack (`Atlas::to_pack`) with every page's px left out: the sizes, the CLUT,
//!         the mist and the sprite table with its bake keys
//! tables  stand-ins, people, creatures, props, flora, the sky, the cues, the ambient layer, the night kit, the decks and
//!         the UI's page table, each field in its struct's order: integers little-endian, a
//!         `Vec` as a u32 length then its items, an `Option` as a u8 then its value, an enum as
//!         the u8 of its place in its list
//! ```
//!
//! Integer only and `no_std`; every reader checks its lengths, and the pack must end where the
//! last table does.

use alloc::vec::Vec;

use crate::atlas::{PackError, Reader};

/// The pack's magic.
pub const MAGIC: &[u8; 4] = b"JPT1";
/// Its version.
pub const VERSION: u16 = 3;

/// A value the tables pack holds.
pub(crate) trait Tab: Sized {
    fn put(&self, o: &mut Vec<u8>);
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError>;
}

macro_rules! tab_int {
    ($($t:ty),*) => {$(
        impl Tab for $t {
            fn put(&self, o: &mut Vec<u8>) {
                o.extend_from_slice(&self.to_le_bytes());
            }
            fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
                let s = r.take(core::mem::size_of::<$t>())?;
                let mut b = [0u8; core::mem::size_of::<$t>()];
                b.copy_from_slice(s);
                Ok(<$t>::from_le_bytes(b))
            }
        }
    )*};
}
tab_int!(u8, i8, u16, i16, u32, i32);

impl Tab for bool {
    fn put(&self, o: &mut Vec<u8>) {
        o.push(u8::from(*self));
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        match r.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(PackError("a bool not 0 or 1")),
        }
    }
}

impl Tab for usize {
    fn put(&self, o: &mut Vec<u8>) {
        (*self as u32).put(o);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(r.u32()? as usize)
    }
}

impl<T: Tab> Tab for Vec<T> {
    fn put(&self, o: &mut Vec<u8>) {
        self.len().put(o);
        for v in self {
            v.put(o);
        }
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let n = r.u32()? as usize;
        // Never reserve more than the bytes left could hold (a byte an item at the least).
        let mut v = Vec::with_capacity(n.min(r.left()));
        for _ in 0..n {
            v.push(T::get(r)?);
        }
        Ok(v)
    }
}

impl<T: Tab> Tab for Option<T> {
    fn put(&self, o: &mut Vec<u8>) {
        match self {
            None => o.push(0),
            Some(v) => {
                o.push(1);
                v.put(o);
            }
        }
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        match r.u8()? {
            0 => Ok(None),
            1 => Ok(Some(T::get(r)?)),
            _ => Err(PackError("an option not 0 or 1")),
        }
    }
}

impl<T: Tab, const N: usize> Tab for [T; N] {
    fn put(&self, o: &mut Vec<u8>) {
        for v in self {
            v.put(o);
        }
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(T::get(r)?);
        }
        v.try_into().map_err(|_| PackError("array length"))
    }
}

impl<A: Tab, B: Tab> Tab for (A, B) {
    fn put(&self, o: &mut Vec<u8>) {
        self.0.put(o);
        self.1.put(o);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok((A::get(r)?, B::get(r)?))
    }
}

impl<A: Tab, B: Tab, C: Tab> Tab for (A, B, C) {
    fn put(&self, o: &mut Vec<u8>) {
        self.0.put(o);
        self.1.put(o);
        self.2.put(o);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok((A::get(r)?, B::get(r)?, C::get(r)?))
    }
}

impl<A: Tab, B: Tab, C: Tab, D: Tab> Tab for (A, B, C, D) {
    fn put(&self, o: &mut Vec<u8>) {
        self.0.put(o);
        self.1.put(o);
        self.2.put(o);
        self.3.put(o);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok((A::get(r)?, B::get(r)?, C::get(r)?, D::get(r)?))
    }
}

impl Tab for jane_core::ids::SpriteId {
    fn put(&self, o: &mut Vec<u8>) {
        self.0.put(o);
    }
    fn get(r: &mut Reader<'_>) -> Result<Self, PackError> {
        Ok(jane_core::ids::SpriteId(u16::get(r)?))
    }
}

/// A struct as its fields in the order named; the struct literal on reading means a field left
/// out is a compile error. Used where the struct is defined, so private fields are reachable.
macro_rules! tab_struct {
    ($t:ty { $($f:ident),* $(,)? }) => {
        impl $crate::tables::Tab for $t {
            fn put(&self, o: &mut alloc::vec::Vec<u8>) {
                $( $crate::tables::Tab::put(&self.$f, o); )*
            }
            fn get(r: &mut $crate::atlas::Reader<'_>) -> Result<Self, $crate::atlas::PackError> {
                Ok(Self { $( $f: $crate::tables::Tab::get(r)?, )* })
            }
        }
    };
}
pub(crate) use tab_struct;

/// A field-less enum as the u8 of its place in `list`, which must name every variant (a
/// variant left out panics on writing, at bake time, and the round-trip tests catch it).
macro_rules! tab_enum {
    ($t:ty, $list:expr) => {
        impl $crate::tables::Tab for $t {
            fn put(&self, o: &mut alloc::vec::Vec<u8>) {
                let list: &[$t] = &$list;
                let i = list.iter().position(|v| v == self).expect("an enum variant its list lacks");
                o.push(i as u8);
            }
            fn get(r: &mut $crate::atlas::Reader<'_>) -> Result<Self, $crate::atlas::PackError> {
                let list: &[$t] = &$list;
                list.get(usize::from(r.u8()?)).copied().ok_or($crate::atlas::PackError("an enum out of its list"))
            }
        }
    };
}
pub(crate) use tab_enum;

tab_enum!(jane_art::sprite::FrameId, jane_art::sprite::FrameId::ALL);
tab_enum!(jane_data::Task, {
    use jane_data::Task as T;
    [T::None, T::Sweep, T::Read, T::Bottles, T::Knit]
});
tab_enum!(jane_art::creature::critter::Critter, jane_art::creature::critter::Critter::ALL);
tab_enum!(jane_art::creature::critter::Pose, {
    use jane_art::creature::critter::Pose as P;
    [P::Stand, P::Peck, P::Look, P::Hop, P::Fly1, P::Fly2, P::Glide]
});
tab_struct!(crate::frame::Src { x, y, w, h });
tab_struct!(crate::frame::Bend { lean, from, span });

/// Writes `v`'s tables.
pub(crate) fn put<T: Tab>(v: &T, o: &mut Vec<u8>) {
    v.put(o);
}

/// Reads a table.
pub(crate) fn get<T: Tab>(r: &mut Reader<'_>) -> Result<T, PackError> {
    T::get(r)
}
