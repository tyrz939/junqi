//! Compile the `dungeons` group into [`model::Dungeons`].

use crate::compile::ctx::Ctx;
use crate::compile::source::Source;
use crate::model;

pub fn compile(src: &Source, cx: &mut Ctx) -> model::Dungeons {
    let _ = (src, cx);
    model::Dungeons {}
}
