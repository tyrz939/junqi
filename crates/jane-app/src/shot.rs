//! The canvas as a PNG (`--shot`, F12), through `jane_art::sheet`'s encoder: no image crate.

use std::path::Path;

/// `0xAARRGGBB` pixels as RGBA bytes, opaque.
pub fn rgba(px: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        let [_, r, g, b] = p.to_be_bytes();
        out.extend_from_slice(&[r, g, b, 255]);
    }
    out
}

/// Write `w` x `h` pixels to `path` as a PNG, making its folder if need be.
pub fn write(path: &str, px: &[u32], w: u16, h: u16) -> Result<(), String> {
    let (w, h) = (u32::from(w), u32::from(h));
    let n = (w * h) as usize;
    if n == 0 || px.len() < n {
        return Err(format!("{path}: no frame drawn yet"));
    }
    if let Some(dir) = Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let png = jane_art::sheet::png(w, h, &rgba(&px[..n]));
    std::fs::write(path, png).map_err(|e| format!("{path}: {e}"))
}

/// `shot.png`, 3 -> `shot-0003.png` (as `jane play --snap-every` numbers its snaps).
pub fn numbered(path: &str, n: u32) -> String {
    let (stem, ext) = path.rsplit_once('.').filter(|(s, _)| !s.is_empty()).unwrap_or((path, "png"));
    format!("{stem}-{n:04}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_becomes_opaque_rgba() {
        assert_eq!(rgba(&[0xff10_2030, 0x0001_0203]), vec![0x10, 0x20, 0x30, 255, 1, 2, 3, 255]);
    }

    #[test]
    fn shots_are_numbered_beside_the_path() {
        assert_eq!(numbered("sheets/app.png", 3), "sheets/app-0003.png");
        assert_eq!(numbered("shot", 12), "shot-0012.png");
    }
}
