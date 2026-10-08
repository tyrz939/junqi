//! The PSP's audio thread (PORT.md §13.4): the module's resident part read from beside the
//! program, the tracker mixer made on the game's thread (every byte it will hold allocated there,
//! so the audio thread never touches the game's heap), and a thread above the game's priority that
//! mixes a block into the next of four buffers and hands it to sceAudio, blocking until the
//! hardware takes it. The four buffers are the ring that keeps it underrun-free: while one plays
//! and one waits, the thread mixes the next, so the game's frame never starves it.
//!
//! A scripted run (`capture`) also writes what it played, as a WAV, and every command it applied
//! with its frame, beside the program (`psp-audio.wav`, `psp-cmds.txt`): `jane audio replay`
//! plays the same commands through the PC's synth and the host's tracker to compare.

use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering};

use jane_audio::tracker::{Bank, Cmd, Mixer, PREFIX, wav_header};
use psp::sys;

use crate::{Bus, Host, Ring, Sound};

/// The mixer's rate: sceAudio's sample-rate converter takes it to the hardware's 44.1 kHz.
pub const RATE: u32 = 22_050;
/// Frames a buffer (23 ms at 22.05 kHz), and the buffers in the ring.
const FRAMES: usize = 512;
const NBUF: usize = 4;
/// Above the game's thread (32) and the painter's (48): lower is sooner.
const PRIORITY: i32 = 16;

static RING_: Ring = Ring::new();
static MIXER: AtomicPtr<Mixer> = AtomicPtr::new(core::ptr::null_mut());
static BUFS: AtomicPtr<Out> = AtomicPtr::new(core::ptr::null_mut());
static CAPTURE: AtomicBool = AtomicBool::new(false);
static WAV_FD: AtomicI32 = AtomicI32::new(-1);
static CMD_FD: AtomicI32 = AtomicI32::new(-1);
/// Since the thread started: microseconds spent mixing, blocks mixed, the hardware found empty.
pub static MIX_US: AtomicU32 = AtomicU32::new(0);
pub static BLOCKS: AtomicU32 = AtomicU32::new(0);
pub static UNDERRUNS: AtomicU32 = AtomicU32::new(0);

/// One buffer, aligned for the hardware's DMA.
#[repr(C, align(64))]
struct Out([i16; 2 * FRAMES]);

fn now_us() -> u32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// A line to fd 1 (the headless runner's stdout), from a stack buffer: no heap.
struct Line {
    buf: [u8; 256],
    len: usize,
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let n = b.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&b[..n]);
        self.len += n;
        Ok(())
    }
}

impl Line {
    const fn new() -> Line {
        Line { buf: [0; 256], len: 0 }
    }

    fn to(&self, fd: i32) {
        // SAFETY: the buffer outlives the call.
        unsafe { sys::sceIoWrite(sys::SceUid(fd), self.buf.as_ptr().cast(), self.len) };
    }
}

fn open(path: &str, flags: sys::IoOpenFlags) -> Option<sys::SceUid> {
    let mut z = Vec::with_capacity(path.len() + 1);
    z.extend_from_slice(path.as_bytes());
    z.push(0);
    // SAFETY: a NUL-terminated path that outlives the call.
    let fd = unsafe { sys::sceIoOpen(z.as_ptr(), flags, 0o777) };
    (fd.0 >= 0).then_some(fd)
}

