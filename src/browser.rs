use std::rc::Rc;

use scarlet_ui::graphics;
use scarlet_ui::prelude::*;
use scarlet_ui::{Application, CanvasView, ComponentElement, Size, Window};

use crate::bridge;
use crate::render::RenderPipeline;

pub struct BrowserApp {
    pipeline: Rc<RenderPipeline>,
    width: f32,
    height: f32,
}

impl BrowserApp {
    pub fn new(html: String, width: u32, height: u32) -> Self {
        let pipeline = RenderPipeline::new(&html, width, height);
        Self {
            pipeline: Rc::new(pipeline),
            width: width as f32,
            height: height as f32,
        }
    }
}

impl Clone for BrowserApp {
    fn clone(&self) -> Self {
        Self {
            pipeline: Rc::clone(&self.pipeline),
            width: self.width,
            height: self.height,
        }
    }
}

impl View for BrowserApp {
    fn create_element(&self) -> Box<dyn Element> {
        Box::new(ComponentElement::new(self.clone()))
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Application for BrowserApp {
    fn body(&self) -> impl View {
        let pipeline = Rc::clone(&self.pipeline);

        Window::new(
            "Carmine",
            CanvasView::new(
                self.width,
                self.height,
                Rc::new(move |buffer, width, height| {
                    let scale = graphics::current_scale_milli().max(1) as f32 / 1000.0;
                    let pixmap = pipeline.render(width, height, scale);
                    bridge::pixmap_to_canvas_bgra(&pixmap, buffer, width, height);
                }),
            ),
        )
        .app_id("org.scarlet-os.carmine")
        .size(Size::new(self.width, self.height))
    }

    fn debug_logging(&self) -> bool {
        false
    }
}
