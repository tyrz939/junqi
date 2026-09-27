//! `jane`: the headless tool. Every subcommand is a function of its arguments and `/data`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

mod audio_cmd;
mod bench;
mod bench_sim;
#[path = "gen.rs"]
mod gen_cmd;
mod hash_cmd;
mod layers;
mod play;
mod scene;
mod serve;
mod sheet_cmd;
mod sheet_terrain;
mod snap;
mod strip;
mod ui_sheet;
mod view;

const USAGE: &str = "usage: jane <command> [options]

commands:
  check [--data DIR]                  compile /data and list every error and warning (no codegen)
  view [--seeds A..B | --seed N] [--out DIR] [--threat]
                                      draw each seed's skeleton as a PNG, with a .txt of its sites and checks
                                      (--threat: colour the ground by daytime threat, not biome)
  view --county [--seed N] [--scale S] [--x X --y Y --size N --zoom Z]
                                      draw a built county's cells, and time each stage
  view --dungeon <id|all> [--seeds A..B | --seed N] [--out DIR] [--no-png]
                                      draw each seed's generated dungeon, with its attempts after validation
                                      (the solver and C1 to C12) and build time
  view --interior <house|cellar|arms|church|all> [--seeds A..B | --seed N] [--out DIR]
                                      draw each seed's hand-built interior, with attempts and the solver's verdict
{GEN}
{BENCH}
{SHEET}
{AUDIO}
{HASH}
{SERVE}
  play --model reader|rusher --seed N | replay verify|record|diff   a player model plays; tapes (`jane play --help`)
  help                                this text";

fn usage() -> String {
    USAGE
        .replace("{GEN}", gen_cmd::USAGE)
        .replace("{BENCH}", &format!("{}{}{}", bench::USAGE, bench_sim::USAGE, bench::USAGE_TUNE))
        .replace("{SHEET}", sheet_cmd::USAGE)
        .replace("{HASH}", hash_cmd::USAGE)
        .replace("{SERVE}", serve::USAGE)
        .replace("{AUDIO}", audio_cmd::USAGE)
}

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
        Some("gen") => match gen_cmd::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane gen: {e}");
                ExitCode::FAILURE
            }
        },
        Some("bench") => match bench::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane bench: {e}");
                ExitCode::FAILURE
            }
        },
        Some("hash") => match hash_cmd::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane hash: {e}");
                ExitCode::FAILURE
            }
        },
        Some(c @ ("play" | "replay")) => play::main(c, &args[1..]),
        Some(c @ ("serve" | "join" | "find")) => {
            let r = match c {
                "serve" => serve::serve(&args[1..]),
                "join" => serve::join(&args[1..]),
                _ => serve::find(&args[1..]),
            };
            match r {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("jane {c}: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("audio") => match audio_cmd::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane audio: {e}");
                ExitCode::FAILURE
            }
        },
        Some("sheet") => match sheet_cmd::run(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("jane sheet: {e}");
                ExitCode::FAILURE
            }
        },
        Some("help" | "-h" | "--help") | None => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("jane: unknown command \"{other}\"\n\n{}", usage());
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
