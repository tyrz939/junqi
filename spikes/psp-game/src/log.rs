//! The log's two-second window: the timings and counts gathered over it (`Window`) and the
//! `GAME` lines `log_window` prints from them (fps, the frame's parts, heap, pages, the painter).

use alloc::string::String;
use core::fmt::Write;
use core::sync::atomic::Ordering;

use crate::shell::Scene;
use crate::{diag, loader};
use jane_render_psp::ge::Ge;
use jane_render_psp::{Lister, UiLister};
use jane_sim::Seat;

use crate::game::World;
use crate::platform::{free_mem, HEAP, SMALL_ARENA};
use crate::workers::{JOBS, JOB_US, PAINTER};
use crate::Line;
use psp::sys;

/// Timings and counts over the log's two-second window.
#[derive(Default)]
pub(crate) struct Window {
    pub(crate) frames: u32,
    pub(crate) ticks: u32,
    pub(crate) step: u32,
    pub(crate) step_worst: u32,
    pub(crate) bufs: u32,
    pub(crate) bufs_worst: u32,
    pub(crate) sim: u32,
    pub(crate) sim_worst: u32,
    pub(crate) tick: u32,
    pub(crate) tick_worst: u32,
    pub(crate) draw: u32,
    pub(crate) list: u32,
    pub(crate) ui: u32,
    pub(crate) ge: u32,
    pub(crate) worst: u32,
    pub(crate) wait: u32,
    pub(crate) start: u32,
    /// The pad and the shell's sample; the shell's after-steps and the painter's hand-off; the
    /// GE's fill waited for (inside `ge`); the vblank waited for; the audio thread's mixing.
    pub(crate) pre: u32,
    pub(crate) post: u32,
    pub(crate) sync: u32,
    pub(crate) show: u32,
    pub(crate) mix0: u32,
    /// The window's largest display list and page working set, bytes.
    pub(crate) list_most: u32,
    pub(crate) pages_most: u32,
    pub(crate) post_worst: u32,
    /// In play: flips shown, the vblanks they took, those past the pacing's target (missed), and
    /// the longest flip (`Ge::present`).
    pub(crate) flips: u32,
    pub(crate) vblanks: u32,
    pub(crate) missed: u32,
    pub(crate) flip_max: u32,
    /// Chunks painted as swatches (painted less landed), the most chunks waiting on screen at
    /// once, the extra vblanks the painter was given, its boosts, and lists the guard cut.
    pub(crate) rough: u32,
    pub(crate) waiting_max: u32,
    /// Frames in a row with swatches showing, now (carried into the next window) and at most.
    pub(crate) wait_run: u32,
    pub(crate) wait_run_max: u32,
    /// Frames with a swatch on screen that is mostly flat colour (`Present::chunks_mush`), and
    /// the most in a row as above.
    pub(crate) mush: u32,
    pub(crate) mush_run: u32,
    pub(crate) mush_run_max: u32,
    pub(crate) extra: u32,
    pub(crate) boosts: u32,
    pub(crate) list_full: u32,
    /// Quads, polygons and terrain patches the lister left out for want of room, summed over the
    /// window's frames (`Lister::left_out`).
    pub(crate) quads_dropped: u32,
    pub(crate) polys_skipped: u32,
    pub(crate) patches_skipped: u32,
    /// The play tick at the window's end.
    pub(crate) at_tick: u32,
    /// The sim's phases summed, microseconds (`jane_sim::metrics::Phase`).
    pub(crate) sim_phases: [u32; jane_sim::metrics::PHASES],
}

