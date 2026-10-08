//! Built zones kept on disk (PORT.md §13.13; ARCHITECTURE.md §9's blueprint disk cache): a
//! console's Continue, Load or New Game on a seed it has built reads the county back in seconds
//! where building it takes a minute (§13.12). One file a zone, `<seed>-<zone>.bp`, so a zone can be
//! read alone (zones built on demand, §13.3 phase 2 item 1).
//!
//! **What is kept:** a packed blueprint only (the console form, `Blueprint::pack`), as postcard
//! writes its serde form (varints, no floats, the same bytes on every target), after a fixed
//! 48-byte header, little-endian: `JBPC`, the format ([`FORMAT`]), the zone, a zero, the seed,
//! the worldgen's source stamp (`jane_world::SOURCE_STAMP`), the content hash, the blueprint's
//! own hash (`jane_world::hash::hash_packed`), the body's length and its xxh3.
//!
//! **What is refused, and rebuilt:** a file whose header names another format, stamp, content,
//! seed or zone; a body of another length or sum; a body that does not decode, or decodes to a
//! blueprint of another zone, not packed, or of another hash. A cache can only make a build
//! faster, never another: what it hands back hashes as the build did.
//!
//! **Memory:** a zone is written as it is encoded, through [`Sink`], a few KB at a time (two
//! passes: the first only counts and sums), so no encoding is ever held whole; a zone read holds
//! its file and the blueprint it decodes, one zone at a time (the county is the largest, about
//! 2 MB of file).
//!
//! **Seeds kept:** the last [`KEEP`] seeds read or built (`seeds.txt`, newest first); a seed
//! pushed off the end has its zones removed.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Write as _;

use jane_core::{Blueprint, ZONE_COUNT, ZoneId};
use xxhash_rust::xxh3::Xxh3;

use crate::blueprints::{Blueprints, BuildError, build_one_packed_with};

/// The encoding's version: bump it when a kept type's serde form changes (a field added, an
/// enum's order), so an older file is refused, not misread.
pub const FORMAT: u16 = 1;

/// The seeds whose zones are kept: the console's three save slots and a New Game.
pub const KEEP: usize = 4;

const MAGIC: [u8; 4] = *b"JBPC";
const HEADER: usize = 48;
/// Bytes an encoding hands its [`Sink`] at a time.
const CHUNK: usize = 16 * 1024;
const INDEX: &str = "seeds.txt";

/// Where the bytes go: false if they could not be written (the file is then not kept).
pub trait Sink {
    fn put(&mut self, bytes: &[u8]) -> bool;
}

/// Files by name in one directory, the platform's: the PSP's Memory Stick, a PC's cache
/// directory, memory in a test.
pub trait Store {
    /// The file `name`, whole; none if there is none or it cannot be read.
    fn read(&mut self, name: &str) -> Option<Vec<u8>>;
    /// The file `name` written from what `fill` puts in the sink: kept only if `fill` and every
    /// write returned true, replacing any file of that name (a temporary, then a rename, so a
    /// power cut leaves the old file or none, never half of one).
    fn write(&mut self, name: &str, fill: &mut dyn FnMut(&mut dyn Sink) -> bool) -> bool;
    /// The file `name` gone, if there is one.
    fn remove(&mut self, name: &str);
}

/// Files in memory: tests, and a harness without a disk.
#[derive(Clone, Debug, Default)]
pub struct MemStore {
    pub files: alloc::collections::BTreeMap<String, Vec<u8>>,
}

impl Store for MemStore {
    fn read(&mut self, name: &str) -> Option<Vec<u8>> {
        self.files.get(name).cloned()
    }

    fn write(&mut self, name: &str, fill: &mut dyn FnMut(&mut dyn Sink) -> bool) -> bool {
        struct V(Vec<u8>);
        impl Sink for V {
            fn put(&mut self, bytes: &[u8]) -> bool {
                self.0.extend_from_slice(bytes);
                true
            }
        }
        let mut v = V(Vec::new());
        let ok = fill(&mut v);
        if ok {
            self.files.insert(String::from(name), v.0);
        }
        ok
    }

    fn remove(&mut self, name: &str) {
        self.files.remove(name);
    }
}

/// How a set of blueprints came to be: zones read back, zones built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub read: u8,
    pub built: u8,
    /// Zones built whose file could not be written.
    pub unwritten: u8,
}

/// The blueprint cache over a [`Store`].
#[derive(Debug)]
pub struct ZoneCache<S: Store> {
    pub store: S,
}

