//! --font and SVG output, end to end, with the box font from
//! scripts/gen_test_font.py (tests/fixtures/boxes.ttf).

use image::{GenericImageView, Rgba};
use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_placehold");

fn font() -> String {
    let p: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "boxes.ttf"]
        .iter()
        .collect();
    p.to_str().unwrap().to_string()
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("placehold-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(BIN).args(args).output().unwrap()
}

#[test]
fn font_label_is_centered_on_the_advance() {
    let dir = tempdir("font");
    let out = dir.join("f.png");
    let o = run(&[
        "100x50",
        "--font",
        &font(),
        "--text",
        "0",
        "--font-size",
        "40",
        "--bg",
        "000",
        "--fg",
        "fff",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let img = image::open(&out).unwrap();
    assert_eq!(img.dimensions(), (100, 50));
    // The '0' box spans x 80..520 of a 600-unit advance at 40 px: the
    // advance is 24 px, centered at 38..62, ink at about 41..59.
    let lit: Vec<u32> = (0..100)
        .filter(|&x| img.get_pixel(x, 25) == Rgba([255, 255, 255, 255]))
        .collect();
    assert!(!lit.is_empty());
    let (first, last) = (*lit.first().unwrap(), *lit.last().unwrap());
    assert!((40..=42).contains(&first), "{lit:?}");
    assert!((57..=59).contains(&last), "{lit:?}");
    assert_eq!(img.get_pixel(5, 5), Rgba([0, 0, 0, 255]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_glyphs_and_ascii_only_bitmap_are_noted() {
    let dir = tempdir("notes");
    let o = run(&[
        "64",
        "--font",
        &font(),
        "--text",
        "0Z",
        "-o",
        dir.join("a.png").to_str().unwrap(),
    ]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("no glyph for 'Z'"));

    let o = run(&[
        "64",
        "--text",
        "\u{53e3}",
        "-o",
        dir.join("b.png").to_str().unwrap(),
    ]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("ASCII only"));

    let o = run(&["64", "--font", "/no/such/font.ttf"]);
    assert!(!o.status.success());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_output_by_format_and_by_extension() {
    let dir = tempdir("svg");
    let o = run(&[
        "640x480",
        "--format",
        "svg",
        "--pattern",
        "checker",
        "--radius",
        "12",
        "--border",
        "3",
        "--shadow",
        "--out-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let svg = std::fs::read_to_string(dir.join("640x480_959595.svg")).unwrap();
    assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(svg.contains("width=\"640\" height=\"480\""));
    assert!(svg.contains("<pattern id=\"bgp\""));
    assert!(svg.contains("clip-path=\"url(#clip)\""));
    assert_eq!(
        svg.matches(">640x480</tspan>").count(),
        2,
        "label and shadow"
    );
    assert!(svg.contains("font-family=\"sans-serif\""));

    let path = dir.join("label.svg");
    let o = run(&[
        "300x100",
        "--text",
        "<b>&",
        "--font-family",
        "Georgia, \"Times New Roman\", serif",
        "-o",
        path.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(svg.contains(">&lt;b&gt;&amp;</tspan>"));
    assert!(svg.contains("font-family=\"Georgia, &quot;Times New Roman&quot;, serif\""));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn svg_embeds_the_font() {
    let dir = tempdir("embed");
    let path = dir.join("e.svg");
    let o = run(&["200x100", "--font", &font(), "-o", path.to_str().unwrap()]);
    assert!(o.status.success());
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(
        svg.contains("@font-face{font-family:\"placehold-label\";src:url(data:font/ttf;base64,")
    );
    assert!(svg.contains("font-family=\"placehold-label, sans-serif\""));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn font_size_and_scale_conflict() {
    let o = run(&["64", "--font-size", "20", "--scale", "2"]);
    assert!(!o.status.success());
}

#[test]
fn svg_keeps_spaces_alpha_once_and_pixel_center_gradient() {
    let dir = tempdir("svgfix");
    let path = dir.join("a.svg");
    let o = run(&[
        "300x100",
        "--text",
        " 0  0 ",
        "--pattern",
        "checker",
        "--bg",
        "ff000080",
        "-o",
        path.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(svg.contains("xml:space=\"preserve\""));
    assert!(svg.contains("> 0  0 </tspan>"));
    // Opaque pattern colors, the alpha once on the rect.
    assert!(svg.contains("fill=\"url(#bgp)\" fill-opacity=\"0.502\""));
    assert_eq!(svg.matches("fill-opacity").count(), 1, "{svg}");

    let o = run(&[
        "301x10",
        "--pattern",
        "gradient",
        "--no-text",
        "-o",
        path.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    let svg = std::fs::read_to_string(&path).unwrap();
    assert!(svg.contains("gradientUnits=\"userSpaceOnUse\" x1=\"0.5\" y1=\"0\" x2=\"300.5\""));
    let o = run(&[
        "1x10",
        "--pattern",
        "gradient",
        "--no-text",
        "-o",
        path.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("linearGradient"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn size_flags_that_would_be_ignored_are_errors() {
    let o = run(&["64", "--font-size", "20"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("--scale"));
    let o = run(&[
        "64",
        "--font",
        &font(),
        "--scale",
        "200",
        "-o",
        "/dev/null.png",
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("at most 128"));
}
