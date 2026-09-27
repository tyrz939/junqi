//! `sheet scene --layers` (PRESENTATION.md §1.7): a frame's height layer and the T2 height field,
//! composed on the CPU as the G-buffer and `scatter.wgsl` compose them, as false-colour PNGs (grey
//! by height, a band of red every 8 px so a step shows, blue where nothing stands; in the field,
//! green where what stands there floats: its bottom is off the ground).

use jane_present::frame::CHUNK_PX;
use jane_present::{AtlasPages, Frame, Pass, Tint};

/// A frame's G-buffer as the field is built from it: per canvas px its height, its depth and
/// whose it is (a sprite's index in the frame plus one; 0 the terrain).
pub struct Heights {
    pub h: Vec<u8>,
    pub depth: Vec<u8>,
    pub id: Vec<u16>,
    pub w: usize,
    pub rows: usize,
}

/// The G-buffer's heights, depths and ids for `frame`.
pub fn heights(frame: &Frame, atlas: &AtlasPages) -> Heights {
    let (w, h) = (usize::from(frame.canvas.0), usize::from(frame.canvas.1));
    let mut hv = vec![0u8; w * h];
    let mut dv = vec![0u8; w * h];
    let mut iv = vec![0u16; w * h];
    let mut depth = vec![0u8; frame.sprites.len()];
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
                            iv[i] = 0;
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
                    let index = cmds.start as usize + k;
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
                            dv[i] = depth[index];
                            iv[i] = (index + 1).min(usize::from(u16::MAX)) as u16;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Heights { h: hv, depth: dv, id: iv, w, rows: h }
}

/// `scatter.wgsl`'s `FLOAT` and `WALK`: a run whose lowest px is this high stands on the ground,
/// and one followed this far down does too.
const FLOAT: i32 = 6;
const WALK: usize = 192;

/// `scatter.wgsl`'s `run_bottom`: the bottom of the run of its sprite's px straight down the
/// screen that px `(x, y)` is part of, 0 if the run reaches the ground.
fn run_bottom(g: &Heights, x: usize, y: usize) -> i32 {
    let who = g.id[y * g.w + x];
    let mut low = i32::from(g.h[y * g.w + x]);
    let mut hidden = 0;
    for yy in y + 1..(y + 1 + WALK).min(g.rows) {
        let i = yy * g.w + x;
        match g.id[i].cmp(&who) {
            std::cmp::Ordering::Equal => {
                low = i32::from(g.h[i]);
                hidden = 0;
                if low <= FLOAT {
                    return 0;
                }
            }
            std::cmp::Ordering::Greater => hidden += 1,
            std::cmp::Ordering::Less => {
                let bottom = low - hidden * 5 / 4;
                return if bottom <= FLOAT { 0 } else { bottom };
            }
        }
    }
    0
}

/// The scatter as `scatter.wgsl` does it: each lifted px raised on `(x, y + rows_up(h))` and the
/// rows of its depth behind it (no deeper than it is wide; the terrain a px wider each side), the
/// tallest kept as the top and the lowest run's bottom as the bottom. `(top, bottom)` per canvas px.
pub fn field(g: &Heights) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (g.w, g.rows);
    let mut top = vec![0u8; w * h];
    let mut bottom = vec![u8::MAX; w * h];
    for y in 0..h {
        for x in 0..w {
            let z = g.h[y * w + x];
            if i32::from(z) <= jane_present::shadow::GROUND {
                continue;
            }
            let who = g.id[y * w + x];
            // Not one of the frame's casters, or the terrain's relief: it stands in no field.
            if (who != 0 && g.depth[y * w + x] == 0) || (who == 0 && i32::from(z) <= jane_present::shadow::RELIEF) {
                continue;
            }
            let lo = if who == 0 { 0 } else { run_bottom(g, x, y).min(i32::from(z)) as u8 };
            let mut d = i32::from(g.depth[y * w + x].max(1));
            if who != 0 {
                // `scatter.wgsl`'s `footprint`: no deeper than it is wide, and round.
                let same = |xx: i32| xx >= 0 && xx < w as i32 && g.id[y * w + xx as usize] == who;
                let run = |dx: i32| (0..64u8).find(|&e| !same(x as i32 + dx * (i32::from(e) + 1))).unwrap_or(64);
                let (l, r) = (run(-1), run(1));
                let mid = d.min(2 * ((i32::from(l) + i32::from(r)) / 2) + 2);
                let u = (f32::from(l.min(r)) + 0.5) / ((f32::from(l) + f32::from(r) + 1.0) * 0.5);
                let round_off = (1.0 - (1.0 - u) * (1.0 - u)).max(0.0).sqrt();
                let mid_f = f32::from(u8::try_from(mid).unwrap_or(u8::MAX));
                d = ((mid_f * round_off).round() as i32).clamp(mid.min(2), mid);
            }
            let y0 = y as i32 + jane_present::rows_up(i32::from(z)) - (d - 1);
            let spread = i32::from(g.id[y * w + x] == 0);
            for dx in -spread..=spread {
                let xx = x as i32 + dx;
                if xx < 0 || xx >= w as i32 {
                    continue;
                }
                for k in 0..d {
                    let yy = y0 + k;
                    if yy >= 0 && yy < h as i32 {
                        let j = yy as usize * w + xx as usize;
                        top[j] = top[j].max(z);
                        bottom[j] = bottom[j].min(lo);
                    }
                }
            }
        }
    }
    (top, bottom)
}

/// Grey, a false colour by value so steps show; green where `floor` (if given) says what stands
/// there floats.
pub fn png(v: &[u8], floor: Option<&[u8]>, w: usize, h: usize) -> Vec<u8> {
    let rgba: Vec<u8> = v
        .iter()
        .enumerate()
        .flat_map(|(i, &z)| {
            let g = (u32::from(z) * 4).min(255) as u8;
            let band = if z % 8 < 4 { 0 } else { 40 };
            let floats = z > 0 && floor.is_some_and(|f| f[i] > 0 && f[i] < u8::MAX);
            if floats {
                [g / 3, g.max(90), g / 3, 255]
            } else {
                [g, g.saturating_sub(band), if z == 0 { 60 } else { g }, 255]
            }
        })
        .collect();
    jane_art::sheet::png(w as u32, h as u32, &rgba)
}
