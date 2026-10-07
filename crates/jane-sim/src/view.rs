//! What one seat sees (ARCHITECTURE.md §11): her zone, read only. `jane-present` builds its
//! frame from this and nothing else. Landed so far: the seat, the ground and who stands on it,
//! the clock; what USE would do, her conversation, the quest log, the bench and the fire, the
//! craft row, the journal, and the light rule. The rest of §11 lands with the systems it reads.
//!
//! What the headless player reads besides (`jane-bot`, ARCHITECTURE.md §8: a model is a policy
//! over View): the tick and `frozen`, the zone's marks and rects by name, the rows of its trigger
//! table, action lists of either source, a unit by id, the spells learned, and names. All
//! derived and read-only.

use jane_core::action::Action;
use jane_core::blueprint::{Mark, PropSpawn};
use jane_core::{
    Angle, Blueprint, Cell, DialogueId, ItemId, ListRef, QuestId, Rect, SpellId, Sym, TextRef, Tick, Tile, Vec2, ZoneId,
};
use jane_data::{DialogueNode, Light};

use crate::ids::{PropIx, Seat, UnitId};
use crate::input::InputFrame;
use crate::interact::{Focus, Here, focus_of, near_bench, near_rest, spawn_of};
use crate::runtime::{ZoneRuntime, ZoneTrigger};
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
    bps: &'a crate::blueprints::Blueprints,
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
            bps: self.blueprints(),
            indoor: self.blueprint(p.zone).indoor,
        })
    }
}

impl<'a> View<'a> {
    pub fn seat(&self) -> Seat {
        self.seat
    }

    /// Every zone's blueprint (the county's roads for a step's way, `route`).
    pub fn blueprints(&self) -> &'a crate::blueprints::Blueprints {
        self.bps
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

    /// The zone's own light, permille: what an interior is lit by (1000 outdoors).
    pub fn ambient(&self) -> jane_core::Permille {
        self.bp.ambient
    }

    pub fn tile(&self, cx: i32, cy: i32) -> Tile {
        self.rt.grid.tile_at(cx, cy)
    }