pub(crate) fn log_window(
    w: &Window,
    span: u32,
    ge: &Ge,
    lister: &Lister,
    ui: &UiLister,
    world: Option<&World>,
    scene: Scene,
    amb: [u32; 4],
) {
    let (maxf, totf) = free_mem();
    let st = ge.stats;
    let per = |us: u32, n: u32| us / n.max(1);
    say!(
        "GAME scene={scene:?} fps={}.{} ticks/s={} step={}us step_worst={}us sim={}us sim_worst={}us tick={}us tick_worst={}us bufs={}us bufs_worst={}us draw={}us ui={}us list={}us ge={}us worst={}us quads={} ui_quads={} ui_misses={} batches={} misses={} pages_ram={} loads={} uploads={} images={} live={} peak={} free={totf} maxfree={maxf}",
        w.frames * 100 / (span / 100_000).max(1) / 10,
        w.frames * 100 / (span / 100_000).max(1) % 10,
        w.ticks * 1000 / (span / 1000).max(1),
        per(w.step, w.ticks),
        w.step_worst,
        per(w.sim, w.ticks),
        w.sim_worst,
        per(w.tick, w.ticks),
        w.tick_worst,
        per(w.bufs, w.ticks),
        w.bufs_worst,
        per(w.draw, w.frames),
        per(w.ui, w.frames),
        per(w.list, w.frames),
        per(w.ge, w.frames),
        w.worst,
        st.quads,
        ui.quads.len(),
        ui.misses,
        st.batches,
        lister.misses,
        st.ram_pages_bytes,
        st.page_loads,
        st.uploads,
        ge.image_bytes(),
        HEAP.live.get(),
        HEAP.peak.get(),
    );
    // SAFETY: plain syscalls.
    let (stack_main, stack_painter) = unsafe {
        (
            sys::sceKernelGetThreadStackFreeSize(sys::SceUid(0)),
            sys::sceKernelGetThreadStackFreeSize(sys::SceUid(PAINTER.load(Ordering::Relaxed))),
        )
    };
    say!(
        "GAME small live={} peak={} pool={SMALL_ARENA} list_most={} pages_most={} stack_free main={stack_main} painter={stack_painter} patches={:?} {:?}",
        HEAP.small_live.get(),
        HEAP.small_peak.get(),
        w.list_most,
        w.pages_most,
        lister.patch_bytes(),
        ge.patch_bytes()
    );
    let mix = jane_audio_psp::psp::MIX_US.load(Ordering::Relaxed).wrapping_sub(w.mix0);
    let f = w.frames.max(1);
    say!(
        "GAME parts missed={} post_worst={}us pre={}us post={}us sync={}us show={}us audio={}us/frame ({}%) lister=[water {} pools {} n {} casters {} blocks {} silh {} sprites {} water2 {} silh_casters {} silh_blocks {} order {} lamp_polys {}] ge=[prep {} plan {} world {} ui {}]",
        w.missed,
        w.post_worst,
        w.pre / f,
        w.post / f,
        w.sync / f,
        w.show / f,
        mix / f,
        mix / (span / 100).max(1),
        lister.prof[0] / f,
        lister.prof[1] / f,
        lister.prof[2] / f,
        lister.prof[3] / f,
        lister.prof[4] / f,
        lister.prof[5] / f,
        lister.prof[6] / f,
        lister.prof[7] / f,
        lister.prof[8] / f,
        lister.prof[9] / f,
        lister.prof[10] / f,
        lister.prof[11] / f,
        ge.parts[0] / f,
        ge.parts[1] / f,
        ge.parts[2] / f,
        ge.parts[3] / f,
    );
    // The window's pacing in play, one line for a walk test to read (`tools/psp-walk-test.py`):
    // flips shown, the vblanks they took, those past the pacing's target (missed), the longest
    // flip; swatches (chunks painted that the painter did not land: the first view's too) and
    // the frames they showed (`wait_frames`) and most at once; the painter's extra vblanks and
    // boosts; lists the guard cut; paint jobs that hit an allocation failure (the run's).
    say!(
        "GAME pace scene={scene:?} tick={} ticks={} flips={} vblanks={} missed={} flip_max={} swatches={} wait_frames={} wait_max={} wait_streak={} mush_frames={} mush_streak={} extra_waits={} boosts={} list_full={} paint_fail={} quads_dropped={} polys_skipped={} patches_skipped={}",
        w.at_tick,
        w.ticks,
        w.flips,
        w.vblanks,
        w.missed,
        w.flip_max,
        w.rough,
        w.wait,
        w.waiting_max,
        w.wait_run_max,
        w.mush,
        w.mush_run_max,
        w.extra,
        w.boosts,
        w.list_full,
        diag::paint_fails(),
        w.quads_dropped,
        w.polys_skipped,
        w.patches_skipped
    );
    if let Some(wd) = world {
        let p = &wd.present.prof;
        let t = w.ticks.max(1);
        say!(
            "GAME tickparts units={} emotes={} props={} lights={} paint={} walls={} sky_atmos={} fx={} ambient={} head={} props_scan={} lights_scan={} amb=[blocks {} flocks {} water {} cap {}] rows_built={} casters={} blocks={} sprites={} parts={}",
            p[0] / t,
            p[1] / t,
            p[8] / t,
            p[9] / t,
            p[2] / t,
            p[3] / t,
            p[4] / t,
            p[5] / t,
            p[6] / t,
            p[7] / t,
            p[10] / t,
            p[11] / t,
            amb[0] / t,
            amb[1] / t,
            amb[2] / t,
            amb[3] / t,
            lister.prof[2],
            wd.present.frame().casters.len(),
            wd.present.frame().blocks.len(),
            wd.present.frame().sprites.len(),
            wd.present.frame().parts.len(),
        );
        // The sim's phases (`jane_sim::Phase`), microseconds a tick.
        let mut line = String::new();
        for (p, us) in jane_sim::Phase::ALL.iter().zip(w.sim_phases) {
            let _ = write!(line, " {}={}", p.name().replace(' ', "_"), us / t);
        }
        say!("GAME simparts{line}");
        say!(
            "GAME jobs n={} wall_us={} frames_waiting={} painted={} landed={}",
            JOBS.load(Ordering::Relaxed),
            JOB_US.load(Ordering::Relaxed) / JOBS.load(Ordering::Relaxed).max(1),
            w.wait,
            wd.present.chunks_painted(),
            wd.present.chunks_landed()
        );
        let ld = |a: &core::sync::atomic::AtomicU32| a.load(Ordering::Relaxed);
        say!(
            "GAME baked landed={} refused={} fails={} reads={} read_us={} decodes={} decode_us={} stand_ins_shown={} ram={}",
            ld(&loader::LANDED),
            ld(&loader::REFUSED),
            ld(&loader::FAILS),
            ld(&loader::READS),
            ld(&loader::READ_US) / ld(&loader::READS).max(1),
            ld(&loader::DECODES),
            ld(&loader::DECODE_US) / ld(&loader::DECODES).max(1),
            wd.present.stand_ins_shown,
            ld(&loader::RAM)
        );
        if let Some(v) = wd.sim.view(Seat(0)) {
            if let Some(u) = v.unit(v.me().unit) {
                let (x, y) = u.pos.cell();
                say!("GAME her zone={} cell=({x}, {y})", v.zone().name());
            }
        }
    }
}
