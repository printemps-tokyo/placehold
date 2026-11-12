//! Labels drawn with a TrueType/OpenType font (`--font`), anti-aliased and
//! alpha-blended onto the image. Layout uses the font's own advances and
//! kerning; lines are stacked by the font's line height and centered.
//!
//! Every size here is an em size in pixels, what CSS calls font-size, so a
//! PNG and an SVG of the same label agree. ab_glyph scales by the height
//! from descender to ascender instead; `px_scale` converts (the same
//! arithmetic as ab_glyph's `pt_to_px_scale`). There is no text shaping:
//! each character maps to one glyph, which suits Latin, Greek, Cyrillic and
//! CJK labels but not scripts that need contextual forms.

use ab_glyph::{point, Font, FontVec, GlyphId, PxScale, ScaleFont};
use anyhow::{anyhow, Context, Result};
use image::{Rgba, RgbaImage};

use crate::{split_label, Size};

/// Largest label size in pixels. A glyph is rasterized into a coverage
/// buffer of its own size, so this bounds memory per glyph.
pub const MAX_FONT_PX: f32 = 1024.0;

/// A glyph whose pixel box is larger than this is not drawn: some fonts have
/// outlines far taller than their line height, and the rasterizer allocates
/// the whole box.
const MAX_GLYPH_AREA: f32 = 4096.0 * 4096.0;

/// A loaded label font.
pub struct LabelFont {
    font: FontVec,
    /// The file bytes, kept so the SVG writer can embed them.
    data: Vec<u8>,
}

impl LabelFont {
    /// Load a .ttf/.otf file (for a .ttc collection, the first face).
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let data =
            std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_bytes(data).with_context(|| format!("not a usable font: {}", path.display()))
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let font = FontVec::try_from_vec_and_index(data.clone(), 0)
            .map_err(|_| anyhow!("not a TrueType/OpenType font"))?;
        Ok(LabelFont { font, data })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// The ab_glyph scale for an em size of `em` pixels.
    fn px_scale(&self, em: f32) -> PxScale {
        let per_em = self.font.units_per_em().unwrap_or(1000.0);
        let height = self.font.height_unscaled();
        if per_em > 0.0 && height.is_finite() && height > 0.0 {
            PxScale::from(em * height / per_em)
        } else {
            PxScale::from(em)
        }
    }

    /// Characters of `text` the font has no glyph for (notdef), in order,
    /// without repeats. Line breaks are ignored.
    pub fn missing_chars(&self, text: &str) -> Vec<char> {
        let mut out = Vec::new();
        for ch in text.chars() {
            if ch != '\n' && self.font.glyph_id(ch).0 == 0 && !out.contains(&ch) {
                out.push(ch);
            }
        }
        out
    }

