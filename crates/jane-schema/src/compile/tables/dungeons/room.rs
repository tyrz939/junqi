//! `.room` files (`jane/src/world/dungeon/room.ts`): the parser, turn and mirror, and the lint.
//!
//! The parser accepts exactly what `parseRoom` accepts, with two exceptions forced by an
//! integer, byte-indexed model: `heat` must be a whole number (the TypeScript took `Number()`
//! of anything, NaN included), and grid characters must be ASCII. Like `parseRoom`, it stops
//! at the first error; the lint (`lintRoom`, rules 1-6 and the few the mine added) reports
//! them all. Every error names the file and line.

use std::collections::BTreeSet;

use crate::model::{BAY_H, BAY_W, MOUTH_X, MOUTH_Y, RoomCoop, RoomSide, RoomSocketKind};

/// A room's rim stays inside these cells of its bay, which keeps a wall between it and any corridor.
const RIM_LO: i32 = 3;
const RIM_HI_X: i32 = 33;
const RIM_HI_Y: i32 = 25;

/// An error: the 1-based line it is on (0 when it is about the whole file), and what is wrong.
pub type LineError = (usize, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Door {
    pub id: String,
    pub side: RoomSide,
    /// 0-based bay along its side.
    pub bay: i32,
    pub required: bool,
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Socket {
    pub id: String,
    pub kind: RoomSocketKind,
    pub cx: i32,
    pub cy: i32,
    pub w: i32,
    pub h: i32,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub id: String,
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TRect {
    pub id: String,
    pub cx: i32,
    pub cy: i32,
    pub w: i32,
    pub h: i32,
    /// The header line (0 for `room`).
    pub line: usize,
}

/// A parsed template, with the lines things came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    /// `rooms/mine/plate.a.room`.
    pub file: String,
    pub id: String,
    pub pool: String,
    pub bays: (i32, i32),
    pub turns: Vec<i32>,
    pub mirror: bool,
    pub doors: Vec<Door>,
    pub sockets: Vec<Socket>,
    pub marks: Vec<Mark>,
    pub rects: Vec<TRect>,
    pub needs_verbs: Vec<String>,
    pub needs_items: Vec<(String, u16)>,
    pub grants: Vec<String>,
    pub blocks: Vec<(String, String)>,
    pub coop: Vec<RoomCoop>,
    pub heat_max: u16,
    /// Rows of ASCII bytes, the rim included.
    pub grid: Vec<Vec<u8>>,
    /// Legend tiles in the order the grid first shows them: character, tile name.
    pub tiles: Vec<(u8, String)>,
    /// The line of each grid row.
    pub row_lines: Vec<usize>,
    /// The line of each header key's first occurrence.
    pub head_lines: Vec<(String, usize)>,
}

impl Room {
    pub fn gw(&self) -> i32 {
        self.grid[0].len() as i32
    }

    pub fn gh(&self) -> i32 {
        self.grid.len() as i32
    }

    /// The line of grid row `y` (clamped), for errors about a cell.
    pub fn row_line(&self, y: i32) -> usize {
        let y = y.clamp(0, self.gh() - 1) as usize;
        self.row_lines[y]
    }

    pub fn head_line(&self, key: &str) -> usize {
        self.head_lines.iter().find(|(k, _)| k == key).map_or(0, |(_, l)| *l)
    }

    pub fn tile_of(&self, ch: u8) -> Option<&str> {
        self.tiles.iter().find(|(c, _)| *c == ch).map(|(_, t)| t.as_str())
    }
}

/// Does something stand here that feet cannot cross? Dressing is solid unless it is a torch on the wall.
pub fn socket_is_solid(s: &Socket) -> bool {
    match s.kind {
        RoomSocketKind::Plate
        | RoomSocketKind::Page
        | RoomSocketKind::Spawn
        | RoomSocketKind::Boss
        | RoomSocketKind::Unit => false,
        RoomSocketKind::Dress => !s.id.starts_with("dress:torch"),
        _ => true,
    }
}

// --- parse -------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Legend {
    Door(String),
    Tile(String),
    Mark(String),
    Socket { kind: RoomSocketKind, name: Option<String>, w: i32, h: i32, options: Vec<String> },
}

/// `^(\w+)\s+(.*)$` on a trimmed line.
fn header_line(line: &str) -> Option<(&str, &str)> {
    let end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(line.len());
    if end == 0 {
        return None;
    }
    let rest = &line[end..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((&line[..end], rest.trim()))
}

fn digits(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `^(\d+)x(\d+)$`.
fn size(s: &str) -> Option<(i32, i32)> {
    let (a, b) = s.split_once('x')?;
    Some((digits(a)?, digits(b)?))
}

/// Split on runs of whitespace, as `split(/\s+/)` does on a trimmed string.
fn words(s: &str) -> Vec<&str> {
    let v: Vec<&str> = s.split_whitespace().collect();
    if v.is_empty() { vec![""] } else { v }
}

pub fn parse(text: &str, file: &str) -> Result<Room, LineError> {
    let fail = |line: usize, msg: String| -> Result<Room, LineError> { Err((line, msg)) };
    let text = text.replace('\r', "");
    let mut head: Vec<(String, String, usize)> = Vec::new();
    let mut grid: Vec<(String, usize)> = Vec::new();
    let mut legend_lines: Vec<(String, usize)> = Vec::new();
    let mut part = 0; // 0 head, 1 grid, 2 legend
    for (n, raw) in text.split('\n').enumerate() {
        let lineno = n + 1;
        let line = raw.trim_end();
        let t = line.trim();
        match part {
            0 => {
                if t.is_empty() || t.starts_with("//") {
                    continue;
                }
                if t == "grid" {
                    part = 1;
                    continue;
                }
                match header_line(t) {
                    Some((k, v)) => head.push((k.to_owned(), v.to_owned(), lineno)),
                    None => return fail(lineno, format!("cannot read header line \"{line}\"")),
                }
            }
            1 => {
                if t == "legend" {
                    part = 2;
                } else if !t.is_empty() {
                    grid.push((line.to_owned(), lineno));
                }
            }
            _ => {
                if !t.is_empty() && !t.starts_with("//") {
                    legend_lines.push((t.to_owned(), lineno));
                }
            }
        }
    }
    let all = |key: &'static str| head.iter().filter(move |(k, _, _)| k == key);
    let one = |key: &'static str| -> Result<(String, usize), LineError> {
        all(key).next().map(|(_, v, l)| (v.clone(), *l)).ok_or((0, format!("no \"{key}\" line")))
    };
    if grid.len() < 5 {
        return fail(0, "no grid".into());
    }
    if let Some((_, l)) = grid.iter().find(|(r, _)| !r.is_ascii()) {
        return fail(*l, "grid characters must be ASCII".into());
    }
    let gw = grid[0].0.len();
    for (y, (row, l)) in grid.iter().enumerate() {
        if row.len() != gw {
            return fail(*l, format!("grid row {y} is {} wide, row 0 is {gw}", row.len()));
        }
    }
    let rows: Vec<Vec<u8>> = grid.iter().map(|(r, _)| r.as_bytes().to_vec()).collect();
    let row_lines: Vec<usize> = grid.iter().map(|(_, l)| *l).collect();
    let gh = rows.len();

    // The legend, in insertion order.
    let mut legend: Vec<(u8, Legend, usize)> = Vec::new();
    for (l, lineno) in &legend_lines {
        let lineno = *lineno;
        let mut chars = l.chars();
        let ch = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        if ch.is_whitespace() || !rest.starts_with(char::is_whitespace) {
            return fail(lineno, format!("cannot read legend line \"{l}\""));
        }
        if !ch.is_ascii() {
            return fail(lineno, format!("legend \"{ch}\": grid characters must be ASCII"));
        }
        let ch = ch as u8;
        if ch == b'#' || ch == b'.' || ch == b'_' {
            return fail(lineno, format!("legend may not redefine \"{}\"", ch as char));
        }
        if legend.iter().any(|(c, _, _)| *c == ch) {
            return fail(lineno, format!("legend defines \"{}\" twice", ch as char));
        }
        let w = words(rest.trim());
        let second = |what: &str| -> Result<String, LineError> {
            w.get(1).map(|s| (*s).to_owned()).ok_or((
                lineno,
                format!("{what} \"{}\" has no {}", ch as char, if what == "tile" { "name" } else { "id" }),
            ))
        };
        let e = match w[0] {
            "door" => Legend::Door(second("door")?),
            "tile" => Legend::Tile(second("tile")?),
            "mark" => Legend::Mark(second("mark")?),
            first => {
                let mut split = first.split(':');
                let kind_word = split.next().unwrap_or("");
                let name = split.next().map(str::to_owned);
                let Some(kind) = RoomSocketKind::from_word(kind_word) else {
                    return fail(lineno, format!("legend \"{}\": unknown socket kind \"{kind_word}\"", ch as char));
                };
                let mut rest: Vec<&str> = w[1..].to_vec();
                let (mut sw, mut sh) = (1, 1);
                if let Some((a, b)) = rest.first().and_then(|r| size(r)) {
                    sw = a;
                    sh = b;
                    rest.remove(0);
                }
                let options =
                    rest.join(" ").split('|').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
                Legend::Socket { kind, name, w: sw, h: sh, options }
            }
        };
        legend.push((ch, e, lineno));
    }
    let get = |ch: u8| legend.iter().find(|(c, _, _)| *c == ch);

    // Doors, as declared in the header, then found on the rim.
    let (bays_s, bays_line) = one("bays")?;
    let Some(bays) = size(&bays_s) else { return fail(bays_line, format!("bays \"{bays_s}\" is not WxH")) };
    let (doors_s, doors_line) = one("doors")?;
    let mut declared: Vec<(String, bool)> = Vec::new();
    for d in doors_s.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let ok = (|| {
            let (id, word) = d.split_once(char::is_whitespace)?;
            let word = word.trim_start();
            let mut c = id.chars();
            let side = c.next()?;
            if !"nesw".contains(side) || digits(c.as_str()).is_none() {
                return None;
            }
            match word {
                "required" => Some((id.to_owned(), true)),
                "optional" => Some((id.to_owned(), false)),
                _ => None,
            }
        })();
        match ok {
            Some((id, req)) => {
                // A Map: the last declaration of an id wins, where the first was put.
                if let Some(e) = declared.iter_mut().find(|(i, _)| *i == id) {
                    e.1 = req;
                } else {
                    declared.push((id, req));
                }
            }
            None => return fail(doors_line, format!("cannot read door \"{d}\"")),
        }
    }
    let mut doors: Vec<Door> = Vec::new();
    for (ch, e, lineno) in &legend {
        let Legend::Door(id) = e else { continue };
        let mut cells = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                if c == ch {
                    cells.push((x as i32, y as i32));
                }
            }
        }
        if cells.len() != 3 {
            return fail(*lineno, format!("door {id} is {} cells, not 3", cells.len()));
        }
        let side = match id.chars().next() {
            Some('n') => RoomSide::N,
            Some('e') => RoomSide::E,
            Some('s') => RoomSide::S,
            Some('w') => RoomSide::W,
            _ => return fail(*lineno, format!("door id \"{id}\" does not start with n, e, s or w")),
        };
        let Some((_, required)) = declared.iter().find(|(d, _)| d == id) else {
            return fail(*lineno, format!("door {id} is drawn and not declared"));
        };
        let bay = digits(&id[1..]).unwrap_or(0) - 1;
        doors.push(Door { id: id.clone(), side, bay, required: *required, cx: cells[1].0, cy: cells[1].1 });
    }
    for (id, _) in &declared {
        if !doors.iter().any(|d| d.id == *id) {
            return fail(doors_line, format!("door {id} is declared and not drawn"));
        }
    }
    doors.sort_by(|a, b| a.id.cmp(&b.id));

    // Sockets: every connected run of a socket character is one socket.
    let mut sockets: Vec<Socket> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    let mut tiles: Vec<(u8, String)> = Vec::new();
    let mut taken = vec![false; gw * gh];
    let mut counters: Vec<(String, usize)> = Vec::new();
    let mut count = |key: String| -> usize {
        if let Some(e) = counters.iter_mut().find(|(k, _)| *k == key) {
            e.1 += 1;
            e.1 - 1
        } else {
            counters.push((key, 1));
            0
        }
    };
    for y in 0..gh {
        for x in 0..gw {
            let ch = rows[y][x];
            if ch == b'#' || ch == b'.' || ch == b'_' || taken[y * gw + x] {
                continue;
            }
            let Some((_, e, _)) = get(ch) else {
                return fail(row_lines[y], format!("grid ({x},{y}): \"{}\" is not in the legend", ch as char));
            };
            match e {
                Legend::Door(_) => {}
                Legend::Tile(t) => {
                    if !tiles.iter().any(|(c, _)| *c == ch) {
                        tiles.push((ch, t.clone()));
                    }
                }
                Legend::Mark(id) => {
                    if marks.iter().any(|m| m.id == *id) {
                        return fail(row_lines[y], format!("mark \"{id}\" is drawn twice"));
                    }
                    marks.push(Mark { id: id.clone(), cx: x as i32, cy: y as i32 });
                }
                Legend::Socket { kind, name, w: ew, h: eh, options } => {
                    let (ew, eh) = (*ew as usize, *eh as usize);
                    // The rectangle this run fills, by reading order: x is its left edge, y its top.
                    let mut w = 0;
                    while x + w < gw && rows[y][x + w] == ch && !taken[y * gw + x + w] && w < ew {
                        w += 1;
                    }
                    let mut h = 0;
                    while y + h < gh && rows[y + h][x] == ch && h < eh {
                        h += 1;
                    }
                    if w != ew || h != eh {
                        return fail(
                            row_lines[y],
                            format!("socket \"{}\" at ({x},{y}) is {w}x{h}, the legend says {ew}x{eh}", ch as char),
                        );
                    }
                    for j in y..y + h {
                        for i in x..x + w {
                            if rows[j][i] != ch {
                                return fail(
                                    row_lines[y],
                                    format!("socket \"{}\" at ({x},{y}) is not a full {ew}x{eh} rectangle", ch as char),
                                );
                            }
                            taken[j * gw + i] = true;
                        }
                    }
                    let id = match name {
                        // Dressing is named for what it is (`dress:torch`) and there are many of each.
                        Some(n) if *kind == RoomSocketKind::Dress => {
                            let k = count(format!("dress:{n}"));
                            format!("dress:{n}:{k}")
                        }
                        Some(n) => {
                            let id = format!("{}:{n}", kind.word());
                            if sockets.iter().any(|s| s.id == id) {
                                return fail(
                                    row_lines[y],
                                    format!("socket \"{id}\" is drawn twice; only unnamed sockets repeat"),
                                );
                            }
                            id
                        }
                        None => {
                            let k = count(kind.word().to_owned());
                            format!("{}:{k}", kind.word())
                        }
                    };
                    sockets.push(Socket {
                        id,
                        kind: *kind,
                        cx: x as i32,
                        cy: y as i32,
                        w: w as i32,
                        h: h as i32,
                        options: options.clone(),
                    });
                }
            }
        }
    }

    let mut rects = vec![TRect { id: "room".into(), cx: 1, cy: 1, w: gw as i32 - 2, h: gh as i32 - 2, line: 0 }];
    for (_, r, lineno) in all("rect") {
        let w: Vec<&str> = r.split_whitespace().collect();
        let nums: Option<Vec<i32>> = w.get(1..).and_then(|n| n.iter().map(|s| digits(s)).collect());
        match nums {
            Some(n) if w.len() == 5 => {
                rects.push(TRect { id: w[0].to_owned(), cx: n[0], cy: n[1], w: n[2], h: n[3], line: *lineno });
            }
            _ => return fail(*lineno, format!("cannot read rect \"{r}\"")),
        }
    }

    let (turn_s, turn_line) = one("turn")?;
    let turn_words = words(&turn_s);
    let mut turns = Vec::new();
    for t in turn_words.iter().filter(|t| **t != "mirror") {
        match digits(t) {
            Some(v @ (0 | 90 | 180 | 270)) => turns.push(v),
            _ => return fail(turn_line, format!("turn \"{t}\" is not 0, 90, 180 or 270")),
        }
    }
    if !turns.contains(&0) {
        return fail(turn_line, "turn must include 0".into());
    }

    let mut needs_verbs = Vec::new();
    let mut needs_items = Vec::new();
    let (needs_s, needs_line) = all("needs").next().map_or(("-".to_owned(), 0), |(_, v, l)| (v.clone(), *l));
    for n in needs_s.split(',').map(str::trim) {
        if n == "-" || n.is_empty() {
            continue;
        }
        let w: Vec<&str> = n.split_whitespace().collect();
        let qty = match w.get(2) {
            None => Some(1),
            Some(q) => digits(q),
        };
        match (w.first().copied(), w.len(), qty) {
            (Some("verb"), 2 | 3, Some(_)) => needs_verbs.push(w[1].to_owned()),
            (Some("item"), 2 | 3, Some(q)) => needs_items.push((w[1].to_owned(), q as u16)),
            _ => return fail(needs_line, format!("cannot read need \"{n}\"")),
        }
    }
    let list = |key: &'static str| -> Vec<(String, usize)> {
        all(key)
            .flat_map(|(_, v, l)| v.split(',').map(|s| (s.trim().to_owned(), *l)).collect::<Vec<_>>())
            .filter(|(s, _)| !s.is_empty() && s != "-")
            .collect()
    };
    let mut blocks = Vec::new();
    for (b, l) in list("blocks") {
        let w: Vec<&str> = b.split_whitespace().collect();
        if w.len() == 3 && w[1] == "until" {
            blocks.push((w[0].to_owned(), w[2].to_owned()));
        } else {
            return fail(l, format!("cannot read block \"{b}\""));
        }
    }
    let mut coop = Vec::new();
    for (c, l) in list("coop") {
        match RoomCoop::from_word(&c) {
            Some(t) => coop.push(t),
            None => return fail(l, format!("unknown coop tag \"{c}\"")),
        }
    }
    let heat_max = match all("heat").next() {
        None => 0,
        Some((_, v, l)) => match digits(v).and_then(|h| u16::try_from(h).ok()) {
            Some(h) => h,
            None => return fail(*l, format!("heat \"{v}\" is not a whole number")),
        },
    };
    let mut head_lines: Vec<(String, usize)> = Vec::new();
    for (k, _, l) in &head {
        if !head_lines.iter().any(|(h, _)| h == k) {
            head_lines.push((k.clone(), *l));
        }
    }
    Ok(Room {
        file: file.to_owned(),
        id: one("id")?.0,
        pool: one("pool")?.0,
        bays,
        turns,
        mirror: turn_words.contains(&"mirror"),
        doors,
        sockets,
        marks,
        rects,
        needs_verbs,
        needs_items,
        grants: list("grants").into_iter().map(|(s, _)| s).collect(),
        blocks,
        coop,
        heat_max,
        grid: rows,
        tiles,
        row_lines,
        head_lines,
    })
}

