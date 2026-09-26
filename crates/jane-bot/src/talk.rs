//! A conversation (`bot.ts talkThrough`): read each line, and at a choice take the line that
//! moves the story (a quest given or handed in; something taught, grown or given; a rest that is
//! a moment, not the night), else the one that ends the talk, else the first. Both models answer
//! alike: a question that only leads to more talk is left unasked.

use jane_core::action::Action;
use jane_sim::{Command, View};

use crate::Act;
use crate::sense::list_has;
use crate::task::Ctx;

/// Lines advanced in a conversation before giving up on it.
pub const MAX_LINES: u32 = 80;

#[derive(Debug, Default)]
pub struct Talk {
    /// Lines advanced in the current conversation.
    pub lines: u32,
    /// Conversations had.
    pub talks: u32,
}

/// The answer to an open conversation (`None` when there is none).
pub fn answer(v: &View<'_>, cx: &mut Ctx) -> Option<Act> {
    let Some(d) = v.dialogue() else {
        if cx.talk.lines > 0 {
            cx.talk.lines = 0;
            cx.talk.talks += 1;
        }
        return None;
    };
    cx.talk.lines += 1;
    if cx.talk.lines > MAX_LINES {
        return Some(Act::press(Command::CloseDialogue));
    }
    if !d.awaiting_choice {
        return Some(Act::press(Command::Advance));
    }
    let Some(node) = d.node else { return Some(Act::press(Command::Advance)) };
    let done = v.quests_done();
    let active: Vec<_> = v.quests().map(|q| q.quest).collect();
    let score = |o: &jane_data::DialogueOption| -> i32 {
        let has = |p: &dyn Fn(&Action) -> bool| o.actions.is_some_and(|l| list_has(v, l, &|a: &Action| p(a)));
        let mut s = 0;
        if has(&|a| matches!(a, Action::Quest(q) if !done.contains(q) && !active.contains(q))) {
            s += 100;
        }
        if has(&|a| matches!(a, Action::HandIn(_))) {
            s += 100;
        }
        if has(&|a| matches!(a, Action::Learn(_) | Action::Grow { .. } | Action::Give(_))) {
            s += 40;
        }
        // Rest a moment rather than sleep the evening away.
        if has(&|a| matches!(a, Action::Rest { until: None })) {
            s += 20;
        }
        // A line that ends the talk over one that leads to more of it: the Rusher always, the
        // Reader too (it has read the question; the answer is in the log).
        if o.goto.is_none() {
            s += 10;
        }
        s
    };
    let mut best = 0u8;
    let mut best_s = i32::MIN;
    for (i, o) in node.options.iter().enumerate() {
        let s = score(o);
        if s > best_s {
            best_s = s;
            best = i as u8;
        }
    }
    Some(Act::press(Command::Choose { option: best }))
}