    /// The zone's render-only paint over its tiles, in paint order (a roof's slate, a wood's
    /// pines; PORT.md §6.i). The presentation reads it; nothing in the sim does.
    pub fn paint(&self) -> &'a [(Rect, jane_core::Material)] {
        &self.bp.paint
    }

    pub fn flags(&self, cx: i32, cy: i32) -> u8 {
        self.rt.grid.flags_at(cx, cy)
    }

    /// The version of this zone's [`flags`](Self::flags): the same number, the same flags
    /// (`ZoneGrid::generation`). For a reader that derives something from every cell's flags and
    /// would know whether it still holds; never state.
    pub fn flags_generation(&self) -> u64 {
        self.rt.grid.generation()
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

    /// [`props_in`](Self::props_in) with the caller's scratch for the block query, so a caller
    /// that asks every tick (presentation) allocates nothing once `scratch` is warm.
    pub fn for_props_in(&self, r: Rect, scratch: &mut Vec<PropIx>, mut f: impl FnMut(&'a Prop)) {
        let cat = jane_data::catalog();
        self.rt.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, scratch);
        for &i in scratch.iter() {
            let p = &self.zone.props[i as usize];
            let d = cat.story.prop(p.def);
            if !p.hidden
                && Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h)).overlaps(r)
            {
                f(p);
            }
        }
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

    /// Cupboard `prop` as she sees it from where she stands: what it holds, `None` unless it is a
    /// cupboard of her zone within her reach (`store.rs`). The window shows it beside her bag and
    /// closes when this goes `None`. What is in it is the party's: every seat reads the same.
    pub fn store(
        &self,
        prop: crate::ids::PropId,
    ) -> Option<&'a [Option<jane_core::Stack>; crate::tuning::STORE_SLOTS]> {
        let p = self.prop(prop)?;
        let def = jane_data::catalog().story.prop(p.def);
        let near = crate::interact::prop_distance_sq(def, p, self.body().pos)
            <= i64::from(crate::store::STORE_REACH_FX).pow(2);
        (def.store && near && self.body().alive).then(|| crate::store::slots_of(self.state, self.zone.id, prop))
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
        let wet = crate::light::prop_wetness(self.zone, self.rt, p);
        crate::light::light_showing(jane_data::catalog().story.prop(p.def), p, self.state.clock, wet)
    }

    /// The county's night by its lamps, 18:30 to 06:30; each lamp keeps it a few minutes early
    /// or late (`light::lamp_lit`, which [`Self::light_showing`] asks).
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
        let (mut near, mut props) = (Vec::new(), Vec::new());
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
            &mut props,
        );
        Some(a)
    }

    // --- her side of a fight (PLAY-PLAN §2.1; `target.rs`, `cast.rs`, `walk.rs`) -----------------

    /// Her fight: her target as the sim last validated it, the cast building, her swings, her
    /// click-walk.
    pub fn fight(&self) -> &'a crate::state::Fight {
        &self.me().fight
    }

    /// Her target as the sim last validated it (the ring is drawn under this).
    pub fn target(&self) -> Option<crate::input::TargetRef> {
        self.me().fight.target
    }

    /// May she hold `t` as her target now? The client drops one the sim would not keep.
    pub fn target_valid(&self, t: crate::input::TargetRef) -> bool {
        self.zone.unit(self.me().unit).is_some_and(|b| crate::target::valid(self.zone, b, t))
    }

    /// Is `t` a foe of hers (a red ring), as opposed to a prop (gold) or a friend?
    pub fn target_hostile(&self, t: crate::input::TargetRef) -> bool {
        self.zone.unit(self.me().unit).is_some_and(|b| crate::target::hostile(self.zone, b, t))
    }

    /// Where a target is: a unit's feet, a prop's middle.
    pub fn target_pos(&self, t: crate::input::TargetRef) -> Option<Vec2> {
        crate::target::pos_of(self.zone, t)
    }

    /// Foes Tab cycles through, facing `dir` (her aim, else her facing): nearest first.
    pub fn tab_order(&self, dir: Angle) -> Vec<UnitId> {
        let mut out = Vec::new();
        if let Some(b) = self.zone.unit(self.me().unit) {
            crate::target::foes_in_front(self.zone, self.rt, b, dir, &mut out);
        }
        out
    }

    /// Tab (`back`: LB, Shift-Tab) from her current target.
    pub fn tab_next(&self, dir: Angle, current: Option<crate::input::TargetRef>, back: bool) -> Option<UnitId> {
        let order = self.tab_order(dir);
        if back { crate::target::tab_prev(&order, current) } else { crate::target::tab_next(&order, current) }
    }

    /// The cast a seat's body is building here, if `unit` is one: what, since when, until when.
    /// Everyone's cast bar and hand glow are drawn from this.
    pub fn casting(&self, unit: UnitId) -> Option<crate::state::PendingCast> {
        self.state.players.iter().find(|p| p.unit == unit && p.zone == self.zone.id && p.connected)?.fight.cast
    }

    /// A placed prop's row: where it leads, what it holds, its label.
    pub fn prop_spawn(&self, p: &Prop) -> Option<&'a PropSpawn> {
        spawn_of(self.bp, p)
    }

    /// What a prop still holds for the taking (a chest, a lost thing on the ground); empty once
    /// it is emptied or used. A `shows_loot` row is drawn as the first of it.
    pub fn prop_loot(&self, p: &'a Prop) -> &'a [jane_core::Stack] {
        if p.used { &[] } else { crate::interact::loot_of(self.bp, p) }
    }

    /// A string a row or this zone's generator wrote: the words of a thing read, a label, a
    /// toast (`TextRef::Local` is her zone's blueprint's). Empty for a local text that is not
    /// there.
    pub fn text(&self, r: TextRef) -> &'a str {
        match r {
            TextRef::Text(t) => jane_data::catalog().text(t),
            TextRef::Local(_) => self.bp.text(r).unwrap_or(""),
        }
    }

    /// A prop by id in her zone.
    pub fn prop(&self, id: crate::ids::PropId) -> Option<&'a Prop> {
        self.zone.prop_ix(id).map(|i| &self.zone.props[i as usize])
    }
}

