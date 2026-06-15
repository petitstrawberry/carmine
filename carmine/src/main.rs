use std::fs;

use carmine_core::{
    DEFAULT_HTML,
    render::{RenderOptions, RenderPipeline},
};
use clap::Parser;

#[cfg(feature = "scarlet")]
use scarlet_ui::Application;

mod fetch;
mod paint_signal;
mod resolve;

#[cfg(feature = "scarlet")]
mod bridge;
#[cfg(feature = "scarlet")]
mod browser;
#[cfg(feature = "scarlet")]
mod webview;

#[derive(Parser)]
#[command(name = "carmine", about = "A pure-Rust HTML/CSS renderer")]
struct Args {
    file: Option<String>,

    #[arg(long)]
    dump_png: Option<String>,

    #[arg(long, default_value_t = 800)]
    width: u32,

    #[arg(long, default_value_t = 600)]
    height: u32,

    #[arg(long, default_value_t = 0.0)]
    body_margin: f32,
}

fn main() {
    println!("[carmine] starting");

    let args = Args::parse();

    let html = match &args.file {
        Some(path) => {
            let raw = if path.starts_with("http://") || path.starts_with("https://") {
                match fetch::fetch_url(path) {
                    Ok(content) => {
                        println!("[carmine] fetched: {} ({} bytes)", path, content.len());
                        content
                    }
                    Err(e) => {
                        println!("[carmine] fetch failed: {}", e);
                        DEFAULT_HTML.to_string()
                    }
                }
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => {
                        println!("[carmine] loaded: {}", path);
                        content
                    }
                    Err(e) => {
                        println!("[carmine] failed to read {}: {}", path, e);
                        DEFAULT_HTML.to_string()
                    }
                }
            };
            resolve::resolve_external_css(&raw, path)
        }
        None => DEFAULT_HTML.to_string(),
    };

    let render_options = RenderOptions {
        body_margin: args.body_margin,
    };

    if let Some(path) = args.dump_png {
        let mut pipeline =
            RenderPipeline::with_options(&html, args.width, args.height, render_options);
        let pixmap = pipeline.render_owned(args.width, args.height, 1.0);
        match pixmap.save_png(&path) {
            Ok(()) => println!("[carmine] wrote: {}", path),
            Err(e) => println!("[carmine] failed to write {}: {}", path, e),
        }
        return;
    }

    run_viewer(html, args.width, args.height, render_options);
}

#[cfg(feature = "scarlet")]
fn run_viewer(html: String, width: u32, height: u32, render_options: RenderOptions) {
    let mut app = browser::BrowserApp::new(html, width, height, render_options);
    match app.run() {
        Ok(()) => println!("[carmine] exited"),
        Err(e) => println!("[carmine] error: {}", e),
    }
}

#[cfg(not(feature = "scarlet"))]
fn run_viewer(_html: String, _width: u32, _height: u32, _render_options: RenderOptions) {
    eprintln!(
        "[carmine] interactive viewer requires the `scarlet` feature; use --dump-png for headless rendering"
    );
}
