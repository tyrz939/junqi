//! `config.json` (PRESENTATION.md §4): the player's own settings beside the saves. Anything
//! missing takes its default; anything unreadable is ignored with a line on stderr, never a
//! refusal to start.

use jane_present::input::{
    BINDINGS, Bindings, action, action_name, key_code, key_name, mouse_button, mouse_name, pad_input, pad_name,
};
use jane_sim::input::AssistProfile;
use serde::{Deserialize, Serialize};

use crate::saves::Dirs;

/// One changed binding: an action by its data name, and what it is bound to now, by name
/// (`"E"`, `"Space"`, `"Left click"`, `"B"`). A column left out keeps the compiled one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BindingRow {
    pub action: String,
    pub keys: Option<[String; 2]>,
    pub mouse: Option<String>,
    pub pad: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The name New Game offers.
    pub name: String,
    /// `auto`, `soft` or `wgpu`; the command line wins.
    pub backend: Option<String>,
    /// `off`, `pad` or `mouse`; `None` follows the device.
    pub assist: Option<String>,
    /// The slot last saved or loaded.
    pub last_slot: Option<u8>,
    /// Bindings that differ from `data/bindings.json`.
    pub bindings: Vec<BindingRow>,
    /// Each slot's seed as last written, so a load draws the county forming before the save
    /// has been decoded (the save's header does not carry it).
    pub slot_seeds: Vec<Option<u32>>,
    /// Who this machine is to a host (never shown): a returning guest gets her own body and
    /// bags back by it (PLATFORM.md §8). Made on the first join.
    pub client_token: Option<u64>,
    /// The address last joined, offered again on the Join screen.
    pub last_host: Option<String>,
}

/// A pad or mouse column's "nothing bound here".
const NONE: &str = "none";

impl Config {
    /// The bindings in force: the compiled table with this config's rows laid over it. A row
    /// naming an action, key or button this build does not know is skipped with a line on stderr.
    pub fn bindings(&self) -> Bindings {
        let mut b = Bindings::default();
        for r in &self.bindings {
            let Some(a) = action(&r.action) else {
                eprintln!("jane-app: config.json: no action {}", r.action);
                continue;
            };
            let Some(row) = b.rows.iter_mut().find(|x| x.action == a) else { continue };
            if let Some(keys) = &r.keys {
                for (i, k) in keys.iter().enumerate() {
                    row.keys[i] = if k.is_empty() { 0 } else { key_code(k).unwrap_or(row.keys[i]) };
                }
            }
            if let Some(m) = &r.mouse {
                row.mouse = if m == NONE { None } else { mouse_button(m).or(row.mouse) };
            }
            if let Some(p) = &r.pad {
                row.pad = if p == NONE { None } else { pad_input(p).or(row.pad) };
            }
        }
        b
    }

    /// Keeps the rows of `b` that differ from the compiled table.
    pub fn set_bindings(&mut self, b: &Bindings) {
        self.bindings.clear();
        for row in &b.rows {
            let Some(base) = BINDINGS.iter().find(|x| x.action == row.action) else { continue };
            if base == row {
                continue;
            }
            let key = |k: u16| if k == 0 { String::new() } else { key_name(k).to_owned() };
            self.bindings.push(BindingRow {
                action: action_name(row.action).to_owned(),
                keys: (row.keys != base.keys).then(|| [key(row.keys[0]), key(row.keys[1])]),
                mouse: (row.mouse != base.mouse)
                    .then(|| row.mouse.map_or(NONE.to_owned(), |m| mouse_name(m).to_owned())),
                pad: (row.pad != base.pad).then(|| row.pad.map_or(NONE.to_owned(), |p| pad_name(p).to_owned())),
            });
        }
    }

    /// The aim assist this config asks for; `None` follows the device.
    pub fn assist(&self) -> Option<AssistProfile> {
        match self.assist.as_deref() {
            Some("off") => Some(AssistProfile::Off),
            Some("pad") => Some(AssistProfile::Pad),
            Some("mouse") => Some(AssistProfile::Mouse),
            _ => None,
        }
    }

    pub fn set_assist(&mut self, a: Option<AssistProfile>) {
        self.assist = a.map(|a| {
            match a {
                AssistProfile::Off => "off",
                AssistProfile::Pad => "pad",
                AssistProfile::Mouse => "mouse",
            }
            .to_owned()
        });
    }

    /// Remembers slot `n` holds a game of `seed`.
    pub fn set_slot_seed(&mut self, n: u8, seed: u32) {
        let i = usize::from(n);
        if self.slot_seeds.len() <= i {
            self.slot_seeds.resize(i + 1, None);
        }
        self.slot_seeds[i] = Some(seed);
    }

    pub fn load(dirs: &Dirs) -> Config {
        let path = dirs.config();
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                eprintln!("jane-app: {}: {e}; using defaults", path.display());
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self, dirs: &Dirs) -> Result<(), String> {
        std::fs::create_dir_all(&dirs.root).map_err(|e| format!("{}: {e}", dirs.root.display()))?;
        let s = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(dirs.config(), s).map_err(|e| format!("{}: {e}", dirs.config().display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_are_the_rows_that_differ_and_they_lay_back_over_the_table() {
        let mut b = Bindings::default();
        let use_ = b.rows.iter_mut().find(|r| r.action == jane_present::input::Action::Use).unwrap();
        use_.keys[1] = jane_present::input::sc::SPACE;
        use_.pad = None;
        let mut c = Config::default();
        c.set_bindings(&b);
        assert_eq!(c.bindings.len(), 1);
        assert_eq!(c.bindings[0].action, "use");
        assert_eq!(c.bindings[0].keys, Some(["E".into(), "Space".into()]));
        assert_eq!(c.bindings[0].pad.as_deref(), Some("none"));
        assert_eq!(c.bindings[0].mouse, None);
        let s = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(back.bindings(), b);
        assert_eq!(Config::default().bindings(), Bindings::default());
    }

    #[test]
    fn a_config_round_trips_and_a_partial_one_fills_in() {
        let c = Config {
            name: "Tess".into(),
            last_slot: Some(2),
            bindings: vec![BindingRow {
                action: "use".into(),
                keys: Some(["F".into(), String::new()]),
                ..BindingRow::default()
            }],
            ..Config::default()
        };
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&s).unwrap(), c);
        let partial: Config = serde_json::from_str(r#"{"name":"Ada"}"#).unwrap();
        assert_eq!(partial.name, "Ada");
        assert!(partial.bindings.is_empty());
    }
}
