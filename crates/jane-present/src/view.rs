//! The view buffers (PRESENTATION.md §3.6): `jane_sim::view::View` walked once a tick into
//! what the HUD, the dialogue box and the window draw from. The buffers persist: their strings
//! and vectors are cleared and refilled, never dropped, and the timed things (toasts, the
//! banner, the bars' damage lag, the bar's flashes) count presenter ticks here.
//!
//! **Rule:** what the sim decides (what USE would do, whether she may save, the bench in reach,
//! what the craft row makes) is read through `View`, never worked out again here.

use jane_core::ids::SpriteId;
use jane_core::{ItemId, QuestId, SpellId, ZoneId};
use jane_data::BarSlot;
use jane_sim::View;
use jane_sim::event::{Event, EventKind, ToastKind, events_for};
use jane_sim::interact::{FocusRef, Verb};
use jane_sim::state::Speaker;
use jane_sim::tuning::{BAG_SLOTS, BAR_SLOTS, CRAFT_INPUTS, ENERGY_MAX, GCD};
use jane_sim::units::{max_hp, max_mp};

use crate::text::{self, Tone};

/// Ticks a toast stays (§3.2), and the last of them it fades over.
pub const TOAST_TICKS: u32 = 180;
pub const TOAST_FADE: u32 = 40;
/// At most this many toasts show; a fourth pushes the oldest out.
pub const TOASTS: usize = 3;
/// Ticks the zone banner stays.
pub const BANNER_TICKS: u32 = 200;
/// Ticks a bar's lag tail holds before it falls, and permille it falls a tick.
pub const LAG_HOLD: u32 = 30;
pub const LAG_FALL: u16 = 14;
/// Ticks a bar slot flashes after a cast or a refusal.
pub const FLASH_TICKS: u8 = 12;
/// Ticks the target frame outlives the last blow between her and it.
pub const TARGET_TICKS: u32 = 600;
/// Ticks the save card stays (§3.2, `ui::saved`): about two seconds; a failure stays twice as
/// long, so it is read.
pub const SAVED_TICKS: u32 = 130;
pub const SAVE_FAILED_TICKS: u32 = 260;

/// The save card: the world was written down (or was not).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCard {
    /// "Saved", "Saved by the teal coat", "Couldn't save: ...".
    pub text: String,
    pub ok: bool,
    /// The presenter tick it went up.
    pub born: u32,
}

impl SavedCard {
    /// Ticks it stays.
    pub fn ticks(&self) -> u32 {
        if self.ok { SAVED_TICKS } else { SAVE_FAILED_TICKS }
    }
}

/// A toast on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub tone: Tone,
    /// The presenter tick it arrived (or was last repeated).
    pub born: u32,
    /// How many times it has said the same thing (merged).
    pub count: u16,
}

/// One of her bars with its damage lag, permille of the whole.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gauge {
    pub now: i32,
    pub max: i32,
    pub frac: u16,
    pub lag: u16,
    held_until: u32,
}

impl Gauge {
    fn set(&mut self, now: i32, max: i32, tick: u32) {
        let frac = if max <= 0 { 0 } else { (i64::from(now.clamp(0, max)) * 1000 / i64::from(max)) as u16 };
        if frac < self.frac && self.lag <= self.frac {
            // A drop: the lag starts where the bar was and holds.
            self.lag = self.frac;
        }
        if frac < self.frac {
            self.held_until = tick + LAG_HOLD;
        }
        if frac >= self.lag {
            self.lag = frac;
        } else if tick >= self.held_until {
            self.lag = self.lag.saturating_sub(LAG_FALL).max(frac);
        }
        (self.now, self.max, self.frac) = (now, max, frac);
    }
}

/// A status chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusChip {
    pub icon: SpriteId,
    pub name: jane_core::TextId,
    pub ticks_left: u32,
    pub total: u32,
    pub harmful: bool,
}

/// The target or boss frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetFrame {
    pub name: String,
    pub hp: Gauge,
    pub hostile: bool,
    pub boss: bool,
}

/// A line of the quest tracker.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestLine {
    pub quest: Option<QuestId>,
    pub title: String,
    pub step: String,
    pub ready: bool,
    /// The story's own line.
    pub main: bool,
}

