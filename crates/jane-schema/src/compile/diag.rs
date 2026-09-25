//! Diagnostics: every problem the compile finds, all at once, each naming where it is.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// `data/items/town.json: items.julies_key.use[0]`, as precise as the stage knows.
    pub at: String,
    pub msg: String,
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.msg)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub errors: Vec<Diag>,
    pub warnings: Vec<Diag>,
}

impl Diagnostics {
    pub fn error(&mut self, at: impl Into<String>, msg: impl Into<String>) {
        self.errors.push(Diag { at: at.into(), msg: msg.into() });
    }

    pub fn warn(&mut self, at: impl Into<String>, msg: impl Into<String>) {
        self.warnings.push(Diag { at: at.into(), msg: msg.into() });
    }

    /// `ok` or an error.
    pub fn need(&mut self, ok: bool, at: impl Into<String>, msg: impl Into<String>) {
        if !ok {
            self.error(at, msg);
        }
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for d in &self.errors {
            writeln!(f, "error: {d}")?;
        }
        for d in &self.warnings {
            writeln!(f, "warning: {d}")?;
        }
        write!(f, "{} error(s), {} warning(s)", self.errors.len(), self.warnings.len())
    }
}
