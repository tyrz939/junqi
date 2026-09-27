//! Traces (VERIFICATION.md §3.1): a played session as it happened, for the experience metrics
//! (L4), the dossier (L7) and the truth audits over play (L5).
//!
//! **An observer, never a writer.** [`Observer::observe`] reads the sim after each step (its
//! state, its views, the step's events) and appends [`Record`]s; it holds `&Sim` and nothing in
//! the state reads it back, so a traced run steps and hashes exactly as an untraced one. What a
//! bot adds (its decisions, a quest target first in view) it appends itself with [`Trace::push`].
//!
//! `.jtr` is a save's shape: `JTRC`, the header's length (u32 le), the header in postcard, then
//! the records and the footer in postcard in an lz4 block, size prepended. A trace is a pure
//! function of `(content_hash, seed, model, seats, minutes)`: no wall time, no addresses.
//!
//! What is recorded, and how often:
//!
//! | Kind | When |
//! | --- | --- |
//! | [`Kind::Pos`] | on every zone change, per seat |
//! | [`Kind::Sample`] | every [`SAMPLE_EVERY`] ticks per seat: where, health, kit, what is in view, how much of it is new |
//! | [`Kind::Event`] | the step's events that matter to a metric ([`Ev`]), as routed to the seat |
//! | [`Kind::Journal`] | every journal write, with its words resolved (the journal is the party's) |
//! | [`Kind::Hash`] | every [`HASH_EVERY`] ticks |
//! | [`Kind::Decision`], [`Kind::Sight`], [`Kind::Note`] | written by the player model's harness |
//!
//! "In view" is the camera's rect about her (`QUEST-TREE.md` §1: 48 × 27 cells): the same rect
//! for every model, whatever the renderer's window.

use std::collections::BTreeSet;

use jane_core::{Rect, ZoneId};
use serde::{Deserialize, Serialize};

use crate::event::{Event, EventKind, QuestChange};
use crate::ids::{Seat, UnitId};
use crate::sim::Sim;
use crate::state::{FactKey, JournalKind, Source};

pub const MAGIC: [u8; 4] = *b"JTRC";
pub const TRACE_VERSION: u16 = 1;
/// The camera, cells (QUEST-TREE.md §1).
pub const VIEW_W: i32 = jane_core::view::VIEW_W_CELLS as i32;
pub const VIEW_H: i32 = jane_core::view::VIEW_H_CELLS as i32;
/// Ticks between samples.
pub const SAMPLE_EVERY: u32 = 60;
/// Ticks between hashes.
pub const HASH_EVERY: u32 = 600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub version: u16,
    pub content_hash: u64,
    pub build: String,
    pub seed: u32,
    /// The player model (`reader`, `pair:split`, ...).
    pub model: String,
    pub seats: u8,
    /// The frames asked for, in real minutes.
    pub minutes: u32,
    /// `(ticks since midnight, day)` at the first frame.
    pub started_clock: (u32, u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub tick: u32,
    /// Frames stepped since the trace began: real time played (a night slept moves the tick, not
    /// the frame).
    pub frame: u32,
    pub seat: Option<u8>,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Pos {
        zone: u8,
        cell: (i32, i32),
    },
    Sample(Sample),
    Event(Ev),
    /// A journal write: its kind and source (`state::JournalKind`, `state::Source`, by index),
    /// what it is about in words, where she stood.
    Journal {
        kind: u8,
        how: u8,
        what: String,
        zone: u8,
        at: (u16, u16),
    },
    /// The model changed objective: what it is now, the quest step it serves, and the words it
    /// acts on (the step's text, a sign, a line).
    Decision {
        objective: String,
        quest: Option<u16>,
        step: Option<u8>,
        because: String,
    },
    /// A quest step's target first came into view (the harness knows where it is; the model
    /// does not), and how far from her it stood.
    Sight {
        quest: u16,
        step: u8,
        zone: u8,
        cell: (i32, i32),
        dist: u32,
    },
    /// A line of the model's own log (a milestone, a thing given up on).
    Note(String),
    Hash(u64),
}

