//! `jane mapsheet [--out DIR] [--src DIR]`: the county's review sheet, from its source
//! (`data/world/county`): the macro plan drawn top-down, its carve, 1:1 crops of each kind of joint,
//! and `county-notes.txt` with what looks wrong. The railway is the baked county's
//! (`assets/world/county.bp`): every track cell and every rail cell, and the halt's platform.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;

use jane_core::blueprint::SpanKind;
use jane_world::county::terraced::macro_check::{Issue, check, off_route};
use jane_world::county::terraced::macro_plan::{Border, GateKind, MH, MW, MacroPlan, Reg, Role, Wet, idx};
use jane_world::skeleton::Biome;

pub const USAGE: &str = "  mapsheet [--out DIR] [--src DIR]
                                      the county's source (data/world/county) as sheets: its macro plan, its carve (1 px to
                                      2 cells), 1:1 crops of each kind of joint, and notes, with the baked county's railway
                                      (assets/world/county.bp) drawn on (default out: progress/mapsheet)";

type Rgb = [u8; 3];

struct Img {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Img {
    fn new(w: usize, h: usize, c: Rgb) -> Self {
        let mut px = vec![255u8; w * h * 4];
        for p in px.chunks_mut(4) {
            p[..3].copy_from_slice(&c);
        }
        Self { w, h, px }
    }
    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = (y as usize * self.w + x as usize) * 4;
            self.px[i..i + 3].copy_from_slice(&c);
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for j in 0..h {
            for i in 0..w {
                self.put(x + i, y + j, c);
            }
        }
    }
    fn line(&mut self, a: (i32, i32), b: (i32, i32), t: i32, c: Rgb) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let n = dx.abs().max(dy.abs()).max(1);
        for k in 0..=n {
            let (x, y) = (a.0 + dx * k / n, a.1 + dy * k / n);
            self.rect(x - t / 2, y - t / 2, t, t, c);
        }
    }
    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r {
                    self.put(cx + x, cy + y, c);
                }
            }
        }
    }
    fn ring(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for y in -r..=r {
            for x in -r..=r {
                let d = x * x + y * y;
                if d <= r * r && d >= (r - 1) * (r - 1) && y <= 1 {
                    self.put(cx + x, cy + y, c);
                }
            }
        }
    }
    /// A triangle pointing along (dx, dy), half-size `r`.
    fn tri(&mut self, cx: i32, cy: i32, r: i32, dir: (i32, i32), c: Rgb) {
        let (px, py) = (-dir.1, dir.0);
        for u in -r..=r {
            for v in -r..=r {
                // u along dir, v across; the tip at u = r, the base at u = -r.
                if v.abs() * 2 <= r - u {
                    self.put(cx + u * dir.0 + v * px, cy + u * dir.1 + v * py, c);
                }
            }
        }
    }
    fn text(&mut self, x: i32, y: i32, s: &str, k: i32, c: Rgb) {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch.to_ascii_uppercase());
            for row in 0..5 {
                for col in 0..3 {
                    if g >> (14 - (row * 3 + col)) & 1 == 1 {
                        self.rect(cx + col * k, y + row * k, k, k, c);
                    }
                }
            }
            cx += 4 * k;
        }
    }
    fn shadow_text(&mut self, x: i32, y: i32, s: &str, k: i32, c: Rgb) {
        self.text(x + 1, y + 1, s, k, [0, 0, 0]);
        self.text(x, y, s, k, c);
    }
    fn blit(&mut self, o: &Img, x0: usize, y0: usize) {
        for y in 0..o.h {
            let (a, b) = (((y0 + y) * self.w + x0) * 4, (y * o.w) * 4);
            self.px[a..a + o.w * 4].copy_from_slice(&o.px[b..b + o.w * 4]);
        }
    }
}

/// A 3 x 5 font, rows top to bottom, 3 bits a row, packed from bit 14.
fn glyph(c: char) -> u16 {
    let rows: [u16; 5] = match c {
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [3, 4, 4, 4, 3],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [3, 4, 5, 5, 3],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 2],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [2, 5, 5, 5, 2],
        'P' => [6, 5, 6, 4, 4],
        'Q' => [2, 5, 5, 7, 3],
        'R' => [6, 5, 6, 5, 5],
        'S' => [3, 4, 2, 1, 6],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [6, 1, 2, 4, 7],
        '3' => [6, 1, 2, 1, 6],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 6, 1, 6],
        '6' => [3, 4, 7, 5, 7],
        '7' => [7, 1, 2, 2, 2],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 6],
        '.' => [0, 0, 0, 0, 2],
        ',' => [0, 0, 0, 2, 4],
        ':' => [0, 2, 0, 2, 0],
        '-' => [0, 0, 7, 0, 0],
        '\'' => [2, 2, 0, 0, 0],
        '/' => [1, 1, 2, 4, 4],
        '(' => [2, 4, 4, 4, 2],
        ')' => [2, 1, 1, 1, 2],
        '>' => [4, 2, 1, 2, 4],
        '=' => [0, 7, 0, 7, 0],
        '+' => [0, 2, 7, 2, 0],
        _ => [0; 5],
    };
    rows.iter().fold(0, |a, r| (a << 3) | r)
}

const LEVEL: [Rgb; 4] = [[104, 118, 92], [112, 152, 86], [172, 178, 110], [214, 198, 152]];
const LAKE: Rgb = [40, 78, 132];
const POND: Rgb = [56, 96, 150];
const RIM: Rgb = [120, 164, 196];
const RIVER: Rgb = [84, 140, 214];
const CLIFF: Rgb = [58, 40, 30];
const ROAD: Rgb = [232, 214, 168];
const INK: Rgb = [20, 20, 24];
/// The baked railway, one colour for track and rail so the line reads against roads and water.
const TRACK: Rgb = [236, 48, 132];
/// The halt's platform. It is a named rect, not a tile of its own.
const PLATFORM: Rgb = [120, 255, 230];

#[allow(clippy::match_same_arms)]
fn biome_tint(b: Biome) -> Rgb {
    match b {
        Biome::Field => [226, 200, 90],
        Biome::Hedge => [120, 170, 70],
        Biome::Wood => [24, 90, 48],
        Biome::Foothill => [150, 110, 76],
        Biome::Reed => [170, 190, 80],
        Biome::Marsh => [60, 140, 130],
        Biome::WetWood => [40, 100, 100],
        Biome::Garden => [230, 120, 160],
        Biome::Slag => [96, 80, 100],
        Biome::Yard => [160, 160, 172],
        Biome::Hill => [186, 170, 150],
        Biome::Town => [240, 210, 170],
    }
}

fn biome_name(b: Biome) -> &'static str {
    match b {
        Biome::Field => "FIELD",
        Biome::Hedge => "HEDGE",
        Biome::Wood => "WOOD",
        Biome::Foothill => "FOOTHILL",
        Biome::Reed => "REED",
        Biome::Marsh => "MARSH",
        Biome::WetWood => "WET WOOD",
        Biome::Garden => "GARDEN",
        Biome::Slag => "SLAG",
        Biome::Yard => "YARD",
        Biome::Hill => "HILL",
        Biome::Town => "TOWN",
    }
}

