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
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

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
        // The folder keeps the game's old name, so saves made before it was The Bell at Nine are
        // still found (PORT.md §13.13).
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

    /// Slot `n`'s note: what the picker shows that the save's header does not hold.
    pub fn meta(&self, n: u8) -> PathBuf {
        self.root.join(format!("slot{}.meta.json", n + 1))
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
    /// The note written beside it, when it is there and still names this save.
    pub meta: Option<SlotMeta>,
}

/// A slot's note, `slotN.meta.json` beside `slotN.jane` (decided 2026-10-01): the place's own
/// name, the minute, the story's step and the real time it was written, read off the view as
/// the save is made. The save itself is the sim's and is untouched (`SAVE_VERSION` stays); a
/// slot without a note, or whose note names another save, shows what its header holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotMeta {
    /// The header of the save this describes: a note left over from another save is ignored.
    pub summary: Option<jane_sim::Summary>,
    /// "The Lowfields", in the zone's region.
    pub place: String,
    /// From 1, as the HUD says it.
    pub day: u32,
    /// "21:14".
    pub clock: String,
    pub night: bool,
    /// The first quest the tracker shows, and its open step.
    pub quest: String,
    pub step: String,
    /// Seconds since 1970, real time.
    pub saved_unix: u64,
    /// The tracker's choice for this seat, as quest content ids: what it tracks, and every quest
    /// of the log it had seen (so one untracked stays so). Absent in an older note: the defaults.
    #[serde(default)]
    pub tracked: Option<Vec<String>>,
    #[serde(default)]
    pub seen: Vec<String>,
    /// The map's ink and her pins, this seat's (`jane_present::memory`). Absent in an older note:
    /// a blank map.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub map: Vec<MapRow>,
}

/// One mark of the map's ink, or one pin, as the note keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRow {
    /// The zone's content id ("county").
    pub zone: String,
    pub x: i32,
    pub y: i32,
    /// "fire", "made", "cold", "sign", "name" or "pin".
    pub kind: String,
    /// A sign's words, a place's name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// A made fire's burn left, of 255.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub burn: u8,
    /// Learned and not yet inked: on the chart at the next rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde asks for a reference
fn is_zero(v: &u8) -> bool {
    *v == 0
}

/// The map's memory as the note's rows.
pub fn map_rows(m: &jane_present::memory::MapMemory) -> Vec<MapRow> {
    use jane_present::memory::{FireState, Note};
    let mark = |k: &jane_present::memory::MapMark, pending: bool| {
        let (kind, text, burn) = match &k.note {
            Note::Fire(FireState::Kept) => ("fire", String::new(), 0),
            Note::Fire(FireState::Made { burn }) => ("made", String::new(), *burn),
            Note::Fire(FireState::Cold) => ("cold", String::new(), 0),
            Note::Sign(t) => ("sign", t.clone(), 0),
            Note::Name(t) => ("name", t.clone(), 0),
        };
        MapRow { zone: k.zone.name().into(), x: k.at.0, y: k.at.1, kind: kind.into(), text, burn, pending }
    };
    let mut out: Vec<MapRow> = m.inked.iter().map(|k| mark(k, false)).collect();
    out.extend(m.pending.iter().map(|k| mark(k, true)));
    out.extend(m.pins.iter().map(|p| MapRow {
        zone: p.zone.name().into(),
        x: p.at.0,
        y: p.at.1,
        kind: "pin".into(),
        ..MapRow::default()
    }));
    out
}

