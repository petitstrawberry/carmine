use std::fs;

use clap::Parser;
use scarlet_ui::Application;

mod bridge;
mod browser;
mod render;

const DEFAULT_HTML: &str = r#"<!DOCTYPE html>
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
<p>A pure-Rust web browser for Scarlet OS.</p>
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

#[derive(Parser)]
#[command(name = "carmine", about = "A pure-Rust web browser for Scarlet OS")]
struct Args {
    file: Option<String>,

    #[arg(long)]
    dump_png: Option<String>,

    #[arg(long, default_value_t = 800)]
    width: u32,

    #[arg(long, default_value_t = 600)]
    height: u32,
}

fn main() {
    println!("[carmine] starting");

    let args = Args::parse();

    let html = match &args.file {
        Some(path) => match fs::read_to_string(path) {
            Ok(content) => {
                println!("[carmine] loaded: {}", path);
                content
            }
            Err(e) => {
                println!("[carmine] failed to read {}: {}", path, e);
                DEFAULT_HTML.to_string()
            }
        },
        None => DEFAULT_HTML.to_string(),
    };

    if let Some(path) = args.dump_png {
        let pipeline = render::RenderPipeline::new(&html, args.width, args.height);
        let pixmap = pipeline.render(args.width, args.height, 1.0);
        match pixmap.save_png(&path) {
            Ok(()) => println!("[carmine] wrote: {}", path),
            Err(e) => println!("[carmine] failed to write {}: {}", path, e),
        }
        return;
    }

    let mut app = browser::BrowserApp::new(html, args.width, args.height);
    match app.run() {
        Ok(()) => println!("[carmine] exited"),
        Err(e) => println!("[carmine] error: {}", e),
    }
}
