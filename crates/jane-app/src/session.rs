//! The app's end of the session (ARCHITECTURE.md §7; PRESENTATION.md §3.1, §3.2): how the
//! window's game starts (alone, hosting, or joining), the client token a guest keeps, and the
//! save a host writes when anyone at the table rests.
//!
//! **Hooks for the menus** (the Title's Host and Join, the pause menu's "Open this world"):
//!
//! | `AppIntent` | Call |
//! | --- | --- |
//! | `Host` (pause menu, a world already playing) | `session.open_to_lan(host_config(..), port)`: the local session becomes a host with the same sim |
//! | `Host` (title, a new world or a slot) | [`start`] with `host` set, or `Session::host(sim, cfg, port)` |
//! | `Join(addr)` | [`join`], then keep drawing while [`jane_net::Session::status`] says `joining`; the sim appears when welcomed |
//! | the Join screen's list | `jane_net::discovery::Finder::new(port)`, `ask()` now and then, `poll()` each frame |
//! | the Host screen's wait toggle | `Session::Host(h)` → `h.set_wait(on)`; open or closed → `h.set_open(on)` |
//!
//! The command line reaches the same calls: `--host [--port P] [--seats N] [--delay D] [--wait]`
//! and `--join ADDR[:PORT] [--token N]`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use jane_net::{GuestConfig, HostConfig, Session};
use jane_sim::{ClientToken, Sim};

use crate::Args;

/// The host's table as the command line asked for it.
pub fn host_config(args: &Args) -> HostConfig {
    HostConfig {
        delay: args.delay,
        seats: args.seats,
        wait: args.wait,
        plays: true,
        name: format!("{}'s world", args.name),
        desync_dir: Some(PathBuf::from(".")),
        ..HostConfig::default()
    }
}

/// Alone, or hosting `sim` on the LAN.
pub fn start(sim: Sim, args: &Args) -> Result<Session, String> {
    if !args.host {
        return Ok(Session::local(sim));
    }
    let s = Session::host(sim, host_config(args), args.port).map_err(|e| format!("host on port {}: {e}", args.port))?;
    println!("jane-app: hosting on port {}; others join with --join <this machine's address>:{}", args.port, args.port);
    Ok(s)
}

/// How long a joiner keeps dialling a host that is not listening yet.
const DIAL_FOR_MS: u64 = 20_000;

/// Knock on `addr` and wait, pumping the window's events (`pump` returns false to give up),
/// until the host has welcomed us with the world, or refused. `clock` is the session's clock
/// (the loop goes on reading it).
pub fn join(args: &Args, addr: &str, clock: Instant, mut pump: impl FnMut(&str) -> bool) -> Result<Session, String> {
    let token = client_token(args.token);
    let mut cfg = GuestConfig::new(token);
    cfg.desync_dir = Some(PathBuf::from("."));
    let now = || clock.elapsed().as_millis() as u64;
    // A host still building its county is not listening yet: dial again for a while.
    let mut s = loop {
        match Session::join(addr, cfg.clone(), None, now()) {
            Ok(s) => break s,
            Err(e) if now() >= DIAL_FOR_MS => return Err(format!("join {addr}: {e}")),
            Err(_) => {
                if !pump(&format!("Jane: dialling {addr}")) {
                    return Err("gave up joining".to_owned());
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    };
    loop {
        s.poll(now());
        let st = s.status();
        if let Some(why) = st.ended {
            return Err(format!("join {addr}: {why}"));
        }
        if s.sim().is_some() && s.seat().is_some() {
            println!("jane-app: joined {addr} in seat {}", s.seat().map_or(0, |x| x.0));
            return Ok(s);
        }
        let doing = st.joining.unwrap_or_default();
        if !pump(&format!("Jane: joining {addr}: {doing}")) {
            return Err("gave up joining".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Where the client token lives: beside the saves (PLATFORM.md §8), `%APPDATA%\Jane` on
/// Windows, `$XDG_CONFIG_HOME/jane` or `~/.config/jane` elsewhere, else the working directory.
fn config_dir() -> PathBuf {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(d) = var("APPDATA") {
            return d.join("Jane");
        }
    } else if let Some(d) = var("XDG_CONFIG_HOME") {
        return d.join("jane");
    } else if let Some(h) = var("HOME") {
        return h.join(".config").join("jane");
    }
    PathBuf::from(".")
}

/// This machine's token (never shown): `--token`, else the one kept in the config directory,
/// else a new one, kept there. A returning guest gets her own body and bags back by it.
pub fn client_token(explicit: Option<u64>) -> ClientToken {
    if let Some(t) = explicit {
        return ClientToken(t.max(1));
    }
    let path = config_dir().join("client-token");
    if let Some(t) = std::fs::read_to_string(&path).ok().and_then(|s| s.trim().parse::<u64>().ok()) {
        return ClientToken(t.max(1));
    }
    let t = fresh_token();
    if std::fs::create_dir_all(config_dir()).is_ok() {
        let _ = std::fs::write(&path, t.to_string());
    }
    ClientToken(t)
}

fn fresh_token() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    // splitmix64 over the clock and the process: distinct enough among four friends.
    let mut z = (t.as_nanos() as u64) ^ (u64::from(std::process::id()) << 32);
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)).max(1)
}

/// The host's world to its slot (a stand-in for the Save screen's slots): written when anyone at
/// the table rests, a guest's rest included, and on quitting.
pub fn write_save(path: &Path, sim: &Sim) {
    match std::fs::write(path, sim.save()) {
        Ok(()) => println!("jane-app: saved {}", path.display()),
        Err(e) => eprintln!("jane-app: save {}: {e}", path.display()),
    }
}

/// The world to play: `--save`'s world if the file is there (the host opens her current world;
/// its own seed's county is built), else a new game of `--seed`.
pub fn load_or_new(args: &Args) -> Result<Sim, String> {
    if let Some(p) = args.save.as_deref().map(Path::new).filter(|p| p.exists()) {
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let sim = Sim::from_save(&bytes).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("jane-app: loaded {} (seed {})", p.display(), sim.state().seed);
        return Ok(sim);
    }
    let bps = jane_sim::Blueprints::build(args.seed).map_err(|e| format!("seed {}: {e}", args.seed))?;
    Ok(Sim::new_game_with(bps, &args.name))
}
