//! The dungeon generator (DUNGEONS.md §2.5, `generate.ts`): authored mission, generated space.
//! It returns an ordinary `Blueprint`, so the solver, the save and the renderer do not know a
//! dungeon was generated.
//!
//! 1. choose   a template for each node from its pool, no template twice      (`layout.rs`)
//! 2. embed    rooms onto the bay lattice, critical nodes first                (`layout.rs`)
//! 3. route    a corridor for every edge, along the lines between bays         (`layout.rs`)
//! 4. lock     gates, verb props, lock-ins and their way in, flag gates, state gates (and, in 5,
//!    the one list per control that drives them both ways)
//! 5. fill     lamps on the walls by rule (`lights.rs`), holdings into sockets, the enemy mix by
//!    heat, dressing
//! 6. name     bound things get their contract name, the rest `<zone>_<node>_<socket>`: interned
//!    at build (`MissionNode::names`), so nothing here formats a name the story can lean on
//! 7. emit     the Blueprint and the trigger rows it carries
//!
//! A generated name comes from node and socket, never from a coordinate or the attempt, so a
//! re-rolled layout cannot hand the same jar out twice or lose one (`grow` keys on the name).
//!
//! Dice: the embedding draws from `DunChoose` and `DunEmbed` (`layout.rs`); per room, the
//! pushables and the enemy mix from `DunFill`, the dressing from `DunDress` (`a` = the node).
//! The lamps throw none.

use jane_core::action::{Action, CameraMode, Cond, Condition, FlagKey, FlagOp, FlagTest, Stat};
use jane_core::blueprint::{Door, Mark, PropSpawn, Trigger, TriggerMode, UnitSpawn, Waypoint, ZONE_ATTEMPTS};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Cell, Key, NameId, PropDefId, Rect, Sfc32, TemplateId, TextRef, Tile, UnitDefId, ZoneId};
use jane_data::{
    BAY_H, BAY_W, BORDER, MissionDef, MissionEdgeKind, MissionNodeKind, MissionNodeNames, MissionPlacement,
    MissionProp, RoomDoor, RoomShape, RoomSide, RoomSocketKind, RoomTemplate, catalog,
};

use super::bind::Binding;
use super::layout::{Corridor, Lanes, Layout, embed, embed_fallback};
use super::lights::{LitCorridor, LitRoom, family_index, mount_on, place_lamps};
use crate::steps::{Step, dice};

/// A placed room, as the checks and the viewer want it.
#[derive(Clone, Debug)]
pub struct RoomInfo {
    /// Node index.
    pub node: usize,
    pub template: jane_core::TemplateId,
    pub shape: &'static RoomShape,
    /// Zone cell of the shape's (0, 0).
    pub x: i32,
    pub y: i32,
    /// The whole floor (the template's `room` rect), in zone cells.
    pub rect: Rect,
    /// A cell to walk to that stands for "in this room".
    pub centre: (i32, i32),
    /// Doors in use: indices into `shape.doors`, in corridor order.
    pub doors: Vec<usize>,
    /// The names of this node's sockets, marks and rects for this template.
    pub names: &'static MissionNodeNames,
}

impl RoomInfo {
    /// The shape's box in zone cells.
    pub fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, i32::from(self.shape.w), i32::from(self.shape.h))
    }
}

/// A prop an edge stands in its corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeLock {
    pub edge: usize,
    pub kind: MissionEdgeKind,
    pub prop: Key,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockinInfo {
    pub node: usize,
    pub gate: Key,
    pub way_in: Key,
    pub mark: Key,
    pub rect: Key,
    pub lock: Key,
    pub clear: Key,
}

/// A prop that flips a state, and the state (an index into `MissionDef::states`) it flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlInfo {
    pub state: u8,
    pub prop: Key,
}

/// Everything the checks and the viewer want to know about how a blueprint was made.
#[derive(Clone, Debug)]
pub struct BuildInfo {
    pub mission: &'static MissionDef,
    pub layout: Option<Layout>,
    /// In placement order.
    pub rooms: Vec<RoomInfo>,
    pub locks: Vec<EdgeLock>,
    pub lockins: Vec<LockinInfo>,
    pub controls: Vec<ControlInfo>,
    /// What each corridor carved, by corridor (the layout's order): the viewer draws these.
    pub corridors: Vec<LitCorridor>,
    /// Why there is no dungeon here, when there is not. A candidate with errors is never accepted.
    pub errors: Vec<String>,
    /// The attempts before this one that were refused, each with the first reason given
    /// (filled by `build_with`).
    pub rejected: Vec<(u8, String)>,
}

impl BuildInfo {
    /// The room a node stands in, if it was placed.
    pub fn room_of(&self, node: usize) -> Option<&RoomInfo> {
        self.rooms.iter().find(|r| r.node == node)
    }
}

/// A candidate: the blueprint and how it was made (PORT.md §6.e `Built { blueprint, info }`).
#[derive(Clone, Debug)]
pub struct Built {
    pub blueprint: Blueprint,
    pub info: BuildInfo,
}

const fn out(side: RoomSide) -> (i32, i32) {
    match side {
        RoomSide::N => (0, -1),
        RoomSide::S => (0, 1),
        RoomSide::W => (-1, 0),
        RoomSide::E => (1, 0),
    }
}

fn footprint(p: PropDefId) -> (i32, i32) {
    let d = catalog().story.prop(p);
    (i32::from(d.w), i32::from(d.h))
}

fn cell(x: i32, y: i32) -> Cell {
    Cell::new(x as u16, y as u16)
}

/// The prop rows the generator names itself.
#[derive(Debug)]
struct Rows {
    chest: Option<PropDefId>,
    way_in: Option<PropDefId>,
    door: Option<PropDefId>,
    notice: Option<PropDefId>,
    /// What a jar or a page gives when the mission does not say (DUNGEONS.md §3): row, stat,
    /// amount, hidden once found.
    found: [(Option<PropDefId>, Stat, i16, bool); 3],
}

impl Rows {
    fn new() -> Self {
        let id = |s: &str| catalog().story.prop_id(s);
        Self {
            chest: id("chest"),
            way_in: id("way_in"),
            door: id("door"),
            notice: id("notice"),
            found: [
                (id("jar"), Stat::Strength, 2, false),
                (id("jar_big"), Stat::Strength, 14, false),
                (id("leaf_page"), Stat::Spirit, 2, true),
            ],
        }
    }
}

/// The canvas: the blueprint being written and the cells a prop or a mark has claimed.
#[derive(Debug)]
struct Kit {
    bp: Blueprint,
    claimed: Vec<bool>,
    w: i32,
    h: i32,
}

