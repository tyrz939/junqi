//! The game: an SDL2 window, the fixed 60-tick loop, the devices into `InputFrame`s, the `soft`
//! backend (PORT.md §7.1 step 3; PRESENTATION.md, the window, §1.11, §4).
//!
//! No SDL type leaves this crate: `jane-present` sees a `DeviceState` and hands back a `Frame`.

mod app;
mod audio;
mod config;
mod console;
mod devices;
mod game;
mod handle;
mod lan;
mod saves;
mod screen;
mod script;
mod shot;

use std::process::ExitCode;

pub const USAGE: &str = "jane-app [--new] [--seed N] [--name NAME] [--scale K] [--backend auto|soft|gl2|wgpu]
         [--ticks N] [--shot PATH] [--script STEPS] [--data-dir DIR]
  --new           skip the title: New Game at once (with --seed and --name)
  --seed N        the county New Game builds (default: from the clock)
  --name NAME     the heroine's name (default: the last one given, else Jane)
  --scale K       the window starts at K x 768 x 432 (default 2, or 1 where 2 does not fit)
  --backend B     auto (default: wgpu at T2 where an adapter can draw it, else gl2 at T1 where OpenGL 2.1
                  or GLES 2 can, else soft), soft (T0), gl2 (T1), wgpu (T2)
  --ticks N       run N ticks (title included), then exit (tests, automation)
  --shot PATH     write the canvas as a PNG on exit; F12 writes PATH-0001.png and on
  --script STEPS  inputs at ticks: \"tick 60 key E; tick 90 click 384 200; tick 120 shot a.png\"
  --data-dir DIR  where saves and config.json live (default: beside the exe when a file called
                  portable is there, else the user's data folder)
  --bot MODEL     a headless player (reader or rusher) plays the seat; the UI shows it
  --loading L     the loading screen: scroll (default: a train window and a line per stage) or map
                  (a developer's view: the county's skeleton forming, which gives the county away)
Playing together on a LAN (the title's Host and Join do the same; ARCHITECTURE.md §7):
  --host          New Game at once, open to the LAN; you play seat 0 and others join you
  --join ADDR     join the host at ADDR[:PORT] at once; the county comes from the host
  --port P        the port to host on or dial (default 7777)
  --seats N       at most N at the table, you included (2 to 4, default 4)
  --delay D       frames of input delay, 2 to 6 (default 3)
  --wait          never drop a player whose input stalls (default: got up after 10 s)
  --token N       who you are to a host (default: kept in config.json)";

/// Which backend draws (PRESENTATION.md §1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    /// wgpu where it can, else gl2, else soft.
    Auto,
    Soft,
    Gl2,
    Wgpu,
}

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    pub seed: u32,
    pub name: String,
    pub scale: Option<u32>,
    pub ticks: Option<u64>,
    pub shot: Option<String>,
    pub backend: BackendChoice,
    /// Straight into New Game, no title.
    pub new: bool,
    /// `--seed` was given: New Game from the title uses it too.
    pub seed_given: bool,
    pub script: Option<String>,
    pub data_dir: Option<String>,
    /// A headless player takes the seat (`reader` or `rusher`).
    pub bot: Option<String>,
    /// Host at once: New Game opened to the LAN.
    pub host: bool,
    /// Join this host at once.
    pub join: Option<String>,
    pub port: u16,
    pub seats: u8,
    pub delay: u8,
    pub wait: bool,
    pub token: Option<u64>,
    /// `--loading map`: the loading screen draws the county's skeleton forming (a developer's
    /// view) instead of the train window and its lines.
    pub loading_map: bool,
}

