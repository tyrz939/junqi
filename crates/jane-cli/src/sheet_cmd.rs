//! `jane sheet`: contact sheets from jane-art as PNGs (ART.md §5, PRESENTATION.md §6), and
//! `--bless`, which rewrites jane-art's golden hashes. Today the step-1 sheets: the demo
//! sprites' layers and lighting, the font, the chrome and the palette.

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
        _ => return Err(format!("usage:\n{USAGE}")),
    }
    Ok(())
}
