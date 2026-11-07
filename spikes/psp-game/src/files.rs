//! Files: a file open for reading (opened again across a sleep, PORT.md §13.13), the baked
//! zones' directory, the saves and settings on the Memory Stick (`Stick`), writes and appends,
//! the script's BMP shots and the dashcam's capture files.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt::Write;
use core::sync::atomic::{AtomicPtr, Ordering};

use alloc::format;

use crate::shell::{Saves, Shell};
use jane_core::ZoneId;
use jane_render_psp::capture;
use jane_render_psp::ge::Ge;
use jane_sim::{Seat, Sim};

use crate::platform::now_us;
use crate::Line;
use psp::sys;

// ---------------------------------------------------------------- files

/// A file, open for reading, and its path: a read that fails is tried once more on the file
/// opened again (a Memory Stick handle goes stale across a sleep: the owner's PSP-1000 lost
/// its text after a resume, PORT.md §13.13).
pub(crate) struct File {
    pub(crate) fd: Cell<sys::SceUid>,
    path: Vec<u8>,
    /// Times it was opened again.
    pub(crate) reopens: Cell<u32>,
}

fn open_fd(z: &[u8]) -> Option<sys::SceUid> {
    // SAFETY: a NUL-terminated path that outlives the call.
    let fd = unsafe { sys::sceIoOpen(z.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    (fd.0 >= 0).then_some(fd)
}

impl File {
    pub(crate) fn open(path: &str) -> Option<File> {
        let mut z = Vec::with_capacity(path.len() + 1);
        z.extend_from_slice(path.as_bytes());
        z.push(0);
        let fd = open_fd(&z)?;
        Some(File { fd: Cell::new(fd), path: z, reopens: Cell::new(0) })
    }

    fn size(&self) -> u32 {
        let fd = self.fd.get();
        // SAFETY: an open fd.
        unsafe {
            let n = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
            sys::sceIoLseek(fd, 0, sys::IoWhence::Set);
            n as u32
        }
    }

    /// The file closed and opened again (after a resume, or a read that failed).
    pub(crate) fn reopen(&self) -> bool {
        // SAFETY: our fd, closed once; a stale one's close fails harmlessly.
        unsafe { sys::sceIoClose(self.fd.get()) };
        self.reopens.set(self.reopens.get() + 1);
        match open_fd(&self.path) {
            Some(fd) => {
                self.fd.set(fd);
                true
            }
            None => false,
        }
    }

    /// Fills `buf` from byte `at`, the whole of it; on a failed or short read the file is
    /// opened again and the read tried once more.
    pub(crate) fn read_at(&self, at: u32, buf: &mut [u8]) -> bool {
        self.read_once(at, buf) || (self.reopen() && self.read_once(at, buf))
    }

    fn read_once(&self, at: u32, buf: &mut [u8]) -> bool {
        let fd = self.fd.get();
        // SAFETY: an open fd; `buf` is ours for the call.
        unsafe {
            if sys::sceIoLseek(fd, i64::from(at), sys::IoWhence::Set) != i64::from(at) {
                return false;
            }
            let mut done = 0usize;
            while done < buf.len() {
                let n = sys::sceIoRead(fd, buf[done..].as_mut_ptr().cast(), (buf.len() - done) as u32);
                if n <= 0 {
                    return false;
                }
                done += n as usize;
            }
            true
        }
    }

    pub(crate) fn read_all(&self) -> Option<Vec<u8>> {
        let mut v = alloc::vec![0u8; self.size() as usize];
        self.read_at(0, &mut v).then_some(v)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // SAFETY: an open fd, closed once.
        unsafe { sys::sceIoClose(self.fd.get()) };
    }
}

/// The directory the baked zones lie in (the pack's): set once, never freed.
pub(crate) static BAKED_DIR: AtomicPtr<String> = AtomicPtr::new(core::ptr::null_mut());

/// A baked zone's file, whole, from beside the pack (`jane_sim::baked`'s reader).
pub(crate) fn read_baked(zone: ZoneId) -> Option<Vec<u8>> {
    let p = BAKED_DIR.load(Ordering::Acquire);
    if p.is_null() {
        return None;
    }
    // SAFETY: stored once from a leaked box, never freed or written again.
    let dir = unsafe { &*p };
    File::open(&format!("{dir}{}", jane_sim::baked::file_name(zone))).and_then(|f| f.read_all())
}

/// The first of `dirs` holding `name`.
pub(crate) fn find(dirs: &[String], name: &str) -> Option<(String, File)> {
    for d in dirs {
        let mut p = d.clone();
        p.push_str(name);
        if let Some(f) = File::open(&p) {
            return Some((p, f));
        }
    }
    None
}

// ---------------------------------------------------------------- saves and shots

/// Saves on the Memory Stick (PORT.md §13.13): `ms0:/PSP/SAVEDATA/JANE00001/slotN.jane`, the
/// sim's bytes as the PC writes them, and `slotN.txt`, the note. Written to a temporary file, then
/// renamed over the slot, so a power cut mid-write leaves the old save.
pub(crate) struct Stick {
    pub(crate) dir: String,
}

/// The player's settings beside the saves (`jane_present::pad_psp::Settings`).
pub(crate) const SETTINGS: &str = "settings.txt";

pub(crate) fn cpath(s: &str) -> Vec<u8> {
    let mut z = Vec::with_capacity(s.len() + 1);
    z.extend_from_slice(s.as_bytes());
    z.push(0);
    z
}

/// `bytes` added to the end of `path` (made if missing); its length after, or none.
pub(crate) fn append_file(path: &str, bytes: &[u8]) -> Option<u32> {
    let z = cpath(path);
    // SAFETY: a NUL-terminated path; `bytes` outlives the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            z.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 < 0 {
            return None;
        }
        let n = sys::sceIoWrite(fd, bytes.as_ptr().cast(), bytes.len());
        let len = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
        sys::sceIoClose(fd);
        (n >= 0).then_some(len as u32)
    }
}