/// Every [`SAMPLE_EVERY`] ticks, per seat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub zone: u8,
    pub cell: (i32, i32),
    /// She moved a cell or more since the last sample.
    pub moving: bool,
    pub talking: bool,
    /// Hurt, or hurting something, in the last three seconds.
    pub fighting: bool,
    pub alive: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub energy: i32,
    /// Stacks of food, potions and keys in her bag.
    pub food: u16,
    pub potions: u16,
    pub keys: u16,
    pub hour: u8,
    pub night: bool,
    /// Standing in a showing light (a lamp, a fire), or indoors.
    pub lit: bool,
    /// The region under her (0 Lowfields, 1 Waters, 2 Works).
    pub region: u8,
    pub in_view: InView,
    /// Things in view for the first time this session (props with a verb, units, places).
    pub new_things: u16,
    /// Fog blocks seen for the first time since the last sample (the map growing).
    pub new_ground: u16,
}

/// What stands in the camera's rect about her.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InView {
    /// Units other than her party: creatures and people.
    pub units: u16,
    pub hostiles: u16,
    /// Things with words: a sign, a plate, a note, a book.
    pub readables: u16,
    /// Things to take: a chest not yet opened, a thing on the ground, a drop.
    pub pickups: u16,
    /// Doors, fires, beds and benches.
    pub places: u16,
    /// The edge of a named patch of the county (a site, a farm, a wood).
    pub sites: u16,
}

impl InView {
    /// Nothing at all (VERIFICATION.md L4 "nothing-to-see": no unit, no readable, no pickup and
    /// no site edge).
    pub fn empty(&self) -> bool {
        self.units == 0 && self.readables == 0 && self.pickups == 0 && self.places == 0 && self.sites == 0
    }
}

/// The events a metric reads, as the seat was told them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ev {
    Zone {
        zone: u8,
        first: bool,
    },
    /// 0 given, 1 progress, 2 ready, 3 done.
    Quest {
        quest: u16,
        change: u8,
    },
    /// Hit points she lost, and the row of what hit her.
    Hurt {
        amount: i32,
        by: Option<u16>,
    },
    /// A unit she had hurt died.
    Kill {
        def: u16,
    },
    /// She fell: to what last hurt her, in which region.
    Died {
        by: Option<u16>,
        region: u8,
    },
    Rest,
    Learn {
        spell: u16,
    },
    Loot {
        item: u16,
        qty: u16,
    },
    Consequence {
        id: u16,
    },
    Bell {
        strikes: u8,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Footer {
    pub ticks: u32,
    pub frames: u32,
    pub records: u32,
    /// Filled by whoever computes them (`jane-bot`'s experience metrics), by name.
    pub metrics: Vec<(String, i64)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    pub header: Header,
    pub records: Vec<Record>,
    pub footer: Footer,
}

#[derive(Debug)]
pub enum TraceError {
    NotATrace,
    Truncated,
    Version(u16),
    Decode(postcard::Error),
    Decompress(String),
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraceError::NotATrace => write!(f, "not a trace (no JTRC)"),
            TraceError::Truncated => write!(f, "truncated"),
            TraceError::Version(v) => write!(f, "trace version {v}, this build reads {TRACE_VERSION}"),
            TraceError::Decode(e) => write!(f, "decode: {e}"),
            TraceError::Decompress(e) => write!(f, "decompress: {e}"),
        }
    }
}

impl std::error::Error for TraceError {}

impl Trace {
    pub fn new(header: Header) -> Trace {
        Trace { header, records: Vec::new(), footer: Footer::default() }
    }

    pub fn push(&mut self, tick: u32, frame: u32, seat: Option<Seat>, kind: Kind) {
        self.records.push(Record { tick, frame, seat: seat.map(|s| s.0), kind });
    }

    /// Close it: the footer's counts from the records.
    pub fn finish(&mut self, ticks: u32, frames: u32) {
        self.footer.ticks = ticks;
        self.footer.frames = frames;
        self.footer.records = self.records.len() as u32;
    }