#[allow(clippy::match_same_arms)]
fn gate_colour(k: GateKind) -> Rgb {
    match k {
        GateKind::RampRoad => [228, 120, 40],
        GateKind::Stair => [250, 250, 250],
        GateKind::Span => [150, 100, 60],
        GateKind::Underpass => [250, 250, 250],
        GateKind::CompanyGate => [170, 30, 30],
        GateKind::SteppingStones => [200, 230, 250],
        GateKind::Verb(_) => [140, 60, 190],
        GateKind::Gap => [200, 200, 200],
        GateKind::Ledge => [250, 220, 40],
    }
}

fn span_colour(k: SpanKind) -> Rgb {
    match k {
        SpanKind::Bridge => [214, 200, 170],
        SpanKind::Footbridge => [150, 100, 60],
        SpanKind::Viaduct => [176, 166, 150],
        SpanKind::Walkway => [124, 124, 144],
        SpanKind::Rail => [96, 64, 52],
    }
}

/// The standing water (a rim of light on its inside edge), the river through the lake, and the fall.
fn draw_water(im: &mut Img, p: &MacroPlan, k: i32) {
    let wet = |x: i32, y: i32| if (0..MW).contains(&x) && (0..MH).contains(&y) { p.wet[idx(x, y)] } else { Wet::Dry };
    for y in 0..MH {
        for x in 0..MW {
            let w = wet(x, y);
            if !w.body() {
                continue;
            }
            im.rect(x * k, y * k, k, k, if w == Wet::Pond { POND } else { LAKE });
        }
    }
    for y in 0..MH {
        for x in 0..MW {
            if !wet(x, y).body() {
                continue;
            }
            for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !(0..MW).contains(&nx) || !(0..MH).contains(&ny) || wet(nx, ny).body() {
                    continue;
                }
                match (dx, dy) {
                    (1, 0) => im.rect(x * k + k - 1, y * k, 1, k, RIM),
                    (-1, 0) => im.rect(x * k, y * k, 1, k, RIM),
                    (0, 1) => im.rect(x * k, y * k + k - 1, k, 1, RIM),
                    _ => im.rect(x * k, y * k, k, 1, RIM),
                }
            }
        }
    }
    let ctr = |c: (i32, i32)| (c.0 * k + k / 2, c.1 * k + k / 2);
    // The lake cell a river cell opens onto, for the inlet and the outlet.
    let lake_next = |q: (u8, u8)| -> Option<(i32, i32)> {
        let (x, y) = (i32::from(q.0), i32::from(q.1));
        [(1, 0), (0, 1), (-1, 0), (0, -1)]
            .iter()
            .map(|&(dx, dy)| (x + dx, y + dy))
            .find(|&(nx, ny)| wet(nx, ny) == Wet::Lake)
    };
    let mut reaches: Vec<Vec<(i32, i32)>> = Vec::new();
    let cell = |q: &(u8, u8)| (i32::from(q.0), i32::from(q.1));
    if let Some(&last) = p.river_in.last() {
        let mut pts: Vec<(i32, i32)> = p.river_in.iter().map(cell).collect();
        pts.extend(lake_next(last));
        reaches.push(pts);
    }
    if let Some(&first) = p.river_out.first() {
        let mut pts: Vec<(i32, i32)> = lake_next(first).into_iter().collect();
        pts.extend(p.river_out.iter().map(cell));
        reaches.push(pts);
    }
    for pts in reaches {
        for pair in pts.windows(2) {
            im.line(ctr(pair[0]), ctr(pair[1]), (k / 4).max(2), RIVER);
        }
    }
    if let Some(f) = p.fall {
        let (cx, cy) = ctr((i32::from(f.0), i32::from(f.1)));
        im.tri(cx, cy, 4, (0, 1), INK);
        im.tri(cx, cy, 3, (0, 1), [255, 255, 255]);
    }
}