impl Kit {
    fn get(&self, x: i32, y: i32) -> Tile {
        self.bp.tiles.read(x, y, Tile::Void)
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, t: Tile) {
        self.bp.tiles.fill_rect(Rect::new(x, y, w, h), t);
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    fn claim(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for j in y..y + h {
            for i in x..x + w {
                if self.inside(i, j) {
                    self.claimed[(j * self.w + i) as usize] = true;
                }
            }
        }
    }

    /// A `w x h` footprint at (x, y) sits on open, unclaimed floor.
    fn fits(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        (y..y + h).all(|j| {
            (x..x + w).all(|i| {
                self.inside(i, j) && self.get(i, j).flags() & F_SOLID == 0 && !self.claimed[(j * self.w + i) as usize]
            })
        })
    }

    fn mark(&mut self, name: Key, x: i32, y: i32) {
        self.bp.marks.insert(name, Mark { cell: cell(x, y), facing: None });
        self.claim(x - 1, y - 1, 3, 3);
    }

    fn rect(&mut self, name: Key, r: Rect) {
        self.bp.rects.insert(name, r);
    }

    fn prop(&mut self, key: Key, def: PropDefId, x: i32, y: i32, w: i32, h: i32) -> &mut PropSpawn {
        self.claim(x, y, w, h);
        self.bp.props.push(PropSpawn {
            key,
            def,
            cell: cell(x, y),
            locked: false,
            key_tag: None,
            hidden: false,
            on: false,
            to: None,
            loot: Vec::new(),
            use_list: None,
            release: None,
            needs: Vec::new(),
            talk: None,
            label: None,
            night_lock: None,
            under: None,
            under_when: None,
        });
        self.bp.props.last_mut().expect("just pushed")
    }

    fn unit(&mut self, key: Key, def: UnitDefId, x: i32, y: i32, patrol: Vec<Waypoint>, phase: u8) {
        self.claim(x, y, 1, 1);
        self.bp.units.push(UnitSpawn { key, def, cell: cell(x, y), facing: None, patrol, phase });
    }
}

fn flag_is_set(flag: NameId, not: bool) -> Cond {
    Cond { not, c: Condition::Flag { key: FlagKey::Named(Key::Name(flag)), test: FlagTest::NonZero } }
}

/// A room as the scatter walks it: its node, its floor rect, its doors' centre cells.
type ScatterRoom = (usize, Rect, Vec<(i32, i32)>);

/// One candidate's build.
struct Gen<'l> {
    m: &'static MissionDef,
    zone: ZoneId,
    seed: u32,
    attempt: u8,
    layout: &'l Layout,
    k: Kit,
    info: BuildInfo,
    lanes: Lanes,
    rows: Rows,
    /// The interned `@self`.
    at_self: Option<NameId>,
    /// Node -> index into `info.rooms`.
    room_at: Vec<Option<usize>>,
    /// Gates that stand or fall with a state: (state, value it is open for, gate).
    state_gates: Vec<(u8, u8, Key)>,
    /// Every wall lamp, for the lists that switch them.
    lamps: Vec<Key>,
    /// The template harness: every door of every room opens onto a dead-end stub with a mark at
    /// its end ([`door_mark`]), so a room can be solved alone from each door.
    stubs: bool,
}

/// The mark at the end of a harness stub: `<zone>_room_<node>_door_<door>`.
pub fn door_mark(m: &MissionDef, node: usize, door: &RoomDoor) -> String {
    format!("{}_room_{}_door_{}", m.id, m.nodes[node].id, door.id)
}