/// Header keys `parseRoom` reads; any other is ignored there and warned about here.
pub const HEADER_KEYS: [&str; 11] =
    ["id", "pool", "bays", "turn", "doors", "needs", "grants", "blocks", "coop", "heat", "rect"];

// --- turn and mirror ---------------------------------------------------------------------

/// A template as it will be stamped (`room.ts` `Shape`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub turn: i32,
    pub mirror: bool,
    pub w: i32,
    pub h: i32,
    pub bays: (i32, i32),
    /// Rows of bytes. Sockets and marks are floor here.
    pub cells: Vec<Vec<u8>>,
    /// In id order.
    pub doors: Vec<Door>,
    /// Boxes, index-aligned with the template's sockets: (x, y, w, h).
    pub sockets: Vec<(i32, i32, i32, i32)>,
    pub marks: Vec<(i32, i32)>,
    pub rects: Vec<(i32, i32, i32, i32)>,
    pub ox: i32,
    pub oy: i32,
    pub fits: bool,
}

#[derive(Clone, Copy)]
enum Step {
    Mirror(i32),
    Turn(i32),
}

fn apply(steps: &[Step], x: i32, y: i32) -> (i32, i32) {
    let (mut px, mut py) = (x, y);
    for s in steps {
        (px, py) = match *s {
            Step::Mirror(w0) => (w0 - 1 - px, py),
            Step::Turn(h0) => (h0 - 1 - py, px),
        };
    }
    (px, py)
}

