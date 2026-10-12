//! `jane mapsheet [--out DIR] [--from N] [--to M]`: MAP.md R0's review sheet. Each seed's macro
//! plan (`jane_world::county::terraced::macro_plan`) drawn top-down, one PNG per seed and a
//! contact sheet of them all, plus `notes.txt` with what looks wrong per seed.

use std::fmt::Write as _;
use std::path::PathBuf;

use jane_core::blueprint::SpanKind;
use jane_world::county::terraced::macro_check::{Issue, off_route};
use jane_world::county::terraced::macro_plan::{Border, GateKind, MH, MW, MacroPlan, Reg, Role, Wet, idx, plan};
use jane_world::skeleton::Biome;

pub const USAGE: &str = "  mapsheet [--out DIR] [--from N] [--to M] [--tag T]
                                      MAP.md R0: the macro plan of seeds N..M (default 1..24) as PNGs, a contact sheet and notes.txt";

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
        (SpanKind::Rail, "RAIL"),
    ] {
        im.rect(x, y + 1, 12, 6, INK);
        im.rect(x + 1, y + 2, 10, 4, span_colour(kind));
        im.text(x + 16, y + 1, n, 1, t);
        y += 10;
    }
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
fn notes(seed: u32, p: &MacroPlan, issues: &[Issue]) -> String {
    let mut s = String::new();
    let rg = region_gates(p);
    let _ = write!(
        s,
        "seed {seed}: {} layout, roll {}; border gates (all/road) L-W {}/{} L-Wa {}/{} Wa-W {}/{}; ledges {}; first walk Halt-Julie's {}s, Julie's-Castle {}s, Castle-mine {}s, Castle-School {}s",
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

pub fn run(args: &[String]) -> Result<(), String> {
    let mut out = PathBuf::from("progress/2026-10-09_69_map-r0");
    let mut tag = String::from("v3");
    let (mut from, mut to) = (1u32, 24u32);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut next = |what: &str| it.next().cloned().ok_or_else(|| format!("{what} needs a value"));
        match a.as_str() {
            "--out" => out = PathBuf::from(next("--out")?),
            "--tag" => tag = next("--tag")?,
            "--from" => from = next("--from")?.parse().map_err(|_| "--from: a number")?,
            "--to" => to = next("--to")?.parse().map_err(|_| "--to: a number")?,
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let n = (to + 1 - from) as usize;
    let (cols, tile, gap) = (6usize, (MW * 3) as usize, 8usize);
    let rows = n.div_ceil(cols);
    let mut sheet = Img::new(cols * (tile + gap) + gap, rows * (tile + gap + 12) + gap, [24, 24, 28]);
    let mut text = String::new();
    let mut archs: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for (i, seed) in (from..=to).enumerate() {
        let (p, issues) = plan(seed);
        let big = draw(&p, 8);
        let (pw, ph) = (big.w + 250, big.h);
        let mut page = Img::new(pw, ph, [24, 24, 28]);
        page.blit(&big, 0, 0);
        let x = big.w as i32 + 10;
        page.text(x, 8, &format!("SEED {seed}"), 3, [255, 255, 255]);
        page.text(x, 30, &format!("{} LAYOUT, ROLL {}", p.arch.to_uppercase(), p.attempt + 1), 1, [200, 200, 200]);
        legend(&mut page, x, 48);
        let mut by = 520;
        page.text(x, by, "BIOMES", 2, [255, 255, 255]);
        by += 14;
        for b in Biome::ALL {
            page.rect(x, by, 10, 8, biome_tint(b));
            page.text(x + 14, by + 1, biome_name(b), 1, [255, 255, 255]);
            by += 10;
        }
        let mut y = 380;
        let rg = region_gates(&p);
        for (n, (a, r)) in ["ESCARPMENT L-W", "RIVER L-WA", "SLAG CLIFFS WA-W"].iter().zip(rg) {
            page.text(x, y, &format!("{n}: {a} GATES, {r} ROAD"), 1, [255, 255, 255]);
            y += 9;
        }
        for i in &issues {
            page.text(x, y + 6, &i.code.to_uppercase(), 1, [255, 90, 90]);
            y += 9;
        }
        std::fs::write(
            out.join(format!("macro-seed-{seed:02}-{tag}.png")),
            jane_art::sheet::png(pw as u32, ph as u32, &page.px),
        )
        .map_err(|e| e.to_string())?;
        let small = draw(&p, 3);
        let (cx, cy) = (gap + (i % cols) * (tile + gap), gap + (i / cols) * (tile + gap + 12) + 12);
        sheet.blit(&small, cx, cy);
        sheet.text(cx as i32, cy as i32 - 9, &format!("SEED {seed}"), 1, [255, 255, 255]);
        *archs.entry(p.arch).or_insert(0) += 1;
        text.push_str(&notes(seed, &p, &issues));
        let mix: Vec<String> =
            p.districts.iter().map(|d| format!("{}={}", d.role.name(), biome_name(d.biome).to_lowercase())).collect();
        let _ = write!(text, "\n    biomes: {}", mix.join(", "));
        text.push('\n');
    }
    std::fs::write(
        out.join(format!("contact-sheet-{tag}.png")),
        jane_art::sheet::png(sheet.w as u32, sheet.h as u32, &sheet.px),
    )
    .map_err(|e| e.to_string())?;
    let mix: Vec<String> = archs.iter().map(|(a, c)| format!("{a} {c}")).collect();
    let text = format!("archetypes over seeds {from}..{to}: {}\n{text}", mix.join(", "));
    std::fs::write(out.join(format!("notes-{tag}.txt")), &text).map_err(|e| e.to_string())?;
    println!("{text}wrote {n} maps and contact-sheet.png to {}", out.display());
    Ok(())
}