pub(crate) fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    let z = cpath(path);
    // SAFETY: a NUL-terminated path; `bytes` outlives the calls.
    unsafe {
        let fd = sys::sceIoOpen(
            z.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 < 0 {
            return Err(format!("cannot write {path} ({:x})", fd.0));
        }
        let n = sys::sceIoWrite(fd, bytes.as_ptr().cast(), bytes.len());
        sys::sceIoClose(fd);
        if n < 0 || n as usize != bytes.len() {
            return Err(format!("{path}: short write"));
        }
    }
    Ok(())
}

impl Stick {
    pub(crate) fn new() -> Stick {
        for d in ["ms0:/PSP", "ms0:/PSP/SAVEDATA", "ms0:/PSP/SAVEDATA/JANE00001"] {
            let z = cpath(d);
            // SAFETY: a NUL-terminated path; an existing directory is an error we ignore.
            unsafe { sys::sceIoMkdir(z.as_ptr(), 0o777) };
        }
        Stick { dir: String::from("ms0:/PSP/SAVEDATA/JANE00001/") }
    }

    pub(crate) fn put(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (path, tmp) = (format!("{}{name}", self.dir), format!("{}{name}.tmp", self.dir));
        write_file(&tmp, bytes)?;
        let (zt, zp) = (cpath(&tmp), cpath(&path));
        // SAFETY: NUL-terminated paths.
        unsafe {
            sys::sceIoRemove(zp.as_ptr());
            if sys::sceIoRename(zt.as_ptr(), zp.as_ptr()) < 0 {
                return Err(format!("cannot rename {tmp}"));
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, name: &str) -> Option<Vec<u8>> {
        File::open(&format!("{}{name}", self.dir)).and_then(|f| f.read_all())
    }
}

impl Saves for Stick {
    fn read(&mut self, n: u8) -> Option<Vec<u8>> {
        self.get(&format!("slot{}.jane", n + 1))
    }
    fn write(&mut self, n: u8, bytes: &[u8]) -> Result<(), String> {
        let r = self.put(&format!("slot{}.jane", n + 1), bytes);
        say!("GAME save slot={} bytes={} {:?}", n + 1, bytes.len(), r);
        r
    }
    fn read_note(&mut self, n: u8) -> Option<String> {
        self.get(&format!("slot{}.txt", n + 1)).and_then(|b| String::from_utf8(b).ok())
    }
    fn write_note(&mut self, n: u8, note: &str) -> Result<(), String> {
        self.put(&format!("slot{}.txt", n + 1), note.as_bytes())
    }
}

/// The screen as a BMP at `path` (a script's shot).
pub(crate) fn shot(ge: &Ge, path: &str) {
    let px = ge.shown();
    let (w, h) = (480usize, 272usize);
    let mut b = Vec::with_capacity(54 + w * h * 3);
    let size = (54 + w * h * 3) as u32;
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0]);
    b.extend_from_slice(&(w as u32).to_le_bytes());
    b.extend_from_slice(&(h as u32).to_le_bytes());
    b.extend_from_slice(&[1, 0, 24, 0, 0, 0, 0, 0]);
    b.extend_from_slice(&((w * h * 3) as u32).to_le_bytes());
    b.extend_from_slice(&[0; 16]);
    for y in (0..h).rev() {
        for x in 0..w {
            let c = px[y * 512 + x];
            b.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
        }
    }
    say!("GAME shot {path} {:?}", write_file(path, &b));
}