const fn cw(s: RoomSide) -> RoomSide {
    match s {
        RoomSide::N => RoomSide::E,
        RoomSide::E => RoomSide::S,
        RoomSide::S => RoomSide::W,
        RoomSide::W => RoomSide::N,
    }
}

pub fn shape_of(t: &Room, turn: i32, mirror: bool) -> Shape {
    let mut w = t.gw();
    let mut h = t.gh();
    let mut bays = t.bays;
    let mut steps: Vec<Step> = Vec::new();
    let mut doors: Vec<Door> = t.doors.clone();
    if mirror {
        let cols = bays.0;
        steps.push(Step::Mirror(w));
        for d in &mut doors {
            let ns = matches!(d.side, RoomSide::N | RoomSide::S);
            d.side = match d.side {
                RoomSide::E => RoomSide::W,
                RoomSide::W => RoomSide::E,
                s => s,
            };
            if ns {
                d.bay = cols - 1 - d.bay;
            }
        }
    }
    for _ in 0..turn / 90 {
        let rows = bays.1;
        steps.push(Step::Turn(h));
        for d in &mut doors {
            if matches!(d.side, RoomSide::E | RoomSide::W) {
                d.bay = rows - 1 - d.bay;
            }
            d.side = cw(d.side);
        }
        (w, h) = (h, w);
        bays = (bays.1, bays.0);
    }
    let at = |x: i32, y: i32| apply(&steps, x, y);
    let boxed = |cx: i32, cy: i32, bw: i32, bh: i32| {
        let (ax, ay) = at(cx, cy);
        let (bx, by) = at(cx + bw - 1, cy + bh - 1);
        (ax.min(bx), ay.min(by), (bx - ax).abs() + 1, (by - ay).abs() + 1)
    };
    let mut cells = vec![vec![b'#'; w as usize]; h as usize];
    let mut socket_at = vec![false; (t.gw() * t.gh()) as usize];
    let gw = t.gw();
    for s in &t.sockets {
        for j in 0..s.h {
            for i in 0..s.w {
                socket_at[((s.cy + j) * gw + s.cx + i) as usize] = true;
            }
        }
    }
    for m in &t.marks {
        socket_at[(m.cy * gw + m.cx) as usize] = true;
    }
    for y in 0..t.gh() {
        for x in 0..gw {
            let (nx, ny) = at(x, y);
            cells[ny as usize][nx as usize] =
                if socket_at[(y * gw + x) as usize] { b'.' } else { t.grid[y as usize][x as usize] };
        }
    }
    // In id order, so that two shapes offering the same doors are joined up the same way.
    let mut out_doors: Vec<Door> = doors
        .into_iter()
        .map(|d| {
            let (cx, cy) = at(d.cx, d.cy);
            Door { id: format!("{}{}", d.side.letter(), d.bay + 1), cx, cy, ..d }
        })
        .collect();
    out_doors.sort_by(|a, b| a.id.cmp(&b.id));
    let mut shape = Shape {
        turn,
        mirror,
        w,
        h,
        bays,
        cells,
        sockets: t.sockets.iter().map(|s| boxed(s.cx, s.cy, s.w, s.h)).collect(),
        marks: t.marks.iter().map(|m| at(m.cx, m.cy)).collect(),
        rects: t.rects.iter().map(|r| boxed(r.cx, r.cy, r.w, r.h)).collect(),
        doors: out_doors,
        ox: 0,
        oy: 0,
        fits: true,
    };
    // Where in its bays: every n/s door fixes x, every e/w door fixes y, and they must agree.
    let mut ox: Option<i32> = None;
    let mut oy: Option<i32> = None;
    for d in &shape.doors {
        let ns = matches!(d.side, RoomSide::N | RoomSide::S);
        if ns {
            let want = d.bay * BAY_W + MOUTH_X - d.cx;
            if ox.is_some_and(|o| o != want) {
                shape.fits = false;
            }
            ox = Some(want);
            if d.cy != if d.side == RoomSide::N { 0 } else { h - 1 } {
                shape.fits = false;
            }
        } else {
            let want = d.bay * BAY_H + MOUTH_Y - d.cy;
            if oy.is_some_and(|o| o != want) {
                shape.fits = false;
            }
            oy = Some(want);
            if d.cx != if d.side == RoomSide::W { 0 } else { w - 1 } {
                shape.fits = false;
            }
        }
        if d.bay < 0 || d.bay >= if ns { bays.0 } else { bays.1 } {
            shape.fits = false;
        }
    }
    shape.ox = ox.unwrap_or_else(|| (bays.0 * BAY_W + 1 - w).div_euclid(2));
    shape.oy = oy.unwrap_or_else(|| (bays.1 * BAY_H + 1 - h).div_euclid(2));
    if shape.ox < RIM_LO || shape.ox + w - 1 > (bays.0 - 1) * BAY_W + RIM_HI_X {
        shape.fits = false;
    }
    if shape.oy < RIM_LO || shape.oy + h - 1 > (bays.1 - 1) * BAY_H + RIM_HI_Y {
        shape.fits = false;
    }
    // A socket that is not square cannot be turned on its side: the prop it holds has one footprint.
    if (turn == 90 || turn == 270) && t.sockets.iter().any(|s| s.w != s.h) {
        shape.fits = false;
    }
    shape
}

