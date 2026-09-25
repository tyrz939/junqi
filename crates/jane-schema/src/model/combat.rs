//! Spells, effects, units, items, recipes.
//!
//! Units: every time is a `Tick` (60 Hz), every distance and speed an `Fx` (1/256 px; a metre
//! is a cell, `METRE_FX = 2048`), every direction an `Angle`, every fraction a `Permille`, and
//! hp, mp, energy and damage are `Milli` (ARCHITECTURE.md §2, §6). Each field says which.

use jane_core::Angle;
use jane_core::action::{ListRef, School, Stat};
use jane_core::ids::{DialogueId, EffectId, ItemId, NameId, QuestId, SpellId, SpriteId, TextId, UnitDefId};
use jane_core::num::{Fx, Milli, Permille, Tick};

use crate::emit::Emit;
use crate::{model, model_enum};

/// A value per school, indexed by [`School`] in its declared order (heal, physical, frost,
/// fire, nature, blast, shock). Total: a school content leaves out holds the table's default.
pub type BySchool = [Permille; 7];

/// The slot of `school` in a [`BySchool`].
pub const fn school_index(school: School) -> usize {
    school as usize
}

model_enum! {
    /// What a spell lands on (`catalog.ts SpellKind`).
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum SpellKind {
        /// A swing at the reach in front of the caster.
        Melee,
        /// A projectile (or a fan or ring of them).
        Bolt,
        /// Lands on the caster (content writes `"self"`).
        #[cfg_attr(feature = "compile", serde(rename = "self"))]
        OnSelf,
        /// Lands on the friend nearest the aim line, or on the caster when nobody is there.
        Ally,
        /// Does its [`WorldSpell`] to the prop in front of the caster.
        World,
        /// A pool on the ground that pulses.
        Ground,
    }
}

model_enum! {
    /// What a world spell does to the prop in front of the caster.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum WorldSpell {
        Repair,
        Grow,
    }
}

model_enum! {
    /// A unit's animation (`state.ts Anim`); a spell names the one its caster plays.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum CastAnim {
        Idle,
        Walk,
        Attack,
        Cast,
        Hurt,
        Dead,
    }
}

model_enum! {
    /// Who a unit fights for (`state.ts Faction`).
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum Faction {
        Undead,
        Beast,
        Bandit,
        /// The party's side: people, animals and the player. Always an `Npc` or `Player`.
        Friendly,
    }
}

model_enum! {
    /// What drives a unit (`state.ts Controller`).
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum Controller {
        Player,
        Ai,
        Npc,
        /// `sim/snake.ts`: its own locomotion and phase clock; needs a [`SnakeBody`].
        Snake,
    }
}

model_enum! {
    /// What a unit can see.
    #[cfg_attr(feature = "compile", derive(serde::Deserialize), serde(rename_all = "lowercase"))]
    pub enum UnitSight {
        /// Anything in reach (content leaves `sight` out; it never writes this).
        #[cfg_attr(feature = "compile", serde(skip))]
        Any,
        /// Only notices, and only keeps, a target standing in a prop's light (the Factory's sentries).
        Lit,
    }
}

model! {
    /// How a spell rolls its amount: `stat / div + irandom(stat / var_div) + flat`.
    /// 2020 melee: strength / 8 + irandom(strength / 32).
    pub struct SpellPower {
        pub stat: Stat,
        /// The fixed part's divisor, in thousandths (`1.25` is 1250). Always > 0.
        pub div: u32,
        /// The random part's divisor, in thousandths (`32` is 32 000). Always > 0.
        pub var_div: u32,
        /// Added to every roll, `Milli` (0 when content leaves it out).
        pub flat: Milli,
    }
}

model! {
    /// A bolt's splash: `hit / div` on every other enemy near the impact.
    pub struct BoltSplash {
        /// From the impact to a body's edge, `Fx` (content writes px).
        pub radius: Fx,
        /// The hit's divisor, a whole number ≥ 1.
        pub div: u16,
    }
}

model! {
    /// A ground spell's pool.
    pub struct GroundPool {
        /// `Fx` (content writes metres). Always > 0.
        pub radius: Fx,
        /// How long the pool lies, `Tick` (content writes seconds). Always > 0.
        pub duration: Tick,
        /// Between pulses, `Tick` (content writes seconds; 30 ticks when left out, as `combat.ts`).
        pub pulse: Tick,
    }
}

