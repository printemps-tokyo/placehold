//! Command-line entry point for placehold.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use image::DynamicImage;

use placehold::font::{LabelFont, MAX_FONT_PX};
use placehold::svg::{self, Pattern, SvgSpec};
use placehold::{
    apply_border, apply_radius, default_filename, draw_shadowed_label, ext_for_output, parse_color,
    parse_size, render, render_checker, render_diag, render_gradient, Size,
};

/// Generate placeholder images locally (solid color + size label).
#[derive(Parser, Debug)]
#[command(name = "placehold", version, about, long_about = None)]
struct Cli {
    /// One or more sizes: "WxH" (e.g. 640x480) or "N" (meaning NxN).
    #[arg(required = true)]
    sizes: Vec<String>,

    /// Background color (3/4/6/8-digit hex, e.g. "959595", "#abc", "0000").
    #[arg(long, default_value = "959595")]
    bg: String,

    /// Text/label color (3/4/6/8-digit hex).
    #[arg(long, default_value = "ffffff")]
    fg: String,

    /// Override the centered label (default: the dimensions, e.g. "640x480").
    #[arg(long, conflicts_with = "no_text")]
    text: Option<String>,

    /// Do not draw any label (solid color only).
    #[arg(long)]
    no_text: bool,

    /// Output format / extension (used for default filenames).
    #[arg(long, value_parser = ["png", "jpg", "jpeg", "webp", "svg"], default_value = "png")]
    format: String,

    /// Draw the label with this TrueType/OpenType font (.ttf/.otf; the first
    /// face of a .ttc). SVG output embeds it.
    #[arg(long, value_name = "FILE")]
    font: Option<PathBuf>,

