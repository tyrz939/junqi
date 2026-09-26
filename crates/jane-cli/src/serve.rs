//! `jane serve`: the headless host (ARCHITECTURE.md §7, PORT.md §12): a world on this machine
//! that nobody here plays, stepped at 60 Hz whenever every seat's input has come, with one
//! status line (tick, seats, hash). A Pi can host this way with no SDL. The first to join sits
//! in seat 0, the world's own.
//!
//! `jane join`: a headless guest played by a bot model (`jane-bot`), for soaks and the
//! two-process test: it joins a host by address, plays its seat, and prints the same status line.

use std::fmt::Write as _;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use jane_bot::{Act, Bot, Model};
use jane_net::link::TcpListen;
use jane_net::{GuestConfig, Host, HostConfig, Phase, Session};
use jane_sim::{ClientToken, Command, Sim};

pub const USAGE: &str = "  serve [--seed N | --save PATH] [--name NAME] [--port P] [--seats N] [--delay D] [--wait]
        [--ticks N] [--every S] [--record TAPE]
                                      host a world headless on the LAN (nobody here plays); one status line
                                      (tick, seats, hash) every S seconds (default 5); --save is loaded if
                                      there and written whenever anyone rests and at the end; --record writes
                                      a new game's session as a .jrp (`jane replay verify` re-simulates it)
  join ADDR[:PORT] [--model reader|rusher|idle] [--token N] [--ticks N] [--every S]
                                      a headless guest played by a bot model; the same status line";

struct Opts {
    seed: u32,
    save: Option<PathBuf>,
    record: Option<PathBuf>,
    name: String,
    port: u16,
    seats: u8,
    delay: u8,
    wait: bool,
    ticks: Option<u64>,
    every: u64,
    model: Option<Model>,
    token: Option<u64>,
    addr: Option<String>,
}

