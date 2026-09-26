//! The game: an SDL2 window, the fixed 60-tick loop, the devices into `InputFrame`s, the `soft`
//! backend (PORT.md §7.1 step 3; PRESENTATION.md, the window, §1.11, §4).
//!
//! No SDL type leaves this crate: `jane-present` sees a `DeviceState` and hands back a `Frame`.

mod devices;
mod game;
mod screen;
mod session;
mod shot;

use std::process::ExitCode;

pub const USAGE: &str = "jane-app [--seed N] [--name NAME] [--scale K] [--ticks N] [--shot PATH]
         [--save PATH] [--host [--port P] [--seats N] [--delay D] [--wait]] [--join ADDR[:PORT] [--token N]]
  --seed N      the county (default: from the clock; printed at start)
  --name NAME   the heroine's name (default Jane)
  --scale K     the window starts at K x 768 x 432 (default 2, or 1 where 2 does not fit)
  --ticks N     run N ticks, then exit (tests, automation)
  --shot PATH   write the canvas as a PNG on exit; F12 writes PATH-0001.png and on
  --save PATH   the world's slot: loaded if it is there, written whenever anyone rests and on exit
LAN co-op (ARCHITECTURE.md §7):
  --host        open this world to the LAN; you play seat 0, others join you
  --port P      the port to host on or join (default 7777)
  --seats N     at most N at the table, you included (default 4)
  --delay D     frames of input delay, 2 to 6 (default 3)
  --wait        never drop a seat whose input stalls (default: dropped after 10 s)
  --join ADDR   join the host at ADDR[:PORT]; the county comes from the host
  --token N     who you are to the host (default: kept in the config directory)";

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    pub seed: u32,
    pub name: String,
    pub scale: Option<u32>,
    pub ticks: Option<u64>,
    pub shot: Option<String>,
    pub save: Option<String>,
    pub host: bool,
    pub join: Option<String>,
    pub port: u16,
    pub seats: u8,
    pub delay: u8,
    pub wait: bool,
    pub token: Option<u64>,
}

fn parse(args: &[String], clock_seed: u32) -> Result<Args, String> {
    let mut out = Args {
        seed: clock_seed,
        name: "Jane".into(),
        scale: None,
        ticks: None,
        shot: None,
        save: None,
        host: false,
        join: None,
        port: jane_net::wire::DEFAULT_PORT,
        seats: 4,
        delay: jane_net::wire::DEFAULT_DELAY,
        wait: false,
        token: None,
    };
    let mut port = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a}: needs a value"));
        let num = |s: &String| s.parse::<u64>().map_err(|_| format!("{a}: not a number: {s}"));
        let small = |n: u64, lo: u64, hi: u64| {
            if (lo..=hi).contains(&n) { Ok(n) } else { Err(format!("{a}: {n} is not in {lo}..={hi}")) }
        };
        match a.as_str() {
            "--seed" => out.seed = u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?,
            "--name" => out.name.clone_from(value()?),
            "--scale" => out.scale = Some(u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?),
            "--ticks" => out.ticks = Some(num(value()?)?),
            "--shot" => out.shot = Some(value()?.clone()),
            "--save" => out.save = Some(value()?.clone()),
            "--host" => out.host = true,
            "--join" => out.join = Some(value()?.clone()),
            "--port" => port = Some(small(num(value()?)?, 1, 65535)? as u16),
            "--seats" => out.seats = small(num(value()?)?, 1, 4)? as u8,
            "--delay" => {
                let (lo, hi) = (jane_net::wire::MIN_DELAY, jane_net::wire::MAX_DELAY);
                out.delay = small(num(value()?)?, u64::from(lo), u64::from(hi))? as u8;
            }
            "--wait" => out.wait = true,
            "--token" => out.token = Some(num(value()?)?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if out.name.trim().is_empty() {
        return Err("--name: she needs a name".into());
    }
    if out.host && out.join.is_some() {
        return Err("--host and --join: one or the other".into());
    }
    if let Some(p) = port {
        out.port = p;
        // A port given with --join is where to dial.
        if let Some(j) = &mut out.join {
            if j.rsplit_once(':').is_none_or(|(_, q)| q.parse::<u16>().is_err()) {
                *j = format!("{j}:{p}");
            }
        }
    }
    Ok(out)
}

/// A seed from the clock, for a New Game nobody chose a seed for.
fn clock_seed() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    (t.as_secs() as u32) ^ t.subsec_nanos()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(&args, clock_seed()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("jane-app: {e}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match game::run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("jane-app: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn arguments() {
        let got = parse(&a("--seed 7 --name Tess --ticks 300 --shot sheets/app.png --scale 3"), 1).unwrap();
        let d = parse(&[], 99).unwrap();
        assert_eq!(
            got,
            Args {
                seed: 7,
                name: "Tess".into(),
                scale: Some(3),
                ticks: Some(300),
                shot: Some("sheets/app.png".into()),
                ..d.clone()
            }
        );
        assert_eq!((d.seed, d.name.as_str(), d.ticks, d.scale), (99, "Jane", None, None));
        assert!(!d.host && d.join.is_none() && d.port == 7777 && d.seats == 4 && d.delay == 3);
        assert!(parse(&a("--seed"), 1).is_err());
        assert!(parse(&a("--seed x"), 1).is_err());
        assert!(parse(&a("--wat"), 1).is_err());
        // LAN co-op.
        let h = parse(&a("--host --port 7800 --seats 2 --delay 4 --wait"), 1).unwrap();
        assert!(h.host && h.wait);
        assert_eq!((h.port, h.seats, h.delay), (7800, 2, 4));
        assert_eq!(parse(&a("--join 192.168.1.20"), 1).unwrap().join.as_deref(), Some("192.168.1.20"));
        assert_eq!(parse(&a("--join pi --port 7801"), 1).unwrap().join.as_deref(), Some("pi:7801"));
        assert_eq!(parse(&a("--join pi:7802 --port 7801"), 1).unwrap().join.as_deref(), Some("pi:7802"));
        assert!(parse(&a("--host --join x"), 1).is_err());
        assert!(parse(&a("--seats 5"), 1).is_err());
        assert!(parse(&a("--delay 1"), 1).is_err());
    }
}
