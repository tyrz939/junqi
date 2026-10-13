//! The county as authored data (`data/world/county/`): what the terraced county is laid from, in place
//! of a seed's searches. Three plain text files, read and written here, `no_std` plus `alloc`:
//!
//! - **`plan.txt`**: the macro plan (MAP.md 4.1) on its 96 x 96 grid of 16-cell macro cells: the dice the
//!   generated dressing throws on, the districts, gates, sites, routes, crossings and the river, then two
//!   character grids (the districts and the water).
//! - **`places.txt`**: where every patch, small place, anchor and story stands, and the stories' place
//!   names. The skeleton and the stories stage put each where the file says and search for nothing.
//! - **`routes.txt`**: the roads and footpaths as polylines of cells, and the lit stretches of road.
//!
//! The format: one record a line, words split on spaces, `"quoted text"` one word, `x,y` a cell, `#` to the
//! end of a line a comment. A line that starts with a space continues the record above it (a long polyline).
//! [`parse`] reads all three; [`write_plan`], [`write_places`] and [`write_routes`] write them back, so a
//! file read and written again is the same file.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use jane_core::StoryId;
use jane_core::blueprint::SpanKind;

use super::macro_plan::{
    Border, CELLS, Crossing, District, Gate, GateKind, MH, MW, MacroPlan, ROLES, Role, Route, Site, Verb, Wet, idx,
};
use crate::skeleton::Biome;

/// The county's source, read.
#[derive(Clone, Debug)]
pub struct Source {
    pub plan: MacroPlan,
    /// The carve's dice (the plan's seed and attempt are the rest of the dressing's).
    pub salt: u8,
    pub places: Places,
    pub routes: Routes,
}

/// Where the skeleton's things and the stories stand (`places.txt`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places {
    /// Each named patch by its row in the area table, at a macro cell, in row order.
    pub areas: Vec<(u8, (i32, i32))>,
    /// The small places nobody asked for by name: a kind from the small-place table, at a macro cell.
    pub pois: Vec<(u8, (i32, i32))>,
    /// The small places the story needs, by row in the anchor table, at a macro cell.
    pub anchors: Vec<(u8, (i32, i32))>,
    /// Each story's place, in the order of the story table.
    pub stories: Vec<StoryAt>,
}

/// A story's place: a cell inside the country place it claims (none: it is not placed), and the name its
/// place goes by when the story's own row gives it none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryAt {
    pub story: StoryId,
    pub at: Option<(i32, i32)>,
    pub name: Option<String>,
}

/// The roads and paths (`routes.txt`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Routes {
    /// Each of the plan's routes, in its order, as the line of cells its metal is laid along.
    pub roads: Vec<Vec<(i32, i32)>>,
    /// The footpaths, in the order they are trodden.
    pub paths: Vec<PathLine>,
    /// The macro cells of road that are lit.
    pub lit: Vec<(i32, i32)>,
}

/// A footpath: the burial's (`row` none) or a row of `data/paths.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathLine {
    pub row: Option<u8>,
    pub line: Vec<(i32, i32)>,
}

/// Why the source would not read: the file, the line (from 1) and what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError {
    pub file: &'static str,
    pub line: usize,
    pub what: String,
}

impl core::fmt::Display for SourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.what)
    }
}

// --- names --------------------------------------------------------------------------------------

const ROLE_NAMES: [&str; 14] = [
    "station_fields",
    "sallow_bottom",
    "top_field",
    "castle_terrace",
    "foothills",
    "museum_terrace",
    "library_terrace",
    "butterfly_hollow",
    "lake",
    "crown",
    "lip",
    "cutting",
    "plateau",
    "slagmere",
];

const BIOME_NAMES: [&str; 12] =
    ["field", "hedge", "wood", "foothill", "reed", "marsh", "wet_wood", "garden", "slag", "yard", "hill", "town"];

const fn role_name(r: Role) -> &'static str {
    ROLE_NAMES[r as usize]
}

fn gate_kind_name(k: GateKind) -> &'static str {
    match k {
        GateKind::RampRoad => "ramp",
        GateKind::Stair => "stair",
        GateKind::Span => "span",
        GateKind::Underpass => "underpass",
        GateKind::CompanyGate => "company_gate",
        GateKind::SteppingStones => "stones",
        GateKind::Verb(Verb::Repair) => "verb_repair",
        GateKind::Verb(Verb::Grow) => "verb_grow",
        GateKind::Verb(Verb::Explosion) => "verb_explosion",
        GateKind::Verb(Verb::Electric) => "verb_electric",
        GateKind::Gap => "gap",
        GateKind::Ledge => "ledge",
    }
}

