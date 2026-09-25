//! Compile the `story` group into [`model::Story`].

use crate::compile::ctx::Ctx;
use crate::compile::source::Source;
use crate::model;

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Story {
    let _ = (src, cx);
    model::Story {}
}