/// One plan as a map: `k` pixels a macro cell; icons and labels only when `k >= 6`.
fn draw(p: &MacroPlan, k: i32) -> Img {
    let (w, h) = (MW * k, MH * k);
    let mut im = Img::new(w as usize, h as usize, [0, 0, 0]);
    let lvl = |x: i32, y: i32| p.level_at(x, y) as usize;
    for y in 0..MH {
        for x in 0..MW {
            let mut c = LEVEL[lvl(x, y)];
            let tint = biome_tint(p.districts[p.district_at(x, y) as usize].biome);
            for k3 in 0..3 {
                c[k3] = ((u32::from(c[k3]) * 62 + u32::from(tint[k3]) * 38) / 100) as u8;
            }
            // The river valley floor: ground within a cell of the Lowfields/Waters border, both sides.
            let r = p.reg_at(x, y);
            if r != Reg::Works {
                let other = if r == Reg::Lowfields { Reg::Waters } else { Reg::Lowfields };
                let near = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        (0..MW).contains(&(x + dx)) && (0..MH).contains(&(y + dy)) && p.reg_at(x + dx, y + dy) == other
                    })
                });
                if near {
                    for k3 in 0..3 {
                        c[k3] = c[k3].midpoint(LEVEL[0][k3]);
                    }
                }
            }
            im.rect(x * k, y * k, k, k, c);
        }
    }
    draw_water(&mut im, p, k);
    // Edges: a cliff where the level or the region changes, a faint line between same-level districts.
    for y in 0..MH {
        for x in 0..MW {
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= MW || ny >= MH || p.district_at(x, y) == p.district_at(nx, ny) {
                    continue;
                }
                let t = if p.reg_at(x, y) != p.reg_at(nx, ny) {
                    (k / 3).max(2)
                } else if lvl(x, y) != lvl(nx, ny) {
                    (k / 4).max(1)
                } else {
                    0
                };
                let (ex, ey) = ((x + dx) * k, (y + dy) * k);
                let col = if t == 0 {
                    [(LEVEL[lvl(x, y)][0] / 2) + 40, (LEVEL[lvl(x, y)][1] / 2) + 40, (LEVEL[lvl(x, y)][2] / 2) + 40]
                } else {
                    CLIFF
                };
                let t = t.max(1);
                if dx == 1 {
                    im.rect(ex - t / 2, y * k, t, k, col);
                } else {
                    im.rect(x * k, ey - t / 2, k, t, col);
                }
            }
        }
    }
    let ctr = |c: (i32, i32)| (c.0 * k + k / 2, c.1 * k + k / 2);
    for r in &p.routes {
        for pair in r.pts.windows(2) {
            im.line(
                ctr((i32::from(pair[0].0), i32::from(pair[0].1))),
                ctr((i32::from(pair[1].0), i32::from(pair[1].1))),
                (k / 4).max(1),
                ROAD,
            );
        }
    }
    let big = k >= 6;
    for c in &p.crossings {
        let (cx, cy) = ctr((i32::from(c.at.0), i32::from(c.at.1)));
        let u = if big { 4 } else { 2 };
        let (w, h) = if c.along_x { (2 * u + 1, u) } else { (u, 2 * u + 1) };
        im.rect(cx - w / 2 - 1, cy - h / 2 - 1, w + 2, h + 2, INK);
        im.rect(cx - w / 2, cy - h / 2, w, h, span_colour(c.kind));
    }
    for g in &p.gates {
        let (a, b) = (ctr((i32::from(g.a.0), i32::from(g.a.1))), ctr((i32::from(g.b.0), i32::from(g.b.1))));
        let (cx, cy) = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        let col = gate_colour(g.kind);
        let u = if big { 4 } else { 2 };
        match g.kind {
            GateKind::Ledge => {
                let d = g.dir();
                im.tri(cx, cy, u + 1, d, INK);
                im.tri(cx, cy, u, d, col);
            }
            GateKind::RampRoad => {
                im.tri(cx, cy, u + 1, (0, -1), INK);
                im.tri(cx, cy, u, (0, -1), col);
            }
            GateKind::Stair => {
                im.rect(cx - u, cy - u, 2 * u + 1, 2 * u + 1, INK);
                for i in 0..3 {
                    im.rect(cx - u + 1, cy - u + 1 + i * (u - 1), 2 * u - 1, 1.max(u / 3), col);
                }
            }
            GateKind::Span => {
                im.rect(cx - u - 1, cy - u / 2 - 1, 2 * u + 3, u + 3, INK);
                im.rect(cx - u, cy - u / 2, 2 * u + 1, u + 1, col);
            }
            GateKind::Underpass => {
                im.ring(cx, cy + u / 2, u + 1, INK);
                im.ring(cx, cy + u / 2, u, col);
            }
            GateKind::CompanyGate => {
                im.rect(cx - u, cy - u, 2 * u + 1, 2 * u + 1, INK);
                im.rect(cx - u + 1, cy - u + 1, 2 * u - 1, 2 * u - 1, col);
            }
            GateKind::SteppingStones => {
                for i in -1..=1 {
                    im.disc(cx + i * (u - 1) * 2 / 2, cy + i, (u / 3).max(1), col);
                }
            }
            GateKind::Verb(_) => {
                im.disc(cx, cy, u, INK);
                im.disc(cx, cy, u - 1, col);
            }
            GateKind::Gap => im.rect(cx - 1, cy - 1, if big { 3 } else { 2 }, if big { 3 } else { 2 }, col),
        }
        if big && !matches!(g.kind, GateKind::Gap | GateKind::Stair | GateKind::RampRoad | GateKind::Ledge)
            || (big && g.border == Border::Wall && !matches!(g.kind, GateKind::Gap | GateKind::Ledge))
        {
            im.shadow_text(cx + u + 3, cy - 2, g.name.trim_start_matches("The "), 1, [255, 255, 255]);
        }
    }
    for s in &p.sites {
        let (cx, cy) = ctr((i32::from(s.pos.0), i32::from(s.pos.1)));
        let col = if s.dungeon { [244, 150, 40] } else { [255, 232, 80] };
        let r = if big { 4 } else { 2 };
        im.disc(cx, cy, r + 1, INK);
        im.disc(cx, cy, r, col);
        if big {
            im.shadow_text(cx + 7, cy - 3, s.name, 1, [255, 255, 255]);
        }
    }
    im
}

fn legend(im: &mut Img, x: i32, mut y: i32) {
    let t = [255, 255, 255];
    im.text(x, y, "LEVELS", 2, t);
    y += 14;
    for (i, n) in ["0 VALLEY FLOOR", "1 TERRACE", "2 UPLAND, WORKS", "3 THE CROWN"].iter().enumerate() {
        im.rect(x, y, 10, 8, LEVEL[i]);
        im.text(x + 14, y + 1, n, 1, t);
        y += 11;
    }
    y += 8;
    im.text(x, y, "GATES", 2, t);
    y += 14;
    let kinds = [
        (GateKind::RampRoad, "RAMP ROAD"),
        (GateKind::Stair, "STAIR"),
        (GateKind::Span, "SPAN, VIADUCT"),
        (GateKind::Underpass, "UNDERPASS"),
        (GateKind::CompanyGate, "COMPANY GATE"),
        (GateKind::SteppingStones, "STEPPING STONES"),
        (GateKind::Verb(jane_world::county::terraced::macro_plan::Verb::Repair), "VERB GATE"),
        (GateKind::Gap, "GAP IN A HEDGE"),
        (GateKind::Ledge, "LEDGE, HOP DOWN"),
    ];
    for (kind, n) in kinds {
        let (cx, cy) = (x + 5, y + 4);
        let col = gate_colour(kind);
        match kind {
            GateKind::Ledge => im.tri(cx, cy, 4, (0, 1), col),
            GateKind::RampRoad => im.tri(cx, cy, 4, (0, -1), col),
            GateKind::Stair => im.rect(cx - 4, cy - 3, 9, 6, col),
            GateKind::Span => im.rect(cx - 5, cy - 2, 11, 5, col),
            GateKind::Underpass => im.ring(cx, cy + 2, 4, col),
            GateKind::CompanyGate => im.rect(cx - 4, cy - 4, 9, 9, col),
            GateKind::SteppingStones => {
                for i in -1..=1 {
                    im.disc(cx + i * 3, cy, 1, col);
                }
            }
            GateKind::Verb(_) => im.disc(cx, cy, 4, col),
            GateKind::Gap => im.rect(cx - 1, cy - 1, 3, 3, col),
        }
        im.text(x + 16, y + 1, n, 1, t);
        y += 11;
    }
    y += 6;
    im.rect(x, y + 3, 12, 2, ROAD);
    im.text(x + 16, y + 1, "ROAD", 1, t);
    y += 11;
    im.rect(x, y + 1, 12, 3, CLIFF);
    im.text(x + 16, y + 1, "CLIFF, REGION WALL", 1, t);
    y += 17;
    im.text(x, y, "WATER AND SPANS", 2, t);
    y += 14;
    im.rect(x, y, 12, 8, LAKE);
    im.text(x + 16, y + 1, "WATER", 1, t);
    y += 11;
    im.rect(x, y + 3, 12, 3, RIVER);
    im.text(x + 16, y + 1, "RIVER", 1, t);
    y += 11;
    im.tri(x + 6, y + 4, 4, (0, 1), [255, 255, 255]);
    im.text(x + 16, y + 1, "FALL", 1, t);
    y += 11;
    for (kind, n) in [
        (SpanKind::Bridge, "BRIDGE"),
        (SpanKind::Footbridge, "FOOTBRIDGE"),
        (SpanKind::Viaduct, "VIADUCT"),
        (SpanKind::Walkway, "WALKWAY"),
        (SpanKind::Rail, "RAIL ARCH"),
    ] {
        im.rect(x, y + 1, 12, 6, INK);
        im.rect(x + 1, y + 2, 10, 4, span_colour(kind));
        im.text(x + 16, y + 1, n, 1, t);
        y += 10;
    }
    im.rect(x, y, 12, 8, TRACK);
    im.text(x + 16, y + 1, "TRACK", 1, t);
    y += 11;
    im.rect(x, y, 12, 8, PLATFORM);
    im.text(x + 16, y + 1, "PLATFORM", 1, t);
}