/// Quests a newly given one is tracked by default while fewer than this are (the main line's
/// always is).
pub const AUTO_TRACK: usize = 5;

/// Which quests the tracker shows (PRESENTATION.md §3.2): this window's seat's choice, never the
/// sim's, kept beside the save slot (`slotN.meta.json`). A quest new to the log is tracked by
/// default, the main line's always and a side quest's while fewer than [`AUTO_TRACK`] are; one
/// untracked stays untracked; a quest gone from the log (done, set aside) is forgotten, so taken
/// again it is new again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tracking {
    /// Tracked, in the order they were.
    pub on: Vec<QuestId>,
    /// Every quest of the log this has seen, tracked or not.
    pub seen: Vec<QuestId>,
}

impl Tracking {
    pub fn is_on(&self, q: QuestId) -> bool {
        self.on.contains(&q)
    }

    /// Track it, or stop.
    pub fn toggle(&mut self, q: QuestId) {
        match self.on.iter().position(|&t| t == q) {
            Some(i) => {
                self.on.remove(i);
            }
            None => self.on.push(q),
        }
        if !self.seen.contains(&q) {
            self.seen.push(q);
        }
    }

    /// The log as it stands: new quests tracked by default, gone ones forgotten.
    pub fn sync(&mut self, log: &[QuestId]) {
        self.on.retain(|q| log.contains(q));
        self.seen.retain(|q| log.contains(q));
        let cat = jane_data::catalog();
        for &q in log {
            if self.seen.contains(&q) {
                continue;
            }
            self.seen.push(q);
            if cat.story.quest(q).main || self.on.len() < AUTO_TRACK {
                self.on.push(q);
            }
        }
    }

    /// As content ids, for the slot's note: `(tracked, seen)`.
    pub fn ids(&self) -> (Vec<String>, Vec<String>) {
        let id = |q: &QuestId| jane_data::catalog().story.quest(*q).id.to_owned();
        (self.on.iter().map(id).collect(), self.seen.iter().map(id).collect())
    }

    /// From a slot's note; an id the content no longer has is dropped.
    pub fn from_ids(on: &[String], seen: &[String]) -> Tracking {
        let q = |v: &[String]| -> Vec<QuestId> {
            v.iter().filter_map(|s| jane_data::catalog().story.quest_id(s)).collect()
        };
        Tracking { on: q(on), seen: q(seen) }
    }
}

/// What USE would do now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    pub verb: &'static str,
    pub label: String,
    /// Held to push.
    pub hold: bool,
}

/// A bar or bag slot as drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotData {
    pub item: Option<ItemId>,
    pub spell: Option<SpellId>,
    pub icon: Option<SpriteId>,
    pub count: u16,
    /// Permille of a cooldown to run, and of the GCD.
    pub cooldown: u16,
    pub gcd: u16,
    pub usable: bool,
    pub flash: u8,
    pub flash_bad: bool,
}

/// The HUD's buffer.
#[derive(Clone, Debug, Default)]
pub struct HudView {
    pub hp: Gauge,
    pub mp: Gauge,
    pub en: Gauge,
    pub statuses: Vec<StatusChip>,
    pub target: Option<TargetFrame>,
    pub zone: Option<ZoneId>,
    pub zone_name: &'static str,
    pub clock: String,
    /// Ticks since midnight, and the day (from 1).
    pub clock_ticks: u32,
    pub day: u32,
    pub night: bool,
    pub tracker: Vec<QuestLine>,
    pub prompt: Option<Prompt>,
    pub bar: [SlotData; BAR_SLOTS],
    /// The zone banner: its words and the tick it went up.
    pub banner: Option<(&'static str, u32)>,
    pub toasts: Vec<Toast>,
    pub party: u8,
}

/// The stats card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatsCard {
    pub strength: u16,
    pub spirit: u16,
    pub hp_max: i32,
    pub mp_max: i32,
    pub kills: u32,
    pub deaths: u32,
    pub casts: u32,
}

/// A spell in the book.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpellRow {
    pub id: SpellId,
    pub bound: Option<u8>,
}

