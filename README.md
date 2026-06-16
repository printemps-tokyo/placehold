# placehold

> Generate placeholder images locally — solid color with the size label. Fast Rust CLI.

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

Or download a prebuilt binary from the [Releases](https://github.com/printemps-tokyo/placehold/releases) page.

## Usage

```bash
placehold 640x480                 # -> 640x480_959595.png
placehold 100                     # square -> 100x100_959595.png
placehold 515x230 --bg "#c8c8c8"  # custom background
placehold 1200x630 --text "OGP"   # custom label
placehold 64 --no-text            # solid color only
placehold 300x150 --format jpg
placehold 800x600 --pattern checker        # checkerboard background
placehold 800x600 --pattern diag           # diagonal stripes
placehold 800x600 --pattern gradient       # left-to-right gradient
placehold 600x400 --radius 24 --border 4   # rounded corners + border (png)
placehold 512 --format webp
placehold 320x240 640x480 800x600 --out-dir mockups   # batch
placehold 200x200 -o avatar.png   # explicit output path (single size)
```

Each generated file is named `<W>x<H>[_<bg>].<ext>` by default. The run summary
is printed to stderr.

## Options

| Option | Description |
| --- | --- |
| `<sizes>...` | One or more sizes: `WxH` (e.g. `640x480`) or `N` (meaning `NxN`) |
| `--bg <hex>` | Background color, e.g. `959595` or `#abc` (default `959595`) |
| `--fg <hex>` | Label color (default `ffffff`) |
| `--text <str>` | Override the centered label (default: the dimensions) |
| `--no-text` | Draw a solid color with no label |
| `--format <png\|jpg\|webp>` | Output format (default `png`) |
| `--pattern <solid\|checker\|diag\|gradient>` | Background pattern (default `solid`) |
| `--cell <n>` | Checker/diagonal cell (stripe) size in pixels (default: auto) |
| `--radius <n>` | Rounded-corner radius in pixels (transparent corners; use png/webp) |
| `--border <n>` | Border thickness in pixels |
| `--border-color <hex>` | Border color (default: the label color) |
| `--scale <n>` | Fixed label scale (default: auto-fit) |
| `-o, --output <file>` | Output path (single size only) |
| `--out-dir <dir>` | Directory to write into (default: current directory) |

## Notes

- Colors accept 3- or 6-digit hex, with or without a leading `#`.
- The label is rendered with the public-domain `font8x8` bitmap font and scaled
  to fit; pass `--scale` to fix it.
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

The label font is [font8x8](https://github.com/dhepper/font8x8) by Daniel Hepper
(public domain), via the `font8x8` crate.

## License

[MIT](./LICENSE) (c) printemps.tokyo
