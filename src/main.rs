use std::fs;

use carmine::{DEFAULT_HTML, render::RenderPipeline};
use clap::Parser;

#[cfg(feature = "scarlet")]
use scarlet_ui::Application;

#[cfg(feature = "scarlet")]
mod bridge;
#[cfg(feature = "scarlet")]
mod browser;

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
        let pipeline = RenderPipeline::new(&html, args.width, args.height);
        let pixmap = pipeline.render(args.width, args.height, 1.0);
        match pixmap.save_png(&path) {
            Ok(()) => println!("[carmine] wrote: {}", path),
            Err(e) => println!("[carmine] failed to write {}: {}", path, e),
        }
        return;
    }

    run_viewer(html, args.width, args.height);
}

#[cfg(feature = "scarlet")]
fn run_viewer(html: String, width: u32, height: u32) {
    let mut app = browser::BrowserApp::new(html, width, height);
    match app.run() {
        Ok(()) => println!("[carmine] exited"),
        Err(e) => println!("[carmine] error: {}", e),
    }
}

#[cfg(not(feature = "scarlet"))]
fn run_viewer(_html: String, _width: u32, _height: u32) {
    eprintln!(
        "[carmine] interactive viewer requires the `scarlet` feature; use --dump-png for headless rendering"
    );
}