const GATE_KINDS: [GateKind; 12] = [
    GateKind::RampRoad,
    GateKind::Stair,
    GateKind::Span,
    GateKind::Underpass,
    GateKind::CompanyGate,
    GateKind::SteppingStones,
    GateKind::Verb(Verb::Repair),
    GateKind::Verb(Verb::Grow),
    GateKind::Verb(Verb::Explosion),
    GateKind::Verb(Verb::Electric),
    GateKind::Gap,
    GateKind::Ledge,
];

const fn border_name(b: Border) -> &'static str {
    match b {
        Border::Open => "open",
        Border::Joined => "joined",
        Border::Wall => "wall",
    }
}

const fn span_name(k: SpanKind) -> &'static str {
    match k {
        SpanKind::Footbridge => "footbridge",
        SpanKind::Bridge => "bridge",
        SpanKind::Viaduct => "viaduct",
        SpanKind::Walkway => "walkway",
        SpanKind::Rail => "rail",
    }
}

const SPAN_KINDS: [SpanKind; 5] =
    [SpanKind::Footbridge, SpanKind::Bridge, SpanKind::Viaduct, SpanKind::Walkway, SpanKind::Rail];

const fn over_name(o: super::macro_plan::Over) -> &'static str {
    use super::macro_plan::Over;
    match o {
        Over::River => "river",
        Over::Valley => "valley",
        Over::Cutting => "cutting",
        Over::Yard => "yard",
    }
}

/// The water grid's characters, by `Wet` in order.
const WET_CHARS: [u8; 5] = [b'.', b'~', b'L', b'm', b'o'];
const WETS: [Wet; 5] = [Wet::Dry, Wet::River, Wet::Lake, Wet::Mere, Wet::Pond];

/// The district grid's character for district `d`.
const fn district_char(d: u8) -> u8 {
    b'a' + d
}

/// A name the plan holds as `&'static str`: read once, kept for the life of the program (a bake reads
/// the source once).
fn keep(s: &str) -> &'static str {
    String::from(s).leak()
}

// --- the tokenizer ------------------------------------------------------------------------------

/// A record: its first line's number and its words, continuation lines joined on.
struct Record {
    line: usize,
    words: Vec<String>,
}

fn records(text: &str) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let body = strip_comment(raw);
        if body.trim().is_empty() {
            continue;
        }
        let words = split(body);
        let cont = raw.starts_with(' ') || raw.starts_with('\t');
        match out.last_mut() {
            Some(r) if cont => r.words.extend(words),
            _ => out.push(Record { line: n + 1, words }),
        }
    }
    out
}

fn strip_comment(s: &str) -> &str {
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Words split on spaces; a quoted word keeps its spaces and loses its quotes.
fn split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s.trim_start();
    while !rest.is_empty() {
        if let Some(q) = rest.strip_prefix('"') {
            let end = q.find('"').unwrap_or(q.len());
            out.push(String::from(&q[..end]));
            rest = q.get(end + 1..).unwrap_or("").trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(String::from(&rest[..end]));
            rest = rest[end..].trim_start();
        }
    }
    out
}

/// A reader of one file's records, which says where it went wrong.
struct Reader {
    file: &'static str,
    line: usize,
}

impl Reader {
    fn err<T>(&self, what: impl Into<String>) -> Result<T, SourceError> {
        Err(SourceError { file: self.file, line: self.line, what: what.into() })
    }

    fn int<T: core::str::FromStr>(&self, w: Option<&String>, what: &str) -> Result<T, SourceError> {
        match w.and_then(|w| w.parse().ok()) {
            Some(v) => Ok(v),
            None => self.err(format!("{what}: a number, not {:?}", w.map_or("nothing", |w| w.as_str()))),
        }
    }

