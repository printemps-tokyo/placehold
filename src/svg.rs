//! SVG output: the same placeholder as a vector image. Patterns become SVG
//! `<pattern>`s and a `<linearGradient>` that reproduce the raster ones
//! (`alt_color` shade, cell sizes, stripe direction), the radius clips the
//! whole image, and the label is real text.

use base64_lite::encode as b64;
use image::Rgba;

use crate::font::LabelFont;
use crate::{alt_color, split_label, Size};

/// Background pattern, as the CLI names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Checker,
    Diag,
    Gradient,
}

/// Everything the SVG needs.
pub struct SvgSpec<'a> {
    pub size: Size,
    pub bg: Rgba<u8>,
    pub fg: Rgba<u8>,
    pub pattern: Pattern,
    pub cell: Option<u32>,
    pub radius: Option<u32>,
    pub border: Option<(u32, Rgba<u8>)>,
    pub shadow: Option<Rgba<u8>>,
    pub label: Option<&'a str>,
    /// Label size in pixels; chosen to fit when `None`.
    pub font_px: Option<f32>,
    /// CSS font-family list for the label when no font file is embedded.
    pub font_family: &'a str,
    /// A font file to embed as a data URI (`@font-face`), which makes the
    /// image render the same everywhere at the cost of its size.
    pub embed: Option<&'a LabelFont>,
}

/// Escape text for XML character data and attribute values.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Not allowed in XML 1.0 at all.
            '\u{0}'..='\u{8}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'..='\u{1f}'
            | '\u{fffe}'
            | '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

/// `fill="#rrggbb"` plus an opacity attribute when the color is translucent.
fn paint(attr: &str, c: Rgba<u8>) -> String {
    let [r, g, b, a] = c.0;
    let mut s = format!("{attr}=\"#{r:02x}{g:02x}{b:02x}\"");
    if a != 255 {
        s.push_str(&format!(
            " {attr}-opacity=\"{}\"",
            round3(f32::from(a) / 255.0)
        ));
    }
    s
}

/// The color with full alpha.
fn opaque(c: Rgba<u8>) -> Rgba<u8> {
    Rgba([c.0[0], c.0[1], c.0[2], 255])
}

/// `fill="url(#bgp)"`, with the background's alpha applied once.
fn pattern_fill(bg: Rgba<u8>) -> String {
    let mut s = "fill=\"url(#bgp)\"".to_string();
    if bg.0[3] != 255 {
        s.push_str(&format!(
            " fill-opacity=\"{}\"",
            round3(f32::from(bg.0[3]) / 255.0)
        ));
    }
    s
}

fn round3(v: f32) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Rough advance in em for a font we cannot measure: CJK and other wide
/// characters take a full em, everything else about 0.6.
fn estimate_em(line: &str) -> f32 {
    line.chars()
        .map(|c| if u32::from(c) >= 0x1100 { 1.0 } else { 0.6 })
        .sum()
}

