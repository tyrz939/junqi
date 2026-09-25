//! `jane`: the headless tool. Every subcommand is a function of its arguments and `/data`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

mod view;

const USAGE: &str = "usage: jane <command> [options]

commands:
  check [--data DIR]                  compile /data and list every error and warning (no codegen)
  view [--seeds A..B | --seed N] [--out DIR]
                                      draw each seed's county as a PNG (today: the skeleton's land)
  help                                this text";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => check(&args[1..]),
        Some("view") => match view::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane view: {e}");
                ExitCode::FAILURE
            }
        },
        Some("help" | "-h" | "--help") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("jane: unknown command \"{other}\"\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The data dir: `--data DIR`, else `data/` under the workspace root.
fn data_dir(args: &[String]) -> PathBuf {
    if let Some(i) = args.iter().position(|a| a == "--data") {
        if let Some(d) = args.get(i + 1) {
            return PathBuf::from(d);
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn check(args: &[String]) -> ExitCode {
    let dir = data_dir(args);
    let t0 = Instant::now();
    let built = jane_schema::compile::build(&dir);
    let ms = t0.elapsed().as_millis();
    for d in &built.diag.errors {
        println!("error: {d}");
    }
    for d in &built.diag.warnings {
        println!("warning: {d}");
    }
    match built.catalog {
        Some(c) => {
            println!(
                "ok: {} in {ms} ms: {} names, {} texts, {} sprites, {} lists; content hash {:016x}; {} warning(s)",
                dir.display(),
                c.names.len(),
                c.texts.len(),
                c.sprites.len(),
                c.lists.len(),
                c.content_hash,
                built.diag.warnings.len()
            );
            ExitCode::SUCCESS
        }
        None => {
            println!("{} error(s), {} warning(s) in {ms} ms", built.diag.errors.len(), built.diag.warnings.len());
            ExitCode::FAILURE
        }
    }
}