fn parse(args: &[String], clock_seed: u32) -> Result<Args, String> {
    let mut out = Args {
        seed: clock_seed,
        name: "Jane".into(),
        scale: None,
        ticks: None,
        shot: None,
        backend: BackendChoice::Auto,
        new: false,
        seed_given: false,
        script: None,
        data_dir: None,
        bot: None,
        host: false,
        join: None,
        port: jane_net::wire::DEFAULT_PORT,
        seats: 4,
        delay: jane_net::wire::DEFAULT_DELAY,
        wait: false,
        token: None,
        loading_map: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a}: needs a value"));
        let num = |s: &String| s.parse::<u64>().map_err(|_| format!("{a}: not a number: {s}"));
        match a.as_str() {
            "--seed" => {
                out.seed = u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?;
                out.seed_given = true;
            }
            "--new" => out.new = true,
            "--script" => out.script = Some(value()?.clone()),
            "--data-dir" => out.data_dir = Some(value()?.clone()),
            "--bot" => {
                let m = value()?.clone();
                if !matches!(m.as_str(), "reader" | "rusher") {
                    return Err(format!("--bot: reader or rusher, not {m}"));
                }
                out.bot = Some(m);
            }
            "--name" => out.name.clone_from(value()?),
            "--host" => {
                out.host = true;
                out.new = true;
            }
            "--join" => out.join = Some(value()?.clone()),
            "--port" => out.port = u16::try_from(num(value()?)?).map_err(|_| format!("{a}: not a port"))?,
            "--seats" => out.seats = num(value()?)?.clamp(2, 4) as u8,
            "--delay" => {
                let d = num(value()?)?;
                let (lo, hi) = (u64::from(jane_net::wire::MIN_DELAY), u64::from(jane_net::wire::MAX_DELAY));
                if !(lo..=hi).contains(&d) {
                    return Err(format!("{a}: {lo} to {hi}"));
                }
                out.delay = d as u8;
            }
            "--wait" => out.wait = true,
            "--loading" => {
                out.loading_map = match value()?.as_str() {
                    "scroll" => false,
                    "map" => true,
                    l => return Err(format!("--loading: scroll or map, not {l}")),
                }
            }
            "--token" => out.token = Some(num(value()?)?),
            "--scale" => out.scale = Some(u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?),
            "--ticks" => out.ticks = Some(num(value()?)?),
            "--shot" => out.shot = Some(value()?.clone()),
            "--backend" => {
                out.backend = match value()?.as_str() {
                    "auto" => BackendChoice::Auto,
                    "soft" => BackendChoice::Soft,
                    "gl2" => BackendChoice::Gl2,
                    "wgpu" => BackendChoice::Wgpu,
                    b => return Err(format!("--backend: auto, soft, gl2 or wgpu, not {b}")),
                }
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if out.name.trim().is_empty() {
        return Err("--name: she needs a name".into());
    }
    if out.host && out.join.is_some() {
        return Err("--host and --join: one or the other".into());
    }
    Ok(out)
}

/// A seed from the clock, for a New Game nobody chose a seed for.
pub fn clock_seed() -> u32 {
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
        let d = parse(&[], 99).unwrap();
        let got =
            parse(&a("--seed 7 --name Tess --ticks 300 --shot sheets/app.png --scale 3 --backend wgpu"), 1).unwrap();
        assert_eq!(
            got,
            Args {
                seed: 7,
                name: "Tess".into(),
                scale: Some(3),
                ticks: Some(300),
                shot: Some("sheets/app.png".into()),
                backend: BackendChoice::Wgpu,
                new: false,
                seed_given: true,
                script: None,
                data_dir: None,
                bot: None,
                ..d.clone()
            }
        );
        assert_eq!((d.seed, d.name.as_str(), d.ticks, d.scale), (99, "Jane", None, None));
        assert!(parse(&a("--seed"), 1).is_err());
        assert!(parse(&a("--seed x"), 1).is_err());
        assert!(parse(&a("--wat"), 1).is_err());
        assert!(parse(&a("--backend gl9"), 1).is_err());
        assert_eq!(parse(&a("--backend gl2"), 1).unwrap().backend, BackendChoice::Gl2);
        assert_eq!(d.backend, BackendChoice::Auto);
        // Playing together.
        assert!(!d.host && d.join.is_none() && d.port == 7777 && d.seats == 4 && d.delay == 3 && !d.wait);
        let h = parse(&a("--host --port 7800 --seats 2 --delay 4 --wait"), 1).unwrap();
        assert!(h.host && h.new && h.wait);
        assert_eq!((h.port, h.seats, h.delay), (7800, 2, 4));
        assert_eq!(parse(&a("--join 10.0.0.2 --token 5"), 1).unwrap().join.as_deref(), Some("10.0.0.2"));
        assert!(parse(&a("--host --join x"), 1).is_err());
        assert!(parse(&a("--delay 9"), 1).is_err());
        // The loading screen: the scroll unless the map is asked for.
        assert!(!d.loading_map && parse(&a("--loading map"), 1).unwrap().loading_map);
        assert!(parse(&a("--loading atlas"), 1).is_err());
    }
}
