use ego_tree::NodeRef;
use scraper::{Html, Node as ScraperNode};

pub fn resolve_external_css(html_text: &str, base_path: &str) -> String {
    let document = Html::parse_document(html_text);
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
