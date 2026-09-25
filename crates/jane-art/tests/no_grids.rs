//! ART.md §1, "no pixel grids in source", mechanically. Over every file in `jane-art/src`:
//!
//! 1. no three or more consecutive string literals of eight or more characters drawn from
//!    `[.a-zA-Z0-9#]` (consecutive: only whitespace and commas between them);
//! 2. no `include_bytes!` (nor `include_str!`);
//! 3. no integer array literal of more than 32 elements, counting the integers of nested arrays
//!    and tuples, outside `palette.rs`, `hash.rs` and the named lookup tables of `canvas.rs`
//!    (Bayer, sphere normals, sine, falloff), which are listed here by name.
//!
//! There is no exception list beyond those names.

use std::path::{Path, PathBuf};

/// Files whose tables are colour and hash constants, not pictures.
const FREE_FILES: [&str; 2] = ["palette.rs", "hash.rs"];
/// `canvas.rs` tables allowed to be long, by the prefix of their const or static name.
const CANVAS_TABLES: [&str; 4] = ["BAYER", "SPHERE", "SINE", "FALLOFF"];
const MAX_INTS: usize = 32;

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// The source with comments and string and char literals blanked (same length, newlines kept),
/// and the string literals themselves with their byte spans.
fn scan(src: &str) -> (String, Vec<(String, usize, usize)>) {
    let b: Vec<char> = src.chars().collect();
    let mut code = String::with_capacity(src.len());
    let mut lits = Vec::new();
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                code.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                code.push(blank(b[i]));
                i += 1;
            }
            code.push_str("  ");
            i += 2;
        } else if c == '"' || (c == 'r' && (next == Some('"') || next == Some('#')) && !prev_ident(&b, i)) {
            // A string, or a raw string r"..." / r#"..."#.
            let start = i;
            let mut hashes = 0;
            let raw = c == 'r';
            if raw {
                i += 1;
                while b.get(i) == Some(&'#') {
                    hashes += 1;
                    i += 1;
                }
                if b.get(i) != Some(&'"') {
                    // `r#ident`: not a string.
                    for &ch in &b[start..i] {
                        code.push(ch);
                    }
                    continue;
                }
            }
            i += 1;
            let mut text = String::new();
            while let Some(&ch) = b.get(i) {
                if !raw && ch == '\\' {
                    text.push(ch);
                    if let Some(&e) = b.get(i + 1) {
                        text.push(e);
                    }
                    i += 2;
                    continue;
                }
                if ch == '"' && (0..hashes).all(|k| b.get(i + 1 + k) == Some(&'#')) {
                    i += 1 + hashes;
                    break;
                }
                text.push(ch);
                i += 1;
            }
            for &ch in &b[start..i] {
                code.push(blank(ch));
            }
            lits.push((text, start, i));
        } else if c == '\'' {
            // A char literal ('x', '\n', '\u{..}', '£'), or a lifetime.
            let end = if next == Some('\\') {
                (i + 2..b.len()).find(|&k| b[k] == '\'').map(|k| k + 1)
            } else if b.get(i + 2) == Some(&'\'') {
                Some(i + 3)
            } else {
                None
            };
            match end {
                Some(e) => {
                    for _ in i..e {
                        code.push(' ');
                    }
                    i = e;
                }
                None => {
                    code.push(c);
                    i += 1;
                }
            }
        } else {
            code.push(c);
            i += 1;
        }
    }
    (code, lits)
}

fn prev_ident(b: &[char], i: usize) -> bool {
    i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_')
}

fn grid_like(s: &str) -> bool {
    s.chars().count() >= 8 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '#')
}

/// Rule 1 over one file: `(line, literal)` of each run of three or more.
fn string_runs(src: &str, lits: &[(String, usize, usize)]) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut run: Vec<&(String, usize, usize)> = Vec::new();
    let flush = |run: &mut Vec<&(String, usize, usize)>, out: &mut Vec<String>| {
        if run.len() >= 3 {
            let line = chars[..run[0].1].iter().filter(|&&c| c == '\n').count() + 1;
            out.push(format!("line {line}: {} string rows like {:?}", run.len(), run[0].0));
        }
        run.clear();
    };
    for lit in lits {
        if !grid_like(&lit.0) {
            flush(&mut run, &mut out);
            continue;
        }
        let joined = run.last().is_some_and(|prev| chars[prev.2..lit.1].iter().all(|&c| c.is_whitespace() || c == ','));
        if !joined {
            flush(&mut run, &mut out);
        }
        run.push(lit);
    }
    flush(&mut run, &mut out);
    out
}

fn is_int(t: &str) -> bool {
    let t = t.trim().trim_start_matches('-');
    let t = ["u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize"]
        .iter()
        .find_map(|s| t.strip_suffix(s))
        .unwrap_or(t);
    let t = t.trim_end_matches('_');
    if let Some(h) = t.strip_prefix("0x") {
        return !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit() || c == '_');
    }
    if let Some(h) = t.strip_prefix("0b") {
        return !h.is_empty() && h.chars().all(|c| c == '0' || c == '1' || c == '_');
    }
    !t.is_empty() && t.starts_with(|c: char| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_digit() || c == '_')
}

