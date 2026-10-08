//! `jane sheet`: contact sheets from jane-art as PNGs (ART.md §5, PRESENTATION.md §6), and
//! `--bless`, which rewrites jane-art's golden hashes. Today the step-1 sheets: the demo
//! sprites' layers and lighting, the font, the chrome and the palette; from step 2 the people:
//! every look's frames, every seat and variant, and the build-by-hair-by-coat grid; and `scene`,
//! one whole frame of a played seed through the presenter and `soft`.

use std::path::{Path, PathBuf};

use jane_art::sheet::{self, Image};
use jane_art::{Font, demo, looks, sheet_person};

pub const USAGE: &str = "  sheet layers <what> [--frame F] [--out DIR]
                                      a sprite's albedo, normal, emissive and height at 4x
  sheet light <what> [--frame F] [--night] [--out DIR]
                                      a sprite lit from eight directions and overhead, with shadows
  sheet font | chrome | palette [--out DIR]
                                      every glyph in every face; the chrome pieces; the palette ramps
  sheet unit <id> [--seat N] [--out DIR]
                                      a look's frames, every cycle, seat and variant, and the dead
  sheet close <id> [frame ...] [--scale N] [--out DIR]
                                      a few frames of a look up close (down, side, up at 8x)
  sheet facings <id> ... [--scale N] [--out DIR]
                                      looks in their eight facings, standing and mid-walk (west mirrored)
  sheet silhouettes <id> ... [--out DIR]
                                      looks' standing frames filled black, then as drawn
  sheet units [--out DIR]             every look standing and dead, at 1x and 2x
  sheet fires [--out DIR]             the fires she makes cold, laid, lit and ash, day and night;
                                      the match flare; deadwood and matches
  sheet person --grid [--out DIR]     every build by every hair and coat
  sheet all [--out DIR]               every sheet above, for every sprite
  sheet list                          the sprites <what> can name
  sheet scene [--seed N] [--minutes M | --ticks T] [--model reader|rusher] [--night | --hour H[:MM]]
              [--wide] [--backend soft|gl2|wgpu] [--at ZONE[:MARK] | --at MARK]
              [--weather clear|mist|rain|storm] [--cast SPELL[:TICKS] [--spawn UNIT]] [--rows KEY=V,..]
              [--film N[:EVERY] [--walk [push-]DIR[:TICKS],..]] [--crop X,Y,W,H] [--zoom Z] [--layers] [--show-sun]
              [--knows SPELL,..] [--learn SPELL,..] [--grow strength|spirit] [--ui] [--psp]
              [--out PATH.png | --out DIR]
                                      --learn learns spells after the rest (--knows ones before, out of
                                      sight, so a --learn is not her first) and --grow finds a jar or a
                                      page: the lesson's moment, filmed with the HUD and its card (--ui
                                      draws the HUD alone);
                                      a model plays the seed from New Game (default 1 minute), then one
                                      frame is drawn headless through the presenter and soft (T0), or
                                      gl2 (T1, a hidden window's GL context) or wgpu (T2) with the gpu
                                      feature; --night sets the clock to 22:00 first; --at travels to a
                                      zone's mark (its way in by default; a bare mark is the county's;
                                      a dungeon's room by its node; ZONE:X,Y a cell)
                                      first, god on: a frame inside a dungeon; --weather holds the sky;
                                      --cast casts east (--spawn puts a unit in its way); --rows sets Features rows; --film writes N more
                                      ticks' frames; --wide draws 21:9 (840 x 360); --crop and --zoom
                                      write a close look; gl2 takes bench frames' row flags; --layers
                                      also writes the frame's heights and the T2 height field (a px h up
                                      stood rows_up(h) rows down), --show-sun draws wgpu's sun term
                                      alone (red reached, green N dot L)
  sheet ui [screen ...] [--out DIR]   the UI in states play rarely shows at once (hud, dead, choice,
                                      tooltip, popover, drag, pause), headless through soft
  sheet audio [--out DIR]             every sound effect, bed, song and scene as WAV, songs and scenes as
                                      a waveform over a spectrogram (jane audio)
  sheet --bless                       rewrite crates/jane-art/tests/golden.txt from the current art";

/// jane-art's golden file, from this crate's manifest.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../jane-art/tests/golden.txt")
}

fn write(out: &Path, name: &str, img: &Image) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join(format!("{name}.png"));
    std::fs::write(&path, img.png()).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

