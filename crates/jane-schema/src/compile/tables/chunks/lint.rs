//! Lay a parsed chunk's layers out into props, units, marks, rects and fills, and lint it: every
//! character is in the legend and in its own layer, every legend entry is drawn, keyed things are
//! drawn once, every prop is a whole footprint of its def, nothing leaves the box, every gate is on
//! the ring just outside the box with walkable ground inside it, and nobody stands in a wall.
//! Everything is reported, not only the first.

use jane_core::Tile;
use jane_core::tile::F_SOLID;

use super::parse::{Chunk, Entry, Layer, LineError, Side};

/// What the lint needs to know about the rest of the catalog.
pub trait Facts {
    /// A prop def's footprint and whether it blocks feet; `None`: no such def.
    fn prop(&self, def: &str) -> Option<(i32, i32, bool)>;
    /// Is there such a unit def?
    fn unit(&self, def: &str) -> bool;
}

pub fn tile_named(name: &str) -> Option<Tile> {
    Tile::ALL.iter().copied().find(|t| t.name() == name)
}

/// A laid-out thing: its legend entry (index into `Chunk::legend`) and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub entry: usize,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The chunk's layers read into things, each list in reading order of its top-left cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Laid {
    /// Row-major tile per cell (`Tile::Void` where the character was not a tile).
    pub tiles: Vec<Tile>,
    pub props: Vec<Placed>,
    pub units: Vec<Placed>,
    pub marks: Vec<Placed>,
    pub rects: Vec<Placed>,
    /// Per fill entry (legend index), its cells in reading order.
    pub fills: Vec<(usize, Vec<(i32, i32)>)>,
    /// Row-major.
    pub claims: Vec<bool>,
}

impl Laid {
    fn tile(&self, c: &Chunk, x: i32, y: i32) -> Tile {
        self.tiles[(y * c.w + x) as usize]
    }
}