    fn cell(&self, w: Option<&String>, what: &str) -> Result<(i32, i32), SourceError> {
        let Some(w) = w else { return self.err(format!("{what}: a cell x,y is missing")) };
        let parsed = w.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)));
        match parsed {
            Some(c) => Ok(c),
            None => self.err(format!("{what}: a cell x,y, not {w:?}")),
        }
    }

    fn macro_cell(&self, w: Option<&String>, what: &str) -> Result<(u8, u8), SourceError> {
        let (x, y) = self.cell(w, what)?;
        if !(0..MW).contains(&x) || !(0..MH).contains(&y) {
            return self.err(format!("{what}: {x},{y} is off the {MW} x {MH} macro grid"));
        }
        Ok((x as u8, y as u8))
    }

    fn word<'w>(&self, w: Option<&'w String>, what: &str) -> Result<&'w str, SourceError> {
        match w {
            Some(w) => Ok(w),
            None => self.err(format!("{what} is missing")),
        }
    }

    fn one_of<T: Copy>(&self, w: Option<&String>, what: &str, names: &[&str], vals: &[T]) -> Result<T, SourceError> {
        let w = self.word(w, what)?;
        match names.iter().position(|n| *n == w) {
            Some(i) => Ok(vals[i]),
            None => self.err(format!("{what}: one of {}, not {w:?}", names.join(" "))),
        }
    }
}

// --- plan.txt ------------------------------------------------------------------------------------

