//! Save slots on disk (PRESENTATION.md §3.2; ARCHITECTURE.md §3.5). The sim encodes and decodes;
//! this file only puts the bytes somewhere and reads a slot's summary from its header without
//! decoding the world.
//!
//! **Where** (decided 2026-09-27): beside the executable when a file called `portable` (any
//! content) sits there, so a game on a stick keeps its saves with it; otherwise the user's own
//! data folder, `%APPDATA%\Jane` on Windows, `~/Library/Application Support/Jane` on a Mac and
//! `$XDG_DATA_HOME/jane` (`~/.local/share/jane`) elsewhere. `--data-dir` overrides both (tests,
//! scripts). `config.json` lives in the same folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use jane_sim::save::read_header;

/// How many slots the lists show.
pub const SLOTS: u8 = 3;

/// The folder saves and the config live in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirs {
    pub root: PathBuf,
}

impl Dirs {
    /// `asked` if given, else beside the exe when it is portable, else the user's data folder.
    pub fn find(asked: Option<&str>) -> Dirs {
        if let Some(a) = asked {
            return Dirs { root: PathBuf::from(a) };
        }
        let beside = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(b) = beside.as_ref().filter(|b| b.join("portable").exists()) {
            return Dirs { root: b.clone() };
        }
        let home = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let root = if cfg!(windows) {
            home("APPDATA").map(|p| p.join("Jane"))
        } else if cfg!(target_os = "macos") {
            home("HOME").map(|p| p.join("Library/Application Support/Jane"))
        } else {
            home("XDG_DATA_HOME").map(|p| p.join("jane")).or_else(|| home("HOME").map(|p| p.join(".local/share/jane")))
        };
        Dirs { root: root.or(beside).unwrap_or_else(|| PathBuf::from(".")) }
    }

    pub fn slot(&self, n: u8) -> PathBuf {
        self.root.join(format!("slot{}.jane", n + 1))
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config.json")
    }
}

/// What a slot shows without loading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotInfo {
    pub summary: jane_sim::Summary,
    pub modified: Option<SystemTime>,
}

/// Slot `n`'s summary; `None` for an empty or unreadable slot (an unreadable slot is an empty
/// slot, §3.2).
pub fn info(dirs: &Dirs, n: u8) -> Option<SlotInfo> {
    let path = dirs.slot(n);
    let bytes = std::fs::read(&path).ok()?;
    let (h, _) = read_header(&bytes).ok()?;
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    Some(SlotInfo { summary: h.summary, modified })
}

/// The most recently written slot.
pub fn latest(dirs: &Dirs) -> Option<u8> {
    (0..SLOTS).filter_map(|n| info(dirs, n).map(|i| (n, i.modified))).max_by_key(|(_, m)| *m).map(|(n, _)| n)
}

/// Writes `bytes` to slot `n`: a temporary file beside it, then a rename, so a crash mid-write
/// never leaves half a save.
pub fn write(dirs: &Dirs, n: u8, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(&dirs.root).map_err(|e| format!("{}: {e}", dirs.root.display()))?;
    let path = dirs.slot(n);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read(dirs: &Dirs, n: u8) -> Result<Vec<u8>, String> {
    let path = dirs.slot(n);
    std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// "just now", "5 min ago", "3 h ago", "2 days ago".
pub fn age(when: Option<SystemTime>) -> String {
    let Some(secs) = when.and_then(|w| SystemTime::now().duration_since(w).ok()).map(|d| d.as_secs()) else {
        return String::new();
    };
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> Dirs {
        let d = std::env::temp_dir().join(format!("jane-saves-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Dirs { root: d }
    }

    #[test]
    fn a_slot_round_trips_and_its_header_reads_without_the_world() {
        let dirs = temp("round");
        assert!(info(&dirs, 0).is_none(), "no folder yet: empty");
        let sim = jane_sim::Sim::new_game(3, "Tess");
        let bytes = sim.save();
        write(&dirs, 1, &bytes).unwrap();
        let got = info(&dirs, 1).expect("slot 2 reads");
        assert_eq!(got.summary, sim.summary());
        assert_eq!(read(&dirs, 1).unwrap(), bytes);
        assert_eq!(latest(&dirs), Some(1));
        // Junk is an empty slot, never an error.
        std::fs::write(dirs.slot(2), b"not a save").unwrap();
        assert!(info(&dirs, 2).is_none());
        let _ = std::fs::remove_dir_all(&dirs.root);
    }

    #[test]
    fn ages_read_as_words() {
        let now = SystemTime::now();
        assert_eq!(age(Some(now)), "just now");
        assert_eq!(age(Some(now - std::time::Duration::from_secs(300))), "5 min ago");
        assert_eq!(age(None), "");
    }

    #[test]
    fn an_asked_folder_wins() {
        assert_eq!(Dirs::find(Some("x/y")).root, PathBuf::from("x/y"));
        assert!(Dirs::find(Some("x")).config().ends_with("config.json"));
    }
}
