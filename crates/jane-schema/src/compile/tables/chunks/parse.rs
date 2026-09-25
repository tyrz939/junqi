//! `.chunk` files: the county's authored places as data (PORT.md §6.f). The parser; the lint that
//! needs the grid laid out is `lint.rs`.
//!
//! # The format
//!
//! A `.chunk` is a `.room` with a chunk's header, several layers of grid, and a legend. Every
//! coordinate is in **box cells**: `(0, 0)` is the box's top-left, `x` runs east, `y` south. A
//! chunk never names a cell outside its box: gates (one step outside it) and `around` rects (the
//! box grown) are the only ways it reaches past it, and both are header fields, not cells.
//!
//! ```text
//! // Comment lines start with // (header and legend only).
//! id      station                       the site it stands at (sites.json), and the file's stem
//! box     36x34                         width x height, cells
//! anchor  centre                        the box cell on the site's origin: `centre` (w/2, h/2) or `X Y`
//! pin     w 4                           optional: this side of the box stands at this county coordinate
//! face    e                             the side her way in looks out of; some gate is on it
//! gates   e17, w15, s8                  side and cell along it: a walkable cell one step outside the box
//! slots   night_parcel 5 12, ...        optional: named exact cells inside the box (a door's hole)
//! around  halt_approach n30 e30 s30 w4  optional, repeatable: a rect of the box grown by n e s w cells
//! grid                                  the ground: one tile character per cell, every cell
//! ,,,,!!!...
//! things                                optional: props (a def-sized block of one character) and units
//! ..LL..t.
//! names                                 optional: marks (one cell), rects (top-left and bottom-right
//! ...s....                                corner, or one cell for 1x1) and fill regions (any cells)
//! claims                                optional: `x` for a cell nothing placed later by name may take
//! ..xxx...
//! legend
//! , tile Grass
//! L prop lamp_post                              anonymous, repeatable: every block is one
//! T prop sign timetable {"talk": "timetable"}   keyed: drawn once; JSON is PropSpawn's other fields
//! h unit town_hen - south                       anonymous (`-`), repeatable, facing optional
//! t unit town_traveller traveller east [[3,4],[9,4,600]]   keyed; a route of waypoints (x, y, dwell)
//! s mark start east                             drawn once; facing optional
//! p rect platform                               drawn at two corners (or once, for a 1x1 rect)
//! c fill graveyard_coffins                      a region a `data/tuning/chunks.json` fill places in
//! ```
//!
//! Layers come in that order (`grid`, then any of `things`, `names`, `claims`), each exactly `box`
//! in size; `.` is empty in every layer but `grid` and may not be defined. A legend character means
//! one thing in the whole file; tiles go in `grid`, props and units in `things`, marks, rects and
//! fills in `names`. Grid characters may be any non-space character (the town needs more than ASCII
//! has); a row's width is counted in characters. A prop's JSON takes `locked`, `keyTag`, `hidden`,
//! `on`, `to` (`{"zone", "mark"}`), `loot`, `use`, `release`, `needs`, `talk`, `label`, `nightLock`,
//! and nothing else. A `.chunk` is a pure grid plus its fills: it carries no procedural op.
//!
//! Like `room.rs`, the parser stops at the first error; the lint reports them all.

use jane_core::action::Facing;

/// An error: the 1-based line it is on (0 when it is about the whole file), and what is wrong.
pub type LineError = (usize, String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    N,
    E,
    S,
    W,
}

impl Side {
    pub fn from_char(c: char) -> Option<Side> {
        match c {
            'n' => Some(Side::N),
            'e' => Some(Side::E),
            's' => Some(Side::S),
            'w' => Some(Side::W),
            _ => None,
        }
    }

    pub fn letter(self) -> char {
        match self {
            Side::N => 'n',
            Side::E => 'e',
            Side::S => 's',
            Side::W => 'w',
        }
    }
}

pub fn facing_named(s: &str) -> Option<Facing> {
    match s {
        "east" => Some(Facing::East),
        "south" => Some(Facing::South),
        "west" => Some(Facing::West),
        "north" => Some(Facing::North),
        _ => None,
    }
}

