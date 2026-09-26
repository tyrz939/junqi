//! The build a peer says it is (ARCHITECTURE.md §7: a join from another build is refused): the
//! package version and the commit it was built from, when git can say.

use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|s| !s.is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let id = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "nogit".to_owned());
    println!("cargo:rustc-env=JANE_BUILD_ID={id}");
    // Rebuilt when the commit moves: the HEAD file, and the branch it names. Only files that
    // exist are named (a missing one would rerun this on every build).
    let mut watch = Vec::new();
    watch.extend(git(&["rev-parse", "--git-path", "HEAD"]));
    if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch.extend(git(&["rev-parse", "--git-path", &r]));
    }
    for w in watch {
        if Path::new(&w).exists() {
            println!("cargo:rerun-if-changed={w}");
        }
    }
}
