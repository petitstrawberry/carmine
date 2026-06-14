pub mod render;

pub use render::{RenderOptions, RenderPipeline};
pub use tiny_skia::Pixmap;

pub const DEFAULT_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
<style>
body { background-color: #fafafa; padding: 24px; margin: 0; }
h1 { color: #dc143c; font-size: 28px; margin: 0 0 8px 0; }
p { color: #333; font-size: 14px; margin: 0 0 8px 0; }
.container { background-color: #fff; padding: 20px; margin-bottom: 16px; border-radius: 8px; border: 1px solid #e0e0e0; }
.highlight { color: #0066cc; font-weight: bold; }
.nav { display: flex; flex-direction: row; gap: 16px; padding: 12px; background-color: #1a1a2e; }
.nav a { color: #e0e0e0; font-size: 14px; }
.card-row { display: flex; flex-direction: row; gap: 12px; }
.card { background-color: #f0f4ff; padding: 16px; border-radius: 6px; border: 1px solid #c0d0f0; }
.card h3 { font-size: 16px; color: #333; margin: 0 0 8px 0; }
.card p { font-size: 12px; color: #666; }
</style>
</head>
<body>
<div class="nav">
<a>Home</a>
<a>About</a>
<a>Settings</a>
</div>
<div class="container">
<h1>Carmine</h1>
<p>A pure-Rust HTML/CSS renderer.</p>
<p>Powered by <span class="highlight">html5ever</span>, <span class="highlight">taffy</span>, and <span class="highlight">tiny-skia</span>.</p>
</div>
<div class="card-row">
<div class="card">
<h3>HTML5</h3>
<p>scraper + html5ever parsing</p>
</div>
<div class="card">
<h3>CSS3</h3>
<p>Selectors + cascade engine</p>
</div>
<div class="card">
<h3>Layout</h3>
<p>taffy flexbox engine</p>
</div>
</div>
</body>
</html>"#;