impl Gen<'_> {
    fn err(&mut self, e: impl std::fmt::Display) {
        self.info.errors.push(format!("dungeon \"{}\": {e}", self.m.id));
    }

    /// A name the mission provides (a spawn's), else one made for this blueprint (a lamp's).
    fn key_for(&mut self, name: &str) -> Key {
        let c = catalog();
        match self.m.provides.iter().find(|p| c.name(p.name) == name) {
            Some(p) => Key::Name(p.name),
            None => self.k.bp.local(name),
        }
    }

    fn text(&mut self, s: &str) -> TextRef {
        match self.k.bp.texts.iter().position(|t| t == s) {
            Some(i) => TextRef::Local(i as u16),
            None => self.k.bp.push_text(s.to_owned()),
        }
    }

    fn room(&self, node: usize) -> &RoomInfo {
        &self.info.rooms[self.room_at[node].expect("a corridor joins placed rooms")]
    }

    fn door_at(&self, node: usize, door: usize) -> (&'static RoomDoor, i32, i32) {
        let r = self.room(node);
        let d = &r.shape.doors[door];
        (d, r.x + i32::from(d.x), r.y + i32::from(d.y))
    }

    // --- rooms -------------------------------------------------------------------------------

    fn rooms(&mut self) {
        let dungeons = &catalog().dungeons;
        let (wall, floor) = (self.m.wall, self.m.floor);
        let layout = self.layout;
        for p in &layout.placements {
            let node = usize::from(p.node);
            let n = &self.m.nodes[node];
            let template = dungeons.template(p.template);
            let Some(shape) = template.shape(p.turn, p.mirror) else {
                self.err(format!("{} has no turn {:?} mirror {}", template.id, p.turn, p.mirror));
                continue;
            };
            let Some(names) = n.names.iter().find(|r| r.template == p.template) else {
                self.err(format!("node \"{}\" has no names for {}", n.id, template.id));
                continue;
            };
            let x = BORDER + i32::from(p.bay.0) * BAY_W + shape.ox;
            let y = BORDER + i32::from(p.bay.1) * BAY_H + shape.oy;
            let doors: Vec<usize> = if self.stubs {
                (0..shape.doors.len()).collect()
            } else {
                layout.corridors.iter().flat_map(|c| [c.a, c.b]).filter(|u| u.node == node).map(|u| u.door).collect()
            };
            // The three cells of each door in use, and the three just inside them.
            let mut open: Vec<(i32, i32)> = Vec::new();
            for &di in &doors {
                let d = &shape.doors[di];
                let along = if matches!(d.side, RoomSide::N | RoomSide::S) { (1, 0) } else { (0, 1) };
                let (ox, oy) = out(d.side);
                for s in -1..=1 {
                    let (cx, cy) = (i32::from(d.x) + along.0 * s, i32::from(d.y) + along.1 * s);
                    open.push((cx, cy));
                    open.push((cx - ox, cy - oy));
                }
            }
            for j in 0..shape.h {
                for i in 0..shape.w {
                    let is_open = || open.contains(&(i32::from(i), i32::from(j)));
                    let tile = match shape.cell(template.tiles, i, j) {
                        jane_data::RoomCell::Wall => wall,
                        jane_data::RoomCell::Floor => floor,
                        // A sill is kept only under a door that is in use; a door nobody uses is wall again.
                        jane_data::RoomCell::Sill => {
                            if is_open() {
                                Tile::Sill
                            } else {
                                floor
                            }
                        }
                        jane_data::RoomCell::Door => {
                            if is_open() {
                                floor
                            } else {
                                wall
                            }
                        }
                        jane_data::RoomCell::Tile(t) => t,
                    };
                    self.k.bp.tiles.set(x + i32::from(i), y + i32::from(j), tile);
                }
            }
            let room = shape.rects[0];
            let centre =
                template.mark_index("centre").map_or((x + i32::from(shape.w >> 1), y + i32::from(shape.h >> 1)), |i| {
                    (x + i32::from(shape.marks[i].x), y + i32::from(shape.marks[i].y))
                });
            self.room_at[node] = Some(self.info.rooms.len());
            self.info.rooms.push(RoomInfo {
                node,
                template: p.template,
                shape,
                x,
                y,
                rect: Rect::new(x + room.x, y + room.y, room.w, room.h),
                centre,
                doors,
                names,
            });
        }
    }

    // --- corridors ---------------------------------------------------------------------------

    /// From just outside the rim to the edge of the lane: the doorway's own few cells.
    fn carve_stub(&mut self, node: usize, door: usize, port: usize) {
        let (d, x, y) = self.door_at(node, door);
        let (px, py) = self.lanes.cell(port);
        let floor = self.m.floor;
        match d.side {
            RoomSide::N => self.k.fill(x - 1, py + 2, 3, y - (py + 2), floor),
            RoomSide::S => self.k.fill(x - 1, y + 1, 3, py - 2 - y, floor),
            RoomSide::W => self.k.fill(px + 2, y - 1, x - (px + 2), 3, floor),
            RoomSide::E => self.k.fill(x + 1, y - 1, px - 2 - x, 3, floor),
        }
    }

    fn corridors(&mut self) {
        let (m, floor, layout) = (self.m, self.m.floor, self.layout);
        for c in &layout.corridors {
            let boss = |node: usize| matches!(m.nodes[node].kind, MissionNodeKind::Boss | MissionNodeKind::Miniboss);
            let mut lit = LitCorridor { rects: Vec::new(), dark: boss(c.a.node) || boss(c.b.node) };
            // The stubs are the doorways' own few cells: the room's pair of lamps beside the door lights them.
            self.carve_stub(c.a.node, c.a.door, usize::from(c.lane[0]));
            self.carve_stub(c.b.node, c.b.door, usize::from(c.lane[c.lane.len() - 1]));
            for (i, &n) in c.lane.iter().enumerate() {
                let (ax, ay) = self.lanes.cell(usize::from(n));
                let r = Rect::new(ax - 1, ay - 1, 3, 3);
                self.k.fill(r.x, r.y, r.w, r.h, floor);
                lit.rects.push(r);
                let Some(&next) = c.lane.get(i + 1) else { break };
                let (bx, by) = self.lanes.cell(usize::from(next));
                let r = Rect::new(ax.min(bx) - 1, ay.min(by) - 1, (bx - ax).abs() + 3, (by - ay).abs() + 3);
                self.k.fill(r.x, r.y, r.w, r.h, floor);
                lit.rects.push(r);
            }
            self.info.corridors.push(lit);
        }
        if self.stubs {
            self.door_stubs();
        }
    }

    /// A rect the zone's own contract (`zones.json`) names and nothing here drew is the whole
    /// zone: the story's rows for the Burial name `everywhere`, which the generator calls
    /// `<zone>_all` (the TypeScript's `burial.ts` aliased it the same way).
    fn zone_rects(&mut self) {
        let (w, h) = (self.k.w, self.k.h);
        for &r in catalog().county.zone(self.zone).contract.rects {
            if !self.k.bp.rects.contains_key(&Key::Name(r)) {
                self.k.rect(Key::Name(r), Rect::new(0, 0, w, h));
            }
        }
    }

    /// Every door of every room onto a stub ending in a marked 3x3 of floor (the harness's).
    fn door_stubs(&mut self) {
        let floor = self.m.floor;
        for p in &self.layout.placements {
            let node = usize::from(p.node);
            let Some(ri) = self.room_at[node] else { continue };
            let shape = self.info.rooms[ri].shape;
            for (di, d) in shape.doors.iter().enumerate() {
                let port = self.lanes.port(i32::from(p.bay.0), i32::from(p.bay.1), shape.bays, d);
                self.carve_stub(node, di, port);
                let (px, py) = self.lanes.cell(port);
                self.k.fill(px - 1, py - 1, 3, 3, floor);
                let name = door_mark(self.m, node, d);
                let key = self.k.bp.local(&name);
                self.k.mark(key, px, py);
            }
        }
    }

    // --- locks -------------------------------------------------------------------------------

    /// A gate in the one corridor cell that is always there: just outside the far room's door.
    fn place_gate(&mut self, c: &Corridor, key: Key, rows: [PropDefId; 2]) -> Option<&mut PropSpawn> {
        let (d, x, y) = self.door_at(c.b.node, c.b.door);
        let (ox, oy) = out(d.side);
        let (gx, gy) = (x + ox, y + oy);
        if footprint(rows[0]) != (3, 1) || footprint(rows[1]) != (1, 3) {
            self.err("gate rows must be 3x1 and 1x3");
            return None;
        }
        Some(if matches!(d.side, RoomSide::N | RoomSide::S) {
            self.k.prop(key, rows[0], gx - 1, gy, 3, 1)
        } else {
            self.k.prop(key, rows[1], gx, gy - 1, 1, 3)
        })
    }

    /// A prop that fills the corridor: in the middle of a straight stub, or of the last lane
    /// before the far room.
    fn place_block(&mut self, c: &Corridor, key: Key, def: PropDefId) -> Option<&mut PropSpawn> {
        let (fw, fh) = footprint(def);
        let (cx, cy, vertical) = if c.lane.len() == 1 {
            let (cx, cy) = self.lanes.cell(usize::from(c.lane[0]));
            let side = self.door_at(c.b.node, c.b.door).0.side;
            (cx, cy, matches!(side, RoomSide::N | RoomSide::S))
        } else {
            let (ax, ay) = self.lanes.cell(usize::from(c.lane[c.lane.len() - 2]));
            let (bx, by) = self.lanes.cell(usize::from(c.lane[c.lane.len() - 1]));
            ((ax + bx) >> 1, (ay + by) >> 1, ax == bx)
        };
        let row = catalog().story.prop(def).id;
        if vertical {
            if fw != 3 {
                self.err(format!("\"{row}\" is {fw} wide and cannot seal a corridor of 3"));
                return None;
            }
            Some(self.k.prop(key, def, cx - 1, cy - (fh >> 1), fw, fh))
        } else {
            if fh > 3 {
                self.err(format!("\"{row}\" is {fh} tall and cannot stand in a corridor of 3"));
                return None;
            }
            let x0 = cx - (fw >> 1);
            // The steps are two cells tall: the corridor narrows to a neck exactly as long as they are.
            if fh < 3 {
                self.k.fill(x0, cy - 1 + fh, fw, 3 - fh, self.m.wall);
            }
            Some(self.k.prop(key, def, x0, cy - 1, fw, fh))
        }
    }

    fn locks(&mut self) {
        let m = self.m;
        let (w, h) = (self.k.w, self.k.h);
        self.k.rect(Key::Name(m.all_rect), Rect::new(0, 0, w, h));
        let layout = self.layout;
        for c in &layout.corridors {
            let edge = &m.edges[c.edge];
            let from = c.a.node;
            let mut gate_key: Option<Key> = None;
            let gate = edge.gate;
            for kind in std::iter::once(&edge.kind).chain(edge.also) {
                match *kind {
                    MissionEdgeKind::Key { tag, label } => {
                        let Some(g) = gate else { continue };
                        let key = Key::Name(g.name);
                        if let Some(p) = self.place_gate(c, key, g.rows) {
                            p.locked = true;
                            p.key_tag = Some(Key::Name(tag));
                            p.label = label.map(TextRef::Text);
                        }
                        gate_key = Some(key);
                        self.info.locks.push(EdgeLock { edge: c.edge, kind: *kind, prop: key });
                    }
                    MissionEdgeKind::Verb { prop, needs, name, label, toast, use_list, .. } => {
                        let key = Key::Name(name);
                        let mut list = vec![Action::Hide(key)];
                        if let Some(t) = toast {
                            list.push(Action::Toast(TextRef::Text(t)));
                        }
                        if let Some(l) = use_list {
                            let b = Binding::new(&m.nodes[from], self.at_self, Some(key));
                            list.extend(b.actions(&mut self.k.bp, l));
                        }
                        let list = self.k.bp.push_list(list);
                        if let Some(p) = self.place_block(c, key, prop) {
                            p.needs = needs.to_vec();
                            p.use_list = Some(list);
                            p.label = label.map(TextRef::Text);
                        }
                        self.info.locks.push(EdgeLock { edge: c.edge, kind: *kind, prop: key });
                    }
                    MissionEdgeKind::Oneway { flag, trigger, label } => {
                        let Some(g) = gate else { continue };
                        let key = Key::Name(g.name);
                        let label = match label {
                            Some(t) => TextRef::Text(t),
                            None => self.text("The gate"),
                        };
                        if let Some(p) = self.place_gate(c, key, g.rows) {
                            p.locked = true;
                            p.label = Some(label);
                        }
                        gate_key = Some(key);
                        let when = self.k.bp.push_conds(vec![flag_is_set(flag, false)]);
                        let actions = self.k.bp.push_list(vec![Action::Unlock(key)]);
                        self.k.bp.triggers.insert(
                            Key::Name(trigger),
                            Trigger {
                                rect: Key::Name(m.all_rect),
                                mode: TriggerMode::While,
                                once: true,
                                when: Some(when),
                                actions,
                                reset: None,
                            },
                        );
                        self.info.locks.push(EdgeLock { edge: c.edge, kind: *kind, prop: key });
                    }
                    MissionEdgeKind::State { var, is, label } => {
                        // Passable only while the state has this value. No key fits it: the state's
                        // controls drive it, and the list that does so is written in `fill`, once,
                        // for every such gate.
                        let Some(g) = gate else { continue };
                        let key = Key::Name(g.name);
                        let initial = m.states[usize::from(var)].initial;
                        if let Some(p) = self.place_gate(c, key, g.rows) {
                            p.locked = is != initial;
                            p.label = label.map(TextRef::Text);
                        }
                        gate_key = Some(key);
                        self.state_gates.push((var, is, key));
                        self.info.locks.push(EdgeLock { edge: c.edge, kind: *kind, prop: key });
                    }
                    MissionEdgeKind::Open | MissionEdgeKind::Sight | MissionEdgeKind::Lockin { .. } => {}
                }
            }
            let lockin =
                std::iter::once(&edge.kind).chain(edge.also).find(|k| matches!(k, MissionEdgeKind::Lockin { .. }));
            if let Some(&lockin) = lockin {
                let gate_key = match (gate_key, gate) {
                    (Some(k), _) => k,
                    // A lock-in with no key of its own: the gate stands open until the room seals.
                    (None, Some(g)) => {
                        let key = Key::Name(g.name);
                        self.place_gate(c, key, g.rows);
                        key
                    }
                    (None, None) => {
                        self.err(format!("edge {} seals a room and has no gate", c.edge));
                        continue;
                    }
                };
                self.write_lockin(c, lockin, gate_key);
            }
        }
        if gate_missing(m) {
            self.err("an edge that gates has no gate row");
        }
    }

    /// One function writes every lock-in, so none can forget its reset (DUNGEONS.md §2.5). The way
    /// in is hidden until the room seals, so it can never be a way round a keyed gate; it is how a
    /// friend who was late, or who died and walked back, follows.
    fn write_lockin(&mut self, c: &Corridor, spec: MissionEdgeKind, gate: Key) {
        let MissionEdgeKind::Lockin { spawn, toast, reward, rect: rect_name, mark: mark_name, way_in, lock, clear } =
            spec
        else {
            return;
        };
        let m = self.m;
        let zone = self.zone;
        let node = c.b.node;
        let n = &m.nodes[node];
        let (d, x, y) = self.door_at(node, c.b.door);
        let (ox, oy) = out(d.side);
        let (template, shape, room_rect, rx, ry) = {
            let r = self.room(node);
            (catalog().dungeons.template(r.template), r.shape, r.rect, r.x, r.y)
        };
        // The rect that seals the room is the template's `inner`, or the whole floor. Keep it the
        // whole floor unless there is a reason: the clear only fires while someone stands in it, and
        // a death only re-opens the gate when nobody alive is left in it, so a rect that stops short
        // of the walls forgets whoever is at a lever by the wall. The gate stands outside the room,
        // in the corridor, so it cannot land on anyone however early the rect starts (C12).
        let rect = template.rect_index("inner").map_or(room_rect, |i| {
            let r = shape.rects[i];
            Rect::new(rx + r.x, ry + r.y, r.w, r.h)
        });
        self.k.rect(Key::Name(rect_name), rect);
        // Where she lands: straight in from the door, the first clear floor cell that is well inside
        // the rect (two cells, so that arriving there is arriving in the fight, not on its edge).
        let reach = i32::from(shape.w.max(shape.h));
        let (mut mx, mut my) = (x, y);
        for step in 1..reach {
            mx = x - ox * step;
            my = y - oy * step;
            if mx >= rect.x + 2
                && my >= rect.y + 2
                && mx < rect.right() - 2
                && my < rect.bottom() - 2
                && self.k.fits(mx, my, 1, 1)
            {
                break;
            }
        }
        let mark = Key::Name(mark_name);
        self.k.mark(mark, mx, my);
        // The way in: beside the gate, on the side she is shut out on.
        let way_in = Key::Name(way_in);
        let (gx, gy) = (x + ox, y + oy);
        let vertical = matches!(d.side, RoomSide::N | RoomSide::S);
        let wx = if vertical {
            gx - 1
        } else if ox > 0 {
            gx + 1
        } else {
            gx - 2
        };
        let wy = if vertical { gy + oy } else { gy };
        let label = self.text("A way in");
        match self.rows.way_in {
            Some(def) => {
                let p = self.k.prop(way_in, def, wx, wy, 2, 1);
                p.hidden = true;
                p.to = Some(Door { zone, mark });
                p.label = Some(label);
            }
            None => self.err("no \"way_in\" prop row"),
        }

        let boss = n
            .holds
            .iter()
            .find(|hd| hd.unit.is_some() && (hd.socket.starts_with("boss") || n.kind == MissionNodeKind::Boss))
            .map(|hd| Key::Name(hd.key));
        let spawned: Vec<(Key, UnitDefId, Key)> = spawn
            .map(|s| s.at.iter().zip(s.names).map(|(&at, &u)| (Key::Name(u), s.def, Key::Name(at))).collect())
            .unwrap_or_default();
        let foes: Vec<Key> =
            if spawn.is_some() { spawned.iter().map(|s| s.0).collect() } else { boss.into_iter().collect() };
        let rect_key = Key::Name(rect_name);
        let mut actions = vec![Action::Lock(gate), Action::Show(way_in)];
        let mut reset = vec![Action::Unlock(gate), Action::Hide(way_in)];
        for &(u, def, at) in &spawned {
            actions.push(Action::Spawn { key: u, def, at });
            reset.push(Action::Despawn(u));
        }
        let boss_fight = spawn.is_none() && boss.is_some();
        if boss_fight {
            actions.push(Action::Aggro(foes[0]));
            actions.push(Action::Camera { mode: CameraMode::Lock, rect: Some(rect_key) });
            reset.push(Action::Camera { mode: CameraMode::Follow, rect: None });
        }
        let toast = match toast {
            Some(t) => TextRef::Text(t),
            None => self.text("The gate drops behind you."),
        };
        actions.push(Action::Toast(toast));
        let mut done = vec![
            Action::Unlock(gate),
            Action::Hide(way_in),
            Action::Flag { key: FlagKey::Named(Key::Name(clear)), op: FlagOp::Set(1) },
        ];
        if let Some(r) = reward {
            done.push(Action::Unlock(Key::Name(r)));
        }
        if boss_fight {
            done.push(Action::Camera { mode: CameraMode::Follow, rect: None });
        }
        let mut when = vec![flag_is_set(clear, true)];
        when.extend(foes.first().map(|&u| Cond { not: true, c: Condition::Dead(u) }));
        let bp = &mut self.k.bp;
        let lock_trigger = Trigger {
            rect: rect_key,
            mode: TriggerMode::Enter,
            once: true,
            when: Some(bp.push_conds(when)),
            actions: bp.push_list(actions),
            reset: Some(bp.push_list(reset)),
        };
        let clear_when = foes.iter().map(|&u| Cond { not: false, c: Condition::Dead(u) }).collect::<Vec<_>>();
        let clear_trigger = Trigger {
            rect: rect_key,
            mode: TriggerMode::While,
            once: true,
            when: (!clear_when.is_empty()).then(|| bp.push_conds(clear_when)),
            actions: bp.push_list(done),
            reset: None,
        };
        bp.triggers.insert(Key::Name(lock), lock_trigger);
        bp.triggers.insert(Key::Name(clear), clear_trigger);
        self.info.lockins.push(LockinInfo {
            node,
            gate,
            way_in,
            mark,
            rect: rect_key,
            lock: Key::Name(lock),
            clear: Key::Name(clear),
        });
    }

    // --- lamps: on the walls, by rule (lights.rs) --------------------------------------------

    fn lamps(&mut self) {
        let m = self.m;
        let Some(plan) = m.lights else { return };
        let wall = m.wall;
        let (w, h) = (self.k.w, self.k.h);
        // A lamp the mission hangs itself (a family name in a holding) keeps its wall cell.
        let mut reserved = vec![false; (w * h) as usize];
        // A door or a way out stands against its wall: no lamp hangs over it, nor in the cell beside it.
        let mut doors: Vec<Rect> = Vec::new();
        for r in &self.info.rooms {
            let template = catalog().dungeons.template(r.template);
            for hd in m.nodes[r.node].holds {
                let Some(si) = template.socket_index(hd.socket) else { continue };
                let s = r.shape.sockets[si];
                if matches!(hd.prop, Some(MissionProp::Family(_))) {
                    if let Some((_, x, y)) = mount_on(&self.k.bp.tiles, wall, r.x + s.x, r.y + s.y) {
                        reserved[(y * w + x) as usize] = true;
                    }
                }
                if hd.unit.is_none()
                    && (template.sockets[si].kind == RoomSocketKind::Exit
                        || matches!(hd.prop, Some(MissionProp::Row(p)) if Some(p) == self.rows.door)
                        || hd.to.is_some())
                {
                    doors.push(Rect::new(r.x + s.x, r.y + s.y, s.w, s.h));
                }
            }
        }
        let rooms: Vec<LitRoom> = self
            .info
            .rooms
            .iter()
            .map(|r| LitRoom {
                node: r.node,
                dark: m.nodes[r.node].dark,
                x: r.x,
                y: r.y,
                w: i32::from(r.shape.w),
                h: i32::from(r.shape.h),
            })
            .collect();
        let node_ids: Vec<&str> = m.nodes.iter().map(|n| n.id).collect();
        let lamps = place_lamps(
            m.id,
            &node_ids,
            &plan,
            &self.k.bp.tiles,
            wall,
            &rooms,
            &self.info.corridors,
            &reserved,
            &doors,
        );
        let on = plan.state.is_some_and(|(var, is)| m.states[usize::from(var)].initial == is);
        for l in lamps {
            let key = self.key_for(&l.name);
            let def = plan.family[family_index(l.side)];
            self.k.prop(key, def, l.x, l.y, 1, 1).on = on;
            self.lamps.push(key);
        }
    }

    /// What a control of this state does to the lamps on the way to `value`.
    fn lamps_to(&self, state: u8, value: u8) -> Vec<Action> {
        match self.m.lights.and_then(|p| p.state) {
            Some((var, is)) if var == state => {
                self.lamps.iter().map(|&prop| Action::Switch { prop, on: Some(value == is) }).collect()
            }
            _ => Vec::new(),
        }
    }

    // --- fill: marks, rects, holdings --------------------------------------------------------

    fn fill(&mut self) {
        let m = self.m;
        for ri in 0..self.info.rooms.len() {
            let r = self.info.rooms[ri].clone();
            let n = &m.nodes[r.node];
            let template = catalog().dungeons.template(r.template);
            let shape = r.shape;
            for (i, sr) in shape.rects.iter().enumerate() {
                let name = Key::Name(r.names.rects[i]);
                if !self.k.bp.rects.contains_key(&name) {
                    self.k.rect(name, Rect::new(r.x + sr.x, r.y + sr.y, sr.w, sr.h));
                }
            }
            for (i, mk) in shape.marks.iter().enumerate() {
                self.k.mark(Key::Name(r.names.marks[i]), r.x + i32::from(mk.x), r.y + i32::from(mk.y));
            }
            let mut held: Vec<usize> = Vec::new();
            for hd in n.holds {
                let Some(si) = template.socket_index(hd.socket) else {
                    self.err(format!(
                        "node \"{}\" holds socket \"{}\", which {} does not have",
                        n.id, hd.socket, template.id
                    ));
                    continue;
                };
                held.push(si);
                let s = shape.sockets[si];
                let key = Key::Name(hd.key);
                if let Some(unit) = hd.unit {
                    let mut patrol = Vec::new();
                    for pt in hd.patrol {
                        let Some(mi) = template.mark_index(pt.mark) else {
                            self.err(format!(
                                "\"{}\" patrols by mark \"{}\", which {} does not have",
                                n.id, pt.mark, template.id
                            ));
                            continue;
                        };
                        let mk = shape.marks[mi];
                        patrol.push(Waypoint {
                            cell: cell(r.x + i32::from(mk.x), r.y + i32::from(mk.y)),
                            dwell: pt.dwell,
                        });
                    }
                    if patrol.len() < 2 {
                        patrol.clear();
                    }
                    self.k.unit(key, unit, r.x + s.x + (s.w >> 1), r.y + s.y + (s.h >> 1), patrol, m.phase);
                    continue;
                }
                let (def, px, py) = match hd.prop {
                    Some(MissionProp::Row(p)) => (p, r.x + s.x, r.y + s.y),
                    // A lamp the mission names by its family (`gas_lamp`) hangs on the wall its socket stands against.
                    Some(MissionProp::Family(f)) => match mount_on(&self.k.bp.tiles, m.wall, r.x + s.x, r.y + s.y) {
                        Some((side, x, y)) => (f[family_index(side)], x, y),
                        None => {
                            self.err(format!(
                                "lamp socket {} of {} has no wall beside it to hang on",
                                hd.socket, template.id
                            ));
                            continue;
                        }
                    },
                    None => match self.rows.chest {
                        Some(c) => (c, r.x + s.x, r.y + s.y),
                        None => {
                            self.err("no \"chest\" prop row");
                            continue;
                        }
                    },
                };
                if footprint(def) != (s.w, s.h) {
                    let row = catalog().story.prop(def).id;
                    self.err(format!(
                        "\"{row}\" does not fit socket {} of {} ({}x{})",
                        hd.socket, template.id, s.w, s.h
                    ));
                    continue;
                }
                let guarded = !hd.guarded_by.is_empty();
                let bind = Binding::new(n, self.at_self, Some(key));
                let mut list: Option<Vec<Action>> = hd.use_list.map(|l| bind.actions(&mut self.k.bp, l));
                if list.is_none() {
                    if let Some(&(_, stat, amount, hide)) = self.rows.found.iter().find(|f| f.0 == Some(def)) {
                        let mut v = vec![Action::Grow { stat, amount, id: key }];
                        if hide {
                            v.push(Action::Hide(key));
                        }
                        list = Some(v);
                    }
                }
                // The wall notice is the map: reading it shows every room this seed placed. The
                // generator knows them all, so no mission has to list them (and none can list one
                // that was dropped).
                if list.is_none() && Some(def) == self.rows.notice {
                    let rooms: Vec<Key> = self.info.rooms.iter().map(|x| Key::Name(x.names.rects[0])).collect();
                    let names = self.k.bp.push_names(rooms);
                    list = Some(vec![Action::Reveal(names)]);
                }
                if let Some(state) = hd.controls {
                    // One generated list per control, never hand-written, or a gate is forgotten in one direction.
                    let sv = &m.states[usize::from(state)];
                    let drive = |g: &mut Self, value: u8| -> Vec<Action> {
                        let mut v = vec![Action::Flag {
                            key: FlagKey::Named(Key::Name(sv.flag)),
                            op: FlagOp::Set(i32::from(value != sv.initial)),
                        }];
                        for &(var, is, gate) in &g.state_gates {
                            if var == state {
                                v.push(if is == value { Action::Unlock(gate) } else { Action::Lock(gate) });
                            }
                        }
                        v.extend(g.lamps_to(state, value));
                        if let Some(l) = hd.becomes[usize::from(value)] {
                            v.extend(bind.actions(&mut g.k.bp, l));
                        }
                        v
                    };
                    let there = drive(self, sv.initial);
                    let back = drive(self, 1 - sv.initial);
                    let bp = &mut self.k.bp;
                    let when = bp.push_conds(vec![flag_is_set(sv.flag, false)]);
                    let then = bp.push_list(there);
                    let els = Some(bp.push_list(back));
                    let mut v = vec![Action::If { when, then, els }];
                    v.extend(list.unwrap_or_default());
                    list = Some(v);
                    self.info.controls.push(ControlInfo { state, prop: key });
                }
                let use_list = list.map(|v| self.k.bp.push_list(v));
                let release = hd.release.map(|l| bind.list(&mut self.k.bp, l));
                let zone_to = hd.to.map(|t| Door { zone: t.zone, mark: Key::Name(t.mark) });
                let p = self.k.prop(key, def, px, py, s.w, s.h);
                p.locked = hd.locked || guarded;
                p.key_tag = hd.key_tag.map(Key::Name);
                p.hidden = hd.hidden;
                p.on = hd.on.unwrap_or(false);
                p.loot = hd.loot.map(<[_]>::to_vec).unwrap_or_default();
                p.talk = hd.talk;
                p.needs = hd.needs.to_vec();
                p.use_list = use_list;
                p.release = release;
                p.to = zone_to;
                p.label = hd.label.map(TextRef::Text);
                if guarded {
                    match hd.free_trigger {
                        Some(t) => {
                            let bp = &mut self.k.bp;
                            let when = hd
                                .guarded_by
                                .iter()
                                .map(|&u| Cond { not: false, c: Condition::Dead(Key::Name(u)) })
                                .collect();
                            let trigger = Trigger {
                                rect: Key::Name(r.names.rects[0]),
                                mode: TriggerMode::While,
                                once: true,
                                when: Some(bp.push_conds(when)),
                                actions: bp.push_list(vec![Action::Unlock(key)]),
                                reset: None,
                            };
                            bp.triggers.insert(Key::Name(t), trigger);
                        }
                        None => self.err(format!("guarded holding {} has no trigger name", hd.socket)),
                    }
                }
            }
            for t in n.triggers {
                let bind = Binding::new(n, None, None);
                let bp = &mut self.k.bp;
                let trigger = Trigger {
                    rect: Key::Name(t.rect),
                    mode: t.mode,
                    once: t.once,
                    when: t.when.map(|w| bind.conds(bp, w)),
                    actions: bind.list(bp, t.actions),
                    reset: t.reset.map(|l| bind.list(bp, l)),
                };
                bp.triggers.insert(Key::Name(t.name), trigger);
            }

            let fits_socket = |def: &PropDefId, s: Rect| footprint(*def) == (s.w, s.h);
            // Things to push that the mission did not name: whatever the template offers, seeded.
            let mut fill_rng = dice(self.seed, self.zone, Step::DunFill, self.attempt, r.node as i32, 0);
            for (si, ts) in template.sockets.iter().enumerate() {
                if held.contains(&si) || ts.kind != RoomSocketKind::Push {
                    continue;
                }
                let s = shape.sockets[si];
                let options: Vec<PropDefId> = ts.options.iter().copied().filter(|o| fits_socket(o, s)).collect();
                if let Some(&def) = fill_rng.pick(&options) {
                    self.k.prop(Key::Name(r.names.sockets[si]), def, r.x + s.x, r.y + s.y, s.w, s.h);
                }
            }

            self.enemies(&r, template, &mut fill_rng);

            let mut dress_rng = dice(self.seed, self.zone, Step::DunDress, self.attempt, r.node as i32, 0);
            for (si, ts) in template.sockets.iter().enumerate() {
                if ts.kind != RoomSocketKind::Dress {
                    continue;
                }
                let name = ts.id.split(':').nth(1).unwrap_or("");
                let Some(row) = m.dress.iter().find(|d| d.name == name) else { continue };
                if !dress_rng.chance(row.chance) {
                    continue;
                }
                let s = shape.sockets[si];
                let options: Vec<PropDefId> = row.props.iter().copied().filter(|o| fits_socket(o, s)).collect();
                if let Some(&def) = dress_rng.pick(&options) {
                    self.k.prop(Key::Name(r.names.sockets[si]), def, r.x + s.x, r.y + s.y, s.w, s.h);
                }
            }
        }
    }

    /// The scatter (DUNGEONS.md: "Dressing (barrel piles, torch runs) is the scatter"): after
    /// everything the mission and the templates place, each room's open floor takes the
    /// dungeon's floor marks (`dress.scatter_floor`: bones, stains, papers, leaves) and the cells
    /// under its wall's face take its wall hangings (`dress.scatter_wall`: cobwebs, notices,
    /// portraits, moss), each cell by the row's chance. Dressing only: a solid row is refused,
    /// nothing stands within three cells of a doorway, nothing on a cell anything has claimed,
    /// no two hangings within two cells of each other along a wall. Seeded per node
    /// (`DunDress`, `b` = 1), so it moves nothing else the dice decide.
    fn scatter(&mut self) {
        let m = self.m;
        let row = |name: &str| m.dress.iter().find(|d| d.name == name);
        let (floor_row, wall_row) = (row("scatter_floor"), row("scatter_wall"));
        if floor_row.is_none() && wall_row.is_none() {
            return;
        }
        let rooms: Vec<ScatterRoom> = self
            .info
            .rooms
            .iter()
            .map(|r| {
                let doors = r.shape.doors.iter().map(|d| (r.x + i32::from(d.x), r.y + i32::from(d.y))).collect();
                (r.node, r.rect, doors)
            })
            .collect();
        let mut n = 0;
        for (node, rect, doors) in rooms {
            let mut rng = dice(self.seed, self.zone, Step::DunDress, self.attempt, node as i32, 1);
            let mut last_hung = (i32::MIN, i32::MIN);
            for y in rect.y..rect.bottom() {
                for x in rect.x..rect.right() {
                    let t = self.k.get(x, y);
                    if !(t == m.floor || m.alt.contains(&t)) || !self.k.fits(x, y, 1, 1) {
                        continue;
                    }
                    if doors.iter().any(|&(dx, dy)| (x - dx).abs() <= 3 && (y - dy).abs() <= 3) {
                        continue;
                    }
                    let hung = self.k.get(x, y - 1) == m.wall;
                    let Some(r) = (if hung { wall_row } else { floor_row }) else { continue };
                    if !rng.chance(r.chance) {
                        continue;
                    }
                    let Some(&def) = rng.pick(r.props) else { continue };
                    let d = catalog().story.prop(def);
                    let (w, h) = (i32::from(d.w), i32::from(d.h));
                    let clear = self.k.fits(x, y, w, h)
                        && doors.iter().all(|&(dx, dy)| (x + w - 1 - dx).abs() > 3 || (y + h - 1 - dy).abs() > 3);
                    if d.solid || !clear || (hung && last_hung.1 == y && x - last_hung.0 < 3) {
                        continue;
                    }
                    let key = self.key_for(&format!("{}_scatter_{n}", m.id));
                    n += 1;
                    self.k.prop(key, def, x, y, w, h);
                    if hung {
                        last_hung = (x, y);
                    }
                }
            }
        }
    }

    /// Heat (DUNGEONS.md §2.7): a node's enemy cost is its share of the dungeon's base heat,
    /// capped by what the template says the room can hold fairly, spent on the dungeon's own
    /// bestiary with a seeded mix. At most two to a spawn socket.
    fn enemies(&mut self, r: &RoomInfo, template: &'static RoomTemplate, rng: &mut Sfc32) {
        let m = self.m;
        let n = &m.nodes[r.node];
        let spawns: Vec<Rect> = template
            .sockets
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == RoomSocketKind::Spawn)
            .map(|(i, _)| r.shape.sockets[i])
            .collect();
        let mut left = i32::from(n.heat_cost.min(template.heat_max));
        let mut load = vec![0u8; spawns.len()];
        let mut count = 0;
        while left > 0 {
            let can: Vec<(UnitDefId, u16)> =
                m.budget.enemies.iter().copied().filter(|&(_, c)| i32::from(c) <= left).collect();
            let open: Vec<usize> = (0..spawns.len()).filter(|&i| load[i] < 2).collect();
            let (Some(&(id, cost)), Some(&at)) = (rng.pick(&can), rng.pick(&open)) else { break };
            let s = spawns[at];
            // The second of a pair stands one cell along, if that cell is floor; otherwise on the same spot.
            let dx = i32::from(load[at] == 1 && self.k.fits(r.x + s.x + 1, r.y + s.y, 1, 1));
            let key = self.key_for(&format!("{}_{}_spawn_{count}", m.id, n.id));
            count += 1;
            self.k.unit(key, id, r.x + s.x + dx, r.y + s.y, Vec::new(), m.phase);
            load[at] += 1;
            left -= i32::from(cost);
        }
    }
}

