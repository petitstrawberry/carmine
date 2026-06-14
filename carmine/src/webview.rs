use std::rc::Rc;

use scarlet_ui::graphics;
use scarlet_ui::prelude::*;
use scarlet_ui::{CanvasView, ComponentElement};

use carmine_core::render::{RenderOptions, RenderPipeline};

use crate::bridge;

pub struct WebView {
    pipeline: Rc<RenderPipeline>,
    width: f32,
    height: f32,
}

impl WebView {
    pub fn new(html: &str, width: u32, height: u32, options: RenderOptions) -> Self {
        let pipeline = RenderPipeline::with_options(html, width, height, options);
        Self {
            pipeline: Rc::new(pipeline),
            width: width as f32,
            height: height as f32,
        }
    }
}

impl Clone for WebView {
    fn clone(&self) -> Self {
        Self {
            pipeline: Rc::clone(&self.pipeline),
            width: self.width,
            height: self.height,
        }
    }
}

impl View for WebView {
    fn create_element(&self) -> Box<dyn Element> {
        let pipeline = Rc::clone(&self.pipeline);
        CanvasView::new(
            self.width,
            self.height,
            Rc::new(move |buffer, width, height| {
                let scale = graphics::current_scale_milli().max(1) as f32 / 1000.0;
                let pixmap = pipeline.render(width, height, scale);
                bridge::pixmap_to_canvas_bgra(&pixmap, buffer, width, height);
            }),
        )
        .create_element()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