/// A quest in the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestRow {
    pub id: Option<QuestId>,
    pub title: String,
    pub body: String,
    /// Each step, and whether it is done.
    pub steps: Vec<(String, bool)>,
    pub ready: bool,
    pub done: bool,
    /// The story's own line: it cannot be abandoned.
    pub main: bool,
    /// The tracker shows it.
    pub tracked: bool,
}

/// The cupboard she opened, beside her bag (`jane_sim::store`).
#[derive(Clone, Debug)]
pub struct StoreView {
    pub prop: jane_sim::ids::PropId,
    /// Its label, else its row's name ("Dresser", "Left Luggage").
    pub name: String,
    pub slots: Vec<SlotData>,
    /// Slots with something in them.
    pub used: usize,
}

/// The window's buffer.
#[derive(Clone, Debug, Default)]
pub struct WindowView {
    /// The cupboard open beside her bag, while she is within reach of it. Set by USE on one (the
    /// sim's `Store` event), dropped when the window closes ([`WindowView::close_store`]) or the
    /// sim says it is out of reach.
    pub store: Option<StoreView>,
    /// A cupboard was opened this tick: the app opens the window on it ([`WindowView::take_opened`]).
    opened: bool,
    pub bag: Vec<SlotData>,
    pub craft: [SlotData; CRAFT_INPUTS],
    pub craft_out: Option<SlotData>,
    pub at_bench: bool,
    pub stats: StatsCard,
    pub book: Vec<SpellRow>,
    pub quests: Vec<QuestRow>,
}

impl WindowView {
    /// A cupboard was opened since the last call.
    pub fn take_opened(&mut self) -> bool {
        std::mem::take(&mut self.opened)
    }

    /// The window closed: the cupboard with it.
    pub fn close_store(&mut self) {
        self.store = None;
    }
}

/// Her conversation as the box draws it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogueView {
    pub speaker: String,
    pub text: String,
    pub options: Vec<String>,
    pub choosing: bool,
    /// Another line follows this one (else the next press closes it).
    pub more: bool,
    /// Which line this is: a new key restarts the reveal.
    pub key: (u32, usize, u16, u32),
}

/// Her own state beside the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeView {
    pub dead: bool,
    pub respawn_ticks: u32,
    pub can_save: bool,
    pub at_bench: bool,
    pub frozen: bool,
    /// The story has closed (`View::the_end`).
    pub the_end: u8,
}

/// Every buffer the UI reads.
#[derive(Clone, Debug, Default)]
pub struct ViewBuffers {
    pub tick: u32,
    pub seed: u32,
    pub heroine: String,
    pub me: MeView,
    pub hud: HudView,
    pub window: WindowView,
    pub dialogue: Option<DialogueView>,
    /// The save card, while it shows.
    pub saved: Option<SavedCard>,
    /// Which quests the tracker shows (the app keeps it with the slot).
    pub track: Tracking,
    /// The log's quests this tick, for [`Tracking::sync`].
    log_ids: Vec<QuestId>,
    /// The hostile she is fighting, and the last tick a blow passed between them.
    fighting: Option<(jane_sim::UnitId, u32)>,
    scratch: String,
}

impl ViewBuffers {
    pub fn new() -> ViewBuffers {
        let mut b = ViewBuffers::default();
        b.window.bag.resize(BAG_SLOTS, SlotData::default());
        b.hud.toasts.reserve(TOASTS + 1);
        b.hud.tracker.reserve(4);
        b.hud.statuses.reserve(8);
        b
    }

