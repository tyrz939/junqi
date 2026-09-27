//! Props, quests, dialogue, triggers, the clock, the start.
//!
//! Units: footprints are cells; a light's radius is `Fx` (1/256 px; the TypeScript wrote px);
//! a flicker is `Permille` of full brightness; colours are `0xRRGGBB`; hours are 0..=23.

use jane_core::action::{CondsRef, FactKey, ListRef, School, Stack};
use jane_core::blueprint::Trigger;
use jane_core::ids::{
    DialogueId, ItemId, NameId, PropDefId, QuestId, SpellId, SpriteId, TextId, TriggerId, UnitDefId, ZoneId,
};
use jane_core::num::{Fx, Permille};

use crate::emit::Emit;
use crate::{model, model_enum};

model_enum! {
    /// What switches a prop on (its `use` list runs when it lands): a world verb, or a damage
    /// school (2020's wall torches lit on frost damage). Never `heal`.
    pub enum Answers {
        Repair,
        Grow,
        Physical,
        Frost,
        Fire,
        Nature,
        Blast,
        Shock,
    }
}

impl Answers {
    /// The damage school this answers to, if it answers to one.
    pub const fn school(self) -> Option<School> {
        match self {
            Answers::Repair | Answers::Grow => None,
            Answers::Physical => Some(School::Physical),
            Answers::Frost => Some(School::Frost),
            Answers::Fire => Some(School::Fire),
            Answers::Nature => Some(School::Nature),
            Answers::Blast => Some(School::Blast),
            Answers::Shock => Some(School::Shock),
        }
    }
}

model! {
    /// A prop's light. The sim counts a point as lit inside two thirds of `radius`
    /// (`r * r * 4 / 9` in Fx², ARCHITECTURE.md §2); presentation draws the rest.
    pub struct Light {
        /// Radius, `Fx` (1/256 px; content writes px).
        pub radius: Fx,
        /// Colour, `0xRRGGBB` (content writes `"#rrggbb"`).
        pub color: u32,
        /// How far the brightness dips as it flickers, permille of full (0 = steady). Presentation only.
        pub flicker: Permille,
        /// Shows what is there and keeps nothing off (the Burial's blue torches): a unit that
        /// shuns light walks straight through it, and it never counts as warm.
        pub cold: bool,
        /// Light that comes down from the sky: a glade's sunbeam or moonbeam, a dry bank in the
        /// sun, the library's hole in the roof. Grow works in it and in no other prop's light (a
        /// lamp, a fire, a torch, a light stone, a bloomed bud); out in the county the sun itself
        /// counts by day (`jane_sim::light::grows_at`).
        pub sky: bool,
    }
}

model! {
    /// A prop row (`data/props.json` and `data/props/*.json`). What a placed prop leads to, holds,
    /// runs and says lives on its spawn (`jane_core::blueprint::PropSpawn`); this is what every
    /// prop of the kind shares.
    pub struct PropDef {
        /// The content id (`"lamp_post"`): for tools, debug views and errors.
        pub id: &'static str,
        /// What it is called: the use prompt's and the locked line's subject.
        pub name: TextId,
        /// Its look (ART.md §5).
        pub sprite: SpriteId,
        /// Footprint width, cells (>= 1).
        pub w: u8,
        /// Footprint height, cells (>= 1).
        pub h: u8,
        /// The rows of the footprint that block, counted from its front (bottom) edge: 1 to `h`.
        /// In the 3/4 view an upright thing (a tree, a chest, a stove) is drawn standing on the
        /// front of its footprint with its top over the back rows, so she can step into them from
        /// above and be drawn behind it (`solid_rect`). `h` when the row does not say.
        pub base: u8,
        /// Blocks movement.
        pub solid: bool,
        /// Blocks line of sight.
        pub block_los: bool,
        /// Moves when walked into and held against (hold-to-push).
        pub push: bool,
        /// Can be picked up and carried.
        pub carry: bool,
        /// A crafting bench: crafting works within reach of one.
        pub bench: bool,
        /// What switches it on; `None` = nothing does.
        pub answers: Option<Answers>,
        /// Its `use` list runs only the first time.
        pub once: bool,
        /// A door that leads nowhere: solid while locked, open once unlocked.
        pub gate: bool,
        /// Pressure plate: `use` when a unit or a pushable covers it, `release` when clear.
        pub plate: bool,
        /// Its light shows only while it is `on`.
        pub light_when_on: bool,
        /// Its light shows only while the lamps are lit (18:30 to 06:30).
        pub night_only: bool,
        /// Its light shows only while the lamps are out (06:30 to 18:30).
        pub day_only: bool,
        /// A bed or a fire: saving works within reach of one, and dying wakes her at the last one used.
        pub rest: bool,
        /// Gathered things vanish once looted.
        pub hide_when_used: bool,
        pub light: Option<Light>,
        /// Zone wetness (0..=255, the rain ramp of ARCHITECTURE.md §4.6.b) at or above which its
        /// light goes out; `None` = rain never puts it out. Only a prop with a light has one.
        pub douse: Option<u8>,
        /// Drawn as what it holds (its first loot item's icon, at ground scale); the sprite is
        /// only for when it holds nothing.
        pub shows_loot: bool,
        /// A floor decal: draws under units.
        pub flat: bool,
        /// The use prompt, instead of the default for what it does ("Open", "Read", "Use").
        pub prompt: Option<TextId>,
        /// What is said of it when tried while locked, after its label; `None` = "is locked".
        pub locked_says: Option<TextId>,
    }
}