/// The dashcam's frames (ten seconds at 60, more at less; 90 words x 4 = 360 B a frame,
/// 600 frames = 216000 B (211 KiB)), and how long after L + R + START it is saved.
pub(crate) const DASH_FRAMES: usize = 600;
pub(crate) const CAPTURE_AFTER_US: u32 = 5_000_000;

/// The capture's header: the build, the settings, where she is and the clocks.
pub(crate) fn capture_header(shell: &Shell, sim: Option<&Sim>, script: Option<&str>) -> String {
    use core::fmt::Write as _;
    let mut h = String::with_capacity(512);
    let _ = writeln!(h, "commit {}", env!("JANE_COMMIT"));
    let _ = writeln!(h, "source_stamp {:016x}", jane_world::SOURCE_STAMP);
    // SAFETY: plain syscalls.
    let (cpu, bus) = unsafe { (sys::scePowerGetCpuClockFrequencyInt(), sys::scePowerGetBusClockFrequencyInt()) };
    let _ = writeln!(h, "clock cpu {cpu} bus {bus}");
    let _ = writeln!(h, "time_us {}", now_us());
    for l in shell.settings().write().lines() {
        let _ = writeln!(h, "setting {l}");
    }
    if let Some(sim) = sim {
        let _ = writeln!(h, "seed {}", sim.state().seed);
        let _ = writeln!(h, "state_hash {:016x}", sim.hash());
        if let Some(v) = sim.view(Seat(0)) {
            let (clock, day) = v.clock();
            let _ = writeln!(h, "zone {}", v.zone().name());
            if let Some(u) = v.unit(v.me().unit) {
                let (x, y) = u.pos.cell();
                let _ = writeln!(h, "cell {x} {y}");
            }
            let mut when = String::new();
            jane_present::text::clock(clock, &mut when);
            let _ = writeln!(h, "game_time day {} {when}", day + 1);
            let _ = writeln!(h, "weather {:?}", v.weather().kind);
        }
    }
    if let Some(t) = script {
        let _ = writeln!(h, "script {}", t.lines().next().unwrap_or(""));
    }
    h
}

/// The dashcam's frames to `capture-<n>.bin` (the first number not taken), and the banner says
/// so.
pub(crate) fn save_capture(rec: &capture::Recorder, header: &str, stick: &Stick, shell: &mut Shell) {
    let bytes = rec.encode(header);
    let n = (0..1000u32).find(|n| !file_exists(&format!("{}capture-{n}.bin", stick.dir))).unwrap_or(999);
    let path = format!("{}capture-{n}.bin", stick.dir);
    let r = write_file(&path, &bytes);
    say!("GAME capture saved {path} frames={} bytes={} {r:?}", rec.frames(), bytes.len());
    shell.banner = if r.is_ok() { format!("Capture saved {n}") } else { String::from("Capture not saved") };
}

/// Whether a file is there.
pub(crate) fn file_exists(path: &str) -> bool {
    open_fd(&cpath(path)).is_some_and(|fd| {
        // SAFETY: our handle, closed once.
        unsafe { sys::sceIoClose(fd) };
        true
    })
}
