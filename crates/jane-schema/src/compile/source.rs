//! Reading `/data`: one base file per table plus fragments beside it, merged in path order
//! (PORT.md §5.2). A fragment is the same shape as its base; an id defined twice is an error.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::Value;

use super::diag::Diagnostics;

/// One row of a keyed table, and where it came from.
#[derive(Clone, Debug)]
pub struct Row {
    /// Relative to the data dir, `/`-separated: `items/town.json`.
    pub file: String,
    pub value: Value,
}

/// All of `/data`, parsed, not yet typed.
#[derive(Debug, Default)]
pub struct Source {
    pub root: PathBuf,
    /// Every JSON file by relative path, in path order.
    files: IndexMap<String, Value>,
    /// Every non-JSON file (`rooms/*/*.room`, `chunks/*.chunk`) by relative path, in path order.
    texts: IndexMap<String, String>,
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, root, out)?;
        } else {
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push((rel, p));
        }
    }
    Ok(())
}

impl Source {
    /// Read and parse every file under `root`. Unparseable JSON is an error per file.
    pub fn read(root: &Path, diag: &mut Diagnostics) -> Source {
        let mut paths = Vec::new();
        if let Err(e) = walk(root, root, &mut paths) {
            diag.error(root.display().to_string(), format!("cannot read the data dir: {e}"));
        }
        paths.sort_by(|a, b| a.0.cmp(&b.0));
        let mut s = Source { root: root.to_path_buf(), ..Source::default() };
        for (rel, p) in paths {
            let text = match std::fs::read_to_string(&p) {
                Ok(t) => t,
                Err(e) => {
                    diag.error(&rel, format!("cannot read: {e}"));
                    continue;
                }
            };
            if rel.ends_with(".json") {
                match serde_json::from_str::<Value>(&text) {
                    Ok(v) => {
                        s.files.insert(rel, v);
                    }
                    Err(e) => diag.error(&rel, format!("not JSON: {e}")),
                }
            } else {
                s.texts.insert(rel, text);
            }
        }
        s
    }

    /// Build a source from in-memory files (tests).
    pub fn from_files(files: &[(&str, &str)]) -> Result<Source, String> {
        let mut s = Source::default();
        for (rel, text) in files {
            if rel.ends_with(".json") {
                let v = serde_json::from_str(text).map_err(|e| format!("{rel}: {e}"))?;
                s.files.insert((*rel).to_owned(), v);
            } else {
                s.texts.insert((*rel).to_owned(), (*text).to_owned());
            }
        }
        s.files.sort_keys();
        s.texts.sort_keys();
        Ok(s)
    }

    /// One JSON file.
    pub fn file(&self, rel: &str) -> Option<&Value> {
        self.files.get(rel)
    }

    /// Every JSON file, in path order: for checks that read every list the data holds, whatever
    /// table it is in (a quest given and handed in by some row).
    pub fn json_files(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.files.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Every JSON file directly in `dir` (not the base file beside it), in path order.
    pub fn files_in(&self, dir: &str) -> impl Iterator<Item = (&str, &Value)> {
        let prefix = format!("{dir}/");
        self.files
            .iter()
            .filter(move |(k, _)| k.starts_with(&prefix) && !k[prefix.len()..].contains('/'))
            .map(|(k, v)| (k.as_str(), v))
    }

    /// Every non-JSON file under `dir` with the extension `ext`, in path order.
    pub fn texts_in(&self, dir: &str, ext: &str) -> impl Iterator<Item = (&str, &str)> {
        let prefix = format!("{dir}/");
        let ext = format!(".{ext}");
        self.texts
            .iter()
            .filter(move |(k, _)| k.starts_with(&prefix) && k.ends_with(&ext))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// A keyed table: `<table>.json` (an object of rows) merged with `<table>/*.json`.
    pub fn table(&self, table: &str, diag: &mut Diagnostics) -> IndexMap<String, Row> {
        self.table_of(table, |_| true, diag)
    }

    /// [`Source::table`] over the files `keep` accepts: two keyspaces sharing a directory (the
    /// sprites' looks and the tiles' looks under `looks/`) each merged on its own.
    pub fn table_of(&self, table: &str, keep: impl Fn(&str) -> bool, diag: &mut Diagnostics) -> IndexMap<String, Row> {
        let mut out: IndexMap<String, Row> = IndexMap::new();
        let base = format!("{table}.json");
        let parts = self.files.get(&base).map(|v| (base.as_str(), v)).into_iter().chain(self.files_in(table));
        for (file, v) in parts.filter(|(f, _)| keep(f)) {
            let Some(obj) = v.as_object() else {
                diag.error(file, format!("{table}: expected an object of rows"));
                continue;
            };
            for (id, row) in obj {
                if let Some(prev) = out.get(id) {
                    diag.error(file, format!("{table}: \"{id}\" is defined twice (first in {})", prev.file));
                    continue;
                }
                out.insert(id.clone(), Row { file: file.to_owned(), value: row.clone() });
            }
        }
        out
    }

    /// A list table: `<table>.json` (an array of rows) followed by `<table>/*.json` in path order.
    pub fn list(&self, table: &str, diag: &mut Diagnostics) -> Vec<Row> {
        let mut out = Vec::new();
        let base = format!("{table}.json");
        let parts = self.files.get(&base).map(|v| (base.as_str(), v)).into_iter().chain(self.files_in(table));
        for (file, v) in parts {
            let Some(arr) = v.as_array() else {
                diag.error(file, format!("{table}: expected an array of rows"));
                continue;
            };
            out.extend(arr.iter().map(|row| Row { file: file.to_owned(), value: row.clone() }));
        }
        out
    }
}

/// Type a row. The error names the file, the row and serde's reason (an unknown field names itself).
pub fn typed<T: serde::de::DeserializeOwned>(row: &Row, at: &str, diag: &mut Diagnostics) -> Option<T> {
    match serde_json::from_value::<T>(row.value.clone()) {
        Ok(v) => Some(v),
        Err(e) => {
            diag.error(format!("{}: {at}", row.file), e.to_string());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_in_path_order_and_refuses_duplicates() {
        let s = Source::from_files(&[
            ("items.json", r#"{"a": {"n": 1}}"#),
            ("items/z.json", r#"{"c": {"n": 3}}"#),
            ("items/b.json", r#"{"b": {"n": 2}, "a": {"n": 9}}"#),
            ("items/deep/x.json", r#"{"q": {}}"#),
        ])
        .unwrap();
        let mut d = Diagnostics::default();
        let t = s.table("items", &mut d);
        assert_eq!(t.keys().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(t["a"].value["n"], 1);
        assert_eq!(d.errors.len(), 1);
        assert!(d.errors[0].msg.contains("defined twice"));
    }

    #[test]
    fn lists_concatenate() {
        let s = Source::from_files(&[("clock.json", "[1, 2]"), ("clock/a.json", "[3]")]).unwrap();
        let mut d = Diagnostics::default();
        let l = s.list("clock", &mut d);
        assert_eq!(l.iter().map(|r| r.value.as_i64().unwrap()).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(l[2].file, "clock/a.json");
    }
}
