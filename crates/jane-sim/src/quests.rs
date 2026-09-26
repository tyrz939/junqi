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

use jane_core::QuestId;
use jane_data::ReqTarget;

use crate::ctx::{Ctx, WorldOp};
use crate::event::{EventKind, QuestChange, ToastKind};
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
    cx.world.quests.active.push(QuestProgress { quest: q, counts: vec![0; n] });
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
