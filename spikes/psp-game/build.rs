//! The commit the build is of, for a capture's header (PORT.md §13.13): `JANE_COMMIT`, the short
//! hash and `+` when the tree had changes; `unknown` without git.

use std::process::Command;

fn main() {
    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let hash = run(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    let dirty = run(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=JANE_COMMIT={hash}{}", if dirty { "+" } else { "" });
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
}
