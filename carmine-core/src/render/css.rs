use std::collections::HashMap;

use cssparser::{Delimiter, ParseError, Parser as CssParser, ParserInput};
use ego_tree::NodeId;
use scraper::{Html, Node as ScraperNode, Selector};

#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub rules: Vec<CssRule>,
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
    pub specificity: u32,
    pub pseudo: Option<PseudoElement>,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: String,
    pub value: String,
    pub important: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoElement {
    Before,
    After,
}

pub type MatchMap = HashMap<NodeId, Vec<(Declaration, u32)>>;
pub type PseudoMatchMap = HashMap<NodeId, Vec<(Declaration, u32, PseudoElement)>>;

pub fn parse_css(css: &str) -> Stylesheet {
    parse_css_for_viewport(css, f32::INFINITY)
}

pub fn parse_css_for_viewport(css: &str, viewport_width: f32) -> Stylesheet {
    let cleaned = strip_comments(css);
    let filtered = expand_matching_media_rules(&cleaned, viewport_width);
    parse_flat_css(&filtered)
}

fn parse_flat_css(css: &str) -> Stylesheet {
    let mut input = ParserInput::new(css);
    let mut parser = CssParser::new(&mut input);
    let mut rules = Vec::new();
    let mut variables: HashMap<String, String> = HashMap::new();

    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }

        let sel_start = parser.position();
        if parser
            .parse_until_before::<_, _, ()>(Delimiter::CurlyBracketBlock, |p| {
                while p.next().is_ok() {}
                Ok(())
            })
            .is_err()
        {
            break;
        }
        let selector_text = parser.slice_from(sel_start).trim().to_string();

        if parser.expect_curly_bracket_block().is_err() {
            break;
        }

        let block_result: Result<(), ParseError<()>> = parser.parse_nested_block(|p| {
            let (decls, vars) = parse_decl_block_with_vars(p);
            if selector_text.trim() == ":root" {
                for (k, v) in vars {
                    variables.insert(k, v);
                }
            }
            let (selector_text, pseudo) = split_pseudo_selector(&selector_text);
            if let Ok(selector) = Selector::parse(&selector_text) {
                let spec = compute_specificity(&selector_text);
                rules.push(CssRule {
                    selector,
                    declarations: decls,
                    specificity: spec,
                    pseudo,
                });
            }
            Ok(())
        });

        if block_result.is_err() {
            break;
        }
    }

    for rule in &mut rules {
        for decl in &mut rule.declarations {
            decl.value = substitute_vars(&decl.value, &variables);
        }
    }

    Stylesheet { rules, variables }
}

fn expand_matching_media_rules(css: &str, viewport_width: f32) -> String {
    let mut output = String::with_capacity(css.len());
    let mut idx = 0usize;

    while idx < css.len() {
        if css[idx..].starts_with("@media") {
            let condition_start = idx + "@media".len();
            let Some(open_rel) = css[condition_start..].find('{') else {
                break;
            };
            let open = condition_start + open_rel;
            let condition = css[condition_start..open].trim();
            let Some(close) = find_matching_brace(css, open) else {
                break;
            };
            if media_condition_matches(condition, viewport_width) {
                output.push_str(&expand_matching_media_rules(
                    &css[open + 1..close],
                    viewport_width,
                ));
            }
            idx = close + 1;
            continue;
        }

        let Some(ch) = css[idx..].chars().next() else {
            break;
        };
        output.push(ch);
        idx += ch.len_utf8();
    }

    output
}