/// A demo sprite, or a frame of a look (`--frame`, `down` when not given; `base` for a prop),
/// and the row its feet stand on.
fn sprite(what: Option<&String>, args: &[String]) -> Result<(String, jane_art::Canvas, i32), String> {
    let what = what.ok_or_else(|| format!("name a sprite: {}, or a look", demo::NAMES.join(", ")))?;
    if let Some(c) = demo::sprite(what) {
        let foot = c.h() - 1;
        return Ok((what.clone(), c, foot));
    }
    let sets = looks::render(what).map_err(|e| format!("{e}; the demo sprites are {}", demo::NAMES.join(", ")))?;
    let first = sets.first().ok_or("no sets")?;
    let default = first.set.frames.first().map_or("down", |(f, _)| f.name());
    let frame = args.iter().position(|a| a == "--frame").and_then(|i| args.get(i + 1)).map_or(default, |s| s);
    let id = jane_art::sprite::FrameId::by_name(frame)
        .ok_or_else(|| format!("no frame \"{frame}\"; try down, side_1, up_b, dead, idle, base, on"))?;
    let c = first.set.frame(id).ok_or("no such frame")?.clone();
    // A unit stands on its anchor; a prop on the canvas's last row.
    let foot = if first.set.ax == 0 { c.h() - 1 } else { first.set.ay };
    Ok((format!("{what}-{frame}"), c, foot))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let out =
        PathBuf::from(args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).map_or("sheets", |s| s));
    let night = args.iter().any(|a| a == "--night");
    let font = Font::build();
    match args.first().map(String::as_str) {
        Some("--bless") => {
            let path = golden_path();
            std::fs::write(&path, demo::golden_file(&font)).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("blessed {}", path.display());
            crate::sheet_terrain::bless()?;
        }
        Some("layers") => {
            let (name, c, _) = sprite(args.get(1), args)?;
            write(&out, &format!("layers-{name}"), &sheet::layers(&c, &name, &font))?;
        }
        Some("light") => {
            let (name, c, foot) = sprite(args.get(1), args)?;
            let suffix = if night { "-night" } else { "" };
            let img = if demo::sprite(args[1].as_str()).is_some() {
                sheet::lit(&c, &name, &font, night)
            } else {
                sheet::lit_upright(&c, &name, &font, night, foot)
            };
            write(&out, &format!("light-{name}{suffix}"), &img)?;
        }
        Some("unit") => {
            let what = args.get(1).ok_or("name a look: jane, town_grocer, ...")?;
            let mut sets = looks::render(what)?;
            let seat = args.iter().position(|a| a == "--seat").and_then(|i| args.get(i + 1)?.parse::<u8>().ok());
            if let Some(s) = seat {
                sets.retain(|r| r.seat == s);
            }
            let scale =
                args.iter().position(|a| a == "--scale").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(4);
            let suffix = seat.map_or(String::new(), |s| format!("-seat{s}"));
            write(&out, &format!("unit-{what}{suffix}"), &sheet_person::unit(&sets, &font, scale))?;
        }
        Some("close") => {
            let what = args.get(1).ok_or("name a look")?;
            let sets = looks::render(what)?;
            let names: Vec<&String> = args[2..].iter().take_while(|a| !a.starts_with("--")).collect();
            let ids: Vec<jane_art::sprite::FrameId> = if names.is_empty() {
                use jane_art::sprite::FrameId;
                vec![FrameId::Down, FrameId::Side, FrameId::Up]
            } else {
                names.iter().filter_map(|n| jane_art::sprite::FrameId::by_name(n)).collect()
            };
            let scale =
                args.iter().position(|a| a == "--scale").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(8);
            write(&out, &format!("close-{what}"), &sheet_person::closeup(&sets[0], &ids, &font, scale))?;
        }
        Some("facings") => {
            let mut sets = Vec::new();
            for name in args[1..].iter().take_while(|a| !a.starts_with("--")) {
                sets.extend(looks::render(name)?.into_iter().filter(|r| r.seat == 0 && r.variant == 0));
            }
            let scale =
                args.iter().position(|a| a == "--scale").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(3);
            write(&out, "facings", &sheet_person::facings(&sets, &font, scale))?;
        }
        Some("silhouettes") => {
            let mut sets = Vec::new();
            for name in args[1..].iter().take_while(|a| !a.starts_with("--")) {
                sets.extend(looks::render(name)?.into_iter().filter(|r| r.seat == 0 && r.variant == 0));
            }
            write(&out, "silhouettes", &sheet_person::silhouettes(&sets, &font))?;
        }
        Some("units") => {
            let mut sets = looks::family(looks::Family::Person)?;
            sets.extend(looks::family(looks::Family::Creature)?);
            write(&out, "units", &sheet_person::units(&sets, &font))?;
        }
        Some("icons") => {
            write(&out, "icons", &jane_art::sheet_kit::icons(&looks::family(looks::Family::Icon)?, &font))?;
        }
        Some("buildings") => {
            write(&out, "buildings", &jane_art::sheet_kit::props(&looks::family(looks::Family::Building)?, &font))?;
        }
        Some("props") => {
            let mut sets = looks::family(looks::Family::Prop)?;
            if let Some(f) = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)) {
                sets.retain(|r| f.split(',').any(|p| r.name.contains(p)));
            }
            write(&out, "props", &jane_art::sheet_kit::props(&sets, &font))?;
        }
        Some("fires") => write(&out, "fires", &jane_art::sheet_kit::fires(&font)?)?,
        Some("creatures") => {
            write(&out, "creatures", &sheet_person::units(&looks::family(looks::Family::Creature)?, &font))?;
        }
        Some("person") if args.iter().any(|a| a == "--grid") => {
            let Some((_, jane_data::Look::Person(base))) = looks::find("jane") else {
                return Err("no look for jane".into());
            };
            write(&out, "person-grid", &sheet_person::grid(base, &font)?)?;
        }
        Some("font") => write(&out, "font", &sheet::font_sheet(&font))?,
        Some("chrome") => write(&out, "chrome", &sheet::chrome_sheet(&font))?,
        Some("palette") => write(&out, "palette", &sheet::palette_sheet(&font))?,
        Some("all") => {
            for name in demo::NAMES {
                let c = demo::sprite(name).ok_or("demo list out of step")?;
                write(&out, &format!("layers-{name}"), &sheet::layers(&c, name, &font))?;
                write(&out, &format!("light-{name}"), &sheet::lit(&c, name, &font, false))?;
                write(&out, &format!("light-{name}-night"), &sheet::lit(&c, name, &font, true))?;
            }
            write(&out, "font", &sheet::font_sheet(&font))?;
            write(&out, "chrome", &sheet::chrome_sheet(&font))?;
            write(&out, "palette", &sheet::palette_sheet(&font))?;
            write(&out, "units", &sheet_person::units(&looks::all()?, &font))?;
        }
        Some("list") => {
            println!("{}", demo::NAMES.join("\n"));
            for r in looks::all()? {
                println!("{}", r.key());
            }
        }
        Some("scene") => scene(args)?,
        // PLAY-PLAN.md §2.2's frames: the pit cold, laid, flaring, lit at night, ash; the Halt card.
        Some("fire-scenes") => {
            let dir = out.join("p2-fires");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            for f in crate::scene::fire_scenes(1, &dir)? {
                println!("{f}");
            }
        }
        Some("audio") => crate::audio_cmd::sheet(&out)?,
        Some("terrain") => crate::sheet_terrain::terrain(args, &out, &font)?,
        Some("flora") => crate::sheet_terrain::flora(&out, &font)?,
        Some("critters") => write(&out, "critters", &critters(&font))?,
        Some("county") => crate::sheet_terrain::county(args, &out, &font)?,
        Some("ui") => {
            let names: Vec<String> = args[1..].iter().take_while(|a| !a.starts_with("--")).cloned().collect();
            crate::ui_sheet::run(&out, &names)?;
        }
        _ => return Err(format!("usage:\n{USAGE}\n{}", crate::sheet_terrain::USAGE)),
    }
    Ok(())
}

