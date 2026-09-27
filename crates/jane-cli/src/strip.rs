//! `jane sheet scene --film N[:EVERY] --strip PATH [--tiers t0,t1,t2] [--half]`: a film as one
//! contact sheet, a row a tier, a column a frame, each column headed by its tick and each row by
//! its tier: how a moment reads on every tier at once (PRESENTATION.md §6; the lesson's review,
//! §2.1).

use jane_art::font::{Face, Font};
use jane_art::sheet::{Image, label};

use crate::scene::{Opts, Shot, Which, film};

/// Films `o` on each of `tiers` and writes the strip to `path`. `half` draws each frame at half
/// size (box-filtered); `close` crops each frame first.
pub fn strip(
    bps: &jane_sim::Blueprints,
    o: &Opts,
    tiers: &[Which],
    (n, every): (u32, u32),
    half: bool,
    close: Option<((u16, u16, u16, u16), u16)>,
    path: &std::path::Path,
) -> Result<(), String> {
    let mut rows: Vec<(Which, Vec<(u32, Shot)>)> = Vec::new();
    for &which in tiers {
        let o = Opts { backend: which, ..o.clone() };
        let mut frames = Vec::new();
        film(bps.clone(), &o, n, every, |k, shot| {
            let shot = close.map_or_else(|| shot.crop((0, 0, shot.w, shot.h), 1), |(r, z)| shot.crop(r, z));
            frames.push((k, if half { halve(&shot) } else { shot }));
            Ok(())
        })?;
        rows.push((which, frames));
    }
    let (fw, fh) = rows.iter().find_map(|(_, f)| f.first()).map_or((1, 1), |(_, s)| (u32::from(s.w), u32::from(s.h)));
    let cols = rows.iter().map(|(_, f)| f.len()).max().unwrap_or(0) as u32;
    let (left, top, gap) = (44u32, 22u32, 4u32);
    let w = left + cols * (fw + gap);
    let h = top + rows.len() as u32 * (fh + gap);
    let mut img = Image::new(w, h, [24, 22, 30, 255]);
    let font = Font::build();
    let ink = [220, 212, 196];
    if let Some((_, frames)) = rows.first() {
        for (c, (k, _)) in frames.iter().enumerate() {
            let x = left + c as u32 * (fw + gap);
            let secs = format!("+{k} ({}.{} s)", k / 60, k % 60 * 10 / 60);
            label(&mut img, &font, x + 2, 5, &secs, Face::Fine, ink);
        }
    }
    for (r, (which, frames)) in rows.iter().enumerate() {
        let y = top + r as u32 * (fh + gap);
        label(&mut img, &font, 6, y + fh / 2 - 6, &which.name().to_uppercase(), Face::Fine, ink);
        for (c, (_, s)) in frames.iter().enumerate() {
            let x = left + c as u32 * (fw + gap);
            for sy in 0..u32::from(s.h).min(fh) {
                for sx in 0..u32::from(s.w).min(fw) {
                    let p = s.px[(sy * u32::from(s.w) + sx) as usize];
                    img.set(x + sx, y + sy, [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
                }
            }
        }
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, img.png()).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{} ({} tiers, {cols} frames)", path.display(), rows.len());
    Ok(())
}

/// A frame at half size, each px the mean of four.
fn halve(s: &Shot) -> Shot {
    let (w, h) = (s.w / 2, s.h / 2);
    let mut px = Vec::with_capacity(usize::from(w) * usize::from(h));
    let at = |x: u16, y: u16| s.px[usize::from(y) * usize::from(s.w) + usize::from(x)];
    for y in 0..h {
        for x in 0..w {
            let q = [at(2 * x, 2 * y), at(2 * x + 1, 2 * y), at(2 * x, 2 * y + 1), at(2 * x + 1, 2 * y + 1)];
            let ch = |sh: u32| q.iter().map(|c| (c >> sh) & 0xff).sum::<u32>() / 4;
            px.push(0xff00_0000 | ch(16) << 16 | ch(8) << 8 | ch(0));
        }
    }
    Shot { w, h, px, line: String::new(), layers: None }
}