/// The file of `zone`'s blueprint for `seed`.
pub fn file_name(seed: u32, zone: ZoneId) -> String {
    format!("{seed}-{}.bp", zone.name())
}

/// What a file must say to be read back.
struct Head {
    zone: ZoneId,
    seed: u32,
    hash: u64,
    len: u32,
    sum: u64,
}

impl Head {
    fn bytes(&self) -> [u8; HEADER] {
        let mut b = [0u8; HEADER];
        b[0..4].copy_from_slice(&MAGIC);
        b[4..6].copy_from_slice(&FORMAT.to_le_bytes());
        b[6] = self.zone as u8;
        b[8..12].copy_from_slice(&self.seed.to_le_bytes());
        b[12..20].copy_from_slice(&jane_world::SOURCE_STAMP.to_le_bytes());
        b[20..28].copy_from_slice(&jane_data::catalog().content_hash.to_le_bytes());
        b[28..36].copy_from_slice(&self.hash.to_le_bytes());
        b[36..40].copy_from_slice(&self.len.to_le_bytes());
        b[40..48].copy_from_slice(&self.sum.to_le_bytes());
        b
    }
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from(u32_at(b, at)) | u64::from(u32_at(b, at + 4)) << 32
}

/// postcard's output: counted and summed, and handed to a sink [`CHUNK`] bytes at a time (or to
/// none, for the first pass).
struct Out<'a> {
    sink: Option<&'a mut dyn Sink>,
    buf: Vec<u8>,
    len: u64,
    sum: Xxh3,
    ok: bool,
}

impl Out<'_> {
    fn flush(&mut self) {
        if let Some(s) = self.sink.as_deref_mut() {
            self.ok = self.ok && s.put(&self.buf);
        }
        self.sum.update(&self.buf);
        self.buf.clear();
    }
}

impl postcard::ser_flavors::Flavor for Out<'_> {
    type Output = (u64, u64, bool);

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.len += data.len() as u64;
        let mut data = data;
        while !data.is_empty() {
            let n = (CHUNK - self.buf.len()).min(data.len());
            self.buf.extend_from_slice(&data[..n]);
            data = &data[n..];
            if self.buf.len() == CHUNK {
                self.flush();
            }
        }
        Ok(())
    }

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.try_extend(&[data])
    }

    fn finalize(mut self) -> postcard::Result<(u64, u64, bool)> {
        self.flush();
        Ok((self.len, self.sum.digest(), self.ok))
    }
}

/// `bp` through postcard into `sink` (or only counted), as `(length, xxh3, written)`.
fn encode(bp: &Blueprint, sink: Option<&mut dyn Sink>) -> Option<(u64, u64, bool)> {
    let out = Out { sink, buf: Vec::with_capacity(CHUNK), len: 0, sum: Xxh3::new(), ok: true };
    postcard::serialize_with_flavor(bp, out).ok()
}

impl<S: Store> ZoneCache<S> {
    pub fn new(store: S) -> Self {
        ZoneCache { store }
    }

    /// `zone`'s packed blueprint for `seed` as it was kept, if a file is there and passes every
    /// check (the module's doc); none otherwise.
    pub fn load_zone(&mut self, seed: u32, zone: ZoneId) -> Option<Blueprint> {
        let file = self.store.read(&file_name(seed, zone))?;
        let (bp, promised) = decode(&file, seed, zone)?;
        drop(file);
        (jane_world::hash::hash_packed(&bp) == promised).then_some(bp)
    }

    /// `bp` (packed) kept as `seed`'s zone, streamed out as it is encoded. False if it could
    /// not be written (or is not packed); nothing half-written is kept.
    pub fn store_zone(&mut self, seed: u32, bp: &Blueprint) -> bool {
        if bp.packed.is_none() {
            return false;
        }
        // First pass: the length and the sum the header carries; nothing held.
        let Some((len, sum, _)) = encode(bp, None) else { return false };
        let Ok(len) = u32::try_from(len) else { return false };
        let head = Head { zone: bp.zone, seed, hash: jane_world::hash::hash_packed(bp), len, sum };
        self.store.write(&file_name(seed, bp.zone), &mut |sink| {
            sink.put(&head.bytes())
                && encode(bp, Some(sink)).is_some_and(|(l, s, ok)| ok && l == u64::from(len) && s == sum)
        })
    }