/// What the headless player reads (see the module doc).
impl<'a> View<'a> {
    /// Ticks advanced since New Game.
    pub fn tick(&self) -> Tick {
        self.state.tick
    }

    /// Step calls since New Game (the replay and wire clock).
    pub fn frame(&self) -> u32 {
        self.state.frame
    }

    /// Alone and talking: the world holds still (`Sim::frozen`).
    pub fn frozen(&self) -> bool {
        let mut n = 0;
        let mut talking = false;
        for p in self.state.connected() {
            n += 1;
            talking = p.dialogue.is_some();
        }
        n == 1 && talking
    }

    /// Whether the party has seen cell `(cx, cy)` of this zone (the fog's seen-bits, `fog.rs`):
    /// what the map charts and the debug view shades.
    pub fn seen(&self, cx: i32, cy: i32) -> bool {
        let g = self.rt.fog;
        let n = g.cells as i32;
        crate::fog::fog_seen(&self.zone.fog, g, cx.div_euclid(n), cy.div_euclid(n))
    }

    /// Cells on a side of one fog block here (2 indoors, 8 out).
    pub fn fog_block(&self) -> u32 {
        self.rt.fog.cells
    }

    /// Seats sitting down now: the party penalty's head count (the HUD shows it).
    pub fn party(&self) -> u8 {
        self.state.connected().count() as u8
    }

    /// 21:00 to 06:00.
    pub fn is_night(&self) -> bool {
        self.state.is_night()
    }

    /// Made fires, a fire's rest over time and unbanked finds are the rule (`fire.rs`).
    pub fn fires_made(&self) -> bool {
        self.state.fires_made
    }

