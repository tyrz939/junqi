//! The performance overlay for real hardware (PORT.md §13.13): L + R + SELECT shows (and again
//! turns to its second page, then off), top left in the UI's fine face, the last second's fps and worst frame, each part's ms a frame, the
//! audio thread's share, the clocks in effect, free RAM and its largest block, the VRAM and RAM
//! page caches, the ground waiting for the painter, the zones held and the builder ahead. While
//! it is on, its lines are appended to `perf.txt` in the save folder every two seconds (the file
//! kept under [`LOG_MOST`] bytes) so numbers from a PSP can be sent. Off, it costs a flag test
//! a frame. The second page: the GE's milliseconds by pass (timed by its signals) and the
//! presenter's tick by part.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The parts timed a frame, microseconds: the sim's steps, the presenter's ticks, the
/// presenter's draw, the UI, the lister, the GE list's build (its wait for the GE taken out),
/// the GE's wait, the vblank's.
pub const PARTS: usize = 8;
const NAMES: [&str; PARTS] = ["sim", "tick", "draw", "ui", "list", "ge", "gewait", "vbl"];
/// The presenter's tick parts shown on the second page: the view buffers', then
/// `Present::prof`'s first ten.
pub const TPARTS: usize = 11;
const TNAMES: [&str; TPARTS] =
    ["bufs", "units", "emotes", "paint", "walls", "sky", "fx", "amb", "head", "props", "lights"];
const PASSES: usize = jane_render_psp::capture::pass::N;

/// The log's most bytes: past it nothing more is written (the file stays readable).
pub const LOG_MOST: u32 = 256 * 1024;

/// What the glue knows once a second, beside the frames' timings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Facts {
    pub cpu: i32,
    pub bus: i32,
    /// The audio thread's mixing, microseconds since the window began.
    pub mix_us: u32,
    pub free: u32,
    pub largest: u32,
    /// VRAM page slots filled, of all; uploads (VRAM misses) and RAM page hits and loads in the
    /// window.
    pub slots: (usize, usize),
    pub uploads: u32,
    pub hits: u32,
    pub loads: u32,
    pub load_fails: u32,
    /// Chunks on screen waiting for the painter, and whether a job is out.
    pub waiting: usize,
    pub job_out: bool,
    pub zones: usize,
    pub ahead: &'static str,
    pub resumes: u32,
    pub reopens: u32,
}

#[derive(Debug, Default)]
pub struct Perf {
    pub on: bool,
    /// The page shown: 1 the frame's parts, 2 the GE's passes and the tick's parts.
    pub page: u8,
    pass_sum: [u32; PASSES],
    ge_total: u32,
    tpart_sum: [u32; TPARTS],
    start: u32,
    last: u32,
    frames: u32,
    worst: u32,
    sum: [u32; PARTS],
    /// The lines shown, made at each window's end.
    pub lines: Vec<String>,
    windows: u32,
    pub log_bytes: u32,
}

impl Perf {
    /// Off, the first page, the second, off again; on, a window begins now.
    pub fn toggle(&mut self, now: u32) {
        self.page = (self.page + 1) % 3;
        self.on = self.page != 0;
        self.lines.clear();
        self.reset(now);
    }

    fn reset(&mut self, now: u32) {
        (self.start, self.last, self.frames, self.worst, self.sum) = (now, now, 0, 0, [0; PARTS]);
        (self.pass_sum, self.ge_total, self.tpart_sum) = ([0; PASSES], 0, [0; TPARTS]);
    }

    /// The frame's GE passes and its whole list (microseconds), and its tick's parts.
    pub fn detail(&mut self, passes: &[u32; PASSES], ge_total: u32, tparts: &[u32; TPARTS]) {
        for (s, p) in self.pass_sum.iter_mut().zip(passes) {
            *s += p;
        }
        self.ge_total += ge_total;
        for (s, p) in self.tpart_sum.iter_mut().zip(tparts) {
            *s += p;
        }
    }

    /// One frame's end at `now` and its parts. True when a second has passed: the glue then
    /// hands [`Perf::close`] its facts.
    pub fn frame(&mut self, now: u32, parts: [u32; PARTS]) -> bool {
        self.worst = self.worst.max(now.wrapping_sub(self.last));
        self.last = now;
        self.frames += 1;
        for (s, p) in self.sum.iter_mut().zip(parts) {
            *s += p;
        }
        now.wrapping_sub(self.start) >= 1_000_000
    }

    /// Closes the window: the lines made from it and `f`. Returns them as text for the log
    /// every second window (else `None`).
    pub fn close(&mut self, now: u32, f: &Facts) -> Option<String> {
        let span = now.wrapping_sub(self.start).max(1);
        let n = self.frames.max(1);
        let ms = |us: u32| format!("{}.{}", us / 1000, us % 1000 / 100);
        let fps10 = u64::from(self.frames) * 10_000_000 / u64::from(span);
        let mut parts = String::new();
        let mut lines = Vec::with_capacity(8);
        lines.push(format!("fps {}.{} worst {}ms cpu {} bus {}", fps10 / 10, fps10 % 10, ms(self.worst), f.cpu, f.bus));
        for (k, name) in NAMES.iter().enumerate() {
            if k == 4 {
                lines.push(core::mem::take(&mut parts));
            }
            parts.push_str(&format!("{name} {} ", ms(self.sum[k] / n)));
        }
        lines.push(parts);
        let pct10 = u64::from(f.mix_us) * 1000 / u64::from(span);
        lines.push(format!("audio {}.{}% free {}K big {}K", pct10 / 10, pct10 % 10, f.free / 1024, f.largest / 1024));
        lines.push(format!(
            "vram {}/{} up {} ram hit {} load {} fail {}",
            f.slots.0, f.slots.1, f.uploads, f.hits, f.loads, f.load_fails
        ));
        lines.push(format!(
            "paint wait {}{} zones {} ahead {}",
            f.waiting,
            if f.job_out { " job" } else { "" },
            f.zones,
            f.ahead
        ));
        lines.push(format!("resumes {} reopens {}", f.resumes, f.reopens));
        // The second page: the frame's line, then the GE by pass and the tick by part; the
        // log keeps both.
        let mut page2 = Vec::with_capacity(10);
        page2.push(lines[0].clone());
        page2.push(format!("GE ms/frame by pass (all {})", ms(self.ge_total / n)));
        let mut row = String::new();
        let mut k = 0;
        for (id, name) in jane_render_psp::capture::pass::NAMES.iter().enumerate() {
            let us = self.pass_sum[id] / n;
            if us < 50 {
                continue;
            }
            row.push_str(&format!("{name} {} ", ms(us)));
            k += 1;
            if k % 4 == 0 {
                page2.push(core::mem::take(&mut row));
            }
        }
        if !row.is_empty() {
            page2.push(row);
        }
        page2.push(format!("tick ms/frame (with bufs {})", ms(self.sum[1] / n)));
        let mut row = String::new();
        for (k, name) in TNAMES.iter().enumerate() {
            row.push_str(&format!("{name} {} ", ms(self.tpart_sum[k] / n)));
            if k == 4 {
                page2.push(core::mem::take(&mut row));
            }
        }
        page2.push(row);
        let (show, other) = if self.page == 2 { (page2, lines) } else { (lines, page2) };
        self.lines = show;
        self.windows += 1;
        self.reset(now);
        (self.windows % 2 == 0).then(|| {
            let mut s = format!("t={}s\n", now / 1_000_000);
            for l in self.lines.iter().chain(other.iter().skip(1)) {
                s.push_str(l);
                s.push('\n');
            }
            s
        })
    }
}
