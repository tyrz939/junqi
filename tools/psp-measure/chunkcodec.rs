//! THROWAWAY (PLAN-psp-streaming.md): dump a sample of console-form county chunks (every 3rd chunk
//! each way: 1,024) for codecs.py, and try a domain coder on them: each px coded as which of its
//! distinct neighbours (left, up, up-right, up-left) it equals, else a literal, through static
//! rANS models (32 contexts from the neighbours' equalities), trained on half, tested on the rest.
//! To run: copy to crates/jane-cli/examples/, then
//! cargo run --release -p jane-cli --example chunkcodec -- OUT_DIR 3
//! Results (2026-10-18): idx 5.13 KB, height 0.50 KB a chunk; host decode ~0.8 ms (too slow).
use std::io::Write;
use std::time::Instant;

use jane_art::terrain::{self, CHUNK_CELLS, CHUNK_PX, Chunk, Painter, TileMap, TileSource};

const SIDE: usize = CHUNK_PX as usize;
const N: usize = SIDE * SIDE;
const OUT: u16 = 256;
const CTX: usize = 32;
const ESC: usize = 4;
const PB: u32 = 12;
const PT: u32 = 1 << PB;

struct Ck {
    clut: Vec<u32>,
    idx: Vec<u8>,
    hgt: Vec<u8>,
}

struct Model {
    f: Vec<u32>,
    c: Vec<u32>,
    lut: Vec<u16>,
}

impl Model {
    fn from_counts(counts: &[u64]) -> Model {
        let n = counts.len();
        let tot: u64 = counts.iter().map(|&c| c + 1).sum();
        let mut f: Vec<u32> = counts.iter().map(|&c| (((c + 1) * u64::from(PT)) / tot).max(1) as u32).collect();
        let mut s: i64 = f.iter().map(|&v| i64::from(v)).sum();
        while s != i64::from(PT) {
            let i = (0..n).max_by_key(|&i| f[i]).unwrap();
            if s > i64::from(PT) {
                let d = (s - i64::from(PT)).min(i64::from(f[i]) - 1);
                f[i] -= d as u32;
                s -= d;
            } else {
                f[i] += (i64::from(PT) - s) as u32;
                s = i64::from(PT);
            }
        }
        let mut c = vec![0u32; n + 1];
        for i in 0..n {
            c[i + 1] = c[i] + f[i];
        }
        let mut lut = vec![0u16; PT as usize];
        for i in 0..n {
            for k in c[i]..c[i + 1] {
                lut[k as usize] = i as u16;
            }
        }
        Model { f, c, lut }
    }
}

struct Models {
    sym: Vec<Model>,
    lit: Model,
}

const W: usize = SIDE + 3;

#[inline]
fn ctx_and_cands(pad: &[u16], k: usize) -> (usize, [u16; 4]) {
    let (l, u, ur, ul, ll) = (pad[k - 1], pad[k - W], pad[k - W + 1], pad[k - W - 1], pad[k - 2]);
    let ctx = usize::from(l == u)
        | usize::from(u == ur) << 1
        | usize::from(l == ul) << 2
        | usize::from(ul == u) << 3
        | usize::from(l == ll) << 4;
    let mut c = [OUT; 4];
    let mut n = 0;
    for v in [l, u, ur, ul] {
        if v != OUT && !c[..n].contains(&v) {
            c[n] = v;
            n += 1;
        }
    }
    (ctx, c)
}

fn padded(p: &[u8], pad: &mut [u16]) {
    pad.fill(OUT);
    for y in 0..SIDE {
        for x in 0..SIDE {
            pad[(y + 1) * W + 2 + x] = u16::from(p[y * SIDE + x]);
        }
    }
}

/// (model, symbol) pairs, model CTX = the literal model.
fn symbols(p: &[u8], pad: &mut [u16], out: &mut Vec<(u16, u16)>) {
    padded(p, pad);
    out.clear();
    for y in 0..SIDE {
        for x in 0..SIDE {
            let k = (y + 1) * W + 2 + x;
            let (ctx, c) = ctx_and_cands(pad, k);
            match c.iter().position(|&e| e == pad[k]) {
                Some(s) => out.push((ctx as u16, s as u16)),
                None => {
                    out.push((ctx as u16, ESC as u16));
                    out.push((CTX as u16, pad[k]));
                }
            }
        }
    }
}

fn encode(syms: &[(u16, u16)], m: &Models) -> Vec<u8> {
    let mut x: u32 = 1 << 23;
    let mut out = Vec::new();
    for &(mi, s) in syms.iter().rev() {
        let md = if usize::from(mi) == CTX { &m.lit } else { &m.sym[usize::from(mi)] };
        let (f, c) = (md.f[usize::from(s)], md.c[usize::from(s)]);
        let xmax = ((1u32 << 23) >> PB << 8) * f;
        while x >= xmax {
            out.push(x as u8);
            x >>= 8;
        }
        x = (x / f << PB) + x % f + c;
    }
    out.extend_from_slice(&x.to_le_bytes());
    out.reverse();
    out
}