/// A patrol waypoint: box cell and ticks to stand there.
pub type Waypoint = (i32, i32, Option<u32>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Tile(String),
    /// `extra` is the JSON object, unparsed: the compile types it.
    Prop {
        def: String,
        key: Option<String>,
        extra: Option<String>,
    },
    Unit {
        def: String,
        key: Option<String>,
        facing: Option<Facing>,
        route: Vec<Waypoint>,
    },
    Mark {
        name: String,
        facing: Option<Facing>,
    },
    Rect(String),
    Fill(String),
}

impl Entry {
    pub fn kind(&self) -> &'static str {
        match self {
            Entry::Tile(_) => "tile",
            Entry::Prop { .. } => "prop",
            Entry::Unit { .. } => "unit",
            Entry::Mark { .. } => "mark",
            Entry::Rect(_) => "rect",
            Entry::Fill(_) => "fill",
        }
    }

    /// The layer an entry is drawn in.
    pub fn layer(&self) -> Layer {
        match self {
            Entry::Tile(_) => Layer::Grid,
            Entry::Prop { .. } | Entry::Unit { .. } => Layer::Things,
            Entry::Mark { .. } | Entry::Rect(_) | Entry::Fill(_) => Layer::Names,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Legend {
    pub ch: char,
    pub entry: Entry,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Grid,
    Things,
    Names,
    Claims,
}

impl Layer {
    pub const ALL: [Layer; 4] = [Layer::Grid, Layer::Things, Layer::Names, Layer::Claims];

    pub fn word(self) -> &'static str {
        match self {
            Layer::Grid => "grid",
            Layer::Things => "things",
            Layer::Names => "names",
            Layer::Claims => "claims",
        }
    }
}

/// One layer's rows, and the line each came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rows {
    pub rows: Vec<Vec<char>>,
    pub lines: Vec<usize>,
}

impl Rows {
    pub fn at(&self, x: i32, y: i32) -> char {
        self.rows[y as usize][x as usize]
    }

    /// The line of row `y` (clamped), for errors about a cell.
    pub fn line(&self, y: i32) -> usize {
        let y = y.clamp(0, self.lines.len() as i32 - 1);
        self.lines.get(y as usize).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub line: usize,
}

/// A parsed chunk, with the lines things came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// `chunks/station.chunk`.
    pub file: String,
    pub id: String,
    pub w: i32,
    pub h: i32,
    /// The box cell on the site's origin.
    pub anchor: (i32, i32),
    pub pin: Option<(Side, i32)>,
    pub face: Side,
    pub gates: Vec<(Side, i32)>,
    pub slots: Vec<(Named, i32, i32)>,
    /// Margins north, east, south, west.
    pub around: Vec<(Named, [i32; 4])>,
    /// Indexed by `Layer as usize`; `grid` is always there, an absent layer is empty rows of `.`.
    pub layers: [Rows; 4],
    pub has: [bool; 4],
    pub legend: Vec<Legend>,
    /// The line of each header key's first occurrence.
    pub head_lines: Vec<(String, usize)>,
}

impl Chunk {
    pub fn layer(&self, l: Layer) -> &Rows {
        &self.layers[l as usize]
    }

    pub fn entry(&self, ch: char) -> Option<&Legend> {
        self.legend.iter().find(|l| l.ch == ch)
    }

    pub fn head_line(&self, key: &str) -> usize {
        self.head_lines.iter().find(|(k, _)| k == key).map_or(0, |(_, l)| *l)
    }
}

/// Header keys; `around` may repeat, the others may not.
pub const HEADER_KEYS: [&str; 8] = ["id", "box", "anchor", "pin", "face", "gates", "slots", "around"];