    /// Every zone of `seed`, packed: each read back if kept, else built
    /// (`build_one_packed_with`, saying its stages to `report`) and kept as soon as it is built.
    /// The same blueprints `Blueprints::build_packed_with` makes. `seed` becomes the newest
    /// seed kept; one pushed past [`KEEP`] has its zones removed.
    pub fn blueprints(&mut self, seed: u32, report: jane_world::Report<'_>) -> Result<(Blueprints, Tally), BuildError> {
        self.touch(seed);
        let mut tally = Tally::default();
        let mut built: Vec<Arc<Blueprint>> = Vec::with_capacity(ZONE_COUNT);
        for z in ZoneId::ALL {
            let bp = match self.load_zone(seed, z) {
                Some(bp) => {
                    tally.read += 1;
                    bp
                }
                None => {
                    let bp = build_one_packed_with(z, seed, report)?;
                    tally.built += 1;
                    if !self.store_zone(seed, &bp) {
                        tally.unwritten += 1;
                    }
                    bp
                }
            };
            built.push(Arc::new(bp));
        }
        let zones: [Arc<Blueprint>; ZONE_COUNT] = built.try_into().unwrap_or_else(|_| unreachable!("thirteen zones"));
        Ok((Blueprints::from_parts(seed, zones), tally))
    }

    /// Whether `seed` is among the seeds kept (its files not read or checked: a loading
    /// screen's guess at reading rather than building).
    pub fn has_seed(&mut self, seed: u32) -> bool {
        self.seeds().contains(&seed)
    }

    /// The seeds kept, newest first.
    pub fn seeds(&mut self) -> Vec<u32> {
        let text = self.store.read(INDEX).unwrap_or_default();
        core::str::from_utf8(&text).unwrap_or("").lines().filter_map(|l| l.trim().parse().ok()).collect()
    }

    /// `seed` the newest kept; the oldest past [`KEEP`] removed, zones and all.
    fn touch(&mut self, seed: u32) {
        let mut seeds = self.seeds();
        if seeds.first() == Some(&seed) {
            return;
        }
        seeds.retain(|&s| s != seed);
        seeds.insert(0, seed);
        for old in seeds.split_off(KEEP.min(seeds.len())) {
            for z in ZoneId::ALL {
                self.store.remove(&file_name(old, z));
            }
        }
        let mut text = String::new();
        for s in &seeds {
            let _ = writeln!(text, "{s}");
        }
        self.store.write(INDEX, &mut |sink| sink.put(text.as_bytes()));
    }
}

/// A file's header checked and its body decoded: the blueprint and the hash its header promised,
/// if every check before that hash passes.
fn decode(file: &[u8], seed: u32, zone: ZoneId) -> Option<(Blueprint, u64)> {
    if file.len() < HEADER {
        return None;
    }
    let h = &file[..HEADER];
    let fits = h[0..4] == MAGIC
        && u16::from_le_bytes([h[4], h[5]]) == FORMAT
        && h[6] == zone as u8
        && u32_at(h, 8) == seed
        && u64_at(h, 12) == jane_world::SOURCE_STAMP
        && u64_at(h, 20) == jane_data::catalog().content_hash
        && u32_at(h, 36) as usize == file.len() - HEADER;
    let body = &file[HEADER..];
    if !fits || xxhash_rust::xxh3::xxh3_64(body) != u64_at(h, 40) {
        return None;
    }
    let mut bp: Blueprint = postcard::from_bytes(body).ok()?;
    // Lists read back grow by doubling past serde's first guess: held at their length, as built.
    bp.shrink_to_fit();
    (bp.zone == zone && bp.packed.is_some()).then_some((bp, u64_at(h, 28)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oldest_seed_past_keep_is_let_go() {
        let mut c = ZoneCache::new(MemStore::default());
        for seed in 1..=KEEP as u32 + 1 {
            c.touch(seed);
            for z in ZoneId::ALL {
                c.store.files.insert(file_name(seed, z), alloc::vec![1]);
            }
        }
        assert_eq!(c.seeds(), [5, 4, 3, 2]);
        assert!(!c.store.files.contains_key(&file_name(1, ZoneId::County)), "seed 1's zones removed");
        assert!(c.store.files.contains_key(&file_name(2, ZoneId::School)));
        c.touch(2);
        assert_eq!(c.seeds(), [2, 5, 4, 3], "a seed read again is the newest");
        assert!(c.has_seed(3) && !c.has_seed(1));
    }
}
