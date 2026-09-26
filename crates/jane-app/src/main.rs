//! The game: an SDL2 window, the fixed 60-tick loop, the devices into `InputFrame`s, the `soft`
//! backend (PORT.md §7.1 step 3; PRESENTATION.md, the window, §1.11, §4).
//!
//! No SDL type leaves this crate: `jane-present` sees a `DeviceState` and hands back a `Frame`.

mod app;
mod config;
mod devices;
mod game;
mod handle;
mod saves;
mod screen;
mod script;
mod shot;

use std::process::ExitCode;

pub const USAGE: &str = "jane-app [--new] [--seed N] [--name NAME] [--scale K] [--backend auto|soft|wgpu]
         [--ticks N] [--shot PATH] [--script STEPS] [--data-dir DIR]
  --new           skip the title: New Game at once (with --seed and --name)
  --seed N        the county New Game builds (default: from the clock)
  --name NAME     the heroine's name (default: the last one given, else Jane)
  --scale K       the window starts at K x 768 x 432 (default 2, or 1 where 2 does not fit)
  --backend B     auto (default: wgpu at T2 where an adapter can draw it, else soft), soft (T0), wgpu (T2)
  --ticks N       run N ticks (title included), then exit (tests, automation)
  --shot PATH     write the canvas as a PNG on exit; F12 writes PATH-0001.png and on
  --script STEPS  inputs at ticks: \"tick 60 key E; tick 90 click 384 200; tick 120 shot a.png\"
  --data-dir DIR  where saves and config.json live (default: beside the exe when a file called
                  portable is there, else the user's data folder)
  --bot MODEL     a headless player (reader or rusher) plays the seat; the UI shows it";

/// Which backend draws (PRESENTATION.md §1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    /// wgpu where it can, else soft.
    Auto,
    Soft,
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
            "--scale" => out.scale = Some(u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?),
            "--ticks" => out.ticks = Some(num(value()?)?),
            "--shot" => out.shot = Some(value()?.clone()),
            "--backend" => {
                out.backend = match value()?.as_str() {
                    "auto" => BackendChoice::Auto,
                    "soft" => BackendChoice::Soft,
                    "wgpu" => BackendChoice::Wgpu,
                    b => return Err(format!("--backend: auto, soft or wgpu, not {b}")),
                }
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if out.name.trim().is_empty() {
        return Err("--name: she needs a name".into());
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
            }
        );
        let d = parse(&[], 99).unwrap();
        assert_eq!((d.seed, d.name.as_str(), d.ticks, d.scale), (99, "Jane", None, None));
        assert!(parse(&a("--seed"), 1).is_err());
        assert!(parse(&a("--seed x"), 1).is_err());
        assert!(parse(&a("--wat"), 1).is_err());
        assert!(parse(&a("--backend gl9"), 1).is_err());
        assert_eq!(d.backend, BackendChoice::Auto);
    }
}