fn region_gates(p: &MacroPlan) -> [(usize, usize); 3] {
    let mut out = [(0, 0); 3];
    for (i, (ra, rb)) in
        [(Reg::Lowfields, Reg::Works), (Reg::Lowfields, Reg::Waters), (Reg::Waters, Reg::Works)].into_iter().enumerate()
    {
        for g in &p.gates {
            if g.kind == GateKind::Ledge || g.border != Border::Wall {
                continue;
            }
            let (x, y) = (p.districts[g.da as usize].reg, p.districts[g.db as usize].reg);
            if (x == ra && y == rb) || (x == rb && y == ra) {
                out[i].0 += 1;
                out[i].1 += usize::from(g.road);
            }
        }
    }
    out
}

/// Road time in seconds: macro-cell length at 7.5 cells a second, times 1.25 for the road's wind.
fn route_secs(p: &MacroPlan, from: &str, to: &str) -> i32 {
    p.routes.iter().find(|r| r.from == from && r.to == to).map_or(-1, |r| r.cells / 5)
}

/// The water and the spans of a plan in a line: the lake and its share of its district, the mere
/// and ponds, the river's two reaches, the fall, the mouth, and the crossings by kind.
fn water_line(p: &MacroPlan) -> String {
    let n = |w: Wet| p.wet.iter().filter(|&&q| q == w).count();
    let area = p.districts[Role::Lake.id() as usize].area.max(1) as usize;
    let lake = n(Wet::Lake);
    let ponds = p.wet.iter().enumerate().filter(|&(_, &w)| w == Wet::Pond).count();
    let mouth = p.river_out.last().map_or_else(|| String::from("none"), |m| format!("({},{})", m.0, m.1));
    let kinds = |k: SpanKind| p.crossings.iter().filter(|c| c.kind == k).count();
    format!(
        "lake {lake} ({} permille of The Lake, {area} cells), mere {}, ponds {ponds} cells; river in {} / out {} cells, fall {}, mouth {mouth}; crossings: bridge {}, footbridge {}, viaduct {}, walkway {}, rail {}",
        lake * 1000 / area,
        n(Wet::Mere),
        p.river_in.len(),
        p.river_out.len(),
        if p.fall.is_some() { "yes" } else { "no" },
        kinds(SpanKind::Bridge),
        kinds(SpanKind::Footbridge),
        kinds(SpanKind::Viaduct),
        kinds(SpanKind::Walkway),
        kinds(SpanKind::Rail),
    )
}

/// What looks off about a plan, for the owner's review.
fn notes(p: &MacroPlan, issues: &[Issue]) -> String {
    let mut s = String::new();
    let rg = region_gates(p);
    let _ = write!(
        s,
        "the county: {} layout, roll {}; border gates (all/road) L-W {}/{} L-Wa {}/{} Wa-W {}/{}; ledges {}; first walk Halt-Julie's {}s, Julie's-Castle {}s, Castle-mine {}s, Castle-School {}s",
        p.arch,
        p.attempt + 1,
        rg[0].0,
        rg[0].1,
        rg[1].0,
        rg[1].1,
        rg[2].0,
        rg[2].1,
        p.gates.iter().filter(|g| g.kind == GateKind::Ledge).count(),
        route_secs(p, "station", "julie_house"),
        route_secs(p, "julie_house", "town"),
        route_secs(p, "town", "gold_mine"),
        route_secs(p, "town", "gold_mine")
            + route_secs(p, "town", "graveyard")
            + route_secs(p, "graveyard", "factory")
            + route_secs(p, "factory", "school"),
    );
    let mut flags: Vec<String> = Vec::new();
    for i in issues {
        flags.push(format!("FAILS {}: {}", i.code, i.text));
    }
    for (i, d) in p.districts.iter().enumerate() {
        let n = p
            .gates
            .iter()
            .filter(|g| {
                g.kind != GateKind::Ledge
                    && !matches!(g.kind, GateKind::Verb(_))
                    && (usize::from(g.da) == i || usize::from(g.db) == i)
            })
            .count();
        if n <= 2 && !d.role.pocket() {
            flags.push(format!("{} has only {n} walk-in gates", d.role.name()));
        }
        if d.area < 150 || d.area > 1250 {
            flags.push(format!("{} is {} macro cells", d.role.name(), d.area));
        }
        if d.role == Role::Crown && n != 1 {
            flags.push(format!("crown has {n} ways up"));
        }
    }
    for st in &p.sites {
        let off = off_route(p, st.id);
        if off > 7 && !matches!(st.id, "car_wood" | "burial" | "butterfly_forest") {
            flags.push(format!("{} stands {off} macro cells (~{} cells) off every road", st.name, off * 16));
        }
        if matches!(st.id, "burial" | "butterfly_forest") && off > 14 {
            flags.push(format!("{} is {off} macro cells from any road", st.name));
        }
    }
    let _ = write!(s, "\n    water: {}", water_line(p));
    if flags.is_empty() {
        s.push_str("; no flags");
    } else {
        for f in flags {
            let _ = write!(s, "\n    - {f}");
        }
    }
    s
}

// ---- MAP R4 WP4: the carved sketch --------------------------------------------------------------

use jane_core::{Blueprint, Key, Rect, Tile};
use jane_world::county::terraced::carve::joins::JoinKind;
use jane_world::county::terraced::carve::{self, Carved, W};
use jane_world::county::terraced::macro_plan::STEP_CELLS;

const PAPER: Rgb = [24, 24, 28];
const FACE: Rgb = [52, 36, 28];
const NIGHT: Rgb = [250, 120, 120];
const WHITE: Rgb = [255, 255, 255];

/// A full-resolution canvas, one pixel a cell, where a mark of higher priority wins a downsample.
struct Canvas {
    img: Img,
    prio: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        Self { img: Img::new(W as usize, W as usize, PAPER), prio: vec![0; (W * W) as usize] }
    }
    fn put(&mut self, x: i32, y: i32, c: Rgb, p: u8) {
        if (0..W).contains(&x) && (0..W).contains(&y) && self.prio[(y * W + x) as usize] <= p {
            self.prio[(y * W + x) as usize] = p;
            self.img.put(x, y, c);
        }
    }
    fn rect(&mut self, r: Rect, c: Rgb, p: u8) {
        for y in r.y.max(0)..r.bottom().min(W) {
            for x in r.x.max(0)..r.right().min(W) {
                self.put(x, y, c, p);
            }
        }
    }
    fn outline(&mut self, r: Rect, c: Rgb, p: u8) {
        for x in r.x..r.right() {
            self.put(x, r.y, c, p);
            self.put(x, r.bottom() - 1, c, p);
        }
        for y in r.y..r.bottom() {
            self.put(r.x, y, c, p);
            self.put(r.right() - 1, y, c, p);
        }
    }
    /// The canvas at 1 pixel to `d` cells, each pixel the highest-priority mark of its block.
    fn down(&self, d: i32) -> Img {
        let n = (W / d) as usize;
        let mut o = Img::new(n, n, PAPER);
        for y in 0..n as i32 {
            for x in 0..n as i32 {
                let (mut best, mut bp) = ([0u8; 3], 0u8);
                for j in 0..d {
                    for i in 0..d {
                        let q = ((y * d + j) * W + x * d + i) as usize;
                        if self.prio[q] >= bp {
                            bp = self.prio[q];
                            best.copy_from_slice(&self.img.px[q * 4..q * 4 + 3]);
                        }
                    }
                }
                o.put(x, y, best);
            }
        }
        o
    }
    fn crop(&self, x0: i32, y0: i32, n: i32) -> Img {
        let mut o = Img::new(n as usize, n as usize, PAPER);
        for y in 0..n {
            for x in 0..n {
                let a = (((y0 + y) * W + x0 + x) * 4) as usize;
                o.put(x, y, [self.img.px[a], self.img.px[a + 1], self.img.px[a + 2]]);
            }
        }
        o
    }
}

