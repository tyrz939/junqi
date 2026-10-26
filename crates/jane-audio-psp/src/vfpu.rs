//! The PSP's VFPU spike (PLAN-PSP-SPEED.md, "VFPU: where it pays"): the mixer's group pan and
//! send ([`jane_audio::tracker::Pan`]) four frames at a time, and its micro-bench against the
//! integer one. PSP only, on in play unless a script's `novfpu` word turns it off.
//!
//! - Each quad: `lv.q` the group's four frames and the buses', `vi2f.q` them to floats, `vscl.q`
//!   by the gain (`g / 4096`, a power of two off the integer's, so exact), `vadd.q`, `vf2id.q`
//!   (rounding down, as `>> 12` does) and `sv.q` back. The sum is the integer's while the product
//!   fits a float's 24 bits; past it (a loud group, a full gain) a frame can be one unit off:
//!   the bench prints the largest difference it saw.
//! - The VFPU's registers are not LLVM's: nothing the compiler emits touches them, so the gains
//!   loaded in one `asm!` are there in the next (no call sits between). The thread running this
//!   must be made with `ThreadAttributes::VFPU` (the audio thread and the game's are), or the
//!   first VFPU instruction faults.
//! - Every pointer must be on a 16-byte line (`lv.q`/`sv.q` fault otherwise): the mixer's buses
//!   are [`Lane`]s; anything else falls back to [`pan_int`].

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use jane_audio::tracker::{BLOCK, Lane, pan_int};
use psp::vfpu_asm;

/// The gains as the VFPU's scales: `g / 4096` as a float's bits (exact: a power of two).
fn scale(g: i32) -> u32 {
    (g as f32 * (1.0 / 4096.0)).to_bits()
}

fn aligned(p: *const i32) -> bool {
    p as usize & 15 == 0
}

/// [`jane_audio::tracker::Pan`] on the VFPU: the quads there, a tail of under four frames (and
/// any slice off a 16-byte line) in integers.
pub fn pan(m: &[i32], l: &mut [i32], r: &mut [i32], s: Option<&mut [i32]>, g: (i32, i32, i32)) {
    let n = m.len().min(l.len()).min(r.len());
    let sp = s.as_ref().map(|s| s.as_ptr());
    if !(aligned(m.as_ptr()) && aligned(l.as_ptr()) && aligned(r.as_ptr()) && sp.is_none_or(aligned))
        || s.as_ref().is_some_and(|s| s.len() < n)
    {
        pan_int(m, l, r, s, g);
        return;
    }
    let quads = n / 4;
    let (gl, gr, gs) = (scale(g.0), scale(g.1), scale(g.2));
    let (mp, lp, rp) = (m.as_ptr(), l.as_mut_ptr(), r.as_mut_ptr());
    // SAFETY: the gains into the VFPU's matrix 7 (no other code here uses it); this thread has
    // the VFPU (see the module's note).
    unsafe {
        vfpu_asm!(
            "mtv {0}, S700",
            "mtv {1}, S701",
            "mtv {2}, S702",
            "nop",
            in(reg) gl,
            in(reg) gr,
            in(reg) gs,
            options(nostack, nomem),
        );
    }
    match s {
        None => {
            for q in 0..quads {
                // SAFETY: four frames at `4 * q` of each slice, inside `n`, each on a 16-byte
                // line (checked above); the gains loaded above are still in S700..S702.
                unsafe {
                    let (mq, lq, rq) = (mp.add(4 * q), lp.add(4 * q), rp.add(4 * q));
                    vfpu_asm!(
                        "lv.q C000, 0({0})",
                        "lv.q C010, 0({1})",
                        "lv.q C020, 0({2})",
                        "vi2f.q C000, C000, 0",
                        "vi2f.q C010, C010, 0",
                        "vi2f.q C020, C020, 0",
                        "vscl.q C100, C000, S700",
                        "vscl.q C110, C000, S701",
                        "vadd.q C010, C010, C100",
                        "vadd.q C020, C020, C110",
                        "vf2id.q C010, C010, 0",
                        "vf2id.q C020, C020, 0",
                        "sv.q C010, 0({1})",
                        "sv.q C020, 0({2})",
                        in(reg) mq,
                        in(reg) lq,
                        in(reg) rq,
                        options(nostack),
                    );
                }
            }
        }
        Some(s) => {
            let sq = s.as_mut_ptr();
            for q in 0..quads {
                // SAFETY: as above, with the send's slice too.
                unsafe {
                    let (mq, lq, rq, ssq) = (mp.add(4 * q), lp.add(4 * q), rp.add(4 * q), sq.add(4 * q));
                    vfpu_asm!(
                        "lv.q C000, 0({0})",
                        "lv.q C010, 0({1})",
                        "lv.q C020, 0({2})",
                        "lv.q C030, 0({3})",
                        "vi2f.q C000, C000, 0",
                        "vi2f.q C010, C010, 0",
                        "vi2f.q C020, C020, 0",
                        "vi2f.q C030, C030, 0",
                        "vscl.q C100, C000, S700",
                        "vscl.q C110, C000, S701",
                        "vscl.q C120, C000, S702",
                        "vadd.q C010, C010, C100",
                        "vadd.q C020, C020, C110",
                        "vadd.q C030, C030, C120",
                        "vf2id.q C010, C010, 0",
                        "vf2id.q C020, C020, 0",
                        "vf2id.q C030, C030, 0",
                        "sv.q C010, 0({1})",
                        "sv.q C020, 0({2})",
                        "sv.q C030, 0({3})",
                        in(reg) mq,
                        in(reg) lq,
                        in(reg) rq,
                        in(reg) ssq,
                        options(nostack),
                    );
                }
            }
            let t = 4 * quads;
            pan_int(&m[t..n], &mut l[t..n], &mut r[t..n], Some(&mut s[t..n]), g);
            return;
        }
    }
    let t = 4 * quads;
    pan_int(&m[t..n], &mut l[t..n], &mut r[t..n], None, g);
}