    /// What the party found since its last rest, and who found it (`fire.rs`): a jar lying where
    /// its finder fell is drawn as a glint.
    pub fn unbanked(&self) -> &'a [crate::state::Unbanked] {
        &self.state.growth.unbanked
    }

    /// The story's close (STORY.md §10): 0 while it is open; after an ending's last page, which
    /// one (1 the shield held, 2 the Ball back in the hill, 3 the Sunday train), from the world's
    /// flag `the_end`. The presentation closes on it: the last page stays up, then the title.
    pub fn the_end(&self) -> u8 {
        let Some(s) = self.sym("the_end") else { return 0 };
        let v = self.state.flags.get(&crate::state::FlagKey::Named(s)).copied().unwrap_or(0);
        u8::try_from(v).unwrap_or(0)
    }

    /// A world flag by name, 0 when it was never set (or the name never interned): what the
    /// presentation reads to hear the county (`bell_stopped` silences the bell at nine and six;
    /// `omen:early_bell` rings it at ten to nine on a Tuesday). Read-only, like everything here.
    pub fn flag(&self, name: &str) -> i32 {
        let Some(s) = self.sym(name) else { return 0 };
        self.state.flags.get(&crate::state::FlagKey::Named(s)).copied().unwrap_or(0)
    }

    /// The day of the week, 0 Sunday (`GameState::weekday`): the train's whistle and the early
    /// bell keep to it.
    pub fn weekday(&self) -> u8 {
        self.state.weekday()
    }

    /// The spells the world has learned (growth is the party's).
    pub fn learned(&self) -> &'a [SpellId] {
        &self.state.growth.spells
    }

    /// A name's sym, content or generated (`None`: never interned).
    pub fn sym(&self, name: &str) -> Option<Sym> {
        self.state.syms.find(name)
    }

    /// A sym's name.
    pub fn name(&self, s: Sym) -> &'a str {
        self.state.syms.name(s)
    }

    /// A mark of this zone by name.
    pub fn mark(&self, s: Sym) -> Option<Mark> {
        self.rt.marks.get(&s).copied()
    }

    /// A rect of this zone by name.
    pub fn rect(&self, s: Sym) -> Option<Rect> {
        self.rt.rects.get(&s).copied()
    }

    /// A key as this zone's rows write it (a content name, or one of the blueprint's own), as a
    /// sym: what `mark`, `rect` and a prop's `key` are asked by.
    pub fn key_sym(&self, k: jane_core::Key) -> Sym {
        crate::sym::of_key(k, &self.rt.locals)
    }

    /// Every mark of this zone with its name, in the blueprint's order.
    pub fn marks(&self) -> impl Iterator<Item = (Sym, Mark)> + 'a {
        let locals: &'a [Sym] = &self.rt.locals;
        self.bp.marks.iter().map(move |(&k, &m)| (crate::sym::of_key(k, locals), m))
    }

    /// Every rect of this zone with its name, in the blueprint's order.
    pub fn rects(&self) -> impl Iterator<Item = (Sym, Rect)> + 'a {
        let locals: &'a [Sym] = &self.rt.locals;
        self.bp.rects.iter().map(move |(&k, &r)| (crate::sym::of_key(k, locals), r))
    }

    /// The skeleton's patches of this zone as placed (the county's; none elsewhere), each with its
    /// name, in the skeleton's order: what the atmosphere's fog volumes key to (WORLD.md §5.3).
    pub fn areas(&self) -> impl Iterator<Item = (Sym, Rect)> + 'a {
        let locals: &'a [Sym] = &self.rt.locals;
        self.bp.areas.iter().map(move |a| (crate::sym::of_key(a.name, locals), a.rect))
    }

    /// This zone's merged trigger table in the order its bits index, each with whether it fired.
    pub fn triggers(&self) -> impl Iterator<Item = (&'a ZoneTrigger, bool)> + 'a {
        let zone = self.zone;
        self.rt.triggers.iter().enumerate().map(move |(i, t)| (t, zone.triggers.fired.get(i as u32)))
    }

    /// An action list from the catalog or this zone's blueprint.
    pub fn list(&self, r: ListRef) -> &'a [Action] {
        match r {
            ListRef::Catalog(_) => jane_data::catalog().list(r),
            ListRef::Blueprint(_) => self.bp.list(r).unwrap_or(&[]),
        }
    }

    /// A unit of this zone by id, if it is awake and in the world (what `units_in` would show).
    pub fn unit(&self, id: UnitId) -> Option<&'a Unit> {
        self.zone.unit(id).filter(|u| u.awake && !u.hidden)
    }

    /// Every prop of this zone, hidden ones left out, in id order.
    pub fn props(&self) -> impl Iterator<Item = &'a Prop> + 'a {
        self.zone.props.iter().filter(|p| !p.hidden)
    }

    /// Would a body with its feet at `pos` stand clear of everything solid to feet here (terrain,
    /// props' drawn ground boxes, a cliff's lip)? The sim's own test (`units::box_blocked`), finer
    /// than [`flags`](Self::flags): what a reader needs to walk out of the space behind a crate.
    pub fn feet_fit(&self, pos: Vec2) -> bool {
        !crate::units::box_blocked(&self.rt.grid, pos.x, pos.y)
    }

    /// The sim's own sight line from `a` to `b` (what a bolt or a spell asking for sight needs).
    pub fn sight(&self, a: Vec2, b: Vec2) -> bool {
        crate::los::line_of_sight(&self.rt.grid, a, b)
    }

    /// The seat whose body `unit` is, if a connected seat's: her coat is the seat's, the one
    /// thing that tells players apart (PLATFORM.md §2; PRESENTATION.md §3.6 `friend_seat`).
    pub fn seat_of(&self, unit: UnitId) -> Option<Seat> {
        self.state.players.iter().find(|p| p.connected && p.unit == unit).map(|p| p.seat)
    }

    /// The rest of the party sitting down: each seat, the zone her body is in and where it
    /// stands (the map's and the party frame's "where is she"; a follower walks to her).
    pub fn friends(&self) -> impl Iterator<Item = (Seat, ZoneId, Vec2)> + 'a {
        let state = self.state;
        let me = self.seat;
        state.players.iter().filter(move |p| p.connected && p.seat != me).filter_map(move |p| {
            let u = state.zone(p.zone)?.unit(p.unit)?;
            Some((p.seat, p.zone, u.pos))
        })
    }

    /// Whether a damage or heal number is hers to see: she dealt it or took it (PLATFORM.md §2,
    /// "numbers are yours"). A friend's fight shows its sparks, never her arithmetic. False for
    /// any other kind.
    pub fn is_my_number(&self, kind: &crate::event::EventKind) -> bool {
        use crate::event::EventKind;
        let me = self.me().unit;
        match *kind {
            EventKind::Damage { unit, from, .. } | EventKind::Heal { unit, from, .. } => unit == me || from == Some(me),
            _ => false,
        }
    }
}

