//! What one seat sees (ARCHITECTURE.md §11): her zone, read only. `jane-present` builds its
//! frame from this and nothing else. Landed so far: the seat, the ground and who stands on it,
//! the clock; what USE would do, her conversation, the quest log, the bench and the fire, the
//! craft row, the journal, and the light rule. The rest of §11 lands with the systems it reads.

use jane_core::blueprint::PropSpawn;
use jane_core::{Angle, Blueprint, DialogueId, ItemId, QuestId, Rect, SpellId, TextRef, Tile, Vec2, ZoneId};
use jane_data::{DialogueNode, Light};

use crate::ids::{Seat, UnitId};
use crate::input::InputFrame;
use crate::interact::{Focus, Here, focus_of, near_bench, near_rest, spawn_of};
use crate::runtime::ZoneRuntime;
use crate::sim::Sim;
use crate::state::{
    Drop, FactKey, GameState, Ground, JournalEntry, Known, PlayerState, Projectile, Prop, QuestProgress, Speaker, Unit,
    ZoneState,
};

/// A unit as drawn.
#[derive(Clone, Copy, Debug)]
pub struct UnitView<'a> {
    pub unit: &'a Unit,
    /// Where it stood before this tick's moves.
    pub prev_pos: Vec2,
    pub moved: bool,
    /// Per-instance variation (ART.md §3 `vary`), from the id: derived, never saved.
    pub variant: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    seat: Seat,
    state: &'a GameState,
    zone: &'a ZoneState,
    rt: &'a ZoneRuntime,
    bp: &'a Blueprint,
    indoor: bool,
}

/// Her conversation, or the thing she is reading.
#[derive(Clone, Copy, Debug)]
pub struct DialogueView {
    pub speaker: Speaker,
    pub tree: Option<DialogueId>,
    /// The node (`None` while reading a thing's words).
    pub node: Option<&'static DialogueNode>,
    /// The words of a thing read.
    pub read: Option<TextRef>,
    pub line: u16,
    pub awaiting_choice: bool,
}

/// A quest in the log.
#[derive(Clone, Copy, Debug)]
pub struct QuestView<'a> {
    pub quest: QuestId,
    pub ready: bool,
    state: &'a GameState,
    prog: &'a QuestProgress,
}

impl QuestView<'_> {
    /// Progress on requirement `i`, clamped to what it asks for.
    pub fn count(&self, i: usize) -> u16 {
        crate::quests::requirement_count(self.state, self.prog, i)
    }
}

impl Sim {
    /// Seat `seat`'s view; `None` for a seat that is not connected (or not yet in a live zone).
    pub fn view(&self, seat: Seat) -> Option<View<'_>> {
        let p = self.state.player(seat).filter(|p| p.connected)?;
        Some(View {
            seat,
            state: &self.state,
            zone: self.state.zone(p.zone)?,
            rt: self.runtime(p.zone)?,
            bp: self.blueprint(p.zone),
            indoor: self.blueprint(p.zone).indoor,
        })
    }
}

impl<'a> View<'a> {
    pub fn seat(&self) -> Seat {
        self.seat
    }

    pub fn me(&self) -> &'a PlayerState {
        &self.state.players[self.seat.index()]
    }

    pub fn body(&self) -> &'a Unit {
        self.zone.unit(self.me().unit).expect("a connected seat's body is in her zone")
    }

    pub fn heroine(&self) -> &'a str {
        &self.state.name
    }

    pub fn seed(&self) -> u32 {
        self.state.seed
    }

    /// `(ticks since midnight, day)`.
    pub fn clock(&self) -> (u32, u32) {
        (self.state.clock, self.state.day)
    }

    pub fn zone(&self) -> ZoneId {
        self.zone.id
    }

    /// The zone's size in cells.
    pub fn size(&self) -> (u32, u32) {
        (self.rt.grid.w(), self.rt.grid.h())
    }

    pub fn indoor(&self) -> bool {
        self.indoor
    }

    pub fn tile(&self, cx: i32, cy: i32) -> Tile {
        self.rt.grid.tile_at(cx, cy)
    }

    pub fn flags(&self, cx: i32, cy: i32) -> u8 {
        self.rt.grid.flags_at(cx, cy)
    }

    /// Awake, unhidden units standing in `r`, in id order.
    pub fn units_in(&self, r: Rect) -> impl Iterator<Item = UnitView<'a>> + 'a {
        let zone = self.zone;
        let rt = self.rt;
        rt.awake_units.iter().filter_map(move |&id| {
            let u = zone.unit(id)?;
            let (cx, cy) = u.pos.cell();
            if u.hidden || !r.contains(cx, cy) {
                return None;
            }
            let prev_pos = prev_pos(rt, id).unwrap_or(u.pos);
            Some(UnitView { unit: u, prev_pos, moved: prev_pos != u.pos, variant: (id.get() % 4) as u8 })
        })
    }

    /// Props whose footprint may touch `r` (a superset, by block), in id order, hidden ones left out.
    pub fn props_in(&self, r: Rect) -> impl Iterator<Item = &'a Prop> + 'a {
        let cat = jane_data::catalog();
        let zone = self.zone;
        let mut ixs = Vec::new();
        self.rt.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, &mut ixs);
        ixs.into_iter().map(move |i| &zone.props[i as usize]).filter(move |p| {
            let d = cat.story.prop(p.def);
            !p.hidden && Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h)).overlaps(r)
        })
    }
}