    /// Width of one line at `px`, with kerning.
    pub fn line_width(&self, line: &str, px: f32) -> f32 {
        let scaled = self.font.as_scaled(self.px_scale(px));
        let mut width = 0.0;
        let mut prev: Option<GlyphId> = None;
        for ch in line.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                width += scaled.kern(p, id);
            }
            width += scaled.h_advance(id);
            prev = Some(id);
        }
        width
    }

    /// Line height (ascent - descent + line gap) at `px`.
    pub fn line_height(&self, px: f32) -> f32 {
        let s = self.font.as_scaled(self.px_scale(px));
        s.ascent() - s.descent() + s.line_gap()
    }

    /// The largest size at which every line fits in 80% of the image width
    /// and the block fits in 80% of its height (as the bitmap font's
    /// auto-scale does), capped at [`MAX_FONT_PX`].
    pub fn fit_px(&self, size: Size, lines: &[String]) -> f32 {
        let reference = 100.0;
        let widest = lines
            .iter()
            .map(|l| self.line_width(l, reference))
            .fold(0.0f32, f32::max);
        let tall = self.line_height(reference) * lines.len().max(1) as f32;
        let by_w = if widest > 0.0 {
            size.width as f32 * 0.8 * reference / widest
        } else {
            f32::INFINITY
        };
        let by_h = if tall > 0.0 {
            size.height as f32 * 0.8 * reference / tall
        } else {
            f32::INFINITY
        };
        by_w.min(by_h).clamp(1.0, MAX_FONT_PX)
    }

    /// Draw the label centered, with an optional drop shadow (offset by
    /// px/32, at least one pixel, like the bitmap label's offset of scale/4).
    pub fn draw(
        &self,
        img: &mut RgbaImage,
        size: Size,
        fg: Rgba<u8>,
        shadow: Option<Rgba<u8>>,
        text: &str,
        px: Option<f32>,
    ) {
        let lines = split_label(text);
        let px = px
            .unwrap_or_else(|| self.fit_px(size, &lines))
            .clamp(1.0, MAX_FONT_PX);
        let scaled = self.font.as_scaled(self.px_scale(px));
        let line_h = self.line_height(px);
        // Center the block of lines; within a line, the glyphs sit on the
        // baseline at ascent below the line top.
        let block_h = line_h * lines.len() as f32 - scaled.line_gap();
        let top = (size.height as f32 - block_h) / 2.0;
        let off = (px / 32.0).round().max(1.0);
        for (i, line) in lines.iter().enumerate() {
            let x0 = (size.width as f32 - self.line_width(line, px)) / 2.0;
            let baseline = top + i as f32 * line_h + scaled.ascent();
            if let Some(sh) = shadow {
                self.draw_line(img, line, px, x0 + off, baseline + off, sh);
            }
            self.draw_line(img, line, px, x0, baseline, fg);
        }
    }

    fn draw_line(
        &self,
        img: &mut RgbaImage,
        line: &str,
        px: f32,
        x0: f32,
        baseline: f32,
        color: Rgba<u8>,
    ) {
        let scale = self.px_scale(px);
        let scaled = self.font.as_scaled(scale);
        let mut x = x0;
        let mut prev: Option<GlyphId> = None;
        for ch in line.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                x += scaled.kern(p, id);
            }
            let glyph = id.with_scale_and_position(scale, point(x, baseline));
            x += scaled.h_advance(id);
            prev = Some(id);
            let Some(outlined) = self.font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outlined.px_bounds();
            // Skip glyphs wholly off the image (a long label at a fixed
            // size) and boxes too big to rasterize.
            let (w, h) = (img.width() as f32, img.height() as f32);
            let area = bounds.width() * bounds.height();
            if !area.is_finite()
                || area > MAX_GLYPH_AREA
                || bounds.max.x <= 0.0
                || bounds.max.y <= 0.0
                || bounds.min.x >= w
                || bounds.min.y >= h
            {
                continue;
            }
            outlined.draw(|gx, gy, coverage| {
                let px_x = bounds.min.x as i64 + i64::from(gx);
                let px_y = bounds.min.y as i64 + i64::from(gy);
                if px_x < 0
                    || px_y < 0
                    || px_x >= i64::from(img.width())
                    || px_y >= i64::from(img.height())
                {
                    return;
                }
                let dst = img.get_pixel_mut(px_x as u32, px_y as u32);
                *dst = blend(*dst, color, coverage.clamp(0.0, 1.0));
            });
        }
    }
}

/// Source-over blend of `src` at `coverage` onto `dst` (straight alpha).
pub fn blend(dst: Rgba<u8>, src: Rgba<u8>, coverage: f32) -> Rgba<u8> {
    let sa = f32::from(src.0[3]) / 255.0 * coverage;
    if sa <= 0.0 {
        return dst;
    }
    let da = f32::from(dst.0[3]) / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut out = [0u8; 4];
    for ((o, &sc), &dc) in out.iter_mut().zip(&src.0[..3]).zip(&dst.0[..3]) {
        let v = (f32::from(sc) * sa + f32::from(dc) * da * (1.0 - sa)) / out_a;
        *o = v.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_edges() {
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        assert_eq!(blend(blue, red, 0.0), blue);
        assert_eq!(blend(blue, red, 1.0), red);
        let half = blend(blue, red, 0.5);
        assert_eq!(half.0[3], 255);
        assert!(half.0[0] > 120 && half.0[0] < 135, "{half:?}");
        // Onto a transparent pixel, the color keeps its hue.
        let t = blend(Rgba([0, 0, 0, 0]), red, 0.5);
        assert_eq!((t.0[0], t.0[3]), (255, 128));
    }

    #[test]
    fn rejects_non_fonts() {
        assert!(LabelFont::from_bytes(b"not a font".to_vec()).is_err());
    }
}