pub fn lay_out(c: &Chunk, facts: &dyn Facts) -> (Laid, Vec<LineError>) {
    let mut errs: Vec<LineError> = Vec::new();
    let mut laid = Laid::default();
    let (w, h) = (c.w, c.h);
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    let mut drawn = vec![0usize; c.legend.len()];
    let ix = |ch: char| c.legend.iter().position(|l| l.ch == ch);

    // The ground.
    let grid = c.layer(Layer::Grid);
    for y in 0..h {
        for x in 0..w {
            let ch = grid.at(x, y);
            let t = match ix(ch).map(|i| (i, &c.legend[i].entry)) {
                Some((i, Entry::Tile(name))) => {
                    drawn[i] += 1;
                    tile_named(name)
                }
                Some((_, e)) => {
                    errs.push((grid.line(y), format!("grid ({x},{y}): \"{ch}\" is a {}, not a tile", e.kind())));
                    None
                }
                None => {
                    errs.push((grid.line(y), format!("grid ({x},{y}): \"{ch}\" is not in the legend")));
                    None
                }
            };
            laid.tiles.push(t.unwrap_or(Tile::Void));
        }
    }
    for l in &c.legend {
        match &l.entry {
            Entry::Tile(name) if tile_named(name).is_none() => {
                errs.push((l.line, format!("no tile called \"{name}\"")));
            }
            _ => {}
        }
    }

    // Props and units.
    let things = c.layer(Layer::Things);
    let mut taken = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let ch = things.at(x, y);
            if ch == '.' || taken[(y * w + x) as usize] {
                continue;
            }
            let line = things.line(y);
            let Some(i) = ix(ch) else {
                errs.push((line, format!("things ({x},{y}): \"{ch}\" is not in the legend")));
                continue;
            };
            match &c.legend[i].entry {
                Entry::Prop { def, .. } => {
                    let (pw, ph) = facts.prop(def).map_or((1, 1), |(pw, ph, _)| (pw, ph));
                    let whole = (0..ph).all(|j| {
                        (0..pw).all(|k| {
                            inside(x + k, y + j)
                                && things.at(x + k, y + j) == ch
                                && !taken[((y + j) * w + x + k) as usize]
                        })
                    });
                    if !whole {
                        errs.push((
                            line,
                            format!("things ({x},{y}): prop \"{ch}\" ({def}) is not a whole {pw}x{ph} footprint here"),
                        ));
                    }
                    for j in 0..ph {
                        for k in 0..pw {
                            if inside(x + k, y + j) && things.at(x + k, y + j) == ch {
                                taken[((y + j) * w + x + k) as usize] = true;
                            }
                        }
                    }
                    drawn[i] += 1;
                    laid.props.push(Placed { entry: i, x, y, w: pw, h: ph });
                }
                Entry::Unit { .. } => {
                    taken[(y * w + x) as usize] = true;
                    drawn[i] += 1;
                    laid.units.push(Placed { entry: i, x, y, w: 1, h: 1 });
                }
                e => {
                    let layer = e.layer().word();
                    errs.push((line, format!("things ({x},{y}): \"{ch}\" is a {}: it belongs in {layer}", e.kind())));
                }
            }
        }
    }

    // Marks, rects, fills.
    let names = c.layer(Layer::Names);
    let mut cells: Vec<Vec<(i32, i32)>> = vec![Vec::new(); c.legend.len()];
    for y in 0..h {
        for x in 0..w {
            let ch = names.at(x, y);
            if ch == '.' {
                continue;
            }
            let line = names.line(y);
            match ix(ch) {
                None => errs.push((line, format!("names ({x},{y}): \"{ch}\" is not in the legend"))),
                Some(i) if c.legend[i].entry.layer() != Layer::Names => {
                    let e = &c.legend[i].entry;
                    let layer = e.layer().word();
                    errs.push((line, format!("names ({x},{y}): \"{ch}\" is a {}: it belongs in {layer}", e.kind())));
                }
                Some(i) => cells[i].push((x, y)),
            }
        }
    }
    for (i, l) in c.legend.iter().enumerate() {
        let at = &cells[i];
        if at.is_empty() {
            continue;
        }
        drawn[i] += at.len();
        match &l.entry {
            Entry::Mark { name, .. } => {
                if at.len() != 1 {
                    errs.push((l.line, format!("mark {name} is drawn {} times, not once", at.len())));
                }
                laid.marks.push(Placed { entry: i, x: at[0].0, y: at[0].1, w: 1, h: 1 });
            }
            Entry::Rect(name) => {
                let (a, b) = (at[0], *at.last().unwrap_or(&at[0]));
                if at.len() > 2 || b.0 < a.0 {
                    errs.push((
                        l.line,
                        format!("rect {name}: draw its top-left and bottom-right corners (or one cell), not {at:?}"),
                    ));
                }
                laid.rects.push(Placed { entry: i, x: a.0, y: a.1, w: b.0 - a.0 + 1, h: b.1 - a.1 + 1 });
            }
            Entry::Fill(_) => laid.fills.push((i, at.clone())),
            _ => {}
        }
    }
    laid.marks.sort_by_key(|p| (p.y, p.x, p.entry));
    laid.rects.sort_by_key(|p| (p.y, p.x, p.entry));

    // Claims.
    let claims = c.layer(Layer::Claims);
    for y in 0..h {
        for x in 0..w {
            let ch = claims.at(x, y);
            if ch != '.' && ch != 'x' {
                errs.push((claims.line(y), format!("claims ({x},{y}): \"{ch}\": only x (claimed) and . (open)")));
            }
            laid.claims.push(ch == 'x');
        }
    }

    // Every entry drawn; keyed things once.
    for (i, l) in c.legend.iter().enumerate() {
        let n = drawn[i];
        if n == 0 {
            errs.push((l.line, format!("legend \"{}\" ({}) is never drawn", l.ch, l.entry.kind())));
        }
        // (A prop of an unknown def is laid out cell by cell; its own error says why.)
        let once = match &l.entry {
            Entry::Prop { key: Some(k), def, .. } if facts.prop(def).is_some() => Some(k),
            Entry::Unit { key: Some(k), .. } => Some(k),
            _ => None,
        };
        let count = laid.props.iter().chain(&laid.units).filter(|p| p.entry == i).count();
        if let (Some(k), true) = (once, count > 1) {
            errs.push((
                l.line,
                format!("{} {k} is keyed and drawn {count} times; only anonymous things repeat", l.entry.kind()),
            ));
        }
    }

    lint_defs(c, facts, &mut errs);
    lint_box(c, &laid, facts, &mut errs);
    (laid, errs)
}

/// Every def and unit the legend names exists.
fn lint_defs(c: &Chunk, facts: &dyn Facts, errs: &mut Vec<LineError>) {
    for l in &c.legend {
        match &l.entry {
            Entry::Prop { def, .. } if facts.prop(def).is_none() => {
                errs.push((l.line, format!("unknown prop def \"{def}\"")));
            }
            Entry::Unit { def, .. } if !facts.unit(def) => errs.push((l.line, format!("unknown unit def \"{def}\""))),
            _ => {}
        }
    }
}