    pub fn encode(&self) -> Vec<u8> {
        let head = postcard::to_allocvec(&self.header).expect("a header encodes");
        let body = postcard::to_allocvec(&(&self.records, &self.footer)).expect("records encode");
        let packed = lz4_flex::block::compress_prepend_size(&body);
        let mut out = Vec::with_capacity(8 + head.len() + packed.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(head.len() as u32).to_le_bytes());
        out.extend_from_slice(&head);
        out.extend_from_slice(&packed);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Trace, TraceError> {
        if bytes.len() < 8 || bytes[..4] != MAGIC {
            return Err(TraceError::NotATrace);
        }
        let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let rest = &bytes[8..];
        if rest.len() < n {
            return Err(TraceError::Truncated);
        }
        let header: Header = postcard::from_bytes(&rest[..n]).map_err(TraceError::Decode)?;
        if header.version != TRACE_VERSION {
            return Err(TraceError::Version(header.version));
        }
        let raw = lz4_flex::block::decompress_size_prepended(&rest[n..])
            .map_err(|e| TraceError::Decompress(e.to_string()))?;
        let (records, footer): (Vec<Record>, Footer) = postcard::from_bytes(&raw).map_err(TraceError::Decode)?;
        Ok(Trace { header, records, footer })
    }
}

/// The camera's rect about a point.
pub fn camera(cell: (i32, i32)) -> Rect {
    Rect::new(cell.0 - VIEW_W / 2, cell.1 - VIEW_H / 2, VIEW_W, VIEW_H)
}

/// What one seat has been shown, for "new".
#[derive(Debug, Default)]
struct SeatSeen {
    zone: Option<ZoneId>,
    last_cell: Option<(i32, i32)>,
    /// (zone, prop id) of props with a verb seen.
    props: BTreeSet<(u8, u32)>,
    /// (zone, unit id) seen.
    units: BTreeSet<(u8, u32)>,
    /// (zone, area index) whose edge was in view.
    sites: BTreeSet<(u8, u16)>,
    /// Units she hurt lately (a death of one is her kill).
    hurt: BTreeSet<UnitId>,
    /// The row of what last hurt her.
    last_hurt: Option<u16>,
    /// The tick of the last blow either way.
    fought_at: u32,
    /// Fog blocks seen, per zone, at the last sample.
    fog_seen: [u32; jane_core::ids::ZONE_COUNT],
}

/// Reads a sim after each step and writes what it saw into a [`Trace`].
#[derive(Debug, Default)]
pub struct Observer {
    seen: [SeatSeen; 4],
    frames: u32,
}

impl Observer {
    pub fn new() -> Observer {
        Observer::default()
    }

