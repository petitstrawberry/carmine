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
}

impl RenderPipeline {
    pub fn new(html_text: &str, _width: u32, _height: u32) -> Self {
        let html = Html::parse_document(html_text);
        let css_text = css::extract_style_text(&html);
        let stylesheet = css::parse_css(&css_text);
        let matches = css::compute_matches(&html, &stylesheet);
        let pseudo_matches = css::compute_pseudo_matches(&html, &stylesheet);
        println!(
            "[carmine] CSS: {} rules, {} bytes, {} matched elements",
            stylesheet.rules.len(),
            css_text.len(),
            matches.len()
        );
        for rule in &stylesheet.rules {
            println!(
                "[carmine] rule: {} decls=[{}]",
                rule.specificity,
                rule.declarations
                    .iter()
                    .map(|d| format!("{}:{}", d.property, &d.value[..d.value.len().min(30)]))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Self {
            html,
            matches,
            pseudo_matches,
        }
    }

    pub fn render(&self, physical_w: u32, physical_h: u32, scale: f32) -> Pixmap {
        let logical_w = ((physical_w as f32) / scale).round().max(1.0) as u32;
        let logical_h = ((physical_h as f32) / scale).round().max(1.0) as u32;
        let styled =
            style::compute_styles(&self.html, &self.matches, &self.pseudo_matches, logical_w);
        let layout_tree = layout::layout(&styled, logical_w, logical_h);
        paint::paint(&layout_tree, physical_w, physical_h, scale)
    }
}
