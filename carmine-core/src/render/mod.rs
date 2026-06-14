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
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    pub body_margin: f32,
}

impl RenderPipeline {
    pub fn new(html_text: &str, _width: u32, _height: u32) -> Self {
        Self::with_options(html_text, _width, _height, RenderOptions::default())
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
        }
    }

    pub fn render(&self, physical_w: u32, physical_h: u32, scale: f32) -> Pixmap {
        let logical_w = ((physical_w as f32) / scale).round().max(1.0) as u32;
        let logical_h = ((physical_h as f32) / scale).round().max(1.0) as u32;
        let styled = style::compute_styles(
            &self.html,
            &self.matches,
            &self.pseudo_matches,
            logical_w,
            self.options.body_margin,
        );
        let layout_tree = layout::layout(&styled, logical_w, logical_h);
        paint::paint(&layout_tree, physical_w, physical_h, scale)
    }
}