fn read_at(fd: sys::SceUid, at: usize, buf: &mut [u8]) -> bool {
    // SAFETY: an open fd; `buf` is ours for the call.
    unsafe {
        if sys::sceIoLseek(fd, at as i64, sys::IoWhence::Set) != at as i64 {
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
    }
    true
}

/// The game's side on a PSP: the ring, and the module's file for the far effects.
#[derive(Debug)]
pub struct PspHost {
    file: sys::SceUid,
}

impl Host for PspHost {
    fn send(&mut self, c: Cmd) {
        RING_.push(c);
    }

    fn load_far(&mut self, sample: u16, at: usize, len: usize) -> bool {
        let mut frames = alloc::vec![0u8; len];
        if !read_at(self.file, at, &mut frames) {
            return false;
        }
        let m = MIXER.load(Ordering::Acquire);
        if m.is_null() {
            return false;
        }
        // SAFETY: the audio thread runs above this one, so it is never inside the mixer when this
        // thread runs; holding the dispatcher keeps it from waking while the slot is written.
        unsafe {
            let st = sys::sceKernelSuspendDispatchThread();
            let ok = (*m).load_far(sample, &frames);
            sys::sceKernelResumeDispatchThread(st);
            ok
        }
    }
}

/// Starts the sound: reads `jane-psp.jau`'s resident part from the first of `dirs` holding it,
/// makes the mixer for the county's `seed`, starts the audio thread and returns the cue table and
/// its bus for the game to tick. `capture` (a scripted run) writes what it plays beside the
/// program. `None`, with a line on fd 1, if there is no module or no audio channel: the game
/// plays on silent.
pub fn start(dirs: &[String], seed: u32, capture: bool) -> Option<Sound<PspHost>> {
    if capture {
        // How the clock reads a known loop (the emulator's time is a hint, not the hardware's):
        // a million multiply-adds, as the mixer's inner loop does.
        let t = now_us();
        let mut x = 1u32;
        for i in 0..1_000_000u32 {
            x = core::hint::black_box(x.wrapping_mul(3).wrapping_add(i));
        }
        say(format_args!("AUDIO calibration: 1M multiply-adds in {} us ({x})", now_us().wrapping_sub(t)));
    }
    let t0 = now_us();
    let found = dirs.iter().find_map(|d| {
        let mut p = d.clone();
        p.push_str("jane-psp.jau");
        open(&p, sys::IoOpenFlags::RD_ONLY).map(|fd| (d.clone(), fd))
    });
    let Some((dir, file)) = found else {
        say(format_args!("AUDIO no jane-psp.jau; silent"));
        return None;
    };
    let mut prefix = [0u8; PREFIX];
    let ram = read_at(file, 0, &mut prefix).then(|| Bank::ram_len(&prefix)).flatten();
    let Some(ram) = ram else {
        say(format_args!("AUDIO jane-psp.jau is not a module; silent"));
        return None;
    };
    let mut bytes = alloc::vec![0u8; ram];
    if !read_at(file, 0, &mut bytes) {
        say(format_args!("AUDIO short jane-psp.jau; silent"));
        return None;
    }
    let bank = match Bank::parse(bytes) {
        Ok(b) => b,
        Err(e) => {
            say(format_args!("AUDIO {e}; silent"));
            return None;
        }
    };
    let far_at = bank.far_at();
    let mut mixer = Mixer::new(bank, RATE, seed);
    let ram = mixer.ram();
    let bus = Bus::new(PspHost { file }, &mixer.bank().head, far_at);
    mixer.handle(Cmd::Seed(seed));
    let bufs: Vec<Out> = (0..NBUF).map(|_| Out([0; 2 * FRAMES])).collect();
    BUFS.store(alloc::boxed::Box::leak(bufs.into_boxed_slice()).as_mut_ptr(), Ordering::Release);
    MIXER.store(alloc::boxed::Box::into_raw(alloc::boxed::Box::new(mixer)), Ordering::Release);
    if capture {
        let flags = || sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC;
        if let (Some(w), Some(c)) =
            (open(&alloc::format!("{dir}psp-audio.wav"), flags()), open(&alloc::format!("{dir}psp-cmds.txt"), flags()))
        {
            let h = wav_header(0, 2, RATE);
            // SAFETY: an open fd and our header.
            unsafe { sys::sceIoWrite(w, h.as_ptr().cast(), h.len()) };
            WAV_FD.store(w.0, Ordering::Release);
            CMD_FD.store(c.0, Ordering::Release);
            CAPTURE.store(true, Ordering::Release);
        }
    }
    // SAFETY: a thread made once; it runs `audio` for the program's life over the mixer and the
    // buffers stored above, which nothing frees.
    let started = unsafe {
        let id = sys::sceKernelCreateThread(
            b"jane-audio\0".as_ptr(),
            audio,
            PRIORITY,
            32 * 1024,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        );
        if id.0 < 0 { id.0 } else { sys::sceKernelStartThread(id, 0, core::ptr::null_mut()) }
    };
    say(format_args!(
        "AUDIO module {dir}jane-psp.jau in RAM {ram} B, {RATE} Hz, {FRAMES} frames x {NBUF}, thread {started:x}, capture {capture}, made in {} us",
        now_us().wrapping_sub(t0)
    ));
    if started < 0 {
        return None;
    }
    let mut s = Sound::new(bus);
    s.bus.set_volume(jane_present::audio::Volumes::default());
    Some(s)
}

fn say(args: core::fmt::Arguments<'_>) {
    let mut l = Line::new();
    let _ = l.write_fmt(args);
    let _ = l.write_str("\n");
    l.to(1);
}

/// The SRC channel's queue: the hardware is empty when it reads none left.
fn rest() -> i32 {
    // SAFETY: plain syscall.
    unsafe { sys::sceAudioOutput2GetRestSample() }
}

extern "C" fn audio(_argc: usize, _argv: *mut c_void) -> i32 {
    // SAFETY: reserves the sample-rate-converting channel this thread alone outputs to.
    let ch = unsafe { sys::sceAudioSRCChReserve(FRAMES as i32, sys::AudioOutputFrequency::Khz22_05, 2) };
    if ch < 0 {
        say(format_args!("AUDIO no SRC channel ({ch:x}); silent"));
        return 0;
    }
    let bufs = BUFS.load(Ordering::Acquire);
    let capture = CAPTURE.load(Ordering::Acquire);
    let (wav, cmds) = (sys::SceUid(WAV_FD.load(Ordering::Acquire)), CMD_FD.load(Ordering::Acquire));
    let mut k = 0usize;
    let mut frames_out = 0u32;
    let (mut w_us, mut w_blocks, mut w_worst) = (0u32, 0u32, 0u32);
    let mut w_mixed = 0u64;
    let mut started = false;
    loop {
        // SAFETY: the buffers and the mixer were stored before this thread started and are never
        // freed; this thread alone writes the buffers, and touches the mixer only here (the game's
        // far loads hold the dispatcher, so they never land inside this).
        let out = unsafe { &mut (*bufs.add(k)).0 };
        let t = now_us();
        {
            let m = unsafe { &mut *MIXER.load(Ordering::Acquire) };
            while let Some(c) = RING_.pop() {
                if capture && cmds >= 0 {
                    // The frame it lands on and the command's two words (`crate::unpack`).
                    let (a, b) = crate::pack(c);
                    let mut l = Line::new();
                    let _ = writeln!(l, "{} {a:x} {b:x}", m.now);
                    l.to(cmds);
                }
                m.handle(c);
            }
            m.render(out);
        }
        let us = now_us().wrapping_sub(t);
        w_us += us;
        w_worst = w_worst.max(us);
        w_blocks += 1;
        MIX_US.fetch_add(us, Ordering::Relaxed);
        BLOCKS.fetch_add(1, Ordering::Relaxed);
        if started && rest() == 0 {
            UNDERRUNS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: a 64-aligned buffer of FRAMES stereo frames that stays put until it is reused,
        // three buffers later.
        unsafe { sys::sceAudioSRCOutputBlocking(sys::AUDIO_VOLUME_MAX as i32, out.as_mut_ptr().cast()) };
        started = true;
        if capture && wav.0 >= 0 {
            // SAFETY: an open fd and the buffer just output (not reused for three more blocks).
            unsafe { sys::sceIoWrite(wav, out.as_ptr().cast(), out.len() * 2) };
        }
        frames_out += FRAMES as u32;
        k = (k + 1) % NBUF;
        // Every two seconds of sound: the thread's share of the CPU and the mixer's state.
        if frames_out % (2 * RATE / FRAMES as u32 * FRAMES as u32) == 0 {
            let m = unsafe { &*MIXER.load(Ordering::Acquire) };
            let audio_us = w_blocks * FRAMES as u32 * 1000 / (RATE / 1000);
            say(format_args!(
                "AUDIO t={}s cpu={}.{}% mix={}us/block worst={}us voices={} peak_voices={} voice_frames/frame={}.{} underruns={}",
                frames_out / RATE,
                w_us * 100 / audio_us.max(1),
                w_us * 1000 / audio_us.max(1) % 10,
                w_us / w_blocks.max(1),
                w_worst,
                m.voices(),
                m.peak_voices,
                (m.mixed - w_mixed) * 10 / u64::from(w_blocks.max(1) * FRAMES as u32) / 10,
                (m.mixed - w_mixed) * 10 / u64::from(w_blocks.max(1) * FRAMES as u32) % 10,
                UNDERRUNS.load(Ordering::Relaxed)
            ));
            w_mixed = m.mixed;
            (w_us, w_blocks, w_worst) = (0, 0, 0);
            if capture && wav.0 >= 0 {
                let h = wav_header(frames_out, 2, RATE);
                // SAFETY: an open fd; back to the header, then to the end again.
                unsafe {
                    sys::sceIoLseek(wav, 0, sys::IoWhence::Set);
                    sys::sceIoWrite(wav, h.as_ptr().cast(), h.len());
                    sys::sceIoLseek(wav, 0, sys::IoWhence::End);
                }
            }
        }
    }
}
