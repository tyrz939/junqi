//! The storage and decode micro-bench (PLAN-PSP-SPEED.md P0d/P0e), run once at the end of the
//! built-in benchmark and its lines appended to `bench.txt`. Never run in normal play.
//!
//! Reads: the game's own `jane-psp.jpk`, opened a second time, read with `sceIoRead` and with
//! `sceIoReadAsync` + `sceIoWaitAsync`, large and sequential (256 KB reads from the file's start)
//! and 64 KB reads at pseudo-random 64 KB-aligned offsets. Decode: one baked 256x256 T8 chunk (the
//! index plane of a real painted county chunk, `tools/psp-measure/microchunk.rs`) compressed with
//! zstd -19 (`ruzstd`) and lz4 HC (`lz4_flex`), each decoded 100 times and checked against the
//! raw plane's hash. Then the VFPU spike's pan against the integer one (`jane_audio_psp::vfpu`).
//! All times come from the hardware microsecond clock (`sceKernelGetSystemTimeLow`).

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use psp::sys;

const ZST: &[u8] = include_bytes!("../assets/chunk.zst");
const LZ4: &[u8] = include_bytes!("../assets/chunk.lz4");
/// The raw plane's FNV-1a (64-bit): what both decoders must give back.
const RAW_FNV: u64 = 0xa5b8_ee0b_6f59_9524;
const RAW_LEN: usize = 256 * 256;
const REPS: u32 = 100;
/// Bytes read in each sequential test, at most (the file is smaller in a test pack).
const SEQ_BYTES: u32 = 8 << 20;
const SEQ_READ: usize = 256 << 10;
const RND_READ: usize = 64 << 10;
const RND_COUNT: u32 = 64;

fn now() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &c| (h ^ u64::from(c)).wrapping_mul(0x100_0000_01b3))
}

/// `bytes` in `us` as MB/s (1 MB = 2^20 B) with two decimals, e.g. "12.34".
fn mbs(bytes: u64, us: u32) -> String {
    let h = bytes * 1_000_000 * 100 / (u64::from(us.max(1)) << 20);
    format!("{}.{:02}", h / 100, h % 100)
}

/// `us` as milliseconds with three decimals.
fn ms(us: u64) -> String {
    format!("{}.{:03}", us / 1000, us % 1000)
}

/// One read test: `count` reads of `buf.len()` bytes at `at(i)`, sync or async. Returns (bytes, us),
/// or None when a read came up short.
fn reads(fd: sys::SceUid, buf: &mut [u8], count: u32, asynch: bool, at: &dyn Fn(u32) -> u32) -> Option<(u64, u32)> {
    let t0 = now();
    let mut total = 0u64;
    for i in 0..count {
        // SAFETY: an open fd; `buf` is ours and outlives each call (the async one is waited for).
        let n = unsafe {
            if sys::sceIoLseek(fd, i64::from(at(i)), sys::IoWhence::Set) != i64::from(at(i)) {
                return None;
            }
            if asynch {
                let mut res = 0i64;
                if sys::sceIoReadAsync(fd, buf.as_mut_ptr().cast(), buf.len() as u32) < 0 {
                    return None;
                }
                if sys::sceIoWaitAsync(fd, &mut res) < 0 {
                    return None;
                }
                res
            } else {
                i64::from(sys::sceIoRead(fd, buf.as_mut_ptr().cast(), buf.len() as u32))
            }
        };
        if n != buf.len() as i64 {
            return None;
        }
        total += n as u64;
    }
    Some((total, now().wrapping_sub(t0)))
}

fn read_lines(path: &str, out: &mut String) {
    let mut z = Vec::from(path.as_bytes());
    z.push(0);
    // SAFETY: a NUL-terminated path that outlives the call.
    let fd = unsafe { sys::sceIoOpen(z.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        out.push_str("micro read open_failed\n");
        return;
    }
    // SAFETY: an open fd.
    let size = unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::End) } as u32;
    let mut buf = vec![0u8; SEQ_READ];
    let seq_n = (SEQ_BYTES.min(size) / SEQ_READ as u32).max(1);
    let blocks = (size / RND_READ as u32).max(1);
    // A fixed LCG so every run reads the same blocks.
    let rnd = |i: u32| (i.wrapping_mul(2_654_435_761).wrapping_add(12_345) >> 7) % blocks * RND_READ as u32;
    let seq = |i: u32| i * SEQ_READ as u32;
    let tests: [(&str, bool, bool); 4] =
        [("seq_sync", false, false), ("seq_async", false, true), ("rnd64k_sync", true, false), ("rnd64k_async", true, true)];
    for (name, random, asynch) in tests {
        let (n, b) = if random { (RND_COUNT, &mut buf[..RND_READ]) } else { (seq_n, &mut buf[..]) };
        let at: &dyn Fn(u32) -> u32 = if random { &rnd } else { &seq };
        out.push_str(&match reads(fd, b, n, asynch, at) {
            Some((bytes, us)) => format!("micro read {name} mb_s {} bytes {bytes} ms {}\n", mbs(bytes, us), ms(u64::from(us))),
            None => format!("micro read {name} failed\n"),
        });
    }
    // SAFETY: our fd, closed once.
    unsafe { sys::sceIoClose(fd) };
    out.push_str(&format!("micro read file_bytes {size}\n"));
}

/// One codec: `REPS` decodes timed as a whole, the plane checked on the last.
fn timed(out: &mut String, name: &str, packed: usize, mut decode: impl FnMut(&mut [u8]) -> bool) {
    let mut raw = vec![0u8; RAW_LEN];
    let t0 = now();
    let mut ok = true;
    for _ in 0..REPS {
        ok &= decode(&mut raw);
    }
    let us = u64::from(now().wrapping_sub(t0));
    let good = ok && fnv(&raw) == RAW_FNV;
    out.push_str(&format!(
        "micro decode {name} packed {packed} ms_per_chunk {} reps {REPS} ok {good}\n",
        ms(us / u64::from(REPS))
    ));
}

/// The micro-bench's lines for `bench.txt` (the pack at `pack`).
pub fn run(pack: &str) -> String {
    let mut out = String::new();
    read_lines(pack, &mut out);
    let mut zd = ruzstd::decoding::FrameDecoder::new();
    timed(&mut out, "zstd19", ZST.len(), |raw| zd.decode_all(ZST, raw).is_ok_and(|n| n == RAW_LEN));
    timed(&mut out, "lz4hc", LZ4.len(), |raw| lz4_flex::block::decompress_into(LZ4, raw).is_ok_and(|n| n == RAW_LEN));
    // The VFPU spike: the mixer's group pan in integers and on the VFPU, a block each
    // (PLAN-PSP-SPEED.md's VFPU section).
    out.push_str(&jane_audio_psp::vfpu::bench(now));
    out
}
