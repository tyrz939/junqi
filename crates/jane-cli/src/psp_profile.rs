//! `jane psp-profile`: read a PSP detailed capture (PORT.md §13.13, `jane_render_psp::capture`)
//! and print where each frame's time went, with an HTML report of every frame beside it.

#![allow(clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use jane_render_psp::capture::Capture;

pub const USAGE: &str = "  psp-profile <capture.bin> [--out DIR]
                                      summarise a PSP detailed capture (L+R+START) and write
                                      <stem>-profile.html beside it (or into DIR)";

/// The CPU's parts of a frame, in stacking order (the chart's and the worst frames').
const CPU: [&str; 9] = ["sim", "tick", "bufs", "draw", "ui", "list", "ge_build", "ge_wait", "vblank"];
/// The counts the worst frames name.
const COUNTS: [&str; 8] =
    ["n_quads", "n_lights", "n_casters", "n_painted", "n_landed", "n_page_loads", "n_uploads", "n_ticks"];
/// How many GE passes get their own colour; the rest stack as "other".
const GE_SHOWN: usize = 7;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Time,
    Count,
    Bytes,
}

fn kind(name: &str) -> Kind {
    if name.starts_with("n_") || name == "late" {
        Kind::Count
    } else if name == "free_bytes" || name == "largest_bytes" {
        Kind::Bytes
    } else {
        Kind::Time
    }
}

fn unit(k: Kind) -> &'static str {
    match k {
        Kind::Time => "ms",
        Kind::Count => "",
        Kind::Bytes => "KB",
    }
}

/// A value in its unit: ms to 2 places, counts plain (a mean to 1), KB whole.
fn show(k: Kind, v: f64, mean: bool) -> String {
    match k {
        Kind::Time => format!("{:.2}", v / 1000.0),
        Kind::Count if mean => format!("{v:.1}"),
        Kind::Count => format!("{v:.0}"),
        Kind::Bytes => format!("{:.0}", v / 1024.0),
    }
}

#[derive(Clone, Copy, Default)]
struct Stat {
    mean: f64,
    p50: u32,
    p95: u32,
    max: u32,
}

fn stat(col: impl Iterator<Item = u32>) -> Stat {
    let mut v: Vec<u32> = col.collect();
    if v.is_empty() {
        return Stat::default();
    }
    v.sort_unstable();
    let at = |p: usize| v[(v.len() - 1) * p / 100];
    let sum: u64 = v.iter().map(|&x| u64::from(x)).sum();
    Stat { mean: sum as f64 / v.len() as f64, p50: at(50), p95: at(95), max: v[v.len() - 1] }
}

/// A capture read, with each word's stats (fields, then passes).
struct Profile<'a> {
    cap: &'a Capture,
    stats: Vec<Stat>,
}

