//! `Present` (PRESENTATION.md §1.11): `tick()` once per accumulated tick, whether or not the sim
//! ran; `draw(alpha)` pure in the tick and `alpha`.

use jane_sim::event::Event;
use jane_sim::view::View;

use crate::backend::AtlasPages;
use crate::frame::{Frame, Tier};

/// The presenter: its own state, never the sim's.
#[derive(Debug)]
pub struct Present {
    atlas: AtlasPages,
    frame: Frame,
    tick: u32,
}

impl Present {
    /// A presenter at `tier`, its atlas built.
    pub fn new(tier: Tier) -> Present {
        Present { atlas: AtlasPages::default(), frame: Frame::new(tier), tick: 0 }
    }

    /// The pages every backend uploads at boot.
    pub fn atlas(&self) -> &AtlasPages {
        &self.atlas
    }

    /// One tick of presentation: reads the view and this tick's events.
    pub fn tick(&mut self, view: &View<'_>, events: &[Event]) {
        let _ = (view, events);
        self.tick = self.tick.wrapping_add(1);
    }

    /// The frame at `alpha` (0..=255, how far from the last tick to the next) for a canvas of
    /// `canvas` px.
    pub fn draw(&mut self, alpha: u8, canvas: (u16, u16)) -> &Frame {
        let _ = alpha;
        self.frame.canvas = canvas;
        &self.frame
    }
}