/// Every transform the template allows, in a fixed order.
pub fn shapes_of(t: &Room) -> Vec<Shape> {
    let mut out = Vec::new();
    for &turn in &t.turns {
        out.push(shape_of(t, turn, false));
        if t.mirror {
            out.push(shape_of(t, turn, true));
        }
    }
    out
}

// --- lint --------------------------------------------------------------------------------

const fn inward(s: RoomSide) -> (i32, i32) {
    match s {
        RoomSide::N => (0, 1),
        RoomSide::S => (0, -1),
        RoomSide::W => (1, 0),
        RoomSide::E => (-1, 0),
    }
}

const fn along(s: RoomSide) -> (i32, i32) {
    match s {
        RoomSide::N | RoomSide::S => (1, 0),
        RoomSide::E | RoomSide::W => (0, 1),
    }
}

/// What stands on a cell, for the path rules.
const SOLID: u8 = 1;
const PUSH: u8 = 2;

const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Extra tiles are taken as walkable for the path rules unless they are plainly not.
fn blocking_tile_name(name: &str) -> bool {
    ["Wall", "Water", "Rail", "Rubble", "Fence", "Glass", "Hedge", "Cliff", "Void"].iter().any(|w| name.contains(w))
}

/// The six rules of DUNGEONS.md §2.3, and the few the mine added. Empty means clean.
pub fn lint(t: &Room) -> Vec<LineError> {
    let mut errors: Vec<LineError> = Vec::new();
    let gw = t.gw();
    let gh = t.gh();
    let at = |x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || x >= gw || y >= gh { b'#' } else { t.grid[y as usize][x as usize] }
    };
    let ch = |c: u8| c as char;
    let mut door_cells: BTreeSet<(i32, i32)> = BTreeSet::new();
    for d in &t.doors {
        let (ax, ay) = along(d.side);
        for n in -1..=1 {
            door_cells.insert((d.cx + ax * n, d.cy + ay * n));
        }
    }

    // 1. The rim is wall except at declared doors.
    for y in 0..gh {
        for x in 0..gw {
            let rim = x == 0 || y == 0 || x == gw - 1 || y == gh - 1;
            let c = at(x, y);
            if rim && c != b'#' && !door_cells.contains(&(x, y)) {
                errors
                    .push((t.row_line(y), format!("rule 1: rim cell ({x},{y}) is \"{}\", not wall or a door", ch(c))));
            }
            if !rim && door_cells.contains(&(x, y)) {
                errors.push((t.row_line(y), format!("rule 1: door cell ({x},{y}) is not on the rim")));
            }
        }
    }
    // A sill inside every door; none anywhere else. In insertion order, as a Set is.
    let mut sills: Vec<(i32, i32)> = Vec::new();
    for d in &t.doors {
        let (ax, ay) = along(d.side);
        let (ix, iy) = inward(d.side);
        for n in -1..=1 {
            let x = d.cx + ax * n + ix;
            let y = d.cy + ay * n + iy;
            if !sills.contains(&(x, y)) {
                sills.push((x, y));
            }
            if at(x, y) != b'_' {
                errors.push((
                    t.row_line(y),
                    format!("door {}: cell ({x},{y}) just inside it is \"{}\", not a sill", d.id, ch(at(x, y))),
                ));
            }
        }
    }
    for y in 0..gh {
        for x in 0..gw {
            if at(x, y) == b'_' && !sills.contains(&(x, y)) {
                errors.push((t.row_line(y), format!("sill at ({x},{y}) is not inside a door")));
            }
        }
    }

    // What stands where, for the path rules.
    let mut stand = vec![0u8; (gw * gh) as usize];
    for s in &t.sockets {
        let v = if s.kind == RoomSocketKind::Push {
            PUSH
        } else if socket_is_solid(s) {
            SOLID
        } else {
            0
        };
        if v != 0 {
            for j in 0..s.h {
                for i in 0..s.w {
                    stand[((s.cy + j) * gw + s.cx + i) as usize] = v;
                }
            }
        }
    }
    let stand_at = |x: i32, y: i32| -> u8 {
        let i = y * gw + x;
        if i < 0 || i >= gw * gh { 0 } else { stand[i as usize] }
    };
    let floor_tile = |x: i32, y: i32| -> bool {
        let c = at(x, y);
        c != b'#' && !t.tile_of(c).is_some_and(blocking_tile_name)
    };

    // 2. Sockets on floor, clear of sills and of the straight line between facing doors.
    for s in &t.sockets {
        let line = t.row_line(s.cy);
        for j in 0..s.h {
            for i in 0..s.w {
                let x = s.cx + i;
                let y = s.cy + j;
                if x <= 0 || y <= 0 || x >= gw - 1 || y >= gh - 1 {
                    errors.push((line, format!("rule 2: socket {} touches the rim at ({x},{y})", s.id)));
                }
                if !socket_is_solid(s) {
                    continue;
                }
                for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if at(x + dx, y + dy) == b'_' {
                        errors.push((
                            line,
                            format!("rule 2: solid socket {} touches the sill at ({},{})", s.id, x + dx, y + dy),
                        ));
                    }
                }
            }
        }
    }
    for a in &t.doors {
        for b in &t.doors {
            if a.id >= b.id {
                continue;
            }
            let facing = matches!(
                (a.side, b.side),
                (RoomSide::N, RoomSide::S)
                    | (RoomSide::S, RoomSide::N)
                    | (RoomSide::E, RoomSide::W)
                    | (RoomSide::W, RoomSide::E)
            );
            if !facing {
                continue;
            }
            let vertical = matches!(a.side, RoomSide::N | RoomSide::S);
            if if vertical { a.cx != b.cx } else { a.cy != b.cy } {
                continue;
            }
            for s in &t.sockets {
                if !socket_is_solid(s) || s.kind == RoomSocketKind::Push {
                    continue;
                }
                let hit = if vertical {
                    s.cx <= a.cx + 1 && s.cx + s.w > a.cx - 1
                } else {
                    s.cy <= a.cy + 1 && s.cy + s.h > a.cy - 1
                };
                if hit {
                    errors.push((
                        t.row_line(s.cy),
                        format!("rule 2: solid socket {} stands in the line between doors {} and {}", s.id, a.id, b.id),
                    ));
                }
            }
        }
    }

    // 3. Nobody is hit in a doorway.
    for s in &t.sockets {
        if !matches!(s.kind, RoomSocketKind::Spawn | RoomSocketKind::Boss | RoomSocketKind::Unit) {
            continue;
        }
        for d in &t.doors {
            let dist = (s.cx - d.cx).abs().max((s.cy - d.cy).abs());
            if dist < 4 {
                errors
                    .push((t.row_line(s.cy), format!("rule 3: {} is {dist} cells from door {}, under 4", s.id, d.id)));
            }
        }
    }

    // 4. Two cells of clear width between every pair of doors, push props where they start.
    if t.doors.len() > 1 {
        let wide = |x: i32, y: i32| -> bool {
            for j in 0..2 {
                for i in 0..2 {
                    if !floor_tile(x + i, y + j) || stand_at(x + i, y + j) != 0 {
                        return false;
                    }
                }
            }
            true
        };
        // The 2 x 2 block whose far corner is the door's centre cell, reaching into the room.
        let start_of = |d: &Door| {
            let (ix, iy) = inward(d.side);
            (d.cx.min(d.cx + ix), d.cy.min(d.cy + iy))
        };
        let mut seen = vec![false; (gw * gh) as usize];
        let first = start_of(&t.doors[0]);
        let mut queue: Vec<(i32, i32)> = Vec::new();
        if wide(first.0, first.1) {
            seen[(first.1 * gw + first.0) as usize] = true;
            queue.push(first);
        }
        let mut n = 0;
        while n < queue.len() {
            let (x, y) = queue[n];
            n += 1;
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= gw - 1 || ny >= gh - 1 || seen[(ny * gw + nx) as usize] || !wide(nx, ny) {
                    continue;
                }
                seen[(ny * gw + nx) as usize] = true;
                queue.push((nx, ny));
            }
        }
        for d in &t.doors {
            let (x, y) = start_of(d);
            let i = y * gw + x;
            if i < 0 || i >= gw * gh || !seen[i as usize] {
                errors.push((
                    t.row_line(d.cy),
                    format!("rule 4: no path two cells wide from door {} to door {}", t.doors[0].id, d.id),
                ));
            }
        }
    }

    // Every socket can be walked up to from every door.
    if let Some(d0) = t.doors.first() {
        let mut seen = vec![false; (gw * gh) as usize];
        let mut queue = vec![(d0.cx, d0.cy)];
        seen[(d0.cy * gw + d0.cx) as usize] = true;
        let mut n = 0;
        while n < queue.len() {
            let (x, y) = queue[n];
            n += 1;
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= gw || ny >= gh || seen[(ny * gw + nx) as usize] {
                    continue;
                }
                // A push prop is in the way only until it is pushed.
                if !floor_tile(nx, ny) || stand[(ny * gw + nx) as usize] == SOLID {
                    continue;
                }
                seen[(ny * gw + nx) as usize] = true;
                queue.push((nx, ny));
            }
        }
        let seen_at = |i: i32| i >= 0 && i < gw * gh && seen[i as usize];
        for s in &t.sockets {
            let mut near = false;
            for j in -1..=s.h {
                for i in -1..=s.w {
                    near = near || seen_at((s.cy + j) * gw + s.cx + i);
                }
            }
            if !near {
                errors.push((t.row_line(s.cy), format!("socket {} cannot be walked up to", s.id)));
            }
        }
        for m in &t.marks {
            if !seen_at(m.cy * gw + m.cx) {
                errors.push((t.row_line(m.cy), format!("mark {} cannot be walked to", m.id)));
            }
        }
    }

    // 5. A plate has a push prop in the room, and the prop can be pushed onto it.
    let plates: Vec<&Socket> = t.sockets.iter().filter(|s| s.kind == RoomSocketKind::Plate).collect();
    let pushes: Vec<&Socket> = t.sockets.iter().filter(|s| s.kind == RoomSocketKind::Push).collect();
    for plate in &plates {
        let ok = pushes.iter().any(|p| {
            push_path(gw, gh, (p.cx, p.cy, p.w, p.h), (plate.cx, plate.cy, plate.w, plate.h), |x, y| {
                floor_tile(x, y) && at(x, y) != b'_' && (stand[(y * gw + x) as usize] == 0 || inside(p, x, y))
            })
        });
        if !ok {
            errors.push((t.row_line(plate.cy), format!("rule 5: nothing in the room can be pushed onto {}", plate.id)));
        }
    }
    if pushes.len() < plates.len() {
        errors.push((
            plates.first().map_or(0, |p| t.row_line(p.cy)),
            format!("rule 5: {} plates and only {} things to push", plates.len(), pushes.len()),
        ));
    }

    // A hoist must not drop on whoever pulls its lever.
    for r in &t.rects {
        if !r.id.starts_with("drop") {
            continue;
        }
        for s in &t.sockets {
            if s.kind != RoomSocketKind::Lever {
                continue;
            }
            if s.cx >= r.cx - 1 && s.cy >= r.cy - 1 && s.cx + s.w <= r.cx + r.w + 1 && s.cy + s.h <= r.cy + r.h + 1 {
                errors.push((r.line, format!("{} stands in or beside {}: the puller would be under it", s.id, r.id)));
            }
        }
    }
    for r in &t.rects {
        if r.cx < 1 || r.cy < 1 || r.cx + r.w > gw - 1 || r.cy + r.h > gh - 1 {
            errors.push((r.line, format!("rect {} reaches the rim", r.id)));
        }
    }
    for g in &t.grants {
        if !t.sockets.iter().any(|s| s.id == *g) {
            errors.push((t.head_line("grants"), format!("grants names \"{g}\", which is not a socket")));
        }
    }
    for (what, until) in &t.blocks {
        for n in [what, until] {
            if !t.sockets.iter().any(|s| s.id == *n) {
                errors.push((t.head_line("blocks"), format!("blocks names \"{n}\", which is not a socket")));
            }
        }
    }

    // 6. Dressing stands where somebody would have put it. A pile, a shelf or a cot is against a
    // wall (a shelf with its back to it); nothing dressed stands within two cells of a doorway;
    // and no lamp is dressing at all, because lamps go on the walls by rule (lights.ts).
    for s in &t.sockets {
        if s.kind != RoomSocketKind::Dress {
            continue;
        }
        let line = t.row_line(s.cy);
        let name = s.id.split(':').nth(1).unwrap_or("");
        if name == "torch" {
            errors.push((
                line,
                format!("rule 6: {} is a lamp on the floor; lamps hang on walls by rule (lights.ts)", s.id),
            ));
        }
        let row = |y: i32| (0..s.w).all(|i| at(s.cx + i, y) == b'#');
        let col = |x: i32| (0..s.h).all(|j| at(x, s.cy + j) == b'#');
        let backed = if s.w >= s.h { row(s.cy - 1) || row(s.cy + s.h) } else { col(s.cx - 1) || col(s.cx + s.w) };
        let mut touches = false;
        for j in 0..s.h {
            touches = touches || at(s.cx - 1, s.cy + j) == b'#' || at(s.cx + s.w, s.cy + j) == b'#';
        }
        for i in 0..s.w {
            touches = touches || at(s.cx + i, s.cy - 1) == b'#' || at(s.cx + i, s.cy + s.h) == b'#';
        }
        if name == "shelf" && !backed {
            errors.push((line, format!("rule 6: {} at ({},{}) does not have its back to a wall", s.id, s.cx, s.cy)));
        } else if (name == "pile" || name == "cot") && !touches {
            errors.push((
                line,
                format!("rule 6: {} at ({},{}) stands out in the room, not against a wall", s.id, s.cx, s.cy),
            ));
        }
        if !socket_is_solid(s) {
            continue;
        }
        for &(sx, sy) in &sills {
            let dx = (s.cx - sx).max(0).max(sx - (s.cx + s.w - 1));
            let dy = (s.cy - sy).max(0).max(sy - (s.cy + s.h - 1));
            if dx.max(dy) <= 2 {
                errors
                    .push((line, format!("rule 6: {} at ({},{}) crowds the doorway at ({sx},{sy})", s.id, s.cx, s.cy)));
                break;
            }
        }
    }

    // Every transform it claims must fit its bays with its doors on the mouth lines.
    for s in shapes_of(t) {
        if !s.fits {
            errors.push((
                t.head_line("turn"),
                format!(
                    "turn {}{} does not fit {}x{} bays with its doors on the mouth lines",
                    s.turn,
                    if s.mirror { " mirrored" } else { "" },
                    s.bays.0,
                    s.bays.1
                ),
            ));
        }
    }
    errors
}

