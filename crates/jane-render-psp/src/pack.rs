//! The PSP pack's tables (`JPK2`, PORT.md §13.4): read from the file's head, before its pages.
//! The pages stay on the Memory Stick until a frame names one ([`crate::store`]).

use alloc::vec::Vec;

/// Why a pack could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackError(pub &'static str);

/// A page as the table names it: where its bytes are (1024 bytes of CLUT, then the swizzled
/// `T8` px), its size and its category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageInfo {
    pub offset: u32,
    pub w: u16,
    pub h: u16,
    pub cat: u8,
    pub swizzled: bool,
    pub px_bytes: u32,
}

impl PageInfo {
    /// Its bytes in the file: the CLUT and the px.
    pub fn bytes(&self) -> u32 {
        1024 + self.px_bytes
    }
}

/// A frame's record: its page (`u16::MAX`: draws nothing), its rect there, and its anchor from
/// the rect's top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rec {
    pub cat: u8,
    pub frame: u8,
    pub sprite: u16,
    pub vs: u8,
    pub page: u16,
    pub u: u16,
    pub v: u16,
    pub w: u16,
    pub h: u16,
    pub ax: i16,
    pub ay: i16,
}

/// A run of pages loaded together: a unit's sprite (`sprite`), or a whole category
/// (`sprite == u16::MAX`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    pub cat: u8,
    pub sprite: u16,
    pub first: u16,
    pub pages: u16,
}

/// What a presenter `RefId` draws on the PSP: a record index (`u32::MAX`: nothing) and the
/// presenter's anchor from the record's rect's top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefLink {
    pub rec: u32,
    pub ax: i16,
    pub ay: i16,
}

/// The pack's tables.
#[derive(Clone, Debug, Default)]
pub struct Pack {
    pub pages: Vec<PageInfo>,
    pub recs: Vec<Rec>,
    pub groups: Vec<Group>,
    pub refs: Vec<RefLink>,
    pub canonical: u64,
    /// Where the page data begins: the bytes before it are the tables ([`Pack::head`]).
    pub data_off: u32,
}

/// The header's size: read it first, then [`Pack::head_len`] bytes in all.
pub const HEADER: usize = 48;

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], PackError> {
        let s = self.b.get(self.at..self.at + n).ok_or(PackError("short"))?;
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, PackError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PackError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Result<i16, PackError> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, PackError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Result<u64, PackError> {
        Ok(u64::from(self.u32()?) | u64::from(self.u32()?) << 32)
    }
}

impl Pack {
    /// How many bytes of the file hold the tables, from its first [`HEADER`] bytes.
    pub fn head_len(header: &[u8]) -> Result<usize, PackError> {
        let mut r = Rd { b: header, at: 0 };
        if r.take(4)? != b"JPK2" {
            return Err(PackError("not a JPK2 pack"));
        }
        r.at = 20;
        Ok(r.u32()? as usize)
    }

    /// The tables from the file's first [`head_len`](Self::head_len) bytes (or the whole file).
    pub fn head(b: &[u8]) -> Result<Pack, PackError> {
        let mut r = Rd { b, at: 0 };
        if r.take(4)? != b"JPK2" {
            return Err(PackError("not a JPK2 pack"));
        }
        if r.u16()? != 2 {
            return Err(PackError("JPK version"));
        }
        let pages = r.u16()? as usize;
        let sprites = r.u32()? as usize;
        let (page_off, rec_off, data_off) = (r.u32()? as usize, r.u32()? as usize, r.u32()?);
        let canonical = r.u64()?;
        let group_off = r.u32()? as usize;
        let groups = r.u16()? as usize;
        r.u16()?;
        let (ref_off, refs) = (r.u32()? as usize, r.u32()? as usize);
        r.at = page_off;
        let mut p = Pack { canonical, data_off, ..Pack::default() };
        for _ in 0..pages {
            let offset = r.u32()?;
            let (w, h) = (r.u16()?, r.u16()?);
            let (cat, psm, clut, flags) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
            if psm != 5 || clut != 3 {
                return Err(PackError("a page not T8 over an 8888 CLUT"));
            }
            let px_bytes = r.u32()?;
            p.pages.push(PageInfo { offset, w, h, cat, swizzled: flags & 1 != 0, px_bytes });
        }
        r.at = rec_off;
        for _ in 0..sprites {
            let (cat, frame, sprite) = (r.u8()?, r.u8()?, r.u16()?);
            let vs = r.u8()?;
            r.u8()?;
            let (page, u, v, w, h) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?, r.u16()?);
            let (ax, ay) = (r.i16()?, r.i16()?);
            p.recs.push(Rec { cat, frame, sprite, vs, page, u, v, w, h, ax, ay });
        }
        r.at = group_off;
        for _ in 0..groups {
            let cat = r.u8()?;
            r.u8()?;
            p.groups.push(Group { cat, sprite: r.u16()?, first: r.u16()?, pages: r.u16()? });
        }
        r.at = ref_off;
        for _ in 0..refs {
            p.refs.push(RefLink { rec: r.u32()?, ax: r.i16()?, ay: r.i16()? });
        }
        Ok(p)
    }
}