pub fn render(spec: &SvgSpec) -> String {
    let (w, h) = (spec.size.width, spec.size.height);
    let alt = alt_color(spec.bg);
    let mut defs = String::new();
    let mut body = String::new();

    let fill = match spec.pattern {
        Pattern::Solid => paint("fill", spec.bg),
        // The pattern is painted opaque and the rect carries the alpha that
        // bg and its shade share (alt_color keeps it), so a translucent
        // background is translucent once, as in the PNG.
        Pattern::Checker => {
            let c = spec
                .cell
                .filter(|&c| c > 0)
                .unwrap_or_else(|| (w.min(h) / 8).max(8));
            defs.push_str(&format!(
                "<pattern id=\"bgp\" width=\"{t}\" height=\"{t}\" patternUnits=\"userSpaceOnUse\"><rect width=\"{t}\" height=\"{t}\" {alt}/><rect width=\"{c}\" height=\"{c}\" {bg}/><rect x=\"{c}\" y=\"{c}\" width=\"{c}\" height=\"{c}\" {bg}/></pattern>",
                t = 2 * c,
                alt = paint("fill", opaque(alt)),
                bg = paint("fill", opaque(spec.bg)),
            ));
            pattern_fill(spec.bg)
        }
        Pattern::Diag => {
            // Raster: ((x + y) / cell) even is bg, so bands run along
            // x + y = const with period 2 * cell in both axes.
            let c = spec
                .cell
                .filter(|&c| c > 0)
                .unwrap_or_else(|| (w.min(h) / 12).max(6));
            let t = 2 * c;
            defs.push_str(&format!(
                "<pattern id=\"bgp\" width=\"{t}\" height=\"{t}\" patternUnits=\"userSpaceOnUse\"><rect width=\"{t}\" height=\"{t}\" {alt}/><polygon points=\"0,0 {c},0 0,{c}\" {bg}/><polygon points=\"{t},0 {t},{c} {c},{t} 0,{t}\" {bg}/></pattern>",
                alt = paint("fill", opaque(alt)),
                bg = paint("fill", opaque(spec.bg)),
            ));
            pattern_fill(spec.bg)
        }
        // The PNG puts bg at pixel 0 and the shade at pixel w - 1, so the
        // gradient runs between those pixel centers; one pixel wide is bg.
        Pattern::Gradient if w < 2 => paint("fill", spec.bg),
        Pattern::Gradient => {
            let stop = |offset: &str, col: Rgba<u8>| {
                let [r, g, b, _] = col.0;
                format!("<stop offset=\"{offset}\" stop-color=\"#{r:02x}{g:02x}{b:02x}\"/>")
            };
            defs.push_str(&format!(
                "<linearGradient id=\"bgp\" gradientUnits=\"userSpaceOnUse\" x1=\"0.5\" y1=\"0\" x2=\"{}\" y2=\"0\">{}{}</linearGradient>",
                f64::from(w) - 0.5,
                stop("0", spec.bg),
                stop("1", alt)
            ));
            pattern_fill(spec.bg)
        }
    };
    let radius = spec.radius.unwrap_or(0).min(w / 2).min(h / 2);
    if radius > 0 {
        defs.push_str(&format!(
            "<clipPath id=\"clip\"><rect width=\"{w}\" height=\"{h}\" rx=\"{radius}\"/></clipPath>"
        ));
    }
    body.push_str(&format!("<rect width=\"{w}\" height=\"{h}\" {fill}/>"));

    if let Some((t, color)) = spec.border.filter(|(t, _)| *t > 0) {
        let t = t.min(w.div_ceil(2)).min(h.div_ceil(2));
        let half = f64::from(t) / 2.0;
        let rx = (f64::from(radius) - half).max(0.0);
        body.push_str(&format!(
            "<rect x=\"{half}\" y=\"{half}\" width=\"{}\" height=\"{}\"{} fill=\"none\" {} stroke-width=\"{t}\"/>",
            f64::from(w) - f64::from(t),
            f64::from(h) - f64::from(t),
            if rx > 0.0 { format!(" rx=\"{rx}\"") } else { String::new() },
            paint("stroke", color),
        ));
    }

    let mut family = escape(spec.font_family);
    if let Some(font) = spec.embed {
        defs.push_str(&format!(
            "<style>@font-face{{font-family:\"placehold-label\";src:url(data:font/{};base64,{})}}</style>",
            if font.bytes().starts_with(b"OTTO") { "otf" } else { "ttf" },
            b64(font.bytes())
        ));
        family = format!("placehold-label, {family}");
    }

    if let Some(text) = spec.label.filter(|t| !t.is_empty()) {
        let lines = split_label(text);
        // Line height 1.2 em; with an embedded font, its own metrics.
        let px = spec.font_px.unwrap_or_else(|| match spec.embed {
            Some(font) => font.fit_px(spec.size, &lines),
            None => {
                let widest = lines.iter().map(|l| estimate_em(l)).fold(0.0f32, f32::max);
                let by_w = if widest > 0.0 {
                    w as f32 * 0.8 / widest
                } else {
                    f32::INFINITY
                };
                let by_h = h as f32 * 0.8 / (1.2 * lines.len() as f32);
                by_w.min(by_h).clamp(1.0, crate::font::MAX_FONT_PX)
            }
        });
        let px = (px * 100.0).round() / 100.0;
        let line_h = match spec.embed {
            Some(font) => font.line_height(px),
            None => px * 1.2,
        };
        let first = f64::from(h) / 2.0 - f64::from(line_h) * (lines.len() as f64 - 1.0) / 2.0;
        let off = (px / 32.0).round().max(1.0);
        let mut text_el = |dx: f32, color: Rgba<u8>| {
            body.push_str(&format!(
                "<text x=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" xml:space=\"preserve\" style=\"white-space:pre\" font-family=\"{family}\" font-size=\"{px}\" {}>",
                f64::from(w) / 2.0 + f64::from(dx),
                paint("fill", color)
            ));
            for (i, line) in lines.iter().enumerate() {
                let y = first + f64::from(line_h) * i as f64 + f64::from(dx);
                body.push_str(&format!(
                    "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                    f64::from(w) / 2.0 + f64::from(dx),
                    (y * 100.0).round() / 100.0,
                    escape(line)
                ));
            }
            body.push_str("</text>");
        };
        if let Some(sh) = spec.shadow {
            text_el(off, sh);
        }
        text_el(0.0, spec.fg);
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    ));
    if !defs.is_empty() {
        out.push_str(&format!("<defs>{defs}</defs>"));
    }
    if radius > 0 {
        out.push_str(&format!("<g clip-path=\"url(#clip)\">{body}</g>"));
    } else {
        out.push_str(&body);
    }
    out.push_str("</svg>\n");
    out
}