fn inside(s: &Socket, x: i32, y: i32) -> bool {
    x >= s.cx && y >= s.cy && x < s.cx + s.w && y < s.cy + s.h
}

/// Can the footprint `from` (x, y, w, h) be pushed until it overlaps `to`? A flood of the
/// footprint's position; each step needs the new footprint clear and one clear cell behind it
/// for whoever is pushing. `clear` must count the prop's own starting cells as clear.
pub fn push_path(
    gw: i32,
    gh: i32,
    from: (i32, i32, i32, i32),
    to: (i32, i32, i32, i32),
    clear: impl Fn(i32, i32) -> bool,
) -> bool {
    let (fx, fy, w, h) = from;
    let fits = |x: i32, y: i32| {
        for j in 0..h {
            for i in 0..w {
                if x + i < 0 || y + j < 0 || x + i >= gw || y + j >= gh || !clear(x + i, y + j) {
                    return false;
                }
            }
        }
        true
    };
    let overlaps = |x: i32, y: i32| x < to.0 + to.2 && x + w > to.0 && y < to.1 + to.3 && y + h > to.1;
    let mut seen: BTreeSet<(i32, i32)> = BTreeSet::new();
    seen.insert((fx, fy));
    let mut queue = vec![(fx, fy)];
    let mut n = 0;
    while n < queue.len() {
        let (x, y) = queue[n];
        n += 1;
        if overlaps(x, y) {
            return true;
        }
        for (dx, dy) in DIRS {
            let (nx, ny) = (x + dx, y + dy);
            if seen.contains(&(nx, ny)) || !fits(nx, ny) {
                continue;
            }
            // Somewhere to stand on the side she pushes from.
            let behind = if dx != 0 {
                (0..h).any(|j| clear(if dx > 0 { x - 1 } else { x + w }, y + j))
            } else {
                (0..w).any(|i| clear(x + i, if dy > 0 { y - 1 } else { y + h }))
            };
            if !behind {
                continue;
            }
            seen.insert((nx, ny));
            queue.push((nx, ny));
        }
    }
    false
}