impl PropDef {
    /// The cells a solid prop with its footprint's top-left at `(x, y)` blocks: its `base` rows,
    /// the front of its footprint. What the sim stamps solid and the solver floods around.
    pub const fn solid_rect(&self, x: i32, y: i32) -> jane_core::Rect {
        let base = if self.base == 0 || self.base > self.h { self.h } else { self.base };
        jane_core::Rect::new(x, y + self.h as i32 - base as i32, self.w as i32, base as i32)
    }

    /// The light's show rule without the instance: whether a light of this def shows for a prop
    /// that is (or is not) `on`, with the lamps lit or not. Hidden props show none (the caller's).
    pub const fn light_shows(&self, on: bool, lamps_lit: bool) -> bool {
        self.light.is_some()
            && (on || !self.light_when_on)
            && (lamps_lit || !self.night_only)
            && (!lamps_lit || !self.day_only)
    }
}

/// What a quest step asks for, with its target resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReqTarget {
    /// Kill units of this def. Counts only kills made while the quest is in the log.
    Kill(UnitDefId),
    /// Hold this item (read live from the party's bags).
    Acquire(ItemId),
    /// Have been to this location (the `location` verb's name; the `been:` flag it sets).
    Location(NameId),
}

impl Emit for ReqTarget {
    fn emit(&self, out: &mut String) {
        let (v, e): (&str, &dyn crate::emit::EmitDyn) = match self {
            ReqTarget::Kill(u) => ("Kill", u),
            ReqTarget::Acquire(i) => ("Acquire", i),
            ReqTarget::Location(n) => ("Location", n),
        };
        out.push_str("ReqTarget::");
        out.push_str(v);
        out.push('(');
        e.emit_dyn(out);
        out.push(')');
    }
}

model! {
    /// One step of a quest.
    pub struct QuestReq {
        pub target: ReqTarget,
        /// How many (>= 1). A location is 1.
        pub qty: u16,
        /// The step as the tracker shows it (may say `{place:<story>}`).
        pub text: TextId,
    }
}

model! {
    /// A quest row (`data/quests.json` and `data/quests/*.json`).
    pub struct QuestDef {
        /// The content id (`"the_letter"`).
        pub id: &'static str,
        pub name: TextId,
        /// The quest log's text.
        pub description: TextId,
        /// What is said when it is handed in.
        pub completion: TextId,
        /// Who takes it back, as a phrase that follows "Back to": the tracker and the log show it
        /// once every step is done. Never empty; any `{place:<story>}` names a story that exists.
        pub return_to: TextId,
        /// At least one.
        pub requirements: &'static [QuestReq],
        /// Runs once per connected seat, with her as the actor, when it is handed in.
        pub rewards: ListRef,
    }
}