    /// Label size in pixels per em, as CSS font-size (default: chosen to
    /// fit). Needs --font or SVG output; the built-in font uses --scale.
    #[arg(long, value_name = "PX", conflicts_with = "scale",
          value_parser = clap::value_parser!(u32).range(1..=1024))]
    font_size: Option<u32>,

    /// CSS font-family for the SVG label when no --font is embedded.
    #[arg(long, value_name = "LIST", default_value = "sans-serif")]
    font_family: String,

    /// Background pattern.
    #[arg(long, value_parser = ["solid", "checker", "diag", "gradient"], default_value = "solid")]
    pattern: String,

    /// Checker/diagonal cell (stripe) size in pixels (default: auto).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=4096))]
    cell: Option<u32>,

    /// Rounded-corner radius in pixels (transparent corners; use png/webp).
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=10000))]
    radius: Option<u32>,

    /// Border thickness in pixels.
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=10000))]
    border: Option<u32>,

    /// Border color (hex; default: the label color).
    #[arg(long)]
    border_color: Option<String>,

    /// Draw a drop shadow under the label.
    #[arg(long)]
    shadow: bool,

    /// Shadow color (hex; default: black). Implies --shadow.
    #[arg(long)]
    shadow_color: Option<String>,

    /// Fixed text scale of the built-in 8x8 font (default: chosen to fit);
    /// with --font or SVG output, a size of 8 x scale pixels.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=256))]
    scale: Option<u32>,

    /// Output file (single size only). Its extension sets the format.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Directory to write generated files into (default: current directory).
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let bg = parse_color(&cli.bg)?;
    let fg = parse_color(&cli.fg)?;
    // Border color defaults to the label color.
    let border_color = match &cli.border_color {
        Some(hex) => parse_color(hex)?,
        None => fg,
    };
    // Shadow color (black by default); --shadow-color implies --shadow.
    let shadow_color = if cli.shadow || cli.shadow_color.is_some() {
        Some(match &cli.shadow_color {
            Some(hex) => parse_color(hex)?,
            None => image::Rgba([0, 0, 0, 255]),
        })
    } else {
        None
    };
    if cli.radius.is_some_and(|r| r > 0)
        && matches!(cli.format.as_str(), "jpg" | "jpeg")
        && cli.output.is_none()
    {
        eprintln!(
            "placehold: note: --radius needs alpha; jpg corners will be opaque (use png/webp)"
        );
    }

    if cli.output.is_some() && cli.sizes.len() > 1 {
        return Err(anyhow!("--output cannot be used with multiple sizes"));
    }
    if cli.output.is_some() && cli.out_dir.is_some() {
        return Err(anyhow!("--output and --out-dir cannot be used together"));
    }

    let font = match &cli.font {
        Some(path) => Some(LabelFont::load(path)?),
        None => None,
    };
    let vector = wants_svg(&cli);
    if cli.font_size.is_some() && font.is_none() && !vector {
        return Err(anyhow!(
            "--font-size sizes a --font or SVG label; the built-in 8x8 font uses --scale"
        ));
    }
    if (font.is_some() || vector) && cli.scale.is_some_and(|s| s > 128) {
        return Err(anyhow!(
            "with --font or SVG output, --scale is at most 128 (8 x 128 = 1024 pixels)"
        ));
    }
    let font_px = cli
        .font_size
        .map(|p| p as f32)
        .or(cli.scale.map(|s| (s * 8) as f32))
        .map(|p| p.min(MAX_FONT_PX));
    if let Some(text) = &cli.text {
        match &font {
            Some(f) => {
                let missing = f.missing_chars(&text.replace("\\n", "\n"));
                if !missing.is_empty() {
                    eprintln!(
                        "placehold: note: the font has no glyph for {}",
                        missing
                            .iter()
                            .map(|c| format!("{c:?}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                }
            }
            None if !text.is_ascii() && !vector => {
                eprintln!(
                    "placehold: note: the built-in 8x8 font draws ASCII only; pass --font FILE for other characters"
                );
            }
            None => {}
        }
    }

    if let Some(dir) = &cli.out_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }

    for raw in &cli.sizes {
        let size = parse_size(raw)?;
        let label = if cli.no_text {
            None
        } else {
            Some(
                cli.text
                    .clone()
                    .unwrap_or_else(|| format!("{}x{}", size.width, size.height)),
            )
        };

        let (path, ext) = resolve_output(&cli, size)?;
        if ext == "svg" {
            let doc = svg::render(&SvgSpec {
                size,
                bg,
                fg,
                pattern: match cli.pattern.as_str() {
                    "checker" => Pattern::Checker,
                    "diag" => Pattern::Diag,
                    "gradient" => Pattern::Gradient,
                    _ => Pattern::Solid,
                },
                cell: cli.cell,
                radius: cli.radius,
                border: cli.border.map(|t| (t, border_color)),
                shadow: shadow_color,
                label: label.as_deref(),
                font_px,
                font_family: &cli.font_family,
                embed: font.as_ref(),
            });
            std::fs::write(&path, doc)
                .with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!(
                "placehold: wrote {} ({}x{})",
                path.display(),
                size.width,
                size.height
            );
            continue;
        }

        // Render the background without the label, then draw the label so an
        // optional drop shadow can sit under the text.
        let mut img = match cli.pattern.as_str() {
            "checker" => render_checker(size, bg, fg, None, cli.scale, cli.cell),
            "diag" => render_diag(size, bg, fg, None, cli.scale, cli.cell),
            "gradient" => render_gradient(size, bg, fg, None, cli.scale),
            _ => render(size, bg, fg, None, cli.scale),
        };
        match (&font, label.as_deref()) {
            (Some(f), Some(text)) if !text.is_empty() => {
                f.draw(&mut img, size, fg, shadow_color, text, font_px)
            }
            _ => draw_shadowed_label(
                &mut img,
                size,
                fg,
                shadow_color,
                label.as_deref(),
                cli.scale,
            ),
        }
        if let Some(t) = cli.border {
            apply_border(&mut img, t, border_color);
        }
        if let Some(r) = cli.radius {
            apply_radius(&mut img, r);
        }
        save_image(&img, &path, &ext)
            .with_context(|| format!("failed to write {}", path.display()))?;
        eprintln!(
            "placehold: wrote {} ({}x{})",
            path.display(),
            size.width,
            size.height
        );
    }

    Ok(())
}

/// Whether this run writes SVG (by --output extension or --format).
fn wants_svg(cli: &Cli) -> bool {
    match &cli.output {
        Some(out) => ext_for_output(out).is_ok_and(|e| e == "svg"),
        None => cli.format == "svg",
    }
}

/// Resolve the output path and the encoder extension for one size.
/// With --output the extension comes from the given path (so the file's name
/// and contents agree); otherwise it is the default filename + --format.
fn resolve_output(cli: &Cli, size: Size) -> Result<(PathBuf, String)> {
    if let Some(out) = &cli.output {
        let ext = ext_for_output(out)?;
        return Ok((out.clone(), ext));
    }
    let ext = cli.format.clone();
    let name = default_filename(size, Some(&cli.bg), &ext);
    let path = match &cli.out_dir {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    };
    Ok((path, ext))
}

fn save_image(img: &image::RgbaImage, path: &Path, ext: &str) -> Result<()> {
    if ext == "jpg" || ext == "jpeg" {
        // JPEG has no alpha channel; drop it before encoding.
        DynamicImage::ImageRgba8(img.clone()).to_rgb8().save(path)?;
    } else {
        img.save(path)?;
    }
    Ok(())
}