fn ledge_dir(t: Tile) -> (i32, i32) {
    match t {
        Tile::LedgeE => (1, 0),
        Tile::LedgeW => (-1, 0),
        Tile::LedgeN => (0, -1),
        _ => (0, 1),
    }
}

fn join_colour(k: JoinKind) -> Rgb {
    match k {
        JoinKind::Stair => WHITE,
        JoinKind::Ramp => gate_colour(GateKind::RampRoad),
        JoinKind::Ladder => [90, 220, 220],
        JoinKind::Ledge => gate_colour(GateKind::Ledge),
        JoinKind::Stones => gate_colour(GateKind::SteppingStones),
        JoinKind::Closed(v) => gate_colour(GateKind::Verb(v)),
    }
}

/// The sketch painted one pixel a cell: level shades, faces, rims, water, ways, spans, joins, pads.
fn paint(c: &Carved) -> Canvas {
    let mut cv = Canvas::new();
    let mut row: Vec<Option<Tile>> = vec![None; W as usize];
    let mut next: Vec<Option<Tile>> = vec![None; W as usize];
    c.tile_row(0, &mut row);
    for y in 0..W {
        if y + 1 < W {
            next.fill(None);
            c.tile_row(y + 1, &mut next);
        }
        for x in 0..W {
            let lv = c.level_at(x, y) as usize;
            let (col, p) = match row[x as usize] {
                Some(Tile::Cliff) => (FACE, 3),
                Some(Tile::Water) => (if lv == 0 { RIVER } else { LAKE }, 2),
                Some(Tile::Waterfall) => (WHITE, 6),
                Some(Tile::Boardwalk) => (span_colour(SpanKind::Footbridge), 5),
                _ if y + 1 < W && next[x as usize] == Some(Tile::Cliff) => ([236, 232, 200], 3),
                _ => (LEVEL[lv.min(3)], 1),
            };
            cv.put(x, y, col, p);
        }
        std::mem::swap(&mut row, &mut next);
    }
    // Ways (roads) as polylines of cells; their straight legs brighter.
    for w in &c.ways {
        for (i, pair) in w.pts.windows(2).enumerate() {
            let straight = w.straight.iter().any(|&(a, b)| (usize::from(a)..usize::from(b)).contains(&i));
            let (a, b) = (pair[0], pair[1]);
            let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
            for k in 0..=n {
                let (x, y) = (a.0 + (b.0 - a.0) * k / n, a.1 + (b.1 - a.1) * k / n);
                cv.rect(Rect::new(x - 1, y - 1, 3, 3), if straight { [255, 240, 190] } else { ROAD }, 4);
            }
        }
    }
    for (s, m) in c.spans.iter().zip(&c.span_meta) {
        for r in m.embankment.iter().chain(&m.abutments) {
            cv.rect(*r, [150, 140, 130], 5);
        }
        cv.rect(s.rect, span_colour(s.kind), 7);
        cv.outline(s.rect, INK, 7);
    }
    for r in &c.planks {
        cv.rect(*r, span_colour(SpanKind::Footbridge), 7);
    }
    for j in &c.joins {
        cv.rect(j.rect, join_colour(j.kind), 8);
    }
    for r in c.night_rects() {
        cv.outline(Rect::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2), NIGHT, 9);
    }
    for p in &c.pads {
        cv.outline(p.boxr, [255, 232, 80], 6);
    }
    cv
}

/// The join glyphs and pad names, drawn on a view at `1 / d` scale whose origin is cell `(ox, oy)`.
fn annotate(im: &mut Img, c: &Carved, d: i32, (ox, oy): (i32, i32), names: bool) {
    let at = |x: i32, y: i32| ((x - ox) / d, (y - oy) / d);
    let u = if d == 1 { 3 } else { 2 };
    for j in &c.joins {
        let (cx, cy) = at(j.rect.x + j.rect.w / 2, j.rect.y + j.rect.h / 2);
        if cx < 0 || cy < 0 || cx >= im.w as i32 || cy >= im.h as i32 {
            continue;
        }
        match j.kind {
            JoinKind::Ramp => {
                im.tri(cx, cy, u + 1, (0, -1), INK);
                im.tri(cx, cy, u, (0, -1), join_colour(j.kind));
            }
            JoinKind::Ledge => {
                let dir = ledge_dir(j.tile);
                im.tri(cx, cy, u + 1, dir, INK);
                im.tri(cx, cy, u, dir, join_colour(j.kind));
            }
            JoinKind::Stones => {
                for i in -1..=1 {
                    im.disc(cx + i * 2, cy + i, 1, join_colour(j.kind));
                }
            }
            JoinKind::Closed(_) => {
                im.disc(cx, cy, u + 1, INK);
                im.disc(cx, cy, u, join_colour(j.kind));
            }
            JoinKind::Stair | JoinKind::Ladder => {}
        }
    }
    if names {
        for p in &c.pads {
            let (x, y) = at(p.boxr.x, p.boxr.y);
            if x >= 0 && y >= 6 && (x as usize) < im.w && (y as usize) < im.h {
                im.shadow_text(x, y - 6, p.site, 1, WHITE);
            }
        }
    }
}