    /// One presenter tick: this tick's events (hers, her zone's, the party's), then the view.
    pub fn tick(&mut self, v: &View<'_>, events: &[Event]) {
        self.tick = self.tick.wrapping_add(1);
        let now = self.tick;
        self.seed = v.seed();
        if self.heroine != v.heroine() {
            self.heroine.clear();
            self.heroine.push_str(v.heroine());
        }
        let me_unit = v.me().unit;
        for s in &mut self.hud.bar {
            s.flash = s.flash.saturating_sub(1);
        }
        for e in events_for(events, v.me()) {
            match e.kind {
                // A spell learned, a jar or a page found: the lesson's moment says it, not a toast
                // (`crate::lesson`, §3.2).
                EventKind::Toast(ToastKind::Learned(_) | ToastKind::Stronger | ToastKind::WordsStay) => {}
                EventKind::Toast(k) => {
                    let mut s = std::mem::take(&mut self.scratch);
                    s.clear();
                    let tone = text::toast(v, &k, &mut s);
                    if !s.is_empty() {
                        self.push_toast(&s, tone);
                    }
                    self.scratch = s;
                }
                EventKind::Store { prop } => {
                    self.window.store = Some(StoreView { prop, name: String::new(), slots: Vec::new(), used: 0 });
                    self.window.opened = true;
                }
                EventKind::Zone { zone, .. } => {
                    self.window.store = None;
                    self.hud.banner = Some((text::zone_name(zone, v.region()), now));
                    self.fighting = None;
                }
                EventKind::Loot { item, qty } => {
                    let cat = jane_data::catalog();
                    let mut s = std::mem::take(&mut self.scratch);
                    s.clear();
                    s.push_str(text::text(cat.combat.item(item).name));
                    if qty > 1 {
                        use std::fmt::Write as _;
                        let _ = write!(s, " x{qty}");
                    }
                    self.push_toast(&s, Tone::Good);
                    self.scratch = s;
                }
                EventKind::Cast { unit, spell, .. } if unit == me_unit => self.flash(BarSlot::Spell(spell), false),
                EventKind::CastFailed { unit, spell, why } if unit == me_unit && why.says() => {
                    self.flash(BarSlot::Spell(spell), true);
                }
                EventKind::Damage { unit, from: Some(from), .. } => {
                    if from == me_unit && v.unit(unit).is_some_and(|u| u.faction != jane_data::Faction::Friendly) {
                        self.fighting = Some((unit, now));
                    } else if unit == me_unit
                        && v.unit(from).is_some_and(|u| u.faction != jane_data::Faction::Friendly)
                        && self.fighting.is_none_or(|(_, t)| now.saturating_sub(t) > 120)
                    {
                        self.fighting = Some((from, now));
                    }
                }
                _ => {}
            }
        }
        // The save card goes; toasts age out; the banner comes down.
        if self.saved.as_ref().is_some_and(|c| now.wrapping_sub(c.born) >= c.ticks()) {
            self.saved = None;
        }
        self.hud.toasts.retain(|t| now.wrapping_sub(t.born) < TOAST_TICKS);
        if self.hud.banner.is_some_and(|(_, t)| now.wrapping_sub(t) >= BANNER_TICKS) {
            self.hud.banner = None;
        }
        self.read(v);
    }

    /// Puts the save card up: `ok`, the world written down; else what went wrong.
    pub fn saved(&mut self, text: &str, ok: bool) {
        self.saved = Some(SavedCard { text: text.to_owned(), ok, born: self.tick });
    }

    /// Puts a toast up, or bumps the one already saying it.
    pub fn push_toast(&mut self, s: &str, tone: Tone) {
        let now = self.tick;
        if let Some(t) = self.hud.toasts.iter_mut().find(|t| t.text == s) {
            t.born = now;
            t.count = t.count.saturating_add(1);
            return;
        }
        if self.hud.toasts.len() >= TOASTS {
            self.hud.toasts.remove(0);
        }
        self.hud.toasts.push(Toast { text: s.to_owned(), tone, born: now, count: 1 });
    }

    fn flash(&mut self, what: BarSlot, bad: bool) {
        for s in &mut self.hud.bar {
            let hit = match what {
                BarSlot::Spell(id) => s.spell == Some(id),
                BarSlot::Item(id) => s.item == Some(id),
            };
            if hit {
                s.flash = FLASH_TICKS;
                s.flash_bad = bad;
            }
        }
    }