/// The plan and the carve's salt from `plan.txt`.
pub fn parse_plan(text: &str) -> Result<(MacroPlan, u8), SourceError> {
    let mut r = Reader { file: "plan.txt", line: 0 };
    let lines: Vec<&str> = text.lines().collect();
    let (mut seed, mut attempt, mut salt, mut arch) = (None, None, None, None);
    let mut districts: Vec<District> = Vec::new();
    let (mut gates, mut sites, mut routes, mut crossings) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut river_in, mut river_out, mut fall) = (Vec::new(), Vec::new(), None);
    let mut district_of: Option<Vec<u8>> = None;
    let mut wet: Option<Vec<Wet>> = None;
    // The grids are read as raw lines, so the records stop where the first grid starts.
    let grid_at = lines.iter().position(|l| l.starts_with("grid ")).unwrap_or(lines.len());
    let head: String = lines[..grid_at].iter().flat_map(|l| [*l, "\n"]).collect();
    for rec in records(&head) {
        r.line = rec.line;
        let w = &rec.words;
        let mut it = w.iter().skip(1);
        match w[0].as_str() {
            "dice" => {
                seed = Some(r.int::<u32>(it.next(), "dice: the seed")?);
                attempt = Some(r.int::<u8>(it.next(), "dice: the attempt")?);
                salt = Some(r.int::<u8>(it.next(), "dice: the salt")?);
            }
            "arch" => arch = Some(keep(r.word(it.next(), "arch")?)),
            "river_in" | "river_out" => {
                let cells = it.map(|c| r.macro_cell(Some(c), &w[0])).collect::<Result<Vec<_>, _>>()?;
                if w[0] == "river_in" { river_in = cells } else { river_out = cells }
            }
            "fall" => {
                let c = it.next();
                fall = if c.is_some_and(|c| c == "none") { None } else { Some(r.macro_cell(c, "fall")?) };
            }
            "district" => {
                let role = r.one_of(it.next(), "district: its role", &ROLE_NAMES, &ROLES)?;
                if ROLES[districts.len()] != role {
                    return r.err(format!("district {}: the districts go in the order {}", w[1], ROLE_NAMES.join(" ")));
                }
                let level = r.int::<u8>(it.next(), "district: its level")?;
                let biome = r.one_of(it.next(), "district: its biome", &BIOME_NAMES, &Biome::ALL)?;
                let seed = r.cell(it.next(), "district: its seed")?;
                districts.push(District { role, reg: role.reg(), level, seed, area: 0, biome });
            }
            "gate" => {
                let names: Vec<&str> = GATE_KINDS.iter().map(|&k| gate_kind_name(k)).collect();
                let kind = r.one_of(it.next(), "gate: its kind", &names, &GATE_KINDS)?;
                let name = keep(r.word(it.next(), "gate: its name")?);
                let a = r.macro_cell(it.next(), "gate: its first cell")?;
                let b = r.macro_cell(it.next(), "gate: the cell across")?;
                let da = r.one_of(it.next(), "gate: its first district", &ROLE_NAMES, &ROLES)?.id();
                let db = r.one_of(it.next(), "gate: the district across", &ROLE_NAMES, &ROLES)?.id();
                let border = r.one_of(
                    it.next(),
                    "gate: its border",
                    &["open", "joined", "wall"],
                    &[Border::Open, Border::Joined, Border::Wall],
                )?;
                let road = match it.next().map(String::as_str) {
                    None => false,
                    Some("road") => true,
                    Some(o) => return r.err(format!("gate: `road` or nothing after the border, not {o:?}")),
                };
                gates.push(Gate { kind, name, a, b, da, db, road, border });
            }
            "site" => {
                let id = keep(r.word(it.next(), "site: its id")?);
                let name = keep(r.word(it.next(), "site: its name")?);
                let role = r.one_of(it.next(), "site: its district", &ROLE_NAMES, &ROLES)?;
                let pos = r.macro_cell(it.next(), "site: its cell")?;
                let dungeon = match it.next().map(String::as_str) {
                    None => false,
                    Some("dungeon") => true,
                    Some(o) => return r.err(format!("site: `dungeon` or nothing after the cell, not {o:?}")),
                };
                sites.push(Site { id, name, district: role.id(), pos, dungeon });
            }
            "route" => {
                let from = keep(r.word(it.next(), "route: where from")?);
                let to = keep(r.word(it.next(), "route: where to")?);
                let cells = r.int::<i32>(it.next(), "route: its length in cells")?;
                let pts = it.map(|c| r.macro_cell(Some(c), "route")).collect::<Result<Vec<_>, _>>()?;
                routes.push(Route { from, to, pts, cells });
            }
            "crossing" => {
                let at = r.macro_cell(it.next(), "crossing: its cell")?;
                let along_x = r.one_of(it.next(), "crossing: its axis", &["x", "y"], &[true, false])?;
                let names: Vec<&str> = SPAN_KINDS.iter().map(|&k| span_name(k)).collect();
                let kind = r.one_of(it.next(), "crossing: its kind", &names, &SPAN_KINDS)?;
                use super::macro_plan::Over;
                let overs = [Over::River, Over::Valley, Over::Cutting, Over::Yard];
                let names: Vec<&str> = overs.iter().map(|&o| over_name(o)).collect();
                let over = r.one_of(it.next(), "crossing: what it goes over", &names, &overs)?;
                let (mut route, mut gate) = (None, None);
                while let Some(k) = it.next() {
                    match k.as_str() {
                        "route" => route = Some(r.int::<u8>(it.next(), "crossing: its route")?),
                        "gate" => gate = Some(r.int::<u8>(it.next(), "crossing: its gate")?),
                        o => return r.err(format!("crossing: `route N` or `gate N`, not {o:?}")),
                    }
                }
                crossings.push(Crossing { at, along_x, kind, over, route, gate });
            }
            o => return r.err(format!("no record {o:?} in the plan")),
        }
    }
    // The grids.
    let mut k = grid_at;
    while k < lines.len() {
        r.line = k + 1;
        let Some(which) = lines[k].strip_prefix("grid ") else {
            if lines[k].trim().is_empty() || lines[k].starts_with('#') {
                k += 1;
                continue;
            }
            return r.err("a grid's rows follow `grid districts` or `grid water`");
        };
        let which = which.trim();
        let rows = lines.get(k + 1..k + 1 + MH as usize).unwrap_or(&[]);
        if rows.len() != MH as usize {
            return r.err(format!("grid {which}: {MH} rows of {MW}"));
        }
        let mut cells = Vec::with_capacity(CELLS);
        for (y, row) in rows.iter().enumerate() {
            r.line = k + 2 + y;
            if row.len() != MW as usize {
                return r.err(format!("grid {which}: a row is {MW} characters, this is {}", row.len()));
            }
            cells.extend(row.bytes());
        }
        match which {
            "districts" => {
                let mut d = Vec::with_capacity(CELLS);
                for (i, &c) in cells.iter().enumerate() {
                    if !(b'a'..b'a' + ROLES.len() as u8).contains(&c) {
                        r.line = k + 2 + i / MW as usize;
                        return r.err(format!("grid districts: a to n, not {:?}", c as char));
                    }
                    d.push(c - b'a');
                }
                district_of = Some(d);
            }
            "water" => {
                let mut wv = Vec::with_capacity(CELLS);
                for (i, &c) in cells.iter().enumerate() {
                    let Some(n) = WET_CHARS.iter().position(|&q| q == c) else {
                        r.line = k + 2 + i / MW as usize;
                        return r.err(format!("grid water: one of . ~ L m o, not {:?}", c as char));
                    };
                    wv.push(WETS[n]);
                }
                wet = Some(wv);
            }
            o => return r.err(format!("no grid {o:?}")),
        }
        k += 1 + MH as usize;
    }
    r.line = 0;
    let (Some(seed), Some(attempt), Some(salt)) = (seed, attempt, salt) else { return r.err("no `dice` line") };
    let Some(arch) = arch else { return r.err("no `arch` line") };
    if districts.len() != ROLES.len() {
        return r.err(format!("{} districts, not {}", ROLES.len(), districts.len()));
    }
    let Some(district_of) = district_of else { return r.err("no `grid districts`") };
    let Some(wet) = wet else { return r.err("no `grid water`") };
    for &d in &district_of {
        districts[usize::from(d)].area += 1;
    }
    let plan = MacroPlan {
        seed,
        attempt,
        district_of,
        districts,
        gates,
        sites,
        routes,
        arch,
        wet,
        river_in,
        river_out,
        fall,
        crossings,
    };
    Ok((plan, salt))
}

