//! `jane bake-world`: the county's source (`data/world/county/`: plan.txt, places.txt, routes.txt)
//! compiled, laid and proved, written to `assets/world/county.bp`. The game reads only that file; a
//! source that does not hold (a place on a face, a route off its road, a proof refused) writes nothing.

use std::path::PathBuf;

pub const USAGE: &str = "  bake-world [--src DIR] [--out FILE] [--check]
                                      compile data/world/county into assets/world/county.bp (proved first;
                                      nothing is written if it does not hold). --check: only say whether
                                      the file is the source baked";

pub fn run(args: &[String]) -> Result<(), String> {
    // From the repository's root, as every tool is run.
    let mut src = PathBuf::from("data/world/county");
    let mut out = PathBuf::from("assets/world/county.bp");
    let mut check = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--src" => src = PathBuf::from(it.next().ok_or("--src DIR")?),
            "--out" => out = PathBuf::from(it.next().ok_or("--out FILE")?),
            "--check" => check = true,
            o => return Err(format!("bake-world: unknown option {o}\n{USAGE}")),
        }
    }
    let read = |f: &str| std::fs::read_to_string(src.join(f)).map_err(|e| format!("{}: {e}", src.join(f).display()));
    let t = std::time::Instant::now();
    let file =
        jane_sim::baked::bake_county(&read("plan.txt")?, &read("places.txt")?, &read("routes.txt")?, &mut |_| {})?;
    let ms = t.elapsed().as_millis();
    let same = std::fs::read(&out).is_ok_and(|old| old == file);
    if check {
        if !same {
            return Err(format!("{} is stale", out.display()));
        }
        println!("{} is its source baked", out.display());
        return Ok(());
    }
    if same {
        println!("{} unchanged ({} bytes, built and proved in {ms} ms)", out.display(), file.len());
        return Ok(());
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&out, &file).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{} written ({} bytes, built and proved in {ms} ms)", out.display(), file.len());
    Ok(())
}
