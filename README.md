# placehold

> Generate placeholder images locally — solid color with the size label, in any TrueType/OpenType font, as PNG, JPEG, WebP or SVG. Fast Rust CLI.

[![CI](https://github.com/printemps-tokyo/placehold/actions/workflows/ci.yml/badge.svg)](https://github.com/printemps-tokyo/placehold/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

`placehold` makes placeholder images — a solid-colored rectangle with the
dimensions drawn in the center (e.g. "640x480") — entirely on your machine. No
external service, no ImageMagick: it is a single static binary that renders in
pure Rust.

## Why

Mocking up a layout or an EC product grid means dropping in dummy images at
specific sizes. Online placeholder services work until you are offline, rate
limited, or do not want to leak your sizes to a third party. `placehold`
generates them instantly and locally, naming each file by its size.

## Install

```bash
cargo install --git https://github.com/printemps-tokyo/placehold
```

Tagged [releases](https://github.com/printemps-tokyo/placehold/releases) carry prebuilt binaries for Linux (x86_64)
and macOS (x86_64, Apple silicon), with SHA-256 checksums.

## Usage

```bash
placehold 640x480                 # -> 640x480_959595.png
placehold 100                     # square -> 100x100_959595.png
placehold 515x230 --bg "#c8c8c8"  # custom background
placehold 640x480 --bg 0000       # transparent background (png/webp)
placehold 640x480 --bg "#00000080"         # semi-transparent background
placehold 1200x630 --text "OGP"   # custom label
placehold 600x400 --text "HELLO\nWORLD"    # multi-line label
placehold 64 --no-text            # solid color only
placehold 300x150 --format jpg
placehold 800x600 --pattern checker        # checkerboard background
placehold 800x600 --pattern diag           # diagonal stripes
placehold 800x600 --pattern gradient       # left-to-right gradient
placehold 600x400 --radius 24 --border 4   # rounded corners + border (png)
placehold 800x450 --shadow                 # drop shadow under the label
placehold 800x450 --shadow-color "#333"    # ...with a custom shadow color
placehold 512 --format webp
placehold 640x480 --font NotoSansJP-Bold.ttf --text "商品画像\n640×480"   # any font
placehold 640x480 --format svg             # vector output -> 640x480_959595.svg
placehold 1200x630 -o ogp.svg --font-family "Georgia, serif"
placehold 320x240 640x480 800x600 --out-dir mockups   # batch
placehold 200x200 -o avatar.png   # explicit output path (single size)
```

Each generated file is named `<W>x<H>[_<bg>].<ext>` by default. The run summary
is printed to stderr.

## Options

| Option | Description |
| --- | --- |
| `<sizes>...` | One or more sizes: `WxH` (e.g. `640x480`) or `N` (meaning `NxN`) |
| `--bg <hex>` | Background color, e.g. `959595`, `#abc`, or `0000` (default `959595`) |
| `--fg <hex>` | Label color (default `ffffff`) |
| `--text <str>` | Override the centered label (use `\n` for multiple lines) |
| `--no-text` | Draw a solid color with no label (conflicts with `--text`) |
| `--format <png\|jpg\|jpeg\|webp\|svg>` | Output format (default `png`; `jpeg` is an alias for `jpg`) |
| `--font <file>` | Draw the label with a TrueType/OpenType font (`.ttf`, `.otf`, first face of a `.ttc`); SVG output embeds it |
| `--font-size <px>` | Label size in pixels per em (CSS `font-size`), 1-1024 (default: chosen to fit); needs `--font` or SVG output |
| `--font-family <list>` | CSS font-family of the SVG label when no `--font` is embedded (default `sans-serif`) |
| `--pattern <solid\|checker\|diag\|gradient>` | Background pattern (default `solid`) |
| `--cell <n>` | Checker/diagonal cell (stripe) size in pixels (default: auto) |
| `--radius <n>` | Rounded-corner radius in pixels (transparent corners; use png/webp) |
| `--border <n>` | Border thickness in pixels |
| `--border-color <hex>` | Border color (default: the label color) |
| `--shadow` | Draw a drop shadow under the label |
| `--shadow-color <hex>` | Shadow color (default: black; implies `--shadow`) |
| `--scale <n>` | Fixed label scale of the built-in 8x8 font (default: auto-fit); with `--font` or SVG, a size of 8 x n pixels (n up to 128) |
| `-o, --output <file>` | Output path (single size only) |
| `--out-dir <dir>` | Directory to write into (default: current directory) |

## Color formats

Every color option (`--bg`, `--fg`, `--border-color`, `--shadow-color`) accepts
hex with or without a leading `#`:

| Form | Digits | Example | Meaning |
| --- | --- | --- | --- |
| RGB | 3 | `f00` | opaque, each digit doubled (`ff0000`) |
| RGBA | 4 | `f00a` | with alpha, each digit doubled (`ff0000aa`) |
| RRGGBB | 6 | `ff0000` | opaque |
| RRGGBBAA | 8 | `ff000080` | with alpha |

Alpha is kept in PNG and WebP output; JPEG has no alpha channel, so it is
flattened when encoding. `--bg 0000` gives a fully transparent background —
handy for UI mockups where only the label should show.

## Fonts

Without `--font`, the label uses the public-domain `font8x8` bitmap font,
scaled by whole pixels; it covers ASCII only, and placehold says so when the
label has other characters. `--font` loads any TrueType or OpenType file and
draws the label anti-aliased with that font's advances and kerning, which also
covers Japanese and other scripts where one character is one glyph:

```bash
placehold 800x450 --font /System/Library/Fonts/Supplemental/Arial.ttf --shadow
placehold 640x360 --font NotoSansJP-Bold.ttf --text "商品画像\n640×360" --bg e8e0d0 --fg 5a3e2b
```

The size is chosen so every line fits in 80% of the width and the block in
80% of the height, as with the bitmap font; `--font-size` fixes it, in pixels
per em like CSS `font-size`, so a PNG and an SVG of the same label match. A
character the font has no glyph for is reported on stderr (it would draw as
the font's empty box). There is no text shaping: characters are placed one
glyph each, so scripts that need contextual forms or mark positioning (Arabic,
Devanagari, combining accents) do not come out right. Labels are capped at
1024 pixels per em, and glyphs off the image or with an outsized outline box
are skipped, because each glyph is rasterized into a buffer of its own size.

## SVG output

`--format svg`, or an `-o` path ending in `.svg`, writes a vector image with
the same background, pattern, radius, border, shadow and label:

- Patterns are SVG `<pattern>`s with the same cell sizes and the same derived
  second shade as the raster ones, and the gradient runs between the same
  pixel centers as the PNG's. Rendered with librsvg at 300x200, `checker`,
  `gradient` and `solid` matched the PNG on every pixel within a 2% color
  tolerance; `diag` differs only along its anti-aliased stripe edges. A
  translucent background keeps its alpha once over the whole pattern, and
  `--radius` clips the whole image.
- The border follows the rounded corners (the raster border is a square
  ring cut by the radius).
- The label is real text, centered with `text-anchor="middle"`, with its
  spaces kept (`xml:space="preserve"`). Without
  `--font`, it uses `--font-family` and the viewer's fonts, sized from an
  estimate of the text width (0.6 em per character, a full em for CJK). With
  `--font`, the font file is embedded as a base64 `@font-face`, so the image
  looks the same everywhere and the label is sized from the font's real
  metrics, at the cost of the font's size in every file.

## Notes
- JPEG output drops the alpha channel; PNG keeps it.

## Library

```rust
use placehold::{parse_color, parse_size, render};

let size = parse_size("640x480")?;
let img = render(size, parse_color("959595")?, parse_color("fff")?, Some("640x480"), None);
img.save("out.png")?;
# Ok::<(), anyhow::Error>(())
```

## Credits

The built-in label font is [font8x8](https://github.com/dhepper/font8x8) by
Daniel Hepper (public domain), via the `font8x8` crate. Font files are read
and rasterized with [ab_glyph](https://github.com/alexheretic/ab-glyph)
(Apache-2.0) and ttf-parser (MIT or Apache-2.0). The test font
`tests/fixtures/boxes.ttf` is generated by `scripts/gen_test_font.py` and
contains only boxes.

## License

[MIT](./LICENSE) (c) printemps.tokyo