/// The ambient layer's small lives (ART-PLAN M1), each frame at 1x on a grass swatch and again at
/// 4x, then the smoke's puffs and the lily pads.
fn critters(font: &jane_art::Font) -> sheet::Image {
    use jane_art::creature::critter::{self, Critter};
    let grass = [92, 124, 60, 255];
    let rows: Vec<(String, Vec<jane_art::Canvas>)> = Critter::ALL
        .iter()
        .map(|&c| (c.name().to_owned(), critter::render(c).frames.into_iter().map(|(_, f)| f).collect()))
        .chain([
            ("smoke".to_owned(), (1..=6).map(|r| critter::puff(r, r as u32 * 7)).collect()),
            ("lily".to_owned(), (0..6).map(critter::lily_pad).collect()),
        ])
        .collect();
    let (s, cell) = (4u32, 100u32);
    let w = 90 + cell * 6;
    let mut img = sheet::Image::new(w, rows.len() as u32 * 110 + 10, [40, 44, 52, 255]);
    for (r, (name, frames)) in rows.iter().enumerate() {
        let y = 10 + r as u32 * 110;
        sheet::label(&mut img, font, 4, y, name, jane_art::Face::Small, [230, 230, 230]);
        for (k, f) in frames.iter().enumerate() {
            let x = 90 + k as u32 * cell;
            img.fill(x, y, f.w() as u32 * s + 4, f.h() as u32 * s + 4, [grass[0], grass[1], grass[2]]);
            sheet::put_albedo(&mut img, f, x + 2, y + 2, s);
            img.fill(x, y + 80, f.w() as u32 + 4, f.h() as u32 + 4, [grass[0], grass[1], grass[2]]);
            sheet::put_albedo(&mut img, f, x + 2, y + 82, 1);
        }
    }
    img
}

