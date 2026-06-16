//! placehold — generate placeholder images locally.
//!
//! A placeholder is a solid-colored rectangle of a given size with the
//! dimensions drawn in the center (e.g. "640x480"). Everything is rendered in
//! pure Rust with the `image` crate and the public-domain `font8x8` bitmap
//! font, so there is no external service and no ImageMagick dependency.

use anyhow::{anyhow, Result};
use font8x8::legacy::BASIC_LEGACY;
use image::{Rgba, RgbaImage};

/// A parsed image size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// Upper bound on a dimension, to reject absurd inputs early.
pub const MAX_DIM: u32 = 20_000;

/// Upper bound on total pixels, to avoid huge single allocations (an RGBA
/// buffer is 4 bytes per pixel, so 100 MP is ~400 MB).
pub const MAX_PIXELS: u64 = 100_000_000;

/// Parse a size string: "WxH" (e.g. "640x480") or a single "N" meaning NxN.
pub fn parse_size(input: &str) -> Result<Size> {
    let s = input.trim().to_ascii_lowercase();
    let (w, h) = match s.split_once('x') {
        Some((a, b)) => (a, b),
        None => (s.as_str(), s.as_str()),
    };
    let width: u32 = w
        .parse()
        .map_err(|_| anyhow!("invalid size: {input:?} (expected WxH or N)"))?;
    let height: u32 = h
        .parse()
        .map_err(|_| anyhow!("invalid size: {input:?} (expected WxH or N)"))?;
    if width == 0 || height == 0 {
        return Err(anyhow!("size must be positive: {input:?}"));
    }
    if width > MAX_DIM || height > MAX_DIM {
        return Err(anyhow!(
            "size too large (max {MAX_DIM} per side): {input:?}"
        ));
    }
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err(anyhow!(
            "size too large (max {MAX_PIXELS} total pixels): {input:?}"
        ));
    }
    Ok(Size { width, height })
}

/// Validate and normalize an output file extension to a supported format.
/// Returns "png", "jpg", or "jpeg" (lowercased); errors otherwise.
pub fn ext_for_output(path: &std::path::Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") | Some("jpg") | Some("jpeg") => Ok(ext.unwrap()),
        Some(other) => Err(anyhow!(
            "unsupported output extension: .{other} (use .png/.jpg)"
        )),
        None => Err(anyhow!("output path has no extension: {}", path.display())),
    }
}

/// Parse a hex color ("#aabbcc", "aabbcc", "#abc", "abc") into opaque RGBA.
pub fn parse_color(input: &str) -> Result<Rgba<u8>> {
    let h = input.trim().strip_prefix('#').unwrap_or(input.trim());
    let expanded = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => h.to_string(),
        _ => return Err(anyhow!("invalid hex color: {input:?}")),
    };
    let val =
        u32::from_str_radix(&expanded, 16).map_err(|_| anyhow!("invalid hex color: {input:?}"))?;
    Ok(Rgba([
        ((val >> 16) & 0xff) as u8,
        ((val >> 8) & 0xff) as u8,
        (val & 0xff) as u8,
        255,
    ]))
}

/// Build the default output file name: "<W>x<H>[_<bghex>].<ext>".
pub fn default_filename(size: Size, bg_hex: Option<&str>, ext: &str) -> String {
    match bg_hex {
        Some(hex) => format!(
            "{}x{}_{}.{}",
            size.width,
            size.height,
            normalize_hex(hex),
            ext
        ),
        None => format!("{}x{}.{}", size.width, size.height, ext),
    }
}

fn normalize_hex(hex: &str) -> String {
    let h = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if h.len() == 3 {
        h.chars()
            .flat_map(|c| [c, c])
            .collect::<String>()
            .to_ascii_lowercase()
    } else {
        h.to_ascii_lowercase()
    }
}

/// Choose the largest integer text scale so the label fits within ~80% of the
/// image on both axes. Always at least 1.
pub fn auto_scale(size: Size, text_len: u32) -> u32 {
    if text_len == 0 {
        return 1;
    }
    let max_w = (size.width as f32 * 0.8) / (text_len as f32 * 8.0);
    let max_h = (size.height as f32 * 0.8) / 8.0;
    let s = max_w.min(max_h).floor() as i64;
    s.clamp(1, 64) as u32
}

/// Render a placeholder image: a solid `bg` fill, with `text` (if any) drawn
/// centered in `fg`. When `scale` is None it is chosen automatically.
pub fn render(
    size: Size,
    bg: Rgba<u8>,
    fg: Rgba<u8>,
    text: Option<&str>,
    scale: Option<u32>,
) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(size.width, size.height, bg);
    if let Some(t) = text {
        if !t.is_empty() {
            let s = scale.unwrap_or_else(|| auto_scale(size, t.chars().count() as u32));
            draw_text_centered(&mut img, t, fg, s.max(1));
        }
    }
    img
}

