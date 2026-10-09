//! The decks (MAP.md §2.5, §6.2): every piece of every kind of deck (`jane_art::deck`) packed once
//! into the atlas (a console's pack holds them as scene sprites), and each whole span's pieces laid
//! at `Depth::Deck`: over everything on the ground under it, under what stands on it.

use alloc::vec::Vec;

use jane_art::deck::{self, Kind, Laid};

use crate::atlas::{Atlas, RefId};

/// Every deck piece as the atlas holds it, in `jane_art::deck::slot` order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeckArt {
    pub refs: Vec<RefId>,
}

impl DeckArt {
    /// Packs every piece of every kind.
    pub fn build(atlas: &mut Atlas) -> DeckArt {
        let refs = deck::all()
            .into_iter()
            .map(|(k, p)| {
                let c = deck::render(k, p);
                let (_, _, ox, oy) = p.frame();
                let top = c.heights().iter().copied().max().unwrap_or(1).max(1);
                atlas.add_canvas(&c, (-ox as i16, -oy as i16), top, |_, _, t| t)
            })
            .collect();
        DeckArt { refs }
    }

    /// The ref of `kind`'s piece `piece`, if packed.
    pub fn get(&self, kind: Kind, piece: deck::Piece) -> Option<RefId> {
        self.refs.get(deck::slot(kind, piece)).copied()
    }
}

crate::tables::tab_struct!(DeckArt { refs });

/// A span as the decks are drawn from it: its rect (cells), its axis, whether it is whole, its
/// kind, its deck's level and how many levels it stands over the ground under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeckSpan {
    pub rect: jane_core::Rect,
    pub along_x: bool,
    pub whole: bool,
    pub kind: Kind,
    pub level: u8,
    pub over: u8,
}

impl DeckSpan {
    /// The cell row just past its south edge: under one running east-west, past its face, two
    /// cells a level it stands over the ground. What it is sorted at among what stands.
    pub fn south(&self) -> i32 {
        self.rect.bottom() + if self.along_x { 2 * i32::from(self.over.max(1)) } else { 0 }
    }
}

/// The pieces of every span in `spans`, each with its kind, its deck's level and its span's
/// south row ([`DeckSpan::south`]), into `out` (cleared first).
pub fn lay(spans: &[DeckSpan], out: &mut Vec<(Laid, Kind, u8, i32)>, scratch: &mut Vec<Laid>) {
    out.clear();
    for s in spans {
        scratch.clear();
        let r = s.rect;
        deck::layout((r.x, r.y, r.w, r.h), s.along_x, s.whole, i32::from(s.over.max(1)), scratch);
        out.extend(scratch.iter().map(|l| (*l, s.kind, s.level, s.south())));
    }
}