model! {
    /// A spell row (`data/spells.json` and `data/spells/*.json`).
    pub struct SpellDef {
        /// The content id (`"icebolt"`): for tools, debug views and errors.
        pub id: &'static str,
        pub name: TextId,
        pub description: TextId,
        pub icon: SpriteId,
        pub kind: SpellKind,
        pub school: School,
        /// Mana cost, `Milli`.
        pub mp: Milli,
        /// Energy cost, `Milli`.
        pub energy: Milli,
        /// Reach between bodies' bounds, `Fx` (content writes metres). Melee reach for `Melee`.
        pub range: Fx,
        /// `Tick` (content writes seconds).
        pub cooldown: Tick,
        /// Neither starts nor waits on the global cooldown.
        pub gcd_immune: bool,
        pub needs_target: bool,
        pub needs_enemy: bool,
        pub needs_los: bool,
        /// What the caster plays.
        pub anim: CastAnim,
        /// Always there for `Melee` and `Bolt`.
        pub power: Option<SpellPower>,
        /// Bolt speed, `Fx` per tick (content writes px per tick). Always > 0 for a `Bolt`.
        pub speed: Option<Fx>,
        /// Bolts per cast, ≥ 1 (1 when left out).
        pub count: u8,
        /// The arc the bolts cover, `Angle`: 0 is a line, `Angle(u16::MAX)` is a ring (content's
        /// 360) with the bolts evenly spaced, anything between is random inside the arc.
        pub fan: Angle,
        pub splash: Option<BoltSplash>,
        /// Applied to whoever the spell lands on (the caster, for `OnSelf`).
        pub effect: Option<EffectId>,
        /// Handed back to the caster when a melee lands, `Milli` (0 when left out).
        pub restore_energy: Milli,
        /// There exactly when `kind` is `Ground`.
        pub ground: Option<GroundPool>,
        /// There exactly when `kind` is `World`.
        pub world: Option<WorldSpell>,
        /// How long the caster is held still after casting, `Tick` (content writes ticks; 0 when left out).
        pub stop: Tick,
        /// Light the projectile carries, radius in `Fx` (content writes px).
        pub glow: Option<Fx>,
        /// How close to a prop's middle a bolt must end to switch on a prop that answers its
        /// school, `Fx` (content writes px). A bolt's only; the sim's default (14 px) when `None`.
        pub touch: Option<Fx>,
    }
}

model! {
    /// A periodic pulse of an effect. The heal school heals.
    pub struct EffectPulse {
        /// Per pulse, `Milli`.
        pub amount: Milli,
        /// Between pulses, `Tick` (content writes seconds), at least 1.
        pub every: Tick,
        pub school: School,
    }
}

model! {
    /// What an effect adds to every melee hit its bearer lands.
    pub struct OnMelee {
        pub school: School,
        /// `Milli`.
        pub amount: Milli,
        pub effect: Option<EffectId>,
    }
}

model! {
    /// An effect row (`data/effects.json` and `data/effects/*.json`): a status on a unit.
    pub struct EffectDef {
        /// The content id (`"chilled"`).
        pub id: &'static str,
        pub name: TextId,
        pub icon: SpriteId,
        /// `Tick` (content writes seconds). 0 is instant: only `heal` and `mana` land.
        pub duration: Tick,
        pub harmful: bool,
        /// Movement multiplier, `Permille` 0..=1000 (1000 when left out: no change; 0 roots).
        pub speed: Permille,
        /// Cannot act at all.
        pub stun: bool,
        pub pulse: Option<EffectPulse>,
        /// Instant heal on apply, `Milli` (0: none).
        pub heal: Milli,
        /// Instant mana on apply, `Milli` (0: none).
        pub mana: Milli,
        /// Incoming damage multiplier per school, `Permille` (1000: no change, 500: half, 2000: double).
        pub resist: BySchool,
        /// Damage is paid from mp first at this mp per point of damage, `Permille` (0: no shield).
        pub mana_shield: Permille,
        /// Fraction of damage dealt returned as health, `Permille` (0: none).
        pub lifesteal: Permille,
        /// Replaces the 1-in-N crit roll (2 = every other hit); 0 when left out.
        pub crit_one_in: u16,
        pub on_melee: Option<OnMelee>,
        /// Taking a hit restores this much mp, `Milli` (0: none).
        pub mana_on_hit: Milli,
        /// Lands only on a unit whose own row is weak to this school (its resist below 0). Never `Heal`.
        pub only_if_weak: Option<School>,
        /// While it lasts, the unit row's own resists count for nothing (weaknesses stay).
        pub no_resist: bool,
    }
}

impl EffectDef {
    /// The incoming damage multiplier for `school`: 1000 is no change.
    pub const fn resist_of(&self, school: School) -> Permille {
        self.resist[school_index(school)]
    }
}