fn prev_pos(rt: &ZoneRuntime, id: UnitId) -> Option<Vec2> {
    rt.prev_pos.binary_search_by_key(&id, |&(u, _)| u).ok().map(|i| rt.prev_pos[i].1)
}

/// The living world as presentation reads it (ARCHITECTURE.md §4.6, §11): the sky, the puddles,
/// where a scheduled person is. None of it changes state.
impl<'a> View<'a> {
    /// The region she stands in: the county's by the ground under her feet, any other zone's
    /// its own (`living.rs`).
    pub fn region(&self) -> jane_data::Region {
        let (x, y) = self.body().pos.cell();
        self.rt.region_at(x, y)
    }

    /// The region under cell `(cx, cy)` of her zone: the ramps its ground is painted in.
    pub fn region_at(&self, cx: i32, cy: i32) -> jane_data::Region {
        self.rt.region_at(cx, cy)
    }

    /// The sky over her: her region's (`living.rs`). Presentation's mist, rain and storm.
    pub fn weather(&self) -> &'a crate::state::WeatherState {
        &self.state.weather[crate::living::region_ix(self.region())]
    }

    /// The sky over any region (the map's weather column, a far view).
    pub fn weather_of(&self, region: jane_data::Region) -> &'a crate::state::WeatherState {
        &self.state.weather[crate::living::region_ix(region)]
    }

    /// The rain ramp under her feet, 0..=255: puddles, the sound of it. The sim reads the ramp
    /// of a fire's own ground for the douse rule.
    pub fn wetness(&self) -> u8 {
        self.zone.wetness[crate::living::region_ix(self.region())]
    }

    /// The rain ramp at any cell of her zone (a puddle she can see from where she stands).
    pub fn wetness_at(&self, cell: Cell) -> u8 {
        crate::living::wetness_at(self.zone, self.rt, i32::from(cell.x), i32::from(cell.y))
    }

    /// Where a scheduled unit of her zone is and why (a door can say who is behind it); `None`
    /// for a unit with no hours.
    pub fn schedule_state(&self, unit: UnitId) -> Option<crate::living::ScheduleState> {
        crate::living::schedule_state(self.state, self.zone, self.rt, unit)
    }

    /// Where a person of this row keeps hour `hour` today, by the row's hours as written (what a
    /// quest's `returnTo` and a person's own words give): a mark, a patrol, behind a door, or
    /// away; `None` for a row with no hours (always where it is).
    pub fn slot_at_hour(&self, def: jane_core::UnitDefId, hour: u8) -> Option<jane_data::ScheduleSlot> {
        let d = jane_data::catalog().combat.unit(def);
        crate::presence::row_at(d, self.state, hour).map(|r| r.slot)
    }

    /// The hour of the day, 0 to 23.
    pub fn hour(&self) -> u8 {
        self.state.hour() as u8
    }
}

