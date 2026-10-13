//! Zones as baked (`assets/world/<zone>.bp`, `jane bake-world`): a blueprint compiled from its authored
//! source (`data/world/<zone>/`), as built (not packed), so nothing builds it at run time. Only the
//! county is baked so far; every other zone is still built from the seed.
//!
//! **The file:** a 20-byte header, little-endian: `JBKD`, the format ([`FORMAT`]), the zone, a zero, the
//! blueprint's hash (`jane_world::hash::hash`) and the length of its encoding; then that encoding (the
//! blueprint's serde form as postcard writes it, the same bytes on every target) in lz4's block format.
//!
//! **Read:** decompressed, decoded, its lists held at their length, hashed and checked against the header;
//! packed for the console form. A file that fails any of it is refused, never played.
//!
//! **Where from:** PC, the CLI and the tests carry the files in the program (the `baked` feature); a
//! console's shell reads them from beside the program ([`set_reader`]).

use alloc::string::String;
use alloc::vec::Vec;

use jane_core::{Blueprint, ZoneId};

/// The encoding's version: bump it when the blueprint's serde form changes, and bake again.
pub const FORMAT: u16 = 1;

const MAGIC: [u8; 4] = *b"JBKD";
const HEADER: usize = 20;

/// Why a baked zone was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BakedError {
    /// There is no file (the zone is not baked, a console's shell set no reader, or it found none).
    Missing,
    Header,
    Decompress,
    Decode,
    /// It decoded to a blueprint of another zone, or one not as built.
    NotTheZone,
    /// Its hash is not the one its header promised.
    Hash,
}

/// A zone's baked file's name (`county.bp`), in `assets/world/` and beside a console's program.
pub fn file_name(zone: ZoneId) -> String {
    alloc::format!("{}.bp", zone.name())
}

/// The file for `bp`, a zone as built (not packed).
pub fn encode(bp: &Blueprint) -> Vec<u8> {
    assert!(bp.packed.is_none(), "a zone as built");
    let body = postcard::to_allocvec(bp).expect("a blueprint encodes");
    let mut out = Vec::with_capacity(HEADER + body.len() / 4);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT.to_le_bytes());
    out.extend_from_slice(&[bp.zone as u8, 0]);
    out.extend_from_slice(&jane_world::hash::hash(bp).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&lz4_flex::block::compress(&body));
    out
}

/// Zone `zone` in `file`, packed when `pack` (the console form, `Blueprint::pack`).
pub fn decode(file: &[u8], zone: ZoneId, pack: bool) -> Result<Blueprint, BakedError> {
    let h = file.get(..HEADER).ok_or(BakedError::Header)?;
    let le32 = |at: usize| u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]]);
    if h[0..4] != MAGIC || u16::from_le_bytes([h[4], h[5]]) != FORMAT || h[6] != zone as u8 {
        return Err(BakedError::Header);
    }
    let hash = u64::from(le32(8)) | u64::from(le32(12)) << 32;
    let len = le32(16) as usize;
    let body = lz4_flex::block::decompress(&file[HEADER..], len).map_err(|_| BakedError::Decompress)?;
    let mut bp: Blueprint = postcard::from_bytes(&body).map_err(|_| BakedError::Decode)?;
    drop(body);
    if bp.zone != zone || bp.packed.is_some() {
        return Err(BakedError::NotTheZone);
    }
    // Lists read back grow by doubling past serde's first guess: held at their length, as built.
    bp.shrink_to_fit();
    if jane_world::hash::hash(&bp) != hash {
        return Err(BakedError::Hash);
    }
    if pack {
        bp.pack();
    }
    Ok(bp)
}

/// The county's file from its source: `plan.txt`, `places.txt` and `routes.txt`
/// (`jane_world::county::terraced::source`) compiled, laid and proved. What `jane bake-world` writes, and
/// what the committed file must be.
pub fn bake_county(plan: &str, places: &str, routes: &str, report: jane_world::Report<'_>) -> Result<Vec<u8>, String> {
    use jane_world::county::terraced::{build, source};
    let src = source::parse(plan, places, routes).map_err(|e| alloc::format!("{e}"))?;
    let built = build::build(&src, report, false).map_err(|e| alloc::format!("the county does not hold: {e:?}"))?;
    Ok(encode(&built.bp))
}

/// The baked county, as the build carries it.
#[cfg(feature = "baked")]
pub static COUNTY: &[u8] = include_bytes!("../../../assets/world/county.bp");

/// Zone `zone`'s file as the build carries it, if it is baked.
#[cfg(feature = "baked")]
pub fn carried(zone: ZoneId) -> Option<&'static [u8]> {
    (zone == ZoneId::County).then_some(COUNTY)
}

/// Where a build that does not carry its baked zones (a console's: the files are beside the program)
/// reads one's file from, by zone; set once by the shell, before the first New Game.
#[cfg(not(feature = "baked"))]
static READ: once_cell::race::OnceBox<fn(ZoneId) -> Option<Vec<u8>>> = once_cell::race::OnceBox::new();

/// The shell's reader of a baked zone's file (a console's, [`file_name`] beside the program).
#[cfg(not(feature = "baked"))]
pub fn set_reader(read: fn(ZoneId) -> Option<Vec<u8>>) {
    let _ = READ.set(alloc::boxed::Box::new(read));
}

/// Baked zone `zone`, from the build's own copy or the shell's file, packed when `pack`.
pub fn load(zone: ZoneId, pack: bool) -> Result<Blueprint, BakedError> {
    #[cfg(feature = "baked")]
    {
        decode(carried(zone).ok_or(BakedError::Missing)?, zone, pack)
    }
    #[cfg(not(feature = "baked"))]
    {
        let file = READ.get().and_then(|read| read(zone)).ok_or(BakedError::Missing)?;
        decode(&file, zone, pack)
    }
}
