# Carmine

Carmine is a small pure-Rust HTML/CSS renderer.

It parses static HTML, applies a focused CSS cascade, computes layout with
`taffy`, and paints the result into a `tiny-skia` pixmap. Scarlet OS is the
first interactive frontend, but the renderer itself is no longer tied to
Scarlet: it can also run headlessly and dump PNGs without building any Scarlet
UI dependencies.

## Shape

Carmine is split into two layers:

- `carmine` library: renderer core, independent of Scarlet.
- `scarlet` feature: Scarlet UI window and canvas frontend.

The default build enables the Scarlet frontend so existing `cargo run` behavior
still opens a Carmine window. Disable default features when you only need the
renderer or PNG output.

## What It Does

- Loads an HTML file from disk, or a built-in demo page when no file is given.
- Extracts inline `<style>` blocks and applies a small CSS cascade.
- Lays out block, flex, grid, and inline text content with `taffy`.
- Paints into a `tiny-skia` pixmap.
- Presents through Scarlet UI when the `scarlet` feature is enabled.
- Renders headlessly to PNG for comparison and regression checks.

Supported CSS is intentionally focused, but already includes:

- Type, class, id, and descendant-style selector matching through `scraper`.
- CSS variables via `var(...)` from `:root`.
- `::before` and `::after` pseudo-elements.
- Margins, padding, borders, `border-left`, and rounded corners.
- Solid colors, `rgba(...)`, `linear-gradient`, `radial-gradient`, and
  `repeating-linear-gradient`.
- `display: block`, `inline`, `flex`, and `grid`.
- `gap`, `flex-wrap`, `justify-content`, `align-items`, and simple grid tracks.
- Font size units including `px`, `rem`, `vw`, and `clamp(...)`.
- Basic text wrapping, text transform, letter spacing, shadows, and list markers.

## Repository Layout

```text
src/
  lib.rs           renderer library entry point and built-in demo document
  main.rs          CLI entry point
  browser.rs       Scarlet UI window integration, behind the scarlet feature
  bridge.rs        tiny-skia pixmap to Scarlet canvas conversion
  render/
    mod.rs         HTML -> style -> layout -> paint pipeline
    css.rs         CSS parsing, variable substitution, selector matching
    style.rs       computed style and declaration handling
    layout.rs      taffy tree construction and text measurement
    paint.rs       raster painting with tiny-skia
```

## Running

Run the default Scarlet viewer:

```bash
cargo run
```

Render an HTML file in the Scarlet viewer:

```bash
cargo run -- path/to/page.html --width 1024 --height 768
```

Render to PNG without opening a window:

```bash
cargo run -- path/to/page.html --dump-png /tmp/carmine.png --width 900 --height 620
```

Render to PNG without Scarlet dependencies:

```bash
cargo run --no-default-features -- path/to/page.html --dump-png /tmp/carmine.png
```

Build only the renderer core:

```bash
cargo build --no-default-features
```

## Using The Core

The core renderer is available from the library:

```rust
use carmine::render::RenderPipeline;

let html = "<h1>Hello</h1>";
let pipeline = RenderPipeline::new(html, 800, 600);
let pixmap = pipeline.render(800, 600, 1.0);
pixmap.save_png("/tmp/hello.png")?;
```

## Scarlet Integration

The `scarlet` feature enables the interactive frontend:

- `browser.rs` creates a Scarlet UI `Window` with a `CanvasView`.
- `bridge.rs` swizzles tiny-skia RGBA pixels into the BGRA format expected by
  Scarlet UI.


## Development Notes

The renderer is split into four explicit stages:

1. Parse HTML and collect CSS from `<style>` tags.
2. Match selectors and compute inherited style.
3. Build a `taffy` layout tree.
4. Paint the layout tree into a `tiny-skia` pixmap.

This keeps visual mismatches easier to debug. A broken page can usually be
narrowed down to selector matching, style computation, layout, or painting.

Useful checks:

```bash
cargo fmt
cargo build
cargo build --no-default-features
cargo run --no-default-features -- path/to/page.html --dump-png /tmp/out.png
```

## Current Limits

Carmine is still a static document renderer. It does not implement JavaScript,
network loading, forms, focus handling, scrolling, browser font fallback, or the
full CSS layout and painting model.

When in doubt, prefer improving the renderer over changing test pages to fit it.
The useful comparison is whether ordinary HTML and CSS render closer to a modern
browser without adding page-specific shortcuts.