impl<'a> Profile<'a> {
    fn new(cap: &'a Capture) -> Profile<'a> {
        let words = cap.fields.len() + cap.passes.len();
        let stats = (0..words).map(|w| stat(cap.frames.iter().map(|f| f[w]))).collect();
        Profile { cap, stats }
    }

    fn word(&self, frame: usize, w: Option<usize>) -> u32 {
        w.map_or(0, |w| self.cap.frames[frame][w])
    }

    fn pass(&self, p: usize) -> usize {
        self.cap.fields.len() + p
    }

    /// The passes by mean, longest first.
    fn passes_by_mean(&self) -> Vec<usize> {
        let mut p: Vec<usize> = (0..self.cap.passes.len()).collect();
        p.sort_by(|&a, &b| self.stats[self.pass(b)].mean.total_cmp(&self.stats[self.pass(a)].mean));
        p
    }

    /// The share a pass's mean is of the GE's (ge_total, or the passes' sum without it).
    fn ge_mean(&self) -> f64 {
        match self.cap.field("ge_total").map(|w| self.stats[w].mean) {
            Some(m) if m > 0.0 => m,
            _ => (0..self.cap.passes.len()).map(|p| self.stats[self.pass(p)].mean).sum::<f64>().max(1.0),
        }
    }

    fn worst(&self, n: usize) -> Vec<usize> {
        let fw = self.cap.field("frame");
        let mut f: Vec<usize> = (0..self.cap.frames.len()).collect();
        f.sort_by_key(|&i| std::cmp::Reverse(self.word(i, fw)));
        f.truncate(n);
        f
    }

    fn headline(&self) -> (usize, f64, f64) {
        let n = self.cap.frames.len();
        let mean = self.cap.field("frame").map_or(0.0, |w| self.stats[w].mean);
        let fps = if mean > 0.0 { 1e6 / mean } else { 0.0 };
        let lw = self.cap.field("late");
        let late = (0..n).filter(|&i| self.word(i, lw) > 0).count();
        (n, fps, late as f64 * 100.0 / n.max(1) as f64)
    }

    /// The top `k` of `names` (fields) or of the passes in frame `i`, longest first.
    fn top(&self, i: usize, words: &[(String, usize)], k: usize) -> String {
        let mut v: Vec<(&str, u32)> = words.iter().map(|(n, w)| (n.as_str(), self.cap.frames[i][*w])).collect();
        v.sort_by_key(|&(_, x)| std::cmp::Reverse(x));
        v.iter()
            .take(k)
            .filter(|&&(_, x)| x > 0)
            .map(|(n, x)| format!("{n} {:.2}", f64::from(*x) / 1000.0))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn summary(&self) -> String {
        let c = self.cap;
        let mut s = String::new();
        let _ = writeln!(s, "capture v{}", c.version);
        for (k, v) in &c.header {
            let _ = writeln!(s, "  {k} {v}");
        }
        let (n, fps, late) = self.headline();
        let _ = writeln!(s, "frames {n}, mean {fps:.1} fps, late {late:.1}%\n");
        let row = |s: &mut String, name: &str, w: usize| {
            let k = kind(name);
            let st = self.stats[w];
            let _ = write!(
                s,
                "  {name:<14} {:>3} {:>9} {:>9} {:>9} {:>9}",
                unit(k),
                show(k, st.mean, true),
                show(k, f64::from(st.p50), false),
                show(k, f64::from(st.p95), false),
                show(k, f64::from(st.max), false)
            );
        };
        let _ = writeln!(s, "  {:<14} {:>3} {:>9} {:>9} {:>9} {:>9}", "field", "", "mean", "p50", "p95", "max");
        for (w, name) in c.fields.iter().enumerate() {
            row(&mut s, name, w);
            s.push('\n');
        }
        let ge = self.ge_mean();
        let _ = writeln!(
            s,
            "\n  {:<14} {:>3} {:>9} {:>9} {:>9} {:>9} {:>7}",
            "GE pass", "", "mean", "p50", "p95", "max", "share"
        );
        for p in self.passes_by_mean() {
            row(&mut s, &c.passes[p], self.pass(p));
            let _ = writeln!(s, " {:>6.1}%", self.stats[self.pass(p)].mean * 100.0 / ge);
        }
        let cpu: Vec<(String, usize)> =
            CPU.iter().chain(["audio"].iter()).filter_map(|n| c.field(n).map(|w| ((*n).to_string(), w))).collect();
        let gp: Vec<(String, usize)> = c.passes.iter().enumerate().map(|(p, n)| (n.clone(), self.pass(p))).collect();
        let _ = writeln!(s, "\nworst frames (ms)");
        for i in self.worst(10) {
            let _ = writeln!(
                s,
                "  #{i:<5} {:>7.2}  cpu: {}",
                f64::from(self.word(i, c.field("frame"))) / 1000.0,
                self.top(i, &cpu, 5)
            );
            let _ = writeln!(s, "                  ge: {}", self.top(i, &gp, 5));
            let counts: Vec<String> =
                COUNTS.iter().filter_map(|n| c.field(n).map(|w| format!("{n} {}", c.frames[i][w]))).collect();
            let _ = writeln!(s, "                  {}", counts.join("  "));
        }
        s
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A stacked step-area chart of `series` (name, class, ms per frame), with a hover target per frame.
fn chart(series: &[(String, String, Vec<f64>)], guides: bool) -> String {
    const W: f64 = 1000.0;
    const H: f64 = 280.0;
    const L: f64 = 44.0;
    const R: f64 = 8.0;
    const T: f64 = 10.0;
    const B: f64 = 22.0;
    let n = series.first().map_or(0, |s| s.2.len()).max(1);
    let totals: Vec<f64> = (0..n).map(|i| series.iter().map(|s| s.2.get(i).copied().unwrap_or(0.0)).sum()).collect();
    let mut sorted = totals.clone();
    sorted.sort_by(f64::total_cmp);
    let (p95, max) = (sorted[(n - 1) * 95 / 100], sorted[n - 1]);
    // Room for the 30 fps line, not more than a few spikes need: they clip.
    let top = max.min(p95 * 3.0).max(if guides { 36.0 } else { 4.0 });
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0].into_iter().find(|s| top / s <= 6.0).unwrap_or(200.0);
    let top = (top / step).ceil() * step;
    let x = |i: usize| L + i as f64 * (W - L - R) / n as f64;
    let y = |v: f64| T + (H - T - B) * (1.0 - (v / top).min(1.0));
    let mut s = format!(
        "<svg viewBox=\"0 0 {W} {H}\" role=\"img\"><defs><clipPath id=\"c\"><rect x=\"{L}\" y=\"{T}\" width=\"{}\" height=\"{}\"/></clipPath></defs>",
        W - L - R,
        H - T - B
    );
    let mut v = 0.0;
    while v <= top + 1e-9 {
        let _ = write!(
            s,
            "<line class=\"grid\" x1=\"{L}\" x2=\"{x2}\" y1=\"{yy:.1}\" y2=\"{yy:.1}\"/><text class=\"ax\" x=\"{tx}\" y=\"{ty:.1}\" text-anchor=\"end\">{v}</text>",
            x2 = W - R,
            yy = y(v),
            tx = L - 6.0,
            ty = y(v) + 4.0
        );
        v += step;
    }
    let _ = write!(
        s,
        "<text class=\"ax\" x=\"{L}\" y=\"{}\">frame 0</text><text class=\"ax\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{n}</text>",
        H - 6.0,
        W - R,
        H - 6.0
    );
    let mut base = vec![0.0; n];
    s.push_str("<g clip-path=\"url(#c)\">");
    for (_, class, vals) in series {
        let mut up = Vec::with_capacity(n * 2);
        let mut down = Vec::with_capacity(n * 2);
        for (i, slot) in base.iter_mut().enumerate() {
            let b = *slot;
            let t = b + vals.get(i).copied().unwrap_or(0.0);
            up.push(format!("{:.1},{:.1} {:.1},{:.1}", x(i), y(t), x(i + 1), y(t)));
            down.push(format!("{:.1},{:.1} {:.1},{:.1}", x(i + 1), y(b), x(i), y(b)));
            *slot = t;
        }
        down.reverse();
        let _ = write!(s, "<polygon class=\"{class}\" points=\"{} {}\"/>", up.join(" "), down.join(" "));
    }
    s.push_str("</g>");
    if guides {
        for (ms, label) in [(16.7, "16.7 ms · 60 fps"), (33.3, "33.3 ms · 30 fps")] {
            if ms <= top {
                let _ = write!(
                    s,
                    "<line class=\"guide\" x1=\"{L}\" x2=\"{x2}\" y1=\"{yy:.1}\" y2=\"{yy:.1}\"/><text class=\"gl\" x=\"{tx}\" y=\"{ty:.1}\" text-anchor=\"end\">{label}</text>",
                    x2 = W - R,
                    yy = y(ms),
                    tx = W - R - 4.0,
                    ty = y(ms) - 4.0
                );
            }
        }
    }
    for (i, total) in totals.iter().enumerate() {
        let mut tip = format!("frame {i}: {total:.2} ms");
        for (name, _, vals) in series.iter().rev() {
            let _ = write!(tip, "\n{name} {:.2}", vals.get(i).copied().unwrap_or(0.0));
        }
        let _ = write!(
            s,
            "<rect class=\"hit\" x=\"{:.1}\" y=\"{T}\" width=\"{:.2}\" height=\"{}\"><title>{}</title></rect>",
            x(i),
            x(i + 1) - x(i),
            H - T - B,
            esc(&tip)
        );
    }
    s.push_str("</svg>");
    let legend = series.iter().fold(String::new(), |mut l, (name, class, _)| {
        let _ = write!(l, "<span><i class=\"{class}\"></i>{}</span>", esc(name));
        l
    });
    format!("<div class=\"legend\">{legend}</div>{s}")
}

fn html(p: &Profile<'_>, title: &str) -> String {
    let c = p.cap;
    let ms = |w: usize| -> Vec<f64> { c.frames.iter().map(|f| f64::from(f[w]) / 1000.0).collect() };
    let cpu: Vec<(String, String, Vec<f64>)> = CPU
        .iter()
        .filter_map(|n| c.field(n))
        .enumerate()
        .map(|(k, w)| {
            (c.fields[w].clone(), if c.fields[w] == "vblank" { "c0".into() } else { format!("c{}", k % 8 + 1) }, ms(w))
        })
        .collect();
    let order = p.passes_by_mean();
    let mut ge: Vec<(String, String, Vec<f64>)> = order
        .iter()
        .take(GE_SHOWN)
        .enumerate()
        .map(|(k, &q)| (c.passes[q].clone(), format!("c{}", k + 1), ms(p.pass(q))))
        .collect();
    if order.len() > GE_SHOWN {
        let rest: Vec<f64> = (0..c.frames.len())
            .map(|i| order[GE_SHOWN..].iter().map(|&q| f64::from(c.frames[i][p.pass(q)]) / 1000.0).sum())
            .collect();
        ge.push(("other passes".into(), "c0".into(), rest));
    }
    let (n, fps, late) = p.headline();
    let p95 = c.field("frame").map_or(0.0, |w| f64::from(p.stats[w].p95) / 1000.0);
    let head = c.header.iter().fold(String::new(), |mut h, (k, v)| {
        let _ = write!(h, "<dt>{}</dt><dd>{}</dd>", esc(k), esc(v));
        h
    });
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>PSP Frame Profile</title><style>
:root{{--bg:#f9f9f7;--card:#fcfcfb;--ink:#0b0b0b;--ink2:#52514e;--mute:#898781;--grid:#e1e0d9;--axis:#c3c2b7;--s1:#2a78d6;--s2:#eb6834;--s3:#1baf7a;--s4:#eda100;--s5:#e87ba4;--s6:#008300;--s7:#4a3aa7;--s8:#e34948;--s0:#c3c2b7;--guide:#52514e}}
@media (prefers-color-scheme:dark){{:root{{--bg:#0d0d0d;--card:#1a1a19;--ink:#fff;--ink2:#c3c2b7;--grid:#2c2c2a;--axis:#383835;--s1:#3987e5;--s2:#d95926;--s3:#199e70;--s4:#c98500;--s5:#d55181;--s7:#9085e9;--s8:#e66767;--s0:#52514e;--guide:#c3c2b7}}}}
body{{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 system-ui,sans-serif}}main{{max-width:1080px;margin:0 auto;padding:24px 16px}}
h1{{font-size:22px;margin:0 0 4px}}h2{{font-size:16px;margin:28px 0 8px}}.sub{{color:var(--ink2);margin:0 0 16px}}
.tiles{{display:flex;gap:12px;flex-wrap:wrap}}.tile{{background:var(--card);border:1px solid var(--grid);border-radius:8px;padding:10px 14px;min-width:120px}}.tile b{{display:block;font-size:22px}}.tile span{{color:var(--ink2);font-size:12px}}
.card{{background:var(--card);border:1px solid var(--grid);border-radius:8px;padding:12px;overflow-x:auto}}svg{{width:100%;height:auto;display:block}}
.grid{{stroke:var(--grid)}}.ax{{fill:var(--mute);font-size:11px}}.guide{{stroke:var(--guide);stroke-dasharray:4 3}}.gl{{fill:var(--guide);font-size:11px}}
.hit{{fill:transparent}}.hit:hover{{fill:var(--ink);fill-opacity:.12}}
.c0{{fill:var(--s0);background:var(--s0)}}.c1{{fill:var(--s1);background:var(--s1)}}.c2{{fill:var(--s2);background:var(--s2)}}.c3{{fill:var(--s3);background:var(--s3)}}.c4{{fill:var(--s4);background:var(--s4)}}.c5{{fill:var(--s5);background:var(--s5)}}.c6{{fill:var(--s6);background:var(--s6)}}.c7{{fill:var(--s7);background:var(--s7)}}.c8{{fill:var(--s8);background:var(--s8)}}
.legend{{display:flex;flex-wrap:wrap;gap:4px 14px;color:var(--ink2);font-size:12px;margin-bottom:6px}}.legend i{{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:5px;vertical-align:-1px}}
dl{{display:grid;grid-template-columns:max-content 1fr;gap:2px 12px;margin:0;font-size:13px}}dt{{color:var(--ink2)}}dd{{margin:0;font-family:ui-monospace,monospace}}
pre{{font:12px/1.4 ui-monospace,Consolas,monospace;margin:0;white-space:pre}}
</style></head><body><main><h1>PSP frame profile</h1><p class="sub">{title}</p>
<div class="tiles"><div class="tile"><b>{n}</b><span>frames</span></div><div class="tile"><b>{fps:.1}</b><span>mean fps</span></div><div class="tile"><b>{p95:.2} ms</b><span>p95 frame</span></div><div class="tile"><b>{late:.1}%</b><span>late frames</span></div></div>
<h2>CPU per frame (ms)</h2><div class="card">{cpu}</div>
<h2>GE passes per frame (ms)</h2><div class="card">{ge}</div>
<h2>Capture</h2><div class="card"><dl>{head}</dl></div>
<h2>Summary</h2><div class="card"><pre>{sum}</pre></div>
</main></body></html>
"#,
        title = esc(title),
        cpu = chart(&cpu, true),
        ge = chart(&ge, false),
        sum = esc(&p.summary()),
    )
}

/// Read `path`, return the summary and the report written into `out` (default: beside it).
pub fn profile(path: &Path, out: Option<&Path>) -> Result<(String, PathBuf), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let cap = Capture::decode(&bytes).map_err(|e| format!("{}: {}", path.display(), e.0))?;
    if cap.frames.is_empty() {
        return Err(format!("{}: no frames", path.display()));
    }
    let p = Profile::new(&cap);
    let dir = out.map_or_else(|| path.parent().map_or_else(PathBuf::new, Path::to_path_buf), Path::to_path_buf);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stem = path.file_stem().map_or_else(|| "capture".into(), |s| s.to_string_lossy().into_owned());
    let report = dir.join(format!("{stem}-profile.html"));
    let name = path.file_name().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    std::fs::write(&report, html(&p, &name)).map_err(|e| format!("{}: {e}", report.display()))?;
    Ok((p.summary(), report))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let (mut file, mut out) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a directory")?)),
            f if !f.starts_with('-') && file.is_none() => file = Some(PathBuf::from(f)),
            other => return Err(format!("unknown argument \"{other}\"\n{USAGE}")),
        }
    }
    let file = file.ok_or_else(|| format!("which capture?\n{USAGE}"))?;
    let (text, report) = profile(&file, out.as_deref())?;
    print!("{text}");
    println!("\nreport {}", report.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_render_psp::capture::{Recorder, field, pass};

    #[test]
    fn a_synthetic_capture_profiles() {
        let mut r = Recorder::new("build test\nzone county\n".into(), 120).unwrap();
        for k in 0..120u32 {
            let mut f = [0u32; field::N];
            let mut p = [0u32; pass::N];
            f[field::SIM] = 2_000 + k * 10;
            f[field::DRAW] = 6_000 + (k % 7) * 500;
            f[field::GE_WAIT] = if k == 77 { 20_000 } else { 3_000 };
            f[field::VBLANK] = 2_000;
            f[field::FRAME] = f[field::SIM] + f[field::DRAW] + f[field::GE_WAIT] + f[field::VBLANK];
            f[field::LATE] = u32::from(f[field::FRAME] > 16_667);
            f[field::QUADS] = 1_500 + k;
            f[field::FREE] = 4 << 20;
            for (q, w) in p.iter_mut().enumerate() {
                *w = 200 + q as u32 * 50;
            }
            p[usize::from(pass::TERRAIN)] = 4_000;
            f[field::GE_TOTAL] = p.iter().sum();
            assert!(r.push(&f, &p));
        }
        let dir = std::env::temp_dir().join(format!("jane-psp-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("capture-3.bin");
        std::fs::write(&bin, r.encode()).unwrap();
        let (text, report) = profile(&bin, None).unwrap();
        for n in pass::NAMES {
            assert!(text.contains(n), "the summary names pass {n}");
        }
        assert!(text.contains("frames 120"));
        assert!(text.contains("#77 "), "the spiked frame is among the worst:\n{text}");
        assert!(text.contains("build test"));
        assert_eq!(report, dir.join("capture-3-profile.html"));
        let page = std::fs::read_to_string(&report).unwrap();
        assert!(page.contains("<svg") && page.contains("terrain") && page.contains("prefers-color-scheme"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
