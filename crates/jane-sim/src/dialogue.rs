//! Conversations (`sim/dialogue.ts`). A tree is data: start rules (the first whose conditions
//! hold picks the node) and nodes of lines and up to two options, like 2020's
//! `dialogue_add_option_2`.
//!
//! A conversation belongs to one seat (`PlayerState.dialogue`). Alone, the world holds still
//! while she talks (`Sim::frozen`, ARCHITECTURE.md §4.1); with company nothing pauses, and she
//! stands reading while the county carries on. Every transition is one of her commands
//! (`Use`/`Advance`, `Choose`, `CloseDialogue`), so a replay is exact.
//!
//! A line's `tells` are written into the journal as the line is shown (§3.7).

use jane_core::action::FactKey as ContentFact;
use jane_core::{DialogueId, ListRef, TextRef};
use jane_data::{DialogueLine, DialogueNode};

use crate::actions::{Subject, conditions_met, conds, run_actions};
use crate::ctx::Ctx;
use crate::event::EventKind;
use crate::journal;
use crate::state::{Dialogue, FactKey, PlayerState, Source, Speaker};

/// The node a conversation stands at, or the one line of a thing read.
#[derive(Clone, Copy, Debug)]
pub enum Current {
    Node(&'static DialogueNode),
    Read(TextRef),
}

impl Current {
    pub fn lines(self) -> usize {
        match self {
            Current::Node(n) => n.lines.len(),
            Current::Read(_) => 1,
        }
    }

    pub fn options(self) -> usize {
        match self {
            Current::Node(n) => n.options.len(),
            Current::Read(_) => 0,
        }
    }
}

pub fn current(d: &Dialogue) -> Option<Current> {
    if let Some(t) = d.read {
        return Some(Current::Read(t));
    }
    let tree = jane_data::catalog().story.dialogue(d.tree?);
    tree.nodes.get(usize::from(d.node)).map(Current::Node)
}

/// On the node's last line, and it offers a choice.
pub fn awaiting_choice(p: &PlayerState) -> bool {
    let Some(d) = &p.dialogue else { return false };
    current(d).is_some_and(|c| usize::from(d.line) + 1 >= c.lines() && c.options() > 0)
}

/// The facts a line tells, into the journal.
fn tell(cx: &mut Ctx<'_>, line: &DialogueLine) {
    for &f in line.tells {
        let (fact, how) = match f {
            ContentFact::Place(k) => (FactKey::Place(cx.sym(k)), Source::Named),
            ContentFact::Person(k) => (FactKey::Person(cx.sym(k)), Source::Talked),
            ContentFact::Thing(t) => (FactKey::Thing(t), Source::Seen),
            ContentFact::Claim(t) => (FactKey::Claim(t), Source::Told),
            ContentFact::Route(a, b) => (FactKey::Route(cx.sym(a), cx.sym(b)), Source::Told),
            ContentFact::Danger(k) => (FactKey::Danger(cx.sym(k)), Source::Told),
            ContentFact::Rumour(s) => (FactKey::Rumour(s), Source::Heard),
        };
        journal::learn(cx, fact, how);
    }
}

/// The line she is looking at now has been shown: write what it tells.
fn shown(cx: &mut Ctx<'_>) {
    let Some(seat) = cx.actor else { return };
    let Some(d) = cx.world.player(seat).and_then(|p| p.dialogue) else { return };
    if let Some(Current::Node(n)) = current(&d) {
        if let Some(line) = n.lines.get(usize::from(d.line)) {
            tell(cx, line);
        }
    }
}

/// Open `tree` for the actor at the first start row whose conditions hold.
pub fn start(cx: &mut Ctx<'_>, tree: DialogueId, speaker: Speaker) -> bool {
    let Some(seat) = cx.actor else { return false };
    let t = cx.cat.story.dialogue(tree);
    // The start rules are asked with the conversation already open, so a rule can ask what the
    // speaker has heard (`SpeakerKnows`); none holding, it closes again.
    let before = cx.world.players[seat.index()].dialogue.replace(Dialogue {
        tree: Some(tree),
        node: 0,
        line: 0,
        speaker,
        read: None,
    });
    let Some(entry) = t.start.iter().find(|s| s.when.is_none_or(|w| conditions_met(cx, conds(cx, w)))) else {
        cx.world.players[seat.index()].dialogue = before;
        return false;
    };
    let node = entry.node;
    cx.world.players[seat.index()].dialogue = Some(Dialogue { tree: Some(tree), node, line: 0, speaker, read: None });
    cx.emit(EventKind::Dialogue);
    // Someone with a name, spoken to: met.
    if let Speaker::Unit(id) = speaker {
        if let Some(key) = cx.zone.unit(id).and_then(|u| u.key) {
            journal::learn(cx, FactKey::Person(key), Source::Met);
        }
    }
    shown(cx);
    true
}

/// Words on a thing, in the reading box: one line, no tree.
pub fn read(cx: &mut Ctx<'_>, text: TextRef) {
    let Some(seat) = cx.actor else { return };
    cx.world.players[seat.index()].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::None, read: Some(text) });
    cx.emit(EventKind::Dialogue);
}

/// The next line; on the last line, leave the node (a node with options waits for `Choose`).
pub fn advance(cx: &mut Ctx<'_>) {
    let Some(seat) = cx.actor else { return };
    let Some(d) = cx.world.player(seat).and_then(|p| p.dialogue) else { return };
    let Some(cur) = current(&d) else { return };
    if usize::from(d.line) + 1 < cur.lines() {
        if let Some(p) = cx.world.players[seat.index()].dialogue.as_mut() {
            p.line += 1;
        }
        cx.emit(EventKind::Dialogue);
        shown(cx);
        return;
    }
    if cur.options() > 0 {
        return;
    }
    match cur {
        Current::Node(n) => leave_node(cx, n.actions, None, n.goto),
        Current::Read(_) => leave_node(cx, None, None, None),
    }
}

/// Take option `index` on a node's last line: the node's actions, then the option's, and on to
/// the option's `goto`, else the node's, else closed.
pub fn choose(cx: &mut Ctx<'_>, index: u8) {
    let Some(seat) = cx.actor else { return };
    let p = &cx.world.players[seat.index()];
    if !awaiting_choice(p) {
        return;
    }
    let Some(Current::Node(n)) = p.dialogue.as_ref().and_then(current) else { return };
    let Some(o) = n.options.get(usize::from(index)) else { return };
    leave_node(cx, n.actions, o.actions, o.goto.or(n.goto));
}

fn leave_node(cx: &mut Ctx<'_>, first: Option<ListRef>, then: Option<ListRef>, goto: Option<u16>) {
    let Some(seat) = cx.actor else { return };
    let p = &mut cx.world.players[seat.index()];
    let Some(d) = p.dialogue.as_mut() else { return };
    match goto {
        Some(g) => {
            d.node = g;
            d.line = 0;
        }
        None => p.dialogue = None,
    }
    // The actions run after the transition, so a `Talk` or `Travel` in them wins over the close.
    // Their subject is her, not the speaker.
    let body = cx.actor_unit().map_or(Subject::None, Subject::Unit);
    for list in [first, then].into_iter().flatten() {
        run_actions(cx, list, body);
    }
    cx.emit(EventKind::Dialogue);
    if goto.is_some() {
        shown(cx);
    }
}

/// Close whatever she is reading or saying.
pub fn close(cx: &mut Ctx<'_>) {
    let Some(seat) = cx.actor else { return };
    if cx.world.players[seat.index()].dialogue.take().is_some() {
        cx.emit(EventKind::Dialogue);
    }
}