/// An edge whose kinds put a gate in the corridor carries its gate row (the build sees to it).
fn gate_missing(m: &MissionDef) -> bool {
    m.edges.iter().any(|e| {
        e.gate.is_none()
            && std::iter::once(&e.kind).chain(e.also).any(|k| {
                matches!(
                    k,
                    MissionEdgeKind::Key { .. }
                        | MissionEdgeKind::Oneway { .. }
                        | MissionEdgeKind::State { .. }
                        | MissionEdgeKind::Lockin { .. }
                )
            })
    })
}

/// One candidate for `(zone, seed, attempt)`: the last attempt (`ZONE_ATTEMPTS - 1`) stamps the
/// mission's hand-placed fallback. Panics if `zone` is not a generated dungeon.
pub fn build_candidate(zone: ZoneId, seed: u32, attempt: u8) -> Built {
    let m = catalog()
        .dungeons
        .mission_of(zone)
        .unwrap_or_else(|| panic!("zone \"{}\" is not a generated dungeon", zone.name()));
    build_mission(m, seed, attempt)
}

/// [`build_candidate`] for a mission given as data (a test may hand one that asks for something
/// other than the catalog's).
pub fn build_mission(m: &'static MissionDef, seed: u32, attempt: u8) -> Built {
    let layout = if attempt >= ZONE_ATTEMPTS - 1 {
        embed_fallback(m).map_err(|e| vec![e])
    } else {
        embed(m, seed, m.zone, attempt).ok_or_else(Vec::new)
    };
    assemble(m, seed, attempt, layout, (m.cols, m.rows), false)
}

