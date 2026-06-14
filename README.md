# Carmine

Carmine is a small pure-Rust HTML/CSS renderer for Scarlet OS.

It parses static HTML, applies a focused CSS cascade, computes layout with
`taffy`, and paints the result into a `tiny-skia` pixmap. Scarlet OS is the
primary interactive frontend, but the renderer core is fully independent and
can also run headlessly to produce PNG output.

## Workspace Layout

```text
carmine/
├── Cargo.toml                 [workspace]
│
├── carmine-core/              renderer core — zero Scarlet dependency
│   └── src/
│       ├── lib.rs             RenderPipeline, Pixmap re-export, DEFAULT_HTML
│       └── render/
│           ├── mod.rs         HTML → style → layout → paint pipeline
│           ├── css.rs         CSS parsing, var() substitution, selector matching
│           ├── style.rs       cascade, inheritance, value parsing
│           ├── layout.rs      taffy tree construction and text measurement
│           └── paint.rs       raster painting with tiny-skia
│
└── carmine/                   browser application
    └── src/
        ├── main.rs            CLI entry point (clap)
        ├── webview.rs         WebView View — ScarletUI CanvasView wrapper  [scarlet]
        ├── browser.rs         BrowserApp — Window + WebView                [scarlet]
        └── bridge.rs          RGBA → BGRA pixel conversion                 [scarlet]
```

The `scarlet` feature gate controls Scarlet UI integration. Without it, carmine
runs headlessly (PNG dump only). The core crate has no feature flags at all.

## What It Does

- Loads an HTML file from disk or renders a built-in demo page.
- Extracts inline `<style>` blocks and applies a CSS cascade.
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

## Running

Run the Scarlet viewer:

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
cargo build -p carmine-core
```

## Using The Core

The core renderer is a standalone library crate:

```rust
use carmine_core::{RenderPipeline, Pixmap};

let html = "<h1>Hello</h1>";
let pipeline = RenderPipeline::new(html, 800, 600);
let pixmap: Pixmap = pipeline.render(800, 600, 1.0);
pixmap.save_png("/tmp/hello.png")?;
```

## Scarlet Integration

The `scarlet` feature enables the interactive frontend:

- `webview.rs` implements a ScarletUI `View` that wraps a `CanvasView` with
  the render pipeline. It is designed to be reusable — any ScarletUI app can
  embed a `WebView` component.
- `browser.rs` is the `BrowserApp` that creates a `Window` containing a
  `WebView`. Future toolbar and navigation UI will be added here.
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
cargo build -p carmine-core
cargo build -p carmine --no-default-features
cargo build -p carmine --target riscv64gc-unknown-scarlet
cargo run --no-default-features -- path/to/page.html --dump-png /tmp/out.png
```

## Current Limits

Carmine is still a static document renderer. It does not implement JavaScript,
network loading, forms, focus handling, scrolling, browser font fallback, or the
full CSS layout and painting model.

When in doubt, prefer improving the renderer over changing test pages to fit it.
The useful comparison is whether ordinary HTML and CSS render closer to a modern
browser without adding page-specific shortcuts.
