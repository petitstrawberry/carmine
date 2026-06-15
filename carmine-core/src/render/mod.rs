pub mod css;
pub mod layout;
pub mod paint;
pub mod style;

use scraper::Html;
use tiny_skia::Pixmap;

pub struct RenderPipeline {
    html: Html,
    matches: css::MatchMap,
    pseudo_matches: css::PseudoMatchMap,
    options: RenderOptions,
    image_loader: Option<Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>>,

    style_cache: Option<style::StyledNode>,
    layout_cache: Option<layout::LayoutNode>,
    pixmap_cache: Option<Pixmap>,
    cached_key: CacheKey,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct CacheKey {
    logical_w: u32,
    logical_h: u32,
    physical_w: u32,
    physical_h: u32,
    scale_milli: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    pub body_margin: f32,
}

impl RenderPipeline {
    pub fn new(html_text: &str, width: u32, height: u32) -> Self {
        Self::with_options(html_text, width, height, RenderOptions::default())
    }

    pub fn with_options(
        html_text: &str,
        _width: u32,
        _height: u32,
        options: RenderOptions,
    ) -> Self {
        let html = Html::parse_document(html_text);
        let css_text = css::extract_style_text(&html);
        let stylesheet = css::parse_css(&css_text);
        let matches = css::compute_matches(&html, &stylesheet);
        let pseudo_matches = css::compute_pseudo_matches(&html, &stylesheet);
        Self {
            html,
            matches,
            pseudo_matches,
            options,
            image_loader: None,
            style_cache: None,
            layout_cache: None,
            pixmap_cache: None,
            cached_key: CacheKey::default(),
        }
    }

    pub fn set_image_loader<F>(&mut self, loader: F)
    where
        F: Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        self.image_loader = Some(Box::new(loader));
        self.invalidate();
    }

    pub fn content_height(&mut self, physical_w: u32, physical_h: u32, scale: f32) -> f32 {
        self.ensure_pipeline(physical_w, physical_h, scale);
        match &self.layout_cache {
            Some(root) => root.content_bottom(),
            None => physical_h as f32 / scale,
        }
    }

    pub fn render_to_pixmap(&mut self, physical_w: u32, physical_h: u32, scale: f32) -> &Pixmap {
        self.ensure_pipeline(physical_w, physical_h, scale);
        self.pixmap_cache.as_ref().expect("pixmap cache")
    }

    pub fn render_owned(&mut self, physical_w: u32, physical_h: u32, scale: f32) -> Pixmap {
        self.ensure_pipeline(physical_w, physical_h, scale);
        self.pixmap_cache.clone().expect("pixmap cache")
    }

    pub fn invalidate(&mut self) {
        self.style_cache = None;
        self.layout_cache = None;
        self.pixmap_cache = None;
        self.cached_key = CacheKey::default();
    }

    fn ensure_pipeline(&mut self, physical_w: u32, physical_h: u32, scale: f32) {
        let scale_milli = (scale * 1000.0).round() as u32;
        let logical_w = ((physical_w as f32) / scale).round().max(1.0) as u32;
        let logical_h = ((physical_h as f32) / scale).round().max(1.0) as u32;

        let key = CacheKey {
            logical_w,
            logical_h,
            physical_w,
            physical_h,
            scale_milli,
        };

        if key == self.cached_key && self.pixmap_cache.is_some() {
            return;
        }

        if self.style_cache.is_none() || self.cached_key.logical_w != logical_w {
            self.style_cache = Some(style::compute_styles(
                &self.html,
                &self.matches,
                &self.pseudo_matches,
                logical_w,
                self.options.body_margin,
            ));
        }

        if self.layout_cache.is_none()
            || self.cached_key.logical_w != logical_w
            || self.cached_key.logical_h != logical_h
        {
            let styled = self.style_cache.as_ref().expect("style cache");
            self.layout_cache = Some(layout::layout_with_images(
                styled,
                logical_w,
                logical_h,
                self.image_loader.as_ref(),
            ));
        }

        let layout_tree = self.layout_cache.as_ref().expect("layout cache");
        let content_h_logical = layout_tree.content_bottom().ceil().max(logical_h as f32);
        let content_h_physical = ((content_h_logical * scale).ceil() as u32).max(physical_h);
        self.pixmap_cache = Some(paint::paint(
            layout_tree,
            physical_w,
            content_h_physical,
            scale,
        ));

        self.cached_key = key;
    }
}