fn find_matching_brace(css: &str, open: usize) -> Option<usize> {
    let bytes = css.as_bytes();
    let mut depth = 0usize;
    for (idx, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn media_condition_matches(condition: &str, viewport_width: f32) -> bool {
    condition
        .split(',')
        .any(|part| single_media_condition_matches(part.trim(), viewport_width))
}

fn single_media_condition_matches(condition: &str, viewport_width: f32) -> bool {
    let lower = condition.to_ascii_lowercase();
    if lower.contains("not ") || lower.contains("print") {
        return false;
    }
    if lower.contains("screen") || lower.contains("all") || lower.contains("width") {
        for clause in lower.split('(').skip(1) {
            let Some(end) = clause.find(')') else {
                continue;
            };
            let expr = clause[..end].trim();
            let Some((name, value)) = expr.split_once(':') else {
                continue;
            };
            let Some(px) = parse_media_length(value.trim()) else {
                continue;
            };
            match name.trim() {
                "max-width" if viewport_width > px => return false,
                "min-width" if viewport_width < px => return false,
                _ => {}
            }
        }
        return true;
    }
    false
}

fn parse_media_length(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(num) = value.strip_suffix("px") {
        return num.trim().parse().ok();
    }
    if let Some(num) = value
        .strip_suffix("rem")
        .or_else(|| value.strip_suffix("em"))
    {
        return num.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    value.parse().ok()
}

pub fn substitute_vars(value: &str, vars: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    while let Some(start) = result.find("var(") {
        let after = start + 4;
        let rest = &result[after..];
        let mut depth = 1;
        let mut end = 0;
        for (i, b) in rest.bytes().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            break;
        }
        let inner = &rest[..end];
        let (name, fallback) = match inner.find(',') {
            Some(comma) => (inner[..comma].trim(), inner[comma + 1..].trim()),
            None => (inner.trim(), ""),
        };
        let resolved = vars
            .get(name)
            .map(|v| substitute_vars(v, vars))
            .unwrap_or_else(|| fallback.to_string());
        result = format!(
            "{}{}{}",
            &result[..start],
            resolved,
            &result[after + end + 1..]
        );
    }
    result
}

pub fn parse_declarations(text: &str) -> Vec<Declaration> {
    let mut input = ParserInput::new(text);
    let mut parser = CssParser::new(&mut input);
    parse_decl_block(&mut parser)
}

fn parse_decl_block(parser: &mut CssParser) -> Vec<Declaration> {
    let (decls, _) = parse_decl_block_with_vars(parser);
    decls
}

fn parse_decl_block_with_vars(parser: &mut CssParser) -> (Vec<Declaration>, Vec<(String, String)>) {
    let mut declarations = Vec::new();
    let mut variables = Vec::new();

    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }

        let name = match parser.try_parse(|p| p.expect_ident_cloned()) {
            Ok(n) => n,
            Err(_) => {
                let _ = parser.parse_until_after::<_, _, ()>(Delimiter::Semicolon, |p| {
                    while p.next().is_ok() {}
                    Ok(())
                });
                continue;
            }
        };

        let name_str = name.as_ref();
        if name_str.starts_with("--") {
            if parser.expect_colon().is_err() {
                continue;
            }
            let value_start = parser.position();
            let _ = parser.parse_until_before::<_, _, ()>(Delimiter::Semicolon, |p| {
                while p.next().is_ok() {}
                Ok(())
            });
            let value = parser.slice_from(value_start).trim().to_string();
            variables.push((name_str.to_string(), value));
            let _ = parser.next();
            continue;
        }

        if parser.expect_colon().is_err() {
            let _ = parser.parse_until_after::<_, _, ()>(Delimiter::Semicolon, |p| {
                while p.next().is_ok() {}
                Ok(())
            });
            continue;
        }

        let value_start = parser.position();
        let _ = parser.parse_until_before::<_, _, ()>(Delimiter::Semicolon, |p| {
            while p.next().is_ok() {}
            Ok(())
        });
        let raw_value = parser.slice_from(value_start);

        let (value, important) = strip_important(raw_value);
        declarations.push(Declaration {
            property: name.to_lowercase(),
            value,
            important,
        });

        let _ = parser.next();
    }

    (declarations, variables)
}