fn opts(args: &[String], join: bool) -> Result<Opts, String> {
    let mut o = Opts {
        seed: 1,
        save: None,
        record: None,
        name: "Jane".into(),
        port: jane_net::wire::DEFAULT_PORT,
        seats: 4,
        delay: jane_net::wire::DEFAULT_DELAY,
        wait: false,
        ticks: None,
        every: 5,
        model: Some(Model::Rusher),
        token: None,
        addr: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a}: needs a value"));
        let num = |s: &String| s.parse::<u64>().map_err(|_| format!("{a}: not a number: {s}"));
        match a.as_str() {
            "--seed" => o.seed = u32::try_from(num(value()?)?).map_err(|_| format!("{a}: too big"))?,
            "--save" => o.save = Some(PathBuf::from(value()?)),
            "--record" => o.record = Some(PathBuf::from(value()?)),
            "--name" => o.name.clone_from(value()?),
            "--port" => o.port = u16::try_from(num(value()?)?).map_err(|_| format!("{a}: not a port"))?,
            "--seats" => o.seats = num(value()?)?.clamp(1, 4) as u8,
            "--delay" => o.delay = num(value()?)?.clamp(2, 6) as u8,
            "--wait" => o.wait = true,
            "--ticks" => o.ticks = Some(num(value()?)?),
            "--every" => o.every = num(value()?)?.max(1),
            "--token" => o.token = Some(num(value()?)?.max(1)),
            "--model" => {
                let m = value()?;
                o.model =
                    if m == "idle" { None } else { Some(Model::parse(m).ok_or_else(|| format!("no model {m}"))?) };
            }
            s if join && !s.starts_with("--") && o.addr.is_none() => o.addr = Some(s.to_owned()),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    Ok(o)
}

/// Where a status line goes: rewritten in place on a terminal, a line each otherwise.
fn say(line: &str) {
    let mut out = std::io::stdout();
    if out.is_terminal() {
        let _ = write!(out, "\r{line:<100}");
    } else {
        let _ = writeln!(out, "{line}");
    }
    let _ = out.flush();
}

fn log(line: &str) {
    let mut out = std::io::stdout();
    if out.is_terminal() {
        let _ = write!(out, "\r{:<100}\r", "");
    }
    let _ = writeln!(out, "{line}");
}

fn seats_of(sim: &Sim) -> String {
    let seats: Vec<String> = sim.state().connected().map(|p| p.seat.0.to_string()).collect();
    format!("{} [{}]", seats.len(), seats.join(" "))
}

pub fn serve(args: &[String]) -> Result<(), String> {
    let o = opts(args, false)?;
    let sim = match o.save.as_deref().filter(|p| p.exists()) {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
            Sim::from_save(&bytes).map_err(|e| format!("{}: {e}", p.display()))?
        }
        None => Sim::new_game(o.seed, &o.name),
    };
    let seed = sim.state().seed;
    let cfg = HostConfig {
        delay: o.delay,
        seats: o.seats,
        wait: o.wait,
        plays: false,
        name: format!("{}'s world (served)", sim.state().name),
        desync_dir: Some(PathBuf::from(".")),
        ..HostConfig::default()
    };
    let l = TcpListen::bind(o.port).map_err(|e| format!("port {}: {e}", o.port))?;
    let mut host = Host::new(sim, cfg, Box::new(l)).with_port(o.port);
    if o.record.is_some() && !host.record() {
        return Err("--record: only a new game is recorded (a tape begins at New Game)".into());
    }
    match jane_net::discovery::Beacon::bind(o.port) {
        Ok(b) => host = host.with_beacon(b),
        Err(e) => log(&format!("jane serve: no discovery on udp {}: {e}", o.port)),
    }
    log(&format!("jane serve: seed {seed}, listening on port {}; join with --join <address>:{}", o.port, o.port));
    let clock = Instant::now();
    let mut ticks: u64 = 0;
    let mut said = 0;
    loop {
        let now = clock.elapsed().as_millis() as u64;
        host.poll(now);
        let due = now * 60 / 1000;
        let mut stepped = false;
        while ticks < due {
            if host.try_step(now, None).is_none() {
                // A stalled table does not run fast afterwards to make the time up.
                ticks = due;
                break;
            }
            ticks += 1;
            stepped = true;
            if host.take_rested() {
                if let Some(p) = &o.save {
                    write_save(p, host.sim());
                }
            }
        }
        host.poll(now);
        for n in host.drain_notes() {
            log(&format!("jane serve: {n}"));
        }
        let frame = host.sim().state().frame;
        if now >= said + o.every * 1000 {
            said = now;
            say(&status(&host, ticks));
        }
        if o.ticks.is_some_and(|n| u64::from(frame) >= n) {
            break;
        }
        if !stepped {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    log(&status(&host, ticks));
    let c = host.checks();
    log(&format!("jane serve: hash checks with guests: {} agreed, {} differed, last at frame {}", c.ok, c.bad, c.last));
    if let (Some(p), Some(tape)) = (&o.record, host.take_tape()) {
        match std::fs::write(p, tape.encode()) {
            Ok(()) => log(&format!("jane serve: the session, {} frames, is the tape {}", tape.frames, p.display())),
            Err(e) => log(&format!("jane serve: tape {}: {e}", p.display())),
        }
    }
    if let Some(p) = &o.save {
        write_save(p, host.sim());
    }
    host.close();
    // Let the goodbyes go.
    host.poll(clock.elapsed().as_millis() as u64);
    Ok(())
}

fn status(host: &Host, ticks: u64) -> String {
    let sim = host.sim();
    let (hf, h) = host.last_hash().unwrap_or((0, 0));
    let c = host.checks();
    let mut line = format!(
        "tick {ticks} frame {} seats {} hash {h:016x}@{hf} checks {}/{}",
        sim.state().frame,
        seats_of(sim),
        c.ok,
        c.ok + c.bad
    );
    if let Some(s) = host.stall() {
        let who: Vec<String> = (0..4).filter(|i| s.seats & (1 << i) != 0).map(|i| i.to_string()).collect();
        let _ = write!(
            line,
            " waiting for seat {} ({} ms{})",
            who.join(" "),
            s.waited_ms,
            if s.wait { ", wait on" } else { "" }
        );
    }
    line
}

fn write_save(p: &std::path::Path, sim: &Sim) {
    match std::fs::write(p, sim.save()) {
        Ok(()) => log(&format!("jane serve: saved {}", p.display())),
        Err(e) => log(&format!("jane serve: save {}: {e}", p.display())),
    }
}

pub fn join(args: &[String]) -> Result<(), String> {
    let o = opts(args, true)?;
    let addr = o.addr.clone().ok_or("jane join: which host? jane join ADDR[:PORT]")?;
    let token = ClientToken(o.token.unwrap_or_else(|| u64::from(std::process::id()) << 8 | 1));
    let clock = Instant::now();
    let now = || clock.elapsed().as_millis() as u64;
    // A host still building its county is not listening yet.
    let mut s = loop {
        match Session::join(&addr, GuestConfig::new(token), None, now()) {
            Ok(s) => break s,
            Err(e) if now() > 20_000 => return Err(format!("join {addr}: {e}")),
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    };
    let mut bot = o.model.map(Bot::story);
    let mut heard = Vec::new();
    let mut presses: Vec<Command> = Vec::new();
    let mut said = 0;
    let mut stepped: u64 = 0;
    let mut announced = false;
    loop {
        let t = now();
        s.poll(t);
        let mut any = false;
        for _ in 0..8 {
            if s.backlog() == 0 {
                break;
            }
            let seat = s.seat();
            let act = match (&mut bot, s.sim(), seat) {
                (Some(b), Some(sim), Some(seat)) => {
                    b.seat = seat;
                    b.act(sim.view(seat).as_ref(), &heard)
                }
                _ => Act::idle(),
            };
            presses.extend(act.cmds);
            if s.try_step(t, act.frame, &mut presses, false).is_none() {
                break;
            }
            heard.clear();
            heard.extend_from_slice(s.events());
            stepped += 1;
            any = true;
        }
        let st = s.status();
        if let Some(end) = st.ended {
            // What came before the goodbye is stepped.
            while s.backlog() > 0 && s.try_step(t, jane_sim::InputFrame::IDLE, &mut presses, false).is_some() {
                stepped += 1;
            }
            log(&format!("jane join: {end}"));
            break;
        }
        s.poll(t);
        if !announced {
            if let (Some(seat), Some(sim)) = (s.seat(), s.sim()) {
                announced = true;
                log(&format!("jane join: sat down in seat {} at frame {}", seat.0, sim.state().frame));
            }
        }
        if let (true, Session::Guest(g)) = (t >= said + o.every * 1000, &s) {
            said = t;
            if let (Some(sim), Phase::Playing) = (g.sim(), g.phase()) {
                let (hf, h) = g.last_hash().unwrap_or((0, 0));
                let mut line = format!(
                    "seat {} frame {} seats {} hash {h:016x}@{hf}",
                    g.seat().map_or(0, |x| x.0),
                    sim.state().frame,
                    seats_of(sim)
                );
                if let Some(st) = g.stall() {
                    let _ = write!(line, " waiting ({} ms)", st.waited_ms);
                }
                say(&line);
            }
        }
        if let Some(r) = st.desync {
            log(&format!("jane join: {r}"));
        }
        if o.ticks.is_some_and(|n| stepped >= n) {
            break;
        }
        if !any {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if let Session::Guest(g) = &s {
        if let (Some(sim), Some((hf, h))) = (g.sim(), g.last_hash()) {
            log(&format!("jane join: {stepped} frames stepped, frame {}, hash {h:016x}@{hf}", sim.state().frame));
        }
    }
    s.close();
    Ok(())
}
