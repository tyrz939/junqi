//! `jane bake-chunks` (PLAN-PSP-SPEED.md §5, P1b): every chunk of the county painted by the console's
//! own path (`jane_present::chunk_pack::Baker`: a job run and landed into a `T8` slot, as a PSP's
//! worker and game thread do it) and written as one pack (`JCK1`, `chunk_pack`'s doc), each chunk's
//! albedo, height and sidecar compressed on their own: zstd -19 by default, lz4 HC by flag.
//!
//! Staleness: the pack's header carries the digest of every chunk as painted, so `--if-stale` (what
//! `tools/psp-dist.sh` runs) paints the county, compares, and bakes only when the painter's output has
//! moved. A small committed key (`assets/world/county.jck.key`, the digest of the [`probes`]) lets the
//! slow tier catch an art change that was not baked again, as `county.bp`'s test does for the county.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use jane_present::ChunkId;
use jane_present::chunk_pack::{self, Baked, Baker, Codec};
use jane_sim::{Blueprints, Seat, Sim};
use xxhash_rust::xxh3::xxh3_64;

pub const USAGE: &str =
    "  bake-chunks [--out FILE] [--codec zstd|lz4|raw] [--level N] [--threads N] [--if-stale|--check|--verify|--key]
                                      paint every county chunk by the console's path into one pack (JCK1;
                                      default target/psp-dist/bake/county.jck, zstd -19). --if-stale: bake
                                      only if the pack's digest is not the painter's now; --check: only say;
                                      --verify: decode every chunk of the pack against the paint, bit for bit;
                                      --key: only rewrite assets/world/county.jck.key (the probes' digest)";

/// The default pack, beside the PSP's other baked files (`tools/psp-dist.sh`); never committed.
pub const OUT: &str = "target/psp-dist/bake/county.jck";
/// The committed key: the digest of the [`probes`] as painted now.
pub const KEY: &str = "assets/world/county.jck.key";

/// The county's chunks a side (1536 cells, 16 a chunk).
fn county_chunks() -> (u16, u16) {
    let side = (1536 / jane_present::frame::CHUNK_CELLS) as u16;
    (side, side)
}

/// The chunks the committed key and the slow tier paint: the four corners and the middle, and 59
/// spread over the county, a different column and row in each of an 8 x 8 grid's cells (towns, the
/// lake, woods, the works and the edges among them; the test checks that water and lit windows are).
pub fn probes() -> Vec<ChunkId> {
    let (cols, rows) = county_chunks();
    let mut out = vec![
        ChunkId { cx: 0, cy: 0 },
        ChunkId { cx: cols - 1, cy: 0 },
        ChunkId { cx: 0, cy: rows - 1 },
        ChunkId { cx: cols - 1, cy: rows - 1 },
        ChunkId { cx: cols / 2, cy: rows / 2 },
    ];
    let (sx, sy) = (cols / 8, rows / 8);
    for j in 0..8u16 {
        for i in 0..8u16 {
            let id = ChunkId { cx: i * sx + (j * 5 + i * 3) % sx, cy: j * sy + (i * 7 + j) % sy };
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// A chunk baked: its record's hash, its blob, and its frames' bytes (albedo, height, sidecar) and
/// its sidecar's raw bytes.
type Done = (u64, Vec<u8>, [usize; 4]);

/// A digest of chunks' record hashes, in order.
fn digest(hashes: &[u64]) -> u64 {
    let bytes: Vec<u8> = hashes.iter().flat_map(|h| h.to_le_bytes()).collect();
    xxh3_64(&bytes)
}

/// A New Game's county as the console reads it (packed, from `county.bp`), the view's seed the
/// curated one.
fn county() -> Result<Sim, String> {
    let src: Arc<dyn jane_sim::blueprints::ZoneSource> =
        Arc::new(jane_sim::blueprints::Build { packed: true, load: None });
    let bps = Blueprints::on_demand_with(jane_present::bench::SEED, src, &mut |_| {}).map_err(|e| e.to_string())?;
    Ok(Sim::new_game_with(bps, "Jane"))
}

/// Paints `ids` on `threads` threads, each its own sim and console painter, calling `each(i,
/// &baked)` for chunk `ids[i]` (on its thread).
fn paint_all(ids: &[ChunkId], threads: usize, each: &(dyn Fn(usize, &Baked) + Sync)) -> Result<(), String> {
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads.max(1))
            .map(|_| {
                s.spawn(|| -> Result<(), String> {
                    let sim = county()?;
                    let view = sim.view(Seat(0)).ok_or("seat 0 plays")?;
                    let mut baker = Baker::new(&view);
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&id) = ids.get(i) else { return Ok(()) };
                        each(i, &baker.paint(&view, id));
                    }
                })
            })
            .collect();
        workers.into_iter().try_for_each(|w| w.join().map_err(|_| "a painter thread panicked".to_string())?)
    })
}