/// Standard base64 (RFC 4648, with padding), so the crate needs no
/// dependency for the one data URI.
mod base64_lite {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
            ];
            let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(label: Option<&str>) -> SvgSpec<'_> {
        SvgSpec {
            size: Size {
                width: 640,
                height: 480,
            },
            bg: Rgba([0x95, 0x95, 0x95, 255]),
            fg: Rgba([255, 255, 255, 255]),
            pattern: Pattern::Solid,
            cell: None,
            radius: None,
            border: None,
            shadow: None,
            label,
            font_px: None,
            font_family: "sans-serif",
            embed: None,
        }
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        // RFC 4648 section 10.
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_lite::encode(input.as_bytes()), want);
        }
    }

    #[test]
    fn solid_with_label() {
        let svg = render(&spec(Some("640x480")));
        assert!(
            svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"480\"")
        );
        assert!(svg.contains("<rect width=\"640\" height=\"480\" fill=\"#959595\"/>"));
        assert!(svg.contains(">640x480</tspan>"));
        assert!(!svg.contains("<defs>"));
    }

    #[test]
    fn label_is_escaped_and_split() {
        let svg = render(&spec(Some("a<b>&\"c\"\\nnext")));
        assert!(svg.contains(">a&lt;b&gt;&amp;&quot;c&quot;</tspan>"));
        assert!(svg.contains(">next</tspan>"));
        assert!(!svg.contains("<b>"));
    }

    #[test]
    fn patterns_radius_border_and_alpha() {
        let mut s = spec(None);
        s.pattern = Pattern::Checker;
        s.cell = Some(10);
        s.radius = Some(24);
        s.border = Some((4, Rgba([0, 0, 0, 128])));
        s.bg = Rgba([255, 0, 0, 64]);
        let svg = render(&s);
        assert!(svg.contains("<pattern id=\"bgp\" width=\"20\" height=\"20\""));
        assert!(svg.contains("fill-opacity=\"0.251\""));
        assert!(svg.contains("rx=\"24\""));
        assert!(svg.contains("stroke=\"#000000\" stroke-opacity=\"0.502\" stroke-width=\"4\""));
        assert!(svg.contains("rx=\"22\""));
        assert!(svg.contains("clip-path=\"url(#clip)\""));

        s.pattern = Pattern::Gradient;
        assert!(render(&s).contains("<linearGradient id=\"bgp\""));
        s.pattern = Pattern::Diag;
        assert!(render(&s).contains("<polygon points=\"0,0 10,0 0,10\""));
    }
}