/// Which mark a person shows over her head, for one seat (PRESENTATION.md §3.8): the owner's
/// only markers in the world. Presentation draws it; the sim only says which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestMark {
    /// Talking to them now would give her a quest she has not had.
    Offer,
    /// A quest of hers is ready, and talking to them now would take it back.
    HandIn,
}

/// Conversations followed into (`Talk`) and lists nested (`If`, `Send`) before giving up.
const MARK_DEPTH: u8 = 6;

/// The quest marks (PRESENTATION.md §3.8): what a conversation would open on for this seat,
/// asked read only, as `dialogue::start` asks it.
impl View<'_> {
    /// The mark over `unit` for this seat: a hand-in over an offer. Asked as she would ask it
    /// now: the tree's start rules with her as the actor and the person as the speaker, then
    /// every node that start can reach, each `If` taken the way it goes now. A quest hidden
    /// until something happens (a start rule on a flag) shows nothing until it has. `None` for
    /// someone with nothing to say, hostile, hidden or dead.
    pub fn quest_mark(&self, unit: &Unit) -> Option<QuestMark> {
        let cat = jane_data::catalog();
        if !unit.alive || unit.hidden || unit.faction != jane_data::Faction::Friendly {
            return None;
        }
        let tree = cat.combat.unit(unit.def).talk?;
        let ask = crate::actions::Ask {
            cat,
            world: self.state,
            zone: self.zone,
            rt: self.rt,
            bp: self.bp,
            actor: Some(self.seat),
            speaker: Speaker::Unit(unit.id),
        };
        let mut found = (false, false);
        self.mark_tree(&ask, tree, 0, None, &mut found);
        match found {
            (_, true) => Some(QuestMark::HandIn),
            (true, false) => Some(QuestMark::Offer),
            _ => None,
        }
    }

    /// Every person of her zone awake and in the world with a mark for this seat, in id order.
    pub fn quest_marks(&self) -> impl Iterator<Item = (UnitId, QuestMark)> + '_ {
        self.zone.units.iter().filter(|u| u.awake).filter_map(|u| self.quest_mark(u).map(|m| (u.id, m)))
    }

    /// The mark over a thing that gives quests by its words (the lost-property book, the parish
    /// board, a farmhouse door), asked as [`quest_mark`](Self::quest_mark) asks a person: what
    /// using it now would open on, with the prop as the speaker. `None` for one hidden, or one a
    /// use would not read (locked, a way through, still holding something to take, a thing to
    /// carry or a cupboard: `interact::use_prop` does those first).
    pub fn prop_quest_mark(&self, p: &Prop) -> Option<QuestMark> {
        if p.hidden || p.locked {
            return None;
        }
        let cat = jane_data::catalog();
        let spawn = spawn_of(self.bp, p)?;
        let d = cat.story.prop(p.def);
        if spawn.to.is_some() || d.carry || d.store || crate::interact::has_loot(self.bp, p) {
            return None;
        }
        let tree = spawn.talk?;
        let ask = crate::actions::Ask {
            cat,
            world: self.state,
            zone: self.zone,
            rt: self.rt,
            bp: self.bp,
            actor: Some(self.seat),
            speaker: Speaker::Prop(p.id),
        };
        let mut found = (false, false);
        self.mark_tree(&ask, tree, 0, None, &mut found);
        match found {
            (_, true) => Some(QuestMark::HandIn),
            (true, false) => Some(QuestMark::Offer),
            _ => None,
        }
    }

    /// Every thing of her zone with a mark for this seat, in its order.
    pub fn prop_quest_marks(&self) -> impl Iterator<Item = (crate::ids::PropId, QuestMark)> + '_ {
        self.props().filter_map(|p| self.prop_quest_mark(p).map(|m| (p.id, m)))
    }

    /// The cell under every "!" of her zone for this seat, people (near her or not) and things
    /// alike: how crowded the offers stand (PLAY-PLAN.md 0.4, at most three in a place).
    pub fn offer_cells(&self) -> Vec<(i32, i32)> {
        let people = self.zone.units.iter().filter(|u| self.quest_mark(u) == Some(QuestMark::Offer));
        let things = self.props().filter(|p| self.prop_quest_mark(p) == Some(QuestMark::Offer));
        people.map(|u| u.pos.cell()).chain(things.map(|p| (i32::from(p.cell.x), i32::from(p.cell.y)))).collect()
    }

    /// The most "!" within `cells` of any one of them (it counted too), and theirs: the crowd a
    /// place shows her at once (PLAY-PLAN.md 0.4).
    pub fn offer_crowd(&self, cells: i32) -> Vec<(i32, i32)> {
        let all = self.offer_cells();
        let near = |a: (i32, i32)| -> Vec<(i32, i32)> {
            let r2 = i64::from(cells) * i64::from(cells);
            all.iter().copied().filter(|b| i64::from(a.0 - b.0).pow(2) + i64::from(a.1 - b.1).pow(2) <= r2).collect()
        };
        all.iter().map(|&c| near(c)).max_by_key(Vec::len).unwrap_or_default()
    }

    /// Whether talking to `speaker` (someone or something of her zone talking as `tree`) now
    /// would reach a `quest` verb giving `q`, asked as the marks ask it: the "!" for one quest,
    /// which the abandon proofs read (`quests::abandon`: set aside, it is offered again).
    pub fn would_offer(&self, tree: DialogueId, speaker: Speaker, q: QuestId) -> bool {
        let ask = crate::actions::Ask {
            cat: jane_data::catalog(),
            world: self.state,
            zone: self.zone,
            rt: self.rt,
            bp: self.bp,
            actor: Some(self.seat),
            speaker,
        };
        let mut found = (false, false);
        self.mark_tree(&ask, tree, 0, Some(q), &mut found);
        found.0
    }

    fn mark_tree(
        &self,
        ask: &crate::actions::Ask<'_>,
        tree: DialogueId,
        depth: u8,
        want: Option<QuestId>,
        found: &mut (bool, bool),
    ) {
        let t = ask.cat.story.dialogue(tree);
        let Some(entry) =
            t.start.iter().find(|s| s.when.is_none_or(|w| crate::actions::conditions_hold(ask, ask.conds(w))))
        else {
            return;
        };
        // The nodes that start reaches, by `goto` and by either option.
        let mut seen = vec![false; t.nodes.len()];
        let mut todo = vec![entry.node];
        while let Some(i) = todo.pop() {
            let Some(slot) = seen.get_mut(usize::from(i)) else { continue };
            if std::mem::replace(slot, true) {
                continue;
            }
            let n = t.node(i);
            if let Some(l) = n.actions {
                self.mark_list(ask, l, depth, want, found);
            }
            todo.extend(n.goto);
            for o in n.options {
                if let Some(l) = o.actions {
                    self.mark_list(ask, l, depth, want, found);
                }
                todo.extend(o.goto);
            }
        }
    }

    fn mark_list(
        &self,
        ask: &crate::actions::Ask<'_>,
        l: ListRef,
        depth: u8,
        want: Option<QuestId>,
        found: &mut (bool, bool),
    ) {
        if depth > MARK_DEPTH {
            return;
        }
        for a in ask.list(l) {
            match *a {
                Action::Quest(q) if want.is_none_or(|w| w == q) => {
                    found.0 |= crate::quests::active(self.state, q).is_none() && !crate::quests::done(self.state, q);
                }
                Action::HandIn(q) => found.1 |= crate::quests::ready(self.state, q),
                Action::If { when, then, els } => {
                    if crate::actions::conditions_hold(ask, ask.conds(when)) {
                        self.mark_list(ask, then, depth + 1, want, found);
                    } else if let Some(e) = els {
                        self.mark_list(ask, e, depth + 1, want, found);
                    }
                }
                Action::Send { then: Some(t), .. } => self.mark_list(ask, t, depth + 1, want, found),
                Action::Talk(t) => self.mark_tree(ask, t, depth + 1, want, found),
                _ => {}
            }
        }
    }
}