fn scene(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let num =
        |name: &str| flag(name).map(|s| s.parse::<u32>().map_err(|_| format!("{name}: not a number: {s}"))).transpose();
    let seed = num("--seed")?.unwrap_or(1);
    let ticks = match (num("--ticks")?, num("--minutes")?) {
        (Some(t), _) => t,
        (None, m) => m.unwrap_or(1) * 60 * 60,
    };
    let model = jane_bot::Model::parse(flag("--model").unwrap_or("reader")).ok_or("--model: reader or rusher")?;
    let (hour_arg, minute) = match flag("--hour").map(|h| h.split_once(':').unwrap_or((h, "0"))) {
        Some((h, m)) => (
            Some(h.parse::<u32>().map_err(|_| format!("--hour: not an hour: {h}"))?),
            m.parse::<u8>().map_err(|_| format!("--hour: not minutes: {m}"))?.min(59),
        ),
        None => (None, 0),
    };
    let hour = match (args.iter().any(|a| a == "--night"), hour_arg) {
        (_, Some(h)) => Some(u8::try_from(h % 24).expect("an hour")),
        (true, None) => Some(22),
        (false, None) => None,
    };
    let psp = args.iter().any(|a| a == "--psp");
    let canvas = if psp {
        (480, 272)
    } else if args.iter().any(|a| a == "--wide") {
        (840, 360)
    } else {
        (640, 360)
    };
    let backend =
        crate::scene::Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft, gl2 or wgpu")?;
    let gl = crate::scene::GlOpts::parse(args)?;
    let weather = flag("--weather").map(crate::scene::weather).transpose()?;
    let cast = flag("--cast").map(str::to_owned);
    let spawn = flag("--spawn").map(str::to_owned);
    let rows = crate::scene::rows(flag("--rows"))?;
    let name = format!(
        "scene-{seed}-{ticks}{}{}-{}-{}",
        hour.map_or(String::new(), |h| format!("-h{h:02}{minute:02}")),
        weather.map_or(String::new(), |w| format!("-{w:?}").to_lowercase()),
        model.name(),
        backend.name()
    );
    // `--at mine` or `--at mine:guard`: a frame inside a zone, arrived at by the console's tp;
    // `--at lake_bank`, a name that is no zone, is a mark of the county.
    let at = flag("--at").map(|a| match a.split_once(':') {
        Some((z, m)) => (z.to_string(), Some(m.to_string())),
        None => (a.to_string(), None),
    });
    let name = match &at {
        Some((z, m)) => format!("{name}-{z}{}", m.as_deref().map_or(String::new(), |m| format!("-{m}"))),
        None => name,
    };
    let path = match flag("--out") {
        Some(p) if p.ends_with(".png") => PathBuf::from(p),
        Some(dir) => PathBuf::from(dir).join(format!("{name}.png")),
        None => PathBuf::from("sheets").join(format!("{name}.png")),
    };
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let lesson = crate::scene::LessonOpts::parse(args)?;
    let o = crate::scene::Opts {
        seed,
        ticks,
        model,
        hour,
        minute,
        canvas,
        backend,
        at,
        weather,
        cast,
        talk: flag("--talk").map(str::to_owned),
        spawn,
        quests: flag("--quest").map(|q| q.split(',').map(|s| s.trim().to_owned()).collect()).unwrap_or_default(),
        rows,
        gl,
        lesson,
        psp,
        walk: match flag("--walk") {
            None => Vec::new(),
            Some(w) => w
                .split(',')
                .map(|leg| {
                    let (dir, ticks) = leg.split_once(':').unwrap_or((leg, "100000"));
                    // `push-east`: leaning on USE as she goes, so what she walks into is pushed.
                    let (push, dir) = dir.strip_prefix("push-").map_or((false, dir), |d| (true, d));
                    let dir = match dir {
                        "east" => jane_core::Angle::EAST,
                        "west" => jane_core::Angle::WEST,
                        "north" => jane_core::Angle::NORTH,
                        "south" => jane_core::Angle::SOUTH,
                        _ => return Err(format!("--walk DIR[:TICKS],..: east, west, north or south, not {dir}")),
                    };
                    Ok((dir, ticks.parse::<u32>().map_err(|_| format!("--walk: ticks, not {ticks}"))?, push))
                })
                .collect::<Result<Vec<_>, String>>()?,
        },
    };
    // `--film N[:EVERY]`: N ticks more, every EVERY-th a frame, `<name>-<tick>.png` beside the path.
    if let Some(f) = flag("--film") {
        let (n, every) = f.split_once(':').map_or((f, "1"), |p| p);
        let n: u32 = n.parse().map_err(|_| format!("--film N[:EVERY], not {f}"))?;
        let every: u32 = every.parse().map_err(|_| format!("--film N[:EVERY], not {f}"))?;
        let dir = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        let stem = path.file_stem().map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // `--crop` and `--zoom` take each film frame's close look too.
        let close = match (flag("--crop"), flag("--zoom")) {
            (None, None) => None,
            (crop, zoom) => {
                let r = crop.map_or(Ok((0, 0, o.canvas.0, o.canvas.1)), |c| {
                    let v: Vec<u16> = c.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if v.len() == 4 { Ok((v[0], v[1], v[2], v[3])) } else { Err(format!("--crop X,Y,W,H, not {c}")) }
                })?;
                let z = zoom.map_or(Ok(3), |z| z.parse::<u16>().map_err(|_| format!("--zoom: not a number: {z}")))?;
                Some((r, z))
            }
        };
        // `--strip PATH [--tiers t0,t1,t2] [--half]`: the film as one contact sheet, a row a tier.
        if let Some(sp) = flag("--strip") {
            let tiers = match flag("--tiers") {
                Some(t) => t
                    .split(',')
                    .map(|s| {
                        crate::scene::Which::parse(s.trim()).ok_or_else(|| format!("--tiers: t0, t1 or t2, not {s}"))
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                None => vec![backend],
            };
            let half = args.iter().any(|a| a == "--half");
            return crate::strip::strip(&bps, &o, &tiers, (n, every), half, close, std::path::Path::new(sp));
        }
        return crate::scene::film(bps, &o, n, every, |k, shot| {
            let cropped = close.map(|(r, z)| shot.crop(r, z));
            let shot = cropped.as_ref().unwrap_or(shot);
            let p = dir.join(format!("{stem}-{k:03}.png"));
            std::fs::write(&p, shot.png()).map_err(|e| format!("{}: {e}", p.display()))?;
            // The frame's mean brightness beside it: a flash, a lamp coming on, found by eye.
            let luma: u64 = shot
                .px
                .iter()
                .map(|&c| u64::from((c >> 16) & 0xff) + u64::from((c >> 8) & 0xff) + u64::from(c & 0xff))
                .sum();
            println!("{} mean {}", p.display(), luma / (3 * shot.px.len().max(1) as u64));
            Ok(())
        });
    }
    let shot = crate::scene::render(bps, &o)?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // `--crop X,Y,W,H` and `--zoom Z`: a close look at part of the frame.
    let shot = match (flag("--crop"), flag("--zoom")) {
        (None, None) => shot,
        (crop, zoom) => {
            let r = crop.map_or(Ok((0, 0, shot.w, shot.h)), |c| {
                let v: Vec<u16> = c.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if v.len() == 4 { Ok((v[0], v[1], v[2], v[3])) } else { Err(format!("--crop X,Y,W,H, not {c}")) }
            })?;
            let z = zoom.map_or(Ok(3), |z| z.parse::<u16>().map_err(|_| format!("--zoom: not a number: {z}")))?;
            shot.crop(r, z)
        }
    };
    std::fs::write(&path, shot.png()).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some((height, field)) = &shot.layers {
        for (what, png) in [("height", height), ("field", field)] {
            let p = path.with_extension(format!("{what}.png"));
            std::fs::write(&p, png).map_err(|e| format!("{}: {e}", p.display()))?;
            println!("{}", p.display());
        }
    }
    println!(
        "{}
{}",
        shot.line,
        path.display()
    );
    Ok(())
}
