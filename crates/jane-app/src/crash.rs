//! A panic leaves evidence: `crash.log` beside `config.json`, with the message, where, what was
//! being played and a backtrace. A release build aborts on a panic with nothing on screen, so
//! without this a crash in the owner's hands says nothing (the first playtest, September 2026).
//!
//! The log is appended to, never replaced, and the default hook still runs after it (stderr).
//! A release build is stripped, so its backtrace is addresses; the message and the file and line
//! of the panic are the part that is always readable.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// What the app was doing, kept up to date cheaply for the hook to read.
static SEED: AtomicU32 = AtomicU32::new(0);
static TICK: AtomicU64 = AtomicU64::new(0);
static BACKEND: Mutex<String> = Mutex::new(String::new());

/// The log beside the config in `root`.
pub fn log_path(root: &Path) -> PathBuf {
    root.join("crash.log")
}

/// Installs the hook: every panic from here on is appended to `root/crash.log`.
pub fn install(root: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let report = report(&info.to_string(), &bt.to_string());
        let path = log_path(&root);
        let _ = std::fs::create_dir_all(&root);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(report.as_bytes());
            eprintln!("jane-app: the crash is written to {}", path.display());
        }
        default(info);
    }));
}

/// The seat's world, as it changes (a New Game, a load, a join).
pub fn note_seed(seed: u32) {
    SEED.store(seed, Ordering::Relaxed);
}

/// The app's tick, every tick.
pub fn note_tick(tick: u64) {
    TICK.store(tick, Ordering::Relaxed);
}

/// The backend drawing.
pub fn note_backend(name: &str) {
    if let Ok(mut b) = BACKEND.lock() {
        b.clear();
        b.push_str(name);
    }
}

/// One crash's entry.
fn report(panic: &str, backtrace: &str) -> String {
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let backend = BACKEND.lock().map(|b| b.clone()).unwrap_or_default();
    format!(
        "==== jane {} crashed (unix time {when})\nseed {}  app tick {}  backend {backend}\n{panic}\n{backtrace}\n\n",
        env!("CARGO_PKG_VERSION"),
        SEED.load(Ordering::Relaxed),
        TICK.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_is_written_beside_the_config() {
        let dir = std::env::temp_dir().join(format!("jane-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install(dir.clone());
        note_seed(1_781_280_623);
        note_tick(4242);
        note_backend("gl2");
        let r = std::thread::spawn(|| panic!("quads past the buffers")).join();
        assert!(r.is_err());
        let _ = std::panic::take_hook();
        let log = std::fs::read_to_string(log_path(&dir)).expect("the log is written");
        assert!(log.contains("quads past the buffers"), "{log}");
        assert!(log.contains("seed 1781280623  app tick 4242  backend gl2"), "{log}");
        assert!(log.contains("crash.rs"), "the place it panicked: {log}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