impl<'a> View<'a> {
    fn here(&self) -> Here<'a> {
        Here { world: self.state, zone: self.zone, rt: self.rt, bp: self.bp }
    }

    /// What USE would act on now, and the word for it.
    pub fn focus(&self) -> Option<Focus> {
        focus_of(&self.here(), self.body(), &mut Vec::new(), &mut Vec::new())
    }

    pub fn dialogue(&self) -> Option<DialogueView> {
        let me = self.me();
        let d = me.dialogue?;
        let node = crate::dialogue::current(&d).and_then(|c| match c {
            crate::dialogue::Current::Node(n) => Some(n),
            crate::dialogue::Current::Read(_) => None,
        });
        Some(DialogueView {
            speaker: d.speaker,
            tree: d.tree,
            node,
            read: d.read,
            line: d.line,
            awaiting_choice: crate::dialogue::awaiting_choice(me),
        })
    }

    /// The quest log: active quests in the order they were given.
    pub fn quests(&self) -> impl Iterator<Item = QuestView<'a>> + 'a {
        let state = self.state;
        state.quests.active.iter().map(move |prog| QuestView {
            quest: prog.quest,
            ready: crate::quests::ready(state, prog.quest),
            state,
            prog,
        })
    }

    pub fn quests_done(&self) -> &'a [QuestId] {
        &self.state.quests.done
    }

    /// A bench within reach: the bag window shows the craft row.
    pub fn near_bench(&self) -> bool {
        near_bench(&self.here(), self.body().pos, &mut Vec::new())
    }

    /// A bed or a fire within reach: the game can be saved here.
    pub fn near_rest(&self) -> bool {
        near_rest(&self.here(), self.body().pos, &mut Vec::new())
    }

    /// What her craft row makes.
    pub fn craft_output(&self) -> Option<(ItemId, u16)> {
        crate::inventory::craft_output(&self.me().craft)
    }

    /// Every change to what is known, oldest first (§3.7).
    pub fn journal(&self) -> impl Iterator<Item = &'a JournalEntry> + 'a {
        self.state.journal.entries.iter()
    }

    pub fn known(&self, fact: FactKey) -> Option<Known> {
        crate::journal::known(self.state, fact)
    }

    /// THE light rule, shared with the sim (`light.rs`).
    pub fn light_showing(&self, p: &Prop) -> Option<&'static Light> {
        crate::light::light_showing(jane_data::catalog().story.prop(p.def), p, self.lamps_lit(), self.zone.wetness)
    }

    pub fn lamps_lit(&self) -> bool {
        crate::light::lamps_lit(self.state.clock)
    }

    /// Bolts in flight here, in the order they were cast.
    pub fn projectiles(&self) -> &'a [Projectile] {
        &self.zone.projectiles
    }

    /// Pools on the ground here.
    pub fn grounds(&self) -> &'a [Ground] {
        &self.zone.grounds
    }

    /// Stacks on the ground here.
    pub fn drops(&self) -> &'a [Drop] {
        &self.zone.drops
    }

    /// Where a cast of `spell` along `frame`'s aim would go, assist resolved as the sim will
    /// resolve it (ARCHITECTURE.md §5.4): the reticle draws this. `None` when the frame has no
    /// aim (she casts along her facing). Derived and never stored: the sticky unit is read, not
    /// set. The presentation passes the frame it is about to send and the spell on the bar.
    pub fn assisted_aim(&self, frame: &InputFrame, spell: SpellId) -> Option<Angle> {
        let raw = frame.aim?;
        let me = self.me();
        let Some(body) = self.zone.unit(me.unit) else { return Some(raw) };
        let def = jane_data::catalog().combat.spell(spell);
        let mut near = Vec::new();
        let (a, _) = crate::assist::pick(
            self.zone,
            self.rt,
            self.state.tick,
            body,
            me.assist,
            def,
            raw,
            frame.assist,
            &mut near,
        );
        Some(a)
    }

    /// A placed prop's row: where it leads, what it holds, its label.
    pub fn prop_spawn(&self, p: &Prop) -> Option<&'a PropSpawn> {
        spawn_of(self.bp, p)
    }

    /// A prop by id in her zone.
    pub fn prop(&self, id: crate::ids::PropId) -> Option<&'a Prop> {
        self.zone.prop_ix(id).map(|i| &self.zone.props[i as usize])
    }
}

fn prev_pos(rt: &ZoneRuntime, id: UnitId) -> Option<Vec2> {
    rt.prev_pos.binary_search_by_key(&id, |&(u, _)| u).ok().map(|i| rt.prev_pos[i].1)
}

/// The living world as presentation reads it (ARCHITECTURE.md §4.6, §11): the sky, the puddles,
/// where a scheduled person is. None of it changes state.
impl<'a> View<'a> {
    /// The sky over her zone's region (`living.rs`): presentation's mist, rain and storm.
    pub fn weather(&self) -> &'a crate::state::WeatherState {
        crate::living::weather_in(self.state, self.zone.id)
    }

    /// The sky over any region (the map's weather column, a far view).
    pub fn weather_of(&self, region: jane_data::Region) -> &'a crate::state::WeatherState {
        &self.state.weather[crate::living::region_ix(region)]
    }

    /// Her zone's rain ramp, 0..=255: puddles, the sound of it. The sim reads it for the douse rule.
    pub fn wetness(&self) -> u8 {
        self.zone.wetness
    }

    /// Where a scheduled unit of her zone is and why (a door can say who is behind it); `None`
    /// for a unit with no hours.
    pub fn schedule_state(&self, unit: UnitId) -> Option<crate::living::ScheduleState> {
        crate::living::schedule_state(self.state, self.zone, self.rt, unit)
    }
}
