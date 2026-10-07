//! Quests (`sim/quests.ts`): active, ready, done. Three requirement kinds, no more: kill,
//! acquire, location.
//!
//! - `Acquire` is read live from the party's bags, so it cannot drift from what they hold.
//! - `Location` reads the world's `Been` flag, so a place visited before the quest was taken
//!   still counts (a once-only trigger would otherwise lose it for ever).
//! - A kill counts only for quests already in the log (2020's `quest_unit` hook), whoever made it.
//! - Quest news goes to the whole party, wherever they stand.
//! - Rewards are an action list, paid once per connected seat with her as the actor
//!   (`WorldOp::PayRewards`, drained after the zone is put back).
//! - **Abandoning** ([`abandon`], `Command::Abandon`): a side quest leaves the log and is
//!   offerable again, so its giver shows "?" once more. Any seat may, for the whole party (the
//!   log is the party's, as taking a quest is anyone's); everyone hears whose coat it was. The
//!   main line (`QuestDef::main`) refuses. What it had counted of kills is kept
//!   (`Quests::set_aside`: the dead stay dead) and counts again when it is taken again; places
//!   been stay been, as a place visited before a quest is asked always counted. What the giver
//!   handed over with it is taken back from the party's bags (he hands it over again); what only
//!   this quest asks her to bring (no other quest asks for it, nothing uses it, no recipe takes
//!   it, it opens nothing and is not bound) is set down at her feet, where a story thing never
//!   ages out. Everything else stays in the bags. A flag set while it was under way stays set:
//!   every flag is set only by doing, and the softlock search (`tests/abandon.rs`) proves each
//!   side quest is offered again after it, and the story still ends.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use jane_core::{Action, ItemId, QuestId, Stack};
use jane_data::ReqTarget;
use once_cell::race::OnceBox;

use crate::ctx::{Ctx, WorldOp};
use crate::event::{Event, EventKind, QuestChange, ToastKind};
use crate::state::{FlagKey, GameState, QuestProgress};
use crate::sym::of_name;
use crate::under::uncover_all;

pub fn active(state: &GameState, q: QuestId) -> Option<&QuestProgress> {
    state.quests.active.iter().find(|p| p.quest == q)
}

pub fn done(state: &GameState, q: QuestId) -> bool {
    state.quests.done.contains(&q)
}

/// Progress on requirement `i` of an active quest, clamped to what it asks for.
pub fn requirement_count(state: &GameState, prog: &QuestProgress, i: usize) -> u16 {
    let r = &jane_data::catalog().story.quest(prog.quest).requirements[i];
    let n: u32 = match r.target {
        ReqTarget::Kill(_) => u32::from(prog.counts.get(i).copied().unwrap_or(0)),
        // One story, one log: what the party holds between them counts.
        ReqTarget::Acquire(item) => state.connected().map(|p| crate::bag::bag_count(&p.bag[..], item)).sum(),
        ReqTarget::Location(n) => u32::from(state.flags.get(&FlagKey::Been(of_name(n))).is_some_and(|&v| v != 0)),
    };
    n.min(u32::from(r.qty)) as u16
}

/// Every requirement met.
pub fn ready(state: &GameState, q: QuestId) -> bool {
    let Some(prog) = active(state, q) else { return false };
    let def = jane_data::catalog().story.quest(q);
    (0..def.requirements.len()).all(|i| requirement_count(state, prog, i) >= def.requirements[i].qty)
}

/// Give a quest. Nothing happens if it is in the log or done.
pub fn give(cx: &mut Ctx<'_>, q: QuestId) -> bool {
    if active(cx.world, q).is_some() || done(cx.world, q) {
        return false;
    }
    let n = cx.cat.story.quest(q).requirements.len();
    // Taken again after it was set aside: the kills it had counted still count.
    let mut counts = match cx.world.quests.set_aside.iter().position(|p| p.quest == q) {
        Some(i) => cx.world.quests.set_aside.remove(i).counts,
        None => Vec::new(),
    };
    counts.resize(n, 0);
    cx.world.quests.active.push(QuestProgress { quest: q, counts });
    cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Given });
    cx.emit_all(EventKind::Toast(ToastKind::QuestGiven(q)));
    if ready(cx.world, q) {
        cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Ready });
    }
    // A stone pushed off the spot before she was told what is under it: it is there now.
    uncover_all(cx);
    true
}