#[cfg(test)]
mod tests {
    //! Carries the data-only parts of `jane/test/templates.test.ts`: the GOOD room, each rule
    //! broken one at a time, and the transforms keeping sockets on floor and doors on the rim.

    use super::*;

    /// A small room that obeys every rule; the tests below break it one rule at a time.
    pub const GOOD: &str = "
id      test.good
pool    test.good
bays    1x1
turn    0 180 mirror
doors   n1 optional, s1 optional
needs   -
grants  chest:reward
blocks  chest:reward until plate:main
heat    2
grid
#######nnn#######
#......___......#
#...............#
#..PP.......CC..#
#..PP.......CC..#
#...............#
#...BB..........#
#...BB....r.....#
#...............#
#...............#
#......___......#
#######sss#######
legend
n door n1
s door s1
P plate:main 2x2
C chest:reward 2x2
B push:main 2x2 barrel | crate
r spawn
";

    fn good() -> Room {
        parse(GOOD, "test").unwrap()
    }

    fn broken(change: impl Fn(&str) -> String) -> String {
        let r = parse(&change(GOOD), "test").unwrap();
        lint(&r).into_iter().map(|(_, m)| m).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn the_good_room_parses_as_written() {
        let r = good();
        assert_eq!(r.id, "test.good");
        assert_eq!(r.bays, (1, 1));
        assert_eq!(r.turns, [0, 180]);
        assert!(r.mirror);
        assert_eq!(
            r.doors.iter().map(|d| (d.id.as_str(), d.cx, d.cy)).collect::<Vec<_>>(),
            [("n1", 8, 0), ("s1", 8, 11)]
        );
        let ids: Vec<&str> = r.sockets.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["plate:main", "chest:reward", "push:main", "spawn:0"]);
        assert_eq!(r.sockets[2].options, ["barrel", "crate"]);
        assert_eq!(r.grants, ["chest:reward"]);
        assert_eq!(r.blocks, [("chest:reward".to_owned(), "plate:main".to_owned())]);
        assert_eq!(r.heat_max, 2);
        assert_eq!(r.rects[0], TRect { id: "room".into(), cx: 1, cy: 1, w: 15, h: 10, line: 0 });
        // The grid starts on line 12 of the text (the text opens with a newline).
        assert_eq!(r.row_lines[0], 12);
        assert!(lint(&r).is_empty(), "{:?}", lint(&r));
    }

