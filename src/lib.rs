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
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") => Ok(ext.unwrap()),
        Some(other) => Err(anyhow!(
            "unsupported output extension: .{other} (use .png/.jpg/.webp)"
        )),
        None => Err(anyhow!("output path has no extension: {}", path.display())),
    }
}

/// Parse a hex color into RGBA, with or without a leading `#`.
///
/// Accepted digit counts:
/// - 3 ("abc") and 6 ("aabbcc"): opaque RGB
/// - 4 ("abcd") and 8 ("aabbccdd"): RGBA with an explicit alpha channel
///
/// Short forms expand each digit ("f00a" -> "ff0000aa"). Alpha survives in
/// PNG/WebP output; JPEG flattens it when encoding.
pub fn parse_color(input: &str) -> Result<Rgba<u8>> {
    let h = input.trim().strip_prefix('#').unwrap_or(input.trim());
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid hex color: {input:?}"));
    }
    let expanded = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 | 8 => h.to_string(),
        _ => return Err(anyhow!("invalid hex color: {input:?}")),
    };
    // Default to fully opaque when no alpha digits were given.
    let rgba = if expanded.len() == 6 {
        format!("{expanded}ff")
    } else {
        expanded
    };
    let val =
        u32::from_str_radix(&rgba, 16).map_err(|_| anyhow!("invalid hex color: {input:?}"))?;
    Ok(Rgba([
        ((val >> 24) & 0xff) as u8,
        ((val >> 16) & 0xff) as u8,
        ((val >> 8) & 0xff) as u8,
        (val & 0xff) as u8,
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
    if h.len() == 3 || h.len() == 4 {
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
    draw_label(&mut img, size, fg, text, scale);
    img
}

/// Derive a second, visibly different checkerboard color from `bg`: blend
/// toward black for light backgrounds, toward white for dark ones.
pub fn alt_color(bg: Rgba<u8>) -> Rgba<u8> {
    let [r, g, b, a] = bg.0;
    // Perceptual-ish luminance (0..255).
    let lum = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
    let mix = |c: u8, toward: u8| -> u8 {
        // Blend 18% toward the target.
        ((c as u32 * 82 + toward as u32 * 18) / 100) as u8
    };
    let toward = if lum >= 128 { 0 } else { 255 };
    Rgba([mix(r, toward), mix(g, toward), mix(b, toward), a])
}

/// Render a placeholder with a checkerboard background (`bg` + a derived shade)
/// and the centered label. `cell` is the square size in pixels (auto when None).
pub fn render_checker(
    size: Size,
    bg: Rgba<u8>,
    fg: Rgba<u8>,
    text: Option<&str>,
    scale: Option<u32>,
    cell: Option<u32>,
) -> RgbaImage {
    let alt = alt_color(bg);
    let cell = cell
        .filter(|&c| c > 0)
        .unwrap_or_else(|| (size.width.min(size.height) / 8).max(8));
    let mut img = RgbaImage::new(size.width, size.height);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let on = ((x / cell) + (y / cell)).is_multiple_of(2);
        *px = if on { bg } else { alt };
    }
    draw_label(&mut img, size, fg, text, scale);
    img
}

/// Blend two colors: `t` = 0 returns `a`, `t` = 1 returns `b`.
pub fn lerp_color(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgba([
        mix(a.0[0], b.0[0]),
        mix(a.0[1], b.0[1]),
        mix(a.0[2], b.0[2]),
        mix(a.0[3], b.0[3]),
    ])
}

/// Render a placeholder with diagonal stripes (`bg` + a derived shade) and the
/// centered label. `cell` is the stripe width in pixels (auto when None).
pub fn render_diag(
    size: Size,
    bg: Rgba<u8>,
    fg: Rgba<u8>,
    text: Option<&str>,
    scale: Option<u32>,
    cell: Option<u32>,
) -> RgbaImage {
    let alt = alt_color(bg);
    let cell = cell
        .filter(|&c| c > 0)
        .unwrap_or_else(|| (size.width.min(size.height) / 12).max(6));
    let mut img = RgbaImage::new(size.width, size.height);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let on = ((x + y) / cell).is_multiple_of(2);
        *px = if on { bg } else { alt };
    }
    draw_label(&mut img, size, fg, text, scale);
    img
}

/// Render a placeholder with a left-to-right gradient from `bg` to a derived
/// shade, and the centered label.
pub fn render_gradient(
    size: Size,
    bg: Rgba<u8>,
    fg: Rgba<u8>,
    text: Option<&str>,
    scale: Option<u32>,
) -> RgbaImage {
    let alt = alt_color(bg);
    let denom = (size.width.max(2) - 1) as f32;
    let mut img = RgbaImage::new(size.width, size.height);
    for (x, _y, px) in img.enumerate_pixels_mut() {
        *px = lerp_color(bg, alt, x as f32 / denom);
    }
    draw_label(&mut img, size, fg, text, scale);
    img
}

/// Whether pixel `(x, y)` lies inside a `w` x `h` rounded rectangle with corner
/// radius `r`. Pixels in a corner region beyond the corner circle are outside.
pub fn is_inside_rounded(x: u32, y: u32, w: u32, h: u32, r: u32) -> bool {
    if w == 0 || h == 0 {
        return false;
    }
    let r = r.min(w / 2).min(h / 2);
    if r == 0 {
        return true;
    }
    // Corner circle centers (in pixel coordinates).
    let (cx, cy) = (
        if x < r {
            r
        } else if x >= w - r {
            w - 1 - r
        } else {
            x // straight edge column: always inside
        },
        if y < r {
            r
        } else if y >= h - r {
            h - 1 - r
        } else {
            y
        },
    );
    let dx = x as i64 - cx as i64;
    let dy = y as i64 - cy as i64;
    dx * dx + dy * dy <= (r as i64) * (r as i64)
}

/// Clear (make transparent) the pixels outside a rounded rectangle of radius
/// `r`. A no-op when `r` is 0.
pub fn apply_radius(img: &mut RgbaImage, r: u32) {
    if r == 0 {
        return;
    }
    let (w, h) = (img.width(), img.height());
    for (x, y, px) in img.enumerate_pixels_mut() {
        if !is_inside_rounded(x, y, w, h, r) {
            *px = Rgba([0, 0, 0, 0]);
        }
    }
}

/// Draw a `thickness`-pixel border of `color` around the image edges.
pub fn apply_border(img: &mut RgbaImage, thickness: u32, color: Rgba<u8>) {
    if thickness == 0 {
        return;
    }
    let (w, h) = (img.width(), img.height());
    let t = thickness.min(w.div_ceil(2)).min(h.div_ceil(2));
    for (x, y, px) in img.enumerate_pixels_mut() {
        if x < t || y < t || x >= w - t || y >= h - t {
            *px = color;
        }
    }
}

/// Split a label into lines, treating a literal `\n` (backslash-n) or a real
/// newline as a line break.
pub fn split_label(text: &str) -> Vec<String> {
    text.replace("\\n", "\n")
        .split('\n')
        .map(str::to_string)
        .collect()
}

/// Choose the largest scale so a multi-line label block fits within ~80% of the
/// image: width by the longest line, height by the line count.
pub fn auto_scale_block(size: Size, max_line_len: u32, lines: u32) -> u32 {
    if max_line_len == 0 || lines == 0 {
        return 1;
    }
    let max_w = (size.width as f32 * 0.8) / (max_line_len as f32 * 8.0);
    let max_h = (size.height as f32 * 0.8) / (lines as f32 * 8.0);
    (max_w.min(max_h).floor() as i64).clamp(1, 64) as u32
}

/// Draw the centered label (if any) onto an existing image. The label may span
/// multiple lines (split on `\n`); lines are stacked and each is centered.
fn draw_label(
    img: &mut RgbaImage,
    size: Size,
    fg: Rgba<u8>,
    text: Option<&str>,
    scale: Option<u32>,
) {
    let Some(t) = text else { return };
    if t.is_empty() {
        return;
    }
    let lines = split_label(t);
    let max_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u32;
    let s = scale
        .unwrap_or_else(|| auto_scale_block(size, max_len, lines.len() as u32))
        .max(1);

    let block_h = lines.len() as i64 * 8 * s as i64;
    let oy = (img.height() as i64 - block_h) / 2;
    for (i, line) in lines.iter().enumerate() {
        draw_line_centered(img, line, fg, s, oy + i as i64 * 8 * s as i64);
    }
}

/// Draw the centered label with an optional drop shadow.
pub fn draw_shadowed_label(
    img: &mut RgbaImage,
    size: Size,
    fg: Rgba<u8>,
    shadow: Option<Rgba<u8>>,
    text: Option<&str>,
    scale: Option<u32>,
) {
    let Some(t) = text else { return };
    if t.is_empty() {
        return;
    }
    let lines = split_label(t);
    let max_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u32;
    let s = scale
        .unwrap_or_else(|| auto_scale_block(size, max_len, lines.len() as u32))
        .max(1);
    let block_h = lines.len() as i64 * 8 * s as i64;
    let oy = (img.height() as i64 - block_h) / 2;
    let off = (s as i64 / 4).max(1);
    for (i, line) in lines.iter().enumerate() {
        let line_y = oy + i as i64 * 8 * s as i64;
        if let Some(sh) = shadow {
            draw_line(img, line, sh, s, off, line_y + off);
        }
        draw_line(img, line, fg, s, 0, line_y);
    }
}

/// Draw one line horizontally centered (plus a `dx` x-offset) at top `y`.
fn draw_line_centered(img: &mut RgbaImage, text: &str, fg: Rgba<u8>, scale: u32, oy: i64) {
    draw_line(img, text, fg, scale, 0, oy);
}

fn draw_line(img: &mut RgbaImage, text: &str, fg: Rgba<u8>, scale: u32, dx: i64, oy: i64) {
    let chars: Vec<char> = text.chars().collect();
    let text_w = chars.len() as i64 * 8 * scale as i64;
    let ox = (img.width() as i64 - text_w) / 2 + dx;

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
    fn parse_color_alpha_forms() {
        // 4-digit RGBA: each digit doubles, including alpha.
        assert_eq!(parse_color("0000").unwrap(), Rgba([0, 0, 0, 0]));
        assert_eq!(
            parse_color("#f00a").unwrap(),
            Rgba([0xff, 0x00, 0x00, 0xaa])
        );
        // 8-digit RRGGBBAA.
        assert_eq!(
            parse_color("aabbccdd").unwrap(),
            Rgba([0xaa, 0xbb, 0xcc, 0xdd])
        );
        assert_eq!(
            parse_color("#00000080").unwrap(),
            Rgba([0x00, 0x00, 0x00, 0x80])
        );
        // 3- and 6-digit forms stay fully opaque.
        assert_eq!(parse_color("abc").unwrap().0[3], 255);
        assert_eq!(parse_color("aabbcc").unwrap().0[3], 255);
        // Unsupported lengths are rejected.
        assert!(parse_color("12").is_err());
        assert!(parse_color("1234567").is_err());
        assert!(parse_color("123456789").is_err());
        // Non-hex digits are rejected even when the length matches, including
        // a sign prefix that from_str_radix would otherwise accept.
        assert!(parse_color("+abc12").is_err());
        assert!(parse_color("ggggggga").is_err());
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
        // A short RGBA background expands the same way as RGB.
        assert_eq!(
            default_filename(s, Some("#f00a"), "png"),
            "515x230_ff0000aa.png"
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
    fn split_label_breaks_on_backslash_n_and_newlines() {
        assert_eq!(split_label("one"), vec!["one"]);
        assert_eq!(split_label("a\\nb"), vec!["a", "b"]); // literal backslash-n
        assert_eq!(split_label("a\nb"), vec!["a", "b"]); // real newline
        assert_eq!(split_label("a\\nb\nc"), vec!["a", "b", "c"]);
    }

    #[test]
    fn auto_scale_block_accounts_for_line_count() {
        // Two lines get a smaller scale than one line for the same image height.
        let one = auto_scale_block(
            Size {
                width: 400,
                height: 100,
            },
            4,
            1,
        );
        let two = auto_scale_block(
            Size {
                width: 400,
                height: 100,
            },
            4,
            2,
        );
        assert!(two <= one);
        assert!(two >= 1);
    }

    #[test]
    fn shadowed_label_draws_shadow_and_text() {
        let s = Size {
            width: 120,
            height: 60,
        };
        let mut img = RgbaImage::from_pixel(s.width, s.height, Rgba([0, 0, 0, 255]));
        let fg = Rgba([255, 255, 255, 255]);
        let shadow = Rgba([255, 0, 0, 255]);
        draw_shadowed_label(&mut img, s, fg, Some(shadow), Some("X"), Some(4));
        // Both the foreground (white) and the shadow (red) appear somewhere.
        assert!(img.pixels().any(|p| *p == fg));
        assert!(img.pixels().any(|p| *p == shadow));
    }

    #[test]
    fn render_multiline_label_draws_both_lines() {
        // A tall image with a two-line label: foreground pixels appear in both
        // the top and bottom halves.
        let s = Size {
            width: 120,
            height: 120,
        };
        let img = render(
            s,
            Rgba([0, 0, 0, 255]),
            Rgba([255, 255, 255, 255]),
            Some("HELLO\\nWORLD"),
            None,
        );
        let fg = Rgba([255, 255, 255, 255]);
        let top = (0..60).any(|y| (0..120).any(|x| *img.get_pixel(x, y) == fg));
        let bottom = (60..120).any(|y| (0..120).any(|x| *img.get_pixel(x, y) == fg));
        assert!(top && bottom);
    }

    #[test]
    fn alt_color_contrasts_with_background() {
        // Light background -> darker alt; dark background -> lighter alt.
        let light = alt_color(Rgba([200, 200, 200, 255]));
        assert!(light.0[0] < 200);
        let dark = alt_color(Rgba([20, 20, 20, 255]));
        assert!(dark.0[0] > 20);
    }

    #[test]
    fn lerp_color_endpoints_and_middle() {
        let a = Rgba([0, 0, 0, 255]);
        let b = Rgba([100, 200, 50, 255]);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 0.5), Rgba([50, 100, 25, 255]));
    }

    #[test]
    fn render_gradient_spans_bg_to_alt() {
        let s = Size {
            width: 50,
            height: 10,
        };
        let bg = Rgba([200, 200, 200, 255]);
        let img = render_gradient(s, bg, Rgba([0, 0, 0, 255]), None, None);
        // Left edge is the background; right edge is the derived alt color.
        assert_eq!(*img.get_pixel(0, 0), bg);
        assert_eq!(*img.get_pixel(49, 0), alt_color(bg));
    }

    #[test]
    fn render_diag_uses_two_colors() {
        let s = Size {
            width: 40,
            height: 40,
        };
        let bg = Rgba([200, 200, 200, 255]);
        let img = render_diag(s, bg, Rgba([0, 0, 0, 255]), None, None, Some(4));
        // Stripe index (x+y)/cell alternates: (0,0) is bg, (4,0) is alt.
        assert_eq!(*img.get_pixel(0, 0), bg);
        assert_eq!(*img.get_pixel(4, 0), alt_color(bg));
    }

    #[test]
    fn rounded_corners_excluded() {
        // 20x20, radius 6: the very corner (0,0) is outside; the center is inside.
        assert!(!is_inside_rounded(0, 0, 20, 20, 6));
        assert!(is_inside_rounded(10, 10, 20, 20, 6));
        // A straight edge midpoint is inside.
        assert!(is_inside_rounded(10, 0, 20, 20, 6));
        // radius 0 -> everything inside.
        assert!(is_inside_rounded(0, 0, 20, 20, 0));
    }

    #[test]
    fn apply_radius_makes_corner_transparent() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([10, 20, 30, 255]));
        apply_radius(&mut img, 6);
        assert_eq!(img.get_pixel(0, 0).0[3], 0); // corner cleared
        assert_eq!(*img.get_pixel(10, 10), Rgba([10, 20, 30, 255])); // center kept
    }

    #[test]
    fn apply_border_paints_the_edge() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        let red = Rgba([255, 0, 0, 255]);
        apply_border(&mut img, 2, red);
        assert_eq!(*img.get_pixel(0, 0), red);
        assert_eq!(*img.get_pixel(1, 10), red);
        assert_eq!(*img.get_pixel(10, 10), Rgba([0, 0, 0, 255])); // interior untouched
    }

    #[test]
    fn render_checker_uses_two_colors() {
        let s = Size {
            width: 32,
            height: 32,
        };
        let bg = Rgba([200, 200, 200, 255]);
        let img = render_checker(s, bg, Rgba([0, 0, 0, 255]), None, None, Some(8));
        let alt = alt_color(bg);
        // (0,0) is the bg color; the neighboring cell at (8,0) is the alt color.
        assert_eq!(*img.get_pixel(0, 0), bg);
        assert_eq!(*img.get_pixel(8, 0), alt);
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