fn decode(buf: &[u8], m: &Models, p: &mut [u8], pad: &mut [u16]) {
    pad.fill(OUT);
    let mut pos = 4;
    let mut x = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let mut get = |md: &Model, x: &mut u32| -> usize {
        let slot = *x & (PT - 1);
        let s = usize::from(md.lut[slot as usize]);
        *x = md.f[s] * (*x >> PB) + slot - md.c[s];
        while *x < (1 << 23) {
            *x = *x << 8 | u32::from(buf[pos]);
            pos += 1;
        }
        s
    };
    for y in 0..SIDE {
        for xx in 0..SIDE {
            let k = (y + 1) * W + 2 + xx;
            let (ctx, c) = ctx_and_cands(pad, k);
            let s = get(&m.sym[ctx], &mut x);
            let v = if s == ESC { get(&m.lit, &mut x) as u16 } else { c[s] };
            pad[k] = v;
            p[y * SIDE + xx] = v as u8;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = std::path::PathBuf::from(&args[0]);
    let step: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let bytes = std::fs::read("assets/world/county.bp").expect("county.bp");
    let bp = jane_sim::baked::decode(&bytes, jane_core::ZoneId::County, false).expect("decode");
    let seed = 0x5eed_u32;
    let map = TileMap::from_blueprint_seeded(&bp, seed);
    let (w, h) = map.size();
    let mut p = Painter::new();
    p.drop_normals();
    let mut chunk = Chunk::new();
    chunk.drop_normals();
    let mut cks = Vec::new();
    for ky in (1..h / CHUNK_CELLS).step_by(step) {
        for kx in (1..w / CHUNK_CELLS).step_by(step) {
            terrain::paint_chunk(&mut p, &map, seed, kx, ky, &mut chunk);
            let l = &chunk.layers;
            // The palette sorted by frequency, the commonest first.
            let mut cnt: std::collections::HashMap<u32, u32> = Default::default();
            for &c in &l.albedo {
                *cnt.entry(c).or_insert(0) += 1;
            }
            let mut pal: Vec<(u32, u32)> = cnt.into_iter().collect();
            pal.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let clut: Vec<u32> = pal.iter().map(|e| e.0).collect();
            let ix: std::collections::HashMap<u32, u8> = clut.iter().enumerate().map(|(i, &c)| (c, i as u8)).collect();
            let idx: Vec<u8> = l.albedo.iter().map(|c| ix[c]).collect();
            let mut hgt = l.height.clone();
            jane_present::terrain::relative(&mut hgt, &chunk.bases);
            cks.push(Ck { clut, idx, hgt });
        }
    }
    println!("{} sample chunks (every {step}th)", cks.len());
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut f = std::io::BufWriter::new(std::fs::File::create(out_dir.join("chunks.bin")).unwrap());
    for c in &cks {
        f.write_all(&(c.clut.len() as u16).to_le_bytes()).unwrap();
        for &e in &c.clut {
            f.write_all(&e.to_le_bytes()).unwrap();
        }
        f.write_all(&c.idx).unwrap();
        f.write_all(&c.hgt).unwrap();
    }
    drop(f);
    let mut pad = vec![OUT; W * (SIDE + 1)];
    let mut syms = Vec::new();
    for (plane, name) in [(0, "idx"), (1, "hgt")] {
        let get = |c: &Ck| if plane == 0 { c.idx.clone() } else { c.hgt.clone() };
        let (mut cs, mut cl) = (vec![vec![0u64; 5]; CTX], vec![0u64; 256]);
        for c in cks.iter().step_by(2) {
            symbols(&get(c), &mut pad, &mut syms);
            for &(m, s) in &syms {
                if usize::from(m) == CTX {
                    cl[usize::from(s)] += 1;
                } else {
                    cs[usize::from(m)][usize::from(s)] += 1;
                }
            }
        }
        let models = Models { sym: cs.iter().map(|c| Model::from_counts(c)).collect(), lit: Model::from_counts(&cl) };
        let (mut total, mut n, mut dec_ns) = (0usize, 0usize, 0u128);
        let mut back = vec![0u8; N];
        for c in cks.iter().skip(1).step_by(2) {
            let pl = get(c);
            symbols(&pl, &mut pad, &mut syms);
            let enc = encode(&syms, &models);
            let t = Instant::now();
            decode(&enc, &models, &mut back, &mut pad);
            dec_ns += t.elapsed().as_nanos();
            assert_eq!(back, pl, "round trip");
            total += enc.len();
            n += 1;
        }
        println!(
            "{name}: neighbour-match + static rANS: {n} test chunks, mean {:.2} KB, host decode mean {:.0} us",
            total as f64 / n as f64 / 1024.0,
            dec_ns as f64 / n as f64 / 1000.0
        );
    }
}