    #[test]
    fn the_lint_catches_each_rule_it_claims_to() {
        // 1: a hole in the rim that is not a door
        assert!(broken(|t| t.replace("#...............#\n#..PP", "................#\n#..PP")).contains("rule 1"));
        // a door with no sill inside it
        assert!(
            broken(|t| t
                .replace("#......___......#\n#...............#\n#..PP", "#...............#\n#...............#\n#..PP"))
            .contains("not a sill")
        );
        // 2: something solid on the line between two facing doors
        assert!(
            broken(|t| t.replace("#..PP.......CC..#\n#..PP.......CC..#", "#..PP..CC.......#\n#..PP..CC.......#"))
                .contains("rule 2")
        );
        // 3: a spawn socket in a doorway
        assert!(broken(|t| t.replace("#...............#\n#..PP", "#.......r.......#\n#..PP")).contains("rule 3"));
        // 4: a wall across the room leaves no path two cells wide between the doors
        assert!(
            broken(|t| t.replace(
                "#...............#\n#...............#\n#......___......#\n#######sss",
                "#...............#\n#################\n#......___......#\n#######sss"
            ))
            .contains("rule 4")
        );
        // 5: a plate and nothing to push onto it
        assert!(broken(|t| t.replace("BB", "..").replace("B push:main 2x2 barrel | crate\n", "")).contains("rule 5"));
        // 5: the thing to push is walled off from the plate
        assert!(broken(|t| t.replace("#...............#\n#...BB", "#################\n#...BB")).contains("rule 5"));
        // 6: a pile out in the middle of the room, and a torch on the floor
        let pile =
            broken(|t| t.replace("#...BB....r.....#", "#...BB....r..Z..#").replace("r spawn", "r spawn\nZ dress:pile"));
        assert!(pile.contains("rule 6") && pile.contains("stands out in the room"), "{pile}");
        let torch = broken(|t| {
            t.replace("#...BB....r.....#", "#...BB....r....T#").replace("r spawn", "r spawn\nT dress:torch")
        });
        assert!(torch.contains("rule 6") && torch.contains("lamp on the floor"), "{torch}");
        // a shelf with its back to nothing
        let shelf = broken(|t| {
            t.replace("#...BB....r.....#", "#...BB....r..S..#").replace("r spawn", "r spawn\nS dress:shelf")
        });
        assert!(shelf.contains("does not have its back to a wall"), "{shelf}");
    }

