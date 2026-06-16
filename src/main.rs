//! Command-line entry point for placehold.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use image::DynamicImage;

use placehold::{
    apply_border, apply_radius, default_filename, ext_for_output, parse_color, parse_size, render,
    render_checker, render_diag, render_gradient, Size,
};

/// Generate placeholder images locally (solid color + size label).
#[derive(Parser, Debug)]
#[command(name = "placehold", version, about, long_about = None)]
struct Cli {
    /// One or more sizes: "WxH" (e.g. 640x480) or "N" (meaning NxN).
    #[arg(required = true)]
    sizes: Vec<String>,

    /// Background color (hex, e.g. "959595" or "#abc").
    #[arg(long, default_value = "959595")]
    bg: String,

    /// Text/label color (hex).
    #[arg(long, default_value = "ffffff")]
    fg: String,

    /// Override the centered label (default: the dimensions, e.g. "640x480").
    #[arg(long)]
    text: Option<String>,

    /// Do not draw any label (solid color only).
    #[arg(long)]
    no_text: bool,

    /// Output format / extension (used for default filenames).
    #[arg(long, value_parser = ["png", "jpg", "webp"], default_value = "png")]
    format: String,

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

    /// Fixed text scale (default: chosen automatically to fit).
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
    if cli.radius.is_some_and(|r| r > 0) && cli.format == "jpg" && cli.output.is_none() {
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

        let mut img = match cli.pattern.as_str() {
            "checker" => render_checker(size, bg, fg, label.as_deref(), cli.scale, cli.cell),
            "diag" => render_diag(size, bg, fg, label.as_deref(), cli.scale, cli.cell),
            "gradient" => render_gradient(size, bg, fg, label.as_deref(), cli.scale),
            _ => render(size, bg, fg, label.as_deref(), cli.scale),
        };
        if let Some(t) = cli.border {
            apply_border(&mut img, t, border_color);
        }
        if let Some(r) = cli.radius {
            apply_radius(&mut img, r);
        }
        let (path, ext) = resolve_output(&cli, size)?;
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