model! {
    /// One roll of a unit's loot.
    pub struct LootRoll {
        pub item: ItemId,
        /// ≥ 1.
        pub qty: u16,
        /// `Permille`, 1..=1000.
        pub chance: Permille,
    }
}

model! {
    /// Light a unit carries (the 2020 bat wore a red one).
    pub struct UnitGlow {
        /// `Fx` (content writes px).
        pub radius: Fx,
        /// `0xRRGGBB`, from content's `"#rrggbb"`.
        pub color: u32,
    }
}

model! {
    /// A snake's body.
    pub struct SnakeBody {
        /// Segments, ≥ 1.
        pub segments: u16,
        /// Between segments, `Fx` (content writes px). Always > 0.
        pub spacing: Fx,
    }
}

model! {
    /// A boss's phase row. Entered when health falls to `hp_below`: the unit takes its book (and
    /// `run`), and `on_enter` runs once with the boss as the subject. The snake reads the table by
    /// its own clock instead (`sim/snake.ts`).
    pub struct BossPhase {
        /// Fraction of full health, `Permille` 1..=1000. Falling row by row, except for a snake.
        pub hp_below: Permille,
        pub book: &'static [SpellId],
        /// Run speed in this phase, `Fx` per tick (content writes px per tick); the row's `run` when `None`.
        pub run: Option<Fx>,
        pub on_enter: Option<ListRef>,
    }
}

/// Where a scheduled unit is during a slot (ARCHITECTURE.md §4.6.a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleSlot {
    /// Standing at a mark.
    Mark(NameId),
    /// Walking its patrol.
    Patrol,
    /// Hidden inside a prop (a door can say who is behind it).
    Inside(NameId),
    /// Not in the world.
    Absent,
}

impl Emit for ScheduleSlot {
    fn emit(&self, out: &mut String) {
        match self {
            ScheduleSlot::Mark(n) => {
                out.push_str("ScheduleSlot::Mark(");
                n.emit(out);
                out.push(')');
            }
            ScheduleSlot::Patrol => out.push_str("ScheduleSlot::Patrol"),
            ScheduleSlot::Inside(n) => {
                out.push_str("ScheduleSlot::Inside(");
                n.emit(out);
                out.push(')');
            }
            ScheduleSlot::Absent => out.push_str("ScheduleSlot::Absent"),
        }
    }
}

model! {
    /// One row of a unit's schedule: from `hour_from` up to `hour_to` (hours 0..=23; a row may
    /// wrap midnight), the unit is at `slot`.
    pub struct ScheduleRow {
        pub hour_from: u8,
        pub hour_to: u8,
        pub slot: ScheduleSlot,
    }
}

model! {
    /// A unit row (`data/units.json` and `data/units/*.json`).
    pub struct UnitDef {
        /// The content id (`"caretaker"`).
        pub id: &'static str,
        pub name: TextId,
        pub faction: Faction,
        pub controller: Controller,
        /// A whole stat, > 0.
        pub strength: u16,
        /// A whole stat.
        pub spirit: u16,
        /// `Fx` per tick (content writes px per tick; 2020 player walk 1, run 2).
        pub walk: Fx,
        /// `Fx` per tick (content writes px per tick).
        pub run: Fx,
        /// `Fx` (content writes metres).
        pub aggro: Fx,
        /// `Fx` (content writes metres).
        pub leash: Fx,
        /// SnakeBody radius, `Fx` (content writes metres; 1 when left out). Range is measured between bounds.
        pub bounds: Fx,
        pub book: &'static [SpellId],
        /// `Tick` (content writes seconds). 0 = never.
        pub respawn: Tick,
        pub auto_regen: bool,
        pub loot: &'static [LootRoll],
        /// Incoming damage reduction per school, `Permille`: the damage is multiplied by
        /// `1000 - resist` (0: none, 1000: immune, below 0: weak). A school left out is 0.
        pub resist: BySchool,
        pub sprite: SpriteId,
        pub on_death: Option<ListRef>,
        pub talk: Option<DialogueId>,
        pub boss: bool,
        pub glow: Option<UnitGlow>,
        /// Present only between 06:00 and 21:00. Later a two-slot [`ScheduleRow`] list (ARCHITECTURE.md §4.6.a).
        pub day_only: bool,
        /// `day_only` holds only once this quest is done. Only with `day_only`.
        pub day_only_after: Option<QuestId>,
        /// Not there between 06:00 and 21:00. Never with `day_only`.
        pub night_only: bool,
        /// An item it cannot resist: while idle it walks to a drop of it and dies there.
        pub bait: Option<ItemId>,
        /// There exactly when `controller` is `Snake`.
        pub body: Option<SnakeBody>,
        /// Boss phases, in falling `hp_below` (a snake's by its own clock).
        pub phases: &'static [BossPhase],
        pub sight: UnitSight,
        /// It will not step into warm light: it walks to the edge of it and waits there.
        pub shuns_light: bool,
        /// Where the unit is by the hour (ARCHITECTURE.md §4.6.a). Empty: always present (or
        /// `day_only` / `night_only`). Content does not write it yet.
        pub schedule: &'static [ScheduleRow],
        /// Unit defs an idle one takes as a target inside its aggro reach (ARCHITECTURE.md §4.6.c).
        pub hunts: &'static [UnitDefId],
        /// Unit defs an idle one walks its leash away from (ARCHITECTURE.md §4.6.c).
        pub flees: &'static [UnitDefId],
    }
}

