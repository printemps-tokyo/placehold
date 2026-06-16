//! Integration tests: generate real images and decode them back.

use image::{GenericImageView, Rgba};
use placehold::{parse_color, parse_size, render};

#[test]
fn renders_and_encodes_a_png() {
    let dir = tempdir();
    let size = parse_size("120x80").unwrap();
    let bg = parse_color("959595").unwrap();
    let fg = parse_color("ffffff").unwrap();
    let img = render(size, bg, fg, Some("120x80"), None);

    let path = dir.join("out.png");
    img.save(&path).unwrap();

    let decoded = image::open(&path).unwrap();
    assert_eq!(decoded.dimensions(), (120, 80));
    // Top-left corner keeps the background color.
    assert_eq!(decoded.get_pixel(0, 0), Rgba([0x95, 0x95, 0x95, 255]));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_text_is_a_solid_fill() {
    let size = parse_size("16").unwrap();
    let bg = parse_color("#000").unwrap();
    let fg = parse_color("#fff").unwrap();
    let img = render(size, bg, fg, None, None);
    // Every pixel is the background color.
    assert!(img.pixels().all(|p| *p == Rgba([0, 0, 0, 255])));
}

fn tempdir() -> std::path::PathBuf {
    // A unique directory under the OS temp dir without extra dependencies.
    let base = std::env::temp_dir();
    let mut dir = base.join(format!("placehold-test-{}", std::process::id()));
    let mut n = 0;
    while dir.exists() {
        n += 1;
        dir = base.join(format!("placehold-test-{}-{}", std::process::id(), n));
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