/// The note's rows as the map's memory; a row it does not know is passed over.
pub fn map_of(rows: &[MapRow]) -> jane_present::memory::MapMemory {
    use jane_present::memory::{FireState, MapMark, MapMemory, Note, PINS, Pin};
    let mut m = MapMemory::default();
    for r in rows {
        let Some(zone) = jane_core::ZoneId::from_name(&r.zone) else { continue };
        let at = (r.x, r.y);
        let note = match r.kind.as_str() {
            "fire" => Note::Fire(FireState::Kept),
            "made" => Note::Fire(FireState::Made { burn: r.burn }),
            "cold" => Note::Fire(FireState::Cold),
            "sign" => Note::Sign(r.text.clone()),
            "name" => Note::Name(r.text.clone()),
            "pin" => {
                if m.pins.len() < PINS {
                    m.pins.push(Pin { zone, at });
                }
                continue;
            }
            _ => continue,
        };
        let k = MapMark { zone, at, note };
        if r.pending { m.pending.push(k) } else { m.inked.push(k) }
    }
    m
}

impl SlotMeta {
    /// When it was written, real time.
    pub fn saved_at(&self) -> Option<SystemTime> {
        (self.saved_unix > 0).then(|| SystemTime::UNIX_EPOCH + Duration::from_secs(self.saved_unix))
    }
}

/// Now, as [`SlotMeta::saved_unix`] keeps it.
pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Slot `n`'s summary; `None` for an empty or unreadable slot (an unreadable slot is an empty
/// slot, §3.2).
pub fn info(dirs: &Dirs, n: u8) -> Option<SlotInfo> {
    let path = dirs.slot(n);
    let bytes = std::fs::read(&path).ok()?;
    let (h, _) = read_header(&bytes).ok()?;
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let meta = std::fs::read(dirs.meta(n))
        .ok()
        .and_then(|b| serde_json::from_slice::<SlotMeta>(&b).ok())
        .filter(|m| m.summary.as_ref() == Some(&h.summary));
    Some(SlotInfo { summary: h.summary, modified, meta })
}

/// Writes slot `n`'s note beside it (after the save: a note never names a save not written).
pub fn write_meta(dirs: &Dirs, n: u8, meta: &SlotMeta) -> Result<(), String> {
    let path = dirs.meta(n);
    let json = serde_json::to_vec_pretty(meta).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))
}

/// The slot F5 writes: the one last saved to or loaded from, else the first empty one, else the
/// first.
pub fn quick_slot(last: Option<u8>, empty: impl Fn(u8) -> bool) -> u8 {
    last.or_else(|| (0..SLOTS).find(|&n| empty(n))).unwrap_or(0)
}

/// The save card's words: which slot, and whose rest it was when not hers.
pub fn saved_line(n: u8, by: Option<&str>) -> String {
    match by {
        Some(coat) => format!("Saved to slot {} by the {coat} coat", n + 1),
        None => format!("Saved to slot {}", n + 1),
    }
}