fn cells_text(out: &mut String, cells: &[(u8, u8)]) {
    for &(x, y) in cells {
        let _ = write!(out, " {x},{y}");
    }
}

/// `plan.txt` for `plan` carved on `salt`.
pub fn write_plan(plan: &MacroPlan, salt: u8) -> String {
    let mut s = String::new();
    s.push_str(PLAN_HEAD);
    let _ = writeln!(s, "dice {} {} {salt}", plan.seed, plan.attempt);
    let _ = writeln!(s, "arch {}", plan.arch);
    s.push_str("river_in");
    cells_text(&mut s, &plan.river_in);
    s.push_str("\nriver_out");
    cells_text(&mut s, &plan.river_out);
    match plan.fall {
        Some((x, y)) => {
            let _ = writeln!(s, "\nfall {x},{y}");
        }
        None => s.push_str("\nfall none\n"),
    }
    s.push('\n');
    for d in &plan.districts {
        let _ = writeln!(
            s,
            "district {} {} {} {},{}",
            role_name(d.role),
            d.level,
            BIOME_NAMES[d.biome as usize],
            d.seed.0,
            d.seed.1
        );
    }
    s.push('\n');
    for g in &plan.gates {
        let _ = writeln!(
            s,
            "gate {} \"{}\" {},{} {},{} {} {} {}{}",
            gate_kind_name(g.kind),
            g.name,
            g.a.0,
            g.a.1,
            g.b.0,
            g.b.1,
            ROLE_NAMES[usize::from(g.da)],
            ROLE_NAMES[usize::from(g.db)],
            border_name(g.border),
            if g.road { " road" } else { "" }
        );
    }
    s.push('\n');
    for t in &plan.sites {
        let _ = writeln!(
            s,
            "site {} \"{}\" {} {},{}{}",
            t.id,
            t.name,
            role_name(ROLES[usize::from(t.district)]),
            t.pos.0,
            t.pos.1,
            if t.dungeon { " dungeon" } else { "" }
        );
    }
    s.push('\n');
    for rt in &plan.routes {
        let _ = write!(s, "route {} {} {}", rt.from, rt.to, rt.cells);
        for (k, &(x, y)) in rt.pts.iter().enumerate() {
            s.push_str(if k > 0 && k % 16 == 0 { "\n   " } else { " " });
            let _ = write!(s, "{x},{y}");
        }
        s.push('\n');
    }
    s.push('\n');
    for c in &plan.crossings {
        let _ = write!(
            s,
            "crossing {},{} {} {} {}",
            c.at.0,
            c.at.1,
            if c.along_x { "x" } else { "y" },
            span_name(c.kind),
            over_name(c.over)
        );
        if let Some(r) = c.route {
            let _ = write!(s, " route {r}");
        }
        if let Some(g) = c.gate {
            let _ = write!(s, " gate {g}");
        }
        s.push('\n');
    }
    s.push_str("\ngrid districts\n");
    for y in 0..MH {
        s.extend((0..MW).map(|x| district_char(plan.district_of[idx(x, y)]) as char));
        s.push('\n');
    }
    s.push_str("grid water\n");
    for y in 0..MH {
        s.extend((0..MW).map(|x| WET_CHARS[plan.wet[idx(x, y)] as usize] as char));
        s.push('\n');
    }
    s
}

