//! Making a `Present` (PRESENTATION.md §1.11): built at a tier, or from the tables pack a console
//! boots from (`tables`), and what it lends the app and the backends to read (its frame, atlas,
//! sky, camera, memory) and the switches they set on it.

use super::*;

impl Present {
    /// A presenter at `tier`, its atlas built.
    pub fn new(tier: Tier) -> Present {
        let mut atlas = Atlas::with_layers(tier > Tier::T0);
        let stand = StandIns::build(&mut atlas);
        let people = People::build(&mut atlas);
        let creatures = Creatures::build(&mut atlas);
        let kit = Props::build(&mut atlas);
        let terrain = Terrain::build(&mut atlas, LRU);
        let atmos = Atmosphere::new(tier, &mut atlas);
        let cues = Cues::new(&mut atlas);
        let ambient = Ambient::build(tier, &mut atlas, &creatures);
        // The night kit's sprites (NIGHT.md §4.3): what a console draws its overlays from.
        let night_art = terrain.kit().map(|k| crate::night::NightArt::build(&mut atlas, k)).unwrap_or_default();
        // The decks' pieces (MAP.md §2.5): after every other world sprite, so none of theirs moves.
        let deck_art = crate::decks::DeckArt::build(&mut atlas);
        // The UI's page goes last, so no world sprite moves when it grows (PRESENTATION.md §3.1).
        let (ui_art, mut ui_page) = crate::ui::UiArt::build(atlas.pages.pages.len() as u8);
        if atlas.lit() {
            let n = ui_page.albedo.len();
            (ui_page.normal, ui_page.emissive, ui_page.height) = (vec![[128, 128]; n], vec![0; n], vec![0; n]);
        }
        atlas.pages.pages.push(ui_page);
        // Packed: the pages' slack goes (PLAY-PLAN.md §7).
        for p in &mut atlas.pages.pages {
            alloc::sync::Arc::make_mut(&mut p.albedo).shrink_to_fit();
            p.emissive.shrink_to_fit();
            p.normal.shrink_to_fit();
            p.height.shrink_to_fit();
            p.glow.shrink_to_fit();
        }
        Present::assemble(
            tier,
            atlas,
            ui_art,
            stand,
            people,
            creatures,
            kit,
            terrain,
            atmos,
            cues,
            ambient,
            (night_art, deck_art),
            (RESERVE, LRU),
        )
    }