/// Draw `text` centered using the 8x8 bitmap font, scaled by `scale`.
fn draw_text_centered(img: &mut RgbaImage, text: &str, fg: Rgba<u8>, scale: u32) {
    let chars: Vec<char> = text.chars().collect();
    let text_w = chars.len() as i64 * 8 * scale as i64;
    let text_h = 8 * scale as i64;
    let ox = (img.width() as i64 - text_w) / 2;
    let oy = (img.height() as i64 - text_h) / 2;

    for (ci, &ch) in chars.iter().enumerate() {
        let glyph = glyph_for(ch);
        let cell_x = ox + ci as i64 * 8 * scale as i64;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                if (bits >> col) & 1 == 0 {
                    continue;
                }
                // Scale this lit pixel into a `scale` x `scale` block.
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = cell_x + (col * scale + dx) as i64;
                        let py = oy + (row as u32 * scale + dy) as i64;
                        if px >= 0
                            && py >= 0
                            && (px as u32) < img.width()
                            && (py as u32) < img.height()
                        {
                            img.put_pixel(px as u32, py as u32, fg);
                        }
                    }
                }
            }
        }
    }
}

/// Look up an ASCII glyph, falling back to a blank cell for anything outside
/// the basic font's range.
fn glyph_for(ch: char) -> [u8; 8] {
    let code = ch as usize;
    if code < BASIC_LEGACY.len() {
        BASIC_LEGACY[code]
    } else {
        [0; 8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_forms() {
        assert_eq!(
            parse_size("640x480").unwrap(),
            Size {
                width: 640,
                height: 480
            }
        );
        assert_eq!(
            parse_size("100").unwrap(),
            Size {
                width: 100,
                height: 100
            }
        );
        assert_eq!(
            parse_size("12X34").unwrap(),
            Size {
                width: 12,
                height: 34
            }
        );
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("axb").is_err());
        assert!(parse_size("99999x1").is_err());
        // Total-pixel cap: 20000x20000 is within the per-side cap but rejected.
        assert!(parse_size("20000x20000").is_err());
        assert!(parse_size("10000x10000").is_ok());
    }

    #[test]
    fn ext_for_output_validates() {
        use std::path::Path;
        assert_eq!(ext_for_output(Path::new("a/b.PNG")).unwrap(), "png");
        assert_eq!(ext_for_output(Path::new("x.jpg")).unwrap(), "jpg");
        assert_eq!(ext_for_output(Path::new("x.jpeg")).unwrap(), "jpeg");
        assert!(ext_for_output(Path::new("x.gif")).is_err());
        assert!(ext_for_output(Path::new("noext")).is_err());
    }

    #[test]
    fn parse_color_forms() {
        assert_eq!(
            parse_color("#aabbcc").unwrap(),
            Rgba([0xaa, 0xbb, 0xcc, 255])
        );
        assert_eq!(parse_color("abc").unwrap(), Rgba([0xaa, 0xbb, 0xcc, 255]));
        assert_eq!(
            parse_color("959595").unwrap(),
            Rgba([0x95, 0x95, 0x95, 255])
        );
        assert!(parse_color("xyz").is_err());
        assert!(parse_color("12345").is_err());
    }

    #[test]
    fn filename_default() {
        let s = Size {
            width: 515,
            height: 230,
        };
        assert_eq!(default_filename(s, None, "png"), "515x230.png");
        assert_eq!(
            default_filename(s, Some("#ABC"), "jpg"),
            "515x230_aabbcc.jpg"
        );
        assert_eq!(
            default_filename(s, Some("959595"), "png"),
            "515x230_959595.png"
        );
    }

    #[test]
    fn auto_scale_fits() {
        // A wide image gets a larger scale than a tiny one.
        let big = auto_scale(
            Size {
                width: 800,
                height: 600,
            },
            7,
        );
        let small = auto_scale(
            Size {
                width: 40,
                height: 40,
            },
            7,
        );
        assert!(big >= small);
        assert!(small >= 1);
    }

    #[test]
    fn render_fills_background_and_dims() {
        let s = Size {
            width: 60,
            height: 40,
        };
        let img = render(
            s,
            Rgba([10, 20, 30, 255]),
            Rgba([255, 255, 255, 255]),
            Some("60x40"),
            None,
        );
        assert_eq!(img.width(), 60);
        assert_eq!(img.height(), 40);
        // Corner stays the background color (text is centered).
        assert_eq!(*img.get_pixel(0, 0), Rgba([10, 20, 30, 255]));
        // At least one foreground pixel exists somewhere (text was drawn).
        let has_fg = img.pixels().any(|p| *p == Rgba([255, 255, 255, 255]));
        assert!(has_fg);
    }
}