const PLAN_HEAD: &str = "\
# The county's macro plan (MAP.md 4.1): 96 x 96 macro cells of 16 x 16 cells, x east, y south.
# `jane bake-world` compiles this, places.txt and routes.txt into assets/world/county.bp.
#
# dice SEED ATTEMPT SALT: what the generated dressing (trees, scatter, the carve's own detail) throws on.
# district ROLE LEVEL BIOME SEED: in role order; the grid below says which cells are whose.
# gate KIND \"NAME\" A B DISTRICT_A DISTRICT_B BORDER [road]: A is a ledge's top, B its landing.
# site ID \"NAME\" DISTRICT CELL [dungeon]
# route FROM TO LENGTH_IN_CELLS CELLS...: the macro cells a road runs through (routes.txt has its cells).
# crossing CELL AXIS KIND OVER [route N] [gate N]
# grid districts: a district a letter, a = station_fields in the order above.
# grid water: . dry, ~ river, L lake, m mere, o pond.
";

// --- places.txt ----------------------------------------------------------------------------------

/// `places.txt`, read against the catalog's tables (a row is named by its key).
pub fn parse_places(text: &str) -> Result<Places, SourceError> {
    let cat = jane_data::catalog();
    let county = &cat.county;
    let mut r = Reader { file: "places.txt", line: 0 };
    let mut p = Places::default();
    for rec in records(text) {
        r.line = rec.line;
        let w = &rec.words;
        let mut it = w.iter().skip(1);
        match w[0].as_str() {
            "area" => {
                let key = r.word(it.next(), "area: its id")?;
                let Some(row) = county.areas.iter().position(|a| cat.name(a.id) == key) else {
                    return r.err(format!("no patch {key:?} in data/areas.json"));
                };
                p.areas.push((row as u8, macro_of(&r, it.next(), "area")?));
            }
            "poi" => {
                let key = r.word(it.next(), "poi: its kind")?;
                let Some(row) = county.pois.iter().position(|q| cat.name(q.kind) == key) else {
                    return r.err(format!("no small place {key:?} in data/pois.json"));
                };
                p.pois.push((row as u8, macro_of(&r, it.next(), "poi")?));
            }
            "anchor" => {
                let key = r.word(it.next(), "anchor: its id")?;
                let Some(row) = county.anchors.iter().position(|a| cat.name(a.id) == key) else {
                    return r.err(format!("no anchor {key:?} in data/anchors.json"));
                };
                p.anchors.push((row as u8, macro_of(&r, it.next(), "anchor")?));
            }
            "story" => {
                let key = r.word(it.next(), "story: its key")?;
                let Some(s) = county.stories.iter().find(|s| s.key == key) else {
                    return r.err(format!("no story {key:?} in data/stories"));
                };
                let at = match it.next().map(String::as_str) {
                    Some("none") => None,
                    _ => Some(r.cell(w.get(2), "story")?),
                };
                let name = it.next().cloned();
                if name.is_some() == s.name.is_some() {
                    let want = if s.name.is_some() { "none: its row names it" } else { "its place's name" };
                    return r.err(format!("story {key}: {want}"));
                }
                if p.stories.iter().any(|t| t.story == s.id) {
                    return r.err(format!("story {key} twice"));
                }
                p.stories.push(StoryAt { story: s.id, at, name });
            }
            o => return r.err(format!("no record {o:?} in places")),
        }
    }
    r.line = 0;
    for s in county.stories {
        if !p.stories.iter().any(|t| t.story == s.id) {
            return r.err(format!("story {} has no line (`story {} none` if it is not placed)", s.key, s.key));
        }
    }
    p.stories.sort_by_key(|t| county.stories.iter().position(|s| s.id == t.story));
    p.areas.sort_by_key(|&(row, _)| row);
    Ok(p)
}

fn macro_of(r: &Reader, w: Option<&String>, what: &str) -> Result<(i32, i32), SourceError> {
    r.macro_cell(w, what).map(|(x, y)| (i32::from(x), i32::from(y)))
}

