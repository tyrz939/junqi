//! `sheet scene --layers` (PRESENTATION.md §1.7): a frame's height layer and the T2 height field,
//! composed on the CPU as the G-buffer and `scatter.wgsl` compose them, as false-colour PNGs (grey
//! by height, a band of red every 8 px so a step shows, blue where nothing stands).

use jane_present::frame::CHUNK_PX;
use jane_present::{AtlasPages, Frame, Pass, Tint};

/// `(height, depth)` per canvas px as the G-buffer holds them.
pub fn heights(frame: &Frame, atlas: &AtlasPages) -> (Vec<u8>, Vec<u8>, usize, usize) {
    let (w, h) = (usize::from(frame.canvas.0), usize::from(frame.canvas.1));
    let mut hv = vec![0u8; w * h];
    let mut dv = vec![0u8; w * h];
    let mut depth = vec![2u8; frame.sprites.len()];
    for c in &frame.casters {
        if let Some(d) = depth.get_mut(c.sprite as usize) {
            *d = c.depth.max(1);
        }
    }
    for pass in &frame.passes {
        match *pass {
            Pass::Terrain { chunks } => {
                for c in frame.chunks_in(chunks) {
                    let l = frame.layers_of(c);
                    if !l.lit() {
                        continue;
                    }
                    for y in 0..CHUNK_PX {
                        for x in 0..CHUNK_PX {
                            let (cx, cy) = (c.x + x, c.y + y);
                            if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 {
                                continue;
                            }
                            let i = cy as usize * w + cx as usize;
                            hv[i] = l.height[(y * CHUNK_PX + x) as usize];
                            dv[i] = 2;
                        }
                    }
                }
            }
            Pass::Sprites { cmds, .. } => {
                for (k, s) in frame.sprites_in(cmds).iter().enumerate() {
                    if matches!(s.flags.tint, Tint::Ghost(_)) {
                        continue;
                    }
                    let Some(p) = atlas.pages.get(usize::from(s.page)) else { continue };
                    if !p.lit() {
                        continue;
                    }
                    for v in 0..i32::from(s.src.h) {
                        for u in 0..i32::from(s.src.w) {
                            let su = if s.flags.mirror { i32::from(s.src.w) - 1 - u } else { u };
                            let pi = (usize::from(s.src.y) + v as usize) * usize::from(p.w)
                                + usize::from(s.src.x)
                                + su as usize;
                            if p.albedo[pi] <= 1 {
                                continue;
                            }
                            let (cx, cy) = (i32::from(s.x) + u, i32::from(s.y) + v);
                            if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 {
                                continue;
                            }
                            let i = cy as usize * w + cx as usize;
                            hv[i] = p.height[pi];
                            dv[i] = depth[cmds.start as usize + k];
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (hv, dv, w, h)
}

/// The scatter as `scatter.wgsl` does it: each lifted px raised on `(x, y + rows_up(h))`, as deep
/// as its depth and a px wider each side, the tallest kept.
pub fn field(hv: &[u8], dv: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut f = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let z = hv[y * w + x];
            if i32::from(z) <= jane_present::shadow::GROUND {
                continue;
            }
            let d = i32::from(dv[y * w + x].max(1));
            let y0 = y as i32 + jane_present::rows_up(i32::from(z)) - d / 2;
            for dx in -1..=1 {
                let xx = x as i32 + dx;
                if xx < 0 || xx >= w as i32 {
                    continue;
                }
                for k in 0..d {
                    let yy = y0 + k;
                    if yy >= 0 && yy < h as i32 {
                        let j = yy as usize * w + xx as usize;
                        f[j] = f[j].max(z);
                    }
                }
            }
        }
    }
    f
}

/// Grey, a false colour by value so steps show.
pub fn png(v: &[u8], w: usize, h: usize) -> Vec<u8> {
    let rgba: Vec<u8> = v
        .iter()
        .flat_map(|&z| {
            let g = (u32::from(z) * 4).min(255) as u8;
            let band = if z % 8 < 4 { 0 } else { 40 };
            [g, g.saturating_sub(band), if z == 0 { 60 } else { g }, 255]
        })
        .collect();
    jane_art::sheet::png(w as u32, h as u32, &rgba)
}