/// Groups a bench block pans (a busy town block sounds 6 to 10).
const GROUPS: usize = 8;
/// Blocks timed, each way.
const REPS: u32 = 2_000;

/// A fixed LCG's next value.
fn lcg(x: &mut u32) -> u32 {
    *x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *x
}

/// `REPS` mixer blocks of `GROUPS` groups panned and sent (half of them sent) by `f`, from zeroed
/// buses each block: the microseconds, and the last block's buses.
fn run(
    now: fn() -> u32,
    f: jane_audio::tracker::Pan,
    groups: &[Lane<BLOCK>],
    gains: &[(i32, i32, i32)],
) -> (u32, Vec<i32>) {
    let mut bus = [Lane([0i32; BLOCK]); 3];
    let t0 = now();
    for _ in 0..REPS {
        for b in &mut bus {
            b.fill(0);
        }
        let [bl, br, bs] = &mut bus;
        for (m, &g) in groups.iter().zip(gains) {
            let send = if g.2 == 0 { None } else { Some(&mut bs[..]) };
            f(core::hint::black_box(&m[..]), &mut bl[..], &mut br[..], send, g);
        }
    }
    let us = now().wrapping_sub(t0);
    (us, bus.iter().flat_map(|b| b.0).collect())
}

/// The bench's lines for `bench.txt` (`micro vfpu ...`): the integer pan and the VFPU's, the
/// microseconds a mixer block (64 frames, `GROUPS` groups, four of them sent), the speed-up and
/// the largest difference between their buses (in 16-bit units; the integer's is the reference).
/// Run on a thread with the VFPU (the game's).
pub fn bench(now: fn() -> u32) -> String {
    let mut seed = 0x6a09_e667u32;
    // Group buses as loud as a busy block's: a few voices of 16-bit frames, up to about 2^17.
    let groups: Vec<Lane<BLOCK>> = (0..GROUPS)
        .map(|_| {
            let mut l = Lane([0; BLOCK]);
            for x in l.iter_mut() {
                *x = (lcg(&mut seed) >> 14) as i32 - (1 << 17);
            }
            l
        })
        .collect();
    let gains: Vec<(i32, i32, i32)> = (0..GROUPS)
        .map(|k| {
            let (a, b) = ((lcg(&mut seed) >> 20) as i32, (lcg(&mut seed) >> 20) as i32);
            (a, b, if k % 2 == 0 { (lcg(&mut seed) >> 21) as i32 } else { 0 })
        })
        .collect();
    let (int_us, int_out) = run(now, pan_int, &groups, &gains);
    let (vfpu_us, vfpu_out) = run(now, pan, &groups, &gains);
    let maxdiff = int_out.iter().zip(&vfpu_out).map(|(a, b)| (a - b).unsigned_abs()).max().unwrap_or(0);
    let per = |us: u32| {
        let ns = u64::from(us) * 1000 / u64::from(REPS);
        format!("{}.{:03}", ns / 1000, ns % 1000)
    };
    let x100 = u64::from(int_us) * 100 / u64::from(vfpu_us.max(1));
    format!(
        "micro vfpu pan groups {GROUPS} frames {BLOCK} reps {REPS} int_us_block {} vfpu_us_block {} speedup {}.{:02} maxdiff {maxdiff} ok {}\n",
        per(int_us),
        per(vfpu_us),
        x100 / 100,
        x100 % 100,
        maxdiff <= 1
    )
}