    fn read(&mut self, v: &View<'_>) {
        let cat = jane_data::catalog();
        let now = self.tick;
        let me = v.me();
        let body = v.body();
        let t = v.tick().0;
        let (heroine, seed) = (v.heroine(), v.seed());

        // Her.
        self.me = MeView {
            dead: !body.alive,
            respawn_ticks: me.respawn_at.map_or(0, |r| r.0.saturating_sub(t)),
            can_save: v.near_rest(),
            at_bench: v.near_bench(),
            frozen: v.frozen(),
            the_end: v.the_end(),
        };
        let h = &mut self.hud;
        h.hp.set(body.hp.points(), max_hp(body).points(), now);
        h.mp.set(body.mp.points(), max_mp(body).points(), now);
        h.en.set(body.energy.points(), ENERGY_MAX.points(), now);
        h.statuses.clear();
        for s in body.statuses.iter().take(8) {
            let d = cat.combat.effect(s.effect);
            h.statuses.push(StatusChip {
                icon: d.icon,
                name: d.name,
                ticks_left: s.until.0.saturating_sub(t),
                total: d.duration.0.max(1),
                harmful: d.harmful,
            });
        }
        // The zone, the clock and the sky.
        h.zone = Some(v.zone());
        h.zone_name = text::zone_name(v.zone(), v.region());
        let (clock, day) = v.clock();
        h.clock_ticks = clock;
        h.day = day + 1;
        h.night = !(6 * 7200 + 3600..18 * 7200 + 3600).contains(&clock);
        h.clock.clear();
        text::clock(clock, &mut h.clock);
        h.party = v.party();

        // The target: the hostile she is fighting, while it lives and is near.
        h.target = None;
        if let Some((id, when)) = self.fighting {
            match v.unit(id) {
                Some(u) if u.alive && now.saturating_sub(when) < TARGET_TICKS => {
                    let d = cat.combat.unit(u.def);
                    let mut tf = h.target.take().unwrap_or_default();
                    tf.name.clear();
                    text::expand(text::text(d.name), heroine, seed, &mut tf.name);
                    tf.hp.set(u.hp.points(), max_hp(u).points(), now);
                    tf.hostile = u.faction != jane_data::Faction::Friendly;
                    tf.boss = max_hp(u).points() >= 150;
                    h.target = Some(tf);
                }
                _ => self.fighting = None,
            }
        }

        // The tracker: the quests this seat tracks, the main line first, each at its first
        // unfinished step.
        self.log_ids.clear();
        self.log_ids.extend(v.quests().map(|q| q.quest));
        self.track.sync(&self.log_ids);
        h.tracker.clear();
        let track = &self.track;
        let main_first = v
            .quests()
            .filter(|q| cat.story.quest(q.quest).main)
            .chain(v.quests().filter(|q| !cat.story.quest(q.quest).main))
            .filter(|q| track.is_on(q.quest));
        for q in main_first {
            let d = cat.story.quest(q.quest);
            let mut line = QuestLine { quest: Some(q.quest), ready: q.ready, main: d.main, ..QuestLine::default() };
            text::expand(text::text(d.name), heroine, seed, &mut line.title);
            if q.ready {
                line.step.push_str("Back to ");
                text::expand(text::text(d.return_to), heroine, seed, &mut line.step);
            } else if let Some((i, r)) = d.requirements.iter().enumerate().find(|(i, r)| q.count(*i) < r.qty) {
                text::expand(text::text(r.text), heroine, seed, &mut line.step);
                if r.qty > 1 {
                    use std::fmt::Write as _;
                    let _ = write!(line.step, " {} of {}", q.count(i), r.qty);
                }
            }
            h.tracker.push(line);
        }

        // The prompt.
        h.prompt = v.focus().map(|f| {
            let label = match f.target {
                FocusRef::Prop(pid) => v.prop(pid).map_or("", |p| {
                    let d = cat.story.prop(p.def);
                    v.prop_spawn(p).and_then(|s| s.label).map_or(text::text(d.name), |l| v.text(l))
                }),
                FocusRef::Unit(uid) => v.unit(uid).map_or("", |u| text::text(cat.combat.unit(u.def).name)),
                FocusRef::Drop(did) => {
                    v.drops().iter().find(|d| d.id == did).map_or("", |d| text::text(cat.combat.item(d.item).name))
                }
            };
            let label = match f.verb {
                Verb::Take(item) => text::text(cat.combat.item(item).name),
                _ => label,
            };
            let mut p = Prompt { verb: text::verb(f.verb), label: String::new(), hold: f.pushes };
            text::expand(label, heroine, seed, &mut p.label);
            p
        });

        // The bar.
        let gcd_left = body.gcd_until.0.saturating_sub(t);
        for (i, slot) in me.bar.iter().enumerate() {
            let (flash, flash_bad) = (h.bar[i].flash, h.bar[i].flash_bad);
            h.bar[i] = SlotData { flash, flash_bad, ..SlotData::default() };
            let s = &mut h.bar[i];
            match *slot {
                Some(BarSlot::Spell(id)) => {
                    let d = cat.combat.spell(id);
                    s.spell = Some(id);
                    s.icon = Some(d.icon);
                    let until = body.cooldowns.iter().find(|c| c.0 == id).map_or(0, |c| c.1.0);
                    s.cooldown = permille(until.saturating_sub(t), d.cooldown.0);
                    if !d.gcd_immune {
                        s.gcd = permille(gcd_left, GCD.0);
                    }
                    s.usable = body.mp >= d.mp && body.energy >= d.energy;
                }
                Some(BarSlot::Item(id)) => {
                    let d = cat.combat.item(id);
                    s.item = Some(id);
                    s.icon = Some(d.icon);
                    s.count = me.bag.iter().flatten().filter(|st| st.item == id).map(|st| st.qty).sum();
                    let until = body.item_cooldowns.iter().find(|c| c.0 == id).map_or(0, |c| c.1.0);
                    s.cooldown = permille(until.saturating_sub(t), d.cooldown.0);
                    s.usable = s.count > 0;
                }
                None => {}
            }
        }

        // The window.
        let w = &mut self.window;
        w.bag.resize(BAG_SLOTS, SlotData::default());
        for (i, st) in me.bag.iter().enumerate() {
            w.bag[i] = st.map_or(SlotData::default(), |st| stack(st.item, st.qty));
        }
        for (i, st) in me.craft.iter().enumerate() {
            w.craft[i] = st.map_or(SlotData::default(), |st| stack(st.item, st.qty));
        }
        w.at_bench = self.me.at_bench;
        // The cupboard, read through the sim's reach (`View::store`): out of reach, it is gone.
        let open = w.store.as_ref().map(|s| s.prop);
        match open.and_then(|p| v.store(p).map(|slots| (p, slots))) {
            Some((p, slots)) => {
                let st = w.store.as_mut().expect("open");
                if st.name.is_empty() {
                    if let Some(prop) = v.prop(p) {
                        match v.prop_spawn(prop).and_then(|s| s.label) {
                            Some(l) => st.name.push_str(v.text(l)),
                            None => st.name.push_str(text::text(cat.story.prop(prop.def).name)),
                        }
                    }
                }
                st.slots.resize(slots.len(), SlotData::default());
                for (d, s) in st.slots.iter_mut().zip(slots.iter()) {
                    *d = s.map_or(SlotData::default(), |s| stack(s.item, s.qty));
                }
                st.used = slots.iter().flatten().count();
            }
            None => w.store = None,
        }
        w.craft_out = v.craft_output().map(|(item, qty)| stack(item, qty));
        w.stats = StatsCard {
            strength: body.strength,
            spirit: body.spirit,
            hp_max: max_hp(body).points(),
            mp_max: max_mp(body).points(),
            kills: me.stats.kills,
            deaths: me.stats.deaths,
            casts: me.stats.casts,
        };
        w.book.clear();
        // What she casts: her own row's book, then what the party has learned.
        let own = cat.combat.unit(body.def).book;
        for &id in own.iter().chain(v.learned().iter().filter(|s| !own.contains(s))) {
            let bound = me.bar.iter().position(|b| *b == Some(BarSlot::Spell(id))).map(|i| i as u8);
            w.book.push(SpellRow { id, bound });
        }
        // The log: active quests with their steps, then the done ones.
        let mut n = 0;
        for q in v.quests() {
            let d = cat.story.quest(q.quest);
            let row = row_at(&mut w.quests, n);
            n += 1;
            row.id = Some(q.quest);
            row.ready = q.ready;
            row.done = false;
            row.main = d.main;
            row.tracked = self.track.is_on(q.quest);
            text::expand(text::text(d.name), heroine, seed, &mut row.title);
            text::expand(text::text(d.description), heroine, seed, &mut row.body);
            row.steps.clear();
            for (i, r) in d.requirements.iter().enumerate() {
                let mut s = text::expanded(text::text(r.text), heroine, seed);
                if r.qty > 1 {
                    use std::fmt::Write as _;
                    let _ = write!(s, " {} of {}", q.count(i), r.qty);
                }
                row.steps.push((s, q.count(i) >= r.qty));
            }
            if q.ready {
                let mut s = String::from("Back to ");
                text::expand(text::text(d.return_to), heroine, seed, &mut s);
                row.steps.push((s, false));
            }
        }
        for &q in v.quests_done().iter().rev() {
            let d = cat.story.quest(q);
            let row = row_at(&mut w.quests, n);
            n += 1;
            row.id = Some(q);
            row.ready = false;
            row.done = true;
            row.main = d.main;
            row.tracked = false;
            text::expand(text::text(d.name), heroine, seed, &mut row.title);
            text::expand(text::text(d.completion), heroine, seed, &mut row.body);
            row.steps.clear();
        }
        w.quests.truncate(n);

        // Her conversation.
        self.dialogue = v.dialogue().map(|d| {
            let mut out = self.dialogue.take().unwrap_or_default();
            out.speaker.clear();
            out.text.clear();
            out.options.clear();
            let tree_speaker = d.tree.map(|t| text::text(cat.story.dialogue(t).speaker)).filter(|s| !s.is_empty());
            let speaker = match (tree_speaker, d.speaker) {
                (Some(s), _) => s,
                (None, Speaker::Unit(u)) => v.unit(u).map_or("", |u| text::text(cat.combat.unit(u.def).name)),
                (None, Speaker::Prop(p)) => v.prop(p).map_or("", |p| {
                    let def = cat.story.prop(p.def);
                    v.prop_spawn(p).and_then(|s| s.label).map_or(text::text(def.name), |l| v.text(l))
                }),
                (None, Speaker::None) => "",
            };
            text::expand(speaker, heroine, seed, &mut out.speaker);
            let (line_text, lines) = match (d.node, d.read) {
                (_, Some(r)) => (v.text(r), 1),
                (Some(n), None) => (n.lines.get(usize::from(d.line)).map_or("", |l| text::text(l.text)), n.lines.len()),
                (None, None) => ("", 1),
            };
            text::expand(line_text, heroine, seed, &mut out.text);
            out.choosing = d.awaiting_choice;
            if d.awaiting_choice
                && let Some(n) = d.node
            {
                for o in n.options.iter().take(2) {
                    out.options.push(text::expanded(text::text(o.label), heroine, seed));
                }
            }
            out.more =
                !d.awaiting_choice && (usize::from(d.line) + 1 < lines || d.node.is_some_and(|n| n.goto.is_some()));
            let tree = d.tree.map_or(u32::MAX, |t| u32::from(t.0));
            let node = d.node.map_or(usize::MAX, |n| std::ptr::from_ref(n) as usize);
            let read = match d.read {
                Some(jane_core::TextRef::Text(t)) => u32::from(t.0),
                Some(jane_core::TextRef::Local(l)) => 0x8000_0000 | u32::from(l),
                None => u32::MAX,
            };
            out.key = (tree, node, d.line, read);
            out
        });
    }
}