/// `places.txt` for `p`.
pub fn write_places(p: &Places) -> String {
    let cat = jane_data::catalog();
    let county = &cat.county;
    let mut s = String::from(PLACES_HEAD);
    s.push_str("\n# Named patches: the area table's id, the macro cell at its centre.\n");
    for &(row, (x, y)) in &p.areas {
        let _ = writeln!(s, "area {} {x},{y}", cat.name(county.areas[usize::from(row)].id));
    }
    s.push_str("\n# Small places nobody asked for by name: a kind from data/pois.json, a macro cell.\n");
    for &(row, (x, y)) in &p.pois {
        let _ = writeln!(s, "poi {} {x},{y}", cat.name(county.pois[usize::from(row)].kind));
    }
    s.push_str("\n# Small places the story needs: an id from data/anchors.json, a macro cell.\n");
    for &(row, (x, y)) in &p.anchors {
        let _ = writeln!(s, "anchor {} {x},{y}", cat.name(county.anchors[usize::from(row)].id));
    }
    s.push_str("\n# Stories: the key, a cell inside the country place it claims (none: not placed), and the\n");
    s.push_str("# place's name when its row gives none.\n");
    for t in &p.stories {
        let key = county.story(t.story).key;
        match t.at {
            Some((x, y)) => {
                let _ = write!(s, "story {key} {x},{y}");
            }
            None => {
                let _ = write!(s, "story {key} none");
            }
        }
        if let Some(n) = &t.name {
            let _ = write!(s, " \"{n}\"");
        }
        s.push('\n');
    }
    s
}

const PLACES_HEAD: &str = "\
# Where the county's things stand. `jane bake-world` puts each where this says and searches for nothing;
# a place on a face, in water or out of reach stops the bake. Macro cells are 16 cells (plan.txt);
# a story's cell is a cell (the map sheet's).
";

// --- routes.txt ----------------------------------------------------------------------------------

/// `routes.txt`; `plan` and `paths` are what its roads and paths are checked against.
pub fn parse_routes(text: &str, plan: &MacroPlan) -> Result<Routes, SourceError> {
    let paths = jane_data::catalog().county.paths;
    let mut r = Reader { file: "routes.txt", line: 0 };
    let mut out = Routes::default();
    for rec in records(text) {
        r.line = rec.line;
        let w = &rec.words;
        match w[0].as_str() {
            "road" => {
                let k = out.roads.len();
                let (from, to) = (r.word(w.get(1), "road: where from")?, r.word(w.get(2), "road: where to")?);
                match plan.routes.get(k) {
                    Some(rt) if rt.from == from && rt.to == to => {}
                    Some(rt) => return r.err(format!("road {}: the plan's route {k} is {} {}", k, rt.from, rt.to)),
                    None => return r.err(format!("road {k}: the plan has {} routes", plan.routes.len())),
                }
                out.roads.push(polyline(&r, &w[3..])?);
            }
            "path" => {
                let id = r.word(w.get(1), "path: its id")?;
                let row = match id {
                    "burial" => None,
                    _ => match paths.iter().position(|p| p.id == id) {
                        Some(i) => Some(i as u8),
                        None => return r.err(format!("no footpath {id:?} in data/paths.json")),
                    },
                };
                out.paths.push(PathLine { row, line: polyline(&r, &w[2..])? });
            }
            "lit" => {
                let y = r.int::<i32>(w.get(1), "lit: its row")?;
                for run in &w[2..] {
                    let (a, b) = run.split_once('-').unwrap_or((run, run));
                    let (Ok(a), Ok(b)) = (a.parse::<i32>(), b.parse::<i32>()) else {
                        return r.err(format!("lit: a run X or X-X, not {run:?}"));
                    };
                    if !(0..MH).contains(&y) || a < 0 || b < a || b >= MW {
                        return r.err(format!("lit: {y}: {run} is off the macro grid"));
                    }
                    out.lit.extend((a..=b).map(|x| (x, y)));
                }
            }
            o => return r.err(format!("no record {o:?} in routes")),
        }
    }
    r.line = 0;
    if out.roads.len() != plan.routes.len() {
        return r.err(format!("{} roads for the plan's {} routes", out.roads.len(), plan.routes.len()));
    }
    Ok(out)
}

/// A polyline: each `x,y` is reached from the last in steps of one cell (straight or diagonal), each `^x,y`
/// in one jump (over a deck, or where a path took up again).
fn polyline(r: &Reader, words: &[String]) -> Result<Vec<(i32, i32)>, SourceError> {
    let mut line: Vec<(i32, i32)> = Vec::new();
    for w in words {
        let (jump, c) = match w.strip_prefix('^') {
            Some(c) => (true, c),
            None => (false, w.as_str()),
        };
        let (x, y) = r.cell(Some(&String::from(c)), "a line")?;
        match line.last().copied() {
            Some((px, py)) if !jump => {
                let (dx, dy) = (x - px, y - py);
                if !(dx == 0 || dy == 0 || dx.abs() == dy.abs()) || (dx, dy) == (0, 0) {
                    return r.err(format!("{px},{py} to {x},{y} is not a straight or diagonal run"));
                }
                let n = dx.abs().max(dy.abs());
                line.extend((1..=n).map(|k| (px + dx.signum() * k, py + dy.signum() * k)));
            }
            _ => line.push((x, y)),
        }
    }
    Ok(line)
}