fn carved_legend(im: &mut Img, x: i32, mut y: i32) {
    im.text(x, y, "LEVELS", 2, WHITE);
    y += 14;
    for (i, n) in ["0 FLOOR", "1 TERRACE", "2 UPLAND", "3 CROWN"].iter().enumerate() {
        im.rect(x, y, 10, 8, LEVEL[i]);
        im.text(x + 14, y + 1, n, 1, WHITE);
        y += 11;
    }
    let rows = [
        (FACE, "SOUTH FACE"),
        ([236, 232, 200], "RIM"),
        (LAKE, "WATER"),
        (WHITE, "FALL, STAIR"),
        (gate_colour(GateKind::RampRoad), "RAMP"),
        (gate_colour(GateKind::Ledge), "LEDGE, CHEVRON"),
        (gate_colour(GateKind::SteppingStones), "STONES"),
        (gate_colour(GateKind::Verb(jane_world::county::terraced::macro_plan::Verb::Repair)), "CLOSED VERB GATE"),
        (ROAD, "ROAD"),
        ([255, 240, 190], "STRAIGHT LEG"),
        (NIGHT, "NIGHT GATE"),
        ([255, 232, 80], "PAD"),
        (span_colour(SpanKind::Bridge), "BRIDGE"),
        (span_colour(SpanKind::Footbridge), "FOOTBRIDGE"),
        (span_colour(SpanKind::Viaduct), "VIADUCT"),
        (span_colour(SpanKind::Walkway), "WALKWAY"),
        (span_colour(SpanKind::Rail), "RAIL ARCH"),
        (TRACK, "TRACK"),
        (PLATFORM, "PLATFORM"),
    ];
    for (col, n) in rows {
        im.rect(x, y, 10, 8, col);
        im.text(x + 14, y + 1, n, 1, WHITE);
        y += 11;
    }
}

fn carved_notes(c: &Carved) -> String {
    let rg = region_gates(&c.plan);
    let total: usize = c.ways.iter().map(|w| w.pts.len()).sum();
    let straight: usize = c.ways.iter().flat_map(|w| w.straight.iter()).map(|&(a, b)| usize::from(b - a) + 1).sum();
    let kinds = |k: JoinKind| c.joins.iter().filter(|j| j.kind == k).count();
    format!(
        "carve: salt {}; border gates (all/road) L-W {}/{} L-Wa {}/{} Wa-W {}/{}; joins stair {} ramp {} ledge {} stones {} closed {}; missing {}; spans {}, planks {}; night gates {}; road points on straight legs {straight}/{total} ({} permille)",
        c.salt,
        rg[0].0,
        rg[0].1,
        rg[1].0,
        rg[1].1,
        rg[2].0,
        rg[2].1,
        kinds(JoinKind::Stair),
        kinds(JoinKind::Ramp),
        kinds(JoinKind::Ledge),
        kinds(JoinKind::Stones),
        c.joins.iter().filter(|j| matches!(j.kind, JoinKind::Closed(_))).count(),
        c.missing.len(),
        c.spans.len(),
        c.planks.len(),
        c.night_gates.len(),
        straight * 1000 / total.max(1),
    )
}

fn write_png(path: &std::path::Path, im: &Img) -> Result<(), String> {
    std::fs::write(path, jane_art::sheet::png(im.w as u32, im.h as u32, &im.px))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Where to look: a ramp, a stair, a ledge, each span kind, the fall, a lake shore, a meander.
fn crop_targets(c: &Carved) -> Vec<(String, (i32, i32))> {
    let mut v: Vec<(String, (i32, i32))> = Vec::new();
    let ctr = |r: Rect| (r.x + r.w / 2, r.y + r.h / 2);
    for (kind, name) in [(JoinKind::Ramp, "ramp gate"), (JoinKind::Stair, "stair gate"), (JoinKind::Ledge, "ledge")] {
        if let Some(j) = c.joins.iter().find(|j| j.kind == kind) {
            v.push((name.into(), ctr(j.rect)));
        }
    }
    for (k, name) in [
        (SpanKind::Bridge, "bridge"),
        (SpanKind::Footbridge, "footbridge"),
        (SpanKind::Viaduct, "viaduct"),
        (SpanKind::Walkway, "walkway"),
        (SpanKind::Rail, "rail arch"),
    ] {
        if let Some(s) = c.spans.iter().find(|s| s.kind == k) {
            v.push((name.into(), ctr(s.rect)));
        }
    }
    if let Some(f) = c.water.falls.first() {
        v.push(("the fall".into(), ctr(*f)));
    }
    let lake = (0..MH).flat_map(|y| (0..MW).map(move |x| (x, y))).find(|&(x, y)| c.plan.wet[idx(x, y)] == Wet::Lake);
    if let Some((x, y)) = lake {
        v.push(("lake shore".into(), (x * STEP_CELLS, y * STEP_CELLS + STEP_CELLS / 2)));
    }
    let ch = &c.water.channel;
    if ch.len() > 24 {
        let turn = |i: usize| {
            let (a, b, d) = (ch[i], ch[i + 12], ch[i + 24]);
            ((b.x - a.x) * (d.y - b.y) - (b.y - a.y) * (d.x - b.x)).abs()
        };
        if let Some(i) = (0..ch.len() - 24).max_by_key(|&i| turn(i)) {
            v.push(("meander".into(), (ch[i + 12].x, ch[i + 12].y)));
        }
    }
    v
}

/// The baked county, the file the game plays. The sheet reads its railway from here.
fn baked_county() -> Result<Blueprint, String> {
    let path = PathBuf::from("assets/world/county.bp");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bp = jane_sim::baked::decode(&bytes, jane_core::ZoneId::County, false)
        .map_err(|e| format!("{}: {e:?}", path.display()))?;
    if bp.tiles.as_slice().len() != bp.w() as usize * bp.h() as usize {
        return Err(format!("{}: the tile grid is empty", path.display()));
    }
    Ok(bp)
}

/// The halt's platform rect, if the baked county names one.
fn platform_rect(bp: &Blueprint) -> Option<Rect> {
    let id = jane_data::catalog().name_id("platform")?;
    bp.rects.get(&Key::Name(id)).copied()
}

fn is_rail(t: Tile) -> bool {
    matches!(t, Tile::Track | Tile::Rail)
}

/// The railway and the platform on the cell canvas. The platform sits under the rails.
fn stamp_rail_canvas(cv: &mut Canvas, bp: &Blueprint) {
    if let Some(r) = platform_rect(bp) {
        cv.rect(r, PLATFORM, 9);
    }
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let tiles = bp.tiles.as_slice();
    for y in 0..h.min(W) {
        let row = (y as usize) * bp.w() as usize;
        for x in 0..w.min(W) {
            if is_rail(tiles[row + x as usize]) {
                cv.put(x, y, TRACK, 10);
            }
        }
    }
}

/// The same railway on a sheet whose pixels cover `cells` cells each. A pixel is rail when any
/// cell under it is, and the platform otherwise.
fn stamp_rail_img(im: &mut Img, bp: &Blueprint, cells: i32) {
    let plat = platform_rect(bp);
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let tiles = bp.tiles.as_slice();
    let rail_at =
        |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && is_rail(tiles[(y as usize) * (w as usize) + x as usize]);
    for py in 0..im.h as i32 {
        for px in 0..im.w as i32 {
            let mut rail = false;
            let mut on_plat = false;
            for dy in 0..cells {
                for dx in 0..cells {
                    let (x, y) = (px * cells + dx, py * cells + dy);
                    rail |= rail_at(x, y);
                    if let Some(r) = plat {
                        on_plat |= x >= r.x && x < r.right() && y >= r.y && y < r.bottom();
                    }
                }
            }
            if rail {
                im.put(px, py, TRACK);
            } else if on_plat {
                im.put(px, py, PLATFORM);
            }
        }
    }
}

fn ortho((x, y): (i32, i32)) -> [(i32, i32); 4] {
    [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
}

/// Rail cells in reading order.
fn rail_cells(bp: &Blueprint) -> Vec<(i32, i32)> {
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let tiles = bp.tiles.as_slice();
    let mut out = Vec::new();
    for y in 0..h {
        let row = (y as usize) * (w as usize);
        for x in 0..w {
            if is_rail(tiles[row + x as usize]) {
                out.push((x, y));
            }
        }
    }
    out
}

/// Four-connected pieces, longest first.
fn components(cells: &[(i32, i32)]) -> Vec<Vec<(i32, i32)>> {
    let set: HashSet<(i32, i32)> = cells.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut comps = Vec::new();
    for &p in cells {
        if !seen.insert(p) {
            continue;
        }
        let mut comp = Vec::new();
        let mut q = VecDeque::from([p]);
        while let Some(cur) = q.pop_front() {
            comp.push(cur);
            for n in ortho(cur) {
                if set.contains(&n) && seen.insert(n) {
                    q.push_back(n);
                }
            }
        }
        comps.push(comp);
    }
    comps.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    comps
}

/// Tiles inside a rect, by name.
fn tile_mix(bp: &Blueprint, r: Rect) -> String {
    let mut n: BTreeMap<Tile, usize> = BTreeMap::new();
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if let Some(t) = bp.tiles.get(x, y).copied() {
                *n.entry(t).or_default() += 1;
            }
        }
    }
    n.into_iter().map(|(t, k)| format!("{} {k}", t.name().to_ascii_lowercase())).collect::<Vec<_>>().join(", ")
}