fn digits(s: &str) -> Option<i32> {
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `^(\d+)x(\d+)$`.
fn size(s: &str) -> Option<(i32, i32)> {
    let (a, b) = s.split_once('x')?;
    Some((digits(a)?, digits(b)?))
}

/// `e17`: a side letter and a number.
fn side_num(s: &str) -> Option<(Side, i32)> {
    let mut c = s.chars();
    let side = Side::from_char(c.next()?)?;
    Some((side, digits(c.as_str())?))
}

/// The first word of `s` and the rest, trimmed.
fn word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (&s[..end], s[end..].trim())
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn route(json: &str, line: usize) -> Result<Vec<Waypoint>, LineError> {
    let bad = |why: String| Err((line, format!("route {json}: {why}")));
    let raw: Vec<Vec<i64>> = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(e) => return bad(format!("not a list of [x, y] or [x, y, ticks]: {e}")),
    };
    let mut out = Vec::new();
    for p in raw {
        let fit = |v: i64| i32::try_from(v).ok();
        match p.as_slice() {
            [x, y] => out.push((fit(*x).unwrap_or(-1), fit(*y).unwrap_or(-1), None)),
            [x, y, t] => match u32::try_from(*t) {
                Ok(t) => out.push((fit(*x).unwrap_or(-1), fit(*y).unwrap_or(-1), Some(t))),
                Err(_) => return bad(format!("{t} ticks")),
            },
            _ => return bad(format!("{p:?} is not [x, y] or [x, y, ticks]")),
        }
    }
    if out.is_empty() {
        return bad("empty: leave it out".into());
    }
    Ok(out)
}

fn legend_entry(kind: &str, rest: &str, line: usize) -> Result<Entry, LineError> {
    let fail = |msg: String| Err((line, msg));
    let (first, after) = word(rest);
    let need_name = |what: &str, s: &str| -> Result<String, LineError> {
        if is_name(s) { Ok(s.to_owned()) } else { Err((line, format!("{kind}: \"{s}\" is not a {what}"))) }
    };
    let optional_key = |s: &str| -> Result<Option<String>, LineError> {
        if s == "-" { Ok(None) } else { need_name("key (or -)", s).map(Some) }
    };
    match kind {
        "tile" => {
            if !after.is_empty() {
                return fail(format!("tile {first}: one tile name, then nothing"));
            }
            Ok(Entry::Tile(need_name("tile name", first)?))
        }
        "prop" => {
            let def = need_name("prop def", first)?;
            let (key, extra) = if after.is_empty() {
                (None, None)
            } else if after.starts_with('{') {
                (None, Some(after.to_owned()))
            } else {
                let (k, more) = word(after);
                let extra = if more.is_empty() {
                    None
                } else if more.starts_with('{') {
                    Some(more.to_owned())
                } else {
                    return fail(format!("prop {def}: after the key, a JSON object or nothing, not \"{more}\""));
                };
                (optional_key(k)?, extra)
            };
            Ok(Entry::Prop { def, key, extra })
        }
        "unit" => {
            let def = need_name("unit def", first)?;
            let mut rest = after;
            let mut key = None;
            let mut facing = None;
            let mut path = Vec::new();
            if !rest.is_empty() && !rest.starts_with('[') {
                let (k, more) = word(rest);
                key = optional_key(k)?;
                rest = more;
            }
            if !rest.is_empty() && !rest.starts_with('[') {
                let (f, more) = word(rest);
                facing = Some(facing_named(f).ok_or((line, format!("unit {def}: \"{f}\" is not a facing")))?);
                rest = more;
            }
            if !rest.is_empty() {
                path = route(rest, line)?;
            }
            Ok(Entry::Unit { def, key, facing, route: path })
        }
        "mark" => {
            let name = need_name("mark name", first)?;
            let facing = match after {
                "" => None,
                f => Some(facing_named(f).ok_or((line, format!("mark {name}: \"{f}\" is not a facing")))?),
            };
            Ok(Entry::Mark { name, facing })
        }
        "rect" | "fill" => {
            if !after.is_empty() {
                return fail(format!("{kind} {first}: one name, then nothing"));
            }
            let n = need_name(if kind == "rect" { "rect name" } else { "fill id" }, first)?;
            Ok(if kind == "rect" { Entry::Rect(n) } else { Entry::Fill(n) })
        }
        _ => fail(format!("unknown legend kind \"{kind}\" (tile, prop, unit, mark, rect, fill)")),
    }
}

