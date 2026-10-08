//! The terminal's rows run (ENGINE.md §12): what each line means. A change to the world is a
//! `Command::Dev` stamped like any other (ARCHITECTURE.md §1), so a replay replays it; what only
//! reads (seed, hash, pos) or is the app's own (save, load, speed, title) is answered here.

use std::fmt::Write as _;

use jane_core::{Key, ZoneId};
use jane_present::ui::console::{LineKind, ROWS};
use jane_sim::input::{Command, DevOp};
use jane_sim::{Seat, Sim};

/// What a line asks of the app beyond the sim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// A command for her seat.
    Command(Command),
    /// Words back, and their colour.
    Say(String, LineKind),
    Save(u8),
    Load(u8),
    /// Ticks a real tick, in quarters (1, 4, 16); or hold and step.
    Speed(u32),
    Hold,
    Step,
    Title,
    Clear,
    /// A command for another seat, or a sitting-down (`None`): `join` and `leave` (the host's).
    Seat(Option<Seat>, Command),
}

fn err(s: impl Into<String>) -> Vec<Run> {
    vec![Run::Say(s.into(), LineKind::Error)]
}

fn say(s: impl Into<String>) -> Vec<Run> {
    vec![Run::Say(s.into(), LineKind::Out)]
}

/// A zone's mark to arrive at: the one asked for, else the first of the usual ones, else its
/// first.
fn mark_in(sim: &Sim, zone: ZoneId, asked: Option<&str>) -> Option<jane_core::Sym> {
    let syms = &sim.state().syms;
    if let Some(a) = asked {
        return syms.find(a);
    }
    let bp = sim.blueprints().fetch(zone);
    let named: Vec<jane_core::Sym> = bp
        .marks
        .keys()
        .filter_map(|k| match *k {
            Key::Name(n) => Some(jane_sim::sym::of_name(n)),
            Key::Local(_) => None,
        })
        .collect();
    for want in ["start", "front", "entry", "stair_a", "mouth", "gate"] {
        if let Some(s) = syms.find(want).filter(|s| named.contains(s)) {
            return Some(s);
        }
    }
    named.first().copied()
}