/// Split `s` on commas at depth 0.
fn top_level(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out.into_iter().filter(|e| !e.trim().is_empty()).collect()
}

/// The number of integers in an aggregate of integers (`1`, `[1, 2]`, `(1, [2, 3])`), or `None`
/// if anything else is in it.
fn int_leaves(e: &str) -> Option<usize> {
    let e = e.trim();
    let inner = e
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .or_else(|| e.strip_prefix('(').and_then(|x| x.strip_suffix(')')));
    match inner {
        Some(body) => top_level(body).iter().map(|x| int_leaves(x)).sum(),
        None => is_int(e).then_some(1),
    }
}

/// Rule 3 over one file (comments and literals already blanked): `(line, count)` of each long
/// integer array, skipping the tables `allowed` accepts by name.
fn long_arrays(code: &str, allowed: impl Fn(&str) -> bool) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    for (open, _) in code.match_indices('[') {
        let mut depth = 0;
        let Some(close) = (open..bytes.len()).find(|&k| {
            match bytes[k] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            depth == 0
        }) else {
            continue;
        };
        let Some(n) = int_leaves(&code[open..=close]) else { continue };
        if n <= MAX_INTS {
            continue;
        }
        let head = &code[..open];
        if allowed(item_name(head)) {
            continue;
        }
        let line = head.matches('\n').count() + 1;
        out.push(format!("line {line}: an integer array of {n}"));
    }
    out
}

/// The name of the `const` or `static` whose initialiser `head` ends inside, or `""`: the
/// nearest such keyword with no `;` at bracket depth 0 between it and the end of `head`.
fn item_name(head: &str) -> &str {
    let at = ["const ", "static "].iter().filter_map(|k| head.rfind(k).map(|i| i + k.len())).max();
    let Some(at) = at else { return "" };
    let mut depth = 0;
    for c in head[at..].chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ';' if depth == 0 => return "",
            _ => {}
        }
    }
    head[at..].split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("")
}

fn check(name: &str, src: &str) -> Vec<String> {
    let (code, lits) = scan(src);
    let mut bad: Vec<String> = string_runs(src, &lits);
    for m in ["include_bytes!", "include_str!"] {
        if code.contains(m) {
            bad.push(format!("uses {m}"));
        }
    }
    if !FREE_FILES.contains(&name) {
        let canvas = name == "canvas.rs";
        bad.extend(long_arrays(&code, |n| canvas && CANVAS_TABLES.iter().any(|t| n.starts_with(t))));
    }
    bad.into_iter().map(|b| format!("{name}: {b}")).collect()
}

#[test]
fn no_pixel_grids_in_source() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&dir, &mut files);
    assert!(files.len() >= 8, "found only {files:?}");
    let mut bad = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        bad.extend(check(&name, &std::fs::read_to_string(f).unwrap()));
    }
    assert!(bad.is_empty(), "pixel grids in jane-art/src:\n{}", bad.join("\n"));
}

/// The checks catch what they are for.
#[test]
fn the_checks_catch_grids() {
    let rows = "const S: &[&str] = &[\n  \"....##..\",\n  \"...####.\",\n  \"..######\",\n];";
    assert_eq!(check("x.rs", rows).len(), 1, "three string rows");
    assert!(check("x.rs", "const S: [&str; 2] = [\"....##..\", \"...####.\"];").is_empty(), "two are allowed");
    assert!(
        check("x.rs", "let a = (\"cloth_plum\", \"cloth_grey\", \"cloth_blue\");").is_empty(),
        "names are not grids"
    );
    let flat = format!("const A: [u8; 40] = [{}];", ["1"; 40].join(", "));
    assert_eq!(check("x.rs", &flat).len(), 1, "a long flat array");
    let rowed = format!("const G: [[u8; 5]; 8] = [{}];", ["[0, 1, 1, 1, 0]"; 8].join(", "));
    assert!(!check("x.rs", &rowed).is_empty(), "a grid split into short rows");
    let tupled = format!("const T: [(u8, u8); 20] = [{}];", ["(1, 2)"; 20].join(", "));
    assert!(!check("x.rs", &tupled).is_empty(), "a grid as tuples");
    assert!(check("canvas.rs", &flat.replace("const A", "const BAYER8")).is_empty(), "a named canvas table");
    assert!(!check("canvas.rs", &flat).is_empty(), "an unnamed one in canvas.rs");
    assert!(check("palette.rs", &flat).is_empty(), "the palette's tables");
    assert!(check("x.rs", "let v = [0u8; 256]; let w = a[3];").is_empty(), "repeat and index expressions");
    assert_eq!(check("x.rs", "static B: &[u8] = include_bytes!(\"a.bin\");").len(), 1);
    assert!(check("x.rs", "let c = '\"'; let s = \"a, b\"; // \"########\" \"########\" \"########\"").is_empty());
}
