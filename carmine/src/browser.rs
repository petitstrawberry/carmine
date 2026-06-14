use scarlet_ui::prelude::*;
use scarlet_ui::{Application, ComponentElement, Size, Window};

use carmine_core::render::RenderOptions;

use crate::webview::WebView;

pub struct BrowserApp {
    html: String,
    width: u32,
    height: u32,
    options: RenderOptions,
}

impl BrowserApp {
    pub fn new(html: String, width: u32, height: u32, options: RenderOptions) -> Self {
        Self { html, width, height, options }
    }
}

impl Clone for BrowserApp {
    fn clone(&self) -> Self {
        Self {
            html: self.html.clone(),
            width: self.width,
            height: self.height,
            options: self.options,
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
        Window::new(
            "Carmine",
            WebView::new(&self.html, self.width, self.height, self.options),
        )
        .app_id("org.scarlet-os.carmine")
        .size(Size::new(self.width as f32, self.height as f32))
    }

    fn debug_logging(&self) -> bool {
        false
    }
}