    /// The presenter's tables as `JPT1` bytes (`crate::tables`, PORT.md §13.12): the atlas's
    /// sprite table without its px, and each module's table of what it packed. What `jane bake`
    /// writes for a console.
    pub fn tables(&self) -> Vec<u8> {
        use crate::tables::put;
        let mut o = Vec::new();
        o.extend_from_slice(crate::tables::MAGIC);
        o.extend_from_slice(&crate::tables::VERSION.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        let atlas = self.atlas.to_pack_bare();
        o.extend_from_slice(&(atlas.len() as u32).to_le_bytes());
        o.extend_from_slice(&atlas);
        put(&self.stand, &mut o);
        put(&self.people, &mut o);
        put(&self.creatures, &mut o);
        put(&self.kit, &mut o);
        put(&self.terrain.all_flora().to_vec(), &mut o);
        put(self.atmos.art(), &mut o);
        self.cues.put_tables(&mut o);
        self.ambient.put_tables(&mut o);
        put(&self.night_art, &mut o);
        put(&self.deck_art, &mut o);
        put(&self.ui_art, &mut o);
        o
    }

    /// A presenter at `tier` from its tables (`JPT1`, [`tables`](Self::tables)), running no
    /// generator but the terrain painter's own (PORT.md §13.7): what a console boots. Its atlas
    /// holds the sprite table and each page's size, no px; the backend takes the px from its own
    /// pack. Draws the same `Frame` as [`new`](Self::new) at T0.
    pub fn from_tables(tier: Tier, bytes: &[u8]) -> Result<Present, crate::atlas::PackError> {
        Present::from_tables_slots(tier, bytes, RESERVE, LRU, false)
    }

    /// A console's presenter from its tables (PORT.md §13.12): `slots` chunk slots, made now and
    /// never more; chunks painted by jobs ([`set_deferred_paint`](Self::set_deferred_paint));
    /// the chunks' albedo `T8` over a CLUT each (`Frame::t8`).
    pub fn from_tables_console(tier: Tier, bytes: &[u8], slots: usize) -> Result<Present, crate::atlas::PackError> {
        let mut p = Present::from_tables_slots(tier, bytes, slots, slots, true)?;
        p.frame.t8 = true;
        p.terrain.release_flora_px();
        p.terrain.drop_normals();
        p.terrain.stage_jobs();
        p.set_deferred_paint(true);
        p.atmos.console = true;
        Ok(p)
    }

    /// Only the UI page's table from a presenter's tables (`JPT1`): what a console's title and
    /// loading screens draw with before the world, and so before the presenter, is built
    /// (PORT.md §13.13). The tables before it are read and let go.
    pub fn ui_art_from_tables(bytes: &[u8]) -> Result<crate::ui::UiArt, crate::atlas::PackError> {
        use crate::atlas::PackError;
        use crate::tables::get;
        let mut r = crate::atlas::Reader::new(bytes);
        if r.take(4)? != crate::tables::MAGIC {
            return Err(PackError("not a JPT1 pack"));
        }
        if r.u16()? != crate::tables::VERSION {
            return Err(PackError("JPT version"));
        }
        r.u16()?;
        let n = r.len()?;
        r.take(n)?;
        let _: StandIns = get(&mut r)?;
        let _: People = get(&mut r)?;
        let _: Creatures = get(&mut r)?;
        let _: Props = get(&mut r)?;
        let _: Vec<crate::terrain::Flora> = get(&mut r)?;
        let _: crate::atmos::SkyArt = get(&mut r)?;
        let _ = Cues::from_tables(&mut r)?;
        let _ = Ambient::from_tables(Tier::T0, &mut r)?;
        let _: crate::night::NightArt = get(&mut r)?;
        let _: crate::decks::DeckArt = get(&mut r)?;
        let ui_art = get(&mut r)?;
        if r.left() != 0 {
            return Err(PackError("bytes after the tables"));
        }
        Ok(ui_art)
    }

    fn from_tables_slots(
        tier: Tier,
        bytes: &[u8],
        slots: usize,
        most: usize,
        console: bool,
    ) -> Result<Present, crate::atlas::PackError> {
        use crate::atlas::PackError;
        use crate::tables::get;
        let mut r = crate::atlas::Reader::new(bytes);
        if r.take(4)? != crate::tables::MAGIC {
            return Err(PackError("not a JPT1 pack"));
        }
        if r.u16()? != crate::tables::VERSION {
            return Err(PackError("JPT version"));
        }
        r.u16()?;
        let n = r.len()?;
        let atlas = Atlas::from_pack(r.take(n)?)?;
        let stand = get(&mut r)?;
        let people = get(&mut r)?;
        let creatures = get(&mut r)?;
        let kit = get(&mut r)?;
        let mut terrain = Terrain::with_flora(jane_art::terrain::Painter::new(), get(&mut r)?, most);
        // A PC presenter from its tables paints the night as `new`'s does; a console folds it.
        if !console {
            terrain.paint_night();
        }
        let atmos = Atmosphere::with_art(tier, get(&mut r)?);
        let cues = Cues::from_tables(&mut r)?;
        let ambient = Ambient::from_tables(tier, &mut r)?;
        let night_art = get(&mut r)?;
        let deck_art = get(&mut r)?;
        let ui_art = get(&mut r)?;
        if r.left() != 0 {
            return Err(PackError("bytes after the tables"));
        }
        Ok(Present::assemble(
            tier,
            atlas,
            ui_art,
            stand,
            people,
            creatures,
            kit,
            terrain,
            atmos,
            cues,
            ambient,
            (night_art, deck_art),
            (slots, most),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        tier: Tier,
        atlas: Atlas,
        ui_art: crate::ui::UiArt,
        stand: StandIns,
        people: People,
        creatures: Creatures,
        kit: Props,
        terrain: Terrain,
        atmos: Atmosphere,
        cues: Cues,
        ambient: Ambient,
        (night_art, deck_art): (crate::night::NightArt, crate::decks::DeckArt),
        (slots, most): (usize, usize),
    ) -> Present {
        let fx = Fx::new(tier, atmos.features.max_particles);
        // A console's (a fixed slot count): a 480 x 272 view draws a few hundred things, not the
        // PC's thousands, so its lists reserve that (they still grow if a frame wants more).
        let small = slots == most && slots < RESERVE;
        let mut frame = Frame::new(tier);
        let (list, few) = if small {
            let f = &mut frame;
            f.sprites = Vec::with_capacity(CONSOLE_RESERVE);
            f.casters = Vec::with_capacity(CONSOLE_RESERVE / 4);
            f.blocks = Vec::with_capacity(CONSOLE_RESERVE);
            f.ui = Vec::with_capacity(CONSOLE_RESERVE);
            f.water = Vec::with_capacity(CONSOLE_RESERVE / 4);
            (CONSOLE_RESERVE, CONSOLE_RESERVE / 4)
        } else {
            (crate::drawlist::RESERVE, 1024)
        };
        let chunks = ChunkCache::fixed(&mut frame.layers, tier, slots, most, small);
        Present {
            atlas,
            ui_art,
            stand,
            people,
            creatures,
            kit,
            frame,
            tick: 0,
            carrying: Vec::new(),
            carry_beats: Vec::new(),
            sway: [0; 4],
            sway_at: 0,
            canvas: (CANVAS_W, CANVAS_H),
            camera: Camera::default(),
            zone: None,
            zone_cells: (0, 0),
            chunks,
            terrain,
            wants: Vec::with_capacity(64),
            band: Vec::with_capacity(64),
            entered: false,
            units: Vec::with_capacity(256),
            units_next: Vec::with_capacity(256),
            trails: Vec::new(),
            hurt: Vec::with_capacity(64),
            struck: Vec::with_capacity(64),
            props: Vec::with_capacity(few * 2),
            prop_scratch: Vec::with_capacity(few * 2),
            standing: DrawList::with_capacity(list),
            seen: Vec::with_capacity(64),
            ground: DrawList::with_capacity(list / 2),
            lights: Vec::with_capacity(256),
            light_scratch: Vec::with_capacity(few),
            holders: Vec::with_capacity(few),
            sky: sky(12 * jane_core::num::TICKS_PER_HOUR, 0, false, 1000, Region::Lowfields),
            margins: Margins::default(),
            boss_grade: 0,
            turn: None,
            room: crate::turn::Room::Never,
            stage: 0,
            last_stage: 1,
            night_q: 0,
            atmos,
            fx,
            lessons: Lessons::new(tier),
            marks: Vec::with_capacity(16),
            dark: false,
            aimed: None,
            walk_to: None,
            fight: FightOverlay::default(),
            cues,
            hour: 12,
            ambient,
            night_art,
            deck_art,
            deck_spans: Vec::new(),
            deck_laid: Vec::new(),
            deck_scratch: Vec::new(),
            night_cap: crate::night::console_cap(crate::gfx_psp::Graphics::default()),
            night_near: Vec::with_capacity(256),
            lived: Vec::new(),
            emotes: Vec::with_capacity(16),
            emote_marks: Vec::with_capacity(16),
            talk: None,
            walls: Vec::with_capacity(WALLS_KEPT),
            walls_at: 0,
            deferred: false,
            swatched: 0,
            mush: 0,
            samples: None,
            fresh: Vec::new(),
            flat_cells: Vec::new(),
            prop_cache: Vec::new(),
            prop_cache_on: true,
            capped: Vec::new(),
            sampled_this_tick: 0,
            sampled_max_tick: 0,
            stand_in_upgrades: 0,
            stand_ins_ahead: 0,
            county_kept: None,
            county_samples: None,
            renight: false,
            her_was: None,
            view_jumps: 0,
            job_next: None,
            job_out: None,
            job_stale: false,
            clock: None,
            prof: [0; 12],
            prop_marks: Vec::new(),
            marks_next: Vec::new(),
            marks_stale: true,
            unit_marks: Vec::new(),
            unit_marks_next: Vec::new(),
            baked: None,
            baked_landed: 0,
            stand_ins_shown: 0,
        }
    }

    /// This frame's ring under her target, the cast bars and the walk marker (`ui::fight`).
    pub fn fight(&self) -> &FightOverlay {
        &self.fight
    }

    /// The moment under way, and the gifts waiting (what the UI's card, the sound and the app's
    /// hold read).
    pub fn lessons(&self) -> &Lessons {
        &self.lessons
    }

    /// This frame's quest marks over heads, in unit id order (§3.8): what `ui::marks` draws.
    pub fn marks(&self) -> &[QuestMarker] {
        &self.marks
    }

    /// Dark enough that her lantern is lit: the marks glow.
    pub fn dark(&self) -> bool {
        self.dark
    }

    /// The atmosphere (the weather, the fog, the sky): what F2 and the sheet tools read, and
    /// where a sheet holds the weather (`Atmosphere::force`).
    pub fn atmos_mut(&mut self) -> &mut Atmosphere {
        &mut self.atmos
    }

    /// The `Features` rows in force (§1.3).
    pub fn features(&self) -> Features {
        self.atmos.features
    }

    /// Sets the rows, each held to what this presenter's tier draws; they show from the next
    /// tick (the particle pool) or the next frame (the rest).
    pub fn set_features(&mut self, f: Features) {
        let tier = self.frame.tier;
        let mut held = Features::of(tier);
        for k in Features::KEYS {
            if let Some(v) = f.get(k) {
                held.set(tier, k, &v);
            }
        }
        self.atmos.features = held;
    }

    /// The effect pool's parts alive: the effects' and the weather's.
    pub fn fx_count(&self) -> (usize, usize) {
        self.fx.count()
    }

    /// The sky as of the last tick.
    pub fn sky(&self) -> &Sky {
        &self.sky
    }

    /// The pages every backend uploads at boot.
    pub fn atlas(&self) -> &AtlasPages {
        &self.atlas.pages
    }

    /// The mist tile let go of and handed over (a console's GE keeps its own copy; nothing in
    /// the presenter reads it, only a PC renderer's fog does).
    pub fn take_mist(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.atlas.pages.mist)
    }

    /// The whole atlas, its sprite table and bake keys with the pages: what `jane bake` records
    /// (`Atlas::to_pack`, PORT.md §13.4), so the bake and the presenter cannot drift.
    pub fn sprites(&self) -> &Atlas {
        &self.atlas
    }

    /// Lets go of the atlas's px once the backend has them (PLAY-PLAN.md §7): a GPU backend
    /// holds them on the card, and `soft` and `gl2` keep a share of the albedo (`Page::albedo`
    /// is shared, not copied). The page table (sizes, the CLUT, the mist) stays; nothing in play
    /// reads an atlas px.
    pub fn release_atlas(&mut self) {
        for g in &mut self.atlas.pages.pages {
            *g = crate::backend::Page { w: g.w, h: g.h, ..Default::default() };
        }
    }

    /// The UI's page table: what `ui::Ui::new` takes.
    pub fn ui_art(&self) -> &crate::ui::UiArt {
        &self.ui_art
    }

    /// The frame as last drawn (with the `Ui` pass as last finished).
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The frame, for the UI to finish its pass into after [`draw`](Self::draw).
    pub fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    /// Ticks presented since New Game.
    pub fn ticks(&self) -> u32 {
        self.tick
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The presenter's largest heap holdings, bytes by capacity (`jane bench --mem`, PLAY-PLAN.md
    /// §7): the atlas's pages by layer, and the terrain chunk cache's layers.
    pub fn mem(&self) -> Vec<(&'static str, usize)> {
        fn cap<T>(v: &Vec<T>) -> usize {
            v.capacity() * core::mem::size_of::<T>()
        }
        let p = &self.atlas.pages;
        let (mut albedo, mut lit) = (0, 0);
        for g in &p.pages {
            albedo += cap(&*g.albedo) + cap(&g.glow);
            lit += cap(&g.normal) + cap(&g.emissive) + cap(&g.height);
        }
        let chunks: usize = self
            .frame
            .layers
            .iter()
            .map(|l| {
                cap(&l.albedo)
                    + cap(&l.normal)
                    + cap(&l.emissive)
                    + cap(&l.height)
                    + cap(&l.surface)
                    + cap(&l.water)
                    + cap(&l.fence)
                    + cap(&l.glow)
            })
            .sum();
        vec![
            ("atlas px held: albedo (u16 CLUT)", albedo + cap(&p.clut) + cap(&p.mist)),
            ("atlas px held: lit layers", lit),
            ("terrain chunk cache", chunks),
        ]
    }

    /// A chunk's slot and generation in the cache, if it is painted and fresh (the F3 view).
    pub fn chunk(&self, id: ChunkId) -> Option<(u16, u32)> {
        self.chunks.find(id)
    }

    /// The zone's size in cells, as of the last tick.
    pub fn zone_cells(&self) -> (u32, u32) {
        self.zone_cells
    }

    /// Chunks painted so far (each paint is a new `(id, generation)`).
    pub fn chunks_painted(&self) -> u32 {
        self.chunks.painted
    }

    /// Of them, those the terrain painter landed (the rest were swatches standing in).
    pub fn chunks_landed(&self) -> u32 {
        self.chunks.landed
    }

    /// No rain drawn (a console's Graphics page; the weather itself unchanged).
    pub fn set_rain(&mut self, on: bool) {
        self.fx.rain_off = !on;
    }

    /// A console's GE fence (PORT.md §13.13): the chunk slots (a bit each) the display list
    /// last sent reads, and what waits for it to finish; a slot in it is painted, or the slots
    /// let go, only after `wait`. Cleared when waited for.
    pub fn set_chunk_fence(&mut self, mask: u32, wait: fn()) {
        self.chunks.fence = mask;
        self.chunks.on_fence = Some(wait);
    }

    /// Times a chunk's paint waited for the GE.
    pub fn fence_waits(&self) -> u32 {
        self.chunks.fence_waits
    }

    /// Units and props kept this tick (in and round the view).
    pub fn seen(&self) -> (usize, usize) {
        (self.units.len(), self.props.len())
    }

    /// Tells the presenter the canvas before the first frame (else it frames for 768 x 432 until
    /// the first `draw`).
    pub fn set_canvas(&mut self, canvas: (u16, u16)) {
        self.canvas = canvas;
    }

    /// Paints terrain chunks away from the tick (PORT.md §13.12): the tick paints none but the
    /// county's first view, and [`take_paint_job`](Self::take_paint_job) hands out the chunk most
    /// wanted, to be run anywhere and given back to [`land`](Self::land). A chunk not yet
    /// painted shows its swatches meanwhile. Off (the PC's way) unless a console turns it on.
    pub fn set_deferred_paint(&mut self, on: bool) {
        self.deferred = on;
        self.samples = on.then(stand_in::Samples::new);
        self.fresh = Vec::with_capacity(8);
    }
}
