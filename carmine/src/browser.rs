use std::rc::Rc;

use scarlet_ui::prelude::*;
use scarlet_ui::{Application, ComponentElement, Listenable, Size, Window};

use carmine_core::render::RenderOptions;

use crate::paint_signal::PaintSignal;
use crate::webview::WebView;

pub struct BrowserApp {
    html: String,
    width: u32,
    height: u32,
    options: RenderOptions,
    paint_signal: Rc<PaintSignal>,
}

impl BrowserApp {
    pub fn new(html: String, width: u32, height: u32, options: RenderOptions) -> Self {
        Self {
            html,
            width,
            height,
            options,
            paint_signal: Rc::new(PaintSignal::new()),
        }
    }
}

impl Clone for BrowserApp {
    fn clone(&self) -> Self {
        Self {
            html: self.html.clone(),
            width: self.width,
            height: self.height,
            options: self.options,
            paint_signal: Rc::clone(&self.paint_signal),
        }
    }
}

impl View for BrowserApp {
    fn create_element(&self) -> Box<dyn Element> {
        Box::new(ComponentElement::new(self.clone()))
    }

    fn listenables(&self) -> Vec<&dyn Listenable> {
        vec![self.paint_signal.as_ref()]
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Application for BrowserApp {
    fn body(&self) -> impl View {
        let webview = WebView::new(
            &self.html,
            self.width,
            self.height,
            self.options,
            Rc::clone(&self.paint_signal),
        );
        Window::new("Carmine", webview)
            .app_id("org.scarlet-os.carmine")
            .size(Size::new(self.width as f32, self.height as f32))
    }

    fn debug_logging(&self) -> bool {
        false
    }
}
