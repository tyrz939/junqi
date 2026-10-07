#![no_std]
#![no_main]

// Spike: proves nightly Rust + rust-psp + PPSSPP end to end with an integer-only,
// allocation-free stand-in for the sim. Prints a state hash that must match on every target.

psp::module!("psp_smoke", 1, 1);

fn step(state: &mut u64, tick: u32) {
    // xorshift plus tick mix: integers only, explicit width, no usize in state
    let mut x = *state ^ (tick as u64);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
}

struct Out {
    buf: [u8; 96],
    len: usize,
}

impl core::fmt::Write for Out {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let n = b.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&b[..n]);
        self.len += n;
        Ok(())
    }
}

fn psp_main() {
    psp::enable_home_button();
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut t = 0u32;
    while t < 100_000 {
        step(&mut s, t);
        t += 1;
    }
    let mut out = Out { buf: [0; 96], len: 0 };
    let _ = core::fmt::write(&mut out, format_args!("SMOKE hash={:016x} ticks={}
", s, t));
    unsafe {
        psp::sys::sceIoWrite(psp::sys::SceUid(1), out.buf.as_ptr() as *const _, out.len);
    }
    unsafe { psp::sys::sceKernelExitGame() };
}