/// Runs one line against `sim` from seat `me`, returning what to do and say.
pub fn run(line: &str, sim: Option<&Sim>, me: Seat) -> Vec<Run> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some((&cmd, args)) = words.split_first() else { return Vec::new() };
    let cat = jane_data::catalog();
    let num = |i: usize| args.get(i).and_then(|s| s.parse::<i32>().ok());
    let dev = |op: DevOp| vec![Run::Command(Command::Dev(op))];
    // Rows that need no world.
    match cmd {
        "help" => {
            let mut out = String::new();
            for (name, what) in ROWS {
                let _ = writeln!(out, "{name:<7} {what}");
            }
            return say(out.trim_end().to_owned());
        }
        "ver" => {
            return say(format!("jane {} content {:016x}", env!("CARGO_PKG_VERSION"), cat.content_hash));
        }
        "clear" => return vec![Run::Clear],
        "title" => return vec![Run::Title],
        "replay" => return say("replays run headless: jane replay verify <file>"),
        "speed" => {
            return match args.first().copied() {
                Some("0.25") => vec![Run::Speed(1)],
                Some("1") => vec![Run::Speed(4)],
                Some("4") => vec![Run::Speed(16)],
                Some("hold") => vec![Run::Hold],
                Some("step") => vec![Run::Step],
                _ => err("speed 0.25, 1, 4, hold or step"),
            };
        }
        "save" | "load" => {
            let slot = num(0).unwrap_or(1);
            if !(1..=3).contains(&slot) {
                return err("slots are 1 to 3");
            }
            return vec![if cmd == "save" { Run::Save(slot as u8 - 1) } else { Run::Load(slot as u8 - 1) }];
        }
        _ => {}
    }
    let Some(sim) = sim else { return err("no world yet: start a game") };
    let Some(v) = sim.view(me) else { return err("no seat") };
    match cmd {
        "give" => {
            let Some(name) = args.first() else { return err("give <item> [qty]") };
            let Some(item) = cat.combat.item_id(name) else { return err(format!("no item {name}")) };
            let qty = num(1).unwrap_or(1).clamp(1, 999) as u16;
            dev(DevOp::Give { item, qty })
        }
        "god" => dev(DevOp::God(!matches!(args.first().copied(), Some("off" | "0")))),
        "tp" => {
            let Some(zone) = args.first().and_then(|z| ZoneId::from_name(z)) else {
                let names: Vec<&str> = ZoneId::ALL.iter().map(|z| z.name()).collect();
                return err(format!("tp <zone> [mark]: {}", names.join(" ")));
            };
            match mark_in(sim, zone, args.get(1).copied()) {
                Some(mark) => dev(DevOp::Tp { zone, mark }),
                None => err(format!("no such mark in {}", zone.name())),
            }
        }
        "time" => match num(0) {
            Some(h) if (0..24).contains(&h) => dev(DevOp::Time { hour: h as u8 }),
            _ => err("time <hour 0-23>"),
        },
        "hp" => num(0).map_or_else(|| err("hp <points>"), |n| dev(DevOp::Hp(n))),
        "mp" => num(0).map_or_else(|| err("mp <points>"), |n| dev(DevOp::Mp(n))),
        "learn" => match args.first().and_then(|s| cat.combat.spell_id(s)) {
            Some(s) => dev(DevOp::Learn(s)),
            None => err("learn <spell>"),
        },
        "quest" => match args.first().and_then(|s| cat.story.quest_id(s)) {
            Some(q) => dev(DevOp::Quest(q)),
            None => err("quest <quest>"),
        },
        "flag" => {
            let (Some(name), Some(value)) = (args.first(), num(1)) else { return err("flag <name> <value>") };
            match v.sym(name) {
                Some(flag) => dev(DevOp::Flag { flag, value }),
                None => err(format!("no flag {name}")),
            }
        }
        "kill" => dev(DevOp::Kill),
        "spawn" => match args.first().and_then(|s| cat.combat.unit_id(s)) {
            Some(u) => dev(DevOp::Spawn(u)),
            None => err("spawn <unit>"),
        },
        "seed" => say(format!("seed {}", v.seed())),
        "hash" => say(format!("hash {:016x} at frame {}", sim.hash(), v.frame())),
        "pos" => {
            let b = v.body();
            let (cx, cy) = b.pos.cell();
            say(format!(
                "{} cell {cx},{cy}  px {},{}  facing {:?}",
                v.zone().name(),
                b.pos.x.0 >> 8,
                b.pos.y.0 >> 8,
                b.facing
            ))
        }
        "inst" => {
            let (w, h) = v.size();
            let units = v.units_in(jane_core::Rect::new(0, 0, w as i32, h as i32)).count();
            let props = v.props().count();
            say(format!("{}: {units} units awake, {props} props, {w} x {h} cells", v.zone().name()))
        }
        "party" => say(format!("{} sitting down", v.party())),
        "open" => vec![Run::Command(Command::Open(true)), Run::Say("open: others may sit down".into(), LineKind::Good)],
        "close" => vec![Run::Command(Command::Open(false)), Run::Say("closed".into(), LineKind::Good)],
        // An idle body sits down (the host's world must be open): the penalty felt before
        // anyone else has come (PLATFORM.md §2). A token names her, to sit her back down later.
        "join" => {
            let who = args.first().and_then(|t| t.parse::<u64>().ok()).unwrap_or(1000 + u64::from(v.party()));
            if !sim.state().open {
                return err("the world is closed: open first");
            }
            vec![
                Run::Seat(None, Command::Join { who: jane_sim::ClientToken(who) }),
                Run::Say(format!("token {who} sits down"), LineKind::Good),
            ]
        }
        "leave" => match num(0) {
            Some(s) if (1..4).contains(&s) => vec![Run::Seat(Some(Seat(s as u8)), Command::Leave)],
            _ => err("leave <seat 1-3>: she gets up (a guest's seat is hung up on)"),
        },
        _ => err(format!("{cmd}? type help")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_answers_and_mutations_are_dev_commands() {
        let sim = Sim::new_game(7, "Tess");
        for (row, _) in ROWS {
            let out = run(row, Some(&sim), Seat(0));
            assert!(!out.is_empty() || row == "kill", "{row} says nothing");
        }
        let item = jane_data::catalog().combat.items[0];
        let got = run(&format!("give {} 3", item.id), Some(&sim), Seat(0));
        assert_eq!(
            got,
            vec![Run::Command(Command::Dev(DevOp::Give {
                item: jane_data::catalog().combat.item_id(item.id).unwrap(),
                qty: 3
            }))]
        );
        assert!(matches!(
            run("tp house", Some(&sim), Seat(0)).as_slice(),
            [Run::Command(Command::Dev(DevOp::Tp { zone: ZoneId::House, .. }))]
        ));
        assert!(matches!(
            run("time 22", Some(&sim), Seat(0)).as_slice(),
            [Run::Command(Command::Dev(DevOp::Time { hour: 22 }))]
        ));
        assert!(matches!(run("give nothing_at_all", Some(&sim), Seat(0)).as_slice(), [Run::Say(_, LineKind::Error)]));
        assert_eq!(run("save 2", None, Seat(0)), vec![Run::Save(1)]);
        assert!(matches!(run("pos", None, Seat(0)).as_slice(), [Run::Say(_, LineKind::Error)]));
        // join and leave are the host's, for other seats.
        assert!(matches!(run("join", Some(&sim), Seat(0)).as_slice(), [Run::Say(_, LineKind::Error)]), "closed");
        assert_eq!(run("leave 2", Some(&sim), Seat(0)), vec![Run::Seat(Some(Seat(2)), Command::Leave)]);
    }
}
