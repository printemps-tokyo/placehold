//! Command-line entry point for placehold.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use image::DynamicImage;

use placehold::{default_filename, parse_color, parse_size, render, Size};

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

    /// Output format / extension.
    #[arg(long, value_parser = ["png", "jpg"], default_value = "png")]
    format: String,

    /// Fixed text scale (default: chosen automatically to fit).
    #[arg(long)]
    scale: Option<u32>,

    /// Output file (single size only). Default: "<W>x<H>[_<bg>].<ext>".
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
    let ext = cli.format.as_str();

    if cli.output.is_some() && cli.sizes.len() > 1 {
        return Err(anyhow!("--output cannot be used with multiple sizes"));
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

        let img = render(size, bg, fg, label.as_deref(), cli.scale);
        let path = output_path(&cli, size, ext);
        save_image(&img, &path, ext)
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

fn output_path(cli: &Cli, size: Size, ext: &str) -> PathBuf {
    if let Some(out) = &cli.output {
        return out.clone();
    }
    let name = default_filename(size, Some(&cli.bg), ext);
    match &cli.out_dir {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

fn save_image(img: &image::RgbaImage, path: &std::path::Path, ext: &str) -> Result<()> {
    if ext == "jpg" || ext == "jpeg" {
        // JPEG has no alpha channel; drop it before encoding.
        DynamicImage::ImageRgba8(img.clone()).to_rgb8().save(path)?;
    } else {
        img.save(path)?;
    }
    Ok(())
}
