//! `config.json` (PRESENTATION.md §4): the player's own settings beside the saves. Anything
//! missing takes its default; anything unreadable is ignored with a line on stderr, never a
//! refusal to start.

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
}

impl Config {
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
