//! `sign` (ART.md §2.3).

use super::{Kit, Stand, State};
use crate::canvas::Canvas;

pub(crate) fn draw(c: &mut Canvas, k: &Kit, state: State) -> Option<Stand> {
    let _ = (c, k, state);
    None
}