/// A line's polyline words: a vertex where the step changes, `^` before a jump.
fn polyline_words(line: &[(i32, i32)]) -> Vec<String> {
    let mut out = Vec::new();
    let Some(&first) = line.first() else { return out };
    out.push(format!("{},{}", first.0, first.1));
    let unit = |a: (i32, i32), b: (i32, i32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        (dx, dy) != (0, 0) && dx.abs() <= 1 && dy.abs() <= 1
    };
    let mut k = 1;
    while k < line.len() {
        let (a, b) = (line[k - 1], line[k]);
        if !unit(a, b) {
            out.push(format!("^{},{}", b.0, b.1));
            k += 1;
            continue;
        }
        let d = (b.0 - a.0, b.1 - a.1);
        let mut end = k;
        while end + 1 < line.len() && (line[end + 1].0 - line[end].0, line[end + 1].1 - line[end].1) == d {
            end += 1;
        }
        out.push(format!("{},{}", line[end].0, line[end].1));
        k = end + 1;
    }
    out
}

fn write_line(s: &mut String, head: &str, line: &[(i32, i32)]) {
    s.push_str(head);
    for (k, w) in polyline_words(line).iter().enumerate() {
        s.push_str(if k % 12 == 0 { "\n  " } else { " " });
        s.push_str(w);
    }
    s.push('\n');
}

/// `routes.txt` for `routes`, its roads named by `plan`'s routes.
pub fn write_routes(routes: &Routes, plan: &MacroPlan) -> String {
    let paths = jane_data::catalog().county.paths;
    let mut s = String::from(ROUTES_HEAD);
    for (k, line) in routes.roads.iter().enumerate() {
        let rt = &plan.routes[k];
        write_line(&mut s, &format!("\nroad {} {}", rt.from, rt.to), line);
    }
    for p in &routes.paths {
        let id = p.row.map_or("burial", |i| paths[usize::from(i)].id);
        write_line(&mut s, &format!("\npath {id}"), &p.line);
    }
    s.push_str("\n# Lit road: a macro row, then the runs of macro cells along it whose lamps burn.\n");
    let mut lit = routes.lit.clone();
    lit.sort_by_key(|&(x, y)| (y, x));
    lit.dedup();
    let mut k = 0;
    while k < lit.len() {
        let y = lit[k].1;
        let _ = write!(s, "lit {y}");
        while k < lit.len() && lit[k].1 == y {
            let x0 = lit[k].0;
            let mut x1 = x0;
            while k + 1 < lit.len() && lit[k + 1] == (x1 + 1, y) {
                k += 1;
                x1 += 1;
            }
            k += 1;
            if x1 == x0 {
                let _ = write!(s, " {x0}");
            } else {
                let _ = write!(s, " {x0}-{x1}");
            }
        }
        s.push('\n');
    }
    s
}

const ROUTES_HEAD: &str = "\
# The roads and footpaths, as lines of cells. A road's metal and verge are laid along its line, a path's
# trodden dirt along its. Each x,y is reached from the one before in steps of a cell, straight or diagonal;
# ^x,y is a jump (a deck crossed, a path taken up again). Roads go in plan.txt's route order.
";

/// The county's source from its three files.
pub fn parse(plan: &str, places: &str, routes: &str) -> Result<Source, SourceError> {
    let (plan, salt) = parse_plan(plan)?;
    let places = parse_places(places)?;
    let routes = parse_routes(routes, &plan)?;
    Ok(Source { plan, salt, places, routes })
}

/// The name `id`'s place goes by in `places`, if its own row gives none.
pub fn story_name<'p>(places: &'p Places, id: StoryId) -> Option<&'p str> {
    places.stories.iter().find(|t| t.story == id).and_then(|t| t.name.as_deref())
}

/// Each story's place name the source gives, by story.
pub fn story_names(places: &Places) -> Vec<(StoryId, String)> {
    places.stories.iter().filter_map(|t| t.name.as_ref().map(|n| (t.story, n.to_owned()))).collect()
}