impl UnitDef {
    /// The incoming damage reduction for `school`: 0 is none, below 0 is a weakness.
    pub const fn resist_of(&self, school: School) -> Permille {
        self.resist[school_index(school)]
    }
}

model! {
    /// An item row (`data/items.json` and `data/items/*.json`).
    pub struct ItemDef {
        /// The content id (`"julies_letter"`): for tools, saves' debug views and errors.
        pub id: &'static str,
        pub name: TextId,
        pub description: TextId,
        pub icon: SpriteId,
        pub max_stack: u16,
        pub usable: bool,
        pub cooldown: Tick,
        /// Runs on the user; consumes one unless `keep`.
        pub use_list: Option<ListRef>,
        pub keep: bool,
        /// Key tag: unlocks any prop whose `keyTag` matches. One use path for every key.
        pub opens: Option<NameId>,
        /// Cannot be destroyed or dropped.
        pub bound: bool,
        /// Derived: what the story cannot go on without (anything that `opens`, anything a quest
        /// asks the party to acquire). Never ages out on the ground; handed on when its holder leaves.
        pub story: bool,
    }
}

model! {
    /// A recipe. `inputs` are sorted by id: a recipe is keyed by its sorted inputs, never by a
    /// display name (renaming Pansy killed a 2020 recipe).
    pub struct RecipeDef {
        pub inputs: &'static [ItemId],
        pub output: ItemId,
        pub qty: u16,
    }
}

model! {
    pub struct Combat {
        /// Indexed by `SpellId`.
        pub spells: &'static [SpellDef],
        /// Indexed by `EffectId`.
        pub effects: &'static [EffectDef],
        /// Indexed by `UnitDefId`.
        pub units: &'static [UnitDef],
        /// Indexed by `ItemId`.
        pub items: &'static [ItemDef],
        /// Indexed by `RecipeId`, in file order.
        pub recipes: &'static [RecipeDef],
    }
}

impl Combat {
    pub fn spell(&self, id: SpellId) -> &'static SpellDef {
        &self.spells[id.index()]
    }

    pub fn effect(&self, id: EffectId) -> &'static EffectDef {
        &self.effects[id.index()]
    }

    pub fn unit(&self, id: UnitDefId) -> &'static UnitDef {
        &self.units[id.index()]
    }

    pub fn item(&self, id: ItemId) -> &'static ItemDef {
        &self.items[id.index()]
    }

    /// The recipe whose sorted inputs are exactly `inputs` (sorted by the caller).
    pub fn recipe_for(&self, inputs: &[ItemId]) -> Option<&'static RecipeDef> {
        self.recipes.iter().find(|r| r.inputs == inputs)
    }

    /// A spell's id by its content id, by a scan: for tools and tests, never the hot path.
    pub fn spell_id(&self, id: &str) -> Option<SpellId> {
        self.spells.iter().position(|d| d.id == id).map(|i| SpellId(i as u16))
    }

    /// An effect's id by its content id, by a scan: for tools and tests.
    pub fn effect_id(&self, id: &str) -> Option<EffectId> {
        self.effects.iter().position(|d| d.id == id).map(|i| EffectId(i as u16))
    }

    /// A unit def's id by its content id, by a scan: for tools and tests.
    pub fn unit_id(&self, id: &str) -> Option<UnitDefId> {
        self.units.iter().position(|d| d.id == id).map(|i| UnitDefId(i as u16))
    }

    /// An item's id by its content id, by a scan: for tools and tests.
    pub fn item_id(&self, id: &str) -> Option<ItemId> {
        self.items.iter().position(|d| d.id == id).map(|i| ItemId(i as u16))
    }
}