/// One room and nothing else, as a whole (small) zone: the template harness's blueprint
/// (`buildRoomAlone`). The node's holdings fill its sockets; every door opens onto a stub whose
/// end is marked ([`door_mark`]). The room stands in bay (1, 1) of a lattice one bay wider than
/// it all round, stamped as the hand-placed attempt is: exactly this shape, exactly here.
pub fn build_room_alone(m: &'static MissionDef, node: usize, template: TemplateId, shape: &'static RoomShape) -> Built {
    let layout = Layout {
        placements: vec![MissionPlacement {
            node: node as u8,
            template,
            bay: (1, 1),
            turn: shape.turn,
            mirror: shape.mirror,
        }],
        corridors: Vec::new(),
        dropped: (0..m.nodes.len()).filter(|&n| n != node).map(|n| n as u8).collect(),
        fallback: true,
    };
    assemble(m, 1, ZONE_ATTEMPTS - 1, Ok(layout), (shape.bays.0 + 2, shape.bays.1 + 2), true)
}

/// Stamp a layout (or say why there is none) onto a lattice of `cols x rows` bays.
fn assemble(
    m: &'static MissionDef,
    seed: u32,
    attempt: u8,
    layout: Result<Layout, Vec<String>>,
    (cols, rows): (u8, u8),
    stubs: bool,
) -> Built {
    let zone = m.zone;
    let (w, h) = (2 * BORDER + i32::from(cols) * BAY_W, 2 * BORDER + i32::from(rows) * BAY_H);
    let mut bp = Blueprint::new(zone, w as u32, h as u32, m.wall);
    bp.name = TextRef::Text(m.name);
    bp.indoor = m.indoor;
    bp.ambient = m.ambient;
    bp.attempts = attempt + 1;
    let mut info = BuildInfo {
        mission: m,
        layout: None,
        rooms: Vec::new(),
        locks: Vec::new(),
        lockins: Vec::new(),
        controls: Vec::new(),
        corridors: Vec::new(),
        errors: Vec::new(),
        rejected: Vec::new(),
    };
    let layout = match layout {
        Ok(l) => l,
        Err(errors) => {
            info.errors.extend(errors);
            info.errors.push(format!("dungeon \"{}\": embed: no layout found", m.id));
            return Built { blueprint: bp, info };
        }
    };
    let mut g = Gen {
        m,
        zone,
        seed,
        attempt,
        layout: &layout,
        k: Kit { bp, claimed: vec![false; (w * h) as usize], w, h },
        info,
        lanes: Lanes::new(i32::from(cols), i32::from(rows)),
        rows: Rows::new(),
        at_self: catalog().name_id("@self"),
        room_at: vec![None; m.nodes.len()],
        state_gates: Vec::new(),
        lamps: Vec::new(),
        stubs,
    };
    g.rooms();
    if g.info.errors.is_empty() {
        g.corridors();
        g.locks();
        g.lamps();
        g.fill();
        g.scatter();
        g.zone_rects();
    }
    let mut info = g.info;
    let blueprint = g.k.bp;
    info.layout = Some(layout);
    Built { blueprint, info }
}