/// The box around a piece, as (left, top, right, bottom).
fn bounds(cells: &[(i32, i32)]) -> (i32, i32, i32, i32) {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y) in cells {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    (x0, y0, x1, y1)
}

/// How many of each tile name, in name order.
fn name_mix<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for name in names {
        *n.entry(name).or_default() += 1;
    }
    n.into_iter().map(|(name, k)| format!("{} {k}", name.to_ascii_lowercase())).collect::<Vec<_>>().join(", ")
}

fn piece_mix(bp: &Blueprint, cells: &[(i32, i32)]) -> String {
    name_mix(cells.iter().map(|&(x, y)| bp.tiles.get(x, y).copied().map_or("off the map", Tile::name)))
}

fn row_mix(bp: &Blueprint, x0: i32, x1: i32, y: i32) -> String {
    name_mix((x0..=x1).map(|x| bp.tiles.get(x, y).copied().map_or("off the map", Tile::name)))
}

/// True when the piece is one row of exactly `n` track cells.
fn row_of_track(bp: &Blueprint, cells: &[(i32, i32)], n: usize) -> bool {
    if cells.len() != n {
        return false;
    }
    let y = cells[0].1;
    cells.iter().all(|&(x, yy)| yy == y && bp.tiles.get(x, yy).copied() == Some(Tile::Track))
}

fn piece_line(bp: &Blueprint, cells: &[(i32, i32)]) -> String {
    let (x0, y0, x1, y1) = bounds(cells);
    format!(
        "{} cells ({}) at ({x0},{y0})-({x1},{y1}), {} wide and {} tall",
        cells.len(),
        piece_mix(bp, cells),
        x1 - x0 + 1,
        y1 - y0 + 1
    )
}

/// Rows beside a piece, grouped where the same tiles repeat. `dir` is -1 to the north or +1 to the
/// south. The band runs three cells past the piece on each side, so a road or a fence that only
/// meets the track is still in the row. It stops at the map edge or after `limit` rows.
fn side_text(bp: &Blueprint, x0: i32, x1: i32, y_from: i32, dir: i32, limit: i32) -> String {
    let h = bp.h() as i32;
    let (xa, xb) = (x0 - 3, x1 + 3);
    let mut groups: Vec<(i32, i32, String)> = Vec::new();
    let mut y = y_from;
    for _ in 0..limit {
        if y < 0 || y >= h {
            break;
        }
        let mix = row_mix(bp, xa, xb, y);
        if let Some(last) = groups.last_mut() {
            if last.2 == mix {
                last.1 = y;
                y += dir;
                continue;
            }
        }
        groups.push((y, y, mix));
        y += dir;
    }
    if groups.is_empty() {
        return String::from("off the map");
    }
    groups
        .into_iter()
        .map(|(a, b, mix)| if a == b { format!("y{a} {mix}") } else { format!("y{a}-y{b} {mix}") })
        .collect::<Vec<_>>()
        .join("; ")
}

fn hist_text(comps: &[Vec<(i32, i32)>]) -> String {
    let mut n: BTreeMap<usize, usize> = BTreeMap::new();
    for piece in comps {
        *n.entry(piece.len()).or_default() += 1;
    }
    n.into_iter().map(|(len, k)| format!("{len} x{k}")).collect::<Vec<_>>().join(", ")
}

/// The carve's macro line, in macro cells. Each one is sixteen world cells. This is the line the
/// builder was given, not the tiles that got laid.
fn macro_text(c: &Carved) -> String {
    let rail = &c.rail;
    if rail.len() < 2 {
        return String::from("the carve has no macro railway");
    }
    let (a, b) = (rail[0], rail[rail.len() - 1]);
    let st = c.plan.site_cell("station");
    let on_edge = |(x, y): (i32, i32)| x == 0 || y == 0 || x == MW - 1 || y == MH - 1;
    let ends = match (on_edge(a), on_edge(b)) {
        (true, true) => "both ends sit on the macro edge",
        (true, false) => "only the first end sits on the macro edge",
        (false, true) => "only the last end sits on the macro edge",
        (false, false) => "neither end sits on the macro edge",
    };
    let through = if rail.contains(&st) {
        format!("through the station at ({},{})", st.0, st.1)
    } else {
        format!("the station at ({},{}) is not on it", st.0, st.1)
    };
    format!(
        "the carve's macro line is {} cells from ({},{}) {through} to ({},{}), and {ends}",
        rail.len(),
        a.0,
        a.1,
        b.0,
        b.1
    )
}

fn edge_list(cells: &[(i32, i32)], w: i32, h: i32) -> String {
    let edges: Vec<String> = cells
        .iter()
        .copied()
        .filter(|&(x, y)| x == 0 || y == 0 || x == w - 1 || y == h - 1)
        .map(|(x, y)| format!("({x},{y})"))
        .collect();
    if edges.is_empty() {
        String::from("no track or rail cell touches the map edge")
    } else {
        format!("on the map edge: {}", edges.join(", "))
    }
}

