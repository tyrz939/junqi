//! `jane sheet`: contact sheets from jane-art as PNGs (ART.md §5, PRESENTATION.md §6), and
//! `--bless`, which rewrites jane-art's golden hashes. Today the step-1 sheets: the demo
//! sprites' layers and lighting, the font, the chrome and the palette; and `scene`, one whole
//! frame of a played seed through the presenter and `soft`.

use std::path::{Path, PathBuf};

use jane_art::sheet::{self, Image};
use jane_art::{Font, demo};

pub const USAGE: &str = "  sheet layers <what> [--out DIR]     a sprite's albedo, normal, emissive and height at 4x
  sheet light <what> [--night] [--out DIR]
                                      a sprite lit from eight directions and overhead, with shadows
  sheet font | chrome | palette [--out DIR]
                                      every glyph in every face; the chrome pieces; the palette ramps
  sheet all [--out DIR]               every sheet above, for every sprite
  sheet list                          the sprites <what> can name
  sheet scene [--seed N] [--minutes M | --ticks T] [--model reader|rusher] [--night | --hour H]
              [--wide] [--out PATH.png | --out DIR]
                                      a model plays the seed from New Game (default 1 minute), then one
                                      frame is drawn headless through the presenter and soft; --night
                                      sets the clock to 22:00 first; --wide draws 21:9 (1008 x 432)
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

fn sprite(what: Option<&String>) -> Result<(&str, jane_art::Canvas), String> {
    let what = what.ok_or_else(|| format!("name a sprite: {}", demo::NAMES.join(", ")))?;
    let c = demo::sprite(what).ok_or_else(|| format!("no sprite \"{what}\"; try {}", demo::NAMES.join(", ")))?;
    Ok((what.as_str(), c))
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
            let (name, c) = sprite(args.get(1))?;
            write(&out, &format!("layers-{name}"), &sheet::layers(&c, name, &font))?;
        }
        Some("light") => {
            let (name, c) = sprite(args.get(1))?;
            let suffix = if night { "-night" } else { "" };
            write(&out, &format!("light-{name}{suffix}"), &sheet::lit(&c, name, &font, night))?;
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
        }
        Some("list") => println!("{}", demo::NAMES.join("\n")),
        Some("scene") => scene(args)?,
        Some("terrain") => crate::sheet_terrain::terrain(args, &out, &font)?,
        Some("flora") => crate::sheet_terrain::flora(&out, &font)?,
        Some("county") => crate::sheet_terrain::county(args, &out, &font)?,
        _ => return Err(format!("usage:\n{USAGE}\n{}", crate::sheet_terrain::USAGE)),
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
    let name = format!("scene-{seed}-{ticks}{}-{}", hour.map_or(String::new(), |h| format!("-h{h:02}")), model.name());
    let path = match flag("--out") {
        Some(p) if p.ends_with(".png") => PathBuf::from(p),
        Some(dir) => PathBuf::from(dir).join(format!("{name}.png")),
        None => PathBuf::from("sheets").join(format!("{name}.png")),
    };
    let bps = jane_sim::Blueprints::build(seed).map_err(|e| format!("seed {seed}: {e}"))?;
    let shot = crate::scene::render(bps, &crate::scene::Opts { seed, ticks, model, hour, canvas })?;
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