/// The most recently written slot.
pub fn latest(dirs: &Dirs) -> Option<u8> {
    (0..SLOTS)
        .filter_map(|n| info(dirs, n).map(|i| (n, i.meta.and_then(|m| m.saved_at()).or(i.modified))))
        .max_by_key(|(_, m)| *m)
        .map(|(n, _)| n)
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
    fn a_slots_note_round_trips_and_a_stale_one_is_ignored() {
        let dirs = temp("meta");
        let sim = jane_sim::Sim::new_game(3, "Tess");
        write(&dirs, 0, &sim.save()).unwrap();
        assert_eq!(info(&dirs, 0).unwrap().meta, None, "a save without its note still reads");
        let meta = SlotMeta {
            summary: Some(sim.summary()),
            place: "The Lowfields".into(),
            day: 3,
            clock: "21:14".into(),
            night: true,
            quest: "A Letter from Julie".into(),
            step: "Auntie Julie's house".into(),
            saved_unix: unix_now(),
            ..SlotMeta::default()
        };
        write_meta(&dirs, 0, &meta).unwrap();
        let got = info(&dirs, 0).unwrap();
        assert_eq!(got.meta.as_ref(), Some(&meta));
        assert!(got.meta.unwrap().saved_at().is_some());
        // A note naming another save (the slot written over since, its note not): not shown.
        let mut other = sim.summary();
        other.day += 1;
        write_meta(&dirs, 0, &SlotMeta { summary: Some(other), ..meta }).unwrap();
        assert_eq!(info(&dirs, 0).unwrap().meta, None, "a stale note is not shown");
        let _ = std::fs::remove_dir_all(&dirs.root);
    }

    #[test]
    fn quick_save_names_the_slot_it_writes() {
        assert_eq!(quick_slot(Some(1), |_| true), 1, "the slot last used");
        assert_eq!(quick_slot(None, |n| n == 2), 2, "else the first empty one");
        assert_eq!(quick_slot(None, |_| false), 0, "else the first");
        assert_eq!(saved_line(quick_slot(Some(1), |_| false), None), "Saved to slot 2");
        assert_eq!(saved_line(0, Some("teal")), "Saved to slot 1 by the teal coat");
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

    #[test]
    fn the_tracker_rides_in_the_slots_note_and_an_old_note_means_the_defaults() {
        let dirs = temp("track");
        let sim = jane_sim::Sim::new_game(3, "Tess");
        write(&dirs, 0, &sim.save()).unwrap();
        let summary = info(&dirs, 0).unwrap().summary;
        let mut t = jane_present::view::Tracking::default();
        let cat = jane_data::catalog();
        let side = cat.story.quest_id("ames_spectacles").unwrap();
        t.sync(&[cat.story.quest_id("the_letter").unwrap(), side]);
        t.toggle(side);
        let (tracked, seen) = t.ids();
        let meta = SlotMeta { summary: Some(summary.clone()), tracked: Some(tracked), seen, ..SlotMeta::default() };
        write_meta(&dirs, 0, &meta).unwrap();
        let got = info(&dirs, 0).unwrap().meta.unwrap();
        let back = jane_present::view::Tracking::from_ids(got.tracked.as_deref().unwrap(), &got.seen);
        assert_eq!(back, t, "round trip");
        assert!(back.is_on(side), "ticked by hand stays so");
        // A note written before the tracker was kept: no choice, so the defaults.
        let old = format!(
            "{{\"summary\": {}, \"place\": \"\", \"day\": 1, \"clock\": \"\", \"night\": false, \"quest\": \"\", \"step\": \"\", \"saved_unix\": 0}}",
            serde_json::to_string(&summary).unwrap()
        );
        std::fs::write(dirs.meta(0), old).unwrap();
        let got = info(&dirs, 0).unwrap().meta.expect("an older note still reads");
        assert_eq!((got.tracked, got.seen.len()), (None, 0));
        let _ = std::fs::remove_dir_all(&dirs.root);
    }

    #[test]
    fn the_maps_ink_and_her_pins_ride_in_the_slots_note() {
        use jane_core::ZoneId;
        use jane_present::memory::{FireState, MapMark, MapMemory, Note};
        let dirs = temp("map");
        let sim = jane_sim::Sim::new_game(3, "Tess");
        write(&dirs, 0, &sim.save()).unwrap();
        let summary = info(&dirs, 0).unwrap().summary;
        let mut m = MapMemory::default();
        m.learn(MapMark { zone: ZoneId::County, at: (10, 12), note: Note::Sign("EAST: GOLD MINE, 300 m.".into()) });
        m.learn(MapMark { zone: ZoneId::County, at: (300, 40), note: Note::Name("The Long Hedge".into()) });
        m.ink(Some((ZoneId::County, (20, 20), FireState::Kept)));
        m.learn(MapMark { zone: ZoneId::Mine, at: (5, 5), note: Note::Sign("MIND THE SHAFT".into()) });
        m.toggle_pin(ZoneId::County, (500, 500));
        m.toggle_pin(ZoneId::County, (900, 100));
        let meta = SlotMeta { summary: Some(summary), map: map_rows(&m), ..SlotMeta::default() };
        write_meta(&dirs, 0, &meta).unwrap();
        let got = info(&dirs, 0).unwrap().meta.unwrap();
        assert_eq!(map_of(&got.map), m, "the ink, what waits for a rest, and the pins come back as they were");
        assert_eq!(map_of(&got.map).pins.len(), 2);
        let _ = std::fs::remove_dir_all(&dirs.root);
    }
}