/// The record hashes of `ids` as painted now.
pub fn hashes(ids: &[ChunkId], threads: usize) -> Result<Vec<u64>, String> {
    let out: Vec<std::sync::atomic::AtomicU64> = ids.iter().map(|_| std::sync::atomic::AtomicU64::new(0)).collect();
    paint_all(ids, threads, &|i, b| out[i].store(xxh3_64(&b.record()), Ordering::Relaxed))?;
    Ok(out.into_iter().map(std::sync::atomic::AtomicU64::into_inner).collect())
}

/// The committed key's line for the probes' digest.
pub fn key_line(probe: u64) -> String {
    format!("jck{} probe {probe:016x}\n", chunk_pack::VERSION)
}

/// A frame compressor for `codec` at `level`.
pub fn compressor(codec: Codec, level: i32) -> impl Fn(&[u8]) -> Vec<u8> {
    move |raw: &[u8]| match codec {
        Codec::Raw => raw.to_vec(),
        Codec::Zstd => zstd::bulk::compress(raw, level).expect("zstd compresses"),
        Codec::Lz4 => lz4::block::compress(raw, Some(lz4::block::CompressionMode::HIGHCOMPRESSION(level)), false)
            .expect("lz4 HC compresses"),
    }
}

fn every_chunk() -> Vec<ChunkId> {
    let (cols, rows) = county_chunks();
    (0..rows).flat_map(|cy| (0..cols).map(move |cx| ChunkId { cx, cy })).collect()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from(OUT);
    let (mut codec, mut level, mut level_set) = (Codec::Zstd, 19, false);
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(6));
    let (mut if_stale, mut check, mut key_only, mut verify_all) = (false, false, false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(it.next().ok_or("--out FILE")?),
            "--codec" => {
                codec = match it.next().map(String::as_str) {
                    Some("zstd") => Codec::Zstd,
                    Some("lz4") => Codec::Lz4,
                    Some("raw") => Codec::Raw,
                    _ => return Err("--codec zstd|lz4|raw".into()),
                }
            }
            "--level" => {
                level = it.next().and_then(|s| s.parse().ok()).ok_or("--level N")?;
                level_set = true;
            }
            "--threads" => threads = it.next().and_then(|s| s.parse().ok()).ok_or("--threads N")?,
            "--if-stale" => if_stale = true,
            "--check" => check = true,
            "--key" => key_only = true,
            "--verify" => verify_all = true,
            o => return Err(format!("unknown option {o}\n{USAGE}")),
        }
    }
    if codec == Codec::Lz4 && !level_set {
        level = 12;
    }
    if key_only {
        return write_key(digest(&hashes(&probes(), threads)?));
    }
    let ids = every_chunk();
    if verify_all {
        return verify(&out, &ids, threads);
    }
    let t = Instant::now();
    if if_stale || check {
        let now = digest(&hashes(&ids, threads)?);
        let old = std::fs::read(&out).ok();
        let held = old.as_deref().and_then(|b| chunk_pack::Pack::open(b).ok()).map(|p| (p.digest, p.codec));
        let fresh = held.is_some_and(|(d, c)| d == now && (check || c == codec));
        println!("painted {} chunks in {:.1} s", ids.len(), t.elapsed().as_secs_f64());
        if fresh {
            println!("{} is the painter's output now (digest {now:016x})", out.display());
            return Ok(());
        }
        if check {
            return Err(format!(
                "{} is stale or missing (the painter's output has moved): run `cargo run --release -p jane-cli -- \
                 bake-chunks` or `bash tools/psp-dist.sh`",
                out.display()
            ));
        }
    }
    let t = Instant::now();
    let compress = compressor(codec, level);
    let blobs: Vec<std::sync::Mutex<Done>> =
        ids.iter().map(|_| std::sync::Mutex::new((0, Vec::new(), [0; 4]))).collect();
    paint_all(&ids, threads, &|i, b| {
        let mut sizes = [0usize; 4];
        let mut k = 0;
        let blob = b.blob(|raw| {
            let c = compress(raw);
            sizes[k] = c.len();
            k += 1;
            c
        });
        sizes[3] = b.sidecar().len();
        *blobs[i].lock().expect("a blob's lock") = (xxh3_64(&b.record()), blob, sizes);
    })?;
    let blobs: Vec<Done> = blobs.into_iter().map(|m| m.into_inner().expect("a blob's lock")).collect();
    let all = digest(&blobs.iter().map(|b| b.0).collect::<Vec<_>>());
    let probe_ix: Vec<usize> =
        probes().iter().map(|p| usize::from(p.cy) * usize::from(county_chunks().0) + usize::from(p.cx)).collect();
    let probe = digest(&probe_ix.iter().map(|&i| blobs[i].0).collect::<Vec<_>>());
    let sums: [usize; 4] = std::array::from_fn(|k| blobs.iter().map(|b| b.2[k]).sum());
    let sum = |k: usize| sums[k];
    let pack = chunk_pack::write(codec, county_chunks(), all, &blobs.into_iter().map(|b| b.1).collect::<Vec<_>>());
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&out, &pack).map_err(|e| format!("{}: {e}", out.display()))?;
    let n = ids.len();
    println!(
        "{} written: {n} chunks, {} KB ({codec:?} level {level}; a chunk {} B albedo, {} B height, {} B sidecar \
         of {} raw), digest {all:016x}, baked in {:.1} s on {threads} threads",
        out.display(),
        pack.len() / 1024,
        sum(0) / n,
        sum(1) / n,
        sum(2) / n,
        sum(3) / n,
        t.elapsed().as_secs_f64()
    );
    write_key(probe)
}