pub fn parse(text: &str, file: &str) -> Result<Chunk, LineError> {
    let fail = |line: usize, msg: String| -> Result<Chunk, LineError> { Err((line, msg)) };
    let text = text.replace('\r', "");
    let mut head: Vec<(String, String, usize)> = Vec::new();
    let mut layers: [Rows; 4] = Default::default();
    let mut has = [false; 4];
    let mut legend_lines: Vec<(String, usize)> = Vec::new();
    // None: the header; Some(layer); the legend is `in_legend`.
    let mut part: Option<Layer> = None;
    let mut in_legend = false;
    for (n, raw) in text.split('\n').enumerate() {
        let lineno = n + 1;
        let t = raw.trim();
        let section = Layer::ALL.into_iter().find(|l| l.word() == t);
        if in_legend {
            if !t.is_empty() && !t.starts_with("//") {
                legend_lines.push((t.to_owned(), lineno));
            }
            continue;
        }
        if t == "legend" {
            if part.is_none() {
                return fail(lineno, "legend before any grid".into());
            }
            in_legend = true;
            continue;
        }
        if let Some(l) = section {
            let order_ok = match part {
                None => l == Layer::Grid,
                Some(p) => l > p,
            };
            if !order_ok {
                return fail(lineno, format!("\"{}\" out of order: grid, then things, names, claims", l.word()));
            }
            part = Some(l);
            has[l as usize] = true;
            continue;
        }
        match part {
            None => {
                if t.is_empty() || t.starts_with("//") {
                    continue;
                }
                let (k, v) = word(t);
                if v.is_empty() {
                    return fail(lineno, format!("cannot read header line \"{t}\""));
                }
                head.push((k.to_owned(), v.to_owned(), lineno));
            }
            Some(l) => {
                if !t.is_empty() {
                    let rows = &mut layers[l as usize];
                    rows.rows.push(t.chars().collect());
                    rows.lines.push(lineno);
                }
            }
        }
    }
    if !in_legend {
        return fail(0, "no legend".into());
    }

    // The header.
    for (k, _, l) in &head {
        if !HEADER_KEYS.contains(&k.as_str()) {
            return fail(*l, format!("unknown header key \"{k}\""));
        }
        if k != "around" && head.iter().filter(|(h, _, _)| h == k).count() > 1 {
            return fail(*l, format!("\"{k}\" is given twice"));
        }
    }
    let get = |key: &str| head.iter().find(|(k, _, _)| k == key).map(|(_, v, l)| (v.as_str(), *l));
    let need = |key: &str| get(key).ok_or((0, format!("no \"{key}\" line")));
    let (id, id_line) = need("id")?;
    if !is_name(id) {
        return fail(id_line, format!("id \"{id}\" is not a name"));
    }
    let (box_s, box_line) = need("box")?;
    let Some((w, h)) = size(box_s).filter(|(w, h)| *w >= 1 && *h >= 1) else {
        return fail(box_line, format!("box \"{box_s}\" is not WxH"));
    };
    let anchor = match get("anchor") {
        None | Some(("centre", _)) => (w / 2, h / 2),
        Some((v, l)) => match v.split_whitespace().map(digits).collect::<Option<Vec<i32>>>().as_deref() {
            Some([x, y]) => (*x, *y),
            _ => return fail(l, format!("anchor \"{v}\" is not `centre` or `X Y`")),
        },
    };
    let pin = match get("pin") {
        None => None,
        Some((v, l)) => {
            let (s, at) = word(v);
            let side = s.chars().next().filter(|_| s.len() == 1).and_then(Side::from_char);
            match (side, digits(at)) {
                (Some(side), Some(at)) => Some((side, at)),
                _ => return fail(l, format!("pin \"{v}\" is not a side and a county cell (`w 4`)")),
            }
        }
    };
    let (face_s, face_line) = need("face")?;
    let Some(face) = face_s.chars().next().filter(|_| face_s.len() == 1).and_then(Side::from_char) else {
        return fail(face_line, format!("face \"{face_s}\" is not n, e, s or w"));
    };
    let (gates_s, gates_line) = need("gates")?;
    let mut gates = Vec::new();
    for g in gates_s.split(',').map(str::trim) {
        match side_num(g) {
            Some(x) => gates.push(x),
            None => return fail(gates_line, format!("cannot read gate \"{g}\" (a side and a cell along it: `e17`)")),
        }
    }
    let mut slots = Vec::new();
    if let Some((v, l)) = get("slots") {
        for s in v.split(',').map(str::trim) {
            let p: Vec<&str> = s.split_whitespace().collect();
            match (p.as_slice(), p.get(1).and_then(|x| digits(x)), p.get(2).and_then(|y| digits(y))) {
                ([n, _, _], Some(x), Some(y)) if is_name(n) => {
                    slots.push((Named { name: (*n).to_owned(), line: l }, x, y));
                }
                _ => return fail(l, format!("cannot read slot \"{s}\" (a name, x and y)")),
            }
        }
    }
    let mut around = Vec::new();
    for (k, v, l) in &head {
        if k != "around" {
            continue;
        }
        let p: Vec<&str> = v.split_whitespace().collect();
        let mut m = [0; 4];
        let mut seen = [false; 4];
        let ok = p.len() == 5
            && is_name(p[0])
            && p[1..].iter().all(|s| match side_num(s) {
                Some((side, n)) => {
                    let i = side as usize;
                    let fresh = !seen[i];
                    seen[i] = true;
                    m[i] = n;
                    fresh
                }
                None => false,
            });
        if !ok {
            return fail(
                *l,
                format!("cannot read around \"{v}\" (a name, then n, e, s and w margins: `n30 e30 s30 w4`)"),
            );
        }
        around.push((Named { name: p[0].to_owned(), line: *l }, m));
    }

    // The layers: each the box's size.
    if !has[Layer::Grid as usize] {
        return fail(0, "no grid".into());
    }
    for l in Layer::ALL {
        let rows = &mut layers[l as usize];
        if !has[l as usize] {
            *rows = Rows { rows: vec![vec!['.'; w as usize]; h as usize], lines: vec![0; h as usize] };
            continue;
        }
        if rows.rows.len() != h as usize {
            let at = rows.lines.last().copied().unwrap_or(0);
            return fail(at, format!("{} has {} rows, the box is {h} high", l.word(), rows.rows.len()));
        }
        for (y, r) in rows.rows.iter().enumerate() {
            if r.len() != w as usize {
                return fail(rows.lines[y], format!("{} row {y} is {} wide, the box is {w}", l.word(), r.len()));
            }
        }
    }

    // The legend, in file order.
    let mut legend: Vec<Legend> = Vec::new();
    for (l, lineno) in &legend_lines {
        let lineno = *lineno;
        let mut chars = l.chars();
        let ch = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        if !rest.starts_with(char::is_whitespace) {
            return fail(lineno, format!("cannot read legend line \"{l}\""));
        }
        if ch == '.' {
            return fail(lineno, "legend may not define \".\": it is empty ground in every layer".into());
        }
        if legend.iter().any(|e| e.ch == ch) {
            return fail(lineno, format!("legend defines \"{ch}\" twice"));
        }
        let (kind, rest) = word(rest);
        legend.push(Legend { ch, entry: legend_entry(kind, rest, lineno)?, line: lineno });
    }

    let mut head_lines: Vec<(String, usize)> = Vec::new();
    for (k, _, l) in &head {
        if !head_lines.iter().any(|(h, _)| h == k) {
            head_lines.push((k.clone(), *l));
        }
    }
    Ok(Chunk {
        file: file.to_owned(),
        id: id.to_owned(),
        w,
        h,
        anchor,
        pin,
        face,
        gates,
        slots,
        around,
        layers,
        has,
        legend,
        head_lines,
    })
}