/// A unit of `def` died: one more for every active quest that counts it.
pub fn on_kill(cx: &mut Ctx<'_>, def: jane_core::UnitDefId) {
    let cat = cx.cat;
    for qi in 0..cx.world.quests.active.len() {
        let q = cx.world.quests.active[qi].quest;
        let reqs = cat.story.quest(q).requirements;
        let was_ready = ready(cx.world, q);
        let mut touched = false;
        for (i, r) in reqs.iter().enumerate() {
            if r.target != ReqTarget::Kill(def) {
                continue;
            }
            let prog = &mut cx.world.quests.active[qi];
            let c = prog.counts[i];
            if c >= r.qty {
                continue;
            }
            prog.counts[i] = c + 1;
            touched = true;
            cx.emit_all(EventKind::Toast(ToastKind::KillProgress { quest: q, req: i as u8, n: c + 1, of: r.qty }));
        }
        if touched {
            cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Progress });
            if !was_ready && ready(cx.world, q) {
                cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Ready });
            }
        }
    }
}

/// `Location`: she has been here. The first time, every quest that asks for it hears.
pub fn on_location(cx: &mut Ctx<'_>, name: jane_core::Sym) {
    let k = FlagKey::Been(name);
    if cx.world.flags.get(&k).is_some_and(|&v| v != 0) {
        return;
    }
    cx.world.flags.insert(k, 1);
    let cat = cx.cat;
    for qi in 0..cx.world.quests.active.len() {
        let q = cx.world.quests.active[qi].quest;
        let asks = cat
            .story
            .quest(q)
            .requirements
            .iter()
            .any(|r| matches!(r.target, ReqTarget::Location(n) if of_name(n) == name));
        if asks {
            let change = if ready(cx.world, q) { QuestChange::Ready } else { QuestChange::Progress };
            cx.emit_all(EventKind::Quest { quest: q, change });
        }
    }
}

/// Hand in: only when every requirement is met. Everyone in the party is paid, wherever she
/// stands, once each (`WorldOp::PayRewards`).
pub fn hand_in(cx: &mut Ctx<'_>, q: QuestId) -> bool {
    if !ready(cx.world, q) {
        return false;
    }
    cx.world.quests.active.retain(|p| p.quest != q);
    cx.world.quests.done.push(q);
    cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Done });
    cx.emit_all(EventKind::Toast(ToastKind::QuestDone(q)));
    let list = cx.cat.story.quest(q).rewards;
    cx.wops.ops.push(WorldOp::PayRewards { quest: q, list });
    true
}

/// Why a quest stays in the log when asked to go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// The story's own line (`QuestDef::main`).
    Main,
    /// Not in the log (done, or never taken).
    NotInLog,
}

/// Whether the party may set `q` aside now.
pub fn can_abandon(state: &GameState, q: QuestId) -> Result<(), Keep> {
    if jane_data::catalog().story.quest(q).main {
        return Err(Keep::Main);
    }
    if active(state, q).is_none() {
        return Err(Keep::NotInLog);
    }
    Ok(())
}

/// What abandoning `q` does with things: what its giver hands over with it, and what only it
/// asks her to bring. Worked out once from the catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestThings {
    /// A `give` in the same list as the `quest` that gives it: taken back, handed again.
    pub handed: Vec<Stack>,
    /// Set down at her feet.
    pub only: Vec<ItemId>,
}

