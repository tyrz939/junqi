//! THROWAWAY (PLAN-psp-streaming.md): the PSP tracker's cost by section and its voice counts,
//! replaying a PSP run's psp-cmds.txt on the host. To run: copy to crates/jane-cli/examples/, then
//! cargo run --release -p jane-cli --example mixprof -- target/psp-dist/bake/jane-psp.jau \
//!     progress/2026-10-08_59_psp-audio/iter-02-nine-storm/psp-cmds.txt
use std::sync::OnceLock;
use std::time::Instant;

use jane_audio::tracker::{self, Bank, Mixer};

static T0: OnceLock<Instant> = OnceLock::new();
fn now_ns() -> u32 {
    // nanoseconds / 16, so a block's laps fit a u32 (they are differences)
    (T0.get_or_init(Instant::now).elapsed().as_nanos() / 16) as u32
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&a[0]).unwrap();
    let text = std::fs::read_to_string(&a[1]).unwrap();
    let mut cmds = Vec::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let [f, x, y] = w[..] else { continue };
        if let Some(c) = tracker::unpack(u64::from_str_radix(x, 16).unwrap(), u64::from_str_radix(y, 16).unwrap()) {
            cmds.push((f.parse::<u64>().unwrap(), c));
        }
    }
    let mut m = Mixer::new(Bank::parse(bytes).unwrap(), 22_050, 1);
    m.clock = Some(now_ns);
    let end = cmds.last().map_or(0, |c| c.0) + 22_050 * 3;
    let mut out = vec![0i16; 1024];
    let (mut next, mut blocks) = (0, 0u64);
    let mut hist = [0u64; 81];
    let mut worst = 0u128;
    let t_all = Instant::now();
    while m.now < end {
        let mut got = Vec::new();
        while next < cmds.len() && cmds[next].0 <= m.now {
            got.push(format!("{:?}", cmds[next].1));
            m.handle(cmds[next].1);
            next += 1;
        }
        let pr = m.prof;
        let t = Instant::now();
        m.render(&mut out);
        let e = t.elapsed().as_nanos();
        if e > 150_000 && blocks > 2 {
            let d: Vec<u32> = m.prof.iter().zip(pr).map(|(a, b)| a.wrapping_sub(b) * 16 / 1000).collect();
            println!("  slow block {blocks}: {} us, laps {:?} us, voices {}, after {:?}", e / 1000, d, m.voices(), got);
        }
        worst = worst.max(e);
        hist[m.voices().min(80)] += 1;
        blocks += 1;
    }
    let total = t_all.elapsed().as_secs_f64();
    let p: Vec<f64> = m.prof.iter().map(|&v| f64::from(v) * 16.0 / 1e3 / blocks as f64).collect();
    let mean_v: f64 = hist.iter().enumerate().map(|(i, &n)| i as f64 * n as f64).sum::<f64>() / blocks as f64;
    let mut acc = 0u64;
    let p95 = hist
        .iter()
        .enumerate()
        .find(|&(_, &n)| {
            acc += n;
            acc * 100 >= blocks * 95
        })
        .map_or(0, |e| e.0);
    let maxv = hist.iter().rposition(|&n| n > 0).unwrap_or(0);
    println!("{blocks} blocks of 512 frames ({:.1} s audio) in {:.1} ms host", blocks as f64 * 512.0 / 22050.0, total * 1e3);
    println!(
        "per block us: sequence {:.1}, voices {:.1}, groups {:.1}, out(reverb+dc+limiter) {:.1}; worst block {:.1}",
        p[0],
        p[1],
        p[2],
        p[3],
        worst as f64 / 1e3
    );
    println!("voices alive per block: mean {mean_v:.1}, p95 {p95}, max {maxv}");
}