    #[test]
    fn parse_errors_name_their_line() {
        // 6: a letter that is not in the legend is a parse error, not a surprise
        let e = parse(&GOOD.replace("#...BB....r.....#", "#...BB....r..Z..#"), "test").unwrap_err();
        assert!(e.1.contains("not in the legend"), "{e:?}");
        assert_eq!(e.0, 19);
        let e = parse(&GOOD.replace("bays    1x1", "bays    one"), "test").unwrap_err();
        assert_eq!(e, (4, "bays \"one\" is not WxH".to_owned()));
        let e = parse(&GOOD.replace("turn    0 180 mirror", "turn    180"), "test").unwrap_err();
        assert_eq!(e.1, "turn must include 0");
        // Doors are matched before the grid is read, as in parseRoom.
        let e = parse(&GOOD.replace("n door n1\n", ""), "test").unwrap_err();
        assert_eq!(e, (6, "door n1 is declared and not drawn".to_owned()));
        let e = parse(&GOOD.replace("doors   n1 optional, s1 optional", "doors   n1 optional"), "test").unwrap_err();
        assert!(e.1.contains("door s1 is drawn and not declared"), "{e:?}");
        let e = parse(&GOOD.replace("P plate:main 2x2", "P plate:main 3x2"), "test").unwrap_err();
        assert!(e.1.contains("the legend says 3x2"), "{e:?}");
        let e = parse(&GOOD.replace("r spawn", "r spook"), "test").unwrap_err();
        assert!(e.1.contains("unknown socket kind \"spook\""), "{e:?}");
        let e = parse(&GOOD.replace("heat    2", "heat    lots"), "test").unwrap_err();
        assert!(e.1.contains("not a whole number"), "{e:?}");
        let e = parse(&GOOD.replace("heat    2", "heat    2\ncoop    fetch"), "test").unwrap_err();
        assert!(e.1.contains("unknown coop tag"), "{e:?}");
    }

    #[test]
    fn a_wide_room_cannot_be_turned_on_its_side() {
        // A transform that does not fit its bay: a room as wide as a bay cannot be turned on its side.
        let wide = "#".repeat(31);
        let mut t = good();
        t.grid = vec![wide.as_bytes().to_vec(); 6];
        t.row_lines = vec![1; 6];
        t.doors.clear();
        t.sockets.clear();
        t.turns = vec![0, 90];
        t.mirror = false;
        let msgs: Vec<String> = lint(&t).into_iter().map(|(_, m)| m).collect();
        assert!(msgs.iter().any(|m| m.contains("turn 90 does not fit")), "{msgs:?}");
    }

    #[test]
    fn turning_and_mirroring_keep_sockets_on_floor_and_doors_on_the_rim() {
        let t = good();
        for s in shapes_of(&t) {
            assert!(s.fits);
            for d in &s.doors {
                let on_rim = match d.side {
                    RoomSide::N => d.cy == 0,
                    RoomSide::S => d.cy == s.h - 1,
                    RoomSide::W => d.cx == 0,
                    RoomSide::E => d.cx == s.w - 1,
                };
                assert!(on_rim, "turn {} door {}", s.turn, d.id);
            }
            for &(x, y, w, h) in &s.sockets {
                for j in 0..h {
                    for i in 0..w {
                        assert_eq!(s.cells[(y + j) as usize][(x + i) as usize], b'.');
                    }
                }
            }
        }
        // Four quarter turns are the template again.
        let mut t4 = t.clone();
        t4.turns = vec![0, 90, 180, 270];
        let s180 = shape_of(&t4, 180, false);
        let back = Room { grid: s180.cells.clone(), ..t4.clone() };
        let again = shape_of(&back, 180, false);
        assert_eq!(again.cells, shape_of(&t4, 0, false).cells);
        // A mirror swaps east and west and keeps north on top.
        let m = shape_of(&t4, 0, true);
        assert_eq!(m.doors.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["n1", "s1"]);
        assert_eq!(m.doors[0].cx, 16 - 8);
        // A quarter turn puts north on the east.
        let q = shape_of(&t4, 90, false);
        assert_eq!(q.doors.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["e1", "w1"]);
        assert_eq!((q.w, q.h), (12, 17));
    }

    #[test]
    fn push_path_needs_room_behind() {
        // A 1x1 crate in a corridor one wide can be pushed along it, not out of it.
        let open = |x: i32, y: i32| y == 0 && (0..6).contains(&x);
        assert!(push_path(6, 1, (1, 0, 1, 1), (4, 0, 1, 1), open));
        // With nowhere to stand behind it, it cannot move.
        assert!(!push_path(6, 1, (0, 0, 1, 1), (4, 0, 1, 1), open));
    }
}
