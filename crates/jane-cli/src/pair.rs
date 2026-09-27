//! The Co-op pair (VERIFICATION.md §2 L3, P7): two seats over lockstep, a host and a guest
//! joined by `jane-net`'s in-memory links in one process, each seat played by a model on its
//! own peer's sim. The host's sim is traced (both seats' samples and events); each seat's
//! decisions are its own session's, merged into the one trace.

use jane_bot::pair::{Mode, Second};
use jane_bot::run::{Session, header};
use jane_bot::{Bot, Model};
use jane_net::link::mem_listener;
use jane_net::{Guest, GuestConfig, Host, HostConfig, Note};
use jane_sim::trace::Trace;
use jane_sim::{Blueprints, ClientToken, Event, Seat, Sim};

/// What a pair's session came to.
pub struct PairRun {
    pub trace: Trace,
    /// Each seat's log lines.
    pub logs: [Vec<String>; 2],
    pub desyncs: usize,
}

/// Play `mode` on a new game of `bps` for `minutes` of the host's frames.
pub fn play(bps: &Blueprints, mode: Mode, minutes: u32) -> PairRun {
    let (listener, dialer) = mem_listener();
    let sim = Sim::new_game_with(bps.clone(), "Jane");
    let hdr = header(&sim, mode.name(), 2, minutes);
    // The table waits for a slow seat rather than drop it: both are in this process.
    let cfg = HostConfig { wait: true, ..HostConfig::default() };
    let mut host = Host::new(sim, cfg, Box::new(listener));
    let mut lead = Session::named(Bot::story(Model::Reader), hdr.clone());
    let mut second = Second::new(mode, Seat(0));
    let mut other = Session::named(second.bot(), hdr);
    let link = dialer.dial();
    let mut g = Guest::new(Box::new(link), GuestConfig::new(ClientToken(0x5eed)), Some(bps.clone()), 0);
    let want = minutes * 60 * 60;
    let (mut host_presses, mut host_heard): (Vec<jane_sim::Command>, Vec<Event>) = (Vec::new(), Vec::new());
    let (mut g_presses, mut g_heard): (Vec<jane_sim::Command>, Vec<Event>) = (Vec::new(), Vec::new());
    let mut desyncs = 0;
    let mut tick: u64 = 0;
    while host.sim().state().frame < want && tick < u64::from(want) * 3 {
        tick += 1;
        let now = tick * 1000 / 60;
        host.settle(now);
        g.settle();
        host.poll(now);
        g.poll(now);
        host.poll(now);
        let act = {
            let v = host.seat().and_then(|s| host.sim().view(s));
            lead.bot.act(v.as_ref(), &host_heard)
        };
        host_presses.extend(act.cmds);
        if host.try_step(now, Some((act.frame, &mut host_presses))).is_some() {
            host_heard.clear();
            host_heard.extend_from_slice(host.events());
            lead.after(host.sim(), Some(&host_heard));
        }
        for n in host.drain_notes() {
            if matches!(n, Note::Desync(_)) {
                desyncs += 1;
            }
        }
        host.poll(now);
        g.poll(now);
        for _ in 0..4 {
            if g.backlog() == 0 {
                break;
            }
            let act = {
                let seat = g.seat();
                if let Some(s) = seat {
                    other.bot.seat = s;
                }
                let v = seat.and_then(|s| g.sim().and_then(|sim| sim.view(s)));
                second.act(&mut other.bot, v.as_ref(), &g_heard)
            };
            g_presses.extend(act.cmds);
            if g.try_step(now, Some((act.frame, &mut g_presses))).is_none() {
                break;
            }
            g_heard.clear();
            g_heard.extend_from_slice(g.events());
            if let Some(sim) = g.sim() {
                other.record_at(sim, lead.frames());
            }
        }
        g.poll(now);
    }

    let logs = [
        lead.bot.log.iter().map(jane_bot::Milestone::line).collect(),
        other.bot.log.iter().map(jane_bot::Milestone::line).collect(),
    ];
    let (_, mut trace) = lead.finish(host.sim());
    let (_, t2) = other.finish(host.sim());
    trace.records.extend(t2.records);
    trace.records.sort_by_key(|r| r.frame);
    trace.footer.records = trace.records.len() as u32;
    PairRun { trace, logs, desyncs }
}