    /// Frames observed so far.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// After a step: `events` are what the step emitted (all of them; each seat's are picked out
    /// as `events_for` would).
    pub fn observe(&mut self, sim: &Sim, events: &[Event], out: &mut Trace) {
        self.frames += 1;
        let st = sim.state();
        let tick = st.tick.0;
        let frame = self.frames;
        // The journal is the party's: its new entries once, with the seat none. Each write pushed
        // one entry and said one `Journal` event; the ring may have let an older one go.
        let writes = events.iter().filter(|e| matches!(e.kind, EventKind::Journal(_))).count();
        let entries = &st.journal.entries;
        if writes > 0 {
            for e in &entries[entries.len().saturating_sub(writes)..] {
                out.push(
                    tick,
                    frame,
                    None,
                    Kind::Journal {
                        kind: journal_kind_ix(e.fact.kind()),
                        how: source_ix(e.how),
                        what: fact_words(st, e.fact),
                        zone: e.zone.index() as u8,
                        at: (e.at.x, e.at.y),
                    },
                );
            }
        }
        for p in st.players.iter().filter(|p| p.connected) {
            let s = p.seat;
            let me = p.unit;
            let seen = &mut self.seen[s.index()];
            for e in crate::event::events_for(events, p) {
                let ev = match e.kind {
                    EventKind::Zone { zone, first } => Some(Ev::Zone { zone: zone.index() as u8, first }),
                    EventKind::Quest { quest, change } => Some(Ev::Quest {
                        quest: quest.0,
                        change: match change {
                            QuestChange::Given => 0,
                            QuestChange::Progress => 1,
                            QuestChange::Ready => 2,
                            QuestChange::Done => 3,
                        },
                    }),
                    EventKind::Damage { unit, from, amount, .. } if unit == me => {
                        seen.fought_at = tick;
                        let by = from.and_then(|f| st.zone(p.zone).and_then(|z| z.unit(f))).map(|u| u.def.0);
                        if by.is_some() {
                            seen.last_hurt = by;
                        }
                        Some(Ev::Hurt { amount: amount.0 / 1000, by })
                    }
                    EventKind::Damage { unit, from: Some(f), .. } if f == me => {
                        seen.fought_at = tick;
                        if seen.hurt.len() > 64 {
                            seen.hurt.pop_first();
                        }
                        seen.hurt.insert(unit);
                        None
                    }
                    EventKind::Death { unit, def, .. } if unit != me && seen.hurt.remove(&unit) => {
                        Some(Ev::Kill { def: def.0 })
                    }
                    EventKind::PlayerDied => {
                        let region = st
                            .zone(p.zone)
                            .and_then(|z| z.unit(me))
                            .and_then(|u| sim.runtime(p.zone).map(|rt| rt.region_at(u.pos.cell().0, u.pos.cell().1)))
                            .map_or(0, region_ix);
                        Some(Ev::Died { by: seen.last_hurt.take(), region })
                    }
                    EventKind::Rest => Some(Ev::Rest),
                    EventKind::Learn(sp) => Some(Ev::Learn { spell: sp.0 }),
                    EventKind::Loot { item, qty } => Some(Ev::Loot { item: item.0, qty }),
                    EventKind::Consequence(c) => Some(Ev::Consequence { id: c.0 }),
                    EventKind::Bell { strikes, church: false, .. } => Some(Ev::Bell { strikes }),
                    _ => None,
                };
                if let Some(ev) = ev {
                    out.push(tick, frame, Some(s), Kind::Event(ev));
                }
            }
            let Some(v) = sim.view(s) else { continue };
            let cell = v.body().pos.cell();
            if seen.zone != Some(v.zone()) {
                seen.zone = Some(v.zone());
                seen.last_cell = Some(cell);
                out.push(tick, frame, Some(s), Kind::Pos { zone: v.zone().index() as u8, cell });
            }
            if tick % SAMPLE_EVERY == 0 {
                let sample = sample(sim, &v, seen, tick);
                out.push(tick, frame, Some(s), Kind::Sample(sample));
            }
        }
        if tick % HASH_EVERY == 0 {
            out.push(tick, frame, None, Kind::Hash(sim.hash()));
        }
    }
}

fn sample(sim: &Sim, v: &crate::View<'_>, seen: &mut SeatSeen, tick: u32) -> Sample {
    let cat = jane_data::catalog();
    let st = sim.state();
    let me = v.body();
    let cell = me.pos.cell();
    let zi = v.zone().index() as u8;
    let moving = seen.last_cell.is_some_and(|c| c != cell);
    seen.last_cell = Some(cell);
    let cam = camera(cell);
    let mut iv = InView::default();
    let mut new_things = 0u16;
    for u in v.units_in(cam) {
        let u = u.unit;
        if u.id == me.id || v.seat_of(u.id).is_some() || !u.alive {
            continue;
        }
        iv.units += 1;
        if u.faction != jane_data::Faction::Friendly {
            iv.hostiles += 1;
        }
        if seen.units.insert((zi, u.id.get())) {
            new_things += 1;
        }
    }
    for p in v.props_in(cam) {
        let def = cat.story.prop(p.def);
        let Some(s) = v.prop_spawn(p) else { continue };
        let mut thing = false;
        if s.talk.is_some() {
            iv.readables += 1;
            thing = true;
        }
        if !p.used && !s.loot.is_empty() {
            iv.pickups += 1;
            thing = true;
        }
        if s.to.is_some() || def.rest || def.bench {
            iv.places += 1;
            thing = true;
        }
        thing |= s.use_list.is_some() || def.push || def.carry;
        if thing && seen.props.insert((zi, p.id.get())) {
            new_things += 1;
        }
    }
    iv.pickups += v
        .drops()
        .iter()
        .filter(|d| {
            let (x, y) = d.pos.cell();
            cam.contains(x, y)
        })
        .count() as u16;
    for (i, (_, r)) in v.areas().enumerate() {
        if r.overlaps(cam) && !(r.contains(cam.x, cam.y) && r.contains(cam.right() - 1, cam.bottom() - 1)) {
            iv.sites += 1;
            if seen.sites.insert((zi, i as u16)) {
                new_things += 1;
            }
        }
    }
    // The map growing: fog blocks seen in this zone since the last sample.
    let mut fog = 0u32;
    if let Some(zs) = st.zone(v.zone()) {
        fog = zs.fog.iter().map(|w| w.count_ones()).sum();
    }
    let was = std::mem::replace(&mut seen.fog_seen[v.zone().index()], fog);
    let new_ground = fog.saturating_sub(was).min(u32::from(u16::MAX)) as u16;
    let (mut food, mut potions, mut keys) = (0u16, 0u16, 0u16);
    for s in v.me().bag.iter().flatten() {
        let d = cat.combat.item(s.item);
        if d.id == "apple" || d.id.ends_with("_food") || d.id == "bread" || d.id == "grapes" {
            food += s.qty;
        } else if d.id.starts_with("potion_") {
            potions += s.qty;
        } else if d.opens.is_some() {
            keys += s.qty;
        }
    }
    let lit = v.indoor()
        || st
            .zone(v.zone())
            .zip(sim.runtime(v.zone()))
            .is_some_and(|(z, rt)| crate::light::lit_at(z, rt, st.clock, me.pos, true));
    Sample {
        zone: zi,
        cell,
        moving,
        talking: v.dialogue().is_some(),
        fighting: seen.fought_at + 180 >= tick && seen.fought_at > 0,
        alive: me.alive,
        hp: me.hp.points(),
        max_hp: crate::units::max_hp(me).points(),
        mp: me.mp.points(),
        energy: me.energy.points(),
        food,
        potions,
        keys,
        hour: v.hour(),
        night: v.is_night(),
        lit,
        region: region_ix(v.region()),
        in_view: iv,
        new_things,
        new_ground,
    }
}

fn region_ix(r: jane_data::Region) -> u8 {
    crate::living::region_ix(r) as u8
}

pub fn journal_kind_ix(k: JournalKind) -> u8 {
    match k {
        JournalKind::Place => 0,
        JournalKind::Person => 1,
        JournalKind::Thing => 2,
        JournalKind::Claim => 3,
        JournalKind::Route => 4,
        JournalKind::Danger => 5,
        JournalKind::Rumour => 6,
        JournalKind::Consequence => 7,
    }
}

/// The journal kinds' names, by [`journal_kind_ix`].
pub const JOURNAL_KINDS: [&str; 8] = ["place", "person", "thing", "claim", "route", "danger", "rumour", "consequence"];

pub fn source_ix(s: Source) -> u8 {
    match s {
        Source::Seen => 0,
        Source::Visited => 1,
        Source::Named => 2,
        Source::Met => 3,
        Source::Talked => 4,
        Source::Dead => 5,
        Source::Held => 6,
        Source::Read => 7,
        Source::Told => 8,
        Source::Confirmed => 9,
        Source::Contradicted => 10,
        Source::Walked => 11,
        Source::AttackedIn => 12,
        Source::Fled => 13,
        Source::Heard => 14,
    }
}

/// The sources' names, by [`source_ix`].
pub const SOURCES: [&str; 15] = [
    "seen",
    "visited",
    "named",
    "met",
    "talked",
    "dead",
    "held",
    "read",
    "told",
    "confirmed",
    "contradicted",
    "walked",
    "attacked in",
    "fled",
    "heard",
];

/// A fact in words: a place's or person's name, a thing's, a claim's text.
fn fact_words(st: &crate::GameState, f: FactKey) -> String {
    let cat = jane_data::catalog();
    match f {
        FactKey::Place(s) | FactKey::Person(s) | FactKey::Danger(s) => st.syms.name(s).to_owned(),
        FactKey::Thing(t) => match t {
            jane_core::action::Thing::Item(i) => cat.combat.item(i).id.to_owned(),
            other @ jane_core::action::Thing::Prop(_) => format!("{other:?}"),
        },
        FactKey::Claim(t) => cat.text(t).to_owned(),
        FactKey::Route(a, b) => format!("{} to {}", st.syms.name(a), st.syms.name(b)),
        FactKey::Rumour(r) => format!("rumour {}", r.0),
        FactKey::Consequence(c) => cat.living.consequences.get(usize::from(c.0)).map_or("?", |r| r.id).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trace_reads_back_as_it_was_written() {
        let mut t = Trace::new(Header {
            version: TRACE_VERSION,
            content_hash: 7,
            build: "test".into(),
            seed: 3,
            model: "reader".into(),
            seats: 1,
            minutes: 2,
            started_clock: (17 * 3600, 0),
        });
        t.push(0, 1, Some(Seat(0)), Kind::Pos { zone: 0, cell: (4, 5) });
        t.push(60, 60, Some(Seat(0)), Kind::Sample(Sample { hp: 10, ..Sample::default() }));
        t.push(61, 61, None, Kind::Event(Ev::Died { by: Some(3), region: 1 }));
        t.push(62, 62, None, Kind::Note("done".into()));
        t.finish(62, 62);
        let bytes = t.encode();
        assert_eq!(Trace::decode(&bytes).expect("decodes"), t);
        assert!(matches!(Trace::decode(b"JANE0000"), Err(TraceError::NotATrace)));
    }
}