fn row_at(v: &mut Vec<QuestRow>, i: usize) -> &mut QuestRow {
    if v.len() <= i {
        v.push(QuestRow::default());
    }
    let r = &mut v[i];
    r.title.clear();
    r.body.clear();
    &mut v[i]
}

fn stack(item: ItemId, qty: u16) -> SlotData {
    let d = jane_data::catalog().combat.item(item);
    SlotData { item: Some(item), icon: Some(d.icon), count: qty, usable: d.usable, ..SlotData::default() }
}

/// `left` of `total` as permille, rounded up so a sweep never shows done early.
fn permille(left: u32, total: u32) -> u16 {
    if left == 0 || total == 0 {
        return 0;
    }
    (u64::from(left.min(total)) * 1000).div_ceil(u64::from(total)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lag_holds_then_falls_to_the_bar() {
        let mut g = Gauge::default();
        g.set(100, 100, 0);
        assert_eq!((g.frac, g.lag), (1000, 1000));
        g.set(40, 100, 1);
        assert_eq!((g.frac, g.lag), (400, 1000), "the tail starts where the bar was");
        for t in 2..=LAG_HOLD {
            g.set(40, 100, t);
        }
        assert_eq!(g.lag, 1000, "held");
        for t in LAG_HOLD + 1..LAG_HOLD + 200 {
            g.set(40, 100, t);
        }
        assert_eq!(g.lag, 400, "fallen to the bar and no further");
        g.set(90, 100, 500);
        assert_eq!((g.frac, g.lag), (900, 900), "a heal takes the tail with it");
    }

    #[test]
    fn toasts_merge_and_push_the_oldest_out() {
        let mut b = ViewBuffers::new();
        b.push_toast("Too tired", Tone::Refused);
        b.push_toast("Too tired", Tone::Refused);
        assert_eq!(b.hud.toasts.len(), 1);
        assert_eq!(b.hud.toasts[0].count, 2);
        for s in ["a", "b", "c"] {
            b.push_toast(s, Tone::Plain);
        }
        let texts: Vec<&str> = b.hud.toasts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["a", "b", "c"]);
    }

    #[test]
    fn a_cooldown_rounds_up() {
        assert_eq!(permille(0, 90), 0);
        assert_eq!(permille(1, 90000), 1);
        assert_eq!(permille(45, 90), 500);
        assert_eq!(permille(200, 90), 1000);
    }

    #[test]
    fn the_buffers_read_a_new_game() {
        let sim = jane_sim::Sim::new_game(7, "Tess");
        let v = sim.view(jane_sim::Seat(0)).expect("seat 0 plays");
        let mut b = ViewBuffers::new();
        b.tick(&v, &[]);
        assert_eq!(b.heroine, "Tess");
        assert!(b.hud.hp.max > 0 && b.hud.hp.frac == 1000);
        assert_eq!(b.window.bag.len(), BAG_SLOTS);
        assert!(!b.hud.clock.is_empty());
        assert!(b.hud.day >= 1);
        assert!(b.hud.zone_name.starts_with("The") || b.hud.zone_name.contains("House"));
    }

    #[test]
    fn tracking_defaults_toggles_and_forgets() {
        let cat = jane_data::catalog();
        let main = cat.story.quest_id("the_letter").unwrap();
        let side: Vec<QuestId> = cat
            .story
            .quests
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.main)
            .map(|(i, _)| QuestId(i as u16))
            .take(7)
            .collect();
        let mut t = Tracking::default();
        // New Game: the letter, tracked.
        t.sync(&[main]);
        assert!(t.is_on(main));
        // Side quests tracked as they come while fewer than five are; past that, not.
        let mut log = vec![main];
        log.extend(&side);
        t.sync(&log);
        assert_eq!(t.on.len(), AUTO_TRACK, "{:?}", t.on);
        assert!(!t.is_on(side[5]) && !t.is_on(side[6]));
        // Untracked stays untracked; tracked by hand stays tracked.
        t.toggle(side[0]);
        t.toggle(side[6]);
        t.sync(&log);
        assert!(!t.is_on(side[0]) && t.is_on(side[6]));
        // The main line is tracked even with the cap full.
        let mut u = Tracking::default();
        u.sync(&side[..5]);
        u.sync(&[&side[..5], &[main][..]].concat());
        assert!(u.is_on(main) && u.on.len() == 6);
        // Gone from the log (set aside), forgotten: taken again, it is new again.
        let without: Vec<QuestId> = log.iter().copied().filter(|&q| q != side[0]).collect();
        t.sync(&without);
        assert!(!t.seen.contains(&side[0]));
        t.toggle(side[1]);
        t.sync(&log);
        assert!(t.is_on(side[0]), "taken again, tracked by default");
        // Through content ids and back, as the slot's note keeps it.
        let (on, seen) = t.ids();
        assert_eq!(Tracking::from_ids(&on, &seen), t);
        assert_eq!(Tracking::from_ids(&["no_such_quest".into()], &[]), Tracking::default());
    }
}