/// What the baked railway is, for the sheet's notes. The tiles are separate pieces, so this lists
/// the pieces. It does not trace one line through them.
fn rail_notes(bp: &Blueprint, c: &Carved) -> String {
    let cells = rail_cells(bp);
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let mut track = 0usize;
    let mut rails = 0usize;
    for &(x, y) in &cells {
        match bp.tiles.get(x, y).copied() {
            Some(Tile::Track) => track += 1,
            Some(Tile::Rail) => rails += 1,
            _ => {}
        }
    }
    let comps = components(&cells);
    let plat = match platform_rect(bp) {
        Some(r) => format!("platform ({},{}) {}x{} holds {}", r.x, r.y, r.w, r.h, tile_mix(bp, r)),
        None => String::from("no platform rect"),
    };
    let mut s = String::new();
    let _ = write!(
        s,
        "\n    rail: {track} track, {rails} rail, {} separate pieces ({}); {plat}; {}",
        comps.len(),
        hist_text(&comps),
        macro_text(c)
    );
    if comps.is_empty() {
        s.push_str("\n    - no track or rail cells");
        return s;
    }
    let (x0, y0, x1, y1) = bounds(&comps[0]);
    let _ = write!(s, "\n    - the longest piece is {}", piece_line(bp, &comps[0]));
    let _ = write!(s, "\n    - north of it, x{}-x{}: {}", x0 - 3, x1 + 3, side_text(bp, x0, x1, y0 - 1, -1, 8));
    let _ = write!(s, "\n    - south of it, x{}-x{}: {}", x0 - 3, x1 + 3, side_text(bp, x0, x1, y1 + 1, 1, 16));
    let _ = write!(s, "\n    - {}", edge_list(&cells, w, h));
    let mut scraps = Vec::new();
    let mut others = Vec::new();
    for piece in &comps[1..] {
        if row_of_track(bp, piece, 13) {
            scraps.push(piece.as_slice());
        } else {
            others.push(piece.as_slice());
        }
    }
    if scraps.is_empty() {
        s.push_str("\n    - no separate one-row runs of 13 track cells");
    } else {
        let spans = scraps
            .iter()
            .map(|piece| {
                let (x0, y, x1, _) = bounds(piece);
                format!("({x0},{y})-({x1},{y})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            s,
            "\n    - {} separate one-row runs of 13 track cells, none joined to the longest piece: {spans}",
            scraps.len()
        );
    }
    if others.is_empty() {
        s.push_str("\n    - no other pieces");
    } else {
        s.push_str("\n    - other pieces:");
        for piece in others {
            let _ = write!(s, " {}", piece_line(bp, piece));
        }
    }
    s
}

pub fn run(args: &[String]) -> Result<(), String> {
    /// A crop's side, in cells.
    const N: i32 = 144;
    let mut out = PathBuf::from("progress/mapsheet");
    let mut src = PathBuf::from("data/world/county");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut next = |what: &str| it.next().cloned().ok_or_else(|| format!("{what} needs a value"));
        match a.as_str() {
            "--out" => out = PathBuf::from(next("--out")?),
            "--src" => src = PathBuf::from(next("--src")?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let read = |f: &str| std::fs::read_to_string(src.join(f)).map_err(|e| format!("{}: {e}", src.join(f).display()));
    let source =
        jane_world::county::terraced::source::parse(&read("plan.txt")?, &read("places.txt")?, &read("routes.txt")?)
            .map_err(|e| e.to_string())?;
    let p = &source.plan;
    let issues = check(p);
    let mut text = notes(p, &issues);
    let mix: Vec<String> =
        p.districts.iter().map(|d| format!("{}={}", d.role.name(), biome_name(d.biome).to_lowercase())).collect();
    let _ = writeln!(text, "\n    biomes: {}", mix.join(", "));
    let bp = baked_county()?;

    // The macro plan, 1 px to 1/8 of a macro cell. The railway is stamped at the same scale.
    let mut big = draw(p, 8);
    let cells = W / big.w as i32;
    if cells >= 1 {
        stamp_rail_img(&mut big, &bp, cells);
    }
    let mut page = Img::new(big.w + 250, big.h, PAPER);
    page.blit(&big, 0, 0);
    let x = big.w as i32 + 10;
    page.text(x, 8, "THE COUNTY", 3, WHITE);
    page.text(x, 30, &format!("{} LAYOUT", p.arch.to_uppercase()), 1, [200, 200, 200]);
    legend(&mut page, x, 48);
    let mut by = 520;
    page.text(x, by, "BIOMES", 2, WHITE);
    by += 14;
    for b in Biome::ALL {
        page.rect(x, by, 10, 8, biome_tint(b));
        page.text(x + 14, by + 1, biome_name(b), 1, WHITE);
        by += 10;
    }
    let mut y = 430;
    for (n, (a, r)) in ["ESCARPMENT L-W", "RIVER L-WA", "SLAG CLIFFS WA-W"].iter().zip(region_gates(p)) {
        page.text(x, y, &format!("{n}: {a} GATES, {r} ROAD"), 1, WHITE);
        y += 9;
    }
    for i in &issues {
        page.text(x, y + 6, &i.code.to_uppercase(), 1, [255, 90, 90]);
        y += 9;
    }
    write_png(&out.join("county-plan.png"), &page)?;

    // The carve, 1 px to 2 cells.
    let c = carve::carve_proven(p.clone(), source.salt).map_err(|e| format!("the carve refused: {e:?}"))?;
    let mut cv = paint(&c);
    stamp_rail_canvas(&mut cv, &bp);
    let mut big = cv.down(2);
    annotate(&mut big, &c, 2, (0, 0), true);
    let mut page = Img::new(big.w + 190, big.h, PAPER);
    page.blit(&big, 0, 0);
    let x = big.w as i32 + 8;
    page.text(x, 8, "THE COUNTY", 3, WHITE);
    page.text(x, 30, &format!("{} LAYOUT, SALT {}", c.plan.arch.to_uppercase(), c.salt), 1, [200, 200, 200]);
    carved_legend(&mut page, x, 48);
    write_png(&out.join("county-carved.png"), &page)?;
    text.push_str(&carved_notes(&c));
    text.push_str(&rail_notes(&bp, &c));
    text.push('\n');

    // 1:1 crops of each kind of joint.
    let targets = crop_targets(&c);
    let cols = 4usize;
    let (cw, ch) = (N as usize + 8, N as usize + 20);
    let mut sheet = Img::new(cols * cw + 8, targets.len().div_ceil(cols).max(1) * ch + 20, PAPER);
    sheet.text(8, 6, &format!("THE COUNTY: 1 PX = 1 CELL, {N} CELLS A SIDE"), 1, WHITE);
    for (i, (label, (x, y))) in targets.iter().enumerate() {
        let o = ((*x - N / 2).clamp(0, W - N), (*y - N / 2).clamp(0, W - N));
        let mut cr = cv.crop(o.0, o.1, N);
        annotate(&mut cr, &c, 1, o, false);
        let (px, py) = (8 + (i % cols) * cw, 20 + (i / cols) * ch + 12);
        sheet.blit(&cr, px, py);
        sheet.text(px as i32, py as i32 - 9, &format!("{label} ({x},{y})"), 1, WHITE);
    }
    write_png(&out.join("county-crops.png"), &sheet)?;
    std::fs::write(out.join("county-notes.txt"), &text).map_err(|e| e.to_string())?;
    println!("{text}wrote county-plan.png, county-carved.png and county-crops.png to {}", out.display());
    Ok(())
}
