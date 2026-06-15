use ego_tree::NodeRef;
use scraper::{Html, Node as ScraperNode};

pub fn resolve_external_css(html_text: &str, base_path: &str) -> String {
    let html_text = resolve_frames(html_text, base_path, 0);
    let document = Html::parse_document(&html_text);
    let mut extra_css = String::new();
    let mut hrefs: Vec<String> = Vec::new();

    let root = document.tree.root();
    collect_link_hrefs(&root, &mut hrefs);
    drop(document);

    for href in &hrefs {
        let resolved = resolve_href(href, base_path);
        match fetch_css(&resolved) {
            Ok(css) => {
                println!("[carmine] external CSS: {} ({} bytes)", resolved, css.len());
                extra_css.push_str(&css);
                extra_css.push('\n');
            }
            Err(e) => {
                println!("[carmine] external CSS failed: {} : {}", resolved, e);
            }
        }
    }

    if extra_css.is_empty() {
        return html_text.to_string();
    }

    let style_tag = format!("<style>\n{}\n</style>", extra_css);
    if let Some(pos) = html_text.find("</head>") {
        let mut result = String::with_capacity(html_text.len() + style_tag.len());
        result.push_str(&html_text[..pos]);
        result.push_str(&style_tag);
        result.push_str(&html_text[pos..]);
        result
    } else {
        format!("{}{}", style_tag, html_text)
    }
}

fn collect_link_hrefs(node: &NodeRef<ScraperNode>, out: &mut Vec<String>) {
    if let ScraperNode::Element(el) = node.value() {
        if el.name().eq_ignore_ascii_case("link") {
            let rel = el.attr("rel").unwrap_or("");
            if rel.eq_ignore_ascii_case("stylesheet") {
                if let Some(href) = el.attr("href") {
                    out.push(href.to_string());
                }
            }
        }
    }
    for child in node.children() {
        collect_link_hrefs(&child, out);
    }
}

fn resolve_frames(html_text: &str, base_path: &str, depth: usize) -> String {
    if depth > 3 {
        return html_text.to_string();
    }

    let document = Html::parse_document(html_text);
    let root = document.tree.root();
    let mut framesets = Vec::new();
    collect_framesets(&root, &mut framesets);
    if let Some(frameset) = framesets.first() {
        return expand_frameset(frameset, base_path, depth);
    }

    let mut iframes = Vec::new();
    collect_iframe_srcs(&root, &mut iframes);
    if iframes.is_empty() {
        return html_text.to_string();
    }

    let mut expanded = html_text.to_string();
    for src in iframes {
        let resolved = resolve_href(&src, base_path);
        if let Ok(child) = fetch_html(&resolved) {
            let child =
                resolve_external_css(&resolve_frames(&child, &resolved, depth + 1), &resolved);
            let replacement = format!(
                "<div class=\"carmine-iframe\" style=\"display:block;border:2px inset #ddd;overflow:auto;\">{}</div>",
                frame_document_markup(&child)
            );
            expanded = replace_first_iframe(&expanded, &replacement).unwrap_or(expanded);
        }
    }
    expanded
}

#[derive(Clone)]
struct FramesetSpec {
    cols: Option<String>,
    rows: Option<String>,
    frames: Vec<String>,
}

fn collect_framesets(node: &NodeRef<ScraperNode>, out: &mut Vec<FramesetSpec>) {
    if let ScraperNode::Element(el) = node.value() {
        if el.name().eq_ignore_ascii_case("frameset") {
            let mut frames = Vec::new();
            collect_frame_srcs(node, &mut frames);
            out.push(FramesetSpec {
                cols: el.attr("cols").map(|s| s.to_string()),
                rows: el.attr("rows").map(|s| s.to_string()),
                frames,
            });
            return;
        }
    }
    for child in node.children() {
        collect_framesets(&child, out);
    }
}

fn collect_frame_srcs(node: &NodeRef<ScraperNode>, out: &mut Vec<String>) {
    if let ScraperNode::Element(el) = node.value() {
        if el.name().eq_ignore_ascii_case("frame") {
            if let Some(src) = el.attr("src") {
                out.push(src.to_string());
            }
        }
    }
    for child in node.children() {
        collect_frame_srcs(&child, out);
    }
}

fn collect_iframe_srcs(node: &NodeRef<ScraperNode>, out: &mut Vec<String>) {
    if let ScraperNode::Element(el) = node.value() {
        if el.name().eq_ignore_ascii_case("iframe") {
            if let Some(src) = el.attr("src") {
                out.push(src.to_string());
            }
        }
    }
    for child in node.children() {
        collect_iframe_srcs(&child, out);
    }
}