/// What her active quests still want (PRESENTATION.md §3.8, the quest sparkles): the items a
/// step asks her to hold and does not yet have enough of, and the places a step asks her to
/// have been and she has not. Read through [`View::quest_wants`], once a tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestWants {
    pub items: Vec<ItemId>,
    pub places: Vec<Sym>,
}

impl QuestWants {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.places.is_empty()
    }
}

/// The quest sparkles (PRESENTATION.md §3.8): what a step still wants, and whether a prop or a
/// thing lying on the ground is what it wants. Read only.
impl<'a> View<'a> {
    /// What her active quests' unfinished steps want, for this seat.
    pub fn quest_wants(&self) -> QuestWants {
        let mut w = QuestWants::default();
        let cat = jane_data::catalog();
        for q in self.quests() {
            for (i, r) in cat.story.quest(q.quest).requirements.iter().enumerate() {
                if q.count(i) >= r.qty {
                    continue;
                }
                match r.target {
                    jane_data::ReqTarget::Acquire(item) if !w.items.contains(&item) => w.items.push(item),
                    jane_data::ReqTarget::Location(n) => {
                        let s = crate::sym::of_name(n);
                        if !w.places.contains(&s) {
                            w.places.push(s);
                        }
                    }
                    _ => {}
                }
            }
        }
        w
    }

