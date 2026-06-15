pub mod css;
pub mod layout;
pub mod paint;
pub mod style;

use scraper::Html;
use tiny_skia::Pixmap;

pub struct RenderPipeline {
    html: Html,
    css_text: String,
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

    pub fn with_options(html_text: &str, width: u32, _height: u32, options: RenderOptions) -> Self {
        let html = Html::parse_document(html_text);
        let css_text = css::extract_style_text(&html);
        let stylesheet = css::parse_css_for_viewport(&css_text, width as f32);
        let matches = css::compute_matches(&html, &stylesheet);
        let pseudo_matches = css::compute_pseudo_matches(&html, &stylesheet);
        Self {
            html,
            css_text,
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
            if self.cached_key.logical_w != logical_w {
                let stylesheet = css::parse_css_for_viewport(&self.css_text, logical_w as f32);
                self.matches = css::compute_matches(&self.html, &stylesheet);
                self.pseudo_matches = css::compute_pseudo_matches(&self.html, &stylesheet);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::layout::LayoutNode;

    fn render_layout(html: &str, width: u32, height: u32) -> LayoutNode {
        let mut pipeline = RenderPipeline::new(html, width, height);
        pipeline.ensure_pipeline(width, height, 1.0);
        pipeline.layout_cache.take().expect("layout cache")
    }

    fn collect_by_tag<'a>(node: &'a LayoutNode, tag: &str, out: &mut Vec<&'a LayoutNode>) {
        if node.tag.as_deref() == Some(tag) {
            out.push(node);
        }
        for child in &node.children {
            collect_by_tag(child, tag, out);
        }
    }

    fn rendered_text(node: &LayoutNode) -> String {
        let mut text = String::new();
        if !node.inline_fragments.is_empty() {
            for fragment in &node.inline_fragments {
                text.push_str(&fragment.text);
            }
        } else if let Some(ref node_text) = node.text {
            text.push_str(node_text);
        }
        for child in &node.children {
            text.push_str(&rendered_text(child));
        }
        text
    }

    #[test]
    fn styled_inline_fragments_preserve_strong_and_punctuation() {
        let root = render_layout(
            r#"<!doctype html><style>strong{font-weight:700}</style>
            <div class="callout">Confirmed path: <strong>/var/www/html/index.html</strong>. You are not hitting a host web server.</div>"#,
            800,
            300,
        );
        let mut divs = Vec::new();
        collect_by_tag(&root, "div", &mut divs);
        let callout = divs.first().expect("callout div");

        let text = callout
            .inline_fragments
            .iter()
            .map(|fragment| fragment.text.as_str())
            .collect::<String>();

        assert_eq!(
            text,
            "Confirmed path: /var/www/html/index.html. You are not hitting a host web server."
        );
        assert!(callout.inline_fragments.iter().any(|fragment| {
            fragment.text == "/var/www/html/index.html" && fragment.style.font_weight >= 700
        }));
    }

    #[test]
    fn pseudo_before_and_list_items_survive_layout() {
        let root = render_layout(
            r#"<!doctype html><style>
            .badge{display:inline-flex;gap:10px}.badge::before{content:"";width:10px;height:10px}
            ul{padding-left:18px}.list{line-height:1.6}
            </style>
            <span class="badge">Scarlet OS web stack</span>
            <ul class="list"><li>Architecture: RISC-V 64-bit</li><li>Process: httpd</li></ul>"#,
            800,
            300,
        );

        let mut spans = Vec::new();
        collect_by_tag(&root, "span", &mut spans);
        let badge = spans.first().expect("badge span");
        assert!(badge.pseudo_before.is_some());
        assert_eq!(rendered_text(badge), "Scarlet OS web stack");

        let mut items = Vec::new();
        collect_by_tag(&root, "li", &mut items);
        assert_eq!(items.len(), 2);
        assert_eq!(rendered_text(items[0]), "Architecture: RISC-V 64-bit");
        assert_eq!(rendered_text(items[1]), "Process: httpd");
    }

    #[test]
    fn table_grid_layout_cells_do_not_overlap() {
        let root = render_layout(
            r#"<!doctype html><table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>"#,
            400,
            300,
        );
        let mut tds = Vec::new();
        collect_by_tag(&root, "td", &mut tds);
        assert!(tds.len() >= 4, "expected 4 cells, got {}", tds.len());

        let a = &tds[0];
        let b = &tds[1];
        let c = &tds[2];
        assert!(
            b.x >= a.x + a.width,
            "col1 and col2 should not overlap: a.x={} a.w={} b.x={}",
            a.x,
            a.width,
            b.x
        );
        assert!(
            c.y >= a.y + a.height,
            "row1 and row2 should not overlap: a.y={} a.h={} c.y={}",
            a.y,
            a.height,
            c.y
        );
    }

    #[test]
    fn table_grid_colspan_spans_multiple_columns() {
        let root = render_layout(
            r#"<!doctype html><table>
            <tr><td>a</td><td>b</td><td>c</td></tr>
            <tr><td colspan="2">wide</td><td>d</td></tr>
            </table>"#,
            400,
            300,
        );
        let mut tds = Vec::new();
        collect_by_tag(&root, "td", &mut tds);
        assert!(tds.len() >= 5, "expected 5 cells, got {}", tds.len());
        let wide = &tds[3];
        assert!(
            wide.width > tds[0].width * 1.5,
            "colspan=2 cell should be wider: wide={} col0={}",
            wide.width,
            tds[0].width
        );
    }
}
