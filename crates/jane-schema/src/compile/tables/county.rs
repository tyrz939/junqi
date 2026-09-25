//! Compile the `county` group into [`model::County`].

use crate::compile::ctx::Ctx;
use crate::compile::source::Source;
use crate::model;

pub fn compile(src: &Source, cx: &mut Ctx) -> model::County {
    let _ = (src, cx);
    model::County {}
}
