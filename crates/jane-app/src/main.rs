//! The SDL2 game binary.

fn main() {
    let v = sdl2::version::version();
    println!("SDL {v}");
}
