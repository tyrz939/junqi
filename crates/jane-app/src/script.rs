//! `--script`: a few inputs at given ticks, for screenshots and smoke runs without hands.
//!
//! ```text
//! tick 60 key E; tick 90 down W; tick 150 up W; tick 200 click 384 300; tick 210 move 100 100;
//! tick 220 rclick 40 40; tick 230 type Tess; tick 240 shot sheets/ui-bag.png
//! ```
//!
//! `key` presses and releases a key by its cap name (`E`, `Tab`, `Esc`, `F2`, `Space`, `1`);
//! `down` and `up` hold and let go; `click`, `rclick` and `move` are canvas px; `type` enters
//! text as the keyboard would; `shot` writes the canvas as a PNG; `bot reader` (or `rusher`)
//! hands the seat to a headless player from `jane-bot` until `bot off` (or, with `bot talk`, a
//! reader until the first conversation opens), to reach a place worth a look. Ticks count from the app's start, title screen included.

use jane_present::input::key_code;

/// One step of a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Key(u16),
    Down(u16),
    Up(u16),
    Click(i32, i32),
    RightClick(i32, i32),
    Move(i32, i32),
    Type(String),
    Shot(String),
    /// A headless player takes the seat (`reader`, `rusher`), or gives it back (`off`).
    Bot(String),
}

/// A parsed script: steps by tick, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    pub steps: Vec<(u64, Step)>,
    next: usize,
}

impl Script {
    pub fn parse(s: &str) -> Result<Script, String> {
        let mut steps = Vec::new();
        for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let mut w = part.split_whitespace();
            let bad = || format!("script: cannot read \"{part}\"");
            if w.next() != Some("tick") {
                return Err(bad());
            }
            let t: u64 = w.next().and_then(|n| n.parse().ok()).ok_or_else(bad)?;
            let verb = w.next().ok_or_else(bad)?;
            let rest: Vec<&str> = w.collect();
            let key = |i: usize| rest.get(i).and_then(|k| key_code(k)).ok_or_else(bad);
            let num = |i: usize| rest.get(i).and_then(|n| n.parse::<i32>().ok()).ok_or_else(bad);
            let step = match verb {
                "key" => Step::Key(key(0)?),
                "down" => Step::Down(key(0)?),
                "up" => Step::Up(key(0)?),
                "click" => Step::Click(num(0)?, num(1)?),
                "rclick" => Step::RightClick(num(0)?, num(1)?),
                "move" => Step::Move(num(0)?, num(1)?),
                "type" => Step::Type(rest.join(" ")),
                "shot" => Step::Shot(rest.first().ok_or_else(bad)?.to_string()),
                "bot" => match rest.first().copied() {
                    Some(m @ ("reader" | "rusher" | "off" | "talk")) => Step::Bot(m.to_string()),
                    _ => return Err(bad()),
                },
                _ => return Err(bad()),
            };
            steps.push((t, step));
        }
        steps.sort_by_key(|s| s.0);
        Ok(Script { steps, next: 0 })
    }

    /// The steps due at or before `tick` not yet taken.
    pub fn due(&mut self, tick: u64) -> Vec<Step> {
        let mut out = Vec::new();
        while let Some((t, s)) = self.steps.get(self.next) {
            if *t > tick {
                break;
            }
            out.push(s.clone());
            self.next += 1;
        }
        out
    }

    #[allow(dead_code)] // the tests read it
    pub fn done(&self) -> bool {
        self.next >= self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::input::sc;

    #[test]
    fn a_script_reads_and_runs_in_tick_order() {
        let mut s =
            Script::parse("tick 120 key Tab; tick 60 key E ; tick 61 click 10 20; tick 70 type Tess Mary").unwrap();
        assert_eq!(s.due(59), vec![]);
        assert_eq!(s.due(61), vec![Step::Key(sc::E), Step::Click(10, 20)]);
        assert_eq!(s.due(100), vec![Step::Type("Tess Mary".into())]);
        assert_eq!(s.due(500), vec![Step::Key(sc::TAB)]);
        assert!(s.done());
        assert!(Script::parse("tick x key E").is_err());
        assert!(Script::parse("tick 1 key Nope").is_err());
        assert!(Script::parse("tick 1 dance").is_err());
    }
}