/// Is a box cell open to feet: a tile that does not block and no solid prop over it?
fn walkable(c: &Chunk, laid: &Laid, facts: &dyn Facts, x: i32, y: i32) -> bool {
    if laid.tile(c, x, y).flags() & F_SOLID != 0 {
        return false;
    }
    !laid.props.iter().any(|p| {
        let solid = match &c.legend[p.entry].entry {
            Entry::Prop { def, .. } => facts.prop(def).is_some_and(|(_, _, s)| s),
            _ => false,
        };
        solid && x >= p.x && y >= p.y && x < p.x + p.w && y < p.y + p.h
    })
}

/// The header's cells against the box, and the names within one chunk.
fn lint_box(c: &Chunk, laid: &Laid, facts: &dyn Facts, errs: &mut Vec<LineError>) {
    let (w, h) = (c.w, c.h);
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    if !inside(c.anchor.0, c.anchor.1) {
        errs.push((
            c.head_line("anchor"),
            format!("anchor ({},{}) is outside the {w}x{h} box", c.anchor.0, c.anchor.1),
        ));
    }
    let gl = c.head_line("gates");
    for (n, &(side, along)) in c.gates.iter().enumerate() {
        let (len, inner) = match side {
            Side::N => (w, (along, 0)),
            Side::S => (w, (along, h - 1)),
            Side::W => (h, (0, along)),
            Side::E => (h, (w - 1, along)),
        };
        let name = format!("{}{along}", side.letter());
        if along >= len {
            errs.push((gl, format!("gate {name} is off the {} side ({len} cells)", side.letter())));
            continue;
        }
        if c.gates[..n].contains(&(side, along)) {
            errs.push((gl, format!("gate {name} is given twice")));
        }
        if !walkable(c, laid, facts, inner.0, inner.1) {
            errs.push((gl, format!("gate {name}: the cell inside it ({},{}) is not walkable", inner.0, inner.1)));
        }
    }
    if !c.gates.iter().any(|(s, _)| *s == c.face) {
        errs.push((c.head_line("face"), format!("face {}: no gate on that side", c.face.letter())));
    }
    for (n, (s, x, y)) in c.slots.iter().enumerate() {
        if !inside(*x, *y) {
            errs.push((s.line, format!("slot {} ({x},{y}) is outside the box", s.name)));
        }
        if c.slots[..n].iter().any(|(o, _, _)| o.name == s.name) {
            errs.push((s.line, format!("slot {} is given twice", s.name)));
        }
    }
    for p in &laid.units {
        let Entry::Unit { def, key, route, .. } = &c.legend[p.entry].entry else { continue };
        let who = key.as_deref().unwrap_or(def);
        let line = c.layer(Layer::Things).line(p.y);
        if !walkable(c, laid, facts, p.x, p.y) {
            errs.push((line, format!("unit {who} at ({},{}) stands on ground feet cannot take", p.x, p.y)));
        }
        for &(x, y, _) in route {
            if !inside(x, y) {
                errs.push((c.legend[p.entry].line, format!("unit {who}: waypoint ({x},{y}) is outside the box")));
            }
        }
    }
    // A name is given once per kind within a chunk (a prop and a mark may share one: the fountain).
    let mut names: Vec<(&'static str, String, usize)> = Vec::new();
    for l in &c.legend {
        match &l.entry {
            Entry::Prop { key: Some(k), .. } => names.push(("prop", k.clone(), l.line)),
            Entry::Unit { key: Some(k), .. } => names.push(("unit", k.clone(), l.line)),
            Entry::Mark { name, .. } => names.push(("mark", name.clone(), l.line)),
            Entry::Rect(name) => names.push(("rect", name.clone(), l.line)),
            Entry::Fill(id) => names.push(("fill", id.clone(), l.line)),
            _ => {}
        }
    }
    for (a, _) in &c.around {
        names.push(("rect", a.name.clone(), a.line));
    }
    for (n, (kind, name, line)) in names.iter().enumerate() {
        if names[..n].iter().any(|(k, m, _)| k == kind && m == name) {
            errs.push((*line, format!("{kind} \"{name}\" is named twice in this chunk")));
        }
    }
}