model! {
    /// One line of a dialogue node, and the facts saying it writes into the journal
    /// (ARCHITECTURE.md §3.7: Person Talked, Claim Told, Rumour Heard).
    pub struct DialogueLine {
        pub text: TextId,
        pub tells: &'static [FactKey],
    }
}

model! {
    /// A choice at the end of a node. Choosing runs the node's `actions` then the option's, and
    /// goes to the option's `goto`, else the node's, else closes.
    pub struct DialogueOption {
        pub label: TextId,
        /// A node index in the same tree.
        pub goto: Option<u16>,
        pub actions: Option<ListRef>,
    }
}

model! {
    /// A node: lines shown one at a time, then up to two options, or `actions` and `goto`.
    pub struct DialogueNode {
        /// The node's name in the data, for tools and errors.
        pub id: &'static str,
        /// At least one.
        pub lines: &'static [DialogueLine],
        /// At most two.
        pub options: &'static [DialogueOption],
        /// Runs on leaving the node (after the transition, so a `talk` or `travel` wins over the close).
        pub actions: Option<ListRef>,
        /// A node index in the same tree; `None` closes the conversation.
        pub goto: Option<u16>,
    }
}

model! {
    /// A start rule: the first whose conditions hold picks the node.
    pub struct DialogueStart {
        /// `None` = always. The last rule of every tree has none.
        pub when: Option<CondsRef>,
        /// A node index in the same tree.
        pub node: u16,
    }
}

model! {
    /// A dialogue tree (`data/dialogue.json` and `data/dialogue/*.json`).
    pub struct DialogueTree {
        /// The content id (`"dog"`).
        pub id: &'static str,
        /// Who is speaking, as the box heads it (may be empty: a thing that speaks for itself).
        pub speaker: TextId,
        /// At least one; the last is unconditional.
        pub start: &'static [DialogueStart],
        /// Indexed by the tree's node index: node names in sorted order.
        pub nodes: &'static [DialogueNode],
    }
}

impl DialogueTree {
    pub fn node(&self, i: u16) -> &'static DialogueNode {
        &self.nodes[usize::from(i)]
    }

    /// A node's index by its name, by a scan: for tools and tests.
    pub fn node_index(&self, name: &str) -> Option<u16> {
        self.nodes.iter().position(|n| n.id == name).map(|i| i as u16)
    }
}

impl Emit for Trigger {
    fn emit(&self, out: &mut String) {
        out.push_str("jane_core::blueprint::Trigger { rect: ");
        self.rect.emit(out);
        out.push_str(", mode: ");
        self.mode.emit(out);
        out.push_str(", once: ");
        self.once.emit(out);
        out.push_str(", when: ");
        self.when.emit(out);
        out.push_str(", actions: ");
        self.actions.emit(out);
        out.push_str(", reset: ");
        self.reset.emit(out);
        out.push_str(" }");
    }
}

model! {
    /// A trigger row (`data/triggers.json` and `data/triggers/*.json`): the catalog's half of a
    /// zone's trigger table, merged with the rows its builder wrote.
    pub struct TriggerDef {
        /// The content id (`"arrive_stoop"`).
        pub id: &'static str,
        /// The zone whose blueprint has the rect.
        pub zone: ZoneId,
        /// The rect (a `Key::Name`), mode (default `Enter`), once, conditions, actions and reset.
        /// `reset` runs when a player dies after it fired, with her as the actor.
        pub trigger: Trigger,
    }
}

model! {
    /// Something the whole county does at an hour of the day: the bell at nine. Runs once when
    /// the hour turns, with no actor, so it holds no player-scoped verb (checked at build).
    pub struct ClockDef {
        /// 0..=23.
        pub hour: u8,
        /// 0, 10, 20, 30, 40 or 50: a ten-minute mark, which the clock step and a bed's night
        /// both work (content leaves it out for the hour).
        pub minute: u8,
        pub actions: ListRef,
    }
}

/// An action-bar slot: a spell or an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BarSlot {
    Spell(SpellId),
    Item(ItemId),
}

impl Emit for BarSlot {
    fn emit(&self, out: &mut String) {
        match self {
            BarSlot::Spell(s) => {
                out.push_str("BarSlot::Spell(");
                s.emit(out);
            }
            BarSlot::Item(i) => {
                out.push_str("BarSlot::Item(");
                i.emit(out);
            }
        }
        out.push(')');
    }
}