fn expand_frameset(frameset: &FramesetSpec, base_path: &str, depth: usize) -> String {
    let vertical = frameset.cols.is_some();
    let sizes = parse_frame_sizes(
        frameset
            .cols
            .as_deref()
            .or(frameset.rows.as_deref())
            .unwrap_or("*"),
        frameset.frames.len(),
    );

    let direction = if vertical { "row" } else { "column" };
    let mut body = format!(
        "<div class=\"carmine-frameset\" style=\"display:flex;flex-direction:{};width:100vw;height:100vh;margin:0;\">",
        direction
    );
    for (idx, src) in frameset.frames.iter().enumerate() {
        let resolved = resolve_href(src, base_path);
        let child = fetch_html(&resolved)
            .map(|html| {
                resolve_external_css(&resolve_frames(&html, &resolved, depth + 1), &resolved)
            })
            .unwrap_or_else(|err| format!("<p>Frame load failed: {} ({})</p>", resolved, err));
        let basis = sizes
            .get(idx)
            .copied()
            .unwrap_or(1.0 / frameset.frames.len().max(1) as f32);
        body.push_str(&format!(
            "<div class=\"carmine-frame\" style=\"flex:0 0 {:.4}%;height:100%;overflow:auto;border-right:1px solid #888;\">{}</div>",
            basis * 100.0,
            frame_document_markup(&child)
        ));
    }
    body.push_str("</div>");

    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;width:100%;height:100%;}}</style></head><body>{}</body></html>",
        body
    )
}

fn parse_frame_sizes(spec: &str, count: usize) -> Vec<f32> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let mut fixed = Vec::new();
    let mut star_count = 0usize;
    for part in &parts {
        if part.ends_with('*') || *part == "" {
            fixed.push(None);
            star_count += 1;
        } else if let Ok(percent) = part.trim_end_matches('%').parse::<f32>() {
            fixed.push(Some(percent / 100.0));
        } else {
            fixed.push(None);
            star_count += 1;
        }
    }
    while fixed.len() < count {
        fixed.push(None);
        star_count += 1;
    }

    let used: f32 = fixed.iter().flatten().sum();
    let star = if star_count > 0 {
        ((1.0 - used).max(0.0)) / star_count as f32
    } else {
        0.0
    };
    fixed
        .into_iter()
        .take(count)
        .map(|value| value.unwrap_or(star))
        .collect()
}

fn extract_body_like(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let (Some(start), Some(end)) = (lower.find("<body"), lower.rfind("</body>")) {
        if let Some(open_end) = html[start..].find('>') {
            return html[start + open_end + 1..end].to_string();
        }
    }
    html.to_string()
}

fn frame_document_markup(html: &str) -> String {
    let attrs = body_presentational_attrs(html);
    format!(
        "<div class=\"carmine-frame-document\"{}>{}</div>",
        attrs,
        extract_body_like(html)
    )
}

fn body_presentational_attrs(html: &str) -> String {
    let document = Html::parse_document(html);
    let root = document.tree.root();
    collect_body_presentational_attrs(&root).unwrap_or_default()
}

fn collect_body_presentational_attrs(node: &NodeRef<ScraperNode>) -> Option<String> {
    if let ScraperNode::Element(el) = node.value() {
        if el.name().eq_ignore_ascii_case("body") {
            let mut attrs = String::new();
            if let Some(background) = el.attr("background") {
                attrs.push_str(" background=\"");
                attrs.push_str(&escape_html_attr(background));
                attrs.push('"');
            }
            if let Some(bgcolor) = el.attr("bgcolor") {
                attrs.push_str(" bgcolor=\"");
                attrs.push_str(&escape_html_attr(bgcolor));
                attrs.push('"');
            }
            return Some(attrs);
        }
    }
    for child in node.children() {
        if let Some(attrs) = collect_body_presentational_attrs(&child) {
            return Some(attrs);
        }
    }
    None
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn replace_first_iframe(html: &str, replacement: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<iframe")?;
    let after_start = &lower[start..];
    let end = if let Some(close) = after_start.find("</iframe>") {
        start + close + "</iframe>".len()
    } else {
        start + after_start.find('>')? + 1
    };
    Some(format!("{}{}{}", &html[..start], replacement, &html[end..]))
}

fn resolve_href(href: &str, base_path: &str) -> String {
    resolve_href_public(href, base_path)
}

pub fn resolve_href_public(href: &str, base_path: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    if base_path.starts_with("http://") || base_path.starts_with("https://") {
        if let Ok(base) = url::Url::parse(base_path) {
            if let Ok(resolved) = base.join(href) {
                return resolved.to_string();
            }
        }
    }

    if href.starts_with('/') {
        return href.to_string();
    }

    if let Some(slash) = base_path.rfind('/') {
        format!("{}/{}", &base_path[..slash], href)
    } else {
        href.to_string()
    }
}

fn fetch_css(path: &str) -> Result<String, String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        crate::fetch::fetch_url(path)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{}", e))
    }
}

fn fetch_html(path: &str) -> Result<String, String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        crate::fetch::fetch_url(path)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{}", e))
    }
}