    /// Whether prop `p` is something a step wants her to use, read, take or open: it holds a
    /// wanted item, or using it or talking to it gives one or marks a wanted place. Hidden and
    /// emptied things are not.
    pub fn prop_wanted(&self, w: &QuestWants, p: &'a Prop) -> bool {
        if w.is_empty() || p.hidden {
            return false;
        }
        if self.prop_loot(p).iter().any(|s| w.items.contains(&s.item)) {
            return true;
        }
        let Some(spawn) = self.prop_spawn(p) else { return false };
        let does = |a: &Action| match *a {
            Action::Give(s) => w.items.contains(&s.item),
            Action::Location(k) => w.places.contains(&self.key_sym(k)),
            _ => false,
        };
        let mut hit = false;
        if let Some(l) = spawn.use_list {
            self.visit_list(l, 0, &mut |a| hit |= does(a));
        }
        if let (false, Some(t)) = (hit, spawn.talk) {
            for n in jane_data::catalog().story.dialogue(t).nodes {
                for l in n.actions.into_iter().chain(n.options.iter().filter_map(|o| o.actions)) {
                    self.visit_list(l, 0, &mut |a| hit |= does(a));
                }
            }
        }
        hit
    }

    /// Whether a thing lying on the ground is an item a step wants.
    pub fn drop_wanted(&self, w: &QuestWants, d: &Drop) -> bool {
        w.items.contains(&d.item)
    }

    fn visit_list(&self, l: ListRef, depth: u8, f: &mut impl FnMut(&Action)) {
        if depth > MARK_DEPTH {
            return;
        }
        for a in self.list(l) {
            f(a);
            match *a {
                Action::If { then, els, .. } => {
                    self.visit_list(then, depth + 1, f);
                    if let Some(e) = els {
                        self.visit_list(e, depth + 1, f);
                    }
                }
                Action::Send { then: Some(t), .. } => self.visit_list(t, depth + 1, f),
                _ => {}
            }
        }
    }
}