/// [`QuestThings`] for `q`.
pub fn things(q: QuestId) -> &'static QuestThings {
    static ALL: OnceBox<Vec<QuestThings>> = OnceBox::new();
    let all = ALL.get_or_init(|| {
        let cat = jane_data::catalog();
        let quests = cat.story.quests;
        let mut out: Vec<QuestThings> = vec![QuestThings::default(); quests.len()];
        // A reward list that gives the next quest pays for the last one: what it gives is not
        // handed over with the next.
        let rewards: Vec<jane_core::ListRef> = quests.iter().map(|d| d.rewards).collect();
        for (i, list) in cat.lists.iter().enumerate() {
            if rewards.contains(&jane_core::ListRef::Catalog(i as u16)) {
                continue;
            }
            for a in *list {
                if let Action::Quest(given) = *a {
                    for b in *list {
                        if let Action::Give(st) = *b {
                            let h = &mut out[given.index()].handed;
                            match h.iter_mut().find(|s| s.item == st.item) {
                                Some(s) => s.qty = s.qty.max(st.qty),
                                None => h.push(st),
                            }
                        }
                    }
                }
            }
        }
        let asked = |item: ItemId| {
            quests.iter().filter(|d| d.requirements.iter().any(|r| r.target == ReqTarget::Acquire(item))).count()
        };
        for (i, d) in quests.iter().enumerate() {
            for r in d.requirements {
                let ReqTarget::Acquire(item) = r.target else { continue };
                let def = cat.combat.item(item);
                let plain = !def.usable && !def.keep && !def.bound && def.opens.is_none();
                let cooked = cat.combat.recipes.iter().any(|rc| rc.inputs.contains(&item) || rc.output == item);
                let t = &mut out[i];
                if plain
                    && !cooked
                    && asked(item) == 1
                    && !t.handed.iter().any(|s| s.item == item)
                    && !t.only.contains(&item)
                {
                    t.only.push(item);
                }
            }
        }
        Box::new(out)
    });
    &all[q.index()]
}

/// Up to `qty` of `item` out of the party's bags, the acting seat's first; returns how many.
fn take_from_party(cx: &mut Ctx<'_>, item: ItemId, qty: u16) -> u16 {
    let first = cx.actor.map_or(0, crate::ids::Seat::index);
    let n = cx.world.players.len();
    let mut left = qty;
    for k in 0..n {
        let i = (first + k) % n;
        let p = &mut cx.world.players[i];
        if left == 0 || !p.connected {
            continue;
        }
        let got = crate::bag::bag_remove(&mut p.bag[..], item, left);
        if got > 0 {
            crate::bag::ring_settle(&mut p.bag[..]);
            left -= got;
            cx.events.push(Event { to: Some(crate::ids::Seat(i as u8)), in_zone: None, kind: EventKind::Bag });
        }
    }
    qty - left
}

/// Set a side quest aside, for the whole party (the module's notes say what happens to its
/// count and its things). The main line refuses with a word to whoever asked.
pub fn abandon(cx: &mut Ctx<'_>, q: QuestId) -> bool {
    let Some(seat) = cx.actor else { return false };
    match can_abandon(cx.world, q) {
        Ok(()) => {}
        Err(Keep::Main) => {
            cx.events.push(Event { to: Some(seat), in_zone: None, kind: EventKind::Toast(ToastKind::StoryOwn) });
            return false;
        }
        Err(Keep::NotInLog) => return false,
    }
    let Some(i) = cx.world.quests.active.iter().position(|p| p.quest == q) else { return false };
    let prog = cx.world.quests.active.remove(i);
    cx.world.quests.set_aside.retain(|p| p.quest != q);
    if prog.counts.iter().any(|&c| c > 0) {
        cx.world.quests.set_aside.push(prog);
    }
    let t = things(q);
    for s in &t.handed {
        take_from_party(cx, s.item, s.qty);
    }
    let at = cx.actor_unit().and_then(|b| cx.zone.unit(b)).map(|u| u.pos);
    for &item in &t.only {
        let n = take_from_party(cx, item, u16::MAX);
        if n > 0
            && let Some(pos) = at
        {
            crate::inventory::spawn_drop(cx, item, n, pos);
        }
    }
    cx.emit_all(EventKind::Quest { quest: q, change: QuestChange::Abandoned });
    cx.emit_all(EventKind::Toast(ToastKind::QuestAbandoned { quest: q, by: seat }));
    true
}
