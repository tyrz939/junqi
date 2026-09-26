//! Static SDL2 on Windows (`sdl2` `static-link`) reads the registry for audio device names and the
//! mouse settings; sdl2-sys does not ask for the library that holds those calls, so the app does.

fn main() {
    if std::env::var_os("CARGO_CFG_TARGET_OS").is_some_and(|os| os == "windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
