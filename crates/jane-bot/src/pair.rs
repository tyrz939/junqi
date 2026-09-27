//! The Co-op pair's second seat (VERIFICATION.md §2 L3): **together**, a Reader who keeps to the
//! leader (walks to her when she is more than [`KEEP`] cells off, through the door she took when
//! she is in another zone) and otherwise plays as the Reader does beside her (fights what comes,
//! eats, talks, reads); **split**, a Rusher on its own objectives. The table itself (a host and a
//! guest over `jane-net`'s in-memory links) is the caller's (`jane sweep`, `jane play --model
//! pair:together`).

use jane_core::Fx;
use jane_core::num::CELL_FX;
use jane_sim::{Event, Seat, View};

use crate::task::{Status, Task};
use crate::{Act, Bot, Model};

/// Cells she lets the leader get ahead before walking after her.
pub const KEEP: i32 = 8;

/// How the pair plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A Reader leads; the second keeps to her.
    Together,
    /// A Reader and a Rusher, each on their own.
    Split,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "together" | "pair:together" | "pair" => Some(Mode::Together),
            "split" | "pair:split" => Some(Mode::Split),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Mode::Together => "pair:together",
            Mode::Split => "pair:split",
        }
    }

    /// The second seat's model.
    pub const fn second(self) -> Model {
        match self {
            Mode::Together => Model::Reader,
            Mode::Split => Model::Rusher,
        }
    }
}

/// The second seat's policy over its bot.
#[derive(Debug)]
pub struct Second {
    pub mode: Mode,
    pub leader: Seat,
    following: Option<Task>,
}

impl Second {
    pub fn new(mode: Mode, leader: Seat) -> Second {
        Second { mode, leader, following: None }
    }

    /// Its bot.
    pub fn bot(&self) -> Bot {
        Bot::story(self.mode.second())
    }

    /// This frame's act for `bot` on a lockstep peer (`events` since the last call).
    pub fn act(&mut self, bot: &mut Bot, v: Option<&View<'_>>, events: &[Event]) -> Act {
        if self.mode == Mode::Split {
            return bot.act(v, events);
        }
        let Some(v) = v else { return Act::idle() };
        // Her own conversation, fight or meal first: the Reader's.
        let busy = v.dialogue().is_some()
            || crate::fight::threat(v, &mut bot.ctx).is_some()
            || crate::fight::eat(v).is_some()
            || !v.body().alive;
        let leader = v.friends().find(|&(s, _, _)| s == self.leader);
        if !busy {
            if let Some((_, zone, at)) = leader {
                let far = zone != v.zone() || crate::nav::dist(v.body().pos, at) > i64::from(KEEP * CELL_FX);
                if far {
                    bot.ctx.observe(v, events);
                    let task = match self.following.take() {
                        Some(t) => Some(t),
                        None if zone == v.zone() => Some(Task::Walk { to: at, near: Fx(3 * CELL_FX) }),
                        None => crate::story::route(v, &bot.ctx, zone),
                    };
                    if let Some(mut t) = task {
                        match t.tick(v, &mut bot.ctx) {
                            Status::Act(a) => {
                                // A walk after her is planned again as she moves.
                                if !matches!(t, Task::Walk { .. }) || v.frame() % 60 != 0 {
                                    self.following = Some(t);
                                }
                                return a;
                            }
                            Status::Done | Status::Failed(_) => {}
                        }
                    }
                }
            }
        }
        self.following = None;
        bot.act(Some(v), events)
    }
}