fn strip_important(value: &str) -> (String, bool) {
    let trimmed = value.trim();
    if let Some(idx) = trimmed.rfind('!') {
        let after = trimmed[idx..].trim();
        if after.eq_ignore_ascii_case("!important") {
            return (trimmed[..idx].trim().to_string(), true);
        }
    }
    (trimmed.to_string(), false)
}

pub fn compute_matches(html: &Html, stylesheet: &Stylesheet) -> MatchMap {
    let mut matches: MatchMap = HashMap::new();

    for rule in &stylesheet.rules {
        if rule.pseudo.is_some() {
            continue;
        }
        for element_ref in html.select(&rule.selector) {
            let id = element_ref.id();
            for decl in &rule.declarations {
                matches
                    .entry(id)
                    .or_default()
                    .push((decl.clone(), rule.specificity));
            }
        }
    }

    matches
}

pub fn compute_pseudo_matches(html: &Html, stylesheet: &Stylesheet) -> PseudoMatchMap {
    let mut matches: PseudoMatchMap = HashMap::new();

    for rule in &stylesheet.rules {
        let Some(pseudo) = rule.pseudo else {
            continue;
        };
        for element_ref in html.select(&rule.selector) {
            let id = element_ref.id();
            for decl in &rule.declarations {
                matches
                    .entry(id)
                    .or_default()
                    .push((decl.clone(), rule.specificity, pseudo));
            }
        }
    }

    matches
}

fn split_pseudo_selector(selector_text: &str) -> (String, Option<PseudoElement>) {
    for (marker, pseudo) in [
        ("::before", PseudoElement::Before),
        ("::after", PseudoElement::After),
        (":before", PseudoElement::Before),
        (":after", PseudoElement::After),
    ] {
        if let Some(idx) = selector_text.find(marker) {
            let base = selector_text[..idx].trim();
            return (
                if base.is_empty() {
                    "*".to_string()
                } else {
                    base.to_string()
                },
                Some(pseudo),
            );
        }
    }

    (selector_text.trim().to_string(), None)
}

fn compute_specificity(selector_text: &str) -> u32 {
    let mut ids = 0u32;
    let mut classes = 0u32;
    let mut types = 0u32;

    for part in selector_text.split_whitespace() {
        for ch in part.chars() {
            match ch {
                '#' => ids += 1,
                '.' => classes += 1,
                _ => {}
            }
        }
        if !part.starts_with('#') && !part.starts_with('.') && part != "*" && !part.contains(':') {
            let tag_part: String = part
                .chars()
                .take_while(|c| *c != '.' && *c != '#' && *c != '[')
                .collect();
            if !tag_part.is_empty() {
                types += 1;
            }
        }
    }

    ids * 256 + classes * 16 + types
}

fn strip_comments(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn extract_style_text(html: &Html) -> String {
    let mut css = String::new();
    for node in html.tree.nodes() {
        if let ScraperNode::Element(el) = node.value() {
            if el.name().eq_ignore_ascii_case("style") {
                for child in node.children() {
                    if let ScraperNode::Text(text) = child.value() {
                        css.push_str(text);
                    }
                }
            }
        }
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_css_for_viewport_expands_matching_width_media() {
        let stylesheet = parse_css_for_viewport(
            ".wide{display:block}@media screen and (max-width: 56rem){.narrow{display:none}}",
            800.0,
        );

        assert_eq!(stylesheet.rules.len(), 2);
        assert!(stylesheet.rules.iter().any(|rule| {
            rule.declarations
                .iter()
                .any(|decl| decl.property == "display" && decl.value == "none")
        }));
    }

    #[test]
    fn parse_css_for_viewport_skips_non_matching_width_media() {
        let stylesheet = parse_css_for_viewport(
            "@media screen and (max-width: 40rem){.narrow{display:none}}",
            800.0,
        );

        assert!(stylesheet.rules.is_empty());
    }
}