/// Every chunk of the pack at `out` decoded and compared with the chunk painted now, bit for bit.
fn verify(out: &std::path::Path, ids: &[ChunkId], threads: usize) -> Result<(), String> {
    let t = Instant::now();
    let bytes = std::fs::read(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pack = chunk_pack::Pack::open(&bytes).map_err(|e| format!("{}: {e:?}", out.display()))?;
    if (pack.cols, pack.rows) != county_chunks() {
        return Err(format!("{}: {}x{} chunks, not the county's", out.display(), pack.cols, pack.rows));
    }
    let bad = AtomicUsize::new(0);
    let first = std::sync::Mutex::new(None);
    paint_all(ids, threads, &|i, b| {
        let mut got = Baked::default();
        let ok = pack
            .blob(ids[i])
            .is_some_and(|blob| chunk_pack::Decoder::new().decode(pack.codec, blob, &mut got).is_ok() && got == *b);
        if !ok {
            bad.fetch_add(1, Ordering::Relaxed);
            first.lock().expect("the first's lock").get_or_insert(ids[i]);
        }
    })?;
    let bad = bad.into_inner();
    if bad > 0 {
        return Err(format!(
            "{}: {bad} of {} chunks do not decode to the paint (the first {:?}): bake it again",
            out.display(),
            ids.len(),
            first.into_inner().expect("the first's lock")
        ));
    }
    println!(
        "{}: all {} chunks decode ({:?}) bit for bit to the console's paint, in {:.1} s",
        out.display(),
        ids.len(),
        pack.codec,
        t.elapsed().as_secs_f64()
    );
    Ok(())
}

/// The committed key rewritten if `probe` is not what it holds.
fn write_key(probe: u64) -> Result<(), String> {
    let line = key_line(probe);
    if std::fs::read_to_string(KEY).ok().as_deref().map(str::trim) == Some(line.trim()) {
        println!("{KEY} unchanged ({})", line.trim());
        return Ok(());
    }
    std::fs::write(KEY, &line).map_err(|e| format!("{KEY}: {e}"))?;
    println!("{KEY} written ({}): commit it", line.trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::chunk_pack::{Decoder, Pack};

    /// The probes painted by the console's path, baked by each codec and decoded by the console's
    /// decoders, are bit for bit the chunks as painted; and the committed key is their digest now,
    /// so an art or painter change not baked again fails here.
    #[test]
    fn baked_chunks_decode_to_the_console_paint_and_the_key_is_fresh() {
        let ids = probes();
        let painted: Vec<std::sync::Mutex<Option<Baked>>> = ids.iter().map(|_| std::sync::Mutex::new(None)).collect();
        paint_all(&ids, 4, &|i, b| *painted[i].lock().unwrap() = Some(b.clone())).expect("the probes paint");
        let painted: Vec<Baked> = painted.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect();
        assert!(painted.iter().any(|b| !b.water.is_empty()), "a probe holds water");
        assert!(painted.iter().any(|b| !b.windows.is_empty() || !b.openings.is_empty()), "a probe holds a house");
        assert!(painted.iter().any(|b| !b.placed.is_empty()), "a probe holds trees");
        for (codec, level) in [(Codec::Zstd, 19), (Codec::Lz4, 12), (Codec::Raw, 0)] {
            let blobs: Vec<Vec<u8>> = painted.iter().map(|b| b.blob(compressor(codec, level))).collect();
            // A pack of the probes alone, one row.
            let bytes = chunk_pack::write(codec, (blobs.len() as u16, 1), 7, &blobs);
            let pack = Pack::open(&bytes).expect("the pack opens");
            let (mut d, mut out) = (Decoder::new(), Baked::default());
            for (i, want) in painted.iter().enumerate() {
                d.decode(pack.codec, pack.blob(ChunkId { cx: i as u16, cy: 0 }).unwrap(), &mut out).unwrap();
                assert!(&out == want, "{codec:?}: probe {:?} decodes to another chunk", ids[i]);
            }
        }
        // The same chunk painted again, by another painter, is the same.
        let again = hashes(&ids[..3], 1).unwrap();
        assert_eq!(again, painted[..3].iter().map(|b| xxh3_64(&b.record())).collect::<Vec<_>>());
        let probe = digest(&painted.iter().map(|b| xxh3_64(&b.record())).collect::<Vec<_>>());
        let key = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(KEY);
        let held = std::fs::read_to_string(&key).unwrap_or_default();
        assert!(
            held.trim() == key_line(probe).trim(),
            "the console's chunks have changed since they were baked ({KEY} holds {:?}, the painter now {:?}): \
             run `cargo run --release -p jane-cli -- bake-chunks --key` and commit {KEY} (tools/psp-dist.sh \
             bakes the pack again by itself)",
            held.trim(),
            key_line(probe).trim()
        );
    }

    /// The bake's painter is the console's: each chunk a console presenter
    /// (`Present::from_tables_console`) paints on its worker and lands as she walks from a New Game's
    /// start, the baker paints the same, and a baked chunk laid into a slot is that slot. The view
    /// she starts in is painted in the tick instead (`Terrain::paint`), whose landing tags the water
    /// and wet ground in the CLUT's alpha by the heights before they are made relative: its colours
    /// are the same, its alpha may not be on raised ground.
    #[test]
    fn the_baker_paints_what_the_console_presenter_lands() {
        use jane_core::Angle;
        use jane_present::{ChunkLayers, Present, Tier};
        use jane_sim::{InputFrame, StepInput};
        let a = Present::new(Tier::T0);
        let mut b = Present::from_tables_console(Tier::T0, &a.tables(), 12).unwrap();
        drop(a);
        let mut sim = county().unwrap();
        let mut by_job = Vec::new();
        // A job a tick, as a worker takes them, walking east then standing till nothing on screen
        // waits for the painter.
        for t in 0..600 {
            let frame = if t < 240 { InputFrame::walk(Angle::EAST) } else { InputFrame::IDLE };
            let frames = [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE];
            sim.step(&StepInput { frames, commands: &[] });
            let events = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).unwrap();
            b.tick(&v, &events);
            if let Some(mut job) = b.take_paint_job(&v) {
                by_job.push(job.id);
                job.run();
                b.land(job);
            } else if t > 240 && b.chunks_waiting() == 0 {
                break;
            }
        }
        assert_eq!(b.chunks_waiting(), 0, "the view is painted");
        let v = sim.view(Seat(0)).unwrap();
        let mut baker = Baker::new(&v);
        let f = b.draw(128, (480, 272));
        assert!(f.t8);
        let mut seen = 0;
        for c in &f.chunks {
            let held = &f.layers[usize::from(c.slot)];
            let mut laid = ChunkLayers::new_t8();
            baker.paint(&v, c.id).lay(&mut laid);
            if !by_job.contains(&c.id) {
                let rgb = |l: &ChunkLayers, k: usize| l.t8_abgr(k) & 0x00ff_ffff;
                assert!((0..laid.height.len()).all(|k| rgb(&laid, k) == rgb(held, k)), "chunk {:?}'s colours", c.id);
                continue;
            }
            let n = usize::from(held.clut_n);
            assert_eq!(laid.clut_n, held.clut_n, "chunk {:?}'s CLUT", c.id);
            assert!(laid.clut[..n] == held.clut[..n], "chunk {:?}'s CLUT", c.id);
            assert!(laid.albedo == held.albedo, "chunk {:?}'s albedo", c.id);
            assert!(laid.height == held.height, "chunk {:?}'s heights", c.id);
            assert_eq!((&laid.glow, &laid.water), (&held.glow, &held.water), "chunk {:?}", c.id);
            seen += 1;
        }
        assert!(seen >= 2, "{seen} chunks on screen painted by the worker");
    }
}