/// Slots on the action bar.
pub const BAR_SLOTS: usize = 8;

model! {
    /// New Game (`data/start.json`): what every new body carries, and where she stands.
    pub struct StartDef {
        /// Into the bag, in order.
        pub items: &'static [Stack],
        /// The action bar, slot by slot.
        pub bar: [Option<BarSlot>; BAR_SLOTS],
        /// Given at New Game, in order.
        pub quests: &'static [QuestId],
        /// The county mark New Game stands her on.
        pub mark: NameId,
    }
}

model! {
    /// A claim that comes true on some seeds (`data/omens.json`; PLAN.md §5, WORLD.md §7.3).
    /// Rolled once at New Game from the world stream, in row order: true, it sets its flag to 1,
    /// and the rows that read the flag (triggers, clock rows, lists) make the county do what the
    /// claim says. Nothing in the game says which are true.
    pub struct OmenDef {
        /// The content id (`"mine_no_exit"`).
        pub id: &'static str,
        /// The chance it is true on a seed, out of 1000.
        pub chance: u16,
        /// Where its world is.
        pub region: crate::model::Region,
        /// True, it costs a fight: at most one lethal omen of a region comes true on a seed (the
        /// first in row order; a later one rolled true is false).
        pub lethal: bool,
        /// The flag set to 1 when it is true (`omen:<id>`): what it sets.
        pub flag: NameId,
        /// What the county claims, in the words a row says it (for tools and the dossier).
        pub claim: TextId,
    }
}

model! {
    pub struct Story {
        /// Indexed by `PropDefId`.
        pub props: &'static [PropDef],
        /// Indexed by `QuestId`.
        pub quests: &'static [QuestDef],
        /// Indexed by `DialogueId`.
        pub dialogue: &'static [DialogueTree],
        /// Indexed by `TriggerId`.
        pub triggers: &'static [TriggerDef],
        /// In file order (base, then fragments in path order): rows of one hour run in this order.
        pub clock: &'static [ClockDef],
        pub start: StartDef,
        /// In row order: the order they are rolled in.
        pub omens: &'static [OmenDef],
    }
}

impl Story {
    pub fn prop(&self, id: PropDefId) -> &'static PropDef {
        &self.props[id.index()]
    }

    pub fn quest(&self, id: QuestId) -> &'static QuestDef {
        &self.quests[id.index()]
    }

    pub fn dialogue(&self, id: DialogueId) -> &'static DialogueTree {
        &self.dialogue[id.index()]
    }

    pub fn trigger(&self, id: TriggerId) -> &'static TriggerDef {
        &self.triggers[id.index()]
    }

    /// A zone's catalog triggers with their ids, in id order.
    pub fn triggers_in(&self, zone: ZoneId) -> impl Iterator<Item = (TriggerId, &'static TriggerDef)> {
        let all: &'static [TriggerDef] = self.triggers;
        all.iter().enumerate().filter(move |(_, t)| t.zone == zone).map(|(i, t)| (TriggerId(i as u16), t))
    }

    /// The clock rows for an hour and a minute, in row order.
    pub fn clock_at(&self, hour: u8, minute: u8) -> impl Iterator<Item = &'static ClockDef> {
        let all: &'static [ClockDef] = self.clock;
        all.iter().filter(move |c| c.hour == hour && c.minute == minute)
    }

    /// A row id by its content id, by a scan: for tools and tests, never the hot path.
    pub fn prop_id(&self, id: &str) -> Option<PropDefId> {
        self.props.iter().position(|p| p.id == id).map(|i| PropDefId(i as u16))
    }

    pub fn quest_id(&self, id: &str) -> Option<QuestId> {
        self.quests.iter().position(|q| q.id == id).map(|i| QuestId(i as u16))
    }

    pub fn dialogue_id(&self, id: &str) -> Option<DialogueId> {
        self.dialogue.iter().position(|d| d.id == id).map(|i| DialogueId(i as u16))
    }

    pub fn trigger_id(&self, id: &str) -> Option<TriggerId> {
        self.triggers.iter().position(|t| t.id == id).map(|i| TriggerId(i as u16))
    }
}
