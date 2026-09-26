//! `jane sheet`: contact sheets from jane-art as PNGs (ART.md §5, PRESENTATION.md §6), and
//! `--bless`, which rewrites jane-art's golden hashes. Today the step-1 sheets: the demo
//! sprites' layers and lighting, the font, the chrome and the palette; from step 2 the people:
//! every look's frames, every seat and variant, and the build-by-hair-by-coat grid; and `scene`,
//! one whole frame of a played seed through the presenter and `soft`.

use std::path::{Path, PathBuf};

use jane_art::sheet::{self, Image};
use jane_art::{Font, demo, looks, person, sheet_person};

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
  sheet silhouettes <id> ... [--out DIR]
                                      looks' standing frames filled black, then as drawn
  sheet units [--out DIR]             every look standing and dead, at 1x and 2x
  sheet person --grid [--out DIR]     every build by every hair and coat
  sheet all [--out DIR]               every sheet above, for every sprite
  sheet list                          the sprites <what> can name
  sheet scene [--seed N] [--minutes M | --ticks T] [--model reader|rusher] [--night | --hour H]
              [--wide] [--backend soft|wgpu] [--out PATH.png | --out DIR]
                                      a model plays the seed from New Game (default 1 minute), then one
                                      frame is drawn headless through the presenter and soft (T0), or
                                      wgpu (T2) with the gpu feature; --night sets the clock to 22:00
                                      first; --wide draws 21:9 (1008 x 432)
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

/// A demo sprite, or a frame of a look (`--frame`, `down` when not given).
fn sprite(what: Option<&String>, args: &[String]) -> Result<(String, jane_art::Canvas), String> {
    let what = what.ok_or_else(|| format!("name a sprite: {}, or a look", demo::NAMES.join(", ")))?;
    if let Some(c) = demo::sprite(what) {
        return Ok((what.clone(), c));
    }
    let frame = args.iter().position(|a| a == "--frame").and_then(|i| args.get(i + 1)).map_or("down", |s| s);
    let id = person::frame_ids()
        .find(|f| f.name() == frame)
        .ok_or_else(|| format!("no frame \"{frame}\"; try down, side_1, up_b, dead"))?;
    let sets = looks::render(what).map_err(|e| format!("{e}; the demo sprites are {}", demo::NAMES.join(", ")))?;
    let c = sets.first().and_then(|r| r.set.frame(id)).ok_or("no such frame")?.clone();
    Ok((format!("{what}-{frame}"), c))
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
        }
        Some("layers") => {
            let (name, c) = sprite(args.get(1), args)?;
            write(&out, &format!("layers-{name}"), &sheet::layers(&c, &name, &font))?;
        }
        Some("light") => {
            let (name, c) = sprite(args.get(1), args)?;
            let suffix = if night { "-night" } else { "" };
            let img = if demo::sprite(args[1].as_str()).is_some() {
                sheet::lit(&c, &name, &font, night)
            } else {
                sheet::lit_upright(&c, &name, &font, night, person::AY)
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
                names.iter().filter_map(|n| person::frame_ids().find(|f| f.name() == n.as_str())).collect()
            };
            let scale =
                args.iter().position(|a| a == "--scale").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(8);
            write(&out, &format!("close-{what}"), &sheet_person::closeup(&sets[0], &ids, &font, scale))?;
        }
        Some("silhouettes") => {
            let mut sets = Vec::new();
            for name in args[1..].iter().take_while(|a| !a.starts_with("--")) {
                sets.extend(looks::render(name)?.into_iter().filter(|r| r.seat == 0 && r.variant == 0));
            }
            write(&out, "silhouettes", &sheet_person::silhouettes(&sets, &font))?;
        }
        Some("units") => write(&out, "units", &sheet_person::units(&looks::all()?, &font))?,
        Some("person") if args.iter().any(|a| a == "--grid") => {
            let (_, jane_data::Look::Person(base)) = looks::find("jane").ok_or("no look for jane")?;
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
        _ => return Err(format!("usage:\n{USAGE}")),
    }
    Ok(())
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
    let hour = match (args.iter().any(|a| a == "--night"), num("--hour")?) {
        (_, Some(h)) => Some(u8::try_from(h % 24).expect("an hour")),
        (true, None) => Some(22),
        (false, None) => None,
    };
    let canvas = if args.iter().any(|a| a == "--wide") { (1008, 432) } else { (768, 432) };
    let backend = crate::scene::Which::parse(flag("--backend").unwrap_or("soft")).ok_or("--backend: soft or wgpu")?;
    let name = format!(
        "scene-{seed}-{ticks}{}-{}-{}",
        hour.map_or(String::new(), |h| format!("-h{h:02}")),
        model.name(),
        backend.name()
    );
    let path = match flag("--out") {
        Some(p) if p.ends_with(".png") => PathBuf::from(p),
        Some(dir) => PathBuf::from(dir).join(format!("{name}.png")),
        None => PathBuf::from("sheets").join(format!("{name}.png")),
    };
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let shot = crate::scene::render(bps, &crate::scene::Opts { seed, ticks, model, hour, canvas, backend })?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, shot.png()).map_err(|e| format!("{}: {e}", path.display()))?;
    println!(
        "{}
{}",
        shot.line,
        path.display()
    );
    Ok(())
}
